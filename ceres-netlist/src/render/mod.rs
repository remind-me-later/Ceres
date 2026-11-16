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

mod inputs;
mod settle;

pub use inputs::{RenderInputs, Wires};
use settle::{MAX_DELTAS, first_slot, flipped, same_address, same_drivers};

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
#[derive(Clone, Copy, PartialEq, Eq, Default)]
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
    /// Mode 0 held for a nanosecond or more within the last edge, and fell
    /// before its end: a sprite at X 167 stops mode 3 4 ns after the pixel
    /// counter reaches it (not compared: an edge sets it).
    pub mode0_pulse: bool,
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
    mode0_pulse,
});

/// The LCD's pins as render drives them ([`NetRender::pins`]): CP, ST, LD.
pub type Pins = (bool, bool, u8);

/// The changes of the LCD's pins within an edge, delta by delta.
///
/// A reader of the pins that wakes on their changes, as the model's
/// testbench does, sees the pulses within an edge too: a write to LCDC's
/// window bit cuts the dot clock for 3 ns, and CP with it
/// (`lcdc-write-latch-delay`).
#[derive(Clone, Copy, Default)]
pub struct PinLog {
    /// The pins as the edge found them.
    pub before: Pins,
    /// The pins after each change, the last as the edge leaves them.
    pub changes: [Pins; Self::CAPACITY],
    pub len: usize,
}

impl PinLog {
    /// More changes than an edge makes: past it, the last is replaced.
    pub const CAPACITY: usize = 16;

    const fn start(&mut self, pins: Pins) {
        self.before = pins;
        self.len = 0;
    }

    /// The pins now, if they changed.
    fn note(&mut self, pins: Pins) {
        let last = if self.len == 0 {
            self.before
        } else {
            self.changes[self.len - 1]
        };
        if pins != last {
            if self.len == Self::CAPACITY {
                self.len -= 1;
            }
            self.changes[self.len] = pins;
            self.len += 1;
        }
    }
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
            mode0_pulse: false,
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
        self.edge_with_pins(inputs, vram, &mut PinLog::default());
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
    #[must_use]
    pub const fn pins(&self) -> Pins {
        (self.wires.cp, self.wires.st, self.wires.ld)
    }
}
