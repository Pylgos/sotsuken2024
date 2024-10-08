use std::time::Duration;

use anyhow::Result;
use tokio::{select, time::sleep};

use eframe::egui;
use vrrop_control_client::{Client, SetTargetVelocity};

#[tokio::main]
async fn main() -> Result<()> {
    let client = Client::new("127.0.0.1:23456").await?;
    let task = tokio::spawn(async move {
        loop {
            client
                .set_target_velocity(SetTargetVelocity {
                    vx: 1.0,
                    vy: 2.0,
                    vtheta: 3.0,
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
