//! The replay of render (`NetRender`) through the traces that have its
//! fields: the 36 after the clocks' (see `BLOCKS` in
//! `hdl/board/gameboy_tb.vhd`).

use {
    super::{Line, Report, bit, hex8},
    ceres_netlist::{
        lcd_control::NetLcdControl,
        render::{NetRender, NetSpriteStore, RenderInputs},
    },
};

/// The fields of a line that has render's.
pub const RENDER_LINE_FIELDS: usize = 73;

/// Render in a line of a trace: its flip-flops and latches, what it takes
/// in from the other blocks, and what it gives out.
#[expect(clippy::struct_excessive_bools, reason = "the trace's levels")]
pub struct RenderLine {
    pub net: NetRender,
    pub line_start: bool,
    pub stop_scan: bool,
    pub x_match: bool,
    pub md: u8,
    pub attributes: u8,
    pub sprite_line: u8,
    pub ma: u16,
    pub cp: bool,
    pub st: bool,
    pub ld: u8,
}

/// Render's fields, after the clocks'.
#[expect(clippy::too_many_lines, reason = "a field at a time")]
#[expect(clippy::cognitive_complexity, reason = "a field at a time")]
pub fn parse(fields: &[&str]) -> Option<RenderLine> {
    let &[
        flags,
        px,
        fetch_count,
        fine,
        sprite_step,
        win_x,
        win_y,
        bg_tile,
        bg_low,
        bg_shift_a,
        bg_shift_b,
        sp_low_a,
        sp_low_b,
        sp_a,
        sp_b,
        sp_palette,
        sp_priority,
        scy,
        scx,
        wy,
        wx,
        bgp,
        obp0,
        obp1,
        ins,
        md,
        attributes,
        sprite_line,
        ma,
        pins,
        ld,
        scanned,
        fetched,
        xs,
        lines,
        indexes,
        ..,
    ] = fields
    else {
        return None;
    };
    let f = |i| bit(flags, i);
    Some(RenderLine {
        net: NetRender {
            scy: hex8(scy)?,
            scx: hex8(scx)?,
            wy: hex8(wy)?,
            wx: hex8(wx)?,
            bgp: hex8(bgp)?,
            obp0: hex8(obp0)?,
            obp1: hex8(obp1)?,
            mode3: f(0)?,
            voga: f(1)?,
            px: hex8(px)?,
            px9: f(2)?,
            fetch_count: hex8(fetch_count)?,
            lyzu: f(3)?,
            lovy: f(4)?,
            fetching: f(5)?,
            nyka: f(6)?,
            pory: f(7)?,
            pygo: f(8)?,
            started: f(9)?,
            bg_tile: hex8(bg_tile)?,
            bg_low: hex8(bg_low)?,
            bg_shift_a: hex8(bg_shift_a)?,
            bg_shift_b: hex8(bg_shift_b)?,
            sary: f(10)?,
            wy_latch: f(11)?,
            pyco: f(12)?,
            nunu: f(13)?,
            nopa: f(14)?,
            in_window: f(15)?,
            win_x: hex8(win_x)?,
            win_y: hex8(win_y)?,
            rydy: f(16)?,
            sovy: f(17)?,
            fine: hex8(fine)?,
            puxa: f(18)?,
            nyze: f(19)?,
            paho: f(20)?,
            discard: f(21)?,
            ryfa: f(22)?,
            rene: f(23)?,
            line_started: f(24)?,
            sobu: f(25)?,
            suda: f(26)?,
            sprite_step: hex8(sprite_step)?,
            busy: f(27)?,
            tobu: f(28)?,
            vonu: f(29)?,
            seba: f(30)?,
            tyfo: f(31)?,
            sp_low_a: hex8(sp_low_a)?,
            sp_low_b: hex8(sp_low_b)?,
            sp_a: hex8(sp_a)?,
            sp_b: hex8(sp_b)?,
            sp_palette: hex8(sp_palette)?,
            sp_priority: hex8(sp_priority)?,
            store: NetSpriteStore {
                scanned: hex8(scanned)?,
                fetched: u16::from_str_radix(fetched, 16).ok()?,
                xs: slots(xs, 2)?,
                lines: slots(lines, 1)?,
                indexes: slots(indexes, 2)?,
                index: 0,
            },
            ..NetRender::default()
        },
        line_start: bit(ins, 0)?,
        stop_scan: bit(ins, 1)?,
        x_match: bit(ins, 2)?,
        md: hex8(md)?,
        attributes: hex8(attributes)?,
        sprite_line: hex8(sprite_line)?,
        ma: u16::from_str_radix(ma, 16).ok()?,
        cp: bit(pins, 0)?,
        st: bit(pins, 1)?,
        ld: hex8(ld)?,
    })
}

/// The ten slots of a field, `width` hexadecimal digits each.
fn slots(field: &str, width: usize) -> Option<[u8; 10]> {
    let mut slots = [0; 10];
    for (slot, value) in slots.iter_mut().enumerate() {
        *value = u8::from_str_radix(field.get(slot * width..(slot + 1) * width)?, 16).ok()?;
    }
    Some(slots)
}

/// What render takes in at the end of `line`'s phase, with the clock `clk`
/// and LCDC `lcdc`.
pub fn inputs(line: &Line, clk: bool, lcdc: u8) -> Option<RenderInputs> {
    let (lcd, render) = (line.lcd.as_ref()?, line.render.as_ref()?);
    let scan = lcd.scan.as_ref()?;
    Some(RenderInputs {
        clk,
        ppu_reset: lcd.ppu_reset,
        hard_reset: line.clocks.as_ref()?.net.reset2(),
        hclk: lcd.net.vena,
        mode1: lcd.net.mode1,
        ly: lcd.net.ly,
        lcdc,
        addr: line.addr,
        wr: line.wr,
        data: line.data,
        line_start: render.line_start,
        stop_scan: render.stop_scan,
        ceno: scan.net.ceno,
        save: scan.save,
        // The Y comparator's difference, Y - LY - 1: its low four bits.
        scan_line: scan.y.wrapping_add(!lcd.net.ly) & 0x0F,
        wuvu: lcd.net.wuvu,
        dma_run: line.dma.run,
        dma_index: line.dma.count,
        mode2: lcd.mode2,
        scan_index: scan.net.index,
        tile: scan.y,
        attributes: render.attributes,
        md: render.md,
    })
}

/// The flip-flops and latches of `net` that differ from `model`'s.
#[expect(clippy::cognitive_complexity, reason = "a field at a time")]
pub fn differences(net: &NetRender, model: &NetRender) -> Vec<&'static str> {
    let mut differ = Vec::new();
    macro_rules! check {
        ($($field:ident),*) => {
            $(if net.$field != model.$field {
                differ.push(stringify!($field));
            })*
        };
    }
    check!(
        scy,
        scx,
        wy,
        wx,
        bgp,
        obp0,
        obp1,
        mode3,
        voga,
        px,
        px9,
        fetch_count,
        lyzu,
        lovy,
        fetching,
        nyka,
        pory,
        pygo,
        started,
        bg_tile,
        bg_low,
        bg_shift_a,
        bg_shift_b,
        sary,
        wy_latch,
        pyco,
        nunu,
        nopa,
        in_window,
        win_x,
        win_y,
        rydy,
        sovy,
        fine,
        puxa,
        nyze,
        paho,
        discard,
        ryfa,
        rene,
        line_started,
        sobu,
        suda,
        sprite_step,
        busy,
        tobu,
        vonu,
        seba,
        tyfo,
        sp_low_a,
        sp_low_b,
        sp_a,
        sp_b,
        sp_palette,
        sp_priority
    );
    // The store, but its index lines, which the trace lacks.
    let (ours, theirs) = (&net.store, &model.store);
    for (name, differs) in [
        ("scanned", ours.scanned != theirs.scanned),
        ("fetched", ours.fetched != theirs.fetched),
        ("xs", ours.xs != theirs.xs),
        ("lines", ours.lines != theirs.lines),
        ("indexes", ours.indexes != theirs.indexes),
    ] {
        if differs {
            differ.push(name);
        }
    }
    differ
}

/// Takes the model's flip-flops and latches, keeping the wires and the
/// inputs.
pub fn resync(net: &mut NetRender, model: &NetRender) {
    *net = NetRender {
        wires: net.wires,
        inputs: net.inputs,
        store: NetSpriteStore {
            index: net.store.index,
            ..model.store
        },
        ..*model
    };
    net.settle_wires();
}

/// Replays render, edge by edge: the edge that starts line `n + 1`'s phase
/// (the clock as line `n`'s closing edge leaves it), with what render takes
/// in at the end of that phase, against the model at its end. LCDC follows
/// the CPU's writes from the trace's start (the chip's reset clears it).
pub fn replay(name: &str, lines: &[Line], report: &mut Report) {
    let mut control = NetLcdControl { lcdc: 0 };
    let mut lcdcs = Vec::with_capacity(lines.len());
    for line in lines {
        let reset2 = line.clocks.as_ref().is_some_and(|c| c.net.reset2());
        control.settle(
            reset2,
            (line.wr && line.addr == 0xFF40).then_some(line.data),
        );
        lcdcs.push(control.lcdc);
    }
    let Some(start) = lines.iter().position(|l| l.render.is_some()) else {
        return;
    };
    let (Some(first), Some(lcd)) = (lines[start].render.as_ref(), lines[start].lcd.as_ref()) else {
        return;
    };
    let mut net = first.net;
    // The wires settle from the first line's state.
    let Some(at_start) = inputs(&lines[start], lcd.clk, lcdcs[start]) else {
        return;
    };
    net.inputs = at_start;
    net.edge(at_start);
    resync(&mut net, &first.net);
    for n in start..lines.len() - 1 {
        let (line, ends) = (&lines[n], &lines[n + 1]);
        let (Some(clk), Some(model)) = (line.lcd.as_ref().map(|l| l.clk), ends.render.as_ref())
        else {
            return;
        };
        let Some(after) = inputs(ends, clk, lcdcs[n + 1]) else {
            return;
        };
        let before = net;
        net.edge(after);
        report.phases += 1;
        let place = format!("{name}:{}", n + 2);
        for (what, happened) in [
            ("render: mode 3 ends", before.mode3 && !net.mode3),
            (
                "render: the fine scroll ends",
                before.discard && !net.discard,
            ),
            (
                "render: the window starts",
                !before.in_window && net.in_window,
            ),
            ("render: a sprite fetch starts", !before.sobu && net.sobu),
            (
                "render: a CPU write of SCX, SCY, WX or WY",
                after.wr
                    && matches!(after.addr, 0xFF42 | 0xFF43 | 0xFF4A | 0xFF4B)
                    && ends.phase == 'e',
            ),
        ] {
            if happened {
                report.saw(what);
            }
        }
        let differ = differences(&net, &model.net);
        if !differ.is_empty() {
            report.differ(
                format!("render, phase {}: {}", ends.phase, differ.join(" ")),
                place.clone(),
            );
            resync(&mut net, &model.net);
        }
        let ma = net.ma().unwrap_or(0);
        let (cp, st, ld) = net.pins();
        if net.wires.x_match != model.x_match || net.wires.line != model.sprite_line {
            report.differ(
                format!("render: the store's match, phase {}", ends.phase),
                format!(
                    "{place} (x_match {} line {:X}, model {} {:X})",
                    net.wires.x_match, net.wires.line, model.x_match, model.sprite_line
                ),
            );
        }
        if ma != model.ma || cp != model.cp || st != model.st || ld != model.ld {
            report.differ(
                format!("render: pins, phase {}", ends.phase),
                format!(
                    "{place} (ma {ma:04X} cp {cp} st {st} ld {ld}, model {:04X} {} {} {})",
                    model.ma, model.cp, model.st, model.ld
                ),
            );
        }
    }
}
