//! The clocks, replayed from the power-on.

use super::*;

/// The clocks and resets, edge by edge, with each line's inputs: the
/// latches, the resets and the clocks they make, against the model's.
///
/// The trace takes a line two delta cycles after the edge: the latches have
/// followed it, but AFER, a flip-flop, changes its output a `tco` later, so
/// a line shows `cpu_sys_reset` as the edge before left it. A trace from the
/// power-on starts at its second line: the first is taken at time 0, before
/// the latches settle.
pub fn replay_clocks(name: &str, lines: &[Line], report: &mut Report, from_power_on: bool) {
    let first = usize::from(from_power_on);
    let (Some(start), Some(following)) = (
        lines.get(first).and_then(|l| l.clocks.as_ref()),
        lines.get(first + 1).and_then(|l| l.clocks.as_ref()),
    ) else {
        return;
    };
    let mut net = NetClocks {
        cpu_sys_reset: following.net.cpu_sys_reset,
        ..start.net
    };
    for (n, line) in lines.iter().enumerate().skip(first + 1) {
        let Some(clocks) = line.clocks.as_ref() else {
            return;
        };
        let before = net;
        net.step(clocks.inputs);
        report.phases += 1;
        if before.sys_reset && !net.sys_reset {
            report.saw("clocks: the power-on wait ends");
        }
        if before.cpu_sys_reset && !net.cpu_sys_reset {
            report.saw("clocks: the CPU's reset ends");
        }
        if !clocks.inputs.reset_pin_n {
            report.saw("clocks: /RESET is low");
        }
        if !clocks.inputs.clk_ena {
            report.saw("clocks: the core's clocks stop (HALT)");
        }
        if !clocks.inputs.osc_ena {
            report.saw("clocks: the oscillator stops (STOP)");
        }
        // The trace takes a line as `phase` changes, two delta cycles after
        // the latches (`phase_i`, then the port). The clocks decoded from
        // `phase_i` have changed by then, those decoded through another
        // signal (`clk_1mhz_i`, `data_phase_i`) not yet: they still show
        // the phase before the edge.
        let (shown, outputs) = clocks_shown(before, net, clocks.inputs);
        if shown != clocks.net || outputs != clocks.outputs {
            report.differ(
                format!("clocks, phase {}", line.phase),
                format!(
                    "{name}:{} ({shown:?} {outputs:?}, model {:?} {:?})",
                    n + 1,
                    clocks.net,
                    clocks.outputs
                ),
            );
            net = clocks.net;
            net.cpu_sys_reset = lines
                .get(n + 1)
                .and_then(|l| l.clocks.as_ref())
                .map_or(clocks.net.cpu_sys_reset, |c| c.net.cpu_sys_reset);
        }
    }
}
/// The clocks' state and outputs as a line of the trace shows them, after
/// the step `after` from `before` with `inputs` (see `replay_clocks`).
pub const fn clocks_shown(
    before: NetClocks,
    after: NetClocks,
    inputs: ClockInputs,
) -> (NetClocks, ClockOutputs) {
    let now = after.outputs(inputs);
    let then = before.outputs(inputs);
    (
        NetClocks {
            cpu_sys_reset: before.cpu_sys_reset,
            ..after
        },
        ClockOutputs {
            clk_1mhz: then.clk_1mhz,
            data_phase: then.data_phase,
            exec_phase: then.exec_phase,
            phi: then.phi,
            reset2: before.cpu_sys_reset || after.sys_reset,
            ..now
        },
    )
}
