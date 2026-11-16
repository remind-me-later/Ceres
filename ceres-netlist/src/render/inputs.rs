//! What render takes in, and its wires: each concurrent statement of
//! the VHDL, a delta cycle behind what it is made of.

#[cfg_attr(
    not(test),
    expect(clippy::wildcard_imports, reason = "render's parts share its items")
)]
use super::*;

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
    /// The match as it stops mode 3 (`x_match_held` of
    /// `hdl/rtl/render.vhd`): its rise 4 ns late, as FEPO's on the die, its
    /// fall at once.
    pub x_match_held: bool,
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
    /// The CPU's write strobe as it reaches the registers' latches: the
    /// clocks decode it from the phase, three deltas behind render's clock
    /// (`cpu_wr_sync` in `hdl/rtl/clocks.vhd`, `cpu_wr` in
    /// `hdl/rtl/external_bus.vhd`). Bit 0 is a delta behind, bit 2 three.
    pub cpu_wr: u8,
    /// The LCD's pins and what makes LD: the planes' top pixels, which of
    /// them shows, its colour, the palette, the shade; CP and ST.
    pub bg_a: bool,
    pub bg_b: bool,
    pub obj_a: bool,
    pub obj_b: bool,
    pub show_sprite: bool,
    pub pixel: u8,
    pub palette: u8,
    pub ld: u8,
    pub cp: bool,
    pub st: bool,
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
        x_match_held: false,
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
        cpu_wr: 0,
        bg_a: false,
        bg_b: false,
        obj_a: false,
        obj_b: false,
        show_sprite: false,
        pixel: 0,
        palette: 0,
        ld: 0,
        cp: false,
        st: false,
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
    x_match_held,
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
    cpu_wr,
    bg_a,
    bg_b,
    obj_a,
    obj_b,
    show_sprite,
    pixel,
    palette,
    ld,
    cp,
    st,
});

impl NetRender {
    /// The write strobe's and the LCD's pins' wires, a delta on.
    // @hw die: TOBA SEMU RYPO CP; RUJU POME POFY RUZE ST; WOXA XULA NULY,
    // RAJY TADE, RUTA RYFU POKA, ..., PERO PATY REMY RAVO LD0 LD1
    pub(super) const fn next_pins(cur: &Self, n: &mut Wires) {
        let (i, w) = (&cur.inputs, &cur.wires);
        n.cpu_wr = (w.cpu_wr << 1 | i.cpu_wr_sync as u8) & 7;
        let lcdc = i.lcdc;
        n.bg_a = (cur.bg_shift_a & 0x80 != 0) & (lcdc & 1 != 0);
        n.bg_b = (cur.bg_shift_b & 0x80 != 0) & (lcdc & 1 != 0);
        n.obj_a = (cur.sp_a & 0x80 != 0) & (lcdc & 2 != 0);
        n.obj_b = (cur.sp_b & 0x80 != 0) & (lcdc & 2 != 0);
        n.show_sprite = (w.obj_a | w.obj_b) & !((cur.sp_priority & 0x80 != 0) & (w.bg_a | w.bg_b));
        n.pixel = if w.show_sprite {
            (w.obj_a as u8) << 1 | w.obj_b as u8
        } else {
            (w.bg_b as u8) << 1 | w.bg_a as u8
        };
        n.palette = if !w.show_sprite {
            cur.bgp
        } else if cur.sp_palette & 0x80 != 0 {
            cur.obp1
        } else {
            cur.obp0
        };
        n.ld = w.palette >> (2 * w.pixel) & 3;
        n.cp = (cur.px9 & w.clkpipe) | w.fine_done;
        n.st = !cur.line_started;
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
        n.stop_mode3 = (cur.px == 167) & !w.x_match_held;
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
        Self::next_pins(cur, n);

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
        // Its rise comes from the edge's schedule (`delta`).
        n.x_match_held = w.x_match_held & w.x_match;
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

    /// The wires settled from the flip-flops and latches as they are,
    /// without an edge: after the state was set from outside (a replay's
    /// resynchronisation), as at the end of a phase, where the delayed match
    /// (`x_match_held`) has caught up.
    ///
    /// # Panics
    ///
    /// If the wires do not settle.
    pub fn settle_wires(&mut self, vram: &[u8; 0x2000]) {
        let mut deltas = 0;
        loop {
            let mut wires = self.next_wires(vram);
            // Its delay has run out, as at the end of every phase.
            wires.x_match_held = self.wires.x_match;
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
}
