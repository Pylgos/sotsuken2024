use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Instant, SystemTime},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{CameraIntrinsics, ImagesMessage, OdometryMessage};

#[derive(Serialize, Deserialize, Clone)]
enum Entry {
    Odometry(OdometryEntry),
    Images(ImagesEntry),
}

impl Entry {
    fn stamp(&self) -> SystemTime {
        match self {
            Self::Odometry(event) => event.message.stamp,
            Self::Images(event) => event.odometry.stamp,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct OdometryEntry {
    message: OdometryMessage,
}

#[derive(Serialize, Deserialize, Clone)]
struct ImagesEntry {
    odometry: OdometryMessage,
    color_intrinsics: CameraIntrinsics,
    color_image_path: PathBuf,
    depth_intrinsics: CameraIntrinsics,
    depth_image_path: PathBuf,
    depth_unit: f32,
}

#[derive(Debug)]
pub struct Recorder {
    dest_dir: PathBuf,
    entries_file: File,
}

impl Recorder {
    pub fn new(dest_dir: impl AsRef<Path>) -> Result<Self> {
        let dest_dir = dest_dir.as_ref().to_path_buf();
        fs::create_dir_all(&dest_dir)?;
        let entries_file = fs::File::create(dest_dir.join("entries.jsonl"))?;
        fs::create_dir(dest_dir.join("images"))?;
        Ok(Self {
            dest_dir,
            entries_file,
        })
    }

    pub fn feed_images(&mut self, msg: &ImagesMessage) -> Result<()> {
        let img_dir = Path::new("images");
        let stamp = msg
            .odometry
            .stamp
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis();
        let color_image_path = img_dir.join(format!("color_{stamp}.jpg"));
        let depth_image_path = img_dir.join(format!("depth_{stamp}.png"));
        fs::File::create(self.dest_dir.join(&color_image_path))?.write_all(&msg.color_image)?;
        fs::File::create(self.dest_dir.join(&depth_image_path))?.write_all(&msg.depth_image)?;
        let entry = ImagesEntry {
            odometry: msg.odometry.clone(),
            color_intrinsics: msg.color_intrinsics,
            color_image_path,
            depth_intrinsics: msg.depth_intrinsics,
            depth_image_path,
            depth_unit: msg.depth_unit,
        };
        let mut serialized = serde_json::to_string(&Entry::Images(entry))?;
        serialized.push('\n');
        self.entries_file.write_all(serialized.as_bytes())?;
        Ok(())
    }

    pub fn feed_odometry(&mut self, msg: &OdometryMessage) -> Result<()> {
        let event = OdometryEntry {
            message: msg.clone(),
        };
        let mut serialized = serde_json::to_string(&Entry::Odometry(event))?;
        serialized.push('\n');
        self.entries_file.write_all(serialized.as_bytes())?;
        Ok(())
    }
}

pub enum Event {
    Odometry(OdometryMessage),
    Images(ImagesMessage),
}

pub struct Player {
    entries: Vec<Entry>,
    idx: usize,
    first_stamp: SystemTime,
    start_instant: Instant,
    start_time: SystemTime,
    bag_dir: PathBuf,
}

impl Player {
    pub fn new(src_dir: impl AsRef<Path>) -> Result<Self> {
        let bag_dir = src_dir.as_ref().to_path_buf();
        let entries_str = fs::read_to_string(bag_dir.join("entries.jsonl"))?;
        let entries = entries_str
            .lines()
            .map(|line| serde_json::from_str(line).map_err(Into::into))
            .collect::<Result<Vec<Entry>>>()?;
        let first_stamp = entries
            .first()
            .ok_or_else(|| anyhow::anyhow!("No entry found"))?
            .stamp();
        Ok(Self {
            entries,
            idx: 0,
            first_stamp,
            start_instant: Instant::now(),
            start_time: SystemTime::now(),
            bag_dir,
        })
    }

    pub fn poll_next_event_time(&self) -> Option<Instant> {
        let event = self.next_entry()?;
        Some(self.start_instant + (event.stamp().duration_since(self.first_stamp).unwrap()))
    }

    pub fn next_event(&mut self) -> Result<Option<Event>> {
        let Some(entry) = self.next_entry() else {
            return Ok(None);
        };
        self.idx += 1;
        match entry {
            Entry::Odometry(mut entry) => {
                entry.message.stamp = self.start_time
                    + (entry
                        .message
                        .stamp
                        .duration_since(self.first_stamp)
                        .unwrap());
                Ok(Some(Event::Odometry(entry.message.clone())))
            }
            Entry::Images(mut entry) => {
                entry.odometry.stamp = self.start_time
                    + (entry
                        .odometry
                        .stamp
                        .duration_since(self.first_stamp)
                        .unwrap());
                let msg = ImagesMessage {
                    odometry: entry.odometry.clone(),
                    color_image: fs::read(self.bag_dir.join(entry.color_image_path))?,
                    color_intrinsics: entry.color_intrinsics,
                    depth_image: fs::read(self.bag_dir.join(entry.depth_image_path))?,
                    depth_intrinsics: entry.depth_intrinsics,
                    depth_unit: entry.depth_unit,
                };
                Ok(Some(Event::Images(msg)))
            }
        }
    }

    fn next_entry(&self) -> Option<Entry> {
        self.entries.get(self.idx).cloned()
    }
}
