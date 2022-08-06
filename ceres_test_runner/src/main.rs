use {
    ceres_core::{Gb, Model},
    std::{
        env,
        fs::File,
        io::Read,
        path::{Path, PathBuf},
        process::ExitCode,
    },
};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Invalid number of arguments.. ABORTING");
        return ExitCode::FAILURE;
    }

    let path = &args[1];

    let gb = Gb::new(Model::Cgb, |_, _| {}, 1);

    read_file_into(&PathBuf::from(path), gb.cartridge_rom_mut()).unwrap();

    gb.init().unwrap();

    while gb.test_running() {
        gb.run_frame();
    }

    if gb.get_test_result() == 0 {
        println!("OK!");
        ExitCode::SUCCESS
    } else {
        println!("FAILED!");
        ExitCode::FAILURE
    }
}

fn read_file_into(path: &Path, buf: &mut [u8]) -> Result<(), std::io::Error> {
    let mut f = File::open(path)?;
    let _ = f.read(buf).unwrap();
    Ok(())
}
