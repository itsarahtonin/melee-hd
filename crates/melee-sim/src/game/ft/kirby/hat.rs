//! His copies' hats (`ftkirby.c`): a hat model of its own drawn on his head (`hat.jobj`, its
//! DObjs listed in `hat.x14`), and the hat's model variants (`hat.x24`), shown and hidden with
//! his own.

use melee_data::archive::DataRef;

use super::super::super::handles::{DObjListId, FighterId};
use super::super::super::state::Running;
use super::super::KIRBY_COSTUME_KINDS;
use super::super::bind::{DataStack, JointStack, next_joint};
use super::super::dynamics::BoneSlot;
use super::super::parts::{MAX_SECOND_DOBJS, set_bone_bit};
use super::{KIND_KIRBY, u};
use crate::hsd::dobj;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::hsd::mobj::{self, MObjClass};
use crate::hsd::pobj::{self, PObjClass};

/// Where `ftParts_800753D4` keeps its descriptor copy as a copy's hat is set up from the
/// swallow: 0x24 into its frame, under the frames of the calls that lead there from a
/// fighter's animation callback (`Fighter_procAnim`, a GObj process, so always at the same
/// depth): `ftKb_SpecialNDrink_Anim` or `ftKb_SpecialAirNDrink_Anim` (0x40 each), then
/// `ftKb_SpecialN_800F1BAC` (0x28), `800F10D4` (0xB8), `800EF040` (0x38) and
/// `ftParts_800753D4` (0x80), as ssbm-rs's ports size them. ssbm-rs's probe logs the key at
/// 0x804EE8EC (2026-10-10_12-54-40's tick 8579,
/// `$SSBM_RS_LOCAL/research/card-chain/vs-12-54-40-8579.log`), so the callback runs with the
/// stack at `ANIM_CB_SP`. A copy carried over a death (`ftKb_Init_OnDeath`) comes another way,
/// which isn't ported.
const ANIM_CB_SP: u32 = 0x804E_EAA0;
const KIRBY_COPY_STACK: u32 = ANIM_CB_SP - (0x40 + 0x28 + 0xB8 + 0x38 + 0x80) + 0x24;

/// `u` offsets of the hat's model variants (`hat.x24`, an `FtPartsVis`): the variants'
/// count, which lookups are shown, and the lookups.
const VIS_MODEL_NUM: usize = 0x24;
const VIS_CLEARED: usize = 0x28;
const VIS_LOOKUPS: usize = 0x30;
/// `u` offsets of a copy costume's animated textures (`x44`, a `CostumeTObjList`): their
/// count, their indices, and the textures.
const TEX_COUNT: usize = 0x44;
const TEX_INDICES: usize = 0x48;
const TEX_TOBJS: usize = 0x4C;
/// How deep `ftAnim_GetNextMatAnimJointInTree` follows a tree.
const MAX_JOINT_DEPTH: usize = 30;
/// The parts a parts table has room for (`ftKb_LoadHatParts`'s index array).
const MAX_PARTS: usize = 0x8C;

impl Running<'_> {
    /// A copy's hat model put on, `ftKb_LoadHat` (inlined in each kind's): its DObj list from
    /// the fighters' pool, its model loaded with the fighters' shapes (`hat_joint`), its DObjs
    /// listed (`ftParts_80075650`) and its variants hidden (`ftParts_8007487C` with the
    /// copy's `desc`, the first costume's).
    fn kb_load_hat(&mut self, gobj: GObjId, kind: i32) {
        let f = self.game.fighter_id(gobj);
        let copy = self.game.ft.kirby.copies[kind as usize].expect("a copy without its data");
        let list = DObjListId(
            self.game
                .fighter_dobj_lists
                .alloc(vec![None; MAX_SECOND_DOBJS]),
        );
        let addr = crate::hsd::heap::obj_alloc(crate::hsd::heap::Kind::FighterX2040);
        self.game.fighter_dobj_lists.set_addr(list.0, addr);
        self.game.fighters[f.0]
            .u
            .set_dobj_list_at(u::HAT_DOBJS, Some(list));
        pobj::set_default_class(&mut self.game.objs.pobjs, PObjClass::Fighter);
        let joint = self.data.ptr(copy);
        let hat = jobj::load_joint(&mut self.game.objs, self.data, joint);
        let fp = &mut self.game.fighters[f.0];
        fp.u.set_jobj_at(u::HAT_JOBJ, hat);
        fp.x2225_b2 = true;
        pobj::set_default_class(&mut self.game.objs.pobjs, PObjClass::Hsd);
        self.purin_list_hat_dobjs(hat, list);
        self.kb_hat_vis_init(f, copy.at(4), 0, u::HAT_DOBJS, u::HAT_DOBJS);
    }

    /// A copy's hat model taken off (`ftKb_SpecialN_800EFAF0`'s inline): the model and its
    /// DObjs gone, and its list back to the pool.
    fn kb_unload_hat(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let Some(hat) = self.game.fighters[f.0].u.jobj_at(u::HAT_JOBJ) else {
            return;
        };
        jobj::remove_all(&mut self.game.objs, Some(hat));
        let fp = &mut self.game.fighters[f.0];
        fp.u.set_jobj_at(u::HAT_JOBJ, None);
        let list = fp.u.dobj_list_at(u::HAT_DOBJS);
        fp.u.set_dobj_list_at(u::HAT_DOBJS, None);
        if let Some(l) = list {
            let addr = self.game.fighter_dobj_lists.addr(l.0);
            crate::hsd::heap::obj_free(crate::hsd::heap::Kind::FighterX2040, addr);
            self.game.fighter_dobj_lists.free(l.0);
        }
    }

    /// Samus's copy's hat put on, `ftKb_SpecialN_800F04F8`, unless he wears one.
    #[doc(alias = "ftKb_SpecialN_800F04F8")]
    pub fn kb_samus_hat_on(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).u.jobj_at(u::HAT_JOBJ).is_none() {
            self.kb_load_hat(gobj, 13);
        }
    }

    /// Samus's copy's hat taken off, `ftKb_SpecialN_800F05A8`.
    #[doc(alias = "ftKb_SpecialN_800F05A8")]
    pub fn kb_samus_hat_off(&mut self, gobj: GObjId) {
        self.kb_unload_hat(gobj);
    }

    /// A copy's hat on his head, the kinds' `ftKb_LoadHat` pairs: put on unless he wears one.
    fn kb_head_hat_on(&mut self, gobj: GObjId, kind: i32) {
        if self.game.fighter(gobj).u.jobj_at(u::HAT_JOBJ).is_none() {
            self.kb_load_hat(gobj, kind);
        }
    }

    /// Mario's copy's hat put on, `ftKb_SpecialN_800EFA40`.
    #[doc(alias = "ftKb_SpecialN_800EFA40")]
    pub fn kb_mario_hat_on(&mut self, gobj: GObjId) {
        self.kb_head_hat_on(gobj, 0);
    }

    /// Captain Falcon's copy's hat put on, `ftKb_SpecialN_800EFC58`.
    #[doc(alias = "ftKb_SpecialN_800EFC58")]
    pub fn kb_captain_hat_on(&mut self, gobj: GObjId) {
        self.kb_head_hat_on(gobj, 2);
    }

    /// Luigi's copy's hat put on, `ftKb_SpecialN_800F0710`.
    #[doc(alias = "ftKb_SpecialN_800F0710")]
    pub fn kb_luigi_hat_on(&mut self, gobj: GObjId) {
        self.kb_head_hat_on(gobj, 17);
    }

    /// Pikachu's copy's hat put on, `ftKb_SpecialN_800F03DC`, with its ears' dynamic bones
    /// (`ftCo_8009D2A4`).
    #[doc(alias = "ftKb_SpecialN_800F03DC")]
    pub fn kb_pikachu_hat_on(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).u.jobj_at(u::HAT_JOBJ).is_none() {
            self.kb_load_hat(gobj, 12);
            let f = self.game.fighter_id(gobj);
            self.kb_hat_chains(f, 12, 2);
        }
    }

    /// A copy's hat with its dynamic bones put on: the hat unless he wears one, and its
    /// chains from the copy's `hat_dynamics[dyn_idx]`.
    fn kb_head_hat_chains_on(&mut self, gobj: GObjId, kind: i32, dyn_idx: u32) {
        if self.game.fighter(gobj).u.jobj_at(u::HAT_JOBJ).is_none() {
            self.kb_load_hat(gobj, kind);
            let f = self.game.fighter_id(gobj);
            self.kb_hat_chains(f, kind, dyn_idx);
        }
    }

    /// Sheik's copy's hat put on, `ftKb_SpecialN_800EFF9C`, with its dynamic bones
    /// (`ftCo_8009D704`).
    #[doc(alias = "ftKb_SpecialN_800EFF9C")]
    #[doc(alias = "ftCo_8009D704")]
    pub fn kb_seak_hat_on(&mut self, gobj: GObjId) {
        self.kb_head_hat_chains_on(gobj, 7, 2);
    }

    /// Zelda's copy's hat put on, `ftKb_SpecialN_800F0938`, with its dynamic bones from the
    /// copy's first dynamics (`ftCo_8009D5EC`).
    #[doc(alias = "ftKb_SpecialN_800F0938")]
    #[doc(alias = "ftCo_8009D5EC")]
    pub fn kb_zelda_hat_on(&mut self, gobj: GObjId) {
        self.kb_head_hat_chains_on(gobj, 19, 0);
    }

    /// Fox's copy's hat put on, `ftKb_SpecialN_800EFB4C`.
    #[doc(alias = "ftKb_SpecialN_800EFB4C")]
    pub fn kb_fox_hat_on(&mut self, gobj: GObjId) {
        self.kb_head_hat_on(gobj, 1);
    }

    /// Yoshi's copy's hat put on, `ftKb_SpecialN_800F0604`.
    #[doc(alias = "ftKb_SpecialN_800F0604")]
    pub fn kb_yoshi_hat_on(&mut self, gobj: GObjId) {
        self.kb_head_hat_on(gobj, 14);
    }

    /// Bowser's copy's hat put on, `ftKb_SpecialN_800EFD64`, with its dynamic bones from the
    /// copy's second dynamics (`ftCo_8009D4D4`).
    #[doc(alias = "ftKb_SpecialN_800EFD64")]
    #[doc(alias = "ftCo_8009D4D4")]
    pub fn kb_koopa_hat_on(&mut self, gobj: GObjId) {
        self.kb_head_hat_chains_on(gobj, 5, 1);
    }

    /// Link's copy's hat put on, `ftKb_SpecialN_800EFE80`, with its dynamic bones
    /// (`ftCo_8009D074`).
    #[doc(alias = "ftKb_SpecialN_800EFE80")]
    #[doc(alias = "ftCo_8009D074")]
    pub fn kb_link_hat_on(&mut self, gobj: GObjId) {
        self.kb_head_hat_chains_on(gobj, 6, 2);
    }

    /// Young Link's copy's hat put on, `ftKb_SpecialN_800F0A54`, with its dynamic bones
    /// (`ftCo_8009D18C`).
    #[doc(alias = "ftKb_SpecialN_800F0A54")]
    #[doc(alias = "ftCo_8009D18C")]
    pub fn kb_clink_hat_on(&mut self, gobj: GObjId) {
        self.kb_head_hat_chains_on(gobj, 20, 2);
    }

    /// Ness's copy's hat put on, `ftKb_SpecialN_800F00B8`.
    #[doc(alias = "ftKb_SpecialN_800F00B8")]
    pub fn kb_ness_hat_on(&mut self, gobj: GObjId) {
        self.kb_head_hat_on(gobj, 8);
    }

    /// Peach's copy's hat put on, `ftKb_SpecialN_800F01C4`.
    #[doc(alias = "ftKb_SpecialN_800F01C4")]
    pub fn kb_peach_hat_on(&mut self, gobj: GObjId) {
        self.kb_head_hat_on(gobj, 9);
    }

    /// The Ice Climbers' copy's hat put on, `ftKb_SpecialN_800F02D0`.
    #[doc(alias = "ftKb_SpecialN_800F02D0")]
    pub fn kb_popo_hat_on(&mut self, gobj: GObjId) {
        self.kb_head_hat_on(gobj, 10);
    }

    /// Dr. Mario's copy's hat put on, `ftKb_SpecialN_800F0B70`.
    #[doc(alias = "ftKb_SpecialN_800F0B70")]
    pub fn kb_drmario_hat_on(&mut self, gobj: GObjId) {
        self.kb_head_hat_on(gobj, 21);
    }

    /// Ganondorf's copy's hat put on, `ftKb_SpecialN_800F0D98`.
    #[doc(alias = "ftKb_SpecialN_800F0D98")]
    pub fn kb_ganon_hat_on(&mut self, gobj: GObjId) {
        self.kb_head_hat_on(gobj, 25);
    }

    /// Marth's copy's hat put on, `ftKb_SpecialN_800F081C`, with its dynamic bones from the
    /// copy's second dynamics (`ftCo_8009D920`).
    #[doc(alias = "ftKb_SpecialN_800F081C")]
    #[doc(alias = "ftCo_8009D920")]
    pub fn kb_mars_hat_on(&mut self, gobj: GObjId) {
        self.kb_head_hat_chains_on(gobj, 18, 1);
    }

    /// Pichu's copy's hat put on, `ftKb_SpecialN_800F0C7C`, with its ears' dynamic bones
    /// (`ftCo_8009D3BC`).
    #[doc(alias = "ftKb_SpecialN_800F0C7C")]
    #[doc(alias = "ftCo_8009D3BC")]
    pub fn kb_pichu_hat_on(&mut self, gobj: GObjId) {
        self.kb_head_hat_chains_on(gobj, 23, 2);
    }

    /// Roy's copy's hat put on, `ftKb_SpecialN_800F0EA4`, with its dynamic bones from the
    /// copy's second dynamics (`ftCo_8009DA38`).
    #[doc(alias = "ftKb_SpecialN_800F0EA4")]
    #[doc(alias = "ftCo_8009DA38")]
    pub fn kb_emblem_hat_on(&mut self, gobj: GObjId) {
        self.kb_head_hat_chains_on(gobj, 26, 1);
    }

    /// The hat taken off for Mario's, Fox's, Captain Falcon's, Luigi's, Ness's, Peach's,
    /// Yoshi's, the Ice Climbers', Dr. Mario's and Ganondorf's copies
    /// (`ftKb_SpecialN_800EFAF0`, `_800EFBFC`, `_800EFD08`, `_800F07C0`, `_800F0168`,
    /// `_800F0274`, `_800F06B4`, `_800F0380`, `_800F0C20`, `_800F0E48`).
    #[doc(alias = "ftKb_SpecialN_800EFAF0")]
    #[doc(alias = "ftKb_SpecialN_800EFBFC")]
    #[doc(alias = "ftKb_SpecialN_800F06B4")]
    #[doc(alias = "ftKb_SpecialN_800EFD08")]
    #[doc(alias = "ftKb_SpecialN_800F07C0")]
    #[doc(alias = "ftKb_SpecialN_800F0168")]
    #[doc(alias = "ftKb_SpecialN_800F0274")]
    #[doc(alias = "ftKb_SpecialN_800F0380")]
    #[doc(alias = "ftKb_SpecialN_800F0C20")]
    #[doc(alias = "ftKb_SpecialN_800F0E48")]
    pub fn kb_head_hat_off(&mut self, gobj: GObjId) {
        self.kb_unload_hat(gobj);
    }

    /// Pikachu's, Sheik's, Zelda's, Bowser's (and Giga Bowser's), Link's, Young Link's,
    /// Marth's, Pichu's and Roy's copies' hats taken off (`ftKb_SpecialN_800F0494`,
    /// `_800F0054`, `_800F09F0`, `_800EFE1C`, `_800EFF38`, `_800F0B0C`, `_800F08D4`,
    /// `_800F0D34`, `_800F0F5C`), and their dynamic bones (`ftCo_UnloadDynamicBones`).
    #[doc(alias = "ftKb_SpecialN_800F0494")]
    #[doc(alias = "ftKb_SpecialN_800EFE1C")]
    #[doc(alias = "ftKb_SpecialN_800EFF38")]
    #[doc(alias = "ftKb_SpecialN_800F0B0C")]
    #[doc(alias = "ftKb_SpecialN_800F0054")]
    #[doc(alias = "ftKb_SpecialN_800F09F0")]
    #[doc(alias = "ftKb_SpecialN_800F08D4")]
    #[doc(alias = "ftKb_SpecialN_800F0D34")]
    #[doc(alias = "ftKb_SpecialN_800F0F5C")]
    pub fn kb_pikachu_hat_off(&mut self, gobj: GObjId) {
        self.kb_unload_hat(gobj);
        let f = self.game.fighter_id(gobj);
        self.unload_dynamic_bones(f);
    }

    /// The fighter's dynamic bone chains given back, `ftCo_UnloadDynamicBones`: none left.
    #[doc(alias = "ftCo_UnloadDynamicBones")]
    pub fn unload_dynamic_bones(&mut self, f: FighterId) {
        let n = self.game.fighters[f.0].dynamics_num;
        for i in 0..n.max(0) as usize {
            let mut desc = self.game.fighters[f.0].dynamic_bone_sets[i]
                .dyn_desc
                .clone();
            self.game.free_dynamics_chain(&mut desc);
            self.game.fighters[f.0].dynamic_bone_sets[i].dyn_desc = desc;
        }
        self.game.fighters[f.0].dynamics_num = 0;
    }

    /// A copy hat's dynamic bone chains (`hat_dynamics[dyn_idx]`), the fighter's own
    /// replaced: each down the hat's tree to the joint its entry names
    /// (`ftCo_SetupKirbyHatBone`), as Pikachu's copy's `ftCo_8009D2A4` and the others set them.
    #[doc(alias = "ftCo_8009D2A4")]
    fn kb_hat_chains(&mut self, f: FighterId, kind: i32, dyn_idx: u32) {
        let copy = self.game.ft.kirby.copies[kind as usize].expect("a copy without its data");
        let dynamics = self
            .data
            .ptr(copy.at(0xC + 4 * dyn_idx))
            .expect("a copy hat without dynamics");
        let n = self.data.u32(dynamics) as i32;
        self.game.fighters[f.0].dynamics_num = n;
        assert!(
            n < 10,
            "the original's assertion failed: fighter dynamics num over!"
        );
        for i in 0..n.max(0) as u32 {
            let bones = self.data.ptr(dynamics.at(4)).expect("ftDynamics bones");
            let entry = bones.at(0x18 * i);
            let steps = self.data.u32(entry) as i32;
            let mut cur = self.game.fighters[f.0]
                .u
                .jobj_at(u::HAT_JOBJ)
                .expect("a hat's chains without the hat");
            for _ in 0..steps {
                if let Some(c) = self.game.objs.jobjs[cur].child {
                    cur = c;
                    continue;
                }
                while self.game.objs.jobjs[cur].next.is_none() {
                    cur = self.game.objs.jobjs[cur]
                        .parent
                        .expect("a hat's chain past its tree");
                }
                cur = self.game.objs.jobjs[cur].next.expect("a next joint");
            }
            let count = self.data.u32(entry.at(8));
            let k = i as usize;
            let mut desc = self.game.fighters[f.0].dynamic_bone_sets[k]
                .dyn_desc
                .clone();
            self.game
                .load_dynamics_chain(self.data, Some(cur), &mut desc, count);
            let fp = &mut self.game.fighters[f.0];
            fp.dynamic_bone_sets[k].dyn_desc = desc;
            fp.dynamic_bone_sets[k].bone_id = BoneSlot::Id(0);
            let mut desc = self.game.fighters[f.0].dynamic_bone_sets[k]
                .dyn_desc
                .clone();
            self.game
                .set_dynamics_params(self.data, entry.at(4), &mut desc);
            self.game.fighters[f.0].dynamic_bone_sets[k].dyn_desc = desc;
        }
    }

    /// A copy costume put on (a hat of parts on his own bones), `ftKb_LoadHatParts` (inlined
    /// in each kind's): the costume's optional parts hung on his model
    /// (`ftKb_SpecialN_800EF040`), its two DObj lists from the fighters' pool, the costume's
    /// DObjs onto his bones (`ftKb_SpecialN_800EF0E4`) with their material animations
    /// (`ftKb_SpecialN_800EF35C`) and its low-poly model's (`ftKb_SpecialN_800EF438`), its
    /// variants hidden (`ftParts_8007487C`) and its animated textures found
    /// (`ftAnim_80070200`).
    fn kb_load_hat_parts(&mut self, gobj: GObjId, kind: i32) {
        let f = self.game.fighter_id(gobj);
        let copy = self.game.ft.kirby.copies[kind as usize].expect("a copy without its data");
        self.kb_hat_add_parts(f, kind, copy);
        for off in [u::HAT_DOBJS, u::HAT_DOBJS2] {
            let l = DObjListId(
                self.game
                    .fighter_dobj_lists
                    .alloc(vec![None; MAX_SECOND_DOBJS]),
            );
            let addr = crate::hsd::heap::obj_alloc(crate::hsd::heap::Kind::FighterX2040);
            self.game.fighter_dobj_lists.set_addr(l.0, addr);
            self.game.fighters[f.0].u.set_dobj_list_at(off, Some(l));
        }
        let mut indices = [None; MAX_PARTS];
        self.kb_hat_costume_dobjs(f, kind, &mut indices);
        self.kb_hat_costume_matanims(f, kind, &indices);
        self.kb_hat_low_dobjs(f, copy);
        let costume = u32::from(self.game.fighters[f.0].costume_id);
        self.kb_hat_vis_init(f, copy, costume, u::HAT_DOBJS, u::HAT_DOBJS2);
        self.kb_hat_textures_init(f, copy.at(8));
    }

    /// The copy costume's model and material animation for Kirby's costume
    /// (`ftKb_Init_803C9FC8[kind][costume]`).
    fn kb_copy_costume(&self, f: FighterId, kind: i32) -> (Option<DataRef>, Option<DataRef>) {
        let i = KIRBY_COSTUME_KINDS
            .iter()
            .position(|&k| k == kind as usize)
            .expect("a copy without its costume");
        let costume = usize::from(self.game.fighters[f.0].costume_id);
        self.game.ft.kirby.models[i][costume]
    }

    /// The costume's optional parts (the copy's mask, `hat_dynamics[1]`, over his
    /// `Fighter_804D6540` entries) hung on his model, `ftKb_SpecialN_800EF040`.
    #[doc(alias = "ftKb_SpecialN_800EF040")]
    fn kb_hat_add_parts(&mut self, f: FighterId, kind: i32, copy: DataRef) {
        let mask = self.data.u32(copy.at(0x10));
        if mask == 0 {
            return;
        }
        let (joint, _) = self.kb_copy_costume(f, kind);
        let joint = joint.expect("a copy costume without its model");
        let table = self.load_common(0x14);
        let entry = self
            .data
            .ptr(table.at(4 * KIND_KIRBY as u32))
            .expect("Fighter_804D6540[kind]");
        let count = self.data.u32(entry.at(4));
        let parts = self.data.ptr(entry).expect("Fighter_804D6540[kind]->x0");
        for i in 0..count {
            if mask & (1 << i) != 0 {
                self.attach_part_model(f, parts.at(4 * i), joint, KIRBY_COPY_STACK);
            }
        }
    }

    /// Files each joint of the tree at `root` in the ID table under the part (with a joint,
    /// `flags_b1`) it stands for, in order (`ftKb_SpecialN_insert_joint_refs`).
    fn kb_hat_insert_joint_refs(&mut self, f: FighterId, root: DataRef) {
        let parts = self.game.fighters[f.0]
            .parts
            .expect("a fighter without parts")
            .0;
        let mut joint = Some(root);
        let mut stack = JointStack::new();
        let mut part = 0usize;
        while let Some(j) = joint {
            while self.game.fighter_parts[parts][part].x8[0] & 0x40 == 0 {
                part += 1;
            }
            let p = self.game.fighter_parts[parts][part]
                .joint
                .expect("HSD_IDInsertToTable with a part that has no joint");
            self.game.objs.ids.insert(j, p);
            part += 1;
            joint = next_joint(self.data, j, &mut stack);
        }
    }

    /// Each joint's DObjs of the tree at `root` loaded onto the part it stands for (after the
    /// part's own), listed in the hat's list at `u` offset `list`, the part marked with the
    /// bit `mark` (`byte`, `bit` for [`set_bone_bit`]); `indices` gets each part's first
    /// index in the list. Returns how many are listed.
    fn kb_hat_load_tree(
        &mut self,
        f: FighterId,
        root: DataRef,
        list: usize,
        mark: (usize, u32),
        mut indices: Option<&mut [Option<u32>; MAX_PARTS]>,
    ) -> u32 {
        pobj::set_default_class(&mut self.game.objs.pobjs, PObjClass::Fighter);
        self.kb_hat_insert_joint_refs(f, root);
        let fp = &self.game.fighters[f.0];
        let parts = fp.parts.expect("a fighter without parts").0;
        let l =
            fp.u.dobj_list_at(list)
                .expect("a hat without its DObj list");
        let mut joint = Some(root);
        let mut stack = JointStack::new();
        let (mut part, mut total) = (0usize, 0u32);
        while let Some(j) = joint {
            while self.game.fighter_parts[parts][part].x8[0] & 0x40 == 0 {
                part += 1;
            }
            let part_jobj = self.game.fighter_parts[parts][part].joint;
            let desc = self.data.ptr(j.at(0x10));
            let d = dobj::load_desc(&mut self.game.objs, self.data, desc);
            if let Some(ix) = indices.as_deref_mut() {
                ix[part] = Some(total);
            }
            if let Some(d) = d {
                let existing = jobj::get_dobj(&self.game.objs.jobjs, part_jobj);
                dobj::resolve_refs_all(&mut self.game.objs, self.data, Some(d), desc);
                match existing {
                    None => jobj::add_dobj(&mut self.game.objs, part_jobj, Some(d)),
                    Some(mut last) => {
                        while let Some(n) = self.game.objs.dobjs[last].next {
                            last = n;
                        }
                        // `lb_8000CE30`.
                        self.game.objs.dobjs[last].next = Some(d);
                    }
                }
                let mut cur = Some(d);
                let mut group = 0;
                while let Some(c) = cur {
                    assert!(
                        (total as usize) < MAX_SECOND_DOBJS,
                        "the original's assertion failed: fighter parts model dobj num over!"
                    );
                    self.game.fighter_dobj_lists[l.0][total as usize] = Some(c);
                    if let Some(m) = self.game.objs.dobjs[c].mobj {
                        mobj::change_class(&mut self.game.objs.mobjs, m, MObjClass::Fighter);
                    }
                    cur = self.game.objs.dobjs[c].next;
                    total += 1;
                    group += 1;
                }
                assert!(
                    group < 0x80,
                    "the original's assertion failed: fighter dobj num over!"
                );
                let bone = &mut self.game.fighter_parts[parts][part];
                set_bone_bit(bone, mark.0, mark.1, true);
            }
            part += 1;
            joint = next_joint(self.data, j, &mut stack);
        }
        pobj::set_default_class(&mut self.game.objs.pobjs, PObjClass::Hsd);
        total
    }

    /// The copy costume's DObjs onto his bones (`flags_b6`), in the hat's first list,
    /// `ftKb_SpecialN_800EF0E4`.
    #[doc(alias = "ftKb_SpecialN_800EF0E4")]
    fn kb_hat_costume_dobjs(
        &mut self,
        f: FighterId,
        kind: i32,
        indices: &mut [Option<u32>; MAX_PARTS],
    ) {
        let (joint, _) = self.kb_copy_costume(f, kind);
        let root = joint.expect("a copy costume without its model");
        let total = self.kb_hat_load_tree(f, root, u::HAT_DOBJS, (0, 6), Some(indices));
        self.game.fighters[f.0].set_u_i32(u::HAT_DOBJS_COUNT, total as i32);
    }

    /// The copy costume's material animations on its DObjs, `ftKb_SpecialN_800EF35C`: the
    /// material animation tree's joints stand for the parts but the optional ones, each on
    /// the first DObj its part got.
    #[doc(alias = "ftKb_SpecialN_800EF35C")]
    fn kb_hat_costume_matanims(
        &mut self,
        f: FighterId,
        kind: i32,
        indices: &[Option<u32>; MAX_PARTS],
    ) {
        let (_, matanim) = self.kb_copy_costume(f, kind);
        let l = self.game.fighters[f.0]
            .u
            .dobj_list_at(u::HAT_DOBJS)
            .expect("a hat without its DObj list");
        let mut cur = matanim;
        let mut stack = DataStack::<MAX_JOINT_DEPTH>::new();
        let mut part = 0i32;
        while let Some(ma) = cur {
            if self.part_mask(KIND_KIRBY, part) != 0 {
                part += 1;
                continue;
            }
            if let Some(anim) = self.data.ptr(ma.at(8)) {
                let i = indices[part as usize].expect("a part's DObj index the walk never set");
                let d = self.game.fighter_dobj_lists[l.0][i as usize];
                dobj::add_anim_all(&mut self.game.objs, self.data, d, Some(anim), None);
            }
            part += 1;
            cur = next_matanim_joint(self.data, ma, &mut stack);
        }
    }

    /// The copy's low-poly model's DObjs onto his bones (`flags2_b7`), in the hat's second
    /// list, `ftKb_SpecialN_800EF438`: its tree is the copy's `hat_dynamics[2]`; without one
    /// the list's first entry is null.
    #[doc(alias = "ftKb_SpecialN_800EF438")]
    fn kb_hat_low_dobjs(&mut self, f: FighterId, copy: DataRef) {
        let Some(root) = self.data.ptr(copy.at(0x14)) else {
            let l = self.game.fighters[f.0]
                .u
                .dobj_list_at(u::HAT_DOBJS2)
                .expect("a hat without its second list");
            self.game.fighter_dobj_lists[l.0][0] = None;
            return;
        };
        let total = self.kb_hat_load_tree(f, root, u::HAT_DOBJS2, (1, 7), None);
        self.game.fighters[f.0].set_u_i32(u::HAT_DOBJS2_COUNT, total as i32);
    }

    /// The copy costume's animated textures, stopped, `ftAnim_80070200` on his (`x44`): `desc`
    /// is the count and each costume's indices among the first list's textures.
    fn kb_hat_textures_init(&mut self, f: FighterId, desc: DataRef) {
        let n = self.data.u32(desc);
        assert!(
            n <= 5,
            "the original's assertion failed: fighter tobj num over!"
        );
        let table = self.data.ptr(desc.at(4)).expect("ftData_x8_x8::xC");
        let costume = u32::from(self.game.fighters[f.0].costume_id);
        let indices = self
            .data
            .ptr(table.at(4 * costume))
            .or_else(|| self.data.ptr(table));
        let fp = &mut self.game.fighters[f.0];
        fp.set_u_i32(TEX_COUNT, n as i32);
        fp.u.set_data_at(TEX_INDICES, indices);
        let list = fp.u.dobj_list_at(u::HAT_DOBJS);
        let count = fp.u_i32(u::HAT_DOBJS_COUNT) as u32;
        for i in 0..n {
            let indices = indices.expect("texture animations without their list");
            let index = i32::from(self.data.u16(indices.at(2 * i)));
            let t = self.nth_tobj(list, count, index);
            self.game.fighters[f.0]
                .u
                .set_tobj_at(TEX_TOBJS + 4 * i as usize, Some(t));
            let a = self.game.objs.tobjs[t].aobj;
            assert!(
                a.is_some(),
                "the original's assertion failed: can't find fighter texture anim!"
            );
            crate::hsd::aobj::set_rate(&mut self.game.objs.aobjs, a, 0.0);
        }
    }

    /// A copy costume taken off, `ftKb_SpecialN_800EF69C`: each part that got its DObjs
    /// (`flags_b6`, `flags2_b7`) back to its own (after its last, `xC_u.xD`, in the main or
    /// second model's list) or none, the lists back to the pool (the second's word kept),
    /// and the optional parts taken down (`ftParts_800755E8`, last first).
    #[doc(alias = "ftKb_SpecialN_800EF69C")]
    #[doc(alias = "ftParts_800755E8")]
    fn kb_unload_hat_parts(&mut self, gobj: GObjId, kind: i32) {
        let f = self.game.fighter_id(gobj);
        let copy = self.game.ft.kirby.copies[kind as usize].expect("a copy without its data");
        if let Some(l) = self.game.fighters[f.0].u.dobj_list_at(u::HAT_DOBJS) {
            let fp = &self.game.fighters[f.0];
            let parts = fp.parts.expect("a fighter without parts").0;
            let (main, second) = (fp.dobj_list.data, fp.x203C.data);
            for i in 0..self.parts_table(KIND_KIRBY).len() as usize {
                let bone = &self.game.fighter_parts[parts][i];
                let Some(j) = bone.joint else { continue };
                if bone.x8[0] & 0x02 == 0 && bone.x8[1] & 0x01 == 0 {
                    continue;
                }
                if bone.x8[1] & 0x02 != 0 {
                    let xd = usize::from(bone.xC_u[1] >> 1);
                    let list = if bone.x8[1] & 0x04 != 0 { second } else { main };
                    let d = self.game.fighter_dobj_lists[list.expect("a model's DObj list").0][xd];
                    let next = d.and_then(|d| self.game.objs.dobjs[d].next);
                    dobj::remove_all(&mut self.game.objs, next);
                    if let Some(d) = d {
                        self.game.objs.dobjs[d].next = None;
                    }
                } else {
                    let first = jobj::get_dobj(&self.game.objs.jobjs, Some(j));
                    dobj::remove_all(&mut self.game.objs, first);
                    self.game.objs.jobjs[j].u = jobj::Contents::DObjs(None);
                }
                let bone = &mut self.game.fighter_parts[parts][i];
                set_bone_bit(bone, 0, 6, false);
                set_bone_bit(bone, 1, 7, false);
            }
            let low = self.game.fighters[f.0].u.dobj_list_at(u::HAT_DOBJS2);
            for list in [Some(l), low].into_iter().flatten() {
                let addr = self.game.fighter_dobj_lists.addr(list.0);
                crate::hsd::heap::obj_free(crate::hsd::heap::Kind::FighterX2040, addr);
                self.game.fighter_dobj_lists.free(list.0);
            }
            self.game.fighters[f.0]
                .u
                .set_dobj_list_at(u::HAT_DOBJS, None);
        }
        let mask = self.data.u32(copy.at(0x10));
        if mask == 0 {
            return;
        }
        let table = self.load_common(0x14);
        let entry = self
            .data
            .ptr(table.at(4 * KIND_KIRBY as u32))
            .expect("Fighter_804D6540[kind]");
        let count = self.data.u32(entry.at(4));
        let descs = self.data.ptr(entry).expect("Fighter_804D6540[kind]->x0");
        for i in (0..count).rev() {
            if mask & (1 << i) == 0 {
                continue;
            }
            let part = usize::from(self.data.u8(descs.at(4 * i)));
            let parts = self.game.fighters[f.0]
                .parts
                .expect("a fighter without parts")
                .0;
            let (j, j2) = {
                let b = &self.game.fighter_parts[parts][part];
                (b.joint, b.x4_jobj2)
            };
            jobj::remove(&mut self.game.objs, j);
            jobj::remove(&mut self.game.objs, j2);
            let b = &mut self.game.fighter_parts[parts][part];
            b.joint = None;
            b.x4_jobj2 = None;
            set_bone_bit(b, 0, 1, false);
            set_bone_bit(b, 0, 2, false);
        }
    }

    /// Falco's copy's costume put on, `ftKb_SpecialN_800F130C`, unless he wears one.
    #[doc(alias = "ftKb_SpecialN_800F130C")]
    pub fn kb_falco_hat_on(&mut self, gobj: GObjId) {
        if self
            .game
            .fighter(gobj)
            .u
            .dobj_list_at(u::HAT_DOBJS)
            .is_none()
        {
            self.kb_load_hat_parts(gobj, 22);
        }
    }

    /// Falco's copy's costume taken off, `ftKb_SpecialN_800F13F0`.
    #[doc(alias = "ftKb_SpecialN_800F13F0")]
    pub fn kb_falco_hat_off(&mut self, gobj: GObjId) {
        self.kb_unload_hat_parts(gobj, 22);
    }

    /// Donkey Kong's copy's costume put on, `ftKb_SpecialN_800F0FC0`, unless he wears one.
    #[doc(alias = "ftKb_SpecialN_800F0FC0")]
    pub fn kb_donkey_hat_on(&mut self, gobj: GObjId) {
        if self
            .game
            .fighter(gobj)
            .u
            .dobj_list_at(u::HAT_DOBJS)
            .is_none()
        {
            self.kb_load_hat_parts(gobj, 3);
        }
    }

    /// Donkey Kong's copy's costume taken off, `ftKb_SpecialN_800F10A4`.
    #[doc(alias = "ftKb_SpecialN_800F10A4")]
    pub fn kb_donkey_hat_off(&mut self, gobj: GObjId) {
        self.kb_unload_hat_parts(gobj, 3);
    }

    /// Jigglypuff's copy's costume put on, `ftKb_SpecialN_800F10D4`, unless he wears one, with
    /// its dynamic bones on his own parts from the copy's fourth dynamics (`ftCo_8009D81C`).
    #[doc(alias = "ftKb_SpecialN_800F10D4")]
    #[doc(alias = "ftCo_8009D81C")]
    pub fn kb_purin_hat_on(&mut self, gobj: GObjId) {
        if self
            .game
            .fighter(gobj)
            .u
            .dobj_list_at(u::HAT_DOBJS)
            .is_none()
        {
            self.kb_load_hat_parts(gobj, 15);
            let f = self.game.fighter_id(gobj);
            self.kb_costume_chains(f, 15, 3);
        }
    }

    /// Jigglypuff's copy's costume taken off, `ftKb_SpecialN_800F11AC`, and its dynamic bones.
    #[doc(alias = "ftKb_SpecialN_800F11AC")]
    pub fn kb_purin_hat_off(&mut self, gobj: GObjId) {
        self.kb_unload_hat_parts(gobj, 15);
        let f = self.game.fighter_id(gobj);
        self.unload_dynamic_bones(f);
    }

    /// Mewtwo's copy's costume put on, `ftKb_SpecialN_800F11F0`, unless he wears one, with its
    /// tail's dynamic bones on his own parts from the copy's fifth dynamics (`ftCo_8009DB50`).
    #[doc(alias = "ftKb_SpecialN_800F11F0")]
    #[doc(alias = "ftCo_8009DB50")]
    pub fn kb_mewtwo_hat_on(&mut self, gobj: GObjId) {
        if self
            .game
            .fighter(gobj)
            .u
            .dobj_list_at(u::HAT_DOBJS)
            .is_none()
        {
            self.kb_load_hat_parts(gobj, 16);
            let f = self.game.fighter_id(gobj);
            self.kb_costume_chains(f, 16, 4);
        }
    }

    /// Mewtwo's copy's costume taken off, `ftKb_SpecialN_800F12C8`, and its dynamic bones.
    #[doc(alias = "ftKb_SpecialN_800F12C8")]
    pub fn kb_mewtwo_hat_off(&mut self, gobj: GObjId) {
        self.kb_unload_hat_parts(gobj, 16);
        let f = self.game.fighter_id(gobj);
        self.unload_dynamic_bones(f);
    }

    /// A copy costume's dynamic bone chains (`hat_dynamics[dyn_idx]`), the fighter's own
    /// replaced: each on his own part its entry names, that part's physics on (`flags_b0`),
    /// the chain's bone slot `FtPart_TopN` (`ftCo_8009D81C`, `ftCo_8009DB50`).
    fn kb_costume_chains(&mut self, f: FighterId, kind: i32, dyn_idx: u32) {
        let copy = self.game.ft.kirby.copies[kind as usize].expect("a copy without its data");
        let dynamics = self
            .data
            .ptr(copy.at(0xC + 4 * dyn_idx))
            .expect("a copy costume without dynamics");
        let n = self.data.u32(dynamics) as i32;
        self.game.fighters[f.0].dynamics_num = n;
        assert!(
            n < 10,
            "the original's assertion failed: fighter dynamics num over!"
        );
        let parts = self.game.fighters[f.0]
            .parts
            .expect("a fighter without parts");
        for i in 0..n.max(0) as u32 {
            let bones = self.data.ptr(dynamics.at(4)).expect("ftDynamics bones");
            let entry = bones.at(0x18 * i);
            let bone = self.data.u32(entry) as usize;
            let b = &mut self.game.fighter_parts[parts.0][bone];
            set_bone_bit(b, 0, 0, true);
            let joint = b.joint;
            let count = self.data.u32(entry.at(8));
            let k = i as usize;
            let mut desc = self.game.fighters[f.0].dynamic_bone_sets[k]
                .dyn_desc
                .clone();
            self.game
                .load_dynamics_chain(self.data, joint, &mut desc, count);
            let fp = &mut self.game.fighters[f.0];
            fp.dynamic_bone_sets[k].dyn_desc = desc;
            fp.dynamic_bone_sets[k].bone_id = BoneSlot::Id(0);
            let mut desc = self.game.fighters[f.0].dynamic_bone_sets[k]
                .dyn_desc
                .clone();
            self.game
                .set_dynamics_params(self.data, entry.at(4), &mut desc);
            self.game.fighters[f.0].dynamic_bone_sets[k].dyn_desc = desc;
        }
    }

    /// Mr. Game & Watch's copy's costume put on, `ftKb_SpecialN_800F14B4`, unless he wears one:
    /// its last variant lookup (`hat_dynamics[3]`) in the costume's and his own slot 4, hidden
    /// on the costume, those DObjs' materials in the copy's color (`ftKb_SpecialN_800F1420`),
    /// his outline in its other, and his model's scale again.
    #[doc(alias = "ftKb_SpecialN_800F14B4")]
    #[doc(alias = "ftKb_SpecialN_800F1420")]
    pub fn kb_gamewatch_hat_on(&mut self, gobj: GObjId) {
        if self
            .game
            .fighter(gobj)
            .u
            .dobj_list_at(u::HAT_DOBJS)
            .is_some()
        {
            return;
        }
        self.kb_load_hat_parts(gobj, 24);
        let f = self.game.fighter_id(gobj);
        let copy = self.game.ft.kirby.copies[24].expect("a copy without its data");
        let lookup = self.data.ptr(copy.at(0x18));
        self.game.fighters[f.0]
            .u
            .set_data_at(VIS_LOOKUPS + 16, lookup);
        self.game.fighters[f.0].x5AC.xC[4] = lookup;
        self.kb_hat_lookup(f, 4, false, u::HAT_DOBJS);
        let c = self.kb_gw_color(4);
        let list = self.game.fighters[f.0]
            .u
            .dobj_list_at(u::HAT_DOBJS)
            .expect("a hat without its DObj list");
        let lookup = lookup.expect("FtPartsVisLookup");
        for i in 0..self.data.u32(lookup) {
            let entry = self
                .data
                .ptr(lookup.at(4))
                .expect("FtPartsVisLookup::x4")
                .at(8 * i);
            let bytes = self.data.ptr(entry.at(4));
            for j in 0..self.data.u32(entry) {
                let b = bytes.expect("TempS::x4").at(j);
                let index = usize::from(self.data.u8(b));
                let Some(d) = self.game.fighter_dobj_lists[list.0][index] else {
                    continue;
                };
                if let Some(m) = self.game.objs.dobjs[d].mobj
                    && let Some(mat) = self.game.objs.mobjs[m].mat.as_mut()
                {
                    mat.diffuse = crate::hsd::tobj::Color {
                        r: c.r,
                        g: c.g,
                        b: c.b,
                        a: c.a,
                    };
                }
            }
        }
        self.game.fighters[f.0].x610_color_rgba[1] = self.kb_gw_color(8);
        self.update_model_scale(gobj);
    }

    /// Mr. Game & Watch's copy's costume taken off, `ftKb_SpecialN_800F15D8`: his slot 4
    /// emptied and his model's scale again.
    #[doc(alias = "ftKb_SpecialN_800F15D8")]
    pub fn kb_gamewatch_hat_off(&mut self, gobj: GObjId) {
        self.kb_unload_hat_parts(gobj, 24);
        self.game.fighter_mut(gobj).x5AC.xC[4] = None;
        self.update_model_scale(gobj);
    }

    /// His copy costume's animated textures each back to their first frame and stepped
    /// (`ftAnim_800705E0` on his `x44`).
    pub(super) fn kb_hat_textures_restart(&mut self, f: FighterId) {
        let n = self.game.fighters[f.0].u_i32(TEX_COUNT) as usize;
        for i in 0..n {
            let t = self.game.fighters[f.0]
                .u
                .tobj_at(TEX_TOBJS + 4 * i)
                .expect("a copy costume without its texture");
            let a = self.game.objs.tobjs[t].aobj;
            let objs = &mut self.game.objs;
            crate::hsd::aobj::req_anim(&mut objs.aobjs, &mut objs.fobjs, a, 0.0);
            crate::hsd::tobj::anim(objs, self.data, Some(t));
        }
    }

    /// His copy costume's texture `i` set to `frame` and stepped (`ftAnim_80070458` on his
    /// `x44`); the original asserts it has one.
    pub(super) fn kb_hat_texture_set(&mut self, f: FighterId, i: i32, frame: f32) {
        let fp = &self.game.fighters[f.0];
        assert!(
            (i as u32) < fp.u_i32(TEX_COUNT) as u32,
            "texture no exist! {} {i}",
            fp.player_idx
        );
        let t =
            fp.u.tobj_at(TEX_TOBJS + 4 * i as usize)
                .expect("a copy costume without its texture");
        let a = self.game.objs.tobjs[t].aobj;
        let objs = &mut self.game.objs;
        crate::hsd::aobj::req_anim(&mut objs.aobjs, &mut objs.fobjs, a, frame);
        crate::hsd::tobj::anim(objs, self.data, Some(t));
    }

    /// The hat's model variants, all hidden, `ftParts_8007487C` on the hat's (`hat.x24`):
    /// `desc` is an `FtPartsDesc` (the variants' count and table), `costume` picks the table's
    /// row (the first row's lookups standing in for missing ones), and the lists are the
    /// DObj lists of lookup 2 (`low`) and the rest (`main`), as `u` offsets.
    pub(super) fn kb_hat_vis_init(
        &mut self,
        f: FighterId,
        desc: DataRef,
        costume: u32,
        main: usize,
        low: usize,
    ) {
        let model_num = self.data.u32(desc);
        assert!(
            model_num <= 11,
            "the original's assertion failed: fighter parts model num over!"
        );
        let table = self.data.ptr(desc.at(4)).expect("FtPartsDesc::vis_table");
        let fp = &mut self.game.fighters[f.0];
        fp.u[VIS_MODEL_NUM..VIS_MODEL_NUM + 4].copy_from_slice(&model_num.to_be_bytes());
        for k in 0..4u32 {
            let mine = self.data.ptr(table.at(0x10 * costume + 4 * k));
            let lookup = mine.or_else(|| self.data.ptr(table.at(4 * k)));
            fp.u.set_data_at(VIS_LOOKUPS + 4 * k as usize, lookup);
        }
        fp.u.set_data_at(VIS_LOOKUPS + 16, None);
        for k in 0..5 {
            fp.u[VIS_CLEARED + k] = 1;
        }
        for (idx, list) in [(0, main), (1, main), (2, low), (3, main), (4, main)] {
            self.kb_hat_lookup(f, idx, false, list);
        }
    }

    /// Shows (`ftParts_80074CA0`) or hides (`ftParts_80074D7C`) every DObj one of the hat's
    /// variant lookups names in the DObj list at `u` offset `list`, unless it already is.
    fn kb_hat_lookup(&mut self, f: FighterId, idx: usize, show: bool, list: usize) {
        let fp = &self.game.fighters[f.0];
        let Some(lookup) = fp.u.data_at(VIS_LOOKUPS + 4 * idx) else {
            return;
        };
        let shown = fp.u[VIS_CLEARED + idx] != 0;
        if shown == show {
            return;
        }
        let model_num =
            u32::from_be_bytes(fp.u[VIS_MODEL_NUM..VIS_MODEL_NUM + 4].try_into().unwrap());
        let list =
            fp.u.dobj_list_at(list)
                .expect("a hat without its DObj list");
        for i in 0..model_num {
            let entry = lookup.at(8 * i);
            let n = self.data.u32(entry) as i32;
            for j in 0..n {
                let t = self
                    .data
                    .ptr(entry.at(4))
                    .expect("FtPartsVisLookup::x4")
                    .at(8 * j as u32);
                let count = self.data.u32(t) as i32;
                let bytes = self.data.ptr(t.at(4));
                for k in 0..count {
                    let b = bytes.expect("TempS::x4").at(k as u32);
                    let index = usize::from(self.data.u8(b));
                    let d = self.game.fighter_dobj_lists[list.0][index];
                    let dobjs = &mut self.game.objs.dobjs;
                    if show {
                        crate::hsd::dobj::clear_flags(dobjs, d, 1);
                    } else {
                        crate::hsd::dobj::set_flags(dobjs, d, 1);
                    }
                }
            }
        }
        self.game.fighters[f.0].u[VIS_CLEARED + idx] = u8::from(show);
    }

    /// A model group shown or hidden on his copy's hat too, `ftKb_UnkIntBoolFunc0` (a model
    /// event of `ftParts_800750C8`): nothing without a hat's DObjs. A hat model shows its
    /// variants with his; a hat of parts (on his own bones) shows its own in place of his,
    /// and for the low-poly group its second list, when it has one; while his first variant
    /// is 2 to 6 (his mouth full) the hat's stay hidden.
    #[doc(alias = "ftKb_UnkIntBoolFunc0")]
    pub fn kb_hat_vis(&mut self, f: FighterId, idx: usize, show: bool) {
        let fp = &self.game.fighters[f.0];
        if fp.u.dobj_list_at(u::HAT_DOBJS).is_none() {
            return;
        }
        let model = fp.u.jobj_at(u::HAT_JOBJ).is_some();
        if show {
            if model {
                self.kb_hat_lookup(f, idx, true, u::HAT_DOBJS);
                return;
            }
            if (2..=6).contains(&fp.x5F4_arr[0].idx) {
                self.kb_hat_lookup(f, idx, false, u::HAT_DOBJS);
                return;
            }
            if idx == 2 {
                let low =
                    fp.u.dobj_list_at(u::HAT_DOBJS2)
                        .expect("a hat's second list");
                if self.game.fighter_dobj_lists[low.0][0].is_some() {
                    let second = self.game.fighters[f.0].x203C.data;
                    self.hide_parts_vis(f, 2, second);
                    self.kb_hat_lookup(f, 2, true, u::HAT_DOBJS2);
                }
            } else {
                let main = fp.dobj_list.data;
                self.hide_parts_vis(f, idx, main);
                self.kb_hat_lookup(f, idx, true, u::HAT_DOBJS);
            }
        } else {
            if !model && idx == 2 {
                self.kb_hat_lookup(f, 2, false, u::HAT_DOBJS2);
                return;
            }
            self.kb_hat_lookup(f, idx, false, u::HAT_DOBJS);
        }
    }
}

/// The next material animation joint after `joint` in a depth-first walk,
/// `ftAnim_GetNextMatAnimJointInTree`: its child, else its next sibling, else the nearest
/// ancestor's next sibling (`HSD_MatAnimJoint`: child, next, animation).
fn next_matanim_joint(
    data: &melee_data::archive::Archives,
    joint: DataRef,
    stack: &mut DataStack<MAX_JOINT_DEPTH>,
) -> Option<DataRef> {
    if let Some(child) = data.ptr(joint) {
        stack.push(joint);
        assert!(
            stack.len() < MAX_JOINT_DEPTH,
            "fighter joint depth num over!"
        );
        return Some(child);
    }
    if let Some(next) = data.ptr(joint.at(4)) {
        return Some(next);
    }
    while let Some(parent) = stack.last() {
        stack.pop();
        if let Some(next) = data.ptr(parent.at(4)) {
            return Some(next);
        }
    }
    None
}
