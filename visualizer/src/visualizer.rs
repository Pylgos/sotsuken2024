use std::str::FromStr;
use std::{collections::HashSet, time::Duration};

use anyhow::Result;
use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;
use gstreamer_gl as gst_gl;
use gst_gl::prelude::*;
use realsense_rust as rs;
use rs::{
    frame::{ColorFrame, DepthFrame},
    kind::{Rs2CameraInfo, Rs2Format, Rs2ProductLine, Rs2StreamKind},
    pipeline::InactivePipeline,
};
use tokio::task::JoinHandle;

use crate::decoder::{VideoDecoder, VideoFrame};

pub struct Visualizer {
    pipeline: gst::Pipeline,
    decoder: VideoDecoder,
    join_handle: JoinHandle<()>,
}

impl Visualizer {
    pub fn new(gl_context: gst_gl::GLContext, gl_display: gst_gl::GLDisplay) -> Result<Self> {
        let udpsrc = gst::ElementFactory::make("udpsrc")
            .property("port", 5000)
            .build()?;
        let capsfilter = gst::ElementFactory::make("capsfilter")
            .property("caps", gst::Caps::from_str("application/x-rtp,media=video,encoding-name=H264")?)
            .build()?;
        let rtph264depay = gst::ElementFactory::make("rtph264depay").build()?;
        let h264parse = gst::ElementFactory::make("h264parse").build()?;
        let queue = gst::ElementFactory::make("queue").build()?;
        let decoder = VideoDecoder::new()?;
        let pipeline = gst::Pipeline::default();
        pipeline.add_many([
            &udpsrc,
            &capsfilter,
            &rtph264depay,
            &h264parse,
            &queue,
            decoder.element(),
        ])?;
        gst::Element::link_many([
            &udpsrc,
            &capsfilter,
            &rtph264depay,
            &h264parse,
            &queue,
            decoder.element(),
        ])?;

        pipeline.bus().unwrap().set_sync_handler(move |_, msg| {
            match msg.view() {
                gst::MessageView::NeedContext(ctxt) => {
                    let context_type = ctxt.context_type();
                    if context_type == *gst_gl::GL_DISPLAY_CONTEXT_TYPE {
                        if let Some(el) =
                            msg.src().map(|s| s.downcast_ref::<gst::Element>().unwrap())
                        {
                            let context = gst::Context::new(context_type, true);
                            context.set_gl_display(&gl_display);
                            el.set_context(&context);
                        }
                    }
                    println!("NeedContext: {}", context_type);
                    if context_type == "gst.gl.app_context"
                        || context_type == "gst.gl.local_context"
                    {
                        if let Some(el) =
                            msg.src().map(|s| s.downcast_ref::<gst::Element>().unwrap())
                        {
                            let mut context = gst::Context::new(context_type, true);
                            {
                                let context = context.get_mut().unwrap();
                                let s = context.structure_mut();
                                s.set("context", &gl_context);
                            }
                            el.set_context(&context);
                        }
                    }
                }
                _ => {}
            }

            gst::BusSyncReply::Pass
        });

        let join_handle = tokio::task::spawn_blocking({
            let pipeline = pipeline.clone();
            move || {
                let bus = pipeline.bus().unwrap();
                pipeline.set_state(gst::State::Playing).unwrap();

                for msg in bus.iter_timed(gst::ClockTime::NONE) {
                    use gst::MessageView;

                    match msg.view() {
                        MessageView::Eos(..) => break,
                        MessageView::Error(err) => {
                            pipeline.set_state(gst::State::Null).unwrap();
                            println!("err: {:?}", err);
                            break;
                        }
                        MessageView::StateChanged(event) => {
                            if event.current() == gst::State::Null {
                                break;
                            }
                        }
                        _ => (),
                    }
                }

                pipeline.set_state(gst::State::Null).unwrap();
            }
        });

        Ok(Self {
            pipeline,
            join_handle,
            decoder,
        })
    }

    pub async fn recv(&mut self) -> Option<VideoFrame> {
        self.decoder.recv().await
    }

    pub fn try_recv(&mut self) -> Result<VideoFrame, tokio::sync::mpsc::error::TryRecvError> {
        self.decoder.try_recv()
    }

    pub async fn close(self) -> Result<()> {
        self.decoder.close().await?;
        self.pipeline.set_state(gst::State::Null)?;
        self.join_handle.await?;
        Ok(())
    }
}
