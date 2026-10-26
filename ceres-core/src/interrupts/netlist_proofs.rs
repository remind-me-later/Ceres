//! Kani proofs that ceres's IF is the netlist's
//! (`cargo kani -p ceres-core --harness netlist_if`).
//!
//! `NetIf` transcribes `hdl/rtl/interrupts.vhd`, and the latch of the
//! requests and the acknowledge of `hdl/rtl/sm83_interrupts.vhd`, phase by
//! phase. Each flag is set by a rise of its source, forced to the data bus
//! while the CPU writes IF (E to G), and cleared by the acknowledge (G and H
//! of the interrupt entry's last M-cycle); clearing wins. A read of IF, and
//! the entry's choice of interrupt, take the flags as they were at the start
//! of E.
//!
//! Ceres's M-cycle boundary is the start of the netlist's E: a read there
//! sees the four T-cycles before it, an IF write lands after the first
//! T-cycle (`ConflictType::WriteCpu`), and the entry acknowledges after the
//! second (`bus.defer(2)` in `dispatch_interrupt`). So the T-cycle in which
//! a source rises in ceres stands for these phases of the netlist:
//!
//! | T-cycle | phases            |
//! | ------- | ----------------- |
//! | 1       | E, F, G           |
//! | 2       | H                 |
//! | 3, 4    | A to D (the next) |
//!
//! and the proof is a bisimulation under that mapping: from equal IF and IE,
//! an M-cycle with any access and any rises of the sources gives the same
//! read, the same interrupt taken, and equal IF after.

use super::{INT_MASK, Interrupts};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Phase {
    E,
    F,
    G,
    H,
    A,
    B,
    C,
    D,
}

const PHASES: [Phase; 8] = [
    Phase::E,
    Phase::F,
    Phase::G,
    Phase::H,
    Phase::A,
    Phase::B,
    Phase::C,
    Phase::D,
];

/// The ceres T-cycle (0 to 3) that a netlist phase stands for.
const fn t_cycle(phase: Phase) -> usize {
    match phase {
        Phase::E | Phase::F | Phase::G => 0,
        Phase::H => 1,
        Phase::A | Phase::B => 2,
        Phase::C | Phase::D => 3,
    }
}

#[derive(Clone, Copy)]
enum Access {
    None,
    ReadIf,
    WriteIf(u8),
    /// The interrupt entry's last M-cycle: its push goes to RAM.
    Entry,
}

fn any_access() -> Access {
    match kani::any::<u8>() & 3 {
        0 => Access::None,
        1 => Access::ReadIf,
        2 => Access::WriteIf(kani::any()),
        _ => Access::Entry,
    }
}

/// The lowest interrupt that is requested and enabled, as a bit.
const fn lowest(requested: u8) -> u8 {
    requested & requested.wrapping_neg()
}

/// The netlist's IF, and IE.
struct NetIf {
    flags: u8,
    ie: u8,
}

impl NetIf {
    /// An M-cycle from E to D, with the CPU's access and the sources that
    /// rise in each phase. Returns what a read reads and the interrupt the
    /// entry takes.
    fn m_cycle(&mut self, access: Access, rises: &[u8; 8]) -> (Option<u8>, Option<u8>) {
        // The read's latch, and the requests' latch, close at the start of E.
        let read = matches!(access, Access::ReadIf).then_some(self.flags | !INT_MASK);
        let selected = lowest(self.ie & self.flags);
        let taken = matches!(access, Access::Entry).then_some(selected);
        for (phase, &rise) in PHASES.iter().zip(rises) {
            let write = match access {
                Access::WriteIf(val) if *phase <= Phase::G => Some(val & INT_MASK),
                _ => None,
            };
            let ack = match access {
                Access::Entry if (Phase::G..=Phase::H).contains(phase) => selected,
                _ => 0,
            };
            let mut flags = self.flags;
            // A rise sets the flag, unless something forces it.
            flags |= rise & INT_MASK;
            if let Some(val) = write {
                flags = val;
            }
            flags &= !ack;
            self.flags = flags;
        }
        (read, taken)
    }
}

/// Ceres's M-cycle: the access at the boundary, the requests of each
/// T-cycle, the IF write after the first and the acknowledge after the
/// second.
fn ceres_m_cycle(
    ints: &mut Interrupts,
    access: Access,
    rises: &[u8; 8],
) -> (Option<u8>, Option<u8>) {
    let mut requests = [0; 4];
    for (phase, &rise) in PHASES.iter().zip(rises) {
        requests[t_cycle(*phase)] |= rise & INT_MASK;
    }
    let read = matches!(access, Access::ReadIf).then(|| ints.read_if());
    let queue = ints.read_ie() & INT_MASK & ints.read_if() & INT_MASK;
    let taken = matches!(access, Access::Entry).then_some(lowest(queue));
    for (t, &request) in requests.iter().enumerate() {
        ints.ifr |= request;
        match access {
            Access::WriteIf(val) if t == 0 => ints.write_if(val),
            Access::Entry if t == 1 && queue != 0 => ints.acknowledge_interrupt(lowest(queue)),
            _ => {}
        }
    }
    (read, taken)
}

/// From equal IF and IE, an M-cycle with any access and any rises of the
/// sources gives the same read, the same interrupt taken, and equal IF.
#[kani::proof]
#[kani::unwind(9)]
fn netlist_if_bisimulation() {
    let flags = kani::any::<u8>() & INT_MASK;
    let ie: u8 = kani::any();
    let mut net = NetIf { flags, ie };
    let mut ints = Interrupts::default();
    ints.write_if(flags);
    ints.write_ie(ie);
    let access = any_access();
    let rises: [u8; 8] = kani::any();
    let (net_read, net_taken) = net.m_cycle(access, &rises);
    let (ceres_read, ceres_taken) = ceres_m_cycle(&mut ints, access, &rises);
    assert!(net_read == ceres_read, "the read differs");
    assert!(net_taken == ceres_taken, "the interrupt taken differs");
    assert!(net.flags == ints.ifr, "IF parts");
    kani::cover!(
        matches!(access, Access::WriteIf(_)) && rises[3] != 0 && net.flags & rises[3] != 0,
        "a rise in H survives the write"
    );
    kani::cover!(
        matches!(access, Access::Entry)
            && net_taken != Some(0)
            && net.flags & net_taken.unwrap_or(0) != 0,
        "the source rises again after the acknowledge"
    );
}
