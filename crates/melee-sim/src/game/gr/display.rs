//! Drawing the stage's objects (`melee/gr/grdisplay.c`): each stage object draws in its own
//! camera layer, through the game camera or one it names, either as a plain joint tree or,
//! for objects that receive shadows, joint by joint with each fighter's shadow switched by
//! the stage's shadow check. The shadows' switches are state.

use super::super::code;
use super::super::state::{Obj, Running};
use super::ground::{ground_of, jobj_of};
use crate::hsd::display;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::mtx::Mtx34;

/// `HSD_GOBJ_PLINK_FIGHTER`.
const P_LINK_FIGHTER: u8 = 8;
/// `JOBJ_TRSP_SHIFT` and the root flags' shift.
const TRSP_SHIFT: u32 = 18;
const ROOT_SHIFT: u32 = 28;

/// `grDisplay_801C5F60`, a stage object's camera's render callback.
pub const STAGE_CAMERA_RENDER: u32 = 0x801C_5F60;

impl Running<'_> {
    /// The first fighter GObj; the next are each one's `next` (walks over them change no
    /// links).
    fn first_fighter(&self) -> Option<GObjId> {
        self.game.world.first_in(P_LINK_FIGHTER)
    }

    /// The stage's shadow check for a fighter (at its position, `ftLib_GetPos`) against a
    /// joint (`stage_info.on_check_shadow_render`).
    #[doc(alias = "grLast_OnCheckShadowRender")]
    #[doc(alias = "grBattle_OnCheckShadowRender")]
    fn stage_receives_shadow(&mut self, g: GObjId, j: JObjId) -> bool {
        let Some(f) = self.game.stage_info.on_check_shadow_render else {
            panic!("stage_info.on_check_shadow_render is null")
        };
        let check = super::registry::by_address(self.game.stage_info.grkind, f.0, |d| {
            d.on_check_shadow_render
        })
        .and_then(|s| s.shadow)
        .unwrap_or_else(|| code::unported_call(&format!("on_check_shadow_render {:#010x}", f.0)));
        let pos = self.game.fighters[self.game.fighter_id(g).0].cur_pos;
        check(self, pos, j)
    }

    /// Updates which fighters' shadows a frame turns on, `lbShadow_8000EEE0` on each fighter.
    #[doc(alias = "ftLib_CastsShadow")]
    #[doc(alias = "ftLib_GetShadow")]
    #[doc(alias = "lbShadow_8000EEE0")]
    pub fn shadow_update(&mut self, g: GObjId) {
        let f = self.game.fighter_id(g);
        let fp = &self.game.fighters[f.0];
        let ls = &fp.shadow;
        let any = ls.x0_b0 || ls.x0_b1 || ls.x0_b2 || ls.x0_b3 || ls.x0_b4 || ls.x0_b5;
        let casts = !fp.dead_flag && !fp.cloak && !fp.is_sub_fighter;
        let s = ls.shadow.expect("a fighter without a shadow");
        crate::hsd::shadow::set_active(&mut self.game.objs, s, !any && casts);
    }

    /// Turns on at most five fighters' shadows, `lbShadow_8000EFEC`: past the fifth whose
    /// shadow nothing holds off, the rest are held off this time.
    #[doc(alias = "lbShadow_8000EFEC")]
    pub fn shadow_limit(&mut self) {
        let mut f = self.first_fighter();
        while let Some(g) = f {
            self.game.fighter_mut(g).shadow.x0_b2 = false;
            f = self.game.world.gobj(g).next;
        }
        let mut count = 0;
        let mut f = self.first_fighter();
        while let Some(g) = f {
            f = self.game.world.gobj(g).next;
            let ls = &mut self.game.fighter_mut(g).shadow;
            let any = ls.x0_b0 || ls.x0_b1 || ls.x0_b2 || ls.x0_b3 || ls.x0_b4 || ls.x0_b5;
            if !any {
                count += 1;
                if count > 5 {
                    ls.x0_b2 = true;
                }
            }
            self.shadow_update(g);
        }
    }

    /// A shadow-receiving stage object's joints drawn one by one, `grDisplay_801C5B90`:
    /// before each joint draws, each fighter's shadow is switched by the stage's check and
    /// the shadows limited to five.
    #[doc(alias = "grDisplay_801C5B90")]
    fn stage_disp_all(&mut self, j: Option<JObjId>, vmtx: Option<Mtx34>, trsp: u32) {
        let Some(j) = j else { return };
        let flags = self.game.objs.jobjs[j].flags;
        if flags & jobj::INSTANCE != 0 {
            if flags & jobj::HIDDEN != 0 {
                return;
            }
            let objs = &mut self.game.objs;
            jobj::setup_matrix(objs, self.data, Some(j));
            let child = objs.jobjs[j].child.expect("an instance without its tree");
            if objs.jobjs[child].mtx_is_dirty() {
                jobj::setup_matrix_sub(objs, self.data, child);
            }
            let m = objs.jobjs[child].mtx.hsd_inverse_concat(&objs.jobjs[j].mtx);
            let c = objs.cobjs.current.expect("cobj");
            let m = objs.cobjs[c].view_mtx.concat(&m);
            self.stage_disp_all(Some(child), Some(m), trsp);
            return;
        }
        if flags & (trsp << TRSP_SHIFT) != 0 {
            let mut f = self.first_fighter();
            while let Some(g) = f {
                let casts = self.game.camera.shadows() || self.stage_receives_shadow(g, j);
                self.game.fighter_mut(g).shadow.x0_b4 = !casts;
                f = self.game.world.gobj(g).next;
            }
            self.shadow_limit();
            let mut hooks = code::ParticleHooks;
            display::jobj_disp(&mut self.game.objs, self.data, &mut hooks, j, vmtx, trsp, 0);
        }
        if flags & (trsp << ROOT_SHIFT) != 0 {
            let mut c = self.game.objs.jobjs[j].child;
            while let Some(x) = c {
                self.stage_disp_all(Some(x), vmtx, trsp);
                c = self.game.objs.jobjs[x].next;
            }
        }
    }

    /// A stage object's camera's render callback, `grDisplay_801C5F60`: through its camera,
    /// render layer 0, every pass of render link 3.
    #[doc(alias = "grDisplay_801C5F60")]
    pub fn render_stage_camera(&mut self, gobj: GObjId) {
        let Some(Obj::CObj(c)) = self.game.world.gobj(gobj).obj else {
            panic!("a stage object's camera GObj holds no CObj");
        };
        if display::camera_begin(&mut self.game.objs, self.data, Some(c)) {
            self.game.world.gobj_mut(gobj).gxlink_prios = 8;
            self.game.camera.set_layer(0);
            self.render_links(gobj, 7);
            display::camera_end(&mut self.game.objs);
        }
    }

    /// A stage object's render callback, `grDisplay_801C5DB0`: in its own camera layer, and
    /// through the camera it names (or, for an object that follows the game camera, only
    /// while that camera draws); the stage's fog off unless it keeps it, HAL's state caches
    /// out of date, the object's tree (shadow by shadow if it receives them), and the stage's
    /// fog back on.
    #[doc(alias = "grDisplay_801C5DB0")]
    pub fn render_stage_object(&mut self, gobj: GObjId, pass: i32) {
        let gid = ground_of(self.game, gobj);
        let gp = &self.game.grounds[gid.0];
        if gp.pass_flags.b012 != self.game.camera.layer() {
            return;
        }
        let rendering = self
            .game
            .world
            .rendering_camera
            .expect("the drawing camera");
        let rendering_obj = self.game.world.gobj(rendering).obj;
        let (x18, follows) = (gp.camera, gp.flags.follows_camera);
        match x18 {
            Some(cam) => {
                if self.game.world.gobj(cam).obj != rendering_obj {
                    if !follows {
                        return;
                    }
                    let Some(cg) = self.game.camera.gobj() else {
                        return;
                    };
                    let Some(Obj::CObj(c)) = self.game.world.gobj(cg).obj else {
                        return;
                    };
                    if self.game.objs.cobjs.current != Some(c) {
                        return;
                    }
                }
            }
            None => {
                if let Some(Obj::CObj(c)) = rendering_obj
                    && super::setup::is_stage_camera(self.game, c)
                {
                    return;
                }
            }
        }
        let cam = &self.game.camera;
        if cam.draw_collisions() || !cam.stage_visible() {
            return;
        }
        let gp = &self.game.grounds[gid.0];
        let (keeps_fog, shadowed) = (gp.flags.keeps_fog, gp.flags.shadowed);
        if !keeps_fog {
            display::fog_set(&mut self.game.objs, None);
        }
        display::state_invalidate(&mut self.game.objs, u32::MAX);
        if shadowed {
            let j = jobj_of(self.game, gobj);
            self.stage_disp_all(j, None, display::pass_trsp(pass));
        } else {
            let mut f = self.first_fighter();
            while let Some(g) = f {
                f = self.game.world.gobj(g).next;
                self.game.fighter_mut(g).shadow.x0_b4 = false;
                self.shadow_update(g);
            }
            self.render_jobj(gobj, pass);
        }
        if !keeps_fog
            && !self.game.camera.draw_collisions()
            && self.game.stage_info.bits.fog_enabled
            && let Some(fg) = self.game.stage_info.fog
            && let Some(Obj::Fog(f)) = self.game.world.gobj(fg).obj
        {
            display::fog_set(&mut self.game.objs, Some(f));
        }
    }
}
