#[cfg(feature = "debugging_capability")]
use std::println;

use {
    crate::{Gb, KEY1_SPEED_B, KEY1_SWITCH_B},
    core::{fmt::Display, intrinsics::unlikely},
    Ld8::{Dhl, A, B, C, D, E, H, L},
};

const ZF_B: u16 = 0x80;
const NF_B: u16 = 0x40;
const HF_B: u16 = 0x20;
const CF_B: u16 = 0x10;

#[derive(Clone, Copy)]
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

impl Display for Ld8 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            A => write!(f, "A"),
            B => write!(f, "B"),
            C => write!(f, "C"),
            D => write!(f, "D"),
            E => write!(f, "E"),
            H => write!(f, "H"),
            L => write!(f, "L"),
            Dhl => write!(f, "(HL)"),
        }
    }
}

impl Ld8 {
    #[inline]
    fn read(self, gb: &mut Gb) -> u8 {
        match self {
            Self::A => (gb.af >> 8) as u8,
            Self::B => (gb.bc >> 8) as u8,
            Self::C => (gb.bc & 0xFF) as u8,
            Self::D => (gb.de >> 8) as u8,
            Self::E => (gb.de & 0xFF) as u8,
            Self::H => (gb.hl >> 8) as u8,
            Self::L => (gb.hl & 0xFF) as u8,
            Self::Dhl => gb.cpu_read(gb.hl),
        }
    }

    #[inline]
    fn write(self, gb: &mut Gb, val: u8) {
        match self {
            Self::A => {
                gb.af &= 0x00FF;
                gb.af |= u16::from(val) << 8;
            }
            Self::B => {
                gb.bc &= 0x00FF;
                gb.bc |= u16::from(val) << 8;
            }
            Self::C => {
                gb.bc &= 0xFF00;
                gb.bc |= u16::from(val);
            }
            Self::D => {
                gb.de &= 0x00FF;
                gb.de |= u16::from(val) << 8;
            }
            Self::E => {
                gb.de &= 0xFF00;
                gb.de |= u16::from(val);
            }
            Self::H => {
                gb.hl &= 0x00FF;
                gb.hl |= u16::from(val) << 8;
            }
            Self::L => {
                gb.hl &= 0xFF00;
                gb.hl |= u16::from(val);
            }
            Self::Dhl => gb.cpu_write(gb.hl, val),
        }
    }
}

impl Gb {
    pub(crate) fn run_cpu(&mut self) -> ! {
        loop {
            if self.cpu_ei_delay {
                self.ime = true;
                self.cpu_ei_delay = false;
            }

            if unlikely(self.cpu_halted) {
                self.empty_cycle();
            } else {
                let opcode = self.imm_u8();
                self.run_hdma();

                if unlikely(self.halt_bug) {
                    self.pc = self.pc.wrapping_sub(1);
                    self.halt_bug = false;
                }

                self.exec(opcode);
            }

            self.catch_up();

            // any interrupts?
            if self.ifr & self.ie & 0x1F == 0 {
                continue;
            }

            // interrupt exits halt
            self.cpu_halted = false;

            if !self.ime {
                continue;
            }

            // push pc to stack
            self.empty_cycle();
            self.sp = self.sp.wrapping_sub(1);
            self.cpu_write(self.sp, (self.pc >> 8) as u8);
            self.sp = self.sp.wrapping_sub(1);
            self.cpu_write(self.sp, (self.pc & 0xFF) as u8);
            self.empty_cycle();

            self.ime = false;
            // recompute, maybe ifr changed
            let ints = self.ifr & self.ie & 0x1F;
            let trail_zeros = (ints.trailing_zeros() & 7) as u16;
            // get rightmost interrupt
            let int = u8::from(ints != 0) << trail_zeros;
            // compute direction of interrupt vector
            self.pc = 0x40 | trail_zeros << 3;
            // acknowledge
            self.ifr &= !int;
        }
    }

    #[inline]
    fn cpu_write(&mut self, addr: u16, val: u8) {
        self.empty_cycle();
        self.catch_up();
        self.write_mem(addr, val);
    }

    #[inline]
    fn cpu_read(&mut self, addr: u16) -> u8 {
        self.empty_cycle();
        self.catch_up();
        self.read_mem(addr)
    }

    #[inline]
    fn catch_up(&mut self) {
        self.advance_cycles(self.delay_cycles);
        self.delay_cycles = 0;
    }

    #[inline]
    fn empty_cycle(&mut self) {
        self.delay_cycles += 4;
    }

    #[inline]
    fn imm_u8(&mut self) -> u8 {
        let val = self.cpu_read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        val
    }

    #[inline]
    fn imm_u16(&mut self) -> u16 {
        let lo = u16::from(self.imm_u8());
        let hi = u16::from(self.imm_u8());
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

    #[cfg(feature = "debugging_capability")]
    fn regid2regname(id: u8) -> &'static str {
        match id {
            0 => "AF",
            1 => "BC",
            2 => "DE",
            3 => "HL",
            4 => "SP",
            _ => unreachable!(),
        }
    }

    #[cfg(feature = "debugging_capability")]
    fn get_src_name(opcode: u8) -> &'static str {
        let reg_id = ((opcode >> 1) + 1) & 3;
        let lo = opcode & 1 != 0;
        if reg_id == 0 {
            if lo {
                return "A";
            }
            return "(HL)";
        }
        if lo {
            return Self::regid2reglowname(reg_id);
        }
        Self::regid2reghighname(reg_id)
    }

    #[cfg(feature = "debugging_capability")]
    fn regid2reghighname(id: u8) -> &'static str {
        match id {
            0 => "A",
            1 => "B",
            2 => "D",
            3 => "H",
            _ => unreachable!(),
        }
    }

    #[cfg(feature = "debugging_capability")]
    fn get_condition_name(opcode: u8) -> &'static str {
        match (opcode >> 3) & 0x3 {
            0 => "NZ",
            1 => "Z",
            2 => "NC",
            3 => "C",
            _ => unreachable!(),
        }
    }

    #[cfg(feature = "debugging_capability")]
    fn regid2reglowname(id: u8) -> &'static str {
        match id {
            1 => "C",
            2 => "E",
            3 => "L",
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
            return (*self.regid2reg(reg_id) & 0xFF) as u8;
        }
        (*self.regid2reg(reg_id) >> 8) as u8
    }

    #[inline]
    fn set_src_val(&mut self, opcode: u8, val: u8) {
        let reg_id = ((opcode >> 1) + 1) & 3;
        let lo = opcode & 1 != 0;

        if reg_id == 0 {
            if lo {
                self.af &= 0xFF;
                self.af |= u16::from(val) << 8;
            } else {
                self.cpu_write(self.hl, val);
            }
        } else if lo {
            *self.regid2reg(reg_id) &= 0xFF00;
            *self.regid2reg(reg_id) |= u16::from(val);
        } else {
            *self.regid2reg(reg_id) &= 0xFF;
            *self.regid2reg(reg_id) |= u16::from(val) << 8;
        }
    }

    // ###########
    // # Opcodes #
    // ###########

    #[inline]
    fn ld(&mut self, t: Ld8, s: Ld8) {
        let val = s.read(self);
        t.write(self, val);

        #[cfg(feature = "debugging_capability")]
        {
            println!("ld {t}, {s}");
        }
    }

    #[inline]
    fn ld_a_drr(&mut self, opcode: u8) {
        let reg_id = (opcode >> 4) + 1;
        self.af &= 0xFF;
        let tmp = *self.regid2reg(reg_id);
        self.af |= u16::from(self.cpu_read(tmp)) << 8;

        #[cfg(feature = "debugging_capability")]
        {
            println!("ld A, ({})", Self::regid2regname(reg_id));
        }
    }

    #[inline]
    fn ld_drr_a(&mut self, opcode: u8) {
        let reg_id = (opcode >> 4) + 1;
        let tmp = *self.regid2reg(reg_id);
        self.cpu_write(tmp, (self.af >> 8) as u8);

        #[cfg(feature = "debugging_capability")]
        {
            println!("ld ({}), A", Self::regid2regname(reg_id));
        }
    }

    #[inline]
    fn ld_da16_a(&mut self) {
        let addr = self.imm_u16();
        self.cpu_write(addr, (self.af >> 8) as u8);

        #[cfg(feature = "debugging_capability")]
        {
            println!("ld ({:#06x}), A", addr);
        }
    }

    #[inline]
    fn ld_a_da16(&mut self) {
        self.af &= 0xFF;
        let addr = self.imm_u16();
        self.af |= u16::from(self.cpu_read(addr)) << 8;

        #[cfg(feature = "debugging_capability")]
        {
            println!("ld A, ({:#06x})", addr);
        }
    }

    #[inline]
    fn ld_dhli_a(&mut self) {
        let addr = self.hl;
        self.cpu_write(addr, (self.af >> 8) as u8);
        self.hl = addr.wrapping_add(1);

        #[cfg(feature = "debugging_capability")]
        {
            println!("ld (HL+), A");
        }
    }

    #[inline]
    fn ld_dhld_a(&mut self) {
        let addr = self.hl;
        self.cpu_write(addr, (self.af >> 8) as u8);
        self.hl = addr.wrapping_sub(1);

        #[cfg(feature = "debugging_capability")]
        {
            println!("ld (HL-), A");
        }
    }

    #[inline]
    fn ld_a_dhli(&mut self) {
        let addr = self.hl;
        let val = u16::from(self.cpu_read(addr));
        self.af &= 0xFF;
        self.af |= val << 8;
        self.hl = addr.wrapping_add(1);

        #[cfg(feature = "debugging_capability")]
        {
            println!("ld A, (HL+)");
        }
    }

    #[inline]
    fn ld_a_dhld(&mut self) {
        let addr = self.hl;
        let val = u16::from(self.cpu_read(addr));
        self.af &= 0xFF;
        self.af |= val << 8;
        self.hl = addr.wrapping_sub(1);

        #[cfg(feature = "debugging_capability")]
        {
            println!("ld A, (HL-)");
        }
    }

    #[inline]
    fn ldh_da8_a(&mut self) {
        let tmp = u16::from(self.imm_u8());
        let a = (self.af >> 8) as u8;
        self.cpu_write(0xFF00 | tmp, a);

        #[cfg(feature = "debugging_capability")]
        {
            println!("ldh ({:#04x}), A", tmp);
        }
    }

    #[inline]
    fn ldh_a_da8(&mut self) {
        let tmp = u16::from(self.imm_u8());
        self.af &= 0xFF;
        self.af |= u16::from(self.cpu_read(0xFF00 | tmp)) << 8;

        #[cfg(feature = "debugging_capability")]
        {
            println!("ldh A, ({:#04x})", tmp);
        }
    }

    #[inline]
    fn ldh_dc_a(&mut self) {
        self.cpu_write(0xFF00 | self.bc & 0xFF, (self.af >> 8) as u8);

        #[cfg(feature = "debugging_capability")]
        {
            println!("ldh (C), A");
        }
    }

    #[inline]
    fn ldh_a_dc(&mut self) {
        self.af &= 0xFF;
        self.af |= u16::from(self.cpu_read(0xFF00 | self.bc & 0xFF)) << 8;

        #[cfg(feature = "debugging_capability")]
        {
            println!("ldh A, (C)");
        }
    }

    #[inline]
    fn ld_hr_d8(&mut self, opcode: u8) {
        let reg_id = ((opcode >> 4) + 1) & 0x03;
        *self.regid2reg(reg_id) &= 0xFF;
        let tmp = u16::from(self.imm_u8());
        *self.regid2reg(reg_id) |= tmp << 8;

        #[cfg(feature = "debugging_capability")]
        {
            println!("ld {}, {:#04x}", Self::regid2reghighname(reg_id), tmp);
        }
    }

    #[inline]
    fn ld_lr_d8(&mut self, opcode: u8) {
        let reg_id = (opcode >> 4) + 1;
        *self.regid2reg(reg_id) &= 0xFF00;
        let tmp = u16::from(self.imm_u8());
        *self.regid2reg(reg_id) |= tmp;

        #[cfg(feature = "debugging_capability")]
        {
            println!("ld {}, {:#04x}", Self::regid2reglowname(reg_id), tmp);
        }
    }

    #[inline]
    fn ld_dhl_d8(&mut self) {
        let tmp = self.imm_u8();
        self.cpu_write(self.hl, tmp);

        #[cfg(feature = "debugging_capability")]
        {
            println!("ld (HL), {:#02x}", tmp);
        }
    }

    #[inline]
    fn ld16_sp_hl(&mut self) {
        let val = self.hl;
        self.sp = val;
        self.empty_cycle();

        #[cfg(feature = "debugging_capability")]
        {
            println!("ld SP, HL");
        }
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
    fn add_a_r(&mut self, opcode: u8) {
        let val = u16::from(self.get_src_val(opcode));
        self.add(val);

        #[cfg(feature = "debugging_capability")]
        {
            println!("add A, {}", Self::get_src_name(opcode));
        }
    }

    #[inline]
    fn add_a_d8(&mut self) {
        let val = u16::from(self.imm_u8());
        self.add(val);

        #[cfg(feature = "debugging_capability")]
        {
            println!("add A, {:#02x}", val);
        }
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
    fn sub_a_r(&mut self, opcode: u8) {
        let val = u16::from(self.get_src_val(opcode));
        self.sub(val);

        #[cfg(feature = "debugging_capability")]
        {
            println!("sub A, {}", Self::get_src_name(opcode));
        }
    }

    #[inline]
    fn sub_a_d8(&mut self) {
        let val = u16::from(self.imm_u8());
        self.sub(val);

        #[cfg(feature = "debugging_capability")]
        {
            println!("sub A, {:#02x}", val);
        }
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
    fn sbc_a_r(&mut self, opcode: u8) {
        let val = u16::from(self.get_src_val(opcode));
        self.sbc(val);

        #[cfg(feature = "debugging_capability")]
        {
            println!("sbc A, {}", Self::get_src_name(opcode));
        }
    }

    #[inline]
    fn sbc_a_d8(&mut self) {
        let val = u16::from(self.imm_u8());
        self.sbc(val);

        #[cfg(feature = "debugging_capability")]
        {
            println!("sbc A, {:#02x}", val);
        }
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
    fn adc_a_r(&mut self, opcode: u8) {
        let val = u16::from(self.get_src_val(opcode));
        self.adc(val);

        #[cfg(feature = "debugging_capability")]
        {
            println!("adc A, {}", Self::get_src_name(opcode));
        }
    }

    #[inline]
    fn adc_a_d8(&mut self) {
        let val = u16::from(self.imm_u8());
        self.adc(val);

        #[cfg(feature = "debugging_capability")]
        {
            println!("adc A, {:#02x}", val);
        }
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
    fn or_a_r(&mut self, opcode: u8) {
        let val = u16::from(self.get_src_val(opcode));
        self.or(val);

        #[cfg(feature = "debugging_capability")]
        {
            println!("or A, {}", Self::get_src_name(opcode));
        }
    }

    #[inline]
    fn or_a_d8(&mut self) {
        let val = u16::from(self.imm_u8());
        self.or(val);

        #[cfg(feature = "debugging_capability")]
        {
            println!("or A, {:#02x}", val);
        }
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
    fn xor_a_r(&mut self, opcode: u8) {
        let val = u16::from(self.get_src_val(opcode));
        self.xor(val);

        #[cfg(feature = "debugging_capability")]
        {
            println!("xor A, {}", Self::get_src_name(opcode));
        }
    }

    #[inline]
    fn xor_a_d8(&mut self) {
        let val = u16::from(self.imm_u8());
        self.xor(val);

        #[cfg(feature = "debugging_capability")]
        {
            println!("xor A, {:#02x}", val);
        }
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
    fn and_a_r(&mut self, opcode: u8) {
        let val = u16::from(self.get_src_val(opcode));
        self.and(val);

        #[cfg(feature = "debugging_capability")]
        {
            println!("and A, {}", Self::get_src_name(opcode));
        }
    }

    #[inline]
    fn and_a_d8(&mut self) {
        let val = u16::from(self.imm_u8());
        self.and(val);

        #[cfg(feature = "debugging_capability")]
        {
            println!("and A, {:#02x}", val);
        }
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
    fn cp_a_r(&mut self, opcode: u8) {
        let val = u16::from(self.get_src_val(opcode));
        self.cp(val);

        #[cfg(feature = "debugging_capability")]
        {
            println!("cp A, {}", Self::get_src_name(opcode));
        }
    }

    #[inline]
    fn cp_a_d8(&mut self) {
        let val = u16::from(self.imm_u8());
        self.cp(val);

        #[cfg(feature = "debugging_capability")]
        {
            println!("cp A, {:#02x}", val);
        }
    }

    #[inline]
    fn inc_lr(&mut self, opcode: u8) {
        let reg_id = (opcode >> 4) + 1;
        let val = (*self.regid2reg(reg_id) + 1) & 0xFF;
        *self.regid2reg(reg_id) = (*self.regid2reg(reg_id) & 0xFF00) | val;

        self.af &= !(NF_B | ZF_B | HF_B);

        if ((*self.regid2reg(reg_id)) & 0x0F) == 0 {
            self.af |= HF_B;
        }

        if ((*self.regid2reg(reg_id)) & 0xFF) == 0 {
            self.af |= ZF_B;
        }

        #[cfg(feature = "debugging_capability")]
        {
            println!("inc {}", Self::regid2reglowname(reg_id));
        }
    }

    #[inline]
    fn dec_lr(&mut self, opcode: u8) {
        let reg_id = (opcode >> 4) + 1;
        let val = (*self.regid2reg(reg_id)).wrapping_sub(1) & 0xFF;
        (*self.regid2reg(reg_id)) = ((*self.regid2reg(reg_id)) & 0xFF00) | val;

        self.af &= !(ZF_B | HF_B);
        self.af |= NF_B;

        if ((*self.regid2reg(reg_id)) & 0x0F) == 0xF {
            self.af |= HF_B;
        }

        if ((*self.regid2reg(reg_id)) & 0xFF) == 0 {
            self.af |= ZF_B;
        }

        #[cfg(feature = "debugging_capability")]
        {
            println!("dec {}", Self::regid2reglowname(reg_id));
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

        #[cfg(feature = "debugging_capability")]
        {
            println!("inc {}", Self::regid2reghighname(reg_id));
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

        #[cfg(feature = "debugging_capability")]
        {
            println!("dec {}", Self::regid2reghighname(reg_id));
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

        #[cfg(feature = "debugging_capability")]
        {
            println!("inc (HL)");
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

        #[cfg(feature = "debugging_capability")]
        {
            println!("dec (HL)");
        }
    }

    #[inline]
    fn inc_rr(&mut self, opcode: u8) {
        let reg_id = (opcode >> 4) + 1;
        let reg = self.regid2reg(reg_id);
        *reg = reg.wrapping_add(1);
        self.empty_cycle();

        #[cfg(feature = "debugging_capability")]
        {
            println!("inc {}", Self::regid2regname(reg_id));
        }
    }

    #[inline]
    fn dec_rr(&mut self, opcode: u8) {
        let reg_id = (opcode >> 4) + 1;
        let reg = self.regid2reg(reg_id);
        *reg = reg.wrapping_sub(1);
        self.empty_cycle();

        #[cfg(feature = "debugging_capability")]
        {
            println!("dec {}", Self::regid2regname(reg_id));
        }
    }

    #[inline]
    fn ld_hl_sp_r8(&mut self) {
        self.af &= 0xFF00;
        #[allow(clippy::cast_possible_wrap)]
        let offset_signed = self.imm_u8() as i8;
        #[allow(clippy::cast_sign_loss)]
        let offset = offset_signed as u16;
        self.empty_cycle();
        self.hl = self.sp.wrapping_add(offset);

        if (self.sp & 0xF) + (offset & 0xF) > 0xF {
            self.af |= HF_B;
        }

        if (self.sp & 0xFF) + (offset & 0xFF) > 0xFF {
            self.af |= CF_B;
        }

        #[cfg(feature = "debugging_capability")]
        {
            println!("ld (HL), SP+{:#02x}", offset_signed);
        }
    }

    #[inline]
    fn rlca(&mut self) {
        let carry = (self.af & 0x8000) != 0;

        self.af = (self.af & 0xFF00) << 1;
        if carry {
            self.af |= CF_B | 0x0100;
        }

        #[cfg(feature = "debugging_capability")]
        {
            println!("rlca");
        }
    }

    #[inline]
    fn rrca(&mut self) {
        let carry = (self.af & 0x100) != 0;
        self.af = (self.af >> 1) & 0xFF00;
        if carry {
            self.af |= CF_B | 0x8000;
        }

        #[cfg(feature = "debugging_capability")]
        {
            println!("rrca");
        }
    }

    #[inline]
    fn rrc_r(&mut self, opcode: u8) {
        let val = self.get_src_val(opcode);
        let carry = (val & 0x01) != 0;
        self.af &= 0xFF00;
        let val = val >> 1 | u8::from(carry) << 7;
        self.set_src_val(opcode, val);
        if carry {
            self.af |= CF_B;
        }
        if val == 0 {
            self.af |= ZF_B;
        }

        #[cfg(feature = "debugging_capability")]
        {
            println!("rrc {}", Self::get_src_name(opcode));
        }
    }

    #[inline]
    fn do_jump_to_immediate(&mut self) {
        let addr = self.imm_u16();
        self.pc = addr;
        self.empty_cycle();

        #[cfg(feature = "debugging_capability")]
        {
            println!("jp {:#06x}", addr);
        }
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
            self.empty_cycle();
            self.empty_cycle();
        }

        #[cfg(feature = "debugging_capability")]
        {
            println!("jp {}", Self::get_condition_name(opcode));
        }
    }

    #[inline]
    fn jp_hl(&mut self) {
        let addr = self.hl;
        self.pc = addr;

        #[cfg(feature = "debugging_capability")]
        {
            println!("jp (HL)");
        }
    }

    #[inline]
    fn do_jump_relative(&mut self) {
        #[allow(clippy::cast_possible_wrap)]
        let relative_addr_signed = self.imm_u8() as i8;
        #[allow(clippy::cast_sign_loss)]
        let relative_addr = relative_addr_signed as u16;

        let pc = self.pc.wrapping_add(relative_addr);
        self.pc = pc;
        self.empty_cycle();

        #[cfg(feature = "debugging_capability")]
        {
            println!("jr {:#02x}", relative_addr_signed);
        }
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
            self.empty_cycle();
        }

        #[cfg(feature = "debugging_capability")]
        {
            println!("jr {}", Self::get_condition_name(opcode));
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

        #[cfg(feature = "debugging_capability")]
        {
            println!("call {:#06x}", addr);
        }
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
            self.empty_cycle();
            self.empty_cycle();
        }

        #[cfg(feature = "debugging_capability")]
        {
            println!("call {}", Self::get_condition_name(opcode));
        }
    }

    #[inline]
    fn ret(&mut self) {
        self.pc = u16::from(self.cpu_read(self.sp));
        self.sp = self.sp.wrapping_add(1);
        self.pc |= u16::from(self.cpu_read(self.sp)) << 8;
        self.sp = self.sp.wrapping_add(1);
        self.empty_cycle();

        #[cfg(feature = "debugging_capability")]
        {
            println!("ret");
        }
    }

    #[inline]
    fn reti(&mut self) {
        self.ret();
        self.ime = true;

        #[cfg(feature = "debugging_capability")]
        {
            println!("reti");
        }
    }

    #[inline]
    fn ret_cc(&mut self, opcode: u8) {
        self.empty_cycle();

        if self.condition(opcode) {
            self.ret();
        }

        #[cfg(feature = "debugging_capability")]
        {
            println!("ret {}", Self::get_condition_name(opcode));
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
        self.pc = u16::from(opcode) ^ 0xC7;

        #[cfg(feature = "debugging_capability")]
        {
            println!("rst {:#02x}", opcode);
        }
    }

    #[inline]
    fn halt(&mut self) {
        self.catch_up();

        if self.ie & self.ifr & 0x1F == 0 {
            self.cpu_halted = true;
        } else if self.ime {
            self.cpu_halted = false;
            self.pc = self.pc.wrapping_sub(1);
        } else {
            self.cpu_halted = false;
            self.halt_bug = true;
        }

        #[cfg(feature = "debugging_capability")]
        {
            println!("halt");
        }
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

        #[cfg(feature = "debugging_capability")]
        {
            println!("stop");
        }
    }

    #[inline]
    fn di(&mut self) {
        self.ime = false;

        #[cfg(feature = "debugging_capability")]
        {
            println!("di");
        }
    }

    #[inline]
    fn ei(&mut self) {
        self.cpu_ei_delay = true;

        #[cfg(feature = "debugging_capability")]
        {
            println!("ei");
        }
    }

    #[inline]
    fn ccf(&mut self) {
        self.af ^= CF_B;
        self.af &= !(HF_B | NF_B);

        #[cfg(feature = "debugging_capability")]
        {
            println!("ccf");
        }
    }

    #[inline]
    fn scf(&mut self) {
        self.af |= CF_B;
        self.af &= !(HF_B | NF_B);

        #[cfg(feature = "debugging_capability")]
        {
            println!("scf");
        }
    }

    #[inline]
    #[allow(clippy::unused_self)]
    fn nop(&mut self) {
        #[cfg(feature = "debugging_capability")]
        {
            println!("nop");
        }
    }

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

        #[cfg(feature = "debugging_capability")]
        {
            println!("daa");
        }
    }

    #[inline]
    fn cpl(&mut self) {
        self.af ^= 0xFF00;
        self.af |= HF_B | NF_B;

        #[cfg(feature = "debugging_capability")]
        {
            println!("cpl");
        }
    }

    #[inline]
    fn push_rr(&mut self, opcode: u8) {
        let reg_id = ((opcode >> 4) + 1) & 3;
        let val = *self.regid2reg(reg_id);
        self.sp = self.sp.wrapping_sub(1);
        self.cpu_write(self.sp, (val >> 8) as u8);
        self.sp = self.sp.wrapping_sub(1);
        self.cpu_write(self.sp, (val & 0xFF) as u8);

        #[cfg(feature = "debugging_capability")]
        {
            println!("push {}", Self::regid2regname(reg_id));
        }
    }

    #[inline]
    fn pop_rr(&mut self, opcode: u8) {
        let mut val = u16::from(self.cpu_read(self.sp));
        self.sp = self.sp.wrapping_add(1);
        val |= u16::from(self.cpu_read(self.sp)) << 8;
        self.sp = self.sp.wrapping_add(1);
        let reg_id = ((opcode >> 4) + 1) & 3;
        *self.regid2reg(reg_id) = val;
        self.af &= 0xFFF0;

        #[cfg(feature = "debugging_capability")]
        {
            println!("pop {}", Self::regid2regname(reg_id));
        }
    }

    #[inline]
    fn ld_rr_d16(&mut self, opcode: u8) {
        let reg_id = (opcode >> 4) + 1;
        let imm = self.imm_u16();
        *self.regid2reg(reg_id) = imm;

        #[cfg(feature = "debugging_capability")]
        {
            println!("ld {}, {:#06x}", Self::regid2regname(reg_id), imm);
        }
    }

    #[inline]
    fn ld_da16_sp(&mut self) {
        let val = self.sp;
        let addr = self.imm_u16();
        self.cpu_write(addr, (val & 0xFF) as u8);
        self.cpu_write(addr.wrapping_add(1), (val >> 8) as u8);

        #[cfg(feature = "debugging_capability")]
        {
            println!("ld ({:#06x}), SP", addr);
        }
    }

    #[inline]
    fn add_sp_r8(&mut self) {
        let sp = self.sp;
        #[allow(clippy::cast_possible_wrap)]
        let offset_signed = self.imm_u8() as i8;
        #[allow(clippy::cast_sign_loss)]
        let offset = offset_signed as u16;
        self.empty_cycle();
        self.empty_cycle();
        self.sp = self.sp.wrapping_add(offset);
        self.af &= 0xFF00;

        if (sp & 0xF) + (offset & 0xF) > 0xF {
            self.af |= HF_B;
        }

        if (sp & 0xFF) + (offset & 0xFF) > 0xFF {
            self.af |= CF_B;
        }

        #[cfg(feature = "debugging_capability")]
        {
            println!("add SP, {:#04x}", offset_signed);
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

        if (u32::from(hl) + u32::from(rr)) & 0x10000 != 0 {
            self.af |= CF_B;
        }

        self.empty_cycle();

        #[cfg(feature = "debugging_capability")]
        {
            println!("add HL, {}", Self::regid2regname(reg_id));
        }
    }

    #[inline]
    fn rlc_r(&mut self, opcode: u8) {
        let val = self.get_src_val(opcode);
        let carry = val & 0x80 != 0;
        self.af &= 0xFF00;
        self.set_src_val(opcode, val << 1 | u8::from(carry));
        if carry {
            self.af |= CF_B;
        }
        if val == 0 {
            self.af |= ZF_B;
        }

        #[cfg(feature = "debugging_capability")]
        {
            println!("rlc {}", Self::get_src_name(opcode));
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

        #[cfg(feature = "debugging_capability")]
        {
            println!("rra");
        }
    }

    #[inline]
    fn rr_r(&mut self, opcode: u8) {
        let val = self.get_src_val(opcode);
        let carry = (self.af & CF_B) != 0;
        let bit1 = (val & 1) != 0;

        self.af &= 0xFF00;
        let val = val >> 1 | u8::from(carry) << 7;
        self.set_src_val(opcode, val);
        if bit1 {
            self.af |= CF_B;
        }
        if val == 0 {
            self.af |= ZF_B;
        }

        #[cfg(feature = "debugging_capability")]
        {
            println!("rr {}", Self::get_src_name(opcode));
        }
    }

    #[inline]
    fn sla_r(&mut self, opcode: u8) {
        let val = self.get_src_val(opcode);
        let carry = val & 0x80 != 0;
        self.af &= 0xFF00;
        let res = val << 1;
        self.set_src_val(opcode, res);
        if carry {
            self.af |= CF_B;
        }
        if res == 0 {
            self.af |= ZF_B;
        }

        #[cfg(feature = "debugging_capability")]
        {
            println!("sla {}", Self::get_src_name(opcode));
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

        #[cfg(feature = "debugging_capability")]
        {
            println!("sra {}", Self::get_src_name(opcode));
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

        #[cfg(feature = "debugging_capability")]
        {
            println!("srl {}", Self::get_src_name(opcode));
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

        #[cfg(feature = "debugging_capability")]
        {
            println!("swap {}", Self::get_src_name(opcode));
        }
    }

    #[inline]
    fn bit_r(&mut self, opcode: u8) {
        let val = self.get_src_val(opcode);
        let bit_no = (opcode >> 3) & 7;
        let bit = 1 << bit_no;
        if opcode & 0xC0 == 0x40 {
            // bit
            self.af &= 0xFF00 | CF_B;
            self.af |= HF_B;
            if bit & val == 0 {
                self.af |= ZF_B;
            }

            #[cfg(feature = "debugging_capability")]
            {
                println!("bit {}, {}", bit_no, Self::get_src_name(opcode));
            }
        } else if opcode & 0xC0 == 0x80 {
            // res
            self.set_src_val(opcode, val & !bit);

            #[cfg(feature = "debugging_capability")]
            {
                println!("res {}, {}", bit_no, Self::get_src_name(opcode));
            }
        } else {
            // set
            self.set_src_val(opcode, val | bit);

            #[cfg(feature = "debugging_capability")]
            {
                println!("set {}, {}", bit_no, Self::get_src_name(opcode));
            }
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

        #[cfg(feature = "debugging_capability")]
        {
            println!("rla");
        }
    }

    #[inline]
    fn rl_r(&mut self, opcode: u8) {
        let val = self.get_src_val(opcode);
        let carry = self.af & CF_B != 0;
        let bit7 = val & 0x80 != 0;

        self.af &= 0xFF00;
        let val = val << 1 | u8::from(carry);
        self.set_src_val(opcode, val);
        if bit7 {
            self.af |= CF_B;
        }
        if val == 0 {
            self.af |= ZF_B;
        }

        #[cfg(feature = "debugging_capability")]
        {
            println!("rl {}", Self::get_src_name(opcode));
        }
    }

    #[inline]
    fn ill(&mut self) {
        self.ie = 0;
        self.cpu_halted = true;

        #[cfg(feature = "debugging_capability")]
        {
            println!("illegal opcode");
        }
    }

    #[allow(clippy::too_many_lines)]
    #[inline]
    fn exec(&mut self, opcode: u8) {
        match opcode {
            0x00 | 0x7F | 0x49 | 0x52 | 0x5B | 0x64 | 0x6D => self.nop(),
            0x40 => self.ld_b_b(),
            0x78 => self.ld(A, B),
            0x79 => self.ld(A, C),
            0x7A => self.ld(A, D),
            0x7B => self.ld(A, E),
            0x7C => self.ld(A, H),
            0x7D => self.ld(A, L),
            0x47 => self.ld(B, A),
            0x41 => self.ld(B, C),
            0x42 => self.ld(B, D),
            0x43 => self.ld(B, E),
            0x44 => self.ld(B, H),
            0x45 => self.ld(B, L),
            0x4F => self.ld(C, A),
            0x48 => self.ld(C, B),
            0x4A => self.ld(C, D),
            0x4B => self.ld(C, E),
            0x4C => self.ld(C, H),
            0x4D => self.ld(C, L),
            0x57 => self.ld(D, A),
            0x50 => self.ld(D, B),
            0x51 => self.ld(D, C),
            0x53 => self.ld(D, E),
            0x54 => self.ld(D, H),
            0x55 => self.ld(D, L),
            0x5F => self.ld(E, A),
            0x58 => self.ld(E, B),
            0x59 => self.ld(E, C),
            0x5A => self.ld(E, D),
            0x5C => self.ld(E, H),
            0x5D => self.ld(E, L),
            0x67 => self.ld(H, A),
            0x60 => self.ld(H, B),
            0x61 => self.ld(H, C),
            0x62 => self.ld(H, D),
            0x63 => self.ld(H, E),
            0x65 => self.ld(H, L),
            0x6F => self.ld(L, A),
            0x68 => self.ld(L, B),
            0x69 => self.ld(L, C),
            0x6A => self.ld(L, D),
            0x6B => self.ld(L, E),
            0x6C => self.ld(L, H),
            0x7E => self.ld(A, Dhl),
            0x46 => self.ld(B, Dhl),
            0x4E => self.ld(C, Dhl),
            0x56 => self.ld(D, Dhl),
            0x5E => self.ld(E, Dhl),
            0x66 => self.ld(H, Dhl),
            0x6E => self.ld(L, Dhl),
            0x77 => self.ld(Dhl, A),
            0x70 => self.ld(Dhl, B),
            0x71 => self.ld(Dhl, C),
            0x72 => self.ld(Dhl, D),
            0x73 => self.ld(Dhl, E),
            0x74 => self.ld(Dhl, H),
            0x75 => self.ld(Dhl, L),
            0x3E | 0x06 | 0x16 | 0x26 => self.ld_hr_d8(opcode),
            0x0E | 0x1E | 0x2E => self.ld_lr_d8(opcode),
            0x36 => self.ld_dhl_d8(),
            0x0A | 0x1A => self.ld_a_drr(opcode),
            0x02 | 0x12 => self.ld_drr_a(opcode),
            0xFA => self.ld_a_da16(),
            0xEA => self.ld_da16_a(),
            0x3A => self.ld_a_dhld(),
            0x32 => self.ld_dhld_a(),
            0x2A => self.ld_a_dhli(),
            0x22 => self.ld_dhli_a(),
            0xF2 => self.ldh_a_dc(),
            0xE2 => self.ldh_dc_a(),
            0xF0 => self.ldh_a_da8(),
            0xE0 => self.ldh_da8_a(),
            0x87 | 0x80 | 0x81 | 0x82 | 0x83 | 0x84 | 0x85 | 0x86 => self.add_a_r(opcode),
            0xC6 => self.add_a_d8(),
            0x8F | 0x88 | 0x89 | 0x8A | 0x8B | 0x8C | 0x8D | 0x8E => self.adc_a_r(opcode),
            0xCE => self.adc_a_d8(),
            0x97 | 0x90 | 0x91 | 0x92 | 0x93 | 0x94 | 0x95 | 0x96 => self.sub_a_r(opcode),
            0xD6 => self.sub_a_d8(),
            0x9F | 0x98 | 0x99 | 0x9A | 0x9B | 0x9C | 0x9D | 0x9E => self.sbc_a_r(opcode),
            0xDE => self.sbc_a_d8(),
            0xBF | 0xB8 | 0xB9 | 0xBA | 0xBB | 0xBC | 0xBD | 0xBE => self.cp_a_r(opcode),
            0xFE => self.cp_a_d8(),
            0xA7 | 0xA0 | 0xA1 | 0xA2 | 0xA3 | 0xA4 | 0xA5 | 0xA6 => self.and_a_r(opcode),
            0xE6 => self.and_a_d8(),
            0xB7 | 0xB0 | 0xB1 | 0xB2 | 0xB3 | 0xB4 | 0xB5 | 0xB6 => self.or_a_r(opcode),
            0xF6 => self.or_a_d8(),
            0xAF | 0xA8 | 0xA9 | 0xAA | 0xAB | 0xAC | 0xAD | 0xAE => self.xor_a_r(opcode),
            0xEE => self.xor_a_d8(),
            0x3C | 0x04 | 0x14 | 0x24 => self.inc_hr(opcode),
            0x3D | 0x05 | 0x15 | 0x25 => self.dec_hr(opcode),
            0x0C | 0x1C | 0x2C => self.inc_lr(opcode),
            0x0D | 0x1D | 0x2D => self.dec_lr(opcode),
            0x34 => self.inc_dhl(),
            0x35 => self.dec_dhl(),
            0x07 => self.rlca(),
            0x17 => self.rla(),
            0x0F => self.rrca(),
            0x1F => self.rra(),
            0xC3 => self.jp_a16(),
            0xE9 => self.jp_hl(),
            0x18 => self.jr_d(),
            0xCD => self.call_nn(),
            0xC9 => self.ret(),
            0xD9 => self.reti(),
            0xC2 | 0xCA | 0xD2 | 0xDA => self.jp_cc(opcode),
            0x20 | 0x28 | 0x30 | 0x38 => self.jr_cc(opcode),
            0xC4 | 0xCC | 0xD4 | 0xDC => self.call_cc_a16(opcode),
            0xC0 | 0xC8 | 0xD0 | 0xD8 => self.ret_cc(opcode),
            0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => self.rst(opcode),
            0x76 => self.halt(),
            0x10 => self.stop(),
            0xF3 => self.di(),
            0xFB => self.ei(),
            0x3F => self.ccf(),
            0x37 => self.scf(),
            0x27 => self.daa(),
            0x2F => self.cpl(),
            0x01 | 0x11 | 0x21 | 0x31 => self.ld_rr_d16(opcode),
            0x08 => self.ld_da16_sp(),
            0xF9 => self.ld16_sp_hl(),
            0xF8 => self.ld_hl_sp_r8(),
            0xC5 | 0xD5 | 0xE5 | 0xF5 => self.push_rr(opcode),
            0xC1 | 0xD1 | 0xE1 | 0xF1 => self.pop_rr(opcode),
            0x09 | 0x19 | 0x29 | 0x39 => self.add_hl_rr(opcode),
            0xE8 => self.add_sp_r8(),
            0x03 | 0x13 | 0x23 | 0x33 => self.inc_rr(opcode),
            0x0B | 0x1B | 0x2B | 0x3B => self.dec_rr(opcode),
            0xCB => self.exec_cb(),
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
