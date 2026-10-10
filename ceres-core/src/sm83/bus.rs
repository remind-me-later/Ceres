//! The bus the CPU runs against.

/// The bus the SM83 executes against. The CPU is fully decoupled from the
/// rest of the system: on every M-cycle it either performs one internal
/// cycle (`tick`) or one bus access (`read`/`write`).
///
/// # Timing model
///
/// Time is consumed in M-cycle quanta (4 T-cycles), deferred by the CPU and
/// flushed by the bus at access boundaries:
///
/// - `tick` defers one internal M-cycle's 4 T-cycles.
/// - `read`/`write` first flush any deferred time (so the access observes
///   the machine after all preceding M-cycles of the instruction), perform
///   the access, then defer the access's own 4 T-cycles.
/// - The host flushes whatever remains at the end of each `step`.
pub(crate) trait Bus {
    // -- Time and memory ------------------------------------------------------

    /// One internal (no bus access) M-cycle: defer 4 T-cycles.
    fn tick(&mut self);

    /// Flush all deferred time, perform the bus read at that instant, then
    /// defer the read M-cycle's 4 T-cycles.
    fn read(&mut self, addr: u16) -> u8;

    /// Flush all deferred time, perform the bus write at that instant, then
    /// defer the write M-cycle's 4 T-cycles.
    fn write(&mut self, addr: u16, val: u8);

    /// Read without consuming time or triggering side effects.
    fn peek(&self, addr: u16) -> u8;

    /// Flush all deferred time.
    fn flush(&mut self);

    /// Interrupt-dispatch tail: flush all but `t_cycles` T-cycles, which
    /// stay deferred across the step boundary.
    fn defer(&mut self, t_cycles: i32);

    /// Discards the time deferred so far (it was already accounted for).
    fn drop_deferred(&mut self);

    /// Advance the machine by `t_cycles` T-cycles immediately (nothing is
    /// deferred when this is called at the start of a step).
    fn advance(&mut self, t_cycles: i32);

    /// Whether the machine is CGB hardware (regardless of ROM mode).
    fn is_cgb_hardware(&self) -> bool;

    // -- Interrupts -----------------------------------------------------------

    /// `(IF & IE) != 0` — some enabled interrupt line is asserted.
    fn interrupts_pending(&self) -> bool;

    /// Raw IF register (upper 3 bits set).
    fn read_if(&self) -> u8;

    /// Raw IE register.
    fn read_ie(&self) -> u8;

    /// Clear the acknowledged interrupt bit in IF.
    fn ack_interrupt(&mut self, bit: u8);

    /// SM83 illegal-opcode behavior: IE is cleared.
    fn clear_ie(&mut self);

    // -- OAM bug and DMA ------------------------------------------------------

    /// An internal M-cycle with `addr` on the address bus: flushes the
    /// deferred time, triggers the DMG OAM bug for `addr`, defers 4 T-cycles.
    fn tick_oam_bug(&mut self, addr: u16);

    /// The DMG OAM bug for an address placed on the bus (no time passes).
    fn trigger_oam_bug(&mut self, addr: u16);

    /// Runs the OAM DMA for the cycles it is owed (0 unless `wake` is set; on
    /// a wake-up the DMA is given one M-cycle).
    fn dma_run(&mut self, wake: bool);

    /// The CPU is about to halt: an OAM DMA whose last step is due takes it
    /// first (a halted DMA does not move, and that step frees OAM).
    fn dma_finish_before_halt(&mut self);

    /// Run a pending HDMA transfer chunk, if any.
    fn tick_hdma(&mut self);

    /// An HBlank transfer is requested and has not run yet, on a CGB-C
    /// (gambatte's HALT then prefetches the next opcode).
    fn hdma_request_pending(&self) -> bool;

    /// HALT prefetched the next opcode for a pending transfer: the transfer
    /// runs at the wake in the time of that fetch. The CPU keeps the opcode
    /// (`Sm83::prefetched`), the machine the timing (`hdma_halt_prefetch`).
    fn note_halt_prefetch(&mut self);

    // -- HALT and STOP --------------------------------------------------------

    /// The CPU entered (or, with `false`, left) HALT.
    fn set_halted(&mut self, halted: bool);

    /// The CPU is in STOP mode (waiting for a joypad press).
    fn is_stopped(&self) -> bool;

    /// Enter STOP mode: DIV write, DIV freeze when interrupts are disabled,
    /// PPU stop, clock stop. `ime` is the CPU's current IME state.
    fn enter_stop(&mut self, ime: bool);

    /// Cancel STOP mode (`ppu.leave_stop_mode` + unfreeze the clock).
    fn wake_from_stop(&mut self);

    /// Leave STOP mode without touching the speed-switch halt countdown.
    fn leave_stop(&mut self);

    // -- CGB speed switch -----------------------------------------------------

    /// KEY1 speed-switch requested (`key1.is_requested`).
    fn speed_switch_requested(&self) -> bool;

    /// Start the CGB speed switch (SameBoy's `stop` speed-switch block).
    fn begin_speed_switch(&mut self, interrupt_pending: bool);

    /// Cancel the post-speed-switch halt.
    fn clear_speed_switch_halt(&mut self);

    /// The post-speed-switch halt expired since the last call.
    fn take_unhalt(&mut self) -> bool;
}
