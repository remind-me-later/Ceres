//! The replay of the CPU core (`NetCore`) through the traces that have its
//! fields: the 14 after the bus side's (see `BLOCKS` in
//! `hdl/board/gameboy_tb.vhd`). The core takes the chip's data bus, IF,
//! the clocks and the resets from the trace, and its state and outputs are
//! compared with the model's at the end of every phase.

use {
    super::{Line, Report, bit, hex8},
    ceres_netlist::latch_race::LatchRace,
    ceres_netlist::sm83::{
        CoreInputs, CoreOutputs, Flags, HighByte, NetAlu, NetCore, NetCoreInterrupts, NetRegisters,
        NetSequencer, Register,
    },
};

/// The fields of a line that has the core's.
pub const CORE_LINE_FIELDS: usize = 105;

/// The core in a line of a trace: its state (the masters of the registers
/// and of the flags are those the trace shows: they differ only in G and
/// H, while a register is written).
pub struct CoreLine {
    pub net: NetCore,
    /// The bits of the chip's data bus that were a strong 0 in the phase:
    /// only those pull the core's bus down (not the keeper's weak 0, nor
    /// an X where drivers fight, which the data bus's field shows as 0).
    pub data_low: u8,
}

fn hex16(s: &str) -> Option<u16> {
    u16::from_str_radix(s, 16).ok()
}

/// The core's fields, after the bus side's; IE is among the earlier ones.
pub fn parse(fields: &[&str], ie: u8) -> Option<CoreLine> {
    let &[
        bits,
        opcode,
        ir_next,
        mcyc,
        af,
        bc,
        de,
        hl,
        wz,
        sp,
        pc,
        pending,
        db,
        data_low,
    ] = fields
    else {
        return None;
    };
    let b = |i| bit(bits, i);
    let seq = NetSequencer {
        opcode: hex8(opcode)?,
        ir_next: hex8(ir_next)?,
        mcyc: hex8(mcyc)?,
        sel_reg_w: b(13)?,
        cb_next: b(14)?,
        cb_flag: b(15)?,
        halt: b(0)?,
        stop: b(1)?,
        halt_delayed: b(2)?,
        stop_delayed: b(3)?,
        pending_delayed: b(4)?,
        not_ready_1: b(5)?,
        not_ready_2: b(6)?,
        ime_state: b(7)?,
        ime: b(8)?,
        no_nmi: b(9)?,
        no_nmi_1: b(10)?,
        entering: b(11)?,
        int_entry: b(12)?,
        ..NetSequencer::POWER_ON
    };
    let pair = |s: &str| hex16(s).map(u16::to_be_bytes);
    let [a, f] = pair(af)?;
    let [rb, rc] = pair(bc)?;
    let [rd, re] = pair(de)?;
    let [rh, rl] = pair(hl)?;
    let [rw, rz] = pair(wz)?;
    let [sph, spl] = pair(sp)?;
    let [pch, pcl] = pair(pc)?;
    let flags = Flags::from_bits(f);
    Some(CoreLine {
        net: NetCore {
            seq,
            regs: NetRegisters {
                a: Register::new(a),
                b: Register::new(rb),
                c: Register::new(rc),
                d: Register::new(rd),
                e: Register::new(re),
                h: Register::new(rh),
                l: Register::new(rl),
                w: Register::new(rw),
                z: Register::new(rz),
                sph: Register::new(sph),
                spl: Register::new(spl),
                pch: Register::new(pch),
                pcl: Register::new(pcl),
            },
            alu: NetAlu {
                flags,
                masters: flags,
                cond_fail: b(16)?,
                z7_master: b(17)?,
                z7_delayed: b(18)?,
            },
            ints: NetCoreInterrupts {
                ie_next: ie,
                ie,
                pending: hex8(pending)?,
            },
            db: hex8(db)?,
            ..NetCore::POWER_ON
        },
        data_low: hex8(data_low)?,
    })
}

/// What the core takes in during `line`'s phase: the clocks from the phase
/// and the core's own enables (as the clocks take them), the resets, IF,
/// the chip's data bus where it was a strong 0, the chip's decoding of the
/// address.
pub fn inputs(line: &Line) -> Option<CoreInputs> {
    let (clocks, bus) = (line.clocks.as_ref()?, line.bus.as_ref()?);
    let pins = clocks.inputs;
    let phase = line.phase;
    // @hw die: BELE BYJU BALY BOGA; BUVU BYXO BEDO; BELU BYRY BUDE BEVA;
    // BAPY BERU BUFA BYLY; BATE BASU BUKE
    let clk = phase == 'a' || !pins.osc_ena;
    Some(CoreInputs {
        clk,
        exec_phase: pins.clk_ena && !clk,
        data_phase: pins.clk_ena && matches!(phase, 'e' | 'f' | 'g' | 'h'),
        write_phase: pins.clk_ena && matches!(phase, 'g' | 'h'),
        pch_phase: if matches!(phase, 'g' | 'h' | 'a') {
            !pins.clk_ena
        } else {
            true
        },
        // The trace shows cpu_sys_reset as the phase had it.
        sys_reset: clocks.net.cpu_sys_reset,
        pwron_reset: !pins.reset_pin_n,
        clk_ready: clocks.outputs.osc_stable,
        wake: bus.net.jp_wakeup,
        oe_n: pins.t1 && !pins.t2,
        // @hw die: TUNA SYRO, TULO TUTU
        internal_access: line.addr >> 9 == 0x7F,
        shadow_access: bus.boot_sel,
        shadow_override: pins.t2 && !pins.t1,
        int: line.flags,
        d: !line.core.as_ref()?.data_low,
    })
}

/// The flip-flops, latches and registers of `net` that differ from
/// `model`'s.
pub fn differences(net: &NetCore, model: &NetCore) -> Vec<&'static str> {
    let (s, m) = (&net.seq, &model.seq);
    let (r, q) = (&net.regs, &model.regs);
    let mut differ = Vec::new();
    for (name, differs) in [
        ("opcode", s.opcode != m.opcode),
        ("ir_next", s.ir_next != m.ir_next),
        ("mcyc", s.mcyc != m.mcyc),
        ("sel_reg_w", s.sel_reg_w != m.sel_reg_w),
        ("cb_next", s.cb_next != m.cb_next),
        ("cb_flag", s.cb_flag != m.cb_flag),
        ("halt", s.halt != m.halt),
        ("stop", s.stop != m.stop),
        ("halt_delayed", s.halt_delayed != m.halt_delayed),
        ("stop_delayed", s.stop_delayed != m.stop_delayed),
        ("pending_delayed", s.pending_delayed != m.pending_delayed),
        ("not_ready_1", s.not_ready_1 != m.not_ready_1),
        ("not_ready_2", s.not_ready_2 != m.not_ready_2),
        ("ime_state", s.ime_state != m.ime_state),
        ("ime", s.ime != m.ime),
        ("no_nmi", s.no_nmi != m.no_nmi),
        ("no_nmi_1", s.no_nmi_1 != m.no_nmi_1),
        ("entering", s.entering != m.entering),
        ("int_entry", s.int_entry != m.int_entry),
        ("a", r.a.q != q.a.q),
        ("f", net.alu.flags != model.alu.flags),
        ("b", r.b.q != q.b.q),
        ("c", r.c.q != q.c.q),
        ("d", r.d.q != q.d.q),
        ("e", r.e.q != q.e.q),
        ("h", r.h.q != q.h.q),
        ("l", r.l.q != q.l.q),
        ("w", r.w.q != q.w.q),
        ("z", r.z.q != q.z.q),
        ("sp", (r.sph.q, r.spl.q) != (q.sph.q, q.spl.q)),
        ("pc", (r.pch.q, r.pcl.q) != (q.pch.q, q.pcl.q)),
        ("cond_fail", net.alu.cond_fail != model.alu.cond_fail),
        ("z7_master", net.alu.z7_master != model.alu.z7_master),
        ("z7_delayed", net.alu.z7_delayed != model.alu.z7_delayed),
        ("ie", net.ints.ie != model.ints.ie),
        ("pending", net.ints.pending != model.ints.pending),
        ("db", net.db != model.db),
    ] {
        if differs {
            differ.push(name);
        }
    }
    differ
}

/// Takes the model's state: the registers' and the flags' masters too,
/// but in G and H, where they may hold a new value the trace does not
/// show, ours are kept.
pub fn resync(net: &mut NetCore, model: &NetCore, phase: char) {
    let mut taken = NetCore {
        clk: net.clk,
        exec_phase: net.exec_phase,
        fetch: net.fetch,
        sampled: net.sampled,
        ..*model
    };
    if matches!(phase, 'g' | 'h') {
        let (ours, theirs) = (&net.regs, &mut taken.regs);
        for (our, their) in [
            (ours.a, &mut theirs.a),
            (ours.b, &mut theirs.b),
            (ours.c, &mut theirs.c),
            (ours.d, &mut theirs.d),
            (ours.e, &mut theirs.e),
            (ours.h, &mut theirs.h),
            (ours.l, &mut theirs.l),
            (ours.w, &mut theirs.w),
            (ours.z, &mut theirs.z),
            (ours.sph, &mut theirs.sph),
            (ours.spl, &mut theirs.spl),
            (ours.pch, &mut theirs.pch),
            (ours.pcl, &mut theirs.pcl),
        ] {
            their.master = our.master;
        }
        taken.alu.masters = net.alu.masters;
        taken.ints.ie_next = net.ints.ie_next;
    }
    *net = taken;
}

/// The core's outputs that differ from the model's, as `line` shows the
/// chip's address, request, read, write and acknowledges, and the clocks'
/// enables.
pub fn output_differences(out: &CoreOutputs, line: &Line) -> Vec<&'static str> {
    let (Some(bus), Some(clocks)) = (line.bus.as_ref(), line.clocks.as_ref()) else {
        return Vec::new();
    };
    let mut differ = Vec::new();
    for (name, differs) in [
        ("addr", out.addr != line.addr),
        ("mreq", out.mreq != bus.mreq),
        ("rd", out.rd != bus.read),
        ("wr", out.wr != bus.write),
        ("acks", out.inta & 0x1F != line.acks),
        ("halt_n", out.halt_n != clocks.inputs.clk_ena),
        ("stop_n", out.stop_n != clocks.inputs.osc_ena),
    ] {
        if differs {
            differ.push(name);
        }
    }
    differ
}

/// The race of the core's request against the chip's decodings
/// (`LatchRace`) in `line`'s phase, from the core's outputs as the phase
/// before ended (`before`) and as this one ends (`out`), against the
/// latches' opening the trace shows.
fn check_race(
    line: &Line,
    unmapped_before: Option<bool>,
    before: &CoreOutputs,
    out: &CoreOutputs,
    place: &str,
    report: &mut Report,
) {
    let (Some(bus), Some(clocks)) = (line.bus.as_ref(), line.clocks.as_ref()) else {
        return;
    };
    let pins = clocks.inputs;
    let race = LatchRace {
        addr_before: before.addr,
        addr: out.addr,
        high_byte: out.high_byte,
        ctl_mreq_before: before.ctl_mreq,
        ctl_mreq: out.ctl_mreq,
        boot_mapped: !unmapped_before.unwrap_or(bus.net.unmapped),
        t2_only: pins.t2 && !pins.t1,
    };
    let ours = race.glitch().map(|a| a & 0x7FFF);
    let theirs = super::bus::latch_glitch(line);
    let vram = out.addr >> 13 == 0b100;
    let starts = out.ctl_mreq && !before.ctl_mreq;
    let seen = if ours.is_some() {
        Some(if line.phase == 'a' {
            "core: the address latches close as an access ends"
        } else if vram {
            "core: the address latches open for a moment: VRAM, through HL or SP"
        } else if out.addr >= 0xFE00 {
            "core: the address latches open for a moment: FE00 to FFFF"
        } else if out.addr >> 8 == 0 {
            "core: the address latches open for a moment: the boot ROM"
        } else {
            "core: the address latches open for a moment: elsewhere"
        })
    } else if starts && vram {
        match out.high_byte {
            HighByte::Register => {
                Some("core: a VRAM access through BC, DE or PC: the pulse is dropped")
            }
            HighByte::W => Some("core: a VRAM access through W: no pulse"),
            _ => None,
        }
    } else {
        None
    };
    if let Some(what) = seen {
        report.saw(what);
    }
    if ours != theirs {
        report.differ(
            format!("core: the address latches' glitch, phase {}", line.phase),
            format!("{place} ({ours:04X?}, model {theirs:04X?}; {race:04X?})"),
        );
    }
}

/// Replays the core, phase by phase, from the trace's first line, and
/// checks the race of its request against the chip's decodings
/// (`LatchRace`) with the latches' openings the trace shows.
pub fn replay(name: &str, lines: &[Line], report: &mut Report) {
    let Some(first) = lines.first() else {
        return;
    };
    let (Some(start), Some(first_inputs)) = (first.core.as_ref(), inputs(first)) else {
        return;
    };
    let mut net = start.net;
    let mut out_before = net.settle(&first_inputs);
    for (n, line) in lines.iter().enumerate().skip(1) {
        let (Some(model), Some(i)) = (line.core.as_ref(), inputs(line)) else {
            return;
        };
        let before = net;
        let out = net.phase(&i);
        report.phases += 1;
        let unmapped_before = lines[n - 1].bus.as_ref().map(|b| b.net.unmapped);
        check_race(
            line,
            unmapped_before,
            &out_before,
            &out,
            &format!("{name}:{}", n + 1),
            report,
        );
        out_before = out;
        for (what, happened) in [
            (
                "core: an interrupt's entry",
                out.inta != 0 && line.phase == 'h',
            ),
            (
                "core: an entry chooses between requests",
                out.inta != 0 && line.phase == 'h' && net.ints.pending.count_ones() > 1,
            ),
            ("core: HALT starts", net.seq.halt && !before.seq.halt),
            ("core: HALT ends", !net.seq.halt && before.seq.halt),
            ("core: STOP starts", net.seq.stop && !before.seq.stop),
            (
                "core: a condition fails",
                net.alu.cond_fail && !before.alu.cond_fail,
            ),
            (
                "core: an opcode after CB",
                net.seq.table_cb() && line.phase == 'b',
            ),
            (
                "core: IE is written",
                out.wr && out.addr == 0xFFFF && line.phase == 'h',
            ),
            ("core: an opcode is fetched", out.m1 && line.phase == 'h'),
        ] {
            if happened {
                report.saw(what);
            }
        }
        let line_no = n + 1;
        let state = differences(&net, &model.net);
        if !state.is_empty() {
            report.differ(
                format!("core, phase {}: {}", line.phase, state.join(" ")),
                format!(
                    "{name}:{line_no} (opcode {:02X} m{})",
                    model.net.seq.opcode, model.net.seq.mcyc
                ),
            );
            resync(&mut net, &model.net, line.phase);
        }
        let outputs = output_differences(&out, line);
        if !outputs.is_empty() {
            report.differ(
                format!("core outputs, phase {}: {}", line.phase, outputs.join(" ")),
                format!(
                    "{name}:{line_no} (opcode {:02X} m{}: {out:04X?}, model addr {:04X})",
                    model.net.seq.opcode, model.net.seq.mcyc, line.addr
                ),
            );
        }
    }
}
