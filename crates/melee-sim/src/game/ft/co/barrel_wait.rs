//! Waiting in a stage's barrel (`ftCo_BarrelWait.c`): Kongo Jungle's and Kongo Jungle N64's
//! barrels catch a fighter that falls in (`ftCo_800C0874`) and hold it, invisible and
//! intangible, where the barrel is, until the barrel shoots it out along the way it points; A
//! or B asks the barrel to shoot at once.

use super::super::super::gr::{ground, kongo, oldkongo};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::ids::ms;
use super::{PAD_A, PAD_B};
use crate::fpu::reg;
use crate::game::structs::HitCapsule;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `ftCo_8009EAF8`, the waiting fighter's death hook.
pub const DEATH2: u32 = 0x8009_EAF8;
const ANIM: u32 = 0x8009_EC08;
const IASA: u32 = 0x8009_EC0C;
const PHYS: u32 = 0x8009_EC3C;
const COLL: u32 = 0x8009_EC40;
/// `ftCo_8009EC44`, which keeps the fighter at the barrel.
pub const ACCESSORY1: u32 = 0x8009_EC44;

/// `Gr_Kind_Kongo` and `Gr_Kind_OldKongo`, the stages with a barrel.
const GR_KIND_KONGO: i32 = 4;
const GR_KIND_OLDKONGO: i32 = 0x1E;

/// Runs one of the barrel state's callbacks and hooks, if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DEATH2 => barrel_drop(g),
        ANIM | PHYS | COLL => barrel_wait_nothing(g, gobj),
        IASA => g.barrel_wait_iasa(gobj),
        ACCESSORY1 => g.barrel_wait_follow(gobj),
        _ => return false,
    }
    true
}

/// The barrel's object (`stage_info.x72C`).
fn barrel_gobj(g: &Running) -> GObjId {
    g.game.stage_info.x72C.expect("no barrel on the stage")
}

/// Asks the stage's barrel to shoot its fighter at once, `Ground_801C4DD0`.
#[doc(alias = "Ground_801C4DD0")]
pub fn barrel_release(g: &mut Running) -> bool {
    match g.game.stage_info.grkind {
        GR_KIND_KONGO => kongo::barrel_release(g.game, barrel_gobj(g)),
        GR_KIND_OLDKONGO => oldkongo::barrel_release(g.game, barrel_gobj(g)),
        _ => {}
    }
    true
}

/// Has the stage's barrel let go of its fighter, `Ground_801C4E20`.
#[doc(alias = "Ground_801C4E20")]
pub fn barrel_letgo(g: &mut Running) -> bool {
    match g.game.stage_info.grkind {
        GR_KIND_KONGO => kongo::barrel_drop(g.game, barrel_gobj(g)),
        GR_KIND_OLDKONGO => oldkongo::barrel_drop(g.game, barrel_gobj(g)),
        _ => {}
    }
    true
}

/// The waiting fighter dies: the barrel lets it go, `ftCo_8009EAF8`.
#[doc(alias = "ftCo_8009EAF8")]
fn barrel_drop(g: &mut Running) {
    barrel_letgo(g);
}

/// `ftCo_BarrelWait_Anim`, `ftCo_BarrelWait_Phys` and `ftCo_BarrelWait_Coll`: nothing.
#[doc(alias = "ftCo_BarrelWait_Anim")]
#[doc(alias = "ftCo_BarrelWait_Phys")]
#[doc(alias = "ftCo_BarrelWait_Coll")]
fn barrel_wait_nothing(_g: &mut Running, _gobj: GObjId) {}

impl Running<'_> {
    /// Caught by a stage's barrel, `ftCo_8009EB18`: lets go of what it holds and whom it
    /// holds, waits still, invisible and intangible, the camera box's extents the common
    /// data's.
    #[doc(alias = "ftCo_8009EB18")]
    pub fn enter_stage_barrel(&mut self, gobj: GObjId) {
        // `ftCo_ReleaseItemAndVictim`.
        self.damage_hooks(gobj);
        self.drop_heavy_item(gobj);
        self.release_victim(gobj);
        self.change_motion_state(gobj, ms::BARREL_WAIT, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.death2_cb = Some(CodeRef(DEATH2));
        fp.accessory1_cb = Some(CodeRef(ACCESSORY1));
        self.stop_dead(gobj);
        let c = self.common();
        let h = (c.f32(0x4DC), c.f32(0x4E0));
        let v = (c.f32(0x4E4), c.f32(0x4E8), c.f32(0x4EC));
        let fp = self.game.fighter_mut(gobj);
        fp.x1988 = 2;
        fp.invisible = true;
        // `ftCommon_8007E2F4`.
        fp.x1A6A = 0x1FF;
        let b = fp.x890_cameraBox.expect("a fighter without a camera box");
        let cb = &mut self.game.camera.boxes[b];
        (cb.target_ext.h.x, cb.target_ext.h.y) = h;
        (cb.target_ext.v.x, cb.target_ext.v.y, cb.target_ext.v.z) = v;
        // `ftCommon_8007EFC0`.
        self.game.fighter_mut(gobj).x209A = 1;
    }

    /// `ftCo_BarrelWait_IASA`: A or B asks the barrel to shoot.
    #[doc(alias = "ftCo_BarrelWait_IASA")]
    pub fn barrel_wait_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).input.pressed_buttons & (PAD_A | PAD_B) != 0 {
            barrel_release(self);
        }
    }

    /// Inside, the fighter is where the barrel is, `ftCo_8009EC44`.
    #[doc(alias = "ftCo_8009EC44")]
    pub fn barrel_wait_follow(&mut self, gobj: GObjId) {
        let (pos, _) = ground::barrel(self.game);
        self.game.fighter_mut(gobj).cur_pos = pos;
    }

    /// Shot out of a stage's barrel at `pos`, `ftCo_8009EC70`: kept from the barrels for the
    /// common data's while, put there with its collision and camera box, the shot's knockback
    /// at `kb_angle` (degrees, the way the barrel points, mirrored for a fighter facing right)
    /// and damage, and tumbling off.
    #[doc(alias = "ftCo_8009EC70")]
    pub fn shoot_from_stage_barrel(
        &mut self,
        gobj: GObjId,
        pos: Vec3,
        hit: &HitCapsule,
        kb_angle: f32,
    ) {
        let timer = self.common().u32(0x5E0);
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.bury_timer_2 = timer as _;
        fp.cur_pos = pos;
        fp.coll_data.set_position(pos);
        self.init_camera_box(f);
        self.clear_attacker(gobj);
        self.barrel_knockback(gobj, hit, kb_angle);
        self.take_damage_coins(f, reg(hit.damage));
        self.enter_damage(gobj, ms::DAMAGE_FLY_ROLL, 0.0);
    }
}
