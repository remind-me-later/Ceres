# Tracing the emulator

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
- `--state bug.bess` starts from a BESS state, such as one the
  [debugger](debugger.md) saved. SameBoy saves and loads the same format, so
  both emulators can be traced from the same moment and diffed. `--model` must
  be the model the state was saved with.
- `--skip-boot` starts after the boot ROM, as Doctor's logs do. They also
  read LY as 0x90 all the time, which Ceres doesn't fake, so they match a
  trace up to the first LY read.
- `--frames N` (1 by default), `--max-lines N` and `--out FILE` set how much
  is written and where. A frame is a few megabytes.
