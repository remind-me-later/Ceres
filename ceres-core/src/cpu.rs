use {
    crate::{
        Gb, IF_LCD_B, IF_P1_B, IF_SERIAL_B, IF_TIMER_B, IF_VBLANK_B, KEY1_SPEED_B, KEY1_SWITCH_B,
    },
    Reg8::{A, B, C, D, E, H, L},
};

const ZF_B: u16 = 0x80;
const NF_B: u16 = 0x40;
const HF_B: u16 = 0x20;
const CF_B: u16 = 0x10;

trait Ld
where
    Self: Copy,
{
    fn get(self, gb: &mut Gb) -> u8;
    fn set(self, gb: &mut Gb, val: u8);
}

#[derive(Clone, Copy)]
enum Reg8 {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
}

impl Ld for Reg8 {
    fn get(self, gb: &mut Gb) -> u8 {
        match self {
            Reg8::A => (gb.af >> 8) as u8,
            Reg8::B => (gb.bc >> 8) as u8,
            Reg8::C => gb.bc as u8,
            Reg8::D => (gb.de >> 8) as u8,
            Reg8::E => gb.de as u8,
            Reg8::H => (gb.hl >> 8) as u8,
            Reg8::L => gb.hl as u8,
        }
    }

    fn set(self, gb: &mut Gb, val: u8) {
        match self {
            Reg8::A => {
                gb.af &= 0x00ff;
                gb.af |= (val as u16) << 8;
            }
            Reg8::B => {
                gb.bc &= 0x00ff;
                gb.bc |= (val as u16) << 8;
            }
            Reg8::C => {
                gb.bc &= 0xff00;
                gb.bc |= val as u16;
            }
            Reg8::D => {
                gb.de &= 0x00ff;
                gb.de |= (val as u16) << 8;
            }
            Reg8::E => {
                gb.de &= 0xff00;
                gb.de |= val as u16;
            }
            Reg8::H => {
                gb.hl &= 0x00ff;
                gb.hl |= (val as u16) << 8;
            }
            Reg8::L => {
                gb.hl &= 0xff00;
                gb.hl |= val as u16;
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Dhl;

impl Ld for Dhl {
    fn get(self, gb: &mut Gb) -> u8 {
        gb.read_mem(gb.hl)
    }

    fn set(self, gb: &mut Gb, val: u8) {
        gb.write_mem(gb.hl, val);
    }
}

impl Gb {
    pub(crate) fn run(&mut self) {
        if self.cpu_ei_delay {
            self.ime = true;
            self.cpu_ei_delay = false;
        }

        if self.cpu_halted {
            self.tick();
        } else {
            let opcode = self.imm8();

            if self.cpu_halt_bug {
                self.pc = self.pc.wrapping_sub(1);
                self.cpu_halt_bug = false;
            }

            self.exec(opcode);
        }

        if !self.any_interrrupt() {
            return;
        }

        // handle interrupt
        if self.cpu_halted {
            self.cpu_halted = false;
        }

        if self.ime {
            self.ime = false;

            self.tick();

            let pc = self.pc;
            self.internal_push(pc);

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

            self.tick();
            // acknowledge
            self.ifr &= !interrupt;
        }
    }

    #[must_use]
    fn any_interrrupt(&self) -> bool {
        self.ifr & self.ie & 0x1f != 0
    }

    fn imm8(&mut self) -> u8 {
        let addr = self.pc;
        self.pc = self.pc.wrapping_add(1);
        self.read_mem(addr)
    }

    fn imm16(&mut self) -> u16 {
        let lo = self.imm8();
        let hi = self.imm8();
        u16::from_le_bytes([lo, hi])
    }

    fn internal_pop(&mut self) -> u16 {
        let lo = self.read_mem(self.sp);
        self.sp = self.sp.wrapping_add(1);
        let hi = self.read_mem(self.sp);
        self.sp = self.sp.wrapping_add(1);
        u16::from_le_bytes([lo, hi])
    }

    fn internal_push(&mut self, val: u16) {
        let [lo, hi] = u16::to_le_bytes(val);
        self.tick();
        self.sp = self.sp.wrapping_sub(1);
        self.write_mem(self.sp, hi);
        self.sp = self.sp.wrapping_sub(1);
        self.write_mem(self.sp, lo);
    }

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

    fn get_src_value(&mut self, opcode: u8) -> u8 {
        let reg_id = ((opcode >> 1) + 1) & 3;
        let lo = opcode & 1 != 0;
        if reg_id == 0 {
            if lo {
                return (self.af >> 8) as u8;
            }
            return self.read_mem(self.hl);
        }
        if lo {
            return *self.regid2reg(reg_id) as u8;
        }
        return (*self.regid2reg(reg_id) >> 8) as u8;
    }

    fn set_src_value(&mut self, opcode: u8, value: u8) {
        let reg_id = ((opcode >> 1) + 1) & 3;
        let lo = opcode & 1 != 0;

        if reg_id == 0 {
            if lo {
                self.af &= 0xFF;
                self.af |= (value as u16) << 8;
            } else {
                self.write_mem(self.hl, value);
            }
        } else if lo {
            *self.regid2reg(reg_id) &= 0xFF00;
            *self.regid2reg(reg_id) |= value as u16;
        } else {
            *self.regid2reg(reg_id) &= 0xFF;
            *self.regid2reg(reg_id) |= (value as u16) << 8;
        }
    }

    fn ld<L, R>(&mut self, lhs: R, rhs: L)
    where
        L: Ld,
        R: Ld,
    {
        let val = rhs.get(self);
        lhs.set(self, val);
    }

    fn ld_a_drr(&mut self, opcode: u8) {
        let register_id = (opcode >> 4) + 1;
        self.af &= 0xFF;
        let tmp = *self.regid2reg(register_id);
        self.af |= (self.read_mem(tmp) as u16) << 8;
    }

    fn ld_drr_a(&mut self, opcode: u8) {
        let register_id = (opcode >> 4) + 1;
        let tmp = *self.regid2reg(register_id);
        self.write_mem(tmp, (self.af >> 8) as u8);
    }

    fn ld_da16_a(&mut self) {
        let addr = self.imm16();
        self.write_mem(addr, (self.af >> 8) as u8);
    }

    fn ld_a_da16(&mut self) {
        self.af &= 0xFF;
        let addr = self.imm16();
        self.af |= (self.read_mem(addr) as u16) << 8;
    }

    fn ld_dhli_a(&mut self) {
        let addr = self.hl;
        self.write_mem(addr, (self.af >> 8) as u8);
        self.hl = addr.wrapping_add(1);
    }

    fn ld_dhld_a(&mut self) {
        let addr = self.hl;
        self.write_mem(addr, (self.af >> 8) as u8);
        self.hl = addr.wrapping_sub(1);
    }

    fn ld_a_dhli(&mut self) {
        let addr = self.hl;
        let val = self.read_mem(addr) as u16;
        self.af &= 0xFF;
        self.af |= val << 8;
        self.hl = addr.wrapping_add(1);
    }

    fn ld_a_dhld(&mut self) {
        let addr = self.hl;
        let val = self.read_mem(addr) as u16;
        self.af &= 0xFF;
        self.af |= val << 8;
        self.hl = addr.wrapping_sub(1);
    }

    fn ld_da8_a(&mut self) {
        let temp = self.imm8();
        self.write_mem(0xFF00 | temp as u16, (self.af >> 8) as u8);
    }

    fn ld_a_da8(&mut self) {
        self.af &= 0xFF;
        let temp = self.imm8();
        self.af |= (self.read_mem(0xFF00 | temp as u16) as u16) << 8;
    }

    fn ld_dc_a(&mut self) {
        self.write_mem(0xFF00 | (self.bc & 0xFF), (self.af >> 8) as u8);
    }

    fn ld_a_dc(&mut self) {
        self.af &= 0xFF;
        self.af |= (self.read_mem(0xFF00 | (self.bc & 0xFF)) as u16) << 8;
    }

    fn ld_hr_d8(&mut self, opcode: u8) {
        let register_id = ((opcode >> 4) + 1) & 0x03;
        *self.regid2reg(register_id) &= 0xFF;
        let tmp = self.imm8() as u16;
        *self.regid2reg(register_id) |= tmp << 8;
    }

    fn ld_lr_d8(&mut self, opcode: u8) {
        let register_id = (opcode >> 4) + 1;
        *self.regid2reg(register_id) &= 0xFF00;
        let tmp = self.imm8() as u16;
        *self.regid2reg(register_id) |= tmp;
    }

    fn ld_dhl_d8(&mut self) {
        let data = self.imm8();
        self.write_mem(self.hl, data);
    }

    fn ld16_sp_hl(&mut self) {
        let val = self.hl;
        self.sp = val;
        self.tick();
    }

    fn add_a_r(&mut self, opcode: u8) {
        let value = self.get_src_value(opcode) as u16;
        let a = self.af >> 8;
        self.af = (a + value) << 8;
        if (a + value) as u8 == 0 {
            self.af |= ZF_B;
        }
        if (a & 0xF) + (value & 0xF) > 0x0F {
            self.af |= HF_B;
        }
        if (a as u16) + (value as u16) > 0xFF {
            self.af |= CF_B;
        }
    }

    fn sub_a_r(&mut self, opcode: u8) {
        let value = self.get_src_value(opcode) as u16;
        let a = self.af >> 8;
        self.af = (a.wrapping_sub(value) << 8) | NF_B;
        if a == value {
            self.af |= ZF_B;
        }
        if (a & 0xF) < (value & 0xF) {
            self.af |= HF_B;
        }
        if a < value {
            self.af |= CF_B;
        }
    }

    fn sub_a_d8(&mut self) {
        let value = self.imm8() as u16;
        let a = self.af >> 8;
        self.af = (a.wrapping_sub(value) << 8) | NF_B;
        if a == value {
            self.af |= ZF_B;
        }
        if (a & 0xF) < (value & 0xF) {
            self.af |= HF_B;
        }
        if a < value {
            self.af |= CF_B;
        }
    }

    fn sbc_a_r(&mut self, opcode: u8) {
        let value = self.get_src_value(opcode) as u16;
        let a = self.af >> 8;
        let carry = ((self.af & CF_B) != 0) as u16;
        let res = a.wrapping_sub(value).wrapping_sub(carry);
        self.af = (res << 8) | NF_B;

        if res as u8 == 0 {
            self.af |= ZF_B;
        }
        if (a & 0xF) < (value & 0xF) + carry {
            self.af |= HF_B;
        }
        if res > 0xFF {
            self.af |= CF_B;
        }
    }

    fn sbc_a_d8(&mut self) {
        let value = self.imm8() as u16;
        let a = self.af >> 8;
        let carry = ((self.af & CF_B) != 0) as u16;
        let res = a.wrapping_sub(value).wrapping_sub(carry);
        self.af = (res << 8) | NF_B;

        if res as u8 == 0 {
            self.af |= ZF_B;
        }
        if (a & 0xF) < (value & 0xF) + carry {
            self.af |= HF_B;
        }
        if res > 0xFF {
            self.af |= CF_B;
        }
    }

    fn add_a_d8(&mut self) {
        let value = self.imm8() as u16;
        let a = self.af >> 8;
        self.af = (a + value) << 8;
        if (a + value) as u8 == 0 {
            self.af |= ZF_B;
        }
        if (a & 0xF) + (value & 0xF) > 0x0F {
            self.af |= HF_B;
        }
        if a + value > 0xFF {
            self.af |= CF_B;
        }
    }

    fn adc_a_r(&mut self, opcode: u8) {
        let value = self.get_src_value(opcode) as u16;
        let a = self.af >> 8;
        let carry = ((self.af & CF_B) != 0) as u16;
        self.af = (a + value + carry) << 8;
        if (a + value + carry) as u8 == 0 {
            self.af |= ZF_B;
        }
        if (a & 0xF) + (value & 0xF) + carry > 0x0F {
            self.af |= HF_B;
        }
        if a + value + carry > 0xFF {
            self.af |= CF_B;
        }
    }

    fn adc_a_d8(&mut self) {
        let value = self.imm8() as u16;
        let a = self.af >> 8;
        let carry = ((self.af & CF_B) != 0) as u16;
        self.af = (a + value + carry) << 8;
        if (a + value + carry) as u8 == 0 {
            self.af |= ZF_B;
        }
        if (a & 0xF) + (value & 0xF) + carry > 0x0F {
            self.af |= HF_B;
        }
        if a + value + carry > 0xFF {
            self.af |= CF_B;
        }
    }

    fn or_a_r(&mut self, opcode: u8) {
        let value = self.get_src_value(opcode) as u16;
        let a = self.af >> 8;
        self.af = (a | value) << 8;
        if (a | value) == 0 {
            self.af |= ZF_B;
        }
    }

    fn or_a_d8(&mut self) {
        let value = self.imm8() as u16;
        let a = self.af >> 8;
        self.af = (a | value) << 8;
        if (a | value) == 0 {
            self.af |= ZF_B;
        }
    }

    fn xor_a_r(&mut self, opcode: u8) {
        let value = self.get_src_value(opcode) as u16;
        let a = self.af >> 8;
        self.af = (a ^ value) << 8;
        if (a ^ value) == 0 {
            self.af |= ZF_B;
        }
    }

    fn xor_a_d8(&mut self) {
        let value = self.imm8() as u16;
        let a = self.af >> 8;
        self.af = (a ^ value) << 8;
        if (a ^ value) == 0 {
            self.af |= ZF_B;
        }
    }

    fn and_a_r(&mut self, opcode: u8) {
        let value = self.get_src_value(opcode) as u16;
        let a = self.af >> 8;
        self.af = ((a & value) << 8) | HF_B;
        if (a & value) == 0 {
            self.af |= ZF_B;
        }
    }

    fn and_a_d8(&mut self) {
        let value = self.imm8() as u16;
        let a = self.af >> 8;
        self.af = ((a & value) << 8) | HF_B;
        if (a & value) == 0 {
            self.af |= ZF_B;
        }
    }

    fn cp_a_r(&mut self, opcode: u8) {
        let value = self.get_src_value(opcode) as u16;
        let a = self.af >> 8;
        self.af &= 0xFF00;
        self.af |= NF_B;
        if a == value {
            self.af |= ZF_B;
        }
        if (a & 0xF) < (value & 0xF) {
            self.af |= HF_B;
        }
        if a < value {
            self.af |= CF_B;
        }
    }

    fn cp_a_d8(&mut self) {
        let value = self.imm8() as u16;
        let a = self.af >> 8;
        self.af &= 0xFF00;
        self.af |= NF_B;
        if a == value {
            self.af |= ZF_B;
        }
        if (a & 0xF) < (value & 0xF) {
            self.af |= HF_B;
        }
        if a < value {
            self.af |= CF_B;
        }
    }

    fn inc_lr(&mut self, opcode: u8) {
        let register_id = (opcode >> 4) + 1;
        let value = (*self.regid2reg(register_id) + 1) & 0xff;
        *self.regid2reg(register_id) = (*self.regid2reg(register_id) & 0xFF00) | value;

        self.af &= !(NF_B | ZF_B | HF_B);

        if ((*self.regid2reg(register_id)) & 0x0F) == 0 {
            self.af |= HF_B;
        }

        if ((*self.regid2reg(register_id)) & 0xFF) == 0 {
            self.af |= ZF_B;
        }
    }

    fn dec_lr(&mut self, opcode: u8) {
        let register_id = (opcode >> 4) + 1;
        let value = (*self.regid2reg(register_id)).wrapping_sub(1) & 0xff;
        (*self.regid2reg(register_id)) = ((*self.regid2reg(register_id)) & 0xFF00) | value;

        self.af &= !(ZF_B | HF_B);
        self.af |= NF_B;

        if ((*self.regid2reg(register_id)) & 0x0F) == 0xF {
            self.af |= HF_B;
        }

        if ((*self.regid2reg(register_id)) & 0xFF) == 0 {
            self.af |= ZF_B;
        }
    }

    fn inc_hr(&mut self, opcode: u8) {
        let register_id = ((opcode >> 4) + 1) & 0x03;
        *self.regid2reg(register_id) = (*self.regid2reg(register_id)).wrapping_add(0x100);
        self.af &= !(NF_B | ZF_B | HF_B);

        if ((*self.regid2reg(register_id)) & 0x0F00) == 0 {
            self.af |= HF_B;
        }

        if ((*self.regid2reg(register_id)) & 0xFF00) == 0 {
            self.af |= ZF_B;
        }
    }

    fn dec_hr(&mut self, opcode: u8) {
        let register_id = ((opcode >> 4) + 1) & 0x03;
        *self.regid2reg(register_id) = (*self.regid2reg(register_id)).wrapping_sub(0x100);
        self.af &= !(ZF_B | HF_B);
        self.af |= NF_B;

        if ((*self.regid2reg(register_id)) & 0x0F00) == 0xF00 {
            self.af |= HF_B;
        }

        if ((*self.regid2reg(register_id)) & 0xFF00) == 0 {
            self.af |= ZF_B;
        }
    }

    fn inc_dhl(&mut self) {
        let value = self.read_mem(self.hl).wrapping_add(1);
        self.write_mem(self.hl, value);

        self.af &= !(NF_B | ZF_B | HF_B);
        if (value & 0x0F) == 0 {
            self.af |= HF_B;
        }

        if value == 0 {
            self.af |= ZF_B;
        }
    }

    fn dec_dhl(&mut self) {
        let value = self.read_mem(self.hl).wrapping_sub(1);
        self.write_mem(self.hl, value);

        self.af &= !(ZF_B | HF_B);
        self.af |= NF_B;
        if (value & 0x0F) == 0x0F {
            self.af |= HF_B;
        }

        if value == 0 {
            self.af |= ZF_B;
        }
    }

    fn inc_rr(&mut self, opcode: u8) {
        let register_id = (opcode >> 4) + 1;
        let reg = self.regid2reg(register_id);
        *reg = reg.wrapping_add(1);
        self.tick();
    }

    fn dec_rr(&mut self, opcode: u8) {
        let register_id = (opcode >> 4) + 1;
        let reg = self.regid2reg(register_id);
        *reg = reg.wrapping_sub(1);
        self.tick();
    }

    fn ld_hl_sp_r8(&mut self) {
        self.af &= 0xFF00;
        let offset = self.imm8() as i8 as u16;
        self.tick();
        self.hl = self.sp.wrapping_add(offset);

        if (self.sp & 0xF) + (offset & 0xF) > 0xF {
            self.af |= HF_B;
        }

        if (self.sp & 0xFF) + (offset & 0xFF) > 0xFF {
            self.af |= CF_B;
        }
    }

    fn rlca(&mut self) {
        let carry = (self.af & 0x8000) != 0;

        self.af = (self.af & 0xFF00) << 1;
        if carry {
            self.af |= CF_B | 0x0100;
        }
    }

    fn rrca(&mut self) {
        let carry = (self.af & 0x100) != 0;
        self.af = (self.af >> 1) & 0xFF00;
        if carry {
            self.af |= CF_B | 0x8000;
        }
    }

    fn rrc_r(&mut self, opcode: u8) {
        let value = self.get_src_value(opcode);
        let carry = (value & 0x01) != 0;
        self.af &= 0xFF00;
        let val = value >> 1 | (carry as u8) << 7;
        self.set_src_value(opcode, val);
        if carry {
            self.af |= CF_B;
        }
        if val == 0 {
            self.af |= ZF_B;
        }
    }

    fn do_jump_to_immediate(&mut self) {
        let addr = self.imm16();
        self.pc = addr;
        self.tick();
    }

    fn jp_nn(&mut self) {
        self.do_jump_to_immediate();
    }

    fn jp_f(&mut self, opcode: u8) {
        if self.condition(opcode) {
            self.do_jump_to_immediate();
        } else {
            let pc = self.pc.wrapping_add(2);
            self.pc = pc;
            self.tick();
            self.tick();
        }
    }

    fn jp_hl(&mut self) {
        let addr = self.hl;
        self.pc = addr;
    }

    fn do_jump_relative(&mut self) {
        let relative_addr = self.imm8() as i8 as u16;
        let pc = self.pc.wrapping_add(relative_addr);
        self.pc = pc;
        self.tick();
    }

    fn jr_d(&mut self) {
        self.do_jump_relative();
    }

    fn jr_f(&mut self, opcode: u8) {
        if self.condition(opcode) {
            self.do_jump_relative();
        } else {
            self.pc = self.pc.wrapping_add(1);
            self.tick();
        }
    }

    fn do_call(&mut self) {
        let addr = self.imm16();
        let pc = self.pc;
        self.internal_push(pc);
        self.pc = addr;
    }

    fn call_nn(&mut self) {
        self.do_call();
    }

    fn call_f_nn(&mut self, opcode: u8) {
        if self.condition(opcode) {
            self.do_call();
        } else {
            let pc = self.pc.wrapping_add(2);
            self.pc = pc;
            self.tick();
            self.tick();
        }
    }

    fn do_return(&mut self) {
        let pc = self.internal_pop();
        self.pc = pc;
        self.tick();
    }

    fn ret(&mut self) {
        self.do_return();
    }

    fn ret_f(&mut self, opcode: u8) {
        self.tick();
        if self.condition(opcode) {
            self.do_return();
        }
    }

    fn condition(&self, opcode: u8) -> bool {
        match (opcode >> 3) & 0x3 {
            0 => self.af & ZF_B == 0,
            1 => self.af & ZF_B != 0,
            2 => self.af & CF_B == 0,
            3 => self.af & CF_B != 0,
            _ => unreachable!(),
        }
    }

    fn reti(&mut self) {
        self.ret();
        self.ei();
    }

    fn rst(&mut self, opcode: u8) {
        self.sp = self.sp.wrapping_sub(1);
        self.write_mem(self.sp, ((self.pc) >> 8) as u8);
        self.sp = self.sp.wrapping_sub(1);
        self.write_mem(self.sp, ((self.pc) & 0xFF) as u8);
        self.pc = opcode as u16 ^ 0xC7;
    }

    fn halt(&mut self) {
        self.cpu_halted = true;

        if self.any_interrrupt() && !self.ime {
            self.cpu_halted = false;
            self.cpu_halt_bug = true;
        }
    }

    fn stop(&mut self) {
        self.imm8();

        if self.key1 & KEY1_SWITCH_B == 0 {
            self.cpu_halted = true;
        } else {
            self.double_speed = !self.double_speed;

            self.key1 &= !KEY1_SWITCH_B;
            self.key1 ^= KEY1_SPEED_B;
        }
    }

    fn di(&mut self) {
        self.ime = false;
    }

    fn ei(&mut self) {
        self.cpu_ei_delay = true;
    }

    fn ccf(&mut self) {
        self.af ^= CF_B;
        self.af &= !(HF_B | NF_B);
    }

    fn scf(&mut self) {
        self.af |= CF_B;
        self.af &= !(HF_B | NF_B);
    }

    fn nop() {}

    fn daa(&mut self) {
        let mut result = self.af >> 8;

        self.af &= !(0xFF00 | ZF_B);

        if self.af & NF_B == 0 {
            if self.af & HF_B != 0 || (result & 0x0F) > 0x09 {
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

        if (result & 0xFF) == 0 {
            self.af |= ZF_B;
        }

        if (result & 0x100) == 0x100 {
            self.af |= CF_B;
        }

        self.af &= !HF_B;
        self.af |= result << 8;
    }

    fn cpl(&mut self) {
        self.af ^= 0xFF00;
        self.af |= HF_B | NF_B;
    }

    fn push(&mut self, opcode: u8) {
        let register_id = ((opcode >> 4) + 1) & 3;
        let val = *self.regid2reg(register_id);
        self.internal_push(val);
    }

    fn pop(&mut self, opcode: u8) {
        let val = self.internal_pop();
        let register_id = ((opcode >> 4) + 1) & 3;
        *self.regid2reg(register_id) = val;
        self.af &= 0xfff0;
    }

    fn ld16_nn(&mut self, opcode: u8) {
        let register_id = (opcode >> 4) + 1;
        let val = self.imm16();
        *self.regid2reg(register_id) = val;
    }

    fn ld16_nn_sp(&mut self) {
        let val = self.sp;
        let addr = self.imm16();
        self.write_mem(addr, (val & 0xff) as u8);
        self.write_mem(addr.wrapping_add(1), (val >> 8) as u8);
    }

    fn add_sp_r8(&mut self) {
        let sp = self.sp;
        let offset = self.imm8() as i8 as u16;
        self.tick();
        self.tick();
        self.sp = self.sp.wrapping_add(offset);
        self.af &= 0xFF00;

        /* A new instruction, a new meaning for Half Carry! */
        if (sp & 0xF) + (offset & 0xF) > 0xF {
            self.af |= HF_B;
        }

        if (sp & 0xFF) + (offset & 0xFF) > 0xFF {
            self.af |= CF_B;
        }
    }

    fn add_hl_rr(&mut self, opcode: u8) {
        let register_id = (opcode >> 4) + 1;
        let hl = self.hl;
        let rr = *self.regid2reg(register_id);
        self.hl = hl.wrapping_add(rr);

        self.af &= !(NF_B | CF_B | HF_B);

        if ((hl & 0xFFF) + (rr & 0xFFF)) & 0x1000 != 0 {
            self.af |= HF_B;
        }

        if (hl as u32 + rr as u32) & 0x10000 != 0 {
            self.af |= CF_B;
        }

        self.tick();
    }

    fn rlc_r(&mut self, opcode: u8) {
        let value = self.get_src_value(opcode);
        let carry = (value & 0x80) != 0;
        self.af &= 0xFF00;
        self.set_src_value(opcode, (value << 1) | (carry as u8));
        if carry {
            self.af |= CF_B;
        }
        if value == 0 {
            self.af |= ZF_B;
        }
    }

    fn rra(&mut self) {
        let bit1 = (self.af & 0x0100) != 0;
        let carry = (self.af & CF_B) != 0;

        self.af = (self.af >> 1) & 0xFF00;
        if carry {
            self.af |= 0x8000;
        }
        if bit1 {
            self.af |= CF_B;
        }
    }

    fn rr_r(&mut self, opcode: u8) {
        let value = self.get_src_value(opcode);
        let carry = (self.af & CF_B) != 0;
        let bit1 = (value & 0x1) != 0;

        self.af &= 0xFF00;
        let value = (value >> 1) | ((carry as u8) << 7);
        self.set_src_value(opcode, value);
        if bit1 {
            self.af |= CF_B;
        }
        if value == 0 {
            self.af |= ZF_B;
        }
    }

    fn sla_r(&mut self, opcode: u8) {
        let value = self.get_src_value(opcode);
        let carry = (value & 0x80) != 0;
        self.af &= 0xFF00;
        self.set_src_value(opcode, value << 1);
        if carry {
            self.af |= CF_B;
        }
        if value & 0x7F == 0 {
            self.af |= ZF_B;
        }
    }

    fn sra_r(&mut self, opcode: u8) {
        let value = self.get_src_value(opcode);
        let bit7 = value & 0x80;
        self.af &= 0xFF00;
        if value & 1 != 0 {
            self.af |= CF_B;
        }
        let value = (value >> 1) | bit7;
        self.set_src_value(opcode, value);
        if value == 0 {
            self.af |= ZF_B;
        }
    }

    fn srl_r(&mut self, opcode: u8) {
        let value = self.get_src_value(opcode);
        self.af &= 0xFF00;
        self.set_src_value(opcode, value >> 1);
        if value & 1 != 0 {
            self.af |= CF_B;
        }
        if value >> 1 == 0 {
            self.af |= ZF_B;
        }
    }

    fn swap_r(&mut self, opcode: u8) {
        let value = self.get_src_value(opcode);
        self.af &= 0xFF00;
        self.set_src_value(opcode, (value >> 4) | (value << 4));
        if value == 0 {
            self.af |= ZF_B;
        }
    }

    fn bit_r(&mut self, opcode: u8) {
        let value = self.get_src_value(opcode);
        let bit = 1 << ((opcode >> 3) & 7);
        if opcode & 0xC0 == 0x40 {
            /* Bit */
            self.af &= 0xFF00 | CF_B;
            self.af |= HF_B;
            if bit & value == 0 {
                self.af |= ZF_B;
            }
        } else if opcode & 0xC0 == 0x80 {
            /* res */
            self.set_src_value(opcode, value & !bit);
        } else if opcode & 0xC0 == 0xC0 {
            /* set */
            self.set_src_value(opcode, value | bit);
        }
    }

    fn rla(&mut self) {
        let bit7 = (self.af & 0x8000) != 0;
        let carry = (self.af & CF_B) != 0;

        self.af = (self.af & 0xFF00) << 1;
        if carry {
            self.af |= 0x0100;
        }
        if bit7 {
            self.af |= CF_B;
        }
    }

    fn rl_r(&mut self, opcode: u8) {
        let value = self.get_src_value(opcode);
        let carry = (self.af & CF_B) != 0;
        let bit7 = (value & 0x80) != 0;

        self.af &= 0xFF00;
        let value = (value << 1) | carry as u8;
        self.set_src_value(opcode, value);
        if bit7 {
            self.af |= CF_B;
        }
        if value == 0 {
            self.af |= ZF_B;
        }
    }

    /// # Panics
    ///
    /// will panic on illegal opcode
    fn exec(&mut self, opcode: u8) {
        match opcode {
            0x7f => self.ld(A, A),
            0x78 => self.ld(A, B),
            0x79 => self.ld(A, C),
            0x7a => self.ld(A, D),
            0x7b => self.ld(A, E),
            0x7c => self.ld(A, H),
            0x7d => self.ld(A, L),
            0x7e => self.ld(A, Dhl),
            0x47 => self.ld(B, A),
            0x40 => {
                // TODO: debugger
                self.ld(B, B);
            }
            0x41 => self.ld(B, C),
            0x42 => self.ld(B, D),
            0x43 => self.ld(B, E),
            0x44 => self.ld(B, H),
            0x45 => self.ld(B, L),
            0x46 => self.ld(B, Dhl),
            0x4f => self.ld(C, A),
            0x48 => self.ld(C, B),
            0x49 => self.ld(C, C),
            0x4a => self.ld(C, D),
            0x4b => self.ld(C, E),
            0x4c => self.ld(C, H),
            0x4d => self.ld(C, L),
            0x4e => self.ld(C, Dhl),
            0x57 => self.ld(D, A),
            0x50 => self.ld(D, B),
            0x51 => self.ld(D, C),
            0x52 => self.ld(D, D),
            0x53 => self.ld(D, E),
            0x54 => self.ld(D, H),
            0x55 => self.ld(D, L),
            0x56 => self.ld(D, Dhl),
            0x5f => self.ld(E, A),
            0x58 => self.ld(E, B),
            0x59 => self.ld(E, C),
            0x5a => self.ld(E, D),
            0x5b => self.ld(E, E),
            0x5c => self.ld(E, H),
            0x5d => self.ld(E, L),
            0x5e => self.ld(E, Dhl),
            0x67 => self.ld(H, A),
            0x60 => self.ld(H, B),
            0x61 => self.ld(H, C),
            0x62 => self.ld(H, D),
            0x63 => self.ld(H, E),
            0x64 => self.ld(H, H),
            0x65 => self.ld(H, L),
            0x66 => self.ld(H, Dhl),
            0x6f => self.ld(L, A),
            0x68 => self.ld(L, B),
            0x69 => self.ld(L, C),
            0x6a => self.ld(L, D),
            0x6b => self.ld(L, E),
            0x6c => self.ld(L, H),
            0x6d => self.ld(L, L),
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
            0xc3 => self.jp_nn(),
            0xe9 => self.jp_hl(),
            0x18 => self.jr_d(),
            0xcd => self.call_nn(),
            0xc9 => self.ret(),
            0xd9 => self.reti(),
            0xc2 | 0xca | 0xd2 | 0xda => self.jp_f(opcode),
            0x20 | 0x28 | 0x30 | 0x38 => self.jr_f(opcode),
            0xc4 | 0xcc | 0xd4 | 0xdc => self.call_f_nn(opcode),
            0xc0 | 0xc8 | 0xd0 | 0xd8 => self.ret_f(opcode),
            0xc7 | 0xcf | 0xd7 | 0xdf | 0xe7 | 0xef | 0xf7 | 0xff => self.rst(opcode),
            0x76 => self.halt(),
            0x10 => self.stop(),
            0xf3 => self.di(),
            0xfb => self.ei(),
            0x3f => self.ccf(),
            0x37 => self.scf(),
            0x00 => Self::nop(),
            0x27 => self.daa(),
            0x2f => self.cpl(),
            0x01 | 0x11 | 0x21 | 0x31 => self.ld16_nn(opcode),
            0x08 => self.ld16_nn_sp(),
            0xf9 => self.ld16_sp_hl(),
            0xf8 => self.ld_hl_sp_r8(),
            0xc5 | 0xd5 | 0xe5 | 0xf5 => self.push(opcode),
            0xc1 | 0xd1 | 0xe1 | 0xf1 => self.pop(opcode),
            0x09 | 0x19 | 0x29 | 0x39 => self.add_hl_rr(opcode),
            0xe8 => self.add_sp_r8(),
            0x03 | 0x13 | 0x23 | 0x33 => self.inc_rr(opcode),
            0x0b | 0x1b | 0x2b | 0x3b => self.dec_rr(opcode),
            0xcb => self.exec_cb(),
            _ => panic!("Illegal opcode {}", opcode),
        }
    }

    fn exec_cb(&mut self) {
        let opcode = self.imm8();
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
