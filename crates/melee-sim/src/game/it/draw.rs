//! Drawing the items (`melee/it/itdraw.c`): an item held by a fighter draws with the
//! fighter's shake, and an item with outline models draws them around itself in the camera's
//! first pass. The flags the drawing leaves (`xDCF`) are state its material setup reads.

use super::super::camera;
use super::super::code;
use super::super::state::{Obj, Running, UserData};
use super::{flag, item, item_mut, set_flag};
use crate::hsd::display;
use crate::hsd::gobj::GObjId;
use crate::mtx::Mtx34;
use crate::vec::Vec3;

impl Running<'_> {
    /// Whether the fighter shows the item it holds or the two it keeps, `ftLib_IsItemVisible`.
    #[doc(alias = "ftLib_IsItemVisible")]
    fn item_visible(&self, owner: GObjId, it: GObjId) -> bool {
        let fp = self.game.fighter(owner);
        let (held, other) = (
            fp.item_gobj == Some(it),
            fp.x197C == Some(it) || fp.x1980 == Some(it),
        );
        if !held && !other {
            return true;
        }
        !(fp.invisible || fp.x221E_b5 || (!fp.x221E_b3 && held) || (!fp.x221E_b4 && other))
    }

    /// The item's tree drawn for the pass, through the view shifted by `shake`,
    /// `it_8026EB18`.
    #[doc(alias = "it_8026EB18")]
    fn draw_item_tree(&mut self, gobj: GObjId, pass: i32, shake: Option<Vec3>) {
        let vmtx = shake.map(|p| {
            let c = self.game.objs.cobjs.current.expect("a current camera");
            let mut m = Mtx34::IDENTITY;
            m.m[0][3] = p.x;
            m.m[1][3] = p.y;
            m.m[2][3] = p.z;
            self.game.objs.cobjs[c].view_mtx.concat(&m)
        });
        let j = match self.game.world.gobj(gobj).obj {
            Some(Obj::JObj(j)) => Some(j),
            _ => None,
        };
        let mut hooks = code::ParticleHooks;
        let trsp = display::pass_trsp(pass);
        display::jobj_disp_all(&mut self.game.objs, self.data, &mut hooks, j, vmtx, trsp, 0);
    }

    /// An item's render callback, `it_8026EECC`: an item that draws (`xDAA` bit 7), unless
    /// a fighter holding it hides it, takes the fighter's shake, then in the camera's pass 0
    /// draws its tree; an item with outline models draws them in pass 1 instead, which no
    /// port covers yet. The debug drawings after (`it_8026ECE0`: hit and hurt capsules and the
    /// like) only draw.
    #[doc(alias = "it_8026EECC")]
    pub fn render_item(&mut self, gobj: GObjId, pass: i32) {
        let ip = item(self.game, gobj);
        if !flag(ip.xDAA, 7) {
            return;
        }
        let mut pos = Vec3::ZERO;
        if ip.xDC8_word.flags.x13 {
            let owner = ip.owner;
            let fighter_owner = owner.filter(|&o| {
                matches!(
                    self.game.world.gobj(o).user_data,
                    Some(UserData::Fighter(_))
                )
            });
            if let Some(o) = fighter_owner
                && !self.item_visible(o, gobj)
            {
                return;
            }
            set_flag(&mut item_mut(self.game, gobj).xDCF_flag, 7, false);
            if let Some(o) = fighter_owner {
                let d = &self.game.fighter(o).dmg;
                if d.x18B8 != 0.0 || d.x18BC != 0.0 {
                    pos = Vec3::new(d.x18B8, d.x18BC, 0.0);
                    set_flag(&mut item_mut(self.game, gobj).xDCF_flag, 7, true);
                }
            }
        }
        let outlined = flag(item(self.game, gobj).xDCF_flag, 3);
        match camera::pass(&self.game.camera.cam) {
            1 if outlined => code::unported_call("it_8026EECC's outline passes"),
            0 if !outlined => {
                let ip = item_mut(self.game, gobj);
                set_flag(&mut ip.xDCF_flag, 4, false);
                set_flag(&mut ip.xDCF_flag, 5, false);
                let shake = flag(ip.xDCF_flag, 7).then_some(pos);
                self.draw_item_tree(gobj, pass, shake);
            }
            _ => {}
        }
    }
}
