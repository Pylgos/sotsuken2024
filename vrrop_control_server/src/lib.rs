use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use anyhow::Result;
use tokio::{net::UdpSocket, select};
use tokio_util::sync::{CancellationToken, DropGuard};
pub use vrrop_control_common::ControlMessage;
pub use vrrop_control_common::SetTargetVelocity;

pub struct Callbacks {
    on_control_command: Box<dyn Fn(&ControlMessage) + Send>,
}

impl Callbacks {
    pub fn new(on_control_command: impl Fn(&ControlMessage) + 'static + Send) -> Self {
        Self {
            on_control_command: Box::new(on_control_command),
        }
    }
}

pub struct Server {
    _cancellation_guard: DropGuard,
}

impl Server {
    pub async fn new(port: u16, callbacks: Callbacks) -> Result<Self> {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), port);
        let sock = UdpSocket::bind(addr).await?;
        let cancelation = CancellationToken::new();
        let cancellation_clone = cancelation.clone();
        tokio::spawn(async move {
            loop {
                let mut buf = Vec::with_capacity(1500);
                let recv_result = select! {
                    _ = cancellation_clone.cancelled() => break,
                    recv_result = sock.recv_buf(&mut buf) => recv_result,
                };
                match recv_result {
                    Ok(n) => {
                        let data = &buf[..n];
                        match ControlMessage::deserialize(data) {
                            Ok(command) => {
                                (callbacks.on_control_command)(&command);
                            }
                            Err(e) => {
                                eprintln!("Error deserializing UDP packet: {:?}", e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error receiving UDP packet: {:?}", e);
                        break;
                    }
                };
            }
        });
        Ok(Self {
            _cancellation_guard: cancelation.drop_guard(),
        })
    }
}
