#![cfg(test)]

use {
    ceres_core::{Cartridge, Gb, Model},
    crc::{Crc, CRC_64_ECMA_182},
    std::{
        fs::File,
        io::Read,
        path::{Path, PathBuf},
    },
};

pub const CRC64: Crc<u64> = Crc::<u64>::new(&CRC_64_ECMA_182);

pub struct DummyAudio;

impl ceres_core::Audio for DummyAudio {
    fn play(&mut self, _: ceres_core::Sample, _: ceres_core::Sample) {}
}

fn secs_to_frames(secs: u32) -> u32 {
    secs * 60
}

fn read_file_into(path: &Path) -> Result<Box<[u8]>, std::io::Error> {
    let mut f = File::open(path)?;
    let metadata = f.metadata().unwrap();
    let len = metadata.len();
    let mut buf = vec![0; len as usize].into_boxed_slice();
    let _ = f.read(&mut buf).unwrap();
    Ok(buf)
}

fn run_test(test_rom_path: &str, run_for_secs: u32, crc_expect: u64) {
    let path = PathBuf::from(test_rom_path);

    let rom = read_file_into(&path).unwrap();
    let cart = Cartridge::new(rom, None).unwrap();
    let mut gb = Gb::new(Model::Cgb, DummyAudio, 1, cart);

    for _ in 0..secs_to_frames(run_for_secs) {
        gb.run_frame();
    }

    let obtained = CRC64.checksum(gb.pixel_data_rgb());

    assert_eq!(obtained, crc_expect);
}

#[test]
fn blargg_cpu_instrs() {
    run_test("blargg/cpu_instrs/cpu_instrs.gb", 31, 15902498174298407339);
}

#[test]
fn blargg_instr_timing() {
    run_test(
        "blargg/instr_timing/instr_timing.gb",
        1,
        15573883656270665917,
    );
}
