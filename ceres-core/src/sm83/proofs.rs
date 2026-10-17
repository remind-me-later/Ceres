//! Kani proofs of the SM83 instructions (`cargo kani -p ceres-core`).
//!
//! Each proof runs an instruction from any register state, on a bus whose
//! reads return any bytes, and compares the result with the opcode tables
//! written as plain arithmetic.

use super::{Bus, Sm83};

/// A bus whose reads return any bytes, in order, and which remembers the
/// addresses read and the last write.
struct AnyBus {
    reads: [u8; 3],
    read_addrs: [u16; 3],
    read_count: usize,
    write: Option<(u16, u8)>,
}

impl AnyBus {
    fn new() -> Self {
        Self {
            reads: kani::any(),
            read_addrs: [0; 3],
            read_count: 0,
            write: None,
        }
    }
}

impl Bus for AnyBus {
    fn tick(&mut self) {}

    fn read(&mut self, addr: u16) -> u8 {
        let i = self.read_count;
        self.read_count += 1;
        if let Some(slot) = self.read_addrs.get_mut(i) {
            *slot = addr;
        }
        self.reads.get(i).copied().unwrap_or_else(kani::any)
    }

    fn write(&mut self, addr: u16, val: u8) {
        self.write = Some((addr, val));
    }

    fn defer(&mut self, _: i32) {}
    fn interrupts_pending(&self) -> bool {
        kani::any()
    }
    fn read_if(&self) -> u8 {
        kani::any()
    }
    fn read_ie(&self) -> u8 {
        kani::any()
    }
    fn ack_interrupt(&mut self, _: u8) {}
    fn clear_ie(&mut self) {}
    fn tick_hdma(&mut self) {}
    fn tick_oam_bug(&mut self, _: u16) {}
    fn trigger_oam_bug(&mut self, _: u16) {}
    fn dma_run(&mut self, _: bool) {}
    fn dma_finish_before_halt(&mut self) {}
    fn drop_deferred(&mut self) {}
    fn set_halted(&mut self, _: bool) {}
    fn advance(&mut self, _: i32) {}
    fn is_cgb_hardware(&self) -> bool {
        kani::any()
    }
    fn wake_from_stop(&mut self) {}
    fn enter_stop(&mut self, _: bool) {}
    fn flush(&mut self) {}
    fn peek(&self, _: u16) -> u8 {
        kani::any()
    }
    fn is_stopped(&self) -> bool {
        kani::any()
    }
    fn speed_switch_requested(&self) -> bool {
        kani::any()
    }
    fn begin_speed_switch(&mut self, _: bool) {}
    fn leave_stop(&mut self) {}
    fn clear_speed_switch_halt(&mut self) {}
    fn take_unhalt(&mut self) -> bool {
        kani::any()
    }
    fn hdma_request_pending(&self) -> bool {
        kani::any()
    }
    fn note_halt_prefetch(&mut self) {}
}

/// Any CPU state where the low nibble of F is zero.
fn any_cpu() -> Sm83 {
    Sm83 {
        af: kani::any::<u16>() & 0xFFF0,
        bc: kani::any(),
        de: kani::any(),
        hl: kani::any(),
        sp: kani::any(),
        pc: kani::any(),
        ..Sm83::default()
    }
}

/// The operand at the opcode tables' index: B, C, D, E, H, L, (HL) and A.
const HL_BYTE: usize = 6;
const A: usize = 7;

/// The registers as the opcode tables see them, with the byte at HL.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Regs {
    r: [u8; 8],
    z: bool,
    n: bool,
    h: bool,
    c: bool,
    sp: u16,
    pc: u16,
}

fn regs(cpu: &Sm83, hl_byte: u8) -> Regs {
    let [b, c] = cpu.bc.to_be_bytes();
    let [d, e] = cpu.de.to_be_bytes();
    let [h, l] = cpu.hl.to_be_bytes();
    let [a, f] = cpu.af.to_be_bytes();
    assert!(f & 0x0F == 0, "the low nibble of F is zero");
    Regs {
        r: [b, c, d, e, h, l, hl_byte, a],
        z: f & 0x80 != 0,
        n: f & 0x40 != 0,
        h: f & 0x20 != 0,
        c: f & 0x10 != 0,
        sp: cpu.sp,
        pc: cpu.pc,
    }
}

/// The registers after an instruction: the byte at HL is the one written, if
/// any (always to the address HL held before).
fn regs_after(cpu: &Sm83, bus: &AnyBus, before_hl: u16, hl_byte: u8) -> Regs {
    let hl_byte = bus.write.map_or(hl_byte, |(addr, val)| {
        assert!(addr == before_hl, "only (HL) is written");
        val
    });
    regs(cpu, hl_byte)
}

const fn hl(r: &Regs) -> u16 {
    u16::from_be_bytes([r.r[4], r.r[5]])
}

/// ADD, ADC, SUB, SBC, AND, XOR, OR and CP (`kind` 0 to 7) of A and `v`.
fn alu_model(r: &mut Regs, kind: u8, v: u8) {
    let a = i32::from(r.r[A]);
    let v = i32::from(v);
    let carry = i32::from(r.c && (kind == 1 || kind == 3));
    let res = match kind {
        0 | 1 => {
            let sum = a + v + carry;
            r.n = false;
            r.h = a % 16 + v % 16 + carry > 15;
            r.c = sum > 255;
            sum % 256
        }
        2 | 3 | 7 => {
            let diff = a - v - carry;
            r.n = true;
            r.h = a % 16 - v % 16 - carry < 0;
            r.c = diff < 0;
            diff.rem_euclid(256)
        }
        _ => {
            r.n = false;
            r.h = kind == 4;
            r.c = false;
            match kind {
                4 => a & v,
                5 => a ^ v,
                _ => a | v,
            }
        }
    };
    r.z = res == 0;
    if kind != 7 {
        r.r[A] = res as u8;
    }
}

/// ALU A,r (0x80-0xBF).
#[kani::proof]
fn alu_a_r() {
    let mut cpu = any_cpu();
    let mut bus = AnyBus::new();
    let op: u8 = kani::any();
    kani::assume((0x80..=0xBF).contains(&op));
    let before = regs(&cpu, bus.reads[0]);
    let index = usize::from(op & 7);

    cpu.exec(&mut bus, op);

    let mut expected = before;
    alu_model(&mut expected, (op >> 3) & 7, before.r[index]);
    assert!(
        regs_after(&cpu, &bus, hl(&before), before.r[HL_BYTE]) == expected,
        "result"
    );
    if index == HL_BYTE {
        assert!(
            bus.read_count == 1 && bus.read_addrs[0] == hl(&before),
            "reads (HL)"
        );
    } else {
        assert!(bus.read_count == 0, "no read");
    }
}

/// ALU A,d8 (0xC6, 0xCE ... 0xFE).
#[kani::proof]
fn alu_a_d8() {
    let mut cpu = any_cpu();
    let mut bus = AnyBus::new();
    let kind: u8 = kani::any();
    kani::assume(kind < 8);
    let op = 0xC6 | (kind << 3);
    let before = regs(&cpu, 0);

    cpu.exec(&mut bus, op);

    let mut expected = before;
    alu_model(&mut expected, kind, bus.reads[0]);
    expected.pc = before.pc.wrapping_add(1);
    assert!(regs_after(&cpu, &bus, hl(&before), 0) == expected, "result");
    assert!(
        bus.read_count == 1 && bus.read_addrs[0] == before.pc,
        "reads the immediate"
    );
}

/// The CB-prefixed rotates, shifts, SWAP, BIT, RES and SET.
#[kani::proof]
fn cb() {
    let mut cpu = any_cpu();
    let mut bus = AnyBus::new();
    let op = bus.reads[0];
    let index = usize::from(op & 7);
    let bit = (op >> 3) & 7;
    let before = regs(&cpu, bus.reads[1]);

    cpu.exec(&mut bus, 0xCB);

    let v = before.r[index];
    let mut expected = before;
    expected.pc = before.pc.wrapping_add(1);
    match op >> 6 {
        0 => {
            let carry_in = u8::from(before.c);
            let (res, carry) = match bit {
                0 => (v << 1 | v >> 7, v >> 7),
                1 => (v >> 1 | v << 7, v & 1),
                2 => (v << 1 | carry_in, v >> 7),
                3 => (v >> 1 | carry_in << 7, v & 1),
                4 => (v << 1, v >> 7),
                5 => (v >> 1 | (v & 0x80), v & 1),
                6 => (v << 4 | v >> 4, 0),
                _ => (v >> 1, v & 1),
            };
            expected.r[index] = res;
            expected.z = res == 0;
            expected.n = false;
            expected.h = false;
            expected.c = carry != 0;
        }
        1 => {
            expected.z = v >> bit & 1 == 0;
            expected.n = false;
            expected.h = true;
        }
        2 => expected.r[index] = v & !(1 << bit),
        _ => expected.r[index] = v | 1 << bit,
    }
    assert!(
        regs_after(&cpu, &bus, hl(&before), before.r[HL_BYTE]) == expected,
        "result"
    );
    assert!(bus.read_addrs[0] == before.pc, "reads the opcode");
    if index == HL_BYTE {
        assert!(
            bus.read_count == 2 && bus.read_addrs[1] == hl(&before),
            "reads (HL)"
        );
    }
}

/// INC r and DEC r (0x04, 0x05 ... 0x3C, 0x3D).
#[kani::proof]
fn inc_dec_r() {
    let mut cpu = any_cpu();
    let mut bus = AnyBus::new();
    let op: u8 = kani::any();
    kani::assume(op & 0xC6 == 0x04);
    let index = usize::from((op >> 3) & 7);
    let before = regs(&cpu, bus.reads[0]);

    cpu.exec(&mut bus, op);

    let v = before.r[index];
    let mut expected = before;
    if op & 1 == 0 {
        expected.r[index] = v.wrapping_add(1);
        expected.n = false;
        expected.h = v % 16 == 15;
    } else {
        expected.r[index] = v.wrapping_sub(1);
        expected.n = true;
        expected.h = v % 16 == 0;
    }
    expected.z = expected.r[index] == 0;
    assert!(
        regs_after(&cpu, &bus, hl(&before), before.r[HL_BYTE]) == expected,
        "result"
    );
}

/// LD r,r' (0x40-0x7F but HALT).
#[kani::proof]
fn ld_r_r() {
    let mut cpu = any_cpu();
    let mut bus = AnyBus::new();
    let op: u8 = kani::any();
    kani::assume((0x40..=0x7F).contains(&op) && op != 0x76);
    let before = regs(&cpu, bus.reads[0]);

    cpu.exec(&mut bus, op);

    let mut expected = before;
    expected.r[usize::from((op >> 3) & 7)] = before.r[usize::from(op & 7)];
    assert!(
        regs_after(&cpu, &bus, hl(&before), before.r[HL_BYTE]) == expected,
        "result"
    );
}

/// LD r,d8 (0x06, 0x0E ... 0x3E).
#[kani::proof]
fn ld_r_d8() {
    let mut cpu = any_cpu();
    let mut bus = AnyBus::new();
    let r: u8 = kani::any();
    kani::assume(r < 8);
    let before = regs(&cpu, 0);

    cpu.exec(&mut bus, 0x06 | r << 3);

    let mut expected = before;
    expected.r[usize::from(r)] = bus.reads[0];
    expected.pc = before.pc.wrapping_add(1);
    assert!(regs_after(&cpu, &bus, hl(&before), 0) == expected, "result");
    assert!(
        bus.read_count == 1 && bus.read_addrs[0] == before.pc,
        "reads the immediate"
    );
}

/// ADD HL,rr (0x09, 0x19, 0x29, 0x39).
#[kani::proof]
fn add_hl_rr() {
    let mut cpu = any_cpu();
    let mut bus = AnyBus::new();
    let pair: u8 = kani::any();
    kani::assume(pair < 4);
    let before = regs(&cpu, 0);
    let hl = u32::from(cpu.hl);
    let rr = u32::from([cpu.bc, cpu.de, cpu.hl, cpu.sp][usize::from(pair)]);

    cpu.exec(&mut bus, 0x09 | pair << 4);

    let mut expected = before;
    let sum = hl + rr;
    [expected.r[4], expected.r[5]] = ((sum % 0x1_0000) as u16).to_be_bytes();
    expected.n = false;
    expected.h = hl % 0x1000 + rr % 0x1000 > 0xFFF;
    expected.c = sum > 0xFFFF;
    assert!(regs_after(&cpu, &bus, 0, 0) == expected, "result");
}

/// ADD SP,e (0xE8) and LD HL,SP+e (0xF8).
#[kani::proof]
fn sp_plus_e() {
    let mut cpu = any_cpu();
    let mut bus = AnyBus::new();
    let ld_hl: bool = kani::any();
    let before = regs(&cpu, 0);
    let e = bus.reads[0];

    cpu.exec(&mut bus, if ld_hl { 0xF8 } else { 0xE8 });

    let sp = i32::from(before.sp);
    let res = (sp + i32::from(e.cast_signed())).rem_euclid(0x1_0000) as u16;
    let mut expected = before;
    if ld_hl {
        [expected.r[4], expected.r[5]] = res.to_be_bytes();
    } else {
        expected.sp = res;
    }
    expected.pc = before.pc.wrapping_add(1);
    expected.z = false;
    expected.n = false;
    expected.h = sp % 16 + i32::from(e) % 16 > 15;
    expected.c = sp % 256 + i32::from(e) > 255;
    assert!(regs_after(&cpu, &bus, 0, 0) == expected, "result");
}

/// The decimal value of a BCD byte.
const fn from_bcd(v: u8) -> i32 {
    (v >> 4) as i32 * 10 + (v & 0xF) as i32
}

const fn is_bcd(v: u8) -> bool {
    v >> 4 <= 9 && v & 0xF <= 9
}

/// ADC A,d8 or SBC A,d8 then DAA, on BCD numbers, give the BCD sum or
/// difference, with the decimal carry.
#[kani::proof]
fn daa_after_bcd_arithmetic() {
    let mut cpu = any_cpu();
    let mut bus = AnyBus::new();
    let subtract: bool = kani::any();
    let a = cpu.a();
    let v = bus.reads[0];
    let carry = i32::from(cpu.af & 0x10 != 0);
    kani::assume(is_bcd(a) && is_bcd(v));

    cpu.exec(&mut bus, if subtract { 0xDE } else { 0xCE });
    cpu.exec(&mut bus, 0x27);

    let res = if subtract {
        from_bcd(a) - from_bcd(v) - carry
    } else {
        from_bcd(a) + from_bcd(v) + carry
    };
    let r = regs(&cpu, 0);
    assert!(is_bcd(r.r[A]), "BCD result");
    assert!(from_bcd(r.r[A]) == res.rem_euclid(100), "decimal result");
    assert!(r.c == !(0..100).contains(&res), "decimal carry");
    assert!(r.z == (r.r[A] == 0), "zero");
    assert!(r.n == subtract && !r.h, "N kept, H cleared");
}

/// No opcode panics, and the low nibble of F stays zero, whatever the state.
#[kani::proof]
fn any_opcode_keeps_f_low_nibble_zero() {
    let mut cpu = Sm83 {
        ime: kani::any(),
        ime_toggle: kani::any(),
        halt_bug: kani::any(),
        is_halted: kani::any(),
        ..any_cpu()
    };
    let mut bus = AnyBus::new();

    cpu.exec(&mut bus, kani::any());

    assert!(cpu.af & 0x0F == 0, "the low nibble of F is zero");
}

/// `set_af` (the post-boot registers) clears the low nibble of F too.
#[kani::proof]
fn set_af_clears_f_low_nibble() {
    let mut cpu = Sm83::default();
    cpu.set_af(kani::any());
    assert!(cpu.af & 0x0F == 0, "the low nibble of F is zero");
}
