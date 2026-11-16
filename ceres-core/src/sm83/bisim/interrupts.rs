//! The interrupt entry, HALT and the wake from it, against the netlist's
//! core.

use super::*;

/// An interrupt's entry, from where it begins (`relation::entry_from`): ceres's
/// dispatch after its first read, and the fetch of the vector's opcode,
/// against the core's next five M-cycles, with any lines from IF and the
/// pushes anywhere but IF (they may write IE, which can take the request
/// away: then neither acknowledges, and both jump to 0). The accesses and
/// the acknowledges agree, and `boundary` holds with the opcode read.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_interrupt_entry() {
    let mut memory = AnyMemory::new();
    let mut cpu = any_cpu();
    // Where ceres's step chose to dispatch: EI's toggle taken, IME set, PC
    // where the dispatch reads, maybe after the HALT bug.
    cpu.halt_bug = kani::any();
    kani::assume(!cpu.ime_toggle);
    let mut net = Net {
        core: core_entering_any(&cpu),
        low_before: kani::any(),
        confirm: false,
        int: 0,
        int_before: memory.line(Some(0)),
        acks: 0,
        halt_n: true,
        may_halt: false,
    };
    kani::assume(entry_from(&net.core, &cpu));
    memory.ie = net.core.ints.ie;
    kani::assume(settled_in(
        &net.core,
        PHASES - 1,
        kani::any(),
        kani::any(),
        memory.line(Some(0)),
        true,
    ));

    // The dispatch, whose first read was the fetch's M-cycle, and the next
    // step up to its instruction: the fetch at the vector.
    cpu.dispatch_interrupt(&mut memory);
    memory.end_step();
    let begun = cpu.begin(&mut memory);
    assert!(memory.count == 6, "the dispatch takes five M-cycles");
    core_follows(&mut net, &memory, 1..6);
    assert!(
        matches!(begun, Begin::Fetched(next) if boundary(&net.core, &cpu, next)),
        "the vector's opcode begins"
    );
}
/// HALT, from where it was fetched (`relation::boundary`): ceres executes it
/// (its read of the next opcode, whose time it drops) and the next step up
/// to its instruction, in the same M-cycle; the core one M-cycle. Both halt
/// (`relation::in_halt`), or, with a request pending (the HALT bug), the next
/// step's fetch or dispatch shares HALT's M-cycle, PC not moved past it.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_halt() {
    let mut memory = AnyMemory::new();
    let mut cpu = any_cpu();
    let mut net = Net {
        core: core_between_any(&cpu, HALT),
        low_before: kani::any(),
        confirm: false,
        int: 0,
        int_before: memory.line(None),
        acks: 0,
        halt_n: true,
        may_halt: false,
    };
    kani::assume(boundary(&net.core, &cpu, HALT));
    memory.ie = net.core.ints.ie;
    kani::assume(settled_in(
        &net.core,
        PHASES - 1,
        kani::any(),
        kani::any(),
        memory.line(None),
        true,
    ));

    cpu.take_ime_toggle();
    cpu.exec(&mut memory, HALT);
    memory.end_step();
    let begun = cpu.begin(&mut memory);
    if begun == Begin::Dispatch {
        _ = memory.read(cpu.pc);
    }
    assert!(memory.count == 1, "HALT's M-cycle");
    core_follows(&mut net, &memory, 0..1);
    match begun {
        Begin::Halted => {
            assert!(in_halt(&net.core, &cpu), "both halt");
            assert!(
                halted_after(&cpu),
                "ceres halted, past its first halted step"
            );
        }
        Begin::Fetched(next) => assert!(boundary(&net.core, &cpu, next), "the HALT bug's fetch"),
        Begin::Dispatch => assert!(entry_from(&net.core, &cpu), "the HALT bug's dispatch"),
    }
}
/// Ceres halted as the halted harnesses take it: past its first halted
/// step (which shares HALT's M-cycle), EI's toggle taken, no HALT bug.
pub(super) const fn halted_after(cpu: &Sm83) -> bool {
    cpu.is_halted & !cpu.just_halted & !cpu.ime_toggle & !cpu.halt_bug
}
/// A halted M-cycle, from both halted (`relation::in_halt`; ceres past its
/// first halted step, which shares HALT's M-cycle), the core halting
/// (`halted` clear: HALT's M-cycle just ended) or halted: ceres's halted
/// step samples the lines as its time passes, the core runs an M-cycle.
/// Without a request both are still halted; with one, ceres has sampled it
/// and the core latched it (`relation::waking`).
pub(super) fn halted_mcycle(halted: bool) {
    let mut memory = AnyMemory::new();
    let mut cpu = any_cpu();
    // Halted: EI's toggle was taken by the step that fetched HALT.
    cpu.is_halted = true;
    cpu.ime_toggle = false;
    let mut net = Net {
        core: core_in_halt_any(&cpu, halted),
        low_before: kani::any(),
        confirm: false,
        int: 0,
        int_before: memory.line(None),
        acks: 0,
        halt_n: !halted,
        may_halt: true,
    };
    kani::assume(in_halt(&net.core, &cpu));
    memory.ie = net.core.ints.ie;
    kani::assume(settled_in(
        &net.core,
        PHASES - 1,
        kani::any(),
        kani::any(),
        memory.line(None),
        !halted,
    ));

    let requested = cpu.sample(&mut memory);
    core_follows(&mut net, &memory, 0..1);
    if requested {
        assert!(waking(&net.core, &cpu), "a request latched: both wake");
        assert!(halted_after(&cpu), "ceres halted until its choice");
        assert!(
            net.core.ints.pending == memory.line(Some(0)) & memory.ie,
            "the latch holds the lines from the M-cycle's E edge, and IE"
        );
    } else {
        assert!(
            cpu.choose(&mut memory, false) == Begin::Halted,
            "no request: ceres still halted"
        );
        assert!(in_halt(&net.core, &cpu), "both still halted");
        assert!(
            halted_after(&cpu),
            "ceres halted, past its first halted step"
        );
    }
}
/// The halted M-cycle after HALT's, the core halting.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_halted_from_halt() {
    halted_mcycle(false);
}
/// A halted M-cycle, the core halted.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_halted() {
    halted_mcycle(true);
}
/// The wake-up, from where a halted M-cycle ended with a request
/// (`relation::waking`): ceres's step goes on from its sample (EI's toggle,
/// the wake-up, the dispatch or the fetch), the core runs an M-cycle, its
/// halt reset. The next instruction begins, or an interrupt's entry.
// @assumes transcription-sm83
// @assumes sm83-memory-as-the-chip-drives-it
// @assumes sm83-requests-at-e
// @assumes sm83-not-stopped
// @assumes sm83-settles-in-one-pass
#[kani::proof]
#[kani::unwind(9)]
fn netlist_sm83_wake() {
    let mut memory = AnyMemory::new();
    let mut cpu = any_cpu();
    // Halted: EI's toggle was taken by the step that fetched HALT.
    cpu.is_halted = true;
    cpu.ime_toggle = false;
    let mut core = core_in_halt_any(&cpu, true);
    core.ints.pending = kani::any();
    core.sampled.int_pending = true;
    let mut net = Net {
        core,
        low_before: kani::any(),
        confirm: false,
        int: 0,
        int_before: memory.line(None),
        acks: 0,
        halt_n: false,
        may_halt: true,
    };
    kani::assume(waking(&net.core, &cpu));
    memory.ie = net.core.ints.ie;
    // The latch, open the whole halted M-cycle, took the lines as they were
    // from its E edge on, which ceres's sample found a request in.
    kani::assume(net.core.ints.pending == memory.line(None) & memory.ie);
    kani::assume(settled_in(
        &net.core,
        PHASES - 1,
        kani::any(),
        kani::any(),
        memory.line(None),
        false,
    ));

    let begun = cpu.choose(&mut memory, true);
    if begun == Begin::Dispatch {
        _ = memory.read(cpu.pc);
    }
    assert!(memory.count == 1, "the wake-up's M-cycle");
    core_follows(&mut net, &memory, 0..1);
    match begun {
        Begin::Fetched(next) => assert!(
            boundary(&net.core, &cpu, next),
            "woken, the next instruction begins"
        ),
        Begin::Dispatch => assert!(
            entry_from(&net.core, &cpu),
            "woken, an interrupt's entry begins"
        ),
        Begin::Halted => panic!("a request wakes ceres"),
    }
}
