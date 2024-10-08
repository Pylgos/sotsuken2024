use serde::{Deserialize, Serialize};

// pub mod control {
//     tonic::include_proto!("control"); // The string specified here must match the proto package name
// }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlMessage {
    SetTargetVelocity(SetTargetVelocity),
}

impl ControlMessage {
    pub fn serialize(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }

    pub fn deserialize(data: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(data)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetTargetVelocity {
    pub vx: f32,
    pub vy: f32,
    pub vtheta: f32,
}
