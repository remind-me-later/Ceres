use {
    super::registers::{Reg16, Reg8},
    crate::Gb,
};

pub trait Get<T: Copy>
where
    Self: Copy,
{
    fn get(self, gb: &mut Gb) -> T;
}

pub trait Set<T: Copy>
where
    Self: Copy,
{
    fn set(self, gb: &mut Gb, val: T);
}

impl Get<u8> for Reg8 {
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

impl Set<u8> for Reg8 {
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

impl Get<u16> for Reg16 {
    fn get(self, gb: &mut Gb) -> u16 {
        gb.reg.read16(self)
    }
}

impl Set<u16> for Reg16 {
    fn set(self, gb: &mut Gb, val: u16) {
        gb.reg.write16(self, val);
    }
}

#[derive(Clone, Copy)]
pub struct Imm;

impl Get<u8> for Imm {
    fn get(self, gb: &mut Gb) -> u8 {
        gb.imm8()
    }
}

impl Get<u16> for Imm {
    fn get(self, gb: &mut Gb) -> u16 {
        gb.imm16()
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

impl Get<u8> for Indirect {
    fn get(self, gb: &mut Gb) -> u8 {
        let addr = self.into_addr(gb);
        gb.read_mem(addr)
    }
}

impl Set<u8> for Indirect {
    fn set(self, gb: &mut Gb, val: u8) {
        let addr = self.into_addr(gb);
        gb.write_mem(addr, val);
    }
}

#[derive(Clone, Copy)]
pub struct IndirectIncreaseHL;

impl Get<u8> for IndirectIncreaseHL {
    fn get(self, gb: &mut Gb) -> u8 {
        let addr = gb.reg.read16(Reg16::HL);
        let ret = gb.read_mem(addr);
        gb.reg.write16(Reg16::HL, addr.wrapping_add(1));
        ret
    }
}

impl Set<u8> for IndirectIncreaseHL {
    fn set(self, gb: &mut Gb, val: u8) {
        let addr = gb.reg.read16(Reg16::HL);
        gb.write_mem(addr, val);
        gb.reg.write16(Reg16::HL, addr.wrapping_add(1));
    }
}

#[derive(Clone, Copy)]
pub struct IndirectDecreaseHL;

impl Get<u8> for IndirectDecreaseHL {
    fn get(self, gb: &mut Gb) -> u8 {
        let addr = gb.reg.read16(Reg16::HL);
        let ret = gb.read_mem(addr);
        gb.reg.write16(Reg16::HL, addr.wrapping_sub(1));
        ret
    }
}

impl Set<u8> for IndirectDecreaseHL {
    fn set(self, gb: &mut Gb, val: u8) {
        let addr = gb.reg.read16(Reg16::HL);
        gb.write_mem(addr, val);
        gb.reg.write16(Reg16::HL, addr.wrapping_sub(1));
    }
}

#[derive(Clone, Copy)]
pub enum JumpCondition {
    Zero,
    NotZero,
    Carry,
    NotCarry,
}

impl JumpCondition {
    pub(super) fn check(self, gb: &Gb) -> bool {
        use JumpCondition::{Carry, NotCarry, NotZero, Zero};
        match self {
            Zero => gb.reg.zf(),
            NotZero => !gb.reg.zf(),
            Carry => gb.reg.cf(),
            NotCarry => !gb.reg.cf(),
        }
    }
}
