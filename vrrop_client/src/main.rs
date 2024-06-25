use anyhow::Result;
use client::{Client, ImagesMessage, OdometryMessage};
use slam_core::SlamCore;
use std::{
    net::SocketAddr,
    str::FromStr,
    sync::Arc,
    time::Duration,
};

mod client;
mod slam_core;
mod slam_core_sys;

#[tokio::main]
async fn main() -> Result<()> {
    let image_interval = Duration::from_secs(1);

    let client = Client::new(SocketAddr::from_str("127.0.0.1:6677")?).await?;
    let mut slam_core = SlamCore::new();
    let image_sender = client.image_sender();
    let odometry_sender = client.odometry_sender();
    let last_image_send = Arc::new(std::sync::Mutex::new(std::time::SystemTime::now()));
    let color_intrinsics = *slam_core.color_intrinsics();
    let depth_intrinsics = *slam_core.depth_intrinsics();
    slam_core.register_odometry_event_handler(move |ev| {
        let stamp = std::time::SystemTime::now();
        match odometry_sender.try_send(OdometryMessage {
            stamp,
            translation: ev.translation,
            rotation: ev.rotation,
        }) {
            Ok(_) => {}
            Err(_) => eprintln!("odometry message dropped!"),
        }
        {
            let mut guard = last_image_send.lock().unwrap();
            if stamp.duration_since(*guard).unwrap() < image_interval {
                return;
            }
            *guard = stamp;
        }
        match image_sender.try_send(ImagesMessage {
            stamp,
            color: ev.color_image,
            color_intrinsics,
            depth: ev.depth_image,
            depth_intrinsics,
        }) {
            Ok(_) => {}
            Err(_) => eprintln!("image message dropped!"),
        
        };
    });
    tokio::signal::ctrl_c().await?;
    println!("Exiting...");
    Ok(())
}

