use ceres_core::{AudioCallback, GbBuilder, Model, Sample};
use ceres_test_runner::load_test_rom;
pub struct NoAudio;
impl AudioCallback for NoAudio { fn audio_sample(&self, _l: Sample, _r: Sample) {} }
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let model = match a[2].as_str() { "dmg" => Model::DmgB, "cgbc" => Model::CgbC, _ => Model::CgbE };
    let rom = load_test_rom(&a[1]).unwrap();
    let mut gb = GbBuilder::new(48000, NoAudio).with_model(model).with_run_bootrom(true)
        .with_rom(rom.into_boxed_slice()).unwrap().build();
    for f in 0..a[3].parse::<u32>().unwrap() {
        gb.run_frame();
        if f % 10 == 0 { println!("frame {f} pc={:04x}", gb.cpu_pc()); }
    }
}
