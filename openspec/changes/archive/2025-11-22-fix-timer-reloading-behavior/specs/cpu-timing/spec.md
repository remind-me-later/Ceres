## ADDED Requirements

### Requirement: Timer Reload Timing

The SM83 CPU emulator SHALL implement the correct timing for TIMA register reloading upon overflow.

#### Scenario: TIMA overflow delay

- **WHEN** the TIMA register overflows (increments from 0xFF to 0x00)
- **THEN** the TIMA register SHALL contain 0x00 for 4 T-cycles (1 M-cycle)
- **AND** the TIMA register SHALL be reloaded with the value from TMA after this delay
- **AND** the timer interrupt SHALL be requested upon reload

### Requirement: Timer Register Write Behavior

The SM83 CPU emulator SHALL implement the correct behavior for writes to TIMA and TMA registers during the reload delay.

#### Scenario: TIMA write during reload

- **WHEN** a value is written to TIMA during the 4-cycle reload delay
- **THEN** the write SHALL be ignored
- **AND** the reload from TMA SHALL still occur after the delay

#### Scenario: TMA write during reload

- **WHEN** a value is written to TMA during the 4-cycle reload delay
- **THEN** the new value SHALL be written to TMA
- **AND** the TIMA register SHALL be reloaded with this new TMA value after the delay
