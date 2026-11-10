//! Ceres's mode 3 loop and the netlist's pipeline side by side on whole
//! lines, concretely
//! (`cargo test -p ceres-core --features check cosim`).
//!
//! `check::related` on the lines these walk, in seconds, before Kani takes
//! every state: each line starts as `check::start_agrees` has it, and steps
//! dot by dot as `check::step_agrees` does. A failure prints both sides dot
//! by dot.

use {
    super::check::{
        Before, Ghost, INSIDE, Mode3Loop, before_start, ceres_dot, ceres_first_dot, expected,
        related, signed, step_agrees,
    },
    crate::ppu::display::{Display, fetcher::FetcherStep, state::State},
    ceres_netlist::render_edge::{EdgeInputs, NO_X, NetPipeline, SLOTS},
    core::fmt::Write as _,
};

/// LCDC with the LCD and the background on, and its object and window
/// enables.
const LCDC_ON: u8 = 0x91;
const LCDC_OBJ_EN: u8 = 2;
const LCDC_WIN_EN: u8 = 0x20;
/// SCX's address.
const SCX: u16 = 0xFF43;

/// A line: SCX, the armed window's WX if it is on, and the objects' X from
/// the nearest.
struct Line {
    scx: u8,
    wx: Option<u8>,
    objects: &'static [u8],
}

/// Both sides, the ghost, and the dots walked (the table's rows).
struct Walk {
    l: Mode3Loop,
    net: NetPipeline,
    g: Ghost,
    dot: u32,
    table: String,
}

/// The netlist's inputs in mode 3 with nothing written (`check::quiet`).
const fn inputs(l: &Mode3Loop, clk: bool) -> EdgeInputs {
    let mut lcdc = LCDC_ON;
    if l.objects_enabled {
        lcdc |= LCDC_OBJ_EN;
    }
    if l.window_enabled {
        lcdc |= LCDC_WIN_EN;
    }
    EdgeInputs {
        clk,
        ppu_reset: false,
        hard_reset: false,
        lcdc,
        ly: 0,
        hclk: false,
        mode1: false,
        line_start: false,
        stop_scan: false,
        ceno: false,
        cpu_wr_sync: false,
        addr: 0,
        data: 0,
    }
}

/// What the netlist's fields are named by, where they differ.
macro_rules! differing {
    ($a:expr, $b:expr; $($field:ident)*) => {{
        let mut out = String::new();
        $(
            if $a.$field != $b.$field {
                _ = write!(out, " {}={:?}/{:?}", stringify!($field), $a.$field, $b.$field);
            }
        )*
        out
    }};
}

const fn step_name(step: FetcherStep) -> &'static str {
    match step {
        FetcherStep::GetTileT1 => "T1",
        FetcherStep::GetTileT2 => "T2",
        FetcherStep::DataLowT1 => "L1",
        FetcherStep::DataLowT2 => "L2",
        FetcherStep::DataHighT1 => "H1",
        FetcherStep::DataHighT2 => "H2",
        FetcherStep::Push => "Pu",
    }
}

impl Walk {
    /// The line before ceres's first dot of the loop (`h = 162`): the
    /// netlist as `check::before_start` has it, ceres's position at the
    /// lead-in's start, the window armed if it is on.
    fn start(line: &Line) -> Self {
        let mut d = Display::default();
        let armed = line.wx.is_some();
        d.window.wy_triggered = armed;
        let mut xs = [NO_X; SLOTS];
        d.objs.count = line.objects.len();
        for (i, &x) in line.objects.iter().enumerate() {
            // Ceres's are sorted descending, the next one last.
            d.objs.entries[line.objects.len() - 1 - i].x = x;
            xs[i] = x;
        }
        let l = Mode3Loop {
            d,
            mixer_scx: line.scx,
            objects_enabled: !line.objects.is_empty(),
            window_enabled: armed,
            wx: line.wx.unwrap_or(0),
        };
        let carried = NetPipeline {
            mode3: false,
            voga: false,
            px: 0,
            fetch_count: 0,
            lyzu: false,
            nyka: false,
            pory: false,
            pygo: false,
            started: false,
            fine: 0,
            puxa: false,
            nyze: false,
            discard: false,
            ryfa: false,
            rene: false,
            scx: line.scx,
            wy: 0,
            wx: l.wx,
            sary: false,
            wy_latch: armed,
            pyco: false,
            nunu: false,
            nopa: false,
            in_window: false,
            win_x: 0,
            win_y: 0,
            rydy: false,
            sovy: false,
            xs,
            fetched: 0,
            sobu: false,
            suda: false,
            sprite_step: 0,
            busy: false,
            tobu: false,
            vonu: false,
            seba: false,
            tyfo: false,
            inputs: inputs(&l, false),
        };
        let g = Ghost::at_rest(&l.d.bg);
        Self {
            net: before_start(&carried),
            l,
            g,
            dot: 0,
            table: String::from(
                "dot | state                 w   pos len st | wxT bf tx | px fc fine lyzu nyka pory pygo st ryfa rene | pyco nunu nopa inw rydy sovy wx | ghost: push extra frozen age win fine | differs from expected\n",
            ),
        }
    }

    /// Ceres's first dot of the loop and its two edges, as
    /// `check::start_agrees` runs them.
    fn first_dot(&mut self) {
        let snapshot = Before::of(&self.l.d);
        ceres_first_dot(&mut self.l);
        self.net.edge(inputs(&self.l, true));
        self.net.edge(inputs(&self.l, false));
        let rest = Ghost {
            age: 0,
            ..Ghost::at_rest(&self.l.d.bg)
        };
        self.g = rest.after(snapshot, &self.l.d);
        self.row();
    }

    /// A dot, as `check::step_agrees` runs it: `Some` once ceres has drawn
    /// the line, with whether the netlist's mode 3 stopped at the dot's
    /// second edge and not before. While the line goes on, the netlist's
    /// mode 3 must not stop.
    fn dot(&mut self) -> Option<bool> {
        let drawn = matches!(self.l.d.state, State::HBlankStart);
        let before = Before::of(&self.l.d);
        if !drawn {
            ceres_dot(&mut self.l);
        }
        self.net.edge(inputs(&self.l, true));
        let stopped_early = self.net.stop_mode3();
        self.net.edge(inputs(&self.l, false));
        if drawn {
            return Some(!stopped_early && self.net.stop_mode3());
        }
        self.g = self.g.after(before, &self.l.d);
        self.row();
        assert!(
            !stopped_early && !self.net.stop_mode3(),
            "the netlist's mode 3 stopped before ceres drew the line\n{}",
            self.table
        );
        None
    }

    fn related(&self) -> bool {
        related(&self.l, &self.net, self.g)
    }

    /// A row of the table: both sides after the dot, and where the netlist
    /// is not what `check::expected` gives.
    fn row(&mut self) {
        self.dot += 1;
        let (d, net, g) = (&self.l.d, &self.net, self.g);
        let e = expected(&self.l, net, g);
        let differs = differing!(net, e;
            mode3 voga px fetch_count lyzu nyka pory pygo started fine puxa nyze discard ryfa
            rene sary wy_latch pyco nunu nopa in_window win_x win_y rydy sovy fetched sobu suda
            sprite_step busy tobu vonu seba tyfo
        );
        _ = writeln!(
            self.table,
            "{:3} | {:21} {} {:5} {:3} {} | {:3} {:2} {:2} | {:3} {:1} {:4} {:4} {:4} {:4} {:4} {:2} {:4} {:4} | {:4} {:4} {:4} {:3} {:4} {:4} {:2} | {:11} {:5} {:6} {:3} {:3} {:4} |{}",
            self.dot,
            format!("{:?}", d.state),
            d.wait,
            signed(d.bg.position),
            d.bg.fifo.len(),
            step_name(d.bg.fetcher.step),
            u8::from(d.window.wx_triggered),
            u8::from(d.window.being_fetched),
            d.window.tile_x,
            net.px,
            net.fetch_count,
            net.fine,
            u8::from(net.lyzu),
            u8::from(net.nyka),
            u8::from(net.pory),
            u8::from(net.pygo),
            u8::from(net.started),
            u8::from(net.ryfa),
            u8::from(net.rene),
            u8::from(net.pyco),
            u8::from(net.nunu),
            u8::from(net.nopa),
            u8::from(net.in_window),
            u8::from(net.rydy),
            u8::from(net.sovy),
            net.win_x,
            g.since_push,
            g.extra,
            u8::from(g.frozen),
            g.age,
            g.window,
            g.window_fine,
            differs,
        );
    }
}

/// The lines the proofs cover from their start: the window off, any SCX,
/// objects at X 8 or more.
#[test]
fn idle_lines_stay_related() {
    let objects: [&[u8]; 7] = [
        &[],
        &[8],
        &[9],
        &[80],
        &[160],
        &[20, 20],
        &[8, 16, 17, 100, 166],
    ];
    for scx in 0..8 {
        for xs in objects {
            let mut w = Walk::start(&Line {
                scx,
                wx: None,
                objects: xs,
            });
            w.first_dot();
            loop {
                assert!(w.related(), "SCX {scx}, objects {xs:?}\n{}", w.table);
                if let Some(stopped) = w.dot() {
                    assert!(stopped, "SCX {scx}, objects {xs:?}\n{}", w.table);
                    break;
                }
            }
        }
    }
}

/// The armed window's lines, up to the dot whose check reaches WX.
#[test]
fn armed_lines_stay_related_up_to_wx() {
    for scx in 0..8 {
        for wx in [7, 8, 9, 14, 15, 16, 17, 50, 100, 159, 165] {
            let mut w = Walk::start(&Line {
                scx,
                wx: Some(wx),
                objects: &[],
            });
            w.first_dot();
            while w.g.window == 0 {
                assert!(w.related(), "SCX {scx}, WX {wx}\n{}", w.table);
                assert!(w.dot().is_none(), "SCX {scx}, WX {wx}\n{}", w.table);
            }
        }
    }
}

/// The window's lines from start to end, with the objects off: related at
/// every dot, through the window's start and inside it, and the netlist's
/// mode 3 stops a dot after ceres has drawn the line.
#[test]
fn window_lines_stay_related() {
    for scx in 0..8 {
        for wx in 7..=165 {
            let mut w = Walk::start(&Line {
                scx,
                wx: Some(wx),
                objects: &[],
            });
            w.first_dot();
            loop {
                assert!(w.related(), "SCX {scx}, WX {wx}\n{}", w.table);
                if let Some(stopped) = w.dot() {
                    assert!(stopped, "SCX {scx}, WX {wx}\n{}", w.table);
                    break;
                }
            }
            assert!(
                w.g.window != 0,
                "SCX {scx}, WX {wx}: the window did not start"
            );
        }
    }
}

/// A small generator of the values a harness leaves free (xorshift).
struct Free(u64);

impl Free {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn byte(&mut self) -> u8 {
        self.next().to_le_bytes()[3]
    }

    fn flag(&mut self) -> bool {
        self.byte() & 1 != 0
    }

    /// An address the CPU's bus carries: SCX's, WY's, LCDC's or any other
    /// but WX's (the window's lines hold WX).
    fn address(&mut self) -> u16 {
        match self.byte() % 4 {
            0 => SCX,
            1 => 0xFF4A,
            2 => 0xFF40,
            _ => 0xFF00 | u16::from(self.byte() & 0x3F),
        }
    }

    /// Inputs as `check::quiet` takes them, with the bus anywhere: a strobe
    /// on SCX's address carries `scx`'s low bits.
    fn inputs(&mut self, l: &Mode3Loop, clk: bool, scx: u8) -> EdgeInputs {
        let quiet = inputs(l, clk);
        let addr = self.address();
        let data = if addr == SCX {
            (self.byte() & !7) | (scx & 7)
        } else {
            self.byte()
        };
        EdgeInputs {
            lcdc: (quiet.lcdc & (LCDC_OBJ_EN | LCDC_WIN_EN))
                | (self.byte() & !(LCDC_OBJ_EN | LCDC_WIN_EN)),
            ly: self.byte(),
            hclk: self.flag(),
            cpu_wr_sync: self.flag() && self.flag(),
            addr,
            data,
            ..quiet
        }
    }
}

/// The bus and the line's inputs of an edge, for a failure's message.
fn shown(i: &EdgeInputs) -> String {
    format!(
        "strobe {} addr {:04X} data {:02X} lcdc {:02X} ly {} hclk {}",
        i.cpu_wr_sync, i.addr, i.data, i.lcdc, i.ly, i.hclk
    )
}

impl Walk {
    /// The line after `dots` dots, unless it ended before.
    fn after(line: &Line, dots: u32) -> Option<Self> {
        let mut w = Self::start(line);
        w.first_dot();
        for _ in 1..dots {
            if w.dot().is_some() {
                return None;
            }
        }
        Some(w)
    }

    /// What the relation leaves free of the pair, taken from `free`: the
    /// netlist's WY latch and line counter, the bus and the line's inputs
    /// before the dot (a strobe perhaps high already), the sprite store and
    /// ceres's list (the objects are off), and past the window's start the
    /// lead-in's rerun.
    fn free_pair(&mut self, free: &mut Free) {
        self.net.wy = free.byte();
        self.net.sary = free.flag();
        self.net.win_y = free.byte();
        self.l.d.bg.fractional = free.flag();
        if self.g.window != 0 {
            self.g.rerun = free.flag();
        }
        let quiet = inputs(&self.l, false);
        self.net.inputs = EdgeInputs {
            lcdc: quiet.lcdc ^ (free.byte() & !(LCDC_OBJ_EN | LCDC_WIN_EN)),
            ly: free.byte(),
            hclk: free.flag(),
            addr: free.address(),
            data: free.byte(),
            cpu_wr_sync: free.flag() && free.flag(),
            ..quiet
        };
        if self.net.inputs.cpu_wr_sync && self.net.inputs.addr == SCX {
            self.net.inputs.data = self.net.scx;
        }
        for x in &mut self.net.xs {
            *x = free.byte();
        }
        self.l.d.objs.count = usize::from(free.byte() % 11);
        let mut x = 8 + free.byte() % 160;
        for i in (0..self.l.d.objs.count).rev() {
            self.l.d.objs.entries[i].x = x;
            x = x.saturating_add(free.byte() % 4);
        }
    }

    /// A dot from the pair with free inputs: any bus (an SCX strobe carries
    /// the mixer's low bits), LCDC's other bits, LY and hclk, and any SCX in
    /// the mixer after it; past the window's start, the mixer may have
    /// taken a write of SCX's low bits in the last dot, whose strobe rises
    /// in this one. What went wrong, if the pair does not stay related.
    fn free_dot(&mut self, free: &mut Free) -> Result<(), String> {
        let scx = self.l.mixer_scx;
        let mut rising = free.inputs(&self.l, true, scx);
        let mut falling = free.inputs(&self.l, false, scx);
        if self.g.window != 0 && free.flag() {
            self.l.mixer_scx = free.byte();
            rising.cpu_wr_sync = true;
            rising.addr = SCX;
            rising.data = self.l.mixer_scx;
        }
        if rising.cpu_wr_sync && rising.addr == SCX {
            // The bus holds what the strobe carries.
            falling.addr = rising.addr;
            falling.data = rising.data;
        }
        if !self.related() {
            return Err(String::from("not related before the dot"));
        }
        let mixer = free.byte();
        let before = format!(
            "window {}, {:?}; net wy {} sary {} win_y {} scx {:02X}; mixer {:02X} then {mixer:02X}; \
             before: {}; rising: {}; falling: {}",
            self.g.window,
            self.l.d.state,
            self.net.wy,
            self.net.sary,
            self.net.win_y,
            self.net.scx,
            self.l.mixer_scx,
            shown(&self.net.inputs),
            shown(&rising),
            shown(&falling),
        );
        if step_agrees(&mut self.l, &mut self.net, self.g, mixer, rising, falling) {
            Ok(())
        } else {
            Err(before)
        }
    }
}

/// A dot from the pairs the window's lines pass through, with what a Kani
/// harness leaves free taken at random (`Walk::free_pair`,
/// `Walk::free_dot`). What Kani refutes on a pair the lines reach shows
/// here in seconds, with the values that refute it.
#[test]
fn window_dots_with_free_inputs() {
    let mut free = Free(0x9E37_79B9_7F4A_7C15);
    for wx in [7, 20, 21, 22, 23, 24, 25, 26, 27, 164, 165] {
        for scx in [0, 3, 5] {
            let line = Line {
                scx,
                wx: Some(wx),
                objects: &[],
            };
            let mut dots = 1;
            while Walk::after(&line, dots).is_some() {
                for trial in 0..24 {
                    let Some(mut w) = Walk::after(&line, dots) else {
                        break;
                    };
                    w.free_pair(&mut free);
                    let went = w.free_dot(&mut free);
                    assert!(
                        went.is_ok(),
                        "WX {wx}, SCX {scx}, after dot {dots}, trial {trial}: {}\n{}",
                        went.err().unwrap_or_default(),
                        w.table
                    );
                }
                dots += 1;
            }
        }
    }
}

/// The window's start dot by dot, for one line of each phase of the
/// background's fetch (`-- --ignored --nocapture`).
#[test]
#[ignore = "prints the window's start"]
#[expect(clippy::print_stdout, reason = "the table is what this is for")]
fn window_start_table() {
    for wx in 20..28 {
        let mut w = Walk::start(&Line {
            scx: 0,
            wx: Some(wx),
            objects: &[],
        });
        w.first_dot();
        while w.g.window < INSIDE && w.dot().is_none() {}
        let rows: Vec<&str> = w.table.lines().collect();
        let from = rows.len().saturating_sub(11).max(1);
        println!("WX {wx}\n{}", rows[0]);
        for row in &rows[from..] {
            println!("{row}");
        }
        println!();
    }
}
