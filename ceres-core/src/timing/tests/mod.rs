mod div;
mod serial;
mod timer;

use crate::{GbBuilder, Model, test_util::setup_gb};

type _Gb = crate::Gb<crate::test_util::DummyAudio>;

fn setup_cgb() -> crate::Gb<crate::test_util::DummyAudio> {
    GbBuilder::new(44100, crate::test_util::DummyAudio)
        .with_model(Model::CgbE)
        .build()
}

fn _advance_to_ly(gb: &mut crate::Gb<crate::test_util::DummyAudio>, ly: u8) {
    for _ in 0..10_000_000 {
        if gb.ppu.read_ly() == ly {
            return;
        }
        gb.advance_dots(1);
    }
    panic!("LY={} never reached", ly);
}

fn _advance_to_mode(gb: &mut crate::Gb<crate::test_util::DummyAudio>, mode: u8) {
    for _ in 0..10_000_000 {
        if gb.ppu.read_stat() & 0x03 == mode {
            return;
        }
        gb.advance_dots(1);
    }
    panic!("Mode={} never reached", mode);
}

fn check_timer_period(tac: u8, period: i32, init_tima: u8) {
    let mut gb = setup_gb();
    gb.write_mem(0xFF06, init_tima); // TMA
    gb.write_mem(0xFF05, init_tima); // TIMA
    gb.write_mem(0xFF07, tac); // TAC

    // Wait for first increment
    let mut dots = 0;
    while gb.read_mem(0xFF05) == init_tima {
        gb.advance_dots(4);
        dots += 4;
        if dots > 40000 {
            panic!("Timer never incremented");
        }
    }

    let start_dots = dots;
    // Wait for second increment
    while gb.read_mem(0xFF05) == init_tima + 1 {
        gb.advance_dots(4);
        dots += 4;
    }

    let elapsed = dots - start_dots;
    assert_eq!(
        elapsed, period,
        "Timer period mismatch for TAC=0x{:02X}: expected {}, got {}",
        tac, period, elapsed
    );
}
