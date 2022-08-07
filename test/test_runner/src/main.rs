use {
    ceres_core::{Gb, Model},
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

        let gb = Gb::new(Model::Cgb, |_, _| {}, 1);

        read_file_into(&path, gb.cartridge_rom_mut()).unwrap();

        gb.init().unwrap();

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

fn read_file_into(path: &Path, buf: &mut [u8]) -> Result<(), std::io::Error> {
    let mut f = File::open(path)?;
    let _ = f.read(buf).unwrap();
    Ok(())
}
