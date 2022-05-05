#[derive(Default)]
pub struct Dma {
    is_active: bool,
    source: u8,
    addr: u16,
    is_restarting: bool,
    t_cycles: i8,
}

impl Dma {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read(&self) -> u8 {
        self.source
    }

    pub fn write(&mut self, value: u8) {
        if self.is_active {
            self.is_restarting = true;
        }

        self.t_cycles = -8; // two m-cycles delay
        self.source = value;
        self.addr = u16::from(value) << 8;
        self.is_active = true;
    }

    pub fn emulate(&mut self) -> Option<u16> {
        self.t_cycles = self.t_cycles.wrapping_add(4);

        if self.is_active && self.t_cycles >= 4 {
            self.t_cycles -= 4;
            let addr = self.addr;
            self.addr = self.addr.wrapping_add(1);
            if self.addr & 0xff >= 0xa0 {
                self.is_active = false;
                self.is_restarting = false;
            }
            Some(addr)
        } else {
            None
        }
    }

    pub fn is_active(&self) -> bool {
        self.is_active && (self.t_cycles > 0 || self.is_restarting)
    }
}
