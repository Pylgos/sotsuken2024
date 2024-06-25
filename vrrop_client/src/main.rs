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

// fn encode_color(img: &ColorImage) -> Result<Vec<u8>> {
//     tokio::task::block_in_place(|| {
//         let mut dst = Vec::new();
//         let mut enc = JpegEncoder::new_with_quality(&mut dst, 50);
//         enc.encode_image(img)?;
//         Ok(dst)
//     })
// }

// fn encode_depth(img: &DepthImage) -> Result<Vec<u8>> {
//     let mut dst = Vec::new();
//     let enc = PngEncoder::new(&mut dst);
//     enc.write_image(
//         &img.as_bytes()[..image_size(img)],
//         img.width(),
//         img.height(),
//         ExtendedColorType::L16,
//     )?;
//     Ok(dst)
// }

// fn image_size<V: image::GenericImageView>(img: &V) -> usize {
//     let channels = <V::Pixel as image::Pixel>::CHANNEL_COUNT as usize;
//     let subpixel_size = std::mem::size_of::<<V::Pixel as image::Pixel>::Subpixel>();
//     img.width() as usize * img.height() as usize * subpixel_size * channels
// }

// async fn process_odometry_event(ev: slam_core::OdometryEvent) -> Result<()> {
//     let color_image_size = image_size(&ev.color_image);
//     let depth_image_size = image_size(&ev.depth_image);
//     let color_img = ev.color_image;
//     let depth_img = ev.depth_image;
//     let (encoded_color_res, encoded_depth_res) = tokio::join!(
//         tokio::task::spawn_blocking(move || encode_color(&color_img).unwrap()),
//         tokio::task::spawn_blocking(move || encode_depth(&depth_img).unwrap())
//     );
//     let encoded_color = encoded_color_res?;
//     let encoded_depth = encoded_depth_res?;

//     println!("translation: {:?}", ev.translation);
//     println!("rotation   : {:?}", ev.rotation.euler_angles());
//     println!(
//         "encoded color: {:3.0}% {:}",
//         (encoded_color.len() as f32 / color_image_size as f32) * 100.0,
//         encoded_color.len(),
//     );
//     println!(
//         "encoded depth: {:3.0}% {:}",
//         (encoded_depth.len() as f32 / depth_image_size as f32) * 100.0,
//         encoded_depth.len(),
//     );

//     Ok(())
// }

// #[tokio::main]
// async fn main() {
//     let mut core = SlamCore::new();
//     let tokio_rt = tokio::runtime::Handle::current();
//     core.register_odometry_event_handler(move |ev| {
//         let _enter = tokio_rt.enter();
//         tokio::spawn(async move {
//             process_odometry_event(ev).await.unwrap();
//         });
//     });
//     tokio::time::sleep(Duration::from_secs(10)).await;
// }

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
            color_intrinsics: color_intrinsics,
            depth: ev.depth_image,
            depth_intrinsics: depth_intrinsics,
        }) {
            Ok(_) => {}
            Err(_) => eprintln!("image message dropped!"),
        
        };
    });
    tokio::signal::ctrl_c().await?;
    println!("Exiting...");
    Ok(())
}
