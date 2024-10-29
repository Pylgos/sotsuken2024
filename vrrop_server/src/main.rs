use anyhow::Result;
use clap::Parser;
use server::{ImagesMessage, OdometryMessage, Server};
use slam_core::SlamCore;
use std::{sync::Arc, time::Duration};

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

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let image_interval = Duration::from_millis(args.image_interval);

    let server = Server::new(args.port).await?;

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
    tokio::signal::ctrl_c().await?;
    println!("Exiting...");
    server.shutdown().await?;
    Ok(())
}
