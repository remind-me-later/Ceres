//! The instruction classes, each from every pair of states that
//! `relation::boundary` relates, against the netlist's core.

use super::*;

/// The 8-bit arithmetic and logic on A with a register: ADD, ADC, SUB,
/// SBC, AND, XOR, OR and CP with B, C, D, E, H, L or A.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_alu_a_r() {
    instruction_agrees(|op, _| (0x80..=0xBF).contains(&op) && op & 7 != 6, &[1], 1);
}
/// LD r,d8: B, C, D, E, H, L or A takes the byte after the opcode (two
/// M-cycles).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ld_r_d8() {
    instruction_agrees(|op, _| op & 0xC7 == 0x06 && op != 0x36, &[2], 2);
}
/// LD r,r': between B, C, D, E, H, L and A.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ld_r_r() {
    instruction_agrees(
        |op, _| (0x40..=0x7F).contains(&op) && op & 7 != 6 && op & 0x38 != 0x30,
        &[1],
        1,
    );
}
/// INC r and DEC r.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_inc_dec_r() {
    instruction_agrees(|op, _| op & 0xC6 == 0x04 && op & 0x38 != 0x30, &[1], 1);
}
/// The rotations of A (RLCA, RRCA, RLA, RRA), DAA, CPL, SCF and CCF.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_a_and_flags() {
    instruction_agrees(|op, _| op & 0xC7 == 0x07, &[1], 1);
}
/// NOP, JP HL, DI and EI.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_nop_jp_hl_di_ei() {
    instruction_agrees(|op, _| matches!(op, 0x00 | 0xE9 | 0xF3 | 0xFB), &[1], 1);
}
/// The 8-bit arithmetic and logic on A with the byte after the opcode.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_alu_a_d8() {
    instruction_agrees(|op, _| op & 0xC7 == 0xC6, &[2], 2);
}
/// LD r,(HL).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ld_r_hl() {
    instruction_agrees(|op, _| op & 0xC7 == 0x46 && op != 0x76, &[2], 2);
}
/// LD (HL),r.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ld_hl_r() {
    instruction_agrees(|op, _| op & 0xF8 == 0x70 && op != 0x76, &[2], 2);
}
/// The 8-bit arithmetic and logic on A with (HL).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_alu_a_hl() {
    instruction_agrees(|op, _| op & 0xC7 == 0x86, &[2], 2);
}
/// INC rr and DEC rr.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_inc_dec_rr() {
    instruction_agrees(|op, _| op & 0xC7 == 0x03, &[2], 2);
}
/// ADD HL,rr.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_add_hl_rr() {
    instruction_agrees(|op, _| op & 0xCF == 0x09, &[2], 2);
}
/// LD (BC),A, LD (DE),A, LD (HL+),A and LD (HL-),A, and their loads of A.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ld_indirect_a() {
    instruction_agrees(|op, _| op & 0xC7 == 0x02, &[2], 2);
}
/// LDH (C),A and LDH A,(C).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ldh_c() {
    instruction_agrees(|op, _| matches!(op, 0xE2 | 0xF2), &[2], 2);
}
/// LD SP,HL.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ld_sp_hl() {
    instruction_agrees(|op, _| op == 0xF9, &[2], 2);
}
/// LD rr,d16.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ld_rr_d16() {
    instruction_agrees(|op, _| op & 0xCF == 0x01, &[3], 3);
}
/// INC (HL), DEC (HL) and LD (HL),d8.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_hl_mem() {
    instruction_agrees(|op, _| matches!(op, 0x34..=0x36), &[3], 3);
}
/// POP rr.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_pop() {
    instruction_agrees(|op, _| op & 0xCF == 0xC1, &[3], 3);
}
/// LDH (a8),A and LDH A,(a8).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ldh_a8() {
    instruction_agrees(|op, _| matches!(op, 0xE0 | 0xF0), &[3], 3);
}
/// LD HL,SP+e.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ld_hl_sp_e() {
    instruction_agrees(|op, _| op == 0xF8, &[3], 3);
}
/// JR and JR cc, its condition false (two M-cycles).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_jr_2() {
    instruction_agrees(|op, _| op == 0x18 || op & 0xE7 == 0x20, &[2, 3], 2);
}
/// JR and JR cc, its condition true (three M-cycles).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_jr_3() {
    instruction_agrees(|op, _| op == 0x18 || op & 0xE7 == 0x20, &[2, 3], 3);
}
/// RET cc, its condition false (two M-cycles).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ret_cc_2() {
    instruction_agrees(|op, _| op & 0xE7 == 0xC0, &[2, 5], 2);
}
/// JP and JP cc, its condition false (three M-cycles).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_jp_3() {
    instruction_agrees(|op, _| op == 0xC3 || op & 0xE7 == 0xC2, &[3, 4], 3);
}
/// CALL and CALL cc, its condition false (three M-cycles).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_call_3() {
    instruction_agrees(|op, _| op == 0xCD || op & 0xE7 == 0xC4, &[3, 6], 3);
}
/// After the CB prefix, the rotations, shifts and bit operations on a register (two M-cycles).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_cb_r() {
    instruction_agrees(|op, cb| op == 0xCB && cb & 7 != 6, &[2, 3, 4], 2);
}
/// After the CB prefix, BIT b,(HL) (three M-cycles).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_cb_bit_hl() {
    instruction_agrees(|op, cb| op == 0xCB && cb & 0xC7 == 0x46, &[2, 3, 4], 3);
}
/// JP and JP cc, its condition true (four M-cycles).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_jp_4() {
    instruction_agrees(|op, _| op == 0xC3 || op & 0xE7 == 0xC2, &[3, 4], 4);
}
/// PUSH rr.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_push() {
    instruction_agrees(|op, _| op & 0xCF == 0xC5, &[4], 4);
}
/// RST.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_rst() {
    instruction_agrees(|op, _| op & 0xC7 == 0xC7, &[4], 4);
}
/// RET and RETI.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ret_reti() {
    instruction_agrees(|op, _| matches!(op, 0xC9 | 0xD9), &[4], 4);
}
/// LD (a16),A and LD A,(a16).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ld_a16_a() {
    instruction_agrees(|op, _| matches!(op, 0xEA | 0xFA), &[4], 4);
}
/// ADD SP,e.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_add_sp_e() {
    instruction_agrees(|op, _| op == 0xE8, &[4], 4);
}
/// After the CB prefix, the rotations, shifts, RES and SET on (HL) (four M-cycles).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_cb_hl() {
    instruction_agrees(
        |op, cb| op == 0xCB && cb & 7 == 6 && cb & 0xC0 != 0x40,
        &[2, 3, 4],
        4,
    );
}
/// LD (a16),SP.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ld_a16_sp() {
    instruction_agrees(|op, _| op == 0x08, &[5], 5);
}
/// RET cc, its condition true (five M-cycles).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_ret_cc_5() {
    instruction_agrees(|op, _| op & 0xE7 == 0xC0, &[2, 5], 5);
}
/// CALL and CALL cc, its condition true (six M-cycles).
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_call_6() {
    instruction_agrees(|op, _| op == 0xCD || op & 0xE7 == 0xC4, &[3, 6], 6);
}
