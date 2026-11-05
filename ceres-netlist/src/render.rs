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
//! OAM (`crate::oam::NetOam`), the VRAM bus and the board's VRAM settle
//! with render, as they feed each other within an edge. What they take
//! from the other blocks reaches them at its time: the clocks decoded from
//! the phase with the edge; the line timing's, the DMA's and LCDC's
//! flip-flops 1 ns after it; the scan's flip-flops clocked by `wuvu` (the
//! line's start, `ceno`, `scanning`, the stop's rise) and the data pins at
//! 2 ns; the scan's index at 3 ns, a port behind mode 2; the CPU's address,
//! strobes and data bus at 4 ns, after what drives them. VRAM's data
//! follows its address 2 ns later (`render_settled`).

use crate::oam::{NetOam, NetOamInputs};

/// What render takes in.
#[derive(Clone, Copy, Default)]
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
    /// The CPU's write: the address and the data bus; and the data bus
    /// while render's own mode 3 is high (STAT's mode bits, in a read of
    /// it), which OAM's data lines follow.
    pub addr: u16,
    pub wr: bool,
    pub data: u8,
    pub data_mode3: u8,
    /// From the OAM scan: the line starts, the scan stops.
    pub line_start: bool,
    pub stop_scan: bool,
    /// The OAM scan's `ceno` (the scan's last read is under way) and save
    /// strobes (bit `i` for slot `i`), and the Y comparator's line.
    pub ceno: bool,
    pub save: u16,
    pub scan_line: u8,
    /// The line timing's `wuvu`, which clocks the sprite store's index, and
    /// `wosu`, for OAM.
    pub wuvu: bool,
    pub wosu: bool,
    /// What addresses OAM, besides the store: the DMA's index while it
    /// runs, the scan's in mode 2, the CPU's (its address's bits 7 to 2)
    /// otherwise.
    pub dma_run: bool,
    pub dma_index: u8,
    /// The scan runs: mode 2, unless OAM DMA hides OAM from it.
    pub scanning: bool,
    pub scan_index: u8,
    /// For OAM: the DMA's clock, whether it reads the cartridge bus, and
    /// the data pins.
    pub dma_phi: bool,
    pub dma_addr_ext: bool,
    pub d: u8,
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

impl RenderInputs {
    /// What render takes at power-on: all low.
    pub const POWER_ON: Self = Self {
        clk: false,
        ppu_reset: false,
        hard_reset: false,
        hclk: false,
        mode1: false,
        ly: 0,
        lcdc: 0,
        addr: 0,
        wr: false,
        data: 0,
        data_mode3: 0,
        line_start: false,
        stop_scan: false,
        ceno: false,
        save: 0,
        scan_line: 0,
        wuvu: false,
        wosu: false,
        dma_run: false,
        dma_index: 0,
        scanning: false,
        scan_index: 0,
        dma_phi: false,
        dma_addr_ext: false,
        d: 0,
        rd: false,
        non_vram_mreq: false,
        write: false,
        cpu_wr_sync: false,
        cpu_ext_cs_en: false,
        clk_t4: false,
        vram_to_oam: false,
        dma_addr: 0,
    };
}

flat_eq!(RenderInputs {
    clk,
    ppu_reset,
    hard_reset,
    hclk,
    mode1,
    ly,
    lcdc,
    addr,
    wr,
    data,
    data_mode3,
    line_start,
    stop_scan,
    ceno,
    save,
    scan_line,
    wuvu,
    wosu,
    dma_run,
    dma_index,
    scanning,
    scan_index,
    dma_phi,
    dma_addr_ext,
    d,
    rd,
    non_vram_mreq,
    write,
    cpu_wr_sync,
    cpu_ext_cs_en,
    clk_t4,
    vram_to_oam,
    dma_addr,
});

/// The wires: render's concurrent statements, each a delta cycle behind
/// what it is made of.
#[derive(Clone, Copy, Default)]
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
    /// Mode 2: the scan runs and OAM DMA does not hide OAM from it.
    pub mode2: bool,
    /// The scan's index inside it (`index_i`), which its line start
    /// resets a delta after it starts the scan, and as it leaves it
    /// through its port (`index`), a delta later: OAM's address sees mode 2
    /// a delta before the index (`hdl/rtl/oam_scan.vhd`).
    pub index_i: u8,
    pub scan_index: u8,
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
    /// What drives OAM's data lines, as last scheduled.
    pub oam_driver: Option<(u8, u8)>,
}

impl Wires {
    /// Render's wires at power-on: all low, nothing driving.
    pub const POWER_ON: Self = Self {
        stop_mode3: false,
        dot: false,
        segu: false,
        clkpipe: false,
        fetch_reset: false,
        fetch_done: false,
        fetch_clock: false,
        myso: false,
        bg_tile_clock: false,
        wy_match: false,
        wxy_match: false,
        win_reset: false,
        win_start: false,
        wx_clk: false,
        fine_reset: false,
        fine_done: false,
        restart: false,
        fetch_start: false,
        step_reset: false,
        sycu: false,
        loaded: false,
        sprite_cycles: false,
        sp_data: 0,
        sprite_on: 0,
        rendering: false,
        enabled: false,
        matches: 0,
        selected: 0,
        x_match: false,
        line: 0,
        oam_index: 0,
        mode2: false,
        index_i: 0,
        scan_index: 0,
        render_ma: None,
        cpu_vram: false,
        cpu_write: false,
        mcs_n: false,
        mrd_n: false,
        mwr_n: false,
        md_pins: 0,
        md_driven: false,
        md: 0,
        oam_driver: None,
    };
}

flat_eq!(Wires {
    stop_mode3,
    dot,
    segu,
    clkpipe,
    fetch_reset,
    fetch_done,
    fetch_clock,
    myso,
    bg_tile_clock,
    wy_match,
    wxy_match,
    win_reset,
    win_start,
    wx_clk,
    fine_reset,
    fine_done,
    restart,
    fetch_start,
    step_reset,
    sycu,
    loaded,
    sprite_cycles,
    sp_data,
    sprite_on,
    rendering,
    enabled,
    matches,
    selected,
    x_match,
    line,
    oam_index,
    mode2,
    index_i,
    scan_index,
    render_ma => same_address,
    cpu_vram,
    cpu_write,
    mcs_n,
    mrd_n,
    mwr_n,
    md_pins,
    md_driven,
    md,
    oam_driver => same_drivers,
});

/// VRAM at power-on: the board's arbitrary bytes, from its 32-bit xorshift
/// with seed 2 (`arbitrary` in `hdl/board/gameboy.vhd`).
#[must_use]
#[expect(clippy::cast_possible_truncation, reason = "the low byte")]
pub fn vram_power_on() -> Box<[u8; 0x2000]> {
    let mut x: u32 = 2;
    let mut vram = Box::new([0; 0x2000]);
    for i in 0..vram.len() {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        vram[i] = x as u8;
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

impl NetSpriteStore {
    /// The sprite store at power-on: empty.
    pub const POWER_ON: Self = Self {
        scanned: 0,
        xs: [0; 10],
        lines: [0; 10],
        indexes: [0; 10],
        fetched: 0,
        index: 0,
    };
}

/// The netlist's render: its registers, flip-flops and latches, and its
/// wires, as the last edge left them.
#[derive(Clone, Copy, Default)]
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
    /// OAM, which settles with render: the sprite's tile and attributes are
    /// its latches.
    pub oam: NetOam,
    pub wires: Wires,
    /// The inputs as the last delta cycle had them.
    pub inputs: RenderInputs,
}

flat_eq!(NetRender {
    scy,
    scx,
    wy,
    wx,
    bgp,
    obp0,
    obp1,
    mode3,
    voga,
    px,
    px9,
    fetch_count,
    lyzu,
    lovy,
    fetching,
    nyka,
    pory,
    pygo,
    started,
    bg_tile,
    bg_low,
    bg_shift_a,
    bg_shift_b,
    sary,
    wy_latch,
    pyco,
    nunu,
    nopa,
    in_window,
    win_x,
    win_y,
    rydy,
    sovy,
    fine,
    puxa,
    nyze,
    paho,
    discard,
    ryfa,
    rene,
    line_started,
    sobu,
    suda,
    sprite_step,
    busy,
    tobu,
    vonu,
    seba,
    tyfo,
    sp_low_a,
    sp_low_b,
    sp_a,
    sp_b,
    sp_palette,
    sp_priority,
    store,
    vram_ma,
    render_settled => same_address,
    md_kept,
    oam,
    wires,
    inputs,
});

/// A flip-flop's change, `tco` after its clock's edge.
#[derive(Clone, Copy)]
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
    OamBus(u8, u8),
    Maka(bool),
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
        }
    }
}

/// How many flip-flops (`Tx::target`) a change can be pending for.
const TARGETS: usize = 38;

/// No change is pending for a target.
const NEVER: u32 = u32::MAX;

/// The changes scheduled for later nanoseconds of an edge.
struct Schedule {
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
const fn flipped(byte: u8) -> u8 {
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
const fn same_address(a: Option<u16>, b: Option<u16>) -> bool {
    match (a, b) {
        (Some(x), Some(y)) => x == y,
        (None, None) => true,
        _ => false,
    }
}

/// The same bytes on OAM's two data buses, or both undriven.
const fn same_drivers(a: Option<(u8, u8)>, b: Option<(u8, u8)>) -> bool {
    match (a, b) {
        (Some((a_even, a_odd)), Some((b_even, b_odd))) => (a_even == b_even) & (a_odd == b_odd),
        (None, None) => true,
        _ => false,
    }
}

/// The lowest of the sprite store's slots whose bit is set in `bits`.
const fn first_slot(bits: u16) -> Option<usize> {
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
const INPUTS_AT: u32 = 1;
/// The OAM scan's flip-flops clocked by `wuvu` (`start_scan`, `anel`,
/// `ceno`, the index, `byba`), which changes 1 ns after the edge: what they
/// make (the line's start, `ceno`, the index, the stop's rise) reaches
/// render 2 ns after the edge.
const SCAN_AT: u32 = 2;
/// VBlank (`mode1`): a flip-flop clocked by `nype`, itself a flip-flop
/// clocked by hclk, which changes 1 ns after the edge (`hdl/rtl/lcd_timing.vhd`).
/// So it changes after SARY, which hclk clocks: as a frame starts, WY's
/// latch sees SARY's new level before VBlank ends.
const MODE1_AT: u32 = 3;
/// The CPU's access: its address, strobes and data bus, which settles
/// after what drives it (a register read takes the modes as they change).
const DATA_AT: u32 = 4;
/// The data pins, which follow OAM DMA's address (its count changes 1 ns
/// after `dma_phi` rises) through the cartridge bus.
const PINS_AT: u32 = 2;

/// The deltas an edge may take before it is taken to oscillate.
const MAX_DELTAS: usize = 1000;

const fn rose(before: bool, now: bool) -> bool {
    !before & now
}

const fn fell(before: bool, now: bool) -> bool {
    before & !now
}

impl NetRender {
    /// Render, the sprite store and OAM at power-on: the data pins' keeper
    /// high (the pads' pull-ups), OAM's cells and latches FF, the rest 0.
    #[must_use]
    pub fn power_on() -> Self {
        Self {
            scy: 0,
            scx: 0,
            wy: 0,
            wx: 0,
            bgp: 0,
            obp0: 0,
            obp1: 0,
            mode3: false,
            voga: false,
            px: 0,
            px9: false,
            fetch_count: 0,
            lyzu: false,
            lovy: false,
            fetching: false,
            nyka: false,
            pory: false,
            pygo: false,
            started: false,
            bg_tile: 0,
            bg_low: 0,
            bg_shift_a: 0,
            bg_shift_b: 0,
            sary: false,
            wy_latch: false,
            pyco: false,
            nunu: false,
            nopa: false,
            in_window: false,
            win_x: 0,
            win_y: 0,
            rydy: false,
            sovy: false,
            fine: 0,
            puxa: false,
            nyze: false,
            paho: false,
            discard: false,
            ryfa: false,
            rene: false,
            line_started: false,
            sobu: false,
            suda: false,
            sprite_step: 0,
            busy: false,
            tobu: false,
            vonu: false,
            seba: false,
            tyfo: false,
            sp_low_a: 0,
            sp_low_b: 0,
            sp_a: 0,
            sp_b: 0,
            sp_palette: 0,
            sp_priority: 0,
            store: NetSpriteStore::POWER_ON,
            vram_ma: 0,
            render_settled: None,
            md_kept: 0xFF,
            oam: NetOam::power_on(),
            wires: Wires::POWER_ON,
            inputs: RenderInputs::POWER_ON,
        }
    }

    /// An edge of the PPU's clock: `inputs` are what render takes in after
    /// it (the clock among them), which reach it 1 ns after the edge, the
    /// clock at once.
    ///
    /// # Panics
    ///
    /// If render does not settle (a loop of its wires oscillates).
    pub fn edge(&mut self, inputs: RenderInputs, vram: &mut [u8; 0x2000]) {
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
            Tx::OamBus(even, odd) => {
                self.oam.bus_even = even;
                self.oam.bus_odd = odd;
            }
            Tx::Maka(v) => self.oam.maka = v,
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
        n.stop_mode3 = (cur.px == 167) & !w.x_match;
        n.dot = !cur.rydy & cur.started & !w.x_match & !w.stop_mode3 & i.clk;
        n.segu = !w.dot;
        n.clkpipe = w.segu | cur.discard;
        // @hw die: LEBO MOCE, NYXU; MYSO, XUHA; MOFU
        n.fetch_reset = i.stop_scan | w.win_start | w.restart;
        n.fetch_done = count2 & count0 & !w.fetch_reset;
        // not (clk and not fetch_done)
        n.fetch_clock = !i.clk | w.fetch_done;
        n.myso = cur.mode3 & count0 & !cur.lyzu;
        n.bg_tile_clock = w.myso & !count1;
        // @hw die: PAFU ROGE, NOGY NUKO, XOFO, NUNY, VETU
        n.wy_match = (i.lcdc & 0x20 != 0) & (cur.wy == i.ly);
        n.wxy_match = cur.wy_latch & (cur.wx == cur.px);
        n.win_reset = (i.lcdc & 0x20 == 0) | i.line_start;
        n.win_start = cur.in_window & !cur.nopa;
        n.wx_clk = w.restart & cur.in_window;
        // @hw die: PASO, POVA, TEVO
        n.fine_reset = !cur.mode3 | w.restart;
        n.fine_done = cur.puxa & !cur.nyze;
        n.restart = (cur.ryfa & !cur.rene)
            | (cur.sovy & !cur.rydy)
            | (cur.mode3 & !cur.started & cur.nyka & cur.pory);
        // @hw die: RYCE TEKY, SECA; SYCU TUVO, WUTY, TEXY; XONO
        n.fetch_start = cur.sobu & !cur.suda;
        n.step_reset = w.fetch_start | i.ppu_reset | i.line_start;
        n.sycu = (cur.sprite_step & 1 != 0) & cur.mode3 & !cur.tyfo;
        n.loaded = cur.tyfo & (cur.sprite_step & 1 != 0) & cur.seba & cur.vonu;
        n.sprite_cycles = cur.mode3 & ((cur.sprite_step & 2 != 0) | cur.vonu);
        n.sp_data = if (cur.oam.second_odd & 0x20 != 0) & w.sprite_cycles {
            flipped(w.md)
        } else {
            w.md
        };
        n.sprite_on = if w.loaded { !(cur.sp_a | cur.sp_b) } else { 0 };

        // @hw die: ACYL BOGE
        n.mode2 = i.scanning & !i.dma_run;
        // The index: its count arrives with the scan's other outputs; as
        // they start a line, it is reset a delta after `scanning` rises.
        n.index_i = if i.line_start { 0 } else { i.scan_index };
        n.scan_index = w.index_i;

        // The sprite store's.
        n.rendering = cur.mode3 & !i.ceno;
        n.enabled = w.rendering & (i.lcdc & 2 != 0);
        n.matches = 0;
        for slot in 0..10 {
            if w.enabled & (store.xs[slot] == cur.px) {
                n.matches |= 1 << slot;
            }
        }
        // The lowest slot that matches.
        n.selected = w.matches & (!w.matches).wrapping_add(1);
        n.x_match = w.matches != 0;
        n.line = match first_slot(w.selected) {
            Some(slot) => store.lines[slot],
            None => i.scan_line,
        };

        // OAM's address, bits 7 to 2: the CPU's, the DMA's, the scan's, or
        // the store's index lines in mode 3.
        n.oam_index = if !w.mode2 & !cur.mode3 & !i.dma_run {
            (i.addr >> 2 & 0x3F) as u8
        } else if i.dma_run {
            i.dma_index >> 2
        } else if w.mode2 {
            w.scan_index
        } else {
            store.index
        };
        // The VRAM bus's, and the board's VRAM.
        // @hw die: TEFA SOSE TUCA TOLE ROPY SERE, TEGU SALE RUVY SAZO; SUTU
        // TODE SOKY ROSU; RYLU..RONU; TUJA..RAGU
        n.render_ma = cur.ma();
        let fexx_ffxx = i.addr >> 9 == 0x7F;
        let sose = (i.addr & 0x8000 != 0) & !fexx_ffxx & !i.non_vram_mreq;
        n.cpu_vram = sose & i.cpu_ext_cs_en & !cur.mode3;
        n.cpu_write = w.cpu_vram & sose & i.write;
        let sprite_mrd = w.sprite_cycles & !((cur.sprite_step & 1 == 0) & cur.tyfo);
        n.mcs_n = !(cur.fetching | i.vram_to_oam | w.sprite_cycles | w.cpu_vram);
        // (not (sose and write) nand not mode3) and not sprite_mrd and ...
        n.mrd_n = ((sose & i.write) | cur.mode3) & !sprite_mrd & !cur.fetching & !i.vram_to_oam;
        n.mwr_n = !(sose & i.cpu_wr_sync & w.cpu_vram);
        let vram_drives = !w.mcs_n & !w.mrd_n & w.mwr_n;
        n.md_driven = w.cpu_write | vram_drives;
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

    /// What a CPU read of VRAM takes, in G and H.
    // @hw die: TYVY SEBY, RUGA..TOKU (not_if1)
    #[must_use]
    pub const fn read_vram(&self) -> Option<u8> {
        if self.wires.cpu_vram & self.inputs.rd & self.inputs.clk_t4 {
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
        let oam_inputs = self.oam_inputs();
        self.oam.settle_wires(&oam_inputs);
        self.wires.oam_driver = self.oam.bus_drivers(&oam_inputs);
    }

    /// The chip's data bus as OAM's data lines see it: what the other
    /// blocks drive (with STAT's mode bits as render's mode 3 has them),
    /// and VRAM's data in a CPU read of it.
    const fn data_bus(&self) -> u8 {
        let others = if self.mode3 {
            self.inputs.data_mode3
        } else {
            self.inputs.data
        };
        match self.read_vram() {
            Some(byte) => others & byte,
            None => others,
        }
    }

    /// What OAM takes in: the CPU's access, the DMA's, the scan's, and
    /// render's mode 3, sprite fetch, index lines and VRAM's data.
    #[must_use]
    pub const fn oam_inputs(&self) -> NetOamInputs {
        let i = &self.inputs;
        NetOamInputs {
            addr: i.addr,
            rd: i.rd,
            wr: i.wr,
            data: self.data_bus(),
            clk_t4: i.clk_t4,
            dma_run: i.dma_run,
            dma_phi: i.dma_phi,
            dma_index: i.dma_index,
            dma_addr_ext: i.dma_addr_ext,
            vram_to_oam: i.vram_to_oam,
            d: i.d,
            md: self.wires.md,
            mode2: self.wires.mode2,
            mode3: self.mode3,
            scan_index: self.wires.scan_index,
            render_index: self.store.index,
            wosu: i.wosu,
            wuvu: i.wuvu,
            fetch_read: self.fetch_read(),
            fetch_hold: self.fetch_hold(),
        }
    }

    /// OAM's delta cycle: its wires, RAM and latches; MAKA on the clock's
    /// falling edges; the data lines 1 ns after their driver changes (a
    /// later change replaces one still pending, as VHDL's inertial delay
    /// does).
    // @hw die: MAKA (dffr); CEDE AZAR AZUL APAG
    fn oam_delta(&self, prev: &Self, next: &mut Self, schedule: &mut Schedule) {
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
    fn store_delta(&self, prev: &Self, next: &mut Self, schedule: &mut Schedule) {
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
        self.wires.stop_mode3 & !self.inputs.mode1
    }

    /// The sprite fetch reads OAM, and holds the second latches.
    // @hw die: TUVO WEFY TACU VAPE, SAKY TYSO
    #[must_use]
    pub const fn fetch_read(&self) -> bool {
        self.mode3 & (self.sprite_step & 6 == 0) & !self.tyfo
    }

    #[must_use]
    pub const fn fetch_hold(&self) -> bool {
        self.mode3 & (self.sprite_step & 6 == 0) & !((self.sprite_step & 1 == 0) & self.tyfo)
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
        let tile_map_cycle = self.fetching & !count12;
        let bit_plane_cycle = self.fetching & count12;
        if tile_map_cycle & !self.in_window {
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
        } else if bit_plane_cycle & !self.in_window {
            Some(self.tile_row(map_y))
        } else if bit_plane_cycle {
            Some(self.tile_row(self.win_y))
        } else if self.wires.sprite_cycles {
            // The stored line counts from the bottom: flipped in Y, it is
            // the row; otherwise its inverse is.
            let attributes = self.oam.second_odd;
            let row = self.wires.line ^ if attributes & 0x40 != 0 { 0 } else { 0x0F };
            let tile = if i.lcdc & 4 != 0 {
                self.oam.second_even & 0xFE | row >> 3 & 1
            } else {
                self.oam.second_even
            };
            Some((tile as u16) << 4 | ((row & 7) as u16) << 1 | self.vonu as u16)
        } else {
            None
        }
    }

    /// A bit plane's address: the tile, its row, the fetcher's plane.
    const fn tile_row(&self, row: u8) -> u16 {
        let high = (self.bg_tile & 0x80 == 0) & (self.inputs.lcdc & 0x10 == 0);
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
        let bg_a = (self.bg_shift_a & 0x80 != 0) & (lcdc & 1 != 0);
        let bg_b = (self.bg_shift_b & 0x80 != 0) & (lcdc & 1 != 0);
        let obj_a = (self.sp_a & 0x80 != 0) & (lcdc & 2 != 0);
        let obj_b = (self.sp_b & 0x80 != 0) & (lcdc & 2 != 0);
        let show_sprite = (obj_a | obj_b) & !((self.sp_priority & 0x80 != 0) & (bg_a | bg_b));
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
        let cp = (self.px9 & self.wires.clkpipe) | self.wires.fine_done;
        (cp, !self.line_started, palette >> (2 * pixel) & 3)
    }
}
