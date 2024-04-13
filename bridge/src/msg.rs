use packed_struct::prelude::*;

#[derive(PackedStruct, Debug, Clone, PartialEq, Eq)]
#[packed_struct(endian="lsb", bit_numbering="msb0")]
pub struct Message {
    /// 前後の移動速度 [mm/s]
    #[packed_field(bits="0..16")]
    pub forward_vel: i16,
    /// 半時計回りの回転速度 [mrad/s]
    #[packed_field(bits="16..32")]
    pub turn_vel: i16, 
}
