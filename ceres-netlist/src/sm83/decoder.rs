//! The SM83's first decoder (`hdl/rtl/sm83_decoder.vhd`): which step of
//! which instruction the core is in. Each step is a line of the PLA on the
//! die, which matches the opcode, the M-cycle, the CB prefix ("cb") and the
//! interrupt's entry ("int"); the names are the netlist's, without its
//! prefix `ctl_op_`. A line matches in `exec_phase` (B to H) only: the
//! decoder is precharged in phase A.
//!
//! @hw die: dmg-schematics, category dec1 (cell decoder1)

/// A pattern of bits, the most significant first, `-` for a bit that does
/// not count.
struct Pattern {
    mask: u8,
    value: u8,
}

impl Pattern {
    const fn new(bytes: &[u8]) -> Self {
        let (mut mask, mut value, mut i) = (0_u8, 0_u8, 0);
        while i < bytes.len() {
            mask <<= 1;
            value <<= 1;
            match bytes[i] {
                b'0' => mask |= 1,
                b'1' => {
                    mask |= 1;
                    value |= 1;
                }
                _ => {}
            }
            i += 1;
        }
        Self { mask, value }
    }

    const fn matches(self, bits: u8) -> bool {
        bits & self.mask == self.value
    }
}

/// What the decoder's lines match: the opcode, the M-cycle, the CB prefix
/// and the interrupt's entry.
#[derive(Clone, Copy)]
struct Inputs {
    op: u8,
    m: u8,
    cb: bool,
    int: bool,
    /// The decoder is precharged while `exec_phase` is low.
    exec: bool,
}

/// One line of the PLA: the patterns of cb, int, the M-cycle and the
/// opcode. The decoder writes its lines as constants (`const { .. }`), so
/// that the patterns are parsed as the crate compiles.
struct Line {
    cb: Pattern,
    int: Pattern,
    m: Pattern,
    op: Pattern,
}

impl Line {
    const fn new(cb: &[u8], int: &[u8], m: &[u8], op: &[u8]) -> Self {
        Self {
            cb: Pattern::new(cb),
            int: Pattern::new(int),
            m: Pattern::new(m),
            op: Pattern::new(op),
        }
    }

    const fn matches(self, i: Inputs) -> bool {
        i.exec
            & self.cb.matches(i.cb as u8)
            & self.int.matches(i.int as u8)
            & self.m.matches(i.m)
            & self.op.matches(i.op)
    }
}

/// The decoder's lines that are on.
#[derive(Clone, Copy)]
#[expect(clippy::struct_excessive_bools, reason = "the decoder's lines")]
pub struct Steps {
    pub nop_or_stop: bool,
    pub halt: bool,
    pub di_or_ei: bool,
    pub cb_prefix: bool,
    pub alu: bool,
    pub alu_a_r: bool,
    pub alu_a_hl_m0: bool,
    pub alu_a_n_m0: bool,
    pub rxxa: bool,
    pub aux_alu_m0: bool,
    pub inc_or_dec_m: bool,
    pub inc_or_dec_r: bool,
    pub inc_or_dec_hl_m0: bool,
    pub inc_or_dec_hl_m1: bool,
    pub inc_ss_m0: bool,
    pub dec_ss_m0: bool,
    pub add_hl_ss_m0: bool,
    pub add_hl_ss_m1: bool,
    pub ld_dd_nn_m0: bool,
    pub ld_dd_nn_m1: bool,
    pub ld_dd_nn_m2: bool,
    pub ld_sp_hl_m0: bool,
    pub add_sp_e_m0: bool,
    pub add_sp_e_m1: bool,
    pub add_sp_e_m2: bool,
    pub add_sp_e_m3: bool,
    pub ldhl_sp_e_m0: bool,
    pub ldhl_sp_e_m1: bool,
    pub ldhl_sp_e_m2: bool,
    pub ld_or_halt: bool,
    pub ld_r_r_m0: bool,
    pub ld_r_hl_m0: bool,
    pub ld_hl_r_m0: bool,
    pub ld_x_n: bool,
    pub ld_x_n_m0: bool,
    pub ld_r_n_m2: bool,
    pub ld_hl_n_m1: bool,
    pub ld_xxx_a_m0: bool,
    pub ld_a_xxx_m0: bool,
    pub ld_a_xxx_m1: bool,
    pub ld_hli_m0: bool,
    pub ld_hld_m0: bool,
    pub ld_a_n: bool,
    pub ld_n_a_m0: bool,
    pub ld_n_a_m1: bool,
    pub ld_a_n_m0: bool,
    pub ld_a_n_m1: bool,
    pub ld_c_a_m0: bool,
    pub ld_a_c_m0: bool,
    pub ldx_m0: bool,
    pub ldx_m1: bool,
    pub ldx_nn_a_m2: bool,
    pub ldx_a_nn_m2: bool,
    pub ldx_a_nn_m3: bool,
    pub ld_nn_sp_m0: bool,
    pub ld_nn_sp_m1: bool,
    pub ld_nn_sp_m2: bool,
    pub ld_nn_sp_m3: bool,
    pub push_qq_m0: bool,
    pub push_qq_m1: bool,
    pub push_qq_m2: bool,
    pub pop_qq_m0: bool,
    pub pop_qq_m1: bool,
    pub pop_qq_m2: bool,
    pub jr_cc_e_m0: bool,
    pub jr_m0: bool,
    pub jr_m1: bool,
    pub jr_m2: bool,
    pub jp_cc_nn_m1: bool,
    pub jp_m0: bool,
    pub jp_m1: bool,
    pub jp_m2: bool,
    pub jp_hl: bool,
    pub call_cc_nn_m1: bool,
    pub call_m0: bool,
    pub call_m1: bool,
    pub call_m2: bool,
    pub call_m3: bool,
    pub call_m4: bool,
    pub ret_cc_m0: bool,
    pub ret_cc_m1: bool,
    pub ret_or_reti_m0: bool,
    pub retx_m2: bool,
    pub retx_m3: bool,
    pub rst_t_m0: bool,
    pub rst_t_m1: bool,
    pub rst_t_m2: bool,
    pub cb_r: bool,
    pub cb_hl_m0: bool,
    pub cb_rxx_or_sxx_or_swap: bool,
    pub bit_op: bool,
    pub bit_b_hl_m1: bool,
    pub cb_rxx_or_sxx_or_swap_or_res_m1: bool,
    pub res_b_r_m0: bool,
    pub res_b_hl_m1: bool,
    pub set_b_r_m0: bool,
    pub set_b_hl_m1: bool,
    pub res_or_set_b_hl_m0: bool,
    pub int_entry_m0: bool,
    pub int_entry_m4: bool,
    pub int_entry_m5: bool,
    pub int_entry_m6: bool,
    pub m6: bool,
    pub m7: bool,
}

/// The steps for the opcode `op` in its M-cycle `m`, after the CB prefix
/// (`cb`) or in the interrupt's entry (`int`); all off while `exec` is low.
#[must_use]
#[expect(clippy::too_many_lines, reason = "a line of the PLA a line")]
pub const fn decode(op: u8, m: u8, cb: bool, int: bool, exec: bool) -> Steps {
    let i = Inputs {
        op,
        m,
        cb,
        int,
        exec,
    };
    Steps {
        // No operand: NOP and STOP (00, 10), HALT (76), DI and EI (F3,
        // FB), the CB prefix
        nop_or_stop: const { Line::new(b"0", b"0", b"0--", b"000-0000") }.matches(i),
        halt: const { Line::new(b"0", b"0", b"0--", b"01110110") }.matches(i),
        di_or_ei: const { Line::new(b"0", b"0", b"0--", b"1111-011") }.matches(i),
        cb_prefix: const { Line::new(b"0", b"0", b"0--", b"11001011") }.matches(i),

        // The ALU on A: ALU A,r, ALU A,(HL), ALU A,n; the rotations of A
        // and the other ALU operations of 00 to 3F
        alu: const { Line::new(b"0", b"0", b"---", b"10------") }.matches(i)
            | const { Line::new(b"0", b"0", b"---", b"11---110") }.matches(i),
        alu_a_r: const { Line::new(b"0", b"0", b"0--", b"10------") }.matches(i)
            & !const { Pattern::new(b"-----110") }.matches(i.op),
        alu_a_hl_m0: const { Line::new(b"0", b"0", b"-00", b"10---110") }.matches(i),
        alu_a_n_m0: const { Line::new(b"0", b"0", b"-00", b"11---110") }.matches(i),
        rxxa: const { Line::new(b"0", b"0", b"---", b"000--111") }.matches(i),
        aux_alu_m0: const { Line::new(b"0", b"0", b"0--", b"00---111") }.matches(i),

        // INC r, DEC r, INC (HL), DEC (HL)
        inc_or_dec_m: const { Line::new(b"0", b"0", b"---", b"00---10-") }.matches(i),
        inc_or_dec_r: const { Line::new(b"0", b"0", b"0--", b"00---10-") }.matches(i)
            & !const { Pattern::new(b"0011010-") }.matches(i.op),
        inc_or_dec_hl_m0: const { Line::new(b"0", b"0", b"-00", b"0011010-") }.matches(i),
        inc_or_dec_hl_m1: const { Line::new(b"0", b"0", b"-01", b"0011010-") }.matches(i),

        // 16 bits: INC rr, DEC rr, ADD HL,rr, LD rr,nn, LD SP,HL, ADD SP,e,
        // LD HL,SP+e
        inc_ss_m0: const { Line::new(b"0", b"0", b"-00", b"00--0011") }.matches(i),
        dec_ss_m0: const { Line::new(b"0", b"0", b"-00", b"00--1011") }.matches(i),
        add_hl_ss_m0: const { Line::new(b"0", b"0", b"--0", b"00--1001") }.matches(i),
        add_hl_ss_m1: const { Line::new(b"0", b"0", b"-01", b"00--1001") }.matches(i),
        ld_dd_nn_m0: const { Line::new(b"0", b"0", b"-00", b"00--0001") }.matches(i),
        ld_dd_nn_m1: const { Line::new(b"0", b"0", b"-01", b"00--0001") }.matches(i),
        ld_dd_nn_m2: const { Line::new(b"0", b"0", b"-10", b"00--0001") }.matches(i),
        ld_sp_hl_m0: const { Line::new(b"0", b"0", b"-00", b"11111001") }.matches(i),
        add_sp_e_m0: const { Line::new(b"0", b"0", b"000", b"11101000") }.matches(i),
        add_sp_e_m1: const { Line::new(b"0", b"0", b"001", b"11101000") }.matches(i),
        add_sp_e_m2: const { Line::new(b"0", b"0", b"010", b"11101000") }.matches(i),
        add_sp_e_m3: const { Line::new(b"0", b"0", b"011", b"11101000") }.matches(i),
        ldhl_sp_e_m0: const { Line::new(b"0", b"0", b"-00", b"11111000") }.matches(i),
        ldhl_sp_e_m1: const { Line::new(b"0", b"0", b"-01", b"11111000") }.matches(i),
        ldhl_sp_e_m2: const { Line::new(b"0", b"0", b"-10", b"11111000") }.matches(i),

        // Loads of 8 bits: LD r,r, LD r,(HL), LD (HL),r, LD r,n, LD (HL),n
        ld_or_halt: const { Line::new(b"0", b"0", b"---", b"01------") }.matches(i),
        ld_r_r_m0: const { Line::new(b"0", b"0", b"0--", b"01------") }.matches(i)
            & !const { Pattern::new(b"-----110") }.matches(i.op)
            & !const { Pattern::new(b"01110---") }.matches(i.op),
        ld_r_hl_m0: const { Line::new(b"0", b"0", b"-00", b"01---110") }.matches(i)
            & !const { Pattern::new(b"01110110") }.matches(i.op),
        ld_hl_r_m0: const { Line::new(b"0", b"0", b"-00", b"01110---") }.matches(i)
            & !const { Pattern::new(b"-----110") }.matches(i.op),
        ld_x_n: const { Line::new(b"0", b"0", b"---", b"00---110") }.matches(i),
        ld_x_n_m0: const { Line::new(b"0", b"0", b"-00", b"00---110") }.matches(i),
        ld_r_n_m2: const { Line::new(b"0", b"0", b"-01", b"00---110") }.matches(i)
            & !const { Pattern::new(b"00110110") }.matches(i.op),
        ld_hl_n_m1: const { Line::new(b"0", b"0", b"-01", b"00110110") }.matches(i),

        // Loads of A: LD (BC),A to LD A,(HL-); LDH (n),A, LDH A,(n), LDH
        // (C),A, LDH A,(C), LD (nn),A, LD A,(nn)
        ld_xxx_a_m0: const { Line::new(b"0", b"0", b"-00", b"00--0010") }.matches(i),
        ld_a_xxx_m0: const { Line::new(b"0", b"0", b"-00", b"00--1010") }.matches(i),
        ld_a_xxx_m1: const { Line::new(b"0", b"0", b"-01", b"00--1010") }.matches(i),
        ld_hli_m0: const { Line::new(b"0", b"0", b"-00", b"0010-010") }.matches(i),
        ld_hld_m0: const { Line::new(b"0", b"0", b"-00", b"0011-010") }.matches(i),
        ld_a_n: const { Line::new(b"0", b"0", b"---", b"111100-0") }.matches(i),
        ld_n_a_m0: const { Line::new(b"0", b"0", b"-00", b"11100000") }.matches(i),
        ld_n_a_m1: const { Line::new(b"0", b"0", b"-01", b"11100000") }.matches(i),
        ld_a_n_m0: const { Line::new(b"0", b"0", b"-00", b"11110000") }.matches(i),
        ld_a_n_m1: const { Line::new(b"0", b"0", b"-01", b"11110000") }.matches(i),
        ld_c_a_m0: const { Line::new(b"0", b"0", b"-00", b"11100010") }.matches(i),
        ld_a_c_m0: const { Line::new(b"0", b"0", b"-00", b"11110010") }.matches(i),
        ldx_m0: const { Line::new(b"0", b"0", b"000", b"111-1010") }.matches(i),
        ldx_m1: const { Line::new(b"0", b"0", b"001", b"111-1010") }.matches(i),
        ldx_nn_a_m2: const { Line::new(b"0", b"0", b"010", b"11101010") }.matches(i),
        ldx_a_nn_m2: const { Line::new(b"0", b"0", b"010", b"11111010") }.matches(i),
        ldx_a_nn_m3: const { Line::new(b"0", b"0", b"011", b"11111010") }.matches(i),

        // LD (nn),SP
        ld_nn_sp_m0: const { Line::new(b"0", b"0", b"000", b"00001000") }.matches(i),
        ld_nn_sp_m1: const { Line::new(b"0", b"0", b"001", b"00001000") }.matches(i),
        ld_nn_sp_m2: const { Line::new(b"0", b"0", b"010", b"00001000") }.matches(i),
        ld_nn_sp_m3: const { Line::new(b"0", b"0", b"011", b"00001000") }.matches(i),

        // PUSH and POP
        push_qq_m0: const { Line::new(b"0", b"0", b"-00", b"11--0101") }.matches(i),
        push_qq_m1: const { Line::new(b"0", b"0", b"-01", b"11--0101") }.matches(i),
        push_qq_m2: const { Line::new(b"0", b"0", b"-10", b"11--0101") }.matches(i),
        pop_qq_m0: const { Line::new(b"0", b"0", b"-00", b"11--0001") }.matches(i),
        pop_qq_m1: const { Line::new(b"0", b"0", b"-01", b"11--0001") }.matches(i),
        pop_qq_m2: const { Line::new(b"0", b"0", b"-10", b"11--0001") }.matches(i),

        // Jumps: JR e, JR cc,e, JP nn, JP cc,nn, JP HL
        jr_cc_e_m0: const { Line::new(b"0", b"0", b"-00", b"001--000") }.matches(i),
        jr_m0: const { Line::new(b"0", b"0", b"-00", b"001--000") }.matches(i)
            | const { Line::new(b"0", b"0", b"-00", b"00011000") }.matches(i),
        jr_m1: const { Line::new(b"0", b"0", b"-01", b"001--000") }.matches(i)
            | const { Line::new(b"0", b"0", b"-01", b"00011000") }.matches(i),
        jr_m2: const { Line::new(b"0", b"0", b"-10", b"001--000") }.matches(i)
            | const { Line::new(b"0", b"0", b"-10", b"00011000") }.matches(i),
        jp_cc_nn_m1: const { Line::new(b"0", b"0", b"-01", b"110--010") }.matches(i),
        jp_m0: const { Line::new(b"0", b"0", b"-00", b"110--010") }.matches(i)
            | const { Line::new(b"0", b"0", b"-00", b"11000011") }.matches(i),
        jp_m1: const { Line::new(b"0", b"0", b"-01", b"110--010") }.matches(i)
            | const { Line::new(b"0", b"0", b"-01", b"11000011") }.matches(i),
        jp_m2: const { Line::new(b"0", b"0", b"-10", b"110--010") }.matches(i)
            | const { Line::new(b"0", b"0", b"-10", b"11000011") }.matches(i),
        jp_hl: const { Line::new(b"0", b"0", b"0--", b"11101001") }.matches(i),

        // Calls and returns: CALL nn, CALL cc,nn, RET, RETI, RET cc, RST
        call_cc_nn_m1: const { Line::new(b"0", b"0", b"-01", b"110--100") }.matches(i),
        call_m0: const { Line::new(b"0", b"0", b"000", b"110--100") }.matches(i)
            | const { Line::new(b"0", b"0", b"000", b"11001101") }.matches(i),
        call_m1: const { Line::new(b"0", b"0", b"001", b"110--100") }.matches(i)
            | const { Line::new(b"0", b"0", b"001", b"11001101") }.matches(i),
        call_m2: const { Line::new(b"0", b"0", b"010", b"110--100") }.matches(i)
            | const { Line::new(b"0", b"0", b"010", b"11001101") }.matches(i),
        call_m3: const { Line::new(b"0", b"0", b"011", b"110--100") }.matches(i)
            | const { Line::new(b"0", b"0", b"011", b"11001101") }.matches(i),
        call_m4: const { Line::new(b"0", b"0", b"100", b"110--100") }.matches(i)
            | const { Line::new(b"0", b"0", b"100", b"11001101") }.matches(i),
        ret_cc_m0: const { Line::new(b"0", b"0", b"-00", b"110--000") }.matches(i),
        ret_cc_m1: const { Line::new(b"0", b"0", b"-01", b"110--000") }.matches(i),
        ret_or_reti_m0: const { Line::new(b"0", b"0", b"-00", b"110-1001") }.matches(i),
        retx_m2: const { Line::new(b"0", b"0", b"010", b"110--000") }.matches(i)
            | const { Line::new(b"0", b"0", b"010", b"110-1001") }.matches(i),
        retx_m3: const { Line::new(b"0", b"0", b"011", b"110--000") }.matches(i)
            | const { Line::new(b"0", b"0", b"011", b"110-1001") }.matches(i),
        rst_t_m0: const { Line::new(b"0", b"0", b"-00", b"11---111") }.matches(i),
        rst_t_m1: const { Line::new(b"0", b"0", b"-01", b"11---111") }.matches(i),
        rst_t_m2: const { Line::new(b"0", b"0", b"-10", b"11---111") }.matches(i),

        // After the CB prefix: the rotations, shifts and SWAP, BIT, RES,
        // SET, on a register or (HL)
        cb_r: const { Line::new(b"1", b"0", b"0--", b"--------") }.matches(i)
            & !const { Pattern::new(b"-----110") }.matches(i.op),
        cb_hl_m0: const { Line::new(b"1", b"0", b"-00", b"-----110") }.matches(i),
        cb_rxx_or_sxx_or_swap: const { Line::new(b"1", b"0", b"---", b"00------") }.matches(i),
        bit_op: const { Line::new(b"1", b"0", b"---", b"01------") }.matches(i),
        bit_b_hl_m1: const { Line::new(b"1", b"0", b"-01", b"01---110") }.matches(i),
        cb_rxx_or_sxx_or_swap_or_res_m1: const { Line::new(b"1", b"0", b"-01", b"-0---110") }
            .matches(i),
        res_b_r_m0: const { Line::new(b"1", b"0", b"-00", b"10------") }.matches(i)
            & !const { Pattern::new(b"-----110") }.matches(i.op),
        res_b_hl_m1: const { Line::new(b"1", b"0", b"-01", b"10---110") }.matches(i),
        set_b_r_m0: const { Line::new(b"1", b"0", b"-00", b"11------") }.matches(i)
            & !const { Pattern::new(b"-----110") }.matches(i.op),
        set_b_hl_m1: const { Line::new(b"1", b"0", b"-01", b"11---110") }.matches(i),
        res_or_set_b_hl_m0: const { Line::new(b"1", b"0", b"-00", b"1----110") }.matches(i),

        // The interrupt's entry, M-cycles 0, 4, 5 and 6
        int_entry_m0: const { Line::new(b"0", b"1", b"000", b"--------") }.matches(i),
        int_entry_m4: const { Line::new(b"0", b"1", b"100", b"--------") }.matches(i),
        int_entry_m5: const { Line::new(b"0", b"1", b"101", b"--------") }.matches(i),
        int_entry_m6: const { Line::new(b"0", b"1", b"110", b"--------") }.matches(i),

        // M-cycles 6 and 7 of any instruction: 7 when a condition fails or
        // while the core is idle
        m6: const { Line::new(b"0", b"0", b"110", b"--------") }.matches(i),
        m7: const { Line::new(b"0", b"-", b"111", b"--------") }.matches(i),
    }
}
