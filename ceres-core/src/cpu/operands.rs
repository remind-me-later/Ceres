use super::{
    registers::{Reg16, Reg8},
    Cpu,
};

pub trait Get<T: Copy>
where
    Self: Copy,
{
    fn get(self, cpu: &mut Cpu) -> T;
}

pub trait Set<T: Copy>
where
    Self: Copy,
{
    fn set(self, cpu: &mut Cpu, val: T);
}

impl Get<u8> for Reg8 {
    fn get(self, cpu: &mut Cpu) -> u8 {
        match self {
            Reg8::A => cpu.reg.a,
            Reg8::B => cpu.reg.b,
            Reg8::C => cpu.reg.c,
            Reg8::D => cpu.reg.d,
            Reg8::E => cpu.reg.e,
            Reg8::H => cpu.reg.h,
            Reg8::L => cpu.reg.l,
        }
    }
}

impl Set<u8> for Reg8 {
    fn set(self, cpu: &mut Cpu, val: u8) {
        match self {
            Reg8::A => cpu.reg.a = val,
            Reg8::B => cpu.reg.b = val,
            Reg8::C => cpu.reg.c = val,
            Reg8::D => cpu.reg.d = val,
            Reg8::E => cpu.reg.e = val,
            Reg8::H => cpu.reg.h = val,
            Reg8::L => cpu.reg.l = val,
        }
    }
}

impl Get<u16> for Reg16 {
    fn get(self, cpu: &mut Cpu) -> u16 {
        cpu.reg.read16(self)
    }
}

impl Set<u16> for Reg16 {
    fn set(self, cpu: &mut Cpu, val: u16) {
        cpu.reg.write16(self, val);
    }
}

#[derive(Clone, Copy)]
pub struct Imm;

impl Get<u8> for Imm {
    fn get(self, cpu: &mut Cpu) -> u8 {
        cpu.imm8()
    }
}

impl Get<u16> for Imm {
    fn get(self, cpu: &mut Cpu) -> u16 {
        cpu.imm16()
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
    fn into_addr(self, cpu: &mut Cpu) -> u16 {
        match self {
            Indirect::BC => cpu.reg.read16(Reg16::BC),
            Indirect::DE => cpu.reg.read16(Reg16::DE),
            Indirect::HL => cpu.reg.read16(Reg16::HL),
            Indirect::Immediate => cpu.imm16(),
            Indirect::HighC => u16::from(cpu.reg.c) | 0xff00,
            Indirect::HighImmediate => u16::from(cpu.imm8()) | 0xff00,
        }
    }
}

impl Get<u8> for Indirect {
    fn get(self, cpu: &mut Cpu) -> u8 {
        let addr = self.into_addr(cpu);
        cpu.mem.read(addr)
    }
}

impl Set<u8> for Indirect {
    fn set(self, cpu: &mut Cpu, val: u8) {
        let addr = self.into_addr(cpu);
        cpu.mem.write(addr, val);
    }
}

#[derive(Clone, Copy)]
pub struct IndirectIncreaseHL;

impl Get<u8> for IndirectIncreaseHL {
    fn get(self, cpu: &mut Cpu) -> u8 {
        let addr = cpu.reg.read16(Reg16::HL);
        let ret = cpu.mem.read(addr);
        cpu.reg.write16(Reg16::HL, addr.wrapping_add(1));
        ret
    }
}

impl Set<u8> for IndirectIncreaseHL {
    fn set(self, cpu: &mut Cpu, val: u8) {
        let addr = cpu.reg.read16(Reg16::HL);
        cpu.mem.write(addr, val);
        cpu.reg.write16(Reg16::HL, addr.wrapping_add(1));
    }
}

#[derive(Clone, Copy)]
pub struct IndirectDecreaseHL;

impl Get<u8> for IndirectDecreaseHL {
    fn get(self, cpu: &mut Cpu) -> u8 {
        let addr = cpu.reg.read16(Reg16::HL);
        let ret = cpu.mem.read(addr);
        cpu.reg.write16(Reg16::HL, addr.wrapping_sub(1));
        ret
    }
}

impl Set<u8> for IndirectDecreaseHL {
    fn set(self, cpu: &mut Cpu, val: u8) {
        let addr = cpu.reg.read16(Reg16::HL);
        cpu.mem.write(addr, val);
        cpu.reg.write16(Reg16::HL, addr.wrapping_sub(1));
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
    pub fn check(self, cpu: &Cpu) -> bool {
        use JumpCondition::{Carry, NotCarry, NotZero, Zero};
        match self {
            Zero => cpu.reg.zf(),
            NotZero => !cpu.reg.zf(),
            Carry => cpu.reg.cf(),
            NotCarry => !cpu.reg.cf(),
        }
    }
}
