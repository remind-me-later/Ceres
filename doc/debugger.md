# Debugger

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

| Command                                                              | Does                                                         |
| -------------------------------------------------------------------- | ------------------------------------------------------------ |
| `continue`, `interrupt`                                              | run, stop                                                    |
| `step`, `next`, `finish`                                             | one instruction, over calls, out of the function             |
| `breakpoint <expr> [to <end>] [if <cond>]`, `delete [id]`            | stop before an instruction                                   |
| `watch/r`, `/w`, `/rw <expr> [to <end>] [if <cond>]`, `unwatch [id]` | stop after a read or write; `old` and `new` in the condition |
| `list`, `softbreak on`                                               | breakpoints and watchpoints; stop at `ld b, b`               |
| `print/x`, `/d`, `/b`, `/a <expr>`                                   | evaluate, or assign: `p a = 5`, `p [wCounter] = 3`           |
| `examine/<count> <expr>`, `disassemble/<count> [expr]`               | memory, code                                                 |
| `registers`, `backtrace`                                             | the CPU, the calls that led here                             |
| `lcd`, `timer`, `dma`, `ints`, `cartridge`                           | the hardware's state, the banks mapped                       |
| `ticks`, `reset`, `save <file>`, `load <file>`                       | time between stops, reset, BESS states                       |

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
[trace tool](tracing.md) can load too. A loaded state is a starting point, not an
exact rewind: BESS doesn't hold every internal phase of the PPU.
