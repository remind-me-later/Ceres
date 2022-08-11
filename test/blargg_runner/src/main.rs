use {
    ceres_core::{Cartridge, Gb, Model},
    crc::{Crc, CRC_64_ECMA_182},
    std::{fs::File, io::Read, path::Path, process::ExitCode},
};

// Consts

pub const CRC64: Crc<u64> = Crc::<u64>::new(&CRC_64_ECMA_182);

const TESTS: [BlarggTest; 4] = [
    BlarggTest {
        name: r"cpu_instrs.gb",
        rom_path: r"test/blargg_runner/blargg/cpu_instrs/cpu_instrs.gb",
        duration_secs: 31,
        crc_expect: 15902498174298407339,
    },
    BlarggTest {
        name: r"mem_timing.gb",
        rom_path: r"test/blargg_runner/blargg/mem_timing/mem_timing.gb",
        duration_secs: 3,
        crc_expect: 3467152117817621442,
    },
    BlarggTest {
        name: r"instr_timing.gb",
        rom_path: r"test/blargg_runner/blargg/instr_timing/instr_timing.gb",
        duration_secs: 1,
        crc_expect: 15573883656270665917,
    },
    BlarggTest {
        name: r"halt_bug.gb",
        rom_path: r"test/blargg_runner/blargg/halt_bug.gb",
        duration_secs: 2,
        crc_expect: 12606407190118406814,
    },
];

struct BlarggTest<'a> {
    name: &'a str,
    rom_path: &'a str,
    duration_secs: u32,
    crc_expect: u64,
}

impl<'a> BlarggTest<'a> {
    fn run(&self) -> Result<(), ()> {
        let path = Path::new(self.rom_path);
        let rom = read_file_into(path).unwrap();
        let cart = Cartridge::new(rom, None).unwrap();
        let mut gb = Gb::new(Model::Cgb, DummyAudio, 1, cart);

        for _ in 0..secs_to_frames(self.duration_secs) {
            gb.run_frame();
        }

        let crc_got = CRC64.checksum(gb.pixel_data_rgb());

        if crc_got == self.crc_expect {
            println!("{}.. ok", self.name);
            Ok(())
        } else {
            println!(
                "{}.. FAILED:\n\texpected: {}, got: {}",
                self.name, self.crc_expect, crc_got
            );
            Err(())
        }
    }
}

fn secs_to_frames(secs: u32) -> u32 {
    secs * 60
}

fn read_file_into(path: &Path) -> Result<Box<[u8]>, std::io::Error> {
    let mut f = File::open(path)?;
    let len = f.metadata().unwrap().len();
    let mut buf = vec![0; len as usize].into_boxed_slice();
    let _ = f.read(&mut buf).unwrap();
    Ok(buf)
}

fn main() -> ExitCode {
    if TESTS.iter().map(BlarggTest::run).any(|e| e.is_err()) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

// Dummy impls
pub struct DummyAudio;

impl ceres_core::Audio for DummyAudio {
    fn play(&mut self, _: ceres_core::Sample, _: ceres_core::Sample) {}
}
