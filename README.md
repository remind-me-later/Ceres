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

## Testing

To run the integration tests:

`cargo test --package ceres-test-runner`

## Key bindings

| Gameboy | Emulator |
| ------- | -------- |
| Dpad    | WASD     |
| A       | K        |
| B       | L        |
| Start   | M        |
| Select  | N        |

## Debugging

Ceres has a console debugger in the style of SameBoy's. Build a frontend with
the `debugger` feature and start it with `--debugger` from a terminal: the
game runs in its window and the console reads commands in the terminal.

```sh
cargo run -p ceres-gtk --features debugger -- --debugger game.gb
```

Ctrl-C stops the game, `continue` runs it again, Ctrl-D closes the console
and lets the game run on. `help` lists the commands, `help <command>`
explains one. Commands can be shortened as in SameBoy (`c`, `s`, `n`, `bt`,
`x`), and an empty line repeats the last one.

| Command | Does |
| --- | --- |
| `continue`, `interrupt` | run, stop |
| `step`, `next`, `finish` | one instruction, over calls, out of the function |
| `breakpoint <expr> [to <end>] [if <cond>]`, `delete [id]` | stop before an instruction |
| `watch/r`, `/w`, `/rw <expr> [to <end>] [if <cond>]`, `unwatch [id]` | stop after a read or write; `old` and `new` in the condition |
| `list`, `softbreak on` | breakpoints and watchpoints; stop at `ld b, b` |
| `print/x`, `/d`, `/b`, `/a <expr>` | evaluate, or assign: `p a = 5`, `p [wCounter] = 3` |
| `examine/<count> <expr>`, `disassemble/<count> [expr]` | memory, code |
| `registers`, `backtrace` | the CPU, the calls that led here |
| `lcd`, `timer`, `dma`, `ints`, `cartridge` | the hardware's state, the banks mapped |
| `ticks`, `reset`, `save <file>`, `load <file>` | time between stops, reset, BESS states |

Addresses are written as in symbol files, a bank and an address in hex:
`39:4579`, `BOOT:0064`. Elsewhere numbers are decimal unless `$hex` or
`%binary`. Expressions have registers, symbols, `[x]` and `{x}` for the byte
and word at x, and C's operators.

If an [RGBDS](https://rgbds.gbdev.io/) symbol file is next to the ROM
(`game.sym` for `game.gb`, from `rgblink -n game.sym`), labels work
everywhere: `b SplashScreen`, `d PlayMusic`, `w/w wPlayerName`. A breakpoint
or watchpoint on a label stops only in the label's bank (or in the boot ROM,
for `BOOT:` labels); on a plain address it stops in any bank.

`save bug.bess` keeps the moment as a BESS state, which SameBoy and the
trace tool below can load too. A loaded state is a starting point, not an
exact rewind: BESS doesn't hold every internal phase of the PPU.

## Tracing the emulator

`ceres-test-runner`'s `trace` example writes a line per instruction in
[Gameboy Doctor](https://github.com/robert/gameboy-doctor)'s format, the CPU's
state before it runs, as fast as the machine allows:

```sh
cargo run -p ceres-test-runner --example trace -- game.gb --frames 2 --timing --io
```

```text
A:FF F:00 B:FB C:1F D:D0 E:00 H:C7 L:B1 SP:DFF3 PC:C365 PCMEM:F0,44,FE,90 CY:264520 LY:135 DOT:416
  R FF44=87 CY:264536 LY:135 DOT:424
```

- `--timing` adds the clock (half dots) and where the display is.
- `--io` adds the instruction's I/O register accesses.
- `--state bug.bess` starts from a BESS state, such as one the debugger
  saved. SameBoy saves and loads the same format, so both emulators can be
  traced from the same moment and diffed. `--model` must be the model the
  state was saved with.
- `--skip-boot` starts after the boot ROM, as Doctor's logs do. They also
  read LY as 0x90 all the time, which Ceres doesn't fake, so they match a
  trace up to the first LY read.
- `--frames N` (1 by default), `--max-lines N` and `--out FILE` set how much
  is written and where. A frame is a few megabytes.

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
