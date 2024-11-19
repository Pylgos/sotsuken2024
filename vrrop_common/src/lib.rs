use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OdometryMessage {
    pub stamp: std::time::SystemTime,
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImagesMessage {
    pub odometry: OdometryMessage,
    pub color_image: Vec<u8>,
    pub color_intrinsics: CameraIntrinsics,
    pub depth_image: Vec<u8>,
    pub depth_intrinsics: CameraIntrinsics,
    pub depth_unit: f32,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct CameraIntrinsics {
    pub width: u32,
    pub height: u32,
    pub fx: f32,
    pub fy: f32,
    pub cx: f32,
    pub cy: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    Reset,
    SaveStats(Stats),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stats {
    pub images_stamps: Vec<std::time::SystemTime>,
    pub images_original_sizes: Vec<usize>,
    pub images_latencies: Vec<std::time::Duration>,
    pub odometry_stamps: Vec<std::time::SystemTime>,
    pub odometry_original_sizes: Vec<usize>,
    pub odometry_latencies: Vec<std::time::Duration>,
}
