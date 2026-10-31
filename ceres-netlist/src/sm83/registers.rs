//! The SM83's registers (`hdl/rtl/sm83_registers.vhd`): A, B to L, W and
//! Z, SP and PC, and the buses that read them: the ALU's operands, the
//! IDU's input (the address the core puts out), and the data bus for the
//! writes of PC and SP.
//!
//! A register takes its new value in `write_phase` while its write enable
//! is on, and shows it when the enable falls, at the end of the M-cycle:
//! during the M-cycle, the buses read the old value.
//!
//! @hw die: dmg-schematics, categories reg-a, reg-bc, reg-de, reg-hl,
//! reg-sp, reg-pc, reg-wz and reg-bus

use super::{control::Controls, decoder::Steps};

/// One register of 8 bits: a master that takes its input in `write_phase`
/// while the write enable is on, and a slave that follows it while the
/// enable is off.
// @hw die: dff_cc_ee_q_x1_reg_bit, dff_cc_ee_q_x2_reg_bit,
// dff_cc_ee_q_n_reg_wz_bit, dff_cc_ee_pch_d_reg_sp_bit,
// dffs_cc_ee_pch_d_reg_pc_bit
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Register {
    pub q: u8,
    pub master: u8,
}

impl Register {
    /// A register that shows `q`, its master alike.
    #[must_use]
    pub const fn new(q: u8) -> Self {
        Self { q, master: q }
    }

    /// The slave follows the master while the write enable is off.
    pub const fn show(&mut self, we: bool) {
        if !we {
            self.q = self.master;
        }
    }

    /// The master takes `d` in `write_phase` while the write enable is on.
    pub const fn take(&mut self, we: bool, write_phase: bool, d: u8) {
        if we && write_phase {
            self.master = d;
        }
    }
}

/// The registers. Those the die keeps inverted (SP, PC, W and Z) start at
/// FF, as in dmg-sim, the others at 0.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NetRegisters {
    pub a: Register,
    pub b: Register,
    pub c: Register,
    pub d: Register,
    pub e: Register,
    pub h: Register,
    pub l: Register,
    pub w: Register,
    pub z: Register,
    pub sph: Register,
    pub spl: Register,
    pub pch: Register,
    pub pcl: Register,
}

/// The buses the registers drive in a phase.
#[derive(Clone, Copy)]
pub struct Buses {
    pub op1: u8,
    pub op2: u8,
    pub idu_in: u16,
    /// What drives the address's high byte, for the delay with which it
    /// leaves the core: W, HL or SP, anything (else it is precharged).
    pub addr_from_w: bool,
    pub addr_late: bool,
    pub addr_driven: bool,
    /// The bits PC or SP pull down on the data bus (CALL, RST, the
    /// interrupt's entry; LD (nn),SP).
    pub db_pull: u8,
}

/// What the registers take in a phase: the ALU's result, the IDU's, the
/// data bus, the interrupt's vector.
#[derive(Clone, Copy)]
pub struct RegisterInputs {
    pub op: u8,
    pub res: u8,
    pub idu_out: u16,
    pub db: u8,
    /// The vector's bits 7 to 3, in place.
    pub int_vector: u8,
    pub write_phase: bool,
    pub sys_reset: bool,
}

const fn all8(on: bool) -> u8 {
    if on { 0xFF } else { 0 }
}

impl NetRegisters {
    pub const POWER_ON: Self = Self {
        a: Register::new(0),
        b: Register::new(0),
        c: Register::new(0),
        d: Register::new(0),
        e: Register::new(0),
        h: Register::new(0),
        l: Register::new(0),
        w: Register::new(0xFF),
        z: Register::new(0xFF),
        sph: Register::new(0xFF),
        spl: Register::new(0xFF),
        pch: Register::new(0xFF),
        pcl: Register::new(0xFF),
    };

    /// The slaves follow their masters where the write enables are off.
    pub const fn show(&mut self, c: &Controls) {
        self.a.show(c.reg_a_we);
        self.b.show(c.reg_b_we);
        self.c.show(c.reg_c_we);
        self.d.show(c.reg_d_we);
        self.e.show(c.reg_e_we);
        self.h.show(c.reg_h_we);
        self.l.show(c.reg_l_we);
        self.w.show(c.reg_w_we);
        self.z.show(c.reg_z_we);
        self.sph.show(c.reg_sp_we);
        self.spl.show(c.reg_sp_we);
        self.pch.show(c.reg_pc_we);
        self.pcl.show(c.reg_pc_we);
    }

    /// The buses, with the flags (for PUSH AF) as F's bits.
    #[must_use]
    #[expect(clippy::many_single_char_names, reason = "the registers' names")]
    pub const fn buses(&self, s: &Steps, c: &Controls, op: u8, flags: u8) -> Buses {
        let (a, b, cr, d, e, h, l) = (
            self.a.q, self.b.q, self.c.q, self.d.q, self.e.q, self.h.q, self.l.q,
        );
        let (w, z, sph, spl, pch, pcl) = (
            self.w.q, self.z.q, self.sph.q, self.spl.q, self.pch.q, self.pcl.q,
        );
        // Operand 1: A, H or L (ADD HL,rr), PCL (JR), SPL or SPH (ADD SP,e,
        // LD HL,SP+e), or FF (DEC).
        // @hw die: REG_A_OUT (a), REG_HL_OUT (y3), REG_PC_OUT (y4),
        // REG_SP_OUT (y2), REG_BUS_PCH_A (b)
        let op1 = (all8(s.alu) & a)
            | (all8(s.add_hl_ss_m1) & h)
            | (all8(s.add_hl_ss_m0) & l)
            | (all8(s.jr_m1) & pcl)
            | all8(c.ff_to_op1_en)
            | (all8(c.add_sp_e_or_ldhl_sp_e_m1) & spl)
            | (all8(c.add_sp_e_or_ldhl_sp_e_m2) & sph);
        // Operand 2: a register, Z, SP (ADD HL,SP), FF (the sign of ADD
        // SP,e), or the flags (PUSH AF).
        // @hw die: REG_A_OUT (b), REG_BC_OUT, REG_DE_OUT, REG_HL_OUT (y2),
        // REG_WZ_OUT (nand), REG_SP_OUT (y3), REG_BUS_PCH_A (c)
        let add_hl_sp = op & 0x30 == 0x30;
        let op2 = (all8(c.reg_a_to_op2_en) & a)
            | (all8(c.reg_b_to_op2_en) & b)
            | (all8(c.reg_c_to_op2_en) & cr)
            | (all8(c.reg_d_to_op2_en) & d)
            | (all8(c.reg_e_to_op2_en) & e)
            | (all8(c.reg_h_to_op2_en) & h)
            | (all8(c.reg_l_to_op2_en) & l)
            | (all8(c.reg_z_to_op2_en) & z)
            | (all8(s.add_hl_ss_m0 && add_hl_sp) & spl)
            | (all8(s.add_hl_ss_m1 && add_hl_sp) & sph)
            | all8(c.ff_to_op2_en)
            | (all8(c.flags_to_op2_en) & flags & 0xF0);
        // The IDU's input, the address: a register pair, PC, SP or WZ, with
        // FF as the high byte for LDH.
        // @hw die: REG_BC_OUT, REG_DE_OUT (y1 and d_zero), REG_HL_OUT (y1),
        // REG_PC_OUT (y2, y3), REG_SP_OUT (y4, y5), REG_WZ_OUT (oai21,
        // oai31), REG_BUS_PCH_B
        let from_w = s.jr_m2 || c.ld_nn_sp_or_ldx_exec;
        let lo = (all8(c.reg_bc_to_idu_en) & cr)
            | (all8(c.reg_de_to_idu_en) & e)
            | (all8(c.reg_hl_to_idu_en) & l)
            | (all8(c.reg_pc_to_idu_en) & pcl)
            | (all8(c.reg_sp_to_idu_en) & spl)
            | (all8(from_w || c.ld_n_a_or_ld_a_n_m1) & z);
        let ff_high = c.ld_c_a_or_ld_a_c_m0 || c.ld_n_a_or_ld_a_n_m1;
        let hi = (all8(c.reg_bc_to_idu_en) & b)
            | (all8(c.reg_de_to_idu_en) & d)
            | all8(ff_high)
            | (all8(c.reg_hl_to_idu_en) & h)
            | (all8(c.reg_pc_to_idu_en || s.jr_m1) & pch)
            | (all8(c.reg_sp_to_idu_en) & sph)
            | (all8(from_w) & w);
        // PC and SP onto the data bus: their bits that are 0 pull it down.
        // @hw die: REG_PC_OUT (y1), REG_SP_OUT (y1)
        let db_pull = (all8(c.reg_pcl_to_db_en) & !pcl)
            | (all8(c.reg_pch_to_db_en) & !pch)
            | (all8(s.ld_nn_sp_m2) & !spl)
            | (all8(s.ld_nn_sp_m3) & !sph);
        Buses {
            op1,
            op2,
            idu_in: u16::from_be_bytes([hi, lo]),
            addr_from_w: from_w,
            addr_late: c.reg_hl_to_idu_en || c.reg_sp_to_idu_en,
            addr_driven: c.reg_bc_to_idu_en
                || c.reg_de_to_idu_en
                || c.reg_hl_to_idu_en
                || c.reg_pc_to_idu_en
                || s.jr_m1
                || c.reg_sp_to_idu_en
                || ff_high
                || from_w,
            db_pull,
        }
    }

    /// The masters take their new values in `write_phase`.
    #[expect(clippy::many_single_char_names, reason = "the registers' names")]
    #[expect(clippy::similar_names, reason = "the netlist's names")]
    pub const fn take(&mut self, s: &Steps, c: &Controls, i: RegisterInputs) {
        let wp = i.write_phase;
        let [idu_hi, idu_lo] = i.idu_out.to_be_bytes();
        // The general registers: A, B, D and H take the high bus, C, E and
        // L the low one, from the ALU, from WZ or from the IDU.
        // @hw die: REG_WZ_OUT (aoi_a, aoi_b), REG_NOT
        let new_hi = (all8(c.alu_to_reg_gp_en) & i.res)
            | (all8(c.reg_wz_to_reg_gp_en) & self.w.q)
            | (all8(c.idu_to_reg_gp_en) & idu_hi);
        let new_lo = (all8(c.alu_to_reg_gp_en) & i.res)
            | (all8(c.reg_wz_to_reg_gp_en) & self.z.q)
            | (all8(c.idu_to_reg_gp_en) & idu_lo);
        self.a.take(c.reg_a_we, wp, new_hi);
        self.b.take(c.reg_b_we, wp, new_hi);
        self.c.take(c.reg_c_we, wp, new_lo);
        self.d.take(c.reg_d_we, wp, new_hi);
        self.e.take(c.reg_e_we, wp, new_lo);
        self.h.take(c.reg_h_we, wp, new_hi);
        self.l.take(c.reg_l_we, wp, new_lo);

        // W and Z take the data bus, or the IDU's result (LD (nn),SP, JR).
        // @hw die: REG_W, REG_Z, REG_WZ_OUT (muxi_a, muxi_b)
        let new_z = if s.ld_nn_sp_m2 { idu_lo } else { i.db };
        let new_w = if c.ld_nn_sp_or_jr_exec { idu_hi } else { i.db };
        let (w, z) = (self.w.q, self.z.q);
        self.w.take(c.reg_w_we, wp, new_w);
        self.z.take(c.reg_z_we, wp, new_z);

        // SP takes the IDU's result or WZ; PC too, or the vector of RST or
        // of the interrupt's entry. The reset clears PC.
        // @hw die: REG_SP_OUT (y6, y7), REG_SPH, REG_SPL; REG_PC_OUT (y5,
        // y6), REG_PCH, REG_PCL
        let new_sph = (all8(c.reg_wz_to_reg_sp_en) & w) | (all8(c.idu_to_reg_sp_en) & idu_hi);
        let new_spl = (all8(c.idu_to_reg_sp_en) & idu_lo) | (all8(c.reg_wz_to_reg_sp_en) & z);
        self.sph.take(c.reg_sp_we, wp, all8(wp) & new_sph);
        self.spl.take(c.reg_sp_we, wp, all8(wp) & new_spl);
        let rst_vector = all8(s.rst_t_m2) & i.op & 0x38;
        let new_pch = (all8(c.reg_wz_to_reg_pc_en) & w) | (all8(c.idu_to_reg_pc_en) & idu_hi);
        let new_pcl = (all8(c.reg_wz_to_reg_pc_en) & z)
            | (all8(c.idu_to_reg_pc_en) & idu_lo)
            | rst_vector
            | i.int_vector;
        if i.sys_reset {
            self.pch.master = 0;
            self.pcl.master = 0;
        } else {
            self.pch.take(c.reg_pc_we, wp, all8(wp) & new_pch);
            self.pcl.take(c.reg_pc_we, wp, all8(wp) & new_pcl);
        }
    }
}
