use crate::Gb;

pub const ZF_FLAG: u16 = 0x80;
pub const NF_FLAG: u16 = 0x40;
pub const HF_FLAG: u16 = 0x20;
pub const CF_FLAG: u16 = 0x10;

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

#[derive(Default)]
pub struct Regs {}

impl Gb {
    pub(crate) fn a(&self) -> u8 {
        (self.af >> 8) as u8
    }

    pub(crate) fn set_a(&mut self, val: u8) {
        self.af &= 0x00ff;
        self.af |= (val as u16) << 8;
    }

    pub(super) fn rid16(&mut self, id: u8) -> &mut u16 {
        match id {
            0 => &mut self.af,
            1 => &mut self.bc,
            2 => &mut self.de,
            3 => &mut self.hl,
            4 => &mut self.sp,
            _ => unreachable!(),
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

    pub(super) fn hf(&self) -> bool {
        self.af & HF_FLAG != 0
    }

    pub(super) fn cf(&self) -> bool {
        self.af & CF_FLAG != 0
    }

    pub(super) fn set_zf(&mut self, zf: bool) {
        if zf {
            self.af |= ZF_FLAG;
        } else {
            self.af &= !ZF_FLAG;
        }
    }

    pub(super) fn set_nf(&mut self, nf: bool) {
        if nf {
            self.af |= NF_FLAG;
        } else {
            self.af &= !NF_FLAG;
        }
    }

    pub(super) fn set_hf(&mut self, hf: bool) {
        if hf {
            self.af |= HF_FLAG;
        } else {
            self.af &= !HF_FLAG;
        }
    }

    pub(super) fn set_cf(&mut self, cf: bool) {
        if cf {
            self.af |= CF_FLAG;
        } else {
            self.af &= !CF_FLAG;
        }
    }
}
