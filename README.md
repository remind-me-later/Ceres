# Ceres

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

The emulator has several frontends, the most complete and recommended is the `gtk4` one. Other options are:

- `winit` a minimal cli frontend, shows an image but doesn't have any GUI, should work on Windows, Mac and Linux.
- `egui` uses the closs-platform `egui` library, should work on Windows, Mac and Linux.
- `gtk4` uses the `gtk4` toolkit, should work on Linux.

## Build

Required:

- [RGBDS toolchain](https://rgbds.gbdev.io/): To build GameBoy boot roms.

To build:

- After cloning the repo run `git submodule update --init --recursive`.
- Enter the `gb-bootroms` directory and `make`.
- In `Cargo.toml` select the frontend. For example, in case you want the gtk4 frontend use
  `default-members = ["ceres-gtk"]`, the other options are `ceres-egui` and `ceres`.
- In the root directory `cargo build`

## Quick start

- In the root directory `cargo run`.

## Debugging and Tracing

Ceres includes a comprehensive execution tracing system for debugging and performance analysis. Traces use the Chrome Trace Event Format and can be analyzed with Perfetto or Chrome's built-in tracing viewer.

**Quick Start:**

```bash
# Run tests with tracing enabled
RUST_LOG=trace cargo test test_chrome_trace_export --package ceres-test-runner --ignored

# View traces in Perfetto
# Visit https://ui.perfetto.dev and load ceres-test-runner/target/traces/*.json
```

**Documentation:**

- **[Comprehensive Tracing Guide](docs/TRACING_GUIDE.md)** - Complete guide covering trace generation, viewing, SQL analysis, and debugging workflows
- **[Test Runner Tracing](ceres-test-runner/README.md#execution-tracing)** - Quick start for test tracing
- **[SQL Query Library](examples/sql/)** - Pre-built queries for common debugging tasks (tight loops, hotspots, PPU timing, etc.)

**Features:**

- CPU execution traces with full register state
- PPU mode changes and timing
- DMA transfers
- Memory access patterns
- SQL-based trace analysis using Perfetto's trace_processor
- Zero overhead when disabled

See the [Tracing Guide](docs/TRACING_GUIDE.md) for detailed usage instructions.

## Key bindings

| Gameboy | Emulator |
| ------- | -------- |
| Dpad    | WASD     |
| A       | K        |
| B       | L        |
| Start   | M        |
| Select  | N        |

## Folder organization

- `ceres-core` contains the core emulator logic, such as cpu, apu and ppu emulation. In the future this module should
  work in no std environments.
- `ceres-std` contains code for audio and threading, for use with different frontends.
- `ceres-wgpu` contains the rendering code for the frontends using wgpu.
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
