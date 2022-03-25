use bitflags::bitflags;

use crate::Model;

bitflags! {
    pub struct Flags: u8 {
        const ZERO = 0b_1000_0000;
        const SUBTRACT = 0b_0100_0000;
        const HALF_CARRY = 0b_0010_0000;
        const CARRY = 0b_0001_0000;
    }
}

#[derive(Clone, Copy)]
pub enum Register8 {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
}

#[derive(Clone, Copy)]
pub enum Register16 {
    AF,
    BC,
    DE,
    HL,
    PC,
    SP,
}

pub struct Registers {
    pub pc: u16,
    pub sp: u16,
    pub a: u8,
    pub f: Flags,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
}

impl Registers {
    pub const fn new(model: Model, boot_rom: bool) -> Self {
        let pc = if boot_rom { 0x0000 } else { 0x0100 };

        match model {
            Model::Dmg => Self {
                pc,
                sp: 0xfffe,
                a: 0x01,
                f: Flags::empty(),
                b: 0xff,
                c: 0x13,
                d: 0x00,
                e: 0xc1,
                h: 0x84,
                l: 0x03,
            },
            Model::Mgb => Self {
                pc,
                sp: 0xfffe,
                a: 0xff,
                f: Flags::from_bits_truncate(0xb0),
                b: 0x00,
                c: 0x13,
                d: 0x00,
                e: 0xd8,
                h: 0x01,
                l: 0x4d,
            },
            Model::Cgb => Self {
                pc,
                sp: 0xfffe,
                a: 0x11,
                f: Flags::from_bits_truncate(0x80),
                b: 0x00,
                c: 0x00,
                d: 0x00,
                e: 0x08,
                h: 0x00,
                l: 0x7c,
            },
        }
    }

    pub const fn read8(&self, register: Register8) -> u8 {
        match register {
            Register8::A => self.a,
            Register8::B => self.b,
            Register8::C => self.c,
            Register8::D => self.d,
            Register8::E => self.e,
            Register8::H => self.h,
            Register8::L => self.l,
        }
    }

    pub fn write8(&mut self, register: Register8, val: u8) {
        match register {
            Register8::A => self.a = val,
            Register8::B => self.b = val,
            Register8::C => self.c = val,
            Register8::D => self.d = val,
            Register8::E => self.e = val,
            Register8::H => self.h = val,
            Register8::L => self.l = val,
        }
    }

    pub const fn read16(&self, register: Register16) -> u16 {
        match register {
            Register16::AF => u16::from_be_bytes([self.a, self.f.bits()]),
            Register16::BC => u16::from_be_bytes([self.b, self.c]),
            Register16::DE => u16::from_be_bytes([self.d, self.e]),
            Register16::HL => u16::from_be_bytes([self.h, self.l]),
            Register16::PC => self.pc,
            Register16::SP => self.sp,
        }
    }

    pub fn write16(&mut self, register: Register16, val: u16) {
        match register {
            Register16::AF => {
                let [hi, lo] = u16::to_be_bytes(val);
                self.a = hi;
                self.f = Flags::from_bits_truncate(lo);
            }
            Register16::BC => {
                let [hi, lo] = u16::to_be_bytes(val);
                self.b = hi;
                self.c = lo;
            }
            Register16::DE => {
                let [hi, lo] = u16::to_be_bytes(val);
                self.d = hi;
                self.e = lo;
            }
            Register16::HL => {
                let [hi, lo] = u16::to_be_bytes(val);
                self.h = hi;
                self.l = lo;
            }
            Register16::PC => self.pc = val,
            Register16::SP => self.sp = val,
        }
    }

    pub fn increase_pc(&mut self) {
        self.pc = self.pc.wrapping_add(1);
    }

    pub fn decrease_pc(&mut self) {
        self.pc = self.pc.wrapping_sub(1);
    }

    pub fn increase_sp(&mut self) {
        self.sp = self.sp.wrapping_add(1);
    }

    pub fn decrease_sp(&mut self) {
        self.sp = self.sp.wrapping_sub(1);
    }

    // flags

    pub const fn zf(&self) -> bool {
        self.f.contains(Flags::ZERO)
    }

    pub const fn nf(&self) -> bool {
        self.f.contains(Flags::SUBTRACT)
    }

    pub const fn hf(&self) -> bool {
        self.f.contains(Flags::HALF_CARRY)
    }

    pub const fn cf(&self) -> bool {
        self.f.contains(Flags::CARRY)
    }

    pub fn set_zf(&mut self, zf: bool) {
        self.f.set(Flags::ZERO, zf);
    }

    pub fn set_nf(&mut self, nf: bool) {
        self.f.set(Flags::SUBTRACT, nf);
    }

    pub fn set_hf(&mut self, hf: bool) {
        self.f.set(Flags::HALF_CARRY, hf);
    }

    pub fn set_cf(&mut self, cf: bool) {
        self.f.set(Flags::CARRY, cf);
    }
}