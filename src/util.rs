use anyhow::anyhow;
use byte_unit::{Byte, UnitType};

pub trait TryIntoAdjustedByte {
    fn adjusted(self) -> anyhow::Result<byte_unit::AdjustedByte>;
}

pub trait IntoAdjustedByte {
    fn adjusted(self) -> byte_unit::AdjustedByte;
}

impl TryIntoAdjustedByte for f64 {
    fn adjusted(self) -> anyhow::Result<byte_unit::AdjustedByte> {
        Byte::from_f64(self)
            .ok_or(anyhow!("size too large"))
            .map(|b| b.get_appropriate_unit(UnitType::Binary))
    }
}

impl IntoAdjustedByte for u64 {
    fn adjusted(self) -> byte_unit::AdjustedByte {
        Byte::from_u64(self).get_appropriate_unit(UnitType::Binary)
    }
}
