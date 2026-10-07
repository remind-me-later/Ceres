//! The STAT interrupt line and the LY=LYC comparison.

use {
    super::{model_ge_cgb_d, state::State},
    crate::{
        interrupts::Interrupts,
        ppu::{Ppu, STAT_IF_HBLANK_B, STAT_IF_LYC_B, STAT_IF_OAM_B, STAT_IF_VBLANK_B, STAT_LYC_B},
    },
};

/// `mode_for_interrupt` while the line is held by the HBlank or the OAM
/// condition as VBlank starts.
pub const MODE_VBLANK_ENTRY: i8 = 4;

#[derive(Clone)]
pub struct StatIrq {
    /// `ly_for_comparison`; `-1` is SameBoy's `(uint16_t)-1`.
    pub ly_for_comparison: i32,
    /// The mode the STAT interrupt sees (-1: none).
    pub mode_for_interrupt: i8,
    pub stat_interrupt_line: bool,
    pub lyc_interrupt_line: bool,
    pub delayed_glitch_hblank_interrupt: bool,
    /// The STAT register as it was when VBlank entry began; at normal speed
    /// the CGB does not see the enables written later in the entry hold
    /// (`MODE_VBLANK_ENTRY`).
    pub entry_stat: u8,
    /// Dots until line 0's mode 2 interrupt condition pulses (0: none).
    pub line0_pulse: u8,
}

impl Default for StatIrq {
    fn default() -> Self {
        Self {
            ly_for_comparison: 0,
            mode_for_interrupt: -1,
            stat_interrupt_line: false,
            lyc_interrupt_line: false,
            delayed_glitch_hblank_interrupt: false,
            entry_stat: 0,
            line0_pulse: 0,
        }
    }
}

impl Ppu {
    pub(in crate::ppu) fn stat_update(&mut self, ints: &mut Interrupts) {
        if self.lcdc & 0x80 == 0 {
            return;
        }

        let previous = self.d.irq.stat_interrupt_line;

        // LY=LYC flag
        // `model <= CGB_C` in SameBoy's ordering: everything before CGB-D.
        let le_c = !model_ge_cgb_d(self.model) && !self.double_speed();
        if self.d.irq.ly_for_comparison != -1 || le_c {
            if self.d.irq.ly_for_comparison == i32::from(self.lyc) {
                self.d.irq.lyc_interrupt_line = true;
                self.stat |= STAT_LYC_B;
            } else {
                if self.d.irq.ly_for_comparison != -1 {
                    self.d.irq.lyc_interrupt_line = false;
                }
                self.stat &= !STAT_LYC_B;
            }
        }

        self.d.irq.stat_interrupt_line = match self.d.irq.mode_for_interrupt {
            0 => self.stat & STAT_IF_HBLANK_B != 0,
            1 => self.stat & STAT_IF_VBLANK_B != 0,
            2 => self.stat & STAT_IF_OAM_B != 0,
            MODE_VBLANK_ENTRY => {
                let stat = if self.hw_cgb() && !self.double_speed() {
                    self.d.irq.entry_stat
                } else {
                    self.stat
                };
                stat & (STAT_IF_HBLANK_B | STAT_IF_OAM_B) != 0
            }
            _ => false,
        };

        if self.stat & STAT_IF_LYC_B != 0 && self.d.irq.lyc_interrupt_line {
            self.d.irq.stat_interrupt_line = true;
        }

        if self.d.irq.stat_interrupt_line && !previous {
            ints.request_lcd();
        }
    }

    /// The DMA's last cycle: during the OAM scan edge it raises the mode 2
    /// bits (SameBoy `GB_dma_run`).
    pub fn dma_finished(&mut self, ints: &mut Interrupts) {
        if self.d.state == State::OamScanObject {
            self.stat |= 2;
            self.stat_update(ints);
        }
    }

    /// Re-evaluates the STAT interrupt line (after a DMA start).
    pub fn refresh_stat(&mut self, ints: &mut Interrupts) {
        self.stat_update(ints);
    }

    pub(in crate::ppu) fn write_lyc_reg(&mut self, val: u8, ints: &mut Interrupts) {
        let state = self.d.state;
        let cgb = self.hw_cgb();
        // These are the states around LY changes; the display routine calls
        // `stat_update` itself so LYC writes conflict on the right dot.
        if state == State::Line153LycGlitch && cgb {
            self.d.irq.ly_for_comparison = 153;
            self.stat_update(ints);
            self.d.irq.ly_for_comparison = 0;
        }
        self.lyc = val;
        if !cgb
            || !matches!(
                state,
                State::LineOamWriteLock
                    | State::VBlankLy
                    | State::Line153Compare
                    | State::Line153CompareZero
            )
        {
            if state == State::Line153LyZero && cgb {
                self.d.irq.ly_for_comparison = 153;
                self.stat_update(ints);
                self.d.irq.ly_for_comparison = -1;
            } else {
                self.stat_update(ints);
            }
        }
    }

    pub(in crate::ppu) fn write_stat_reg(&mut self, val: u8, ints: &mut Interrupts) {
        let old = self.stat;
        self.stat &= 7;
        self.stat |= val & !7;
        self.stat |= 0x80;

        if self.lcdc & 0x80 == 0 {
            // With the LCD off the LY=LYC flag stays as it was: enabling the
            // LYC interrupt while it is set raises it (on the DMG any write
            // does, with the glitch that sets all the enables).
            if old & STAT_LYC_B != 0
                && old & STAT_IF_LYC_B == 0
                && (!self.hw_cgb() || val & STAT_IF_LYC_B != 0)
            {
                ints.request_lcd();
            }
            return;
        }

        // The CGB's entry hold sees the enables written up to two dots before
        // the VBlank condition is evaluated.
        if self.d.irq.mode_for_interrupt == MODE_VBLANK_ENTRY
            && self.d.state == State::VBlankLyCompare
            && self.d.wait >= 2
        {
            self.d.irq.entry_stat = self.stat;
        }

        // On the DMG the HBlank interrupt condition begins when HBlankStart runs,
        // a dot after the mode bits change: a write in between does not see it.
        let in_hblank_gap = !self.hw_cgb()
            && self.d.state == State::HBlankStart
            && self.d.irq.mode_for_interrupt == 0;
        if in_hblank_gap {
            self.d.irq.mode_for_interrupt = 3;
        }

        // Annoying edge timing case.
        if self.double_speed()
            && self.d.state == State::OamScanObject
            && self.d.objs.index == 0
            && self.d.wait == 1
            && !self.d.half_dot
            && val & 0x20 != 0
        {
            self.d.irq.mode_for_interrupt = 2;
            self.stat_update(ints);
            self.d.irq.mode_for_interrupt = -1;
        } else {
            self.stat_update(ints);
        }

        if in_hblank_gap {
            self.d.irq.mode_for_interrupt = 0;
        }
    }
}
