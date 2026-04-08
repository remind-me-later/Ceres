mod dma;
mod speed_switch;

use crate::{GbBuilder, Model, test_util::DummyAudio};

type Gb = crate::Gb<DummyAudio>;

fn setup_cgb() -> Gb {
    let mut gb = GbBuilder::new(44100, DummyAudio)
        .with_model(Model::CgbE)
        .build();
    gb.write_mem(0xFF50, 0x01);
    gb.write_mem(0xFF40, 0x00);
    gb
}

fn _run_gdma(gb: &mut Gb, src: u16, dst_vram_offset: u16, len_blocks: u8) {
    gb.write_mem(0xFF51, (src >> 8) as u8);
    gb.write_mem(0xFF52, src as u8);
    gb.write_mem(0xFF53, (dst_vram_offset >> 8) as u8);
    gb.write_mem(0xFF54, dst_vram_offset as u8);
    gb.write_mem(0xFF55, len_blocks - 1);
    gb.run_hdma();
}
