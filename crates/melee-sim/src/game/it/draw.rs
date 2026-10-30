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
use melee_data::archive::DataRef;

/// An item's pickup range's colour (`it_804D5168`).
const PICKUP_RANGE: [u8; 4] = [0xFF, 0x40, 0x80, 0x80];

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

    /// Hides (`it_8026EC54`, `it_80272A3C` on each) or shows (`it_8026EBC8`, `it_80272A18`)
    /// the item's bones a list names (a count, then a pointer to the bones' indices), each
    /// only if its parent isn't hidden.
    #[doc(alias = "it_8026EC54")]
    #[doc(alias = "it_8026EBC8")]
    fn item_bones_hidden(&mut self, gobj: GObjId, list: (u16, Option<DataRef>), hide: bool) {
        let (n, at) = list;
        let table = item(self.game, gobj).xBBC_dynamicBoneTable;
        for k in 0..u32::from(n) {
            let idx = self
                .data
                .u8(at.expect("an outline list without bones").at(k));
            let t = table.expect("an item without a bone table");
            let j = self.game.it.bone_tables[t.0].bones[usize::from(idx)];
            let parent = j.and_then(|j| self.game.objs.jobjs[j].parent);
            let parent = parent.expect("jobj.h: jobj");
            if self.game.objs.jobjs[parent].flags & crate::hsd::jobj::HIDDEN != 0 {
                continue;
            }
            if hide {
                super::lib2725::hide_tree(self.game, j);
            } else {
                super::lib2725::show_tree(self.game, j);
            }
        }
    }

    /// An item's render callback, `it_8026EECC`: an item that draws (`xDAA` bit 7), unless
    /// a fighter holding it hides it, takes the fighter's shake, then in the camera's pass 0
    /// draws its tree; an item with outline models (Mr. Game & Watch's) draws in pass 1
    /// instead, its outline twice around it. Then the debug drawings, drawn or not
    /// (`it_8026ECE0`), and HAL's state reset after any.
    #[doc(alias = "it_8026EECC")]
    pub fn render_item(&mut self, gobj: GObjId, pass: i32) {
        self.render_item_model(gobj, pass);
        if self.item_debug_drawings(gobj, pass) {
            crate::lb::coll_draw::push(
                &mut self.game.objs,
                crate::lb::coll_draw::DebugDraw::ItemEnd,
            );
        }
    }

    /// An item's debug drawings, `it_8026ECE0`, by its display flags (`xDAA`, set by the
    /// develop tools): with `b6` its hits' bubbles (`b2`) and, unless held, its hurt
    /// capsules' (`b1`, coloured by its invincibility `xD0C`); its pickup range (`b4`, for an
    /// item that can be picked up), an enemy's stomp range (`b3`); a coin only its range
    /// (`b0`).
    #[doc(alias = "it_8026ECE0")]
    fn item_debug_drawings(&mut self, gobj: GObjId, pass: i32) -> bool {
        use crate::lb::coll_draw;
        let data = self.data;
        let i = super::item_of(self.game, gobj);
        let game = &mut *self.game;
        let ip = &mut game.items[i.0];
        let objs = &mut game.objs;
        let mut drew = false;
        if ip.kind == super::kinds::UNK4 {
            // The coin (`It_Kind_Unk4`): its range while it can be picked up.
            let v = ip.xDD4_itemVar.coin();
            let f = v.x18.get();
            if flag(ip.xDAA, 0) && (f.others || f.owner) {
                let sub = &mut v.sub;
                let (a, b, x0) = (sub.x14.get(), sub.x8.get(), sub.x0.get());
                return coll_draw::coin_range(objs, a, b, x0, ip.scl, pass);
            }
            return false;
        }
        if flag(ip.xDAA, 6) {
            if flag(ip.xDAA, 2) {
                for h in &ip.x5D4_hitboxes {
                    drew |= coll_draw::hit(objs, &h.hit, pass, ip.scl);
                }
            }
            if !ip.xDC8_word.flags.x13 && flag(ip.xDAA, 1) {
                let n = usize::from(ip.xAC8_hurtboxNum);
                let state = (ip.xD0C != 0).then_some(ip.xD0C as u32);
                for h in &mut ip.xACC_itemHurtbox[..n] {
                    drew |= coll_draw::hurt(objs, data, h, state, pass, None, 0.0);
                }
            }
        }
        if flag(ip.xDAA, 4) && ip.xDC8_word.flags.x15 {
            let range = crate::quat::Quat {
                x: ip.xBCC_unk.x,
                y: ip.xBCC_unk.y,
                z: ip.xBD4_grabRange.x,
                w: ip.xBD4_grabRange.y,
            };
            drew |= coll_draw::range(objs, range, ip.pos, PICKUP_RANGE, pass, ip.facing_dir);
        }
        if flag(ip.xDAA, 3) && flag(ip.xDD0_flag, 0) && coll_draw::stomp(objs, &ip.xB54, pass) {
            drew = true;
        }
        drew
    }

    /// The model part of `it_8026EECC`.
    fn render_item_model(&mut self, gobj: GObjId, pass: i32) {
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
            1 if outlined => {
                // `it_8026EECC_inline_1`, `_inline_2` and `_inline_3`: the outline's two
                // passes around the model's.
                let lists = super::kinds::gamewatch::outline_lists(self, gobj);
                for (b4, b5) in [(true, false), (false, false), (true, true)] {
                    let ip = item_mut(self.game, gobj);
                    set_flag(&mut ip.xDCF_flag, 4, b4);
                    set_flag(&mut ip.xDCF_flag, 5, b5);
                    let shake = flag(ip.xDCF_flag, 7).then_some(pos);
                    if b4 {
                        self.item_bones_hidden(gobj, lists.0, true);
                        self.item_bones_hidden(gobj, lists.1, false);
                        self.draw_item_tree(gobj, pass, shake);
                        self.item_bones_hidden(gobj, lists.0, false);
                        self.item_bones_hidden(gobj, lists.1, true);
                    } else {
                        self.draw_item_tree(gobj, pass, shake);
                    }
                }
            }
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
