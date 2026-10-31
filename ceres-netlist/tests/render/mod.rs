//! The replay of render (`NetRender`) through the traces that have its
//! fields: the 36 after the clocks' (see `BLOCKS` in
//! `hdl/board/gameboy_tb.vhd`).

use {
    super::{Line, Report, bit, hex8},
    ceres_netlist::{
        lcd_control::NetLcdControl,
        oam::NetOam,
        render::{NetRender, NetSpriteStore, RenderInputs, vram_power_on},
    },
};

/// The fields of a line that has render's.
pub const RENDER_LINE_FIELDS: usize = 73;
/// ... and OAM's and VRAM's, after render's.
pub const OAM_LINE_FIELDS: usize = 79;

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
    /// OAM and VRAM (older traces lack them).
    pub oam_vram: Option<OamVramLine>,
}

/// OAM and VRAM in a line of a trace, and the data pins and the CPU core's
/// memory request outside VRAM, which they take in.
#[expect(clippy::struct_excessive_bools, reason = "the trace's levels")]
pub struct OamVramLine {
    pub d: u8,
    pub non_vram_mreq: bool,
    pub wuje: bool,
    pub maka: bool,
    pub mcs_n: bool,
    pub mrd_n: bool,
    pub mwr_n: bool,
    pub first_odd: u8,
    pub bus_even: u8,
    pub bus_odd: u8,
    pub vram_addr: u16,
}

/// OAM's and VRAM's fields, after render's.
fn parse_oam_vram(fields: &[&str]) -> Option<OamVramLine> {
    let &[d, bits, first_odd, bus_even, bus_odd, vram_addr] = fields else {
        return None;
    };
    Some(OamVramLine {
        d: hex8(d)?,
        non_vram_mreq: bit(bits, 0)?,
        wuje: bit(bits, 3)?,
        maka: bit(bits, 4)?,
        mcs_n: bit(bits, 5)?,
        mrd_n: bit(bits, 6)?,
        mwr_n: bit(bits, 7)?,
        first_odd: hex8(first_odd)?,
        bus_even: hex8(bus_even)?,
        bus_odd: hex8(bus_odd)?,
        vram_addr: u16::from_str_radix(vram_addr, 16).ok()?,
    })
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
        ref rest @ ..,
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
        oam_vram: match rest {
            [] => None,
            rest => Some(parse_oam_vram(rest)?),
        },
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

/// What render takes in at the end of `line`'s phase, after the edge that
/// ends `edge`'s (the clock as it leaves it), with LCDC `lcdc`. The clocks'
/// outputs are decoded from the phase and the CPU core's strobes: the trace
/// shows the next phase's for some (see `clocks_shown`).
pub fn inputs(edge: &Line, line: &Line, lcdc: u8) -> Option<RenderInputs> {
    let clk = edge.lcd.as_ref()?.clk;
    let pins = line.clocks.as_ref()?.inputs;
    let phase = line.phase;
    let (lcd, render) = (line.lcd.as_ref()?, line.render.as_ref()?);
    let scan = lcd.scan.as_ref()?;
    let oam_vram = render.oam_vram.as_ref()?;
    let from_vram = line.dma.source >> 5 == 0b100;
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
        data_mode3: line.data,
        line_start: render.line_start,
        stop_scan: render.stop_scan,
        ceno: scan.net.ceno,
        save: scan.save,
        // The Y comparator's difference, Y - LY - 1: its low four bits.
        scan_line: scan.y.wrapping_add(!lcd.net.ly) & 0x0F,
        wuvu: lcd.net.wuvu,
        dma_run: line.dma.run,
        dma_index: line.dma.count,
        scanning: scan.net.scanning,
        scan_index: scan.net.index,
        wosu: lcd.net.wosu,
        dma_phi: line.dma_phi?,
        dma_addr_ext: line.dma.run && !from_vram,
        d: oam_vram.d,
        rd: line.rd,
        non_vram_mreq: oam_vram.non_vram_mreq,
        write: pins.write,
        // @hw die: AFAS AREV APOV; AGUT AWOD ABUZ; BAPY BERU BUFA BYLY
        cpu_wr_sync: pins.write && matches!(phase, 'e' | 'f' | 'g'),
        cpu_ext_cs_en: (pins.t1 && !pins.t2) || (pins.mreq && !matches!(phase, 'a' | 'b')),
        clk_t4: pins.clk_ena && matches!(phase, 'g' | 'h'),
        vram_to_oam: line.dma.run && from_vram,
        dma_addr: u16::from_be_bytes([line.dma.source, line.dma.count]),
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
pub fn resync(net: &mut NetRender, model: &NetRender, vram: &[u8; 0x2000]) {
    *net = NetRender {
        wires: net.wires,
        inputs: net.inputs,
        vram_ma: net.vram_ma,
        oam: net.oam,
        render_settled: net.render_settled,
        md_kept: net.md_kept,
        store: NetSpriteStore {
            index: net.store.index,
            ..model.store
        },
        ..*model
    };
    net.settle_wires(vram);
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
    let Some(first) = lines[start].render.as_ref() else {
        return;
    };
    let mut net = NetRender {
        md_kept: 0xFF,
        oam: oam_from(&lines[start]).unwrap_or_default(),
        ..first.net
    };
    let mut vram = vram_power_on();
    // The wires settle from the first line's state.
    let Some(at_start) = inputs(&lines[start], &lines[start], lcdcs[start]) else {
        return;
    };
    net.inputs = at_start;
    // Only the wires: an edge from the default wires (/MCS and /MWR low)
    // would write VRAM.
    net.settle_wires(&vram);
    for n in start..lines.len() - 1 {
        let (line, ends) = (&lines[n], &lines[n + 1]);
        let Some(model) = ends.render.as_ref() else {
            return;
        };
        let Some(after) = inputs(line, ends, lcdcs[n + 1]) else {
            return;
        };
        let before = net;
        net.edge(after, &mut vram);
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
            resync(&mut net, &model.net, &vram);
        }
        compare_vram(&net, model, &place, report);
        compare_oam(&mut net, ends, "render", &place, report);
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

/// The VRAM bus of `net` against the model's: VRAM's data, the address
/// lines, /MCS, /MRD and /MWR.
fn compare_vram(net: &NetRender, model: &RenderLine, place: &str, report: &mut Report) {
    let Some(theirs) = model.oam_vram.as_ref() else {
        return;
    };
    let w = &net.wires;
    if w.cpu_write && !w.mwr_n {
        report.saw("render: a CPU write of VRAM");
    }
    // While nothing drives the data pins, the board's keeper holds them
    // against the pads' pull-ups (the bits it holds low resolve to 'W'), and
    // which value it caught last depends on how many deltas the address and
    // /MCS take through the chip's ports and the board. Nothing takes VRAM's
    // data then (the fetches, OAM DMA and the CPU's reads all select VRAM),
    // so it is compared only while driven.
    let driven = w.md_driven;
    if !driven {
        report.saw("render: VRAM's data pins float (not compared)");
    }
    let md = if driven { w.md } else { model.md };
    let ours = (md, net.vram_ma, w.mcs_n, w.mrd_n, w.mwr_n);
    let model = (
        model.md,
        theirs.vram_addr & 0x1FFF,
        theirs.mcs_n,
        theirs.mrd_n,
        theirs.mwr_n,
    );
    if ours != model {
        let what = if (ours.1, ours.2, ours.3, ours.4) != (model.1, model.2, model.3, model.4) {
            "render: the VRAM bus's address or controls"
        } else if driven {
            "render: VRAM's data, driven"
        } else {
            "render: VRAM's data, floating"
        };
        report.differ(
            what.to_owned(),
            format!("{place} ({ours:02X?}, model {model:02X?})"),
        );
    }
}

/// OAM's latches, data lines, WUJE and MAKA in a line of a trace.
type OamState = (u8, u8, u8, u8, u8, u8, bool, bool);

fn oam_state(line: &Line) -> Option<OamState> {
    let (lcd, render) = (line.lcd.as_ref()?, line.render.as_ref()?);
    let (scan, oam_vram) = (lcd.scan.as_ref()?, render.oam_vram.as_ref()?);
    let oam = scan.oam.as_ref()?;
    Some((
        oam.first,
        oam_vram.first_odd,
        scan.y,
        render.attributes,
        oam_vram.bus_even,
        oam_vram.bus_odd,
        oam_vram.wuje,
        oam_vram.maka,
    ))
}

/// OAM as `line` has it, its RAM as at power-on (the traces start there).
pub fn oam_from(line: &Line) -> Option<NetOam> {
    let mut oam = NetOam::power_on();
    (
        oam.first_even,
        oam.first_odd,
        oam.second_even,
        oam.second_odd,
        oam.bus_even,
        oam.bus_odd,
        oam.wuje,
        oam.maka,
    ) = oam_state(line)?;
    Some(oam)
}

/// OAM's latches, data lines, WUJE and MAKA of `net` after a step, against
/// the model at the end of the phase (`ends`); after a difference they take
/// the model's.
pub fn compare_oam(net: &mut NetRender, ends: &Line, who: &str, place: &str, report: &mut Report) {
    let Some(theirs) = oam_state(ends) else {
        return;
    };
    let ours = (
        net.oam.first_even,
        net.oam.first_odd,
        net.oam.second_even,
        net.oam.second_odd,
        net.oam.bus_even,
        net.oam.bus_odd,
        net.oam.wuje,
        net.oam.maka,
    );
    if ours != theirs {
        report.differ(
            format!("{who}: OAM, phase {}", ends.phase),
            format!("{place} (latches and buses {ours:02X?}, model {theirs:02X?})"),
        );
        let o = &mut net.oam;
        (
            o.first_even,
            o.first_odd,
            o.second_even,
            o.second_odd,
            o.bus_even,
            o.bus_odd,
            o.wuje,
            o.maka,
        ) = theirs;
        net.wires.oam_driver = net.oam.bus_drivers(&net.oam_inputs());
    }
}
