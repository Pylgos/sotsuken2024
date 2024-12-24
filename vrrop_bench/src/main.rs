use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use futures::pin_mut;
use tokio::{select, time::sleep_until};
use vrrop_common::bag;

#[derive(clap::Parser)]
struct Args {
    #[clap(short, long, default_value = "bag")]
    bag: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let mut player = bag::Player::new(&args.bag)?;
    let ctrl_c = tokio::signal::ctrl_c();
    pin_mut!(ctrl_c);
    loop {
        let Some(next_time) = player.poll_next_event_time() else {
            break;
        };
        select! {
            _ = sleep_until(tokio::time::Instant::from_std(next_time)) => {}
            _ = &mut ctrl_c => {
                break;
            }
        }
        let Some(event) = player.next_event()? else {
            break;
        };
        match event {
            bag::Event::Odometry(msg) => {
                println!("Odometry: {:?}", msg.stamp);
            }
            bag::Event::Images(msg) => {
                println!("Image: {:?}", msg.odometry.stamp);
            }
        }
    }
    Ok(())
}
