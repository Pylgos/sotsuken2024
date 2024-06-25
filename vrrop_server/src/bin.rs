use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let server = vrrop_server::Server::new(vrrop_server::Callbacks::new(
        |msg| {
            println!("odometry received");
            println!("translation: {}", msg.translation);
            println!("rotation: {}", msg.rotation);
        },
        |images| {
            println!("images received");
            println!("color image size: {}", images.color.len());
            println!("depth image size: {}", images.depth.len());
        },
    ))
    .await?;
    tokio::signal::ctrl_c().await?;
    Ok(())
}
