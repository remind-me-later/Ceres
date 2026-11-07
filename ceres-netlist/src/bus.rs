//! The netlist's bus side and the board, phase by phase.
//!
//! The external bus (the cartridge bus's pins and latches, the data bus's
//! precharge), HRAM, the boot ROM's mapping, the joypad and the serial
//! port; and the board's work RAM and cartridge (its ROM, RAM and MBC).
//!
//! Transcribed from `hdl/rtl/external_bus.vhd`, `high_ram.vhd`,
//! `boot_rom.vhd`, `joypad.vhd`, `serial.vhd` and the board's
//! `hdl/board/gameboy.vhd` and `cartridge.vhd`, for the replay of the
//! model's traces (`tests/replay.rs`) and for `NetChip`. What they take
//! from the CPU core (its address, `read`, `write`, `mreq`, and the data it
//! drives) is the chip's boundary with the core.

/// The board's RAM chips at power-on: the arbitrary bytes of a 32-bit
/// xorshift with `seed` (`arbitrary` in `hdl/board/gameboy.vhd`; work RAM
/// has seed 1, VRAM seed 2).
#[must_use]
#[expect(clippy::cast_possible_truncation, reason = "the low byte")]
pub fn arbitrary(length: usize, seed: u32) -> Vec<u8> {
    let mut x = seed;
    let mut bytes = vec![0; length];
    for i in 0..length {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        bytes[i] = x as u8;
    }
    bytes
}

/// The cartridge's MBC, from the header's byte 0147.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Mbc {
    None,
    Mbc1,
    Mbc5,
}

/// The board's cartridge (`hdl/board/cartridge.vhd`).
pub struct Cartridge {
    pub rom: Vec<u8>,
    pub ram: Vec<u8>,
    pub mbc: Mbc,
    pub has_ram: bool,
    pub enabled: bool,
    pub rom_bank: u16,
    pub ram_bank: u8,
    pub mode: bool,
}

impl Cartridge {
    /// A cartridge with `rom`, its MBC and RAM as its header says.
    #[must_use]
    pub fn new(rom: Vec<u8>) -> Self {
        // A ROM too short for a header has neither.
        let (kind, ram_size) = if rom.len() > 0x149 {
            (rom[0x147], rom[0x149])
        } else {
            (0, 0)
        };
        let mbc = match kind {
            0x01..=0x03 => Mbc::Mbc1,
            0x19..=0x1E => Mbc::Mbc5,
            _ => Mbc::None,
        };
        let has_ram = ram_size != 0;
        Self {
            rom,
            ram: vec![0; 0x40000],
            mbc,
            has_ram,
            enabled: false,
            rom_bank: 1,
            ram_bank: 0,
            mode: false,
        }
    }

    /// The MBC's reset (/RESET low).
    pub const fn reset(&mut self) {
        self.enabled = false;
        self.rom_bank = 1;
        self.ram_bank = 0;
        self.mode = false;
    }

    /// Where the ROM and the RAM are read at `a`: the ROM's offset and
    /// whether it is selected, the RAM's and whether it is.
    #[must_use]
    pub const fn decode(
        &self,
        a: u16,
        rd_n: bool,
        cs_n: bool,
        reset: bool,
    ) -> (usize, bool, usize, bool) {
        let low = (a & 0x3FFF) as usize;
        let a14 = a & 0x4000 != 0;
        match self.mbc {
            Mbc::None => (
                (a & 0x7FFF) as usize,
                a & 0x8000 == 0,
                (a & 0x1FFF) as usize,
                !cs_n & (a & 0x2000 != 0),
            ),
            Mbc::Mbc1 => {
                let bank = if reset | !a14 {
                    0
                } else if self.rom_bank.is_multiple_of(0x20) {
                    1
                } else {
                    (self.rom_bank & 0x1F) as usize
                };
                let upper = if self.mode | a14 {
                    (self.ram_bank & 3) as usize
                } else {
                    0
                };
                (
                    (upper * 32 + bank) * 0x4000 + low,
                    reset | ((a & 0x8000 == 0) & !rd_n),
                    upper * 0x2000 + (a & 0x1FFF) as usize,
                    self.enabled & !cs_n & !a14,
                )
            }
            Mbc::Mbc5 => {
                let bank = if reset | !a14 {
                    0
                } else {
                    self.rom_bank as usize
                };
                let ram = if a14 {
                    self.ram_bank as usize * 0x2000 + (a & 0x1FFF) as usize
                } else {
                    (a & 0x1FFF) as usize
                };
                (
                    bank * 0x4000 + low,
                    a & 0x8000 == 0,
                    ram,
                    self.enabled & !cs_n & !a14,
                )
            }
        }
    }

    /// The rise of /WR: the MBC's registers take the data pins, as the RAM
    /// does if selected.
    pub fn write(&mut self, a: u16, d: u8, cs_n: bool) {
        let region = a >> 12;
        match self.mbc {
            Mbc::Mbc1 => match region / 2 {
                0 => self.enabled = d & 0x0F == 0x0A,
                1 => self.rom_bank = u16::from(d & 0x1F),
                2 => self.ram_bank = d & 3,
                3 => self.mode = d & 1 != 0,
                _ => {}
            },
            Mbc::Mbc5 => match region {
                0 => self.enabled = d & 0x0F == 0x0A,
                2 => self.rom_bank = self.rom_bank & 0x100 | u16::from(d),
                3 => self.rom_bank = self.rom_bank & 0xFF | u16::from(d & 1) << 8,
                4 => self.ram_bank = d & 0x0F,
                _ => {}
            },
            Mbc::None => {}
        }
        let (_, _, ram_offset, ram_selected) = self.decode(a, true, cs_n, false);
        if self.has_ram & ram_selected & (ram_offset < self.ram.len()) {
            self.ram[ram_offset] = d;
        }
    }

    /// What the cartridge drives on the data pins, if anything.
    #[must_use]
    #[expect(clippy::similar_names, reason = "the ROM's and the RAM's")]
    pub fn read(&self, pins: &BusPins, reset: bool) -> Option<u8> {
        let (rom_offset, rom_selected, ram_offset, ram_selected) =
            self.decode(pins.a, pins.rd_n, pins.cs_n, reset);
        let reads = !pins.rd_n;
        if self.has_ram & ram_selected & reads & pins.wr_n {
            if ram_offset < self.ram.len() {
                Some(self.ram[ram_offset])
            } else {
                None
            }
        } else if (self.rom.len() > 0) & rom_selected & reads {
            // Past the ROM's end, the address wraps (the model's ROM reads
            // its image modulo its length: a smaller ROM mirrors).
            Some(self.rom[rom_offset % self.rom.len()])
        } else {
            None
        }
    }
}

/// The board: its work RAM (C000 to DFFF, and its echo) and its cartridge.
pub struct NetBoard {
    pub work_ram: Vec<u8>,
    pub cartridge: Cartridge,
}

impl NetBoard {
    /// The board at power-on, with the cartridge's `rom`.
    #[must_use]
    pub fn power_on(rom: Vec<u8>) -> Self {
        Self {
            work_ram: arbitrary(0x2000, 1),
            cartridge: Cartridge::new(rom),
        }
    }

    /// What the board's chips drive on the data pins, if anything.
    #[must_use]
    pub fn drive(&self, pins: &BusPins, reset: bool) -> Option<u8> {
        if !pins.cs_n & (pins.a & 0x4000 != 0) & !pins.rd_n & pins.wr_n {
            Some(self.work_ram[usize::from(pins.a & 0x1FFF)])
        } else {
            self.cartridge.read(pins, reset)
        }
    }

    /// The rise of /WR: work RAM takes the data pins if selected, and the
    /// cartridge sees the write.
    pub fn write(&mut self, pins: &BusPins) {
        if !pins.cs_n & (pins.a & 0x4000 != 0) {
            self.work_ram[usize::from(pins.a & 0x1FFF)] = pins.d;
        }
        self.cartridge.write(pins.a, pins.d, pins.cs_n);
    }
}

/// The cartridge bus's pins.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BusPins {
    pub a: u16,
    pub d: u8,
    pub rd_n: bool,
    pub wr_n: bool,
    pub cs_n: bool,
}

/// What the bus side takes in, in a phase: the CPU core's access, the
/// clocks', OAM DMA's, the divider, the joypad's and the serial port's pins.
#[expect(clippy::struct_excessive_bools, reason = "the strobes' levels")]
pub struct BusInputs {
    /// The core's address, its read and write, its memory request, and the
    /// chip's data bus.
    pub addr: u16,
    pub read: bool,
    pub write: bool,
    pub mreq: bool,
    pub data: u8,
    /// The clocks': `clk_t4`, `exec_phase`, `cpu_wr_sync`, `cpu_ext_cs_en`,
    /// `clk_1mhz`, and the A edge.
    pub clk_t4: bool,
    pub exec_phase: bool,
    pub cpu_wr_sync: bool,
    pub cpu_ext_cs_en: bool,
    pub clk_1mhz: bool,
    pub starts_a: bool,
    pub reset2: bool,
    /// /RESET (the cartridge's MBC resets with it).
    pub reset: bool,
    /// OAM DMA reads the cartridge bus, at its address.
    pub dma_addr_ext: bool,
    pub dma_addr: u16,
    /// The divider.
    pub div: u16,
    /// The joypad's input pins (P10 to P13), SIN, and SCK as the other end
    /// drives it.
    pub p10_13: u8,
    pub sin: bool,
    pub sck: bool,
    /// The address the external bus's latches took while the core's
    /// request pulsed for a moment inside the phase, as an access inside the
    /// chip started (the core's timing, from the trace): `None` if they did
    /// not open so.
    pub latch_glitch: Option<u16>,
}

/// The netlist's bus side, between phases.
#[derive(Clone, Copy)]
#[expect(clippy::struct_excessive_bools, reason = "the netlist's flip-flops")]
pub struct NetBus {
    /// HRAM (FF80 to FFFE).
    pub high_ram: [u8; 127],
    /// FF50: the boot ROM is unmapped.
    pub unmapped: bool,
    /// The external bus's address latches (A0 to A14) and data latches.
    pub latched_addr: u16,
    pub latched_data: u8,
    /// The data latches took the pins while nothing drove them.
    pub latched_floating: bool,
    /// The joypad: P1's flip-flops, the read latch, the debounce history,
    /// the wake-up latch.
    pub p1: u8,
    pub jp_read: u8,
    pub jp_history: u8,
    pub jp_wakeup: bool,
    /// The serial port: SB, SC's transfer and internal clock, the half
    /// clock, the bit counter, the output flip-flop.
    pub sb: u8,
    pub transfer: bool,
    pub internal_clock: bool,
    /// The half clock: unknown at power-on, until a write of SC clears it.
    pub half: Option<bool>,
    pub count: u8,
    pub ser_out: bool,
    /// The last phase's write strobes, divider bit 5, serial clock and data
    /// bus, for their edges.
    pub before: Before,
}

/// What edges are taken against.
#[derive(Clone, Copy)]
#[expect(clippy::struct_excessive_bools, reason = "the strobes' levels")]
pub struct Before {
    pub write_ff50: bool,
    pub write_p1: bool,
    pub write_sc: bool,
    pub div5: bool,
    pub serial_clock: bool,
    pub non_vram_mreq: bool,
    pub data: u8,
    pub pins: BusPins,
}

/// What a phase of the bus side gives the rest of the chip and the board.
#[derive(Clone, Copy)]
#[expect(clippy::struct_excessive_bools, reason = "the strobes' levels")]
pub struct BusOutputs {
    pub pins: BusPins,
    /// Something drives the data pins (else they float, held by the
    /// board's keeper against the pads' pull-ups).
    pub pins_driven: bool,
    pub non_vram_mreq: bool,
    pub boot_sel: bool,
    /// What the bus side drives on the chip's data bus for a read.
    pub data_out: Option<u8>,
    pub int_serial: bool,
    pub int_jp: bool,
    pub cpu_wakeup: bool,
    pub sck: bool,
    pub sout: bool,
}

impl NetBus {
    /// The bus side at power-on.
    #[must_use]
    pub const fn power_on() -> Self {
        Self {
            high_ram: [0; 127],
            unmapped: false,
            latched_addr: 0,
            latched_data: 0,
            latched_floating: true,
            p1: 0,
            jp_read: 0,
            jp_history: 0,
            jp_wakeup: false,
            sb: 0,
            transfer: false,
            internal_clock: false,
            half: None,
            count: 0,
            ser_out: false,
            before: Before {
                write_ff50: false,
                write_p1: false,
                write_sc: false,
                div5: false,
                serial_clock: true,
                non_vram_mreq: false,
                data: 0xFF,
                pins: BusPins {
                    a: 0,
                    d: 0xFF,
                    rd_n: false,
                    wr_n: true,
                    cs_n: true,
                },
            },
        }
    }

    /// One phase, with what the core, the clocks and the DMA give in it, the
    /// board on the pins and the boot ROM's image.
    #[expect(clippy::too_many_lines, reason = "the blocks in turn")]
    #[expect(clippy::cognitive_complexity, reason = "the blocks in turn")]
    #[expect(clippy::similar_names, reason = "SB's and SC's write strobes")]
    pub fn phase(&mut self, i: &BusInputs, board: &mut NetBoard, boot_rom: &[u8]) -> BusOutputs {
        let b = self.before;
        let (cpu_rd, cpu_wr) = (i.read, i.cpu_wr_sync);
        let addr = i.addr;

        // The boot ROM's mapping: FF50 bit 0 unmaps it, as the write ends.
        // @hw die: the boot ROM's FF50 latch
        let write_ff50 = cpu_wr & (addr == 0xFF50);
        if i.reset2 {
            self.unmapped = false;
        } else if b.write_ff50 & !write_ff50 {
            self.unmapped |= b.data & 1 != 0;
        } else {
            // The flip-flop holds.
        }
        let boot_sel = !self.unmapped & (addr >> 8 == 0);

        // HRAM.
        let hram_index = (addr & 0x7F) as usize;
        let hram_selected = (addr >> 7 == 0x1FF) & (addr != 0xFFFF);
        if hram_selected & cpu_wr {
            self.high_ram[hram_index] = i.data;
        }

        // The external bus.
        // @hw die: SORE TEVY TEXO; ALOR..NYRE (dlatch); SOBY SEPY; TYNU TUMA
        // TOZA TYHO; MOCA LAGU LYWE MOTY; TYMU UGAC URUN..USUF; SOMA..SAZY
        let vram_addr = addr >> 13 == 0b100;
        let non_vram_mreq = i.mreq & !vram_addr;
        if non_vram_mreq {
            self.latched_addr = addr & 0x7FFF;
        } else if i.starts_a & b.non_vram_mreq {
            // As an access ends, the core changes its address's high byte
            // before its request falls, and the low byte after: the latches
            // take the high byte of the precharged bus, 00.
            self.latched_addr = self.latched_addr & 0x00FF | addr & 0x7F00;
        } else if let Some(glitch) = i.latch_glitch {
            // @fact cartridge-bus-edges
            self.latched_addr = glitch & 0x7FFF;
        } else {
            // The latches hold.
        }
        let a15_cpu = !(i.cpu_ext_cs_en & (addr & 0x8000 == 0) & !boot_sel);
        let a = if i.dma_addr_ext {
            i.dma_addr
        } else {
            u16::from(a15_cpu) << 15 | self.latched_addr
        };
        let cs = if i.dma_addr_ext {
            i.dma_addr & 0x8000 != 0
        } else {
            i.cpu_ext_cs_en & ((addr >> 13 == 0b101) | (addr >> 14 == 0b11)) & (addr >> 9 != 0x7F)
        };
        let pins_out = non_vram_mreq & i.write;
        let rd_n = !i.dma_addr_ext & pins_out;
        let wr_n = !(!i.dma_addr_ext & i.cpu_wr_sync & non_vram_mreq);
        let mut pins = BusPins {
            a,
            d: 0xFF,
            rd_n,
            wr_n,
            cs_n: !cs,
        };
        // The data pins: the CPU's data while it writes outside VRAM, else
        // what the board drives, else the board's keeper, against the pads'
        // pull-ups (`pins_driven` false).
        let board_drives = if pins_out {
            None
        } else {
            board.drive(&pins, i.reset)
        };
        let pins_driven = pins_out | board_drives.is_some();
        pins.d = match (pins_out, i.latch_glitch, i.write, board_drives) {
            // The core's pulse also drives the pins with the chip's data
            // bus for a moment, as a write: the keeper holds that.
            (true, _, _, _) | (false, Some(_), true, None) => i.data,
            (false, _, _, Some(byte)) => byte,
            (false, _, _, None) => b.pins.d,
        };
        // The board's chips take the data pins as /WR rises.
        if !b.pins.wr_n & pins.wr_n {
            board.write(&b.pins);
        }
        if i.reset {
            board.cartridge.reset();
        }
        let read_pins = i.read & non_vram_mreq & i.clk_t4;
        if !read_pins {
            self.latched_data = pins.d;
            self.latched_floating = !pins_driven;
        }

        // The joypad.
        // @hw die: the joypad's P1 flip-flops, read latch, KEVU..; the
        // debounce, the wake-up latch
        let write_p1 = cpu_wr & (addr == 0xFF00);
        let read_p1 = cpu_rd & (addr == 0xFF00);
        if i.reset2 {
            self.p1 = 0;
        } else if b.write_p1 & !write_p1 {
            self.p1 = b.data;
        } else {
            // The flip-flops hold.
        }
        let inputs = i.p10_13 & 0x0F;
        if !read_p1 {
            self.jp_read = inputs;
        }
        let pressed = inputs != 0x0F;
        if i.reset2 {
            self.jp_history = 0;
        } else if i.starts_a {
            self.jp_history = (self.jp_history << 1 | u8::from(pressed)) & 0x0F;
        } else {
            // The shift register holds.
        }
        if i.clk_1mhz {
            self.jp_wakeup = pressed;
        }

        // The serial port.
        // @hw die: the serial port's SB, SC, its clock and counter
        let write_sb = cpu_wr & (addr == 0xFF01);
        let write_sc = cpu_wr & (addr == 0xFF02);
        if i.reset2 {
            self.internal_clock = false;
            self.transfer = false;
        } else {
            if b.write_sc & !write_sc {
                self.transfer = b.data & 0x80 != 0;
            }
            if b.write_sc & !write_sc {
                self.internal_clock = b.data & 1 != 0;
            }
        }
        let div5 = i.div & 0x20 != 0;
        if write_sc {
            self.half = Some(false);
        } else if b.div5 & !div5 {
            self.half = match self.half {
                Some(half) => Some(!half),
                None => None,
            };
        } else {
            // The flip-flop holds.
        }
        let serial_clock = if !self.transfer {
            true
        } else if self.internal_clock {
            // An unknown half clock: as the model's 'U', no edge.
            match self.half {
                Some(half) => !half,
                None => true,
            }
        } else {
            i.sck
        };
        if i.reset2 {
            self.sb = 0;
        } else if write_sb {
            self.sb = i.data;
        } else if serial_clock & !b.serial_clock {
            self.sb = self.sb << 1 | u8::from(i.sin);
        } else {
            // The shift register holds.
        }
        if i.reset2 {
            self.ser_out = false;
        } else if !serial_clock & b.serial_clock {
            self.ser_out = self.sb & 0x80 != 0;
        } else {
            // The flip-flop holds.
        }
        if write_sc | i.reset2 {
            self.count = 0;
        } else if serial_clock & !b.serial_clock {
            self.count = (self.count + 1) & 0x0F;
        } else {
            // The counter holds.
        }
        // The eighth bit's interrupt clears the transfer at once.
        if self.count & 8 != 0 {
            self.transfer = false;
        }

        // What the bus side drives on the chip's data bus.
        let data_out = if !i.exec_phase {
            // The precharge, in phase A.
            Some(0xFF)
        } else if read_pins {
            // Floating pins read FF: where the board's keeper holds a 0
            // against the pads' pull-ups, the model's line is unknown (W),
            // which reads as 1.
            Some(if self.latched_floating {
                0xFF
            } else {
                self.latched_data
            })
        } else if cpu_rd & boot_sel {
            let at = usize::from(addr & 0x0FFF);
            if at < boot_rom.len() {
                Some(boot_rom[at])
            } else {
                None
            }
        } else if cpu_rd {
            match addr {
                0xFF50 => Some(0xFE | u8::from(self.unmapped)),
                0xFF00 => Some(0xC0 | self.p1 & 0x30 | self.jp_read),
                0xFF01 => Some(self.sb),
                0xFF02 => Some(u8::from(self.transfer) << 7 | 0x7E | u8::from(self.internal_clock)),
                _ if hram_selected => Some(self.high_ram[hram_index]),
                _ => None,
            }
        } else {
            None
        };

        self.before = Before {
            write_ff50,
            write_p1,
            write_sc,
            div5,
            serial_clock,
            non_vram_mreq,
            data: i.data,
            pins,
        };
        BusOutputs {
            pins,
            pins_driven,
            non_vram_mreq,
            boot_sel,
            data_out,
            int_serial: self.count & 8 != 0,
            int_jp: (self.jp_history & 1 != 0) & (self.jp_history & 8 != 0),
            cpu_wakeup: self.jp_wakeup,
            sck: if self.internal_clock {
                serial_clock
            } else {
                i.sck
            },
            sout: self.ser_out,
        }
    }
}
