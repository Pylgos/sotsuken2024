use anyhow::Result;
use gst::glib::prelude::*;
use gst::prelude::*;
use gst_gl::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;
use gstreamer_gl as gst_gl;
use gstreamer_video as gst_video;

use super::texture_copy;

pub struct VideoFrame {
    info: gst_video::VideoInfo,
    buffer: gst::Buffer,
}

impl VideoFrame {
    pub fn copy_to_texture2d(&self, context: &gst_gl::GLContext, dst: gl::types::GLuint) {
        let video_frame = gst_gl::gl_video_frame::GLVideoFrame::from_buffer_readable(
            self.buffer.clone(),
            &self.info,
        )
        .unwrap();
        let sync_meta = video_frame.buffer().meta::<gst_gl::GLSyncMeta>().unwrap();
        sync_meta.wait(context);
        texture_copy::copy_texture(
            video_frame.texture_id(0).unwrap() as _,
            dst,
            self.info.width() as _,
            self.info.height() as _,
        );
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
        let glupload = gst::ElementFactory::make("glupload").build()?;
        let glcolorconvert = gst::ElementFactory::make("glcolorconvert").build()?;
        let appsink = gst_app::AppSink::builder()
            .caps(
                &gst_video::VideoCapsBuilder::new()
                    .features([gst_gl::CAPS_FEATURE_MEMORY_GL_MEMORY])
                    .format(gst_video::VideoFormat::Rgba)
                    .build(),
            )
            .sync(false)
            // .max_buffers(1)
            // .qos(false)
            // .drop(true)
            // .max_time(Some(gst::ClockTime::from_mseconds(100)))
            .build();

        bin.add_many([&decoder, &glupload, &glcolorconvert, appsink.as_ref()])?;

        gst::Element::link_many([&decoder, &glupload, &glcolorconvert, appsink.as_ref()])?;

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

                    let mut buffer = sample.buffer_owned().unwrap();
                    {
                        let context = match (buffer.n_memory() > 0)
                            .then(|| buffer.peek_memory(0))
                            .and_then(|m| m.downcast_memory_ref::<gst_gl::GLBaseMemory>())
                            .map(|m| m.context())
                        {
                            Some(context) => context.clone(),
                            None => {
                                gst::element_error!(
                                    appsink,
                                    gst::ResourceError::Failed,
                                    ("Failed to get GL context from buffer")
                                );

                                return Err(gst::FlowError::Error);
                            }
                        };

                        if let Some(meta) = buffer.meta::<gst_gl::GLSyncMeta>() {
                            meta.set_sync_point(&context);
                        } else {
                            let buffer = buffer.make_mut();
                            let meta = gst_gl::GLSyncMeta::add(buffer, &context);
                            meta.set_sync_point(&context);
                        }

                        let _ = sender.try_send(VideoFrame { info, buffer });

                        Ok(gst::FlowSuccess::Ok)
                    }
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

    pub async fn recv(&mut self) -> Option<VideoFrame> {
        self.receiver.recv().await
    }

    pub fn try_recv(&mut self) -> Result<VideoFrame, tokio::sync::mpsc::error::TryRecvError> {
        self.receiver.try_recv()
    }
}
