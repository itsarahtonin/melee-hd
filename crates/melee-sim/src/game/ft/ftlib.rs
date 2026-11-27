//! What other code asks of fighters (`ftlib.c`).

use super::super::state::{Game, Running};
use crate::game::ft::CpuKind;
use crate::game::ft::types::SmashState;
use crate::game::pl::player::PlayerKind;
use crate::hsd::gobj::GObjId;
use crate::mp::coll::GroundOrAir;

impl Running<'_> {
    /// The kinds of the items a container the fighter opened gave, for its player's records,
    /// `ftLib_800874CC`.
    #[doc(alias = "ftLib_800874CC")]
    pub fn record_items_got(&mut self, gobj: GObjId, kinds: &[i32]) {
        let slot = i32::from(self.game.fighter(gobj).player_idx);
        self.count_items_got(slot, kinds);
    }

    /// Whether two GObjs are the same player's fighters, `ftLib_IsSamePlayer`.
    #[doc(alias = "ftLib_IsSamePlayer")]
    pub fn is_same_player(&self, a: Option<GObjId>, b: Option<GObjId>) -> bool {
        let (Some(a), Some(b)) = (a, b) else {
            return false;
        };
        a == b || self.game.fighter(a).player_idx == self.game.fighter(b).player_idx
    }

    /// Whether the fighter holds a heavy item (a crate), `ft_800878BC`.
    #[doc(alias = "ft_800878BC")]
    pub fn ft_holds_heavy(&self, gobj: GObjId) -> bool {
        self.game
            .fighter(gobj)
            .item_gobj
            .is_some_and(|it| crate::game::it::lib26b1::is_heavy(self.game, self.data, it))
    }

    /// Whether a computer plays the fighter, `ftCo_IsCpuControlled`.
    #[doc(alias = "ftCo_IsCpuControlled")]
    pub fn is_cpu_controlled(&self, fp: &super::super::structs::Fighter) -> bool {
        let kind = self
            .game
            .player_kind(i32::from(fp.player_idx), i32::from(fp.is_sub_fighter));
        kind == PlayerKind::Cpu && fp.cpu.kind != CpuKind::Manual
    }
}

/// `ftCo_MS_Rebirth` and `ftCo_MS_RebirthWait`: the action states of a fighter on the respawn
/// platform.
const MS_REBIRTH: i32 = 12;
/// `ftCo_MS_DeadDown` and `ftCo_MS_DeadUpFallHitCameraIce`, the first and last death states.
const MS_DEAD_DOWN: i32 = 0;
const MS_DEAD_UP_FALL_HIT_CAMERA_ICE: i32 = 10;
const MS_REBIRTH_WAIT: i32 = 13;

/// The fields the player blocks and the match read and write.
impl Game {
    #[doc(alias = "ftLib_IsSleeping")]
    #[doc(alias = "ftLib_IsSleeping_8008731C")]
    pub fn ft_is_sleeping(&self, gobj: GObjId) -> bool {
        self.fighter(gobj).is_sleeping
    }

    /// Whether the fighter is in hitstun (`x221C_b6`), `ftLib_IsInHitstun`.
    #[doc(alias = "ftLib_IsInHitstun")]
    pub fn ft_is_in_hitstun(&self, gobj: GObjId) -> bool {
        self.fighter(gobj).in_hitstun
    }

    /// Whether the fighter is on its respawn platform, `ftLib_IsRebirth`.
    #[doc(alias = "ftLib_IsRebirth")]
    pub fn ft_is_rebirth(&self, gobj: GObjId) -> bool {
        (MS_REBIRTH..=MS_REBIRTH_WAIT).contains(&self.fighter(gobj).motion_id)
    }

    /// Whether the fighter is in one of the death states, `ftLib_IsDead`.
    #[doc(alias = "ftLib_IsDead")]
    pub fn ft_is_dead(&self, gobj: GObjId) -> bool {
        (MS_DEAD_DOWN..=MS_DEAD_UP_FALL_HIT_CAMERA_ICE).contains(&self.fighter(gobj).motion_id)
    }

    /// The fighter's percent, truncated, `ftLib_GetPercent`.
    #[doc(alias = "ftLib_GetPercent")]
    pub fn ft_percent(&self, gobj: GObjId) -> i32 {
        gekko_fp::fctiwz(crate::fpu::reg(self.fighter(gobj).dmg.percent))
    }

    /// The grounded fighter's floor's slope in radians, `ftLib_GetGroundSlopeAngle`.
    #[doc(alias = "ftLib_GetGroundSlopeAngle")]
    pub fn ft_ground_slope_angle(&self, gobj: GObjId) -> f32 {
        let fp = self.fighter(gobj);
        assert!(
            fp.ground_or_air == GroundOrAir::Ground,
            "the original's assertion failed: ftlib.c:1517"
        );
        let n = &fp.coll_data.floor.normal;
        crate::math::atan2(-n.x, n.y)
    }

    /// Whether another fighter pushes this one, `ftLib_IsBeingNudged`.
    #[doc(alias = "ftLib_IsBeingNudged")]
    pub fn ft_is_being_nudged(&self, gobj: GObjId) -> bool {
        self.fighter(gobj).nudge_vel.x != 0.0
    }

    /// Whether the fighter holds any of `buttons`, `ft_800877F8`.
    #[doc(alias = "ft_800877F8")]
    pub fn ft_holds(&self, gobj: GObjId, buttons: u32) -> bool {
        self.fighter(gobj).input.held_buttons[0] & buttons != 0
    }

    /// Whether the fighter pressed any of `buttons` this frame, `ft_80087818`.
    #[doc(alias = "ft_80087818")]
    pub fn ft_pressed(&self, gobj: GObjId, buttons: u32) -> bool {
        self.fighter(gobj).input.pressed_buttons & buttons != 0
    }

    /// Whether the fighter holds an item of `kind` (`ft_80087900`, the Screw Attack;
    /// `ft_80087944`, Mr. Saturn).
    #[doc(alias = "ft_80087900")]
    #[doc(alias = "ft_80087944")]
    pub fn ft_holds_item_kind(&self, gobj: GObjId, kind: i32) -> bool {
        // `itGetKind`.
        self.fighter(gobj)
            .item_gobj
            .is_some_and(|it| crate::game::it::item(self, it).kind == kind)
    }

    /// Whether the fighter is resized, metal or otherwise changed, `ft_80087AEC`.
    #[doc(alias = "ft_80087AEC")]
    #[doc(alias("ft_80087aec"))]
    pub fn ft_is_altered(&self, gobj: GObjId) -> bool {
        let fp = self.fighter(gobj);
        fp.scale.y != fp.scale.x || fp.cloak || fp.is_metal || fp.bunny_hood.is_some()
    }

    /// What moves the fighter most, `ft_80087B34`: 0 its own velocity, 1 knockback, 2 a
    /// shield's push.
    #[doc(alias = "ft_80087B34")]
    #[doc(alias("ft_80087b34"))]
    pub fn ft_strongest_motion(&self, gobj: GObjId) -> i32 {
        use crate::fpu::reg;
        use gekko_fp as fp;
        let f = self.fighter(gobj);
        let sq = |x: f32, y: f32| fp::fadds(fp::fmuls(reg(x), reg(x)), fp::fmuls(reg(y), reg(y)));
        let own = sq(f.self_vel.x, f.self_vel.y);
        let shield = sq(f.atk_shield_kb_vel.x, f.atk_shield_kb_vel.y);
        let kb = sq(f.kb_vel.x, f.kb_vel.y);
        if own > kb {
            if own > shield {
                return 0;
            }
            return 2;
        }
        if kb > shield {
            return 1;
        }
        2
    }

    #[doc(alias = "ftLib_GetPos")]
    pub fn ft_pos(&self, gobj: GObjId) -> crate::vec::Vec3 {
        self.fighter(gobj).cur_pos
    }

    /// The depth of the fighter's camera box's target extent (`target_ext.v.z`),
    /// `ftLib_80086B80`.
    #[doc(alias = "ftLib_80086B80")]
    pub fn ft_camera_depth(&self, gobj: GObjId) -> f32 {
        let b = self
            .fighter(gobj)
            .camera_box
            .expect("a fighter without a camera box");
        self.camera.subject(b).target_ext.v.z
    }

    #[doc(alias = "ftLib_GetKind")]
    pub fn ft_kind(&self, gobj: GObjId) -> i32 {
        self.fighter(gobj).kind
    }

    /// The slot of whoever last hit the fighter, `ftLib_GetLastAttackerSlot`.
    #[doc(alias = "ftLib_GetLastAttackerSlot")]
    pub fn ft_last_attacker(&self, gobj: GObjId) -> i32 {
        self.fighter(gobj).dmg.source_player
    }

    /// Whether the fighter asks for its name tag, `ftLib_80086F4C`: not cloaked (`x221F_b2`),
    /// not dead (`x221F_b1`) and its tag countdown (`x209A`) down to 1. The HUD's tags read it.
    #[doc(alias = "ftLib_80086F4C")]
    #[doc(alias("ft_80086f4c"))]
    pub fn ft_shows_name_tag(&self, gobj: GObjId) -> bool {
        let fp = self.fighter(gobj);
        !fp.cloaked && !fp.dead && fp.name_tag_timer == 1
    }

    /// `ftLib_SetPercent`: the integer converted to the fighter's float percent (`frsp`).
    #[doc(alias = "ftLib_SetPercent")]
    pub fn ft_set_percent(&mut self, gobj: GObjId, percent: i32) {
        self.fighter_mut(gobj).dmg.percent = crate::fpu::mem(f64::from(percent));
    }

    #[doc(alias = "ftLib_EnableInput")]
    pub fn ft_enable_input(&mut self, gobj: GObjId) {
        self.fighter_mut(gobj).input_disabled = false;
    }

    /// No fighter takes input, `ftLib_DisableAllInput` (`ftLib_DisableInput` on each).
    #[doc(alias = "ftLib_DisableAllInput")]
    pub fn ft_disable_all_input(&mut self) {
        let mut cur = self.world.first_in(super::common::FIGHTER_P_LINK);
        while let Some(g) = cur {
            self.ft_disable_input(g);
            cur = self.world.gobjs[g.0].next;
        }
    }

    /// Every fighter takes input, `ftLib_EnableAllInput`.
    #[doc(alias = "ftLib_EnableAllInput")]
    pub fn ft_enable_all_input(&mut self) {
        let mut cur = self.world.first_in(super::common::FIGHTER_P_LINK);
        while let Some(g) = cur {
            self.ft_enable_input(g);
            cur = self.world.gobjs[g.0].next;
        }
    }

    /// How many fighters are out, `ftLib_CountFighters`.
    #[doc(alias = "ftLib_CountFighters")]
    pub fn ft_count(&self) -> i32 {
        let mut n = 0;
        let mut cur = self.world.first_in(super::common::FIGHTER_P_LINK);
        while let Some(g) = cur {
            n += 1;
            cur = self.world.gobjs[g.0].next;
        }
        n
    }

    /// Hitlag the fighter takes at its next hit check (`dmg.x1958`), `ftLib_80086A4C`: a
    /// hammer's or a stage switch's.
    #[doc(alias = "ftLib_80086A4C")]
    #[doc(alias("ft_80086a4c"))]
    pub fn ft_set_pending_hitlag(&mut self, gobj: GObjId, v: f32) {
        self.fighter_mut(gobj).dmg.pending_hitlag_frames = v;
    }

    /// `ft_80087AC0`: the fighter turns invisible when the argument is zero.
    #[doc(alias = "ft_80087AC0")]
    #[doc(alias("ft_80087ac0"))]
    pub fn ft_set_visible(&mut self, gobj: GObjId, arg: i32) {
        self.fighter_mut(gobj).invisible = arg == 0;
    }

    /// The fighter's invincibility (`x1988`: 1 invincible, 2 intangible), `ft_80087C58`.
    #[doc(alias = "ft_80087C58")]
    #[doc(alias("ft_80087c58"))]
    pub fn ft_invincibility(&self, gobj: GObjId) -> i32 {
        self.fighter(gobj).hit_status
    }

    /// Sets the fighter's invincibility (`x1988`), `ft_80087C64`.
    #[doc(alias = "ft_80087C64")]
    #[doc(alias("ft_80087c64"))]
    pub fn ft_set_invincibility(&mut self, gobj: GObjId, v: i32) {
        self.fighter_mut(gobj).hit_status = v;
    }

    /// Whether the blast zones leave the fighter alone (`x2228_b5`), `ft_80087BAC`.
    #[doc(alias = "ft_80087BAC")]
    #[doc(alias("ft_80087bac"))]
    pub fn ft_set_blast_immune(&mut self, gobj: GObjId, arg: i32) {
        self.fighter_mut(gobj).blast_immune = arg & 1 != 0;
    }

    /// The fighter's stick, `ftLib_GetLStick`.
    #[doc(alias = "ftLib_GetLStick")]
    pub fn ft_lstick(&self, gobj: GObjId) -> (f32, f32) {
        let s = &self.fighter(gobj).input.lstick[0];
        (s.x, s.y)
    }

    /// The fighter's facing, `ftLib_GetFacingDir`.
    #[doc(alias = "ftLib_GetFacingDir")]
    pub fn ft_facing_dir(&self, gobj: GObjId) -> f32 {
        self.fighter(gobj).facing_dir
    }

    /// Whether the fighter is charging a smash, `ftLib_IsChargingSmash`.
    #[doc(alias = "ftLib_IsChargingSmash")]
    pub fn ft_is_charging_smash(&self, gobj: GObjId) -> bool {
        // `SmashState_Charging`.
        self.fighter(gobj).smash_attrs.state == SmashState::Charging
    }

    /// Whether the fighter is on the ground or in the air, `ftLib_GetGroundAir`.
    #[doc(alias = "ftLib_GetGroundAir")]
    pub fn ft_ground_air(&self, gobj: GObjId) -> GroundOrAir {
        self.fighter(gobj).ground_or_air
    }

    #[doc(alias = "ftLib_GetMotionId")]
    pub fn ft_motion_id(&self, gobj: GObjId) -> i32 {
        self.fighter(gobj).motion_id
    }

    /// The fighter's height scale, `ftLib_GetScale`.
    #[doc(alias = "ftLib_GetScale")]
    pub fn ft_scale(&self, gobj: GObjId) -> f32 {
        self.fighter(gobj).scale.y
    }
}

impl Running<'_> {
    /// The joint of the fighter's part `part` (`fp->parts[part].joint`), `ftLib_GetPartJObj`.
    #[doc(alias = "ftLib_GetPartJObj")]
    pub fn ft_part_jobj(&self, gobj: GObjId, part: i32) -> Option<crate::hsd::jobj::JObjId> {
        self.bone_joint(gobj, part)
    }

    /// Sets the computer player's level, its state started afresh, `ft_80087BEC`.
    #[doc(alias = "ft_80087BEC")]
    pub fn ft_set_cpu_level(&mut self, gobj: GObjId, level: i32) {
        let f = self.game.fighter_id(gobj);
        let c = &self.game.fighters[f.0].cpu;
        let (kind, spawn_arg) = (c.kind, c.spawn_arg);
        self.init_cpu(f, kind, level, spawn_arg);
    }

    /// Sets the computer player's kind, its state started afresh, `ft_80087BC0`.
    #[doc(alias = "ft_80087BC0")]
    pub fn ft_set_cpu_type(&mut self, gobj: GObjId, kind: i32) {
        let f = self.game.fighter_id(gobj);
        let c = &self.game.fighters[f.0].cpu;
        let (level, spawn_arg) = (c.level, c.spawn_arg);
        self.init_cpu(f, kind, level, spawn_arg);
    }

    /// How high above the fighter its name tag goes: its character's own height while
    /// invisible, else its attributes', `ftLib_GetNameTagHeight`.
    #[doc(alias = "ftLib_GetNameTagHeight")]
    pub fn ft_name_tag_height(&self, gobj: GObjId) -> f64 {
        let fp = self.game.fighter(gobj);
        let h = if fp.invisible {
            let d = fp.ft_data.expect("a fighter without data");
            let attrs = self.data.ptr(d).expect("ftData::x0");
            self.data.f32(attrs.at(0xFC))
        } else {
            fp.co_attrs.name_tag_height
        };
        crate::fpu::reg(h)
    }
}

/// Rumbles a fighter's controller (id 0) with pattern `kind` for `frames` if its controller
/// rumbles and it's awake and alive, `ftLib_StartRumble`.
#[doc(alias = "ftLib_StartRumble")]
pub fn start_rumble(r: &mut Running, gobj: GObjId, kind: i32, frames: i32) {
    let fp = r.game.fighter(gobj);
    let (slot, sub) = (i32::from(fp.player_idx), i32::from(fp.is_sub_fighter));
    if !r.game.rumble_enabled(slot, sub) {
        return;
    }
    let fp = r.game.fighter(gobj);
    if !fp.is_sleeping && !fp.stamina_dead {
        let port = fp.pad_port;
        r.rumble_start(port, 0, kind, frames);
    }
}

/// Rumbles every fighter's controller, `ftLib_StartRumbleAll`.
#[doc(alias = "ftLib_StartRumbleAll")]
pub fn start_rumble_all(r: &mut Running, kind: i32, frames: i32) {
    let mut cur = r.game.world.first_in(super::common::FIGHTER_P_LINK);
    while let Some(g) = cur {
        cur = r.game.world.gobjs[g.0].next;
        start_rumble(r, g, kind, frames);
    }
}
