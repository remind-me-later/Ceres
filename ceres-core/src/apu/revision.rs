use crate::Model;

/// The APU revision of a model, ordered from the oldest to the newest.
///
/// Many glitches changed between revisions, so they are checked against an
/// ordering (e.g. `rev <= Revision::CgbC`). All the DMG-family models share a
/// revision, but the Game Boy Light is distinguished for a wave RAM glitch.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Revision {
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
    pub(super) const fn new(model: Model) -> Self {
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
    pub(super) fn is_cgb(self) -> bool {
        self >= Self::Cgb0
    }

    /// The AGB mixes the channels digitally: there are no per-channel DACs.
    pub(super) fn is_agb(self) -> bool {
        self == Self::Agb
    }

    /// A PCM register read misses some bits of a sample that changes in the
    /// same M-cycle. The CGB-C and older do it, except the CGB-A: SameSuite's
    /// `channel_1_freq_change_timing-A` reads the sample unglitched there,
    /// where the CGB-0, B and C (`-cgb0BC`) do not.
    pub(super) fn has_pcm_glitch(self) -> bool {
        self.is_cgb() && self <= Self::CgbC && self != Self::CgbA
    }

    /// The CGB-D and E share a set of glitches.
    pub(super) const fn is_cgb_de(self) -> bool {
        matches!(self, Self::CgbD | Self::CgbE)
    }
}
