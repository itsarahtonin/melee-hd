//! Drawing the fighters (`melee/ft/ftdrawcommon.c`): which model groups show for each pass,
//! the shake offset's view matrix, and the joint trees drawn, all from the fighters' render
//! callbacks in the display pass.

use gekko_fp as fp;

use super::super::camera;
use super::super::code;
use super::super::state::{Obj, Running};
use crate::fpu::{mem, reg};
use crate::game::handles::FighterId;
use crate::hsd::display::{self, FighterMaterial};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;
use crate::mtx::Mtx34;

/// `HSD_GOBJ_PLINK_FIGHTER`.
const P_LINK_FIGHTER: u8 = 8;

impl Running<'_> {
    /// Each fighter's model groups, by `xC[1]` having a lookup: `hide` hidden, then `show`
    /// shown.
    fn set_models(&mut self, hide: [usize; 3], show: usize) {
        // Showing parts changes no GObj links.
        let mut next = self.game.world.first_in(P_LINK_FIGHTER);
        while let Some(g) = next {
            next = self.game.world.gobj(g).next;
            let f = self.game.fighter_id(g);
            if self.game.fighters[f.0].x5AC.xC[1].is_some() {
                for h in hide {
                    self.set_parts_vis(f, h, false);
                }
                self.set_parts_vis(f, show, true);
            }
        }
    }

    /// Every fighter with group 1 shows it instead of groups 0, 2 and 4 (the models shadows
    /// render), `ftDrawCommon_80081200`.
    #[doc(alias = "ftDrawCommon_80081200")]
    pub fn show_main_models(&mut self) {
        self.set_models([0, 2, 4], 1);
    }

    /// And back: group 0 instead of 1, 2 and 4, `ftDrawCommon_80081168`.
    #[doc(alias = "ftDrawCommon_80081168")]
    pub fn show_low_models(&mut self) {
        self.set_models([1, 2, 4], 0);
    }
}

impl Running<'_> {
    /// Whether the fighter draws this time, `ftLib_UpdateScreenVisibility`: through the game
    /// camera, a fighter whose camera box's bone is off its screen (the screen position kept
    /// in `x2188`) is marked off screen (`x221F_b0`) and draws only if the box's sphere still
    /// reaches the screen; through any other camera, or a fighter the camera ignores, it's on
    /// screen.
    #[doc(alias = "ftLib_UpdateScreenVisibility")]
    pub fn update_screen_visibility(&mut self, g: GObjId) -> bool {
        let f = self.game.fighter_id(g);
        let fp = &self.game.fighters[f.0];
        if !fp.x2229_b3
            && !fp.x2220_b7
            && let Some(cg) = camera::gobj(&self.game.camera.cam)
        {
            let c = match self.game.world.gobj(cg).obj {
                Some(Obj::CObj(c)) => Some(c),
                _ => None,
            };
            if c == self.game.objs.cobjs.current {
                let b = fp.x890_cameraBox.expect("a fighter without a camera box");
                let shown = match camera::box_to_screen(self.game, self.data, b) {
                    Some((p, inside)) => {
                        let fp = &mut self.game.fighters[f.0];
                        fp.x2188.x = p[0];
                        fp.x2188.y = p[1];
                        inside
                    }
                    None => false,
                };
                if shown {
                    self.game.fighters[f.0].x221F_b0 = false;
                    return true;
                }
                self.game.fighters[f.0].x221F_b0 = true;
                return camera::box_on_screen(self.game, self.data, b, 15.0);
            }
        }
        self.game.fighters[f.0].x221F_b0 = false;
        true
    }

    /// The view matrix shifted by the fighter's damage shake, if it shakes,
    /// `ftDrawCommon_8008051C`.
    #[doc(alias = "ftDrawCommon_8008051C")]
    fn shake_view(&self, f: FighterId) -> Option<Mtx34> {
        let d = &self.game.fighters[f.0].dmg;
        if d.x18B8 == 0.0 && d.x18BC == 0.0 {
            return None;
        }
        let c = self.game.objs.cobjs.current.expect("a current camera");
        let mut t = Mtx34::IDENTITY;
        t.m[0][3] = mem(fp::fadds(reg(d.x18B8), 0.0));
        t.m[1][3] = mem(fp::fadds(reg(d.x18BC), 0.0));
        t.m[2][3] = mem(fp::fadds(0.0, 0.0));
        Some(self.game.objs.cobjs[c].view_mtx.concat(&t))
    }

    /// What the fighter's materials read while its trees draw (`ftMaterial_800BF2B8`'s
    /// fighter, `HSD_GObj_804D7814`'s).
    pub fn material_info(&self, f: FighterId) -> FighterMaterial {
        let fp = &self.game.fighters[f.0];
        let o = self.active_overlay(f);
        FighterMaterial {
            refract: fp.x2226_b5.then(|| {
                let i = usize::from(fp.smash_attrs.x2134_vibrateFrame);
                *self.game.refract.tobjs.get(i).expect("refraction's chain")
            }),
            swapped: !fp.x2223_b2 && (!fp.x2228_b0 || !fp.x2224_b0) && (fp.is_metal || fp.x2227_b3),
            translucent: fp.x61D != 0xFF,
            x2223_b2: fp.x2223_b2,
            overlay_light: o.x7C_flag2 && o.x7C_light_enable,
            light: fp.x588,
            lights: self.game.ft.lobj0,
        }
    }

    /// Draws a tree with the fighter's materials.
    fn draw_fighter_tree(
        &mut self,
        f: FighterId,
        j: Option<JObjId>,
        vmtx: Option<Mtx34>,
        pass: i32,
    ) {
        let info = self.material_info(f);
        self.game.objs.display.fighter = Some(info);
        let mut hooks = code::ParticleHooks;
        let trsp = display::pass_trsp(pass);
        display::jobj_disp_all(&mut self.game.objs, self.data, &mut hooks, j, vmtx, trsp, 0);
        self.game.objs.display.fighter = None;
    }

    fn fighter_root(&self, g: GObjId) -> Option<JObjId> {
        match self.game.world.gobj(g).obj {
            Some(Obj::JObj(j)) => Some(j),
            _ => None,
        }
    }

    /// The fighter's per-kind drawing, `ftData_UnkMtxFunc0`: Kirby's and Jigglypuff's,
    /// which no port covers yet.
    fn kind_draw(&mut self, f: FighterId) {
        match self.game.fighters[f.0].kind {
            4 => code::unported_call("ftKb_UnkMtxFunc0"),
            15 => code::unported_call("ftPr_Init_UnkMtxFunc0"),
            _ => {}
        }
    }

    /// A fighter drawn normally, `ftDrawCommon_800805C8`: the debug drawings (none on a
    /// retail disc), then, if `main`, its model groups for the pass (the low models unless
    /// metal or the like), its light, its tree through the shaken view, and its accessory;
    /// and its afterimage.
    #[doc(alias = "ftDrawCommon_800805C8")]
    fn draw_fighter(&mut self, g: GObjId, pass: i32, main: bool) {
        let f = self.game.fighter_id(g);
        if self.game.fighters[f.0].x21FC_flag[0] & 0xFE != 0 {
            code::unported_call("ftDrawCommon_800805C8's debug drawings");
        }
        if self.game.fighters[f.0].x21FC_flag[0] & 1 == 0 {
            return;
        }
        let fp = &self.game.fighters[f.0];
        if !fp.invisible && !fp.x221E_b5 && main {
            self.set_parts_vis(f, 1, false);
            self.set_parts_vis(f, 4, false);
            let fp = &self.game.fighters[f.0];
            if fp.is_metal || fp.x2226_b5 || fp.x2227_b3 {
                self.set_parts_vis(f, 0, false);
                self.set_parts_vis(f, 2, true);
            } else {
                self.set_parts_vis(f, 2, false);
                self.set_parts_vis(f, 0, true);
            }
            self.fighter_light(f);
            let fp = &mut self.game.fighters[f.0];
            fp.x2223_b2 = false;
            fp.x2227_b7 = true;
            fp.x2228_b0 = false;
            let vmtx = self.shake_view(f);
            let root = self.fighter_root(g);
            self.draw_fighter_tree(f, root, vmtx, pass);
            self.kind_draw(f);
            self.fighter_light_reset(f);
        }
        if let Some(a) = self.game.fighters[f.0].x20A0_accessory {
            self.draw_fighter_tree(f, Some(a), None, pass);
        }
        self.draw_afterimage(f, pass);
    }

    /// A sword's afterimage, `ftCo_800C2600`: in the translucent pass, once the ring holds
    /// two of the sword's positions, the strip between them ([`display::Draw::Afterimage`]),
    /// its colors from the character's sword attributes. A held sword item's isn't ported.
    #[doc(alias = "ftCo_800C2600")]
    fn draw_afterimage(&mut self, f: FighterId, pass: i32) {
        let fp = &self.game.fighters[f.0];
        if pass != 2 || fp.x2100 <= 1 {
            return;
        }
        if fp.x2101_bits_8 {
            code::unported_call("it_80285300 (a held sword's afterimage)");
        }
        // Link's and Young Link's sword attributes, Marth's and Roy's; any other character's
        // are whatever the stack held.
        match fp.kind {
            6 | 20 | 18 | 26 => {}
            k => panic!("ftCo_800C2600: character {k} has no sword attributes to read"),
        }
        self.game
            .objs
            .display
            .draws
            .push(display::Draw::Afterimage(f));
    }

    /// A fighter drawn into the magnifier, `ftDrawCommon_80080C28`: with its light, its
    /// low models (when it has them), its tree through the shaken view (refraction's users
    /// don't draw), and its accessory.
    #[doc(alias = "ftDrawCommon_80080C28")]
    pub fn draw_magnified(&mut self, g: GObjId, pass: i32) {
        let f = self.game.fighter_id(g);
        if self.game.fighters[f.0].x21FC_flag[0] & 1 == 0 {
            return;
        }
        let fp = &self.game.fighters[f.0];
        if !fp.invisible && !fp.x221E_b5 && !fp.x2226_b5 {
            self.fighter_light(f);
            if self.game.fighters[f.0].x5AC.xC[1].is_some() {
                self.set_parts_vis(f, 0, false);
                self.set_parts_vis(f, 2, false);
                self.set_parts_vis(f, 4, false);
                self.set_parts_vis(f, 1, true);
            }
            let fp = &mut self.game.fighters[f.0];
            fp.x2223_b2 = false;
            fp.x2223_b3 = false;
            fp.x2227_b7 = false;
            fp.x2228_b0 = true;
            let vmtx = self.shake_view(f);
            let root = self.fighter_root(g);
            self.draw_fighter_tree(f, root, vmtx, pass);
            self.kind_draw(f);
            // `ftCo_800C8AF0` stops refraction only for `x2226_b5`, which doesn't draw here.
            self.fighter_light_reset(f);
        }
        if let Some(a) = self.game.fighters[f.0].x20A0_accessory {
            self.draw_fighter_tree(f, Some(a), None, pass);
        }
    }

    /// The fighter's group 4 models (its outline) drawn for a pass, the first (`x2223_b3`
    /// clear) or the second time, the inlines of `ftDrawCommon_80080E18`.
    fn draw_outline(&mut self, g: GObjId, pass: i32, second: bool) {
        let f = self.game.fighter_id(g);
        let fp = &self.game.fighters[f.0];
        if fp.x21FC_flag[0] & 1 == 0 || fp.invisible || fp.x221E_b5 || fp.x2226_b5 {
            return;
        }
        self.set_parts_vis(f, 0, false);
        self.set_parts_vis(f, 1, false);
        self.set_parts_vis(f, 2, false);
        self.set_parts_vis(f, 4, true);
        let fp = &mut self.game.fighters[f.0];
        fp.x2223_b2 = true;
        fp.x2223_b3 = second;
        let vmtx = self.shake_view(f);
        let root = self.fighter_root(g);
        self.draw_fighter_tree(f, root, vmtx, pass);
    }

    /// A fighter's render callback, `ftDrawCommon_80080E18`: a sleeping fighter or one off
    /// screen doesn't draw; in the camera's pass 1 a fighter with an outline draws it, itself
    /// and the outline again; in pass 0 it draws itself (its models only without an outline).
    #[doc(alias = "ftDrawCommon_80080E18")]
    pub fn render_fighter(&mut self, g: GObjId, pass: i32) {
        if self.game.fighter(g).is_sleeping || !self.update_screen_visibility(g) {
            return;
        }
        let f = self.game.fighter_id(g);
        match camera::pass(&self.game.camera.cam) {
            1 => {
                if self.game.fighters[f.0].x2220_b7 {
                    self.place_by_camera(g);
                }
                if self.game.fighters[f.0].x5AC.xC[4].is_some() {
                    self.draw_outline(g, pass, false);
                    self.draw_fighter(g, pass, true);
                    self.draw_outline(g, pass, true);
                }
            }
            0 => {
                self.game.fighters[f.0].x2223_b3 = false;
                let main = self.game.fighters[f.0].x5AC.xC[4].is_none();
                self.draw_fighter(g, pass, main);
            }
            _ => {}
        }
    }
}
