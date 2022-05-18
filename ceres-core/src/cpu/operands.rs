use {
    super::registers::{Reg16, Reg8},
    crate::Gb,
};

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
            Reg8::A => gb.reg.a,
            Reg8::B => gb.reg.b,
            Reg8::C => gb.reg.c,
            Reg8::D => gb.reg.d,
            Reg8::E => gb.reg.e,
            Reg8::H => gb.reg.h,
            Reg8::L => gb.reg.l,
        }
    }
}

impl Set for Reg8 {
    fn set(self, gb: &mut Gb, val: u8) {
        match self {
            Reg8::A => gb.reg.a = val,
            Reg8::B => gb.reg.b = val,
            Reg8::C => gb.reg.c = val,
            Reg8::D => gb.reg.d = val,
            Reg8::E => gb.reg.e = val,
            Reg8::H => gb.reg.h = val,
            Reg8::L => gb.reg.l = val,
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
            Indirect::BC => gb.reg.read16(Reg16::BC),
            Indirect::DE => gb.reg.read16(Reg16::DE),
            Indirect::HL => gb.reg.read16(Reg16::HL),
            Indirect::Immediate => gb.imm16(),
            Indirect::HighC => u16::from(gb.reg.c) | 0xff00,
            Indirect::HighImmediate => u16::from(gb.imm8()) | 0xff00,
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
