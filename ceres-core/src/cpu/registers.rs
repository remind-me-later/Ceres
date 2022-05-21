use {
    super::{CF_B, HF_B, NF_B, ZF_B},
    crate::Gb,
};

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

    pub(super) fn cf(&self) -> bool {
        self.af & CF_B != 0
    }

    pub(super) fn set_zf(&mut self, zf: bool) {
        if zf {
            self.af |= ZF_B;
        } else {
            self.af &= !ZF_B;
        }
    }

    pub(super) fn set_nf(&mut self, nf: bool) {
        if nf {
            self.af |= NF_B;
        } else {
            self.af &= !NF_B;
        }
    }

    pub(super) fn set_hf(&mut self, hf: bool) {
        if hf {
            self.af |= HF_B;
        } else {
            self.af &= !HF_B;
        }
    }

    pub(super) fn set_cf(&mut self, cf: bool) {
        if cf {
            self.af |= CF_B;
        } else {
            self.af &= !CF_B;
        }
    }
}
