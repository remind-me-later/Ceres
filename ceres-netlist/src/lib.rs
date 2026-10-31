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
//!
//! The crate is written so that Aeneas (through Charon) can take it to Lean
//! for proofs: booleans combine with `&` and `|`, not `&&` and `||` (each
//! lazy operator is a branch, and Aeneas's symbolic execution forks on every
//! one); loops run over ranges and index arrays, with no iterators,
//! closures, statics or `Option` helpers, which Aeneas does not model; and
//! the largest structs compare with `flat_eq!`, as the derived `PartialEq`
//! nests a branch per field.

#![expect(
    clippy::missing_inline_in_public_items,
    reason = "the workspace's release profile links with fat LTO, which inlines across crates"
)]
#![expect(
    clippy::needless_bitwise_bool,
    reason = "Aeneas (Lean) takes this crate: a lazy && or || is a branch, and its symbolic execution forks on every one"
)]
#![expect(
    clippy::needless_range_loop,
    clippy::if_then_some_else_none,
    clippy::len_zero,
    clippy::manual_map,
    clippy::option_if_let_else,
    reason = "Aeneas (Lean) takes this crate: it models loops over ranges, indexing and match, not iterators, closures, ranges' contains or Option's helpers"
)]

/// `PartialEq` and `Eq` for a struct, as one flat AND of its fields'
/// equalities. The derived ones chain `&&`, a branch a field, which nests
/// too deeply for Aeneas's Lean output on the largest structs. The pattern
/// names every field, so a field added to the struct and not here fails to
/// compile. A field written `field => same` compares with the function
/// `same` instead of `==` (an `Option`'s, whose equality Aeneas does not
/// model).
macro_rules! flat_eq {
    (@field $mine:expr, $theirs:expr) => {
        ($mine == $theirs)
    };
    (@field $mine:expr, $theirs:expr, $same:path) => {
        $same($mine, $theirs)
    };
    ($name:ident { $($field:ident $(=> $same:path)?),+ $(,)? }) => {
        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                let Self { $($field),+ } = self;
                true $(& flat_eq!(@field *$field, other.$field $(, $same)?))+
            }
        }

        impl Eq for $name {}
    };
}

pub mod bus;
pub mod chip;
pub mod clocks;
pub mod dma;
pub mod interrupts;
pub mod lcd_control;
pub mod lcd_timing;
pub mod oam;
pub mod oam_scan;
pub mod render;
pub mod sm83;
pub mod timer;

pub use clocks::Phase;
