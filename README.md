# Ceres

A (very experimental) GameBoy/Color emulator written in rust.

![logo](https://github.com/remind-me-later/ceres-images/blob/main/ceres.webp?raw=true)

## Screenshots

<p align="center" width="100%">
    <img width="25%" alt="Kirby's Dream Land" src="https://github.com/remind-me-later/ceres-images/blob/main/kirby_dream.webp?raw=true">
    <img width="25%" alt="Pokémon Silver" src="https://github.com/remind-me-later/ceres-images/blob/main/pokemon_silver.webp?raw=true">
    <img width="25%" alt="Pokémon Crystal" src="https://github.com/remind-me-later/ceres-images/blob/main/pokemon_crystal.webp?raw=true">
    <img width="25%" alt="Zelda Link's Awakening Intro" src="https://github.com/remind-me-later/ceres-images/blob/main/zelda_yume_1.webp?raw=true">
    <img width="25%" alt="Zelda Link's Awakening Title" src="https://github.com/remind-me-later/ceres-images/blob/main/zelda_yume_2.webp?raw=true">
</p>

## Frontends

The emulator has 3 frontends:

- `winit` a minimal cli frontend, shows an image but doesn't have any GUI,
  should work on Windows, Mac and Linux.
- `egui` uses the closs-platform `egui` library, should work on Windows,
  Mac and Linux.
- `gtk4` uses the `gtk4` toolkit, should work on Linux.

## Build

Required:

- [RGBDS toolchain](https://rgbds.gbdev.io/): To build GameBoy boot roms.

To build:

- After cloning the repo run `git submodule update --init --recursive`.
- Enter the `gb-bootroms` directory and `make`.
- In `Cargo.toml` select the frontend.
  For example, in case you want the gtk4 frontend use `default-members = ["ceres-gtk"]`,
  the other options are `ceres-egui` and `ceres`.
- In the root directory `cargo build`

## Quick start

- In the root directory `cargo run`.

## Key bindings

| Gameboy | Emulator |
| ------- | -------- |
| Dpad    | WASD     |
| A       | K        |
| B       | L        |
| Start   | M        |
| Select  | N        |

## Folder organization

- `ceres-core` contains the core emulator logic, such as cpu, apu and ppu emulation.
  In the future this module should work in no std environments.
- `ceres-std` contains code for audio and threading, for use with different frontends.
- `ceres-wgpu` containd the rendering code for the frontends using wgpu.
- `ceres-winit` contains the `winit` frontend.
- `ceres-egui` contains the `egui` frontend.
- `ceres-gtk` contains the `gtk` frontend.

## Thanks

### Documentation

- [Pan Docs](https://gbdev.io/pandocs/)
- [Gameboy Bootstrap ROM](https://gbdev.gg8.se/wiki/articles/Gameboy_Bootstrap_ROM#Contents_of_the_ROM)
- [Gameboy Development Wiki](https://gbdev.gg8.se/wiki/articles/Main_Page)

### Tests

- [Game Boy Test Roms](https://github.com/c-sp/gameboy-test-roms)

### Reference

- [Gameboy Emulator in C# 8](https://github.com/DaveTCode/gameboy-emulator-dotnet)
- [GoBoy](https://github.com/Humpheh/goboy)
- [Mooneye GB](https://github.com/Gekkio/mooneye-gb)
- [retrio/gb](https://github.com/retrio/gb)
- [SameBoy](https://github.com/LIJI32/SameBoy)
- [GiiBiiAdvance](https://github.com/AntonioND/giibiiadvance)
