//! The SM83's IDU (`hdl/rtl/sm83_idu.vhd`): it adds 1 to its 16 bits or
//! subtracts 1, or, for JR, adjusts PC's high byte by the carry or borrow
//! of the ALU's PCL + e. Its carry chain is precharged while `data_phase`
//! is low: before E only bit 0 flips.
//!
//! @hw die: dmg-schematics, category idu

use super::{control::Controls, decoder::Steps};

/// The IDU's result for its input `idu_in`; `z7` is e's sign and `carry7`
/// the carry of PCL + e (JR).
#[must_use]
pub fn idu(s: &Steps, c: &Controls, data_phase: bool, z7: bool, carry7: bool, idu_in: u16) -> u16 {
    // JR: +1 on the high byte when e is positive and PCL + e carried, -1
    // when e is negative and it did not; the chain then starts at bit 8.
    // @hw die: IDU_CTL
    let inc = c.idu_inc || (s.jr_m1 && carry7 && !z7);
    let dec = c.idu_dec || (s.jr_m1 && !carry7 && z7);
    let at8 = s.jr_m1 && (carry7 ^ z7);

    // A bit passes the carry on when it is 1 (counting up) or 0 (down).
    // @hw die: IDU_MUX_L, IDU_MUX_H, IDU_BIT0, IDU_BIT123456, IDU_BIT7
    let ena = |bit: u32| if idu_in >> bit & 1 != 0 { inc } else { dec };
    let mut flip = u16::from(inc || dec);
    let mut carry_on = data_phase && ena(0);
    for bit in 1..8 {
        flip |= u16::from(carry_on) << bit;
        carry_on = carry_on && ena(bit);
    }
    carry_on = data_phase && (carry_on || at8);
    for bit in 8..16 {
        flip |= u16::from(carry_on) << bit;
        carry_on = carry_on && ena(bit);
    }
    // @hw die: IDU_XOR_L, IDU_XOR_H
    idu_in ^ flip
}
