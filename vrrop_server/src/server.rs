use std::{
    collections::HashMap,
    panic,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

use anyhow::{anyhow, Result};
use futures::{FutureExt, SinkExt, StreamExt, TryStreamExt};
use image::{
    codecs::{jpeg::JpegEncoder, png::PngEncoder},
    EncodableLayout, ExtendedColorType, ImageEncoder,
};
use nalgebra::{UnitQuaternion, Vector3};
use tokio::{
    net::{TcpListener, TcpStream, UdpSocket},
    select,
    sync::broadcast,
    task::JoinHandle,
    time::sleep,
};
use tokio_tungstenite::WebSocketStream;
use vrrop_common::{CameraIntrinsics, Command, PongMessage, UdpClientMessage, UdpServerMessage};

use crate::slam_core::{ColorImage, DepthImage};

#[derive(Debug, Clone, Copy)]
pub struct OdometryMessage {
    pub stamp: std::time::SystemTime,
    pub translation: Vector3<f32>,
    pub rotation: UnitQuaternion<f32>,
}

#[derive(Clone)]
pub struct ImagesMessage {
    pub odometry: OdometryMessage,
    pub color: Arc<ColorImage>,
    pub color_intrinsics: CameraIntrinsics,
    pub depth: Arc<DepthImage>,
    pub depth_intrinsics: CameraIntrinsics,
}

#[derive(Debug)]
pub struct Server {
    image_sender: broadcast::Sender<vrrop_common::ImagesMessage>,
    odometry_sender: broadcast::Sender<vrrop_common::OdometryMessage>,
    _dummy_image_receiver: broadcast::Receiver<vrrop_common::ImagesMessage>,
    _dummy_odometry_receiver: broadcast::Receiver<vrrop_common::OdometryMessage>,
    serve_websocket_join_handle: JoinHandle<()>,
    serve_udp_join_handle: JoinHandle<()>,
}

async fn serve_websocket(
    port: u16,
    image_receiver: broadcast::Sender<vrrop_common::ImagesMessage>,
    callbacks: Arc<Callbacks>,
) -> Result<()> {
    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    futures::stream::try_unfold(listener, move |listener| async move {
        match listener.accept().await {
            Ok((stream, peer_addr)) => Ok(Some(((stream, peer_addr), listener))),
            Err(e) => Err(anyhow!(e)),
        }
    })
    .map_ok(|(stream, peer_addr)| {
        println!("Accepted websocket connection from {peer_addr}");
        let image_receiver = image_receiver.subscribe();
        let callbacks = callbacks.clone();
        tokio::spawn(async move {
            let websocket = tokio_tungstenite::accept_async(stream).await?;
            handle_websocket_connection(websocket, image_receiver, callbacks).await
        })
        .map(move |e| {
            let res = e.unwrap();
            match res {
                Ok(_) => {}
                Err(e) => eprintln!("Error handling websocket connection from {peer_addr}: {e}"),
            }
            Ok(())
        })
    })
    .try_buffer_unordered(5)
    .for_each(|res| async {
        match res {
            Ok(_) => {}
            Err(e) => eprintln!("Error accepting websocket connection: {:?}", e),
        }
    })
    .await;
    Ok(())
}

async fn handle_websocket_connection(
    websocket: WebSocketStream<TcpStream>,
    mut image_receiver: broadcast::Receiver<vrrop_common::ImagesMessage>,
    callbacks: Arc<Callbacks>,
) -> Result<()> {
    let (mut writer, mut reader) = websocket.split();
    loop {
        select! {
            res = image_receiver.recv() => {
                match res {
                    Ok(images) => {
                let encoded_msg = bincode::serialize(&images)?;
                writer.send(tokio_tungstenite::tungstenite::Message::binary(encoded_msg)).await?;
                    }
                    Err(broadcast::error::RecvError::Closed) => return Ok(()),
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
            res = reader.next() => {
                match res {
                    Some(Ok(msg)) => {
                        let cmd: Command = bincode::deserialize(&msg.into_data())?;
                        (callbacks.on_command)(cmd);
                    }
                    Some(Err(e)) => return Err(anyhow!(e)),
                    None => return Ok(()),
                }
            }
        }
    }
}

async fn serve_udp(
    port: u16,
    mut odometry_receiver: broadcast::Receiver<vrrop_common::OdometryMessage>,
) -> Result<()> {
    let udp_sock = Arc::new(UdpSocket::bind(("0.0.0.0", port)).await?);
    let mut clients = HashMap::new();
    loop {
        let mut buf = [0u8; 2048];
        select! {
            res = udp_sock.recv_from(&mut buf) => {
                let (n, src) = res?;
                clients.insert(src, Instant::now());
                let data = &buf[..n];
                let msg: UdpClientMessage = bincode::deserialize(data)?;
                match msg {
                    UdpClientMessage::Ping(ping) => {
                        let pong = UdpServerMessage::Pong(PongMessage {
                            client_time: ping.client_time,
                            server_time: SystemTime::now(),
                        });
                        let encoded_msg = bincode::serialize(&pong)?;
                        udp_sock.send_to(&encoded_msg, src).await?;
                    }
                }
            }
            res = odometry_receiver.recv() => {
                match res {
                    Ok(msg) => {
                        clients = clients
                            .into_iter()
                            .filter_map(|(src, time)| {
                                if time.elapsed().as_secs() < 5 {
                                    Some((src, time))
                                } else {
                                    None
                                }
                            })
                            .collect();
                        let encoded_msg = bincode::serialize(&UdpServerMessage::Odometry(msg))?;
                        for src in clients.keys() {
                            udp_sock.send_to(&encoded_msg, src).await?;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => return Ok(()),
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        }
    }
}

pub struct Callbacks {
    pub on_command: Box<dyn Fn(Command) + Send + Sync>,
}

impl Server {
    pub async fn new(port: u16, callbacks: Callbacks) -> Result<Self> {
        let callbacks = Arc::new(callbacks);
        let (image_sender, _image_receiver) = broadcast::channel(2);
        let (odometry_sender, _odometry_receiver) = broadcast::channel(10);
        let serve_websocket_join_handle = tokio::spawn({
            let image_sender = image_sender.clone();
            async move {
                loop {
                    match serve_websocket(port, image_sender.clone(), callbacks.clone()).await {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("Error serving websocket: {:?}", e);
                            sleep(Duration::from_secs(1)).await;
                        }
                    }
                }
            }
        });
        let serve_udp_join_handle = tokio::spawn({
            let odometry_sender = odometry_sender.clone();
            async move {
                loop {
                    match serve_udp(port, odometry_sender.subscribe()).await {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("Error serving udp: {:?}", e);
                            sleep(Duration::from_secs(1)).await;
                        }
                    }
                }
            }
        });
        Ok(Self {
            image_sender,
            odometry_sender,
            _dummy_image_receiver: _image_receiver,
            _dummy_odometry_receiver: _odometry_receiver,
            serve_websocket_join_handle,
            serve_udp_join_handle,
        })
    }

    pub async fn shutdown(self) -> Result<()> {
        self.serve_websocket_join_handle.abort();
        self.serve_udp_join_handle.abort();
        match self.serve_websocket_join_handle.await {
            Ok(()) => {}
            Err(join_err) => {
                if join_err.is_panic() {
                    panic::resume_unwind(join_err.into_panic())
                }
            }
        }
        match self.serve_udp_join_handle.await {
            Ok(()) => {}
            Err(join_err) => {
                if join_err.is_panic() {
                    panic::resume_unwind(join_err.into_panic())
                }
            }
        }
        Ok(())
    }

    pub fn odometry_sender(&self) -> broadcast::Sender<vrrop_common::OdometryMessage> {
        self.odometry_sender.clone()
    }

    pub fn image_sender(&self) -> broadcast::Sender<vrrop_common::ImagesMessage> {
        self.image_sender.clone()
    }
}

pub fn encode_odometry_message(msg: &OdometryMessage) -> vrrop_common::OdometryMessage {
    vrrop_common::OdometryMessage {
        stamp: msg.stamp,
        translation: msg.translation.into(),
        rotation: (*msg.rotation.into_inner().as_vector()).into(),
    }
}

pub async fn encode_images_message(msg: &ImagesMessage) -> Result<vrrop_common::ImagesMessage> {
    let (color, depth) = tokio::join!(
        encode_color(msg.color.clone()),
        encode_depth(msg.depth.clone())
    );
    Ok(vrrop_common::ImagesMessage {
        odometry: encode_odometry_message(&msg.odometry),
        color_image: color?,
        color_intrinsics: msg.color_intrinsics,
        depth_image: depth?,
        depth_intrinsics: msg.depth_intrinsics,
        depth_unit: 0.001,
    })
}

async fn encode_color(img: Arc<ColorImage>) -> Result<Vec<u8>> {
    tokio::task::spawn_blocking(move || {
        let mut dst = Vec::new();
        let enc = JpegEncoder::new_with_quality(&mut dst, 70);
        enc.write_image(
            &img.as_bytes()[..image_size(img.as_ref())],
            img.width(),
            img.height(),
            ExtendedColorType::Rgb8,
        )?;
        Ok(dst)
    })
    .await?
}

async fn encode_depth(img: Arc<DepthImage>) -> Result<Vec<u8>> {
    tokio::task::spawn_blocking(move || {
        let mut dst = Vec::new();
        let enc = PngEncoder::new(&mut dst);
        enc.write_image(
            &img.as_bytes()[..image_size(img.as_ref())],
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
