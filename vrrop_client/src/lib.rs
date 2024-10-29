use anyhow::{anyhow, bail, Context, Result};
use futures::{never::Never, StreamExt, TryStreamExt};
use image::{ImageBuffer, Luma, Rgb};
use nalgebra::{Quaternion, UnitQuaternion, Vector3, Vector4};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::net::{lookup_host, ToSocketAddrs};
use tokio::{net::UdpSocket, select, task::JoinHandle, time::sleep};
use tokio_util::sync::CancellationToken;
use vrrop_common::CameraIntrinsics;

mod pointcloud;
pub use pointcloud::GridIndex;
pub use pointcloud::PointCloud;

#[derive(Debug, Clone)]
pub enum ServerMessage {}

#[derive(Debug, Copy, Clone)]
pub struct OdometryMessage {
    pub stamp: std::time::SystemTime,
    pub translation: Vector3<f32>,
    pub rotation: UnitQuaternion<f32>,
}

#[derive(Debug, Clone)]
pub struct ImagesMessage {
    pub odometry: OdometryMessage,
    pub color: ImageBuffer<Rgb<u8>, Vec<u8>>,
    pub color_intrinsics: CameraIntrinsics,
    pub depth: ImageBuffer<Luma<u16>, Vec<u16>>,
    pub depth_intrinsics: CameraIntrinsics,
    pub depth_unit: f32,
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

pub struct Client {
    connect_loop: JoinHandle<()>,
    cancel: CancellationToken,
}

async fn decode_images_message(compressed: vrrop_common::ImagesMessage) -> Result<ImagesMessage> {
    let color_image = compressed.color_image;
    let depth_image = compressed.depth_image;
    let color = tokio::task::spawn_blocking(move || image::load_from_memory(&color_image));
    let depth = tokio::task::spawn_blocking(move || image::load_from_memory(&depth_image));
    Ok(ImagesMessage {
        odometry: decode_odometry_message(compressed.odometry),
        color: color.await??.to_rgb8(),
        color_intrinsics: compressed.color_intrinsics,
        depth: depth.await??.to_luma16(),
        depth_intrinsics: compressed.depth_intrinsics,
        depth_unit: compressed.depth_unit,
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

async fn connect(
    target: SocketAddr,
    callbacks: Arc<Callbacks>,
    cancel: CancellationToken,
) -> Result<()> {
    let udp_sock = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
    udp_sock.connect(target).await?;

    let url = format!("ws://{}", target);
    let ws_stream = tokio_tungstenite::connect_async(&url).await?.0;
    println!("Connected to {}", url);
    let (_ws_writer, ws_reader) = ws_stream.split();

    let ws_read_loop = tokio::spawn({
        let callbacks = Arc::clone(&callbacks);
        async move {
            ws_reader
                .map_err(|e| anyhow!(e))
                .and_then(|msg| async {
                    handle_images_message(&msg.into_data(), &callbacks).await?;
                    anyhow::Ok(())
                })
                .try_for_each(|_| async { Ok(()) })
                .await
        }
    });
    let ws_read_abort_handle = ws_read_loop.abort_handle();

    let udp_recv_loop: JoinHandle<Result<Never>> = tokio::spawn({
        let udp_sock = Arc::clone(&udp_sock);
        let callbacks = Arc::clone(&callbacks);
        async move {
            loop {
                let mut data = [0u8; 1024];
                let n = udp_sock.recv(&mut data).await?;
                let msg = decode_odometry_message(bincode::deserialize(&data[..n]).unwrap());
                (callbacks.on_odometry)(msg);
            }
        }
    });
    let udp_recv_abort_handle = udp_recv_loop.abort_handle();

    let udp_send_loop: JoinHandle<Result<Never>> = tokio::spawn({
        let udp_sock = Arc::clone(&udp_sock);
        async move {
            loop {
                udp_sock.send(&[]).await?;
                sleep(Duration::from_millis(100)).await;
            }
        }
    });
    let udp_send_abort_handle = udp_send_loop.abort_handle();

    select! {
        res = ws_read_loop => {
            match res.unwrap() {
                Ok(()) => {
                    bail!("WebSocket connection closed");
                }
                Err(e) => {
                    Err(anyhow!(e).context("Reading Websocket failed"))
                }
            }
        }
        res = udp_recv_loop => {
            match res.unwrap() {
                Ok(a) => match a {},
                Err(e) => {
                    Err(e.context("Reading UDP failed"))
                }
            }
        }
        res = udp_send_loop => {
            match res.unwrap() {
                Ok(a) => match a {},
                Err(e) => {
                    Err(e.context("Sending UDP failed"))
                }
            }
        }
        _ = cancel.cancelled() => {
            ws_read_abort_handle.abort();
            udp_recv_abort_handle.abort();
            udp_send_abort_handle.abort();
            Ok(())
        }
    }
}

impl Client {
    pub async fn new(target: impl ToSocketAddrs, callbacks: Callbacks) -> Result<Self> {
        let target = lookup_host(target)
            .await?
            .next()
            .context("Failed to resolve host")?;
        let callbacks = Arc::new(callbacks);
        let cancel = CancellationToken::new();

        let connect_loop = tokio::spawn({
            let cancel = cancel.clone();
            async move {
                loop {
                    match connect(target, Arc::clone(&callbacks), cancel.clone()).await {
                        Ok(_) => return,
                        Err(e) => {
                            eprintln!("Error: {:?}", e);
                        }
                    }
                    sleep(Duration::from_secs(1)).await;
                }
            }
        });

        Ok(Self {
            connect_loop,
            cancel,
        })
    }

    pub async fn shutdown(self) {
        self.cancel.cancel();
        self.connect_loop.await.unwrap();
    }
}
