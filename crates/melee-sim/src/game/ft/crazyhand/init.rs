//! Crazy Hand's character callbacks (`ftcrazyhand.c`): creation and his attributes, and his
//! empty hit hook (`ftcrazyhandwait10.c`'s `ftCh_Init_80156014`).

use super::super::super::state::Running;
use super::super::masterhand::u;
use super::{ITEMS, addr, attr, ms, mv};
use crate::hsd::gobj::GObjId;

impl Running<'_> {
    /// His spawn, `ftCh_Init_OnDeath`: nothing.
    #[doc(alias = "ftCh_Init_OnDeath")]
    pub fn ch_on_death(&mut self, _gobj: GObjId) {}

    /// A hit's hook for him (`Fighter_8006D...`'s no-reaction branch), `ftCh_Init_80156014`:
    /// nothing.
    #[doc(alias = "ftCh_Init_80156014")]
    pub fn ch_hit_nop(&mut self, _gobj: GObjId) {}

    /// His creation, `ftCh_Init_OnLoad`: his attributes pushed, his lasers, bullets and bombs
    /// registered, no normal motion, no knockback, his place, the nearest opponent, and the
    /// first attack timer at computer level 1.
    #[doc(alias = "ftCh_Init_OnLoad")]
    pub fn ch_on_load(&mut self, gobj: GObjId) {
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
        let (x, y) = (self.ch_da_f32(f, attr::X18), self.ch_da_f32(f, attr::X1C));
        let fp = &mut self.game.fighters[f.0];
        fp.no_normal_motion = true;
        fp.x2229_b6 = true;
        fp.no_kb = true;
        fp.x222A_b0 = true;
        fp.x222A_b1 = true;
        fp.x2229_b3 = true;
        fp.cur_pos.x = x;
        fp.cur_pos.y = y;
        fp.cur_pos.z = 0.0;
        for off in [0x28, 0x2C, 0x30, 0x34] {
            fp.set_mv_i32(off, 0);
        }
        for off in mv::HUMS {
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
        fp.set_u_i32(u::NEXT_ATTACK, ms::DAMAGE2);
        fp.set_u_i32(u::SINCE_WALK, 0);
        fp.cpu.level = 1;
        self.ch_reset_timer(gobj);
    }

    /// The next attack's timer from his computer level (`ftBossLib_8015BD24` with `x0`, `x8`
    /// and `x4`).
    pub(crate) fn ch_reset_timer(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let (level, speed) = (fp.cpu.level, fp.u_f32(u::TIMER_SPEED));
        let (base, hi, lo) = (
            self.ch_da_i32(f, attr::X0),
            self.ch_da_i32(f, attr::X8),
            self.ch_da_i32(f, attr::X4),
        );
        let t = self.boss_attack_timer(level, speed, base, hi, lo);
        self.game.fighters[f.0].set_u_f32(u::TIMER, t);
    }

    /// `ftCh_Init_LoadSpecialAttrs`: his attributes copied again.
    #[doc(alias = "ftCh_Init_LoadSpecialAttrs")]
    pub fn ch_load_special_attrs(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.copy_attrs(f, attr::SIZE);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::INIT_ON_DEATH => g.ch_on_death(gobj),
        addr::INIT_80156014 => g.ch_hit_nop(gobj),
        _ => return false,
    }
    true
}
