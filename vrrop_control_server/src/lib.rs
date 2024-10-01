use anyhow::Result;
use tokio::{net::UdpSocket, select};
use tokio_util::sync::{CancellationToken, DropGuard};
pub use vrrop_control_common::ControlCommand;

pub struct Callbacks {
    on_control_command: Box<dyn Fn(&ControlCommand) + Send>,
}

impl Callbacks {
    pub fn new(on_control_command: impl Fn(&ControlCommand) + 'static + Send) -> Self {
        Self {
            on_control_command: Box::new(on_control_command),
        }
    }
}

pub struct Server {
    _cancellation_guard: DropGuard,
}

impl Server {
    pub async fn new(callbacks: Callbacks) -> Result<Self> {
        let sock = UdpSocket::bind("0.0.0.0:23456").await?;
        let cancelation = CancellationToken::new();
        let cancellation_clone = cancelation.clone();
        tokio::spawn(async move {
            loop {
                let mut buf = vec![0; 1024];
                let recv_result = select! {
                    _ = cancellation_clone.cancelled() => break,
                    recv_result = sock.recv_buf(&mut buf) => recv_result,
                };
                match recv_result {
                    Ok(n) => match ControlCommand::deserialize(&buf[..n]) {
                        Ok(command) => {
                            (callbacks.on_control_command)(&command);
                        }
                        Err(e) => {
                            eprintln!("Error deserializing UDP packet: {:?}", e);
                        }
                    },
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
