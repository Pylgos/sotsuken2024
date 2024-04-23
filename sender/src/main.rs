use std::str::FromStr;
use std::{collections::HashSet, time::Duration};

use anyhow::{Context as _, Result};
// use realsense_rust::{
//     config::Config,
//     context::Context,
//     frame::{ColorFrame, DepthFrame},
//     kind::{Rs2CameraInfo, Rs2Format, Rs2ProductLine, Rs2StreamKind},
//     pipeline::InactivePipeline,
// };
use gst::glib;
use gst::prelude::*;
use gst_video::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;
use realsense_rust as rs;
use rs::frame::FrameEx;
use rs::{
    frame::{ColorFrame, DepthFrame},
    kind::{Rs2CameraInfo, Rs2Format, Rs2ProductLine, Rs2StreamKind},
    pipeline::{ActivePipeline, InactivePipeline},
};
use tokio::task::JoinHandle;

const WIDTH: u32 = 640;
const HEIGHT: u32 = 480;
const FRAMERATE: u32 = 60;

struct Rs2AppSrc {
    app_src: gst_app::AppSrc,
}

impl Rs2AppSrc {
    fn new() -> Result<Self> {
        let mut queried_device = HashSet::new();
        queried_device.insert(Rs2ProductLine::D400);
        let context = rs::context::Context::new()?;
        let devices = context.query_devices(queried_device);
        let pipeline = InactivePipeline::try_from(&context)?;
        let mut config = rs::config::Config::new();
        config
            .enable_device_from_serial(devices[0].info(Rs2CameraInfo::SerialNumber).unwrap())?
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
        let packed_depth_height = (HEIGHT as f64 / 1.5).ceil() as u32;
        let video_info = gst_video::VideoInfo::builder(gst_video::VideoFormat::Rgb, WIDTH, HEIGHT + packed_depth_height)
            .fps(gst::Fraction::new(FRAMERATE as _, 1))
            .build()?;
        let mut pipeline = pipeline.start(Some(config))?;

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
                                    plane_data[..color_frame_data.len()].copy_from_slice(color_frame_data);
                                    plane_data[color_frame_data.len()..][..depth_frame_data.len()].copy_from_slice(depth_frame_data);
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
    let videoconvert1 = gst::ElementFactory::make("videoconvert").build()?;
    let capsfilter = gst::ElementFactory::make("capsfilter").property("caps", gst::Caps::from_str("video/x-raw,format=RGBx").unwrap()).build()?;
    let vapostproc = gst::ElementFactory::make("vapostproc").build()?;
    let vah264enc = gst::ElementFactory::make("vah264enc").build()?;
    let queue = gst::ElementFactory::make("queue").build()?;
    let vah264dec = gst::ElementFactory::make("vah264dec").build()?;
    let videoconvert2 = gst::ElementFactory::make("videoconvert").build()?;
    let autovideosink = gst::ElementFactory::make("autovideosink")
        .property("sync", false)
        .build()?;
    let pipeline = gst::Pipeline::default();
    pipeline.add_many([&src.element(), &videoconvert1, &capsfilter, &vapostproc, &vah264enc, &queue, &vah264dec, &videoconvert2, &autovideosink])?;
    gst::Element::link_many([&src.element(), &videoconvert1, &capsfilter, &vapostproc, &vah264enc, &queue, &vah264dec, &videoconvert2, &autovideosink])?;

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
