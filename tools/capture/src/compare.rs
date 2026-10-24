//! Compares melee-sim's state with ssbm-rs's memory: the scheduler's lists, every joint tree,
//! its animations, references, display lists and shapes, and the fighters. Objects pair by
//! where they are reached from (list position, tree position), starting from the importer's
//! maps, so objects either side made since pair too.

use std::collections::{HashMap, HashSet};

use melee_data::archive::{Archives, DataRef};
use melee_sim::game::handles::{BoneTableId, EffectId, FighterId};
use melee_sim::game::mirror::{Cx, Mem, Mirror};
use melee_sim::game::state::{Game, Obj, UserData};
use melee_sim::game::structs::FighterBone;
use melee_sim::hsd::aobj::AObjId;
use melee_sim::hsd::gobj::{GObjId, P_LINKS};
use melee_sim::hsd::jobj::{Contents, JObjId};
use melee_sim::hsd::pobj::Binding;
use melee_sim::hsd::pool::Pool;
use melee_sim::hsd::robj::{self, RObjId, Ref};

use crate::import::{FIGHTER_ATTRS_SIZE, Maps, VS_CONTROLLER, sym, try_box_index};
use crate::{DataMap, FT_ANIM_FILES, FT_KINDS, MEM1};

/// The addresses the allocator model has given each pool slot's objects, noted after every
/// tick: a handle kept past its object's free (a dangling pointer) names the address of the
/// object it was set to, which the slot no longer holds once it's reused. Joints, display
/// objects, materials and textures come from the class allocator, whose blocks are reused in
/// another order than the pools' slots, so a slot's objects can sit at several addresses.
#[derive(Clone, Default)]
pub struct PastAddrs {
    addrs: HashMap<(&'static str, u32), Vec<u32>>,
    /// Archives an earlier scan found that the latest doesn't (their scene's heap went with
    /// them, and their bytes since changed), where they were last: what the original's
    /// pointers into them still name.
    gone: Vec<crate::Loaded>,
}

impl PastAddrs {
    /// Notes the archives the latest scan no longer finds, at their last place.
    pub fn note_archives(&mut self, before: &[crate::Loaded], now: &[crate::Loaded]) {
        for l in before {
            if !now.iter().any(|n| n.id == l.id) {
                self.gone.retain(|g| g.id != l.id);
                self.gone.push(l.clone());
            }
        }
        self.gone.retain(|g| !now.iter().any(|n| n.id == g.id));
    }

    /// Where data in an archive no longer loaded was, by its last place.
    pub(crate) fn gone_addr(&self, d: DataRef) -> Option<u32> {
        self.gone
            .iter()
            .find(|g| g.id == d.archive)
            .map(|g| g.data + d.offset)
    }

    /// Notes each slot's address, where it's new.
    pub fn note(&mut self, game: &Game) {
        fn pool<T, const K: u8>(
            addrs: &mut HashMap<(&'static str, u32), Vec<u32>>,
            kind: &'static str,
            p: &Pool<T, K>,
        ) {
            for slot in 0..p.slots().len() as u32 {
                let a = p.addr(slot);
                if a == 0 {
                    continue;
                }
                let v = addrs.entry((kind, slot)).or_default();
                if v.last() != Some(&a) {
                    v.push(a);
                }
            }
        }
        let a = &mut self.addrs;
        let o = &game.objs;
        pool(a, "JObjId", &o.jobjs.pool);
        pool(a, "DObjId", &o.dobjs.pool);
        pool(a, "MObjId", &o.mobjs.pool);
        pool(a, "TObjId", &o.tobjs.pool);
        pool(a, "GeneratorId", &o.generators.pool);
        pool(a, "GObjId", &game.world.gobjs);
        pool(a, "FighterId", &game.fighters);
        pool(a, "ItemId", &game.items);
        pool(a, "EffectId", &game.ef.effects);
        pool(a, "QueuedId", &game.ef.queued);
    }

    /// Whether slot `o` of `kind`'s pool has held an object at `addr`.
    fn had(&self, kind: &'static str, o: u32, addr: u32) -> bool {
        self.addrs
            .get(&(kind, o))
            .is_some_and(|v| v.contains(&addr))
    }
}

/// The pool a handle kind names objects of, for the kinds whose slots carry the allocator
/// model's addresses.
fn pool_kind(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "JObjId" => "JObjId",
        "DObjId" => "DObjId",
        "MObjId" => "MObjId",
        "TObjId" => "TObjId",
        "GeneratorId" => "GeneratorId",
        "GObjId" => "GObjId",
        "FighterId" | "VictimFighter" => "FighterId",
        "ItemId" | "VictimItem" => "ItemId",
        "EffectId" => "EffectId",
        "QueuedId" => "QueuedId",
        _ => return None,
    })
}

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
    /// ssbm-rs addresses the import paired with objects we have freed since: if the
    /// reference's heap gives such an address to a new object, it pairs afresh.
    pub(crate) stale: HashSet<(&'static str, u32)>,
    pub diffs: Vec<String>,
    /// The fields code read while they were stale (`Cx::stale_read`).
    pub stale_reads: Vec<String>,
    /// The archives, where the caller has them: fighters' animation buffers then compare
    /// byte for byte where we know what they hold.
    pub archives: Option<&'a Archives>,
    /// The addresses pool slots' earlier objects had, where the caller has followed them.
    pub past: Option<&'a PastAddrs>,
}

impl<'a> Comparer<'a> {
    pub fn new(mem: &'a [u8], data: &'a DataMap, game: &'a Game, maps: Maps) -> Self {
        let o = &game.objs;
        let mut stale = HashSet::new();
        let mut add = |kind: &'static str, entries: Vec<(u32, u32)>, free: &dyn Fn(u32) -> bool| {
            for (t, id) in entries {
                if free(id) {
                    stale.insert((kind, t));
                }
            }
        };
        add(
            "jobj",
            maps.jobjs.iter().map(|(&t, j)| (t, j.0)).collect(),
            &|i| o.jobjs.pool.was_freed(i),
        );
        add(
            "dobj",
            maps.dobjs.iter().map(|(&t, d)| (t, d.0)).collect(),
            &|i| o.dobjs.pool.was_freed(i),
        );
        add(
            "pobj",
            maps.pobjs.iter().map(|(&t, d)| (t, d.0)).collect(),
            &|i| o.pobjs.pool.was_freed(i),
        );
        add(
            "mobj",
            maps.mobjs.iter().map(|(&t, d)| (t, d.0)).collect(),
            &|i| o.mobjs.pool.was_freed(i),
        );
        add(
            "tobj",
            maps.tobjs.iter().map(|(&t, d)| (t, d.0)).collect(),
            &|i| o.tobjs.pool.was_freed(i),
        );
        add(
            "aobj",
            maps.aobjs.iter().map(|(&t, d)| (t, d.0)).collect(),
            &|i| o.aobjs.pool.was_freed(i),
        );
        add(
            "robj",
            maps.robjs.iter().map(|(&t, d)| (t, d.0)).collect(),
            &|i| o.robjs.pool.was_freed(i),
        );
        // A stage object destroyed and another made in one call (Corneria's load hook remakes
        // its city).
        add(
            "ground",
            maps.grounds.iter().map(|(&t, g)| (t, g.0)).collect(),
            &|i| game.grounds.was_freed(i),
        );
        Comparer {
            mem,
            data,
            game,
            maps,
            handles: HashMap::new(),
            seen: HashSet::new(),
            stale,
            diffs: Vec::new(),
            stale_reads: Vec::new(),
            archives: None,
            past: None,
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

    pub(crate) fn data_ref(&self, a: u32) -> Option<DataRef> {
        (a != 0).then(|| self.data.get(a)).flatten()
    }

    /// Pairs `ours` with ssbm-rs's object at `theirs`, or reports they don't pair; whether to
    /// compare them (the first time this comparison meets them).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn pair<T: Copy + PartialEq + std::fmt::Debug>(
        map: &mut HashMap<u32, T>,
        seen: &mut HashSet<(&'static str, u32)>,
        stale: &HashSet<(&'static str, u32)>,
        diffs: &mut Vec<String>,
        kind: &'static str,
        path: &str,
        ours: Option<T>,
        theirs: u32,
    ) -> bool {
        match (ours, theirs) {
            (None, 0) => false,
            (Some(o), t) if t != 0 => {
                let mut known = *map.entry(t).or_insert(o);
                if known != o && stale.contains(&(kind, t)) {
                    map.insert(t, o);
                    known = o;
                }
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

    /// The audio driver, the synth and the AX voices, byte for byte.
    pub fn audio(&mut self) {
        crate::audio::compare(self.mem, &self.game.audio, &mut self.diffs);
        crate::audio::compare_crowd(self.mem, &self.game.crowd, &mut self.diffs);
    }

    /// The scheduler: each process link's GObjs in order, and what they hold.
    pub fn world(&mut self) {
        // The toon texture, which materials' compiled TEV stages name before the display
        // statics compare it (`Comparer::display`).
        let t = self.u32(sym("tobj_toon"));
        if let Some(o) = self.game.objs.mobjs.toon
            && t != 0
        {
            self.maps.tobjs.entry(t).or_insert(o);
        }
        let heads = self.u32(sym("HSD_GObjPLinkHead"));
        let w = &self.game.world;
        // Our GObjs on the process links. A GObj the importer made for a pointer that
        // outlived its GObj is on none; if the reference's heap has since given that address
        // to a new GObj, the new one pairs by its place in the lists instead.
        let mut linked = HashSet::new();
        for link in 0..P_LINKS {
            let mut o = w.plink_head[link];
            while let Some(g) = o {
                linked.insert(g);
                o = w.gobjs[g.0].next;
            }
        }
        for link in 0..P_LINKS {
            let mut t = self.u32(heads + 4 * link as u32);
            let mut o = w.plink_head[link];
            let mut i = 0;
            loop {
                let path = format!("p_link {link} #{i}");
                match (o, t) {
                    (None, 0) => break,
                    (Some(og), tg) if tg != 0 => {
                        let known = *self.maps.gobjs.entry(tg).or_insert(og);
                        if known != og && !linked.contains(&known) {
                            self.maps.gobjs.insert(tg, og);
                        }
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
        self.statics();
        self.effects();
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
                (
                    self.u8(tp + 0xC),
                    crate::playback::code_ref(self.mem, self.u32(tp + 0x14)).0,
                ),
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
            Some(Obj::SObj(s)) => self.sobjs(&format!("{path} sobj"), s, obj),
            None => {}
        }
        match g.user_data {
            Some(UserData::Fighter(f)) => {
                let ud = self.u32(t + 0x2C);
                if Self::pair(
                    &mut self.maps.fighters,
                    &mut self.seen,
                    &self.stale,
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
                    // The SDI colors code's count, which only the code reads.
                    if self.game.rules.player.sdi_colors {
                        let ours = self.game.player_code_state.sdi_count.get(f.0 as usize);
                        let theirs = self.mem[(ud + crate::import::SDI_COUNT - MEM1) as usize];
                        self.check(
                            path,
                            "SDI colors' count",
                            ours.copied().unwrap_or(0),
                            theirs,
                        );
                    }
                    let address = self.game.ft.addresses.get(&f.0).copied();
                    if address != Some(ud) {
                        self.diffs.push(format!(
                            "{path} fighter: address ours {address:x?}, theirs {ud:#x}"
                        ));
                    }
                    self.fighter_blocks(&format!("{path} fighter"), f, ud);
                }
            }
            Some(UserData::Ground(g)) => self.ground(path, g, self.u32(t + 0x2C)),
            Some(UserData::Item(i)) => self.item(path, i, self.u32(t + 0x2C)),
            Some(UserData::ItemLink(l)) => self.item_link(path, l, self.u32(t + 0x2C)),
            Some(UserData::Sound(s)) => self.sound_obj(path, s, self.u32(t + 0x2C)),
            Some(UserData::Raw(a)) => self.check(path, "user data", a.0, self.u32(t + 0x2C)),
            Some(UserData::BgFlash) => {
                let (ud, release) = (self.u32(t + 0x2C), self.u32(t + 0x30));
                self.check(path, "flash release", melee_sim::lb::bgflash::FREE, release);
                let mem = Mem {
                    bytes: self.mem,
                    base: MEM1,
                };
                match self.game.bgflash.user.clone() {
                    Some(u) => u.compare(&mem, ud, &format!("{path} flash"), self),
                    None => self.diff(path, "flash user data", "none", format!("{ud:#x}")),
                }
            }
            Some(UserData::Effect(e)) => {
                self.effect(&format!("{path} effect"), e, self.u32(t + 0x2C))
            }
            Some(UserData::StadiumScreen(which)) => {
                crate::stadium::compare_screen(self, path, which, t)
            }
            Some(UserData::IzumiReflection) => crate::izumi::compare_reflection(self, path, t),
            Some(UserData::MainMenuList(a)) => self.main_menu_list(path, a, t),
            Some(UserData::MainMenuPanel(a)) => self.main_menu_panel(path, a, t),
            Some(UserData::GalleryList(a)) => self.gallery_list(path, a, t),
            Some(UserData::GalleryCursor(a)) => self.gallery_cursor(path, a, t),
            Some(UserData::RulesList(a)) => self.rules_list(path, a, t),
            Some(UserData::RulePlusList(a)) => self.rule_plus_list(path, a, t),
            Some(UserData::ItemSwitch(a)) => self.item_switch(path, a, t),
            Some(UserData::StageSwitch(a)) => self.stage_switch(path, a, t),
            Some(UserData::NameList(a)) => self.name_list(path, a, t),
            Some(UserData::NameNew(a)) => self.name_new(path, a, t),
            Some(UserData::GlyphVariants(a)) => self.glyph_variants(path, a, t),
            None => {
                // Kindless user data ours lacks.
                let ud = self.u32(t + 0x2C);
                if g.user_data_kind == 0xFF && crate::hud::kindless_user_data(ud).is_some() {
                    self.diff(path, "kindless user data", "none", format!("{ud:#x}"));
                }
            }
            Some(ud) => crate::hud::compare_user_data(self, path, ud, t),
        }
    }

    /// The static structs the importer brings in (`Importer::statics`).
    pub fn statics(&mut self) {
        let mem = Mem {
            bytes: self.mem,
            base: MEM1,
        };
        let game = self.game;
        let seed = self.u32(self.u32(sym("HSD_RandSeedPtr")));
        self.check("HSD_RandSeedPtr", "seed", game.objs.rng.seed, seed);
        let arg_buf = self.u32(sym("arg_buf")) != 0;
        self.check("arg_buf", "allocated", game.objs.robjs.arg_buf, arg_buf);
        game.magnify
            .compare(&mem, sym("ifMagnify_804A1DE0"), "magnify", self);
        let (theirs, filter_gobj) = crate::read_coll_statics(self.mem);
        let key = |s: &melee_sim::mp::coll::CollStatics| {
            let v = |v: melee_sim::vec::Vec3| [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()];
            format!(
                "{:?}/{} {:?}/{} {:#x} {} {:#x} {:x?} {} {:?} {:?}",
                s.right_walls,
                s.right_count,
                s.left_walls,
                s.left_count,
                s.max_x.to_bits(),
                s.line_id,
                s.flags,
                v(s.normal),
                s.ecb_tiny,
                s.floor_filter,
                s.x64a8
            )
        };
        self.check("mp", "collision statics", key(&game.coll), key(&theirs));
        self.compare_handle(
            "mp collision floor filter fighter",
            "GObjId",
            game.coll.floor_filter_gobj.map(|g| g.0),
            filter_gobj,
        );
        self.fighter_data_globals();
        for (name, offset) in [
            ("p_ftCommonData", 0),
            ("Fighter_804D6548", melee_sim::game::ft::STALE_WEIGHTS),
            ("Fighter_804D6524", melee_sim::game::ft::SCALE_PARAMS),
        ] {
            // Ours is a pointer in the loaded `ftLoadCommonData`, which sits in their memory.
            let ours = game
                .ft
                .load_common
                .map_or(0, |c| self.u32(self.data.addr(c.at(offset))));
            let theirs = self.u32(sym(name));
            self.check("ft", name, ours, theirs);
        }
        let n = self.u16(sym("staleAttackInstance"));
        self.check("pl", "attack instance", game.pl.stale_attack_instance, n);
        let n = self.u16(sym("unk_804D6480"));
        self.check("pl", "attack record", game.pl.attack_instance, n);
        for kind in 0..FT_KINDS {
            let aram = self.u32(FT_ANIM_FILES + 8 * kind);
            let ours = game
                .ft
                .anim_files
                .get(kind as usize)
                .and_then(|f| f.as_ref());
            self.check(
                "ft",
                &format!("anim file {kind}"),
                ours.map_or(0, |f| f.aram),
                aram,
            );
        }
        game.vs.compare(&mem, VS_CONTROLLER, "vs", self);
        let g = &game.gm;
        self.check(
            "gm",
            "pause flags, debug level, trophies' data loaded",
            (game.scene.unk_10.x0, g.db_level, g.trophy_data),
            (
                self.u8(sym("gm_80479D58") + 0x10),
                self.u32(sym("DbLevel")) as i32,
                self.u32(sym("_Toy_sbss_804D6ED0")) != 0,
            ),
        );
        let unlocks = melee_sim::game::gm::Unlocks::from_memory(&|a| self.u32(a));
        self.check("gm", "unlocks", g.unlocks, unlocks);
        let pads = sym("HSD_PadGameStatus");
        for (i, p) in game.pads.game.iter().enumerate() {
            let at = pads + 0x44 * i as u32;
            let theirs = crate::import::read_pad(self.mem, at);
            self.check(&format!("pads[{i}]"), "status", p, &theirs);
        }
    }

    /// The fighter data's globals, as [`crate::import::Importer`] imports them.
    fn fighter_data_globals(&mut self) {
        use crate::import::*;
        use melee_sim::game::ft::data::{KIND_COUNT, kind_tables};
        let game = self.game;
        let ft = &game.ft;
        for kind in 0..KIND_COUNT as u32 {
            let k = kind as usize;
            let d = self.u32(FT_DATA_LIST + 4 * kind);
            self.compare_data(&format!("ft data {kind}"), ft.data.data_list[k], d);
            let list = self.u32(COSTUME_LISTS + 8 * kind);
            let n = if list != 0 {
                kind_tables(kind as i32).costume_count
            } else {
                0
            };
            for c in 0..u32::from(n) {
                let e = list + 0x18 * c;
                let ours = ft.data.costumes[k][c as usize];
                let path = format!("ft costume {kind}/{c}");
                self.compare_data(&format!("{path} joint"), ours.joint, self.u32(e));
                self.compare_data(&format!("{path} matanim"), ours.matanim, self.u32(e + 4));
                self.check(&path, "x8", ours.x8, self.u32(e + 8));
            }
            self.check(
                "ft",
                &format!("demo anims {kind}"),
                ft.data.demo_anims[k],
                self.u32(DEMO_ANIMS + 8 * kind),
            );
            self.check(
                "ft",
                &format!("ref count {kind}"),
                ft.data.ref_counts[k],
                self.u32(REF_COUNTS + 4 * kind) as i32,
            );
            self.compare_data(
                &format!("ft kirby copy {kind}"),
                ft.kirby.copies[k],
                self.u32(KIRBY_COPIES + 4 * kind),
            );
        }
        for s in 0..6u32 {
            let at = SLOT_WORDS + 8 * s;
            let o = ft.data.slot_words[s as usize];
            self.check(
                "ft",
                &format!("slot word {s}"),
                (o.x0, o.x6),
                (self.u32(at), self.u8(at + 6)),
            );
        }
        self.check(
            "ft",
            "spawn counter",
            ft.spawn_counter,
            self.u32(SPAWN_COUNTER),
        );
        // The free dynamics links, in order.
        let mut ours = ft.dynamics_free;
        let mut theirs = self.u32(DYNAMICS_FREE);
        let mut i = 0;
        while ours.is_some() || theirs != 0 {
            self.compare_handle(
                &format!("ft free dynamics link {i}"),
                "DynamicsId",
                ours.map(|d| d.0),
                theirs,
            );
            if ours.is_none() || theirs == 0 {
                break;
            }
            ours = game.fighter_dynamics[ours.unwrap().0].next;
            theirs = self.u32(theirs + 0x90);
            i += 1;
        }
        for (at, ours, name) in [(LOBJ0, ft.lobj0, "lobj0"), (LOBJ1, ft.lobj1, "lobj1")] {
            let t = self.u32(at);
            match ours {
                Some(_) if t != 0 => self.lobjs(&format!("ft {name}"), ours, t),
                _ => self.check("ft", name, ours.is_some(), t != 0),
            }
        }
        for (at, ours, name) in [
            (METAL_MOBJ, ft.metal_mobj, "metal mobj"),
            (CLEAR_MOBJ, ft.clear_mobj, "clear mobj"),
        ] {
            self.mobj(&format!("ft {name}"), ours, self.u32(at));
        }
        for (i, &kind) in melee_sim::game::ft::KIRBY_COSTUME_KINDS.iter().enumerate() {
            let table = self.u32(KIRBY_TABLES + 4 * kind as u32);
            for c in 0..6u32 {
                let (j, m) = ft.kirby.models[i][c as usize];
                let path = format!("ft kirby costume {kind}/{c}");
                self.compare_data(&format!("{path} joint"), j, self.u32(table + 8 * c));
                self.compare_data(&format!("{path} matanim"), m, self.u32(table + 8 * c + 4));
            }
        }
        for i in 0..5u32 {
            self.compare_data(
                &format!("ft purin hat {i}"),
                ft.purin_hats[i as usize],
                self.u32(PURIN_HATS + 4 * i),
            );
        }
    }

    /// The blocks a fighter owns: its parts table, its attribute copy and its dynamic bone
    /// chains.
    fn fighter_blocks(&mut self, path: &str, f: FighterId, t: u32) {
        let fighter = &self.game.fighters[f.0];
        let mem = Mem {
            bytes: self.mem,
            base: MEM1,
        };
        let tp = self.u32(t + 0x5E8);
        if let Some(p) = fighter.parts
            && tp != 0
        {
            // Past the character's bones only the last entry is written, by Samus's grapple
            // beam or Link's and Young Link's hookshots, for their tip.
            let table = self.u32(self.u32(sym("ftPartsTable")) + 4 * fighter.kind as u32);
            let n = self.u32(table + 8) as usize;
            for (i, bone) in self.game.fighter_parts[p.0].iter().enumerate() {
                if i >= n && i != 139 {
                    continue;
                }
                let at = tp + i as u32 * FighterBone::SIZE;
                bone.compare(&mem, at, &format!("{path}.parts[{i}]"), self);
            }
        }
        for (off, field, ours) in [
            (0x2D4, "dat_attrs", fighter.dat_attrs),
            (0x2D8, "dat_attrs_backup", fighter.dat_attrs_backup),
        ] {
            let ta = self.u32(t + off);
            if let Some(a) = ours
                && ta != 0
            {
                let theirs = &self.mem[(ta - MEM1) as usize..][..FIGHTER_ATTRS_SIZE as usize];
                let o = &self.game.fighter_attrs[a.0];
                if let Some(i) = (0..o.len()).find(|&i| o[i] != theirs[i]) {
                    self.diffs.push(format!(
                        "{path}.{field}: byte {i:#x}: ours {:#04x}, theirs {:#04x}",
                        o[i], theirs[i]
                    ));
                }
            }
        }
        // The DObj lists' first `count` entries, and the fighter's light.
        for (field, list, at) in [
            ("dobj_list", fighter.dobj_list.data, t + 0x5EC),
            ("x203C", fighter.x203C.data, t + 0x203C),
        ] {
            let (count, data) = (self.u32(at), self.u32(at + 4));
            if let Some(l) = list
                && data != 0
            {
                let ours = self.game.fighter_dobj_lists[l.0].clone();
                for i in 0..count {
                    let o = ours.get(i as usize).copied().flatten();
                    let d = self.u32(data + 4 * i);
                    self.dobjs_one(&format!("{path}.{field}[{i}]"), o, d);
                }
            }
        }
        let shadow = self.u32(t + 0x20A4 + 4);
        if let Some(s) = fighter.x20A4.shadow
            && shadow != 0
        {
            self.shadow(&format!("{path}.x20A4.shadow"), s, shadow);
        }
        let light = self.u32(t + 0x588);
        if fighter.x588.is_some() && light != 0 {
            self.lobjs(&format!("{path}.x588"), fighter.x588, light);
        }
        // Each chain link, where both sides have one (the handles compare as the fighter
        // does).
        for i in 0..fighter.dynamic_bone_sets.len() {
            let mut ours = fighter.dynamic_bone_sets[i].dyn_desc.data;
            let mut theirs = self.u32(t + 0x2F0 + 0x18 * i as u32 + 4);
            let mut k = 0;
            while let Some(d) = ours
                && theirs != 0
            {
                let link = self.game.fighter_dynamics[d.0].clone();
                link.compare(
                    &mem,
                    theirs,
                    &format!("{path}.dynamic_bone_sets[{i}]@{k}"),
                    self,
                );
                ours = link.next;
                theirs = self.u32(theirs + 0x90);
                k += 1;
            }
        }
    }

    /// A shadow: the joints it casts from, its camera and texture, and its settings.
    fn shadow(&mut self, path: &str, s: melee_sim::hsd::shadow::ShadowId, t: u32) {
        if !Self::pair(
            &mut self.maps.shadows,
            &mut self.seen,
            &self.stale,
            &mut self.diffs,
            "shadow",
            path,
            Some(s),
            t,
        ) {
            return;
        }
        let sh = self.game.objs.shadows[s.0].clone();
        let mut node = self.u32(t);
        let mut theirs = Vec::new();
        while node != 0 {
            theirs.push(self.u32(node + 4));
            node = self.u32(node);
        }
        self.check(path, "objects", sh.objects.len(), theirs.len());
        for (i, (&o, &tj)) in sh.objects.iter().zip(&theirs).enumerate() {
            self.compare_handle(&format!("{path}.objects[{i}]"), "JObjId", Some(o.0), tj);
        }
        if let Some(c) = sh.camera {
            self.cobj(&format!("{path}.camera"), c, self.u32(t + 4));
        }
        self.compare_handle(
            &format!("{path}.texture"),
            "TObjId",
            sh.texture.map(|x| x.0),
            self.u32(t + 8),
        );
        let f = |c: &Self, o: u32| c.u32(t + o);
        self.check(
            path,
            "scale, translation",
            [sh.scale_s, sh.scale_t, sh.trans_s, sh.trans_t].map(f32::to_bits),
            [f(self, 0xC), f(self, 0x10), f(self, 0x14), f(self, 0x18)],
        );
        self.check(
            path,
            "active, intensity",
            (sh.active, sh.intensity),
            (self.u32(t + 0x1C) != 0, self.u8(t + 0x20)),
        );
        let tex = self.u32(t + 8);
        let desc = if tex != 0 { self.u32(tex + 0x58) } else { 0 };
        if desc != 0 {
            let i = sh.image;
            self.check(
                path,
                "image",
                (i.format, i.width, i.height, i.allocated),
                (
                    self.u32(desc + 8),
                    self.u16(desc + 4),
                    self.u16(desc + 6),
                    self.u32(desc) != 0,
                ),
            );
        }
    }

    /// A joint, its later siblings and its descendants.
    pub fn jobj_tree(&mut self, path: &str, o: Option<JObjId>, t: u32) {
        let mut stack = vec![(o, t, path.to_string())];
        while let Some((o, t, path)) = stack.pop() {
            if !Self::pair(
                &mut self.maps.jobjs,
                &mut self.seen,
                &self.stale,
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
            // A stale w (`JObj::stale_w`) is a stack word the original never set: it isn't
            // compared.
            let w = if j.stale_w {
                f32::from_bits(self.u32(t + 0x28))
            } else {
                q.w
            };
            let srt: Vec<u32> = [
                q.x,
                q.y,
                q.z,
                w,
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
            // A stack descriptor's key is a stack address in memory, which isn't data.
            let id = j.id.filter(|&k| melee_sim::hsd::id::is_data_key(k));
            self.check(&path, "id", id, self.data_ref(self.u32(t + 0x84)));
            let u = self.u32(t + 0x18);
            match j.u {
                Contents::Spline(s) => self.check(&path, "contents", s, self.data_ref(u)),
                Contents::Particles(s) => {
                    self.check(&path, "contents", s, self.data_ref(u));
                    // Which entries are still to spawn: the top bit of each entry's word.
                    let mut p = u;
                    let (mut ours, mut theirs) = (Vec::new(), Vec::new());
                    while p != 0 {
                        let e = self.data_ref(p);
                        ours.push(e.is_some_and(|e| self.game.objs.jobj.pending_ptcl.contains(&e)));
                        theirs.push(self.u32(p + 4) & 0x8000_0000 != 0);
                        p = self.u32(p);
                    }
                    self.check(&path, "particles to spawn", ours, theirs);
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
            &self.stale,
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
            jo.map(|j| {
                self.game.objs.jobjs[j]
                    .id
                    .filter(|&k| melee_sim::hsd::id::is_data_key(k))
            }),
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
                &self.stale,
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
            &self.stale,
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

    pub(crate) fn cobj(&mut self, path: &str, o: melee_sim::hsd::cobj::CObjId, t: u32) {
        if !Self::pair(
            &mut self.maps.cobjs,
            &mut self.seen,
            &self.stale,
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

    pub(crate) fn lobjs(&mut self, path: &str, o: Option<melee_sim::hsd::lobj::LObjId>, t: u32) {
        let (mut o, mut t) = (o, t);
        let mut i = 0;
        while o.is_some() || t != 0 {
            let lp = format!("{path} {i}");
            if !Self::pair(
                &mut self.maps.lobjs,
                &mut self.seen,
                &self.stale,
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
            &self.stale,
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
            melee_sim::hsd::mobj::MObjClass::Item => sym("it_mobj"),
            melee_sim::hsd::mobj::MObjClass::Fighter => sym("ftMObj"),
        };
        // Classes no port models (items' and effects') were reported when imported.
        let theirs = self.u32(t);
        if [
            sym("hsdMObj"),
            sym("grMaterial_803E0A20"),
            sym("it_mobj"),
            sym("ftMObj"),
        ]
        .contains(&theirs)
        {
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
        let ours = crate::tev::as_compiled(&m.tev);
        self.tobj_chain(path, m.tobj, self.u32(t + 8));
        // After the chain, whose textures the stages and constants name, has paired.
        let mem = self.mem;
        let u32 = |x: u32| u32::from_be_bytes(mem[(x - MEM1) as usize..][..4].try_into().unwrap());
        let u8 = |x: u32| mem[(x - MEM1) as usize];
        let r = crate::tev::Mem { u32: &u32, u8: &u8 };
        let tobjs = &self.maps.tobjs;
        let theirs = crate::tev::read(&r, o.unwrap(), t, &mut |a| tobjs.get(&a).copied())
            .map(|d| crate::tev::as_compiled(&d));
        match theirs {
            Ok(theirs) => self.check(path, "compiled TEV", ours, theirs),
            Err(e) => self.diffs.push(format!("{path}: compiled TEV: {e}")),
        }
    }

    /// A chain of TObjs, `to` ours and `tt` theirs.
    pub(crate) fn tobj_chain(
        &mut self,
        path: &str,
        to: Option<melee_sim::hsd::tobj::TObjId>,
        tt: u32,
    ) {
        let (mut to, mut tt) = (to, tt);
        let mut i = 0;
        while to.is_some() || tt != 0 {
            let tp = format!("{path} tobj {i}");
            if !Self::pair(
                &mut self.maps.tobjs,
                &mut self.seen,
                &self.stale,
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
            // The TEV block's settings and colors (`HSD_TObjTev`), which effects recolor.
            let tev = w(self, 0xA8);
            let ours = x.tev.as_ref().map(|v| {
                let c = |c: &melee_sim::hsd::tobj::Color| u32::from_be_bytes([c.r, c.g, c.b, c.a]);
                let mut b = vec![
                    v.color_op,
                    v.alpha_op,
                    v.color_bias,
                    v.alpha_bias,
                    v.color_scale,
                    v.alpha_scale,
                    v.color_clamp,
                    v.alpha_clamp,
                ];
                b.extend(v.color_in);
                b.extend(v.alpha_in);
                (b, [c(&v.konst), c(&v.tev0), c(&v.tev1), v.active])
            });
            let theirs = (tev != 0).then(|| {
                (
                    (0..16).map(|o| self.u8(tev + o)).collect::<Vec<u8>>(),
                    [
                        self.u32(tev + 0x10),
                        self.u32(tev + 0x14),
                        self.u32(tev + 0x18),
                        self.u32(tev + 0x1C),
                    ],
                )
            });
            self.check(&tp, "TEV", ours, theirs);
            self.tobj_image(&tp, x.imagedesc, w(self, 0x58));
            self.aobj(&format!("{tp} aobj"), x.aobj, w(self, 0x64));
            to = x.next;
            tt = w(self, 8);
            i += 1;
        }
    }

    /// A TObj's image descriptor: loaded data, a static one, refraction's, or a game's heap
    /// copy (Pokémon Stadium's screens).
    fn tobj_image(
        &mut self,
        path: &str,
        ours: Option<melee_sim::hsd::tobj::ImageRef>,
        theirs: u32,
    ) {
        use melee_sim::hsd::tobj::ImageRef;
        let same = match ours {
            None => theirs == 0,
            Some(ImageRef::Data(d)) => {
                self.compare_data(&format!("{path} image"), Some(d), theirs);
                return;
            }
            Some(ImageRef::Static(a)) => a == theirs,
            Some(ImageRef::RefractFrame) => theirs == sym("imagedesc0"),
            Some(ImageRef::Refract(n)) => {
                let base = self.u32(sym(crate::display::REFRACT) + 8);
                base != 0 && theirs == base + 0x18 * u32::from(n)
            }
            Some(ImageRef::Screen(s)) => crate::stadium::screen_at(self.mem, theirs) == Some(s),
            Some(ImageRef::Shadow(s)) => {
                // The image of the texture of the shadow it pairs with, once that's paired (the
                // shadow's own comparison pairs its texture with this TObj).
                let addr = self
                    .maps
                    .shadows
                    .iter()
                    .find(|&(_, &v)| v == s)
                    .map(|(&a, _)| a);
                theirs != 0
                    && addr.is_none_or(|a| {
                        let tex = self.u32(a + 8);
                        tex != 0 && self.u32(tex + 0x58) == theirs
                    })
            }
        };
        if !same {
            self.diff(path, "image", format!("{ours:?}"), format!("{theirs:#x}"));
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
                &self.stale,
                &mut self.diffs,
                "dobj",
                &dp,
                o,
                t,
            ) {
                break;
            }
            self.dobj_fields(&dp, o.unwrap(), t);
            o = self.game.objs.dobjs[o.unwrap()].next;
            t = self.u32(t + 4);
            i += 1;
        }
    }

    /// One DObj, not the ones after it: paired, and compared if this is the first meeting.
    fn dobjs_one(&mut self, path: &str, o: Option<melee_sim::hsd::dobj::DObjId>, t: u32) {
        if Self::pair(
            &mut self.maps.dobjs,
            &mut self.seen,
            &self.stale,
            &mut self.diffs,
            "dobj",
            path,
            o,
            t,
        ) {
            self.dobj_fields(path, o.unwrap(), t);
        }
    }

    /// A DObj's fields, material, animation and shapes.
    fn dobj_fields(&mut self, dp: &str, o: melee_sim::hsd::dobj::DObjId, t: u32) {
        {
            let d = &self.game.objs.dobjs[o];
            self.check(dp, "flags", d.flags, self.u32(t + 0x14));
            self.mobj(&format!("{dp} mobj"), d.mobj, self.u32(t + 8));
            self.aobj(&format!("{dp} aobj"), d.aobj, self.u32(t + 0x10));
            let (mut p, mut tp) = (d.pobj, self.u32(t + 0xC));
            let mut k = 0;
            while p.is_some() || tp != 0 {
                let pp = format!("{dp} pobj {k}");
                if !Self::pair(
                    &mut self.maps.pobjs,
                    &mut self.seen,
                    &self.stale,
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
        }
    }
}

impl Comparer<'_> {
    /// For a handle of a kind whose pool carries the allocator model's addresses: the pool,
    /// whether slot `o` is free, and the address the original gave its object (the last one
    /// there, 0 where the model doesn't know it).
    fn slot_of(&self, kind: &str, o: u32) -> Option<(&'static str, bool, u32)> {
        fn slot<T, const K: u8>(p: &Pool<T, K>, o: u32) -> (bool, u32) {
            (p.is_free(o), p.addr(o))
        }
        let g = self.game;
        let pool = pool_kind(kind)?;
        let (free, addr) = match pool {
            "JObjId" => slot(&g.objs.jobjs.pool, o),
            "DObjId" => slot(&g.objs.dobjs.pool, o),
            "MObjId" => slot(&g.objs.mobjs.pool, o),
            "TObjId" => slot(&g.objs.tobjs.pool, o),
            "GeneratorId" => slot(&g.objs.generators.pool, o),
            "GObjId" => slot(&g.world.gobjs, o),
            "FighterId" => slot(&g.fighters, o),
            "ItemId" => slot(&g.items, o),
            "EffectId" => slot(&g.ef.effects, o),
            "QueuedId" => slot(&g.ef.queued, o),
            _ => unreachable!(),
        };
        Some((pool, free, addr))
    }

    /// A fighter's animation buffer, ours `slot` and theirs at `addr`: every byte we know.
    fn anim_buf(&mut self, path: &str, slot: u32, addr: u32) {
        let Some(archives) = self.archives else {
            return;
        };
        if !self.seen.insert(("anim buf", addr)) {
            return;
        }
        let ours = self.game.fighter_anim_bufs[slot].render(archives, addr);
        let at = (addr - MEM1) as usize;
        let theirs = &self.mem[at..at + ours.len()];
        let differ = ours
            .iter()
            .zip(theirs)
            .enumerate()
            .filter(|(_, (o, t))| o.is_some_and(|o| o != **t));
        let (count, first) = differ.fold((0, None), |(n, f), (i, (o, t))| {
            (n + 1, f.or(Some((i, o.unwrap(), *t))))
        });
        if let Some((i, o, t)) = first {
            self.diffs.push(format!(
                "{path} contents: {count} bytes differ, first at {i:#x}: ours {o:#04x}, theirs {t:#04x} (loads {:?})",
                self.game.fighter_anim_bufs[slot].layers()
            ));
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
        if ours.is_none() && self.maps.unfollowed.contains(&(kind, theirs)) {
            return;
        }
        // An unknown word (`Maps::unknown`) neither side has written since compares by the
        // word itself; one the original wrote and ours didn't differs.
        if let Some(word) = ours.and_then(|o| self.maps.unknown.get(&o)) {
            if *word != theirs {
                self.diffs.push(format!(
                    "{path}: {kind} ours unknown until written ({word:#x}), theirs {theirs:#x}"
                ));
            }
            return;
        }
        let dangling = match (kind, ours) {
            ("GObjId", Some(o)) => self.maps.dangling_gobjs.get(&o),
            ("JObjId", Some(o)) => self.maps.dangling_jobjs.get(&o),
            ("FighterId", Some(o)) => self.maps.dangling_fighters.get(&o),
            ("ItemId", Some(o)) => self.maps.dangling_items.get(&o),
            ("CameraBoxId", Some(o)) => self.maps.dangling_boxes.get(&o),
            ("BoneTableId", Some(o)) => self.maps.dangling_bone_tables.get(&o),
            ("QueuedId", Some(o)) => self.maps.dangling_queued.get(&o),
            _ => None,
        };
        if let Some(&addr) = dangling {
            if addr != theirs {
                self.diffs.push(format!(
                    "{path}: {kind} ours dangling {addr:#x}, theirs {theirs:#x}"
                ));
            }
            return;
        }
        // A pointer to an object we have freed dangles, and so must the original's: its
        // pairing ended with the free, since the original's heap may have handed the block to
        // a new object by now. So it pairs nothing, and compares by the address the allocator
        // model gave the object, whatever lives there since (docs/oracle.md, "Dangling
        // pointers"). Our slot may have been reused since, even freed again, at another
        // address: the field then names the slot's earlier object, whose address the original
        // holds.
        let slot = ours.and_then(|o| self.slot_of(kind, o).map(|s| (o, s)));
        let freed = slot.and_then(|(_, (_, free, addr))| free.then_some(addr));
        if let Some((o, (pool, free, addr))) = slot
            && addr != 0
        {
            let earlier = theirs != addr && self.past.is_some_and(|p| p.had(pool, o, theirs));
            if earlier {
                return;
            }
            if free {
                if addr != theirs {
                    self.diffs.push(format!(
                        "{path}: {kind} ours dangling (freed at {addr:#x}), theirs {theirs:#x}"
                    ));
                }
                return;
            }
        }
        let known = match kind {
            "JObjId" => self.maps.jobjs.get(&theirs).map(|j| j.0),
            "GObjId" => self.maps.gobjs.get(&theirs).map(|g| g.0),
            "FighterId" | "VictimFighter" => self.maps.fighters.get(&theirs).map(|f| f.0),
            "PartsId" => self.maps.fighter_parts.get(&theirs).map(|p| p.0),
            "AttrsId" => self.maps.fighter_attrs.get(&theirs).map(|a| a.0),
            "DynamicsId" => self.maps.fighter_dynamics.get(&theirs).map(|d| d.0),
            "ItemId" | "VictimItem" => self.maps.items.get(&theirs).map(|i| i.0),
            "ItemLinkId" => self.maps.item_links.get(&theirs).map(|l| l.0),
            "BoneTableId" => {
                if let (Some(o), true) = (ours, theirs != 0) {
                    self.bone_table(path, BoneTableId(o), theirs);
                }
                self.maps.bone_tables.get(&theirs).map(|t| t.0)
            }
            "ItemLogicRef" | "ItemStatesRef" => (theirs != 0).then_some(theirs),
            "DObjListId" => self.maps.fighter_dobj_lists.get(&theirs).map(|l| l.0),
            "AnimBufId" => {
                if let (Some(o), true) = (ours, theirs != 0) {
                    self.anim_buf(path, o, theirs);
                }
                self.maps.fighter_anim_bufs.get(&theirs).map(|b| b.0)
            }
            "LObjId" => self.maps.lobjs.get(&theirs).map(|l| l.0),
            "ShadowId" => self.maps.shadows.get(&theirs).map(|s| s.0),
            "DObjId" => self.maps.dobjs.get(&theirs).map(|d| d.0),
            "CObjId" => self.maps.cobjs.get(&theirs).map(|c| c.0),
            "GroundId" => self.maps.grounds.get(&theirs).map(|g| g.0),
            "StartHookId" => self.maps.start_hooks.get(&theirs).map(|h| h.0),
            // A stale item's pointer may be into nothing: it pairs with nothing.
            "CameraBoxId" if theirs != 0 => try_box_index(self.u32(sym("cm_804D645C")), theirs),
            "TObjId" => self.maps.tobjs.get(&theirs).map(|t| t.0),
            "MObjId" => self.maps.mobjs.get(&theirs).map(|m| m.0),
            "QueuedId" => self.maps.queued.get(&theirs).map(|q| q.0),
            "GeneratorId" => self.maps.generators.get(&theirs).map(|g| g.0),
            "EffectId" => self.maps.effects.get(&theirs).map(|e| e.0),
            "TextId" if theirs != 0 => {
                Some(theirs.wrapping_sub(self.u32(sym("HSD_SisLib_804D796C"))))
            }
            _ => None,
        };
        let known = known.or_else(|| self.handles.get(&(kind, theirs)).copied());
        // A pointer to a joint freed during the call: ours is out or points to a joint we
        // freed, theirs to the freed block (no class there).
        if kind == "JObjId"
            && theirs != 0
            && (ours.is_none() || freed.is_some())
            && known.is_none()
            && theirs & 3 == 0
            && (MEM1..MEM1 + self.mem.len() as u32 - 4).contains(&theirs)
            && !(0x8030_0000..0x8050_0000).contains(&self.u32(theirs))
        {
            return;
        }
        // A freed object whose address the model doesn't know (one made outside a step):
        // the field still pairs nothing, and differs only where the original's address is
        // paired with another of our objects.
        if let (Some(o), Some(_)) = (ours, freed) {
            if theirs == 0 || known.is_some_and(|k| k != o) {
                self.diffs.push(format!(
                    "{path}: {kind} ours dangling {o}, theirs {theirs:#x} (paired with {known:?})"
                ));
            }
            return;
        }
        match (ours, theirs, known) {
            (None, 0, _) => {}
            (Some(o), t, Some(k)) if t != 0 && k == o => {}
            // An effect a field names before its GObj is met pairs as the GObj's would.
            (Some(o), t, None) if t != 0 && kind == "EffectId" => {
                self.maps.effects.insert(t, EffectId(o));
            }
            (Some(o), t, None) if t != 0 => {
                self.handles.insert((kind, t), o);
            }
            _ => self.diffs.push(format!(
                "{path}: {kind} ours {ours:?}, theirs {theirs:#x} (paired with {known:?})"
            )),
        }
    }

    fn compare_data(&mut self, path: &str, ours: Option<DataRef>, theirs: u32) {
        // Data in an archive the original's scene let go of: the pointer still names where it
        // was.
        if let Some(o) = ours
            && !self.data.loaded.iter().any(|l| l.id == o.archive)
            && let Some(a) = self.past.and_then(|p| p.gone_addr(o))
        {
            if a != theirs {
                self.diffs.push(format!(
                    "{path}: data ours {o:?} (at {a:#x}, its archive gone), theirs {theirs:#x}"
                ));
            }
            return;
        }
        let t = self.data_ref(theirs);
        // Files can hold the same bytes (Luigi's animation file repeats some of Mario's), and
        // the scan names the trace's copy as either: the same offset into the same bytes is
        // the same data.
        let same_bytes = match (ours, t, self.archives) {
            (Some(o), Some(t), Some(a)) => {
                o.offset == t.offset && a.get(o.archive).data() == a.get(t.archive).data()
            }
            _ => false,
        };
        if ours != t && !same_bytes && !(ours.is_none() && theirs != 0 && t.is_none()) {
            self.diffs
                .push(format!("{path}: data ours {ours:?}, theirs {theirs:#x}"));
        }
    }

    fn mismatch(&mut self, path: &str, ours: String, theirs: String) {
        self.diffs
            .push(format!("{path}: ours {ours}, theirs {theirs}"));
    }

    fn stale_read(&mut self, path: &str) {
        self.stale_reads.push(path.to_owned());
    }
}
