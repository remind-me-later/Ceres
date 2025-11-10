## ADDED Requirements

### Requirement: Breakpoint on `ld b, b`

The emulator core (ceres-core) MUST provide a mechanism to notify a client when the `ld b, b` instruction (opcode
`0x40`) is executed.

#### Scenario: Test ROM completion

- **WHEN** a test ROM executes the `ld b, b` instruction to signal completion
- **THEN** the emulator core invokes a callback provided by the client (e.g., `ceres-test-runner`)
- **AND** the client can gracefully stop the emulation to verify the result.
