use anyhow::Result;
use clap::Parser;
use client::{Client, ImagesMessage, OdometryMessage};
use slam_core::SlamCore;
use std::{net::SocketAddr, str::FromStr, sync::Arc, time::Duration};

mod client;
mod slam_core;
mod slam_core_sys;

#[derive(clap::Parser)]
struct Args {
    #[clap(long, default_value = "127.0.0.1:6677")]
    host: String,
    #[clap(long, default_value = "1000")]
    image_interval: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let image_interval = Duration::from_millis(args.image_interval);

    let client = Client::new(SocketAddr::from_str(&args.host)?).await?;

    let mut slam_core = SlamCore::new();
    let image_sender = client.image_sender();
    let odometry_sender = client.odometry_sender();
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
        match odometry_sender.try_send(odometry) {
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
        match image_sender.try_send(ImagesMessage {
            odometry,
            color: ev.color_image,
            color_intrinsics,
            depth: ev.depth_image,
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
    Ok(())
}
