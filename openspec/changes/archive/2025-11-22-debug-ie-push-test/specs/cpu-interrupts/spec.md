# CPU Interrupts Specification Deltas

## ADDED Requirements

### Requirement: IE Register Modification During Interrupt Dispatch

The system SHALL re-evaluate the interrupt queue if the IE register is modified during the PC push operations of an
interrupt dispatch sequence.

#### Scenario: IE written during upper byte push cancels interrupt

- **GIVEN** an interrupt is being dispatched with IME enabled
- **AND** the stack pointer is $0000 before decrement
- **WHEN** the upper byte of PC is pushed to address $FFFF (IE register)
- **AND** the written value clears the interrupt bit that triggered the dispatch
- **THEN** the interrupt dispatch SHALL be cancelled
- **AND** PC SHALL be set to $0000 instead of the interrupt vector
- **AND** IF SHALL retain the interrupt flag (not cleared)
- **AND** IME SHALL be set to 0

#### Scenario: IE written during lower byte push does not cancel interrupt

- **GIVEN** an interrupt is being dispatched with IME enabled
- **AND** the stack pointer is $0001 before the first decrement
- **WHEN** the lower byte of PC is pushed to address $FFFF (IE register)
- **AND** the written value clears the interrupt bit
- **THEN** the interrupt dispatch SHALL continue normally
- **AND** PC SHALL be set to the appropriate interrupt vector
- **AND** IF SHALL be cleared for the dispatched interrupt
- **AND** IME SHALL be set to 0

#### Scenario: Multiple interrupts with IE modified during upper byte push

- **GIVEN** multiple interrupts are pending (e.g., STAT and VBLANK)
- **AND** an interrupt dispatch begins for the higher priority interrupt
- **AND** the stack pointer is $0000 before decrement
- **WHEN** the upper byte of PC is pushed to address $FFFF (IE register)
- **AND** the written value clears the higher priority interrupt but keeps lower priority
- **THEN** the lower priority interrupt SHALL be dispatched normally
- **AND** PC SHALL be set to the lower priority interrupt vector
- **AND** IF SHALL be cleared for the dispatched interrupt only

### Requirement: Interrupt Dispatch PC Push Timing

The system SHALL perform PC push operations during interrupt dispatch with correct timing and memory write ordering.

#### Scenario: Normal interrupt dispatch push sequence

- **GIVEN** an interrupt is being dispatched
- **WHEN** the push operation begins
- **THEN** SP SHALL be decremented to SP-1
- **AND** the upper byte (PC high) SHALL be written to address [SP-1]
- **THEN** SP SHALL be decremented to SP-2
- **AND** the lower byte (PC low) SHALL be written to address [SP-2]
- **AND** one M-cycle SHALL be consumed for the push operation

#### Scenario: Stack pointer wraps during interrupt push

- **GIVEN** SP is $0001 or $0000 before interrupt dispatch
- **WHEN** the interrupt push begins
- **THEN** SP SHALL wrap around correctly (e.g., $0000 → $FFFF)
- **AND** writes SHALL target high memory addresses including IO registers

### Requirement: Interrupt Dispatch Sequence

The system SHALL dispatch interrupts following the hardware-accurate timing and register modification sequence.

When IME is enabled and an interrupt is both requested (IF) and enabled (IE):

1. The CPU SHALL consume 2 M-cycles for internal processing
2. The CPU SHALL push the current PC onto the stack:
   - Decrement SP and write PC high byte to [SP]
   - **Re-evaluate the interrupt queue if IE or IF was modified by the previous write**
   - Decrement SP and write PC low byte to [SP]
   - Consume 1 M-cycle for the push operation
3. The CPU SHALL set IME to 0 (disable interrupts)
4. The CPU SHALL set PC to the interrupt vector (or $0000 if cancelled)
5. The CPU SHALL clear the IF flag for the dispatched interrupt (unless cancelled)

#### Scenario: Standard interrupt dispatch

- **GIVEN** IME is enabled (set to 1)
- **AND** at least one interrupt is both requested and enabled (IE & IF != 0)
- **WHEN** the interrupt dispatch sequence begins
- **THEN** the CPU SHALL consume 2 M-cycles for internal processing
- **AND** the CPU SHALL push the current PC onto the stack consuming 1 M-cycle
- **AND** IME SHALL be set to 0
- **AND** PC SHALL be set to the appropriate interrupt vector address
- **AND** the IF flag SHALL be cleared for the dispatched interrupt

#### Scenario: Interrupt dispatch with IE modification

- **GIVEN** IME is enabled and an interrupt is pending
- **AND** the stack pointer points to high memory ($0000 or $0001)
- **WHEN** the interrupt dispatch push writes to IE register
- **THEN** the interrupt queue SHALL be re-evaluated with the new IE value
- **AND** the dispatch SHALL proceed or cancel based on the new IE & IF result

#### Scenario: No interrupt dispatch when IME disabled

- **GIVEN** IME is disabled (set to 0)
- **AND** interrupts are pending (IE & IF != 0)
- **WHEN** the CPU attempts to check for interrupts
- **THEN** no interrupt SHALL be dispatched
- **AND** PC SHALL continue normal execution
- **AND** the CPU SHALL wake from HALT state if halted
