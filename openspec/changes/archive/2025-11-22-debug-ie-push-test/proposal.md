# Change: Debug IE Push Test Failure

## Why

The Mooneye `ie_push` acceptance test currently fails in Ceres. This test validates a critical edge case in Game Boy
interrupt handling: what happens when the IE (Interrupt Enable) register is modified during the PC push operations that
occur during interrupt dispatch.

According to the test and SameBoy's behavior, writing to the IE register during the upper byte push of the return
address can cancel an interrupt dispatch mid-flight if the written value clears the interrupt bit, while writes during
the lower byte push are too late to affect the current interrupt.

Ceres currently implements interrupt dispatch by determining which interrupt to service upfront, then performing the
push operations without re-checking the IE register. This approach fails to handle the edge case where IE is modified
during the push sequence itself.

## What Changes

- **ADDED**: Requirements for IE register modification timing during interrupt dispatch
- **ADDED**: Requirements for interrupt queue re-evaluation during PC push operations
- **MODIFIED**: Interrupt dispatch implementation to check IE register state during push operations

## Impact

- Affected specs: `cpu-interrupts`
- Affected code:
  - `ceres-core/src/sm83.rs:196-244` (run_cpu interrupt dispatch logic)
  - `ceres-core/src/sm83.rs:373-381` (push function)
  - `ceres-core/src/interrupts.rs` (interrupt handling logic)
- Test impact: Enables `test_mooneye_interrupts_ie_push` test to pass
- Breaking changes: None (fixes behavior to match hardware)
