//! The simulator's order: the transactions a delta cycle schedules, when
//! they land, and render's, the sprite store's, OAM's and the VRAM bus's
//! deltas.

#[cfg_attr(
    not(test),
    expect(clippy::wildcard_imports, reason = "render's parts share its items")
)]
use super::*;

/// A flip-flop's change, `tco` after its clock's edge.
#[derive(Clone, Copy)]
pub(super) enum Tx {
    Mode3(bool),
    Voga(bool),
    PxLow(u8),
    PxHigh(u8),
    FetchCount(u8),
    Lyzu(bool),
    Lovy(bool),
    Nyka(bool),
    Pory(bool),
    Pygo(bool),
    BgTile(u8),
    /// The background shifter's shift.
    BgShift(u8, u8),
    Sary(bool),
    Pyco(bool),
    Nunu(bool),
    Nopa(bool),
    WinX(u8),
    WinY(u8),
    Sovy(bool),
    Fine(u8),
    Puxa(bool),
    Nyze(bool),
    Paho(bool),
    Ryfa(bool),
    Rene(bool),
    Sobu(bool),
    Suda(bool),
    SpriteStep(u8),
    Tobu(bool),
    Vonu(bool),
    Tyfo(bool),
    Seba(bool),
    /// The sprite shifter's shift, of the pixels in the mask (the others
    /// load the fetched sprite).
    Scanned(u8),
    Fetched(u16),
    RenderSettled(Option<u16>),
    OamBus(u8, u8),
    Maka(bool),
    XMatchHeld(bool),
    SpShift {
        mask: u8,
        a: u8,
        b: u8,
        palette: u8,
        priority: u8,
    },
}
impl Tx {
    /// Mode 3's rise.
    const fn is_mode3_rise(self) -> bool {
        matches!(self, Self::Mode3(true))
    }

    /// The flip-flop it changes, so that a later change replaces it.
    const fn target(self) -> u8 {
        match self {
            Self::Mode3(_) => 0,
            Self::Voga(_) => 1,
            Self::PxLow(_) => 2,
            Self::PxHigh(_) => 3,
            Self::FetchCount(_) => 4,
            Self::Lyzu(_) => 5,
            Self::Lovy(_) => 6,
            Self::Nyka(_) => 7,
            Self::Pory(_) => 8,
            Self::Pygo(_) => 9,
            Self::BgTile(_) => 10,
            Self::BgShift(..) => 11,
            Self::Sary(_) => 12,
            Self::Pyco(_) => 13,
            Self::Nunu(_) => 14,
            Self::Nopa(_) => 15,
            Self::WinX(_) => 16,
            Self::WinY(_) => 17,
            Self::Sovy(_) => 18,
            Self::Fine(_) => 19,
            Self::Puxa(_) => 20,
            Self::Nyze(_) => 21,
            Self::Paho(_) => 22,
            Self::Ryfa(_) => 23,
            Self::Rene(_) => 24,
            Self::Sobu(_) => 25,
            Self::Suda(_) => 26,
            Self::SpriteStep(_) => 27,
            Self::Tobu(_) => 28,
            Self::Vonu(_) => 29,
            Self::Tyfo(_) => 30,
            Self::Seba(_) => 31,
            Self::SpShift { .. } => 32,
            Self::Scanned(_) => 33,
            Self::Fetched(_) => 34,
            Self::RenderSettled(_) => 35,
            Self::OamBus(..) => 36,
            Self::Maka(_) => 37,
            Self::XMatchHeld(_) => 38,
        }
    }
}
/// How many flip-flops (`Tx::target`) a change can be pending for.
pub(super) const TARGETS: usize = 39;
/// No change is pending for a target.
pub(super) const NEVER: u32 = u32::MAX;
/// The changes scheduled for later nanoseconds of an edge.
pub(super) struct Schedule {
    /// The nanosecond of the change pending for each flip-flop (by
    /// `Tx::target`), `NEVER` if none, and the change. Each target changes
    /// its own flip-flops, so the order in which a nanosecond's changes
    /// apply does not matter.
    due: [u32; TARGETS],
    pending: [Tx; TARGETS],
    /// The nanosecond being settled.
    now: u32,
}
impl Schedule {
    /// A flip-flop's change, `tco` from now. As VHDL's inertial delay does,
    /// it replaces a change still pending for the same flip-flop.
    const fn after_tco(&mut self, tx: Tx) {
        self.after(1, tx);
    }

    /// A change `ns` from now, replacing one still pending.
    const fn after(&mut self, ns: u32, tx: Tx) {
        let target = tx.target() as usize;
        self.due[target] = self.now + ns;
        self.pending[target] = tx;
    }

    /// A zero-delay assignment to a flip-flop (its reset) cancels what is
    /// pending for it.
    const fn cancel(&mut self, target: Tx) {
        self.due[target.target() as usize] = NEVER;
    }

    /// The nanosecond of the earliest change pending, `NEVER` if none.
    const fn next_due(&self) -> u32 {
        let mut first = NEVER;
        let mut target = 0;
        while target < TARGETS {
            if self.due[target] < first {
                first = self.due[target];
            }
            target += 1;
        }
        first
    }

    /// Mode 3's rise is pending.
    const fn mode3_rises(&self) -> bool {
        let target = Tx::Mode3(true).target() as usize;
        (self.due[target] != NEVER) & self.pending[target].is_mode3_rise()
    }
}
/// A byte with its bits in the other order: a sprite's row flipped in X
/// (in operations Aeneas models: not `reverse_bits`).
pub(super) const fn flipped(byte: u8) -> u8 {
    let mut out = 0;
    let mut bit = 0;
    while bit < 8 {
        out |= (byte >> bit & 1) << (7 - bit);
        bit += 1;
    }
    out
}
/// The same VRAM address, or both undriven (`Option`'s equality, which
/// Aeneas does not model).
pub(super) const fn same_address(a: Option<u16>, b: Option<u16>) -> bool {
    match (a, b) {
        (Some(x), Some(y)) => x == y,
        (None, None) => true,
        _ => false,
    }
}
/// The same bytes on OAM's two data buses, or both undriven.
pub(super) const fn same_drivers(a: Option<(u8, u8)>, b: Option<(u8, u8)>) -> bool {
    match (a, b) {
        (Some((a_even, a_odd)), Some((b_even, b_odd))) => (a_even == b_even) & (a_odd == b_odd),
        (None, None) => true,
        _ => false,
    }
}
/// The lowest of the sprite store's slots whose bit is set in `bits`.
pub(super) const fn first_slot(bits: u16) -> Option<usize> {
    let mut slot = 0;
    while slot < 10 {
        if bits & 1 << slot != 0 {
            return Some(slot);
        }
        slot += 1;
    }
    None
}
/// When the other blocks' changes reach render: with the flip-flops, 1 ns
/// after the edge; VRAM's data 2 ns after the address, which changes with
/// the flip-flops (`render_settled` in `hdl/rtl/vram_bus.vhd`).
pub(super) const INPUTS_AT: u32 = 1;
/// The OAM scan's flip-flops clocked by `wuvu` (`start_scan`, `anel`,
/// `ceno`, the index, `byba`), which changes 1 ns after the edge: what they
/// make (the line's start, `ceno`, the index, the stop's rise) reaches
/// render 2 ns after the edge.
pub(super) const SCAN_AT: u32 = 2;
/// VBlank (`mode1`): a flip-flop clocked by `nype`, itself a flip-flop
/// clocked by hclk, which changes 1 ns after the edge (`hdl/rtl/lcd_timing.vhd`).
/// So it changes after SARY, which hclk clocks: as a frame starts, WY's
/// latch sees SARY's new level before VBlank ends.
pub(super) const MODE1_AT: u32 = 3;
/// The CPU's access: its address, strobes and data bus, which settles
/// after what drives it (a register read takes the modes as they change).
pub(super) const DATA_AT: u32 = 4;
/// The data pins, which follow OAM DMA's address (its count changes 1 ns
/// after `dma_phi` rises) through the cartridge bus.
pub(super) const PINS_AT: u32 = 2;
/// The deltas an edge may take before it is taken to oscillate.
pub(super) const MAX_DELTAS: usize = 1000;
pub(super) const fn rose(before: bool, now: bool) -> bool {
    !before & now
}
pub(super) const fn fell(before: bool, now: bool) -> bool {
    before & !now
}

impl NetRender {
    /// [`Self::edge`], noting in `log` the LCD's pins each time they change
    /// on the way.
    ///
    /// # Panics
    ///
    /// If render does not settle (a loop of its wires oscillates).
    pub fn edge_with_pins(
        &mut self,
        inputs: RenderInputs,
        vram: &mut [u8; 0x2000],
        log: &mut PinLog,
    ) {
        log.start(self.pins());
        let mut schedule = Schedule {
            due: [NEVER; TARGETS],
            pending: [Tx::Mode3(false); TARGETS],
            now: 0,
        };
        let mut prev = *self;
        // The clocks decoded from the phase change with the clock.
        self.inputs.clk = inputs.clk;
        self.inputs.clk_t4 = inputs.clk_t4;
        self.inputs.cpu_wr_sync = inputs.cpu_wr_sync;
        self.inputs.cpu_ext_cs_en = inputs.cpu_ext_cs_en;
        self.inputs.dma_phi = inputs.dma_phi;
        let mut deltas = 0;
        let mut mode0_held = false;
        loop {
            // The deltas of this nanosecond, until nothing changes; then
            // the word OAM's RAM selects follows the address, a moment later,
            // and the deltas run again.
            loop {
                loop {
                    let next = self.delta(&prev, &mut schedule, vram);
                    prev = *self;
                    if next == *self {
                        break;
                    }
                    *self = next;
                    deltas += 1;
                    assert!(deltas < MAX_DELTAS, "render oscillates");
                    log.note(self.pins());
                }
                if !self.oam.select_word() {
                    break;
                }
            }
            // Mode 0 as this nanosecond leaves it: a pulse a delta wide has
            // no width, and render.vhd's delay swallows it.
            mode0_held |= self.mode0();
            // The next nanosecond: the inputs' next arrival, or the
            // earliest change pending.
            let arrivals = [INPUTS_AT, SCAN_AT, MODE1_AT, DATA_AT];
            let mut at = schedule.next_due();
            for k in 0..arrivals.len() {
                if (arrivals[k] > schedule.now) & (arrivals[k] < at) {
                    at = arrivals[k];
                }
            }
            if at == NEVER {
                self.mode0_pulse = mode0_held & !self.mode0();
                break;
            }
            schedule.now = at;
            prev = *self;
            let was = self.inputs;
            if at == INPUTS_AT {
                self.inputs = RenderInputs {
                    line_start: was.line_start,
                    ceno: was.ceno,
                    scan_index: was.scan_index,
                    scanning: was.scanning,
                    data: was.data,
                    data_mode3: was.data_mode3,
                    addr: was.addr,
                    rd: was.rd,
                    wr: was.wr,
                    d: was.d,
                    mode1: was.mode1,
                    // stop_scan falls with doba, rises with byba.
                    stop_scan: was.stop_scan & inputs.stop_scan,
                    ..inputs
                };
            }
            if at == MODE1_AT {
                self.inputs.mode1 = inputs.mode1;
            }
            if at == DATA_AT {
                self.inputs.data = inputs.data;
                self.inputs.data_mode3 = inputs.data_mode3;
                self.inputs.addr = inputs.addr;
                self.inputs.rd = inputs.rd;
                self.inputs.wr = inputs.wr;
            }
            if at == PINS_AT {
                self.inputs.d = inputs.d;
            }
            if at == SCAN_AT {
                self.inputs.line_start = inputs.line_start;
                self.inputs.ceno = inputs.ceno;
                self.inputs.scanning = inputs.scanning;
                self.inputs.scan_index = inputs.scan_index;
                self.inputs.stop_scan = inputs.stop_scan;
            }
            for target in 0..TARGETS {
                if schedule.due[target] == at {
                    schedule.due[target] = NEVER;
                    self.apply(schedule.pending[target]);
                }
            }
            log.note(self.pins());
        }
    }

    /// A scheduled change takes effect.
    pub(super) const fn apply(&mut self, tx: Tx) {
        match tx {
            Tx::Mode3(v) => self.mode3 = v,
            Tx::Voga(v) => self.voga = v,
            Tx::PxLow(v) => self.px = self.px & 0xF0 | v & 0x0F,
            Tx::PxHigh(v) => self.px = self.px & 0x0F | v << 4,
            Tx::FetchCount(v) => self.fetch_count = v,
            Tx::Lyzu(v) => self.lyzu = v,
            Tx::Lovy(v) => self.lovy = v,
            Tx::Nyka(v) => self.nyka = v,
            Tx::Pory(v) => self.pory = v,
            Tx::Pygo(v) => self.pygo = v,
            Tx::BgTile(v) => self.bg_tile = v,
            Tx::BgShift(a, b) => {
                self.bg_shift_a = a;
                self.bg_shift_b = b;
            }
            Tx::Sary(v) => self.sary = v,
            Tx::Pyco(v) => self.pyco = v,
            Tx::Nunu(v) => self.nunu = v,
            Tx::Nopa(v) => self.nopa = v,
            Tx::WinX(v) => self.win_x = v,
            Tx::WinY(v) => self.win_y = v,
            Tx::Sovy(v) => self.sovy = v,
            Tx::Fine(v) => self.fine = v,
            Tx::Puxa(v) => self.puxa = v,
            Tx::Nyze(v) => self.nyze = v,
            Tx::Paho(v) => self.paho = v,
            Tx::Ryfa(v) => self.ryfa = v,
            Tx::Rene(v) => self.rene = v,
            Tx::Sobu(v) => self.sobu = v,
            Tx::Suda(v) => self.suda = v,
            Tx::SpriteStep(v) => self.sprite_step = v,
            Tx::Tobu(v) => self.tobu = v,
            Tx::Vonu(v) => self.vonu = v,
            Tx::Tyfo(v) => self.tyfo = v,
            Tx::Seba(v) => self.seba = v,
            Tx::Scanned(v) => self.store.scanned = v,
            Tx::Fetched(v) => self.store.fetched = v,
            Tx::RenderSettled(v) => self.render_settled = v,
            Tx::OamBus(even, odd) => {
                self.oam.bus_even = even;
                self.oam.bus_odd = odd;
            }
            Tx::Maka(v) => self.oam.maka = v,
            Tx::XMatchHeld(v) => self.wires.x_match_held = v,
            Tx::SpShift {
                mask,
                a,
                b,
                palette,
                priority,
            } => {
                self.sp_a = self.sp_a & !mask | a & mask;
                self.sp_b = self.sp_b & !mask | b & mask;
                self.sp_palette = self.sp_palette & !mask | palette & mask;
                self.sp_priority = self.sp_priority & !mask | priority & mask;
            }
        }
    }

    /// The VRAM bus's delta cycle (`hdl/rtl/vram_bus.vhd`) and the board's
    /// VRAM's: the address lines, render's address 2 ns later, the keeper,
    /// and VRAM written while /MCS and /MWR are low.
    // @hw die: XANE XEDU, AHOC, and the not_if0 onto ~ma0..~ma12
    pub(super) fn vram_delta(
        &self,
        prev: &Self,
        next: &mut Self,
        schedule: &mut Schedule,
        vram: &mut [u8; 0x2000],
    ) {
        let (cur, i, w) = (self, &self.inputs, &self.wires);
        if !i.vram_to_oam & !cur.mode3 {
            next.vram_ma = i.addr & 0x1FFF;
        } else if i.vram_to_oam {
            next.vram_ma = i.dma_addr & 0x1FFF;
        } else if let Some(ma) = cur.render_settled {
            next.vram_ma = ma;
        } else {
            // Nobody drives the lines: they keep their value.
        }
        if !same_address(w.render_ma, prev.wires.render_ma) {
            schedule.after(2, Tx::RenderSettled(w.render_ma));
        }
        if w.md_driven {
            next.md_kept = w.md_pins;
        }
        if !w.mcs_n & !w.mwr_n {
            vram[usize::from(cur.vram_ma)] = w.md_pins;
        }
    }

    /// OAM's delta cycle: its wires, RAM and latches; MAKA on the clock's
    /// falling edges; the data lines 1 ns after their driver changes (a
    /// later change replaces one still pending, as VHDL's inertial delay
    /// does).
    // @hw die: MAKA (dffr); CEDE AZAR AZUL APAG
    pub(super) fn oam_delta(&self, prev: &Self, next: &mut Self, schedule: &mut Schedule) {
        let (cur, i) = (self, &self.inputs);
        let oam_inputs = cur.oam_inputs();
        next.oam = cur.oam.delta(&oam_inputs);
        if i.hard_reset {
            next.oam.maka = false;
            schedule.cancel(Tx::Maka(false));
        } else if fell(prev.inputs.clk, i.clk) {
            schedule.after_tco(Tx::Maka(i.clk_t4));
        } else {
            // MAKA holds.
        }
        let driver = cur.oam.bus_drivers(&oam_inputs);
        if !same_drivers(driver, cur.wires.oam_driver) {
            next.wires.oam_driver = driver;
            let (even, odd) = driver.unwrap_or((cur.oam.bus_even, cur.oam.bus_odd));
            schedule.after(1, Tx::OamBus(even, odd));
        }
    }

    /// One delta cycle: the wires follow what they are made of as it was,
    /// the latches and the asynchronous resets act at once, and the
    /// flip-flops whose clock had an edge take what they see, for `tco`
    /// later. `prev` is the delta cycle before, for the edges.
    #[expect(
        clippy::too_many_lines,
        reason = "render's processes, in the VHDL's order"
    )]
    #[expect(
        clippy::cognitive_complexity,
        reason = "render's processes, in the VHDL's order"
    )]
    pub(super) fn delta(
        &self,
        prev: &Self,
        schedule: &mut Schedule,
        vram: &mut [u8; 0x2000],
    ) -> Self {
        let cur = self;
        let (i, w, pw) = (&cur.inputs, &cur.wires, &prev.wires);
        let mut next = *cur;
        let count0 = cur.fetch_count & 1 != 0;
        let (clk_rose, clk_fell) = (rose(prev.inputs.clk, i.clk), fell(prev.inputs.clk, i.clk));

        next.wires = self.next_wires(vram);

        // The registers: latches, open while the CPU writes them. The strobe
        // opens them a few deltas after the clock, on the address and data
        // bus the phase finds; those settle later.
        // @hw die: GAVE..FUNY, DATY..BAKE, NESO..NAFU, MYPA..NUKU, PAVO..MENA,
        // XUFU..XANA, MOXY..LUXO
        if i.hard_reset {
            next.scy = 0;
            next.scx = 0;
            next.wy = 0;
            next.wx = 0;
        } else if w.cpu_wr & 4 != 0 {
            match i.addr {
                0xFF42 => next.scy = i.data,
                0xFF43 => next.scx = i.data,
                0xFF4A => next.wy = i.data,
                0xFF4B => next.wx = i.data,
                _ => {}
            }
        } else {
            // The latches hold.
        }
        if w.cpu_wr & 4 != 0 {
            match i.addr {
                0xFF47 => next.bgp = i.data,
                0xFF48 => next.obp0 = i.data,
                0xFF49 => next.obp1 = i.data,
                _ => {}
            }
        }

        // A sprite's match reaches mode 3's stop 4 ns after it rises; its
        // fall cancels a rise still pending.
        // @rule sprite-at-167-mode-0-pulse
        if !w.x_match {
            schedule.cancel(Tx::XMatchHeld(false));
        } else if !pw.x_match {
            schedule.after(4, Tx::XMatchHeld(true));
        } else {
            // The rise is pending or done.
        }

        // Mode 3: set a nanosecond after the scan stops.
        // @hw die: XYMU (nor_latch); VOGA (dffr)
        if i.ppu_reset | cur.voga {
            next.mode3 = false;
            schedule.cancel(Tx::Mode3(false));
        } else if i.stop_scan & !cur.mode3 & !schedule.mode3_rises() {
            schedule.after_tco(Tx::Mode3(true));
        } else {
            // The latch holds.
        }
        if i.line_start {
            next.voga = false;
            schedule.cancel(Tx::Voga(false));
        } else if clk_rose {
            schedule.after_tco(Tx::Voga(w.stop_mode3));
        } else {
            // The flip-flop holds.
        }

        // The pixel counter.
        // @hw die: XEHO SAVY XODU XYDO, TUHU TUKY TAKO SYBE (dffr); WUSA
        if i.line_start {
            next.px = 0;
            schedule.cancel(Tx::PxLow(0));
            schedule.cancel(Tx::PxHigh(0));
        } else {
            if rose(pw.clkpipe, w.clkpipe) {
                schedule.after_tco(Tx::PxLow(cur.px.wrapping_add(1) & 0x0F));
            }
            if fell(prev.px & 8 != 0, cur.px & 8 != 0) {
                schedule.after_tco(Tx::PxHigh((cur.px >> 4).wrapping_add(1) & 0x0F));
            }
        }
        if i.ppu_reset | cur.voga {
            next.px9 = false;
        } else if cur.px & 9 == 9 {
            next.px9 = true;
        } else {
            // The latch holds.
        }

        // The fetcher.
        // @hw die: LAXU MESU NYVA (dffr); LYZU LOVY (dffr), LONY (nand_latch)
        if w.fetch_reset {
            next.fetch_count = 0;
            schedule.cancel(Tx::FetchCount(0));
        } else if rose(pw.fetch_clock, w.fetch_clock) {
            schedule.after_tco(Tx::FetchCount((cur.fetch_count + 1) & 7));
        } else {
            // The counter holds.
        }
        if !cur.mode3 {
            next.lyzu = false;
            schedule.cancel(Tx::Lyzu(false));
        } else if clk_rose {
            schedule.after_tco(Tx::Lyzu(count0));
        } else {
            // The flip-flop holds.
        }
        if w.fetch_reset {
            next.lovy = false;
            schedule.cancel(Tx::Lovy(false));
        } else if clk_fell {
            schedule.after_tco(Tx::Lovy(w.fetch_done));
        } else {
            // The flip-flop holds.
        }
        if w.fetch_reset {
            next.fetching = true;
        } else if cur.lovy | !cur.mode3 {
            next.fetching = false;
        } else {
            // The latch holds.
        }

        // The first fetch of the line.
        // @hw die: NYKA PORY PYGO (dffr), POKY (nor_latch)
        if !cur.mode3 | w.win_start {
            next.nyka = false;
            next.pory = false;
            schedule.cancel(Tx::Nyka(false));
            schedule.cancel(Tx::Pory(false));
        } else {
            if clk_rose {
                schedule.after_tco(Tx::Nyka(w.fetch_done));
            }
            if clk_fell {
                schedule.after_tco(Tx::Pory(cur.nyka));
            }
        }
        if !cur.mode3 {
            next.pygo = false;
            schedule.cancel(Tx::Pygo(false));
        } else if clk_rose {
            schedule.after_tco(Tx::Pygo(cur.pory));
        } else {
            // The flip-flop holds.
        }
        if !cur.mode3 {
            next.started = false;
        } else if cur.pygo {
            next.started = true;
        } else {
            // The latch holds.
        }

        // The tile register, the low byte's latch, the background shifter.
        // @hw die: RAWA..PYJU (dffr_cc_q); LEGU..NEFO (dlatch_ee_q); MYDE..PYBO,
        // TOMY..SOHU (dffsr)
        // @fact window-start-tile-register-pulse
        if fell(pw.bg_tile_clock, w.bg_tile_clock) {
            schedule.after_tco(Tx::BgTile(w.md));
        }
        if w.myso & (cur.fetch_count == 3) {
            next.bg_low = w.md;
        }
        if w.fetch_reset {
            next.bg_shift_a = cur.bg_low;
            next.bg_shift_b = cur.bg_tile;
            schedule.cancel(Tx::BgShift(0, 0));
        } else if rose(pw.clkpipe, w.clkpipe) {
            schedule.after_tco(Tx::BgShift(cur.bg_shift_a << 1, cur.bg_shift_b << 1));
        } else {
            // The shifter holds.
        }

        // The window.
        // @hw die: SARY (dffr), REJO (nor_latch); PYCO NUNU NOPA (dffr), PYNU
        // (nor_latch); WYKA..XOLO, VYNO..TEKE (dffr); RYDY (nor_latch), SOVY
        if i.ppu_reset {
            next.sary = false;
            schedule.cancel(Tx::Sary(false));
        } else if rose(prev.inputs.hclk, i.hclk) {
            schedule.after_tco(Tx::Sary(w.wy_match));
        } else {
            // The flip-flop holds.
        }
        if i.mode1 | i.ppu_reset {
            next.wy_latch = false;
        } else if cur.sary {
            next.wy_latch = true;
        } else {
            // The latch holds.
        }
        if i.ppu_reset {
            next.pyco = false;
            next.nunu = false;
            next.nopa = false;
            schedule.cancel(Tx::Pyco(false));
            schedule.cancel(Tx::Nunu(false));
            schedule.cancel(Tx::Nopa(false));
        } else {
            if rose(pw.dot, w.dot) {
                schedule.after_tco(Tx::Pyco(w.wxy_match));
            }
            if clk_fell {
                schedule.after_tco(Tx::Nunu(cur.pyco));
            }
            if clk_rose {
                schedule.after_tco(Tx::Nopa(cur.in_window));
            }
        }
        if w.win_reset {
            next.in_window = false;
        } else if cur.nunu {
            next.in_window = true;
        } else {
            // The latch holds.
        }
        if w.win_reset {
            next.win_x = 0;
            schedule.cancel(Tx::WinX(0));
        } else if rose(pw.wx_clk, w.wx_clk) {
            schedule.after_tco(Tx::WinX((cur.win_x + 1) & 0x1F));
        } else {
            // The counter holds.
        }
        if i.mode1 | i.ppu_reset {
            next.win_y = 0;
            schedule.cancel(Tx::WinY(0));
        } else if fell(prev.in_window, cur.in_window) {
            schedule.after_tco(Tx::WinY(cur.win_y.wrapping_add(1)));
        } else {
            // The counter holds.
        }
        if cur.pory | i.ppu_reset {
            next.rydy = false;
        } else if w.win_start {
            next.rydy = true;
        } else {
            // The latch holds.
        }
        if i.ppu_reset {
            next.sovy = false;
            schedule.cancel(Tx::Sovy(false));
        } else if clk_rose {
            schedule.after_tco(Tx::Sovy(cur.rydy));
        } else {
            // The flip-flop holds.
        }

        // The fine scroll.
        // @hw die: RYKU ROGA RUBU (dffr); PUXA NYZE (dffr), ROXY (nor_latch);
        // RYFA RENE (dffr)
        if w.fine_reset {
            next.fine = 0;
            schedule.cancel(Tx::Fine(0));
        } else if fell(pw.dot, w.dot) & (cur.fine != 7) {
            schedule.after_tco(Tx::Fine(cur.fine + 1));
        } else {
            // The counter holds.
        }
        if cur.mode3 {
            if rose(pw.dot, w.dot) {
                schedule.after_tco(Tx::Puxa(cur.discard & (cur.fine == cur.scx & 7)));
                schedule.after_tco(Tx::Paho(cur.px & 8 != 0));
            }
            if clk_fell {
                schedule.after_tco(Tx::Nyze(cur.puxa));
            }
        } else {
            next.puxa = false;
            next.nyze = false;
            next.paho = false;
            schedule.cancel(Tx::Puxa(false));
            schedule.cancel(Tx::Nyze(false));
            schedule.cancel(Tx::Paho(false));
        }
        if w.fine_done {
            next.discard = false;
        } else if !cur.mode3 {
            next.discard = true;
        } else {
            // The latch holds.
        }
        if cur.mode3 {
            if rose(pw.segu, w.segu) {
                schedule.after_tco(Tx::Ryfa((cur.fine == 7) & !w.wxy_match));
            }
            if clk_rose {
                schedule.after_tco(Tx::Rene(cur.ryfa));
            }
        } else {
            next.ryfa = false;
            next.rene = false;
            schedule.cancel(Tx::Ryfa(false));
            schedule.cancel(Tx::Rene(false));
        }

        // ST.
        // @hw die: RUJU POME POFY (nor_latch)
        if cur.paho | i.ppu_reset {
            next.line_started = true;
        } else if i.stop_scan {
            next.line_started = false;
        } else {
            // The latch holds.
        }

        // The sprite fetch.
        // @hw die: SOBU SUDA (dffr); TOXE TULY TESE (dffr); TAKA (nand_latch);
        // TOBU VONU SEBA TYFO (dffr)
        if clk_rose {
            schedule.after_tco(Tx::Sobu(w.x_match & !cur.rydy & w.fetch_done & !cur.busy));
        }
        if clk_fell {
            schedule.after_tco(Tx::Suda(cur.sobu));
        }
        if w.step_reset {
            next.sprite_step = 0;
            schedule.cancel(Tx::SpriteStep(0));
        } else if clk_rose & (cur.sprite_step != 5) {
            schedule.after_tco(Tx::SpriteStep(cur.sprite_step + 1));
        } else {
            // The counter holds.
        }
        if w.step_reset {
            next.busy = true;
        } else if w.loaded | (cur.mode3 & !cur.started & cur.nyka & cur.pory) {
            next.busy = false;
        } else {
            // The latch holds.
        }
        if !cur.mode3 {
            next.tobu = false;
            next.vonu = false;
            schedule.cancel(Tx::Tobu(false));
            schedule.cancel(Tx::Vonu(false));
        } else if clk_rose {
            schedule.after_tco(Tx::Tobu(cur.sprite_step & 2 != 0));
            schedule.after_tco(Tx::Vonu(cur.tobu));
        } else {
            // The flip-flops hold.
        }
        if clk_fell {
            schedule.after_tco(Tx::Tyfo(cur.sprite_step & 1 != 0));
            schedule.after_tco(Tx::Seba(cur.mode3 & cur.vonu));
        }

        // The sprite's two bytes.
        // @hw die: PEFO..RYDU, REWO..SEGA (dlatch_ee_q)
        if cur.vonu & w.sycu {
            next.sp_low_a = w.sp_data;
        }
        if (cur.sprite_step & 2 != 0) & w.sycu {
            next.sp_low_b = w.sp_data;
        }

        // The sprite shifter: the fetched sprite loads its free pixels.
        // @hw die: NYLU..VUPY, NURO..WUFY (dffsr)
        let on = w.sprite_on;
        if on != 0 {
            let palette = if cur.oam.second_odd & 0x10 != 0 {
                0xFF
            } else {
                0
            };
            let priority = if cur.oam.second_odd & 0x80 != 0 {
                0xFF
            } else {
                0
            };
            next.sp_a = cur.sp_a & !on | cur.sp_low_a & on;
            next.sp_b = cur.sp_b & !on | cur.sp_low_b & on;
            next.sp_palette = cur.sp_palette & !on | palette & on;
            next.sp_priority = cur.sp_priority & !on | priority & on;
        }
        if rose(pw.clkpipe, w.clkpipe) & (on != 0xFF) {
            schedule.after_tco(Tx::SpShift {
                mask: !on,
                a: cur.sp_a << 1,
                b: cur.sp_b << 1,
                palette: cur.sp_palette << 1,
                priority: cur.sp_priority << 1 | 1,
            });
        }
        self.store_delta(prev, &mut next, schedule);
        self.vram_delta(prev, &mut next, schedule, vram);
        self.oam_delta(prev, &mut next, schedule);
        next
    }

    /// The sprite store's delta cycle (`hdl/rtl/sprite_store.vhd`).
    // @hw die: XADU..XECU (dffr_cc); the drlatch_ee of the X, the dlatch_ee
    // of the line and the index; EBOJ..FONO (dffr); the X matchers, GUVA..
    // GUZE and GEZE..GYGA; FEFY FOVE FEPO; WUZY..WEZA
    pub(super) fn store_delta(&self, prev: &Self, next: &mut Self, schedule: &mut Schedule) {
        let cur = self;
        let (i, w, store) = (&cur.inputs, &cur.wires, &cur.store);
        if rose(prev.inputs.wuvu, i.wuvu) {
            schedule.after_tco(Tx::Scanned(w.oam_index));
        }
        for slot in 0..10 {
            if i.line_start | (store.fetched & 1 << slot != 0) {
                next.store.xs[slot] = 0xFF;
            } else if i.save & 1 << slot != 0 {
                next.store.xs[slot] = cur.oam.second_odd;
            } else {
                // The latches hold.
            }
            if i.save & 1 << slot != 0 {
                next.store.lines[slot] = i.scan_line;
                next.store.indexes[slot] = store.scanned;
            }
        }
        if i.line_start {
            next.store.fetched = 0;
            schedule.cancel(Tx::Fetched(0));
        } else if rose(prev.wires.loaded, w.loaded) {
            // Each slot takes whether it is the one selected: a slot fetched
            // before keeps its X at FF, as its latch holds.
            schedule.after_tco(Tx::Fetched(w.selected));
        } else {
            // The flip-flops hold.
        }
        if !w.rendering {
            next.store.index = store.scanned;
        } else if let Some(slot) = first_slot(w.selected) {
            next.store.index = store.indexes[slot];
        } else {
            // The lines keep their value.
        }
    }
}
