//! Kirby's copies (`ftkirby.c`, `ftkirbyspecialn.c`): taking a copy and its hat, the copied
//! special's variables set anew, a copy's items registered, his copy's own cleanups when he's
//! hit or KO'd, and losing a copy (a hit can knock it out of him, with its star).

use gekko_fp as fp;

use super::super::super::code::unported_call;
use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::kirbycopystar;
use super::super::super::state::Running;
use super::{addr, attr, ms, u};
use crate::fpu::{mem, reg};
use crate::game::ft::FighterKind;
use crate::game::ft::registry::Hook;
use crate::game::it::ItKind;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `HitElement_Cape`.
const ELEMENT_CAPE: u32 = 10;

/// What Kirby's copy of a character runs, its entry's (`Character::kirby_copy`): the original's
/// switches on the copied fighter kind (`u` `HAT_KIND`) and its tables by kind
/// (`ftKb_Init_803C9CC8`'s hats, `ftKb_Init_803C9DD0` and `_803C9E54`'s specials). A new
/// character's copy fills its entry's.
#[derive(Clone, Copy)]
pub struct KirbyCopy {
    /// His neutral special with the copy, on the ground (`ftKb_SpecialN_Enter`) and in the air
    /// (`ftKb_SpecialAirN_Enter`); Inhale for the kinds without one.
    pub special_n: Hook,
    pub special_air_n: Hook,
    /// The copy's hat put on and taken off (`ftKb_Init_803C9CC8`'s pair for the kind, which
    /// `ftKb_SpecialN_800EEEC4` takes off); none for the kinds without a hat.
    pub hat_on: Option<Hook>,
    pub hat_off: Option<Hook>,
    /// The copy's own cleanup when he's KO'd (`ftKb_SpecialN_800F19AC`) and when he's hit
    /// (`ftKb_SpecialN_800F1A8C`).
    pub cleanup: Option<Hook>,
    pub hit_cleanup: Option<Hook>,
    /// The copy data's item entries (its `KirbyHatStruct`'s from `+0xC`) and their item kinds,
    /// which taking the copy registers (`ftKb_SpecialN_800F16D0`).
    pub items: &'static [(u32, ItKind)],
    /// Whether the copy's hat takes his jumps' other states (`ftKb_SpecialN_800F1CD8`).
    pub hat_jumps: bool,
}

/// The copy of a kind with none of its own (Kirby himself, Nana, the hands, the wireframes,
/// the Sandbag): Inhale, no hat, no cleanups or items.
pub const NO_COPY: KirbyCopy = KirbyCopy {
    special_n: |r, g| r.kb_n_enter(g),
    special_air_n: |r, g| r.kb_n_air_enter(g),
    hat_on: None,
    hat_off: None,
    cleanup: None,
    hit_cleanup: None,
    items: &[],
    hat_jumps: false,
};

/// Kirby's copy of fighter kind `kind`, its character's entry's; none for a number that's no
/// fighter kind.
pub fn copy_of(kind: i32) -> Option<&'static KirbyCopy> {
    FighterKind::from_id(kind).map(|k| &k.character().kirby_copy)
}

/// Where a neutral special shared with Kirby's copy of it finds the kind it runs as: the
/// fighter's own (Mario's or Dr. Mario's in Mario's code), or the kind Kirby copied (`u`
/// `HAT_KIND`), which his copies read where the original reads the fighter's.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum KindFrom {
    Fighter,
    Copy,
}

impl Running<'_> {
    /// The kind a shared special runs as ([`KindFrom`]).
    pub(crate) fn special_kind(&self, gobj: GObjId, from: KindFrom) -> i32 {
        let fp = self.game.fighter(gobj);
        match from {
            KindFrom::Fighter => fp.kind,
            KindFrom::Copy => fp.u_i32(u::HAT_KIND),
        }
    }
    /// Whether his copy's hat takes his jumps' other states (Captain Falcon's, Sheik's, the
    /// Ice Climbers' and Samus's), `ftKb_SpecialN_800F1CD8` (`fn_800F1CA0`).
    #[doc(alias = "ftKb_SpecialN_800F1CD8")]
    #[doc(alias = "fn_800F1CA0")]
    pub fn kb_copy_has_hat_jumps(&self, gobj: GObjId) -> bool {
        copy_of(self.game.fighter(gobj).u_i32(u::HAT_KIND)).is_some_and(|c| c.hat_jumps)
    }

    /// Kirby takes a copy of `kind`, `ftKb_SpecialN_800F1BAC`: a new copy's variables, hat and
    /// items, with its sound when he swallows it (`sfx`; the same copy again plays another);
    /// his cleanups for it.
    #[doc(alias = "ftKb_SpecialN_800F1BAC")]
    pub fn kb_copy(&mut self, gobj: GObjId, kind: i32, sfx: bool) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].u_i32(u::HAT_KIND) != kind {
            self.game.fighters[f.0].set_u_i32(u::HAT_KIND, kind);
            self.kb_reset_copy_vars(gobj);
            self.kb_add_hat(gobj, kind);
            self.kb_register_copy_items(kind);
            if sfx {
                self.fighter_play_sfx(f, 0x222F3, 0x7F, 0x40);
            }
        } else if sfx {
            self.fighter_play_sfx(f, 0x222F6, 0x7F, 0x40);
        }
        let fp = &mut self.game.fighters[f.0];
        fp.death3_cb = Some(CodeRef(addr::DEATH3));
        fp.death1_cb = Some(CodeRef(addr::DEATH1));
    }

    /// The copy's hat put on, `ftKb_Init_803C9CC8`'s first of the kind's pair.
    pub fn kb_add_hat(&mut self, gobj: GObjId, kind: i32) {
        match copy_of(kind) {
            Some(c) => {
                if let Some(on) = c.hat_on {
                    on(self, gobj);
                }
            }
            None => unported_call(&format!("the hat of copy kind {kind}")),
        }
    }

    /// A copy's items registered, `ftKb_SpecialN_800F16D0`: from the kind's copy data (its
    /// `KirbyHatStruct`'s entries from `+0xC`).
    #[doc(alias = "ftKb_SpecialN_800F16D0")]
    pub fn kb_register_copy_items(&mut self, kind: i32) {
        // The copy data's entries and their item kinds, the copied kind's.
        let items = copy_of(kind).map_or(&[][..], |c| c.items);
        if items.is_empty() {
            return;
        }
        // Giga Bowser has no copy file, so his copy's data is null and the original reads
        // through it here, a fault on a console (docs/melee-bugs.md, design call 7).
        let Some(copy) = self.game.ft.kirby.copies[kind as usize] else {
            assert_eq!(kind, FighterKind::GKoopa.id(), "a copy without its data");
            panic!(
                "Melee crashes: Kirby's copy of Giga Bowser has no copy data, and ftKb_SpecialN_800F16D0 reads its flame's article at address 0x18"
            );
        };
        for &(i, item) in items {
            let article = self.data.ptr(copy.at(0xC + 4 * i));
            super::super::super::it::item::set_char_article(self.game, article, item.id());
        }
    }

    /// The copied special's variables set anew, `ftKb_SpecialN_800F190C`: Bowser's fuel and
    /// flame, and the rest emptied.
    #[doc(alias = "ftKb_SpecialN_800F190C")]
    pub fn kb_reset_copy_vars(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fuel = self.kb_attr_f32(f, attr::KP_MAX_FUEL);
        let flame = self.kb_attr_f32(f, attr::KP_FLAME_SCALE);
        let fp = &mut self.game.fighters[f.0];
        fp.set_u_i32(0x68, 9);
        fp.set_u_i32(0x6C, 9);
        fp.u.set_gobj_at(0xC0, None);
        fp.set_u_i32(0xC4, 0);
        fp.set_u_f32(0xC8, 0.0);
        fp.set_u_i32(0xCC, 0);
        fp.u.set_gobj_at(0xD0, None);
        fp.u.set_gobj_at(0x74, None);
        fp.u.set_gobj_at(0x78, None);
        fp.u.set_gobj_at(0x7C, None);
        fp.set_u_i32(0x80, 0);
        fp.set_u_f32(0x84, fuel);
        fp.set_u_f32(0x88, flame);
        fp.set_u_f32(0x8C, 1.0);
        fp.set_u_f32(0x90, 1.0);
        fp.set_u_f32(0x94, 1.0);
        fp.u.set_gobj_at(0x98, None);
        fp.set_u_i32(0x9C, 0);
        fp.u.set_gobj_at(0xA0, None);
        fp.u.set_gobj_at(0xA4, None);
        fp.set_u_i32(0xA8, 0);
        fp.set_u_i32(0xAC, 0);
        fp.u.set_gobj_at(0xB0, None);
        fp.set_u_i32(0xB4, 0);
        fp.u.set_gobj_at(0xB8, None);
        fp.set_u_i32(0xBC, 0);
        fp.set_u_i32(0xD4, 1);
        fp.set_u_i32(0xD8, 3);
        fp.u.set_gobj_at(0xDC, None);
    }

    /// His copy's own cleanup when KO'd, `ftKb_SpecialN_800F19AC`.
    #[doc(alias = "ftKb_SpecialN_800F19AC")]
    pub fn kb_copy_cleanup(&mut self, gobj: GObjId) {
        if let Some(f) = copy_of(self.game.fighter(gobj).u_i32(u::HAT_KIND)).and_then(|c| c.cleanup) {
            f(self, gobj);
        }
    }

    /// His copy's own cleanup when hit, `ftKb_SpecialN_800F1A8C`.
    #[doc(alias = "ftKb_SpecialN_800F1A8C")]
    pub fn kb_copy_hit_cleanup(&mut self, gobj: GObjId) {
        if let Some(f) = copy_of(self.game.fighter(gobj).u_i32(u::HAT_KIND)).and_then(|c| c.hit_cleanup) {
            f(self, gobj);
        }
    }

    /// The copy's hat taken off, `ftKb_SpecialN_800EEEC4` (`ftKb_Init_803C9CC8`'s second of
    /// the kind's pair).
    #[doc(alias = "ftKb_SpecialN_800EEEC4")]
    pub fn kb_remove_hat(&mut self, gobj: GObjId, kind: i32) {
        match copy_of(kind) {
            Some(c) => {
                if let Some(off) = c.hat_off {
                    off(self, gobj);
                }
            }
            None => unported_call(&format!("the hat removal of copy kind {kind}")),
        }
    }

    /// An item inhaled let go, `ftKb_SpecialN_800F9090` (none with a copy).
    #[doc(alias = "ftKb_SpecialN_800F9090")]
    pub fn kb_release_item(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if let Some(it) = fp.target_item_gobj
            && fp.kind == FighterKind::Kirby
            && fp.u_i32(u::HAT_KIND) == FighterKind::Kirby
        {
            let capturing = matches!(
                fp.motion_id,
                ms::SPECIAL_N_CAPTURE1 | ms::SPECIAL_AIR_N_CAPTURE1
            );
            crate::game::it::kinds::kirbyspitstar::release(self, it, capturing, 0.0);
            let fp = self.game.fighter_mut(gobj);
            fp.x1A64 = None;
            fp.target_item_gobj = None;
        }
    }

    /// His copy lost, `ftKb_SpecialN_800F5D04`: its cleanup, its variables and hat, and with
    /// `star` its star knocked out of him with a sound; Kirby again.
    #[doc(alias = "ftKb_SpecialN_800F5D04")]
    pub fn kb_lose_copy(&mut self, gobj: GObjId, star: bool) {
        self.kb_copy_cleanup(gobj);
        self.kb_reset_copy_vars(gobj);
        let kind = self.game.fighter(gobj).u_i32(u::HAT_KIND);
        self.kb_remove_hat(gobj, kind);
        if kind != FighterKind::Kirby && star {
            let f = self.game.fighter_id(gobj);
            let fp = &self.game.fighters[f.0];
            let (x, y, facing) = (fp.cur_pos.x, fp.cur_pos.y, fp.facing_dir);
            let dy = reg(self.kb_attr_f32(f, attr::LOSS_STAR_Y));
            let vx = reg(self.kb_attr_f32(f, attr::LOSS_STAR_VEL_X));
            let vy = self.kb_attr_f32(f, attr::LOSS_STAR_VEL_Y);
            let pos = Vec3::new(x, mem(fp::fadds(reg(y), dy)), 0.0);
            let vel = Vec3::new(mem(fp::fmuls(vx, reg(facing))), vy, 0.0);
            kirbycopystar::spawn(self, pos, vel, facing);
            self.fighter_play_sfx(f, 0x22305, 127, 64);
        }
        self.game
            .fighter_mut(gobj)
            .set_u_i32(u::HAT_KIND, FighterKind::Kirby.id());
    }

    /// Whether a hit knocks his copy out, `ftKb_SpecialN_800F5BA4` (a damage's, unless he
    /// holds someone, the player keeps copies or the hit is a cape's): one in the
    /// attributes' odds.
    #[doc(alias = "ftKb_SpecialN_800F5BA4")]
    pub fn kb_maybe_lose_copy(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        if fp.kind != FighterKind::Kirby
            || fp.u_i32(u::HAT_KIND) == FighterKind::Kirby
            || fp.u[u::HAT_FLAGS] & 0x80 != 0
            || fp.victim_gobj.is_some()
            || fp.dmg.x1860_element == ELEMENT_CAPE
        {
            return;
        }
        let odds = gekko_fp::fctiwz(crate::fpu::reg(self.kb_attr_f32(f, attr::LOSE_COPY_ODDS)));
        if self.game.objs.rng.below(odds) == 0 {
            self.kb_lose_copy(gobj, true);
        }
    }

    /// The same for a hit he takes without knockback, `ftKb_SpecialN_800F5C34`: in one of
    /// his specials he stops it, waiting or falling.
    #[doc(alias = "ftKb_SpecialN_800F5C34")]
    pub fn kb_maybe_lose_copy_no_knockback(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        if fp.kind != FighterKind::Kirby
            || fp.u_i32(u::HAT_KIND) == FighterKind::Kirby
            || fp.u[u::HAT_FLAGS] & 0x80 != 0
            || fp.victim_gobj.is_some()
        {
            return;
        }
        let odds = gekko_fp::fctiwz(crate::fpu::reg(self.kb_attr_f32(f, attr::LOSE_COPY_ODDS)));
        if self.game.objs.rng.below(odds) != 0 {
            return;
        }
        let msid = self.game.fighters[f.0].motion_id;
        if (0x18F..0x220).contains(&msid) {
            self.damage_pick_reaction(gobj);
            self.kb_lose_copy(gobj, true);
            if self.game.fighters[f.0].ground_or_air == 0 {
                self.enter_wait(gobj);
            } else {
                self.enter_fall(gobj);
            }
            return;
        }
        self.kb_lose_copy(gobj, true);
    }
}
