#[derive(Clone, Copy)]
pub enum WaveDuty {
    HalfQuarter,
    Quarter,
    Half,
    ThreeQuarters,
}

impl WaveDuty {
    pub const fn duty_byte(self) -> u8 {
        use self::WaveDuty::{Half, HalfQuarter, Quarter, ThreeQuarters};

        match self {
            HalfQuarter => 0b0000_0001,
            Quarter => 0b1000_0001,
            Half => 0b1000_0111,
            ThreeQuarters => 0b0111_1110,
        }
    }
}

impl From<u8> for WaveDuty {
    fn from(val: u8) -> Self {
        use self::WaveDuty::{Half, HalfQuarter, Quarter, ThreeQuarters};
        // bits 7-6
        match (val >> 6) & 3 {
            0 => HalfQuarter,
            1 => Quarter,
            2 => Half,
            _ => ThreeQuarters,
        }
    }
}

impl From<WaveDuty> for u8 {
    fn from(val: WaveDuty) -> Self {
        use self::WaveDuty::{Half, HalfQuarter, Quarter, ThreeQuarters};

        const WAVEDUTY_MASK: u8 = 0x3f;

        let byte = match val {
            HalfQuarter => 0,
            Quarter => 1,
            Half => 2,
            ThreeQuarters => 3,
        };
        // bits 7-6
        (byte << 6) | WAVEDUTY_MASK
    }
}
