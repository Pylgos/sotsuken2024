use anyhow::Result;
use gst::glib::prelude::*;
use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;

pub struct VideoFrame {
    info: gst_video::VideoInfo,
    buffer: gst::Buffer,
}

impl VideoFrame {
    pub fn copy_to(&self, dst: &mut [u8]) -> Result<()> {
        let map = self.buffer.map_readable()?;
        dst.copy_from_slice(map.as_slice());
        Ok(())
    }

    pub fn width(&self) -> u32 {
        self.info.width()
    }
    pub fn height(&self) -> u32 {
        self.info.height()
    }
}

pub struct VideoDecoder {
    bin: gst::Bin,
    receiver: tokio::sync::mpsc::Receiver<VideoFrame>,
}

impl VideoDecoder {
    pub fn new() -> Result<Self> {
        let bin = gst::Bin::with_name("decoderbin");
        let decoder = gst::ElementFactory::make("vah264dec").build()?;
        let videoconvert = gst::ElementFactory::make("videoconvert").build()?;
        let appsink = gst_app::AppSink::builder()
            .caps(
                &gst_video::VideoCapsBuilder::new()
                    .format(gst_video::VideoFormat::Rgba)
                    .build(),
            )
            .sync(false)
            .build();

        bin.add_many([&decoder, &videoconvert, appsink.as_ref()])?;

        gst::Element::link_many([&decoder, &videoconvert, appsink.as_ref()])?;

        let src_pad = gst::GhostPad::with_target(&decoder.static_pad("sink").unwrap())?;
        bin.add_pad(&src_pad)?;

        let (sender, receiver) = tokio::sync::mpsc::channel::<VideoFrame>(10);

        appsink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |appsink| {
                    let sample = appsink.pull_sample().map_err(|_| gst::FlowError::Eos)?;

                    let info = sample
                        .caps()
                        .and_then(|caps| gst_video::VideoInfo::from_caps(caps).ok())
                        .ok_or_else(|| {
                            gst::element_error!(
                                appsink,
                                gst::ResourceError::Failed,
                                ("Failed to get video info from sample")
                            );

                            gst::FlowError::NotNegotiated
                        })?;

                    let buffer = sample.buffer_owned().unwrap();
                    let _ = sender.try_send(VideoFrame { info, buffer });
                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );

        Ok(VideoDecoder { bin, receiver })
    }

    pub fn element(&self) -> &gst::Element {
        self.bin.upcast_ref()
    }

    pub async fn close(self) -> Result<()> {
        println!("Decoder closed");
        Ok(())
    }

    pub fn try_recv(&mut self) -> Result<VideoFrame, tokio::sync::mpsc::error::TryRecvError> {
        self.receiver.try_recv()
    }
}
