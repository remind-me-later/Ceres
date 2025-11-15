## Context

The Mooneye Test Suite contains two tests for conditional call timing:

1. `call_cc_timing.gb` - **Currently passing**
2. `call_cc_timing2.gb` - **Currently failing** (registers = 0x42 indicating test failure)

Both tests validate the timing behavior of the `CALL cc, nn` instruction on real Game Boy hardware. The fact that one
passes and one fails suggests the emulator handles the basic case correctly but has a subtle timing bug in an edge
case.

The current implementation in `ceres-core/src/sm83.rs:636-645`:

```rust
fn call_cc_a16(&mut self, op: u8) {
    if self.satisfies_branch_condition(op) {
        self.do_call();
    } else {
        let pc = self.cpu.pc.wrapping_add(2);
        self.cpu.pc = pc;
        self.tick_m_cycle();
        self.tick_m_cycle();
    }
}
```

This implementation handles two cases:

- **Condition true**: Execute call (3 M-cycles for push, 1 for jump)
- **Condition false**: Skip over 2-byte immediate operand (2 M-cycles)

The issue likely involves the timing of one of these paths or the condition evaluation itself.

## Goals / Non-Goals

**Goals:**

- Identify the specific timing discrepancy causing `call_cc_timing2` to fail
- Fix the timing issue while maintaining correctness of `call_cc_timing` and related tests
- Document the root cause for future reference

**Non-Goals:**

- Rewriting the entire conditional instruction implementation
- Optimizing performance (accuracy takes precedence)
- Fixing other unrelated timing issues

## Decisions

### Decision: Use execution traces for analysis

Traces will be collected for both passing and failing tests to identify the exact point of divergence. The existing
trace collection infrastructure in `ceres-test-runner` can be leveraged.

**Alternatives considered:**

- Manual stepping through test ROM - Too time-consuming, harder to compare
- Comparing final states only - Doesn't reveal where divergence occurs

**Rationale:** Execution traces provide a complete view of the instruction sequence and can be diff'd to find the exact
point where behavior diverges.

### Decision: Compare with SameBoy implementation

SameBoy is the project's gold standard for correct emulation behavior. If Pan Docs documentation is ambiguous, SameBoy's
implementation will be used as the authoritative reference.

**Rationale:** SameBoy is extensively tested against real hardware and is considered highly accurate.

### Decision: Ensure no regressions in passing tests

Any fix must not break currently passing tests, including `call_cc_timing` and other conditional instruction tests
(`jp_cc_timing`, `ret_cc_timing`, etc.).

**Rationale:** The test suite provides regression protection. Breaking passing tests to fix one failing test indicates
an incomplete understanding of the issue.

## Risks / Trade-offs

### Risk: Fix may be architecture-specific

**Mitigation:** Consult Mooneye test documentation to understand which Game Boy models the test targets. Ensure fix is
appropriate for CGB model (default for tests without model hints).

### Risk: Timing fix may reveal additional failing tests

**Mitigation:** Run full Mooneye suite after fix to identify any new failures. These can be addressed in separate
changes.

### Risk: Root cause may be in related code, not call_cc_a16 itself

**Mitigation:** Trace analysis will reveal if issue is in `satisfies_branch_condition`, `do_call`, `push`, or other
called functions. The fix location will be determined by the evidence.

## Open Questions

- Does the test fail on all M-cycles of the instruction, or only specific ones?
- Is the issue specific to certain condition codes (NZ, Z, NC, C)?
- Does `call_timing2` (unconditional call) pass? (Likely yes, since basic calls work)
- Are there similar timing issues in `ret_cc` or `jp_cc` that haven't been caught yet?
