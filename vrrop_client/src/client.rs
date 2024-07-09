use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;
use futures::{SinkExt, StreamExt};
use image::{
    codecs::{jpeg::JpegEncoder, png::PngEncoder},
    EncodableLayout, ExtendedColorType, ImageEncoder,
};
use nalgebra::{UnitQuaternion, Vector3};
use tokio::{
    net::UdpSocket,
    sync::{mpsc, Mutex},
};
use tokio_util::sync::CancellationToken;
use vrrop_common::CameraIntrinsics;

use crate::slam_core::{ColorImage, DepthImage};

#[derive(Debug, Clone, Copy)]
pub struct OdometryMessage {
    pub stamp: std::time::SystemTime,
    pub translation: Vector3<f32>,
    pub rotation: UnitQuaternion<f32>,
}

pub struct ImagesMessage {
    pub odometry: OdometryMessage,
    pub color: ColorImage,
    pub color_intrinsics: CameraIntrinsics,
    pub depth: DepthImage,
    pub depth_intrinsics: CameraIntrinsics,
}

#[derive(Debug)]
pub struct Client {
    image_sender: mpsc::Sender<ImagesMessage>,
    odometry_sender: mpsc::Sender<OdometryMessage>,
    _cancel: tokio_util::sync::DropGuard,
}

async fn client_loop(
    target: SocketAddr,
    image_receiver: mpsc::Receiver<ImagesMessage>,
    odometry_receiver: mpsc::Receiver<OdometryMessage>,
    cancel: CancellationToken,
) -> Result<()> {
    let image_receiver = Arc::new(Mutex::new(image_receiver));
    let odometry_receiver = Arc::new(Mutex::new(odometry_receiver));
    loop {
        let url = format!("ws://{:}", target);
        println!("Connecting to server: {:}", url);
        let ws_stream = loop {
            match tokio_tungstenite::connect_async(&url).await {
                Ok((ws_stream, _)) => {
                    break ws_stream;
                }
                Err(_e) => {
                    // eprintln!("Error connecting to server: {:?}", e);
                    tokio::select! {
                        _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {}
                        _ = cancel.cancelled() => { return Ok(()); }
                    }
                }
            }
        };
        println!("Connected to server: {:}", url);

        // We don't want to send old images
        while image_receiver.lock().await.try_recv().is_ok() {}

        let udp_socket = Arc::new(UdpSocket::bind("0.0.0.0:6678").await.unwrap());
        udp_socket.connect(target).await.unwrap();

        let (mut writer, _reader) = ws_stream.split();
        let image_receiver = image_receiver.clone();
        let image_write_loop = tokio::task::spawn(async move {
            loop {
                let Some(images) = image_receiver.lock().await.recv().await else {
                    break true;
                };

                let msg = encode_images_msssage(images).await.unwrap();
                let encoded_msg = bincode::serialize(&msg).unwrap();

                match writer
                    .send(tokio_tungstenite::tungstenite::Message::binary(encoded_msg))
                    .await
                {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("Error sending images message: {:}", e);
                        break false;
                    }
                }
            }
        });
        let _odometry_write_loop = tokio::task::spawn({
            let odometry_receiver = Arc::clone(&odometry_receiver);
            let udp_socket = Arc::clone(&udp_socket);
            async move {
                loop {
                    let Some(odometry) = odometry_receiver.lock().await.recv().await else {
                        break;
                    };
                    let msg = encode_odometry_message(&odometry);
                    let encoded_msg = bincode::serialize(&msg).unwrap();
                    match udp_socket.send(&encoded_msg).await {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("Error sending odometry message: {:}", e);
                            break;
                        }
                    }
                }
            }
        });
        tokio::select! {
            quit = image_write_loop => {
                println!("Disconnected");
                if quit.unwrap() {
                    break;
                }
            },
        };
    }
    Ok(())
}

impl Client {
    pub async fn new(target: SocketAddr) -> Result<Self> {
        let (image_sender, image_receiver) = mpsc::channel(2);
        let (odometry_sender, odometry_receiver) = mpsc::channel(10);
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();
        let _client_loop = tokio::spawn(async move {
            client_loop(target, image_receiver, odometry_receiver, cancel_clone).await
        });
        Ok(Self {
            image_sender,
            odometry_sender,
            _cancel: cancel.drop_guard(),
        })
    }

    pub fn odometry_sender(&self) -> mpsc::Sender<OdometryMessage> {
        self.odometry_sender.clone()
    }

    pub fn image_sender(&self) -> mpsc::Sender<ImagesMessage> {
        self.image_sender.clone()
    }
}

fn encode_odometry_message(msg: &OdometryMessage) -> vrrop_common::OdometryMessage {
    vrrop_common::OdometryMessage {
        stamp: msg.stamp,
        translation: msg.translation.into(),
        rotation: (*msg.rotation.into_inner().as_vector()).into(),
    }
}

async fn encode_images_msssage(msg: ImagesMessage) -> Result<vrrop_common::ImagesMessage> {
    let (color, depth) = tokio::join!(encode_color(msg.color), encode_depth(msg.depth));
    Ok(vrrop_common::ImagesMessage {
        odometry: encode_odometry_message(&msg.odometry),
        color_image: color?,
        color_intrinsics: msg.color_intrinsics,
        depth_image: depth?,
        depth_intrinsics: msg.depth_intrinsics,
        depth_unit: 0.001,
    })
}

async fn encode_color(img: ColorImage) -> Result<Vec<u8>> {
    tokio::task::spawn_blocking(move || {
        let mut dst = Vec::new();
        let mut enc = JpegEncoder::new_with_quality(&mut dst, 50);
        enc.encode_image(&img)?;
        Ok(dst)
    })
    .await?
}

async fn encode_depth(img: DepthImage) -> Result<Vec<u8>> {
    tokio::task::spawn_blocking(move || {
        let mut dst = Vec::new();
        let enc = PngEncoder::new(&mut dst);
        enc.write_image(
            &img.as_bytes()[..image_size(&img)],
            img.width(),
            img.height(),
            ExtendedColorType::L16,
        )?;
        Ok(dst)
    })
    .await?
}

fn image_size<V: image::GenericImageView>(img: &V) -> usize {
    let channels = <V::Pixel as image::Pixel>::CHANNEL_COUNT as usize;
    let subpixel_size = std::mem::size_of::<<V::Pixel as image::Pixel>::Subpixel>();
    img.width() as usize * img.height() as usize * subpixel_size * channels
}
