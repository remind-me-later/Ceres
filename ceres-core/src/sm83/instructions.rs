//! The opcode decoder and the instruction handlers.

use crate::memory::{IO_START, P1, io_addr};

use super::{Bus, CF, HF, NF, Sm83, ZF};

// Instructions. In the names, r is an 8-bit register and rr a pair, high
// and low are the halves of a pair (B D H A and C E L), dhl is (HL), d8 and
// d16 are immediates, a8 and a16 immediate addresses.
impl Sm83 {
    pub(super) fn exec(&mut self, bus: &mut impl Bus, op: u8) {
        match op {
            0x00 | 0x5B | 0x6D | 0x7F | 0x49 | 0x52 | 0x64 => self.nop(),
            0x01 | 0x11 | 0x21 | 0x31 => self.ld_rr_d16(bus, op),
            0x02 | 0x12 => self.ld_drr_a(bus, op),
            0x03 | 0x13 | 0x23 | 0x33 => self.inc_rr(bus, op),
            0x04 | 0x14 | 0x24 | 0x3C => self.inc_high(op),
            0x05 | 0x15 | 0x25 | 0x3D => self.dec_high(op),
            0x06 | 0x16 | 0x26 | 0x3E => self.ld_high_d8(bus, op),
            0x07 => self.rlca(),
            0x08 => self.ld_da16_sp(bus),
            0x09 | 0x19 | 0x29 | 0x39 => self.add_hl_rr(bus, op),
            0x0A | 0x1A => self.ld_a_drr(bus, op),
            0x0B | 0x1B | 0x2B | 0x3B => self.dec_rr(bus, op),
            0x0C | 0x1C | 0x2C => self.inc_low(op),
            0x0D | 0x1D | 0x2D => self.dec_low(op),
            0x0E | 0x1E | 0x2E => self.ld_low_d8(bus, op),
            0x0F => self.rrca(),
            0x10 => self.stop(bus),
            0x17 => self.rla(),
            0x18 => self.jr_d(bus),
            0x1F => self.rra(),
            0x20 | 0x28 | 0x30 | 0x38 => self.jr_cc(bus, op),
            0x22 => self.ld_dhli_a(bus),
            0x27 => self.daa(),
            0x2A => self.ld_a_dhli(bus),
            0x2F => self.cpl(),
            0x32 => self.ld_dhld_a(bus),
            0x34 => self.inc_dhl(bus),
            0x35 => self.dec_dhl(bus),
            0x36 => self.ld_dhl_d8(bus),
            0x37 => self.scf(),
            0x3A => self.ld_a_dhld(bus),
            0x3F => self.ccf(),
            0x40 => self.ld_b_b(),
            // The LD r,r' that are not NOPs (matched above).
            0x41..=0x75 | 0x77..=0x7F => self.ld(bus, op),
            0x76 => self.halt(bus),
            0x80..=0xBF => self.alu_a_r(bus, op),
            0xC6 | 0xCE | 0xD6 | 0xDE | 0xE6 | 0xEE | 0xF6 | 0xFE => self.alu_a_d8(bus, op),
            0xC0 | 0xC8 | 0xD0 | 0xD8 => self.ret_cc(bus, op),
            0xC1 | 0xD1 | 0xE1 | 0xF1 => self.pop_rr(bus, op),
            0xC2 | 0xCA | 0xD2 | 0xDA => self.jp_cc(bus, op),
            0xC3 => self.jp_a16(bus),
            0xC4 | 0xCC | 0xD4 | 0xDC => self.call_cc_a16(bus, op),
            0xC5 | 0xD5 | 0xE5 | 0xF5 => self.push_rr(bus, op),
            0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => self.rst(bus, op),
            0xC9 => self.ret(bus),
            0xCB => self.exec_cb(bus),
            0xCD => self.call_nn(bus),
            0xD9 => self.reti(bus),
            0xE0 => self.ldh_da8_a(bus),
            0xE2 => self.ldh_dc_a(bus),
            0xE8 => self.add_sp_r8(bus),
            0xE9 => self.jp_hl(),
            0xEA => self.ld_da16_a(bus),
            0xF0 => self.ldh_a_da8(bus),
            0xF2 => self.ldh_a_dc(bus),
            0xF3 => self.di(),
            0xF8 => self.ld_hl_sp_r8(bus),
            0xF9 => self.ld16_sp_hl(bus),
            0xFA => self.ld_a_da16(bus),
            0xFB => self.ei(),
            _ => self.illegal(bus),
        }
    }

    fn exec_cb(&mut self, bus: &mut impl Bus) {
        let op = self.imm8(bus);
        if op < 0x40 {
            self.shift_r(bus, op);
        } else {
            self.bit_r(bus, op);
        }
    }

    fn alu_a_d8(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.imm8(bus);
        self.alu(op, val);
    }

    fn alu_a_r(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.get_r(bus, op);
        self.alu(op, val);
    }

    /// The CB rotates, shifts and SWAP.
    fn shift_r(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.get_r(bus, op);
        let carry_in = self.af & CF != 0;
        let (res, carry) = match op >> 3 {
            // RLC, RRC
            0 => (val.rotate_left(1), val & 0x80 != 0),
            1 => (val.rotate_right(1), val & 1 != 0),
            // RL, RR
            2 => ((val << 1) | u8::from(carry_in), val & 0x80 != 0),
            3 => ((val >> 1) | (u8::from(carry_in) << 7), val & 1 != 0),
            // SLA, SRA
            4 => (val << 1, val & 0x80 != 0),
            5 => ((val >> 1) | (val & 0x80), val & 1 != 0),
            // SWAP, SRL
            6 => (val.rotate_left(4), false),
            _ => (val >> 1, val & 1 != 0),
        };
        self.set_r(bus, op, res);
        self.af &= 0xFF00;
        if carry {
            self.af |= CF;
        }
        if res == 0 {
            self.af |= ZF;
        }
    }

    fn add_hl_rr(&mut self, bus: &mut impl Bus, op: u8) {
        let id = Self::pair_id(op);
        let hl = self.hl;
        let rr = self.get_rr(id);
        let (res, carry) = hl.overflowing_add(rr);
        self.hl = res;

        self.af &= !(NF | CF | HF);
        if (hl & 0xFFF) + (rr & 0xFFF) > 0xFFF {
            self.af |= HF;
        }
        if carry {
            self.af |= CF;
        }

        bus.tick();
    }

    fn add_sp_r8(&mut self, bus: &mut impl Bus) {
        let res = self.sp_plus_imm8(bus);
        bus.tick();
        bus.tick();
        self.sp = res;
    }

    fn bit_r(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.get_r(bus, op);
        let bit = 1 << ((op >> 3) & 7);
        match op & 0xC0 {
            0x40 => {
                // BIT
                self.af &= 0xFF00 | CF;
                self.af |= HF;
                if bit & val == 0 {
                    self.af |= ZF;
                }
            }
            0x80 => self.set_r(bus, op, val & !bit),
            _ => self.set_r(bus, op, val | bit),
        }
    }

    fn call_cc_a16(&mut self, bus: &mut impl Bus, op: u8) {
        if self.satisfies_branch_condition(op) {
            self.do_call(bus);
        } else {
            let _nn = self.imm16(bus);
        }
    }

    fn call_nn(&mut self, bus: &mut impl Bus) {
        self.do_call(bus);
    }

    const fn ccf(&mut self) {
        self.af ^= CF;
        self.af &= !(HF | NF);
    }

    const fn cpl(&mut self) {
        self.af ^= 0xFF00;
        self.af |= HF | NF;
    }

    const fn daa(&mut self) {
        let a = {
            let mut a = self.af >> 8;

            if self.af & NF == 0 {
                if self.af & HF != 0 || a & 0x0F > 0x09 {
                    a += 0x06;
                }
                if self.af & CF != 0 || a > 0x9F {
                    a += 0x60;
                }
            } else {
                if self.af & HF != 0 {
                    a = a.wrapping_sub(0x06) & 0xFF;
                }
                if self.af & CF != 0 {
                    a = a.wrapping_sub(0x60);
                }
            }

            a
        };

        self.af &= !(0xFF00 | ZF | HF);

        if a.trailing_zeros() >= 8 {
            self.af |= ZF;
        }

        if a & 0x100 == 0x100 {
            self.af |= CF;
        }

        self.af |= a << 8;
    }

    fn dec_dhl(&mut self, bus: &mut impl Bus) {
        let val = bus.read(self.hl).wrapping_sub(1);
        bus.write(self.hl, val);

        self.af &= !(ZF | HF);
        self.af |= NF;
        if (val & 0x0F) == 0x0F {
            self.af |= HF;
        }

        if val == 0 {
            self.af |= ZF;
        }
    }

    fn dec_high(&mut self, op: u8) {
        let id = Self::pair_id_af(op);
        let rr = self.get_rr(id).wrapping_sub(0x100);
        self.set_rr(id, rr);
        self.af &= !(ZF | HF);
        self.af |= NF;

        if rr & 0x0F00 == 0xF00 {
            self.af |= HF;
        }

        if rr & 0xFF00 == 0 {
            self.af |= ZF;
        }
    }

    fn dec_low(&mut self, op: u8) {
        let id = Self::pair_id(op);
        let val = self.get_rr(id).wrapping_sub(1) & 0xFF;
        let rr = self.get_rr(id) & 0xFF00 | val;
        self.set_rr(id, rr);

        self.af &= !(ZF | HF);
        self.af |= NF;

        if rr & 0x0F == 0xF {
            self.af |= HF;
        }

        if rr.trailing_zeros() >= 8 {
            self.af |= ZF;
        }
    }

    fn dec_rr(&mut self, bus: &mut impl Bus, op: u8) {
        let id = Self::pair_id(op);
        bus.tick_oam_bug(self.get_rr(id));
        self.set_rr(id, self.get_rr(id).wrapping_sub(1));
    }

    const fn di(&mut self) {
        // DI is NOT delayed, not even on a CGB.
        self.ime = false;
    }

    const fn ei(&mut self) {
        // EI disables interrupts for one more instruction, then enables them.
        if !self.ime && !self.ime_toggle {
            self.ime_toggle = true;
        }
    }

    fn halt(&mut self, bus: &mut impl Bus) {
        // A dummy read at PC flushes the fetch M-cycle before the interrupt
        // lines are sampled; the read's own M-cycle is not charged.
        let next = bus.read(self.pc);
        bus.drop_deferred();

        // The HALT bug also happens on a CGB, in both CGB and DMG modes.
        if bus.interrupts_pending() {
            self.is_halted = false;
            if self.ime {
                self.pc = self.pc.wrapping_sub(1);
            } else {
                self.halt_bug = true;
            }
        } else {
            // A transfer requested during the HALT's fetch has the next
            // opcode prefetched: it runs twice, as with the HALT bug.
            if bus.hdma_request_pending() {
                self.halt_bug = true;
                self.prefetched = Some(next);
                bus.note_halt_prefetch();
            }
            bus.dma_finish_before_halt();
            self.is_halted = true;
            bus.set_halted(true);
        }
        self.just_halted = true;
    }

    fn illegal(&mut self, bus: &mut impl Bus) {
        bus.clear_ie();
        self.is_halted = true;
        bus.set_halted(true);
        #[cfg(feature = "debug")]
        {
            self.has_executed_illegal_opcode = true;
        }
    }

    fn inc_dhl(&mut self, bus: &mut impl Bus) {
        let val = bus.read(self.hl).wrapping_add(1);
        bus.write(self.hl, val);

        self.af &= !(NF | ZF | HF);
        if val.trailing_zeros() >= 4 {
            self.af |= HF;
        }

        if val == 0 {
            self.af |= ZF;
        }
    }

    fn inc_high(&mut self, op: u8) {
        let id = Self::pair_id_af(op);
        let rr = self.get_rr(id).wrapping_add(0x100);
        self.set_rr(id, rr);
        self.af &= !(NF | ZF | HF);

        if rr & 0x0F00 == 0 {
            self.af |= HF;
        }

        if rr & 0xFF00 == 0 {
            self.af |= ZF;
        }
    }

    fn inc_low(&mut self, op: u8) {
        let id = Self::pair_id(op);
        let val = self.get_rr(id).wrapping_add(1) & 0xFF;
        let rr = self.get_rr(id) & 0xFF00 | val;
        self.set_rr(id, rr);

        self.af &= !(NF | ZF | HF);

        if rr.trailing_zeros() >= 4 {
            self.af |= HF;
        }

        if rr.trailing_zeros() >= 8 {
            self.af |= ZF;
        }
    }

    fn inc_rr(&mut self, bus: &mut impl Bus, op: u8) {
        let id = Self::pair_id(op);
        bus.tick_oam_bug(self.get_rr(id));
        self.set_rr(id, self.get_rr(id).wrapping_add(1));
    }

    fn jp_a16(&mut self, bus: &mut impl Bus) {
        self.do_jump_to_immediate(bus);
    }

    fn jp_cc(&mut self, bus: &mut impl Bus, op: u8) {
        if self.satisfies_branch_condition(op) {
            self.do_jump_to_immediate(bus);
        } else {
            let _nn = self.imm16(bus);
        }
    }

    const fn jp_hl(&mut self) {
        self.pc = self.hl;
    }

    fn jr_cc(&mut self, bus: &mut impl Bus, op: u8) {
        if self.satisfies_branch_condition(op) {
            self.do_jump_relative(bus);
        } else {
            let _discard_byte = self.imm8(bus);
        }
    }

    fn jr_d(&mut self, bus: &mut impl Bus) {
        self.do_jump_relative(bus);
    }

    fn ld(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.get_r(bus, op);
        self.set_r(bus, op >> 3, val);
    }

    fn ld16_sp_hl(&mut self, bus: &mut impl Bus) {
        let val = self.hl;
        bus.tick_oam_bug(val);
        self.sp = val;
    }

    fn ld_a_da16(&mut self, bus: &mut impl Bus) {
        let addr = self.imm16(bus);
        let val = bus.read(addr);
        self.set_a(val);
    }

    fn ld_a_dhld(&mut self, bus: &mut impl Bus) {
        let val = bus.read(self.hl);
        self.set_a(val);
        self.hl = self.hl.wrapping_sub(1);
    }

    fn ld_a_dhli(&mut self, bus: &mut impl Bus) {
        let val = bus.read(self.hl);
        self.set_a(val);
        self.hl = self.hl.wrapping_add(1);
    }

    fn ld_a_drr(&mut self, bus: &mut impl Bus, op: u8) {
        let addr = self.get_rr(Self::pair_id(op));
        let val = bus.read(addr);
        self.set_a(val);
    }

    // Sets the debug breakpoint flag. Test ROMs like cgb-acid2 and dmg-acid2
    // use this instruction as a breakpoint to signal test completion.
    #[cfg_attr(
        not(feature = "debug"),
        expect(
            clippy::needless_pass_by_ref_mut,
            reason = "it sets the flag with `debug`"
        )
    )]
    const fn ld_b_b(&mut self) {
        #[cfg(feature = "debug")]
        {
            self.ld_b_b_breakpoint = true;
        }
        self.nop();
    }

    fn ld_da16_a(&mut self, bus: &mut impl Bus) {
        let addr = self.imm16(bus);
        bus.write(addr, self.a());
    }

    fn ld_da16_sp(&mut self, bus: &mut impl Bus) {
        let val = self.sp;
        let addr = self.imm16(bus);
        bus.write(addr, (val & 0xFF) as u8);
        bus.write(addr.wrapping_add(1), (val >> 8) as u8);
    }

    fn ld_dhl_d8(&mut self, bus: &mut impl Bus) {
        let tmp = self.imm8(bus);
        bus.write(self.hl, tmp);
    }

    fn ld_dhld_a(&mut self, bus: &mut impl Bus) {
        let addr = self.hl;
        bus.write(addr, self.a());
        self.hl = addr.wrapping_sub(1);
    }

    fn ld_dhli_a(&mut self, bus: &mut impl Bus) {
        let addr = self.hl;
        bus.write(addr, self.a());
        self.hl = addr.wrapping_add(1);
    }

    fn ld_drr_a(&self, bus: &mut impl Bus, op: u8) {
        let id = Self::pair_id(op);
        let addr = self.get_rr(id);
        bus.write(addr, self.a());
    }

    fn ld_hl_sp_r8(&mut self, bus: &mut impl Bus) {
        let res = self.sp_plus_imm8(bus);
        bus.tick();
        self.hl = res;
    }

    fn ld_high_d8(&mut self, bus: &mut impl Bus, op: u8) {
        let id = Self::pair_id_af(op);
        let hi = u16::from(self.imm8(bus));
        self.set_rr(id, (hi << 8) | self.get_rr(id) & 0xFF);
    }

    fn ld_low_d8(&mut self, bus: &mut impl Bus, op: u8) {
        let id = Self::pair_id(op);
        let lo = u16::from(self.imm8(bus));
        self.set_rr(id, self.get_rr(id) & 0xFF00 | lo);
    }

    fn ld_rr_d16(&mut self, bus: &mut impl Bus, op: u8) {
        let id = Self::pair_id(op);
        let imm = self.imm16(bus);
        self.set_rr(id, imm);
    }

    fn ldh_a_da8(&mut self, bus: &mut impl Bus) {
        let addr = IO_START | u16::from(self.imm8(bus));
        let val = bus.read(addr);
        self.set_a(val);
    }

    fn ldh_a_dc(&mut self, bus: &mut impl Bus) {
        let val = bus.read(IO_START | self.bc & 0xFF);
        self.set_a(val);
    }

    fn ldh_da8_a(&mut self, bus: &mut impl Bus) {
        let tmp = u16::from(self.imm8(bus));
        let a = self.a();
        bus.write(IO_START | tmp, a);
    }

    fn ldh_dc_a(&self, bus: &mut impl Bus) {
        bus.write(IO_START | self.bc & 0xFF, self.a());
    }

    #[expect(clippy::unused_self)]
    const fn nop(&self) {}

    fn pop_rr(&mut self, bus: &mut impl Bus, op: u8) {
        let val = self.pop(bus);
        let id = Self::pair_id_af(op);
        self.set_rr(id, val);
        self.af &= 0xFFF0;
    }

    fn push_rr(&mut self, bus: &mut impl Bus, op: u8) {
        let id = Self::pair_id_af(op);
        self.push(bus, self.get_rr(id));
    }

    fn ret(&mut self, bus: &mut impl Bus) {
        self.pc = self.pop(bus);
        bus.tick();
    }

    fn ret_cc(&mut self, bus: &mut impl Bus, op: u8) {
        bus.tick();

        if self.satisfies_branch_condition(op) {
            self.ret(bus);
        }
    }

    fn reti(&mut self, bus: &mut impl Bus) {
        self.ret(bus);
        self.ime = true;
    }

    fn rla(&mut self) {
        let bit7 = self.af & 0x8000 != 0;
        let carry = self.af & CF != 0;

        self.af = ((self.af & 0xFF00) << 1) | (u16::from(carry) << 8);

        if bit7 {
            self.af |= CF;
        }
    }

    const fn rlca(&mut self) {
        let carry = (self.af & 0x8000) != 0;

        self.af = (self.af & 0xFF00) << 1;
        if carry {
            self.af |= CF | 0x0100;
        }
    }

    fn rra(&mut self) {
        let bit1 = self.af & 0x0100 != 0;
        let carry = self.af & CF != 0;

        self.af = (self.af >> 1) & 0xFF00 | (u16::from(carry) << 15);
        if bit1 {
            self.af |= CF;
        }
    }

    const fn rrca(&mut self) {
        let carry = self.af & 0x100 != 0;
        self.af = (self.af >> 1) & 0xFF00;
        if carry {
            self.af |= CF | 0x8000;
        }
    }

    fn rst(&mut self, bus: &mut impl Bus, op: u8) {
        self.push(bus, self.pc);
        self.pc = u16::from(op) ^ 0xC7;
    }

    const fn scf(&mut self) {
        self.af |= CF;
        self.af &= !(HF | NF);
    }

    fn stop(&mut self, bus: &mut impl Bus) {
        // Port of SameBoy's `stop`.
        bus.flush();

        let exit_by_joyp = bus.peek(io_addr(P1)) & 0xF != 0xF;
        let speed_switch = bus.speed_switch_requested() && !exit_by_joyp;
        let immediate_exit = speed_switch || exit_by_joyp;
        let interrupt_pending = bus.interrupts_pending();

        if !exit_by_joyp {
            if !immediate_exit {
                bus.dma_run(false);
            }
            bus.enter_stop(self.ime);
        }
        // A speed switch with an HBlank transfer requested prefetches the
        // second byte of STOP, which then runs as an opcode (gambatte).
        let prefetch = speed_switch && bus.hdma_request_pending();

        // When entering with IF&IE set, the second byte of STOP is actually
        // executed.
        if !interrupt_pending {
            let operand = bus.read(self.pc);
            if prefetch {
                self.prefetched = Some(operand);
            }
            if !prefetch {
                self.pc = self.pc.wrapping_add(1);
            }
        }

        if speed_switch {
            bus.begin_speed_switch(interrupt_pending);
        }

        if immediate_exit {
            bus.leave_stop();
            bus.dma_run(true);
            if interrupt_pending {
                bus.clear_speed_switch_halt();
            } else {
                bus.dma_run(false);
                self.is_halted = true;
                self.just_halted = true;
                bus.set_halted(true);
                if speed_switch {
                    // A transfer requested before the CPU stopped goes on
                    // during the wait of the speed switch.
                    bus.tick_hdma();
                }
            }
        }
    }
}
