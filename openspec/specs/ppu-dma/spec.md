# ppu-dma Specification

## Purpose
TBD - created by archiving change debug-oam-dma-start-timing. Update Purpose after archive.
## Requirements
### Requirement: OAM DMA Start Timing

The OAM DMA controller SHALL implement precise timing for when OAM becomes inaccessible after writing to the
DMA register ($FF46), matching the behavior verified by hardware tests.

#### Scenario: Fresh DMA start - OAM accessible for 1 M-cycle

- **WHEN** the CPU writes a value to the DMA register ($FF46) to initiate an OAM DMA transfer
- **AND** no previous DMA transfer is active
- **THEN** OAM memory ($FE00-$FE9F) SHALL remain accessible for 1 M-cycle (4 dots)
- **AND** OAM SHALL become inaccessible starting at M-cycle 2 (returning $FF for reads)
- **AND** the actual DMA transfer SHALL begin at M-cycle 2

#### Scenario: DMA restart while previous DMA running

- **WHEN** the CPU writes a value to the DMA register ($FF46) while a previous DMA transfer is still active
- **THEN** OAM memory SHALL remain inaccessible throughout the restart process
- **AND** the previous DMA transfer SHALL NOT be immediately stopped
- **AND** the new DMA transfer SHALL begin at M-cycle 2 relative to the write

#### Scenario: Code execution from OAM during DMA start

- **WHEN** the CPU is executing code from OAM memory ($FE00-$FE9F)
- **AND** the code writes to the DMA register ($FF46)
- **THEN** the instruction following the DMA write SHALL execute successfully from OAM (M-cycle 1)
- **AND** subsequent instructions SHALL fail to read from OAM (M-cycle 2 onwards)

### Requirement: DMA State Tracking

The DMA controller SHALL maintain state that distinguishes between initialization and active transfer phases
to correctly implement OAM accessibility timing.

#### Scenario: DMA enabled state check during startup

- **WHEN** a DMA transfer is initiated by writing to $FF46
- **AND** the DMA is in the startup delay phase (first M-cycle)
- **THEN** OAM accessibility checks SHALL return "accessible"
- **AND** the DMA SHALL be considered "active" for the purpose of preventing new DMA starts

#### Scenario: DMA enabled state check during transfer

- **WHEN** a DMA transfer is in the active transfer phase (M-cycle 2 onwards)
- **THEN** OAM accessibility checks SHALL return "inaccessible"
- **AND** reads from OAM SHALL return $FF
- **AND** writes to OAM SHALL be ignored

### Requirement: Mooneye oam_dma_start Test Compliance

The emulator SHALL pass the `mooneye-test-suite/acceptance/oam_dma_start.gb` test, which validates the
precise timing of OAM DMA initialization.

#### Scenario: Test round 1 - fresh DMA from OAM code

- **WHEN** the test ROM executes code from $FDFF that writes to DMA and immediately executes from $FE00
- **THEN** register B SHALL equal $D7 (incremented from $00 by successful INC B at $FE00)
- **AND** register C SHALL equal $01 (incremented from $00 by successful INC B at $FE01 before RST $38)
- **AND** register D SHALL equal $D7 (value at OAM offset $00, the RST $10 opcode)

#### Scenario: Test round 2 - restarted DMA from OAM code

- **WHEN** the test ROM executes code from $FDFE that writes to DMA while previous DMA is running
- **THEN** register B SHALL equal $00 (not incremented, as INC B at $FE00 was replaced by RST $38)
- **AND** register E SHALL equal $00 (B was not incremented)
- **AND** register D SHALL equal $D7 (value at OAM offset $00, the RST $10 opcode)

