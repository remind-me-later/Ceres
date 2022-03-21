pub struct OamDma {
    is_active: bool,
    source: u8,
    address: u16,
    is_restarting: bool,
    t_cycles: i8,
}

impl OamDma {
    pub const fn new() -> Self {
        Self {
            is_active: false,
            source: 0xff,
            t_cycles: 0,
            address: 0x0000,
            is_restarting: false,
        }
    }

    pub const fn read(&self) -> u8 {
        self.source
    }

    pub fn write(&mut self, value: u8) {
        if self.is_active {
            self.is_restarting = true;
        }

        self.t_cycles = -8; // two m-cycles delay
        self.source = value;
        self.address = u16::from(value) << 8;
        self.is_active = true;
    }

    pub fn emulate(&mut self) -> Option<u16> {
        self.t_cycles = self.t_cycles.wrapping_add(4);

        if self.is_active && self.t_cycles >= 4 {
            self.t_cycles -= 4;
            let address = self.address;
            self.address = self.address.wrapping_add(1);
            if self.address & 0xff >= 0xa0 {
                self.is_active = false;
                self.is_restarting = false;
            }
            Some(address)
        } else {
            None
        }
    }

    pub const fn is_active(&self) -> bool {
        self.is_active && (self.t_cycles > 0 || self.is_restarting)
    }
}
