extern crate alloc;

use alloc::{format, string::String};

use super::{
    registers::{Register16, Register8},
    Cpu,
};
use crate::{cartridge::RumbleCallbacks, AudioCallbacks};

pub trait Dissasemble
where
    Self: Copy,
{
    fn dissasemble<A, R>(self, cpu: &mut Cpu<A, R>) -> String
    where
        A: AudioCallbacks,
        R: RumbleCallbacks;
}

pub trait Get<T: Copy>
where
    Self: Copy + Dissasemble,
{
    fn get<A, R>(self, cpu: &mut Cpu<A, R>) -> T
    where
        A: AudioCallbacks,
        R: RumbleCallbacks;
}

pub trait Set<T: Copy>
where
    Self: Copy + Dissasemble,
{
    fn set<A, R>(self, cpu: &mut Cpu<A, R>, val: T)
    where
        A: AudioCallbacks,
        R: RumbleCallbacks;
}

impl Get<u8> for Register8 {
    fn get<A, R>(self, cpu: &mut Cpu<A, R>) -> u8
    where
        A: AudioCallbacks,
        R: RumbleCallbacks,
    {
        cpu.registers.read8(self)
    }
}

impl Set<u8> for Register8 {
    fn set<A, R>(self, cpu: &mut Cpu<A, R>, val: u8)
    where
        A: AudioCallbacks,
        R: RumbleCallbacks,
    {
        cpu.registers.write8(self, val);
    }
}

impl Get<u16> for Register16 {
    fn get<A, R>(self, cpu: &mut Cpu<A, R>) -> u16
    where
        A: AudioCallbacks,
        R: RumbleCallbacks,
    {
        cpu.registers.read16(self)
    }
}

impl Set<u16> for Register16 {
    fn set<A, R>(self, cpu: &mut Cpu<A, R>, val: u16)
    where
        A: AudioCallbacks,
        R: RumbleCallbacks,
    {
        cpu.registers.write16(self, val);
    }
}

#[derive(Clone, Copy)]
pub struct Immediate;

impl Dissasemble for Immediate {
    fn dissasemble<A, R>(self, cpu: &mut Cpu<A, R>) -> String
    where
        A: AudioCallbacks,
        R: RumbleCallbacks,
    {
        let immediate = cpu.get_immediate_for_print();
        format!("${:x}", immediate)
    }
}

impl Get<u8> for Immediate {
    fn get<A, R>(self, cpu: &mut Cpu<A, R>) -> u8
    where
        A: AudioCallbacks,
        R: RumbleCallbacks,
    {
        cpu.read_immediate()
    }
}

impl Get<u16> for Immediate {
    fn get<A, R>(self, cpu: &mut Cpu<A, R>) -> u16
    where
        A: AudioCallbacks,
        R: RumbleCallbacks,
    {
        cpu.read_immediate16()
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

impl Dissasemble for Indirect {
    fn dissasemble<A, R>(self, cpu: &mut Cpu<A, R>) -> String
    where
        A: AudioCallbacks,
        R: RumbleCallbacks,
    {
        match self {
            Indirect::BC => format!("[bc]"),
            Indirect::DE => format!("[de]"),
            Indirect::HL => format!("[hl]"),
            Indirect::Immediate => {
                let immediate = cpu.get_immediate_for_print_16();
                format!("[${:x}]", immediate)
            }
            Indirect::HighC => {
                format!("[ff00 + c]")
            }
            Indirect::HighImmediate => {
                let immediate = cpu.get_immediate_for_print();
                format!("[ff00 + ${:x}]", immediate)
            }
        }
    }
}

impl Indirect {
    fn into_address<A, R>(self, cpu: &mut Cpu<A, R>) -> u16
    where
        A: AudioCallbacks,
        R: RumbleCallbacks,
    {
        match self {
            Indirect::BC => cpu.registers.read16(Register16::BC),
            Indirect::DE => cpu.registers.read16(Register16::DE),
            Indirect::HL => cpu.registers.read16(Register16::HL),
            Indirect::Immediate => cpu.read_immediate16(),
            Indirect::HighC => u16::from(cpu.registers.read8(Register8::C)) | 0xff00,
            Indirect::HighImmediate => u16::from(cpu.read_immediate()) | 0xff00,
        }
    }
}

impl Get<u8> for Indirect {
    fn get<A, R>(self, cpu: &mut Cpu<A, R>) -> u8
    where
        A: AudioCallbacks,
        R: RumbleCallbacks,
    {
        let address = self.into_address(cpu);
        cpu.memory.read(address)
    }
}

impl Set<u8> for Indirect {
    fn set<A, R>(self, cpu: &mut Cpu<A, R>, val: u8)
    where
        A: AudioCallbacks,
        R: RumbleCallbacks,
    {
        let address = self.into_address(cpu);
        cpu.memory.write(address, val);
    }
}

#[derive(Clone, Copy)]
pub struct IndirectIncreaseHL;

impl Dissasemble for IndirectIncreaseHL {
    fn dissasemble<A, R>(self, _cpu: &mut Cpu<A, R>) -> String
    where
        A: AudioCallbacks,
        R: RumbleCallbacks,
    {
        format!("[hl++]")
    }
}

impl Get<u8> for IndirectIncreaseHL {
    fn get<A, R>(self, cpu: &mut Cpu<A, R>) -> u8
    where
        A: AudioCallbacks,
        R: RumbleCallbacks,
    {
        let address = cpu.registers.read16(Register16::HL);
        let ret = cpu.memory.read(address);
        cpu.registers
            .write16(Register16::HL, address.wrapping_add(1));
        ret
    }
}

impl Set<u8> for IndirectIncreaseHL {
    fn set<A, R>(self, cpu: &mut Cpu<A, R>, val: u8)
    where
        A: AudioCallbacks,
        R: RumbleCallbacks,
    {
        let address = cpu.registers.read16(Register16::HL);
        cpu.memory.write(address, val);
        cpu.registers
            .write16(Register16::HL, address.wrapping_add(1));
    }
}

#[derive(Clone, Copy)]
pub struct IndirectDecreaseHL;

impl Dissasemble for IndirectDecreaseHL {
    fn dissasemble<A, R>(self, _cpu: &mut Cpu<A, R>) -> String
    where
        A: AudioCallbacks,
        R: RumbleCallbacks,
    {
        format!("[hl--]")
    }
}

impl Get<u8> for IndirectDecreaseHL {
    fn get<A, R>(self, cpu: &mut Cpu<A, R>) -> u8
    where
        A: AudioCallbacks,
        R: RumbleCallbacks,
    {
        let address = cpu.registers.read16(Register16::HL);
        let ret = cpu.memory.read(address);
        cpu.registers
            .write16(Register16::HL, address.wrapping_sub(1));
        ret
    }
}

impl Set<u8> for IndirectDecreaseHL {
    fn set<A, R>(self, cpu: &mut Cpu<A, R>, val: u8)
    where
        A: AudioCallbacks,
        R: RumbleCallbacks,
    {
        let address = cpu.registers.read16(Register16::HL);
        cpu.memory.write(address, val);
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

impl Dissasemble for JumpCondition {
    fn dissasemble<A, R>(self, _cpu: &mut Cpu<A, R>) -> String
    where
        A: AudioCallbacks,
        R: RumbleCallbacks,
    {
        match self {
            JumpCondition::Zero => format!("z"),
            JumpCondition::NotZero => format!("nz"),
            JumpCondition::Carry => format!("c"),
            JumpCondition::NotCarry => format!("c"),
        }
    }
}

impl JumpCondition {
    pub fn check<A, R>(self, cpu: &Cpu<A, R>) -> bool
    where
        A: AudioCallbacks,
        R: RumbleCallbacks,
    {
        use JumpCondition::{Carry, NotCarry, NotZero, Zero};
        match self {
            Zero => cpu.registers.zf(),
            NotZero => !cpu.registers.zf(),
            Carry => cpu.registers.cf(),
            NotCarry => !cpu.registers.cf(),
        }
    }
}
