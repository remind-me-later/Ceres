use super::{
    registers::{Register16, Register8},
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

impl Get<u8> for Register8 {
    fn get(self, cpu: &mut Cpu) -> u8 {
        match self {
            Register8::A => cpu.registers.a,
            Register8::B => cpu.registers.b,
            Register8::C => cpu.registers.c,
            Register8::D => cpu.registers.d,
            Register8::E => cpu.registers.e,
            Register8::H => cpu.registers.h,
            Register8::L => cpu.registers.l,
        }
    }
}

impl Set<u8> for Register8 {
    fn set(self, cpu: &mut Cpu, val: u8) {
        match self {
            Register8::A => cpu.registers.a = val,
            Register8::B => cpu.registers.b = val,
            Register8::C => cpu.registers.c = val,
            Register8::D => cpu.registers.d = val,
            Register8::E => cpu.registers.e = val,
            Register8::H => cpu.registers.h = val,
            Register8::L => cpu.registers.l = val,
        }
    }
}

impl Get<u16> for Register16 {
    fn get(self, cpu: &mut Cpu) -> u16 {
        cpu.registers.read16(self)
    }
}

impl Set<u16> for Register16 {
    fn set(self, cpu: &mut Cpu, val: u16) {
        cpu.registers.write16(self, val);
    }
}

#[derive(Clone, Copy)]
pub struct Immediate;

impl Get<u8> for Immediate {
    fn get(self, cpu: &mut Cpu) -> u8 {
        cpu.imm8()
    }
}

impl Get<u16> for Immediate {
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
    fn into_address(self, cpu: &mut Cpu) -> u16 {
        match self {
            Indirect::BC => cpu.registers.read16(Register16::BC),
            Indirect::DE => cpu.registers.read16(Register16::DE),
            Indirect::HL => cpu.registers.read16(Register16::HL),
            Indirect::Immediate => cpu.imm16(),
            Indirect::HighC => u16::from(cpu.registers.c) | 0xff00,
            Indirect::HighImmediate => u16::from(cpu.imm8()) | 0xff00,
        }
    }
}

impl Get<u8> for Indirect {
    fn get(self, cpu: &mut Cpu) -> u8 {
        let address = self.into_address(cpu);
        cpu.mem.read(address)
    }
}

impl Set<u8> for Indirect {
    fn set(self, cpu: &mut Cpu, val: u8) {
        let address = self.into_address(cpu);
        cpu.mem.write(address, val);
    }
}

#[derive(Clone, Copy)]
pub struct IndirectIncreaseHL;

impl Get<u8> for IndirectIncreaseHL {
    fn get(self, cpu: &mut Cpu) -> u8 {
        let address = cpu.registers.read16(Register16::HL);
        let ret = cpu.mem.read(address);
        cpu.registers
            .write16(Register16::HL, address.wrapping_add(1));
        ret
    }
}

impl Set<u8> for IndirectIncreaseHL {
    fn set(self, cpu: &mut Cpu, val: u8) {
        let address = cpu.registers.read16(Register16::HL);
        cpu.mem.write(address, val);
        cpu.registers
            .write16(Register16::HL, address.wrapping_add(1));
    }
}

#[derive(Clone, Copy)]
pub struct IndirectDecreaseHL;

impl Get<u8> for IndirectDecreaseHL {
    fn get(self, cpu: &mut Cpu) -> u8 {
        let address = cpu.registers.read16(Register16::HL);
        let ret = cpu.mem.read(address);
        cpu.registers
            .write16(Register16::HL, address.wrapping_sub(1));
        ret
    }
}

impl Set<u8> for IndirectDecreaseHL {
    fn set(self, cpu: &mut Cpu, val: u8) {
        let address = cpu.registers.read16(Register16::HL);
        cpu.mem.write(address, val);
        cpu.registers
            .write16(Register16::HL, address.wrapping_sub(1));
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
            Zero => cpu.registers.zf(),
            NotZero => !cpu.registers.zf(),
            Carry => cpu.registers.cf(),
            NotCarry => !cpu.registers.cf(),
        }
    }
}
