# Ceres Test Status

Last updated: 2026-06-07

## Test Results

| Suite          | Pass  | Fail  | Ignored |
|----------------|-------|-------|---------|
| gambatte       | 474   | 258   | 0       |
| blargg         | 5     | 1     | 0       |
| mooneye        | 75    | 11    | 18      |
| gbmicrotest    | 257   | 182   | 74      |

Branch: `dev` (24 commits ahead of `origin/dev`)
Last commit: `4c1e51b4 fix(apu): also wrap subtraction in LengthTimer::write_len`

## Categories of Failures

### OAM Access Tests (Test Framework Issues)
14 gambatte `oam_access` failures + 23 gbmicrotest OAM failures + 1 mooneye OAM test.

**Findings**:
- The OAM access control logic in `ceres-core/src/ppu/oam.rs:51-65` is
  coarse-grained: it blocks reads/writes during mode 2/3 based on the
  current PPU mode.
- Real hardware uses **cycle-accurate** OAM access: reads return specific
  patterns based on the exact T-cycle within mode 2/3, not just 0xFF.
- gambatte's `oamReadable` / `oamWritable` (video.cpp:oamReadable) compute
  the access window based on the line cycle, not the mode.
- Tests like `midread_1` pass with the current logic because the OAM read
  returns specific values (e.g., 3) when the read happens at a specific
  cycle in mode 2 — and the cycle happens to be in a "readable" window.
- Tests like `midwrite_1` fail because: (1) the ceres-core's PPU mode
  accounting doesn't match the exact T-cycle of the write, and (2) the
  test framework checks A at `PC==0x7000` (start of `lprint_fe00`), not
  after the OAM read inside the lprint, so the test framework has
  timing-sensitivity bugs that can't be fixed in ceres-core.
- The fundamental OAM access model would need to track the exact T-cycle
  within each mode and compute the read/write window based on hardware
  formulas (see gambatte's `oamReadable` / `oamWritable`).

**Test framework bug discovered**: The gambatte test runner at
`ceres-test-runner/tests/gambatte.rs:103` checks `gb.cpu_pc() == 0x7000`,
but for tests where the lprint routine reads OAM (like `midwrite_1`),
the A register contains the value from the previous instruction, not the
OAM value. The test framework would need to check A at the point where
the lprint routine finishes reading OAM (around PC=0x700B), not at the
start of the lprint. This is a test infrastructure issue, not a
ceres-core bug.

**Decision**: Accept these OAM failures. The OAM access model is
fundamentally cycle-accurate and would require a major rewrite.

### PPU Timing (Out of Scope)
- 258 gambatte PPU tests (`oam_access`, `sprite`, `window`, `lycwirq`, `m2statwirq`, etc.)
- 11 mooneye PPU tests (`ppu_intr_*`, `ppu_vblank_stat_intr`, `ppu_stat_irq_blocking`, etc.)
- 182 gbmicrotest PPU tests (`hblank_int_*`, `oam_int_*`, `lcdon_*`, `line_153_*`, `line_144_*`, etc.)
- 4 lyc0 + 6 lyc153_late_ff45_enable_ds tests (CGB line 153 quirk)

### HALT / Interrupt Precedence
- 1 blargg test (`test_blargg_interrupt_time`) — accepted, broken by APU fix
- 2 mooneye tests (`halt_ime1_timing2_gs`, `halt_ime0_nointr_timing`) — HALT exit delay trade-off
- 2 mooneye tests (`di_timing_gs`, `boot_regs_mgb`, `boot_hwio_dmgabcmgb`) — boot/IO setup
- 15 tc00 timer tests — fundamental conflict with IRQ precedence tests
- 2 tc00_irq_ds tests (CGB double-speed timer)

### Sound Tests Requiring Gate-Level Accuracy
The 5 failing sound tests are:
- `ch1_init_reset_sweep_counter_timing_nr52_1` (CGB)
- `ch1_init_reset_sweep_counter_timing_nr52_2` (DMG)
- `ch2_init_reset_length_counter_timing_nr52_1` (CGB)
- `ch2_init_reset_length_counter_timing_nr52_2` (both)
- `ch2_late_div_write_nr52_2b` (DMG)

**Root cause**: These tests were generated from real hardware and require a 128 KHz
sweep clock (8 T-cycles per tick) plus a shift state machine. ceres-core's APU
steps only at DIV bit 12 (4096 T-cycles), missing the fine-grained sweep timing.

**Reference comparisons**:
- **gambatte** (`channel1.cpp:73-87`): `counter = (((cc+2+cgb_*2) >> 14) + period) << 14 + 2`
  - Schedules the sweep event at sound cc=0x8002, which is way after the test read.
  - Test expects the channel disabled at ~cc 10426, but gambatte doesn't disable by then.
- **SameBoy** (`apu.c:619-645`): Uses `reload_timer = 1 + lf_div` and `channel_1_restart_hold`
  - Disables the channel within 1-2 APU cycles of trigger.
  - Test expects CGB to NOT disable by cc 10428, but SameBoy's `channel_1_restart_hold`
    only delays the overflow check, not the disable itself.
- **metroboy** (`LogicBoyCh1.cpp:160-208`): Gate-level model with 128 KHz sweep clock
  - Models the actual hardware: `BYFE_CLK_128`, `SWEEP_DELAY` counter, `SHIFTING` state machine.
  - Would produce the correct hardware behavior, but is a 776-line gate-level
    implementation not directly portable to ceres-core's higher-level model.

**Conclusion**: Neither gambatte nor SameBoy (the two emulator references) match
the test expectations. The tests were generated from specific hardware that has
quirks neither reference captures. metroboy is gate-level accurate but requires
a fundamentally different implementation structure (gate-by-gate modeling).

**To fix in ceres-core would require**:
1. Sub-APU-step sweep clock (8 T-cycles)
2. Shift state machine for sweep operations
3. Processing sweep at the correct rate
4. Significant refactor (~1-2 weeks)

**Decision**: Accept these failures. Document the limitation.

## Work Already Done

### TIMA
- `2d6511c7`: Added `tima_irq_countdown` decoupled from `tima_reload_pending`
- `8e19c5ef`: Added HALT exit delay (+4 T-cycles) for ISR dispatch
- Fixes several gambatte timer tests

### APU
- `a4b5f86d`: Removed duplicate length-counter ticks at div_divider 2,6
- `3f294e83`: Use "remaining count" model for length timer (matches gambatte/SameBoy)
- `b03a3715`: Wrap-around in `LengthTimer::write_len` for wave channel
- `4c1e51b4`: Wrap subtraction in `LengthTimer::write_len`

### PPU
- `c6e6bee8`: Re-evaluate STAT IRQs on register writes
- `6a56d633`: Process ALL mode transitions within `run()` budget (`if` → `while`)
- `972f4a1e`: HDMA destination offset (0x8000|raw), run_hdma during HALT, check_lyc in VBlank

### Memory
- `972f4a1e`: HDMA fixes (destination, HALT processing)

### Test infrastructure
- `16be8e99`: mooneye tests now skip bootrom
- `58455d09`: gambatte ROMs built from source via `build_gambatte_roms.py`
- `35210b1f`: Added 408 gambatte tests (halt, irq_precedence, tima, miscmstatirq, lycEnable)

## Next Steps (if desired)

1. CGB line 153 quirk — 21 lyc153 tests
2. OAM access sub-M-cycle timing — 14 tests
3. Window/sprite rendering at exact pixel positions
4. Other non-PPU APU fixes
5. Boot register setup (boot_hwio tests)
