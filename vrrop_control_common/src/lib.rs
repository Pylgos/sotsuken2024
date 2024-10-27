use serde::{Deserialize, Serialize};

// pub mod control {
//     tonic::include_proto!("control"); // The string specified here must match the proto package name
// }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlMessage {
    SetTargetVelocity(SetTargetVelocity),
    SetLegLength(f32),
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
    pub forward: f32,
    pub turn: f32,
}
