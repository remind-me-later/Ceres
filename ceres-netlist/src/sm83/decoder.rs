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
#[derive(Clone, Copy)]
struct Pattern {
    mask: u8,
    value: u8,
}

impl Pattern {
    const fn new(text: &str) -> Self {
        let bytes = text.as_bytes();
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
}

impl Inputs {
    /// One line: the patterns of cb, int, the M-cycle and the opcode.
    const fn step(self, cb: &str, int: &str, m: &str, op: &str) -> bool {
        Pattern::new(cb).matches(self.cb as u8)
            && Pattern::new(int).matches(self.int as u8)
            && Pattern::new(m).matches(self.m)
            && Pattern::new(op).matches(self.op)
    }

    const fn op_is(self, op: &str) -> bool {
        Pattern::new(op).matches(self.op)
    }
}

/// The decoder's lines that are on.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
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
pub fn decode(op: u8, m: u8, cb: bool, int: bool, exec: bool) -> Steps {
    if !exec {
        return Steps::default();
    }
    let i = Inputs { op, m, cb, int };
    Steps {
        // No operand: NOP and STOP (00, 10), HALT (76), DI and EI (F3,
        // FB), the CB prefix
        nop_or_stop: i.step("0", "0", "0--", "000-0000"),
        halt: i.step("0", "0", "0--", "01110110"),
        di_or_ei: i.step("0", "0", "0--", "1111-011"),
        cb_prefix: i.step("0", "0", "0--", "11001011"),

        // The ALU on A: ALU A,r, ALU A,(HL), ALU A,n; the rotations of A
        // and the other ALU operations of 00 to 3F
        alu: i.step("0", "0", "---", "10------") || i.step("0", "0", "---", "11---110"),
        alu_a_r: i.step("0", "0", "0--", "10------") && !i.op_is("-----110"),
        alu_a_hl_m0: i.step("0", "0", "-00", "10---110"),
        alu_a_n_m0: i.step("0", "0", "-00", "11---110"),
        rxxa: i.step("0", "0", "---", "000--111"),
        aux_alu_m0: i.step("0", "0", "0--", "00---111"),

        // INC r, DEC r, INC (HL), DEC (HL)
        inc_or_dec_m: i.step("0", "0", "---", "00---10-"),
        inc_or_dec_r: i.step("0", "0", "0--", "00---10-") && !i.op_is("0011010-"),
        inc_or_dec_hl_m0: i.step("0", "0", "-00", "0011010-"),
        inc_or_dec_hl_m1: i.step("0", "0", "-01", "0011010-"),

        // 16 bits: INC rr, DEC rr, ADD HL,rr, LD rr,nn, LD SP,HL, ADD SP,e,
        // LD HL,SP+e
        inc_ss_m0: i.step("0", "0", "-00", "00--0011"),
        dec_ss_m0: i.step("0", "0", "-00", "00--1011"),
        add_hl_ss_m0: i.step("0", "0", "--0", "00--1001"),
        add_hl_ss_m1: i.step("0", "0", "-01", "00--1001"),
        ld_dd_nn_m0: i.step("0", "0", "-00", "00--0001"),
        ld_dd_nn_m1: i.step("0", "0", "-01", "00--0001"),
        ld_dd_nn_m2: i.step("0", "0", "-10", "00--0001"),
        ld_sp_hl_m0: i.step("0", "0", "-00", "11111001"),
        add_sp_e_m0: i.step("0", "0", "000", "11101000"),
        add_sp_e_m1: i.step("0", "0", "001", "11101000"),
        add_sp_e_m2: i.step("0", "0", "010", "11101000"),
        add_sp_e_m3: i.step("0", "0", "011", "11101000"),
        ldhl_sp_e_m0: i.step("0", "0", "-00", "11111000"),
        ldhl_sp_e_m1: i.step("0", "0", "-01", "11111000"),
        ldhl_sp_e_m2: i.step("0", "0", "-10", "11111000"),

        // Loads of 8 bits: LD r,r, LD r,(HL), LD (HL),r, LD r,n, LD (HL),n
        ld_or_halt: i.step("0", "0", "---", "01------"),
        ld_r_r_m0: i.step("0", "0", "0--", "01------")
            && !i.op_is("-----110")
            && !i.op_is("01110---"),
        ld_r_hl_m0: i.step("0", "0", "-00", "01---110") && !i.op_is("01110110"),
        ld_hl_r_m0: i.step("0", "0", "-00", "01110---") && !i.op_is("-----110"),
        ld_x_n: i.step("0", "0", "---", "00---110"),
        ld_x_n_m0: i.step("0", "0", "-00", "00---110"),
        ld_r_n_m2: i.step("0", "0", "-01", "00---110") && !i.op_is("00110110"),
        ld_hl_n_m1: i.step("0", "0", "-01", "00110110"),

        // Loads of A: LD (BC),A to LD A,(HL-); LDH (n),A, LDH A,(n), LDH
        // (C),A, LDH A,(C), LD (nn),A, LD A,(nn)
        ld_xxx_a_m0: i.step("0", "0", "-00", "00--0010"),
        ld_a_xxx_m0: i.step("0", "0", "-00", "00--1010"),
        ld_a_xxx_m1: i.step("0", "0", "-01", "00--1010"),
        ld_hli_m0: i.step("0", "0", "-00", "0010-010"),
        ld_hld_m0: i.step("0", "0", "-00", "0011-010"),
        ld_a_n: i.step("0", "0", "---", "111100-0"),
        ld_n_a_m0: i.step("0", "0", "-00", "11100000"),
        ld_n_a_m1: i.step("0", "0", "-01", "11100000"),
        ld_a_n_m0: i.step("0", "0", "-00", "11110000"),
        ld_a_n_m1: i.step("0", "0", "-01", "11110000"),
        ld_c_a_m0: i.step("0", "0", "-00", "11100010"),
        ld_a_c_m0: i.step("0", "0", "-00", "11110010"),
        ldx_m0: i.step("0", "0", "000", "111-1010"),
        ldx_m1: i.step("0", "0", "001", "111-1010"),
        ldx_nn_a_m2: i.step("0", "0", "010", "11101010"),
        ldx_a_nn_m2: i.step("0", "0", "010", "11111010"),
        ldx_a_nn_m3: i.step("0", "0", "011", "11111010"),

        // LD (nn),SP
        ld_nn_sp_m0: i.step("0", "0", "000", "00001000"),
        ld_nn_sp_m1: i.step("0", "0", "001", "00001000"),
        ld_nn_sp_m2: i.step("0", "0", "010", "00001000"),
        ld_nn_sp_m3: i.step("0", "0", "011", "00001000"),

        // PUSH and POP
        push_qq_m0: i.step("0", "0", "-00", "11--0101"),
        push_qq_m1: i.step("0", "0", "-01", "11--0101"),
        push_qq_m2: i.step("0", "0", "-10", "11--0101"),
        pop_qq_m0: i.step("0", "0", "-00", "11--0001"),
        pop_qq_m1: i.step("0", "0", "-01", "11--0001"),
        pop_qq_m2: i.step("0", "0", "-10", "11--0001"),

        // Jumps: JR e, JR cc,e, JP nn, JP cc,nn, JP HL
        jr_cc_e_m0: i.step("0", "0", "-00", "001--000"),
        jr_m0: i.step("0", "0", "-00", "001--000") || i.step("0", "0", "-00", "00011000"),
        jr_m1: i.step("0", "0", "-01", "001--000") || i.step("0", "0", "-01", "00011000"),
        jr_m2: i.step("0", "0", "-10", "001--000") || i.step("0", "0", "-10", "00011000"),
        jp_cc_nn_m1: i.step("0", "0", "-01", "110--010"),
        jp_m0: i.step("0", "0", "-00", "110--010") || i.step("0", "0", "-00", "11000011"),
        jp_m1: i.step("0", "0", "-01", "110--010") || i.step("0", "0", "-01", "11000011"),
        jp_m2: i.step("0", "0", "-10", "110--010") || i.step("0", "0", "-10", "11000011"),
        jp_hl: i.step("0", "0", "0--", "11101001"),

        // Calls and returns: CALL nn, CALL cc,nn, RET, RETI, RET cc, RST
        call_cc_nn_m1: i.step("0", "0", "-01", "110--100"),
        call_m0: i.step("0", "0", "000", "110--100") || i.step("0", "0", "000", "11001101"),
        call_m1: i.step("0", "0", "001", "110--100") || i.step("0", "0", "001", "11001101"),
        call_m2: i.step("0", "0", "010", "110--100") || i.step("0", "0", "010", "11001101"),
        call_m3: i.step("0", "0", "011", "110--100") || i.step("0", "0", "011", "11001101"),
        call_m4: i.step("0", "0", "100", "110--100") || i.step("0", "0", "100", "11001101"),
        ret_cc_m0: i.step("0", "0", "-00", "110--000"),
        ret_cc_m1: i.step("0", "0", "-01", "110--000"),
        ret_or_reti_m0: i.step("0", "0", "-00", "110-1001"),
        retx_m2: i.step("0", "0", "010", "110--000") || i.step("0", "0", "010", "110-1001"),
        retx_m3: i.step("0", "0", "011", "110--000") || i.step("0", "0", "011", "110-1001"),
        rst_t_m0: i.step("0", "0", "-00", "11---111"),
        rst_t_m1: i.step("0", "0", "-01", "11---111"),
        rst_t_m2: i.step("0", "0", "-10", "11---111"),

        // After the CB prefix: the rotations, shifts and SWAP, BIT, RES,
        // SET, on a register or (HL)
        cb_r: i.step("1", "0", "0--", "--------") && !i.op_is("-----110"),
        cb_hl_m0: i.step("1", "0", "-00", "-----110"),
        cb_rxx_or_sxx_or_swap: i.step("1", "0", "---", "00------"),
        bit_op: i.step("1", "0", "---", "01------"),
        bit_b_hl_m1: i.step("1", "0", "-01", "01---110"),
        cb_rxx_or_sxx_or_swap_or_res_m1: i.step("1", "0", "-01", "-0---110"),
        res_b_r_m0: i.step("1", "0", "-00", "10------") && !i.op_is("-----110"),
        res_b_hl_m1: i.step("1", "0", "-01", "10---110"),
        set_b_r_m0: i.step("1", "0", "-00", "11------") && !i.op_is("-----110"),
        set_b_hl_m1: i.step("1", "0", "-01", "11---110"),
        res_or_set_b_hl_m0: i.step("1", "0", "-00", "1----110"),

        // The interrupt's entry, M-cycles 0, 4, 5 and 6
        int_entry_m0: i.step("0", "1", "000", "--------"),
        int_entry_m4: i.step("0", "1", "100", "--------"),
        int_entry_m5: i.step("0", "1", "101", "--------"),
        int_entry_m6: i.step("0", "1", "110", "--------"),

        // M-cycles 6 and 7 of any instruction: 7 when a condition fails or
        // while the core is idle
        m6: i.step("0", "0", "110", "--------"),
        m7: i.step("0", "-", "111", "--------"),
    }
}
