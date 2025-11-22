# Design: IE Push Interrupt Dispatch Timing

## Context

The Game Boy interrupt dispatch mechanism involves a multi-cycle process:

1. When IME is enabled and an interrupt is requested, the CPU checks `IE & IF` to determine the interrupt queue
2. The CPU performs two tick cycles for internal processing
3. The CPU pushes the current PC onto the stack (high byte first, then low byte)
4. IME is disabled
5. PC is set to the interrupt vector

The edge case arises when the stack pointer is positioned such that the push operation writes to the IE register itself
(at address `$FFFF`). According to hardware behavior verified by Mooneye tests and SameBoy:

- **Upper byte push to IE**: If SP is `$0000` before decrement, the upper byte writes to `$FFFF` (IE). The written value
  should affect interrupt dispatch - if it clears the interrupt bit, the dispatch should be cancelled and PC should jump
  to `$0000` instead.
- **Lower byte push to IE**: If SP is `$0001` before the first decrement, the lower byte writes to `$FFFF` (IE). This
  write occurs too late to cancel the interrupt - the dispatch continues normally.

## Goals / Non-Goals

**Goals:**

- Implement hardware-accurate IE register modification timing during interrupt dispatch
- Pass the `ie_push` Mooneye test (4 rounds)
- Maintain existing interrupt handling behavior for normal cases

**Non-Goals:**

- Optimize interrupt dispatch performance (accuracy first)
- Handle other register modifications during push (out of scope)
- Refactor the entire interrupt system (minimal changes preferred)

## Decisions

### Decision 1: When to Re-check IE Register

**Chosen approach**: Check IE register after the upper byte push, before the lower byte push.

**Rationale**:

- SameBoy's implementation shows that `interrupt_queue` is recalculated after each push byte when relevant registers are
  affected
- The upper byte push happens first and can cancel the interrupt
- The lower byte push is too late to affect the current dispatch

**Alternatives considered**:

- Check IE only at the start: Fails the ie_push test because writes during push are ignored
- Check IE after every write: More complex and unnecessary for correctness

### Decision 2: How to Cancel an Interrupted Interrupt

**Chosen approach**: If IE is modified during upper byte push to clear the interrupt bit, set PC to `$0000` and complete
the push normally.

**Rationale**:

- This matches hardware behavior as verified by the ie_push test Round 1
- The push operation has already started and should complete
- PC defaults to `$0000` when no interrupt should be dispatched

**Alternatives considered**:

- Abort the push entirely: Incorrect per hardware tests
- Continue with original interrupt: Fails the test

### Decision 3: Implementation Location

**Chosen approach**: Modify the interrupt dispatch logic in `run_cpu()` to:

1. Perform the first push write (upper byte)
2. Re-check `IE & IF` to update the interrupt queue
3. Decide whether to continue with interrupt or cancel to `$0000`
4. Perform the second push write (lower byte)
5. Set PC based on the updated interrupt queue

**Rationale**:

- Minimal changes to existing code structure
- Keeps interrupt logic centralized in `run_cpu()`
- Avoids modifying the generic `push()` function which is used for other operations

**Alternatives considered**:

- Make `push()` aware of interrupt context: Too invasive, affects call/push instructions
- Special interrupt push function: Reasonable, but adds code duplication

## Risks / Trade-offs

**Risk**: Performance impact from additional IE checks during interrupt dispatch

- **Mitigation**: Only affects interrupt dispatch path, which is relatively rare (every ~1000 cycles minimum)

**Risk**: Edge cases with other registers modified during push

- **Mitigation**: Out of scope - focus on IE register only as that's what the test validates

**Trade-off**: Code clarity vs accuracy

- Accepting slightly more complex interrupt dispatch logic for hardware accuracy
- Following OpenSpec principle: accuracy over simplicity when it matters

## Migration Plan

1. Implement the new interrupt dispatch logic in `run_cpu()`
2. Enable the previously ignored `test_mooneye_interrupts_ie_push` test
3. Verify no regressions in other Mooneye interrupt tests
4. Run full integration test suite to ensure no behavioral changes for normal cases

**Rollback**: If implementation causes regressions, revert changes to `run_cpu()` and re-ignore the test.

## Open Questions

- Should we handle IF register modifications during push similarly? (The test doesn't check this, so deferring)
- Are there other registers that could be affected by push operations? (Not aware of any test ROMs that check this)
