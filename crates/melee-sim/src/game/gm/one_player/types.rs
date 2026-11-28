//! A single-player mode's saved choice (`OnePlayerChoice`, the game data's `gmm_x0_528_t`):
//! Classic's, Adventure's and All-Star's fighter, stocks, color, level and name tag, and how
//! far its run got. Hand-written as [`super::super::types`] are.

use crate::game::ft::CharacterKind;
use crate::game::mirror::Narrow;

/// The decomp's `gmm_x0_528_t`.
#[doc(alias("gmm_x0_528_t"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "gmm_x0_528_t", size = 0x6)]
pub struct OnePlayerChoice {
    #[at(0x0)]
    pub c_kind: Narrow<CharacterKind, i8>,
    #[at(0x1)]
    pub stocks: u8,
    #[at(0x2)]
    pub color: u8,
    #[at(0x3)]
    pub cpu_level: u8,
    #[at(0x4)]
    pub nametag: u8,
    /// The run's round to resume.
    #[at(0x5)]
    #[doc(alias("x5"))]
    pub round: u8,
}
