## ADDED Requirements

### Requirement: PC-Based Trace Triggering

The system SHALL allow enabling and disabling trace collection based on a program counter (PC) range.

#### Scenario: Trace within PC range

- **GIVEN** trace collection is configured with a start PC of `0x0100` and an end PC of `0x0150`
- **WHEN** the CPU executes an instruction at PC `0x0120`
- **THEN** the instruction SHALL be recorded in the trace buffer.

#### Scenario: Trace outside PC range

- **GIVEN** trace collection is configured with a start PC of `0x0100` and an end PC of `0x0150`
- **WHEN** the CPU executes an instruction at PC `0x0200`
- **THEN** the instruction SHALL NOT be recorded in the trace buffer.
