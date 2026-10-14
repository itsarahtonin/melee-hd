//! Jigglypuff's hats (`ftpurin.c`): the costumes but the first wear a hat, a model of its own
//! loaded from the costume's file and drawn on her left leg's matrix, with its own DObj list
//! and model variants (`u.x2240`, `u.x2248`), and for two costumes a pair of dynamic bone chains.

use melee_data::archive::DataRef;

use super::super::super::handles::{DObjListId, FighterId};
use super::super::super::state::Running;
use super::super::dynamics::BoneSlot;
use super::super::parts::MAX_SECOND_DOBJS;
use super::u;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::pobj::{self, PObjClass};
use crate::mtx::Mtx34;

/// `ftPr_Init_803D05B4`: each costume's hat joint in its file (none for the first).
const HAT_JOINTS: [Option<&str>; 5] = [
    None,
    Some("PlyPurinReHat_TopN_joint"),
    Some("PlyPurinBuHat_TopN_joint"),
    Some("PlyPurinGrHat_TopN_joint"),
    Some("PlyPurinYeHat_TopN_joint"),
];

/// `FtPart_LLegJA`.
const PART_L_LEG_JA: i32 = 6;

/// `u` offsets: the hat's DObj list's pointer (`x2240.data`), and its model variants
/// (`x2248`: the variants' count, which lookups are shown, the lookups).
const LIST: usize = 0x18;
const VIS_MODEL_NUM: usize = 0x1C;
const VIS_CLEARED: usize = 0x20;
const VIS_LOOKUPS: usize = 0x28;

impl Running<'_> {
    /// The costume's hat, `ftPr_Init_8013C360`: its model loaded with the fighters' shapes, its
    /// DObjs listed, its variants hidden, and its chains built; none for the first costume.
    #[doc(alias = "ftPr_Init_8013C360")]
    pub fn purin_load_hat(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let costume = usize::from(fp.costume_id);
        let Some(name) = HAT_JOINTS[costume] else {
            self.game.fighters[f.0].u.set_jobj_at(u::HAT, None);
            return;
        };
        let data = fp.ft_data.expect("a fighter without data");
        let items = self.data.ptr(data.at(0x48)).expect("ftData::x48_items");
        let shifted = self.data.ptr(items.at(4)).expect("ftData::x48_items[1]");
        // `HSD_ArchiveGetPublicAddress` in the costume's file, kept by costume
        // (`ft_8045A1E0`) once found.
        let joint = match self.game.ft.purin_hats[costume] {
            Some(j) => Some(j),
            None => {
                let file = super::super::data::kind_tables(fp.kind).costumes[costume]
                    .file
                    .expect("a costume without a file");
                let archive = self.game.ft.data.files.archive(file);
                let j = self
                    .data
                    .get(archive)
                    .public(name)
                    .map(|offset| DataRef { archive, offset });
                self.game.ft.purin_hats[costume] = j;
                j
            }
        };
        let list = DObjListId(
            self.game
                .fighter_dobj_lists
                .alloc(vec![None; MAX_SECOND_DOBJS]),
        );
        self.game.fighters[f.0].u.set_dobj_list_at(LIST, Some(list));
        pobj::set_default_class(&mut self.game.objs.pobjs, PObjClass::Fighter);
        let hat = jobj::load_joint(&mut self.game.objs, self.data, joint);
        let fp = &mut self.game.fighters[f.0];
        fp.u.set_jobj_at(u::HAT, hat);
        fp.x2225_b2 = true;
        pobj::set_default_class(&mut self.game.objs.pobjs, PObjClass::Hsd);
        self.purin_list_hat_dobjs(hat, list);
        self.purin_hat_vis_init(f, shifted.at(4));
        self.purin_hat_chains(f);
    }

    /// The hat's DObjs into its list, the materials of the fighters' class,
    /// `ftParts_80075650`: depth first through the tree (not into instances).
    #[doc(alias = "ftParts_80075650")]
    fn purin_list_hat_dobjs(&mut self, hat: Option<JObjId>, list: DObjListId) {
        let mut cur = hat;
        let mut n = 0usize;
        while let Some(j) = cur {
            let mut d = jobj::get_dobj(&self.game.objs.jobjs, Some(j));
            while let Some(dd) = d {
                assert!(
                    n < MAX_SECOND_DOBJS,
                    "the original's assertion failed: fighter parts model dobj num over!"
                );
                self.game.fighter_dobj_lists[list.0][n] = Some(dd);
                if let Some(m) = self.game.objs.dobjs[dd].mobj {
                    self.game.objs.mobjs[m].class = crate::hsd::mobj::MObjClass::Fighter;
                }
                d = self.game.objs.dobjs[dd].next;
                n += 1;
            }
            let o = &self.game.objs.jobjs[j];
            cur = if o.flags & jobj::INSTANCE == 0 && o.child.is_some() {
                o.child
            } else if o.next.is_some() {
                o.next
            } else {
                let mut up = j;
                loop {
                    let Some(p) = self.game.objs.jobjs[up].parent else {
                        break None;
                    };
                    if let Some(next) = self.game.objs.jobjs[p].next {
                        break Some(next);
                    }
                    up = p;
                }
            };
        }
    }

    /// The hat's model variants for her costume, all hidden (`ftParts_8007487C` with the hat's
    /// visibility and list): `desc` is the variants' description in her data
    /// (`ftData::x48_items[1] + 4`).
    fn purin_hat_vis_init(&mut self, f: FighterId, desc: DataRef) {
        let costume = u32::from(self.game.fighters[f.0].costume_id);
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
        for idx in 0..5 {
            self.purin_hat_lookup(f, idx, false);
        }
    }

    /// Shows (`ftParts_80074CA0`) or hides (`ftParts_80074D7C`) every DObj one of the hat's
    /// visibility lookups names, unless it already is.
    #[doc(alias = "ftParts_80074CA0")]
    fn purin_hat_lookup(&mut self, f: FighterId, idx: usize, show: bool) {
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
            fp.u.dobj_list_at(LIST)
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

    /// A model group shown or hidden on her hat too, `ftPr_Init_UnkIntBoolFunc0` (a model
    /// event of `ftParts_800750C8`): nothing without a hat.
    #[doc(alias = "ftPr_Init_UnkIntBoolFunc0")]
    pub fn purin_hat_vis(&mut self, f: FighterId, idx: usize, show: bool) {
        if self.game.fighters[f.0].u.jobj_at(u::HAT).is_some() {
            self.purin_hat_lookup(f, idx, show);
        }
    }

    /// The hat's two dynamic bone chains for the costumes that have them (the third and
    /// fourth), `ftCo_8009DC54`: the fighter's chains 1 and 2, down the hat's tree to the
    /// joints her data names, their bone slot `FtPart_TopN`.
    #[doc(alias = "ftCo_8009DC54")]
    fn purin_hat_chains(&mut self, f: FighterId) {
        let fp = &self.game.fighters[f.0];
        let set = match fp.costume_id {
            2 => 0,
            3 => 1,
            _ => return,
        };
        let data = fp.ft_data.expect("a fighter without data");
        self.game.fighters[f.0].dynamics_num = 3;
        let x2c = self.data.ptr(data.at(0x2C)).expect("ftData::x2C");
        for i in 0..2u32 {
            let bone = set * 2 + 1 + i;
            let bones = self.data.ptr(x2c.at(4)).expect("ftDynamics bones");
            let entry = bones.at(0x18 * bone);
            let steps = self.data.u32(entry) as i32;
            let mut cur = self.game.fighters[f.0]
                .u
                .jobj_at(u::HAT)
                .expect("a hat's chains without the hat");
            for _ in 0..steps {
                let o = &self.game.objs.jobjs[cur];
                if let Some(c) = o.child {
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
            let n = self.data.u32(entry.at(8));
            let k = i as usize + 1;
            let mut desc = self.game.fighters[f.0].dynamic_bone_sets[k]
                .dyn_desc
                .clone();
            self.game
                .load_dynamics_chain(self.data, Some(cur), &mut desc, n);
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

    /// Her hat drawn on her left leg's matrix, `ftPr_Init_UnkMtxFunc0` (her per-kind drawing):
    /// nothing without a hat or with it hidden (`x2225_b2`).
    #[doc(alias = "ftPr_Init_UnkMtxFunc0")]
    pub fn purin_draw_hat(&mut self, gobj: GObjId, vmtx: Option<Mtx34>, pass: i32) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let Some(hat) = fp.u.jobj_at(u::HAT) else {
            return;
        };
        if !fp.x2225_b2 {
            return;
        }
        // `HSD_JObjGetMtxPtr`, then `HSD_JObjCopyMtx`.
        let leg = self.bone_joint(gobj, PART_L_LEG_JA);
        jobj::setup_matrix(&mut self.game.objs, self.data, leg);
        let leg = leg.expect("a fighter without LLegJA");
        let m = self.game.objs.jobjs[leg].mtx;
        let jobjs = &mut self.game.objs.jobjs;
        jobjs[hat].mtx = m;
        jobjs[hat].flags |= jobj::USER_DEF_MTX | jobj::MTX_INDEP_PARENT | jobj::MTX_INDEP_SRT;
        jobj::set_mtx_dirty(jobjs, Some(hat));
        self.draw_fighter_tree(f, Some(hat), vmtx, pass);
    }
}
