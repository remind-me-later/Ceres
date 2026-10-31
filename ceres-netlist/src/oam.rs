//! The netlist's OAM: its latches as far as the OAM scan reads them, and
//! the whole of it (`NetOam`).
//!
//! Its two stages of latches of the even bytes (the Y), transcribed from
//! `hdl/rtl/oam.vhd`, for ceres-core's proofs against ceres's object search
//! and for the replay of the model's traces (`tests/replay.rs`), which
//! checks the transcription.

/// What OAM's latches take in after an edge (`hdl/rtl/oam.vhd`), while the
/// CPU does not address OAM.
#[derive(Clone, Copy)]
#[expect(clippy::struct_excessive_bools, reason = "the strobes' levels")]
pub struct OamInputs {
    /// The scan runs (`crate::oam_scan::NetScan::scanning`).
    pub scanning: bool,
    /// OAM DMA runs, and its clock.
    pub run: bool,
    pub dma_phi: bool,
    pub wuvu: bool,
    pub wosu: bool,
    /// The sprite fetch reads OAM, and holds the second latches.
    pub fetch_read: bool,
    pub fetch_hold: bool,
}

impl OamInputs {
    /// Mode 2 for OAM: OAM DMA hides OAM from the scan.
    // @hw die: ACYL BOGE
    #[must_use]
    pub const fn mode2(self) -> bool {
        self.scanning & !self.run
    }

    /// The first latches are open: the scan reads, while `wosu` is low, or
    /// the sprite fetch reads.
    // @hw die: BOTA, AJEP XUJA ASYT BODE
    #[must_use]
    pub const fn latch_open(self) -> bool {
        (self.mode2() & !self.wosu) | self.fetch_read
    }

    /// The RAM is precharged, and the second latches take the first: out of
    /// mode 2 (while OAM DMA runs, only while `dma_phi` is high) unless the
    /// sprite fetch holds them, and in mode 2 as `wuvu` rises.
    // @hw die: CUFE XUJY AVER BYCU COTA
    #[must_use]
    pub const fn precharge(self) -> bool {
        (!self.run | self.dma_phi) & !self.fetch_hold & !(self.mode2() & !(self.wosu & self.wuvu))
    }
}

/// OAM's two stages of latches of the even bytes (Y, or a sprite's tile).
///
/// The first take the data lines while they are open, the second the first
/// while the RAM is precharged; the scan's comparator sees the second.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NetOamLatches {
    pub first: u8,
    pub second: u8,
}

impl NetOamLatches {
    /// After an edge, with what the latches take in after it and the byte
    /// the data lines carry (the RAM's, while the scan reads it).
    // @hw die: YDYV..ECED (dlatch), XUSO..DEPO (dlatch_ee)
    pub const fn settle(&mut self, inputs: OamInputs, data: u8) {
        if inputs.latch_open() {
            self.first = data;
        }
        if inputs.precharge() {
            self.second = self.first;
        }
    }
}

/// What OAM takes in (`hdl/rtl/oam.vhd`), after an edge.
#[derive(Clone, Copy, Default)]
#[expect(clippy::struct_excessive_bools, reason = "the strobes' levels")]
pub struct NetOamInputs {
    /// The CPU's access: the address, its read and write strobes, the data
    /// bus, and the clocks' `clk_t4` (high in G and H).
    pub addr: u16,
    pub rd: bool,
    pub wr: bool,
    pub data: u8,
    pub clk_t4: bool,
    /// OAM DMA: it runs, its clock, its address's low byte, and whether it
    /// reads the cartridge bus (the data pins `d`) or VRAM (`md`).
    pub dma_run: bool,
    pub dma_phi: bool,
    pub dma_index: u8,
    pub dma_addr_ext: bool,
    pub vram_to_oam: bool,
    pub d: u8,
    pub md: u8,
    /// Modes 2 and 3, the scan's index, the sprite store's index lines.
    pub mode2: bool,
    pub mode3: bool,
    pub scan_index: u8,
    pub render_index: u8,
    pub wosu: bool,
    pub wuvu: bool,
    /// The sprite fetch reads OAM, and holds the second latches.
    pub fetch_read: bool,
    pub fetch_hold: bool,
}

/// OAM's wires: its concurrent statements, each a delta cycle behind what
/// it is made of (`hdl/rtl/oam.vhd`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[expect(clippy::struct_excessive_bools, reason = "the netlist's gates")]
pub struct OamWires {
    pub fexx: bool,
    pub free: bool,
    pub cpu_access: bool,
    pub addr: u8,
    pub word: u8,
    pub dma_write: bool,
    pub write: bool,
    pub write_odd: bool,
    pub write_even: bool,
    pub cpu_read_oam: bool,
    pub latch_open: bool,
    pub precharge: bool,
    pub word_line: bool,
    pub read_ram: bool,
}

/// The netlist's OAM, settled delta cycle by delta cycle, with render
/// (`crate::render::NetRender`), whose timing it shares.
///
/// Its RAM (the even bytes, Y and tile, and the odd ones, X and
/// attributes, of 80 words), its data lines and bit lines, its two stages
/// of latches, and the DMA's write timing. As mode 2 starts during a CPU
/// write, the address changes deltas before the write ends, and the model
/// writes the scan's word: the deltas matter.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NetOam {
    pub ram_even: [u8; 80],
    pub ram_odd: [u8; 80],
    /// The data lines: they keep their value when nothing drives them, and
    /// follow their drivers 1 ns later.
    pub bus_even: u8,
    pub bus_odd: u8,
    pub bit_lines_even: u8,
    pub bit_lines_odd: u8,
    pub first_even: u8,
    pub first_odd: u8,
    pub second_even: u8,
    pub second_odd: u8,
    /// WUJE: after the CPU writes FExx, until `dma_phi` rises.
    pub wuje: bool,
    /// MAKA: `clk_t4` half a period later.
    pub maka: bool,
    pub wires: OamWires,
}

impl Default for NetOam {
    fn default() -> Self {
        Self::power_on()
    }
}

/// The deltas a settle may take before OAM is taken to oscillate.
const MAX_DELTAS: usize = 1000;

impl NetOam {
    /// OAM at power-on: its cells all FF (dmg-sim's cells at 0 are the
    /// active-low lines' levels), as are its latches.
    #[must_use]
    pub fn power_on() -> Self {
        Self {
            ram_even: [0xFF; 80],
            ram_odd: [0xFF; 80],
            bus_even: 0,
            bus_odd: 0,
            bit_lines_even: 0xFF,
            bit_lines_odd: 0xFF,
            first_even: 0xFF,
            first_odd: 0xFF,
            second_even: 0xFF,
            second_odd: 0xFF,
            wuje: false,
            maka: false,
            wires: OamWires::default(),
        }
    }

    /// The wires a delta cycle on.
    // @hw die: SARO, AJON AJUJ AMAB; ASAM DUGA APAR BETE; CATY DECY, NAXY
    // POWU; WYJA YLYC YNYC ZONE ZOFE; BOTA, AJEP XUJA ASYT BODE, YVAL YRYV
    // ZODO; CUFE XUJY AVER BYCU COTA WOVU, WAZO WUJY WAHE, WYCY WORY
    #[expect(clippy::cast_possible_truncation, reason = "the address's low byte")]
    #[must_use]
    pub const fn next_wires(&self, i: &NetOamInputs) -> OamWires {
        let w = &self.wires;
        OamWires {
            fexx: i.addr >> 8 == 0xFE,
            // not dma_run and not mode2 and not (mode3 and not dma_run)
            free: !i.dma_run & !i.mode2 & !i.mode3,
            cpu_access: w.fexx & w.free,
            addr: if !i.mode2 & !i.mode3 & !i.dma_run {
                i.addr as u8
            } else if i.dma_run {
                i.dma_index
            } else if i.mode2 {
                i.scan_index << 2
            } else {
                i.render_index << 2 | 3
            },
            word: w.addr >> 1,
            dma_write: i.dma_run & !i.dma_phi & !self.maka,
            write: (w.cpu_access & i.wr) | w.dma_write,
            write_odd: w.write & (w.addr & 1 != 0),
            write_even: w.write & (w.addr & 1 == 0),
            cpu_read_oam: w.fexx & i.rd & !i.clk_t4,
            latch_open: (i.mode2 & !i.wosu) | i.fetch_read | w.cpu_read_oam,
            precharge: (!(w.fexx | i.dma_run) | i.dma_phi)
                & !i.fetch_hold
                & !(i.mode2 & !(i.wosu & i.wuvu)),
            word_line: (w.latch_open | w.write) & !w.precharge,
            read_ram: w.latch_open & w.word_line,
        }
    }

    /// The wires settled from the latches as they are, without running
    /// the processes: after the state was set from outside.
    ///
    /// # Panics
    ///
    /// If the wires do not settle.
    pub fn settle_wires(&mut self, i: &NetOamInputs) {
        let mut deltas = 0;
        loop {
            let wires = self.next_wires(i);
            if wires == self.wires {
                break;
            }
            self.wires = wires;
            deltas += 1;
            assert!(deltas < MAX_DELTAS, "OAM's wires oscillate");
        }
    }

    /// What drives the data lines now, which they follow 1 ns later; `None`
    /// while nothing does.
    #[must_use]
    pub const fn bus_drivers(&self, i: &NetOamInputs) -> Option<(u8, u8)> {
        let w = &self.wires;
        // The CPU's access, unless WUJE (after its write) holds it off.
        let cpu_drives = w.cpu_access & !self.wuje;
        if i.dma_addr_ext {
            Some((i.d, i.d))
        } else if i.vram_to_oam {
            Some((i.md, i.md))
        } else if cpu_drives | (w.free & !w.fexx) {
            Some((i.data, i.data))
        } else if w.read_ram {
            Some((self.bit_lines_even, self.bit_lines_odd))
        } else {
            None
        }
    }

    /// One delta cycle: the wires follow, the latches and the RAM act on
    /// the wires as they are. The data lines follow their driver
    /// (`bus_drivers`) 1 ns later: render's schedule moves them.
    // @hw die: WUJE (nor_latch); oam_a oam_b; YDYV..ECED (dlatch),
    // XUSO..DEPO (dlatch_ee)
    #[must_use]
    pub fn delta(&self, i: &NetOamInputs) -> Self {
        let (cur, w) = (self, &self.wires);
        let mut next = *cur;
        next.wires = cur.next_wires(i);
        if w.fexx & i.wr {
            next.wuje = false;
        } else if i.dma_phi {
            next.wuje = true;
        } else {
            // The latch holds.
        }
        let word = usize::from(w.word);
        if w.precharge {
            next.bit_lines_even = 0xFF;
            next.bit_lines_odd = 0xFF;
        } else {
            if w.write_even {
                next.bit_lines_even = cur.bus_even;
                if let Some(cell) = next.ram_even.get_mut(word) {
                    *cell = cur.bus_even;
                }
            } else if let (true, Some(&cell)) = (w.word_line, cur.ram_even.get(word)) {
                next.bit_lines_even = cell;
            } else {
                // The bit lines hold.
            }
            if w.write_odd {
                next.bit_lines_odd = cur.bus_odd;
                if let Some(cell) = next.ram_odd.get_mut(word) {
                    *cell = cur.bus_odd;
                }
            } else if let (true, Some(&cell)) = (w.word_line, cur.ram_odd.get(word)) {
                next.bit_lines_odd = cell;
            } else {
                // The bit lines hold.
            }
        }
        if w.latch_open {
            next.first_even = cur.bus_even;
            next.first_odd = cur.bus_odd;
        }
        if w.precharge {
            next.second_even = cur.first_even;
            next.second_odd = cur.first_odd;
        }
        next
    }

    /// What a CPU read of OAM takes, in G and H: the first latches.
    // @hw die: LEKO MYNU, WUKU WEWU GUKO WUME
    #[must_use]
    pub const fn read(&self, i: &NetOamInputs) -> Option<u8> {
        let w = &self.wires;
        if w.cpu_access & i.rd & i.clk_t4 {
            Some(if i.addr & 1 != 0 {
                self.first_odd
            } else {
                self.first_even
            })
        } else {
            None
        }
    }
}
