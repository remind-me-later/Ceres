# Serial Communication

## ADDED Requirements

### Requirement: Master Mode Serial Transfer

The system SHALL implement master mode (internal clock) serial communication according to Game Boy hardware
specifications.

#### Scenario: Master transfer with internal clock

- **WHEN** CPU writes data byte to SB register (0xFF01)
- **AND** CPU writes 0x81 to SC register (0xFF02) to start transfer with internal clock
- **THEN** serial transfer SHALL shift out 8 bits at the configured clock rate
- **AND** serial interrupt SHALL be requested after 8 bits are transferred
- **AND** SC bit 7 SHALL be cleared to indicate transfer completion

#### Scenario: Master transfer with disconnected device

- **WHEN** master mode transfer occurs without external device connected
- **AND** no callback is registered
- **THEN** incoming bits SHALL default to 1 (pulled high)
- **AND** SB register SHALL contain 0xFF after transfer completion

#### Scenario: Master transfer with callback device

- **WHEN** master mode transfer occurs with callback registered
- **THEN** callback SHALL be invoked for each bit with `bit_start(outgoing_bit)`
- **AND** callback SHALL be queried for each bit with `bit_end() -> incoming_bit`
- **AND** incoming bits SHALL be shifted into SB register

### Requirement: Slave Mode Serial Transfer

The system SHALL implement slave mode (external clock) serial communication for link cable emulation.

#### Scenario: Slave mode with external clock

- **WHEN** CPU writes data byte to SB register
- **AND** CPU writes 0x80 to SC register (external clock, bit 0 = 0)
- **AND** external device drives clock by calling `external_clock_pulse(bit_in)`
- **THEN** serial transfer SHALL shift bits according to external clock timing
- **AND** serial interrupt SHALL be requested after 8 bits
- **AND** SC bit 7 SHALL be cleared

#### Scenario: Slave mode ignores internal timing

- **WHEN** serial port is in slave mode (SC bit 0 = 0)
- **THEN** internal divider timing SHALL NOT trigger bit transfers
- **AND** only external clock pulses SHALL advance the transfer

### Requirement: Clock Speed Configuration

The system SHALL support all Game Boy serial clock speeds according to hardware specifications.

#### Scenario: DMG normal speed

- **WHEN** system is in DMG mode
- **THEN** internal clock SHALL operate at 8192 Hz (8 KHz)
- **AND** transfer SHALL complete in approximately 1024 machine cycles

#### Scenario: CGB normal speed slow mode

- **WHEN** system is in CGB mode with SC bit 1 = 0
- **THEN** internal clock SHALL operate at 8192 Hz
- **AND** behavior SHALL match DMG timing

#### Scenario: CGB normal speed fast mode

- **WHEN** system is in CGB mode with SC bit 1 = 1
- **AND** CGB is NOT in double speed mode
- **THEN** internal clock SHALL operate at 262144 Hz (262 KHz)

#### Scenario: CGB double speed slow mode

- **WHEN** system is in CGB mode with SC bit 1 = 0
- **AND** CGB is in double speed mode
- **THEN** internal clock SHALL operate at 16384 Hz (16 KHz)

#### Scenario: CGB double speed fast mode

- **WHEN** system is in CGB mode with SC bit 1 = 1
- **AND** CGB is in double speed mode
- **THEN** internal clock SHALL operate at 524288 Hz (524 KHz)

### Requirement: Callback System for External Devices

The system SHALL provide a callback mechanism for implementing external devices and link cable emulation.

#### Scenario: Register callback for bit exchange

- **WHEN** frontend registers a `SerialCallback` implementation
- **THEN** callback SHALL be invoked during master mode transfers
- **AND** callback SHALL provide `bit_start(bit_out: bool)` to receive outgoing bits
- **AND** callback SHALL provide `bit_end() -> bool` to supply incoming bits

#### Scenario: Callback is optional

- **WHEN** no callback is registered
- **THEN** serial transfers SHALL still function
- **AND** incoming bits SHALL default to 1 (disconnected behavior)
- **AND** test ROM output capture SHALL work normally

#### Scenario: Callback lifetime management

- **WHEN** callback is registered during emulation
- **THEN** callback SHALL remain active until replaced or removed
- **AND** callback SHALL be called at correct timing for each bit

### Requirement: Register Access Behavior

The system SHALL implement correct register read/write behavior for SB and SC registers.

#### Scenario: Read SB during transfer

- **WHEN** CPU reads SB register during active transfer
- **THEN** current shifted value SHALL be returned
- **AND** value SHALL reflect partial incoming/outgoing bits

#### Scenario: Write SB during transfer

- **WHEN** CPU writes to SB register during active transfer
- **THEN** new value SHALL replace current value
- **AND** ongoing transfer SHALL continue with new data

#### Scenario: Read SC register

- **WHEN** CPU reads SC register
- **AND** system is in DMG mode
- **THEN** SC bit 1 (speed) SHALL read as 0 (not supported in DMG)
- **WHEN** system is in CGB mode
- **THEN** SC bit 1 SHALL reflect the configured speed setting

#### Scenario: Write SC to start transfer

- **WHEN** CPU writes to SC register with bit 7 = 1
- **THEN** serial transfer SHALL initialize
- **AND** bit counter SHALL reset to 0
- **AND** transfer SHALL begin on next clock edge

### Requirement: Interrupt Handling

The system SHALL request serial interrupts at correct timing according to hardware behavior.

#### Scenario: Interrupt on transfer completion

- **WHEN** 8 bits have been transferred in master or slave mode
- **THEN** serial interrupt (INT 0x58) SHALL be requested
- **AND** SC bit 7 SHALL be cleared atomically
- **AND** interrupt SHALL occur before next instruction executes

#### Scenario: No interrupt on partial transfer

- **WHEN** transfer is in progress but not complete
- **THEN** serial interrupt SHALL NOT be requested
- **AND** SC bit 7 SHALL remain set (indicating transfer active)

### Requirement: Bit Shifting Protocol

The system SHALL implement correct bit-by-bit shifting according to Game Boy serial protocol.

#### Scenario: Shift register behavior

- **WHEN** serial bit is transferred
- **THEN** SB SHALL shift left by 1 bit
- **AND** outgoing bit (MSB) SHALL be transmitted
- **AND** incoming bit SHALL be shifted into LSB
- **AND** process SHALL repeat for 8 bits

#### Scenario: Bit counter tracking

- **WHEN** transfer starts
- **THEN** bit counter SHALL be initialized to 0
- **WHEN** each bit is transferred
- **THEN** bit counter SHALL increment
- **WHEN** bit counter reaches 8
- **THEN** transfer SHALL complete

### Requirement: Test ROM Compatibility

The system SHALL maintain backward compatibility with test ROM serial output capture.

#### Scenario: Blargg test ROM output

- **WHEN** test ROM (e.g., Blargg CPU tests) writes to serial port
- **AND** no callback is registered (disconnected mode)
- **THEN** transferred bytes SHALL be captured to output buffer
- **AND** printable ASCII characters SHALL be stored
- **AND** output buffer SHALL be accessible via `serial_output()` method

#### Scenario: Non-printable character handling

- **WHEN** test ROM sends non-printable characters
- **THEN** newline (0x0A) and carriage return (0x0D) SHALL be captured
- **AND** zero bytes (0x00) SHALL be ignored
- **AND** other non-printable bytes MAY be captured as hex notation

### Requirement: Mode Switching

The system SHALL handle switching between master and slave modes correctly.

#### Scenario: Switch from master to slave

- **WHEN** serial is in master mode (SC bit 0 = 1)
- **AND** CPU writes SC with bit 0 = 0
- **THEN** serial SHALL switch to slave mode
- **AND** internal clock timing SHALL be disabled
- **AND** transfer state SHALL be reset

#### Scenario: Switch from slave to master

- **WHEN** serial is in slave mode (SC bit 0 = 0)
- **AND** CPU writes SC with bit 0 = 1
- **THEN** serial SHALL switch to master mode
- **AND** internal clock timing SHALL be enabled
- **AND** transfer state SHALL be reset

### Requirement: External Clock API

The system SHALL provide an API for external clock control in slave mode.

#### Scenario: External clock pulse in slave mode

- **WHEN** serial is in slave mode
- **AND** external device calls `external_clock_pulse(bit_in: bool)`
- **THEN** one bit SHALL be transferred (shift operation)
- **AND** method SHALL return `false` if transfer is incomplete
- **AND** method SHALL return `true` when 8th bit completes transfer

#### Scenario: External clock ignored in master mode

- **WHEN** serial is in master mode
- **AND** external device calls `external_clock_pulse()`
- **THEN** call SHALL be ignored or return error
- **AND** transfer SHALL continue according to internal clock
