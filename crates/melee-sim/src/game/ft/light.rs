//! The fighters' own light (`ftCo_09F4.c`): a color overlay can light a fighter with a light
//! of its own, aimed by the overlay's angles and the fighter's facing, added to the current
//! lights when the fighter draws.

use gekko_fp as fp;

use super::super::state::Running;
use super::super::structs::ColorOverlay;
use crate::fpu::{mem, reg};
use crate::game::handles::FighterId;
use crate::hsd::display;
use crate::hsd::lobj;
use crate::math;
use crate::vec::Vec3;

/// `LOBJ_ALPHA`.
const ALPHA: u32 = 1 << 4;
/// `MTXDegToRad`'s factor, as a float.
const DEG_TO_RAD: f32 = 0.017_453_292;
/// `node0`'s color, the fighters' light's.
const WHITE: [u8; 4] = [0xFF; 4];

impl Running<'_> {
    /// The fighter's color overlay that is running, `ftCo_800C0658`: the first if it runs an
    /// animation, else the second.
    #[doc(alias = "ftCo_800C0658")]
    pub fn active_overlay(&self, f: FighterId) -> &ColorOverlay {
        let fp = &self.game.fighters[f.0];
        if fp.x408.x28_colanim != [0; 4] {
            &fp.x408
        } else {
            &fp.x488
        }
    }

    /// The lights a fighter draws in, `ftCo_8009F5AC`: with an overlay that lights it, its
    /// light aimed and colored once after a change (`x2221_b3`), then the fighters' lights (or
    /// just the ambient one, for an overlay that only sets alpha) with its own, set up for the
    /// current camera.
    #[doc(alias = "ftCo_8009F5AC")]
    pub fn fighter_light(&mut self, f: FighterId) {
        let o = self.active_overlay(f).clone();
        if !o.x7C_flag2 {
            return;
        }
        let light = self.game.fighters[f.0]
            .x588
            .expect("a fighter without its light");
        if self.game.fighters[f.0].x2221_b3 {
            let root = match self
                .game
                .world
                .gobj(self.game.fighters[f.0].gobj.expect("gobj"))
                .obj
            {
                Some(crate::game::state::Obj::JObj(j)) => j,
                _ => panic!("a fighter GObj without its joint"),
            };
            let j = &self.game.objs.jobjs[root];
            assert!(
                j.flags & crate::hsd::jobj::USE_QUATERNION == 0,
                "jobj rotation"
            );
            let rot_y = j.rotate.y;
            let yz = fp::fmuls(reg(DEG_TO_RAD), reg(o.x78_light_rot_yz));
            let x = fp::frsp(fp::fdiv(
                fp::fmuls(
                    fp::fmuls(reg(DEG_TO_RAD), reg(o.x74_light_rot_x)),
                    reg(rot_y),
                ),
                fp::fdiv(std::f64::consts::PI, 2.0),
            ));
            let neg = mem(fp::fneg(yz));
            let y = mem(fp::fneg(reg(math::sin(neg))));
            let z = math::cos(neg);
            let xs = mem(fp::fmuls(reg(z), reg(math::sin(mem(x)))));
            let z = mem(fp::fmuls(reg(z), reg(math::cos(mem(x)))));
            lobj::set_position(&mut self.game.objs, light, Vec3::new(xs, y, z));
            let c = o.x50_light_color;
            if o.x7C_light_enable {
                let color = [WHITE[0], WHITE[1], WHITE[2], c.a];
                lobj::set_color(&mut self.game.objs.lobjs, light, color);
                lobj::set_flags(&mut self.game.objs.lobjs, Some(light), ALPHA);
            } else {
                lobj::set_color(&mut self.game.objs.lobjs, light, [c.r, c.g, c.b, c.a]);
                lobj::clear_flags(&mut self.game.objs.lobjs, Some(light), ALPHA);
            }
            self.game.fighters[f.0].x2221_b3 = false;
        }
        let (lobj0, lobj1) = (self.game.ft.lobj0, self.game.ft.lobj1);
        if o.x7C_light_enable {
            lobj::replace_current(&mut self.game.objs, lobj0);
        } else {
            lobj::set_current_all(&mut self.game.objs, lobj1);
        }
        lobj::add_current(&mut self.game.objs, Some(light));
        self.setup_lights();
    }

    /// The fighters' lights, with the fighter's own for an alpha overlay, set up again when a
    /// material leaves its alpha to the lights, `ftCo_8009F75C`.
    #[doc(alias = "ftCo_8009F75C")]
    pub fn fighter_light_alpha(&mut self, f: FighterId, alpha: bool) {
        let o = self.active_overlay(f);
        if !(o.x7C_flag2 && o.x7C_light_enable) {
            return;
        }
        let light = self.game.fighters[f.0].x588;
        let ls = &mut self.game.objs.lobjs;
        if alpha {
            lobj::set_flags(ls, light, ALPHA);
        } else {
            lobj::clear_flags(ls, light, ALPHA);
        }
        let (lobj0, lobj1) = (self.game.ft.lobj0, self.game.ft.lobj1);
        if alpha {
            lobj::replace_current(&mut self.game.objs, lobj0);
        } else {
            lobj::set_current_all(&mut self.game.objs, lobj1);
        }
        lobj::add_current(&mut self.game.objs, light);
        self.setup_lights();
    }

    /// The fighters' lights back after a lit fighter draws, `ftCo_8009F7F8`.
    #[doc(alias = "ftCo_8009F7F8")]
    pub fn fighter_light_reset(&mut self, f: FighterId) {
        if self.active_overlay(f).x7C_flag2 {
            let lobj0 = self.game.ft.lobj0;
            lobj::replace_current(&mut self.game.objs, lobj0);
            self.setup_lights();
        }
    }

    /// `HSD_LObjSetupInit(HSD_CObjGetCurrent())`.
    fn setup_lights(&mut self) {
        let c = self
            .game
            .objs
            .cobjs
            .current
            .expect("HSD_LObjSetupInit: no current camera");
        display::lobj_setup_init(&mut self.game.objs, self.data, c);
    }
}
