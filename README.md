# Ceres

![logo](https://github.com/remind-me-later/ceres-images/blob/main/ceres.webp?raw=true)

A (very experimental) GameBoy/Color emulator written in rust.

## Screenshots

<p align="center" width="100%">
    <img width="30%" src="https://github.com/remind-me-later/ceres-images/blob/main/kirby_dream.webp?raw=true"> 
    <img width="30%" src="https://github.com/remind-me-later/ceres-images/blob/main/pokemon_silver.webp?raw=true"> 
    <img width="30%" src="https://github.com/remind-me-later/ceres-images/blob/main/pokemon_crystal.webp?raw=true">   
    <img width="30%" src="https://github.com/remind-me-later/ceres-images/blob/main/zelda_yume_1.webp?raw=true"> 
    <img width="30%" src="https://github.com/remind-me-later/ceres-images/blob/main/zelda_yume_2.webp?raw=true"> 
</p>

## Build

Required:

- A C compiler
- [RGBDS toolchain](https://rgbds.gbdev.io/)

To build just `make`.

## Quick start

- Enter the `emulator` directory.
- To run a given `rom.gb` just type `cargo run --release rom.gb`.
- To see usage of the program: `cargo run --release -- --help`.

## Thanks

### Documentation

- [Gameboy Bootstrap ROM](https://gbdev.gg8.se/wiki/articles/Gameboy_Bootstrap_ROM#Contents_of_the_ROM)
- [Gameboy Development Wiki](https://gbdev.gg8.se/wiki/articles/Main_Page)
- [Pan Docs](https://gbdev.io/pandocs/)

### Tests

- [Game Boy Test Roms](https://github.com/c-sp/gameboy-test-roms)

### Reference

- [Gameboy Emulator in C# 8](https://github.com/DaveTCode/gameboy-emulator-dotnet)
- [GoBoy](https://github.com/Humpheh/goboy)
- [Mooneye GB](https://github.com/Gekkio/mooneye-gb)
- [retrio/gb](https://github.com/retrio/gb)
- [SameBoy](https://github.com/LIJI32/SameBoy)
