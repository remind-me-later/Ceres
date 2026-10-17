//! The 8-bit arithmetic and logic operations.

use super::{CF, HF, NF, Sm83, ZF};

impl Sm83 {
    /// The ALU operation selected by bits 3-5 of `op`.
    pub(super) fn alu(&mut self, op: u8, val: u8) {
        match (op >> 3) & 7 {
            0 => self.add(val, false),
            1 => self.add(val, self.af & CF != 0),
            2 => self.sub(val, false),
            3 => self.sub(val, self.af & CF != 0),
            4 => self.and(val),
            5 => self.xor(val),
            6 => self.or(val),
            _ => self.cp(val),
        }
    }

    fn add(&mut self, val: u8, carry: bool) {
        let val = u16::from(val);
        let a = self.af >> 8;
        let carry = u16::from(carry);
        let res = a + val + carry;
        self.af = res << 8;
        if res.trailing_zeros() >= 8 {
            self.af |= ZF;
        }
        if (a & 0xF) + (val & 0xF) + carry > 0x0F {
            self.af |= HF;
        }
        if res > 0xFF {
            self.af |= CF;
        }
    }

    fn and(&mut self, val: u8) {
        let val = u16::from(val);
        let a = self.af >> 8;
        let a = a & val;
        self.af = (a << 8) | HF;
        if a == 0 {
            self.af |= ZF;
        }
    }

    const fn cp(&mut self, val: u8) {
        let a = self.a();
        self.af &= 0xFF00;
        self.af |= NF;
        if a == val {
            self.af |= ZF;
        }
        if a & 0xF < val & 0xF {
            self.af |= HF;
        }
        if a < val {
            self.af |= CF;
        }
    }

    fn or(&mut self, val: u8) {
        let val = u16::from(val);
        let a = self.af >> 8;
        self.af = (a | val) << 8;
        if a | val == 0 {
            self.af |= ZF;
        }
    }

    fn sub(&mut self, val: u8, carry: bool) {
        let val = u16::from(val);
        let a = self.af >> 8;
        let carry = u16::from(carry);
        let res = a.wrapping_sub(val).wrapping_sub(carry);
        self.af = (res << 8) | NF;
        if res.trailing_zeros() >= 8 {
            self.af |= ZF;
        }
        if (a & 0xF) < (val & 0xF) + carry {
            self.af |= HF;
        }
        if res > 0xFF {
            self.af |= CF;
        }
    }

    fn xor(&mut self, val: u8) {
        let val = u16::from(val);
        let a = self.af >> 8;
        let a = a ^ val;
        self.af = a << 8;
        if a == 0 {
            self.af |= ZF;
        }
    }
}
