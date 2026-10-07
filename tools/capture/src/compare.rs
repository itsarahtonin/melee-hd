//! Compares melee-sim's state with ssbm-rs's memory: the scheduler's lists, every joint tree,
//! its animations, references, display lists and shapes, and the fighters. Objects pair by
//! where they are reached from (list position, tree position), starting from the importer's
//! maps, so objects either side made since pair too.

use std::collections::{HashMap, HashSet};

use melee_data::archive::DataRef;
use melee_sim::game::mirror::{Cx, Mem, Mirror};
use melee_sim::game::state::{Game, Obj, UserData};
use melee_sim::hsd::aobj::AObjId;
use melee_sim::hsd::gobj::{GObjId, P_LINKS};
use melee_sim::hsd::jobj::{Contents, JObjId};
use melee_sim::hsd::pobj::Binding;
use melee_sim::hsd::robj::{self, RObjId, Ref};

use crate::import::{Maps, box_index, sym};
use crate::{DataMap, MEM1};

pub struct Comparer<'a> {
    pub mem: &'a [u8],
    pub data: &'a DataMap,
    pub game: &'a Game,
    /// Pairs as the importer made them; new pairs are added as they're met.
    pub maps: Maps,
    /// Pairs by kind and ssbm-rs address for the mirror's handles.
    handles: HashMap<(&'static str, u32), u32>,
    /// What this comparison has compared already, by kind and ssbm-rs address.
    pub(crate) seen: HashSet<(&'static str, u32)>,
    pub diffs: Vec<String>,
}

impl<'a> Comparer<'a> {
    pub fn new(mem: &'a [u8], data: &'a DataMap, game: &'a Game, maps: Maps) -> Self {
        Comparer {
            mem,
            data,
            game,
            maps,
            handles: HashMap::new(),
            seen: HashSet::new(),
            diffs: Vec::new(),
        }
    }

    pub(crate) fn u8(&self, a: u32) -> u8 {
        self.mem[(a - MEM1) as usize]
    }

    pub(crate) fn u16(&self, a: u32) -> u16 {
        let i = (a - MEM1) as usize;
        u16::from_be_bytes(self.mem[i..i + 2].try_into().unwrap())
    }

    pub(crate) fn u32(&self, a: u32) -> u32 {
        let i = (a - MEM1) as usize;
        u32::from_be_bytes(self.mem[i..i + 4].try_into().unwrap())
    }

    pub(crate) fn diff(
        &mut self,
        path: &str,
        what: &str,
        ours: impl std::fmt::Debug,
        theirs: impl std::fmt::Debug,
    ) {
        self.diffs
            .push(format!("{path}: {what}: ours {ours:?}, theirs {theirs:?}"));
    }

    pub(crate) fn check<T: PartialEq + std::fmt::Debug>(
        &mut self,
        path: &str,
        what: &str,
        ours: T,
        theirs: T,
    ) {
        if ours != theirs {
            self.diff(path, what, ours, theirs);
        }
    }

    fn data_ref(&self, a: u32) -> Option<DataRef> {
        (a != 0).then(|| self.data.get(a)).flatten()
    }

    /// Pairs `ours` with ssbm-rs's object at `theirs`, or reports they don't pair; whether to
    /// compare them (the first time this comparison meets them).
    pub(crate) fn pair<T: Copy + PartialEq + std::fmt::Debug>(
        map: &mut HashMap<u32, T>,
        seen: &mut HashSet<(&'static str, u32)>,
        diffs: &mut Vec<String>,
        kind: &'static str,
        path: &str,
        ours: Option<T>,
        theirs: u32,
    ) -> bool {
        match (ours, theirs) {
            (None, 0) => false,
            (Some(o), t) if t != 0 => {
                let known = *map.entry(t).or_insert(o);
                if known != o {
                    diffs.push(format!(
                        "{path}: ours {o:?}, theirs {t:#x} (paired with {known:?})"
                    ));
                    return false;
                }
                seen.insert((kind, t))
            }
            (o, t) => {
                diffs.push(format!("{path}: ours {o:?}, theirs {t:#x}"));
                false
            }
        }
    }

    /// The scheduler: each process link's GObjs in order, and what they hold.
    pub fn world(&mut self) {
        let heads = self.u32(sym("HSD_GObjPLinkHead"));
        let w = &self.game.world;
        for link in 0..P_LINKS {
            let mut t = self.u32(heads + 4 * link as u32);
            let mut o = w.plink_head[link];
            let mut i = 0;
            loop {
                let path = format!("p_link {link} #{i}");
                match (o, t) {
                    (None, 0) => break,
                    (Some(og), tg) if tg != 0 => {
                        self.maps.gobjs.entry(tg).or_insert(og);
                        self.gobj(&path, og, tg);
                        o = w.gobjs[og.0].next;
                        t = self.u32(tg + 8);
                    }
                    (o, t) => {
                        self.diff(&path, "GObj", o, format!("{t:#x}"));
                        break;
                    }
                }
                i += 1;
            }
        }
    }

    /// The camera, its boxes, its second CObj, its default field of view and the debug
    /// cameras' state. [`Comparer::stage`] compares them with the stage.
    pub fn camera(&mut self) {
        let mem = Mem {
            bytes: self.mem,
            base: MEM1,
        };
        let game = self.game;
        let c = &game.camera;
        c.cam.compare(&mem, sym("game_camera"), "camera", self);
        let base = self.u32(sym("cm_804D645C"));
        for (i, b) in c.boxes.boxes.iter().enumerate() {
            let at = base + i as u32 * melee_sim::game::structs::CmSubject::SIZE;
            b.compare(&mem, at, &format!("camera box {i}"), self);
        }
        for (name, ours) in [
            ("cm_804D6458", c.boxes.free),
            ("cm_804D6460", c.boxes.first),
            ("cm_804D6468", c.boxes.last),
        ] {
            let theirs = self.u32(sym(name));
            self.compare_handle(name, "CameraBoxId", ours.map(|b| b.0), theirs);
        }
        let copy = self.u32(sym("cm_804D6464"));
        match c.cobj_copy {
            Some(o) => self.cobj("camera cobj copy", o, copy),
            None => self.check("camera cobj copy", "CObj", 0, copy),
        }
        let fov = self.u32(sym("cm_803BCCA0") + 0x40);
        self.check("camera", "default fov", c.fov.to_bits(), fov);
        c.debug
            .compare(&mem, sym("cm_80453004"), "camera debug", self);
    }

    fn gobj(&mut self, path: &str, o: GObjId, t: u32) {
        let g = &self.game.world.gobjs[o.0];
        self.check(path, "classifier", g.classifier, self.u16(t));
        self.check(
            path,
            "links and priorities",
            (g.p_link, g.gx_link, g.p_priority, g.render_priority),
            (
                self.u8(t + 2),
                self.u8(t + 3),
                self.u8(t + 4),
                self.u8(t + 5),
            ),
        );
        self.check(
            path,
            "kinds",
            (g.obj_kind, g.user_data_kind),
            (self.u8(t + 6), self.u8(t + 7)),
        );
        self.check(
            path,
            "render",
            g.render.map_or(0, |c| c.0),
            self.u32(t + 0x1C),
        );
        self.check(
            path,
            "gxlink_prios",
            g.gxlink_prios,
            u64::from(self.u32(t + 0x20)) << 32 | u64::from(self.u32(t + 0x24)),
        );
        let mut p = g.proc;
        let mut tp = self.u32(t + 0x18);
        let mut i = 0;
        while p.is_some() || tp != 0 {
            let pp = format!("{path} proc {i}");
            let (Some(op), true) = (p, tp != 0) else {
                self.diff(&pp, "process", p, format!("{tp:#x}"));
                break;
            };
            self.maps.procs.entry(tp).or_insert(op);
            let pr = &self.game.world.procs[op.0];
            let flags = self.u8(tp + 0xD);
            self.check(
                &pp,
                "s_link and function",
                (pr.s_link, pr.on_invoke.0),
                (self.u8(tp + 0xC), self.u32(tp + 0x14)),
            );
            self.check(
                &pp,
                "flags",
                (pr.flags_1, pr.flags_2, pr.flags_3),
                (flags & 0x80 != 0, flags & 0x40 != 0, flags >> 4 & 3),
            );
            p = pr.child;
            tp = self.u32(tp);
            i += 1;
        }
        let obj = self.u32(t + 0x28);
        match g.obj {
            Some(Obj::JObj(j)) => self.jobj_tree(&format!("{path} jobj"), Some(j), obj),
            Some(Obj::CObj(c)) => self.cobj(&format!("{path} cobj"), c, obj),
            Some(Obj::LObj(l)) => self.lobjs(&format!("{path} lobj"), Some(l), obj),
            Some(Obj::Fog(f)) => self.fog(&format!("{path} fog"), f, obj),
            None => {}
        }
        match g.user_data {
            Some(UserData::Fighter(f)) => {
                let ud = self.u32(t + 0x2C);
                if Self::pair(
                    &mut self.maps.fighters,
                    &mut self.seen,
                    &mut self.diffs,
                    "fighter",
                    path,
                    Some(f),
                    ud,
                ) {
                    let mem = Mem {
                        bytes: self.mem,
                        base: MEM1,
                    };
                    let fighter = &self.game.fighters[f.0];
                    fighter.compare(&mem, ud, &format!("{path} fighter"), self);
                }
            }
            Some(UserData::Ground(g)) => self.ground(path, g, self.u32(t + 0x2C)),
            Some(UserData::Item(_)) | None => {}
        }
    }

    /// A joint, its later siblings and its descendants.
    pub fn jobj_tree(&mut self, path: &str, o: Option<JObjId>, t: u32) {
        let mut stack = vec![(o, t, path.to_string())];
        while let Some((o, t, path)) = stack.pop() {
            if !Self::pair(
                &mut self.maps.jobjs,
                &mut self.seen,
                &mut self.diffs,
                "jobj",
                &path,
                o,
                t,
            ) {
                continue;
            }
            let o = o.unwrap();
            let j = &self.game.objs.jobjs[o];
            let floats =
                |s: &Self, a: u32, n: u32| (0..n).map(|i| s.u32(a + 4 * i)).collect::<Vec<_>>();
            self.check(
                &path,
                "ref counts and flags",
                (j.ref_count, j.ref_count_individual, j.flags),
                (self.u16(t + 4), self.u16(t + 6), self.u32(t + 0x14)),
            );
            let q = &j.rotate;
            let srt: Vec<u32> = [
                q.x,
                q.y,
                q.z,
                q.w,
                j.scale.x,
                j.scale.y,
                j.scale.z,
                j.translate.x,
                j.translate.y,
                j.translate.z,
            ]
            .iter()
            .map(|v| v.to_bits())
            .collect();
            self.check(&path, "SRT", srt, floats(self, t + 0x1C, 10));
            let m: Vec<u32> = j.mtx.m.iter().flatten().map(|v| v.to_bits()).collect();
            self.check(&path, "matrix", m, floats(self, t + 0x44, 12));
            let scl = self.u32(t + 0x74);
            self.check(
                &path,
                "scl",
                j.scl
                    .map(|v| vec![v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]),
                (scl != 0).then(|| floats(self, scl, 3)),
            );
            self.check(&path, "id", j.id, self.data_ref(self.u32(t + 0x84)));
            let u = self.u32(t + 0x18);
            match j.u {
                Contents::Spline(s) | Contents::Particles(s) => {
                    self.check(&path, "contents", s, self.data_ref(u))
                }
                Contents::DObjs(d) => self.dobjs(&path, d, u),
            }
            self.aobj(&format!("{path} aobj"), j.aobj, self.u32(t + 0x7C));
            self.robjs(&path, j.robj, self.u32(t + 0x80));
            stack.push((j.next, self.u32(t + 8), format!("{path}>")));
            if j.flags & melee_sim::hsd::jobj::INSTANCE == 0 {
                stack.push((j.child, self.u32(t + 0x10), format!("{path}/")));
            }
        }
    }

    pub(crate) fn aobj(&mut self, path: &str, o: Option<AObjId>, t: u32) {
        if !Self::pair(
            &mut self.maps.aobjs,
            &mut self.seen,
            &mut self.diffs,
            "aobj",
            path,
            o,
            t,
        ) {
            return;
        }
        let a = &self.game.objs.aobjs[o.unwrap()];
        self.check(
            path,
            "state",
            (
                a.flags,
                a.curr_frame.to_bits(),
                a.rewind_frame.to_bits(),
                a.end_frame.to_bits(),
                a.framerate.to_bits(),
            ),
            (
                self.u32(t),
                self.u32(t + 4),
                self.u32(t + 8),
                self.u32(t + 0xC),
                self.u32(t + 0x10),
            ),
        );
        let mut f = a.fobj;
        let mut tf = self.u32(t + 0x14);
        let mut i = 0;
        while f.is_some() || tf != 0 {
            let fp = format!("{path} fobj {i}");
            let (Some(of), true) = (f, tf != 0) else {
                self.diff(&fp, "FObj", f, format!("{tf:#x}"));
                break;
            };
            self.maps.fobjs.entry(tf).or_insert(of);
            let x = &self.game.objs.fobjs[of.0];
            let ad = match self.u32(tf + 4) {
                0 => 0,
                p => self.data_ref(p).map_or(u32::MAX, |d| d.offset),
            };
            self.check(
                &fp,
                "cursor",
                (x.ad, x.ad_head, x.length),
                (
                    ad,
                    self.data_ref(self.u32(tf + 8))
                        .map_or(u32::MAX, |d| d.offset),
                    self.u32(tf + 0xC),
                ),
            );
            self.check(
                &fp,
                "codes",
                (
                    x.flags,
                    x.op,
                    x.op_intrp,
                    x.obj_type,
                    x.frac_value,
                    x.frac_slope,
                ),
                (
                    self.u8(tf + 0x10),
                    self.u8(tf + 0x11),
                    self.u8(tf + 0x12),
                    self.u8(tf + 0x13),
                    self.u8(tf + 0x14),
                    self.u8(tf + 0x15),
                ),
            );
            self.check(
                &fp,
                "keys",
                (x.nb_pack, x.startframe as u16, x.fterm),
                (
                    self.u16(tf + 0x16),
                    self.u16(tf + 0x18),
                    self.u16(tf + 0x1A),
                ),
            );
            self.check(
                &fp,
                "values",
                [x.time, x.p0, x.p1, x.d0, x.d1].map(f32::to_bits),
                [
                    self.u32(tf + 0x1C),
                    self.u32(tf + 0x20),
                    self.u32(tf + 0x24),
                    self.u32(tf + 0x28),
                    self.u32(tf + 0x2C),
                ],
            );
            f = x.next;
            tf = self.u32(tf);
            i += 1;
        }
        let jo = a.hsd_obj;
        self.check(
            path,
            "bound joint",
            jo.map(|j| self.game.objs.jobjs[j].id),
            (self.u32(t + 0x18) != 0).then(|| self.data_ref(self.u32(self.u32(t + 0x18) + 0x84))),
        );
    }

    fn robjs(&mut self, path: &str, o: Option<RObjId>, t: u32) {
        let (mut o, mut t) = (o, t);
        let mut i = 0;
        while o.is_some() || t != 0 {
            let rp = format!("{path} robj {i}");
            if !Self::pair(
                &mut self.maps.robjs,
                &mut self.seen,
                &mut self.diffs,
                "robj",
                &rp,
                o,
                t,
            ) {
                break;
            }
            let r = &self.game.objs.robjs[o.unwrap()];
            self.check(&rp, "flags", r.flags, self.u32(t + 4));
            match r.u {
                Ref::Limit(l) => self.check(&rp, "limit", l.to_bits(), self.u32(t + 8)),
                Ref::IkHint {
                    bone_length,
                    rotate_x,
                } => self.check(
                    &rp,
                    "IK hint",
                    (bone_length.to_bits(), rotate_x.to_bits()),
                    (self.u32(t + 8), self.u32(t + 0xC)),
                ),
                Ref::JObj(_) | Ref::None if r.flags & robj::TYPE_MASK == robj::JOBJ => {}
                _ => {}
            }
            self.aobj(&format!("{rp} aobj"), r.aobj, self.u32(t + 0x18));
            o = r.next;
            t = self.u32(t);
            i += 1;
        }
    }

    fn wobj(&mut self, path: &str, o: Option<melee_sim::hsd::wobj::WObjId>, t: u32) {
        if !Self::pair(
            &mut self.maps.wobjs,
            &mut self.seen,
            &mut self.diffs,
            "wobj",
            path,
            o,
            t,
        ) {
            return;
        }
        let w = &self.game.objs.wobjs[o.unwrap().0];
        self.check(
            path,
            "refs and flags",
            (w.ref_count.0, w.flags),
            (self.u16(t + 4), self.u32(t + 8)),
        );
        self.check(
            path,
            "position",
            [w.pos.x, w.pos.y, w.pos.z].map(f32::to_bits),
            [self.u32(t + 0xC), self.u32(t + 0x10), self.u32(t + 0x14)],
        );
        self.aobj(&format!("{path} aobj"), w.aobj, self.u32(t + 0x18));
        self.robjs(path, w.robj, self.u32(t + 0x1C));
    }

    fn cobj(&mut self, path: &str, o: melee_sim::hsd::cobj::CObjId, t: u32) {
        if !Self::pair(
            &mut self.maps.cobjs,
            &mut self.seen,
            &mut self.diffs,
            "cobj",
            path,
            Some(o),
            t,
        ) {
            return;
        }
        let c = &self.game.objs.cobjs[o];
        let words = |s: &Self, a: u32, n: u32| (0..n).map(|i| s.u32(a + 4 * i)).collect::<Vec<_>>();
        self.check(
            path,
            "refs and flags",
            (c.ref_count.0, c.flags, c.projection_type),
            (self.u16(t + 4), self.u32(t + 8), self.u8(t + 0x50)),
        );
        self.check(
            path,
            "viewport",
            c.viewport.map(f32::to_bits).to_vec(),
            words(self, t + 0xC, 4),
        );
        self.check(
            path,
            "scissor",
            c.scissor,
            [
                self.u16(t + 0x1C),
                self.u16(t + 0x1E),
                self.u16(t + 0x20),
                self.u16(t + 0x22),
            ],
        );
        let mut proj = vec![c.up.x, c.up.y, c.up.z, c.near, c.far];
        proj.extend(c.projection);
        self.check(
            path,
            "up, planes and projection",
            proj.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            words(self, t + 0x2C, 9),
        );
        self.check(
            path,
            "view matrix",
            c.view_mtx
                .m
                .iter()
                .flatten()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            words(self, t + 0x54, 12),
        );
        let pm = self.u32(t + 0x88);
        self.check(
            path,
            "inverse view matrix",
            c.inv_view_mtx.map(|m| {
                m.m.iter()
                    .flatten()
                    .map(|v| v.to_bits())
                    .collect::<Vec<_>>()
            }),
            (pm != 0).then(|| words(self, pm, 12)),
        );
        let (eye, interest, aobj) = (c.eyepos, c.interest, c.aobj);
        self.wobj(&format!("{path} eye"), eye, self.u32(t + 0x24));
        self.wobj(&format!("{path} interest"), interest, self.u32(t + 0x28));
        self.aobj(&format!("{path} aobj"), aobj, self.u32(t + 0x84));
    }

    fn lobjs(&mut self, path: &str, o: Option<melee_sim::hsd::lobj::LObjId>, t: u32) {
        let (mut o, mut t) = (o, t);
        let mut i = 0;
        while o.is_some() || t != 0 {
            let lp = format!("{path} {i}");
            if !Self::pair(
                &mut self.maps.lobjs,
                &mut self.seen,
                &mut self.diffs,
                "lobj",
                &lp,
                o,
                t,
            ) {
                break;
            }
            let l = &self.game.objs.lobjs[o.unwrap()];
            let rgba = |s: &Self, a: u32| [s.u8(a), s.u8(a + 1), s.u8(a + 2), s.u8(a + 3)];
            self.check(
                &lp,
                "refs, flags and priority",
                (l.ref_count.0, l.flags, l.priority),
                (self.u16(t + 4), self.u16(t + 8), self.u16(t + 0xA)),
            );
            self.check(
                &lp,
                "colors",
                (l.color, l.hw_color),
                (rgba(self, t + 0x10), rgba(self, t + 0x14)),
            );
            self.check(
                &lp,
                "params",
                l.params.to_vec(),
                (0..6)
                    .map(|k| self.u32(t + 0x20 + 4 * k))
                    .collect::<Vec<_>>(),
            );
            self.check(
                &lp,
                "shininess and vector",
                [l.shininess, l.lvec.x, l.lvec.y, l.lvec.z].map(f32::to_bits),
                [
                    self.u32(t + 0x38),
                    self.u32(t + 0x3C),
                    self.u32(t + 0x40),
                    self.u32(t + 0x44),
                ],
            );
            self.check(
                &lp,
                "light slots",
                (l.id, l.spec_id),
                (self.u32(t + 0x4C), self.u32(t + 0x90)),
            );
            let (pos, interest, aobj, next) = (l.position, l.interest, l.aobj, l.next);
            self.wobj(&format!("{lp} position"), pos, self.u32(t + 0x18));
            self.wobj(&format!("{lp} interest"), interest, self.u32(t + 0x1C));
            self.aobj(&format!("{lp} aobj"), aobj, self.u32(t + 0x48));
            o = next;
            t = self.u32(t + 0xC);
            i += 1;
        }
    }

    fn mobj(&mut self, path: &str, o: Option<melee_sim::hsd::mobj::MObjId>, t: u32) {
        if !Self::pair(
            &mut self.maps.mobjs,
            &mut self.seen,
            &mut self.diffs,
            "mobj",
            path,
            o,
            t,
        ) {
            return;
        }
        let m = &self.game.objs.mobjs[o.unwrap()];
        let class = match m.class {
            melee_sim::hsd::mobj::MObjClass::Hsd => sym("hsdMObj"),
            melee_sim::hsd::mobj::MObjClass::Ground => sym("grMaterial_803E0A20"),
        };
        // Classes no port models (fighters' `ftMObj`) were reported when imported.
        let theirs = self.u32(t);
        if [sym("hsdMObj"), sym("grMaterial_803E0A20")].contains(&theirs) {
            self.check(path, "class", class, theirs);
        }
        self.check(path, "rendermode", m.rendermode, self.u32(t + 4));
        let mat = self.u32(t + 0xC);
        let theirs = (mat != 0).then(|| {
            let c = |a: u32| [self.u8(a), self.u8(a + 1), self.u8(a + 2), self.u8(a + 3)];
            (
                c(mat),
                c(mat + 4),
                c(mat + 8),
                self.u32(mat + 0xC),
                self.u32(mat + 0x10),
            )
        });
        let ours = m.mat.map(|x| {
            let c = |c: melee_sim::hsd::tobj::Color| [c.r, c.g, c.b, c.a];
            (
                c(x.ambient),
                c(x.diffuse),
                c(x.specular),
                x.alpha.to_bits(),
                x.shininess.to_bits(),
            )
        });
        self.check(path, "material", ours, theirs);
        self.aobj(&format!("{path} aobj"), m.aobj, self.u32(t + 0x14));
        let (mut to, mut tt) = (m.tobj, self.u32(t + 8));
        let mut i = 0;
        while to.is_some() || tt != 0 {
            let tp = format!("{path} tobj {i}");
            if !Self::pair(
                &mut self.maps.tobjs,
                &mut self.seen,
                &mut self.diffs,
                "tobj",
                &tp,
                to,
                tt,
            ) {
                break;
            }
            let x = &self.game.objs.tobjs[to.unwrap()];
            let w = |s: &Self, o: u32| s.u32(tt + o);
            self.check(
                &tp,
                "ids",
                (x.id, x.src, x.mtxid, x.coord),
                (w(self, 0xC), w(self, 0x10), w(self, 0x14), w(self, 0xA4)),
            );
            self.check(
                &tp,
                "flags and blending",
                (x.flags, x.blending.to_bits(), x.tlut_no),
                (w(self, 0x4C), w(self, 0x50), self.u8(tt + 0x70)),
            );
            let q = &x.rotate;
            let srt: Vec<u32> = [
                q.x,
                q.y,
                q.z,
                q.w,
                x.scale.x,
                x.scale.y,
                x.scale.z,
                x.translate.x,
                x.translate.y,
                x.translate.z,
            ]
            .iter()
            .map(|v| v.to_bits())
            .collect();
            let theirs: Vec<u32> = (0..10).map(|k| w(self, 0x18 + 4 * k)).collect();
            self.check(&tp, "SRT", srt, theirs);
            let mtx: Vec<u32> = x.mtx.m.iter().flatten().map(|v| v.to_bits()).collect();
            let theirs: Vec<u32> = (0..12).map(|k| w(self, 0x74 + 4 * k)).collect();
            self.check(&tp, "matrix", mtx, theirs);
            self.aobj(&format!("{tp} aobj"), x.aobj, w(self, 0x64));
            to = x.next;
            tt = w(self, 8);
            i += 1;
        }
    }

    fn dobjs(&mut self, path: &str, o: Option<melee_sim::hsd::dobj::DObjId>, t: u32) {
        let (mut o, mut t) = (o, t);
        let mut i = 0;
        while o.is_some() || t != 0 {
            let dp = format!("{path} dobj {i}");
            if !Self::pair(
                &mut self.maps.dobjs,
                &mut self.seen,
                &mut self.diffs,
                "dobj",
                &dp,
                o,
                t,
            ) {
                break;
            }
            let d = &self.game.objs.dobjs[o.unwrap()];
            self.check(&dp, "flags", d.flags, self.u32(t + 0x14));
            self.mobj(&format!("{dp} mobj"), d.mobj, self.u32(t + 8));
            self.aobj(&format!("{dp} aobj"), d.aobj, self.u32(t + 0x10));
            let (mut p, mut tp) = (d.pobj, self.u32(t + 0xC));
            let mut k = 0;
            while p.is_some() || tp != 0 {
                let pp = format!("{dp} pobj {k}");
                if !Self::pair(
                    &mut self.maps.pobjs,
                    &mut self.seen,
                    &mut self.diffs,
                    "pobj",
                    &pp,
                    p,
                    tp,
                ) {
                    break;
                }
                let po = &self.game.objs.pobjs[p.unwrap()];
                self.check(&pp, "flags", po.flags, self.u16(tp + 0xC));
                if let Binding::ShapeSet(set) = &po.u {
                    let u = self.u32(tp + 0x14);
                    self.aobj(&format!("{pp} shape aobj"), set.aobj, self.u32(u + 0x20));
                }
                p = po.next;
                tp = self.u32(tp + 4);
                k += 1;
            }
            o = d.next;
            t = self.u32(t + 4);
            i += 1;
        }
    }
}

impl Cx for Comparer<'_> {
    fn import_handle(&mut self, _: &'static str, _: u32) -> Option<u32> {
        unreachable!("the comparer only compares")
    }

    fn import_data(&mut self, _: u32) -> Option<DataRef> {
        unreachable!("the comparer only compares")
    }

    fn compare_handle(&mut self, path: &str, kind: &'static str, ours: Option<u32>, theirs: u32) {
        let known = match kind {
            "JObjId" => self.maps.jobjs.get(&theirs).map(|j| j.0),
            "GObjId" => self.maps.gobjs.get(&theirs).map(|g| g.0),
            "FighterId" => self.maps.fighters.get(&theirs).map(|f| f.0),
            "DObjId" => self.maps.dobjs.get(&theirs).map(|d| d.0),
            "GroundId" => self.maps.grounds.get(&theirs).map(|g| g.0),
            "StartHookId" => self.maps.start_hooks.get(&theirs).map(|h| h.0),
            "CameraBoxId" if theirs != 0 => Some(box_index(self.u32(sym("cm_804D645C")), theirs)),
            _ => None,
        };
        let known = known.or_else(|| self.handles.get(&(kind, theirs)).copied());
        match (ours, theirs, known) {
            (None, 0, _) => {}
            (Some(o), t, Some(k)) if t != 0 && k == o => {}
            (Some(o), t, None) if t != 0 => {
                self.handles.insert((kind, t), o);
            }
            _ => self.diffs.push(format!(
                "{path}: {kind} ours {ours:?}, theirs {theirs:#x} (paired with {known:?})"
            )),
        }
    }

    fn compare_data(&mut self, path: &str, ours: Option<DataRef>, theirs: u32) {
        let t = self.data_ref(theirs);
        if ours != t && !(ours.is_none() && theirs != 0 && t.is_none()) {
            self.diffs
                .push(format!("{path}: data ours {ours:?}, theirs {theirs:#x}"));
        }
    }

    fn mismatch(&mut self, path: &str, ours: String, theirs: String) {
        self.diffs
            .push(format!("{path}: ours {ours}, theirs {theirs}"));
    }
}
