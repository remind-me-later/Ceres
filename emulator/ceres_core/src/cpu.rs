use crate::Gb;

const ZF_B: u16 = 0x80;
const NF_B: u16 = 0x40;
const HF_B: u16 = 0x20;
const CF_B: u16 = 0x10;

impl Gb {
    pub(crate) fn run_cpu(&mut self) {
        if self.ei_delay {
            self.ime = true;
            self.ei_delay = false;
        }

        if self.cpu_halted {
            self.tick_m_cycle();
        } else {
            // println!("pc {:0x}", self.pc);

            let op = self.imm8();
            self.run_hdma();

            if self.halt_bug {
                self.pc = self.pc.wrapping_sub(1);
                self.halt_bug = false;
            }

            self.exec(op);
        }

        // any interrupts?
        if self.ifr & self.ie & 0x1F != 0 {
            // interrupt exits halt
            self.cpu_halted = false;

            if self.ime {
                self.tick_m_cycle();
                self.tick_m_cycle();

                self.push(self.pc);

                self.ime = false;
                // recompute, maybe ifr changed
                let ints = self.ifr & self.ie & 0x1F;
                let tz = (ints.trailing_zeros() & 7) as u16;
                // get rightmost interrupt
                let int = u8::from(ints != 0) << tz;
                // compute direction of interrupt vector
                self.pc = 0x40 | tz << 3;
                // acknowledge
                self.ifr &= !int;
            }
        }
    }

    #[inline]
    fn cpu_write(&mut self, addr: u16, val: u8) {
        self.tick_m_cycle();
        self.write_mem(addr, val);
    }

    #[inline]
    fn read(&mut self, addr: u16) -> u8 {
        self.tick_m_cycle();
        self.read_mem(addr)
    }

    #[inline]
    fn tick_m_cycle(&mut self) {
        self.advance_t_cycles(4);
    }

    #[inline]
    fn imm8(&mut self) -> u8 {
        let val = self.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        val
    }

    #[inline]
    fn imm16(&mut self) -> u16 {
        let lo = u16::from(self.imm8());
        let hi = u16::from(self.imm8());
        hi << 8 | lo
    }

    #[inline]
    fn rr(&mut self, id: u8) -> &mut u16 {
        match id {
            0 => &mut self.af,
            1 => &mut self.bc,
            2 => &mut self.de,
            3 => &mut self.hl,
            4 => &mut self.sp,
            _ => unreachable!(),
        }
    }

    fn get_r(&mut self, op: u8) -> u8 {
        let id = ((op >> 1) + 1) & 3;
        let lo = op & 1 != 0;
        if id == 0 {
            if lo {
                (self.af >> 8) as u8
            } else {
                self.read(self.hl)
            }
        } else if lo {
            (*self.rr(id) & 0xFF) as u8
        } else {
            (*self.rr(id) >> 8) as u8
        }
    }

    fn set_r(&mut self, op: u8, val: u8) {
        let id = ((op >> 1) + 1) & 3;
        let lo = op & 1 != 0;
        if id == 0 {
            if lo {
                self.af = u16::from(val) << 8 | self.af & 0xFF;
            } else {
                self.cpu_write(self.hl, val);
            }
        } else if lo {
            let rr = self.rr(id);
            *rr = u16::from(val) | *rr & 0xFF00;
        } else {
            let rr = self.rr(id);
            *rr = u16::from(val) << 8 | *rr & 0xFF;
        }
    }

    #[inline]
    fn ld(&mut self, op: u8) {
        let val = self.get_r(op);
        self.set_r(op >> 3, val);
    }

    #[inline]
    fn ld_a_drr(&mut self, op: u8) {
        let reg_id = (op >> 4) + 1;
        self.af &= 0xFF;
        let tmp = *self.rr(reg_id);
        self.af |= u16::from(self.read(tmp)) << 8;
    }

    #[inline]
    fn ld_drr_a(&mut self, op: u8) {
        let reg_id = (op >> 4) + 1;
        let tmp = *self.rr(reg_id);
        self.cpu_write(tmp, (self.af >> 8) as u8);
    }

    #[inline]
    fn ld_da16_a(&mut self) {
        let addr = self.imm16();
        self.cpu_write(addr, (self.af >> 8) as u8);
    }

    #[inline]
    fn ld_a_da16(&mut self) {
        self.af &= 0xFF;
        let addr = self.imm16();
        self.af |= u16::from(self.read(addr)) << 8;
    }

    #[inline]
    fn ld_dhli_a(&mut self) {
        let addr = self.hl;
        self.cpu_write(addr, (self.af >> 8) as u8);
        self.hl = addr.wrapping_add(1);
    }

    #[inline]
    fn ld_dhld_a(&mut self) {
        let addr = self.hl;
        self.cpu_write(addr, (self.af >> 8) as u8);
        self.hl = addr.wrapping_sub(1);
    }

    #[inline]
    fn ld_a_dhli(&mut self) {
        let addr = self.hl;
        let val = u16::from(self.read(addr));
        self.af &= 0xFF;
        self.af |= val << 8;
        self.hl = addr.wrapping_add(1);
    }

    #[inline]
    fn ld_a_dhld(&mut self) {
        let addr = self.hl;
        let val = u16::from(self.read(addr));
        self.af &= 0xFF;
        self.af |= val << 8;
        self.hl = addr.wrapping_sub(1);
    }

    #[inline]
    fn ldh_da8_a(&mut self) {
        let tmp = u16::from(self.imm8());
        let a = (self.af >> 8) as u8;
        self.cpu_write(0xFF00 | tmp, a);
    }

    #[inline]
    fn ldh_a_da8(&mut self) {
        let tmp = u16::from(self.imm8());
        self.af &= 0xFF;
        self.af |= u16::from(self.read(0xFF00 | tmp)) << 8;
    }

    #[inline]
    fn ldh_dc_a(&mut self) {
        self.cpu_write(0xFF00 | self.bc & 0xFF, (self.af >> 8) as u8);
    }

    #[inline]
    fn ldh_a_dc(&mut self) {
        self.af &= 0xFF;
        self.af |= u16::from(self.read(0xFF00 | self.bc & 0xFF)) << 8;
    }

    #[inline]
    fn ld_hr_d8(&mut self, op: u8) {
        let reg_id = ((op >> 4) + 1) & 0x03;
        *self.rr(reg_id) &= 0xFF;
        let tmp = u16::from(self.imm8());
        *self.rr(reg_id) |= tmp << 8;
    }

    #[inline]
    fn ld_lr_d8(&mut self, op: u8) {
        let reg_id = (op >> 4) + 1;
        *self.rr(reg_id) &= 0xFF00;
        let tmp = u16::from(self.imm8());
        *self.rr(reg_id) |= tmp;
    }

    #[inline]
    fn ld_dhl_d8(&mut self) {
        let tmp = self.imm8();
        self.cpu_write(self.hl, tmp);
    }

    #[inline]
    fn ld16_sp_hl(&mut self) {
        let val = self.hl;
        self.sp = val;
        self.tick_m_cycle();
    }

    #[inline]
    fn add(&mut self, val: u16) {
        let a = self.af >> 8;
        let res = a + val;
        self.af = res << 8;
        if res & 0xFF == 0 {
            self.af |= ZF_B;
        }
        if (a & 0xF) + (val & 0xF) > 0x0F {
            self.af |= HF_B;
        }
        if res > 0xFF {
            self.af |= CF_B;
        }
    }

    #[inline]
    fn add_a_r(&mut self, op: u8) {
        let val = u16::from(self.get_r(op));
        self.add(val);
    }

    #[inline]
    fn add_a_d8(&mut self) {
        let val = u16::from(self.imm8());
        self.add(val);
    }

    #[inline]
    fn sub(&mut self, val: u16) {
        let a = self.af >> 8;
        self.af = (a.wrapping_sub(val) << 8) | NF_B;
        if a == val {
            self.af |= ZF_B;
        }
        if (a & 0xF) < (val & 0xF) {
            self.af |= HF_B;
        }
        if a < val {
            self.af |= CF_B;
        }
    }

    #[inline]
    fn sub_a_r(&mut self, op: u8) {
        let val = u16::from(self.get_r(op));
        self.sub(val);
    }

    #[inline]
    fn sub_a_d8(&mut self) {
        let val = u16::from(self.imm8());
        self.sub(val);
    }

    #[inline]
    fn sbc(&mut self, val: u16) {
        let a = self.af >> 8;
        let carry = u16::from((self.af & CF_B) != 0);
        let res = a.wrapping_sub(val).wrapping_sub(carry);
        self.af = (res << 8) | NF_B;

        if res & 0xFF == 0 {
            self.af |= ZF_B;
        }
        if (a & 0xF) < (val & 0xF) + carry {
            self.af |= HF_B;
        }
        if res > 0xFF {
            self.af |= CF_B;
        }
    }

    #[inline]
    fn sbc_a_r(&mut self, op: u8) {
        let val = u16::from(self.get_r(op));
        self.sbc(val);
    }

    #[inline]
    fn sbc_a_d8(&mut self) {
        let val = u16::from(self.imm8());
        self.sbc(val);
    }

    #[inline]
    fn adc(&mut self, val: u16) {
        let a = self.af >> 8;
        let carry = u16::from((self.af & CF_B) != 0);
        let res = a + val + carry;
        self.af = res << 8;
        if res & 0xFF == 0 {
            self.af |= ZF_B;
        }
        if (a & 0xF) + (val & 0xF) + carry > 0x0F {
            self.af |= HF_B;
        }
        if res > 0xFF {
            self.af |= CF_B;
        }
    }

    #[inline]
    fn adc_a_r(&mut self, op: u8) {
        let val = u16::from(self.get_r(op));
        self.adc(val);
    }

    #[inline]
    fn adc_a_d8(&mut self) {
        let val = u16::from(self.imm8());
        self.adc(val);
    }

    #[inline]
    fn or(&mut self, val: u16) {
        let a = self.af >> 8;
        self.af = (a | val) << 8;
        if (a | val) == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn or_a_r(&mut self, op: u8) {
        let val = u16::from(self.get_r(op));
        self.or(val);
    }

    #[inline]
    fn or_a_d8(&mut self) {
        let val = u16::from(self.imm8());
        self.or(val);
    }

    #[inline]
    fn xor(&mut self, val: u16) {
        let a = self.af >> 8;
        let a = a ^ val;
        self.af = a << 8;
        if a == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn xor_a_r(&mut self, op: u8) {
        let val = u16::from(self.get_r(op));
        self.xor(val);
    }

    #[inline]
    fn xor_a_d8(&mut self) {
        let val = u16::from(self.imm8());
        self.xor(val);
    }

    #[inline]
    fn and(&mut self, val: u16) {
        let a = self.af >> 8;
        let a = a & val;
        self.af = (a << 8) | HF_B;
        if a == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn and_a_r(&mut self, op: u8) {
        let val = u16::from(self.get_r(op));
        self.and(val);
    }

    #[inline]
    fn and_a_d8(&mut self) {
        let val = u16::from(self.imm8());
        self.and(val);
    }

    #[inline]
    fn cp(&mut self, val: u16) {
        let a = self.af >> 8;
        self.af &= 0xFF00;
        self.af |= NF_B;
        if a == val {
            self.af |= ZF_B;
        }
        if (a & 0xF) < (val & 0xF) {
            self.af |= HF_B;
        }
        if a < val {
            self.af |= CF_B;
        }
    }

    #[inline]
    fn cp_a_r(&mut self, op: u8) {
        let val = u16::from(self.get_r(op));
        self.cp(val);
    }

    #[inline]
    fn cp_a_d8(&mut self) {
        let val = u16::from(self.imm8());
        self.cp(val);
    }

    #[inline]
    fn inc_lr(&mut self, op: u8) {
        let reg_id = (op >> 4) + 1;
        let val = self.rr(reg_id).wrapping_add(1) & 0xFF;
        *self.rr(reg_id) = (*self.rr(reg_id) & 0xFF00) | val;

        self.af &= !(NF_B | ZF_B | HF_B);

        if ((*self.rr(reg_id)) & 0x0F) == 0 {
            self.af |= HF_B;
        }

        if ((*self.rr(reg_id)) & 0xFF) == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn dec_lr(&mut self, op: u8) {
        let reg_id = (op >> 4) + 1;
        let val = (*self.rr(reg_id)).wrapping_sub(1) & 0xFF;
        (*self.rr(reg_id)) = ((*self.rr(reg_id)) & 0xFF00) | val;

        self.af &= !(ZF_B | HF_B);
        self.af |= NF_B;

        if ((*self.rr(reg_id)) & 0x0F) == 0xF {
            self.af |= HF_B;
        }

        if ((*self.rr(reg_id)) & 0xFF) == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn inc_hr(&mut self, op: u8) {
        let reg_id = ((op >> 4) + 1) & 0x03;
        *self.rr(reg_id) = (*self.rr(reg_id)).wrapping_add(0x100);
        self.af &= !(NF_B | ZF_B | HF_B);

        if ((*self.rr(reg_id)) & 0x0F00) == 0 {
            self.af |= HF_B;
        }

        if ((*self.rr(reg_id)) & 0xFF00) == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn dec_hr(&mut self, op: u8) {
        let reg_id = ((op >> 4) + 1) & 0x03;
        *self.rr(reg_id) = (*self.rr(reg_id)).wrapping_sub(0x100);
        self.af &= !(ZF_B | HF_B);
        self.af |= NF_B;

        if ((*self.rr(reg_id)) & 0x0F00) == 0xF00 {
            self.af |= HF_B;
        }

        if ((*self.rr(reg_id)) & 0xFF00) == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn inc_dhl(&mut self) {
        let val = self.read(self.hl).wrapping_add(1);
        self.cpu_write(self.hl, val);

        self.af &= !(NF_B | ZF_B | HF_B);
        if (val & 0x0F) == 0 {
            self.af |= HF_B;
        }

        if val == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn dec_dhl(&mut self) {
        let val = self.read(self.hl).wrapping_sub(1);
        self.cpu_write(self.hl, val);

        self.af &= !(ZF_B | HF_B);
        self.af |= NF_B;
        if (val & 0x0F) == 0x0F {
            self.af |= HF_B;
        }

        if val == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn inc_rr(&mut self, op: u8) {
        let reg_id = (op >> 4) + 1;
        let reg = self.rr(reg_id);
        *reg = reg.wrapping_add(1);
        self.tick_m_cycle();
    }

    #[inline]
    fn dec_rr(&mut self, op: u8) {
        let reg_id = (op >> 4) + 1;
        let reg = self.rr(reg_id);
        *reg = reg.wrapping_sub(1);
        self.tick_m_cycle();
    }

    #[inline]
    fn ld_hl_sp_r8(&mut self) {
        self.af &= 0xFF00;
        #[allow(clippy::cast_possible_wrap)]
        let offset_signed = self.imm8() as i8;
        #[allow(clippy::cast_sign_loss)]
        let offset = offset_signed as u16;
        self.tick_m_cycle();
        self.hl = self.sp.wrapping_add(offset);

        if (self.sp & 0xF) + (offset & 0xF) > 0xF {
            self.af |= HF_B;
        }

        if (self.sp & 0xFF) + (offset & 0xFF) > 0xFF {
            self.af |= CF_B;
        }
    }

    #[inline]
    fn rlca(&mut self) {
        let carry = (self.af & 0x8000) != 0;

        self.af = (self.af & 0xFF00) << 1;
        if carry {
            self.af |= CF_B | 0x0100;
        }
    }

    #[inline]
    fn rrca(&mut self) {
        let carry = self.af & 0x100 != 0;
        self.af = (self.af >> 1) & 0xFF00;
        if carry {
            self.af |= CF_B | 0x8000;
        }
    }

    #[inline]
    fn rrc_r(&mut self, op: u8) {
        let val = self.get_r(op);
        let carry = (val & 0x01) != 0;
        self.af &= 0xFF00;
        let val = val >> 1 | u8::from(carry) << 7;
        self.set_r(op, val);
        if carry {
            self.af |= CF_B;
        }
        if val == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn do_jump_to_immediate(&mut self) {
        let addr = self.imm16();
        self.pc = addr;
        self.tick_m_cycle();
    }

    #[inline]
    fn jp_a16(&mut self) {
        self.do_jump_to_immediate();
    }

    #[inline]
    fn jp_cc(&mut self, op: u8) {
        if self.condition(op) {
            self.do_jump_to_immediate();
        } else {
            let pc = self.pc.wrapping_add(2);
            self.pc = pc;
            self.tick_m_cycle();
            self.tick_m_cycle();
        }
    }

    #[inline]
    fn jp_hl(&mut self) {
        self.pc = self.hl;
    }

    #[inline]
    fn do_jump_relative(&mut self) {
        #[allow(clippy::cast_possible_wrap)]
        let relative_addr_signed = self.imm8() as i8;
        #[allow(clippy::cast_sign_loss)]
        let relative_addr = relative_addr_signed as u16;

        let pc = self.pc.wrapping_add(relative_addr);
        self.pc = pc;
        self.tick_m_cycle();
    }

    #[inline]
    fn jr_d(&mut self) {
        self.do_jump_relative();
    }

    #[inline]
    fn jr_cc(&mut self, op: u8) {
        if self.condition(op) {
            self.do_jump_relative();
        } else {
            self.pc = self.pc.wrapping_add(1);
            self.tick_m_cycle();
        }
    }

    #[inline]
    fn do_call(&mut self) {
        let addr = self.imm16();
        self.push(self.pc);
        self.pc = addr;
    }

    #[inline]
    fn call_nn(&mut self) {
        self.do_call();
    }

    #[inline]
    fn call_cc_a16(&mut self, op: u8) {
        if self.condition(op) {
            self.do_call();
        } else {
            let pc = self.pc.wrapping_add(2);
            self.pc = pc;
            self.tick_m_cycle();
            self.tick_m_cycle();
        }
    }

    #[inline]
    fn ret(&mut self) {
        self.pc = self.pop();
        self.tick_m_cycle();
    }

    #[inline]
    fn reti(&mut self) {
        self.ret();
        self.ime = true;
    }

    #[inline]
    fn ret_cc(&mut self, op: u8) {
        self.tick_m_cycle();

        if self.condition(op) {
            self.ret();
        }
    }

    const fn condition(&self, op: u8) -> bool {
        match (op >> 3) & 3 {
            0 => self.af & ZF_B == 0,
            1 => self.af & ZF_B != 0,
            2 => self.af & CF_B == 0,
            3 => self.af & CF_B != 0,
            _ => unreachable!(),
        }
    }

    #[inline]
    fn rst(&mut self, op: u8) {
        self.push(self.pc);
        self.pc = u16::from(op) ^ 0xC7;
    }

    #[inline]
    fn halt(&mut self) {
        if self.ie & self.ifr & 0x1F == 0 {
            self.cpu_halted = true;
        } else if self.ime {
            self.cpu_halted = false;
            self.pc = self.pc.wrapping_sub(1);
        } else {
            self.cpu_halted = false;
            self.halt_bug = true;
        }
    }

    #[inline]
    fn stop(&mut self) {
        self.imm8();

        if self.key1_req {
            self.key1_ena = !self.key1_ena;
            self.key1_req = false;

            // TODO: div should not tick
            for _ in 0..2050 {
                self.tick_m_cycle();
            }
        } else {
            self.cpu_halted = true;
        }
    }

    #[inline]
    fn di(&mut self) {
        self.ime = false;
    }

    #[inline]
    fn ei(&mut self) {
        self.ei_delay = true;
    }

    #[inline]
    fn ccf(&mut self) {
        self.af ^= CF_B;
        self.af &= !(HF_B | NF_B);
    }

    #[inline]
    fn scf(&mut self) {
        self.af |= CF_B;
        self.af &= !(HF_B | NF_B);
    }

    #[allow(clippy::unused_self)]
    #[inline]
    fn nop(&mut self) {}

    // TODO: debugger breakpoint

    #[inline]
    fn ld_b_b(&mut self) {
        self.nop();
    }

    #[inline]
    fn daa(&mut self) {
        let mut res = self.af >> 8;

        self.af &= !(0xFF00 | ZF_B);

        if self.af & NF_B == 0 {
            if self.af & HF_B != 0 || res & 0x0F > 0x09 {
                res += 0x06;
            }

            if self.af & CF_B != 0 || res > 0x9F {
                res += 0x60;
            }
        } else {
            if self.af & HF_B != 0 {
                res = res.wrapping_sub(0x06) & 0xFF;
            }

            if self.af & CF_B != 0 {
                res = res.wrapping_sub(0x60);
            }
        }

        let res = res;

        if res & 0xFF == 0 {
            self.af |= ZF_B;
        }

        if res & 0x100 == 0x100 {
            self.af |= CF_B;
        }

        self.af &= !HF_B;
        self.af |= res << 8;
    }

    #[inline]
    fn cpl(&mut self) {
        self.af ^= 0xFF00;
        self.af |= HF_B | NF_B;
    }

    #[inline]
    fn push(&mut self, val: u16) {
        self.sp = self.sp.wrapping_sub(1);
        self.cpu_write(self.sp, (val >> 8) as u8);
        self.sp = self.sp.wrapping_sub(1);
        self.cpu_write(self.sp, (val & 0xFF) as u8);
        self.tick_m_cycle();
    }

    #[inline]
    fn push_rr(&mut self, op: u8) {
        let reg_id = ((op >> 4) + 1) & 3;
        let val = *self.rr(reg_id);
        self.push(val);
    }

    #[inline]
    fn pop(&mut self) -> u16 {
        let val = u16::from(self.read(self.sp));
        self.sp = self.sp.wrapping_add(1);
        let val = val | u16::from(self.read(self.sp)) << 8;
        self.sp = self.sp.wrapping_add(1);
        val
    }

    #[inline]
    fn pop_rr(&mut self, op: u8) {
        let val = self.pop();
        let reg_id = ((op >> 4) + 1) & 3;
        *self.rr(reg_id) = val;
        self.af &= 0xFFF0;
    }

    #[inline]
    fn ld_rr_d16(&mut self, op: u8) {
        let reg_id = (op >> 4) + 1;
        let imm = self.imm16();
        *self.rr(reg_id) = imm;
    }

    #[inline]
    fn ld_da16_sp(&mut self) {
        let val = self.sp;
        let addr = self.imm16();
        self.cpu_write(addr, (val & 0xFF) as u8);
        self.cpu_write(addr.wrapping_add(1), (val >> 8) as u8);
    }

    #[inline]
    fn add_sp_r8(&mut self) {
        let sp = self.sp;
        #[allow(clippy::cast_possible_wrap)]
        let offset_signed = self.imm8() as i8;
        #[allow(clippy::cast_sign_loss)]
        let offset = offset_signed as u16;
        self.tick_m_cycle();
        self.tick_m_cycle();
        self.sp = self.sp.wrapping_add(offset);
        self.af &= 0xFF00;

        if (sp & 0xF) + (offset & 0xF) > 0xF {
            self.af |= HF_B;
        }

        if (sp & 0xFF) + (offset & 0xFF) > 0xFF {
            self.af |= CF_B;
        }
    }

    #[inline]
    fn add_hl_rr(&mut self, op: u8) {
        let reg_id = (op >> 4) + 1;
        let hl = self.hl;
        let rr = *self.rr(reg_id);
        self.hl = hl.wrapping_add(rr);

        self.af &= !(NF_B | CF_B | HF_B);

        if ((hl & 0xFFF) + (rr & 0xFFF)) & 0x1000 != 0 {
            self.af |= HF_B;
        }

        if (u32::from(hl) + u32::from(rr)) & 0x10000 != 0 {
            self.af |= CF_B;
        }

        self.tick_m_cycle();
    }

    #[inline]
    fn rlc_r(&mut self, op: u8) {
        let val = self.get_r(op);
        let carry = val & 0x80 != 0;
        self.af &= 0xFF00;
        self.set_r(op, val << 1 | u8::from(carry));
        if carry {
            self.af |= CF_B;
        }
        if val == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn rra(&mut self) {
        let bit1 = self.af & 0x0100 != 0;
        let carry = self.af & CF_B != 0;

        self.af = (self.af >> 1) & 0xFF00 | u16::from(carry) << 15;

        if bit1 {
            self.af |= CF_B;
        }
    }

    #[inline]
    fn rr_r(&mut self, op: u8) {
        let val = self.get_r(op);
        let carry = (self.af & CF_B) != 0;
        let bit1 = (val & 1) != 0;

        self.af &= 0xFF00;
        let val = val >> 1 | u8::from(carry) << 7;
        self.set_r(op, val);
        if bit1 {
            self.af |= CF_B;
        }
        if val == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn sla_r(&mut self, op: u8) {
        let val = self.get_r(op);
        let carry = val & 0x80 != 0;
        self.af &= 0xFF00;
        let res = val << 1;
        self.set_r(op, res);
        if carry {
            self.af |= CF_B;
        }
        if res == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn sra_r(&mut self, op: u8) {
        let val = self.get_r(op);
        let bit7 = val & 0x80;
        self.af &= 0xFF00;
        if val & 1 != 0 {
            self.af |= CF_B;
        }
        let val = (val >> 1) | bit7;
        self.set_r(op, val);
        if val == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn srl_r(&mut self, op: u8) {
        let val = self.get_r(op);
        self.af &= 0xFF00;
        self.set_r(op, val >> 1);
        if val & 1 != 0 {
            self.af |= CF_B;
        }
        if val >> 1 == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn swap_r(&mut self, op: u8) {
        let val = self.get_r(op);
        self.af &= 0xFF00;
        self.set_r(op, (val >> 4) | (val << 4));
        if val == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn bit_r(&mut self, op: u8) {
        let val = self.get_r(op);
        let bit_no = (op >> 3) & 7;
        let bit = 1 << bit_no;
        if op & 0xC0 == 0x40 {
            // bit
            self.af &= 0xFF00 | CF_B;
            self.af |= HF_B;
            if bit & val == 0 {
                self.af |= ZF_B;
            }
        } else if op & 0xC0 == 0x80 {
            // res
            self.set_r(op, val & !bit);
        } else {
            // set
            self.set_r(op, val | bit);
        }
    }

    #[inline]
    fn rla(&mut self) {
        let bit7 = (self.af & 0x8000) != 0;
        let carry = (self.af & CF_B) != 0;

        self.af = (self.af & 0xFF00) << 1 | u16::from(carry) << 8;

        if bit7 {
            self.af |= CF_B;
        }
    }

    #[inline]
    fn rl_r(&mut self, op: u8) {
        let val = self.get_r(op);
        let carry = self.af & CF_B != 0;
        let bit7 = val & 0x80 != 0;

        self.af &= 0xFF00;
        let val = val << 1 | u8::from(carry);
        self.set_r(op, val);
        if bit7 {
            self.af |= CF_B;
        }
        if val == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn ill(&mut self, _op: u8) {
        self.ie = 0;
        self.cpu_halted = true;
    }

    #[inline]
    fn exec(&mut self, op: u8) {
        match op {
            0x00 | 0x5B | 0x6D | 0x7F | 0x49 | 0x52 | 0x64 => self.nop(),
            0x01 | 0x11 | 0x21 | 0x31 => self.ld_rr_d16(op),
            0x02 | 0x12 => self.ld_drr_a(op),
            0x03 | 0x13 | 0x23 | 0x33 => self.inc_rr(op),
            0x04 | 0x14 | 0x24 | 0x3C => self.inc_hr(op),
            0x05 | 0x15 | 0x25 | 0x3D => self.dec_hr(op),
            0x06 | 0x16 | 0x26 | 0x3E => self.ld_hr_d8(op),
            0x07 => self.rlca(),
            0x08 => self.ld_da16_sp(),
            0x09 | 0x19 | 0x29 | 0x39 => self.add_hl_rr(op),
            0x0A | 0x1A => self.ld_a_drr(op),
            0x0B | 0x1B | 0x2B | 0x3B => self.dec_rr(op),
            0x0C | 0x1C | 0x2C => self.inc_lr(op),
            0x0D | 0x1D | 0x2D => self.dec_lr(op),
            0x0E | 0x1E | 0x2E => self.ld_lr_d8(op),
            0x0F => self.rrca(),
            0x10 => self.stop(),
            0x17 => self.rla(),
            0x18 => self.jr_d(),
            0x1F => self.rra(),
            0x20 | 0x28 | 0x30 | 0x38 => self.jr_cc(op),
            0x22 => self.ld_dhli_a(),
            0x27 => self.daa(),
            0x2A => self.ld_a_dhli(),
            0x2F => self.cpl(),
            0x32 => self.ld_dhld_a(),
            0x34 => self.inc_dhl(),
            0x35 => self.dec_dhl(),
            0x36 => self.ld_dhl_d8(),
            0x37 => self.scf(),
            0x3A => self.ld_a_dhld(),
            0x3F => self.ccf(),
            0x40 => self.ld_b_b(),
            0x41 | 0x42 | 0x43 | 0x44 | 0x45 | 0x46 | 0x47 | 0x4A | 0x4B | 0x4C | 0x4D | 0x4E
            | 0x4F | 0x48 | 0x50 | 0x51 | 0x53 | 0x54 | 0x55 | 0x56 | 0x57 | 0x5A | 0x5C | 0x5D
            | 0x5E | 0x5F | 0x58 | 0x59 | 0x60 | 0x61 | 0x62 | 0x63 | 0x65 | 0x66 | 0x67 | 0x6A
            | 0x6B | 0x6C | 0x6E | 0x6F | 0x68 | 0x69 | 0x7A | 0x7B | 0x7C | 0x7D | 0x7E | 0x78
            | 0x79 | 0x77 | 0x70 | 0x73 | 0x72 | 0x71 | 0x74 | 0x75 => self.ld(op),
            0x76 => self.halt(),
            0x80 | 0x81 | 0x82 | 0x83 | 0x84 | 0x85 | 0x86 | 0x87 => self.add_a_r(op),
            0x88 | 0x89 | 0x8A | 0x8B | 0x8C | 0x8D | 0x8E | 0x8F => self.adc_a_r(op),
            0x90 | 0x91 | 0x92 | 0x93 | 0x94 | 0x95 | 0x96 | 0x97 => self.sub_a_r(op),
            0x98 | 0x99 | 0x9A | 0x9B | 0x9C | 0x9D | 0x9E | 0x9F => self.sbc_a_r(op),
            0xA0 | 0xA1 | 0xA2 | 0xA3 | 0xA4 | 0xA5 | 0xA6 | 0xA7 => self.and_a_r(op),
            0xA8 | 0xA9 | 0xAA | 0xAB | 0xAC | 0xAD | 0xAE | 0xAF => self.xor_a_r(op),
            0xB0 | 0xB1 | 0xB2 | 0xB3 | 0xB4 | 0xB5 | 0xB6 | 0xB7 => self.or_a_r(op),
            0xB8 | 0xB9 | 0xBA | 0xBB | 0xBC | 0xBD | 0xBE | 0xBF => self.cp_a_r(op),
            0xC0 | 0xC8 | 0xD0 | 0xD8 => self.ret_cc(op),
            0xC1 | 0xD1 | 0xE1 | 0xF1 => self.pop_rr(op),
            0xC2 | 0xCA | 0xD2 | 0xDA => self.jp_cc(op),
            0xC3 => self.jp_a16(),
            0xC4 | 0xCC | 0xD4 | 0xDC => self.call_cc_a16(op),
            0xC5 | 0xD5 | 0xE5 | 0xF5 => self.push_rr(op),
            0xC6 => self.add_a_d8(),
            0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => self.rst(op),
            0xC9 => self.ret(),
            0xCB => self.exec_cb(),
            0xCD => self.call_nn(),
            0xCE => self.adc_a_d8(),
            0xD6 => self.sub_a_d8(),
            0xD9 => self.reti(),
            0xDE => self.sbc_a_d8(),
            0xE0 => self.ldh_da8_a(),
            0xE2 => self.ldh_dc_a(),
            0xE6 => self.and_a_d8(),
            0xE8 => self.add_sp_r8(),
            0xE9 => self.jp_hl(),
            0xEA => self.ld_da16_a(),
            0xEE => self.xor_a_d8(),
            0xF0 => self.ldh_a_da8(),
            0xF2 => self.ldh_a_dc(),
            0xF3 => self.di(),
            0xF6 => self.or_a_d8(),
            0xF8 => self.ld_hl_sp_r8(),
            0xF9 => self.ld16_sp_hl(),
            0xFA => self.ld_a_da16(),
            0xFB => self.ei(),
            0xFE => self.cp_a_d8(),
            _ => self.ill(op),
        }
    }

    #[inline]
    fn exec_cb(&mut self) {
        let op = self.imm8();
        match op >> 3 {
            0 => self.rlc_r(op),
            1 => self.rrc_r(op),
            2 => self.rl_r(op),
            3 => self.rr_r(op),
            4 => self.sla_r(op),
            5 => self.sra_r(op),
            6 => self.swap_r(op),
            7 => self.srl_r(op),
            _ => self.bit_r(op),
        }
    }
}
