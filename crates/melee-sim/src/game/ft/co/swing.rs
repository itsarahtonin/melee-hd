//! Swinging a held item (`ftswing.c`, `ft_0CD1.c`, `ft_0CD3.c`, `ftstarrodswing.c`,
//! `ftlipstickswing.c`): A on the ground with a swing item in hand starts the item's jab, tilt,
//! smash or dash swing, a motion state per item and swing (`lbl_803C6D70`) at the speed
//! `PlCo.dat` gives (`Fighter_804D654C`). The Beam Sword's, the bat's and the parasol's swings
//! share one set of states' code; the fan grows while it hits and plays its own swing; the
//! star rod and the lip's stick spawn their stars and spores on the script's flag.

use gekko_fp as fp;

use super::super::super::code::{self, Val};
use super::super::super::ef::{Arg, SpawnKind};
use super::super::super::it::kinds::{LIPSTICK, STARROD, harisen, lipstick, starrod};
use super::super::super::it::lib26b1;
use super::super::super::state::Running;
use super::PAD_A;
use super::kind_callbacks::{ON_ITEM_DROP, ON_ITEM_PICKUP};
use crate::fpu::reg;
use crate::game::handles::CodeRef;
use crate::hsd::gobj::GObjId;
use crate::lb::joint::world_position;
use crate::vec::Vec3;

/// `swing.x0` (A still held since the swing began) and `swing.x4` (which swing) in `mv`.
const MV_HELD: usize = 0x0;
const MV_SWING: usize = 0x4;

/// The dash swing (`swing.x4`), which slides with the ground's friction.
const SWING_DASH: i32 = 4;

/// `ft_800CD31C`, the swings' hit hook.
pub const TAKE_DMG: u32 = 0x800C_D31C;
/// The fan's hit hooks: hit (`ft_800CD604`) and hitting (`ft_800CD65C`).
pub const FAN_TAKE_DMG: u32 = 0x800C_D604;
pub const FAN_DEAL_DMG: u32 = 0x800C_D65C;

/// `swing.x8` in `mv`: the frames the fan stays grown after a hit (a float).
const MV_FAN_GROWN: usize = 0x8;

/// `ftCommonData` x5A0: the frames the fan stays grown.
const CD_FAN_GROWN: u32 = 0x5A0;

/// The swings' state callbacks: the animations, interrupts, physics and collisions of the
/// sword's, the bat's, the parasol's, the star rod's and the lip's stick's swings, and the
/// fan's interrupt and physics, all the shared ones (`ftCo_800CD1BC` and the others); the
/// fan's animation and collision are its own.
pub const SWING_ANIMS: [u32; 5] = [
    0x800C_D390,
    0x800C_D458,
    0x800C_D520,
    0x800C_D88C,
    0x800C_DB14,
];
pub const SWING_IASAS: [u32; 6] = [
    0x800C_D3B0,
    0x800C_D478,
    0x800C_D540,
    0x800C_D7C4,
    0x800C_D8AC,
    0x800C_DB34,
];
pub const SWING_PHYSES: [u32; 6] = [
    0x800C_D3D0,
    0x800C_D498,
    0x800C_D560,
    0x800C_D7E4,
    0x800C_D8CC,
    0x800C_DB54,
];
pub const SWING_COLLS: [u32; 5] = [
    0x800C_D3F0,
    0x800C_D4B8,
    0x800C_D580,
    0x800C_D8EC,
    0x800C_DB74,
];
pub const FAN_ANIM: u32 = 0x800C_D6A0;
pub const FAN_COLL: u32 = 0x800C_D804;
/// The star rod's and the lip's stick's swing accessories (`ft_800CD914`, `ft_800CDB9C`).
pub const STAR_ROD_HAND: u32 = 0x800C_D914;
pub const LIPSTICK_HAND: u32 = 0x800C_DB9C;

/// The script's swing sound when it asks for none (`cmd_vars[0]`).
const NO_SWING_SFX: u32 = 540_000;

/// `ftLoadCommonData::x8`: each swing item's swing speeds, five to an item.
const SWING_SPEEDS: u32 = 0x8;

/// Each swing item's jab, tilt, smash, second smash and dash swing states (`lbl_803C6D70`).
const SWING_STATES: [[i32; 5]; 6] = [
    [120, 121, 122, -1, 123],
    [124, 125, 126, -1, 127],
    [128, 129, 130, -1, 131],
    [132, 133, 134, -1, 135],
    [136, 137, 138, -1, 139],
    [140, 141, 142, -1, 143],
];

/// `Ft_Kind_Captain`, the one fighter with a second smash swing.
const KIND_CAPTAIN: i32 = 2;

/// `ftCommonData`'s dash swing friction factor.
const CD_X420: u32 = 0x420;

/// Which swing item `kind` is, `fn_800CCEC4`: the sword, the bat, the parasol, the fan, the
/// star rod and the lip's stick.
#[doc(alias = "fn_800CCEC4")]
pub fn swing_type(kind: i32) -> i32 {
    match kind {
        12 => 0,
        11 => 1,
        13 => 2,
        24 => 3,
        22 => 4,
        23 => 5,
        _ => panic!("ftswing.c:82: ellegal swing item!!!"),
    }
}

impl Running<'_> {
    /// Swings the held item (`swing`: 0 jab, 1 tilt, 2 smash, 3 the second smash, 4 dash),
    /// `ftCo_Attack_800CCF58`: a jab or tilt picks up an item instead when there's one.
    #[doc(alias = "ftCo_Attack_800CCF58")]
    pub fn enter_swing(&mut self, gobj: GObjId, swing: i32) {
        if matches!(swing, 0 | 1) && self.pick_up_item_check(gobj) {
            return;
        }
        let fp = self.game.fighter(gobj);
        let item = fp.item_gobj.expect("ftswing.c:124: fp->item_gobj != NULL");
        let ty = swing_type(lib26b1::kind(self.game, item));
        // `get_anim_id`.
        let msid = if swing != 3 {
            SWING_STATES[ty as usize][swing as usize]
        } else if fp.kind == KIND_CAPTAIN {
            ty.wrapping_add(341)
        } else {
            panic!("ftswing.c:97: don't have swing42 motion!!!");
        };
        let speeds = self
            .data
            .ptr(
                self.game
                    .ft
                    .load_common
                    .expect("PlCo.dat isn't loaded")
                    .at(SWING_SPEEDS),
            )
            .expect("no swing speeds");
        let speed = self.data.f32(speeds.at(4 * (5 * ty + swing) as u32));
        match ty {
            0 | 1 => self.enter_sword_swing(gobj, msid, swing, speed),
            2 => self.enter_parasol_swing(gobj, msid, swing, speed),
            3 => self.enter_fan_swing(gobj, msid, swing, speed),
            4 => self.enter_star_rod_swing(gobj, msid, swing, speed),
            _ => self.enter_lipstick_swing(gobj, msid, swing, speed),
        }
    }

    /// A star rod swing, `ftCo_800CD82C`: the script's variables hold the swing's sound and
    /// which star, and the star leaves on its flag (`ft_800CD914`).
    #[doc(alias = "ftCo_800CD82C")]
    pub fn enter_star_rod_swing(&mut self, gobj: GObjId, msid: i32, swing: i32, speed: f32) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = NO_SWING_SFX;
        fp.cmd_vars[1] = 0;
        self.enter_item_swing(gobj, msid, 0, swing, speed);
        let fp = self.game.fighter_mut(gobj);
        fp.accessory4_cb = Some(CodeRef(STAR_ROD_HAND));
        fp.take_dmg_cb = Some(CodeRef(TAKE_DMG));
    }

    /// A lip's stick swing, `ftCo_800CDAB4`: as the star rod's, puffing spores
    /// (`ft_800CDB9C`).
    #[doc(alias = "ftCo_800CDAB4")]
    pub fn enter_lipstick_swing(&mut self, gobj: GObjId, msid: i32, swing: i32, speed: f32) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = NO_SWING_SFX;
        fp.cmd_vars[1] = 0;
        self.enter_item_swing(gobj, msid, 0, swing, speed);
        let fp = self.game.fighter_mut(gobj);
        fp.accessory4_cb = Some(CodeRef(LIPSTICK_HAND));
        fp.take_dmg_cb = Some(CodeRef(TAKE_DMG));
    }

    /// The swing's sound the script asked for (an empty item's own instead), and where on
    /// the hand a shot leaves from, for the star rod's and the lip's stick's accessories.
    fn shooting_swing_hand(&mut self, gobj: GObjId, empty_sfx: i32, offset: Vec3) -> Option<Vec3> {
        let f = self.game.fighter_id(gobj);
        let it = self.game.fighters[f.0].item_gobj?;
        let sfx = self.game.fighters[f.0].cmd_vars[0];
        if sfx != NO_SWING_SFX {
            let id = if lib26b1::is_empty_shooter(self.game, it) {
                empty_sfx
            } else {
                sfx as i32
            };
            self.fighter_play_sfx(f, id, 127, 64);
            self.game.fighters[f.0].cmd_vars[0] = NO_SWING_SFX;
        }
        if !self.game.fighters[f.0].take_throw_flag(0) {
            return None;
        }
        let hand = i32::from(
            self.data
                .u8(self.ft_data_x8(&self.game.fighters[f.0]).at(0x10)),
        );
        let parts = self.game.fighters[f.0].parts.expect("parts");
        let joint = self.game.fighter_parts[parts.0][hand as usize].joint;
        Some(world_position(
            &mut self.game.objs,
            self.data,
            joint,
            Some(offset),
        ))
    }

    /// The star rod swing's accessory, `ft_800CD914`: the swing's sound, and on the script's
    /// flag a star from the hand with its sparkle, or with no charges left a puff of smoke.
    #[doc(alias = "ft_800CD914")]
    pub fn star_rod_hand(&mut self, gobj: GObjId) {
        let Some(it) = self.game.fighter(gobj).item_gobj else {
            return;
        };
        assert_eq!(
            lib26b1::kind(self.game, it),
            STARROD,
            "ftstarrodswing.c:100: itGetKind(fp->item_gobj) == It_Kind_StarRod"
        );
        let offset = starrod::star_offset(self, it);
        let Some(pos) = self.shooting_swing_hand(gobj, 234, offset) else {
            return;
        };
        let facing = self.game.fighter(gobj).facing_dir;
        if !lib26b1::is_empty_shooter(self.game, it) {
            self.fighter_effect(
                gobj,
                SpawnKind::PosOffsetParam,
                1070,
                None,
                &[Arg::Vec(pos), Arg::Float(facing)],
            );
            let big = self.game.fighter(gobj).cmd_vars[1] as i32;
            starrod::fire(self, it, pos, facing, big);
        } else {
            self.fighter_effect(
                gobj,
                SpawnKind::PosOffsetParam,
                1032,
                None,
                &[Arg::Vec(pos), Arg::Float(0.0)],
            );
        }
    }

    /// The lip's stick swing's accessory, `ft_800CDB9C`: the swing's sound, and on the
    /// script's flag spores from the hand, or with no charges left a puff of smoke.
    #[doc(alias = "ft_800CDB9C")]
    pub fn lipstick_hand(&mut self, gobj: GObjId) {
        let it = self.game.fighter(gobj).item_gobj;
        assert!(
            it.is_some_and(|it| lib26b1::kind(self.game, it) == LIPSTICK),
            "ftlipstickswing.c:99: itGetKind(fp->item_gobj) == It_Kind_LipStick"
        );
        let Some(it) = it else {
            return;
        };
        let offset = lipstick::spore_offset(self, it);
        let Some(pos) = self.shooting_swing_hand(gobj, 288, offset) else {
            return;
        };
        if !lib26b1::is_empty_shooter(self.game, it) {
            let fp = self.game.fighter(gobj);
            let (facing, kind) = (fp.facing_dir, fp.cmd_vars[1] as i32);
            lipstick::puff(self, it, pos, facing, kind);
        } else {
            self.fighter_effect(
                gobj,
                SpawnKind::PosOffsetParam,
                1032,
                None,
                &[Arg::Vec(pos), Arg::Float(0.0)],
            );
        }
    }

    /// A Beam Sword or bat swing, `ftCo_800CD350` (`ftCo_800CD418`, the bat's, is the same):
    /// a hit puts the item back in the hand's grip (`ft_800CD31C`).
    #[doc(alias = "ftCo_800CD350")]
    #[doc(alias = "ftCo_800CD418")]
    pub fn enter_sword_swing(&mut self, gobj: GObjId, msid: i32, swing: i32, speed: f32) {
        self.enter_item_swing(gobj, msid, 0, swing, speed);
        self.game.fighter_mut(gobj).take_dmg_cb = Some(CodeRef(TAKE_DMG));
    }

    /// A parasol swing, `ftCo_800CD4E0`: as the sword's, keeping the item's visibility
    /// (`Ft_MF_SkipItemVis`).
    #[doc(alias = "ftCo_800CD4E0")]
    pub fn enter_parasol_swing(&mut self, gobj: GObjId, msid: i32, swing: i32, speed: f32) {
        self.enter_item_swing(gobj, msid, 0x400, swing, speed);
        self.game.fighter_mut(gobj).take_dmg_cb = Some(CodeRef(TAKE_DMG));
    }

    /// A fan swing, `ftCo_800CD5A8`: the script's variable says when the fan plays its swing,
    /// a hit it lands grows it (`ft_800CD65C`), and a hit taken shrinks it back
    /// (`ft_800CD604`).
    #[doc(alias = "ftCo_800CD5A8")]
    pub fn enter_fan_swing(&mut self, gobj: GObjId, msid: i32, swing: i32, speed: f32) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.set_mv_f32(MV_FAN_GROWN, 0.0);
        self.enter_item_swing(gobj, msid, 0, swing, speed);
        let fp = self.game.fighter_mut(gobj);
        fp.deal_dmg_cb = Some(CodeRef(FAN_DEAL_DMG));
        fp.take_dmg_cb = Some(CodeRef(FAN_TAKE_DMG));
    }

    /// The fan's hit hook, `ft_800CD604`: back to its size and its dash swing's state, and
    /// back in hand.
    #[doc(alias = "ft_800CD604")]
    pub fn fan_swing_hit(&mut self, gobj: GObjId) {
        if let Some(it) = self.game.fighter(gobj).item_gobj {
            harisen::shrink(self, it);
            harisen::swing_dash(self, it);
            self.item_pickup_hook(gobj, true);
        }
    }

    /// The fan's hitting hook, `ft_800CD65C`: grown, for `ftCommonData` x5A0 frames.
    #[doc(alias = "ft_800CD65C")]
    pub fn fan_swing_hits(&mut self, gobj: GObjId) {
        if let Some(it) = self.game.fighter(gobj).item_gobj {
            harisen::grow(self, it);
            let frames = self.common().f32(CD_FAN_GROWN);
            self.game
                .fighter_mut(gobj)
                .set_mv_f32(MV_FAN_GROWN, reg(frames));
        }
    }

    /// `ftCo_HarisenSwing_Anim`: a grown fan shrinks back when its frames run out or the
    /// swing ends; the script's variable plays the fan's swing for this swing (1) or its dash
    /// swing (2); then the shared end.
    #[doc(alias = "ftCo_HarisenSwing_Anim")]
    pub fn co_fan_swing_anim(&mut self, gobj: GObjId) {
        if let Some(it) = self.game.fighter(gobj).item_gobj {
            let grown = reg(self.game.fighter(gobj).mv_f32(MV_FAN_GROWN));
            if grown > 0.0 {
                let left = fp::fsubs(grown, 1.0);
                self.game.fighter_mut(gobj).set_mv_f32(MV_FAN_GROWN, left);
                if left <= 0.0 || !self.anim_frames_remaining(gobj) {
                    harisen::shrink(self, it);
                }
            }
            let fp = self.game.fighter(gobj);
            if fp.cmd_vars[0] == 1 {
                let speed = fp.frame_speed_mul;
                match fp.mv_i32(MV_SWING) {
                    0 => harisen::swing(self, it, 3, speed),
                    1 | 4 => harisen::swing(self, it, 4, speed),
                    2 | 3 => harisen::swing(self, it, 5, speed),
                    _ => {}
                }
                self.game.fighter_mut(gobj).cmd_vars[0] = 0;
            }
            if self.game.fighter(gobj).cmd_vars[0] == 2 {
                harisen::swing_dash(self, it);
                self.game.fighter_mut(gobj).cmd_vars[0] = 0;
            }
        }
        self.co_swing_anim(gobj);
    }

    /// `ftCo_HarisenSwing_Coll`: off the ground the fan shrinks back, back in hand, and the
    /// fighter falls.
    #[doc(alias = "ftCo_HarisenSwing_Coll")]
    pub fn co_fan_swing_coll(&mut self, gobj: GObjId) {
        self.swing_coll_then(gobj, |g, gobj| g.fan_swing_hit(gobj));
    }

    /// Enters a swing's state, `ftCo_800CD140`: A held, which swing it is, and the
    /// character's hook for the item leaving its hand (`ftCommon_8007E79C`).
    #[doc(alias = "ftCo_800CD140")]
    pub fn enter_item_swing(
        &mut self,
        gobj: GObjId,
        msid: i32,
        flags: u32,
        swing: i32,
        speed: f32,
    ) {
        self.game.fighter_mut(gobj).x2210 = [0; 4];
        self.change_motion_state(gobj, msid, flags, 0.0, speed, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(MV_HELD, 1);
        fp.set_mv_i32(MV_SWING, swing);
        self.item_drop_hook(gobj, true);
    }

    /// The character's hook for its held item leaving its hand, `ftCommon_8007E79C`
    /// (`ftData_OnItemDrop`).
    #[doc(alias = "ftCommon_8007E79C")]
    pub fn item_drop_hook(&mut self, gobj: GObjId, flag: bool) {
        let kind = self.game.fighter(gobj).kind;
        let hook = ON_ITEM_DROP[kind as usize];
        if hook != 0 {
            code::call(
                self,
                hook,
                &[Val::GObj(Some(gobj)), Val::Int(i32::from(flag))],
            );
        }
    }

    /// The character's hook for an item taken in hand, `ftCommon_8007E7E4`
    /// (`ftData_OnItemPickup`).
    #[doc(alias = "ftCommon_8007E7E4")]
    pub fn item_pickup_hook(&mut self, gobj: GObjId, flag: bool) {
        let kind = self.game.fighter(gobj).kind;
        let hook = ON_ITEM_PICKUP[kind as usize];
        if hook != 0 {
            code::call(
                self,
                hook,
                &[Val::GObj(Some(gobj)), Val::Int(i32::from(flag))],
            );
        }
    }

    /// The swings' animation, `ftCo_SwordSwing_Anim` (`ftCo_800CD1BC`, and the other items'):
    /// at the end it waits, the item back in hand.
    #[doc(alias = "ftCo_SwordSwing_Anim")]
    #[doc(alias = "ftCo_BatSwing_Anim")]
    #[doc(alias = "ftCo_ParasolSwing_Anim")]
    #[doc(alias = "ftCo_StarRodSwing_Anim")]
    #[doc(alias = "ftCo_LipstickSwing_Anim")]
    #[doc(alias = "ftCo_800CD1BC")]
    pub fn co_swing_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
            self.item_pickup_hook(gobj, true);
        }
    }

    /// The swings' interrupt, `ftCo_SwordSwing_IASA` (`ftCo_800CD204`): letting go of A ends
    /// the hold; with A held through, the script's flag swings the second smash.
    #[doc(alias = "ftCo_SwordSwing_IASA")]
    #[doc(alias = "ftCo_BatSwing_IASA")]
    #[doc(alias = "ftCo_ParasolSwing_IASA")]
    #[doc(alias = "ftCo_HarisenSwing_IASA")]
    #[doc(alias = "ftCo_StarRodSwing_IASA")]
    #[doc(alias = "ftCo_LipstickSwing_IASA")]
    #[doc(alias = "ftCo_800CD204")]
    pub fn co_swing_iasa(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.input.held_buttons[0] & PAD_A == 0 {
            fp.set_mv_i32(MV_HELD, 0);
        }
        if fp.take_throw_flag(3) && fp.mv_i32(MV_HELD) != 0 {
            self.enter_swing(gobj, 3);
        }
    }

    /// The swings' physics, `ftCo_SwordSwing_Phys` (`ftCo_800CD278`): the ground's friction,
    /// a dash swing's scaled.
    #[doc(alias = "ftCo_SwordSwing_Phys")]
    #[doc(alias = "ftCo_BatSwing_Phys")]
    #[doc(alias = "ftCo_ParasolSwing_Phys")]
    #[doc(alias = "ftCo_HarisenSwing_Phys")]
    #[doc(alias = "ftCo_StarRodSwing_Phys")]
    #[doc(alias = "ftCo_LipstickSwing_Phys")]
    #[doc(alias = "ftCo_800CD278")]
    pub fn co_swing_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if fp.mv_i32(MV_SWING) != SWING_DASH {
            self.ground_friction_or_anim(gobj);
        } else {
            let friction = fp::fmuls(
                reg(self.common().f32(CD_X420)),
                reg(fp.co_attrs.ground_friction),
            );
            let facing = reg(fp.facing_dir);
            self.ground_anim_motion(gobj, friction, facing);
        }
    }

    /// The swings' collision, `ftCo_SwordSwing_Coll` (and the other items', `ftCo_800CD2C4`
    /// with `ft_800CD31C`): off the ground the item is back in hand and the fighter falls.
    #[doc(alias = "ftCo_SwordSwing_Coll")]
    #[doc(alias = "ftCo_BatSwing_Coll")]
    #[doc(alias = "ftCo_ParasolSwing_Coll")]
    #[doc(alias = "ftCo_StarRodSwing_Coll")]
    #[doc(alias = "ftCo_LipstickSwing_Coll")]
    pub fn co_swing_coll(&mut self, gobj: GObjId) {
        self.swing_coll_then(gobj, |g, gobj| g.swing_item_back(gobj));
    }

    /// A swing's collision, `ftCo_800CD2C4`: off the ground, `cb`, then falling.
    #[doc(alias = "ftCo_800CD2C4")]
    pub fn swing_coll_then(&mut self, gobj: GObjId, cb: super::super::coll::Cb) {
        if !self.ground_coll_slide(gobj) {
            cb(self, gobj);
            self.enter_fall(gobj);
        }
    }

    /// The item back in hand, if one's held, `ft_800CD31C` (the swings' hit hook too).
    #[doc(alias = "ft_800CD31C")]
    pub fn swing_item_back(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).item_gobj.is_some() {
            self.item_pickup_hook(gobj, true);
        }
    }
}
