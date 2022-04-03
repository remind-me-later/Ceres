# Ceres

GameBoy/Color emulator written in rust with a CLI frontend.

## Compatibility

Passes all of [blargg's test ROMs](https://gbdev.gg8.se/wiki/articles/Test_ROMs#Blargg.27s_tests) and many of [mooneye-gb](https://github.com/Gekkio/mooneye-gb) tests, so compatibility with original GameBoy games should be pretty high.

Some GameBoy Color games work (all of the Pokemon games, Link's Awakening DX and Toy Story Racer for example) but there are some that show very broken graphics (Mario Tennis, Hamtaro).

## Build

To build the [SameBoy](https://github.com/LIJI32/SameBoy) bootroms is necessary a C compiler as well as the [RGBDS](https://rgbds.gbdev.io/) toolchain. By default the compiler searchs for the compiled boot roms in the `BootROMs/build/bin` directory, to build them run `make` in the `BootROMs` directory. You can also provide your own binary boot roms by passing the `-b` flag to the program.

To build the emulator `cargo build` should suffice.

## Platforms

The project is developed in Linux but all graphics and sound libraries are cross compatible with all major operating systems so it should be easy to build for them, although it's not tested.

## Keys

| Gameboy | Emulator  |
| ------- | --------- |
| Dpad    | WASD      |
| A       | K         |
| B       | L         |
| Start   | Return    |
| Select  | Backspace |

## Documentation used

- [Pan Docs](https://gbdev.io/pandocs/)
- [Gameboy Development Wiki](https://gbdev.gg8.se/wiki/articles/Main_Page)
- [Gameboy Bootstrap ROM](https://gbdev.gg8.se/wiki/articles/Gameboy_Bootstrap_ROM#Contents_of_the_ROM)

## Thanks

- [retrio/gb](https://github.com/retrio/gb)
- [Mooneye GB](https://github.com/Gekkio/mooneye-gb)
- [GoBoy](https://github.com/Humpheh/goboy)
- [Gameboy Emulator in C# 8](https://github.com/DaveTCode/gameboy-emulator-dotnet)
- [SameBoy](https://github.com/LIJI32/SameBoy)
- [Game Boy Test Roms](https://github.com/c-sp/gameboy-test-roms)
