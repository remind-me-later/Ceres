//! The netlist's clocks and resets, edge by edge.
//!
//! Transcribed from the DMG's part of `hdl/rtl/clocks.vhd`, for the replay
//! of the model's traces (`tests/replay.rs`), which checks the
//! transcription, and for the blocks that run on its phases.
//!
//! The DMG's crystal (CK1, 4 MiHz) is the chip's clock, held high while the
//! oscillator is stopped (STOP). Four latches count its edges: AFUR and APUK
//! are open while the clock is high, ALEF and ADYK while it is low, and
//! each copies the one before it (AFUR the inverse of ADYK), so that one of
//! them changes at every edge and the four name the phase, A to H, of the
//! CPU's M-cycle. The other clocks are the phases decoded. The resets: the
//! /RESET pin, the power-on wait (TUBO) that lasts until the divider's last
//! bit is set, `sys_reset` until then, and `cpu_sys_reset` a flip-flop
//! later, at the start of A.

/// The phases of the CPU's M-cycle: one per edge of the clock, which is
/// high in A, C, E and G. The letters are GateBoy's.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Phase {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
}

impl Phase {
    /// The phase after this one.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::A => Self::B,
            Self::B => Self::C,
            Self::C => Self::D,
            Self::D => Self::E,
            Self::E => Self::F,
            Self::F => Self::G,
            Self::G => Self::H,
            Self::H => Self::A,
        }
    }
}

/// What the clocks take in, as it is after an edge: the pins, the CPU
/// core's enables and access, the test register's reset and the divider's
/// last bit.
#[derive(Clone, Copy)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the pins' and strobes' levels"
)]
pub struct ClockInputs {
    /// CK1, the crystal.
    pub ck1: bool,
    /// /RESET, high on the Game Boy once it is powered.
    pub reset_pin_n: bool,
    /// The test pins, low on the Game Boy.
    pub t1: bool,
    pub t2: bool,
    /// The oscillator runs (low in STOP).
    pub osc_ena: bool,
    /// The CPU core's clocks run (low in HALT).
    pub clk_ena: bool,
    /// The CPU accesses memory, and writes.
    pub mreq: bool,
    pub write: bool,
    /// From the test register FF60.
    pub test_reset: bool,
    /// The divider's last bit (`div(15)`).
    pub div_16hz: bool,
}

/// The netlist's clocks and resets, between edges of the crystal.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the netlist's latches and flip-flops"
)]
pub struct NetClocks {
    /// The chip's clock, as the last edge left it.
    pub clk: bool,
    /// AFUR, ALEF, APUK, ADYK: the phase counter's latches.
    pub afur: bool,
    pub alef: bool,
    pub apuk: bool,
    pub adyk: bool,
    /// TUBO: the chip waits for the CPU core to start its clocks.
    pub waiting: bool,
    /// ASOL: from /RESET until the power-on wait is over.
    pub sys_reset: bool,
    /// AFER: `sys_reset`, at the start of the next A.
    pub cpu_sys_reset: bool,
}

/// The clocks the other blocks take, decoded from the phase.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[expect(clippy::struct_excessive_bools, reason = "the clocks' levels")]
pub struct ClockOutputs {
    /// High in A (and while the oscillator is stopped): the M-cycle starts.
    pub clk_1mhz: bool,
    /// High in G and H.
    pub clk_t4: bool,
    /// High in E to H, the M-cycle's second half.
    pub data_phase: bool,
    /// High in B to H.
    pub exec_phase: bool,
    /// High in B to F.
    pub pch_phase: bool,
    /// A CPU write, in E to G.
    pub cpu_wr_sync: bool,
    /// A CPU access may select the cartridge: C to H.
    pub cpu_ext_cs_en: bool,
    /// PHI, the cartridge's M-cycle clock.
    pub phi: bool,
    /// The power-on wait is over.
    pub osc_stable: bool,
    /// The /RESET pin, and `cpu_sys_reset` or `sys_reset`: the reset of most
    /// registers.
    pub reset: bool,
    pub reset2: bool,
}

impl NetClocks {
    /// The phase the latches name.
    // @hw die: AFUR ALEF APUK ADYK, decoded
    #[must_use]
    pub const fn phase(&self) -> Phase {
        match (self.afur, self.alef, self.apuk, self.adyk) {
            (true, false, false, false) => Phase::A,
            (true, true, false, false) => Phase::B,
            (true, true, true, false) => Phase::C,
            (true, true, true, true) => Phase::D,
            (false, true, true, true) => Phase::E,
            (false, false, true, true) => Phase::F,
            (false, false, false, true) => Phase::G,
            _ => Phase::H,
        }
    }

    /// An edge of the crystal, or a change of what the clocks take in:
    /// `inputs` as they are after it.
    pub const fn step(&mut self, inputs: ClockInputs) {
        let before = self.phase();
        // The oscillator's output, held high while it is stopped.
        // @hw die: CK1_CK2 (pad_xtal), ANOS AVET ARYS, ATAL
        let clk = inputs.ck1 || !inputs.osc_ena;
        // AFER takes `sys_reset` as it was at the edge that starts A.
        // @hw die: AFER (dffr_cc), clocked by BOGA
        let starts_a = clk != self.clk && matches!(before.next(), Phase::A);
        let sys_reset_at_edge = self.sys_reset;
        self.clk = clk;

        // The phase counter: the latches open while the clock is high copy,
        // then those open while it is low.
        // @hw die: AFUR ALEF APUK ADYK (drlatch_ee)
        if inputs.test_reset {
            self.afur = false;
            self.alef = false;
            self.apuk = false;
            self.adyk = false;
        } else if clk {
            self.afur = !self.adyk;
            self.apuk = self.alef;
        } else {
            self.alef = self.afur;
            self.adyk = self.apuk;
        }

        // @hw die: TUBO (nor_latch), UPYF UCOB, UNUT TABA
        let reset = !inputs.reset_pin_n;
        if inputs.clk_ena || reset || !inputs.osc_ena {
            self.waiting = !inputs.clk_ena;
        }
        // @hw die: ASOL (nor_latch), AFAR ALYP
        if reset {
            self.sys_reset = true;
        } else if Self::osc_stable(self.waiting, inputs) {
            self.sys_reset = false;
        } else {
            // The latch holds.
        }
        if inputs.test_reset {
            self.cpu_sys_reset = false;
        } else if starts_a {
            self.cpu_sys_reset = sys_reset_at_edge;
        } else {
            // The flip-flop holds.
        }
    }

    /// The power-on wait is over: the divider's last bit is set while the
    /// chip waits, or a test pin skips the wait.
    const fn osc_stable(waiting: bool, inputs: ClockInputs) -> bool {
        (inputs.t1 ^ inputs.t2) || (waiting && inputs.div_16hz)
    }

    /// The clocks and resets the other blocks take, with `inputs`.
    #[must_use]
    pub const fn outputs(&self, inputs: ClockInputs) -> ClockOutputs {
        let phase = self.phase();
        let (a, c_on, e_on, g_or_h, e_to_g) = (
            matches!(phase, Phase::A),
            !matches!(phase, Phase::A | Phase::B),
            matches!(phase, Phase::E | Phase::F | Phase::G | Phase::H),
            matches!(phase, Phase::G | Phase::H),
            matches!(phase, Phase::E | Phase::F | Phase::G),
        );
        // @hw die: BELE BYJU BALY BOGA
        let clk_1mhz = a || !inputs.osc_ena;
        // @hw die: BELU BYRY BUDE BEVA
        let data_phase = inputs.clk_ena && e_on;
        ClockOutputs {
            clk_1mhz,
            // @hw die: BAPY BERU BUFA BYLY
            clk_t4: inputs.clk_ena && g_or_h,
            data_phase,
            // @hw die: BUVU BYXO BEDO
            exec_phase: inputs.clk_ena && !clk_1mhz,
            // @hw die: BATE BASU BUKE
            pch_phase: if g_or_h || a { !inputs.clk_ena } else { true },
            // @hw die: AFAS AREV APOV
            cpu_wr_sync: inputs.write && e_to_g,
            // @hw die: AGUT AWOD ABUZ
            cpu_ext_cs_en: (inputs.t1 && !inputs.t2) || (inputs.mreq && c_on),
            // @hw die: PHI (pad_out)
            phi: !data_phase,
            osc_stable: Self::osc_stable(self.waiting, inputs),
            // @hw die: ~RESET (pad_in); AVOR ALUR
            reset: !inputs.reset_pin_n,
            reset2: self.cpu_sys_reset || self.sys_reset,
        }
    }
}
