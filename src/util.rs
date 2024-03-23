use anyhow::anyhow;
use byte_unit::{Byte, UnitType};

pub trait TryIntoAdjustedByte {
    fn to_bytes(self) -> anyhow::Result<byte_unit::AdjustedByte>;
}

pub trait IntoAdjustedByte {
    fn to_bytes(self) -> byte_unit::AdjustedByte;
}

impl TryIntoAdjustedByte for f64 {
    fn to_bytes(self) -> anyhow::Result<byte_unit::AdjustedByte> {
        Byte::from_f64(self)
            .ok_or(anyhow!("size too large"))
            .map(|b| b.get_appropriate_unit(UnitType::Binary))
    }
}

impl IntoAdjustedByte for u64 {
    fn to_bytes(self) -> byte_unit::AdjustedByte {
        Byte::from_u64(self).get_appropriate_unit(UnitType::Binary)
    }
}
