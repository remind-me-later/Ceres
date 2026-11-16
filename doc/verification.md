# How ceres is verified

Each layer checks the next against something more trusted:

1. **dmg-sim** (`external/reference-implementations/dmg-sim`), the
   gate-level simulation of the DMG-CPU B's netlist, is the oracle: test
   ROMs and the probes of `hdl/probes` run on it unchanged
   ([hdl/README.md](../hdl/README.md)).
2. **The readable model** (`hdl/rtl`), VHDL written block by block to be
   read, is checked against dmg-sim's recorded signals (`make check` in
   `hdl/`).
3. **ceres-netlist** transcribes the model's blocks into Rust, checked by
   replaying the model's traces through them
   ([ceres-netlist/README.md](../ceres-netlist/README.md)).
4. **ceres-core** is held to ceres-netlist block by block. Each block has
   a step relation (`relation.rs`), a co-simulation that checks it on
   concrete runs (`cosim.rs`, `cargo test -p ceres-core --features verify
   cosim`), and Kani proofs that it holds from every related state
   (`bisim.rs`); `proofs.rs` proves ceres's own rules
   ([verify/kani/README.md](../verify/kani/README.md)).
5. **Lean** (`lean/`) takes the Kani proofs as hypotheses and proves that
   the blocks agree forever and when wired together
   ([lean/README.md](../lean/README.md)).

What each proof assumes, and the evidence for it, is in the knowledge
base ([verify/knowledge/README.md](../verify/knowledge/README.md)), with
the facts about the hardware where the evidence is missing or
contradictory.
