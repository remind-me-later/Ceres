use super::{
    operands::{
        Immediate, Indirect, IndirectDecreaseHL, IndirectIncreaseHL,
        JumpCondition::{Carry, NotCarry, NotZero, Zero},
    },
    registers::{
        Register16::{AF, BC, DE, HL, SP},
        Register8::{A, B, C, D, E, H, L},
    },
    Cpu,
};
use crate::AudioCallbacks;

impl<'a, AR: AudioCallbacks> Cpu<AR> {
    #[allow(clippy::too_many_lines)]
    pub fn execute(&mut self, opcode: u8) {
        match opcode {
            0x7f => self.ld(A, A),
            0x78 => self.ld(A, B),
            0x79 => self.ld(A, C),
            0x7a => self.ld(A, D),
            0x7b => self.ld(A, E),
            0x7c => self.ld(A, H),
            0x7d => self.ld(A, L),
            0x3e => self.ld(A, Immediate),
            0x7e => self.ld(A, Indirect::HL),
            0x47 => self.ld(B, A),
            0x40 => self.ld(B, B),
            0x41 => self.ld(B, C),
            0x42 => self.ld(B, D),
            0x43 => self.ld(B, E),
            0x44 => self.ld(B, H),
            0x45 => self.ld(B, L),
            0x06 => self.ld(B, Immediate),
            0x46 => self.ld(B, Indirect::HL),
            0x4f => self.ld(C, A),
            0x48 => self.ld(C, B),
            0x49 => self.ld(C, C),
            0x4a => self.ld(C, D),
            0x4b => self.ld(C, E),
            0x4c => self.ld(C, H),
            0x4d => self.ld(C, L),
            0x0e => self.ld(C, Immediate),
            0x4e => self.ld(C, Indirect::HL),
            0x57 => self.ld(D, A),
            0x50 => self.ld(D, B),
            0x51 => self.ld(D, C),
            0x52 => self.ld(D, D),
            0x53 => self.ld(D, E),
            0x54 => self.ld(D, H),
            0x55 => self.ld(D, L),
            0x16 => self.ld(D, Immediate),
            0x56 => self.ld(D, Indirect::HL),
            0x5f => self.ld(E, A),
            0x58 => self.ld(E, B),
            0x59 => self.ld(E, C),
            0x5a => self.ld(E, D),
            0x5b => self.ld(E, E),
            0x5c => self.ld(E, H),
            0x5d => self.ld(E, L),
            0x1e => self.ld(E, Immediate),
            0x5e => self.ld(E, Indirect::HL),
            0x67 => self.ld(H, A),
            0x60 => self.ld(H, B),
            0x61 => self.ld(H, C),
            0x62 => self.ld(H, D),
            0x63 => self.ld(H, E),
            0x64 => self.ld(H, H),
            0x65 => self.ld(H, L),
            0x26 => self.ld(H, Immediate),
            0x66 => self.ld(H, Indirect::HL),
            0x6f => self.ld(L, A),
            0x68 => self.ld(L, B),
            0x69 => self.ld(L, C),
            0x6a => self.ld(L, D),
            0x6b => self.ld(L, E),
            0x6c => self.ld(L, H),
            0x6d => self.ld(L, L),
            0x2e => self.ld(L, Immediate),
            0x6e => self.ld(L, Indirect::HL),
            0x77 => self.ld(Indirect::HL, A),
            0x70 => self.ld(Indirect::HL, B),
            0x71 => self.ld(Indirect::HL, C),
            0x72 => self.ld(Indirect::HL, D),
            0x73 => self.ld(Indirect::HL, E),
            0x74 => self.ld(Indirect::HL, H),
            0x75 => self.ld(Indirect::HL, L),
            0x36 => self.ld(Indirect::HL, Immediate),
            0x0a => self.ld(A, Indirect::BC),
            0x02 => self.ld(Indirect::BC, A),
            0x1a => self.ld(A, Indirect::DE),
            0x12 => self.ld(Indirect::DE, A),
            0xfa => self.ld(A, Indirect::Immediate),
            0xea => self.ld(Indirect::Immediate, A),
            0x3a => self.ld(A, IndirectDecreaseHL),
            0x32 => self.ld(IndirectDecreaseHL, A),
            0x2a => self.ld(A, IndirectIncreaseHL),
            0x22 => self.ld(IndirectIncreaseHL, A),
            0xf2 => self.ld(A, Indirect::HighC),
            0xe2 => self.ld(Indirect::HighC, A),
            0xf0 => self.ld(A, Indirect::HighImmediate),
            0xe0 => self.ld(Indirect::HighImmediate, A),

            // ********************
            // * 8-bit arithmetic *
            // ********************
            0x87 => self.add(A),
            0x80 => self.add(B),
            0x81 => self.add(C),
            0x82 => self.add(D),
            0x83 => self.add(E),
            0x84 => self.add(H),
            0x85 => self.add(L),
            0x86 => self.add(Indirect::HL),
            0xc6 => self.add(Immediate),
            0x8f => self.adc(A),
            0x88 => self.adc(B),
            0x89 => self.adc(C),
            0x8a => self.adc(D),
            0x8b => self.adc(E),
            0x8c => self.adc(H),
            0x8d => self.adc(L),
            0x8e => self.adc(Indirect::HL),
            0xce => self.adc(Immediate),
            0x97 => self.sub(A),
            0x90 => self.sub(B),
            0x91 => self.sub(C),
            0x92 => self.sub(D),
            0x93 => self.sub(E),
            0x94 => self.sub(H),
            0x95 => self.sub(L),
            0x96 => self.sub(Indirect::HL),
            0xd6 => self.sub(Immediate),
            0x9f => self.sbc(A),
            0x98 => self.sbc(B),
            0x99 => self.sbc(C),
            0x9a => self.sbc(D),
            0x9b => self.sbc(E),
            0x9c => self.sbc(H),
            0x9d => self.sbc(L),
            0x9e => self.sbc(Indirect::HL),
            0xde => self.sbc(Immediate),
            0xbf => self.cp(A),
            0xb8 => self.cp(B),
            0xb9 => self.cp(C),
            0xba => self.cp(D),
            0xbb => self.cp(E),
            0xbc => self.cp(H),
            0xbd => self.cp(L),
            0xbe => self.cp(Indirect::HL),
            0xfe => self.cp(Immediate),
            0xa7 => self.and(A),
            0xa0 => self.and(B),
            0xa1 => self.and(C),
            0xa2 => self.and(D),
            0xa3 => self.and(E),
            0xa4 => self.and(H),
            0xa5 => self.and(L),
            0xa6 => self.and(Indirect::HL),
            0xe6 => self.and(Immediate),
            0xb7 => self.or(A),
            0xb0 => self.or(B),
            0xb1 => self.or(C),
            0xb2 => self.or(D),
            0xb3 => self.or(E),
            0xb4 => self.or(H),
            0xb5 => self.or(L),
            0xb6 => self.or(Indirect::HL),
            0xf6 => self.or(Immediate),
            0xaf => self.xor(A),
            0xa8 => self.xor(B),
            0xa9 => self.xor(C),
            0xaa => self.xor(D),
            0xab => self.xor(E),
            0xac => self.xor(H),
            0xad => self.xor(L),
            0xae => self.xor(Indirect::HL),
            0xee => self.xor(Immediate),
            0x3c => self.inc(A),
            0x04 => self.inc(B),
            0x0c => self.inc(C),
            0x14 => self.inc(D),
            0x1c => self.inc(E),
            0x24 => self.inc(H),
            0x2c => self.inc(L),
            0x34 => self.inc(Indirect::HL),
            0x3d => self.dec(A),
            0x05 => self.dec(B),
            0x0d => self.dec(C),
            0x15 => self.dec(D),
            0x1d => self.dec(E),
            0x25 => self.dec(H),
            0x2d => self.dec(L),
            0x35 => self.dec(Indirect::HL),
            0x07 => self.rlca(),
            0x17 => self.rla(),
            0x0f => self.rrca(),
            0x1f => self.rra(),

            // ***********
            // * Control *
            // ***********
            0xc3 => self.jp_nn(),
            0xe9 => self.jp_hl(),
            0x18 => self.jr(),
            0xcd => self.call(),
            0xc9 => self.ret(),
            0xd9 => self.reti(),
            0xc2 => self.jp_f(NotZero),
            0xca => self.jp_f(Zero),
            0xd2 => self.jp_f(NotCarry),
            0xda => self.jp_f(Carry),
            0x20 => self.jr_f(NotZero),
            0x28 => self.jr_f(Zero),
            0x30 => self.jr_f(NotCarry),
            0x38 => self.jr_f(Carry),
            0xc4 => self.call_f(NotZero),
            0xcc => self.call_f(Zero),
            0xd4 => self.call_f(NotCarry),
            0xdc => self.call_f(Carry),
            0xc0 => self.ret_f(NotZero),
            0xc8 => self.ret_f(Zero),
            0xd0 => self.ret_f(NotCarry),
            0xd8 => self.ret_f(Carry),
            0xc7 => self.rst(0x00),
            0xcf => self.rst(0x08),
            0xd7 => self.rst(0x10),
            0xdf => self.rst(0x18),
            0xe7 => self.rst(0x20),
            0xef => self.rst(0x28),
            0xf7 => self.rst(0x30),
            0xff => self.rst(0x38),
            0x76 => self.halt(),
            0x10 => self.stop(),
            0xf3 => self.di(),
            0xfb => self.ei(),
            0x3f => self.ccf(),
            0x37 => self.scf(),
            0x00 => self.nop(),
            0x27 => self.daa(),
            0x2f => self.cpl(),

            // **********
            // * 16-bit *
            // **********
            // load
            0x01 => self.ld16(BC),
            0x11 => self.ld16(DE),
            0x21 => self.ld16(HL),
            0x31 => self.ld16(SP),
            0x08 => self.ld16_nn_sp(),
            0xf9 => self.ld16_sp_hl(),
            0xf8 => self.ld16_hl_sp_dd(),
            0xc5 => self.push(BC),
            0xd5 => self.push(DE),
            0xe5 => self.push(HL),
            0xf5 => self.push(AF),
            0xc1 => self.pop(BC),
            0xd1 => self.pop(DE),
            0xe1 => self.pop(HL),
            0xf1 => self.pop(AF),
            // arithmetic
            0x09 => self.add_hl(BC),
            0x19 => self.add_hl(DE),
            0x29 => self.add_hl(HL),
            0x39 => self.add_hl(SP),
            0xe8 => self.add16_sp_dd(),
            0x03 => self.inc16(BC),
            0x13 => self.inc16(DE),
            0x23 => self.inc16(HL),
            0x33 => self.inc16(SP),
            0x0b => self.dec16(BC),
            0x1b => self.dec16(DE),
            0x2b => self.dec16(HL),
            0x3b => self.dec16(SP),
            // cb operations
            0xcb => {
                let opcode = self.read_immediate();
                self.execute_cb(opcode);
            }
            0xd3 | 0xdb | 0xdd | 0xe3 | 0xe4 | 0xeb | 0xec | 0xed | 0xf4 | 0xfc | 0xfd => {
                panic!("Illegal opcode {}", opcode)
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn execute_cb(&mut self, opcode: u8) {
        match opcode {
            0x07 => self.rlc(A),
            0x00 => self.rlc(B),
            0x01 => self.rlc(C),
            0x02 => self.rlc(D),
            0x03 => self.rlc(E),
            0x04 => self.rlc(H),
            0x05 => self.rlc(L),
            0x06 => self.rlc(Indirect::HL),
            0x17 => self.rl(A),
            0x10 => self.rl(B),
            0x11 => self.rl(C),
            0x12 => self.rl(D),
            0x13 => self.rl(E),
            0x14 => self.rl(H),
            0x15 => self.rl(L),
            0x16 => self.rl(Indirect::HL),
            0x0f => self.rrc(A),
            0x08 => self.rrc(B),
            0x09 => self.rrc(C),
            0x0a => self.rrc(D),
            0x0b => self.rrc(E),
            0x0c => self.rrc(H),
            0x0d => self.rrc(L),
            0x0e => self.rrc(Indirect::HL),
            0x1f => self.rr(A),
            0x18 => self.rr(B),
            0x19 => self.rr(C),
            0x1a => self.rr(D),
            0x1b => self.rr(E),
            0x1c => self.rr(H),
            0x1d => self.rr(L),
            0x1e => self.rr(Indirect::HL),
            0x27 => self.sla(A),
            0x20 => self.sla(B),
            0x21 => self.sla(C),
            0x22 => self.sla(D),
            0x23 => self.sla(E),
            0x24 => self.sla(H),
            0x25 => self.sla(L),
            0x26 => self.sla(Indirect::HL),
            0x2f => self.sra(A),
            0x28 => self.sra(B),
            0x29 => self.sra(C),
            0x2a => self.sra(D),
            0x2b => self.sra(E),
            0x2c => self.sra(H),
            0x2d => self.sra(L),
            0x2e => self.sra(Indirect::HL),
            0x3f => self.srl(A),
            0x38 => self.srl(B),
            0x39 => self.srl(C),
            0x3a => self.srl(D),
            0x3b => self.srl(E),
            0x3c => self.srl(H),
            0x3d => self.srl(L),
            0x3e => self.srl(Indirect::HL),
            0x37 => self.swap(A),
            0x30 => self.swap(B),
            0x31 => self.swap(C),
            0x32 => self.swap(D),
            0x33 => self.swap(E),
            0x34 => self.swap(H),
            0x35 => self.swap(L),
            0x36 => self.swap(Indirect::HL),
            0x47 => self.bit(A, 0),
            0x4f => self.bit(A, 1),
            0x57 => self.bit(A, 2),
            0x5f => self.bit(A, 3),
            0x67 => self.bit(A, 4),
            0x6f => self.bit(A, 5),
            0x77 => self.bit(A, 6),
            0x7f => self.bit(A, 7),
            0x40 => self.bit(B, 0),
            0x48 => self.bit(B, 1),
            0x50 => self.bit(B, 2),
            0x58 => self.bit(B, 3),
            0x60 => self.bit(B, 4),
            0x68 => self.bit(B, 5),
            0x70 => self.bit(B, 6),
            0x78 => self.bit(B, 7),
            0x41 => self.bit(C, 0),
            0x49 => self.bit(C, 1),
            0x51 => self.bit(C, 2),
            0x59 => self.bit(C, 3),
            0x61 => self.bit(C, 4),
            0x69 => self.bit(C, 5),
            0x71 => self.bit(C, 6),
            0x79 => self.bit(C, 7),
            0x42 => self.bit(D, 0),
            0x4a => self.bit(D, 1),
            0x52 => self.bit(D, 2),
            0x5a => self.bit(D, 3),
            0x62 => self.bit(D, 4),
            0x6a => self.bit(D, 5),
            0x72 => self.bit(D, 6),
            0x7a => self.bit(D, 7),
            0x43 => self.bit(E, 0),
            0x4b => self.bit(E, 1),
            0x53 => self.bit(E, 2),
            0x5b => self.bit(E, 3),
            0x63 => self.bit(E, 4),
            0x6b => self.bit(E, 5),
            0x73 => self.bit(E, 6),
            0x7b => self.bit(E, 7),
            0x44 => self.bit(H, 0),
            0x4c => self.bit(H, 1),
            0x54 => self.bit(H, 2),
            0x5c => self.bit(H, 3),
            0x64 => self.bit(H, 4),
            0x6c => self.bit(H, 5),
            0x74 => self.bit(H, 6),
            0x7c => self.bit(H, 7),
            0x45 => self.bit(L, 0),
            0x4d => self.bit(L, 1),
            0x55 => self.bit(L, 2),
            0x5d => self.bit(L, 3),
            0x65 => self.bit(L, 4),
            0x6d => self.bit(L, 5),
            0x75 => self.bit(L, 6),
            0x7d => self.bit(L, 7),
            0x46 => self.bit(Indirect::HL, 0),
            0x4e => self.bit(Indirect::HL, 1),
            0x56 => self.bit(Indirect::HL, 2),
            0x5e => self.bit(Indirect::HL, 3),
            0x66 => self.bit(Indirect::HL, 4),
            0x6e => self.bit(Indirect::HL, 5),
            0x76 => self.bit(Indirect::HL, 6),
            0x7e => self.bit(Indirect::HL, 7),
            0xc7 => self.set(A, 0),
            0xcf => self.set(A, 1),
            0xd7 => self.set(A, 2),
            0xdf => self.set(A, 3),
            0xe7 => self.set(A, 4),
            0xef => self.set(A, 5),
            0xf7 => self.set(A, 6),
            0xff => self.set(A, 7),
            0xc0 => self.set(B, 0),
            0xc8 => self.set(B, 1),
            0xd0 => self.set(B, 2),
            0xd8 => self.set(B, 3),
            0xe0 => self.set(B, 4),
            0xe8 => self.set(B, 5),
            0xf0 => self.set(B, 6),
            0xf8 => self.set(B, 7),
            0xc1 => self.set(C, 0),
            0xc9 => self.set(C, 1),
            0xd1 => self.set(C, 2),
            0xd9 => self.set(C, 3),
            0xe1 => self.set(C, 4),
            0xe9 => self.set(C, 5),
            0xf1 => self.set(C, 6),
            0xf9 => self.set(C, 7),
            0xc2 => self.set(D, 0),
            0xca => self.set(D, 1),
            0xd2 => self.set(D, 2),
            0xda => self.set(D, 3),
            0xe2 => self.set(D, 4),
            0xea => self.set(D, 5),
            0xf2 => self.set(D, 6),
            0xfa => self.set(D, 7),
            0xc3 => self.set(E, 0),
            0xcb => self.set(E, 1),
            0xd3 => self.set(E, 2),
            0xdb => self.set(E, 3),
            0xe3 => self.set(E, 4),
            0xeb => self.set(E, 5),
            0xf3 => self.set(E, 6),
            0xfb => self.set(E, 7),
            0xc4 => self.set(H, 0),
            0xcc => self.set(H, 1),
            0xd4 => self.set(H, 2),
            0xdc => self.set(H, 3),
            0xe4 => self.set(H, 4),
            0xec => self.set(H, 5),
            0xf4 => self.set(H, 6),
            0xfc => self.set(H, 7),
            0xc5 => self.set(L, 0),
            0xcd => self.set(L, 1),
            0xd5 => self.set(L, 2),
            0xdd => self.set(L, 3),
            0xe5 => self.set(L, 4),
            0xed => self.set(L, 5),
            0xf5 => self.set(L, 6),
            0xfd => self.set(L, 7),
            0xc6 => self.set(Indirect::HL, 0),
            0xce => self.set(Indirect::HL, 1),
            0xd6 => self.set(Indirect::HL, 2),
            0xde => self.set(Indirect::HL, 3),
            0xe6 => self.set(Indirect::HL, 4),
            0xee => self.set(Indirect::HL, 5),
            0xf6 => self.set(Indirect::HL, 6),
            0xfe => self.set(Indirect::HL, 7),
            0x87 => self.res(A, 0),
            0x8f => self.res(A, 1),
            0x97 => self.res(A, 2),
            0x9f => self.res(A, 3),
            0xa7 => self.res(A, 4),
            0xaf => self.res(A, 5),
            0xb7 => self.res(A, 6),
            0xbf => self.res(A, 7),
            0x80 => self.res(B, 0),
            0x88 => self.res(B, 1),
            0x90 => self.res(B, 2),
            0x98 => self.res(B, 3),
            0xa0 => self.res(B, 4),
            0xa8 => self.res(B, 5),
            0xb0 => self.res(B, 6),
            0xb8 => self.res(B, 7),
            0x81 => self.res(C, 0),
            0x89 => self.res(C, 1),
            0x91 => self.res(C, 2),
            0x99 => self.res(C, 3),
            0xa1 => self.res(C, 4),
            0xa9 => self.res(C, 5),
            0xb1 => self.res(C, 6),
            0xb9 => self.res(C, 7),
            0x82 => self.res(D, 0),
            0x8a => self.res(D, 1),
            0x92 => self.res(D, 2),
            0x9a => self.res(D, 3),
            0xa2 => self.res(D, 4),
            0xaa => self.res(D, 5),
            0xb2 => self.res(D, 6),
            0xba => self.res(D, 7),
            0x83 => self.res(E, 0),
            0x8b => self.res(E, 1),
            0x93 => self.res(E, 2),
            0x9b => self.res(E, 3),
            0xa3 => self.res(E, 4),
            0xab => self.res(E, 5),
            0xb3 => self.res(E, 6),
            0xbb => self.res(E, 7),
            0x84 => self.res(H, 0),
            0x8c => self.res(H, 1),
            0x94 => self.res(H, 2),
            0x9c => self.res(H, 3),
            0xa4 => self.res(H, 4),
            0xac => self.res(H, 5),
            0xb4 => self.res(H, 6),
            0xbc => self.res(H, 7),
            0x85 => self.res(L, 0),
            0x8d => self.res(L, 1),
            0x95 => self.res(L, 2),
            0x9d => self.res(L, 3),
            0xa5 => self.res(L, 4),
            0xad => self.res(L, 5),
            0xb5 => self.res(L, 6),
            0xbd => self.res(L, 7),
            0x86 => self.res(Indirect::HL, 0),
            0x8e => self.res(Indirect::HL, 1),
            0x96 => self.res(Indirect::HL, 2),
            0x9e => self.res(Indirect::HL, 3),
            0xa6 => self.res(Indirect::HL, 4),
            0xae => self.res(Indirect::HL, 5),
            0xb6 => self.res(Indirect::HL, 6),
            0xbe => self.res(Indirect::HL, 7),
        }
    }
}