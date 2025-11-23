# Design: Serial Communication Improvement

## Context

The Game Boy serial port enables communication between two Game Boy units via a link cable. One unit acts as the
"master" (internal clock) and the other as the "slave" (external clock). Data is transferred bit-by-bit using a shift
register protocol over 8 cycles.

Current implementation only supports master mode for test ROM output capture. Real games use serial for multiplayer
(Pokemon trades, Tetris battles) and accessories (Game Boy Printer, Mobile Adapter).

### Reference Implementations

- **SameBoy**: Uses callback system (`GB_serial_transfer_bit_start_callback_t` and
  `GB_serial_transfer_bit_end_callback_t`) to notify frontend about bit transfers. Frontend can implement any external
  device logic.
- **Pan Docs**: Defines exact timing for DMG (8192 Hz) and CGB modes (up to 524288 Hz in double speed), bit shifting
  protocol, and disconnect behavior.

### Constraints

- Must remain `no_std` compatible (no heap allocations in core)
- Backward compatible with existing test ROM output capture
- Should support arbitrary external clock speeds (slave mode can be driven at any rate)
- Callback system must be optional (for headless testing)

## Goals / Non-Goals

### Goals

- Full Pan Docs compliance for serial transfer behavior
- Support both master and slave modes
- Callback system for external device emulation
- Accurate timing for all clock speeds
- Enable link cable emulation between two emulator instances
- Maintain test ROM compatibility

### Non-Goals

- Built-in link cable networking (frontend responsibility via callbacks)
- Specific device emulation (printer, mobile adapter) in core
- Save state serialization (can be added later)
- GUI for link cable configuration (frontend responsibility)

## Decisions

### 1. Callback System Design

**Decision**: Use trait-based callback system similar to `AudioCallback`

```rust
pub trait SerialCallback {
    /// Called when a bit is shifted out (start of transfer for that bit)
    fn bit_start(&mut self, bit_out: bool);

    /// Called when a bit needs to be shifted in (end of transfer for that bit)
    /// Returns the bit value to shift into SB register
    fn bit_end(&mut self) -> bool;
}
```

**Rationale**:

- Matches existing `AudioCallback` pattern in codebase
- Zero-cost abstraction when no callback is provided
- Trait allows frontend flexibility (network, file, second emulator instance)
- `no_std` compatible

**Alternatives Considered**:

- Function pointers: Less flexible, harder to maintain state
- Direct emulator-to-emulator coupling: Violates separation of concerns
- Channel-based communication: Requires `std`, not suitable for core

### 2. State Machine Structure

**Decision**: Maintain unified state machine with mode flag

```rust
pub struct Serial {
    // Transfer state
    count: u8,           // Bit counter (0-7)
    bit_clock: bool,     // Internal clock phase

    // Mode and timing
    is_master: bool,     // Internal clock (true) or external (false)
    div_mask: u8,        // Divider mask for clock speed

    // Registers
    sb: u8,              // Serial transfer data
    sc: u8,              // Serial control

    // Test ROM output (legacy)
    output: String,
    sb_sent: u8,
}
```

**Rationale**:

- Simple and efficient
- Clear separation of master/slave logic in methods
- Minimal memory overhead
- Easy to understand state transitions

**Alternatives Considered**:

- Enum-based state machine: More complex, harder to read
- Separate Master/Slave structs: Code duplication, switch overhead

### 3. External Clock API

**Decision**: Provide method for external clock pulses

```rust
impl Serial {
    /// Drive the external clock (slave mode only)
    /// Returns true if transfer completed
    pub fn external_clock_pulse(&mut self, bit_in: bool) -> bool {
        // Only works in slave mode
        // Shifts bit, increments counter
        // Returns true when 8 bits transferred
    }
}
```

**Rationale**:

- Explicit API for external clocking
- Allows arbitrary external clock speeds
- Returns completion status for interrupt handling
- Frontend controls timing

**Alternatives Considered**:

- Automatic timing: Not possible for external clock
- Callback-driven: Too complex, inverts control flow

### 4. Disconnect Behavior

**Decision**: Default to `true` (1) for incoming bits when no callback provided

```rust
// In run_master:
let bit_in = callback.map_or(true, |cb| cb.bit_end());
self.sb = (self.sb << 1) | u8::from(bit_in);
```

**Rationale**:

- Matches hardware behavior (pulled high)
- Master receives 0xFF when disconnected (Pan Docs compliant)
- Simple to implement

### 5. Timing Implementation

**Decision**: Continue using divider-based timing for master mode

Current implementation triggers serial clock based on DIV register updates. This is accurate and integrates well with
existing timing system.

For slave mode, timing is external - frontend calls `external_clock_pulse` as needed.

**Rationale**:

- Existing timing is accurate
- Minimal changes needed
- Matches SameBoy approach

## Architecture

```text
┌─────────────────────────────────────────────────────────┐
│                    Frontend (egui/gtk)                   │
│  ┌───────────────────────────────────────────────────┐  │
│  │         SerialCallback Implementation             │  │
│  │  (Link cable, network, file, second emulator)     │  │
│  └────────────────┬────────────────┬─────────────────┘  │
│                   │ bit_start()    │ bit_end()           │
└───────────────────┼────────────────┼─────────────────────┘
                    │                │
┌───────────────────┼────────────────┼─────────────────────┐
│  ceres-core       │                │                      │
│  ┌────────────────▼────────────────▼──────────────────┐  │
│  │              Serial Module                         │  │
│  │  ┌──────────────┐         ┌──────────────────┐    │  │
│  │  │ Master Mode  │         │  Slave Mode      │    │  │
│  │  │ (internal    │         │  (external       │    │  │
│  │  │  clock)      │         │   clock)         │    │  │
│  │  │              │         │                  │    │  │
│  │  │ - div_mask   │         │ - waits for      │    │  │
│  │  │ - run_master │         │   external_clock │    │  │
│  │  │              │         │   _pulse()       │    │  │
│  │  └──────────────┘         └──────────────────┘    │  │
│  │                                                    │  │
│  │  SB (shift register) ◄──────────────────────────► │  │
│  │  SC (control register)                            │  │
│  │  Output capture (test ROMs)                       │  │
│  └────────────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────────────┘
```

## Data Flow

### Master Mode (Internal Clock)

1. CPU writes to SB (data to send)
2. CPU writes 0x81 to SC (start transfer, internal clock)
3. `run_master()` called by divider timing system
4. For each bit (0-7):
   - Call `callback.bit_start(bit)` with outgoing bit
   - Shift SB left
   - Call `callback.bit_end()` to get incoming bit
   - Shift incoming bit into SB
   - Increment counter
5. After 8 bits:
   - Clear SC bit 7 (transfer complete)
   - Request serial interrupt
   - Capture to output buffer (test ROM compatibility)

### Slave Mode (External Clock)

1. CPU writes to SB (data to send)
2. CPU writes 0x80 to SC (enable serial, external clock)
3. External device calls `external_clock_pulse(bit_in)` for each bit
4. Serial module:
   - Shifts SB left
   - Shifts incoming bit into SB
   - Increments counter
5. After 8 bits:
   - Clear SC bit 7
   - Request serial interrupt
   - Returns true from `external_clock_pulse` to signal completion

## Migration Plan

### Phase 1: Add Callback System (non-breaking)

- Add `SerialCallback` trait to `lib.rs`
- Add `Option<Box<dyn SerialCallback>>` to Gb struct
- Add registration method `set_serial_callback()`
- Update `run_master()` to use callbacks when available
- Maintain existing test ROM output behavior

### Phase 2: Add Slave Mode Support

- Add `external_clock_pulse()` method
- Update `write_sc()` to handle slave mode properly
- Add tests for slave mode behavior
- Document external clock API

### Phase 3: Documentation and Examples

- Add callback implementation examples to docs
- Document link cable emulation approach
- Update AGENTS.md with serial communication info

### Rollback Plan

If issues arise:

- Callback system is optional - can be disabled
- Existing test ROM functionality unaffected
- Can revert to master-only mode with minimal impact

## Risks / Trade-offs

### Risks

1. **Timing complexity**: Different clock speeds need careful testing

   - **Mitigation**: Use existing divider-based timing, extensive tests

2. **Callback overhead**: Function calls on every bit transfer

   - **Mitigation**: Only 8 calls per byte, negligible compared to CPU emulation

3. **API complexity**: More methods exposed to frontends

   - **Mitigation**: Callbacks are optional, clear documentation

4. **Breaking changes**: Struct layout changes
   - **Mitigation**: Minimize impact, provide migration guide

### Trade-offs

- **Simplicity vs Accuracy**: More complex code for accurate emulation (worth it)
- **Flexibility vs Performance**: Callback system adds indirection (negligible impact)
- **Core vs Frontend responsibility**: Callbacks push complexity to frontend (correct separation)

## Open Questions

1. **Save state format**: How to serialize serial state?

   - **Answer**: Defer to future change, not blocking

2. **Multiple callbacks**: Support for logging + device emulation?

   - **Answer**: Frontend can implement multi-callback wrapper, core provides single callback

3. **CGB double speed mode**: Does serial timing change?
   - **Answer**: Yes, documented in Pan Docs - 16384 Hz vs 8192 Hz base rate

## Testing Strategy

1. **Unit tests**: Individual methods (bit shifting, state transitions)
2. **Integration tests**:
   - Master mode with/without callback
   - Slave mode with external clock
   - CGB speed modes
   - Disconnect behavior (0xFF input)
3. **Test ROM compatibility**: Ensure Blargg tests still pass
4. **Manual testing**: Two emulator instances with link cable emulation

## References

- [Pan Docs - Serial Data Transfer](<https://gbdev.io/pandocs/Serial_Data_Transfer_(Link_Cable).html>)
- [SameBoy serial implementation](https://github.com/LIJI32/SameBoy)
- Existing `ceres-core/src/serial.rs`
- Existing `AudioCallback` pattern in codebase
