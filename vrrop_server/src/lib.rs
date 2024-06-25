use anyhow::Result;
use futures::{StreamExt, TryStreamExt};
use image::{ImageBuffer, Luma, Rgb};
use nalgebra::{Quaternion, UnitQuaternion, Vector3, Vector4};
use vrrop_common::CameraIntrinsics;
use std::sync::Arc;
use tokio::{
    net::{TcpListener, UdpSocket},
    sync::broadcast,
    task::JoinHandle,
};

#[derive(Debug, Clone)]
pub enum ServerMessage {}

#[derive(Debug, Clone)]
pub struct OdometryMessage {
    pub stamp: std::time::SystemTime,
    pub translation: Vector3<f64>,
    pub rotation: UnitQuaternion<f64>,
}

#[derive(Debug, Clone)]
pub struct ImagesMessage {
    pub stamp: std::time::SystemTime,
    pub color: ImageBuffer<Rgb<u8>, Vec<u8>>,
    pub color_intrinsics: CameraIntrinsics,
    pub depth: ImageBuffer<Luma<u16>, Vec<u16>>,
    pub depth_intrinsics: CameraIntrinsics,
}

pub struct Callbacks {
    on_odometry: Box<dyn Fn(OdometryMessage) + Send + Sync>,
    on_images: Box<dyn Fn(ImagesMessage) + Send + Sync>,
}

impl Callbacks {
    pub fn new(
        on_odometry: impl Fn(OdometryMessage) + Send + Sync + 'static,
        on_images: impl Fn(ImagesMessage) + Send + Sync + 'static,
    ) -> Self {
        Self {
            on_odometry: Box::new(on_odometry),
            on_images: Box::new(on_images),
        }
    }
}

pub struct Server {
    accept_loop: JoinHandle<()>,
    server_msg_tx: broadcast::Sender<ServerMessage>,
}

async fn decode_images_message(compressed: vrrop_common::ImagesMessage) -> Result<ImagesMessage> {
    let color_image = compressed.color_image;
    let depth_image = compressed.depth_image;
    let color = tokio::task::spawn_blocking(move || image::load_from_memory(&color_image));
    let depth = tokio::task::spawn_blocking(move || image::load_from_memory(&depth_image));
    Ok(ImagesMessage {
        stamp: compressed.stamp,
        color: color.await??.to_rgb8(),
        color_intrinsics: compressed.color_intrinsics,
        depth: depth.await??.to_luma16(),
        depth_intrinsics: compressed.depth_intrinsics,
    })
}

fn decode_odometry_message(raw: vrrop_common::OdometryMessage) -> OdometryMessage {
    OdometryMessage {
        stamp: raw.stamp,
        translation: Vector3::from_row_slice(&raw.translation),
        rotation: UnitQuaternion::new_normalize(Quaternion::from_vector(Vector4::from_row_slice(
            &raw.rotation,
        ))),
    }
}

async fn handle_images_message(data: &[u8], callbacks: &Callbacks) -> Result<()> {
    let compressed = bincode::deserialize::<vrrop_common::ImagesMessage>(data)?;
    let msg = decode_images_message(compressed).await?;
    (callbacks.on_images)(msg);
    Ok(())
}

async fn handle_connection(
    listener: &TcpListener,
    udp_sock: Arc<UdpSocket>,
    mut server_msg_recv: broadcast::Receiver<ServerMessage>,
    callbacks: Arc<Callbacks>,
) -> Result<()> {
    let (stream, mut peer_addr) = listener.accept().await?;
    peer_addr.set_port(6678);
    udp_sock.connect(peer_addr).await?;
    let ws = tokio_tungstenite::accept_async(stream).await?;
    let (outgoing, incoming) = ws.split();
    tokio::spawn(async move {
        loop {
            match server_msg_recv.recv().await {
                Ok(msg) => {}
                Err(broadcast::error::RecvError::Closed) => {
                    break;
                }
                Err(e) => {
                    eprintln!("Error: {:?}", e);
                }
            }
        }
    });

    tokio::spawn({
        let udp_sock = Arc::clone(&udp_sock);
        let callbacks = Arc::clone(&callbacks);
        async move {
            loop {
                let mut data = [0u8; 1024];
                match udp_sock.recv(&mut data).await {
                    Ok(n) => {
                        let msg =
                            decode_odometry_message(bincode::deserialize(&data[..n]).unwrap());
                        (callbacks.on_odometry)(msg);
                    }
                    Err(e) => {
                        eprintln!("Error: {:?}", e);
                    }
                }
            }
        }
    });
    incoming
        .try_for_each(|msg| async {
            match handle_images_message(&msg.into_data(), &callbacks).await {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Error: {:?}", e);
                }
            }
            Ok(())
        })
        .await?;
    Ok(())
}

impl Server {
    pub async fn new(callbacks: Callbacks) -> Result<Self> {
        let callbacks = Arc::new(callbacks);
        let listener = TcpListener::bind("0.0.0.0:6677").await?;
        let udp_sock = Arc::new(UdpSocket::bind("0.0.0.0:6677").await?);
        let server_msg_tx: broadcast::Sender<ServerMessage> = broadcast::Sender::new(10);
        let server_msg_tx_clone = server_msg_tx.clone();
        let accept_loop = tokio::spawn(async move {
            loop {
                match handle_connection(
                    &listener,
                    udp_sock.clone(),
                    server_msg_tx_clone.subscribe(),
                    callbacks.clone(),
                )
                .await
                {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("Error: {:?}", e);
                    }
                }
            }
        });
        Ok(Self {
            accept_loop,
            server_msg_tx,
        })
    }
}

