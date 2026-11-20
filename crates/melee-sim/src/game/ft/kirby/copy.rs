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
use crate::game::it::ItKind;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `HitElement_Cape`.
const ELEMENT_CAPE: u32 = 10;

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
        matches!(
            self.game.fighter(gobj).u_i32(u::HAT_KIND),
            2 | 7 | 10 | 11 | 13
        )
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
        match kind {
            4 | 11 | 27..=30 | 32 => {}
            13 => self.kb_samus_hat_on(gobj),
            22 => self.kb_falco_hat_on(gobj),
            0 => self.kb_mario_hat_on(gobj),
            2 => self.kb_captain_hat_on(gobj),
            17 => self.kb_luigi_hat_on(gobj),
            12 => self.kb_pikachu_hat_on(gobj),
            7 => self.kb_seak_hat_on(gobj),
            8 => self.kb_ness_hat_on(gobj),
            9 => self.kb_peach_hat_on(gobj),
            19 => self.kb_zelda_hat_on(gobj),
            24 => self.kb_gamewatch_hat_on(gobj),
            1 => self.kb_fox_hat_on(gobj),
            3 => self.kb_donkey_hat_on(gobj),
            5 | 31 => self.kb_koopa_hat_on(gobj),
            6 => self.kb_link_hat_on(gobj),
            14 => self.kb_yoshi_hat_on(gobj),
            20 => self.kb_clink_hat_on(gobj),
            10 => self.kb_popo_hat_on(gobj),
            15 => self.kb_purin_hat_on(gobj),
            16 => self.kb_mewtwo_hat_on(gobj),
            18 => self.kb_mars_hat_on(gobj),
            21 => self.kb_drmario_hat_on(gobj),
            23 => self.kb_pichu_hat_on(gobj),
            25 => self.kb_ganon_hat_on(gobj),
            26 => self.kb_emblem_hat_on(gobj),
            _ => unported_call(&format!("the hat of copy kind {kind}")),
        }
    }

    /// A copy's items registered, `ftKb_SpecialN_800F16D0`: from the kind's copy data (its
    /// `KirbyHatStruct`'s entries from `+0xC`).
    #[doc(alias = "ftKb_SpecialN_800F16D0")]
    pub fn kb_register_copy_items(&mut self, kind: i32) {
        // The copy data's entries and their item kinds, by copied kind.
        let items: &[(u32, i32)] = match kind {
            0 => &[(0, ItKind::KirbyMarioFire.id())],
            17 => &[(0, ItKind::KirbyLuigiFire.id())],
            10 => &[(0, ItKind::KirbyIceClimberIce.id())],
            9 => &[
                (0, ItKind::KirbyPeachToad.id()),
                (1, ItKind::KirbyPeachToadSpore.id()),
            ],
            1 => &[
                (0, ItKind::KirbyFoxLaser.id()),
                (1, ItKind::KirbyFoxBlaster.id()),
            ],
            22 => &[
                (3, ItKind::KirbyFalcoLaser.id()),
                (4, ItKind::KirbyFalcoBlaster.id()),
            ],
            6 => &[
                (0, ItKind::KirbyLinkArrow.id()),
                (1, ItKind::KirbyLinkBow.id()),
            ],
            20 => &[
                (0, ItKind::KirbyCLinkArrow.id()),
                (1, ItKind::KirbyCLinkBow.id()),
            ],
            16 => &[(3, ItKind::KirbyMewtwoShadowBall.id())],
            8 => &[
                (0, ItKind::KirbyNessPKFlush.id()),
                (1, ItKind::KirbyNessPKFlushExplode.id()),
            ],
            12 => &[
                (0, ItKind::KirbyPikachuTJoltGround.id()),
                (1, ItKind::KirbyPikachuTJoltAir.id()),
            ],
            23 => &[
                (0, ItKind::KirbyPichuTJoltGround.id()),
                (1, ItKind::KirbyPichuTJoltAir.id()),
            ],
            13 => &[(0, ItKind::KirbySamusCharge.id())],
            5 | 31 => &[(0, ItKind::KirbyKoopaFlame.id())],
            7 => &[
                (0, ItKind::KirbySeakNeedleThrow.id()),
                (1, ItKind::KirbySeakNeedleHeld.id()),
            ],
            21 => &[(0, ItKind::KirbyDrMarioVitamin.id())],
            24 => &[
                (5, ItKind::KirbyGameWatchChef.id()),
                (6, ItKind::KirbyGameWatchChefPan.id()),
            ],
            14 => &[(5, ItKind::KirbyYoshiEggLay.id())],
            _ => &[],
        };
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
            super::super::super::it::item::set_char_article(self.game, article, item);
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
        match self.game.fighter(gobj).u_i32(u::HAT_KIND) {
            13 => self.samus_drop_charge(gobj, &super::special_samus::KIRBY),
            1 | 22 => self.kb_fx_remove_blaster(gobj),
            8 => self.ness_pkflash_set_null(gobj, &super::special_ness::KIRBY),
            9 => self.peach_toad_on_death2(gobj, &super::special_peach::KIRBY),
            7 => self.seak_drop_needles(gobj, &super::special_seak::KIRBY),
            24 => self.kb_gw_cleanup(gobj),
            3 => self.dk_special_n_destroy_effects_plus(gobj, &super::special_donkey::KIRBY),
            6 | 20 => self.kb_lk_cleanup(gobj),
            14 => self.kb_ys_cleanup(gobj),
            10 => self.popo_remove_ice(gobj, &super::special_iceclimber::KIRBY),
            16 => self.mt_n_lose_ball(gobj, &super::special_mewtwo::KIRBY),
            _ => {}
        }
    }

    /// His copy's own cleanup when hit, `ftKb_SpecialN_800F1A8C`.
    #[doc(alias = "ftKb_SpecialN_800F1A8C")]
    pub fn kb_copy_hit_cleanup(&mut self, gobj: GObjId) {
        match self.game.fighter(gobj).u_i32(u::HAT_KIND) {
            13 => self.samus_drop_charge(gobj, &super::special_samus::KIRBY),
            1 | 22 => self.kb_fx_remove_blaster(gobj),
            8 => self.ness_pkflash_set_null(gobj, &super::special_ness::KIRBY),
            9 => self.peach_toad_on_death2(gobj, &super::special_peach::KIRBY),
            7 => self.seak_drop_needles(gobj, &super::special_seak::KIRBY),
            24 => self.kb_gw_cleanup(gobj),
            3 => self.dk_special_n_destroy_effects_plus(gobj, &super::special_donkey::KIRBY),
            6 | 20 => self.kb_lk_cleanup(gobj),
            14 => self.kb_ys_cleanup(gobj),
            10 => self.popo_remove_ice(gobj, &super::special_iceclimber::KIRBY),
            16 => self.mt_n_drop_ball(gobj, &super::special_mewtwo::KIRBY),
            _ => {}
        }
    }

    /// The copy's hat taken off, `ftKb_SpecialN_800EEEC4` (`ftKb_Init_803C9CC8`'s second of
    /// the kind's pair).
    #[doc(alias = "ftKb_SpecialN_800EEEC4")]
    pub fn kb_remove_hat(&mut self, gobj: GObjId, kind: i32) {
        match kind {
            4 | 11 | 27..=30 | 32 => {}
            13 => self.kb_samus_hat_off(gobj),
            22 => self.kb_falco_hat_off(gobj),
            0 | 1 | 2 | 17 | 8 | 9 | 14 | 10 | 21 | 25 => self.kb_head_hat_off(gobj),
            12 | 7 | 19 | 5 | 6 | 20 | 18 | 23 | 26 | 31 => self.kb_pikachu_hat_off(gobj),
            24 => self.kb_gamewatch_hat_off(gobj),
            3 => self.kb_donkey_hat_off(gobj),
            15 => self.kb_purin_hat_off(gobj),
            16 => self.kb_mewtwo_hat_off(gobj),
            _ => unported_call(&format!("the hat removal of copy kind {kind}")),
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
