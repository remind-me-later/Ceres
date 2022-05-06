// TODO: everything, dummy implementation
#[derive(Default)]
pub struct Serial {
    data: u8,
    control: u8,
}

impl Serial {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read_sb(&self) -> u8 {
        self.data
    }

    pub fn read_sc(&self) -> u8 {
        self.control | 0x7e
    }

    pub fn write_sb(&mut self, val: u8) {
        self.data = val
    }

    pub fn write_sc(&mut self, val: u8) {
        self.control = val
    }
}
