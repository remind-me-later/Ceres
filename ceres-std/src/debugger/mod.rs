//! A debugger for the running game, in the terminal.

pub mod expr;
pub mod symbols;

use {
    ceres_core::{AudioCallback, Gb},
    symbols::Bank,
};

/// The bank mapped where `addr` is, as symbol files number them.
#[must_use]
#[inline]
pub fn mapped_bank<A: AudioCallback>(gb: &Gb<A>, addr: u16) -> Bank {
    if gb.boot_rom_at(addr) {
        return Bank::Boot;
    }
    Bank::Number(match addr {
        0x4000..=0x7FFF => gb.rom_banks().1,
        0x8000..=0x9FFF => u32::from(gb.vram_bank()),
        0xA000..=0xBFFF => gb.ram_bank(),
        0xD000..=0xDFFF => u32::from(gb.wram_bank()),
        _ => 0,
    })
}
