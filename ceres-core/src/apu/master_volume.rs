#[derive(Debug, Default)]
pub struct MasterVolume {
    left_vin: bool,
    left_volume: u8,
    right_vin: bool,
    right_volume: u8,
}

impl MasterVolume {
    pub const fn left_volume(&self) -> u8 {
        self.left_volume
    }

    #[must_use]
    pub fn read_nr50(&self) -> u8 {
        self.right_volume
            | (u8::from(self.right_vin) << 3)
            | (self.left_volume << 4)
            | (u8::from(self.left_vin) << 7)
    }

    pub const fn right_volume(&self) -> u8 {
        self.right_volume
    }

    pub const fn write_nr50(&mut self, val: u8) {
        self.right_volume = val & 7;
        self.right_vin = val & 8 != 0;
        self.left_volume = (val >> 4) & 7;
        self.left_vin = val & 0x80 != 0;
    }
}
