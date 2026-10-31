//! The SM83's sequencer (`hdl/rtl/sm83_sequencer.vhd`): the opcode
//! register, the M-cycle counter, the CB prefix, which of W and Z a read
//! goes to, HALT and STOP, IME and the entry into an interrupt (the DMG's:
//! its NMI pin is not bonded out, and is low).
//!
//! The M-cycle counter takes the next M-cycle at the rise of `clk` (phase
//! A); it goes to 7 when a condition fails and while the core is idle,
//! and 7's step fetches the next opcode.
//!
//! @hw die: dmg-schematics, categories seq, seq-irq and reg-ir

use super::{control::Controls, decoder::Steps};

/// The sequencer's flip-flops and latches.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[expect(clippy::struct_excessive_bools, reason = "the netlist's latches")]
pub struct NetSequencer {
    /// The opcode, and what IR's master took while the core fetched.
    // @hw die: REG_IR (dff_cc_ee_q_x2_reg_bit), REG_IR_NOT_WE
    pub opcode: u8,
    pub ir_next: u8,
    /// The M-cycle. XAYM XIRF XUFU
    pub mcyc: u8,
    /// A read goes to W (else to Z). XDQF
    pub sel_reg_w: bool,
    /// The CB prefix: taken while the core fetches, shown once it does
    /// not. XURG
    pub cb_next: bool,
    pub cb_flag: bool,
    /// HALT and STOP. YNKW, ZUMN
    pub halt: bool,
    pub stop: bool,
    /// Their steps, an M-cycle later. YSBT, ZKAI
    pub halt_delayed: bool,
    pub stop_delayed: bool,
    /// An interrupt was pending at the last M-cycle's end. YOII
    pub pending_delayed: bool,
    /// The oscillator is not stable, an M-cycle and two before. ZORP, ZAZA
    pub not_ready_1: bool,
    pub not_ready_2: bool,
    /// IME: set by EI and RETI, cleared by DI and the entry, shown at the
    /// end of the M-cycle. ZJJE, ~ZIVV
    pub ime_state: bool,
    pub ime: bool,
    /// Not entering an NMI; an M-cycle later. ZRSY, ZKDU
    pub no_nmi: bool,
    pub no_nmi_1: bool,
    /// An interrupt's entry starts; it runs. ZKOG, ZACW
    pub entering: bool,
    pub int_entry: bool,
    /// The NMI's edge detector and latches. YDXA, ~YODP, ZOJZ, ZLOZ, ZRBY
    pub nmi_low_1: bool,
    pub nmi_low_2: bool,
    pub nmi_req: bool,
    pub nmi_entering: bool,
    pub nmi_entry: bool,
}

/// What the flip-flops clocked by `clk` take at its rise: the wires as the
/// phase before left them.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
#[expect(clippy::struct_excessive_bools, reason = "the wires' levels")]
pub struct Sampled {
    /// The next M-cycle, 7 when a condition failed or the core is idle.
    pub next_mcyc: u8,
    pub mread: bool,
    pub halt_step: bool,
    pub stop: bool,
    pub int_pending: bool,
    pub not_ready: bool,
    pub nmi: bool,
}

/// What the sequencer's latches take in a phase.
#[derive(Clone, Copy)]
#[expect(clippy::struct_excessive_bools, reason = "the inputs' levels")]
pub struct SequencerInputs {
    pub clk: bool,
    pub data_phase: bool,
    pub write_phase: bool,
    pub sys_reset: bool,
    pub pwron_reset: bool,
    /// A joypad line went low: STOP ends.
    pub wake: bool,
    pub nmi: bool,
    /// The core's data bus.
    pub db: u8,
    /// An interrupt is enabled, requested and selected.
    pub int_take: bool,
}

impl NetSequencer {
    pub const POWER_ON: Self = Self {
        opcode: 0,
        ir_next: 0,
        mcyc: 0,
        sel_reg_w: false,
        cb_next: false,
        cb_flag: false,
        halt: false,
        stop: false,
        halt_delayed: false,
        stop_delayed: false,
        pending_delayed: false,
        not_ready_1: true,
        not_ready_2: true,
        ime_state: false,
        ime: false,
        no_nmi: true,
        no_nmi_1: true,
        entering: false,
        int_entry: false,
        nmi_low_1: true,
        nmi_low_2: true,
        nmi_req: false,
        nmi_entering: false,
        nmi_entry: false,
    };

    /// The core is idle: halted, or in reset.
    #[must_use]
    pub const fn idle(&self, sys_reset: bool) -> bool {
        self.halt | sys_reset
    }

    /// The decoder's "int": the interrupt's entry, or idle.
    #[must_use]
    pub const fn in_intr(&self, sys_reset: bool) -> bool {
        self.idle(sys_reset) | self.int_entry
    }

    /// The opcode came after the CB prefix (which counts in M-cycles 0 to
    /// 3 only).
    // @hw die: XURI, XTIP, XUDO
    #[must_use]
    pub const fn table_cb(&self) -> bool {
        self.cb_flag & (self.mcyc & 4 == 0)
    }

    /// `clk` rises: the flip-flops take what the phase before left.
    // @hw die: XAYM XIRF XUFU, XDQF; YSBT ZKAI YOII ZORP ZAZA; ZACW ZKDU
    // YDXA ZRBY
    pub const fn clock(&mut self, sampled: Sampled) {
        let was = *self;
        self.mcyc = sampled.next_mcyc;
        self.sel_reg_w = sampled.mread & !was.sel_reg_w;
        self.halt_delayed = sampled.halt_step;
        self.stop_delayed = sampled.stop;
        self.pending_delayed = sampled.int_pending;
        self.not_ready_1 = sampled.not_ready;
        self.not_ready_2 = was.not_ready_1;
        self.int_entry = was.entering | was.nmi_entering;
        self.no_nmi_1 = was.no_nmi;
        self.nmi_low_1 = !sampled.nmi;
        self.nmi_entry = was.nmi_entering;
    }

    /// `exec_phase` falls, at the end of the M-cycle: IME takes effect.
    // @hw die: ZIVV (dff), ZOXC
    pub const fn exec_falls(&mut self) {
        self.ime = self.no_nmi & self.ime_state;
    }

    /// The latches, in a phase.
    pub const fn settle(&mut self, s: &Steps, c: &Controls, i: SequencerInputs) {
        let (data, write) = (i.data_phase, i.write_phase);
        let idle = self.idle(i.sys_reset);
        let op3 = self.opcode & 0x08 != 0;

        // IR and the CB prefix take what the core fetches, in
        // write_phase; HALT clears the prefix.
        // @hw die: REG_IR; XURG (dffre)
        if c.fetch & write {
            self.ir_next = i.db;
        }
        if self.halt {
            self.cb_next = false;
        } else if c.fetch & write {
            self.cb_next = s.cb_prefix;
        } else {
            // The master holds.
        }
        if !c.fetch {
            self.cb_flag = self.cb_next;
        }

        // An instruction ends where it fetches the next opcode (but not
        // after the CB prefix), or while idle.
        // @hw die: XOGS
        let boundary = (data & c.fetch & !s.cb_prefix) | idle;

        // HALT and STOP.
        // @hw die: YNOZ, ZIKS ZUDN, YOLU, ZHZO ZKON, YKUA, YNKW (srlatch),
        // ZUMN (srlatch), ZWLM, XWEE ZJBF, YCNF
        let any_reset = i.sys_reset | i.pwron_reset;
        let halt_set = i.pwron_reset | self.stop_delayed | self.halt_delayed;
        let halt_reset =
            ((!self.not_ready_1 & self.not_ready_2) | self.pending_delayed) & !any_reset;
        if halt_reset {
            self.halt = false;
        } else if halt_set {
            self.halt = true;
        } else {
            // The latch holds.
        }
        if i.wake | any_reset {
            self.stop = false;
        } else if self.stop_delayed {
            self.stop = true;
        } else {
            // The latch holds.
        }

        // IME: EI sets it, DI clears it, RETI sets it, the entry clears
        // it.
        // @hw die: ZJJE (srlatch), ZBPP, ZWUU, ZRSY (srlatch), ZNDA ZGNA
        let ime_set = data & ((op3 & s.di_or_ei) | (c.reti_m3 & self.no_nmi_1));
        let ime_reset =
            (data & ((!op3 & s.di_or_ei) | (self.no_nmi_1 & self.int_entry))) | i.sys_reset;
        if ime_reset {
            self.ime_state = false;
        } else if ime_set {
            self.ime_state = true;
        } else {
            // The latch holds.
        }
        if self.nmi_entering {
            self.no_nmi = false;
        } else if i.sys_reset | c.reti_m3 {
            self.no_nmi = true;
        } else {
            // The latch holds.
        }

        // The entry: IME set and an interrupt requested at the end of an
        // instruction (but not during DI).
        // @hw die: ZAIJ, ZAOC, ZZOM, ZKOG (srlatch), ZOWA, ZFEX, XYGB
        let not_di = op3 | !s.di_or_ei;
        let int_start = self.ime & data & i.int_take & boundary & not_di;
        if s.int_entry_m6 | i.sys_reset {
            self.entering = false;
        } else if int_start {
            self.entering = true;
        } else {
            // The latch holds.
        }

        // NMI: a rise of nmi, at the end of an instruction.
        // @hw die: YEPJ, YODP (dlatch), YNIU, ZOJZ ZLOZ (srlatch), ZYOC,
        // YNEU
        if !i.clk {
            self.nmi_low_2 = self.nmi_low_1;
        }
        let nmi_rise = self.nmi_low_2 & i.nmi;
        let nmi_clear = (self.nmi_entry & s.int_entry_m6) | i.sys_reset;
        if nmi_clear {
            self.nmi_req = false;
            self.nmi_entering = false;
        } else {
            if nmi_rise {
                self.nmi_req = true;
            }
            if boundary & self.no_nmi_1 & self.nmi_req {
                self.nmi_entering = true;
            }
        }
    }
}
