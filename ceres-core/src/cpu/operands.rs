use {super::registers::Reg8, crate::Gb};

pub trait Get
where
    Self: Copy,
{
    fn get(self, gb: &mut Gb) -> u8;
}

pub trait Set
where
    Self: Copy,
{
    fn set(self, gb: &mut Gb, val: u8);
}

impl Get for Reg8 {
    fn get(self, gb: &mut Gb) -> u8 {
        match self {
            Reg8::A => (gb.af >> 8) as u8,
            Reg8::B => (gb.bc >> 8) as u8,
            Reg8::C => gb.bc as u8,
            Reg8::D => (gb.de >> 8) as u8,
            Reg8::E => gb.de as u8,
            Reg8::H => (gb.hl >> 8) as u8,
            Reg8::L => gb.hl as u8,
        }
    }
}

impl Set for Reg8 {
    fn set(self, gb: &mut Gb, val: u8) {
        match self {
            Reg8::A => {
                gb.af &= 0x00ff;
                gb.af |= (val as u16) << 8;
            }
            Reg8::B => {
                gb.bc &= 0x00ff;
                gb.bc |= (val as u16) << 8;
            }
            Reg8::C => {
                gb.bc &= 0xff00;
                gb.bc |= val as u16;
            }
            Reg8::D => {
                gb.de &= 0x00ff;
                gb.de |= (val as u16) << 8;
            }
            Reg8::E => {
                gb.de &= 0xff00;
                gb.de |= val as u16;
            }
            Reg8::H => {
                gb.hl &= 0x00ff;
                gb.hl |= (val as u16) << 8;
            }
            Reg8::L => {
                gb.hl &= 0xff00;
                gb.hl |= val as u16;
            }
        }
    }
}

#[derive(Clone, Copy)]
pub struct Dhl;

impl Get for Dhl {
    fn get(self, gb: &mut Gb) -> u8 {
        gb.read_mem(gb.hl)
    }
}

impl Set for Dhl {
    fn set(self, gb: &mut Gb, val: u8) {
        gb.write_mem(gb.hl, val);
    }
}
