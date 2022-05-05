use bitflags::bitflags;

bitflags! {
    pub struct Flags: u8 {
        const ZERO = 0b_1000_0000;
        const SUBTRACT = 0b_0100_0000;
        const HALF_CARRY = 0b_0010_0000;
        const CARRY = 0b_0001_0000;
    }
}

#[derive(Clone, Copy)]
pub enum Reg8 {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
}

#[derive(Clone, Copy)]
pub enum Reg16 {
    AF,
    BC,
    DE,
    HL,
    SP,
}

pub struct Regs {
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

impl Default for Regs {
    fn default() -> Self {
        Self {
            pc: 0,
            sp: 0,
            a: 0,
            f: Flags::empty(),
            b: 0,
            c: 0,
            d: 0,
            e: 0,
            h: 0,
            l: 0,
        }
    }
}

impl Regs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read16(&self, register: Reg16) -> u16 {
        match register {
            Reg16::AF => u16::from_be_bytes([self.a, self.f.bits()]),
            Reg16::BC => u16::from_be_bytes([self.b, self.c]),
            Reg16::DE => u16::from_be_bytes([self.d, self.e]),
            Reg16::HL => u16::from_be_bytes([self.h, self.l]),
            Reg16::SP => self.sp,
        }
    }

    pub fn write16(&mut self, register: Reg16, val: u16) {
        match register {
            Reg16::AF => {
                let [hi, lo] = u16::to_be_bytes(val);
                self.a = hi;
                self.f = Flags::from_bits_truncate(lo);
            }
            Reg16::BC => {
                let [hi, lo] = u16::to_be_bytes(val);
                self.b = hi;
                self.c = lo;
            }
            Reg16::DE => {
                let [hi, lo] = u16::to_be_bytes(val);
                self.d = hi;
                self.e = lo;
            }
            Reg16::HL => {
                let [hi, lo] = u16::to_be_bytes(val);
                self.h = hi;
                self.l = lo;
            }
            Reg16::SP => self.sp = val,
        }
    }

    pub fn inc_pc(&mut self) {
        self.pc = self.pc.wrapping_add(1);
    }

    pub fn dec_pc(&mut self) {
        self.pc = self.pc.wrapping_sub(1);
    }

    pub fn inc_sp(&mut self) {
        self.sp = self.sp.wrapping_add(1);
    }

    pub fn dec_sp(&mut self) {
        self.sp = self.sp.wrapping_sub(1);
    }

    pub fn zf(&self) -> bool {
        self.f.contains(Flags::ZERO)
    }

    pub fn nf(&self) -> bool {
        self.f.contains(Flags::SUBTRACT)
    }

    pub fn hf(&self) -> bool {
        self.f.contains(Flags::HALF_CARRY)
    }

    pub fn cf(&self) -> bool {
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
