use std::str::FromStr;
use std::{collections::HashSet, time::Duration};

use anyhow::{Context as _, Result};
use gst::{prelude::*, Structure};
use gstreamer as gst;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;
use realsense_rust as rs;
use rs::{
    frame::{ColorFrame, DepthFrame},
    kind::{Rs2CameraInfo, Rs2Format, Rs2ProductLine, Rs2StreamKind},
    pipeline::InactivePipeline,
};

const WIDTH: u32 = 640;
const HEIGHT: u32 = 480;
const FRAMERATE: u32 = 60;

const DEPTH_MIN: u16 = 280;
const DEPTH_MAX: u16 = 6000;

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let i = (h * 6.0).floor() as i32;
    let f = h * 6.0 - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);

    let (r, g, b) = match i % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        5 => (v, p, q),
        _ => unreachable!(),
    };

    // RGB値を8ビット整数に変換します。
    let r = (r * 255.0).round() as u8;
    let g = (g * 255.0).round() as u8;
    let b = (b * 255.0).round() as u8;

    (r, g, b)
}

fn colorize_depth(depth: u16) -> (u8, u8, u8) {
    if !(DEPTH_MIN..DEPTH_MAX).contains(&depth) {
        return (0, 0, 0);
    }
    let normalized_depth = ((depth - DEPTH_MIN) as f32) / (DEPTH_MAX - DEPTH_MIN) as f32;
    // println!("{normalized_depth:}");
    hsv_to_rgb(normalized_depth, 1.0, 1.0)
}

struct Rs2AppSrc {
    app_src: gst_app::AppSrc,
}

impl Rs2AppSrc {
    fn new() -> Result<Self> {
        let mut queried_device = HashSet::new();
        queried_device.insert(Rs2ProductLine::D400);
        let context = rs::context::Context::new()?;
        let devices = context.query_devices(queried_device);
        let device = devices.iter().next().context("No realsense camera found")?;
        let pipeline = InactivePipeline::try_from(&context)?;
        let mut config = rs::config::Config::new();
        config
            .enable_device_from_serial(device.info(Rs2CameraInfo::SerialNumber).unwrap())?
            .disable_all_streams()?
            .enable_stream(
                Rs2StreamKind::Depth,
                None,
                WIDTH as _,
                HEIGHT as _,
                Rs2Format::Z16,
                FRAMERATE as _,
            )?
            .enable_stream(
                Rs2StreamKind::Color,
                None,
                WIDTH as _,
                HEIGHT as _,
                Rs2Format::Rgb8,
                FRAMERATE as _,
            )?;
        let video_info =
            gst_video::VideoInfo::builder(gst_video::VideoFormat::Rgb, WIDTH, HEIGHT * 2)
                .fps(gst::Fraction::new(FRAMERATE as _, 1))
                .build()?;
        let mut pipeline = pipeline.start(Some(config))?;

        for profile in pipeline.profile().streams() {
            println!(
                "{:?} stream intrinsics: {:?}",
                profile.kind(),
                profile.intrinsics()?
            );
        }

        let app_src = gst_app::AppSrc::builder()
            .caps(&video_info.to_caps()?)
            .callbacks(
                gst_app::AppSrcCallbacks::builder()
                    .need_data(move |app_src, _| {
                        match pipeline.wait(Some(Duration::from_millis(500))) {
                            Ok(frames) => {
                                let depth_frames = frames.frames_of_type::<DepthFrame>();
                                let color_frames = frames.frames_of_type::<ColorFrame>();
                                let (Some(depth_frame), Some(color_frame)) =
                                    (depth_frames.first(), color_frames.first())
                                else {
                                    return;
                                };
                                let mut buffer = gst::Buffer::with_size(video_info.size()).unwrap();
                                {
                                    let buf = buffer.get_mut().unwrap();
                                    let mut vframe =
                                        gst_video::VideoFrameRef::from_buffer_ref_writable(
                                            buf,
                                            &video_info,
                                        )
                                        .unwrap();

                                    let plane_data = vframe.plane_data_mut(0).unwrap();
                                    let color_frame_data = unsafe {
                                        std::slice::from_raw_parts::<u8>(
                                            std::mem::transmute(color_frame.get_data()),
                                            color_frame.get_data_size(),
                                        )
                                    };
                                    let depth_frame_data = unsafe {
                                        std::slice::from_raw_parts::<u8>(
                                            std::mem::transmute(depth_frame.get_data()),
                                            depth_frame.get_data_size(),
                                        )
                                    };
                                    plane_data[..color_frame_data.len()]
                                        .copy_from_slice(color_frame_data);
                                    for (dst, raw_depth) in plane_data[color_frame_data.len()..]
                                        .chunks_exact_mut(3)
                                        .zip(depth_frame_data.chunks_exact(2))
                                    {
                                        let depth =
                                            (raw_depth[0] as u16) | ((raw_depth[1] as u16) << 8);
                                        let rgb = colorize_depth(depth);
                                        dst[0] = rgb.0;
                                        dst[1] = rgb.1;
                                        dst[2] = rgb.2;
                                    }
                                }
                                app_src.push_buffer(buffer).unwrap();
                            }
                            Err(_) => {
                                app_src.end_of_stream().unwrap();
                            }
                        }
                    })
                    .build(),
            )
            .do_timestamp(true)
            .format(gst::Format::Time)
            .build();

        Ok(Self { app_src })
    }

    fn element(&self) -> gst::Element {
        self.app_src.clone().dynamic_cast::<gst::Element>().unwrap()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    gst::init()?;
    gst::log::set_active(true);
    gst::log::set_default_threshold(gst::DebugLevel::Info);

    let src: Rs2AppSrc = Rs2AppSrc::new()?;
    let videoconvert = gst::ElementFactory::make("videoconvert").build()?;
    let capsfilter = gst::ElementFactory::make("capsfilter")
        .property(
            "caps",
            gst::Caps::from_str("video/x-raw,format=RGBx").unwrap(),
        )
        .build()?;
    let vapostproc = gst::ElementFactory::make("vapostproc").build()?;
    let vah264enc = gst::ElementFactory::make("vah264enc")
        .property("bitrate", 10_000u32)
        .build()?;
    let rtph264pay = gst::ElementFactory::make("rtph264pay").build()?;
    let udpsink = gst::ElementFactory::make("udpsink")
        .property("sync", false)
        .property("host", "127.0.0.1")
        .property("port", 5000)
        .build()?;
    let pipeline = gst::Pipeline::default();
    pipeline.add_many([
        &src.element(),
        &videoconvert,
        &capsfilter,
        &vapostproc,
        &vah264enc,
        &rtph264pay,
        &udpsink,
    ])?;
    gst::Element::link_many([
        &src.element(),
        &videoconvert,
        &capsfilter,
        &vapostproc,
        &vah264enc,
        &rtph264pay,
        &udpsink,
    ])?;

    tokio::spawn(async move {
        let mut prev_bytes_served = 0;
        loop {
            let bytes_served: u64 = udpsink.property("bytes-served");
            println!(
                "bytes served: {} kB/s",
                (bytes_served - prev_bytes_served) as f64 / 1000.0
            );
            prev_bytes_served = bytes_served;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    let bus = pipeline.bus().unwrap();
    pipeline.set_state(gst::State::Playing)?;

    for msg in bus.iter_timed(gst::ClockTime::NONE) {
        use gst::MessageView;

        match msg.view() {
            MessageView::Eos(..) => break,
            MessageView::Error(err) => {
                pipeline.set_state(gst::State::Null)?;
                println!("err: {:?}", err);
                break;
            }
            _ => (),
        }
    }

    pipeline.set_state(gst::State::Null)?;

    Ok(())
}
