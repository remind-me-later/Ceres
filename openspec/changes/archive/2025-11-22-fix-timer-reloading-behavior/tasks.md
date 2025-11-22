## 1. Implementation

- [ ] 1.1 Update `ceres-core/src/timing.rs` to implement the 4-cycle reload delay state.
- [ ] 1.2 Update `ceres-core/src/timing.rs` to handle TIMA reads during reload (return 0x00).
- [ ] 1.3 Update `ceres-core/src/timing.rs` to handle TIMA writes during reload (ignore).
- [ ] 1.4 Update `ceres-core/src/timing.rs` to handle TMA writes during reload (update reload value).
- [ ] 1.5 Verify `tima_reload` test passes.
- [ ] 1.6 Verify `tima_write_reloading` test passes.
- [ ] 1.7 Verify `tma_write_reloading` test passes.
