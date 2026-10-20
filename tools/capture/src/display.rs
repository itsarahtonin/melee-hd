//! The display pass's statics in a captured state: the current camera, the current and
//! active lights with their GX slot masks, the current joint, display object and material,
//! and refraction's state, imported into melee-sim and compared back.

use melee_sim::hsd::list::{self, SListId};
use melee_sim::hsd::lobj::{LObjId, MAX_GXLIGHT};

use melee_sim::game::mirror::{Cx, Mem, Mirror};
use melee_sim::game::structs::BgFlashData;

use crate::MEM1;
use crate::compare::Comparer;
use crate::import::{Importer, sym};

/// The light statics' symbols and what each holds.
const MASKS: [&str; 4] = [
    "lightmask_diffuse",
    "lightmask_specular",
    "lightmask_attnfunc",
    "lightmask_alpha",
];

impl Importer<'_> {
    /// The display statics: the current camera (`HSD_CObjGetCurrent`), the current lights
    /// (`current_lights`) and the active ones by GX slot, and the current joint, display object
    /// and material, which are null between draws.
    pub fn display(&mut self) {
        let c = self.cap.u32(sym("current"));
        self.game.objs.cobjs.current = (c != 0).then(|| self.cobj(c));
        let mut at = self.cap.u32(sym("current_lights"));
        let mut prev: Option<SListId> = None;
        while at != 0 {
            let l = self.cap.u32(at + 4);
            let l = self.lobj(l);
            let n = list::alloc(&mut self.game.objs.lobjs.nodes, l);
            match prev {
                Some(p) => self.game.objs.lobjs.nodes[p.0].next = Some(n),
                None => self.game.objs.lobjs.current = Some(n),
            }
            prev = Some(n);
            at = self.cap.u32(at);
        }
        let active = sym("active_lights");
        for i in 0..MAX_GXLIGHT {
            let l = self.cap.u32(active + 4 * i as u32);
            self.game.objs.lobjs.active[i] = match l {
                0 => None,
                // A light freed while it was active stays in the table until the next clear
                // (a light GObj's release, `HSD_LObjRemoveAll`, drops only current lights'
                // entries); its block may hold another object by now. It gets a slot of its
                // own, which compares by address.
                l if !self.maps.lobjs.contains_key(&l) && self.cap.u32(l) != sym("hsdLObj") => {
                    let id = match self.maps.dangling_lobjs.iter().find(|e| *e.1 == l) {
                        Some((&id, _)) => LObjId(id),
                        None => {
                            let id = LObjId(self.game.objs.lobjs.pool.alloc(Default::default()));
                            self.maps.dangling_lobjs.insert(id.0, l);
                            id
                        }
                    };
                    Some(id)
                }
                l => Some(self.lobj(l)),
            };
        }
        self.game.objs.lobjs.nb_active = self.cap.u32(sym("nb_active_lights")) as i32;
        let [d, s, a, al] = MASKS.map(|m| self.cap.u32(sym(m)));
        let l = &mut self.game.objs.lobjs;
        (
            l.lightmask_diffuse,
            l.lightmask_specular,
            l.lightmask_attnfunc,
            l.lightmask_alpha,
        ) = (d, s, a, al);
        // The textures materials take shadows and toon shading from (`tobj_shadows`, chained
        // through their own `next`, and `tobj_toon`).
        let t = self.cap.u32(sym("tobj_shadows"));
        self.game.objs.mobjs.shadows = (t != 0).then(|| self.tobj(t));
        let t = self.cap.u32(sym("tobj_toon"));
        self.game.objs.mobjs.toon = (t != 0).then(|| self.tobj(t));
        // The GObjs being drawn (`HSD_GObj_804D7814`, `HSD_GObj_804D7818`).
        let r = self.cap.u32(sym("HSD_GObj_804D7814"));
        self.game.world.rendering = self.gobj_handle(r);
        let r = self.cap.u32(sym("HSD_GObj_804D7818"));
        self.game.world.rendering_camera = self.gobj_handle(r);
        self.game.objs.display.offscreen = self.cap.u32(sym("current_render_pass")) == 3;
        let j = self.cap.u32(sym("current_jobj"));
        self.game.objs.jobj.current = (j != 0).then(|| self.jobj(j));
        for name in ["current_dobj", "current_mobj"] {
            if self.cap.u32(sym(name)) != 0 {
                self.unhandled.push(format!("{name} is set"));
            }
        }
        self.refract();
        self.bg_flash();
    }

    /// The background flash's statics (`lbl_80433658`, `flash_gobj`, `lbl_804D63DC`,
    /// `flash_scale`); its user data comes with its GObj.
    pub fn bg_flash(&mut self) {
        let mem = Mem {
            bytes: &self.cap.call.mem1,
            base: MEM1,
        };
        self.game.bgflash.data = BgFlashData::import(&mem, sym("lbl_80433658"), self);
        let g = self.cap.u32(sym("flash_gobj"));
        self.game.bgflash.gobj = self.gobj_handle(g);
        let t = self.cap.u32(sym("lbl_804D63DC"));
        self.game.bgflash.table = self.import_data(t);
        self.game.bgflash.scale = f32::from_bits(self.cap.u32(sym("flash_scale")));
    }
}

impl Comparer<'_> {
    /// [`Importer::display`]'s statics.
    pub fn display(&mut self) {
        let objs = &self.game.objs;
        let t = self.u32(sym("current"));
        match objs.cobjs.current {
            Some(c) if t != 0 => self.cobj("display current camera", c, t),
            ours => self.check("display", "current camera", ours.is_some(), t != 0),
        }
        let mut node = objs.lobjs.current;
        let mut at = self.u32(sym("current_lights"));
        let mut i = 0;
        while node.is_some() || at != 0 {
            let (Some(n), true) = (node, at != 0) else {
                self.check(
                    "display",
                    "current lights",
                    format!("{i} and more: {}", node.is_some()),
                    format!("{i} and more: {}", at != 0),
                );
                break;
            };
            let l = self.game.objs.lobjs.nodes[n.0];
            let t = self.u32(at + 4);
            self.lobjs(&format!("display current light {i}"), Some(l.data), t);
            node = l.next;
            at = self.u32(at);
            i += 1;
        }
        let active = sym("active_lights");
        for k in 0..MAX_GXLIGHT {
            let t = self.u32(active + 4 * k as u32);
            let o = self.game.objs.lobjs.active[k];
            let path = format!("display active light {k}");
            match o {
                Some(l) if self.maps.dangling_lobjs.contains_key(&l.0) => {
                    self.check(&path, "freed light", self.maps.dangling_lobjs[&l.0], t);
                }
                Some(l) if t != 0 => self.lobjs(&path, Some(l), t),
                _ => self.check(&path, "set", o.is_some(), t != 0),
            }
        }
        let l = &self.game.objs.lobjs;
        let ours = (
            l.nb_active as u32,
            l.lightmask_diffuse,
            l.lightmask_specular,
            l.lightmask_attnfunc,
            l.lightmask_alpha,
        );
        let [d, s, a, al] = MASKS.map(|m| self.u32(sym(m)));
        let theirs = (self.u32(sym("nb_active_lights")), d, s, a, al);
        self.check("display", "active lights and masks", ours, theirs);
        let current = (
            self.game.objs.jobj.current.is_some(),
            self.game.objs.dobjs.current.is_some(),
            self.game.objs.mobjs.current.is_some(),
        );
        let theirs = (
            self.u32(sym("current_jobj")) != 0,
            self.u32(sym("current_dobj")) != 0,
            self.u32(sym("current_mobj")) != 0,
        );
        self.check("display", "current joint, dobj, mobj", current, theirs);
        use melee_sim::game::mirror::Cx;
        let shadows = self.game.objs.mobjs.shadows;
        let t = self.u32(sym("tobj_shadows"));
        self.tobj_chain("display shadow textures", shadows, t);
        let toon = self.game.objs.mobjs.toon;
        let t = self.u32(sym("tobj_toon"));
        self.check("display", "toon texture", toon.is_some(), t != 0);
        self.tobj_chain("display toon texture", toon, t);
        let w = &self.game.world;
        let (r, rc) = (w.rendering.map(|g| g.0), w.rendering_camera.map(|g| g.0));
        let t = self.u32(sym("HSD_GObj_804D7814"));
        self.compare_handle("display rendering GObj", "GObjId", r, t);
        let t = self.u32(sym("HSD_GObj_804D7818"));
        self.compare_handle("display rendering camera GObj", "GObjId", rc, t);
        let pass = self.u32(sym("current_render_pass"));
        self.check(
            "display",
            "render pass offscreen",
            self.game.objs.display.offscreen,
            pass == 3,
        );
        self.refract();
        self.bg_flash();
    }

    /// [`Importer::bg_flash`]'s statics.
    pub fn bg_flash(&mut self) {
        let mem = Mem {
            bytes: self.mem,
            base: MEM1,
        };
        let f = self.game.bgflash.clone();
        f.data.compare(&mem, sym("lbl_80433658"), "bgflash", self);
        let t = self.u32(sym("flash_gobj"));
        self.compare_handle("bgflash gobj", "GObjId", f.gobj.map(|g| g.0), t);
        let t = self.u32(sym("lbl_804D63DC"));
        self.compare_data("bgflash table", f.table, t);
        let t = self.u32(sym("flash_scale"));
        self.check("bgflash", "scale", f.scale.to_bits(), t);
    }
}

/// lbRefract's state (`lbl_804336D0`): users, image, image descriptors, texture chains,
/// projection.
pub(crate) const REFRACT: &str = "lbl_804336D0";
/// An `HSD_ImageDesc`'s size.
const IMAGE_DESC: u32 = 0x18;

impl Importer<'_> {
    /// Which of lbRefract's indirect maps' descriptors `p` is, if any.
    pub(crate) fn refract_map(&self, p: u32) -> Option<u8> {
        let base = self.cap.u32(sym(REFRACT) + 8);
        let data = self.cap.u32(sym("refract_data"));
        if base == 0 || data == 0 {
            return None;
        }
        let n = u32::from(self.cap.call.mem1[(data - crate::MEM1) as usize]);
        let off = p.wrapping_sub(base);
        (off.is_multiple_of(IMAGE_DESC) && off / IMAGE_DESC < n).then_some((off / IMAGE_DESC) as u8)
    }

    /// Refraction's users, table, texture chains and projection.
    pub fn refract(&mut self) {
        let at = sym(REFRACT);
        let r = &mut self.game.refract;
        r.users = self.cap.u32(at) as i32;
        let data = self.cap.u32(sym("refract_data"));
        self.game.refract.data = self.data.get(data);
        if data != 0 && self.game.refract.data.is_none() {
            self.unhandled
                .push(format!("refract_data {data:#x} outside LbRf.dat"));
        }
        let n = if data == 0 {
            0
        } else {
            u32::from(self.cap.call.mem1[(data - crate::MEM1) as usize])
        };
        let list = self.cap.u32(at + 0xC);
        let tobjs = (0..n)
            .filter(|_| list != 0)
            .map(|i| self.tobj(self.cap.u32(list + 4 * i)))
            .collect();
        self.game.refract.tobjs = tobjs;
        let mut m = melee_sim::mtx::Mtx34::default();
        for (i, v) in m.m.iter_mut().flatten().enumerate() {
            *v = f32::from_bits(self.cap.u32(at + 0x10 + 4 * i as u32));
        }
        self.game.refract.texture_mtx = m;
    }
}

impl Comparer<'_> {
    /// [`Importer::refract`]'s state; the texture chains' TObjs compare as materials' do.
    pub fn refract(&mut self) {
        let at = sym(REFRACT);
        let r = self.game.refract.clone();
        self.check("refract", "users", r.users, self.u32(at) as i32);
        let data = self.u32(sym("refract_data"));
        let theirs = self.data_ref(data);
        self.check("refract", "table", r.data, theirs);
        let list = self.u32(at + 0xC);
        let n = r.tobjs.len() as u32;
        let theirs_n = if data == 0 {
            0
        } else {
            u32::from(self.u8(data))
        };
        self.check("refract", "chains", n, theirs_n);
        for (i, t) in r.tobjs.iter().enumerate() {
            if list == 0 {
                break;
            }
            let tt = self.u32(list + 4 * i as u32);
            self.tobj_chain(&format!("refract chain {i}"), Some(*t), tt);
        }
        let ours: Vec<u32> = r
            .texture_mtx
            .m
            .iter()
            .flatten()
            .map(|v| v.to_bits())
            .collect();
        let theirs: Vec<u32> = (0..12).map(|k| self.u32(at + 0x10 + 4 * k)).collect();
        self.check("refract", "projection", ours, theirs);
    }
}

impl Importer<'_> {
    /// A GObj pointer as our handle (none for null).
    fn gobj_handle(&mut self, a: u32) -> Option<melee_sim::hsd::gobj::GObjId> {
        use melee_sim::game::mirror::Cx;
        self.import_handle("GObjId", a)
            .map(melee_sim::hsd::gobj::GObjId)
    }
}
