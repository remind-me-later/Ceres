use std::env;
use ceres_core::{Gb, Model};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

fn main() {
	let args: Vec<String> = env::args().collect();
	if args.len() < 2 {
		println!("Invalid number of arguments.. ABORTING");
		return;
	}
	let path = &args[1];

	fn read_file_into(path: &Path, buf: &mut [u8]) -> Result<(), std::io::Error> {
		let mut f = File::open(path)?;
		let _ = f.read(buf).unwrap();
		Ok(())
	}
	read_file_into(&PathBuf::from(path), Gb::cartridge_rom_mut()).unwrap();
	let gb = Gb::new(Model::Cgb, |_, _| {return;}, 1).unwrap();
	while gb.test_running() {
		gb.run_frame();
	}
	if gb.get_test_result() == 0 {
		println!("OK!");
	}
	else {
		println!("FAILED!");
	}
}
