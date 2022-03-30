mod color;
mod mode;
mod palette;
mod pixel_buffer;
mod registers;
mod sprites;
mod vram;

use self::vram::VramBankRegister;
use crate::interrupts::{Interrupt, InterruptController};
use bitflags::bitflags;
use color::Color;
use core::cmp::Ordering;
pub use mode::PpuMode;
use palette::MonochromeColorIndex;
pub use palette::MonochromePaletteColors;
pub use pixel_buffer::PixelData;
use registers::{Lcdc, Registers, Stat};
use sprites::{ObjectAttributeMemory, SpriteAttributes, SpriteFlags};
use stackvec::StackVec;
use vram::VramBank;

pub const SCREEN_WIDTH: u8 = 160;
pub const SCREEN_HEIGHT: u8 = 144;
pub const SCANLINES_PER_FRAME: u8 = 154;

const SCREEN_PIXELS: u16 = SCREEN_WIDTH as u16 * SCREEN_HEIGHT as u16;
const ACCESS_OAM_CYCLES: i16 = 21 * 4;
const ACCESS_VRAM_CYCLES: i16 = 43 * 4;
const HBLANK_CYCLES: i16 = 50 * 4;
const VBLANK_LINE_CYCLES: i16 = 114 * 4;

bitflags! {
    struct BgAttributes: u8{
        const PALETTE_NUMBER   = 0b0000_0111;
        const VRAM_BANK_NUMBER = 0b0000_1000;
        const X_FLIP           = 0b0010_0000;
        const Y_FLIP           = 0b0100_0000;
        const BG_TO_OAM_PR     = 0b1000_0000;
    }
}

#[derive(Clone, Copy)]
pub enum PpuRegister {
    Lcdc,
    Stat,
    Scy,
    Scx,
    Ly,
    Lyc,
    Wy,
    Wx,
    Bgp,
    Obp0,
    Obp1,
    // cgb
    Bcps,
    Bcpd,
    Ocps,
    Ocpd,
    Opri,
}

#[derive(Clone, Copy)]
pub enum PpuIO {
    PpuRegister(PpuRegister),
    Vram { address: u16 },
    VramBank,
    Oam { address: u16 },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScanlineBackgroundPriority {
    SpritesOnTop,
    BackgroundOnTop,
    Normal,
}

pub struct Ppu {
    registers: Registers,
    monochrome_palette_colors: MonochromePaletteColors,
    vram: VramBank,
    oam: ObjectAttributeMemory,
    cycles: i16,
    pixel_data: PixelData,
    frame_used_window: bool,
    scanline_used_window: bool,
    window_lines_skipped: u16,
    is_frame_done: bool,
    do_render: bool,
}

impl Ppu {
    pub fn new(boot_rom: bool, monochrome_palette_colors: MonochromePaletteColors) -> Self {
        Self {
            registers: Registers::new(boot_rom),
            monochrome_palette_colors,
            vram: VramBank::new(),
            oam: ObjectAttributeMemory::new(),
            pixel_data: PixelData::new(),
            cycles: ACCESS_OAM_CYCLES,
            frame_used_window: false,
            window_lines_skipped: 0,
            scanline_used_window: false,
            is_frame_done: false,
            do_render: true,
        }
    }

    pub fn do_render(&mut self) {
        self.do_render = true
    }

    pub fn dont_render(&mut self) {
        self.do_render = false
    }

    pub fn mut_pixel_data(&mut self) -> &mut PixelData {
        &mut self.pixel_data
    }

    pub fn reset_frame_done(&mut self) {
        self.is_frame_done = false;
    }

    pub fn is_frame_done(&self) -> bool {
        self.is_frame_done
    }

    pub fn is_enabled(&self) -> bool {
        self.registers.lcdc().contains(Lcdc::LCD_ENABLE)
    }

    pub fn read(&mut self, io: PpuIO) -> u8 {
        let mode = self.registers.stat().mode();

        match io {
            PpuIO::PpuRegister(register) => self.registers.read(register),
            PpuIO::Vram { address } => {
                if mode == PpuMode::AccessVram {
                    0xff
                } else {
                    self.vram.read(address)
                }
            }
            PpuIO::VramBank => self.vram.bank(),
            PpuIO::Oam { address } => {
                if mode == PpuMode::AccessVram || mode == PpuMode::AccessOam {
                    0xff
                } else {
                    self.oam.read((address & 0xff) as u8)
                }
            }
        }
    }

    pub fn write(&mut self, io: PpuIO, val: u8) {
        let mode = self.registers.stat().mode();

        match io {
            PpuIO::PpuRegister(register) => self.registers.write(register, val, &mut self.cycles),
            PpuIO::Vram { address } => {
                if mode != PpuMode::AccessVram {
                    self.vram.write(address, val);
                }
            }
            PpuIO::VramBank => self.vram.set_bank(val),
            PpuIO::Oam { address } => {
                if !(mode == PpuMode::AccessVram || mode == PpuMode::AccessOam) {
                    self.oam.write((address & 0xff) as u8, val);
                }
            }
        }
    }

    fn switch_mode(&mut self, mode: PpuMode, interrupt_controller: &mut InterruptController) {
        self.registers.mut_stat().set_mode(mode);
        let scx = self.registers.scx();
        self.cycles += mode.cycles(scx);
        let stat = self.registers.stat();

        match mode {
            PpuMode::AccessOam => {
                if stat.contains(Stat::OAM_INTERRUPT) {
                    interrupt_controller.request(Interrupt::LCD_STAT);
                }

                self.scanline_used_window = false;
            }
            PpuMode::VBlank => {
                interrupt_controller.request(Interrupt::VBLANK);

                if stat.contains(Stat::VBLANK_INTERRUPT) {
                    interrupt_controller.request(Interrupt::LCD_STAT);
                }

                if stat.contains(Stat::OAM_INTERRUPT) {
                    interrupt_controller.request(Interrupt::LCD_STAT);
                }

                self.window_lines_skipped = 0;
                self.frame_used_window = false;
            }
            PpuMode::AccessVram | PpuMode::HBlank => (),
        }
    }

    pub fn mode(&self) -> PpuMode {
        self.registers.stat().mode()
    }

    pub fn tick(
        &mut self,
        interrupt_controller: &mut InterruptController,
        color_mode: bool,
        microseconds_elapsed_times_16: u8,
    ) {
        if !self.registers.lcdc().contains(Lcdc::LCD_ENABLE) {
            return;
        }

        self.cycles -= i16::from(microseconds_elapsed_times_16);
        let stat = self.registers.stat();

        if self.cycles <= 4 && stat.mode() == PpuMode::AccessVram {
            // STAT mode=0 interrupt happens one cycle before the actual mode switch!
            if stat.contains(Stat::HBLANK_INTERRUPT) {
                interrupt_controller.request(Interrupt::LCD_STAT);
            }
        }

        if self.cycles > 0 {
            return;
        }

        match stat.mode() {
            PpuMode::AccessOam => self.switch_mode(PpuMode::AccessVram, interrupt_controller),
            PpuMode::AccessVram => {
                if self.do_render {
                    self.draw_line(color_mode);
                }
                self.switch_mode(PpuMode::HBlank, interrupt_controller);
            }
            PpuMode::HBlank => {
                let ly = self.registers.mut_ly();
                *ly += 1;
                if *ly < 144 {
                    self.switch_mode(PpuMode::AccessOam, interrupt_controller);
                } else {
                    self.switch_mode(PpuMode::VBlank, interrupt_controller);
                }
                self.check_compare_interrupt(interrupt_controller);
            }
            PpuMode::VBlank => {
                let ly = self.registers.mut_ly();
                *ly += 1;
                if *ly > 153 {
                    *ly = 0;
                    self.switch_mode(PpuMode::AccessOam, interrupt_controller);
                    self.is_frame_done = true;
                } else {
                    self.cycles += VBLANK_LINE_CYCLES;
                }
                self.check_compare_interrupt(interrupt_controller);
            }
        };
    }

    fn check_compare_interrupt(&mut self, interrupt_controller: &mut InterruptController) {
        if self.registers.is_on_coincidence_scanline() {
            self.registers.mut_stat().insert(Stat::LY_EQUALS_LYC);
            if self
                .registers
                .stat()
                .contains(Stat::LY_EQUALS_LYC_INTERRUPT)
            {
                interrupt_controller.request(Interrupt::LCD_STAT);
            }
        } else {
            self.registers.mut_stat().remove(Stat::LY_EQUALS_LYC);
        }
    }

    fn draw_line(&mut self, color_mode: bool) {
        let ly = self.registers.ly();
        let scy = self.registers.scy();
        let scx = self.registers.scx();
        let lcdc = self.registers.lcdc();
        let bgp = self.registers.bgp();
        let index_start = SCREEN_WIDTH as usize * ly as usize;
        let mut bg_priority = [ScanlineBackgroundPriority::Normal; SCREEN_WIDTH as usize];

        // draw background
        if lcdc.background_enabled(color_mode) {
            let tile_map_address = lcdc.bg_tile_map_address();
            let y = ly.wrapping_add(scy);
            let row = (y / 8) as u16 * 32;
            let line = ((y % 8) * 2) as u16;

            for i in 0..SCREEN_WIDTH {
                let x = i.wrapping_add(scx);
                let col = (x / 8) as u16;

                let tile_num_address = tile_map_address + row + col;

                let tile_number = self
                    .vram
                    .get_bank(tile_num_address, VramBankRegister::Bank0);

                let background_attributes = if color_mode {
                    BgAttributes::from_bits_truncate(
                        self.vram
                            .get_bank(tile_num_address, VramBankRegister::Bank1),
                    )
                } else {
                    BgAttributes::empty()
                };

                let tile_data_address = if background_attributes.contains(BgAttributes::Y_FLIP) {
                    lcdc.tile_data_address(tile_number) + 14 - line
                } else {
                    lcdc.tile_data_address(tile_number) + line
                };

                let data1 = self.vram.get_bank(
                    tile_data_address - 0x8000,
                    background_attributes
                        .contains(BgAttributes::VRAM_BANK_NUMBER)
                        .into(),
                );

                let data2 = self.vram.get_bank(
                    tile_data_address + 1 - 0x8000,
                    background_attributes
                        .contains(BgAttributes::VRAM_BANK_NUMBER)
                        .into(),
                );

                let color_bit = 1
                    << if background_attributes.contains(BgAttributes::X_FLIP) {
                        x % 8
                    } else {
                        7 - (x % 8)
                    };

                let color_number =
                    (((data2 & color_bit != 0) as u8) << 1) | (data1 & color_bit != 0) as u8;

                let color = if color_mode {
                    self.registers
                        .cgb_bg_palette()
                        .get_color(background_attributes.bits() & 0x7, color_number)
                } else {
                    let monochrome_index = MonochromeColorIndex::from(color_number);
                    self.monochrome_palette_colors
                        .get_color(bgp.color_index(monochrome_index))
                };

                self.pixel_data
                    .set_pixel_color(index_start + i as usize, color);

                bg_priority[i as usize] = if color_number == 0 {
                    ScanlineBackgroundPriority::SpritesOnTop
                } else if background_attributes.contains(BgAttributes::BG_TO_OAM_PR) {
                    ScanlineBackgroundPriority::BackgroundOnTop
                } else {
                    ScanlineBackgroundPriority::Normal
                };
            }
        }

        // draw window
        let wy = self.registers.wy();

        if lcdc.window_enabled(color_mode) && wy <= ly {
            let tile_map_address = lcdc.window_tile_map_address();
            let wx = self.registers.wx().saturating_sub(7);
            let y = ((ly - wy) as u16).wrapping_sub(self.window_lines_skipped) as u8;
            let row = (y / 8) as u16 * 32;
            let line = ((y % 8) * 2) as u16;

            for i in wx..SCREEN_WIDTH {
                self.frame_used_window = true;
                self.scanline_used_window = true;

                let x = i.wrapping_sub(wx);
                let col = (x / 8) as u16;

                let tile_num_address = tile_map_address + row + col;
                let tile_number = self
                    .vram
                    .get_bank(tile_num_address, VramBankRegister::Bank0);

                let background_attributes = if color_mode {
                    BgAttributes::from_bits_truncate(
                        self.vram
                            .get_bank(tile_num_address, VramBankRegister::Bank1),
                    )
                } else {
                    BgAttributes::empty()
                };

                let tile_data_address = if background_attributes.contains(BgAttributes::Y_FLIP) {
                    lcdc.tile_data_address(tile_number) + 14 - line
                } else {
                    lcdc.tile_data_address(tile_number) + line
                };

                let data1 = self.vram.get_bank(
                    tile_data_address - 0x8000,
                    background_attributes
                        .contains(BgAttributes::VRAM_BANK_NUMBER)
                        .into(),
                );

                let data2 = self.vram.get_bank(
                    tile_data_address + 1 - 0x8000,
                    background_attributes
                        .contains(BgAttributes::VRAM_BANK_NUMBER)
                        .into(),
                );

                let color_bit = 1
                    << if background_attributes.contains(BgAttributes::X_FLIP) {
                        x % 8
                    } else {
                        7 - (x % 8)
                    };

                let color_number =
                    (((data2 & color_bit != 0) as u8) << 1) | (data1 & color_bit != 0) as u8;

                let color = if color_mode {
                    self.registers
                        .cgb_bg_palette()
                        .get_color(background_attributes.bits() & 0x7, color_number)
                } else {
                    let monochrome_index = MonochromeColorIndex::from(color_number);
                    self.monochrome_palette_colors
                        .get_color(bgp.color_index(monochrome_index))
                };

                bg_priority[i as usize] = if color_number == 0 {
                    ScanlineBackgroundPriority::SpritesOnTop
                } else if background_attributes.contains(BgAttributes::BG_TO_OAM_PR) {
                    ScanlineBackgroundPriority::BackgroundOnTop
                } else {
                    ScanlineBackgroundPriority::Normal
                };

                self.pixel_data
                    .set_pixel_color(index_start + i as usize, color);
            }
        }

        if self.frame_used_window && !self.scanline_used_window {
            self.window_lines_skipped += 1;
        }

        let mut sprites_to_draw: StackVec<[(usize, SpriteAttributes); 10]>;

        // draw sprites
        if lcdc.contains(Lcdc::OBJECTS_ENABLED) {
            let large_sprites = lcdc.contains(Lcdc::LARGE_SPRITES);
            let sprite_height = if large_sprites { 16 } else { 8 };

            sprites_to_draw = self
                .oam
                .sprite_attributes_iterator()
                .filter(|sprite| ly.wrapping_sub(sprite.y()) < sprite_height)
                .take(10)
                .enumerate()
                .collect();

            if color_mode && self.registers.prioritize_by_oam() {
                sprites_to_draw.sort_unstable_by(|(a_index, a), (b_index, b)| {
                    match a_index.cmp(b_index) {
                        Ordering::Equal => a.x().cmp(&b.x()),
                        other => other.reverse(),
                    }
                });
            } else {
                sprites_to_draw.sort_unstable_by(|(a_index, a), (b_index, b)| {
                    match a.x().cmp(&b.x()) {
                        Ordering::Equal => a_index.cmp(b_index).reverse(),
                        other => other.reverse(),
                    }
                });
            }

            for (_, sprite) in sprites_to_draw {
                let tile_number = if large_sprites {
                    sprite.tile_index() & !1
                } else {
                    sprite.tile_index()
                };

                let tile_data_address = (tile_number as u16 * 16).wrapping_add(
                    if sprite.flags().contains(SpriteFlags::FLIP_Y) {
                        (sprite_height as u16 - 1)
                            .wrapping_sub((ly.wrapping_sub(sprite.y())) as u16)
                            * 2
                    } else {
                        ly.wrapping_sub(sprite.y()) as u16 * 2
                    },
                );

                let data1 = self.vram.get_bank(
                    tile_data_address,
                    sprite.flags().contains(SpriteFlags::TILE_VRAM_BANK).into(),
                );

                let data2 = self.vram.get_bank(
                    tile_data_address + 1,
                    sprite.flags().contains(SpriteFlags::TILE_VRAM_BANK).into(),
                );

                for xi in (0..8).rev() {
                    let target_x = sprite.x().wrapping_add(7 - xi);

                    if target_x >= SCREEN_WIDTH {
                        continue;
                    }

                    if bg_priority[target_x as usize] == ScanlineBackgroundPriority::BackgroundOnTop
                        && !self
                            .registers
                            .lcdc()
                            .cgb_sprite_master_priority_on(color_mode)
                    {
                        continue;
                    }

                    let color_bit = 1
                        << if sprite.flags().contains(SpriteFlags::FLIP_X) {
                            7 - xi
                        } else {
                            xi
                        };

                    let color_number =
                        (((data2 & color_bit != 0) as u8) << 1) | (data1 & color_bit != 0) as u8;

                    if color_number == 0 {
                        continue;
                    }

                    let color = if color_mode {
                        let cgb_palette = sprite.cgb_palette();
                        self.registers
                            .cgb_sprite_palette()
                            .get_color(cgb_palette, color_number)
                    } else {
                        let palette = if sprite.flags().contains(SpriteFlags::NON_CGB_PALETTE) {
                            self.registers.obp1()
                        } else {
                            self.registers.obp0()
                        };
                        let monochrome_index = MonochromeColorIndex::from(color_number);
                        self.monochrome_palette_colors
                            .get_color(palette.color_index(monochrome_index))
                    };

                    if !self
                        .registers
                        .lcdc()
                        .cgb_sprite_master_priority_on(color_mode)
                        && sprite.flags().contains(SpriteFlags::BG_WIN_OVER_OBJ)
                        && bg_priority[target_x as usize] == ScanlineBackgroundPriority::Normal
                    {
                        continue;
                    }

                    self.pixel_data
                        .set_pixel_color(index_start + target_x as usize, color);
                }
            }
        }
    }
}
