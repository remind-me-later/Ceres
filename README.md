# Ceres

A (very experimental) GameBoy/Color emulator written in rust.

![logo](https://github.com/remind-me-later/ceres-images/blob/main/ceres.webp?raw=true)

## Screenshots

<p align="center" width="100%">
    <img width="20%" src="https://github.com/remind-me-later/ceres-images/blob/main/kirby_dream.webp?raw=true"> 
    <img width="20%" src="https://github.com/remind-me-later/ceres-images/blob/main/pokemon_silver.webp?raw=true"> 
    <img width="20%" src="https://github.com/remind-me-later/ceres-images/blob/main/pokemon_crystal.webp?raw=true">   
    <img width="20%" src="https://github.com/remind-me-later/ceres-images/blob/main/zelda_yume_1.webp?raw=true"> 
    <img width="20%" src="https://github.com/remind-me-later/ceres-images/blob/main/zelda_yume_2.webp?raw=true"> 
</p>

## Build

Required:

- [RGBDS toolchain](https://rgbds.gbdev.io/)

To build just:

- Enter the `gb-bootroms` directory and `make`.
- In the root directory `cargo build --release --bin ceres_gtk`

## Quick start

- In the root directory `cargo run --release --bin ceres_gtk`

## Key bindings

| Gameboy | Emulator |
| ------- | -------- |
| Dpad    | WASD     |
| A       | K        |
| B       | L        |
| Start   | M        |
| Select  | N        |

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
