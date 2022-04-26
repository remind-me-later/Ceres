use super::{
    operands::{Get, JumpCondition, Set},
    registers::Register16::{self, HL},
    Cpu,
};

impl Cpu {
    pub fn ld<G, S>(&mut self, lhs: S, rhs: G)
    where
        G: Get<u8>,
        S: Set<u8>,
    {
        let val = rhs.get(self);
        lhs.set(self, val);
    }

    pub fn ld16_sp_hl(&mut self) {
        let val = self.registers.read16(HL);
        self.registers.sp = val;
        self.memory.tick_t_cycle();
    }

    pub fn add<G>(&mut self, rhs: G)
    where
        G: Get<u8>,
    {
        let a = self.registers.a;
        let val = rhs.get(self);
        let (output, carry) = a.overflowing_add(val);
        self.registers.set_zf(output == 0);
        self.registers.set_nf(false);
        self.registers.set_hf((a & 0xf) + (val & 0xf) > 0xf);
        self.registers.set_cf(carry);
        self.registers.a = output;
    }

    pub fn adc<G>(&mut self, rhs: G)
    where
        G: Get<u8>,
    {
        let a = self.registers.a;
        let cy = u8::from(self.registers.cf());
        let val = rhs.get(self);
        let output = a.wrapping_add(val).wrapping_add(cy);
        self.registers.set_zf(output == 0);
        self.registers.set_nf(false);
        self.registers.set_hf((a & 0xf) + (val & 0xf) + cy > 0xf);
        self.registers
            .set_cf(u16::from(a) + u16::from(val) + u16::from(cy) > 0xff);
        self.registers.a = output;
    }

    pub fn sub<G>(&mut self, rhs: G)
    where
        G: Get<u8>,
    {
        let a = self.registers.a;
        let val = rhs.get(self);
        let (output, carry) = a.overflowing_sub(val);
        self.registers.set_zf(output == 0);
        self.registers.set_nf(true);
        self.registers.set_hf(a & 0xf < val & 0xf);
        self.registers.set_cf(carry);
        self.registers.a = output;
    }

    pub fn sbc<G>(&mut self, rhs: G)
    where
        G: Get<u8>,
    {
        let a = self.registers.a;
        let cy = u8::from(self.registers.cf());
        let val = rhs.get(self);
        let output = a.wrapping_sub(val).wrapping_sub(cy);
        self.registers.set_zf(output == 0);
        self.registers.set_nf(true);
        self.registers.set_hf(a & 0xf < (val & 0xf) + cy);
        self.registers
            .set_cf(u16::from(a) < u16::from(val) + u16::from(cy));
        self.registers.a = output;
    }

    pub fn cp<G>(&mut self, rhs: G)
    where
        G: Get<u8>,
    {
        let a = self.registers.a;
        let val = rhs.get(self);
        let result = a.wrapping_sub(val);
        self.registers.set_zf(result == 0);
        self.registers.set_nf(true);
        self.registers
            .set_hf((a & 0xf).wrapping_sub(val & 0xf) & (0xf + 1) != 0);
        self.registers.set_cf(a < val);
    }

    pub fn and<G>(&mut self, rhs: G)
    where
        G: Get<u8>,
    {
        let val = rhs.get(self);
        let a = self.registers.a & val;
        self.registers.a = a;
        self.registers.set_zf(a == 0);
        self.registers.set_nf(false);
        self.registers.set_hf(true);
        self.registers.set_cf(false);
    }

    pub fn or<G>(&mut self, rhs: G)
    where
        G: Get<u8>,
    {
        let val = rhs.get(self);
        let a = self.registers.a | val;
        self.registers.a = a;
        self.registers.set_zf(a == 0);
        self.registers.set_nf(false);
        self.registers.set_hf(false);
        self.registers.set_cf(false);
    }

    pub fn xor<G>(&mut self, rhs: G)
    where
        G: Get<u8>,
    {
        let read = rhs.get(self);
        let a = self.registers.a ^ read;
        self.registers.a = a;
        self.registers.set_zf(a == 0);
        self.registers.set_nf(false);
        self.registers.set_hf(false);
        self.registers.set_cf(false);
    }

    pub fn inc<GS>(&mut self, rhs: GS)
    where
        GS: Get<u8> + Set<u8>,
    {
        let read = rhs.get(self);
        let val = read.wrapping_add(1);
        self.registers.set_zf(val == 0);
        self.registers.set_nf(false);
        self.registers.set_hf(read & 0xf == 0xf);
        rhs.set(self, val);
    }

    pub fn inc16(&mut self, register: Register16) {
        let read = self.registers.read16(register);
        let val = read.wrapping_add(1);
        self.registers.write16(register, val);
        self.memory.tick_t_cycle();
    }

    #[allow(clippy::verbose_bit_mask)]
    pub fn dec<GS>(&mut self, rhs: GS)
    where
        GS: Get<u8> + Set<u8>,
    {
        let read = rhs.get(self);
        let val = read.wrapping_sub(1);
        self.registers.set_zf(val == 0);
        self.registers.set_nf(true);
        self.registers.set_hf(read & 0xf == 0);
        rhs.set(self, val);
    }

    pub fn dec16(&mut self, register: Register16) {
        let read = self.registers.read16(register);
        let val = read.wrapping_sub(1);
        self.registers.write16(register, val);
        self.memory.tick_t_cycle();
    }

    pub fn ld16_hl_sp_dd(&mut self) {
        let offset = self.read_immediate() as i8 as u16;
        let sp = self.registers.sp;
        let val = sp.wrapping_add(offset);
        let tmp = sp ^ val ^ offset;
        self.registers.write16(HL, val);
        self.registers.set_zf(false);
        self.registers.set_nf(false);
        self.registers.set_hf((tmp & 0x10) == 0x10);
        self.registers.set_cf((tmp & 0x100) == 0x100);
        self.memory.tick_t_cycle();
    }

    pub fn rlca(&mut self) {
        let val = self.internal_rlc(self.registers.a);
        self.registers.a = val;
        self.registers.set_zf(false);
    }

    pub fn rla(&mut self) {
        let val = self.internal_rl(self.registers.a);
        self.registers.a = val;
        self.registers.set_zf(false);
    }

    pub fn rrca(&mut self) {
        let val = self.internal_rrc(self.registers.a);
        self.registers.a = val;
        self.registers.set_zf(false);
    }

    pub fn rra(&mut self) {
        let val = self.internal_rr(self.registers.a);
        self.registers.a = val;
        self.registers.set_zf(false);
    }

    pub fn do_jump_to_immediate(&mut self) {
        let address = self.read_immediate16();
        self.registers.pc = address;
        self.memory.tick_t_cycle();
    }

    pub fn jp_nn(&mut self) {
        self.do_jump_to_immediate();
    }

    pub fn jp_f(&mut self, condition: JumpCondition) {
        if condition.check(self) {
            self.do_jump_to_immediate();
        } else {
            let pc = self.registers.pc.wrapping_add(2);
            self.registers.pc = pc;
            self.memory.tick_t_cycle();
            self.memory.tick_t_cycle();
        }
    }

    pub fn jp_hl(&mut self) {
        let address = self.registers.read16(HL);
        self.registers.pc = address;
    }

    fn do_jump_relative(&mut self) {
        let relative_address = self.read_immediate() as i8 as u16;
        let pc = self.registers.pc.wrapping_add(relative_address);
        self.registers.pc = pc;
        self.memory.tick_t_cycle();
    }

    pub fn jr_d(&mut self) {
        self.do_jump_relative();
    }

    pub fn jr_f(&mut self, condition: JumpCondition) {
        if condition.check(self) {
            self.do_jump_relative();
        } else {
            self.registers.increase_pc();
            self.memory.tick_t_cycle();
        }
    }

    pub fn do_call(&mut self) {
        let address = self.read_immediate16();
        let pc = self.registers.pc;
        self.internal_push(pc);
        self.registers.pc = address;
    }

    pub fn call_nn(&mut self) {
        self.do_call();
    }

    pub fn call_f_nn(&mut self, condition: JumpCondition) {
        if condition.check(self) {
            self.do_call();
        } else {
            let pc = self.registers.pc.wrapping_add(2);
            self.registers.pc = pc;
            self.memory.tick_t_cycle();
            self.memory.tick_t_cycle();
        }
    }

    fn do_return(&mut self) {
        let pc = self.internal_pop();
        self.registers.pc = pc;
        self.memory.tick_t_cycle();
    }

    pub fn ret(&mut self) {
        self.do_return();
    }

    pub fn ret_f(&mut self, condition: JumpCondition) {
        self.memory.tick_t_cycle();
        if condition.check(self) {
            self.do_return();
        }
    }

    pub fn reti(&mut self) {
        self.ret();
        self.ei();
    }

    pub fn rst(&mut self, address: u16) {
        let pc = self.registers.pc;
        self.internal_push(pc);
        self.registers.pc = address;
    }

    pub fn halt(&mut self) {
        self.halted = true;

        if self.memory.interrupt_controller().halt_bug_condition() && !self.ime {
            self.halted = false;
            self.halt_bug = true;
        }
    }

    pub fn stop(&mut self) {
        self.read_immediate();

        if self.memory.speed_switch_register().speed_switch_requested() {
            self.memory.switch_speed();
            self.memory
                .mut_speed_switch_register()
                .acknowledge_speed_switch();
        } else {
            self.halted = true;
        }
    }

    pub fn di(&mut self) {
        self.ime = false;
    }

    pub fn ei(&mut self) {
        self.ei_delay = true;
    }

    pub fn ccf(&mut self) {
        self.registers.set_nf(false);
        self.registers.set_hf(false);
        self.registers.set_cf(!self.registers.cf());
    }

    pub fn scf(&mut self) {
        self.registers.set_nf(false);
        self.registers.set_hf(false);
        self.registers.set_cf(true);
    }

    #[allow(clippy::unused_self)]
    pub fn nop(&mut self) {}

    pub fn daa(&mut self) {
        // DAA table in page 110 of the official "Game Boy Programming Manual"
        let mut carry = false;
        if !self.registers.nf() {
            if self.registers.cf() || self.registers.a > 0x99 {
                self.registers.a = self.registers.a.wrapping_add(0x60);
                carry = true;
            }
            if self.registers.hf() || self.registers.a & 0x0f > 0x09 {
                self.registers.a = self.registers.a.wrapping_add(0x06);
            }
        } else if self.registers.cf() {
            carry = true;
            self.registers.a =
                self.registers
                    .a
                    .wrapping_add(if self.registers.hf() { 0x9a } else { 0xa0 });
        } else if self.registers.hf() {
            self.registers.a = self.registers.a.wrapping_add(0xfa);
        }

        self.registers.set_zf(self.registers.a == 0);
        self.registers.set_hf(false);
        self.registers.set_cf(carry);
    }

    pub fn cpl(&mut self) {
        let a = self.registers.a;
        self.registers.a = !a;
        self.registers.set_nf(true);
        self.registers.set_hf(true);
    }

    pub fn push(&mut self, register: Register16) {
        let val = self.registers.read16(register);
        self.internal_push(val);
    }

    pub fn pop(&mut self, register: Register16) {
        let val = self.internal_pop();
        self.registers.write16(register, val);
    }

    pub fn ld16_nn(&mut self, reg: Register16) {
        let value = self.read_immediate16();
        self.registers.write16(reg, value);
    }

    pub fn ld16_nn_sp(&mut self) {
        let val = self.registers.sp;
        let address = self.read_immediate16();
        self.memory.write(address, (val & 0xff) as u8);
        self.memory.write(address.wrapping_add(1), (val >> 8) as u8);
    }

    pub fn add16_sp_dd(&mut self) {
        let offset = self.read_immediate() as i8 as u16;
        let sp = self.registers.sp;
        let val = sp.wrapping_add(offset);
        let tmp = sp ^ val ^ offset;
        self.registers.sp = val;
        self.registers.set_zf(false);
        self.registers.set_nf(false);
        self.registers.set_hf((tmp & 0x10) == 0x10);
        self.registers.set_cf((tmp & 0x100) == 0x100);
        self.memory.tick_t_cycle();
        self.memory.tick_t_cycle();
    }

    pub fn add_hl(&mut self, register: Register16) {
        let hl = self.registers.read16(HL);
        let val = self.registers.read16(register);
        let res = hl.wrapping_add(val);
        self.registers.write16(HL, res);

        self.registers.set_nf(false);

        // check carry from bit 11
        let mask = 0b0111_1111_1111;
        let half_carry = (hl & mask) + (val & mask) > mask;

        self.registers.set_hf(half_carry);
        self.registers.set_cf(hl > 0xffff - val);

        self.memory.tick_t_cycle();
    }

    pub fn rlc<GS>(&mut self, rhs: GS)
    where
        GS: Get<u8> + Set<u8>,
    {
        let read = rhs.get(self);
        let val = self.internal_rlc(read);
        rhs.set(self, val);
    }

    pub fn rl<GS>(&mut self, rhs: GS)
    where
        GS: Get<u8> + Set<u8>,
    {
        let read = rhs.get(self);
        let val = self.internal_rl(read);
        rhs.set(self, val);
    }

    pub fn rrc<GS>(&mut self, rhs: GS)
    where
        GS: Get<u8> + Set<u8>,
    {
        let read = rhs.get(self);
        let val = self.internal_rrc(read);
        rhs.set(self, val);
    }

    pub fn rr<GS>(&mut self, rhs: GS)
    where
        GS: Get<u8> + Set<u8>,
    {
        let read = rhs.get(self);
        let val = self.internal_rr(read);
        rhs.set(self, val);
    }

    pub fn sla<GS>(&mut self, rhs: GS)
    where
        GS: Get<u8> + Set<u8>,
    {
        let read = rhs.get(self);
        let val = read << 1;
        let co = read & 0x80;
        self.registers.set_zf(val == 0);
        self.registers.set_nf(false);
        self.registers.set_hf(false);
        self.registers.set_cf(co != 0);
        rhs.set(self, val);
    }

    pub fn sra<GS>(&mut self, rhs: GS)
    where
        GS: Get<u8> + Set<u8>,
    {
        let read = rhs.get(self);
        let hi = read & 0x80;
        let val = (read >> 1) | hi;
        let co = read & 0x01;
        self.registers.set_zf(val == 0);
        self.registers.set_nf(false);
        self.registers.set_hf(false);
        self.registers.set_cf(co != 0);

        rhs.set(self, val);
    }

    pub fn srl<GS>(&mut self, rhs: GS)
    where
        GS: Get<u8> + Set<u8>,
    {
        let read = rhs.get(self);
        let val = read >> 1;
        let co = read & 0x01;
        self.registers.set_zf(val == 0);
        self.registers.set_nf(false);
        self.registers.set_hf(false);
        self.registers.set_cf(co != 0);
        rhs.set(self, val);
    }

    pub fn swap<GS>(&mut self, rhs: GS)
    where
        GS: Get<u8> + Set<u8>,
    {
        let read = rhs.get(self);
        let val = (read >> 4) | (read << 4);
        self.registers.set_zf(read == 0);
        self.registers.set_nf(false);
        self.registers.set_hf(false);
        self.registers.set_cf(false);
        rhs.set(self, val);
    }

    pub fn bit<G>(&mut self, rhs: G, bit: usize)
    where
        G: Get<u8>,
    {
        let read = rhs.get(self);
        let value = read & (1 << bit);
        self.registers.set_zf(value == 0);
        self.registers.set_nf(false);
        self.registers.set_hf(true);
    }

    pub fn set<GS>(&mut self, rhs: GS, bit: usize)
    where
        GS: Get<u8> + Set<u8>,
    {
        let read = rhs.get(self);
        let val = read | (1 << bit);
        rhs.set(self, val);
    }

    pub fn res<GS>(&mut self, rhs: GS, bit: usize)
    where
        GS: Get<u8> + Set<u8>,
    {
        let read = rhs.get(self);
        let val = read & !(1 << bit);
        rhs.set(self, val);
    }
}
