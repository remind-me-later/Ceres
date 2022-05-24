use {
    crate::{
        Gb, IF_LCD_B, IF_P1_B, IF_SERIAL_B, IF_TIMER_B, IF_VBLANK_B, KEY1_SPEED_B, KEY1_SWITCH_B,
    },
    Ld8::{Dhl, A, B, C, D, E, H, L},
};

const ZF_B: u16 = 0x80;
const NF_B: u16 = 0x40;
const HF_B: u16 = 0x20;
const CF_B: u16 = 0x10;

enum Ld8 {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
    Dhl,
}

impl Ld8 {
    #[inline]
    fn read(self, gb: &mut Gb) -> u8 {
        match self {
            Ld8::A => (gb.af >> 8) as u8,
            Ld8::B => (gb.bc >> 8) as u8,
            Ld8::C => gb.bc as u8,
            Ld8::D => (gb.de >> 8) as u8,
            Ld8::E => gb.de as u8,
            Ld8::H => (gb.hl >> 8) as u8,
            Ld8::L => gb.hl as u8,
            Ld8::Dhl => gb.cpu_read(gb.hl),
        }
    }

    #[inline]
    fn write(self, gb: &mut Gb, val: u8) {
        match self {
            Ld8::A => {
                gb.af &= 0x00ff;
                gb.af |= (val as u16) << 8;
            }
            Ld8::B => {
                gb.bc &= 0x00ff;
                gb.bc |= (val as u16) << 8;
            }
            Ld8::C => {
                gb.bc &= 0xff00;
                gb.bc |= val as u16;
            }
            Ld8::D => {
                gb.de &= 0x00ff;
                gb.de |= (val as u16) << 8;
            }
            Ld8::E => {
                gb.de &= 0xff00;
                gb.de |= val as u16;
            }
            Ld8::H => {
                gb.hl &= 0x00ff;
                gb.hl |= (val as u16) << 8;
            }
            Ld8::L => {
                gb.hl &= 0xff00;
                gb.hl |= val as u16;
            }
            Ld8::Dhl => gb.cpu_write(gb.hl, val),
        }
    }
}

impl Gb {
    pub(crate) fn run(&mut self) {
        if self.cpu_ei_delay {
            self.ime = true;
            self.cpu_ei_delay = false;
        }

        if self.cpu_halted {
            self.advance_cycle();
        } else {
            let opcode = self.imm_u8();

            self.run_hdma();

            self.exec(opcode);
        }

        if self.ifr & self.ie & 0x1f == 0 {
            return;
        }

        // handle interrupt
        if self.cpu_halted {
            self.cpu_halted = false;
        }

        if self.ime {
            self.ime = false;

            self.advance_cycle();

            self.sp = self.sp.wrapping_sub(1);
            self.cpu_write(self.sp, (self.pc >> 8) as u8);
            self.sp = self.sp.wrapping_sub(1);
            self.cpu_write(self.sp, (self.pc & 0xFF) as u8);

            let interrupt = {
                let pending = self.ifr & self.ie & 0x1f;

                if pending == 0 {
                    0
                } else {
                    // get rightmost interrupt
                    1 << pending.trailing_zeros()
                }
            };

            self.pc = match interrupt {
                IF_VBLANK_B => 0x40,
                IF_LCD_B => 0x48,
                IF_TIMER_B => 0x50,
                IF_SERIAL_B => 0x58,
                IF_P1_B => 0x60,
                _ => unreachable!(),
            };

            self.advance_cycle();
            // acknowledge
            self.ifr &= !interrupt;
        }
    }

    #[inline]
    fn cpu_write(&mut self, addr: u16, val: u8) {
        self.advance_cycle();
        self.write_mem(addr, val);
    }

    #[inline]
    fn cpu_read(&mut self, addr: u16) -> u8 {
        self.advance_cycle();
        self.read_mem(addr)
    }

    #[inline]
    fn imm_u8(&mut self) -> u8 {
        let val = self.cpu_read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        val
    }

    #[inline]
    fn imm_u16(&mut self) -> u16 {
        let lo = self.imm_u8() as u16;
        let hi = self.imm_u8() as u16;
        hi << 8 | lo
    }

    #[inline]
    fn regid2reg(&mut self, id: u8) -> &mut u16 {
        match id {
            0 => &mut self.af,
            1 => &mut self.bc,
            2 => &mut self.de,
            3 => &mut self.hl,
            4 => &mut self.sp,
            _ => unreachable!(),
        }
    }

    #[inline]
    fn get_src_val(&mut self, opcode: u8) -> u8 {
        let reg_id = ((opcode >> 1) + 1) & 3;
        let lo = opcode & 1 != 0;
        if reg_id == 0 {
            if lo {
                return (self.af >> 8) as u8;
            }
            return self.cpu_read(self.hl);
        }
        if lo {
            return *self.regid2reg(reg_id) as u8;
        }
        return (*self.regid2reg(reg_id) >> 8) as u8;
    }

    #[inline]
    fn set_src_val(&mut self, opcode: u8, val: u8) {
        let reg_id = ((opcode >> 1) + 1) & 3;
        let lo = opcode & 1 != 0;

        if reg_id == 0 {
            if lo {
                self.af &= 0xFF;
                self.af |= (val as u16) << 8;
            } else {
                self.cpu_write(self.hl, val);
            }
        } else if lo {
            *self.regid2reg(reg_id) &= 0xFF00;
            *self.regid2reg(reg_id) |= val as u16;
        } else {
            *self.regid2reg(reg_id) &= 0xFF;
            *self.regid2reg(reg_id) |= (val as u16) << 8;
        }
    }

    #[inline]
    fn ld(&mut self, t: Ld8, s: Ld8) {
        let val = s.read(self);
        t.write(self, val);
    }

    #[inline]
    fn ld_a_drr(&mut self, opcode: u8) {
        let reg_id = (opcode >> 4) + 1;
        self.af &= 0xFF;
        let tmp = *self.regid2reg(reg_id);
        self.af |= (self.cpu_read(tmp) as u16) << 8;
    }

    #[inline]
    fn ld_drr_a(&mut self, opcode: u8) {
        let reg_id = (opcode >> 4) + 1;
        let tmp = *self.regid2reg(reg_id);
        self.cpu_write(tmp, (self.af >> 8) as u8);
    }

    #[inline]
    fn ld_da16_a(&mut self) {
        let addr = self.imm_u16();
        self.cpu_write(addr, (self.af >> 8) as u8);
    }

    #[inline]
    fn ld_a_da16(&mut self) {
        self.af &= 0xFF;
        let addr = self.imm_u16();
        self.af |= (self.cpu_read(addr) as u16) << 8;
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
        let val = self.cpu_read(addr) as u16;
        self.af &= 0xFF;
        self.af |= val << 8;
        self.hl = addr.wrapping_add(1);
    }

    #[inline]
    fn ld_a_dhld(&mut self) {
        let addr = self.hl;
        let val = self.cpu_read(addr) as u16;
        self.af &= 0xFF;
        self.af |= val << 8;
        self.hl = addr.wrapping_sub(1);
    }

    #[inline]
    fn ld_da8_a(&mut self) {
        let tmp = self.imm_u8() as u16;
        self.cpu_write(0xFF00 | tmp, (self.af >> 8) as u8);
    }

    #[inline]
    fn ld_a_da8(&mut self) {
        let tmp = self.imm_u8() as u16;
        self.af &= 0xFF;
        self.af |= (self.cpu_read(0xFF00 | tmp) as u16) << 8;
    }

    #[inline]
    fn ld_dc_a(&mut self) {
        self.cpu_write(0xFF00 | self.bc & 0xFF, (self.af >> 8) as u8);
    }

    #[inline]
    fn ld_a_dc(&mut self) {
        self.af &= 0xFF;
        self.af |= (self.cpu_read(0xFF00 | self.bc & 0xFF) as u16) << 8;
    }

    #[inline]
    fn ld_hr_d8(&mut self, opcode: u8) {
        let reg_id = ((opcode >> 4) + 1) & 0x03;
        *self.regid2reg(reg_id) &= 0xFF;
        let tmp = self.imm_u8() as u16;
        *self.regid2reg(reg_id) |= tmp << 8;
    }

    #[inline]
    fn ld_lr_d8(&mut self, opcode: u8) {
        let reg_id = (opcode >> 4) + 1;
        *self.regid2reg(reg_id) &= 0xFF00;
        let tmp = self.imm_u8() as u16;
        *self.regid2reg(reg_id) |= tmp;
    }

    #[inline]
    fn ld_dhl_d8(&mut self) {
        let tmp = self.imm_u8();
        self.cpu_write(self.hl, tmp);
    }

    #[inline]
    fn ld16_sp_hl(&mut self) {
        let val = self.hl;
        self.sp = val;
        self.advance_cycle();
    }

    #[inline]
    fn add_a_r(&mut self, opcode: u8) {
        let val = self.get_src_val(opcode) as u16;
        let a = self.af >> 8;
        self.af = (a + val) << 8;
        if (a + val) as u8 == 0 {
            self.af |= ZF_B;
        }
        if (a & 0xF) + (val & 0xF) > 0x0F {
            self.af |= HF_B;
        }
        if (a as u16) + (val as u16) > 0xFF {
            self.af |= CF_B;
        }
    }

    #[inline]
    fn sub_a_r(&mut self, opcode: u8) {
        let val = self.get_src_val(opcode) as u16;
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
    fn sub_a_d8(&mut self) {
        let val = self.imm_u8() as u16;
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
    fn sbc_a_r(&mut self, opcode: u8) {
        let val = self.get_src_val(opcode) as u16;
        let a = self.af >> 8;
        let carry = ((self.af & CF_B) != 0) as u16;
        let res = a.wrapping_sub(val).wrapping_sub(carry);
        self.af = (res << 8) | NF_B;

        if res as u8 == 0 {
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
    fn sbc_a_d8(&mut self) {
        let val = self.imm_u8() as u16;
        let a = self.af >> 8;
        let carry = ((self.af & CF_B) != 0) as u16;
        let res = a.wrapping_sub(val).wrapping_sub(carry);
        self.af = (res << 8) | NF_B;

        if res as u8 == 0 {
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
    fn add_a_d8(&mut self) {
        let val = self.imm_u8() as u16;
        let a = self.af >> 8;
        self.af = (a + val) << 8;
        if (a + val) as u8 == 0 {
            self.af |= ZF_B;
        }
        if (a & 0xF) + (val & 0xF) > 0x0F {
            self.af |= HF_B;
        }
        if a + val > 0xFF {
            self.af |= CF_B;
        }
    }

    #[inline]
    fn adc_a_r(&mut self, opcode: u8) {
        let val = self.get_src_val(opcode) as u16;
        let a = self.af >> 8;
        let carry = ((self.af & CF_B) != 0) as u16;
        self.af = (a + val + carry) << 8;
        if (a + val + carry) as u8 == 0 {
            self.af |= ZF_B;
        }
        if (a & 0xF) + (val & 0xF) + carry > 0x0F {
            self.af |= HF_B;
        }
        if a + val + carry > 0xFF {
            self.af |= CF_B;
        }
    }

    #[inline]
    fn adc_a_d8(&mut self) {
        let val = self.imm_u8() as u16;
        let a = self.af >> 8;
        let carry = ((self.af & CF_B) != 0) as u16;
        self.af = (a + val + carry) << 8;
        if (a + val + carry) as u8 == 0 {
            self.af |= ZF_B;
        }
        if (a & 0xF) + (val & 0xF) + carry > 0x0F {
            self.af |= HF_B;
        }
        if a + val + carry > 0xFF {
            self.af |= CF_B;
        }
    }

    #[inline]
    fn or_a_r(&mut self, opcode: u8) {
        let val = self.get_src_val(opcode) as u16;
        let a = self.af >> 8;
        self.af = (a | val) << 8;
        if (a | val) == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn or_a_d8(&mut self) {
        let val = self.imm_u8() as u16;
        let a = self.af >> 8;
        self.af = (a | val) << 8;
        if (a | val) == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn xor_a_r(&mut self, opcode: u8) {
        let val = self.get_src_val(opcode) as u16;
        let a = self.af >> 8;
        self.af = (a ^ val) << 8;
        if (a ^ val) == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn xor_a_d8(&mut self) {
        let val = self.imm_u8() as u16;
        let a = self.af >> 8;
        self.af = (a ^ val) << 8;
        if (a ^ val) == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn and_a_r(&mut self, opcode: u8) {
        let val = self.get_src_val(opcode) as u16;
        let a = self.af >> 8;
        self.af = ((a & val) << 8) | HF_B;
        if (a & val) == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn and_a_d8(&mut self) {
        let val = self.imm_u8() as u16;
        let a = self.af >> 8;
        self.af = ((a & val) << 8) | HF_B;
        if (a & val) == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn cp_a_r(&mut self, opcode: u8) {
        let val = self.get_src_val(opcode) as u16;
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
    fn cp_a_d8(&mut self) {
        let val = self.imm_u8() as u16;
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
    fn inc_lr(&mut self, opcode: u8) {
        let reg_id = (opcode >> 4) + 1;
        let val = (*self.regid2reg(reg_id) + 1) & 0xff;
        *self.regid2reg(reg_id) = (*self.regid2reg(reg_id) & 0xFF00) | val;

        self.af &= !(NF_B | ZF_B | HF_B);

        if ((*self.regid2reg(reg_id)) & 0x0F) == 0 {
            self.af |= HF_B;
        }

        if ((*self.regid2reg(reg_id)) & 0xFF) == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn dec_lr(&mut self, opcode: u8) {
        let reg_id = (opcode >> 4) + 1;
        let val = (*self.regid2reg(reg_id)).wrapping_sub(1) & 0xff;
        (*self.regid2reg(reg_id)) = ((*self.regid2reg(reg_id)) & 0xFF00) | val;

        self.af &= !(ZF_B | HF_B);
        self.af |= NF_B;

        if ((*self.regid2reg(reg_id)) & 0x0F) == 0xF {
            self.af |= HF_B;
        }

        if ((*self.regid2reg(reg_id)) & 0xFF) == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn inc_hr(&mut self, opcode: u8) {
        let reg_id = ((opcode >> 4) + 1) & 0x03;
        *self.regid2reg(reg_id) = (*self.regid2reg(reg_id)).wrapping_add(0x100);
        self.af &= !(NF_B | ZF_B | HF_B);

        if ((*self.regid2reg(reg_id)) & 0x0F00) == 0 {
            self.af |= HF_B;
        }

        if ((*self.regid2reg(reg_id)) & 0xFF00) == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn dec_hr(&mut self, opcode: u8) {
        let reg_id = ((opcode >> 4) + 1) & 0x03;
        *self.regid2reg(reg_id) = (*self.regid2reg(reg_id)).wrapping_sub(0x100);
        self.af &= !(ZF_B | HF_B);
        self.af |= NF_B;

        if ((*self.regid2reg(reg_id)) & 0x0F00) == 0xF00 {
            self.af |= HF_B;
        }

        if ((*self.regid2reg(reg_id)) & 0xFF00) == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn inc_dhl(&mut self) {
        let val = self.cpu_read(self.hl).wrapping_add(1);
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
        let val = self.cpu_read(self.hl).wrapping_sub(1);
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
    fn inc_rr(&mut self, opcode: u8) {
        let reg_id = (opcode >> 4) + 1;
        let reg = self.regid2reg(reg_id);
        *reg = reg.wrapping_add(1);
        self.advance_cycle();
    }

    #[inline]
    fn dec_rr(&mut self, opcode: u8) {
        let reg_id = (opcode >> 4) + 1;
        let reg = self.regid2reg(reg_id);
        *reg = reg.wrapping_sub(1);
        self.advance_cycle();
    }

    #[inline]
    fn ld_hl_sp_r8(&mut self) {
        self.af &= 0xFF00;
        let offset = self.imm_u8() as i8 as u16;
        self.advance_cycle();
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
        let carry = (self.af & 0x100) != 0;
        self.af = (self.af >> 1) & 0xFF00;
        if carry {
            self.af |= CF_B | 0x8000;
        }
    }

    #[inline]
    fn rrc_r(&mut self, opcode: u8) {
        let val = self.get_src_val(opcode);
        let carry = (val & 0x01) != 0;
        self.af &= 0xFF00;
        let val = val >> 1 | (carry as u8) << 7;
        self.set_src_val(opcode, val);
        if carry {
            self.af |= CF_B;
        }
        if val == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn do_jump_to_immediate(&mut self) {
        let addr = self.imm_u16();
        self.pc = addr;
        self.advance_cycle();
    }

    #[inline]
    fn jp_a16(&mut self) {
        self.do_jump_to_immediate();
    }

    #[inline]
    fn jp_cc(&mut self, opcode: u8) {
        if self.condition(opcode) {
            self.do_jump_to_immediate();
        } else {
            let pc = self.pc.wrapping_add(2);
            self.pc = pc;
            self.advance_cycle();
            self.advance_cycle();
        }
    }

    #[inline]
    fn jp_hl(&mut self) {
        let addr = self.hl;
        self.pc = addr;
    }

    #[inline]
    fn do_jump_relative(&mut self) {
        let relative_addr = self.imm_u8() as i8 as u16;
        let pc = self.pc.wrapping_add(relative_addr);
        self.pc = pc;
        self.advance_cycle();
    }

    #[inline]
    fn jr_d(&mut self) {
        self.do_jump_relative();
    }

    #[inline]
    fn jr_cc(&mut self, opcode: u8) {
        if self.condition(opcode) {
            self.do_jump_relative();
        } else {
            self.pc = self.pc.wrapping_add(1);
            self.advance_cycle();
        }
    }

    #[inline]
    fn do_call(&mut self) {
        let addr = self.imm_u16();
        self.sp = self.sp.wrapping_sub(1);
        self.cpu_write(self.sp, ((self.pc) >> 8) as u8);
        self.sp = self.sp.wrapping_sub(1);
        self.cpu_write(self.sp, (self.pc & 0xFF) as u8);
        self.pc = addr;
    }

    #[inline]
    fn call_nn(&mut self) {
        self.do_call();
    }

    #[inline]
    fn call_cc_a16(&mut self, opcode: u8) {
        if self.condition(opcode) {
            self.do_call();
        } else {
            let pc = self.pc.wrapping_add(2);
            self.pc = pc;
            self.advance_cycle();
            self.advance_cycle();
        }
    }

    #[inline]
    fn ret(&mut self) {
        self.pc = self.cpu_read(self.sp) as u16;
        self.sp = self.sp.wrapping_add(1);
        self.pc |= (self.cpu_read(self.sp) as u16) << 8;
        self.sp = self.sp.wrapping_add(1);
        self.advance_cycle();
    }

    #[inline]
    fn reti(&mut self) {
        self.ret();
        self.ime = true;
    }

    #[inline]
    fn ret_cc(&mut self, opcode: u8) {
        self.advance_cycle();

        if self.condition(opcode) {
            self.ret();
        }
    }

    #[inline]
    fn condition(&self, opcode: u8) -> bool {
        match (opcode >> 3) & 0x3 {
            0 => self.af & ZF_B == 0,
            1 => self.af & ZF_B != 0,
            2 => self.af & CF_B == 0,
            3 => self.af & CF_B != 0,
            _ => unreachable!(),
        }
    }

    #[inline]
    fn rst(&mut self, opcode: u8) {
        self.sp = self.sp.wrapping_sub(1);
        self.cpu_write(self.sp, ((self.pc) >> 8) as u8);
        self.sp = self.sp.wrapping_sub(1);
        self.cpu_write(self.sp, ((self.pc) & 0xFF) as u8);
        self.pc = opcode as u16 ^ 0xC7;
    }

    #[inline]
    fn halt(&mut self) {
        self.cpu_halted = true;
    }

    #[inline]
    fn stop(&mut self) {
        self.imm_u8();

        if self.key1 & KEY1_SWITCH_B == 0 {
            self.cpu_halted = true;
        } else {
            self.double_speed = !self.double_speed;

            self.key1 &= !KEY1_SWITCH_B;
            self.key1 ^= KEY1_SPEED_B;
        }
    }

    #[inline]
    fn di(&mut self) {
        self.ime = false;
    }

    #[inline]
    fn ei(&mut self) {
        self.cpu_ei_delay = true;
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

    #[inline]
    #[allow(clippy::unused_self)]
    fn nop(&mut self) {}

    // TODO: debugger breakpoint
    #[inline]
    fn ld_b_b(&mut self) {
        self.nop();
    }

    #[inline]
    fn daa(&mut self) {
        let mut result = self.af >> 8;

        self.af &= !(0xFF00 | ZF_B);

        if self.af & NF_B == 0 {
            if self.af & HF_B != 0 || result & 0x0F > 0x09 {
                result += 0x06;
            }

            if self.af & CF_B != 0 || result > 0x9F {
                result += 0x60;
            }
        } else {
            if self.af & HF_B != 0 {
                result = result.wrapping_sub(0x06) & 0xFF;
            }

            if self.af & CF_B != 0 {
                result = result.wrapping_sub(0x60);
            }
        }

        if result & 0xFF == 0 {
            self.af |= ZF_B;
        }

        if result & 0x100 == 0x100 {
            self.af |= CF_B;
        }

        self.af &= !HF_B;
        self.af |= result << 8;
    }

    #[inline]
    fn cpl(&mut self) {
        self.af ^= 0xFF00;
        self.af |= HF_B | NF_B;
    }

    #[inline]
    fn push_rr(&mut self, opcode: u8) {
        let reg_id = ((opcode >> 4) + 1) & 3;
        let val = *self.regid2reg(reg_id);
        self.sp = self.sp.wrapping_sub(1);
        self.cpu_write(self.sp, (val >> 8) as u8);
        self.sp = self.sp.wrapping_sub(1);
        self.cpu_write(self.sp, (val & 0xFF) as u8);
    }

    #[inline]
    fn pop_rr(&mut self, opcode: u8) {
        let mut val = self.cpu_read(self.sp) as u16;
        self.sp = self.sp.wrapping_add(1);
        val |= (self.cpu_read(self.sp) as u16) << 8;
        self.sp = self.sp.wrapping_add(1);
        let reg_id = ((opcode >> 4) + 1) & 3;
        *self.regid2reg(reg_id) = val;
        self.af &= 0xfff0;
    }

    #[inline]
    fn ld_rr_d16(&mut self, opcode: u8) {
        let reg_id = (opcode >> 4) + 1;
        *self.regid2reg(reg_id) = self.imm_u16();
    }

    #[inline]
    fn ld_da16_sp(&mut self) {
        let val = self.sp;
        let addr = self.imm_u16();
        self.cpu_write(addr, (val & 0xff) as u8);
        self.cpu_write(addr.wrapping_add(1), (val >> 8) as u8);
    }

    #[inline]
    fn add_sp_r8(&mut self) {
        let sp = self.sp;
        let offset = self.imm_u8() as i8 as u16;
        self.advance_cycle();
        self.advance_cycle();
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
    fn add_hl_rr(&mut self, opcode: u8) {
        let reg_id = (opcode >> 4) + 1;
        let hl = self.hl;
        let rr = *self.regid2reg(reg_id);
        self.hl = hl.wrapping_add(rr);

        self.af &= !(NF_B | CF_B | HF_B);

        if ((hl & 0xFFF) + (rr & 0xFFF)) & 0x1000 != 0 {
            self.af |= HF_B;
        }

        if (hl as u32 + rr as u32) & 0x10000 != 0 {
            self.af |= CF_B;
        }

        self.advance_cycle();
    }

    #[inline]
    fn rlc_r(&mut self, opcode: u8) {
        let val = self.get_src_val(opcode);
        let carry = val & 0x80 != 0;
        self.af &= 0xFF00;
        self.set_src_val(opcode, val << 1 | carry as u8);
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

        self.af = (self.af >> 1) & 0xFF00 | (carry as u16) << 15;

        if bit1 {
            self.af |= CF_B;
        }
    }

    #[inline]
    fn rr_r(&mut self, opcode: u8) {
        let val = self.get_src_val(opcode);
        let carry = (self.af & CF_B) != 0;
        let bit1 = (val & 0x1) != 0;

        self.af &= 0xFF00;
        let val = val >> 1 | (carry as u8) << 7;
        self.set_src_val(opcode, val);
        if bit1 {
            self.af |= CF_B;
        }
        if val == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn sla_r(&mut self, opcode: u8) {
        let val = self.get_src_val(opcode);
        let carry = (val & 0x80) != 0;
        self.af &= 0xFF00;
        self.set_src_val(opcode, val << 1);
        if carry {
            self.af |= CF_B;
        }
        if val & 0x7F == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn sra_r(&mut self, opcode: u8) {
        let val = self.get_src_val(opcode);
        let bit7 = val & 0x80;
        self.af &= 0xFF00;
        if val & 1 != 0 {
            self.af |= CF_B;
        }
        let val = (val >> 1) | bit7;
        self.set_src_val(opcode, val);
        if val == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn srl_r(&mut self, opcode: u8) {
        let val = self.get_src_val(opcode);
        self.af &= 0xFF00;
        self.set_src_val(opcode, val >> 1);
        if val & 1 != 0 {
            self.af |= CF_B;
        }
        if val >> 1 == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn swap_r(&mut self, opcode: u8) {
        let val = self.get_src_val(opcode);
        self.af &= 0xFF00;
        self.set_src_val(opcode, (val >> 4) | (val << 4));
        if val == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn bit_r(&mut self, opcode: u8) {
        let val = self.get_src_val(opcode);
        let bit = 1 << ((opcode >> 3) & 7);
        if opcode & 0xC0 == 0x40 {
            // bit
            self.af &= 0xFF00 | CF_B;
            self.af |= HF_B;
            if bit & val == 0 {
                self.af |= ZF_B;
            }
        } else if opcode & 0xC0 == 0x80 {
            // res
            self.set_src_val(opcode, val & !bit);
        } else {
            // set
            self.set_src_val(opcode, val | bit);
        }
    }

    #[inline]
    fn rla(&mut self) {
        let bit7 = (self.af & 0x8000) != 0;
        let carry = (self.af & CF_B) != 0;

        self.af = (self.af & 0xFF00) << 1 | (carry as u16) << 8;

        if bit7 {
            self.af |= CF_B;
        }
    }

    #[inline]
    fn rl_r(&mut self, opcode: u8) {
        let val = self.get_src_val(opcode);
        let carry = (self.af & CF_B) != 0;
        let bit7 = (val & 0x80) != 0;

        self.af &= 0xFF00;
        let val = (val << 1) | carry as u8;
        self.set_src_val(opcode, val);
        if bit7 {
            self.af |= CF_B;
        }
        if val == 0 {
            self.af |= ZF_B;
        }
    }

    #[inline]
    fn ill(&mut self) {
        self.ie = 0;
        self.cpu_halted = true;
    }

    #[inline]
    fn exec(&mut self, opcode: u8) {
        match opcode {
            0x00 | 0x7f | 0x49 | 0x52 | 0x5b | 0x64 | 0x6d => self.nop(),
            0x40 => self.ld_b_b(),
            0x78 => self.ld(A, B),
            0x79 => self.ld(A, C),
            0x7a => self.ld(A, D),
            0x7b => self.ld(A, E),
            0x7c => self.ld(A, H),
            0x7d => self.ld(A, L),
            0x47 => self.ld(B, A),
            0x41 => self.ld(B, C),
            0x42 => self.ld(B, D),
            0x43 => self.ld(B, E),
            0x44 => self.ld(B, H),
            0x45 => self.ld(B, L),
            0x4f => self.ld(C, A),
            0x48 => self.ld(C, B),
            0x4a => self.ld(C, D),
            0x4b => self.ld(C, E),
            0x4c => self.ld(C, H),
            0x4d => self.ld(C, L),
            0x57 => self.ld(D, A),
            0x50 => self.ld(D, B),
            0x51 => self.ld(D, C),
            0x53 => self.ld(D, E),
            0x54 => self.ld(D, H),
            0x55 => self.ld(D, L),
            0x5f => self.ld(E, A),
            0x58 => self.ld(E, B),
            0x59 => self.ld(E, C),
            0x5a => self.ld(E, D),
            0x5c => self.ld(E, H),
            0x5d => self.ld(E, L),
            0x67 => self.ld(H, A),
            0x60 => self.ld(H, B),
            0x61 => self.ld(H, C),
            0x62 => self.ld(H, D),
            0x63 => self.ld(H, E),
            0x65 => self.ld(H, L),
            0x6f => self.ld(L, A),
            0x68 => self.ld(L, B),
            0x69 => self.ld(L, C),
            0x6a => self.ld(L, D),
            0x6b => self.ld(L, E),
            0x6c => self.ld(L, H),
            0x7e => self.ld(A, Dhl),
            0x46 => self.ld(B, Dhl),
            0x4e => self.ld(C, Dhl),
            0x56 => self.ld(D, Dhl),
            0x5e => self.ld(E, Dhl),
            0x66 => self.ld(H, Dhl),
            0x6e => self.ld(L, Dhl),
            0x77 => self.ld(Dhl, A),
            0x70 => self.ld(Dhl, B),
            0x71 => self.ld(Dhl, C),
            0x72 => self.ld(Dhl, D),
            0x73 => self.ld(Dhl, E),
            0x74 => self.ld(Dhl, H),
            0x75 => self.ld(Dhl, L),
            0x3e | 0x06 | 0x16 | 0x26 => self.ld_hr_d8(opcode),
            0x0e | 0x1e | 0x2e => self.ld_lr_d8(opcode),
            0x36 => self.ld_dhl_d8(),
            0x0a | 0x1a => self.ld_a_drr(opcode),
            0x02 | 0x12 => self.ld_drr_a(opcode),
            0xfa => self.ld_a_da16(),
            0xea => self.ld_da16_a(),
            0x3a => self.ld_a_dhld(),
            0x32 => self.ld_dhld_a(),
            0x2a => self.ld_a_dhli(),
            0x22 => self.ld_dhli_a(),
            0xf2 => self.ld_a_dc(),
            0xe2 => self.ld_dc_a(),
            0xf0 => self.ld_a_da8(),
            0xe0 => self.ld_da8_a(),
            0x87 | 0x80 | 0x81 | 0x82 | 0x83 | 0x84 | 0x85 | 0x86 => self.add_a_r(opcode),
            0xc6 => self.add_a_d8(),
            0x8f | 0x88 | 0x89 | 0x8a | 0x8b | 0x8c | 0x8d | 0x8e => self.adc_a_r(opcode),
            0xce => self.adc_a_d8(),
            0x97 | 0x90 | 0x91 | 0x92 | 0x93 | 0x94 | 0x95 | 0x96 => self.sub_a_r(opcode),
            0xd6 => self.sub_a_d8(),
            0x9f | 0x98 | 0x99 | 0x9a | 0x9b | 0x9c | 0x9d | 0x9e => self.sbc_a_r(opcode),
            0xde => self.sbc_a_d8(),
            0xbf | 0xb8 | 0xb9 | 0xba | 0xbb | 0xbc | 0xbd | 0xbe => self.cp_a_r(opcode),
            0xfe => self.cp_a_d8(),
            0xa7 | 0xa0 | 0xa1 | 0xa2 | 0xa3 | 0xa4 | 0xa5 | 0xa6 => self.and_a_r(opcode),
            0xe6 => self.and_a_d8(),
            0xb7 | 0xb0 | 0xb1 | 0xb2 | 0xb3 | 0xb4 | 0xb5 | 0xb6 => self.or_a_r(opcode),
            0xf6 => self.or_a_d8(),
            0xaf | 0xa8 | 0xa9 | 0xaa | 0xab | 0xac | 0xad | 0xae => self.xor_a_r(opcode),
            0xee => self.xor_a_d8(),
            0x3c | 0x04 | 0x14 | 0x24 => self.inc_hr(opcode),
            0x3d | 0x05 | 0x15 | 0x25 => self.dec_hr(opcode),
            0x0c | 0x1c | 0x2c => self.inc_lr(opcode),
            0x0d | 0x1d | 0x2d => self.dec_lr(opcode),
            0x34 => self.inc_dhl(),
            0x35 => self.dec_dhl(),
            0x07 => self.rlca(),
            0x17 => self.rla(),
            0x0f => self.rrca(),
            0x1f => self.rra(),
            0xc3 => self.jp_a16(),
            0xe9 => self.jp_hl(),
            0x18 => self.jr_d(),
            0xcd => self.call_nn(),
            0xc9 => self.ret(),
            0xd9 => self.reti(),
            0xc2 | 0xca | 0xd2 | 0xda => self.jp_cc(opcode),
            0x20 | 0x28 | 0x30 | 0x38 => self.jr_cc(opcode),
            0xc4 | 0xcc | 0xd4 | 0xdc => self.call_cc_a16(opcode),
            0xc0 | 0xc8 | 0xd0 | 0xd8 => self.ret_cc(opcode),
            0xc7 | 0xcf | 0xd7 | 0xdf | 0xe7 | 0xef | 0xf7 | 0xff => self.rst(opcode),
            0x76 => self.halt(),
            0x10 => self.stop(),
            0xf3 => self.di(),
            0xfb => self.ei(),
            0x3f => self.ccf(),
            0x37 => self.scf(),
            0x27 => self.daa(),
            0x2f => self.cpl(),
            0x01 | 0x11 | 0x21 | 0x31 => self.ld_rr_d16(opcode),
            0x08 => self.ld_da16_sp(),
            0xf9 => self.ld16_sp_hl(),
            0xf8 => self.ld_hl_sp_r8(),
            0xc5 | 0xd5 | 0xe5 | 0xf5 => self.push_rr(opcode),
            0xc1 | 0xd1 | 0xe1 | 0xf1 => self.pop_rr(opcode),
            0x09 | 0x19 | 0x29 | 0x39 => self.add_hl_rr(opcode),
            0xe8 => self.add_sp_r8(),
            0x03 | 0x13 | 0x23 | 0x33 => self.inc_rr(opcode),
            0x0b | 0x1b | 0x2b | 0x3b => self.dec_rr(opcode),
            0xcb => self.exec_cb(),
            _ => self.ill(),
        }
    }

    #[inline]
    fn exec_cb(&mut self) {
        let opcode = self.imm_u8();
        match opcode >> 3 {
            0 => self.rlc_r(opcode),
            1 => self.rrc_r(opcode),
            2 => self.rl_r(opcode),
            3 => self.rr_r(opcode),
            4 => self.sla_r(opcode),
            5 => self.sra_r(opcode),
            6 => self.swap_r(opcode),
            7 => self.srl_r(opcode),
            _ => self.bit_r(opcode),
        }
    }
}
