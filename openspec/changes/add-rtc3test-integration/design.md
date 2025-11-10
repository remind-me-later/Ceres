# Design: rtc3test Integration Test Automation

## Context

The rtc3test ROM validates MBC3 Real-Time Clock functionality through interactive test suites that require button input
to select and run. Unlike existing integration tests that simply run until completion, rtc3test requires:

1. User interaction simulation (button presses at specific times)
2. Longer test durations (8-13 seconds of emulated time)
3. Screenshot comparison after test completion
4. Selective test execution (only run passing tests)

The test ROM provides three subtests, but only "basic tests" and "range tests" currently pass. The "sub-second writes"
test fails due to incomplete RTC implementation.

## Goals / Non-Goals

**Goals:**

- Add automated testing for MBC3 RTC basic functionality
- Extend TestRunner to support button input simulation
- Validate RTC behavior using screenshot comparison
- Run only currently passing tests (basic and range)

**Non-Goals:**

- Fixing RTC implementation to pass sub-second writes test
- Adding support for serial output validation (test uses visual output)
- Testing on DMG model (CGB is sufficient per documentation)
- Real-time RTC behavior (tests use emulated time)

## Decisions

### Decision: Button Press Scheduling API

**Choice:** Add a vector of scheduled button events to `TestConfig` with frame number and button state.

**Rationale:**

- Simple and explicit: tests define exact frame timing for button presses
- Flexible: can schedule multiple buttons at different frames
- Testable: button press behavior is deterministic and repeatable
- Minimal overhead: button events processed only when scheduled

**Alternative considered:** Callback-based approach where test provides a function that decides button state per frame.
Rejected because it's more complex and harder to debug than declarative scheduling.

### Decision: Button Event Structure

```rust
pub struct ButtonEvent {
    pub frame: u32,
    pub button: Button,
    pub action: ButtonAction,
}

pub enum ButtonAction {
    Press,
    Release,
}
```

**Rationale:**

- Explicit press/release allows precise control of button timing
- Frame-based scheduling aligns with existing frame-counting infrastructure
- Can handle both instantaneous presses and held buttons

### Decision: Timeout Values

- Basic tests: 1050 frames (~17.5 seconds: 4s CGB intro + 13s test + 0.5s margin)
- Range tests: 750 frames (~12.5 seconds: 4s CGB intro + 8s test + 0.5s margin)
- Calculation: (CGB intro + test duration + safety margin) × 59.73 fps

**Rationale:**

- Based on documented test durations from game-boy-test-roms-howto.md
- Must account for ~4 second CGB boot intro animation before test ROM starts
- Matches pattern of other timeout constants (actual time + small margin)
- Prevents indefinite execution while allowing completion

### Decision: Test Scope

**Only include basic and range tests in CGB mode.**

**Rationale:**

- Per user requirement: "only subtests passing are basic and range"
- Per documentation: CGB mode is sufficient for RTC validation
- Adding failing tests would require fixing RTC implementation first
- Can add sub-second writes test in future change when RTC is fixed

### Decision: Button Timing

Initial button presses scheduled at frame 240 (~4 seconds after start):

- CGB boot intro animation takes ~4 seconds (~240 frames at 59.73 fps)
- Allows ROM to fully initialize and display the menu after intro completes
- Ensures menu is ready to receive input
- Can be adjusted during implementation if needed

For range tests, schedule Down press at frame 240, then A press at frame 270 (0.5s delay between presses).

**Rationale:**

- Must wait for CGB intro animation to complete before ROM menu is accessible
- Conservative timing prevents race conditions during boot sequence and ROM initialization
- Short delay between button presses mimics realistic user behavior
- Values can be fine-tuned based on actual test behavior during implementation

## Risks / Trade-offs

### Risk: Button Timing Sensitivity

If button presses occur too early (before CGB intro completes), the ROM menu won't be ready to accept input. If too
late, unnecessary frames are wasted.

**Mitigation:**

- Start at frame 240 (~4 seconds) to account for CGB intro animation
- Add safety margin beyond intro duration to ensure menu is fully rendered
- Adjust based on observed behavior during implementation
- Document timing assumptions in test comments

### Risk: Flaky Tests

RTC timing tests may be sensitive to frame timing inaccuracies.

**Mitigation:**

- Tests already have built-in tolerance per rtc3test documentation
- Screenshot comparison is binary (pass/fail), reducing ambiguity
- Use emulated time, not real time (no system clock dependency)

### Risk: Incomplete RTC Implementation

Only 2 of 3 test suites pass, indicating RTC is not fully implemented.

**Mitigation:**

- Clearly document which tests are excluded and why
- Tests validate currently working functionality
- Foundation for adding more tests when RTC is improved

## Migration Plan

No migration needed - this is purely additive. Existing tests are unaffected.

Steps:

1. Add button press infrastructure to TestRunner
2. Add new test file with rtc3test tests
3. Verify tests pass in CI
4. Document test limitations in comments

## Open Questions

None - all requirements are clear from documentation and user requirements.
