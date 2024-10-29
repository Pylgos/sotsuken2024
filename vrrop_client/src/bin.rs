use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let _client = vrrop_client::Client::new(
        "127.0.0.1:6677",
        vrrop_client::Callbacks::new(
            |msg| {
                println!("odometry received");
                println!("translation: {}", msg.translation);
                println!("rotation: {}", msg.rotation);
            },
            |images| {
                println!("images received");
                println!("color image size: {}", images.color.len());
                println!("depth image size: {}", images.depth.len());
                println!("color intrinsics: {:?}", images.color_intrinsics);
                println!("depth intrinsics: {:?}", images.depth_intrinsics);
            },
        ),
    )
    .await?;
    tokio::signal::ctrl_c().await?;
    Ok(())
}
