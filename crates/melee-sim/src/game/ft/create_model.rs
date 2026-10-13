//! The parts of a fighter's creation that build its model's extras and collision: its second
//! model, hurt capsules, dynamic bones, lights, and the fighters' shared models and light.

use melee_data::archive::DataRef;

use super::super::code::unported_call;
use super::super::handles::{CodeRef, DObjListId, FighterId};
use super::super::state::{OBJ_LIGHT, Obj, Running};
use super::bind::next_joint;
use super::dynamics::BoneSlot;
use super::parts::{MAX_SECOND_DOBJS, set_bone_bit, set_bone_xd};
use crate::fpu::{mem, reg};
use crate::hsd::dobj;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::hsd::lobj::{self, LObjId, StaticLightDesc};
use crate::hsd::mobj::MObjClass;
use crate::hsd::pobj::{self, PObjClass};
use crate::vec::Vec3;
use gekko_fp as fp;

/// The fighters' light (`ftCo_09F4.c`'s `node0`): infinite, white, at (0.57, 0.57, 0.57).
const FIGHTER_LIGHT: StaticLightDesc = StaticLightDesc {
    flags: 5,
    color: [0xFF; 4],
    position: Some(Vec3 {
        x: 0.57,
        y: 0.57,
        z: 0.57,
    }),
};

/// The stages' default lights (`Ground_803E06C8`), for a stage without its own
/// (`stage_info.map_plit`): an ambient light, then an infinite one.
const DEFAULT_STAGE_LIGHTS: [StaticLightDesc; 2] = [
    StaticLightDesc {
        flags: 1 << 2,
        color: [0xFF; 4],
        position: None,
    },
    StaticLightDesc {
        flags: (1 << 0) | (1 << 2) | (1 << 3),
        color: [0xFF; 4],
        position: Some(Vec3 {
            x: 0.57,
            y: 0.57,
            z: 0.57,
        }),
    },
];

/// The fighters' light GObj's render callback and process (`ftCo_8009F54C`,
/// `ftCo_8009F480`).
const LIGHT_RENDER: u32 = 0x8009_F54C;
const LIGHT_PROC: u32 = 0x8009_F480;

impl Running<'_> {
    /// Loads the fighter's second model (`ftData::x5C`, its metal or clear parts) onto its
    /// parts' joints, shapes of the fighters' class, and lists its DObjs, `ftCo_800C884C`
    /// (with `ft_800C85B8`).
    #[doc(alias = "ftCo_800C884C")]
    #[doc(alias = "ft_800C85B8")]
    pub fn load_second_model(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let list = DObjListId(
            self.game
                .fighter_dobj_lists
                .alloc(vec![None; MAX_SECOND_DOBJS]),
        );
        self.game.fighters[f.0].x203C.data = Some(list);
        pobj::set_default_class(&mut self.game.objs.pobjs, PObjClass::Fighter);
        let fp = &self.game.fighters[f.0];
        let kind = fp.kind;
        let parts = fp.parts.expect("a fighter without parts").0;
        let first = self
            .data
            .ptr(fp.ft_data.expect("a fighter without data").at(0x5C));
        // Each joint descriptor names its part's joint in the ID table.
        let mut joint = first;
        let mut stack = super::bind::JointStack::new();
        let mut idx = 0;
        while let Some(j) = joint {
            if self.part_mask(kind, idx) != 0 {
                idx += 1;
                continue;
            }
            let part = self.game.fighter_parts[parts][idx as usize].joint;
            if let Some(p) = part {
                self.game.objs.ids.insert(j, p);
            } else {
                panic!("HSD_IDInsertToTable with a part that has no joint");
            }
            idx += 1;
            joint = next_joint(self.data, j, &mut stack);
        }
        let mut joint = first;
        let mut stack = super::bind::JointStack::new();
        let (mut part, mut count) = (0i32, 0usize);
        while let Some(j) = joint {
            if self.part_mask(kind, part) != 0 {
                part += 1;
                continue;
            }
            let part_jobj = self.game.fighter_parts[parts][part as usize].joint;
            let desc = self.data.ptr(j.at(0x10));
            let d = dobj::load_desc(&mut self.game.objs, self.data, desc);
            if let Some(d) = d {
                let existing = jobj::get_dobj(&self.game.objs.jobjs, part_jobj);
                set_bone_bit(
                    &mut self.game.fighter_parts[parts][part as usize],
                    1,
                    5,
                    true,
                );
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
                while let Some(c) = cur {
                    assert!(
                        count < MAX_SECOND_DOBJS,
                        "the original's assertion failed: fighter dobj num over!"
                    );
                    self.game.fighter_dobj_lists[list.0][count] = Some(c);
                    if let Some(m) = self.game.objs.dobjs[c].mobj {
                        self.game.objs.mobjs[m].class = MObjClass::Fighter;
                    }
                    cur = self.game.objs.dobjs[c].next;
                    count += 1;
                }
                let bone = &mut self.game.fighter_parts[parts][part as usize];
                set_bone_xd(bone, (count as i32 - 1) as u8);
                set_bone_bit(bone, 1, 6, true);
            }
            part += 1;
            joint = next_joint(self.data, j, &mut stack);
        }
        self.game.fighters[f.0].x203C.count = count as u32;
        pobj::set_default_class(&mut self.game.objs.pobjs, PObjClass::Hsd);
    }

    /// The fighter's hurt capsules and its `x1670` points from its character's data,
    /// `ftColl_8007B320`.
    #[doc(alias = "ftColl_8007B320")]
    pub fn init_hurt_capsules(&mut self, f: FighterId) {
        let fp = &self.game.fighters[f.0];
        let data = fp.ft_data.expect("a fighter without data");
        let parts = fp.parts.expect("a fighter without parts").0;
        let x30 = self.data.ptr(data.at(0x30)).expect("ftData::x30");
        let dynamics = self.data.ptr(data.at(0x2C)).expect("ftData::x2C");
        let n = self.data.u32(x30) as i32;
        assert!(
            n <= 15,
            "the original's assertion failed: fighter hit num over!"
        );
        self.game.fighters[f.0].hurt_capsules_len = n as u8;
        let inits = self.data.ptr(x30.at(4));
        let raw = |at: DataRef| f32::from_bits(self.data.u32(at));
        let raw3 = |at: DataRef| Vec3::new(raw(at), raw(at.at(4)), raw(at.at(8)));
        for i in 0..n.max(0) as u32 {
            let init = inits.expect("ftData_x30::inits").at(0x28 * i);
            let bone_idx = self.data.u32(init) as i32;
            let joint = self.game.fighter_parts[parts][bone_idx as usize].joint;
            let h = &mut self.game.fighters[f.0].hurt_capsules[i as usize];
            h.capsule.bone_idx = bone_idx;
            h.height = self.data.u32(init.at(4)) as i32;
            h.is_grabbable = self.data.u32(init.at(8)) as i32;
            h.capsule.state = 0;
            h.capsule.bone = joint;
            h.capsule.a_offset = raw3(init.at(0xC));
            h.capsule.b_offset = raw3(init.at(0x18));
            h.capsule.scale = mem(reg(self.data.f32(init.at(0x24))));
        }
        let m = self.data.u32(dynamics.at(8)) as i32;
        assert!(
            m <= 11,
            "the original's assertion failed: fighter dynamics hit num over!"
        );
        self.game.fighters[f.0].x166C = m as u8;
        let points = self.data.ptr(dynamics.at(0xC));
        for i in 0..m.max(0) as u32 {
            let init = points.expect("ftDynamics::x8").at(0x14 * i);
            let bone = self.data.u32(init) as i32;
            let joint = self.game.fighter_parts[parts][bone as usize].joint;
            let p = &mut self.game.fighters[f.0].x1670[i as usize];
            p.x24 = bone;
            p.jobj = joint;
            p.v1 = raw3(init.at(4));
            p.v2 = mem(reg(self.data.f32(init.at(0x10))));
        }
    }

    /// Builds the fighter's dynamic bone chains from its character's data, physics on,
    /// `ftCo_8009CF84`.
    #[doc(alias = "ftCo_8009CF84")]
    pub fn init_bone_dynamics(&mut self, f: FighterId) {
        let fp = &self.game.fighters[f.0];
        let data = fp.ft_data.expect("a fighter without data");
        let x2c = self.data.ptr(data.at(0x2C)).expect("ftData::x2C");
        let n = self.data.u32(x2c) as i32;
        self.game.fighters[f.0].dynamics_num = n;
        assert!(
            n < 10,
            "the original's assertion failed: fighter dynamics num over!"
        );
        let parts = self.game.fighters[f.0]
            .parts
            .expect("a fighter without parts")
            .0;
        for i in 0..n.max(0) as usize {
            let bones = self.data.ptr(x2c.at(4)).expect("ftDynamics bones");
            let entry = bones.at(0x18 * i as u32);
            let bone_id = self.data.u32(entry) as usize;
            let count = self.data.u32(entry.at(8));
            let joint = self.game.fighter_parts[parts][bone_id].joint;
            let mut desc = self.game.fighters[f.0].dynamic_bone_sets[i]
                .dyn_desc
                .clone();
            self.game
                .load_dynamics_chain(self.data, joint, &mut desc, count);
            let set = &mut self.game.fighters[f.0].dynamic_bone_sets[i];
            set.dyn_desc = desc;
            set.bone_id = BoneSlot::Id(0);
            self.set_bone_dynamics(f, i, true, BoneSlot::Id(0));
            let bones = self.data.ptr(x2c.at(4)).expect("ftDynamics bones");
            let src = bones.at(0x18 * i as u32 + 4);
            let mut desc = self.game.fighters[f.0].dynamic_bone_sets[i]
                .dyn_desc
                .clone();
            self.game.set_dynamics_params(self.data, src, &mut desc);
            self.game.fighters[f.0].dynamic_bone_sets[i].dyn_desc = desc;
        }
    }

    /// The fighters' light: the stage's lights (or the defaults), positions in the stage's
    /// scale, on a GObj of their own, `ftCo_8009F4A4`.
    #[doc(alias = "ftCo_8009F4A4")]
    pub fn create_fighter_light(&mut self) {
        let gobj = self.game.world.create(12, 3, 0);
        let first = match super::super::gr::setup::load_fighter_lights(self.game, self.data) {
            Some(first) => first,
            None => {
                let mut prev: Option<LObjId> = None;
                let mut first = None;
                for d in &DEFAULT_STAGE_LIGHTS {
                    let l = lobj::load_static(&mut self.game.objs, d);
                    match prev {
                        Some(p) => self.game.objs.lobjs[p].next = Some(l),
                        None => first = Some(l),
                    }
                    prev = Some(l);
                }
                first
            }
        };
        let first = first.expect("a light list without lights");
        self.game.ft.lobj0 = Some(first);
        self.game.ft.lobj1 = self.first_ambient(first);
        // `Ground_801C2374`: positions and interests in the stage's scale.
        let s = reg(super::super::gr::ground::scale(self.game));
        let scaled = |v: Vec3| {
            Vec3::new(
                mem(fp::fmuls(reg(v.x), s)),
                mem(fp::fmuls(reg(v.y), s)),
                mem(fp::fmuls(reg(v.z), s)),
            )
        };
        let mut cur = Some(first);
        while let Some(l) = cur {
            if let Some(p) = lobj::position(&mut self.game.objs, self.data, Some(l)) {
                lobj::set_position(&mut self.game.objs, l, scaled(p));
            }
            if let Some(i) = lobj::interest(&mut self.game.objs, self.data, Some(l)) {
                lobj::set_interest(&mut self.game.objs, l, scaled(i));
            }
            cur = self.game.objs.lobjs[l].next;
        }
        self.game.world.set_obj(gobj, OBJ_LIGHT, Obj::LObj(first));
        self.game
            .world
            .setup_gx_link(gobj, CodeRef(LIGHT_RENDER), 4, 0);
        lobj::req_anim_all(&mut self.game.objs, Some(first), 0.0);
        self.game.world.setup_proc(gobj, CodeRef(LIGHT_PROC), 1);
    }

    /// The first light of a chain that is ambient and not off, `lb_8000CDC0`. With none the
    /// original returns what its last register held; no stage's lights lack one.
    #[doc(alias = "lb_8000CDC0")]
    fn first_ambient(&self, first: LObjId) -> Option<LObjId> {
        let mut cur = Some(first);
        while let Some(l) = cur {
            let flags = self.game.objs.lobjs[l].flags;
            if flags & 3 == 0 && flags & 0x20 == 0 {
                return Some(l);
            }
            cur = self.game.objs.lobjs[l].next;
        }
        panic!("lb_8000CDC0 found no ambient light and returns a stale register");
    }

    /// The fighters' metal and clear materials, from `PlCo.dat`'s model
    /// (`Fighter_804D6504`), `ftCo_800C8064` and `ftCo_800C8F6C`: each loads the model,
    /// which stays loaded, and keeps its first DObj's material; the clear one's diffuse color
    /// comes from the common data.
    #[doc(alias = "ftCo_800C8064")]
    #[doc(alias = "ftCo_800C8F6C")]
    pub fn load_metal_models(&mut self) {
        let joint = Some(self.load_common(0x50));
        let j = jobj::load_joint(&mut self.game.objs, self.data, joint);
        let d = jobj::get_dobj(&self.game.objs.jobjs, j);
        self.game.ft.metal_mobj = d.and_then(|d| self.game.objs.dobjs[d].mobj);
        let j = jobj::load_joint(&mut self.game.objs, self.data, joint);
        let d = jobj::get_dobj(&self.game.objs.jobjs, j);
        let m = d.and_then(|d| self.game.objs.dobjs[d].mobj);
        self.game.ft.clear_mobj = m;
        let m = m.expect("ftCo_800C8F6C reads a null material");
        let at = self.common().at(0x7D8);
        let c = crate::hsd::tobj::Color {
            r: self.data.u8(at),
            g: self.data.u8(at.at(1)),
            b: self.data.u8(at.at(2)),
            a: self.data.u8(at.at(3)),
        };
        if let Some(mat) = self.game.objs.mobjs[m].mat.as_mut() {
            mat.diffuse = c;
        }
    }

    /// The fighter's own light, `ftCo_8009F578` (`lb_80011AC4` on the fighters' light, which
    /// has no animation).
    #[doc(alias = "ftCo_8009F578")]
    pub fn create_own_light(&mut self, f: FighterId) {
        let l = lobj::load_static(&mut self.game.objs, &FIGHTER_LIGHT);
        self.game.fighters[f.0].x588 = Some(l);
    }

    /// The character's creation callback (`ftData_OnLoad`).
    pub fn on_load(&mut self, gobj: GObjId) {
        let kind = self.game.fighter(gobj).kind;
        match super::data::kind_tables(kind).on_load {
            0 => {}
            0x800E_57AC => self.fox_on_load(gobj),
            0x8013_64AC => self.mars_on_load(gobj),
            0x8011_00EC => self.seak_on_load(gobj),
            0x8013_9334 => self.zelda_on_load(gobj),
            0x8014_9CC4 => self.falco_on_load(gobj),
            0x800E_2AEC => self.ca_on_load(gobj),
            0x8014_EE1C => self.gn_on_load(gobj),
            0x8014_2324 => self.lg_on_load(gobj),
            0x800E_0960 => self.mr_on_load(gobj),
            0x8014_94E4 => self.dr_on_load(gobj),
            0x8012_43E4 => self.pk_on_load(gobj),
            0x8014_9E34 => self.pc_on_load(gobj),
            other => unported_call(&format!("fighter OnLoad {other:#010x}")),
        }
    }

    /// The character's spawn callback (`ftData_OnDeath`).
    pub fn on_death(&mut self, gobj: GObjId) {
        let kind = self.game.fighter(gobj).kind;
        match super::data::kind_tables(kind).on_death {
            0 => {}
            0x800E_5554 => self.fox_on_death(gobj),
            0x8013_6258 => self.mars_on_death(gobj),
            0x8011_0094 => self.seak_on_death(gobj),
            0x8013_92DC => self.zelda_on_death(gobj),
            0x8014_9ACC => self.falco_on_death(gobj),
            0x800E_2888 => self.ca_on_death(gobj),
            0x8014_EBFC => self.gn_on_death(gobj),
            0x8014_22E8 => self.lg_on_death(gobj),
            0x800E_08CC => self.mr_on_death(gobj),
            0x8014_949C => self.dr_on_death(gobj),
            0x8012_4474 => self.pk_on_death(gobj),
            0x8014_9EAC => self.pc_on_death(gobj),
            other => unported_call(&format!("fighter OnDeath {other:#010x}")),
        }
    }

    /// The character's attribute change callback (`ftKindCalcIndiviParamTable`); the
    /// original asserts there is one.
    pub fn load_special_attrs(&mut self, gobj: GObjId) {
        let kind = self.game.fighter(gobj).kind;
        match super::data::kind_tables(kind).load_special_attrs {
            0 => panic!("the original's assertion failed: ftKindCalcIndiviParamTable"),
            0x800E_5858 => self.fox_load_special_attrs(gobj),
            0x8013_64E8 => self.mars_load_special_attrs(gobj),
            0x8011_03C8 => self.seak_load_special_attrs(gobj),
            0x8013_9590 => self.zelda_load_special_attrs(gobj),
            0x8014_9D3C => self.falco_load_special_attrs(gobj),
            0x800E_2B40 | 0x8014_EE40 => self.ca_load_special_attrs(gobj),
            0x8014_254C => self.lg_load_special_attrs(gobj),
            0x800E_0BE4 => self.mr_load_special_attrs(gobj),
            0x8014_9724 => self.dr_load_special_attrs(gobj),
            0x8012_4704 | 0x8014_A1A8 => self.pk_load_special_attrs(gobj),
            other => unported_call(&format!("fighter LoadSpecialAttrs {other:#010x}")),
        }
    }
}

/// The fighters' light's process, `ftCo_8009F480`: animates its lights (`HSD_LObjAnimAll`).
#[doc(alias = "ftCo_8009F480")]
pub fn light_proc(r: &mut Running, gobj: GObjId) {
    let first = match r.game.world.gobjs[gobj.0].obj {
        Some(Obj::LObj(l)) => Some(l),
        None => None,
        other => panic!("the fighters' light GObj holds {other:?}"),
    };
    lobj::anim_all(&mut r.game.objs, r.data, first);
}
