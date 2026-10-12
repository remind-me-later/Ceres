use crate::{
    AudioCallback, Gb, GbBuilder, MemoryAccess, Model, PpuEventKind, Sample, WatchHit,
    timing::UNITS_PER_FRAME,
};
use alloc::{boxed::Box, vec, vec::Vec};
use core::cell::Cell;

/// Hashes the samples it gets, so two runs can be compared.
#[derive(Default)]
struct Hasher(Cell<u64>);

impl AudioCallback for Hasher {
    fn audio_sample(&self, l: Sample, r: Sample) {
        let h = self.0.get().rotate_left(5)
            ^ u64::from(l.cast_unsigned())
            ^ (u64::from(r.cast_unsigned()) << 16);
        self.0.set(h.wrapping_mul(0x0100_0000_01B3));
    }
}

/// A ROM that increments every byte of $C000-$CFFF in turn, forever:
///
/// ```text
/// $0100 ld hl, $C000
/// $0103 ld a, [hl]
/// $0104 inc a
/// $0105 ld [hl+], a
/// $0106 ld a, h
/// $0107 cp $D0
/// $0109 jr nz, $0103
/// $010B ld hl, $C000
/// $010E jr $0103
/// ```
fn rom() -> Box<[u8]> {
    const CODE: [u8; 16] = [
        0x21, 0x00, 0xC0, 0x7E, 0x3C, 0x22, 0x7C, 0xFE, 0xD0, 0x20, 0xF8, 0x21, 0x00, 0xC0, 0x18,
        0xF3,
    ];
    let mut rom = vec![0; 0x8000];
    rom[0x100..0x100 + CODE.len()].copy_from_slice(&CODE);
    rom.into_boxed_slice()
}

fn build() -> Gb<Hasher> {
    build_with(rom())
}

#[expect(clippy::expect_used, reason = "the test ROM is valid")]
fn build_with(rom: Box<[u8]>) -> Gb<Hasher> {
    GbBuilder::new(48000, Hasher::default())
        .with_model(Model::DmgB)
        .with_run_bootrom(false)
        .with_rom(rom)
        .expect("a valid ROM")
        .build()
}

/// A ROM running `code` from the entry point.
fn rom_with(code: &[u8]) -> Box<[u8]> {
    let mut rom = vec![0; 0x8000];
    rom[0x100..0x100 + code.len()].copy_from_slice(code);
    rom.into_boxed_slice()
}

/// What two runs must agree on.
fn state(gb: &Gb<Hasher>) -> (u16, u16, u16, u64, Box<[u8]>) {
    let wram = (0xC000..0xD000).map(|addr| gb.peek_mem(addr)).collect();
    (
        gb.cpu_pc(),
        gb.cpu_af(),
        gb.cpu_hl(),
        gb.apu.audio_callback().0.get(),
        wram,
    )
}

#[test]
fn steps_match_frames() {
    const FRAMES: u32 = 5;

    let mut frames = build();
    for _ in 0..FRAMES {
        frames.run_frame();
    }

    let mut steps = build();
    let mut steps_ended = 0;
    while steps_ended < FRAMES {
        steps_ended += u32::from(steps.step());
    }
    assert_eq!(state(&steps), state(&frames), "step");
    assert_eq!(
        steps.pixel_data_rgba(),
        frames.pixel_data_rgba(),
        "step: screen"
    );

    // Stopping every 1000 steps resumes the same frame.
    let mut interrupted = build();
    let mut interrupted_ended = 0;
    let mut count = 0_u32;
    while interrupted_ended < FRAMES {
        interrupted_ended += u32::from(interrupted.run_frame_until(|_| {
            count += 1;
            count.is_multiple_of(1000)
        }));
    }
    assert_eq!(state(&interrupted), state(&frames), "run_frame_until");
}

#[test]
fn watchpoints() {
    let mut gb = build();
    gb.set_watchpoint(0xC010, MemoryAccess::Write, true);
    gb.set_watchpoint(0xC020, MemoryAccess::Read, true);

    let mut hit = None;
    assert!(!gb.run_frame_until(|gb| {
        hit = gb.take_watch_hit();
        hit.is_some()
    }));
    assert_eq!(
        hit,
        Some(WatchHit {
            addr: 0xC010,
            access: MemoryAccess::Write
        })
    );
    assert_eq!(gb.cpu_pc(), 0x0106, "stops after `ld [hl+], a`");

    assert!(!gb.run_frame_until(|gb| {
        hit = gb.take_watch_hit();
        hit.is_some()
    }));
    assert_eq!(
        hit,
        Some(WatchHit {
            addr: 0xC020,
            access: MemoryAccess::Read
        })
    );
    assert_eq!(gb.cpu_pc(), 0x0104, "stops after `ld a, [hl]`");

    gb.clear_watchpoints();
    assert!(gb.run_frame_until(|gb| gb.take_watch_hit().is_some()));
}

#[test]
fn peek_and_poke() {
    let mut gb = build();
    // `inc a` becomes `dec a`, in the ROM itself.
    gb.poke_mem(0x0104, 0x3D);
    assert_eq!(gb.peek_mem(0x0104), 0x3D);
    assert_eq!(gb.rom_banks(), (0, 1), "the MBC didn't see the write");

    gb.poke_mem(0x8010, 0xAB);
    gb.poke_mem(0xFE00, 0xCD);
    assert_eq!(gb.peek_mem(0x8010), 0xAB);
    assert_eq!(gb.peek_mem(0xFE00), 0xCD);

    let mut unpatched = build();
    let before = gb.peek_mem(0xC000);
    gb.run_frame();
    unpatched.run_frame();
    let passes = unpatched.peek_mem(0xC000).wrapping_sub(before);
    assert_ne!(passes, 0, "the loop ran");
    assert_eq!(
        gb.peek_mem(0xC000),
        before.wrapping_sub(passes),
        "the patched code ran"
    );
}

#[test]
fn clock_units_count_frames() {
    let mut gb = build();
    for _ in 0..3 {
        gb.run_frame();
    }
    // Frames count the same units, and keep what's left of the last one.
    assert_eq!(
        gb.clock_units(),
        3 * u64::from(UNITS_PER_FRAME.cast_unsigned()) + u64::from(gb.units_ran.cast_unsigned())
    );
}

#[test]
fn io_log_matches_the_ppu_trace() {
    // $0100 ldh a, [LY] ; $0102 jr $0100
    let mut gb = build_with(rom_with(&[0xF0, 0x44, 0x18, 0xFC]));
    gb.set_io_log(true);
    gb.set_ppu_trace(true);
    gb.run_frame();

    let log: Vec<_> = gb
        .take_io_log()
        .into_iter()
        .filter(|access| access.addr == 0xFF44)
        .map(|access| {
            assert_eq!(access.access, MemoryAccess::Read);
            (access.value, access.line, access.half_dot)
        })
        .collect();
    let trace: Vec<_> = gb
        .take_ppu_trace()
        .into_iter()
        .filter_map(|event| match event.kind {
            PpuEventKind::LyRead(value) => Some((value, event.timing_line, event.timing_h)),
            _ => None,
        })
        .collect();
    assert!(log.len() > 1000, "LY was read all frame");
    assert_eq!(log, trace);
}

#[test]
fn timer_state() {
    // $0100 ld a, $05 ; $0102 ldh [TAC], a ; $0104 jr $0104
    let mut gb = build_with(rom_with(&[0x3E, 0x05, 0xE0, 0x07, 0x18, 0xFE]));
    for _ in 0..2000 {
        gb.step();
    }
    let timer = gb.timer_state();
    assert_eq!(timer.tac & 7, 5);
    assert_eq!(timer.tima, gb.peek_mem(0xFF05));
    assert_eq!(timer.div_counter.to_le_bytes()[1], gb.peek_mem(0xFF04));
}

#[test]
fn ppu_state() {
    let mut gb = build();
    // Where each mode starts on line 10, in half dots.
    let mut starts = [None; 4];
    let mut last_mode = None;
    for _ in 0..40_000 {
        gb.step();
        let ppu = gb.ppu_state();
        assert!(ppu.lcd_on);
        assert_eq!(ppu.mode, gb.peek_mem(0xFF41) & 3, "STAT shows the mode");
        if ppu.line == 10 && last_mode != Some(ppu.mode) {
            starts[usize::from(ppu.mode)].get_or_insert(ppu.half_dot);
        }
        last_mode = Some(ppu.mode);
        assert!((0..912).contains(&ppu.half_dot), "{ppu:?}");
    }
    assert!(starts[1].is_none(), "no VBlank on line 10: {starts:?}");
    let [Some(hblank), _, Some(scan), Some(drawing)] = starts else {
        unreachable!("every other mode on line 10: {starts:?}");
    };
    assert!(scan < drawing && drawing < hblank, "{starts:?}");
    // Mode 3 lasts 172 dots and more.
    assert!(
        (2 * 172..2 * 300).contains(&(hblank - drawing)),
        "{starts:?}"
    );
}
