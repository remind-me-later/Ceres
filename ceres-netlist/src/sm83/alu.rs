//! The SM83's ALU and flags (`hdl/rtl/sm83_alu.vhd`). Operand 1 goes
//! through a decoder first (SET's and RES's masks, DAA's corrections);
//! operand 2 can be inverted. Bit by bit, generate and propagate give AND,
//! OR, XOR and, with the carry chain, the sum; the shifter rotates, shifts
//! and swaps operand 2. The flags are latched like the registers: taken in
//! `write_phase`, shown when their write enables fall.
//!
//! @hw die: dmg-schematics, categories alu, alu-core, alu-dec, alu-daa and
//! alu-flag

use super::{control::Controls, decoder::Steps};

/// Z, N, H and C.
#[derive(Clone, Copy, PartialEq, Eq)]
#[expect(clippy::struct_excessive_bools, reason = "the flags")]
pub struct Flags {
    pub z: bool,
    pub n: bool,
    pub h: bool,
    pub c: bool,
}

impl Flags {
    /// The flags as F's bits 7 to 4.
    #[must_use]
    pub const fn bits(self) -> u8 {
        (self.z as u8) << 7 | (self.n as u8) << 6 | (self.h as u8) << 5 | (self.c as u8) << 4
    }

    /// The flags of F's bits 7 to 4.
    #[must_use]
    pub const fn from_bits(f: u8) -> Self {
        Self {
            z: f & 0x80 != 0,
            n: f & 0x40 != 0,
            h: f & 0x20 != 0,
            c: f & 0x10 != 0,
        }
    }
}

/// The ALU's latches: the flags, the failed condition, Z's sign.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct NetAlu {
    /// The flags as they show, and as their masters took them.
    pub flags: Flags,
    pub masters: Flags,
    /// The condition of JR, JP, CALL or RET cc failed: found in
    /// `write_phase`, kept into phase A, cleared from B.
    pub cond_fail: bool,
    /// Z's sign, taken in `write_phase` and shown in the next M-cycle.
    pub z7_master: bool,
    pub z7_delayed: bool,
}

/// What the ALU computes in a phase.
pub struct AluResult {
    pub res: u8,
    /// The carries out of each bit.
    pub carry: u8,
    /// The new flags, in `write_phase`.
    pub new: Flags,
}

/// What the ALU takes in besides the decoders' lines.
#[derive(Clone, Copy)]
pub struct AluInputs {
    pub op: u8,
    pub op1: u8,
    pub op2: u8,
    /// A, for DAA; Z, for POP AF's flags.
    pub reg_a: u8,
    pub reg_z: u8,
    pub exec_phase: bool,
    pub write_phase: bool,
}

impl NetAlu {
    /// The flags start at 1, as in dmg-sim (the die keeps them inverted).
    pub const POWER_ON: Self = Self {
        flags: Flags::from_bits(0xF0),
        masters: Flags::from_bits(0xF0),
        cond_fail: false,
        z7_master: false,
        z7_delayed: false,
    };

    /// The result and the new flags.
    #[must_use]
    #[expect(clippy::many_single_char_names, reason = "the netlist's bits")]
    #[expect(clippy::similar_names, reason = "the netlist's names")]
    pub fn compute(&self, s: &Steps, c: &Controls, i: AluInputs) -> AluResult {
        let o = |bit: u32| i.op >> bit & 1 != 0;
        let a = |bit: u32| i.reg_a >> bit & 1 != 0;
        let f = self.flags;
        let exec = i.exec_phase;

        // DAA's comparisons of A's nibbles.
        // @hw die: REG_OA1, REG_OA2, REG_AND
        let daa_lo_gt9 = (a(1) | a(2)) & a(3);
        let daa_hi_gt9 = (a(5) | a(6)) & a(7);
        let daa_hi_ge9 = a(7) & a(4);

        // Operand 1, decoded: SET ORs its bit in, RES ANDs the others in,
        // DAA adds 06, 60, FA, A0 or 9A.
        // @hw die: ALU_DECODER, ALU_NOT1 to ALU_NOT8
        let bit_set = 1_u8 << (i.op >> 3 & 7);
        let high = !f.n & (daa_hi_gt9 | f.c | (daa_hi_ge9 & daa_lo_gt9));
        let daa = u8::from(f.h | (daa_lo_gt9 & !f.n)) << 1
            | u8::from(!f.n & (f.h | daa_lo_gt9)) << 2
            | u8::from(f.h & f.n) << 3
            | u8::from(f.h & f.n) << 4
            | u8::from((f.n & (f.h ^ f.c)) | high) << 5
            | u8::from((f.h & !f.c & f.n) | high) << 6
            | u8::from(f.n & (f.h | f.c)) << 7;
        let mask = |on: bool| if on { 0xFF } else { 0 };
        let op1dec = mask(exec)
            & (i.op1
                | (mask(c.set_exec) & bit_set)
                | (mask(c.res_exec) & !bit_set)
                | (mask(c.daa_m0) & daa));

        // The carry into bit 0.
        // @hw die: ALU_DECODER (y14), ALU_NOT10
        let carry_in = exec
            & (c.cp
                | (!o(0) & s.inc_or_dec_m)
                | (c.add_sp_e_or_ldhl_sp_e_m2 & f.c)
                | (c.sub_or_sbc & (!f.c | !o(3)))
                | (f.c & (s.add_hl_ss_m1 | (c.add_a_x_or_adc_a_x & o(3)))));

        // The shifter.
        // @hw die: ALU_SHIFTER
        let p = |bit: u32| i.op2 >> bit & 1 != 0;
        let shift_bits = [
            (f.c & c.rl) | (p(4) & c.swap) | (p(1) & c.rrx_or_srx) | (p(7) & c.rlc),
            (p(5) & c.swap) | (p(2) & c.rrx_or_srx) | (p(0) & c.rlx_or_slx),
            (p(6) & c.swap) | (p(3) & c.rrx_or_srx) | (p(1) & c.rlx_or_slx),
            (p(7) & c.swap) | (p(4) & c.rrx_or_srx) | (p(2) & c.rlx_or_slx),
            (p(5) & c.rrx_or_srx) | (p(0) & c.swap) | (p(3) & c.rlx_or_slx),
            (p(1) & c.swap) | (p(6) & c.rrx_or_srx) | (p(4) & c.rlx_or_slx),
            (p(2) & c.swap) | (p(5) & c.rlx_or_slx) | (p(7) & c.rrx_or_srx),
            (f.c & c.rr)
                | (p(7) & c.sra_op)
                | (p(0) & c.rrc)
                | (p(3) & c.swap)
                | (p(6) & c.rlx_or_slx),
        ];
        let mut shift = 0_u8;
        for bit in 0..8 {
            shift |= u8::from(exec & shift_bits[bit]) << bit;
        }

        // Generate, propagate, the logic operations, the carries, the
        // result.
        // @hw die: ALU_PGGEN, ALU_AND, ALU_CGEN_L, ALU_CGEN_H, ALU_RESULT,
        // ALU_ZGEN
        let operand2 = i.op2 ^ mask(c.alu_inv_op2);
        let generate = operand2 & op1dec;
        let propagate = operand2 | op1dec;
        let half_sum = propagate & !generate;
        let logic = (generate & mask(c.alu_and)) | (mask(c.alu_or) & propagate) | shift;
        let (mut carry, mut cin, mut res) = (0_u8, carry_in, 0_u8);
        for bit in 0..8 {
            let (g, pr, hs) = (
                generate >> bit & 1 != 0,
                propagate >> bit & 1 != 0,
                half_sum >> bit & 1 != 0,
            );
            let sum = (hs & c.alu_xor) | (c.alu_add & (hs ^ cin)) | (logic >> bit & 1 != 0);
            res |= u8::from(sum) << bit;
            cin = g | (pr & cin);
            carry |= u8::from(cin) << bit;
        }
        let res_zero = res == 0;
        let carry3 = carry & 0x08 != 0;
        let carry7 = carry & 0x80 != 0;

        // The new flags, in write_phase.
        // @hw die: ALU_DECODER (y2, y3, y8, y13)
        let w = i.write_phase;
        let z = |bit: u32| i.reg_z >> bit & 1 != 0;
        let new = Flags {
            z: w & ((s.bit_op & (operand2 & bit_set != 0))
                | (s.pop_qq_m2 & z(7))
                | (res_zero & (s.cb_rxx_or_sxx_or_swap | s.alu | s.inc_or_dec_m | c.daa_m0))
                | (f.z & (c.cpl_m0 | s.add_hl_ss_m0 | c.scf_or_ccf_m0 | s.add_hl_ss_m1))),
            n: w & (c.cpl_m0 | c.alu_inv_cin | (s.pop_qq_m2 & z(6)) | (f.n & c.daa_m0)),
            h: w & (c.alu_and
                | s.bit_op
                | c.cpl_m0
                | (s.pop_qq_m2 & z(5))
                | (!carry3 & c.alu_inv_cin)
                | (carry3 & (c.add_or_adc_or_ldhl_exec | (!o(0) & s.inc_or_dec_m)))),
            c: w & ((c.rrx_or_srx & (operand2 & 0x01 != 0))
                | (c.rlx_or_slx & (operand2 & 0x80 != 0))
                | (s.pop_qq_m2 & z(4))
                | (c.sub_or_sbc_or_cp & !carry7)
                | (f.c & c.cpl_m0)
                | (!o(3) & c.scf_or_ccf_m0)
                | (c.add_or_adc_or_ldhl_exec & carry7)
                | (c.scf_or_ccf_m0 & o(3) & !f.c)
                | (c.daa_m0 & (f.c | (!f.n & carry7)))),
        };
        AluResult { res, carry, new }
    }

    /// The flags follow their masters while their write enables are low.
    // @hw die: FLAG_Z, FLAG_N, FLAG_H, FLAG_C (dffn_ee_pch_d_alu_flag)
    pub const fn show(&mut self, c: &Controls) {
        if !c.flags_znh_we {
            self.flags.z = self.masters.z;
            self.flags.n = self.masters.n;
            self.flags.h = self.masters.h;
        }
        if !c.flags_c_we {
            self.flags.c = self.masters.c;
        }
    }

    /// The masters take the new flags in `write_phase` while their write
    /// enables are on.
    pub const fn take(&mut self, c: &Controls, write_phase: bool, new: Flags) {
        if write_phase {
            if c.flags_znh_we {
                self.masters.z = new.z;
                self.masters.n = new.n;
                self.masters.h = new.h;
            }
            if c.flags_c_we {
                self.masters.c = new.c;
            }
        }
    }

    /// The condition (NZ, Z, NC, C: the opcode's bits 3 and 4) fails:
    /// found in `write_phase`, cleared while `pch_phase` is high.
    // @hw die: ALU_DECODER (y12), ALU_NOT9
    pub const fn settle_condition(
        &mut self,
        c: &Controls,
        op: u8,
        pch_phase: bool,
        write_phase: bool,
    ) {
        let (o3, o4) = (op & 0x08 != 0, op & 0x10 != 0);
        let f = self.flags;
        let fail = (!o3 & o4 & f.c) | (o3 & o4 & !f.c) | (!f.z & o3 & !o4) | (!o3 & !o4 & f.z);
        if pch_phase {
            self.cond_fail = false;
        } else if write_phase {
            self.cond_fail = c.any_cc & fail;
        } else {
            // The node keeps its charge.
        }
    }

    /// Z's sign, kept for the next M-cycle (ADD SP,e and LD HL,SP+e with a
    /// negative e).
    // @hw die: ALU_DFF (dffn_ee_q_alu_sign)
    pub const fn settle_sign(&mut self, data_phase: bool, write_phase: bool, reg_z: u8) {
        if data_phase & write_phase {
            self.z7_master = reg_z & 0x80 != 0;
        }
        if !data_phase {
            self.z7_delayed = self.z7_master;
        }
    }
}
