//! The timer and IF wired together: ceres's M-cycle of the two
//! (`relation`), and Kani's proofs that it is the blocks' steps in sequence
//! and that their proofs against the netlist join (`bisim`).

#[cfg(kani)]
mod bisim;
pub(crate) mod relation;
