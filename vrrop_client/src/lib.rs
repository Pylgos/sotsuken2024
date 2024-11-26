use anyhow::{anyhow, bail, Context, Result};
use futures::SinkExt;
use futures::{never::Never, StreamExt, TryStreamExt};
use image::{ImageBuffer, Luma, Rgb};
use nalgebra::{Quaternion, UnitQuaternion, Vector3, Vector4};
use std::sync::atomic::AtomicI64;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::net::{lookup_host, ToSocketAddrs};
use tokio::sync::mpsc;
use tokio::{net::UdpSocket, select, task::JoinHandle, time::sleep};
use tokio_util::sync::CancellationToken;
use vrrop_common::{CameraIntrinsics, Command, Stats};

mod pointcloud;
pub use pointcloud::GridIndex;
pub use pointcloud::PointCloud;

#[derive(Debug, Clone)]
pub enum ServerMessage {}

#[derive(Debug, Copy, Clone)]
pub struct OdometryMessage {
    pub original_size: usize,
    pub stamp: std::time::SystemTime,
    pub translation: Vector3<f32>,
    pub rotation: UnitQuaternion<f32>,
}

#[derive(Debug, Clone)]
pub struct ImagesMessage {
    pub original_size: usize,
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
    command_sender: mpsc::UnboundedSender<Command>,
    stats: Arc<Mutex<StatsState>>,
    server_time_offset_ns: Arc<AtomicI64>,
}

#[derive(Debug, Clone, Default)]
struct StatsState {
    stats: Stats,
    recording: bool,
}

async fn decode_images_message(
    compressed: vrrop_common::ImagesMessage,
    original_size: usize,
) -> Result<ImagesMessage> {
    let color_image = compressed.color_image;
    let depth_image = compressed.depth_image;
    let color = tokio::task::spawn_blocking(move || image::load_from_memory(&color_image));
    let depth = tokio::task::spawn_blocking(move || image::load_from_memory(&depth_image));
    Ok(ImagesMessage {
        original_size,
        odometry: decode_odometry_message(compressed.odometry, 0),
        color: color.await??.to_rgb8(),
        color_intrinsics: compressed.color_intrinsics,
        depth: depth.await??.to_luma16(),
        depth_intrinsics: compressed.depth_intrinsics,
        depth_unit: compressed.depth_unit,
    })
}

fn decode_odometry_message(
    raw: vrrop_common::OdometryMessage,
    original_size: usize,
) -> OdometryMessage {
    OdometryMessage {
        original_size,
        stamp: raw.stamp,
        translation: Vector3::from_row_slice(&raw.translation),
        rotation: UnitQuaternion::new_normalize(Quaternion::from_vector(Vector4::from_row_slice(
            &raw.rotation,
        ))),
    }
}

async fn handle_images_message(
    data: &[u8],
    callbacks: &Callbacks,
    stats: &Mutex<StatsState>,
    server_time_offset_ns: &Arc<AtomicI64>,
) -> Result<()> {
    let compressed = bincode::deserialize::<vrrop_common::ImagesMessage>(data)?;
    let msg = decode_images_message(compressed, data.len()).await?;
    {
        let mut stats = stats.lock().unwrap();
        if stats.recording {
            let stamp_ns = msg.odometry.stamp.duration_since(UNIX_EPOCH)?.as_nanos();
            let server_time_offset_ns =
                server_time_offset_ns.load(std::sync::atomic::Ordering::Relaxed);
            let now_server_time_ns = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
                as i64
                + server_time_offset_ns;
            let latency_ns = now_server_time_ns - stamp_ns as i64;
            stats.stats.images_stamps.push(msg.odometry.stamp);
            stats.stats.images_original_sizes.push(data.len());
            stats.stats.images_latencies.push(latency_ns);
        }
    }
    (callbacks.on_images)(msg);
    Ok(())
}

async fn handle_udp_message(
    data: &[u8],
    callbacks: &Callbacks,
    stats: &Mutex<StatsState>,
    server_time_offset_ns: &Arc<AtomicI64>,
) -> Result<()> {
    let raw = bincode::deserialize::<vrrop_common::UdpServerMessage>(data)?;
    match raw {
        vrrop_common::UdpServerMessage::Pong(pong) => {
            let Ok(rtt) = pong.client_time.elapsed() else {
                return Ok(());
            };
            let server_time_ns = pong
                .server_time
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_nanos() as i64;
            let now_ns = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_nanos() as i64;
            let rtt_ns = rtt.as_nanos() as i64;
            let new_server_time_offset_ns = server_time_ns - (now_ns - rtt_ns / 2);
            server_time_offset_ns.store(
                new_server_time_offset_ns,
                std::sync::atomic::Ordering::Relaxed,
            );
        }
        vrrop_common::UdpServerMessage::Odometry(odom) => {
            let msg = decode_odometry_message(odom, data.len());
            {
                let mut stats = stats.lock().unwrap();
                if stats.recording {
                    let stamp_ns = msg.stamp.duration_since(UNIX_EPOCH)?.as_nanos();
                    let server_time_offset_ns =
                        server_time_offset_ns.load(std::sync::atomic::Ordering::Relaxed);
                    let now_server_time_ns =
                        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos() as i64
                            + server_time_offset_ns;
                    let latency_ns = now_server_time_ns - stamp_ns as i64;
                    stats.stats.odometry_stamps.push(msg.stamp);
                    stats.stats.odometry_original_sizes.push(data.len());
                    stats.stats.odometry_latencies.push(latency_ns);
                }
            }
            (callbacks.on_odometry)(msg);
        }
    }
    Ok(())
}

async fn connect(
    target: SocketAddr,
    callbacks: Arc<Callbacks>,
    cancel: CancellationToken,
    command_receiver: &mut mpsc::UnboundedReceiver<Command>,
    stats: Arc<Mutex<StatsState>>,
    server_time_offset_ns: Arc<AtomicI64>,
) -> Result<()> {
    let udp_sock = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
    udp_sock.connect(target).await?;

    let url = format!("ws://{}", target);
    let ws_stream = tokio_tungstenite::connect_async(&url).await?.0;
    println!("Connected to {}", url);
    let (mut ws_writer, ws_reader) = ws_stream.split();

    let mut ws_read_loop = tokio::spawn({
        let callbacks = Arc::clone(&callbacks);
        let stats = Arc::clone(&stats);
        let server_time_offset_ns = Arc::clone(&server_time_offset_ns);
        async move {
            ws_reader
                .map_err(|e| anyhow!(e))
                .and_then(|msg| async {
                    handle_images_message(
                        &msg.into_data(),
                        &callbacks,
                        &stats,
                        &server_time_offset_ns,
                    )
                    .await?;
                    anyhow::Ok(())
                })
                .try_for_each(|_| async { Ok(()) })
                .await
        }
    });
    let ws_read_abort_handle = ws_read_loop.abort_handle();

    let mut udp_recv_loop: JoinHandle<Result<Never>> = tokio::spawn({
        let udp_sock = Arc::clone(&udp_sock);
        let callbacks = Arc::clone(&callbacks);
        let stats = Arc::clone(&stats);
        let server_time_offset_ns = Arc::clone(&server_time_offset_ns);
        async move {
            loop {
                let mut data = [0u8; 1024];
                let n = udp_sock.recv(&mut data).await?;
                handle_udp_message(&data[..n], &callbacks, &stats, &server_time_offset_ns).await?;
            }
        }
    });
    let udp_recv_abort_handle = udp_recv_loop.abort_handle();

    let mut udp_send_loop: JoinHandle<Result<Never>> = tokio::spawn({
        let udp_sock = Arc::clone(&udp_sock);
        async move {
            loop {
                let msg = bincode::serialize(&vrrop_common::UdpClientMessage::Ping(
                    vrrop_common::PingMessage {
                        client_time: std::time::SystemTime::now(),
                    },
                ))?;
                udp_sock.send(&msg).await?;
                sleep(Duration::from_millis(100)).await;
            }
        }
    });
    let udp_send_abort_handle = udp_send_loop.abort_handle();

    loop {
        select! {
            res = &mut ws_read_loop => {
                match res.unwrap() {
                    Ok(()) => {
                        bail!("WebSocket connection closed");
                    }
                    Err(e) => {
                        return Err(anyhow!(e).context("Reading Websocket failed"))
                    }
                }
            }
            res = &mut udp_recv_loop => {
                match res.unwrap() {
                    Ok(a) => match a {},
                    Err(e) => {
                        return Err(e.context("Reading UDP failed"))
                    }
                }
            }
            res = &mut udp_send_loop => {
                match res.unwrap() {
                    Ok(a) => match a {},
                    Err(e) => {
                        return Err(e.context("Sending UDP failed"))
                    }
                }
            }
            res = command_receiver.recv() => {
                match res {
                    Some(command) => {
                        ws_writer.send(tokio_tungstenite::tungstenite::Message::binary(bincode::serialize(&command)?)).await?;
                    }
                    None => {
                        break;
                    }
                }
            }
            _ = cancel.cancelled() => {
                break;
            }
        }
    }
    ws_read_abort_handle.abort();
    udp_recv_abort_handle.abort();
    udp_send_abort_handle.abort();
    Ok(())
}

impl Client {
    pub async fn new(target: impl ToSocketAddrs, callbacks: Callbacks) -> Result<Self> {
        let target = lookup_host(target)
            .await?
            .next()
            .context("Failed to resolve host")?;
        let callbacks = Arc::new(callbacks);
        let cancel = CancellationToken::new();
        let (command_sender, mut command_receiver) = mpsc::unbounded_channel();
        let stats = Arc::new(Mutex::new(StatsState::default()));
        let server_time_offset_ns = Arc::new(AtomicI64::new(0));

        let connect_loop = tokio::spawn({
            let cancel = cancel.clone();
            let stats = Arc::clone(&stats);
            let server_time_offset_ns = Arc::clone(&server_time_offset_ns);
            async move {
                loop {
                    match connect(
                        target,
                        Arc::clone(&callbacks),
                        cancel.clone(),
                        &mut command_receiver,
                        Arc::clone(&stats.clone()),
                        Arc::clone(&server_time_offset_ns),
                    )
                    .await
                    {
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
            command_sender,
            stats,
            server_time_offset_ns,
        })
    }

    pub fn send_command(&self, command: Command) {
        self.command_sender.send(command).unwrap();
    }

    pub fn start_recording(&self) {
        let mut stats = self.stats.lock().unwrap();
        stats.recording = true;
    }

    pub fn is_recording(&self) -> bool {
        self.stats.lock().unwrap().recording
    }

    pub fn end_recording(&self) {
        let mut stats = self.stats.lock().unwrap();
        stats.recording = false;
        self.command_sender
            .send(Command::SaveStats(stats.stats.clone()))
            .unwrap();
        stats.stats = Default::default();
    }

    pub async fn shutdown(self) {
        self.cancel.cancel();
        self.connect_loop.await.unwrap();
    }
}
