use {
  crate::{ppu::Mode, CompatMode, Gb, Model::Cgb},
  core::num::NonZeroU8,
};

#[derive(PartialEq, Eq, Default)]
pub enum HdmaState {
  #[default]
  Sleep,
  WaitHBlank,
  HBlankDone,
  General,
}

// IO addresses
const P1: u8 = 0x00;
const SB: u8 = 0x01;
const SC: u8 = 0x02;
const DIV: u8 = 0x04;
const TIMA: u8 = 0x05;
const TMA: u8 = 0x06;
const TAC: u8 = 0x07;
const IF: u8 = 0x0F;
const NR10: u8 = 0x10;
const NR11: u8 = 0x11;
const NR12: u8 = 0x12;
const NR13: u8 = 0x13;
const NR14: u8 = 0x14;
const NR21: u8 = 0x16;
const NR22: u8 = 0x17;
const NR23: u8 = 0x18;
const NR24: u8 = 0x19;
const NR30: u8 = 0x1A;
const NR31: u8 = 0x1B;
const NR32: u8 = 0x1C;
const NR33: u8 = 0x1D;
const NR34: u8 = 0x1E;
const NR41: u8 = 0x20;
const NR42: u8 = 0x21;
const NR43: u8 = 0x22;
const NR44: u8 = 0x23;
const NR50: u8 = 0x24;
const NR51: u8 = 0x25;
const NR52: u8 = 0x26;
const WAV_BEGIN: u8 = 0x30;
const WAV_END: u8 = 0x3F;
const LCDC: u8 = 0x40;
const STAT: u8 = 0x41;
const SCY: u8 = 0x42;
const SCX: u8 = 0x43;
const LY: u8 = 0x44;
const LYC: u8 = 0x45;
const DMA: u8 = 0x46;
const BGP: u8 = 0x47;
const OBP0: u8 = 0x48;
const OBP1: u8 = 0x49;
const WY: u8 = 0x4A;
const WX: u8 = 0x4B;
const KEY0: u8 = 0x4C;
const KEY1: u8 = 0x4D;
const VBK: u8 = 0x4F;
const HDMA1: u8 = 0x51;
const HDMA2: u8 = 0x52;
const HDMA3: u8 = 0x53;
const HDMA4: u8 = 0x54;
const HDMA5: u8 = 0x55;
const BCPS: u8 = 0x68;
const BCPD: u8 = 0x69;
const OCPS: u8 = 0x6A;
const OCPD: u8 = 0x6B;
const OPRI: u8 = 0x6C;
const SVBK: u8 = 0x70;
const PCM12: u8 = 0x76;
const PCM34: u8 = 0x77;
const HRAM_BEG: u8 = 0x80;
const HRAM_END: u8 = 0xFE;
const IE: u8 = 0xFF;

impl Gb {
  #[must_use]
  pub(crate) const fn read_ram(&self, addr: u16) -> u8 {
    self.wram[(addr & 0xFFF) as usize]
  }

  #[must_use]
  pub(crate) fn read_bank_ram(&self, addr: u16) -> u8 {
    let bank = u16::from(self.svbk_true.get()) * 0x1000;
    self.wram[(addr & 0xFFF | bank) as usize]
  }

  fn write_ram(&mut self, addr: u16, val: u8) {
    self.wram[(addr & 0xFFF) as usize] = val;
  }

  fn write_bank_ram(&mut self, addr: u16, val: u8) {
    let bank = u16::from(self.svbk_true.get()) * 0x1000;
    self.wram[(addr & 0xFFF | bank) as usize] = val;
  }

  const fn dma_active(&self) -> bool {
    self.dma_on && (self.dma_cycles > 0 || self.dma_restarting)
  }

  const fn hdma_on(&self) -> bool {
    !matches!(self.hdma_state, HdmaState::Sleep)
  }

  fn read_boot_or_cart(&mut self, addr: u16) -> u8 {
    self
      .boot_rom
      .map_or_else(|| self.cart.read_rom(addr), |b| b[addr as usize])
  }

  // **************
  // * Memory map *
  // **************

  pub(crate) fn read_mem(&mut self, addr: u16) -> u8 {
    match addr {
      0x0000..=0x00FF | 0x0200..=0x08FF => self.read_boot_or_cart(addr),
      0x0100..=0x01FF | 0x0900..=0x7FFF => self.cart.read_rom(addr),
      0x8000..=0x9FFF => self.ppu.read_vram(addr),
      0xA000..=0xBFFF => self.cart.read_ram(addr),
      0xC000..=0xCFFF | 0xE000..=0xEFFF => self.read_ram(addr),
      0xD000..=0xDFFF | 0xF000..=0xFDFF => self.read_bank_ram(addr),
      0xFE00..=0xFE9F => self.ppu.read_oam(addr, self.dma_on),
      0xFEA0..=0xFEFF => 0xFF,
      0xFF00..=0xFFFF => self.read_high((addr & 0xFF) as u8),
    }
  }

  fn read_high(&mut self, addr: u8) -> u8 {
    match addr {
      P1 => self.read_p1(),
      SB => self.sb,
      SC => self.sc | 0x7E,
      DIV => ((self.wide_div_counter >> 8) & 0xFF) as u8,
      TIMA => self.tima,
      TMA => self.tma,
      TAC => 0xF8 | self.tac,
      IF => self.ifr | 0xE0,
      NR10 => self.apu.read_nr10(),
      NR11 => self.apu.read_nr11(),
      NR12 => self.apu.read_nr12(),
      NR14 => self.apu.read_nr14(),
      NR21 => self.apu.read_nr21(),
      NR22 => self.apu.read_nr22(),
      NR24 => self.apu.read_nr24(),
      NR30 => self.apu.read_nr30(),
      NR32 => self.apu.read_nr32(),
      NR34 => self.apu.read_nr34(),
      NR42 => self.apu.read_nr42(),
      NR43 => self.apu.read_nr43(),
      NR44 => self.apu.read_nr44(),
      NR50 => self.apu.read_nr50(),
      NR51 => self.apu.read_nr51(),
      NR52 => self.apu.read_nr52(),
      WAV_BEGIN..=WAV_END => self.apu.read_wave_ram(addr),
      LCDC => self.ppu.read_lcdc(),
      STAT => self.ppu.read_stat(),
      SCY => self.ppu.read_scy(),
      SCX => self.ppu.read_scx(),
      LY => self.ppu.read_ly(),
      LYC => self.ppu.read_lyc(),
      DMA => self.dma,
      BGP => self.ppu.read_bgp(),
      OBP0 => self.ppu.read_obp0(),
      OBP1 => self.ppu.read_obp1(),
      WY => self.ppu.read_wy(),
      WX => self.ppu.read_wx(),
      KEY1 if self.compat_mode == CompatMode::Cgb => {
        0x7E
          | (u8::from(self.double_speed) << 7)
          | u8::from(self.double_speed_request)
      }
      VBK if self.compat_mode == CompatMode::Cgb => self.ppu.read_vbk(),
      HDMA5 if self.compat_mode == CompatMode::Cgb => {
        // active on low
        u8::from(!self.hdma_on()) << 7 | self.hdma5
      }
      BCPS if self.compat_mode == CompatMode::Cgb => self.ppu.bcp().spec(),
      BCPD if self.compat_mode == CompatMode::Cgb => self.ppu.bcp().data(),
      OCPS if self.compat_mode == CompatMode::Cgb => self.ppu.ocp().spec(),
      OCPD if self.compat_mode == CompatMode::Cgb => self.ppu.ocp().data(),
      OPRI if self.compat_mode == CompatMode::Cgb => self.ppu.read_opri(),
      SVBK if self.compat_mode == CompatMode::Cgb => self.svbk | 0xF8,
      PCM12 if self.compat_mode == CompatMode::Cgb => self.apu.pcm12(),
      PCM34 if self.compat_mode == CompatMode::Cgb => self.apu.pcm34(),
      HRAM_BEG..=HRAM_END => self.hram[(addr & 0x7F) as usize],
      IE => self.ie,
      _ => 0xFF,
    }
  }

  pub(crate) fn write_mem(&mut self, addr: u16, val: u8) {
    match addr {
      // assume bootrom doesn't write to rom
      0x0000..=0x08FF | 0x0900..=0x7FFF => self.cart.write_rom(addr, val),
      0x8000..=0x9FFF => self.ppu.write_vram(addr, val),
      0xA000..=0xBFFF => self.cart.write_ram(addr, val),
      0xC000..=0xCFFF | 0xE000..=0xEFFF => self.write_ram(addr, val),
      0xD000..=0xDFFF | 0xF000..=0xFDFF => self.write_bank_ram(addr, val),
      0xFE00..=0xFE9F => self.ppu.write_oam(addr, val, self.dma_active()),
      0xFEA0..=0xFEFF => (),
      0xFF00..=0xFFFF => self.write_high((addr & 0xFF) as u8, val),
    }
  }

  #[allow(clippy::too_many_lines, clippy::cognitive_complexity)]

  fn write_high(&mut self, addr: u8, val: u8) {
    match addr {
      P1 => self.write_joy(val),
      SB => self.sb = val,
      SC => self.sc = val,
      DIV => self.write_div(),
      TIMA => self.write_tima(val),
      TMA => self.write_tma(val),
      TAC => self.write_tac(val),
      IF => self.ifr = val & 0x1F,
      NR10 if self.apu.on() => self.apu.write_nr10(val),
      NR11 if self.apu.on() => self.apu.write_nr11(val),
      NR12 if self.apu.on() => self.apu.write_nr12(val),
      NR13 if self.apu.on() => self.apu.write_nr13(val),
      NR14 if self.apu.on() => self.apu.write_nr14(val),
      NR21 if self.apu.on() => self.apu.write_nr21(val),
      NR22 if self.apu.on() => self.apu.write_nr22(val),
      NR23 if self.apu.on() => self.apu.write_nr23(val),
      NR24 if self.apu.on() => self.apu.write_nr24(val),
      NR30 if self.apu.on() => self.apu.write_nr30(val),
      NR31 if self.apu.on() => self.apu.write_nr31(val),
      NR32 if self.apu.on() => self.apu.write_nr32(val),
      NR33 if self.apu.on() => self.apu.write_nr33(val),
      NR34 if self.apu.on() => self.apu.write_nr34(val),
      NR41 if self.apu.on() => self.apu.write_nr41(val),
      NR42 if self.apu.on() => self.apu.write_nr42(val),
      NR43 if self.apu.on() => self.apu.write_nr43(val),
      NR44 if self.apu.on() => self.apu.write_nr44(val),
      NR50 => self.apu.write_nr50(val),
      NR51 => self.apu.write_nr51(val),
      NR52 => self.apu.write_nr52(val),
      WAV_BEGIN..=WAV_END => self.apu.write_wave_ram(addr, val),
      LCDC => self.ppu.write_lcdc(val, &mut self.ifr),
      STAT => self.ppu.write_stat(val),
      SCY => self.ppu.write_scy(val),
      SCX => self.ppu.write_scx(val),
      LYC => self.ppu.write_lyc(val),
      DMA => {
        if self.dma_on {
          self.dma_restarting = true;
        }

        self.dma_cycles = -8; // two m-cycles delay
        self.dma = val;
        self.dma_addr = u16::from(val) << 8;
        self.dma_on = true;
      }
      BGP => self.ppu.write_bgp(val),
      OBP0 => self.ppu.write_obp0(val),
      OBP1 => self.ppu.write_obp1(val),
      WY => self.ppu.write_wy(val),
      WX => self.ppu.write_wx(val),
      KEY0 if self.model == Cgb && self.boot_rom.is_some() && val == 4 => {
        self.compat_mode = CompatMode::Compat;
      }
      KEY1 if self.compat_mode == CompatMode::Cgb => {
        self.double_speed_request = val & 1 != 0;
      }
      VBK if self.compat_mode == CompatMode::Cgb => self.ppu.write_vbk(val),
      0x50 => {
        if val & 1 != 0 {
          self.boot_rom = None;
        }
      }
      HDMA1 if self.compat_mode == CompatMode::Cgb => {
        self.hdma_src = (u16::from(val) << 8) | (self.hdma_src & 0xF0);
      }
      HDMA2 if self.compat_mode == CompatMode::Cgb => {
        self.hdma_src = (self.hdma_src & 0xFF00) | u16::from(val & 0xF0);
      }
      HDMA3 if self.compat_mode == CompatMode::Cgb => {
        self.hdma_dst = (u16::from(val & 0x1F) << 8) | (self.hdma_dst & 0xF0);
      }
      HDMA4 if self.compat_mode == CompatMode::Cgb => {
        self.hdma_dst = (self.hdma_dst & 0x1F00) | u16::from(val & 0xF0);
      }
      HDMA5 if self.compat_mode == CompatMode::Cgb => {
        use HdmaState::{General, Sleep, WaitHBlank};

        debug_assert!(!matches!(self.hdma_state, HdmaState::General));

        // stop current transfer
        if self.hdma_on() && val & 0x80 == 0 {
          self.hdma_state = Sleep;
          return;
        }

        self.hdma5 = val & 0x7F;
        self.hdma_len = (u16::from(self.hdma5) + 1) * 0x10;
        self.hdma_state = if val & 0x80 == 0 { General } else { WaitHBlank };
      }
      BCPS if self.compat_mode == CompatMode::Cgb => {
        self.ppu.bcp_mut().set_spec(val);
      }
      BCPD if self.compat_mode == CompatMode::Cgb => {
        self.ppu.bcp_mut().set_data(val);
      }
      OCPS if self.compat_mode == CompatMode::Cgb => {
        self.ppu.ocp_mut().set_spec(val);
      }
      OCPD if self.compat_mode == CompatMode::Cgb => {
        self.ppu.ocp_mut().set_data(val);
      }
      OPRI if self.compat_mode == CompatMode::Cgb => self.ppu.write_opri(val),
      SVBK if self.compat_mode == CompatMode::Cgb => {
        let tmp = val & 7;
        self.svbk = tmp;
        self.svbk_true =
          NonZeroU8::new(if tmp == 0 { 1 } else { tmp }).unwrap();
      }
      HRAM_BEG..=HRAM_END => self.hram[(addr & 0x7F) as usize] = val,
      IE => self.ie = val,
      _ => (),
    }
  }

  // *******
  // * DMA *
  // *******

  pub(crate) fn run_dma(&mut self) {
    if !self.dma_on {
      return;
    }

    while self.dma_cycles >= 4 {
      self.dma_cycles -= 4;

      // TODO: reading some ranges should cause problems, $DF is
      // the maximum value accesible to OAM DMA (probably reads
      // from echo RAM should work too, RESEARCH).
      // what happens if reading from IO range? (garbage? 0xff?)
      let val = self.read_mem(self.dma_addr);

      // TODO: writes from DMA can access OAM on modes 2 and 3
      // with some glitches (RESEARCH) and without trouble during
      // VBLANK (what happens in HBLANK?)
      self.ppu.write_oam_direct(self.dma_addr, val);

      self.dma_addr = self.dma_addr.wrapping_add(1);
      if self.dma_addr & 0xFF >= 0xA0 {
        self.dma_on = false;
        self.dma_restarting = false;
      }
    }
  }

  pub(crate) fn run_hdma(&mut self) {
    use HdmaState::{General, HBlankDone, Sleep, WaitHBlank};

    match self.hdma_state {
      General => (),
      WaitHBlank if self.ppu.ppu_mode() == Mode::HBlank => (),
      HBlankDone if self.ppu.ppu_mode() != Mode::HBlank => {
        self.hdma_state = WaitHBlank;
        return;
      }
      _ => return,
    }

    let len = if self.hdma_state == WaitHBlank {
      self.hdma_len -= 0x10;
      self.hdma_state = if self.hdma_len == 0 { Sleep } else { HBlankDone };
      self.hdma5 = ((self.hdma_len / 0x10).wrapping_sub(1) & 0xFF) as u8;
      0x10
    } else {
      self.hdma_state = Sleep;
      self.hdma5 = 0xFF;
      let len = self.hdma_len;
      self.hdma_len = 0;
      len
    };

    for _ in 0..len {
      // TODO: the same problems as normal DMA plus reading from
      // VRAM should copy garbage
      let val = self.read_mem(self.hdma_src);
      self.ppu.write_vram(self.hdma_dst, val);
      self.hdma_dst += 1;
      self.hdma_src += 1;
    }

    // can be outside of loop because HDMA should not
    // access IO range (clk registers, ifr,
    // etc..). If the PPU reads VRAM during an HDMA transfer it
    // should be glitchy anyways
    // TODO: check these timings
    if self.double_speed {
      self.advance_t_cycles(i32::from(len) * 2 * 2);
    } else {
      self.advance_t_cycles(i32::from(len) * 2);
    }
  }
}
