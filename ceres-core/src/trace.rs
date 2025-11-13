//! Execution tracing using the Rust tracing crate.
//!
//! This module provides integration with the standard Rust `tracing` crate
//! for collecting and logging execution traces. The structured logging
//! approach allows for flexible output formats (JSON, plain text, etc.)
//! and works with standard tracing tooling.
//!
//! # Performance
//!
//! Tracing has minimal performance impact when disabled at compile time
//! or filtered out at runtime. When enabled, the overhead is typically
//! low due to the efficient design of the tracing crate.
//!
//! # Example
//!
//! ```rust,ignore
//! // Example of how to configure tracing in your application:
//! use ceres_core::{AudioCallback, Sample, GbBuilder};
//!
//! struct DummyAudio;
//! impl AudioCallback for DummyAudio {
//!     fn audio_sample(&self, _l: Sample, _r: Sample) {}
//! }
//!
//! // Set up tracing subscriber before running the emulator
//! use tracing_subscriber::{fmt, EnvFilter};
//!
//! tracing::subscriber::with_default(
//!     fmt::Subscriber::builder()
//!         .with_env_filter(EnvFilter::from_default_env())
//!         .finish(),
//!     || {
//!         let mut gb = GbBuilder::new(44100, DummyAudio).build();
//!         gb.set_trace_enabled(true);
//!
//!         // Run emulation...
//!         for _ in 0..1000 {
//!             gb.run_cpu();
//!         }
//!     }
//! );
//! ```

/// Trace event for a single executed instruction.
///
/// This function logs detailed information about an executed instruction
/// using the Rust tracing crate. The information includes the program counter,
/// disassembled instruction, register state, and cycle count.
///
/// # Arguments
///
/// * `pc` - Program counter where the instruction was executed
/// * `instruction` - The disassembled instruction string
/// * `a` - Value of the A register
/// * `f` - Value of the F register (flags)
/// * `b` - Value of the B register
/// * `c` - Value of the C register
/// * `d` - Value of the D register
/// * `e` - Value of the E register
/// * `h` - Value of the H register
/// * `l` - Value of the L register
/// * `sp` - Value of the stack pointer
/// * `cycles` - Number of cycles the instruction took
#[expect(clippy::many_single_char_names)]
#[expect(clippy::too_many_arguments)]
#[inline]
pub fn instruction(
    pc: u16,
    instruction: &str,
    a: u8,
    f: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    h: u8,
    l: u8,
    sp: u16,
    cycles: u8,
) {
    tracing::event!(
        target: "cpu_execution",
        tracing::Level::TRACE,
        pc = pc,
        instruction = instruction,
        a = a,
        f = f,
        b = b,
        c = c,
        d = d,
        e = e,
        h = h,
        l = l,
        sp = sp,
        cycles = cycles,
        "EXECUTE_INSTRUCTION"
    );
}

#[cfg(test)]
mod tests {
    // FIXME: Add more comprehensive tests that verify the tracing output

    use super::*;

    #[test]
    fn test_instruction() {
        // Just test that the function can be called without panicking
        // The actual tracing functionality would be tested in integration tests
        instruction(
            0x100, "NOP", 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0000, 4,
        );
    }
}
