use {
    ceres_core::{Cartridge, Gb, Model},
    std::{
        fs::{self, File},
        io::Read,
        path::Path,
    },
};

fn main() {
    let paths = fs::read_dir("../bin/").unwrap();

    for path in paths {
        let path = path.unwrap().path();
        let rom = read_file_into(&path).unwrap();
        let cart = Cartridge::new(rom, None).unwrap();
        let mut gb = Gb::new(Model::Cgb, DummyAudio, 1, cart);

        while gb.test_running() {
            gb.run_frame();
        }

        print!("{}:\t", path.to_string_lossy());
        if gb.get_test_result() == 0 {
            println!("OK");
        } else {
            println!("FAILED");
        }
    }
}

fn read_file_into(path: &Path) -> Result<Box<[u8]>, std::io::Error> {
    let mut f = File::open(path)?;
    let metadata = f.metadata().unwrap();
    let len = metadata.len();
    let mut buf = vec![0; len as usize].into_boxed_slice();
    let _ = f.read(&mut buf).unwrap();
    Ok(buf)
}

pub struct DummyAudio;

impl ceres_core::Audio for DummyAudio {
    fn play(&mut self, _: ceres_core::Sample, _: ceres_core::Sample) {}
}
