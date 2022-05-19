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
pub struct Imm;

impl Get for Imm {
    fn get(self, gb: &mut Gb) -> u8 {
        gb.imm8()
    }
}

#[derive(Clone, Copy)]
pub enum Indirect {
    BC,
    DE,
    HL,
    Immediate,
    HighC,
    HighImmediate,
}

impl Indirect {
    fn into_addr(self, gb: &mut Gb) -> u16 {
        match self {
            Indirect::BC => gb.bc,
            Indirect::DE => gb.de,
            Indirect::HL => gb.hl,
            Indirect::Immediate => gb.imm16(),
            Indirect::HighC => gb.bc | 0xff00,
            Indirect::HighImmediate => gb.imm8() as u16 | 0xff00,
        }
    }
}

impl Get for Indirect {
    fn get(self, gb: &mut Gb) -> u8 {
        let addr = self.into_addr(gb);
        gb.read_mem(addr)
    }
}

impl Set for Indirect {
    fn set(self, gb: &mut Gb, val: u8) {
        let addr = self.into_addr(gb);
        gb.write_mem(addr, val);
    }
}
