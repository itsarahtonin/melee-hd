//! Swinging a held item (`ftswing.c`, `ft_0CD1.c`, `ft_0CD3.c`): A on the ground with a swing
//! item in hand starts the item's jab, tilt, smash or dash swing, a motion state per item and
//! swing (`lbl_803C6D70`) at the speed `PlCo.dat` gives (`Fighter_804D654C`). The Beam
//! Sword's swings are ported; the other swing items' (the bat, the parasol, the fan, the star
//! rod and the lip's stick) aren't.

use gekko_fp as fp;

use super::super::super::code::{self, Val, unported_call};
use super::super::super::it::lib26b1;
use super::super::super::state::Running;
use super::PAD_A;
use super::kind_callbacks::{ON_ITEM_DROP_EXT, ON_ITEM_PICKUP_EXT};
use crate::fpu::reg;
use crate::game::handles::CodeRef;
use crate::hsd::gobj::GObjId;

/// `swing.x0` (A still held since the swing began) and `swing.x4` (which swing) in `mv`.
const MV_HELD: usize = 0x0;
const MV_SWING: usize = 0x4;

/// The dash swing (`swing.x4`), which slides with the ground's friction.
const SWING_DASH: i32 = 4;

/// `ft_800CD31C`, the swings' hit hook.
pub const TAKE_DMG: u32 = 0x800C_D31C;

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
            0 => self.enter_sword_swing(gobj, msid, swing, speed),
            1 => unported_call("ftCo_800CD418 (the bat's swing)"),
            2 => unported_call("ftCo_800CD4E0 (the parasol's swing)"),
            3 => unported_call("ftCo_800CD5A8 (the fan's swing)"),
            4 => unported_call("ftCo_800CD82C (the star rod's swing)"),
            _ => unported_call("ftCo_800CDAB4 (the lip's stick's swing)"),
        }
    }

    /// A Beam Sword swing, `ftCo_800CD350`: a hit puts the item back in the hand's grip
    /// (`ft_800CD31C`).
    #[doc(alias = "ftCo_800CD350")]
    pub fn enter_sword_swing(&mut self, gobj: GObjId, msid: i32, swing: i32, speed: f32) {
        self.enter_item_swing(gobj, msid, 0, swing, speed);
        self.game.fighter_mut(gobj).take_dmg_cb = Some(CodeRef(TAKE_DMG));
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
    /// (`ftData_OnItemDropExt`).
    #[doc(alias = "ftCommon_8007E79C")]
    pub fn item_drop_hook(&mut self, gobj: GObjId, flag: bool) {
        let kind = self.game.fighter(gobj).kind;
        let hook = ON_ITEM_DROP_EXT[kind as usize];
        if hook != 0 {
            code::call(
                self,
                hook,
                &[Val::GObj(Some(gobj)), Val::Int(i32::from(flag))],
            );
        }
    }

    /// The character's hook for an item taken in hand, `ftCommon_8007E7E4`
    /// (`ftData_OnItemPickupExt`).
    #[doc(alias = "ftCommon_8007E7E4")]
    pub fn item_pickup_hook(&mut self, gobj: GObjId, flag: bool) {
        let kind = self.game.fighter(gobj).kind;
        let hook = ON_ITEM_PICKUP_EXT[kind as usize];
        if hook != 0 {
            code::call(
                self,
                hook,
                &[Val::GObj(Some(gobj)), Val::Int(i32::from(flag))],
            );
        }
    }

    /// The swings' animation, `ftCo_SwordSwing_Anim` (`ftCo_800CD1BC`): at the end it waits,
    /// the item back in hand.
    #[doc(alias = "ftCo_SwordSwing_Anim")]
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

    /// The swings' collision, `ftCo_SwordSwing_Coll` (`ftCo_800CD2C4` with `ft_800CD31C`):
    /// off the ground the item is back in hand and the fighter falls.
    #[doc(alias = "ftCo_SwordSwing_Coll")]
    #[doc(alias = "ftCo_800CD2C4")]
    pub fn co_swing_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.swing_item_back(gobj);
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
