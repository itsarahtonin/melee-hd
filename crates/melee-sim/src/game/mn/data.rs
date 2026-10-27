//! What the Data menu's screens share: their statics in one place and the dispatch of their
//! processes. The screens: the special messages (`mninfo.c`), the bonuses
//! (`mninfobonus.c`) and the other records (`mncount.c`).

use super::super::state::Running;
use crate::hsd::gobj::GObjId;

/// The Data menu's screens' statics and user data.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct DataMenus {
    pub info: super::info::Info,
    pub bonus: super::info_bonus::InfoBonus,
    pub count: super::count::Count,
    pub diagram: super::diagram::Diagram,
    pub diagram2: super::diagram2::Diagram2,
    pub diagram3: super::diagram3::Diagram3,
}

/// `powi`: `base` to the `exp`, by repeated multiplication (0 for a base of 0 or a negative
/// power).
#[doc(alias = "powi")]
pub fn pow_int(base: i32, exp: i32) -> i32 {
    if base == 0 || exp < 0 {
        return 0;
    }
    (0..exp).fold(1i32, |r, _| r.wrapping_mul(base))
}

/// `mn_GetDigitAt`: the decimal digit `i` places up of `n` as a signed word.
#[doc(alias = "mn_GetDigitAt")]
pub fn digit_at(n: i32, i: i32) -> i32 {
    n.wrapping_div(pow_int(10, i)).wrapping_rem(10)
}

/// `mn_8022EA78`: `n`'s last `len` decimal digits, zero-padded.
#[doc(alias = "mn_8022EA78")]
pub fn digits(len: i32, n: u32) -> Vec<u8> {
    (0..len)
        .rev()
        .map(|i| (digit_at(n as i32, i) + i32::from(b'0')) as u8)
        .collect()
}

/// `mn_GetDigitCount`: how many decimal digits `n` has, 1 for 0, at most 11.
#[doc(alias = "mn_GetDigitCount")]
pub fn digit_count(n: i32) -> i32 {
    if n == 0 {
        return 1;
    }
    let mut count = 0;
    while n.wrapping_div(pow_int(10, count)) != 0 {
        count += 1;
        if count >= 11 {
            break;
        }
    }
    count
}

impl Running<'_> {
    /// Runs a Data screen's process.
    pub fn data_menus_proc(&mut self, gobj: GObjId, f: u32) -> bool {
        use super::{
            count, diagram as dg, diagram2 as d2, diagram3 as d3, info, info_bonus as bonus,
        };
        match f {
            info::INPUT => self.info_input(gobj),
            info::IN => self.info_in(gobj),
            info::IDLE => self.info_idle(gobj),
            info::GONE => self.info_gone(gobj),
            bonus::INPUT => self.bonus_input(gobj),
            bonus::MODEL => self.bonus_model(gobj),
            count::INPUT => self.count_input(gobj),
            count::IN => self.count_in(gobj),
            count::IDLE => self.count_idle(gobj),
            count::GONE => self.count_gone(gobj),
            dg::INPUT => self.diagram_input(gobj),
            dg::POPUP_INPUT => self.diagram_popup_input(gobj),
            dg::ON_FRAME => self.diagram_on_frame(gobj),
            dg::EXIT => self.diagram_exit(gobj),
            dg::CURSOR => self.diagram_cursor_proc(gobj),
            dg::POPUP_ANIM => self.diagram_popup_anim(gobj),
            d2::INPUT => self.d2_input(gobj),
            d2::THINK => self.d2_think(gobj),
            d2::OUT => self.d2_out(gobj),
            d3::INPUT => self.d3_input(gobj),
            d3::THINK => self.d3_think(gobj),
            d3::OUT => self.d3_out(gobj),

            _ => return false,
        }
        true
    }
}
