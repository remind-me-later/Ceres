//! Rust transcriptions of the blocks of the DMG-CPU B's readable model
//! (`hdl/rtl`, which `make check` holds to dmg-sim, the netlist's
//! simulation), phase by phase or edge by edge: a block is a struct of its
//! flip-flops and latches, and its methods do what the VHDL's processes do
//! at a clock's edge.
//!
//! The transcriptions take nothing from ceres: they are what ceres is held
//! to. `tests/replay.rs` replays the model's traces (`hdl/trace-blocks.sh`)
//! through them, phase by phase, and finds where one parts from the model;
//! ceres-core's Kani proofs (its `netlist_proofs.rs` files, which use this
//! crate under `cfg(kani)` only) prove ceres's blocks alike to them. What the
//! proofs take as given is in `doc/assumptions.toml`.

#![expect(
    clippy::missing_inline_in_public_items,
    reason = "the workspace's release profile links with fat LTO, which inlines across crates"
)]

pub mod dma;
pub mod interrupts;
pub mod lcd_timing;
pub mod oam;
pub mod oam_scan;
pub mod timer;
