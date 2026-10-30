//! Donkey Kong's character callbacks (`ftdonkey.c`): creation, spawn, the attribute copy, the
//! specials' cleanup when he's hit, the item hooks, the charged punch's glow, the knockback
//! hooks and the Hand Slap's hitboxes along the floor.

use gekko_fp as fp;

use super::super::super::code::Val;
use super::super::super::handles::FighterId;
use super::super::super::state::Running;
use super::{U_SWINGS, addr, attr};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// The cargo walks' animations (`ftDk_SM_ThrowFWalkSlow` to `Fast`, the character's
/// animations 296 to 298).
const CARGO_WALK_ANIMS: [i32; 3] = [296, 297, 298];

/// `Collide_FloorMask`.
const COLLIDE_FLOOR_MASK: u32 = 0x18000;

impl Running<'_> {
    /// Donkey Kong's creation, `ftDk_Init_OnLoad`: the cargo walks' animation lengths, which
    /// the original writes into his loaded attributes (docs/engine.md), his attributes, and
    /// the cargo states reading them (`x2222_b0`, `x2CC`).
    #[doc(alias = "ftDk_Init_OnLoad")]
    pub fn dk_on_load(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        // `lbAnim_8001E8F8(ftData_80085E50(fp, n))`: each animation goes through the second
        // buffer, which keeps the last.
        for anim in CARGO_WALK_ANIMS {
            let tree = self.second_figatree(f, anim);
            let _ = super::super::data::figatree_frames(self.data, tree);
        }
        self.push_attrs(f, attr::SIZE);
        self.dk_patch_cargo_walk_frames(f);
        let c = attr::CARGO_WALK_FRAMES;
        self.image_ext_attrs(f, &[c, c + 4, c + 8]);
        let fp = &mut self.game.fighters[f.0];
        fp.x2222_b0 = true;
        fp.x2CC = fp.dat_attrs;
    }

    /// The cargo walks' animation lengths his creation writes over his loaded attributes'
    /// `x8` to `x10`, put in the fighter's: the copies of those attributes (creation's and
    /// every later one) read them from there. They follow from his animations, so they're
    /// worked out here rather than kept.
    fn dk_patch_cargo_walk_frames(&mut self, f: FighterId) {
        for (i, anim) in CARGO_WALK_ANIMS.into_iter().enumerate() {
            let frames = self.dk_anim_frames(f, anim);
            let a = self.game.fighters[f.0]
                .dat_attrs
                .expect("a fighter without attributes");
            let off = attr::CARGO_WALK_FRAMES + 4 * i;
            self.game.fighter_attrs[a.0][off..off + 4]
                .copy_from_slice(&frames.to_bits().to_be_bytes());
        }
    }

    /// The frame count of the character's animation `anim`, as `ftData_80085E50` and
    /// `lbAnim_8001E8F8` find it, without loading it.
    fn dk_anim_frames(&self, f: FighterId, anim: i32) -> f32 {
        let fp = &self.game.fighters[f.0];
        if anim as u32 >= fp.x58C {
            return 0.0;
        }
        let entry = self.anim_entry(f, anim);
        if self.anim_aram(fp, entry) == 0 {
            return 0.0;
        }
        let tree = self.anim_tree(fp.kind, entry);
        super::super::data::figatree_frames(self.data, Some(tree))
    }

    /// `ftDk_Init_LoadSpecialAttrs`: his attributes copied again, the cargo walks' lengths
    /// with them.
    #[doc(alias = "ftDk_Init_LoadSpecialAttrs")]
    pub fn dk_load_special_attrs(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.copy_attrs(f, attr::SIZE);
        self.dk_patch_cargo_walk_frames(f);
    }

    /// The cargo walks' speed scales grown with the fighter (`ftCo_800D105C`'s end, for a
    /// fighter whose cargo attributes are its own, `x2222_b0`).
    pub(crate) fn dk_scale_cargo_walk_rates(&mut self, f: FighterId) {
        let scale = self.game.fighters[f.0].x34_scale.y;
        if scale == 1.0 {
            return;
        }
        let a = self.game.fighters[f.0]
            .x2CC
            .expect("a cargo fighter without cargo attributes");
        for i in 0..3 {
            let off = attr::CARGO_WALK_RATES + 4 * i;
            let b = &mut self.game.fighter_attrs[a.0];
            let v = f32::from_be_bytes(b[off..off + 4].try_into().unwrap());
            let v = mem(fp::fmuls(reg(v), reg(scale)));
            b[off..off + 4].copy_from_slice(&v.to_be_bytes());
        }
    }

    /// Donkey Kong's spawn, `ftDk_Init_OnDeath`: no swings stored, and his first model variant.
    #[doc(alias = "ftDk_Init_OnDeath")]
    pub fn dk_on_death(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).set_u_i32(U_SWINGS, 0);
        self.game.set_part_vis(gobj, 0, 0);
    }

    /// The specials' damage and death hook, `ftDk_Init_8010D774`: the stored swings lost and
    /// the effects gone.
    #[doc(alias = "ftDk_Init_8010D774")]
    pub fn dk_on_damage(&mut self, gobj: GObjId) {
        self.dk_special_n_destroy_effects_plus(gobj);
        self.dk_special_hi_destroy_effects(gobj);
    }

    /// Picking up an item, `ftDk_Init_OnItemPickup` (`Fighter_OnItemPickup(gobj, flag, 1,
    /// 1)`, as Fox's).
    // Ports `ftDk_Init_OnItemPickup`; aliased once a check reaches it.
    pub fn dk_on_item_pickup(&mut self, gobj: GObjId, flag: bool) {
        self.item_pickup_hand(gobj, flag, 1);
    }

    /// Hiding the held item, `ftDk_Init_OnItemInvisible`.
    // Ports `ftDk_Init_OnItemInvisible`; aliased once a check reaches it.
    pub fn dk_on_item_invisible(&mut self, gobj: GObjId) {
        self.item_invisible_hand(gobj, 1);
    }

    /// Showing the held item again, `ftDk_Init_OnItemVisible`.
    // Ports `ftDk_Init_OnItemVisible`; aliased once a check reaches it.
    pub fn dk_on_item_visible(&mut self, gobj: GObjId) {
        self.item_visible_hand(gobj, 1);
    }

    /// Dropping the item, `ftDk_Init_OnItemDrop`.
    // Ports `ftDk_Init_OnItemDrop`; aliased once a check reaches it.
    pub fn dk_on_item_drop(&mut self, gobj: GObjId, flag: bool) {
        self.item_drop_hand(gobj, flag, 1);
    }

    /// The fully charged punch's glow, `ftDk_Init_UnkMotionStates4` (`ftData_UnkMotionStates4`,
    /// run when the color overlay restarts): color animation 57 while the swings are at
    /// their most.
    #[doc(alias = "ftDk_Init_UnkMotionStates4")]
    pub fn dk_charged_glow(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let max = self.dk_attr_i32(f, attr::MAX_ARM_SWINGS);
        if self.game.fighters[f.0].u_i32(U_SWINGS) == max {
            self.start_color_anim(gobj, 57, 0);
        }
    }

    /// Entering knockback, `ftDk_Init_OnKnockbackEnter`: the hurt textures, as Sheik's.
    #[doc(alias = "ftDk_Init_OnKnockbackEnter")]
    pub fn dk_on_knockback_enter(&mut self, gobj: GObjId) {
        self.seak_on_knockback_enter(gobj);
    }

    /// `ftDk_Init_OnKnockbackExit`.
    #[doc(alias = "ftDk_Init_OnKnockbackExit")]
    pub fn dk_on_knockback_exit(&mut self, gobj: GObjId) {
        self.seak_on_knockback_exit(gobj);
    }

    /// The Hand Slap's hitboxes along the floor, `ftDk_Init_8010DB3C` (accessory 4): when the
    /// script raises its flag on the ground, each of the first four hitboxes at its spacing
    /// from the middle, ahead of him, walked there along the floor (or at his feet where the
    /// walk fails), a little above it, off any joint.
    #[doc(alias = "ftDk_Init_8010DB3C")]
    pub fn dk_hand_slap_hits(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        if !fp.take_throw_flag(3) || fp.coll_data.env_flags as u32 & COLLIDE_FLOOR_MASK == 0 {
            return;
        }
        let spacing = reg(self.dk_attr_f32(f, attr::SPECIAL_LW_SPACING));
        let reach = reg(self.dk_attr_f32(f, attr::SPECIAL_LW_REACH));
        let max_wall = self.dk_attr_f32(f, attr::SPECIAL_LW_MAX_WALL);
        let half = fp::fmuls(1.5, spacing);
        for i in 0..4 {
            let fp = &self.game.fighters[f.0];
            let ahead = fp::fmuls(reach, reg(fp.facing_dir));
            let at = f64::from(i as u32);
            let along = fp::fadds(fp::fmsubs(spacing, at, half), ahead);
            let (line, pos) = (fp.coll_data.floor.index, fp.cur_pos);
            let walk = self
                .game
                .geometry
                .as_ref()
                .expect("no collision geometry")
                .walk_floor(line, pos, mem(along), max_wall, 0.0);
            let mut v: Vec3 = match walk {
                Some(w) if w.reached => w.pos,
                _ => pos,
            };
            v.y = mem(fp::fadds(reg(v.y), 2.0));
            super::super::seak::special_s::hit_at(&mut self.game.fighters[f.0].x914[i], v);
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ON_DEATH => g.dk_on_death(gobj),
        addr::UNK_MOTION_STATES4 => g.dk_charged_glow(gobj),
        addr::ON_DAMAGE => g.dk_on_damage(gobj),
        addr::ON_ITEM_INVISIBLE => g.dk_on_item_invisible(gobj),
        addr::ON_ITEM_VISIBLE => g.dk_on_item_visible(gobj),
        addr::ON_KNOCKBACK_ENTER => g.dk_on_knockback_enter(gobj),
        addr::ON_KNOCKBACK_EXIT => g.dk_on_knockback_exit(gobj),
        addr::HAND_SLAP_HITS => g.dk_hand_slap_hits(gobj),
        _ => return false,
    }
    true
}

pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let gobj = |i: usize| args[i].gobj().expect("a null fighter");
    Some(match f {
        addr::ON_ITEM_PICKUP => {
            r.dk_on_item_pickup(gobj(0), args[1].int() != 0);
            Val::None
        }
        addr::ON_ITEM_DROP => {
            r.dk_on_item_drop(gobj(0), args[1].int() != 0);
            Val::None
        }
        _ => return None,
    })
}
