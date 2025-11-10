# cpu-debug Specification Deltas

## ADDED Requirements

### Requirement: LD B,B Breakpoint Detection

The emulator core SHALL provide a mechanism to detect when the `ld b, b` instruction (opcode 0x40) is executed, enabling
test ROMs that use this instruction as a debug breakpoint to signal completion.

#### Scenario: Breakpoint flag set on ld b,b execution

- **WHEN** the CPU executes the `ld b, b` instruction (opcode 0x40)
- **THEN** a debug breakpoint flag is set to true
- **AND** the instruction completes normally as a NOP
- **AND** the flag remains set until explicitly cleared

#### Scenario: Check breakpoint flag status

- **WHEN** a core library user calls the breakpoint check method
- **THEN** the method returns true if `ld b, b` has been executed since the last check
- **AND** the method returns false if `ld b, b` has not been executed or the flag has been reset

#### Scenario: Reset breakpoint flag

- **WHEN** a core library user calls the breakpoint check method
- **THEN** the breakpoint flag is automatically reset to false after being read
- **AND** subsequent checks return false until `ld b, b` is executed again

#### Scenario: Flag survives frame boundaries

- **WHEN** the `ld b, b` instruction is executed during frame N
- **THEN** the breakpoint flag remains set across frame boundaries
- **AND** the flag is still set when checked at the start of frame N+1
- **AND** the flag only resets when explicitly checked by the user

### Requirement: Minimal API Surface

The breakpoint detection mechanism SHALL be implemented with minimal API changes to maintain backward compatibility.

#### Scenario: Single public method

- **WHEN** the breakpoint detection feature is implemented
- **THEN** exactly one new public method is added to the `Gb` struct
- **AND** no existing methods or fields are modified
- **AND** the API remains fully backward compatible

#### Scenario: No callback complexity

- **WHEN** the breakpoint detection mechanism is designed
- **THEN** the implementation uses a simple boolean flag approach
- **AND** no callback or closure mechanisms are required
- **AND** the `no_std` compatibility is preserved

#### Scenario: Flag-based check-and-reset pattern

- **WHEN** the user calls the breakpoint check method
- **THEN** the method returns the current flag state and atomically resets it
- **AND** this check-and-reset pattern prevents missed breakpoints
- **AND** the pattern is simple enough for test runners to use reliably
