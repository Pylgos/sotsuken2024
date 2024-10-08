use std::time::Duration;

use anyhow::Result;
use tokio::{select, time::sleep};

use eframe::egui;
use vrrop_control_client::{Client, SetTargetVelocity};

struct App {}

impl App {
    fn new(cc: &eframe::CreationContext) -> Self {
        Self {}
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Hello World!");
        });
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let native_options = eframe::NativeOptions::default();
    let res = eframe::run_native(
        "My egui App",
        native_options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    );
    match res {
        Ok(_) => (),
        Err(e) => eprintln!("Error: {e}"),
    }
    // let client = Client::new("127.0.0.1:23456").await?;
    // let task = tokio::spawn(async move {
    //     loop {
    //         client
    //             .set_target_velocity(SetTargetVelocity {
    //                 vx: 1.0,
    //                 vy: 2.0,
    //                 vtheta: 3.0,
    //             })
    //             .await
    //             .unwrap();
    //         sleep(Duration::from_secs_f64(0.1)).await
    //     }
    // });
    // let abort = task.abort_handle();
    // select! {
    //     _ = task => (),
    //     _ = tokio::signal::ctrl_c() => { abort.abort(); },
    // }
    Ok(())
}
