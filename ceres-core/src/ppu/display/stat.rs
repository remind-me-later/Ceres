//! The STAT interrupt line and the LY=LYC comparison.

use {
    super::model_ge_cgb_d,
    crate::{
        interrupts::Interrupts,
        ppu::{Ppu, STAT_IF_HBLANK_B, STAT_IF_LYC_B, STAT_IF_OAM_B, STAT_IF_VBLANK_B, STAT_LYC_B},
    },
};

#[derive(Clone)]
pub struct StatIrq {
    /// `ly_for_comparison`; `-1` is SameBoy's `(uint16_t)-1`.
    pub ly_for_comparison: i32,
    /// The mode the STAT interrupt sees (-1: none).
    pub mode_for_interrupt: i8,
    pub stat_interrupt_line: bool,
    pub lyc_interrupt_line: bool,
    pub delayed_glitch_hblank_interrupt: bool,
}

impl Default for StatIrq {
    fn default() -> Self {
        Self {
            ly_for_comparison: 0,
            mode_for_interrupt: -1,
            stat_interrupt_line: false,
            lyc_interrupt_line: false,
            delayed_glitch_hblank_interrupt: false,
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
        if self.d.state == 8 {
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
        if state == 29 && cgb {
            self.d.irq.ly_for_comparison = 153;
            self.stat_update(ints);
            self.d.irq.ly_for_comparison = 0;
        }
        self.lyc = val;
        if !cgb || (state != 35 && state != 26 && state != 15 && state != 16) {
            if state == 14 && cgb {
                self.d.irq.ly_for_comparison = 153;
                self.stat_update(ints);
                self.d.irq.ly_for_comparison = -1;
            } else {
                self.stat_update(ints);
            }
        }
    }

    pub(in crate::ppu) fn write_stat_reg(&mut self, val: u8, ints: &mut Interrupts) {
        self.stat &= 7;
        self.stat |= val & !7;
        self.stat |= 0x80;

        // Annoying edge timing case.
        if self.double_speed()
            && self.d.state == 8
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
    }
}
