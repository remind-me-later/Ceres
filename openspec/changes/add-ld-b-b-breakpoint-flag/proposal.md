# Change: Add LD B,B Debug Breakpoint Flag

## Why

Test ROMs like cgb-acid2 and dmg-acid2 use the `ld b, b` (opcode 0x40) instruction as a debug breakpoint to signal test
completion (see https://github.com/c-sp/game-boy-test-roms/blob/master/src/howto/cgb-acid2.md). Currently, there is no
way for test runners or frontends to detect when this instruction is executed without modifying the core API
extensively.

A simple flag-based approach allows core library users to detect `ld b, b` execution without breaking the existing API
or requiring callbacks that would complicate the `no_std` design.

## What Changes

- Add a boolean flag to the `Gb` struct that is set when the `ld b, b` instruction executes
- Provide a public method to check and reset the flag
- Keep the implementation minimal to avoid API changes or added complexity
- Document the flag's purpose for test ROM debugging

## Impact

- Affected specs: `cpu-debug` (new capability)
- Affected code:
  - `ceres-core/src/lib.rs` (add flag field and accessor method)
  - `ceres-core/src/sm83.rs` (set flag in `ld_b_b` implementation)
- Test impact: None directly, but enables improved test detection in `ceres-test-runner`
- API: Fully backward compatible, adds one new public method
