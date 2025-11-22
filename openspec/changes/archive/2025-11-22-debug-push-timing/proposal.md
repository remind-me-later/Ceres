# Change: Debug Mooneye test_mooneye_push_timing failure

## Why

The Mooneye test `test_mooneye_push_timing` is currently failing with all CPU registers containing 0x42 (Mooneye
failure code). This test verifies the exact M-cycle timing of PUSH instructions by using OAM DMA as a timing sensor to
detect when memory write operations occur.

The test is part of the Mooneye acceptance test suite and validates that our SM83 CPU emulation matches hardware
behavior for PUSH instruction timing, which is critical for accurate emulation.

## Root Cause Analysis

### Test Methodology

The `push_timing` test uses OAM DMA as a timing sensor:

1. Sets up VRAM ($8000+) with known value $81
2. Starts OAM DMA transfer from $8000 to OAM ($FE00-$FE9F)
3. Sets SP to OAM+$10 ($FE10) where PUSH will write
4. Executes PUSH DE at specific timing relative to DMA progress
5. Pops values back to check what was written (DMA value $81 vs CPU value)

From the test source (`push_timing.s`):

```text
; PUSH rr is expected to have the following timing:
; M = 0: instruction decoding
; M = 1: internal delay
; M = 2: memory access for high byte
; M = 3: memory access for low byte
```

The test verifies:

- **Round 1** (nops 2): OAM accessible at M=2 → high byte should be written by CPU (reads back correct value $42)
- **Round 2** (nops 1): OAM accessible at M=3 → low byte should be written by CPU (reads back correct value $24)

### Current Implementation Issue

The current `push()` implementation in `ceres-core/src/sm83.rs`:

```rust
fn push(&mut self, val: u16) {
    let [lo, hi] = val.to_le_bytes();
    self.cpu.sp = self.cpu.sp.wrapping_sub(1);
    self.write_cpu(self.cpu.sp, hi);    // Calls tick_m_cycle() then writes
    self.cpu.sp = self.cpu.sp.wrapping_sub(1);
    self.write_cpu(self.cpu.sp, lo);    // Calls tick_m_cycle() then writes
    self.tick_m_cycle();                 // Extra tick at end
}
```

The problem is in the **order of operations** within `write_cpu()`:

```rust
fn write_cpu(&mut self, addr: u16, val: u8) {
    // ... DMA logging code ...
    self.tick_m_cycle();     // Time advances BEFORE memory access
    self.write_mem(addr, val);
}
```

**This creates the wrong timing pattern:**

- M=0: Instruction decode (implicit)
- M=1: `tick_m_cycle()` from first `write_cpu()`, then write high byte
- M=2: `tick_m_cycle()` from second `write_cpu()`, then write low byte
- M=3: Extra `tick_m_cycle()` at end of `push()`

**Expected hardware timing:**

- M=0: Instruction decode
- M=1: Internal delay
- M=2: Write high byte
- M=3: Write low byte

### Comparison with POP Implementation

The `pop()` function works correctly because `read_cpu()` advances time **before** the read:

```rust
fn pop(&mut self) -> u16 {
    let lo = self.read_cpu(self.cpu.sp);  // M=1: tick then read low byte
    self.cpu.sp = self.cpu.sp.wrapping_add(1);
    let hi = self.read_cpu(self.cpu.sp);  // M=2: tick then read high byte
    self.cpu.sp = self.cpu.sp.wrapping_add(1);
    u16::from_le_bytes([lo, hi])
}
```

This matches hardware POP timing:

- M=0: Instruction decode
- M=1: Read low byte
- M=2: Read high byte

The key difference: **POP doesn't need an internal delay cycle**, so ticking before the read is correct. **PUSH needs an
internal delay**, so it should tick once before any memory writes.

### SameBoy Reference Implementation

According to the SameBoy analysis (from cognitionai):

1. **M=1 (Internal Delay)**: Calls `cycle_oam_bug(GB_REGISTER_SP)` which:
   - Flushes pending cycles
   - Handles OAM bug corruption (DMG only)
   - Sets `pending_cycles = 4` (one M-cycle)

2. **M=2 (Write High Byte)**: Calls `cycle_write(--sp, high_byte)` which:
   - Advances cycles with `GB_advance_cycles(pending_cycles)`
   - Writes the high byte to memory
   - Sets `pending_cycles = 4`

3. **M=3 (Write Low Byte)**: Calls `cycle_write(--sp, low_byte)` which:
   - Advances cycles with `GB_advance_cycles(pending_cycles)`
   - Writes the low byte to memory
   - Sets `pending_cycles = 4`

The critical insight: **SameBoy advances time AFTER each memory write**, but has an explicit internal delay cycle at
M=1 before any writes occur.

### OAM Bug Consideration

The OAM bug is a DMG-specific hardware quirk where certain CPU operations can corrupt OAM during PPU modes 2 and 3.
SameBoy's `cycle_oam_bug` handles this during the internal delay cycle.

For Ceres:

- The test runs on CGB model, where the OAM bug doesn't apply
- However, we should still implement the correct timing structure for DMG compatibility
- The internal delay cycle at M=1 is where OAM bug handling would occur on DMG

## What Changes

1. **Add internal delay cycle to PUSH**: Modify `push()` to tick one M-cycle at the start for the internal delay (M=1)
2. **Change write timing pattern**: Memory writes should occur at M=2 and M=3, not M=1 and M=2
3. **Remove extra tick**: The final `tick_m_cycle()` at the end of `push()` should be removed since we're adding it at
   the start
4. **Consider write_cpu timing**: May need to introduce a variant that writes without advancing time, or restructure the
   timing

## Proposed Solution

### Option 1: Modify write_cpu to separate timing from write

Create a new helper that writes without advancing time:

```rust
fn write_mem_only(&mut self, addr: u16, val: u8) {
    // DMA logging code...
    self.write_mem(addr, val);
}

fn push(&mut self, val: u16) {
    let [lo, hi] = val.to_le_bytes();
    self.tick_m_cycle(); // M=1: internal delay
    self.cpu.sp = self.cpu.sp.wrapping_sub(1);
    self.tick_m_cycle(); // M=2: high byte write cycle
    self.write_mem_only(self.cpu.sp, hi);
    self.cpu.sp = self.cpu.sp.wrapping_sub(1);
    self.tick_m_cycle(); // M=3: low byte write cycle
    self.write_mem_only(self.cpu.sp, lo);
}
```

### Option 2: Restructure push timing to match hardware

Keep the current structure but adjust timing:

```rust
fn push(&mut self, val: u16) {
    let [lo, hi] = val.to_le_bytes();
    self.tick_m_cycle(); // M=1: internal delay
    self.cpu.sp = self.cpu.sp.wrapping_sub(1);
    // Write without tick (write happens AT M=2)
    self.write_mem(self.cpu.sp, hi);
    self.tick_m_cycle(); // M=2 completes
    self.cpu.sp = self.cpu.sp.wrapping_sub(1);
    // Write without tick (write happens AT M=3)
    self.write_mem(self.cpu.sp, lo);
    self.tick_m_cycle(); // M=3 completes
}
```

**However**, this breaks the pattern used by `write_cpu()` elsewhere. We need to carefully audit other uses of `write_cpu()`.

### Recommended Approach

After analysis, **Option 1 is safer** because:

1. It maintains the existing `write_cpu()` semantics for other instructions
2. Makes the PUSH timing explicit and clear
3. Allows for future OAM bug handling at the internal delay cycle (M=1)
4. Matches the structure of SameBoy's implementation

## Impact

- **Affected specs**: CPU instruction timing (new requirement for PUSH timing)
- **Affected code**:
  - `ceres-core/src/sm83.rs`: Modify `push()` function
  - Potentially add `write_mem_only()` helper if using Option 1
- **Risk**: Low. PUSH timing is isolated to the `push()` function used by:
  - `PUSH rr` instructions (opcodes 0xC5, 0xD5, 0xE5, 0xF5)
  - `CALL nn` and `CALL cc, nn` instructions (for pushing PC)
  - `RST` instructions (for pushing PC)
  - Interrupt dispatch (for pushing PC)
- **Dependencies**: None - this is a pure timing fix
- **Testing**:
  - Primary: `test_mooneye_push_timing` should pass
  - Regression: Verify `test_mooneye_call_timing`, `test_mooneye_call_cc_timing` still pass
  - Verify interrupt-related tests still pass (they use push during dispatch)
