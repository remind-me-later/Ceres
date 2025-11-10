# Integration Tests - Spec Delta

This is a **validation-only change** - no new requirements are added, only validation of existing MBC3 behavior.

## Context

This change uses the trace collection capability to debug and validate the existing MBC3 cartridge implementation. If
bugs are found, they will be fixed to restore spec-compliant behavior. An integration test will be added to validate the
existing MBC3 bank switching requirements.

## ADDED Requirements

### Requirement: MBC3 Bank Switching Validation

The test suite SHALL validate MBC3 bank switching behavior using the mbc3-tester ROM.

#### Scenario: MBC3 ROM bank switching in DMG mode

- **GIVEN** the mbc3-tester ROM is loaded
- **WHEN** the ROM executes bank switching logic in DMG mode
- **THEN** the screen output SHALL match the reference screenshot `mbc3-tester-dmg.png`
- **AND** all 128 ROM banks SHALL be accessible

#### Scenario: MBC3 ROM bank switching in CGB mode

- **GIVEN** the mbc3-tester ROM is loaded
- **WHEN** the ROM executes bank switching logic in CGB mode
- **THEN** the screen output SHALL match the reference screenshot `mbc3-tester-cgb.png`
- **AND** all 128 ROM banks SHALL be accessible

#### Scenario: MBC3 ROM bank 0 special case

- **GIVEN** the MBC3 cartridge is initialized
- **WHEN** the program writes 0x00 to the ROM bank register ($2000-$3FFF)
- **THEN** the cartridge SHALL map ROM bank 0x01 (not 0x00)
- **AND** subsequent reads from $4000-$7FFF SHALL access bank 0x01
