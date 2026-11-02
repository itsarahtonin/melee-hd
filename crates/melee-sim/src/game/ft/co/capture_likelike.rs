//! Swallowed by a Like Like (`ftCo_800C78B0.c`): the Like Like's grab (`it::kinds::likelike`)
//! takes a fighter inside, invisible and following it, which mashes against a timer that grows
//! with its percent and its player's handicap and standing; the timer out, the Like Like spits
//! it back out, else it chews on it (its damage a bite at a time) and spits it out when its
//! chewing ends. A hit on the fighter or its KO makes the Like Like let go.

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::it::{self, kinds::likelike};
use super::super::super::state::Running;
use super::ids::ms;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `ftCo_CaptureLikelike_Anim`, `_IASA`, `_Phys` and `_Coll`.
const ANIM: u32 = 0x800C_7A58;
const IASA: u32 = 0x800C_7AD4;
const PHYS: u32 = 0x800C_7AD8;
const COLL: u32 = 0x800C_7ADC;
/// The hold's callbacks: `fn_800C7890` (KO'd), `ftCo_800C7A30` (hit), `fn_800C7AE0` (follows
/// the Like Like).
const DEATH: u32 = 0x800C_7890;
const HIT: u32 = 0x800C_7A30;
const FOLLOW: u32 = 0x800C_7AE0;

/// `ftCommon_MotionVars_capturelikelike::x0`: the Like Like; `x4`: whether its timer ran out
/// (read as `mushroom.x4`).
const LIKELIKE: usize = 0x0;
const SPAT: usize = 0x4;

impl Running<'_> {
    fn likelike_of(&self, gobj: GObjId) -> GObjId {
        self.game
            .fighter(gobj)
            .mv
            .gobj_at(LIKELIKE)
            .expect("a fighter in a Like Like without one")
    }

    /// Swallowed by a Like Like, `ftCo_800C78B0` (the Like Like's grab's callback): what it held
    /// let go, the state, its animation, the hold's callbacks, the Like Like kept, still; the
    /// mash timer `ftCommonData` x740, plus x744 per handicap below x748, plus x74C per place
    /// behind first (from x750), plus x754 per percent, without the mash's shake
    /// (`ftCommon_InitGrab`); hidden behind it (`x1988` 2), invisible, grabs passed over.
    #[doc(alias = "ftCo_800C78B0")]
    pub fn enter_capture_likelike(&mut self, gobj: GObjId, owner: GObjId) {
        // `ftCo_ReleaseItemAndVictim`.
        self.damage_hooks(gobj);
        self.drop_heavy_item(gobj);
        self.release_victim(gobj);
        self.change_motion_state(gobj, ms::CAPTURE_LIKELIKE, 0x1080, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.accessory1_cb = Some(CodeRef(FOLLOW));
        fp.death2_cb = Some(CodeRef(DEATH));
        fp.take_dmg_cb = Some(CodeRef(HIT));
        fp.x221D_b5 = true;
        fp.mv.set_gobj_at(LIKELIKE, Some(owner));
        fp.set_mv_i32(SPAT, 0);
        self.stop_dead(gobj);
        let slot = i32::from(self.game.fighters[f.0].player_idx);
        let rank = self.player_rank(slot).wrapping_add(1);
        let c = self.common();
        let (x740, x744, x748, x74c, x750, x754) = (
            reg(c.f32(0x740)),
            reg(c.f32(0x744)),
            reg(c.f32(0x748)),
            reg(c.f32(0x74C)),
            reg(c.f32(0x750)),
            reg(c.f32(0x754)),
        );
        let place = fp::fmuls(x74c, fp::fsubs(x750, fp::frsp(f64::from(rank))));
        let handicap = self.game.player_handicap(slot);
        let hand = fp::fsubs(x748, fp::frsp(f64::from(handicap)));
        let t = fp::fadds(fp::fmadds(x744, hand, x740), place);
        let fp = &mut self.game.fighters[f.0];
        let timer = fp::fmadds(reg(fp.dmg.x1830_percent), x754, t);
        // `ftCommon_InitGrab(fp, 0, timer)`.
        fp.grab_timer = mem(timer);
        fp.x1A51 = 0;
        fp.x1A50 = 0;
        fp.x2224_b6 = false;
        fp.x1988 = 2;
        fp.invisible = true;
        // `ftCommon_8007E2F4`.
        fp.x1A6A = 0x1FF;
    }

    /// The held fighter hit or KO'd, `ftCo_800C7A30` and `fn_800C7890`: the Like Like lets go
    /// (`it_802DBA68`).
    #[doc(alias = "ftCo_800C7A30")]
    #[doc(alias = "fn_800C7890")]
    pub fn capture_likelike_hit(&mut self, gobj: GObjId) {
        let ll = self.likelike_of(gobj);
        likelike::let_go(self, ll);
    }

    /// `ftCo_CaptureLikelike_Anim`: the timer runs down by x758 a frame and by x75C a mash
    /// (`ftCommon_GrabMash`); once it's out the Like Like spits the fighter out
    /// (`it_802DB9F4`), once.
    #[doc(alias = "ftCo_CaptureLikelike_Anim")]
    pub fn co_capture_likelike_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let c = self.common();
        let (step, mash) = (reg(c.f32(0x758)), c.f32(0x75C));
        let fp = &mut self.game.fighters[f.0];
        fp.grab_timer = mem(fp::fsubs(reg(fp.grab_timer), step));
        let _ = self.grab_mash(gobj, mash);
        if self.game.fighters[f.0].mv_i32(SPAT) == 0 && self.game.fighters[f.0].grab_timer <= 0.0 {
            let ll = self.likelike_of(gobj);
            likelike::spit(self, ll);
            self.game.fighters[f.0].set_mv_i32(SPAT, 1);
        }
    }

    /// `ftCo_CaptureLikelike_IASA`, `_Phys` and `_Coll`: nothing.
    #[doc(alias = "ftCo_CaptureLikelike_IASA")]
    #[doc(alias = "ftCo_CaptureLikelike_Phys")]
    #[doc(alias = "ftCo_CaptureLikelike_Coll")]
    pub fn co_capture_likelike_nothing(&mut self, _gobj: GObjId) {}

    /// Inside the Like Like, the fighter is where it is, `fn_800C7AE0` (`it_8026B294`).
    #[doc(alias = "fn_800C7AE0")]
    pub fn capture_likelike_follow(&mut self, gobj: GObjId) {
        let ll = self.likelike_of(gobj);
        let pos = it::lib26b1::position(self.game, ll);
        self.game.fighter_mut(gobj).cur_pos = pos;
    }

    /// Spat out of a Like Like, `ftCo_800C7B0C`: its collision moved from `from` (the Like
    /// Like's middle) to `at` (its mouth), the fighter where that leaves it, the hit `desc`
    /// (`lbColl_80008D30`) with no attacker (`ftColl_800788D4`), its knockback at no ratio
    /// (`ftColl_80079EA8`) at `angle` degrees (mirrored for a fighter facing right), its damage
    /// taken, then hitstun.
    #[doc(alias = "ftCo_800C7B0C")]
    pub fn likelike_spat_out(
        &mut self,
        gobj: GObjId,
        at: Vec3,
        from: Vec3,
        desc: melee_data::archive::DataRef,
        angle: f32,
    ) {
        let f = self.game.fighter_id(gobj);
        self.fighter_coll_placed(f, from, at, |c, cd| c.air_frame(cd, 6, 0));
        let hit = super::super::attack::device_capsule(self.data, desc);
        self.clear_attacker(gobj);
        let mut angle = reg(angle);
        if angle < 0.0 {
            angle = fp::fadds(angle, 360.0);
        }
        let angle = if reg(self.game.fighters[f.0].facing_dir) < 0.0 {
            angle
        } else {
            fp::fsubs(180.0, angle)
        };
        let weight = self.game.fighters[f.0].co_attrs.weight;
        let kb = self.knockback_of(f, &hit, hit.unk_count, 1.0, 1.0, 1.0, weight);
        let fp = &mut self.game.fighters[f.0];
        fp.dmg.kb_applied = mem(kb);
        fp.dmg.x1848_kb_angle = fp::fctiwz(angle);
        fp.dmg.facing_dir_1 = fp.facing_dir;
        fp.dmg.x184c_damaged_hurtbox = 0;
        fp.dmg.x1854_collpos = fp.cur_pos;
        fp.dmg.x1860_element = hit.element;
        self.take_damage_coins(f, reg(hit.damage));
        self.calc_knockback(f);
        self.damage_reaction(gobj, 0.0);
    }

    /// A Like Like's bite, `ftCo_800C7C60`: `damage` taken (`Fighter_TakeDamage_8006CC7C`).
    #[doc(alias = "ftCo_800C7C60")]
    pub fn likelike_bite(&mut self, gobj: GObjId, damage: i32) {
        let f = self.game.fighter_id(gobj);
        self.take_damage(f, fp::frsp(f64::from(damage)));
    }
}

/// Runs one of the Like Like hold's callbacks, if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        ANIM => g.co_capture_likelike_anim(gobj),
        IASA | PHYS | COLL => g.co_capture_likelike_nothing(gobj),
        HIT | DEATH => g.capture_likelike_hit(gobj),
        FOLLOW => g.capture_likelike_follow(gobj),
        _ => return false,
    }
    true
}
