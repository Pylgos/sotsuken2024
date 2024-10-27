use anyhow::Result;
use tokio::net::{ToSocketAddrs, UdpSocket};
pub use vrrop_control_common::{ControlMessage, SetTargetVelocity};

pub struct Client {
    socket: UdpSocket,
}

impl Client {
    pub async fn new(addr: impl ToSocketAddrs) -> Result<Self> {
        let socket = UdpSocket::bind("127.0.0.1:0").await?;
        socket.connect(addr).await?;
        Ok(Self { socket })
    }

    pub async fn send_message(&self, message: &ControlMessage) -> Result<()> {
        let bytes = message.serialize();
        let bytes_sent = self.socket.send(&bytes).await?;
        if bytes_sent != bytes.len() {
            anyhow::bail!(
                "Failed to send all bytes. Sent: {}, Expected: {}",
                bytes_sent,
                bytes.len()
            );
        }
        Ok(())
    }

    pub async fn set_target_velocity(&self, target_velocity: SetTargetVelocity) -> Result<()> {
        self.send_message(&ControlMessage::SetTargetVelocity(target_velocity))
            .await
    }

    pub async fn set_leg_length(&self, length: f32) -> Result<()> {
        self.send_message(&ControlMessage::SetLegLength(length))
            .await
    }
}
