use anyhow::Result;
use clap::Parser;
use server::{Callbacks, ImagesMessage, OdometryMessage, Server};
use slam_core::SlamCore;
use std::{sync::Arc, time::Duration};
use tokio::select;
use tokio::sync::mpsc;
use vrrop_common::Command;

mod server;
mod slam_core;
mod slam_core_sys;

#[derive(clap::Parser)]
struct Args {
    #[clap(long, short, default_value_t = 6677)]
    port: u16,
    #[clap(long, default_value = "1000")]
    image_interval: u64,
}

fn init_slam_core<'a>(server: &Server, image_interval: Duration) -> Result<SlamCore<'a>> {
    let mut slam_core = SlamCore::new();
    let image_sender = server.image_sender();
    let odometry_sender = server.odometry_sender();
    let last_image_send = Arc::new(std::sync::Mutex::new(std::time::SystemTime::now()));
    let color_intrinsics = *slam_core.color_intrinsics();
    let depth_intrinsics = *slam_core.depth_intrinsics();
    println!("color_intrinsics: {:?}", color_intrinsics);
    println!("depth_intrinsics: {:?}", depth_intrinsics);
    slam_core.register_odometry_event_handler(move |ev| {
        let stamp = std::time::SystemTime::now();
        let pose_is_finite = ev.translation.iter().all(|x| x.is_finite())
            && ev.rotation.as_vector().iter().all(|x| x.is_finite());
        if !pose_is_finite {
            return;
        }
        let odometry = OdometryMessage {
            stamp,
            translation: ev.translation,
            rotation: ev.rotation,
        };
        match odometry_sender.send(odometry) {
            Ok(_) => {}
            Err(_) => {
                // eprintln!("odometry message dropped!");
            }
        }
        {
            let mut guard = last_image_send.lock().unwrap();
            if stamp.duration_since(*guard).unwrap() < image_interval {
                return;
            }
            *guard = stamp;
        }
        match image_sender.send(ImagesMessage {
            odometry,
            color: Arc::new(ev.color_image),
            color_intrinsics,
            depth: Arc::new(ev.depth_image),
            depth_intrinsics,
        }) {
            Ok(_) => {}
            Err(_) => {
                // eprintln!("image message dropped!");
            }
        };
    });
    Ok(slam_core)
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let image_interval = Duration::from_millis(args.image_interval);

    let (command_sender, mut command_receiver) = mpsc::unbounded_channel();
    let server = Server::new(
        args.port,
        Callbacks {
            on_command: Box::new(move |command| {
                command_sender.send(command).unwrap();
            }),
        },
    )
    .await?;
    let mut slam_core: Option<SlamCore> = None;
    slam_core.replace(init_slam_core(&server, image_interval)?);
    loop {
        select! {
            command = command_receiver.recv() => {
                match command {
                    Some(Command::Reset) => {
                        println!("Resetting SLAM core...");
                        slam_core.replace(init_slam_core(&server, image_interval)?);
                    }
                    None => {
                        break;
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                break;
            }

        }
    }
    println!("Exiting...");
    server.shutdown().await?;
    Ok(())
}
