# Ceres

A (very experimental) GameBoy/Color emulator written in rust with a CLI frontend.

## Build

To build the [SameBoy](https://github.com/LIJI32/SameBoy) bootroms is necessary a C compiler as well as the [RGBDS](https://rgbds.gbdev.io/) toolchain. To build them run `make` in the `ceres_core/bootroms` directory. After that `cargo build` to build. [The nightly version of the Rust compiler is needed](https://www.oreilly.com/library/view/rust-programming-by/9781788390637/e07dc768-de29-482e-804b-0274b4bef418.xhtml).

## Run

To run a given `rom.gb` just type `cargo run rom.gb`.
To see usage of the program: `cargo run -- --help`.

## Platforms

We use SDL and OpenGL so the emulator should be compatible with almost anything,
with maybe a bit of massaging :)

## Keys

| Gameboy | Emulator  |
| ------- | --------- |
| Dpad    | WASD      |
| A       | K         |
| B       | L         |
| Start   | Return    |
| Select  | Backspace |

## Thanks

### Documentation

- [Pan Docs](https://gbdev.io/pandocs/)
- [Gameboy Development Wiki](https://gbdev.gg8.se/wiki/articles/Main_Page)
- [Gameboy Bootstrap ROM](https://gbdev.gg8.se/wiki/articles/Gameboy_Bootstrap_ROM#Contents_of_the_ROM)

### Tests

- [Game Boy Test Roms](https://github.com/c-sp/gameboy-test-roms)

### Reference

- [retrio/gb](https://github.com/retrio/gb)
- [Mooneye GB](https://github.com/Gekkio/mooneye-gb)
- [GoBoy](https://github.com/Humpheh/goboy)
- [Gameboy Emulator in C# 8](https://github.com/DaveTCode/gameboy-emulator-dotnet)
- [SameBoy](https://github.com/LIJI32/SameBoy)
