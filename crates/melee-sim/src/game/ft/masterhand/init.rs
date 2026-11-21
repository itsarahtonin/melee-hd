//! The hands' character callbacks (`ftmasterhand.c`, `ftcrazyhand.c`): creation and their
//! attributes.

use super::super::super::state::Running;
use super::{Hand, addr, mv, u};
use crate::hsd::gobj::GObjId;

impl Running<'_> {
    /// His creation, `ftMh_Init_OnLoad` and `ftCh_Init_OnLoad`: his attributes pushed, his
    /// articles registered, no normal motion, no knockback, his place, no lasers, no hums,
    /// the nearest opponent, his first attack, and the first attack timer at computer level 1.
    #[doc(alias = "ftMh_Init_OnLoad")]
    #[doc(alias = "ftCh_Init_OnLoad")]
    pub fn hand_on_load(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        let d = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without data");
        let items = self.data.ptr(d.at(0x48)).expect("ftData::x48_items");
        self.push_attrs(f, h.attrs_size);
        self.boss_check_slot(gobj);
        for (i, &kind) in h.items.iter().enumerate() {
            let article = self.data.ptr(items.at(4 * i as u32));
            super::super::super::it::item::set_char_article(self.game, article, kind);
        }
        let pos = h.point(self, f, h.home);
        let fp = &mut self.game.fighters[f.0];
        fp.no_normal_motion = true;
        fp.spawn_in_air = true;
        fp.no_kb = true;
        fp.ungrabbable = true;
        fp.blast_zone_off = true;
        fp.no_magnifier = true;
        fp.cur_pos = pos;
        for off in h.lasers {
            fp.set_mv_i32(off, 0);
        }
        for off in h.hums {
            fp.set_mv_i32(off, -1);
        }
        fp.set_mv_f32(mv::X1C, 0.0);
        fp.set_mv_i32(mv::HOLDING, 0);
        let cur = fp.cur_pos;
        let target = self.boss_nearest(gobj, cur);
        let fp = &mut self.game.fighters[f.0];
        fp.u.set_gobj_at(u::TARGET, target);
        fp.set_u_f32(u::TIMER_SPEED, 1.0);
        fp.set_u_i32(u::ROW, 0);
        fp.set_u_i32(u::NEXT_ATTACK, h.first_attack);
        fp.set_u_i32(u::SINCE_WALK, 0);
        fp.cpu.level = 1;
        self.hand_reset_timer(gobj, h);
    }

    /// `ftMh_Init_LoadSpecialAttrs` and `ftCh_Init_LoadSpecialAttrs`: his attributes copied
    /// again.
    #[doc(alias = "ftMh_Init_LoadSpecialAttrs")]
    #[doc(alias = "ftCh_Init_LoadSpecialAttrs")]
    pub fn hand_load_special_attrs(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        self.copy_attrs(f, h.attrs_size);
    }
}

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::INIT_ON_DEATH => g.hand_nothing(gobj),
        _ => return false,
    }
    true
}
