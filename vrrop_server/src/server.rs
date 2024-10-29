use std::{
    collections::HashMap,
    panic,
    sync::Arc,
    time::{Duration, Instant},
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
use vrrop_common::CameraIntrinsics;

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
    image_sender: broadcast::Sender<ImagesMessage>,
    odometry_sender: broadcast::Sender<OdometryMessage>,
    serve_websocket_join_handle: JoinHandle<()>,
    serve_udp_join_handle: JoinHandle<()>,
}

// async fn server_loop(
//     port: u16,
//     image_receiver: mpsc::Receiver<ImagesMessage>,
//     odometry_receiver: mpsc::Receiver<OdometryMessage>,
//     cancel: CancellationToken,
// ) -> Result<()> {
//     let image_receiver = Arc::new(Mutex::new(image_receiver));
//     let odometry_receiver = Arc::new(Mutex::new(odometry_receiver));
//     loop {
//         let url = format!("ws://{:}", target);
//         println!("Connecting to server: {:}", url);
//         let ws_stream = loop {
//             match tokio_tungstenite::connect_async(&url).await {
//                 Ok((ws_stream, _)) => {
//                     break ws_stream;
//                 }
//                 Err(_e) => {
//                     // eprintln!("Error connecting to server: {:?}", e);
//                     tokio::select! {
//                         _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {}
//                         _ = cancel.cancelled() => { return Ok(()); }
//                     }
//                 }
//             }
//         };
//         println!("Connected to server: {:}", url);
//
//         // We don't want to send old images
//         while image_receiver.lock().await.try_recv().is_ok() {}
//
//         let udp_socket = Arc::new(UdpSocket::bind("0.0.0.0:6678").await.unwrap());
//         udp_socket.connect(target).await.unwrap();
//
//         let (mut writer, _reader) = ws_stream.split();
//         let image_receiver = image_receiver.clone();
//         let image_write_loop = tokio::task::spawn(async move {
//             loop {
//                 let Some(images) = image_receiver.lock().await.recv().await else {
//                     break true;
//                 };
//
//                 let msg = encode_images_msssage(images).await.unwrap();
//                 let encoded_msg = bincode::serialize(&msg).unwrap();
//
//                 match writer
//                     .send(tokio_tungstenite::tungstenite::Message::binary(encoded_msg))
//                     .await
//                 {
//                     Ok(_) => {}
//                     Err(e) => {
//                         eprintln!("Error sending images message: {:}", e);
//                         break false;
//                     }
//                 }
//             }
//         });
//         let _odometry_write_loop = tokio::task::spawn({
//             let odometry_receiver = Arc::clone(&odometry_receiver);
//             let udp_socket = Arc::clone(&udp_socket);
//             async move {
//                 loop {
//                     let Some(odometry) = odometry_receiver.lock().await.recv().await else {
//                         break;
//                     };
//                     let msg = encode_odometry_message(&odometry);
//                     let encoded_msg = bincode::serialize(&msg).unwrap();
//                     match udp_socket.send(&encoded_msg).await {
//                         Ok(_) => {}
//                         Err(e) => {
//                             eprintln!("Error sending odometry message: {:}", e);
//                             break;
//                         }
//                     }
//                 }
//             }
//         });
//         tokio::select! {
//             quit = image_write_loop => {
//                 println!("Disconnected");
//                 if quit.unwrap() {
//                     break;
//                 }
//             },
//         };
//     }
//     Ok(())
// }

async fn serve_websocket(
    port: u16,
    image_receiver: broadcast::Sender<ImagesMessage>,
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
        tokio::spawn(async move {
            let websocket = tokio_tungstenite::accept_async(stream).await?;
            handle_websocket_connection(websocket, image_receiver).await
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
    mut image_receiver: broadcast::Receiver<ImagesMessage>,
) -> Result<()> {
    let (mut writer, _reader) = websocket.split();
    loop {
        let images = match image_receiver.recv().await {
            Ok(images) => images,
            Err(broadcast::error::RecvError::Closed) => return Ok(()),
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
        };
        let msg = encode_images_msssage(&images).await?;
        let encoded_msg = bincode::serialize(&msg)?;

        writer
            .send(tokio_tungstenite::tungstenite::Message::binary(encoded_msg))
            .await?;
    }
}

async fn serve_udp(
    port: u16,
    mut odometry_receiver: broadcast::Receiver<OdometryMessage>,
) -> Result<()> {
    let udp_sock = Arc::new(UdpSocket::bind(("0.0.0.0", port)).await?);
    let mut clients = HashMap::new();
    loop {
        let mut buf = [0u8; 2048];
        select! {
            res = udp_sock.recv_from(&mut buf) => {
                let (_n, src) = res?;
                clients.insert(src, Instant::now());
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
                        let encoded_msg = bincode::serialize(&encode_odometry_message(&msg))?;
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

impl Server {
    pub async fn new(port: u16) -> Result<Self> {
        let (image_sender, _image_receiver) = broadcast::channel(2);
        let (odometry_sender, _odometry_receiver) = broadcast::channel(10);
        let serve_websocket_join_handle = tokio::spawn({
            let image_sender = image_sender.clone();
            async move {
                loop {
                    match serve_websocket(port, image_sender.clone()).await {
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

    pub fn odometry_sender(&self) -> broadcast::Sender<OdometryMessage> {
        self.odometry_sender.clone()
    }

    pub fn image_sender(&self) -> broadcast::Sender<ImagesMessage> {
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

async fn encode_images_msssage(msg: &ImagesMessage) -> Result<vrrop_common::ImagesMessage> {
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
        let mut enc = JpegEncoder::new_with_quality(&mut dst, 50);
        enc.encode_image(img.as_ref())?;
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
