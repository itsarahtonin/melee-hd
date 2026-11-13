//! A fighter freed while its scene runs (`Fighter_Unload_8006DABC`, the fighter GObj's user
//! data remover): what other fighters and items keep of it let go, its effects, camera box,
//! dynamic bones, sounds, shadow, models and lights released, its player slot's entity
//! cleared, and its blocks back to their pools. A player leaving a match in progress
//! (Training's computer players, a single-player mode's) frees its fighters so
//! (`Player_80031EBC`); Training's reset puts a slot's fighters back where they started
//! (`Player_800328D4`).
//!
//! Each block goes back holding what the fighter left in it (docs/engine.md, a session's heap
//! leftovers): a fighter made over it later in the scene takes its pool's slot as it was, and
//! a later scene's objects take the heap image's layout.

use super::super::handles::FighterId;
use super::super::state::Running;
use crate::hsd::gobj::GObjId;
use crate::lb::hitcapsule::HitOwner;
use crate::vec::Vec3;

const P_LINK_FIGHTER: u8 = 8;
/// `FTKIND_PURIN`, the one kind with a user data remover (`ftData_OnUserDataRemove`).
const PURIN: i32 = 15;

impl Running<'_> {
    /// The fighters' GObjs in their process link's order, with each one's fighter: `gobj`'s
    /// is `f` (its user data is being removed).
    fn fighters_with(&self, gobj: GObjId, f: FighterId) -> Vec<(GObjId, FighterId)> {
        let w = &self.game.world;
        let mut out = Vec::new();
        let mut cur = w.first_in(P_LINK_FIGHTER);
        while let Some(g) = cur {
            let id = if g == gobj {
                f
            } else {
                self.game.fighter_id(g)
            };
            out.push((g, id));
            cur = w.gobj(g).next;
        }
        out
    }

    /// Frees a fighter's user data, `Fighter_Unload_8006DABC`, while its GObj still holds it
    /// (`GObj_RemoveUserData`): its kind's remover (Jigglypuff's hat), the other fighters'
    /// thrown hitboxes and grabs forgetting it, its deferred spawns, every item's `evt_unk`
    /// (`it_8026B7F8`), its camera box, dynamic bones, sounds and shadow, its accessory's,
    /// skeleton's, second joints' and lights' models, its kind's shared flag, its player
    /// slot's entity, and its seven blocks.
    #[doc(alias = "Fighter_Unload_8006DABC")]
    pub fn fighter_unload(&mut self, gobj: GObjId, f: FighterId) {
        if self.game.fighters[f.0].kind == PURIN {
            self.purin_on_user_data_remove(f);
        }
        self.forget_thrower(gobj, f);
        let mut head = self.game.fighters[f.0].x60C.take();
        super::super::ef::queue_clear(self.game, &mut head);
        self.game.fighters[f.0].x60C = head;
        super::super::it::evt_unk::fighter_unloaded(self, gobj);
        if let Some(b) = self.game.fighters[f.0].x890_cameraBox {
            super::super::camera::free_box(&mut self.game.camera.boxes, b);
        }
        self.unload_dynamic_bones(f);
        self.forget_grabber(gobj, f);
        self.stop_fighter_sounds(gobj);
        // `lbShadow_8000EE8C`: `HSD_ShadowRemove` when it has one.
        let shadow = self.game.fighters[f.0].x20A4.shadow;
        crate::hsd::shadow::remove(&mut self.game.objs, shadow);
        let fp = &self.game.fighters[f.0];
        let (accessory, skeleton, x2184, lights) =
            (fp.x20A0_accessory, fp.x8AC_animSkeleton, fp.x2184, fp.x588);
        if accessory.is_some() {
            crate::hsd::jobj::remove_all(&mut self.game.objs, accessory);
        }
        crate::hsd::jobj::remove_all(&mut self.game.objs, skeleton);
        crate::hsd::jobj::unref(&mut self.game.objs, x2184);
        self.forget_kind_flag(gobj, f);
        crate::hsd::lobj::remove_all(&mut self.game.objs, lights);
        let fp = &self.game.fighters[f.0];
        let (slot, sub) = (i32::from(fp.player_idx), i32::from(fp.is_sub_fighter));
        self.game.player_80031fb0(slot, sub);
        self.free_fighter_blocks(f);
    }

    /// Jigglypuff's user data remover, `ftPr_Init_OnUserDataRemove` (`ftPr_Init_8013C494`):
    /// her hat's model and its DObj list freed, if she wears one.
    // Ports `ftPr_Init_8013C494`; aliased once a check reaches it (a Jigglypuff in a hat
    // leaving).
    #[doc(alias = "ftPr_Init_OnUserDataRemove")]
    fn purin_on_user_data_remove(&mut self, f: FighterId) {
        use super::purin::u;
        /// `x2240.data`'s offset in `u`.
        const LIST: usize = 0x18;
        let Some(hat) = self.game.fighters[f.0].u.jobj_at(u::HAT) else {
            return;
        };
        crate::hsd::jobj::remove_all(&mut self.game.objs, Some(hat));
        self.game.fighters[f.0].u.set_jobj_at(u::HAT, None);
        if let Some(list) = self.game.fighters[f.0].u.dobj_list_at(LIST) {
            self.free_x2040_list(list);
        }
        self.game.fighters[f.0].u.set_dobj_list_at(LIST, None);
    }

    /// A DObj list back to `fighter_x2040_alloc_data` (`HSD_ObjFree`), holding its entries.
    fn free_x2040_list(&mut self, list: super::super::handles::DObjListId) {
        let g = &*self.game;
        let data = self.data;
        crate::hsd::export::with_image(|img| super::super::export::x2040_list(g, data, img, list));
        let at = self.game.fighter_dobj_lists.addr(list.0);
        crate::hsd::heap::obj_free(crate::hsd::heap::Kind::FighterX2040, at);
        self.game.fighter_dobj_lists.free(list.0);
    }

    /// Every other fighter thrown by this one forgets its thrower, `ftColl_8007B8E8`.
    #[doc(alias = "ftColl_8007B8E8")]
    fn forget_thrower(&mut self, gobj: GObjId, f: FighterId) {
        for (g, id) in self.fighters_with(gobj, f) {
            if g == gobj {
                continue;
            }
            let hit = &mut self.game.fighters[id.0].x1064_thrownHitbox;
            if hit.x134 == HitOwner::Thrower(gobj) {
                hit.x134 = HitOwner::default();
            }
        }
    }

    /// Every fighter grabbing this one (`x2094`) lets go of it, `ftColl_800765AC`: itself
    /// included.
    #[doc(alias = "ftColl_800765AC")]
    fn forget_grabber(&mut self, gobj: GObjId, f: FighterId) {
        for (_, id) in self.fighters_with(gobj, f) {
            let fp = &mut self.game.fighters[id.0];
            if fp.x2094 == Some(gobj) {
                fp.x2094 = None;
            }
        }
    }

    /// The fighter's shared flag (`ft_8045993C[x61C].x6_b0`) cleared unless another fighter
    /// shares its `x61C`, `ftData_800859A8`.
    #[doc(alias = "ftData_800859A8")]
    fn forget_kind_flag(&mut self, gobj: GObjId, f: FighterId) {
        let x61c = self.game.fighters[f.0].x61C;
        if x61c == -1 {
            return;
        }
        for (_, id) in self.fighters_with(gobj, f) {
            if id != f && self.game.fighters[id.0].x61C == x61c {
                return;
            }
        }
        self.game.ft.data.slot_words[x61c as usize].x6 &= !0x80;
    }

    /// The unload's `HSD_ObjFree`s: the two animation buffers, the parts table, the DObj
    /// lists, the attributes' copy and the fighter, each laid out first as it goes back.
    fn free_fighter_blocks(&mut self, f: FighterId) {
        use crate::hsd::heap::Kind;
        let (g, data) = (&*self.game, self.data);
        crate::hsd::export::with_image(|img| {
            super::super::export::fighter_parts(g, data, img, f);
            super::super::export::fighter(g, data, img, f);
        });
        // And the slot keeps its counted arrays' bytes for the next fighter made there.
        super::super::export::keep_fighter_arrays(self.game, f);
        let fp = &self.game.fighters[f.0];
        let (b1, b2, parts, list, second, attrs) = (
            fp.x59C,
            fp.x5A0,
            fp.parts,
            fp.dobj_list.data,
            fp.x203C.data,
            fp.dat_attrs_backup,
        );
        for b in [b1, b2].into_iter().flatten() {
            let g = &*self.game;
            crate::hsd::export::with_image(|img| super::super::export::anim_buf(g, img, b.0));
            // `HSD_ObjFree` links the buffer in by its first word: `x59C` goes first, so
            // `x5A0`'s holds `x59C`'s address and `x59C`'s the head before it.
            let head = crate::hsd::heap::with(|h| {
                h.allocs[Kind::FighterX59C as usize]
                    .free
                    .last()
                    .copied()
                    .unwrap_or(0)
            });
            if let Some(head) = head {
                self.game.fighter_anim_bufs[b.0].freed(head);
            }
            self.game.fighter_anim_bufs.free(b.0);
        }
        if let Some(p) = parts {
            self.game.fighter_parts.free(p.0);
        }
        if let Some(l) = list {
            let at = self.game.fighter_dobj_lists.addr(l.0);
            crate::hsd::heap::obj_free(Kind::FighterDObjList, at);
            self.game.fighter_dobj_lists.free(l.0);
        }
        if let Some(l) = second {
            self.free_x2040_list(l);
        }
        if let Some(a) = attrs {
            self.game.fighter_attrs.free(a.0);
        }
        self.game.fighters.free(f.0);
    }

    /// Frees a slot's fighters, `Player_80031EBC`: each taken out of play first unless it
    /// sleeps (`ftCo_800D4F24` with its puff), then its GObj.
    #[doc(alias = "Player_80031EBC")]
    pub fn player_free_fighters(&mut self, slot: i32) {
        for i in 0..2 {
            if let Some(g) = self.game.player_entity_at(slot, i) {
                if !self.game.ft_is_sleeping(g) {
                    self.remove_with_partner(g, true);
                }
                crate::hsd::gobj::free(self, g);
            }
        }
    }

    /// A slot's fighters moved to `pos` and back into play there, `Player_800328D4`.
    #[doc(alias = "Player_800328D4")]
    pub fn player_move_to(&mut self, slot: i32, pos: Vec3) {
        self.game.set_player_pos(slot, pos);
        for i in 0..2 {
            if let Some(g) = self.game.player_entity_at(slot, i) {
                self.fighter_respawn_here(g);
            }
        }
    }

    /// A fighter back into play where its slot says, unless it sleeps, `ftLib_80087140`: out
    /// of play with its puff (`ftCo_800D4F24`), its rumble stopped (`ftCommon_8007ED2C`),
    /// spawned anew (`Fighter_Spawn`), then falling or waiting (`ftCommon_8007D92C`).
    #[doc(alias = "ftLib_80087140")]
    fn fighter_respawn_here(&mut self, gobj: GObjId) {
        if self.game.ft_is_sleeping(gobj) {
            return;
        }
        self.remove_with_partner(gobj, true);
        self.stop_port_rumble(gobj);
        self.fighter_spawn(gobj);
        self.fall_or_wait(gobj);
    }

    /// The fighter's port's rumble stopped, `ftCommon_8007ED2C` (`lb_800145C0`).
    #[doc(alias = "ftCommon_8007ED2C")]
    fn stop_port_rumble(&mut self, gobj: GObjId) {
        let port = self.game.fighter(gobj).pad_port;
        self.game.pads.rumble.remove(port);
        self.game.pads.rumble.on(port);
    }
}
