//! The netlist's rendering of a line (mode 3), delta cycle by delta cycle.
//!
//! Transcribed from the DMG's part of `hdl/rtl/render.vhd`, and
//! `hdl/rtl/sprite_store.vhd`, whose X matchers and render's pixel counter
//! feed each other within an edge, for the replay of the model's traces
//! (`tests/replay.rs`), which checks the transcription, and for `NetChip`.
//!
//! Render runs on clocks it makes from the PPU's: the dots (`dot`), the
//! pixels' clock (`clkpipe`), the fetcher's (`fetch_clock`), the tile
//! register's (`bg_tile_clock`), the window's column (`wx_clk`), and the
//! falling edges of the pixel counter's bit 3 and of `in_window`. Which of
//! them ticks at an edge, and with what, depends on the order in which the
//! simulator settles them: the tile register's clock pulses for no time as
//! the window starts (`window-start-tile-register-pulse`). So the
//! transcription settles render as the simulator does. Each concurrent
//! statement of the VHDL is a wire that follows its inputs a delta cycle
//! later, each process reads the signals as they are in the current delta
//! cycle, and the flip-flops' outputs change `tco` (1 ns) later. An edge of
//! the PPU's clock runs deltas until nothing changes, then the next
//! nanosecond's changes, until none are left.
//!
//! What render takes from the other blocks (the OAM scan's line start and
//! stop, the sprite store's match and line, OAM's bytes, the line timing's
//! hclk, LY and VBlank, LCDC, the CPU's writes) changes with the
//! flip-flops, 1 ns after the edge; what the scan's flip-flops clocked by
//! `wuvu` make, a nanosecond later; VRAM's data 2 ns after the flip-flops,
//! as the address reaches VRAM 2 ns after it changes.

/// What render takes in.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[expect(clippy::struct_excessive_bools, reason = "the inputs' levels")]
pub struct RenderInputs {
    /// The PPU's clock (the chip's).
    pub clk: bool,
    pub ppu_reset: bool,
    pub hard_reset: bool,
    /// The line timing's M-cycle clock, VBlank and LY.
    pub hclk: bool,
    pub mode1: bool,
    pub ly: u8,
    pub lcdc: u8,
    /// The CPU's write: the address and the data bus.
    pub addr: u16,
    pub wr: bool,
    pub data: u8,
    /// From the OAM scan: the line starts, the scan stops.
    pub line_start: bool,
    pub stop_scan: bool,
    /// The OAM scan's `ceno` (the scan's last read is under way) and save
    /// strobes (bit `i` for slot `i`), and the Y comparator's line.
    pub ceno: bool,
    pub save: u16,
    pub scan_line: u8,
    /// The line timing's `wuvu`, which clocks the sprite store's index.
    pub wuvu: bool,
    /// What addresses OAM, besides the store: the DMA's index while it
    /// runs, the scan's in mode 2, the CPU's (its address's bits 7 to 2)
    /// otherwise.
    pub dma_run: bool,
    pub dma_index: u8,
    pub mode2: bool,
    pub scan_index: u8,
    /// OAM's even and odd bytes: the sprite's tile and attributes.
    pub tile: u8,
    pub attributes: u8,
    /// The CPU's read strobe, its memory request outside VRAM, and the
    /// clocks' `write`, `cpu_wr_sync`, `cpu_ext_cs_en` and `clk_t4`, for the
    /// VRAM bus.
    pub rd: bool,
    pub non_vram_mreq: bool,
    pub write: bool,
    pub cpu_wr_sync: bool,
    pub cpu_ext_cs_en: bool,
    pub clk_t4: bool,
    /// OAM DMA reads VRAM, at its address.
    pub vram_to_oam: bool,
    pub dma_addr: u16,
}

/// The wires: render's concurrent statements, each a delta cycle behind
/// what it is made of.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[expect(clippy::struct_excessive_bools, reason = "the netlist's gates")]
pub struct Wires {
    pub stop_mode3: bool,
    pub dot: bool,
    pub segu: bool,
    pub clkpipe: bool,
    pub fetch_reset: bool,
    pub fetch_done: bool,
    pub fetch_clock: bool,
    pub myso: bool,
    pub bg_tile_clock: bool,
    pub wy_match: bool,
    pub wxy_match: bool,
    pub win_reset: bool,
    pub win_start: bool,
    pub wx_clk: bool,
    pub fine_reset: bool,
    pub fine_done: bool,
    pub restart: bool,
    pub fetch_start: bool,
    pub step_reset: bool,
    pub sycu: bool,
    pub loaded: bool,
    pub sprite_cycles: bool,
    pub sp_data: u8,
    /// The sprite shifter's free pixels: bit `i` for pixel `i`.
    pub sprite_on: u8,
    /// The sprite store's: rendering (mode 3 after the scan), its matchers'
    /// enable, the slots whose X matches the pixel counter, the first of
    /// them, a match, and the line lines.
    pub rendering: bool,
    pub enabled: bool,
    pub matches: u16,
    pub selected: u16,
    pub x_match: bool,
    pub line: u8,
    /// OAM's address, bits 7 to 2 (`oam_addr` of `hdl/rtl/oam.vhd`).
    pub oam_index: u8,
    /// The VRAM bus's (`hdl/rtl/vram_bus.vhd`) and the board's: render's
    /// address, the CPU addresses VRAM (and writes it), /MCS, /MRD, /MWR, the
    /// data pins (driven, or as the keeper holds them), and VRAM's data as
    /// render and OAM see it.
    pub render_ma: Option<u16>,
    pub cpu_vram: bool,
    pub cpu_write: bool,
    pub mcs_n: bool,
    pub mrd_n: bool,
    pub mwr_n: bool,
    pub md_pins: u8,
    /// Something drives the data pins (the CPU's write, or VRAM): what the
    /// keeper takes.
    pub md_driven: bool,
    pub md: u8,
}

/// VRAM at power-on: the board's arbitrary bytes, from its 32-bit xorshift
/// with seed 2 (`arbitrary` in `hdl/board/gameboy.vhd`).
#[must_use]
pub fn vram_power_on() -> Box<[u8; 0x2000]> {
    let mut x: u32 = 2;
    let mut vram = Box::new([0; 0x2000]);
    for byte in &mut *vram {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        *byte = x.to_le_bytes()[0];
    }
    vram
}

/// The sprite store (`hdl/rtl/sprite_store.vhd`): the ten sprites the OAM
/// scan found on the line, each with its X, its line and its index in OAM.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct NetSpriteStore {
    /// The index OAM's address had at the last rise of `wuvu`.
    pub scanned: u8,
    /// The slots' X (FF matches no pixel), lines and indexes: latches, open
    /// while the scan saves to them.
    pub xs: [u8; 10],
    pub lines: [u8; 10],
    pub indexes: [u8; 10],
    /// The slots fetched on this line, bit `i` for slot `i`.
    pub fetched: u16,
    /// The index lines: the scanned index outside rendering, the selected
    /// slot's while rendering; they keep their value when nothing drives
    /// them.
    pub index: u8,
}

/// The netlist's render: its registers, flip-flops and latches, and its
/// wires, as the last edge left them.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[expect(clippy::struct_excessive_bools, reason = "the netlist's flip-flops")]
pub struct NetRender {
    /// SCY, SCX, WY, WX, BGP, OBP0, OBP1: latches, open while the CPU writes.
    pub scy: u8,
    pub scx: u8,
    pub wy: u8,
    pub wx: u8,
    pub bgp: u8,
    pub obp0: u8,
    pub obp1: u8,
    /// XYMU: mode 3. VOGA: its end, a clock later.
    pub mode3: bool,
    pub voga: bool,
    /// The pixel counter, and WUSA: it passed 9.
    pub px: u8,
    pub px9: bool,
    /// The fetcher's count (0 to 5), and its flip-flops and latches.
    pub fetch_count: u8,
    pub lyzu: bool,
    pub lovy: bool,
    pub fetching: bool,
    pub nyka: bool,
    pub pory: bool,
    pub pygo: bool,
    /// POKY: the first fetch of the line is done.
    pub started: bool,
    /// The tile register, the low byte's latch, the background shifter.
    pub bg_tile: u8,
    pub bg_low: u8,
    pub bg_shift_a: u8,
    pub bg_shift_b: u8,
    /// The window: WY matched (SARY, and the frame's latch), WX matched
    /// (PYCO, NUNU), in it (and NOPA, a clock later), its column and line,
    /// its first tile not fetched yet (RYDY, and SOVY).
    pub sary: bool,
    pub wy_latch: bool,
    pub pyco: bool,
    pub nunu: bool,
    pub nopa: bool,
    pub in_window: bool,
    pub win_x: u8,
    pub win_y: u8,
    pub rydy: bool,
    pub sovy: bool,
    /// The fine scroll: its count, its match with SCX (PUXA, NYZE), the
    /// pixel counter's bit 3 at a dot (PAHO), it discards (ROXY), a tile's
    /// last dot (RYFA, RENE).
    pub fine: u8,
    pub puxa: bool,
    pub nyze: bool,
    pub paho: bool,
    pub discard: bool,
    pub ryfa: bool,
    pub rene: bool,
    /// RUJU: the line's pixels started (ST is its inverse).
    pub line_started: bool,
    /// The sprite fetch: requested (SOBU, SUDA), its steps, TAKA, and the
    /// steps' flip-flops.
    pub sobu: bool,
    pub suda: bool,
    pub sprite_step: u8,
    pub busy: bool,
    pub tobu: bool,
    pub vonu: bool,
    pub seba: bool,
    pub tyfo: bool,
    /// The sprite's two bytes, and the sprite shifter: its two planes, its
    /// palette and its priority.
    pub sp_low_a: u8,
    pub sp_low_b: u8,
    pub sp_a: u8,
    pub sp_b: u8,
    pub sp_palette: u8,
    pub sp_priority: u8,
    pub store: NetSpriteStore,
    /// The VRAM bus's address lines (they keep their value when nothing
    /// drives them), render's address 2 ns later, and the keeper of the data
    /// pins (FF until something drives them: the pads' pull-ups).
    pub vram_ma: u16,
    pub render_settled: Option<u16>,
    pub md_kept: u8,
    pub wires: Wires,
    /// The inputs as the last delta cycle had them.
    pub inputs: RenderInputs,
}

/// A flip-flop's change, `tco` after its clock's edge.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tx {
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
    SpShift {
        mask: u8,
        a: u8,
        b: u8,
        palette: u8,
        priority: u8,
    },
}

impl Tx {
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
        }
    }
}

/// The changes scheduled for later nanoseconds of an edge.
struct Schedule {
    /// (nanosecond, change), in the order they were made.
    pending: Vec<(u32, Tx)>,
    /// The nanosecond being settled.
    now: u32,
}

impl Schedule {
    /// A flip-flop's change, `tco` from now. As VHDL's inertial delay does,
    /// it replaces a change still pending for the same flip-flop.
    fn after_tco(&mut self, tx: Tx) {
        self.after(1, tx);
    }

    /// A change `ns` from now, replacing one still pending.
    fn after(&mut self, ns: u32, tx: Tx) {
        self.pending.retain(|&(_, p)| p.target() != tx.target());
        self.pending.push((self.now + ns, tx));
    }

    /// A zero-delay assignment to a flip-flop (its reset) cancels what is
    /// pending for it.
    fn cancel(&mut self, target: Tx) {
        self.pending.retain(|&(_, p)| p.target() != target.target());
    }
}

/// When the other blocks' changes reach render: with the flip-flops, 1 ns
/// after the edge; VRAM's data 2 ns after the address, which changes with
/// the flip-flops (`render_settled` in `hdl/rtl/vram_bus.vhd`).
const INPUTS_AT: u32 = 1;
/// The OAM scan's flip-flops clocked by `wuvu` (`start_scan`, `anel`,
/// `ceno`, the index, `byba`), which changes 1 ns after the edge: what they
/// make (the line's start, `ceno`, the index, the stop's rise) reaches
/// render 2 ns after the edge.
const SCAN_AT: u32 = 2;

/// The deltas an edge may take before it is taken to oscillate.
const MAX_DELTAS: usize = 1000;

const fn rose(before: bool, now: bool) -> bool {
    !before && now
}

const fn fell(before: bool, now: bool) -> bool {
    before && !now
}

impl NetRender {
    /// An edge of the PPU's clock: `inputs` are what render takes in after
    /// it (the clock among them), which reach it 1 ns after the edge, the
    /// clock at once.
    ///
    /// # Panics
    ///
    /// If render does not settle (a loop of its wires oscillates).
    pub fn edge(&mut self, inputs: RenderInputs, vram: &mut [u8; 0x2000]) {
        let mut schedule = Schedule {
            pending: Vec::new(),
            now: 0,
        };
        let mut prev = *self;
        self.inputs.clk = inputs.clk;
        let mut deltas = 0;
        loop {
            // The deltas of this nanosecond, until nothing changes.
            loop {
                let next = self.delta(&prev, &mut schedule, vram);
                prev = *self;
                if next == *self {
                    break;
                }
                *self = next;
                deltas += 1;
                assert!(deltas < MAX_DELTAS, "render oscillates");
            }
            // The next nanosecond: the inputs at 1 ns, and the flip-flops.
            let inputs_due = [INPUTS_AT, SCAN_AT]
                .into_iter()
                .find(|&at| at > schedule.now);
            let Some(at) = schedule
                .pending
                .iter()
                .map(|&(at, _)| at)
                .chain(inputs_due)
                .min()
            else {
                break;
            };
            schedule.now = at;
            prev = *self;
            let was = self.inputs;
            if at == INPUTS_AT {
                self.inputs = RenderInputs {
                    line_start: was.line_start,
                    ceno: was.ceno,
                    scan_index: was.scan_index,
                    // stop_scan falls with doba, rises with byba.
                    stop_scan: was.stop_scan && inputs.stop_scan,
                    ..inputs
                };
            }
            if at == SCAN_AT {
                self.inputs.line_start = inputs.line_start;
                self.inputs.ceno = inputs.ceno;
                self.inputs.scan_index = inputs.scan_index;
                self.inputs.stop_scan = inputs.stop_scan;
            }
            let (due, later): (Vec<_>, Vec<_>) =
                schedule.pending.iter().partition(|&&(when, _)| when == at);
            schedule.pending = later;
            for (_, tx) in due {
                self.apply(tx);
            }
        }
    }

    /// A scheduled change takes effect.
    const fn apply(&mut self, tx: Tx) {
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

    /// The wires a delta cycle on: each concurrent statement from what it
    /// is made of now.
    #[must_use]
    pub fn next_wires(&self, vram: &[u8; 0x2000]) -> Wires {
        let cur = self;
        let (i, w, store) = (&cur.inputs, &cur.wires, &cur.store);
        let count0 = cur.fetch_count & 1 != 0;
        let count1 = cur.fetch_count & 2 != 0;
        let count2 = cur.fetch_count & 4 != 0;
        let mut wires = *w;
        let n = &mut wires;
        // The wires.
        // @hw die: WODU XUGU XENA, TYFA SOCY VYBO SEGU, SACU
        n.stop_mode3 = cur.px == 167 && !w.x_match;
        n.dot = !cur.rydy && cur.started && !w.x_match && !w.stop_mode3 && i.clk;
        n.segu = !w.dot;
        n.clkpipe = w.segu || cur.discard;
        // @hw die: LEBO MOCE, NYXU; MYSO, XUHA; MOFU
        n.fetch_reset = i.stop_scan || w.win_start || w.restart;
        n.fetch_done = count2 && count0 && !w.fetch_reset;
        // not (clk and not fetch_done)
        n.fetch_clock = !i.clk || w.fetch_done;
        n.myso = cur.mode3 && count0 && !cur.lyzu;
        n.bg_tile_clock = w.myso && !count1;
        // @hw die: PAFU ROGE, NOGY NUKO, XOFO, NUNY, VETU
        n.wy_match = i.lcdc & 0x20 != 0 && cur.wy == i.ly;
        n.wxy_match = cur.wy_latch && cur.wx == cur.px;
        n.win_reset = i.lcdc & 0x20 == 0 || i.line_start;
        n.win_start = cur.in_window && !cur.nopa;
        n.wx_clk = w.restart && cur.in_window;
        // @hw die: PASO, POVA, TEVO
        n.fine_reset = !cur.mode3 || w.restart;
        n.fine_done = cur.puxa && !cur.nyze;
        n.restart = (cur.ryfa && !cur.rene)
            || (cur.sovy && !cur.rydy)
            || (cur.mode3 && !cur.started && cur.nyka && cur.pory);
        // @hw die: RYCE TEKY, SECA; SYCU TUVO, WUTY, TEXY; XONO
        n.fetch_start = cur.sobu && !cur.suda;
        n.step_reset = w.fetch_start || i.ppu_reset || i.line_start;
        n.sycu = cur.sprite_step & 1 != 0 && cur.mode3 && !cur.tyfo;
        n.loaded = cur.tyfo && cur.sprite_step & 1 != 0 && cur.seba && cur.vonu;
        n.sprite_cycles = cur.mode3 && (cur.sprite_step & 2 != 0 || cur.vonu);
        n.sp_data = if i.attributes & 0x20 != 0 && w.sprite_cycles {
            w.md.reverse_bits()
        } else {
            w.md
        };
        n.sprite_on = if w.loaded { !(cur.sp_a | cur.sp_b) } else { 0 };

        // The sprite store's.
        n.rendering = cur.mode3 && !i.ceno;
        n.enabled = w.rendering && i.lcdc & 2 != 0;
        n.matches = (0..10)
            .filter(|&slot| w.enabled && store.xs[slot] == cur.px)
            .fold(0, |m, slot| m | 1 << slot);
        n.selected = w.matches & w.matches.wrapping_neg();
        n.x_match = w.matches != 0;
        n.line = (0..10)
            .find(|&slot| w.selected & 1 << slot != 0)
            .map_or(i.scan_line, |slot| store.lines[slot]);

        // OAM's address, bits 7 to 2: the CPU's, the DMA's, the scan's, or
        // the store's index lines in mode 3.
        n.oam_index = if !i.mode2 && !cur.mode3 && !i.dma_run {
            (i.addr >> 2 & 0x3F) as u8
        } else if i.dma_run {
            i.dma_index >> 2
        } else if i.mode2 {
            i.scan_index
        } else {
            store.index
        };
        // The VRAM bus's, and the board's VRAM.
        // @hw die: TEFA SOSE TUCA TOLE ROPY SERE, TEGU SALE RUVY SAZO; SUTU
        // TODE SOKY ROSU; RYLU..RONU; TUJA..RAGU
        n.render_ma = cur.ma();
        let fexx_ffxx = i.addr >> 9 == 0x7F;
        let sose = i.addr & 0x8000 != 0 && !fexx_ffxx && !i.non_vram_mreq;
        n.cpu_vram = sose && i.cpu_ext_cs_en && !cur.mode3;
        n.cpu_write = w.cpu_vram && sose && i.write;
        let sprite_mrd = w.sprite_cycles && !(cur.sprite_step & 1 == 0 && cur.tyfo);
        n.mcs_n = !(cur.fetching || i.vram_to_oam || w.sprite_cycles || w.cpu_vram);
        // (not (sose and write) nand not mode3) and not sprite_mrd and ...
        n.mrd_n =
            ((sose && i.write) || cur.mode3) && !sprite_mrd && !cur.fetching && !i.vram_to_oam;
        n.mwr_n = !(sose && i.cpu_wr_sync && w.cpu_vram);
        let vram_drives = !w.mcs_n && !w.mrd_n && w.mwr_n;
        n.md_driven = w.cpu_write || vram_drives;
        // md_pins <= to_x01(md) when cpu_write: the bus's own md, a delta
        // behind the CPU's data bus.
        n.md_pins = if w.cpu_write {
            w.md
        } else if vram_drives {
            vram[usize::from(cur.vram_ma)]
        } else {
            cur.md_kept
        };
        n.md = if w.cpu_write { i.data } else { w.md_pins };
        wires
    }

    /// The VRAM bus's delta cycle (`hdl/rtl/vram_bus.vhd`) and the board's
    /// VRAM's: the address lines, render's address 2 ns later, the keeper,
    /// and VRAM written while /MCS and /MWR are low.
    // @hw die: XANE XEDU, AHOC, and the not_if0 onto ~ma0..~ma12
    fn vram_delta(
        &self,
        prev: &Self,
        next: &mut Self,
        schedule: &mut Schedule,
        vram: &mut [u8; 0x2000],
    ) {
        let (cur, i, w) = (self, &self.inputs, &self.wires);
        if !i.vram_to_oam && !cur.mode3 {
            next.vram_ma = i.addr & 0x1FFF;
        } else if i.vram_to_oam {
            next.vram_ma = i.dma_addr & 0x1FFF;
        } else if let Some(ma) = cur.render_settled {
            next.vram_ma = ma;
        } else {
            // Nobody drives the lines: they keep their value.
        }
        if w.render_ma != prev.wires.render_ma {
            schedule.after(2, Tx::RenderSettled(w.render_ma));
        }
        if w.md_driven {
            next.md_kept = w.md_pins;
        }
        if !w.mcs_n && !w.mwr_n {
            vram[usize::from(cur.vram_ma)] = w.md_pins;
        }
    }

    /// What a CPU read of VRAM takes, in G and H.
    // @hw die: TYVY SEBY, RUGA..TOKU (not_if1)
    #[must_use]
    pub const fn read_vram(&self) -> Option<u8> {
        if self.wires.cpu_vram && self.inputs.rd && self.inputs.clk_t4 {
            Some(self.wires.md)
        } else {
            None
        }
    }

    /// The wires settled from the flip-flops and latches as they are,
    /// without an edge: after the state was set from outside (a replay's
    /// resynchronisation).
    ///
    /// # Panics
    ///
    /// If the wires do not settle.
    pub fn settle_wires(&mut self, vram: &[u8; 0x2000]) {
        let mut deltas = 0;
        loop {
            let wires = self.next_wires(vram);
            if wires == self.wires {
                break;
            }
            self.wires = wires;
            deltas += 1;
            assert!(deltas < MAX_DELTAS, "render's wires oscillate");
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
    fn delta(&self, prev: &Self, schedule: &mut Schedule, vram: &mut [u8; 0x2000]) -> Self {
        let cur = self;
        let (i, w, pw) = (&cur.inputs, &cur.wires, &prev.wires);
        let mut next = *cur;
        let count0 = cur.fetch_count & 1 != 0;
        let (clk_rose, clk_fell) = (rose(prev.inputs.clk, i.clk), fell(prev.inputs.clk, i.clk));

        next.wires = self.next_wires(vram);

        // The registers: latches, open while the CPU writes them.
        // @hw die: GAVE..FUNY, DATY..BAKE, NESO..NAFU, MYPA..NUKU, PAVO..MENA,
        // XUFU..XANA, MOXY..LUXO
        if i.hard_reset {
            next.scy = 0;
            next.scx = 0;
            next.wy = 0;
            next.wx = 0;
        } else if i.wr {
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
        if i.wr {
            match i.addr {
                0xFF47 => next.bgp = i.data,
                0xFF48 => next.obp0 = i.data,
                0xFF49 => next.obp1 = i.data,
                _ => {}
            }
        }

        // Mode 3: set a nanosecond after the scan stops.
        // @hw die: XYMU (nor_latch); VOGA (dffr)
        if i.ppu_reset || cur.voga {
            next.mode3 = false;
            schedule.cancel(Tx::Mode3(false));
        } else if i.stop_scan
            && !cur.mode3
            && !schedule.pending.iter().any(|&(_, p)| p == Tx::Mode3(true))
        {
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
        if i.ppu_reset || cur.voga {
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
        } else if cur.lovy || !cur.mode3 {
            next.fetching = false;
        } else {
            // The latch holds.
        }

        // The first fetch of the line.
        // @hw die: NYKA PORY PYGO (dffr), POKY (nor_latch)
        if !cur.mode3 || w.win_start {
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
        if w.myso && cur.fetch_count == 3 {
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
        if i.mode1 || i.ppu_reset {
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
        if i.mode1 || i.ppu_reset {
            next.win_y = 0;
            schedule.cancel(Tx::WinY(0));
        } else if fell(prev.in_window, cur.in_window) {
            schedule.after_tco(Tx::WinY(cur.win_y.wrapping_add(1)));
        } else {
            // The counter holds.
        }
        if cur.pory || i.ppu_reset {
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
        } else if fell(pw.dot, w.dot) && cur.fine != 7 {
            schedule.after_tco(Tx::Fine(cur.fine + 1));
        } else {
            // The counter holds.
        }
        if cur.mode3 {
            if rose(pw.dot, w.dot) {
                schedule.after_tco(Tx::Puxa(cur.discard && cur.fine == cur.scx & 7));
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
                schedule.after_tco(Tx::Ryfa(cur.fine == 7 && !w.wxy_match));
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
        if cur.paho || i.ppu_reset {
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
            schedule.after_tco(Tx::Sobu(
                w.x_match && !cur.rydy && w.fetch_done && !cur.busy,
            ));
        }
        if clk_fell {
            schedule.after_tco(Tx::Suda(cur.sobu));
        }
        if w.step_reset {
            next.sprite_step = 0;
            schedule.cancel(Tx::SpriteStep(0));
        } else if clk_rose && cur.sprite_step != 5 {
            schedule.after_tco(Tx::SpriteStep(cur.sprite_step + 1));
        } else {
            // The counter holds.
        }
        if w.step_reset {
            next.busy = true;
        } else if w.loaded || (cur.mode3 && !cur.started && cur.nyka && cur.pory) {
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
            schedule.after_tco(Tx::Seba(cur.mode3 && cur.vonu));
        }

        // The sprite's two bytes.
        // @hw die: PEFO..RYDU, REWO..SEGA (dlatch_ee_q)
        if cur.vonu && w.sycu {
            next.sp_low_a = w.sp_data;
        }
        if cur.sprite_step & 2 != 0 && w.sycu {
            next.sp_low_b = w.sp_data;
        }

        // The sprite shifter: the fetched sprite loads its free pixels.
        // @hw die: NYLU..VUPY, NURO..WUFY (dffsr)
        let on = w.sprite_on;
        if on != 0 {
            let palette = if i.attributes & 0x10 != 0 { 0xFF } else { 0 };
            let priority = if i.attributes & 0x80 != 0 { 0xFF } else { 0 };
            next.sp_a = cur.sp_a & !on | cur.sp_low_a & on;
            next.sp_b = cur.sp_b & !on | cur.sp_low_b & on;
            next.sp_palette = cur.sp_palette & !on | palette & on;
            next.sp_priority = cur.sp_priority & !on | priority & on;
        }
        if rose(pw.clkpipe, w.clkpipe) && on != 0xFF {
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
        next
    }

    /// The sprite store's delta cycle (`hdl/rtl/sprite_store.vhd`).
    // @hw die: XADU..XECU (dffr_cc); the drlatch_ee of the X, the dlatch_ee
    // of the line and the index; EBOJ..FONO (dffr); the X matchers, GUVA..
    // GUZE and GEZE..GYGA; FEFY FOVE FEPO; WUZY..WEZA
    fn store_delta(&self, prev: &Self, next: &mut Self, schedule: &mut Schedule) {
        let cur = self;
        let (i, w, store) = (&cur.inputs, &cur.wires, &cur.store);
        if rose(prev.inputs.wuvu, i.wuvu) {
            schedule.after_tco(Tx::Scanned(w.oam_index));
        }
        for slot in 0..10 {
            if i.line_start || store.fetched & 1 << slot != 0 {
                next.store.xs[slot] = 0xFF;
            } else if i.save & 1 << slot != 0 {
                next.store.xs[slot] = i.attributes;
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
        } else if let Some(slot) = (0..10).find(|&slot| w.selected & 1 << slot != 0) {
            next.store.index = store.indexes[slot];
        } else {
            // The lines keep their value.
        }
    }

    /// What a read of `addr` drives: SCY, SCX, BGP, OBP0, OBP1, WY, WX.
    // @hw die: the not_if0 of the registers onto the data bus
    #[must_use]
    pub const fn read(&self, addr: u16) -> Option<u8> {
        match addr {
            0xFF42 => Some(self.scy),
            0xFF43 => Some(self.scx),
            0xFF47 => Some(self.bgp),
            0xFF48 => Some(self.obp0),
            0xFF49 => Some(self.obp1),
            0xFF4A => Some(self.wy),
            0xFF4B => Some(self.wx),
            _ => None,
        }
    }

    /// Mode 0 as it is once the edge settles (render.vhd raises it 14 ns
    /// after mode 3 ends, within the phase).
    #[must_use]
    pub const fn mode0(&self) -> bool {
        self.wires.stop_mode3 && !self.inputs.mode1
    }

    /// The sprite fetch reads OAM, and holds the second latches.
    // @hw die: TUVO WEFY TACU VAPE, SAKY TYSO
    #[must_use]
    pub const fn fetch_read(&self) -> bool {
        self.mode3 && self.sprite_step & 6 == 0 && !self.tyfo
    }

    #[must_use]
    pub const fn fetch_hold(&self) -> bool {
        self.mode3 && self.sprite_step & 6 == 0 && !(self.sprite_step & 1 == 0 && self.tyfo)
    }

    /// The VRAM address of a fetch, if one is under way.
    // @hw die: the full adders of ppu-bgscroll, the window's, the tile's,
    // the sprite's
    #[must_use]
    pub const fn ma(&self) -> Option<u16> {
        let i = &self.inputs;
        let map_x = self.px.wrapping_add(self.scx);
        let map_y = i.ly.wrapping_add(self.scy);
        let count12 = self.fetch_count & 6 != 0;
        let tile_map_cycle = self.fetching && !count12;
        let bit_plane_cycle = self.fetching && count12;
        if tile_map_cycle && !self.in_window {
            Some(
                0x1800
                    | ((i.lcdc >> 3 & 1) as u16) << 10
                    | ((map_y >> 3) as u16) << 5
                    | (map_x >> 3) as u16,
            )
        } else if tile_map_cycle {
            Some(
                0x1800
                    | ((i.lcdc >> 6 & 1) as u16) << 10
                    | ((self.win_y >> 3) as u16) << 5
                    | self.win_x as u16,
            )
        } else if bit_plane_cycle && !self.in_window {
            Some(self.tile_row(map_y))
        } else if bit_plane_cycle {
            Some(self.tile_row(self.win_y))
        } else if self.wires.sprite_cycles {
            // The stored line counts from the bottom: flipped in Y, it is
            // the row; otherwise its inverse is.
            let row = self.wires.line ^ if i.attributes & 0x40 != 0 { 0 } else { 0x0F };
            let tile = if i.lcdc & 4 != 0 {
                i.tile & 0xFE | row >> 3 & 1
            } else {
                i.tile
            };
            Some((tile as u16) << 4 | ((row & 7) as u16) << 1 | self.vonu as u16)
        } else {
            None
        }
    }

    /// A bit plane's address: the tile, its row, the fetcher's plane.
    const fn tile_row(&self, row: u8) -> u16 {
        let high = self.bg_tile & 0x80 == 0 && self.inputs.lcdc & 0x10 == 0;
        (high as u16) << 12
            | (self.bg_tile as u16) << 4
            | ((row & 7) as u16) << 1
            | (self.fetch_count >> 2 & 1) as u16
    }

    /// The LCD's pins: CP (the pixel clock), ST (not the line's start), LD
    /// (the pixel's shade).
    // @hw die: TOBA SEMU RYPO CP; RUJU POME POFY RUZE ST; WOXA XULA NULY,
    // RAJY TADE, RUTA RYFU POKA, ..., PERO PATY REMY RAVO LD0 LD1
    #[must_use]
    pub const fn pins(&self) -> (bool, bool, u8) {
        let lcdc = self.inputs.lcdc;
        let bg_a = self.bg_shift_a & 0x80 != 0 && lcdc & 1 != 0;
        let bg_b = self.bg_shift_b & 0x80 != 0 && lcdc & 1 != 0;
        let obj_a = self.sp_a & 0x80 != 0 && lcdc & 2 != 0;
        let obj_b = self.sp_b & 0x80 != 0 && lcdc & 2 != 0;
        let show_sprite = (obj_a || obj_b) && !(self.sp_priority & 0x80 != 0 && (bg_a || bg_b));
        let pixel = if show_sprite {
            (obj_a as u8) << 1 | obj_b as u8
        } else {
            (bg_b as u8) << 1 | bg_a as u8
        };
        let palette = if !show_sprite {
            self.bgp
        } else if self.sp_palette & 0x80 != 0 {
            self.obp1
        } else {
            self.obp0
        };
        let cp = (self.px9 && self.wires.clkpipe) || self.wires.fine_done;
        (cp, !self.line_started, palette >> (2 * pixel) & 3)
    }
}
