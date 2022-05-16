const ZF_FLAG: u8 = 0x80;
const NF_FLAG: u8 = 0x40;
const HF_FLAG: u8 = 0x20;
const CF_FLAG: u8 = 0x10;

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

#[derive(Default)]
pub struct Regs {
    pub pc: u16,
    pub sp: u16,
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
}

impl Regs {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(super) fn read16(&self, reg: Reg16) -> u16 {
        match reg {
            Reg16::AF => u16::from_be_bytes([self.a, self.f]),
            Reg16::BC => u16::from_be_bytes([self.b, self.c]),
            Reg16::DE => u16::from_be_bytes([self.d, self.e]),
            Reg16::HL => u16::from_be_bytes([self.h, self.l]),
            Reg16::SP => self.sp,
        }
    }

    pub(super) fn write16(&mut self, reg: Reg16, val: u16) {
        match reg {
            Reg16::AF => {
                let [hi, lo] = u16::to_be_bytes(val);
                self.a = hi;
                self.f = lo & 0xf0;
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

    pub(super) fn inc_pc(&mut self) {
        self.pc = self.pc.wrapping_add(1);
    }

    pub(super) fn dec_pc(&mut self) {
        self.pc = self.pc.wrapping_sub(1);
    }

    pub(super) fn inc_sp(&mut self) {
        self.sp = self.sp.wrapping_add(1);
    }

    pub(super) fn dec_sp(&mut self) {
        self.sp = self.sp.wrapping_sub(1);
    }

    pub(super) fn zf(&self) -> bool {
        self.f & ZF_FLAG != 0
    }

    pub(super) fn nf(&self) -> bool {
        self.f & NF_FLAG != 0
    }

    pub(super) fn hf(&self) -> bool {
        self.f & HF_FLAG != 0
    }

    pub(super) fn cf(&self) -> bool {
        self.f & CF_FLAG != 0
    }

    pub(super) fn set_zf(&mut self, zf: bool) {
        if zf {
            self.f |= ZF_FLAG;
        } else {
            self.f &= !ZF_FLAG;
        }
    }

    pub(super) fn set_nf(&mut self, nf: bool) {
        if nf {
            self.f |= NF_FLAG;
        } else {
            self.f &= !NF_FLAG;
        }
    }

    pub(super) fn set_hf(&mut self, hf: bool) {
        if hf {
            self.f |= HF_FLAG;
        } else {
            self.f &= !HF_FLAG;
        }
    }

    pub(super) fn set_cf(&mut self, cf: bool) {
        if cf {
            self.f |= CF_FLAG;
        } else {
            self.f &= !CF_FLAG;
        }
    }
}
