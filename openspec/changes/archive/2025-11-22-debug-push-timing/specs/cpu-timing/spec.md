## ADDED Requirements

### Requirement: PUSH Instruction Timing

The SM83 CPU emulator SHALL implement PUSH instruction timing that matches Game Boy hardware behavior for stack
operations.

#### Scenario: PUSH rr executes with correct M-cycle timing

- **WHEN** a PUSH rr instruction (opcodes 0xC5, 0xD5, 0xE5, 0xF5) is executed
- **THEN** the instruction SHALL complete in 4 M-cycles with the following timing:
  - M=0: Instruction decoding (implicit)
  - M=1: Internal delay cycle
  - M=2: Memory write of high byte to (SP-1)
  - M=3: Memory write of low byte to (SP-2)

#### Scenario: PUSH timing verified by OAM DMA test

- **WHEN** the Mooneye push_timing test is executed
- **THEN** the test SHALL pass with registers containing Fibonacci values (B=3, C=5, D=8, E=13, H=21, L=34)
- **AND** memory writes SHALL occur at the exact M-cycles when OAM is accessible during DMA transfer

#### Scenario: PUSH internal delay allows for OAM bug handling

- **WHEN** PUSH is executed on DMG hardware with SP pointing to OAM during PPU Mode 2/3
- **THEN** the M=1 internal delay cycle SHALL provide a timing point where OAM bug corruption logic can be applied
- **AND** the OAM bug handling SHALL occur before any memory writes

### Requirement: CALL Instruction Maintains Correct Timing

The SM83 CPU emulator SHALL ensure that CALL instruction timing remains correct when the PUSH timing is fixed, as CALL
internally uses the push operation for the return address.

#### Scenario: CALL nn timing verification

- **WHEN** the Mooneye call_timing test is executed
- **THEN** the test SHALL continue to pass
- **AND** the CALL instruction SHALL complete in the correct number of M-cycles

#### Scenario: CALL cc, nn timing verification

- **WHEN** the Mooneye call_cc_timing test is executed
- **THEN** the test SHALL continue to pass
- **AND** conditional CALL instructions SHALL complete in the correct number of M-cycles

### Requirement: Interrupt Dispatch Maintains Correct Timing

The SM83 CPU emulator SHALL ensure that interrupt dispatch timing remains correct when the PUSH timing is fixed, as
interrupt dispatch uses the push operation for saving the program counter.

#### Scenario: Interrupt timing verification

- **WHEN** interrupt-related Mooneye tests are executed (ei_timing, intr_timing, halt_ime1_timing, etc.)
- **THEN** all tests SHALL continue to pass
- **AND** interrupt dispatch SHALL push PC to the stack with correct timing
