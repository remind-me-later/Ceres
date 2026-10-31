//! The replay of the APU (`NetApu`) through the traces that have its
//! fields: the 45 after the core's (see `BLOCKS` in
//! `hdl/board/gameboy_tb.vhd`). The APU takes the clocks, the resets, the
//! divider and the CPU's access from the trace; its flip-flops and latches
//! are compared with the model's at the end of every phase.

use {
    super::{Line, Report, bit, hex8},
    ceres_netlist::apu::{
        ApuInputs, Ff, NetApu, NetApuControl, NetNoise, NetSquare, NetSweep, NetWave,
    },
};

/// The fields of a line that has the APU's.
pub const APU_LINE_FIELDS: usize = 150;

/// The APU in a line of a trace: its control, channel 1 (its square part
/// and its sweep) and channel 2.
pub struct ApuLine {
    pub control: NetApuControl,
    pub ch1: NetSquare,
    pub sweep: NetSweep,
    pub ch2: NetSquare,
    pub ch3: NetWave,
    pub ch4: NetNoise,
}

fn hex16(s: &str) -> Option<u16> {
    u16::from_str_radix(s, 16).ok()
}

/// The APU's fields, after the core's: the control's 3, channel 1's 15,
/// channel 2's 9, channel 3's 9, channel 4's 9.
pub fn parse(fields: &[&str]) -> Option<ApuLine> {
    let &[control_bits, nr50, nr51, ref rest @ ..] = fields else {
        return None;
    };
    let (ch1, ch2, ch4) = (rest.get(..15)?, rest.get(15..24)?, rest.get(33..42)?);
    let ch3 = rest.get(24..33)?;
    let b = |i| bit(control_bits, i);
    let ff = |i| b(i).map(Ff::new);
    let control = NetApuControl {
        power: ff(0)?,
        test: b(1)?,
        test_bit5: ff(2)?,
        baza: ff(3)?,
        nr50: hex8(nr50)?,
        nr51: hex8(nr51)?,
        ajer: ff(4)?,
        calo: ff(5)?,
        atep: ff(6)?,
        cemo: ff(7)?,
        cery: ff(8)?,
        atyk: ff(9)?,
        avok: ff(10)?,
        jeso: ff(11)?,
        bara: ff(12)?,
        caru: ff(13)?,
        bylu: ff(14)?,
        ..NetApuControl::default()
    };
    // Channel 1's fields: its bits (the square part's, then the sweep's
    // sweep_step, shifting, shift_done), NR10, the duty, NR12, the
    // frequency, the counter, the step, the length, the period, the
    // volume, the sweep's count, its shifts, the shifted frequency, the
    // augend, the addend.
    let &[
        bits,
        sweep,
        duty,
        envelope,
        acc,
        counter,
        step,
        length,
        period,
        volume,
        sweep_count,
        shifts,
        shifted,
        augend,
        addend,
    ] = ch1
    else {
        return None;
    };
    let acc = hex16(acc)?.to_le_bytes();
    let sweep = NetSweep {
        sweep: hex8(sweep)?,
        acc_low: Ff::new(acc[0]),
        acc_high: Ff::new(acc[1]),
        sweep_count: Ff::new(hex8(sweep_count)?),
        sweep_step: Ff::new(bit(bits, 16)?),
        shifting: bit(bits, 17)?,
        shifts: Ff::new(hex8(shifts)?),
        shift_done: Ff::new(bit(bits, 18)?),
        shifted: Ff::new(hex16(shifted)?),
        augend: Ff::new(hex16(augend)?),
        addend: Ff::new(hex16(addend)?),
        ..NetSweep::default()
    };
    Some(ApuLine {
        control,
        // Channel 1's frequency is the sweep's: its square part keeps none.
        ch1: parse_square(&[
            bits, duty, envelope, "0", counter, step, length, period, volume,
        ])?,
        sweep,
        ch2: parse_square(ch2)?,
        ch3: parse_wave(ch3)?,
        ch4: parse_noise(ch4)?,
    })
}

/// Channel 3's fields: its bits, NR32's level, the frequency, the counter,
/// the position, the length, the bit lines, the sample's byte, the RAM.
fn parse_wave(fields: &[&str]) -> Option<NetWave> {
    let &[
        bits,
        level,
        frequency,
        counter,
        position,
        length,
        bit_lines,
        sample_byte,
        ram,
    ] = fields
    else {
        return None;
    };
    let b = |i| bit(bits, i);
    let ff = |i| b(i).map(Ff::new);
    let mut cells = [0; 16];
    for (cell, byte) in cells.iter_mut().enumerate() {
        *byte = hex8(ram.get(2 * cell..2 * cell + 2)?)?;
    }
    Some(NetWave {
        dac_on: b(0)?,
        length_on: b(1)?,
        trigger: b(2)?,
        start: ff(3)?,
        triggered: b(4)?,
        restart: ff(5)?,
        restarted: ff(6)?,
        running: ff(7)?,
        frozen: b(8)?,
        playing: b(9)?,
        active: ff(10)?,
        reload: ff(11)?,
        wrapped: ff(12)?,
        expired: ff(13)?,
        busa: ff(14)?,
        bano: ff(15)?,
        fetch: ff(16)?,
        azet: ff(17)?,
        level: hex8(level)?,
        frequency: hex16(frequency)?,
        counter: Ff::new(hex16(counter)?),
        position: Ff::new(hex8(position)?),
        length: Ff::new(hex8(length)?),
        bit_lines: hex8(bit_lines)?,
        sample_byte: hex8(sample_byte)?,
        ram: cells,
        ..NetWave::default()
    })
}

/// Channel 3's flip-flops, latches and RAM that differ from the model's.
fn wave_differences(net: &NetWave, model: &NetWave) -> Vec<&'static str> {
    let mut differ = Vec::new();
    for (name, differs) in [
        ("dac_on", net.dac_on != model.dac_on),
        ("level", net.level != model.level),
        ("frequency", net.frequency != model.frequency),
        ("length_on", net.length_on != model.length_on),
        ("trigger", net.trigger != model.trigger),
        ("start", net.start.q != model.start.q),
        ("triggered", net.triggered != model.triggered),
        ("restart", net.restart.q != model.restart.q),
        ("restarted", net.restarted.q != model.restarted.q),
        ("running", net.running.q != model.running.q),
        ("frozen", net.frozen != model.frozen),
        ("playing", net.playing != model.playing),
        ("active", net.active.q != model.active.q),
        ("counter", net.counter.q != model.counter.q),
        ("reload", net.reload.q != model.reload.q),
        ("position", net.position.q != model.position.q),
        ("wrapped", net.wrapped.q != model.wrapped.q),
        ("length", net.length.q != model.length.q),
        ("expired", net.expired.q != model.expired.q),
        ("busa", net.busa.q != model.busa.q),
        ("bano", net.bano.q != model.bano.q),
        ("fetch", net.fetch.q != model.fetch.q),
        ("azet", net.azet.q != model.azet.q),
        ("bit_lines", net.bit_lines != model.bit_lines),
        ("sample_byte", net.sample_byte != model.sample_byte),
        ("ram", net.ram != model.ram),
    ] {
        if differs {
            differ.push(name);
        }
    }
    differ
}

/// Channel 4's fields: its bits, NR42, NR43, the divider, the prescaler,
/// the LFSR, the length, the period, the volume.
fn parse_noise(fields: &[&str]) -> Option<NetNoise> {
    let &[
        bits,
        envelope,
        noise,
        divider,
        prescaler,
        lfsr,
        length,
        period,
        volume,
    ] = fields
    else {
        return None;
    };
    let b = |i| bit(bits, i);
    let ff = |i| b(i).map(Ff::new);
    let volume = hex8(volume)?;
    Some(NetNoise {
        envelope: hex8(envelope)?,
        noise: hex8(noise)?,
        length_on: b(0)?,
        trigger: b(1)?,
        start: ff(2)?,
        triggered: b(3)?,
        restart: ff(4)?,
        restarted: ff(5)?,
        running: ff(6)?,
        frozen: b(7)?,
        active: b(8)?,
        divided: ff(9)?,
        joto: ff(10)?,
        expired: ff(11)?,
        hz64: ff(12)?,
        tick: ff(13)?,
        at_limit: ff(14)?,
        env_stopped: b(15)?,
        divider: Ff::new(hex8(divider)?),
        prescaler: Ff::new(hex16(prescaler)?),
        lfsr: Ff::new(hex16(lfsr)?),
        length: Ff::new(hex8(length)?),
        period: Ff::new(hex8(period)?),
        volume: [
            Ff::new(volume & 1 != 0),
            Ff::new(volume & 2 != 0),
            Ff::new(volume & 4 != 0),
            Ff::new(volume & 8 != 0),
        ],
        ..NetNoise::default()
    })
}

/// A square channel's fields (channel 2's; channel 1's begin alike).
fn parse_square(fields: &[&str]) -> Option<NetSquare> {
    let &[
        bits,
        duty,
        envelope,
        frequency,
        counter,
        step,
        length,
        period,
        volume,
    ] = fields
    else {
        return None;
    };
    let b = |i| bit(bits, i);
    let ff = |i| b(i).map(Ff::new);
    let volume = hex8(volume)?;
    Some(NetSquare {
        duty: hex8(duty)?,
        envelope: hex8(envelope)?,
        frequency: hex16(frequency)?,
        length_on: b(0)?,
        trigger: b(1)?,
        start: ff(2)?,
        triggered: b(3)?,
        restart: ff(4)?,
        restarted: ff(5)?,
        running: ff(6)?,
        frozen: b(7)?,
        active: b(8)?,
        reload: ff(9)?,
        wave: ff(10)?,
        expired: ff(11)?,
        hz64: ff(12)?,
        tick: ff(13)?,
        at_limit: ff(14)?,
        env_stopped: b(15)?,
        counter: Ff::new(hex16(counter)?),
        step: Ff::new(hex8(step)?),
        length: Ff::new(hex8(length)?),
        period: Ff::new(hex8(period)?),
        volume: [
            Ff::new(volume & 1 != 0),
            Ff::new(volume & 2 != 0),
            Ff::new(volume & 4 != 0),
            Ff::new(volume & 8 != 0),
        ],
        ..NetSquare::default()
    })
}

/// What the APU takes in after the edge that ends `edge`'s phase, in the
/// phase `line` ends: the clock as the edge leaves it, the clocks decoded
/// from the phase, the resets, the divider's bit 10, the CPU's access.
pub fn inputs(edge: &Line, line: &Line) -> Option<ApuInputs> {
    let clk = edge.lcd.as_ref()?.clk;
    let clocks = line.clocks.as_ref()?;
    let pins = clocks.inputs;
    let phase = line.phase;
    // @hw die: BELU BYRY BUDE BEVA; BAPY BERU BUFA BYLY; BATE BASU BUKE
    Some(ApuInputs {
        clk,
        data_phase: pins.clk_ena && matches!(phase, 'e' | 'f' | 'g' | 'h'),
        clk_t4: pins.clk_ena && matches!(phase, 'g' | 'h'),
        pch_phase: if matches!(phase, 'g' | 'h' | 'a') {
            !pins.clk_ena
        } else {
            true
        },
        reset2: clocks.net.reset2(),
        div10: line.div & 0x400 != 0,
        t1_only: pins.t1 && !pins.t2,
        addr: line.addr,
        data: line.data,
        cpu_rd: line.rd,
        cpu_wr: line.wr,
    })
}

/// The control's flip-flops and latches that differ from the model's.
fn control_differences(net: &NetApuControl, model: &NetApuControl) -> Vec<&'static str> {
    let mut differ = Vec::new();
    for (name, differs) in [
        ("power", net.power.q != model.power.q),
        ("test", net.test != model.test),
        ("test_bit5", net.test_bit5.q != model.test_bit5.q),
        ("baza", net.baza.q != model.baza.q),
        ("nr50", net.nr50 != model.nr50),
        ("nr51", net.nr51 != model.nr51),
        ("ajer", net.ajer.q != model.ajer.q),
        ("calo", net.calo.q != model.calo.q),
        ("atep", net.atep.q != model.atep.q),
        ("cemo", net.cemo.q != model.cemo.q),
        ("cery", net.cery.q != model.cery.q),
        ("atyk", net.atyk.q != model.atyk.q),
        ("avok", net.avok.q != model.avok.q),
        ("jeso", net.jeso.q != model.jeso.q),
        ("bara", net.bara.q != model.bara.q),
        ("caru", net.caru.q != model.caru.q),
        ("bylu", net.bylu.q != model.bylu.q),
    ] {
        if differs {
            differ.push(name);
        }
    }
    differ
}

/// A square channel's flip-flops and latches that differ from the model's.
fn square_differences(net: &NetSquare, model: &NetSquare) -> Vec<&'static str> {
    let mut differ = Vec::new();
    for (name, differs) in [
        ("duty", net.duty != model.duty),
        ("envelope", net.envelope != model.envelope),
        ("frequency", net.frequency != model.frequency),
        ("length_on", net.length_on != model.length_on),
        ("trigger", net.trigger != model.trigger),
        ("start", net.start.q != model.start.q),
        ("triggered", net.triggered != model.triggered),
        ("restart", net.restart.q != model.restart.q),
        ("restarted", net.restarted.q != model.restarted.q),
        ("running", net.running.q != model.running.q),
        ("frozen", net.frozen != model.frozen),
        ("active", net.active != model.active),
        ("counter", net.counter.q != model.counter.q),
        ("reload", net.reload.q != model.reload.q),
        ("step", net.step.q != model.step.q),
        ("wave", net.wave.q != model.wave.q),
        ("length", net.length.q != model.length.q),
        ("expired", net.expired.q != model.expired.q),
        ("hz64", net.hz64.q != model.hz64.q),
        ("period", net.period.q != model.period.q),
        ("tick", net.tick.q != model.tick.q),
        ("volume", net.volume() != model.volume()),
        ("at_limit", net.at_limit.q != model.at_limit.q),
        ("env_stopped", net.env_stopped != model.env_stopped),
    ] {
        if differs {
            differ.push(name);
        }
    }
    differ
}

/// The sweep's flip-flops and latches that differ from the model's.
fn sweep_differences(net: &NetSweep, model: &NetSweep) -> Vec<&'static str> {
    let mut differ = Vec::new();
    for (name, differs) in [
        ("sweep", net.sweep != model.sweep),
        ("acc", net.acc() != model.acc()),
        ("sweep_count", net.sweep_count.q != model.sweep_count.q),
        ("sweep_step", net.sweep_step.q != model.sweep_step.q),
        ("shifting", net.shifting != model.shifting),
        ("shifts", net.shifts.q != model.shifts.q),
        ("shift_done", net.shift_done.q != model.shift_done.q),
        ("shifted", net.shifted.q != model.shifted.q),
        ("augend", net.augend.q != model.augend.q),
        ("addend", net.addend.q != model.addend.q),
    ] {
        if differs {
            differ.push(name);
        }
    }
    differ
}

/// Channel 4's flip-flops and latches that differ from the model's.
fn noise_differences(net: &NetNoise, model: &NetNoise) -> Vec<&'static str> {
    let mut differ = Vec::new();
    for (name, differs) in [
        ("envelope", net.envelope != model.envelope),
        ("noise", net.noise != model.noise),
        ("length_on", net.length_on != model.length_on),
        ("trigger", net.trigger != model.trigger),
        ("start", net.start.q != model.start.q),
        ("triggered", net.triggered != model.triggered),
        ("restart", net.restart.q != model.restart.q),
        ("restarted", net.restarted.q != model.restarted.q),
        ("running", net.running.q != model.running.q),
        ("frozen", net.frozen != model.frozen),
        ("active", net.active != model.active),
        ("divider", net.divider.q != model.divider.q),
        ("divided", net.divided.q != model.divided.q),
        ("prescaler", net.prescaler.q != model.prescaler.q),
        ("joto", net.joto.q != model.joto.q),
        ("lfsr", net.lfsr.q != model.lfsr.q),
        ("length", net.length.q != model.length.q),
        ("expired", net.expired.q != model.expired.q),
        ("hz64", net.hz64.q != model.hz64.q),
        ("period", net.period.q != model.period.q),
        ("tick", net.tick.q != model.tick.q),
        ("volume", net.volume() != model.volume()),
        ("at_limit", net.at_limit.q != model.at_limit.q),
        ("env_stopped", net.env_stopped != model.env_stopped),
    ] {
        if differs {
            differ.push(name);
        }
    }
    differ
}

/// The APU in the model's state (`model`), its wires settled with
/// `inputs`.
pub fn start_from(model: &ApuLine, inputs: ApuInputs) -> NetApu {
    let mut net = NetApu {
        inputs,
        control: model.control,
        ch1: model.ch1,
        sweep: model.sweep,
        ch2: model.ch2,
        ch3: model.ch3,
        ch4: model.ch4,
    };
    net.settle_wires();
    net
}

/// Replays the APU, edge by edge, from the trace's first line.
#[expect(clippy::too_many_lines, reason = "a block at a time")]
pub fn replay(name: &str, lines: &[Line], report: &mut Report) {
    let Some(first) = lines.first() else {
        return;
    };
    let (Some(start), Some(at_start)) = (first.apu.as_ref(), inputs(first, first)) else {
        return;
    };
    let mut net = start_from(start, at_start);
    for n in 0..lines.len() - 1 {
        let (line, ends) = (&lines[n], &lines[n + 1]);
        let (Some(model), Some(after)) = (ends.apu.as_ref(), inputs(line, ends)) else {
            return;
        };
        let before = net;
        net.edge(after);
        report.phases += 1;
        for (what, happened) in [
            (
                "apu: the power turns on",
                !before.control.power.q && net.control.power.q,
            ),
            (
                "apu: the power turns off",
                before.control.power.q && !net.control.power.q,
            ),
            (
                "apu: the frame sequencer steps (512 Hz)",
                !before.control.bara.q && net.control.bara.q,
            ),
            (
                "apu: channel 1 restarts",
                !before.ch1.restart.q && net.ch1.restart.q,
            ),
            (
                "apu: channel 1's sweep steps",
                !before.sweep.sweep_step.q && net.sweep.sweep_step.q,
            ),
            (
                "apu: channel 1's sweep updates",
                !before.sweep.wires.update && net.sweep.wires.update,
            ),
            (
                "apu: channel 1's sweep overflows",
                !before.sweep.wires.overflow && net.sweep.wires.overflow,
            ),
            (
                "apu: channel 3 restarts",
                !before.ch3.restart.q && net.ch3.restart.q,
            ),
            (
                "apu: channel 3 reads a sample",
                !before.ch3.fetch.q && net.ch3.fetch.q,
            ),
            (
                "apu: the CPU writes wave RAM",
                net.ch3.ram != before.ch3.ram,
            ),
            (
                "apu: a channel's length expires",
                (!before.ch1.expired.q && net.ch1.expired.q)
                    || (!before.ch3.expired.q && net.ch3.expired.q)
                    || (!before.ch4.expired.q && net.ch4.expired.q),
            ),
            (
                "apu: an envelope reaches its limit",
                (!before.ch1.at_limit.q && net.ch1.at_limit.q)
                    || (!before.ch2.at_limit.q && net.ch2.at_limit.q)
                    || (!before.ch4.at_limit.q && net.ch4.at_limit.q),
            ),
            (
                "apu: channel 4 shifts its short LFSR",
                net.ch4.noise & 8 != 0 && before.ch4.lfsr.q != net.ch4.lfsr.q,
            ),
            (
                "apu: channel 4 restarts",
                !before.ch4.restart.q && net.ch4.restart.q,
            ),
            (
                "apu: channel 4's LFSR shifts",
                before.ch4.lfsr.q != net.ch4.lfsr.q,
            ),
            (
                "apu: channel 2 restarts",
                !before.ch2.restart.q && net.ch2.restart.q,
            ),
            (
                "apu: channel 2's duty steps",
                before.ch2.step.q != net.ch2.step.q,
            ),
            (
                "apu: channel 2's envelope ticks",
                !before.ch2.tick.q && net.ch2.tick.q,
            ),
            (
                "apu: channel 2's length expires",
                !before.ch2.expired.q && net.ch2.expired.q,
            ),
        ] {
            if happened {
                report.saw(what);
            }
        }
        compare(
            &mut net,
            model,
            "apu",
            ends.phase,
            &format!("{name}:{}", n + 2),
            report,
        );
    }
}

/// `net` against the model's APU (`model`) at the end of a phase: reports
/// the blocks that differ, as `who` at `place`, and then takes the model's
/// state.
pub fn compare(
    net: &mut NetApu,
    model: &ApuLine,
    who: &str,
    phase: char,
    place: &str,
    report: &mut Report,
) {
    let differ = [
        ("control", control_differences(&net.control, &model.control)),
        ("channel 1", square_differences(&net.ch1, &model.ch1)),
        (
            "channel 1's sweep",
            sweep_differences(&net.sweep, &model.sweep),
        ),
        ("channel 2", square_differences(&net.ch2, &model.ch2)),
        ("channel 3", wave_differences(&net.ch3, &model.ch3)),
        ("channel 4", noise_differences(&net.ch4, &model.ch4)),
    ];
    let mut any = false;
    for (block, fields) in &differ {
        if !fields.is_empty() {
            any = true;
            report.differ(
                format!("{who}: {block}, phase {phase}: {}", fields.join(" ")),
                place.to_owned(),
            );
        }
    }
    if any {
        net.control = NetApuControl {
            wires: net.control.wires,
            ..model.control
        };
        net.ch1 = NetSquare {
            wires: net.ch1.wires,
            ..model.ch1
        };
        net.sweep = NetSweep {
            wires: net.sweep.wires,
            ..model.sweep
        };
        net.ch2 = NetSquare {
            wires: net.ch2.wires,
            ..model.ch2
        };
        net.ch3 = NetWave {
            wires: net.ch3.wires,
            ..model.ch3
        };
        net.ch4 = NetNoise {
            wires: net.ch4.wires,
            ..model.ch4
        };
        net.settle_wires();
    }
}
