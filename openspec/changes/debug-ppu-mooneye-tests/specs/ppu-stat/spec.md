# ppu-stat Specification Delta

## ADDED Requirements

### Requirement: STAT Interrupt Line Edge Detection

The PPU SHALL implement edge-triggered STAT interrupt generation by tracking an internal STAT interrupt line and only
requesting LCD interrupts on rising edge transitions (low to high).

#### Scenario: Multiple STAT sources active but line stays high

- **WHEN** the Mode 2 OAM interrupt is enabled in STAT
- **AND** the LY=LYC interrupt is enabled in STAT
- **AND** the PPU enters Mode 2 (setting the internal STAT line high)
- **AND** the LY=LYC coincidence becomes true while still in Mode 2
- **THEN** only ONE LCD interrupt SHALL be requested (when line first went high)
- **AND** no additional interrupt SHALL fire when coincidence becomes true (line already high)

#### Scenario: STAT line cleared by Mode 3

- **WHEN** the internal STAT interrupt line is high due to LY=LYC coincidence
- **AND** no other STAT interrupt source is enabled
- **AND** the PPU enters Mode 3 (drawing)
- **THEN** the internal STAT line SHALL go low (Mode 3 has no interrupt source)
- **AND** if LY=LYC is still true after Mode 3, the line SHALL go high again
- **AND** this rising edge SHALL request a new LCD interrupt

#### Scenario: Blocking through entire scanline

- **WHEN** the LY=LYC interrupt is enabled
- **AND** LYC is set to match the current line
- **AND** the comparison becomes true before Mode 3
- **AND** the comparison remains true through Mode 3 exit
- **THEN** only one STAT interrupt SHALL fire per scanline
- **AND** the internal line remains high throughout (no Mode 3 clear due to coincidence)

### Requirement: LY=LYC Coincidence During LCD Off

The PPU SHALL retain the LY=LYC coincidence flag state when the LCD is turned off, and SHALL NOT update the coincidence
flag while the LCD is off, regardless of LYC register changes.

#### Scenario: LCD off retains coincidence flag

- **WHEN** the LCD is on and LY equals LYC (coincidence flag is set)
- **AND** the LCD is turned off by clearing LCDC bit 7
- **THEN** the LY=LYC coincidence flag (STAT bit 2) SHALL remain set
- **AND** LY SHALL be reset to 0

#### Scenario: LYC change while LCD off does not update flag

- **WHEN** the LCD is off
- **AND** the LY=LYC coincidence flag is set (retained from when LCD was on)
- **AND** LYC is changed to a non-zero value
- **THEN** the LY=LYC coincidence flag SHALL remain set (not updated)
- **AND** no interrupt SHALL be generated

#### Scenario: LCD enable with matching LYC triggers interrupt

- **WHEN** the LCD is off with LY=0 (reset value)
- **AND** the LY=LYC coincidence flag is false
- **AND** LYC is changed to 0 while LCD is off
- **AND** the LCD is enabled
- **THEN** the comparison clock SHALL restart
- **AND** the LY=LYC coincidence flag SHALL be set (LY=0 matches LYC=0)
- **AND** if the LYC interrupt is enabled, a STAT interrupt SHALL be requested

#### Scenario: LCD enable without coincidence change suppresses interrupt

- **WHEN** the LCD is off with the LY=LYC coincidence flag already set
- **AND** LYC is 0 (matching the retained LY comparison value)
- **AND** the LCD is enabled
- **THEN** the LY=LYC coincidence flag SHALL remain set
- **AND** no STAT interrupt SHALL be requested (no rising edge - flag was already set)

### Requirement: LCD Enable First Line Special Timing

The PPU SHALL implement special timing for the first scanline after LCD enable, where line 0 starts in Mode 0 and
transitions directly to Mode 3, bypassing the normal Mode 2 OAM scan.

#### Scenario: Line 0 mode sequence after LCD enable

- **WHEN** the LCD is enabled by setting LCDC bit 7
- **THEN** line 0 SHALL start in Mode 0 (HBlank)
- **AND** line 0 SHALL transition directly to Mode 3 (drawing)
- **AND** line 0 SHALL NOT enter Mode 2 (OAM scan)
- **AND** lines 1 and onwards SHALL follow normal Mode 2 → Mode 3 → Mode 0 sequence

#### Scenario: 2 T-cycle timing offset on first line

- **WHEN** the LCD is enabled
- **THEN** the first line timing SHALL be "late" by 2 T-cycles compared to normal lines
- **AND** subsequent lines SHALL have standard timing

#### Scenario: STAT mode bits after LCD enable

- **WHEN** the LCD is enabled
- **AND** the STAT register is read immediately after
- **THEN** STAT mode bits (bits 0-1) SHALL indicate Mode 0 (HBlank)
- **AND** after the initial delay, mode bits SHALL indicate Mode 3

### Requirement: VBlank STAT Mode 2 Interrupt

The PPU SHALL fire a STAT interrupt at the start of VBlank (line 144) if the Mode 2 OAM interrupt is enabled in STAT, in
addition to the normal VBlank interrupt.

#### Scenario: Mode 2 OAM interrupt at line 144

- **WHEN** STAT bit 5 (Mode 2 OAM interrupt enable) is set
- **AND** the PPU transitions from line 143 HBlank to line 144 (VBlank)
- **THEN** a STAT interrupt SHALL be requested at the same time as the VBlank interrupt
- **AND** the timing SHALL match VBlank interrupt timing exactly

#### Scenario: Only VBlank interrupt without Mode 2 enabled

- **WHEN** STAT bit 5 (Mode 2 OAM interrupt enable) is NOT set
- **AND** the PPU transitions to VBlank
- **THEN** only the VBlank interrupt SHALL be requested
- **AND** no STAT interrupt SHALL be requested

### Requirement: Mode 0 Timing After Mode 2 Interrupt

The PPU SHALL implement accurate timing from Mode 2 interrupt to Mode 0 transition, including variable Mode 3 duration
based on sprite presence and positions.

#### Scenario: Base timing without sprites

- **WHEN** a STAT Mode 2 interrupt fires
- **AND** no sprites are present on the current scanline
- **AND** there is no scroll offset (SCX mod 8 = 0)
- **THEN** Mode 0 SHALL begin approximately 80 + 172 = 252 cycles after the interrupt
- **AND** OAM SHALL become readable ~46 cycles after the interrupt
- **AND** Mode 3 SHALL begin ~3-4 cycles after the interrupt

#### Scenario: Sprite presence extends Mode 3

- **WHEN** sprites are present on the current scanline
- **THEN** Mode 3 duration SHALL be extended based on sprite count and positions
- **AND** each sprite adds 0-2 extra cycles depending on its X coordinate
- **AND** sprite X coordinates 0-7 add maximum penalty
- **AND** sprite X coordinates ≥168 add no penalty

### Requirement: SCX Effect on HBlank to LY Increment Timing

The PPU SHALL implement SCX-dependent timing for the duration between Mode 0 (HBlank) interrupt and the LY register
increment.

#### Scenario: SCX mod 8 equals 0

- **WHEN** SCX register has value where (SCX mod 8) = 0
- **AND** a Mode 0 (HBlank) STAT interrupt fires
- **THEN** LY SHALL increment 51 cycles after the interrupt

#### Scenario: SCX mod 8 equals 1-4

- **WHEN** SCX register has value where (SCX mod 8) is 1, 2, 3, or 4
- **AND** a Mode 0 (HBlank) STAT interrupt fires
- **THEN** LY SHALL increment 50 cycles after the interrupt

#### Scenario: SCX mod 8 equals 5-7

- **WHEN** SCX register has value where (SCX mod 8) is 5, 6, or 7
- **AND** a Mode 0 (HBlank) STAT interrupt fires
- **THEN** LY SHALL increment 49 cycles after the interrupt
