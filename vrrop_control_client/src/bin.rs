use std::time::Duration;

use anyhow::Result;
use tokio::{select, time::sleep};
use vrrop_control_client::Client;
use vrrop_control_common::SetTargetVelocity;

#[tokio::main]
async fn main() -> Result<()> {
    let client = Client::new("127.0.0.1:23456").await?;
    let task = tokio::spawn(async move {
        loop {
            client
                .set_target_velocity(SetTargetVelocity {
                    forward: 1.0,
                    turn: 2.0,
                })
                .await
                .unwrap();
            sleep(Duration::from_secs_f64(0.1)).await
        }
    });
    let abort = task.abort_handle();
    select! {
        _ = task => (),
        _ = tokio::signal::ctrl_c() => { abort.abort(); },
    }
    Ok(())
}
