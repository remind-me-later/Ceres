#[derive(Clone, Copy)]
pub enum PpuRegister {
    Lcdc,
    Stat,
    Scy,
    Scx,
    Ly,
    Lyc,
    Wy,
    Wx,
    Bgp,
    Obp0,
    Obp1,
    // cgb
    Bcps,
    Bcpd,
    Ocps,
    Ocpd,
    Opri,
}
