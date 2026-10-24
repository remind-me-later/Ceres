# Ceres

[![Test](https://github.com/remind-me-later/Ceres/actions/workflows/test.yml/badge.svg?branch=main)](https://github.com/remind-me-later/Ceres/actions/workflows/test.yml)
[![codecov](https://codecov.io/gh/remind-me-later/Ceres/branch/main/graph/badge.svg)](https://codecov.io/gh/remind-me-later/Ceres)

Some kind of GameBoy/Color emulator written in rust.

![logo](media/logo/ceres.webp)

## Screenshots

<p align="center" width="100%">
    <img width="25%" alt="Tetris" src="media/screenshots/tetris.webp">
    <img width="25%" alt="Pokémon Silver" src="media/screenshots/poke_silver.webp">
    <img width="25%" alt="Pokémon Crystal" src="media/screenshots/poke_crystal.webp">
    <img width="25%" alt="Zelda Link's Awakening" src="media/screenshots/links_awakening.webp">
    <img width="25%" alt="Zelda Oracle of Ages" src="media/screenshots/oracle_of_ages.webp">
</p>

## Frontends

The emulator has several frontends, the most complete and recommended is the
`gtk4` one. Other options are:

- `winit` a minimal cli frontend, shows an image but doesn't have any GUI,
  should work on Windows, Mac and Linux.
- `egui` uses the closs-platform `egui` library, should work on Windows, Mac and
  Linux.
- `gtk4` uses the `gtk4` toolkit, should work on Linux.

## Build

Required:

- [RGBDS toolchain](https://rgbds.gbdev.io/): To build GameBoy boot roms.

To build:

- After cloning the repo run `git submodule update --init --recursive`.
- Enter the `gb-bootroms` directory and `make`.
- In `Cargo.toml` select the frontend. For example, in case you want the gtk4
  frontend use `default-members = ["ceres-gtk"]`, the other options are
  `ceres-egui` and `ceres-winit`.
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

## Testing

The integration tests run the emulator on the ROMs of the
[Game Boy Test Roms](https://github.com/c-sp/gameboy-test-roms) collection
and check the registers, the serial output or the screen against the real
hardware's:

```sh
cargo test --package ceres-test-runner
```

| Suite                               | What it checks                                                                   | Passing      |
| ----------------------------------- | -------------------------------------------------------------------------------- | ------------ |
| Mooneye (and the Wilbertpol fork)   | CPU, timers, interrupts, OAM DMA, PPU timing, MBCs, boot state                   | 164 of 165   |
| Blargg                              | CPU instructions and timing, memory timing, interrupts, HALT bug, sound, OAM bug | 117 of 119   |
| SameSuite                           | APU, DMA, interrupts, PPU, SGB                                                   | 204 of 204   |
| Gambatte                            | LCD and STAT timing, timers, DMA, interrupts, HALT, sound                        | 4674 of 4674 |
| GBMicrotest                         | CPU, timer, DMA and PPU timing, to the cycle                                     | 480 of 482   |
| AGE                                 | LY, STAT, OAM and VRAM access, HALT, speed switch, mode 3 rendering              | 77 of 84     |
| Mealybug Tearoom Tests              | PPU registers changed in the middle of a line                                    | 66 of 89     |
| dmg-acid2, cgb-acid2, cgb-acid-hell | PPU rendering, pixel for pixel                                                   | 4 of 4       |
| Scribbltests, Turtle Tests          | STAT, LY=LYC and window timing                                                   | 14 of 14     |
| rtc3test, mbc3-tester               | MBC3 clock and banks                                                             | 12 of 12     |
| Little Things GB                    | First frame after the LCD turns on, joypad interrupt timing                      | 3 of 6       |
| BullyGB                             | Edge cases of the hardware                                                       | 1 of 2       |
| strikethrough                       | An OAM DMA quirk                                                                 | 3 of 3       |

A test is one ROM on one model, so a ROM that is checked on several models
counts once for each.

The tests that don't pass are marked `#[ignore]` in `ceres-test-runner/tests`,
each with its reason: most fail the same way in SameBoy, the reference Ceres
follows; a few are gaps (the MGB's OAM DMA quirk during HALT, seven Mealybug
ROMs not investigated yet); and 13 Mealybug ROMs have no reference screenshot
upstream.

## Documentation

- [Debugger](doc/debugger.md): a console debugger in the style of SameBoy's.
- [Tracing](doc/tracing.md): a per-instruction trace in Gameboy Doctor's
  format, to diff against other emulators.
- [CGB hardware tests](doc/cgb-hardware-tests.md): what is left to measure on
  a real CGB.

## Folder organization

- `ceres-core` contains the core emulator logic, such as cpu, apu and ppu
  emulation. In the future this module should work in no std environments.
- `ceres-std` contains code for audio, threading and rendering, for use with
  different frontends.
- `ceres-winit` contains the `winit` frontend.
- `ceres-egui` contains the `egui` frontend.
- `ceres-gtk` contains the `gtk` frontend.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the development guide, code style,
and detailed testing instructions.

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
- [MetroBoy/GateBoy](https://github.com/aappleby/metroboy/tree/master)
- [Furrtek schematics](https://github.com/furrtek/DMG-CPU-Inside)
