use crate::Model;

/// The APU revision of a model, ordered from the oldest to the newest.
///
/// Many glitches changed between revisions, so they are checked against an
/// ordering (e.g. `rev <= Revision::CgbC`). All the DMG-family models share a
/// revision, but the Game Boy Light is distinguished for a wave RAM glitch.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Revision {
    Dmg,
    Mgb,
    Cgb0,
    CgbA,
    CgbB,
    CgbC,
    CgbD,
    CgbE,
    Agb,
}

impl Revision {
    pub const fn new(model: Model) -> Self {
        match model {
            Model::Dmg0 | Model::DmgB | Model::Sgb | Model::Sgb2 => Self::Dmg,
            Model::Mgb => Self::Mgb,
            Model::Cgb0 => Self::Cgb0,
            Model::CgbA => Self::CgbA,
            Model::CgbB => Self::CgbB,
            Model::CgbC => Self::CgbC,
            Model::CgbD => Self::CgbD,
            Model::CgbE => Self::CgbE,
            Model::Agb => Self::Agb,
        }
    }

    /// CGB hardware, the AGB included.
    pub fn is_cgb(self) -> bool {
        self >= Self::Cgb0
    }

    /// The AGB mixes the channels digitally: there are no per-channel DACs.
    pub fn is_agb(self) -> bool {
        self == Self::Agb
    }

    /// The CGB-D and E share a set of glitches.
    pub fn is_cgb_de(self) -> bool {
        matches!(self, Self::CgbD | Self::CgbE)
    }
}
