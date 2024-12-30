use anyhow::Result;
use clap::Parser;
use futures::pin_mut;
use server::{
    encode_images_message, encode_odometry_message, Callbacks, ImagesMessage, OdometryMessage,
    Server,
};
use slam_core::SlamCore;
use std::io::Write;
use std::path::PathBuf;
use std::{
    path::Path,
    sync::Arc,
    time::{Duration, UNIX_EPOCH},
};
use tokio::select;
use tokio::sync::{broadcast, mpsc};
use tokio::time::sleep_until;
use vrrop_common::bag::{self, Player, Recorder};
use vrrop_common::{Command, Stats};

mod server;
mod slam_core;
mod slam_core_sys;

#[derive(clap::Parser)]
struct ServeArgs {
    #[clap(long, short, default_value_t = 6677)]
    port: u16,
}

#[derive(clap::Parser)]
struct RecordArgs {
    #[clap(long, short, default_value = "bag")]
    bag_dir: PathBuf,
}

#[derive(clap::Parser)]
struct ReplayArgs {
    #[clap(long, short, default_value = "bag")]
    bag_dir: PathBuf,
    #[clap(long, short, default_value_t = 6677)]
    port: u16,
    #[clap(long = "loop", short, default_value_t = false)]
    loop_: bool,
}

#[derive(clap::Subcommand)]
enum Subcommand {
    Serve(ServeArgs),
    Record(RecordArgs),
    Replay(ReplayArgs),
}

#[derive(clap::Parser)]
struct Args {
    #[clap(subcommand)]
    subcommand: Subcommand,
    #[clap(long, default_value = "1000")]
    image_interval: u64,
}

fn init_slam_core<'a>(
    image_sender: broadcast::Sender<vrrop_common::ImagesMessage>,
    odometry_sender: broadcast::Sender<vrrop_common::OdometryMessage>,
    image_interval: Duration,
) -> Result<SlamCore<'a>> {
    let mut slam_core = SlamCore::new();
    let last_image_send = Arc::new(std::sync::Mutex::new(std::time::SystemTime::now()));
    let color_intrinsics = *slam_core.color_intrinsics();
    let depth_intrinsics = *slam_core.depth_intrinsics();
    println!("color_intrinsics: {:?}", color_intrinsics);
    println!("depth_intrinsics: {:?}", depth_intrinsics);
    let (raw_image_sender, mut raw_image_receiver) = mpsc::unbounded_channel();
    slam_core.register_odometry_event_handler(move |ev| {
        let stamp = std::time::SystemTime::now();
        let pose_is_finite = ev.translation.iter().all(|x| x.is_finite())
            && ev.rotation.as_vector().iter().all(|x| x.is_finite());
        if !pose_is_finite {
            return;
        }
        let odometry = OdometryMessage {
            stamp,
            translation: ev.translation,
            rotation: ev.rotation,
        };
        match odometry_sender.send(encode_odometry_message(&odometry)) {
            Ok(_) => {}
            Err(_) => {
                // eprintln!("odometry message dropped!");
            }
        }
        {
            let mut guard = last_image_send.lock().unwrap();
            if stamp.duration_since(*guard).unwrap() < image_interval {
                return;
            }
            *guard = stamp;
        }
        if let Some((color, depth)) = ev.color_image.zip(ev.depth_image) {
            raw_image_sender
                .send(ImagesMessage {
                    odometry,
                    color: Arc::new(color),
                    color_intrinsics,
                    depth: Arc::new(depth),
                    depth_intrinsics,
                })
                .unwrap();
        }
    });
    tokio::spawn(async move {
        while let Some(msg) = raw_image_receiver.recv().await {
            let encoded = encode_images_message(&msg).await?;
            image_sender.send(encoded).unwrap();
        }
        anyhow::Ok(())
    });
    Ok(slam_core)
}

fn save_stats(stats: Stats, dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let image_stats_path = dir.join("images.csv");
    let mut image_stats_dest = std::fs::File::create(image_stats_path)?;
    writeln!(image_stats_dest, "stamp,size,latency")?;
    for ((stamp, size), &latency) in stats
        .images_stamps
        .iter()
        .zip(stats.images_original_sizes.iter())
        .zip(stats.images_latencies.iter())
    {
        writeln!(
            image_stats_dest,
            "{},{},{}",
            stamp.duration_since(UNIX_EPOCH).unwrap().as_secs_f64(),
            size,
            latency as f64 / 1e9
        )?;
    }
    let odometry_stats_path = dir.join("odometry.csv");
    let mut odometry_stats_dest = std::fs::File::create(odometry_stats_path)?;
    writeln!(odometry_stats_dest, "stamp,size,latency")?;
    for ((stamp, size), &latency) in stats
        .odometry_stamps
        .iter()
        .zip(stats.odometry_original_sizes.iter())
        .zip(stats.odometry_latencies.iter())
    {
        writeln!(
            odometry_stats_dest,
            "{},{},{}",
            stamp.duration_since(UNIX_EPOCH).unwrap().as_secs_f64(),
            size,
            latency as f64 / 1e9
        )?;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let interval = Duration::from_millis(args.image_interval);
    match args.subcommand {
        Subcommand::Serve(args) => serve(interval, args.port).await?,
        Subcommand::Record(args) => record(interval, &args.bag_dir).await?,
        Subcommand::Replay(args) => replay(args.port, &args.bag_dir, args.loop_).await?,
    }
    Ok(())
}

async fn serve(image_interval: Duration, port: u16) -> Result<()> {
    let (command_sender, mut command_receiver) = mpsc::unbounded_channel();
    let server = Server::new(
        port,
        Callbacks {
            on_command: Box::new(move |command| {
                command_sender.send(command).unwrap();
            }),
        },
    )
    .await?;
    let image_sender = server.image_sender();
    let odometry_sender = server.odometry_sender();
    let mut slam_core = Some(init_slam_core(
        image_sender,
        odometry_sender,
        image_interval,
    )?);
    loop {
        select! {
            command = command_receiver.recv() => {
                match command {
                    Some(Command::Reset) => {
                        println!("Resetting SLAM core...");
                        // Shutdown the old slam core
                        drop(slam_core.take());
                        let image_sender = server.image_sender();
                        let odometry_sender = server.odometry_sender();
                        slam_core = Some(init_slam_core(
                            image_sender,
                            odometry_sender,
                            image_interval,
                        )?);
                        println!("SLAM core reset!");
                    }
                    Some(Command::SaveStats(stats)) => {
                        println!("Saving statistics...");
                        save_stats(stats, Path::new("stats"))?;
                    }
                    None => {
                        break;
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                break;
            }

        }
    }
    println!("Exiting...");
    server.shutdown().await?;
    Ok(())
}

async fn record(image_interval: Duration, bag_dir: &Path) -> Result<()> {
    let mut recorder = Recorder::new(bag_dir)?;
    let (image_sender, mut image_receiver) = broadcast::channel(1);
    let (odometry_sender, mut odometry_receiver) = broadcast::channel(1);
    let _slam_core = init_slam_core(image_sender, odometry_sender, image_interval)?;
    loop {
        select! {
            res = image_receiver.recv() => {
                if let Ok(msg) = res {
                    recorder.feed_images(&msg)?;
                }
            }
            res = odometry_receiver.recv() => {
                if let Ok(msg) = res {
                    recorder.feed_odometry(&msg)?;
                }
            }
            _ = tokio::signal::ctrl_c() => {
                break;
            },
        }
    }
    Ok(())
}

async fn replay(port: u16, bag_dir: &Path, loop_: bool) -> Result<()> {
    let server = Server::new(
        port,
        Callbacks {
            on_command: Box::new(|command| {
                if let Command::SaveStats(stats) = command {
                    println!("Saving statistics...");
                    let _ = save_stats(stats, Path::new("stats"));
                }
            }),
        },
    )
    .await?;
    let image_sender = server.image_sender();
    let odometry_sender = server.odometry_sender();
    let ctrl_c = tokio::signal::ctrl_c();
    pin_mut!(ctrl_c);
    'outer: loop {
        let mut player = Player::new(bag_dir)?;
        loop {
            let Some(next_time) = player.poll_next_event_time() else {
                break;
            };
            select! {
                _ = sleep_until(tokio::time::Instant::from_std(next_time)) => {}
                _ = &mut ctrl_c => {
                    break 'outer;
                }
            }
            let Some(event) = player.next_event()? else {
                break;
            };
            match event {
                bag::Event::Odometry(msg) => {
                    odometry_sender.send(msg)?;
                }
                bag::Event::Images(msg) => {
                    image_sender.send(msg)?;
                }
            }
        }
        if !loop_ {
            break;
        }
    }
    Ok(())
}
