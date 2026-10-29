//! Kani proofs that ceres's object search finds the sprites the netlist's
//! OAM scan saves, on the DMG (`cargo kani -p ceres-core --harness
//! netlist_scan`).
//!
//! `NetScan` (`netlist.rs`) transcribes `hdl/rtl/oam_scan.vhd` edge by
//! edge. `netlist_scan_y_match` proves its comparator, the sum of Y and LY's
//! complement, the same as ceres's `on_line` for every Y, line and height.
//! `netlist_scan_selection` runs a whole scan on the netlist's clocks (as
//! the line timing's proof relates them to ceres's half dot: `wuvu` is
//! `h & 2`, `wosu` `(h + 1) & 2`, vclk from `h = 904`) with any Ys, from
//! the line's end to mode 3, and with a write of LCDC bit 2 in any M-cycle
//! around it: it saves the first ten sprites on the line, in index order,
//! one slot each, with the height ceres's search reads for each. The
//! comparator sees each sprite's Y a window late, from OAM's second
//! latches, and the sprite store takes the index from a latch of its own:
//! sprite i is saved at half dots `4 * i - 1` and `4 * i`, ceres reads it at
//! `4 * i - 3`, and a write for the access at `8 * m` lands in the netlist
//! at `8 * m + 1`, in ceres at `8 * m - 2`, so both give sprite i the new
//! height from `i = 2 * m + 1` on. A save never sees half a write.
//! `netlist_scan_dma` runs a whole scan while an OAM DMA runs from any
//! M-cycle to any later one, with the CPU halted in any of them, with OAM's
//! two stages of latches (`NetOamLatches`): the DMA closes the first, which
//! keep the last Y the scan read, and the scan goes on comparing it; ceres's
//! search keeps its `y_bus` (`ObjectSearch::latch`) for the reads its DMA
//! hides (`Dma::runs`, which the DMA's proof shows is the netlist's
//! run, a restart included). Both see the same Y for every sprite, from any
//! Y the latches hold as the scan starts, and the netlist saves the sprites
//! ceres finds. The DMA's run changes at the A edge, `8 * m + 5`, where
//! ceres steps its DMA (the DMA proof's mapping); ceres's read of sprite
//! `2 * m + 2`, at that half dot, comes before the step.
//! `netlist_scan_consider` proves a step of ceres's search, which keeps a
//! sprite when it is on the line and the list has room: sprite after
//! sprite, it keeps the same ten.
//!
//! What it does not cover yet: the half dot of each of ceres's reads, and
//! of its DMA's steps against them, which are measured (its state machine
//! is outside the proof); the CPU addressing OAM during the scan; what the
//! latches hold as the scan starts, which the sprite fetch of the line
//! before sets (during a DMA, the DMA's byte: measured with
//! `hdl/check/dma_fetch.asm`); the scan of line 0 after the LCD turns on;
//! and the order of the sprites for the fetch (the sprite store's and
//! render's).

use super::{
    Entry, MAX_OBJECTS_PER_LINE, ObjectSearch,
    netlist::{Clocks, NetOamLatches, NetScan, OamInputs, SLOTS},
    on_line,
};

/// The visible lines, which a scan precedes.
const LINES: u8 = 144;

/// A Y that the netlist's comparator puts on `line` is one ceres puts there,
/// for both heights.
#[kani::proof]
fn netlist_scan_y_match() {
    let y: u8 = kani::any();
    let line: u8 = kani::any();
    let tall: bool = kani::any();
    kani::assume(line < LINES);
    assert!(NetScan::y_match(y, line, tall) == on_line(y, line, tall));
}

/// The line timing's dividers after the edge at half dot `t`, counted from
/// the start of the line before the scanned one.
const fn wuvu(t: i32) -> bool {
    t & 2 != 0
}

const fn wosu(t: i32) -> bool {
    (t + 1) & 2 != 0
}

/// Half dots per line.
const LINE_H: i32 = 912;
/// The line's end (vclk) rises 8 half dots before the line.
const LINE_END_H: i32 = LINE_H - 8;

/// A DMG write of LCDC in the M-cycle whose access is at half dot `8 * m`
/// reaches ceres's object search at `8 * m - 2` (`cpu_bus/write.rs`,
/// `DmgLcdc`: OBJ_SIZE lands with the second write).
const CERES_LCDC_H: i32 = -2;
/// It reaches the netlist's LCDC in that M-cycle's phase E (traced:
/// gambatte's `late_sizechange` ROMs).
const NET_LCDC_H: i32 = 1;

/// The M-cycle's phase A, whose edge `dma_phi` rises at: OAM DMA's run
/// changes there, and ceres steps its DMA there (the DMA proof's mapping).
/// A read of ceres's search at that half dot comes before the step (traced:
/// gambatte's `late_sp` ROMs).
const A_H: i32 = 5;

/// `dma_phi` after the edge at half dot `h` of a line: high in A to D.
const fn dma_phi(h: i32) -> bool {
    (h - A_H).rem_euclid(8) < 4
}

/// The half dot at which ceres's search reads sprite `index`
/// (`add_object_from_index`, in `OamScanObject`: two dots a sprite from
/// `OamScanStart`). Measured, as the line timing's position of each read;
/// the state machine is not part of the proof.
const fn ceres_read_h(index: u8) -> i32 {
    4 * index as i32 - 3
}

/// The netlist's scan of a visible `line`, from the line's end to mode 3,
/// with LCDC bit 2 (`tall`), OAM DMA's run (`run`) and its clock
/// (`dma_phi`) after the edge at each half dot of the line, OAM's Ys, and
/// the Y OAM's latches hold as it starts: the sprite each slot saves.
///
/// The CPU does not address OAM, and no sprite is fetched, while it runs.
fn net_scan(
    line: u8,
    ys: &[u8; 40],
    tall: impl Fn(i32) -> bool,
    run: impl Fn(i32) -> bool,
    dma_phi: impl Fn(i32) -> bool,
    held: u8,
) -> [Option<u8>; 10] {
    // The scan of the line before has ended (or, for line 0, the one of
    // line 143): the index stays at the last sprite, and the stop's
    // flip-flops are set. The scan ends so again (asserted below): the
    // proof holds line after line.
    let count: u8 = kani::any();
    kani::assume(count <= SLOTS);
    let mut net = NetScan {
        start_scan: false,
        anel: false,
        scanning: false,
        ceno: false,
        index: 39,
        byba: true,
        doba: true,
        not_saving: kani::any(),
        count,
    };

    // OAM's two stages of latches (`oam.vhd`): the first take the Y the
    // scan reads while `wosu` is low, the second the first while the RAM is
    // precharged, as `wuvu` rises; the comparator sees the second. OAM DMA
    // hides OAM from the scan (mode 2 is off): the first latches close, and
    // the second follow them while `dma_phi` is high. The sprite store
    // latches the index as `wuvu` rises (`sprite_store.vhd`, `scanned`).
    // Without DMA, sprite i is compared in index i + 1's window, sprite 39
    // in the one after it.
    let mut latches = NetOamLatches {
        first: held,
        second: held,
    };
    let mut scanned: u8 = 39;
    // The slots, as the save strobes fill them: the sprite's index.
    let mut slots: [Option<u8>; 10] = [None; 10];
    let mut t = LINE_END_H;
    while t < LINE_H + 170 {
        t += 1;
        let h = t - LINE_H;
        let clocks = Clocks {
            rising: t & 1 != 0,
            wuvu_before: wuvu(t - 1),
            wuvu_after: wuvu(t),
            wosu_before: wosu(t - 1),
            vclk: (LINE_END_H..LINE_H).contains(&(t - 1)),
            ly: line,
        };
        let index = net.index;
        net.edge(clocks, latches.second, tall(h - 1));
        if !clocks.wuvu_before && clocks.wuvu_after {
            scanned = index;
        }
        let inputs = OamInputs {
            scanning: net.scanning,
            run: run(h),
            dma_phi: dma_phi(h),
            wuvu: wuvu(t),
            wosu: wosu(t),
            fetch_read: false,
            fetch_hold: false,
        };
        latches.settle(inputs, ys[usize::from(net.index)]);
        if let Some(slot) = net.save(wosu(t), latches.second, line, tall(h)) {
            let entry = &mut slots[usize::from(slot)];
            assert!(
                entry.is_none_or(|i| i == scanned),
                "a slot saves two sprites"
            );
            *entry = Some(scanned);
        }
    }
    assert!(
        !net.scanning && net.index == 39 && scanned == 39 && net.byba && net.doba,
        "the scan does not end"
    );
    assert!(
        net.count <= SLOTS && !net.start_scan && !net.anel && !net.ceno,
        "the scan ends elsewhere"
    );
    slots
}

/// Ceres's search of `line`, with the height it reads for each sprite
/// (`tall`), whether an OAM DMA hides OAM as it reads it (`hidden`), OAM's
/// Ys and what the OAM's latches hold as it starts: the first ten sprites
/// it finds on the line, in index order, and how many it finds.
/// (`netlist_scan_consider` shows its list keeps those.)
fn ceres_search(
    line: u8,
    ys: &[u8; 40],
    tall: impl Fn(u8) -> bool,
    hidden: impl Fn(u8) -> bool,
    held: u8,
) -> ([Option<u8>; 10], usize) {
    let mut search = ObjectSearch::default();
    search.y_bus = held;
    let mut first: [Option<u8>; 10] = [None; 10];
    let mut found = 0;
    for index in 0..40u8 {
        let read = (!hidden(index)).then_some((ys[usize::from(index)], 0));
        let (y, _) = search.latch(read);
        if on_line(y, line, tall(index)) && found < MAX_OBJECTS_PER_LINE {
            first[found] = Some(index);
            found += 1;
        }
    }
    (first, found)
}

/// A whole scan of a visible line with any Ys, and a write of LCDC bit 2 in
/// any M-cycle around it: the netlist saves the first ten sprites on the
/// line, in index order, with the heights ceres's search reads.
#[kani::proof]
#[kani::unwind(200)]
fn netlist_scan_selection() {
    let line: u8 = kani::any();
    kani::assume(line < LINES);
    let ys: [u8; 40] = kani::any();
    // LCDC bit 2 before and after the write, in the M-cycle whose access
    // is at half dot `8 * m` of the scanned line.
    let (tall_before, tall_after): (bool, bool) = (kani::any(), kani::any());
    let m: i32 = kani::any();
    kani::assume((-2..=21).contains(&m));
    let held: u8 = kani::any();

    let net_tall = |h: i32| {
        if h >= 8 * m + NET_LCDC_H {
            tall_after
        } else {
            tall_before
        }
    };
    let slots = net_scan(line, &ys, net_tall, |_| false, dma_phi, held);
    let ceres_tall = |index: u8| {
        if ceres_read_h(index) > 8 * m + CERES_LCDC_H {
            tall_after
        } else {
            tall_before
        }
    };
    let (first, found) = ceres_search(line, &ys, ceres_tall, |_| false, held);
    assert!(slots == first, "the netlist saves other sprites");
    kani::cover!(found == MAX_OBJECTS_PER_LINE, "ten sprites on the line");
    kani::cover!(found == 3, "three sprites on the line");
    kani::cover!(
        tall_before != tall_after && (1..19).contains(&m) && found == MAX_OBJECTS_PER_LINE,
        "the height changes during the scan"
    );
}

/// A whole scan of a visible line with any Ys, while an OAM DMA runs from
/// any M-cycle to any later one, with the CPU halted in any of them: the
/// netlist's comparator and ceres's search see the same Ys, the last read
/// before the DMA for the sprites it hides, and the netlist saves the first
/// ten sprites on the line that ceres's search finds. The Y OAM's latches
/// hold as the scan starts is any, the same on both.
#[kani::proof]
#[kani::unwind(200)]
fn netlist_scan_dma() {
    let line: u8 = kani::any();
    kani::assume(line < LINES);
    let ys: [u8; 40] = kani::any();
    let tall: bool = kani::any();
    let held: u8 = kani::any();
    // The DMA runs from the A edge of M-cycle `start` to that of `end`
    // (the M-cycle whose access is at half dot `8 * m` has its A edge at
    // `8 * m + A_H`), as one transfer: not restarted.
    let (start, end): (i32, i32) = (kani::any(), kani::any());
    kani::assume(-3 <= start && start < end && end <= 22);

    // The CPU halts in any of those M-cycles, and wakes in any later one:
    // `dma_phi` stays high, and the transfer pauses with its run on (a run
    // of any length covers it).
    // @fact oam-scan-during-halted-dma
    let (halt, wake): (i32, i32) = (kani::any(), kani::any());
    kani::assume(start <= halt && halt <= wake && wake <= end);
    let run = |h: i32| (8 * start + A_H..8 * end + A_H).contains(&h);
    let phi = |h: i32| dma_phi(h) || (8 * halt + A_H..8 * wake + A_H).contains(&h);
    let slots = net_scan(line, &ys, |_| tall, run, phi, held);
    // Ceres's search sees the DMA's phase after its last step before the
    // read; a step at M-cycle `m` leaves the transfer running
    // (`Dma::runs`, proved against the netlist's run in the DMA's
    // proof, a restart included) when the netlist runs after that A edge.
    // On the DMG, a halt does not change it.
    let hidden = |index: u8| {
        let step = (ceres_read_h(index) - A_H - 1).div_euclid(8);
        (start..end).contains(&step)
    };
    let (first, found) = ceres_search(line, &ys, |_| tall, hidden, held);
    assert!(slots == first, "the netlist saves other sprites");
    kani::cover!(
        (1..18).contains(&start) && found == MAX_OBJECTS_PER_LINE,
        "a DMA that starts in the scan, ten sprites"
    );
    kani::cover!(
        start < 0 && (1..18).contains(&end) && found > 0,
        "a DMA that ends in the scan"
    );
    kani::cover!(
        halt < wake && (1..18).contains(&halt) && found > 0,
        "the CPU halts during the scan"
    );
}

/// Ceres's search keeps the first ten sprites on the line too: from any list
/// of up to ten sprites, `consider` adds the sprite when it is on the line and
/// the list has room, and keeps the rest. Sprite after sprite, from the empty
/// list at the start of the scan, it keeps the ten the netlist saves.
#[kani::proof]
#[kani::unwind(12)]
fn netlist_scan_consider() {
    let mut search = ObjectSearch::default();
    search.count = kani::any();
    kani::assume(search.count <= MAX_OBJECTS_PER_LINE);
    for entry in &mut search.entries {
        *entry = Entry {
            index: kani::any(),
            x: kani::any(),
            y: kani::any(),
        };
    }
    let before = search.clone();
    let (index, x, y, on): (u8, u8, u8, bool) =
        (kani::any(), kani::any(), kani::any(), kani::any());
    // The sprites before were considered before it.
    kani::assume(
        before.entries[..before.count]
            .iter()
            .all(|e| e.index < index),
    );
    search.consider(index, x, y, on);

    let taken = on && before.count < MAX_OBJECTS_PER_LINE;
    assert!(
        search.count == before.count + usize::from(taken),
        "ceres keeps another number"
    );
    let kept = &search.entries[..search.count];
    for entry in &before.entries[..before.count] {
        assert!(
            kept.iter().any(|e| e.index == entry.index),
            "ceres drops a sprite"
        );
    }
    assert!(
        kept.iter().any(|e| e.index == index) == taken,
        "ceres keeps another sprite"
    );
}
