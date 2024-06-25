use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OdometryMessage {
    pub stamp: std::time::SystemTime,
    pub translation: [f64; 3],
    pub rotation: [f64; 4],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImagesMessage {
    pub stamp: std::time::SystemTime,
    pub color_image: Vec<u8>,
    pub color_intrinsics: CameraIntrinsics,
    pub depth_image: Vec<u8>,
    pub depth_intrinsics: CameraIntrinsics,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct CameraIntrinsics {
    pub width: u32,
    pub height: u32,
    pub fx: f64,
    pub fy: f64,
    pub cx: f64,
    pub cy: f64,
}

