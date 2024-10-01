use std::{
    str::FromStr,
    time::{Duration, Instant},
};

use anyhow::Result;
use gst::prelude::*;
use gstreamer::{self as gst, PadProbeReturn, PadProbeType, ReferenceTimestampMeta};
use tokio::task::JoinHandle;

use crate::decoder::{VideoDecoder, VideoFrame};

pub struct Visualizer {
    pipeline: gst::Pipeline,
    decoder: VideoDecoder,
    join_handle: JoinHandle<()>,
}

impl Visualizer {
    pub fn new() -> Result<Self> {
        let udpsrc = gst::ElementFactory::make("udpsrc")
            .property("port", 5000)
            .build()?;
        let capsfilter = gst::ElementFactory::make("capsfilter")
            .property(
                "caps",
                gst::Caps::from_str("application/x-rtp,media=video,encoding-name=H264")?,
            )
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
