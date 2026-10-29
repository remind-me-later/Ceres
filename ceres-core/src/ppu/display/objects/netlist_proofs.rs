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
//! height from `i = 2 * m + 1` on. A save never sees half a write. `netlist_scan_consider` proves a step of ceres's search,
//! which keeps a sprite when it is on the line and the list has room:
//! sprite after sprite, it keeps the same ten.
//!
//! What it does not cover yet: the half dot of each of ceres's reads, which
//! is measured (its state machine is outside the proof); the Y the netlist
//! reads from OAM (`oam.vhd`, OAM DMA's hold of it, against ceres's
//! `y_bus`); the scan of line 0 after the LCD turns on; and the order of the
//! sprites for the fetch (the sprite store's and render's).

use super::{
    Entry, MAX_OBJECTS_PER_LINE, ObjectSearch,
    netlist::{Clocks, NetScan, SLOTS},
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

/// The half dot at which ceres's search reads sprite `index`
/// (`add_object_from_index`, in `OamScanObject`: two dots a sprite from
/// `OamScanStart`). Measured, as the line timing's position of each read;
/// the state machine is not part of the proof.
const fn ceres_read_h(index: u8) -> i32 {
    4 * index as i32 - 3
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
    let net_tall = |t: i32| {
        if t - LINE_H >= 8 * m + NET_LCDC_H {
            tall_after
        } else {
            tall_before
        }
    };

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

    // The sprite whose Y the comparator sees, and whose index a save
    // stores: as `wuvu` rises, OAM's second latches take the Y read in the
    // window before (`oam.vhd`, `precharge`) and the sprite store latches
    // that window's index (`sprite_store.vhd`, `scanned`). Sprite i is
    // compared in index i + 1's window, sprite 39 in the one after it.
    let mut scanned: u8 = 39;
    // The slots, as the save strobes fill them: the sprite's index.
    let mut slots: [Option<u8>; 10] = [None; 10];
    let mut t = LINE_END_H;
    while t < LINE_H + 170 {
        t += 1;
        let clocks = Clocks {
            rising: t & 1 != 0,
            wuvu_before: wuvu(t - 1),
            wuvu_after: wuvu(t),
            wosu_before: wosu(t - 1),
            vclk: (LINE_END_H..LINE_H).contains(&(t - 1)),
            ly: line,
        };
        let index = net.index;
        net.edge(clocks, ys[usize::from(scanned)], net_tall(t - 1));
        if !clocks.wuvu_before && clocks.wuvu_after {
            scanned = index;
        }
        if let Some(slot) = net.save(wosu(t), ys[usize::from(scanned)], line, net_tall(t)) {
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

    // The first ten sprites on the line, in index order, as ceres's search
    // sees them.
    let mut first: [Option<u8>; 10] = [None; 10];
    let mut found = 0;
    for index in 0..40u8 {
        let tall = if ceres_read_h(index) > 8 * m + CERES_LCDC_H {
            tall_after
        } else {
            tall_before
        };
        if on_line(ys[usize::from(index)], line, tall) && found < MAX_OBJECTS_PER_LINE {
            first[found] = Some(index);
            found += 1;
        }
    }
    assert!(slots == first, "the netlist saves other sprites");
    kani::cover!(found == MAX_OBJECTS_PER_LINE, "ten sprites on the line");
    kani::cover!(found == 3, "three sprites on the line");
    kani::cover!(
        tall_before != tall_after && (1..19).contains(&m) && found == MAX_OBJECTS_PER_LINE,
        "the height changes during the scan"
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
