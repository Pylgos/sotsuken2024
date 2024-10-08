use anyhow::Result;
use vrrop_control_server::{Callbacks, Server};

#[tokio::main]
async fn main() -> Result<()> {
    let _server = Server::new(
        23456,
        Callbacks::new(|command| {
            println!("Received command: {:?}", command);
        }),
    )
    .await?;
    tokio::signal::ctrl_c().await?;
    Ok(())
}
