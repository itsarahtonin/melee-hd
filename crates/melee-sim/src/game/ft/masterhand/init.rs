//! Master Hand's character callbacks (`ftmasterhand.c`): creation and his attributes.

use super::super::super::state::Running;
use super::{ITEMS, addr, attr, ms, mv, u};
use crate::hsd::gobj::GObjId;

impl Running<'_> {
    /// His spawn, `ftMh_Init_OnDeath`: nothing.
    #[doc(alias = "ftMh_Init_OnDeath")]
    pub fn mh_on_death(&mut self, _gobj: GObjId) {}

    /// His creation, `ftMh_Init_OnLoad`: his attributes pushed, his lasers and bullets
    /// registered, no normal motion, no knockback, his place, the nearest opponent, and the
    /// first attack timer at computer level 1.
    #[doc(alias = "ftMh_Init_OnLoad")]
    pub fn mh_on_load(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let d = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without data");
        let items = self.data.ptr(d.at(0x48)).expect("ftData::x48_items");
        self.push_attrs(f, attr::SIZE);
        self.boss_check_slot(gobj);
        for (i, kind) in ITEMS.into_iter().enumerate() {
            let article = self.data.ptr(items.at(4 * i as u32));
            super::super::super::it::item::set_char_article(self.game, article, kind);
        }
        let pos = self.mh_da_vec2(f, attr::X30);
        let fp = &mut self.game.fighters[f.0];
        fp.no_normal_motion = true;
        fp.x2229_b6 = true;
        fp.no_kb = true;
        fp.x222A_b0 = true;
        fp.x222A_b1 = true;
        fp.x2229_b3 = true;
        fp.cur_pos = pos;
        for off in [mv::X34, mv::X38, mv::X3C, mv::X40] {
            fp.set_mv_i32(off, 0);
        }
        for off in [mv::X28, mv::X2C, mv::X30] {
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
        fp.set_u_i32(u::NEXT_ATTACK, ms::SWEEP_LOOP);
        fp.set_u_i32(u::SINCE_WALK, 0);
        fp.cpu.level = 1;
        self.mh_reset_timer(gobj);
    }

    /// The next attack's timer from his computer level (`ftBossLib_8015BD24` with
    /// `x18`, `x20` and `x1C`).
    pub(crate) fn mh_reset_timer(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let (level, speed) = (fp.cpu.level, fp.u_f32(u::TIMER_SPEED));
        let (base, hi, lo) = (
            self.mh_da_i32(f, attr::X18),
            self.mh_da_i32(f, attr::X20),
            self.mh_da_i32(f, attr::X1C),
        );
        let t = self.boss_attack_timer(level, speed, base, hi, lo);
        self.game.fighters[f.0].set_u_f32(u::TIMER, t);
    }

    /// `ftMh_Init_LoadSpecialAttrs`: his attributes copied again.
    #[doc(alias = "ftMh_Init_LoadSpecialAttrs")]
    pub fn mh_load_special_attrs(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.copy_attrs(f, attr::SIZE);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::INIT_ON_DEATH => g.mh_on_death(gobj),
        _ => return false,
    }
    true
}
