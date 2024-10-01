use anyhow::Result;
use vrrop_control_server::{Callbacks, Server};

#[tokio::main]
async fn main() -> Result<()> {
    let _server = Server::new(Callbacks::new(|command| {
        println!("Received command: {:?}", command);
    }))
    .await?;
    tokio::signal::ctrl_c().await?;
    Ok(())
}
