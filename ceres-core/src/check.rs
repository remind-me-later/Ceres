//! Ceres's timer and IF in one M-cycle, as ceres runs them, T-cycle by
//! T-cycle, and that this is the timer's step and then IF's
//! (`timing::check`, `interrupts::check`): Kani proves `in_sequence` for
//! every argument, and Lean (`lean/`) carries the two blocks' bisimulation,
//! wired one into the other, over to this M-cycle.
//!
//! Written as Aeneas takes it, as ceres-netlist is (see its crate
//! documentation).
#![cfg_attr(
    not(kani),
    expect(
        dead_code,
        reason = "outside Kani, only Charon reads the steps (`lean/translate.sh`)"
    )
)]

use crate::{
    interrupts::{
        Interrupts,
        check::{ceres_boundary, ceres_step, ceres_t_cycle, requests, same_ints},
    },
    timing::{
        Clock,
        check::{
            CheckHost, TimerHost, ceres_access, ceres_m_cycle, related, same_clock, same_read,
        },
    },
};
use ceres_netlist::{
    interrupts::{Access as IfAccess, lowest},
    timer::{Access as TimerAccess, NetTimer},
};

/// A DMG in single speed whose timer requests its interrupt in IF, as the
/// machine's does.
pub(crate) struct JointHost {
    serial_div_mask: u16,
    ints: Interrupts,
}

impl JointHost {
    pub(crate) const fn new(serial_div_mask: u16, ints: Interrupts) -> Self {
        Self {
            serial_div_mask,
            ints,
        }
    }

    pub(crate) const fn ints(&self) -> &Interrupts {
        &self.ints
    }
}

impl TimerHost for JointHost {
    fn is_cgb_hardware(&self) -> bool {
        false
    }

    fn is_cgb_mode(&self) -> bool {
        false
    }

    fn double_speed(&self) -> bool {
        false
    }

    fn request_timer_interrupt(&mut self) {
        self.ints.request_timer();
    }

    fn serial_div_mask(&self) -> u16 {
        self.serial_div_mask
    }

    fn serial_clock_edge(&mut self) {}

    fn apu_frame_sequencer_step(&mut self, _: &Clock) {}

    fn apu_envelopes_armed(&mut self) {}

    fn apu_delayed_envelope_tick(&mut self, _: &Clock) {}

    fn apu_m_cycle(&mut self, _: &Clock) {}
}

/// Ceres's M-cycle of the timer and IF: the accesses at the boundary, then
/// four T-cycles, in each of which the timer runs (and may request its
/// interrupt in IF) and then IF takes the other requests, the write and the
/// acknowledge. The timer's read, IF's read and the interrupt an entry
/// takes.
pub(crate) fn ceres_m_cycle_joint(
    clock: &mut Clock,
    host: &mut JointHost,
    timer_access: TimerAccess,
    if_access: IfAccess,
    rises: [u8; 8],
) -> (Option<u8>, Option<u8>, Option<u8>) {
    let timer_read = ceres_access(clock, host, timer_access);
    let (if_read, queue) = ceres_boundary(&host.ints, if_access);
    let taken = match if_access {
        IfAccess::Entry => Some(lowest(queue)),
        IfAccess::None | IfAccess::ReadIf | IfAccess::WriteIf(_) => None,
    };
    let other = requests(rises);
    let mut t = 0;
    while t < 4 {
        clock.run(host, 1);
        ceres_t_cycle(&mut host.ints, if_access, t, other[t], queue);
        t += 1;
    }
    (timer_read, if_read, taken)
}

/// From two equal timers related to a netlist's (`timing::check::related`:
/// so in a state ceres can be in) and two equal IFs, the M-cycle run T-cycle by
/// T-cycle gives what the timer's M-cycle and then IF's give, the timer's
/// request in the T-cycle it came in: the same reads, the same interrupt
/// taken, equal timers and equal IFs; unless the timer requests twice,
/// which its step rules out. True for every argument (Kani's
/// `ceres_timer_and_if_in_sequence`).
#[expect(clippy::too_many_arguments, reason = "every input of the M-cycle")]
pub(crate) fn in_sequence(
    net: NetTimer,
    clock: Clock,
    clock_joint: Clock,
    ints: Interrupts,
    ints_joint: Interrupts,
    timer_access: TimerAccess,
    serial_div_mask: u16,
    if_access: IfAccess,
    rises: [u8; 8],
) -> bool {
    if !related(&net, &clock) | !same_clock(&clock, &clock_joint) | !same_ints(&ints, &ints_joint) {
        return true;
    }
    let mut clock = clock;
    let mut host = CheckHost::new(serial_div_mask);
    let (timer_read, timer_irq, twice) = ceres_m_cycle(&mut clock, &mut host, timer_access);
    let mut ints = ints;
    let (if_read, taken) = ceres_step(&mut ints, if_access, rises, timer_irq);

    let mut clock_joint = clock_joint;
    let mut joint = JointHost::new(serial_div_mask, ints_joint);
    let (joint_timer_read, joint_if_read, joint_taken) =
        ceres_m_cycle_joint(&mut clock_joint, &mut joint, timer_access, if_access, rises);
    twice
        | (same_read(timer_read, joint_timer_read)
            & same_read(if_read, joint_if_read)
            & same_read(taken, joint_taken)
            & same_clock(&clock, &clock_joint)
            & same_ints(&ints, &joint.ints))
}
