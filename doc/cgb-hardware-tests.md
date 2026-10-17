# CGB behaviour to check on hardware

The CGB's LCD timing in ceres is fitted to test ROMs that were verified on
real consoles (gambatte, SameSuite, AGE, Mooneye, Mealybug). Where those tests
leave a timing undecided, ceres picks a value that passes them all, and the
choice is listed here so that a small test ROM run on a real CGB can settle
it.

Each entry says what to measure, why the existing tests do not pin it down,
what ceres does today, and the outcomes that would tell the candidates apart.
Revisions matter: ideally run every test on a CGB-C (CPU CGB C, the revision
gambatte's tests were made on) and a CGB-E (CPU CGB E, SameBoy's default), and
note the revision printed on the CPU.

## How to write a test

Most entries need the same kind of ROM, in the style of gambatte's hwtests:

1. Turn the LCD off, set up STAT/LYC, turn it on (optionally in double
   speed, `KEY1` then `STOP`).
2. Wait a fixed number of cycles from a known event (the LCD turning on, or
   an interrupt), so the read lands on a chosen dot.
3. Read STAT, LY or IF (or let the interrupt fire and record when).
4. Print the value on screen or store it where a flash cart can be dumped.

Then sweep the wait one cycle at a time across the edge in question: the
cycle at which the value changes is the answer. gambatte's ROMs
(`external/test-roms/gambatte`) are good templates: each comes in `_1`/`_2`
pairs one cycle apart.

## Terms

Times below are in half dots (`h`) on the line clock of
`ceres-core/src/ppu/display/timing.rs`: 912 per line, LY changes at
`h = 904` for the comparator. A CPU cycle is 2 half dots at normal speed
and 1 in double speed, and an M-cycle 8 or 4.

## Open questions

### 1. LY read while LY increments

- **Measure:** read LY on every cycle across the end of a line, for an
  increment that carries through several bits (7→8, 0x0F→0x10, 0x53→0x54,
  0x8F→0x90), in normal and double speed. Repeat on a DMG if possible.
- **Why the tests don't decide:** gambatte's CGB tests expect `old & new`
  (7→8 reads 0, 0x53→0x54 reads 0x50) at one read position, which is what
  a ripple counter shows while the carry propagates. They only sample a few
  increments, so they don't show whether every increment does it, or for
  how long. The DMG tests never read at that position.
- **Ceres today:** a CGB read that lands in the half dot where LY changes
  returns `old & new`; the DMG returns the new value.
- **Outcomes:** `old & new` for one cycle (a ripple counter); or another
  intermediate value, such as the low bits cleared before the high ones;
  or none (gambatte's value comes from something else).

### 2. Where a read sees LY, the LY=LYC flag and the mode bits

- **Measure:** read LY and STAT on every cycle across the end of a line
  (with LYC matching the old LY) and across the end of line 143 and line
  153, at normal speed and in double speed, and after switching speed with
  the LCD on (gambatte's `lcd_offset` ROMs show how to shift the CPU's
  cycles against the LCD's).
- **Why the tests don't decide:** each value changes at a different point,
  and the tests fix those points only where they happen to read. To pass
  gambatte's 3023 CGB tests, ceres reads each signal at its own delay:

  | Signal | DMG | CGB normal speed | CGB double speed |
  |---|---|---|---|
  | LY | 4 | 4 | 2 |
  | LY=LYC flag | 3 | 1 | 2, and it holds until the next LY=LYC sample instead of dropping as LY changes |
  | Mode bits | 4 | 4 | 3 |

  The CGB's mode 2 also runs a half dot earlier than the DMG's (from
  `h = 909` to 157 on the delayed clock, against 910 to 158), and mode 1
  shows from `h = 911` of line 143 instead of from line 144. A half dot is
  below what one normal-speed read can resolve, so these were fitted
  through the `lcd_offset` ROMs, whose CPU cycles sit a dot or two off the
  usual grid.
- **Ceres today:** the table above.
- **Outcomes:** for each signal, the first cycle that sees the new value,
  at both speeds. If LY, the flag and the mode bits all change at the
  points ceres predicts, the per-signal delays are real. If they line up
  with each other, ceres is overfitting a single read delay.

### 3. When a STAT write reaches the interrupt

- **Measure:** at normal speed, enable (or disable) one STAT interrupt
  source with a write a few cycles before its source rises: the VBlank
  start (LY 144), the OAM interrupt of line 0 (end of line 153), the
  HBlank of a visible line. Find the last cycle at which the write still
  counts.
- **Why the tests don't decide:** gambatte's model fits these with
  per-source rules (the VBlank event sees STAT at once, the others 2
  cycles late). Ceres fits them with one delay plus an order: the written
  enables reach the line 4 half dots after the write, after that half
  dot's LY=LYC sample and together with the VBlank latch. Both models pass
  the tests; they disagree on writes at half dots the tests don't use.
- **Ceres today:** 4 half dots (normal speed), 3 (double speed). The
  HBlank source also clears a half dot later than on the DMG (`h = 907`
  instead of 906), so that an HBlank enable landing at 906 still raises
  the interrupt (gambatte `m0enable/late_enable_lcdoffset1`).
- **Outcomes:** the write-to-effect delay for each source. If they all
  match, the one-delay model holds.

### 4. When an LYC write reaches the comparison

- **Measure:** write LYC (to the current LY, or away from it) a few cycles
  before the end of a line, sweeping the write one cycle at a time, at both
  speeds and with the CPU's cycles shifted against the LCD's
  (`lcd_offset`). Record whether the LYC interrupt fires and when the
  LY=LYC flag changes.
- **Why the tests don't decide:** on the DMG a written LYC reaches the
  comparator 3 half dots after the write and the comparison follows it at
  once. On the CGB no fixed delay passes gambatte's tests; what does is
  that the new LYC only takes effect at the next LY=LYC sample (once per
  M-cycle, at `h % 8 == 4`) at least 6 half dots after the write (4 in
  double speed). That reads like a comparator that latches LYC on its own
  clock, but the tests only probe a few write positions.
- **Ceres today:** the sample-point rule above.
- **Outcomes:** the write-to-effect delay as a function of the write's
  position in the M-cycle. A staircase (the effect jumps by an M-cycle as
  the write crosses a point) confirms the sampled LYC; a straight line
  means a fixed delay.

### 5. A STAT write at the VBlank latch

- **Measure:** at normal speed, write STAT 4 half dots before LY becomes
  144 (and before line 153 ends), toggling the VBlank and OAM enables in
  different combinations.
- **Why the tests don't decide:** in ceres the written enables reach the
  interrupt line in the same half dot as the VBlank latch, and the line is
  evaluated once for both. That is what lets a VBlank disable at that point
  stop the VBlank interrupt, and an OAM enable at the end of line 143 not
  raise one. The DMG's model evaluates them in sequence. Only those two
  gambatte tests (`m1irq_disable`, `ly143_late_m2enable`) depend on it.
- **Ceres today:** evaluated together on the CGB.
- **Outcomes:** whether either combination raises the interrupt.

### 6. What differs between the CGB-C and the CGB-E

- **Measure:** run the ROMs of entries 1–5 on both revisions, and in
  particular: read LY every cycle across the start of line 153 (when does
  it read 0?), read STAT across the end of line 153 (when does mode 1 end?)
  and read the LY=LYC flag across a line end at normal speed.
- **Why the tests don't decide:** gambatte's tests were made on a CGB-C;
  Mooneye's `-C` and AGE's `cgbE` ROMs on a CGB-D/E class unit. Each set
  fixes its own revision only at the points it reads, and some
  differences may be wrong attributions (a test passing on one revision
  for an unrelated reason).
- **Ceres today:** the same structure on both, with three CGB-D/E
  differences found so far: LY becomes 0 on line 153 at `h = 6` instead
  of 4; at normal speed the LY=LYC flag holds until the next LY=LYC sample
  (as in double speed on both); mode 1 shows until `h = 910` of line 153
  (the CGB-C clears it at 908).
- **Outcomes:** the cycle of each change on each revision. A revision
  difference that shows up in only one of these reads would point at a
  different mechanism than a uniform shift.
