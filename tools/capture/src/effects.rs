//! The effects and the particle system in a captured state: efLib's effects (the user data
//! of effect GObjs) and globals, deferred spawns, and the particle system's banks, particles,
//! generators and app-SRTs, imported into melee-sim and compared back.

use std::collections::{HashMap, HashSet};

use melee_data::archive::DataRef;
use melee_sim::game::ef::{self, Effect, EffectId, ParamEntry, Queued, QueuedId, Update};
use melee_sim::hsd::appsrt::{AppSrt, AppSrtId};
use melee_sim::hsd::generator::{Generator, GeneratorId};
use melee_sim::hsd::gobj::GObjId;
use melee_sim::hsd::jobj::JObjId;
use melee_sim::hsd::list;
use melee_sim::hsd::particle::{
    self, Bank, Callback, GeneratorHook, Particle, ParticleId, Rgba, UserData, UserFunc,
};
use melee_sim::mtx::Mtx34;
use melee_sim::quat::Quat;
use melee_sim::vec::Vec3;

use crate::compare::Comparer;
use crate::import::{Importer, sym};

/// The effect files' table (`efAsync_DatEntries`), 0xC bytes an entry.
const DAT_ENTRY: u32 = 0xC;

impl Importer<'_> {
    /// efLib's globals (the effects themselves come with their GObjs), the effect files the
    /// capture has loaded, and the particle system.
    pub fn effects(&mut self) {
        let entries = sym("efAsync_DatEntries");
        for (i, f) in ef::FILES.iter().enumerate() {
            let data = self.u32_at(entries + DAT_ENTRY * i as u32 + 8);
            self.game.ef.tables[i] = self.data_at(data);
            if let Some((name, _)) = f {
                self.game.ef.files[i] = self
                    .data
                    .loaded
                    .iter()
                    .find(|l| l.name == *name)
                    .map(|l| l.id);
            }
        }
        self.game.ef.count = self.u32_at(sym("efLib_EffectCount"));
        self.game.ef.load_kind = self.u32_at(sym("efLib_LoadKind"));
        let n = self.u32_at(sym("efLib_AnimCount")) as i32;
        self.game.ef.anim_count = n;
        let queue = sym("efLib_AnimQueue");
        for i in 0..n.clamp(0, 32) as u32 {
            let j = self.u32_at(queue + 4 * i);
            self.game.ef.anim_queue[i as usize] = (j != 0).then(|| self.jobj(j));
        }
        let params = sym("efLib_ParamTable");
        for i in 0..8 {
            let at = params + 8 * i as u32;
            let g = self.u32_at(at);
            self.game.ef.params[i] = ParamEntry {
                gobj: self.ef_gobj_handle(g),
                gfx_id: self.h16(at + 4),
                alpha: self.h16(at + 6),
            };
        }
        // The joints' particle callbacks are efLib's once it has set them.
        self.game.objs.jobj.dptcl = self.u32_at(sym("dptcl_callback")) != 0;
        self.game.objs.jobj.sptcl = self.u32_at(sym("sptcl_callback")) != 0;
        self.particles();
        // Every block on the two allocators' free lists gets a freed slot (those no pointer
        // reaches as well), freed in reverse so the pools' free lists run as the allocators' do
        // and new effects reuse them in the same order; each block's first word (`next`) is the
        // link to the block after it, as `HSD_ObjFree` leaves it.
        self.ef_freed.clear();
        let effects = self.free_chain(sym("efLib_AllocData") + 4);
        let queued = self.free_chain(sym("efAsync_AllocData") + 4);
        // Slots for all of them first, then the frees (a slot freed at once would be the next
        // one handed out).
        for &a in &effects {
            if !self.maps.effects.contains_key(&a) {
                let e = EffectId(self.game.ef.effects.alloc(Effect::default()));
                self.maps.effects.insert(a, e);
            }
        }
        for &a in &queued {
            if !self.maps.queued.contains_key(&a) {
                let q = QueuedId(self.game.ef.queued.alloc(Queued::default()));
                self.maps.queued.insert(a, q);
            }
        }
        for a in effects.iter().rev() {
            let e = self.maps.effects[a];
            let link = self.game.ef.effects.free_head().map(EffectId);
            self.game.ef.effects[e.0].next = link;
            self.game.ef.effects.free(e.0);
        }
        for a in queued.iter().rev() {
            let q = self.maps.queued[a];
            let link = self.game.ef.queued.free_head().map(QueuedId);
            self.game.ef.queued[q.0].next = link;
            self.game.ef.queued.free(q.0);
        }
    }

    /// The blocks on efLib's and efAsync's allocators' free lists.
    /// Whether a block is on efLib's free list.
    pub(crate) fn on_effect_free_list(&mut self, a: u32) -> bool {
        self.ef_free_sets().0.contains(&a)
    }

    fn ef_free_sets(&mut self) -> &(HashSet<u32>, HashSet<u32>) {
        if self.ef_free.is_none() {
            let chain = |s: &Self, data: &str| {
                let mut out = HashSet::new();
                let mut b = s.u32_at(sym(data) + 4);
                while b != 0 && out.insert(b) {
                    b = s.u32_at(b);
                }
                out
            };
            self.ef_free = Some((
                chain(self, "efLib_AllocData"),
                chain(self, "efAsync_AllocData"),
            ));
        }
        self.ef_free.as_ref().unwrap()
    }

    fn u32_at(&self, a: u32) -> u32 {
        self.cap.u32(a)
    }

    fn h16(&self, a: u32) -> u16 {
        let i = (a - crate::MEM1) as usize;
        u16::from_be_bytes(self.cap.call.mem1[i..i + 2].try_into().unwrap())
    }

    fn h8(&self, a: u32) -> u8 {
        self.cap.call.mem1[(a - crate::MEM1) as usize]
    }

    fn hf32(&self, a: u32) -> f32 {
        f32::from_bits(self.cap.u32(a))
    }

    fn vec_at(&self, a: u32) -> Vec3 {
        Vec3::new(self.hf32(a), self.hf32(a + 4), self.hf32(a + 8))
    }

    fn data_at(&mut self, a: u32) -> Option<DataRef> {
        if a == 0 {
            return None;
        }
        let d = self.data.get(a);
        if d.is_none() {
            self.unhandled.push(format!(
                "effect data pointer {a:#x} outside loaded archives"
            ));
        }
        d
    }

    fn ef_gobj_handle(&mut self, a: u32) -> Option<GObjId> {
        use melee_sim::game::mirror::Cx;
        self.import_handle("GObjId", a).map(GObjId)
    }

    /// An effect (`EF_Effect`), and the effects its `next` chain reaches. A `next` that names
    /// no live effect (freed since) gets a slot of its own, so it compares by identity.
    pub fn effect(&mut self, a: u32) -> EffectId {
        use melee_sim::game::mirror::Cx;
        if let Some(&e) = self.maps.effects.get(&a) {
            return e;
        }
        let id = EffectId(self.game.ef.effects.alloc(Effect::default()));
        self.maps.effects.insert(a, id);
        // A freed effect a pointer outlived (a spawn's `next`): a slot of its own, freed again
        // after the import, so it pairs by identity and nothing follows it.
        if self.ef_free_sets().0.contains(&a) {
            self.ef_freed.push((true, a));
            return id;
        }
        let next = self.u32_at(a);
        let next = (next != 0).then(|| self.effect(next));
        let update = self.u32_at(a + 0x10);
        let update = (update != 0).then(|| {
            Update::from_addr(update).unwrap_or_else(|| panic!("effect update {update:#x}"))
        });
        let user_data = self.u32_at(a + 0x14);
        let user_data = if update == Some(Update::SetRotYAndTransition) {
            self.import_handle("FighterId", user_data)
                .map(melee_sim::game::handles::FighterId)
        } else {
            None
        };
        let gobj = self.u32_at(a + 4);
        let parent = self.u32_at(a + 8);
        let attach = self.u32_at(a + 0xC);
        let e = Effect {
            next,
            gobj: self.ef_gobj_handle(gobj),
            parent: self.ef_gobj_handle(parent),
            attach_jobj: (attach != 0).then(|| self.jobj(attach)),
            update,
            user_data,
            params: self.vec_at(a + 0x18),
            lifetime: self.h16(a + 0x24),
            expire_flags: self.h8(a + 0x26) as i8,
            scale_flags: self.h8(a + 0x27),
            state_flags: self.h8(a + 0x28),
            is_async: self.h8(a + 0x29),
            left: Default::default(),
        };
        self.game.ef.effects[id.0] = e;
        id
    }

    /// A deferred spawn (`EF_QueuedEffect`) and the rest of its queue.
    pub fn queued(&mut self, a: u32) -> QueuedId {
        if let Some(&q) = self.maps.queued.get(&a) {
            return q;
        }
        let id = QueuedId(self.game.ef.queued.alloc(Queued::default()));
        self.maps.queued.insert(a, id);
        // Freed already (a queue mid-flush still names it): likewise.
        if self.ef_free_sets().1.contains(&a) {
            self.ef_freed.push((false, a));
            return id;
        }
        let next = self.u32_at(a);
        let next = (next != 0).then(|| self.queued(next));
        let j = self.u32_at(a + 0xC);
        let q = Queued {
            next,
            kind: self.h8(a + 4),
            gfx_id: self.u32_at(a + 8) as i32,
            jobj: (j != 0).then(|| self.jobj(j)),
            params: self.vec_at(a + 0x10),
            extra1: self.hf32(a + 0x1C),
            extra2: self.hf32(a + 0x20),
        };
        self.game.ef.queued[id.0] = q;
        id
    }

    /// The particle system: its banks, every particle and generator with the app-SRTs they
    /// hold, the point joints, the counts and the display pass's sorting state.
    pub fn particles(&mut self) {
        for b in 0..particle::BANKS as u32 {
            self.game.objs.particles.banks[b as usize] = self.bank(b);
        }
        let (live, peak) = (
            self.h16(sym("hsd_804D78E2")),
            self.h16(sym("numPeakParticles")),
        );
        let ps = &mut self.game.objs.particles;
        ps.live = live;
        ps.peak = peak;
        let order = self.generator_list();
        let freed_gens = self.free_chain(sym("hsd_804D0F90") + 4);
        let freed_srts = self.free_chain(sym("HSD_PSAppSrt_804D10B0") + 4);
        for i in 0..particle::LINKS as u32 {
            let mut p = self.u32_at(sym("hsd_804D0908") + 4 * i);
            let mut prev: Option<ParticleId> = None;
            while p != 0 {
                let id = self.particle(p);
                match prev {
                    None => self.game.objs.particles.links[i as usize] = Some(id),
                    Some(pr) => self.game.objs.particles[pr].next = Some(id),
                }
                prev = Some(id);
                p = self.u32_at(p);
            }
        }
        for &g in &order {
            let id = self.maps.generators[&g];
            let o = self.generator(g);
            self.game.objs.generators[id] = o;
        }
        let gh = |s: &Self, a: u32| (a != 0).then(|| s.maps.generators[&a]);
        let head = order.first().map(|a| self.maps.generators[a]);
        let (live, peak) = (self.h16(sym("hsd_804D78E0")), self.h16(sym("hsd_804D78DA")));
        let idnum = self.h16(sym("lbl_804D6368"));
        let gs = &mut self.game.objs.generators;
        gs.head = head;
        gs.live = live;
        gs.peak = peak;
        gs.idnum = idnum;
        let cursor = self.u32_at(sym("hsd_804D78F8"));
        self.game.objs.generators.cursor = self.dangling_generator(cursor);
        let hook = self.u32_at(sym("hsd_804D7900"));
        self.game.objs.generators.on_create = (hook != 0).then_some(GeneratorHook(hook));
        let mut n = self.u32_at(sym("hsd_804D78F4"));
        let mut pending = None;
        while n != 0 {
            let g = gh(self, self.u32_at(n + 4)).expect("a pending generator");
            let gs = &mut self.game.objs.generators;
            pending = Some(list::alloc_and_append(&mut gs.nodes, pending, g));
            n = self.u32_at(n);
        }
        self.game.objs.generators.pending = pending;
        let (live, peak) = (self.h16(sym("hsd_804D78DE")), self.h16(sym("hsd_804D78D8")));
        let srts = &mut self.game.objs.app_srts;
        srts.live = live;
        srts.peak = peak;
        for slot in 0..particle::POINT_JOBJS as u32 {
            let j = self.u32_at(sym("hsd_804D08E8") + 4 * slot);
            self.game.objs.particles.point_jobjs[slot as usize] = (j != 0).then(|| self.jobj(j));
        }
        // A freed generator keeps what `HSD_ObjFree` didn't overwrite (its first word becomes
        // the free list's link); of that, the original reads only its app-SRT.
        for &a in &freed_gens {
            let srt = self.u32_at(a + 0x54);
            let srt = self.app_srt(srt);
            let id = self.maps.generators[&a];
            self.game.objs.generators[id].appsrt = srt;
        }
        for a in freed_srts.iter().rev() {
            let id = self.maps.app_srts[a];
            self.game.objs.app_srts.pool.free(id.0);
        }
        for a in freed_gens.iter().rev() {
            let id = self.maps.generators[a];
            self.game.objs.generators.pool.free(id.0);
        }
        self.game.objs.psdisp.frame = self.h8(sym("psFrameNum"));
        for i in 0..particle::LINKS as u32 {
            self.game.objs.psdisp.sorted[i as usize] = self.h8(sym("HSD_PSDisp_8040C360") + i);
            let p = self.u32_at(sym("particle_list") + 4 * i);
            self.game.objs.psdisp.non_edge[i as usize] = (p != 0)
                .then(|| self.maps.particles.get(&p).copied())
                .flatten();
        }
    }

    /// The particle system's generator list, in order.
    fn generator_list(&self) -> Vec<u32> {
        let mut order = Vec::new();
        let mut g = self.u32_at(sym("hsd_804D78FC"));
        while g != 0 {
            order.push(g);
            g = self.u32_at(g);
        }
        order
    }

    /// Slots for the particle system's generators, live and freed, and its freed app-SRTs, so
    /// pointers to them resolve wherever they're met (a stage object's variables come with its
    /// GObj, before [`Importer::particles`]). Freed blocks (the allocators' free lists, head
    /// first) are what pointers outlived (Final Destination's swaying piece keeps its generator
    /// after the generator's life ends, and reads its app-SRT): each gets a slot that
    /// [`Importer::particles`] frees again, the list's head last, so our pools reuse them in the
    /// original's order.
    pub(crate) fn map_generators(&mut self) {
        for g in self.generator_list() {
            let id = GeneratorId(self.game.objs.generators.pool.alloc(Generator::default()));
            self.maps.generators.insert(g, id);
        }
        for a in self.free_chain(sym("HSD_PSAppSrt_804D10B0") + 4) {
            let id = AppSrtId(self.game.objs.app_srts.pool.alloc(AppSrt::default()));
            self.maps.app_srts.insert(a, id);
        }
        for a in self.free_chain(sym("hsd_804D0F90") + 4) {
            let id = GeneratorId(self.game.objs.generators.pool.alloc(Generator::default()));
            self.maps.generators.insert(a, id);
        }
    }

    /// The blocks on an `HSD_ObjAllocData`'s free list, from its head (`freehead`, at `at`).
    fn free_chain(&self, at: u32) -> Vec<u32> {
        let mut out = Vec::new();
        let mut b = self.u32_at(at);
        while b != 0 && !out.contains(&b) {
            out.push(b);
            b = self.u32_at(b);
        }
        out
    }

    /// A generator a pointer names, if it is in the list (the cursor can be one just freed).
    fn dangling_generator(&mut self, a: u32) -> Option<GeneratorId> {
        if a == 0 {
            return None;
        }
        let g = self.maps.generators.get(&a).copied();
        if g.is_none() {
            self.unhandled
                .push(format!("generator {a:#x} outside the generator list"));
        }
        g
    }

    /// Effect bank `b` as the particle system has it registered.
    fn bank(&mut self, b: u32) -> Option<Bank> {
        let tex = self.u32_at(sym("psTexGroupArray") + 4 * b);
        if tex == 0 {
            return None;
        }
        let ptclref = self.u32_at(sym("ptclref_804D0E5C") + 4 * b);
        let count = self.u32_at(sym("psCmdListArray") + 4 * b) as i32;
        let remap = self.u32_at(sym("hsd_804D0948") + 4 * b);
        let form = self.u32_at(sym("psNumCmdList") + 4 * b);
        let tex_ref = self.data_at(tex - 4)?;
        // The command bank: version 0x40 to 0x43 starts the table three words in, less the
        // bank's first index; version 0 two words in.
        let l = self
            .data
            .loaded
            .iter()
            .find(|l| l.id == tex_ref.archive)
            .expect("the texture bank's archive")
            .clone();
        let mut found = None;
        let mut c = l.data;
        while c + 12 <= l.data + l.data_len {
            let v = self.h16(c);
            if (0x40..=0x43).contains(&v) {
                let first = self.u32_at(c + 4);
                if c.wrapping_add(12).wrapping_sub(first.wrapping_mul(4)) == ptclref {
                    found = Some((c, first as i32));
                    break;
                }
            }
            c += 4;
        }
        let (cmd, table, first) = match found {
            Some((c, first)) => (c, c + 12, first),
            None => {
                let c = ptclref - 8;
                assert_eq!(self.h16(c), 0, "bank {b}: no command bank found");
                (c, ptclref, 0)
            }
        };
        Some(Bank {
            cmd: self.data_at(cmd)?,
            table: self.data_at(table)?,
            first,
            count,
            tex: tex_ref,
            remap: self.data_at(remap),
            form: self.data_at(form),
        })
    }

    fn rgba(&self, a: u32) -> Rgba {
        Rgba {
            r: self.h8(a),
            g: self.h8(a + 1),
            b: self.h8(a + 2),
            a: self.h8(a + 3),
        }
    }

    fn app_srt(&mut self, a: u32) -> Option<AppSrtId> {
        if a == 0 {
            return None;
        }
        if let Some(&s) = self.maps.app_srts.get(&a) {
            return Some(s);
        }
        let id = AppSrtId(self.game.objs.app_srts.pool.alloc(AppSrt::default()));
        self.maps.app_srts.insert(a, id);
        let gp = self.u32_at(a + 4);
        let gp = self.dangling_generator(gp);
        let mtx = |s: &Self, at: u32| {
            let mut m = Mtx34::default();
            for (i, v) in m.m.iter_mut().flatten().enumerate() {
                *v = s.hf32(at + 4 * i as u32);
            }
            m
        };
        let s = AppSrt {
            gp,
            translate: self.vec_at(a + 8),
            rot: Quat {
                x: self.hf32(a + 0x14),
                y: self.hf32(a + 0x18),
                z: self.hf32(a + 0x1C),
                w: self.hf32(a + 0x20),
            },
            scale: self.vec_at(a + 0x24),
            status: self.h8(a + 0x30),
            frame_num: self.h8(a + 0x31),
            used_count: self.h16(a + 0x32),
            mmtx: mtx(self, a + 0x34),
            view: mtx(self, a + 0x64),
            view_scale: [self.hf32(a + 0x94), self.hf32(a + 0x98)],
            idnum: self.h16(a + 0xA0),
            xa2: self.h8(a + 0xA2),
        };
        self.game.objs.app_srts[id] = s;
        Some(id)
    }

    fn particle(&mut self, a: u32) -> ParticleId {
        let id = ParticleId(self.game.objs.particles.pool.alloc(Particle::default()));
        self.maps.particles.insert(a, id);
        let u16_ = |s: &Self, o| s.h16(a + o);
        let u8_ = |s: &Self, o| s.h8(a + o);
        let f = |s: &Self, o| s.hf32(a + o);
        assert_eq!(
            self.u32_at(a + 0x90),
            0,
            "particle user data isn't the game's"
        );
        assert_eq!(
            self.u32_at(a + 0x94),
            0,
            "particle callbacks aren't the game's"
        );
        let gen_ = self.u32_at(a + 0x88);
        let srt = self.u32_at(a + 0x8C);
        let list = self.u32_at(a + 0x20);
        let p = Particle {
            next: None,
            kind: self.u32_at(a + 4),
            bank: u8_(self, 8),
            tex_group: u8_(self, 9),
            pose_num: u8_(self, 0xA),
            pal_num: u8_(self, 0xB),
            size_count: u16_(self, 0xC),
            prim_col_count: u16_(self, 0xE),
            env_col_count: u16_(self, 0x10),
            prim_col: self.rgba(a + 0x12),
            env_col: self.rgba(a + 0x16),
            cmd_wait: u16_(self, 0x1A),
            loop_count: u8_(self, 0x1C),
            link_no: u8_(self, 0x1D),
            idnum: u16_(self, 0x1E),
            cmd_list: self.data_at(list),
            cmd_ptr: u16_(self, 0x24),
            cmd_mark_ptr: u16_(self, 0x26),
            cmd_loop_ptr: u16_(self, 0x28),
            life: u16_(self, 0x2A),
            vel: self.vec_at(a + 0x2C),
            grav: f(self, 0x38),
            fric: f(self, 0x3C),
            pos: self.vec_at(a + 0x40),
            size: f(self, 0x4C),
            rotate: f(self, 0x50),
            a_cmp_count: u16_(self, 0x54),
            a_cmp_mode: u8_(self, 0x56),
            a_cmp_param1: u8_(self, 0x57),
            a_cmp_param2: u8_(self, 0x58),
            p_jobj_ofs: u8_(self, 0x59),
            mat_col_count: u16_(self, 0x5A),
            amb_col_count: u16_(self, 0x5C),
            rotate_count: u16_(self, 0x5E),
            size_target: f(self, 0x60),
            rotate_target: f(self, 0x64),
            prim_col_remain: u16_(self, 0x68),
            env_col_remain: u16_(self, 0x6A),
            prim_col_target: self.rgba(a + 0x6C),
            env_col_target: self.rgba(a + 0x70),
            mat_col_remain: u16_(self, 0x74),
            amb_col_remain: u16_(self, 0x76),
            a_cmp_remain: u16_(self, 0x78),
            a_cmp_param1_target: u8_(self, 0x7A),
            a_cmp_param2_target: u8_(self, 0x7B),
            mat_rgb: u8_(self, 0x7C),
            mat_a: u8_(self, 0x7D),
            amb_rgb: u8_(self, 0x7E),
            amb_a: u8_(self, 0x7F),
            mat_rgb_target: u8_(self, 0x80),
            mat_a_target: u8_(self, 0x81),
            amb_rgb_target: u8_(self, 0x82),
            amb_a_target: u8_(self, 0x83),
            trail: f(self, 0x84),
            generator: self.dangling_generator(gen_),
            appsrt: self.app_srt(srt),
            userdata: None::<UserData>,
            callback: None::<Callback>,
        };
        self.game.objs.particles[id] = p;
        id
    }

    fn generator(&mut self, a: u32) -> Generator {
        let u16_ = |s: &Self, o| s.h16(a + o);
        let u8_ = |s: &Self, o| s.h8(a + o);
        let f = |s: &Self, o| s.hf32(a + o);
        let next = self.u32_at(a);
        let j = self.u32_at(a + 0x10);
        let list = self.u32_at(a + 0x20);
        let srt = self.u32_at(a + 0x54);
        let uf = self.u32_at(a + 0x58);
        Generator {
            next: (next != 0).then(|| self.maps.generators[&next]),
            kind: self.u32_at(a + 4),
            random: f(self, 8),
            count: f(self, 0xC),
            jobj: (j != 0).then(|| self.jobj(j)),
            gen_life: u16_(self, 0x14),
            ty: u16_(self, 0x16),
            bank: u8_(self, 0x18),
            link_no: u8_(self, 0x19),
            tex_group: u8_(self, 0x1A),
            idnum: u16_(self, 0x1C),
            life: u16_(self, 0x1E),
            cmd_list: self.data_at(list),
            pos: self.vec_at(a + 0x24),
            vel: self.vec_at(a + 0x30),
            grav: f(self, 0x3C),
            fric: f(self, 0x40),
            size: f(self, 0x44),
            radius: f(self, 0x48),
            angle: f(self, 0x4C),
            num_child: self.u32_at(a + 0x50),
            appsrt: self.app_srt(srt),
            userfunc: (uf != 0).then_some(UserFunc(uf)),
            aux: std::array::from_fn(|i| f(self, 0x60 + 4 * i as u32)),
            rect_flag: u16_(self, 0x90),
        }
    }
}

impl Comparer<'_> {
    fn ef_u16(&self, a: u32) -> u16 {
        self.u16(a)
    }

    fn ef_f32(&self, a: u32) -> u32 {
        self.u32(a)
    }

    fn handle_gobj(&mut self, path: &str, ours: Option<GObjId>, theirs: u32) {
        use melee_sim::game::mirror::Cx;
        self.compare_handle(path, "GObjId", ours.map(|g| g.0), theirs);
    }

    fn handle_jobj(&mut self, path: &str, ours: Option<JObjId>, theirs: u32) {
        use melee_sim::game::mirror::Cx;
        self.compare_handle(path, "JObjId", ours.map(|j| j.0), theirs);
    }

    /// An effect: its fields, the parts of `params` its update reads, and the effects its
    /// `next` chain reaches.
    pub fn effect(&mut self, path: &str, o: EffectId, t: u32) {
        if !Self::pair(
            &mut self.maps.effects,
            &mut self.seen,
            &self.stale,
            &mut self.diffs,
            "effect",
            path,
            Some(o),
            t,
        ) {
            return;
        }
        // A freed effect (a spawn's `next` can name one; no code reads `next`) keeps what its
        // block held, the free list's link aside, and nothing reads it: it pairs by identity
        // only, as the freed slot it is.
        if self.game.ef.effects.is_free(o.0) {
            return;
        }
        let e = self.game.ef.effects[o.0].clone();
        let next = self.u32(t);
        match (e.next, next) {
            (None, 0) => {}
            (Some(n), tn) if tn != 0 => self.effect(&format!("{path} next"), n, tn),
            (n, tn) => self.diff(path, "next", n, format!("{tn:#x}")),
        }
        self.handle_gobj(&format!("{path} gobj"), e.gobj, self.u32(t + 4));
        self.handle_gobj(&format!("{path} parent"), e.parent, self.u32(t + 8));
        self.handle_jobj(
            &format!("{path} attach_jobj"),
            e.attach_jobj,
            self.u32(t + 0xC),
        );
        self.check(
            path,
            "update",
            e.update.map_or(0, |u| u.addr()),
            self.u32(t + 0x10),
        );
        if e.update == Some(Update::SetRotYAndTransition) {
            use melee_sim::game::mirror::Cx;
            let ud = self.u32(t + 0x14);
            self.compare_handle(
                &format!("{path} user_data"),
                "FighterId",
                e.user_data.map(|f| f.0),
                ud,
            );
        }
        // `user_data`'s and the bytes' words the effect knows its block held.
        if let ef::Held::Word(w) = e.left.user_data {
            self.check(path, "user_data's word", w, self.u32(t + 0x14));
        }
        for (i, b) in e.left.x2a.iter().enumerate() {
            if let Some(b) = *b {
                let off = 0x2A + i as u32;
                self.check(path, &format!("x{off:X}"), b, self.u8(t + off));
            }
        }
        let reads = e.update.map_or([false; 3], |u| u.reads_params());
        let ours = [e.params.x, e.params.y, e.params.z];
        for (i, &r) in reads.iter().enumerate() {
            if r || e.param_known(i) {
                let th = self.ef_f32(t + 0x18 + 4 * i as u32);
                self.check(path, &format!("params[{i}]"), ours[i].to_bits(), th);
            }
        }
        self.check(
            path,
            "lifetime and flags",
            (
                e.lifetime,
                e.expire_flags as u8,
                e.scale_flags,
                e.state_flags,
                e.is_async,
            ),
            (
                self.ef_u16(t + 0x24),
                self.u8(t + 0x26),
                self.u8(t + 0x27),
                self.u8(t + 0x28),
                self.u8(t + 0x29),
            ),
        );
    }

    /// A deferred spawn's queue: each entry's kind, id and joint, and the words its kind
    /// reads.
    pub fn queue(&mut self, path: &str, ours: Option<QueuedId>, theirs: u32) {
        let (mut o, mut t, mut i) = (ours, theirs, 0);
        loop {
            let p = format!("{path} #{i}");
            match (o, t) {
                (None, 0) => break,
                (Some(q), tq) if tq != 0 => {
                    let e = self.game.ef.queued[q.0].clone();
                    self.check(
                        &p,
                        "kind and id",
                        (e.kind, e.gfx_id),
                        (self.u8(tq + 4), self.u32(tq + 8) as i32),
                    );
                    self.handle_jobj(&format!("{p} jobj"), e.jobj, self.u32(tq + 0xC));
                    use ef::SpawnKind::*;
                    let words: &[(u32, u32)] = match ef::SpawnKind::from_u8(e.kind) {
                        Some(PosOffset | AttachOffset | CameraShake) => &[
                            (0x10, e.params.x.to_bits()),
                            (0x14, e.params.y.to_bits()),
                            (0x18, e.params.z.to_bits()),
                        ],
                        Some(AttachParam | PosParam) => &[(0x10, e.params.x.to_bits())],
                        Some(PosOffsetParam) => &[
                            (0x10, e.params.x.to_bits()),
                            (0x14, e.params.y.to_bits()),
                            (0x18, e.params.z.to_bits()),
                            (0x1C, e.extra1.to_bits()),
                        ],
                        Some(PosOffsetParam2) => &[
                            (0x10, e.params.x.to_bits()),
                            (0x14, e.params.y.to_bits()),
                            (0x18, e.params.z.to_bits()),
                            (0x1C, e.extra1.to_bits()),
                            (0x20, e.extra2.to_bits()),
                        ],
                        _ => &[],
                    };
                    let words = words.to_vec();
                    for (off, ours) in words {
                        self.check(&p, &format!("word {off:#x}"), ours, self.u32(tq + off));
                    }
                    o = e.next;
                    t = self.u32(tq);
                }
                (o2, t2) => {
                    self.diff(&p, "queued spawn", o2, format!("{t2:#x}"));
                    break;
                }
            }
            i += 1;
        }
    }

    /// efLib's globals, the fighters' deferred spawns and the particle system.
    pub fn effects(&mut self) {
        let game = self.game;
        let entries = sym("efAsync_DatEntries");
        for i in 0..ef::FILES.len() {
            let t = self.u32(entries + DAT_ENTRY * i as u32 + 8);
            // A table in an effects file its scene let go of: the pointer still names where it
            // was (`compare_data`'s rule).
            use melee_sim::game::mirror::Cx;
            self.compare_data(&format!("ef table {i}"), game.ef.tables[i], t);
        }
        self.check(
            "ef",
            "count, load kind and pool",
            (game.ef.count, game.ef.load_kind, game.ef.effects.used()),
            (
                self.u32(sym("efLib_EffectCount")),
                self.u32(sym("efLib_LoadKind")),
                self.u32(sym("efLib_AllocData") + 8),
            ),
        );
        let n = self.u32(sym("efLib_AnimCount")) as i32;
        self.check("ef", "anim count", game.ef.anim_count, n);
        for i in 0..n.clamp(0, 32) as usize {
            let t = self.u32(sym("efLib_AnimQueue") + 4 * i as u32);
            self.handle_jobj(&format!("ef anim queue {i}"), game.ef.anim_queue[i], t);
        }
        for i in 0..8 {
            let at = sym("efLib_ParamTable") + 8 * i as u32;
            let p = game.ef.params[i];
            self.handle_gobj(&format!("ef param {i}"), p.gobj, self.u32(at));
            self.check(
                "ef",
                &format!("param {i}"),
                (p.gfx_id, p.alpha),
                (self.u16(at + 4), self.u16(at + 6)),
            );
        }
        self.check(
            "ef",
            "queued pool",
            game.ef.queued.used(),
            self.u32(sym("efAsync_AllocData") + 8),
        );
        let o = &game.objs.jobj;
        self.check(
            "ef",
            "joints' particle callbacks (efLib's)",
            (
                if o.dptcl { sym("efLib_Cb_DPtcl") } else { 0 },
                if o.sptcl { sym("efLib_Cb_SPtcl") } else { 0 },
            ),
            (
                self.u32(sym("dptcl_callback")),
                self.u32(sym("sptcl_callback")),
            ),
        );
        // The fighters' deferred spawns and the particle system live in the scenes that set
        // them up (`efLib_Init`: a match's, the results screen's) and before any: past such a
        // scene they name its freed heap, which a later scene writes over (the opening movie's
        // frames), and nothing reads them until the next `efLib_Init` makes them anew.
        if !self.match_statics_live() {
            return;
        }
        let fighters: Vec<(u32, melee_sim::game::handles::FighterId)> =
            self.maps.fighters.iter().map(|(&a, &f)| (a, f)).collect();
        for (a, f) in fighters {
            let ours = game.fighters[f.0].x60C;
            self.queue(&format!("fighter {a:#x} x60C"), ours, self.u32(a + 0x60C));
        }
        self.particles();
    }

    /// The particle system, as [`Importer::particles`] reads it: generators in list order,
    /// particles link by link, app-SRTs as first met, then the counts, cursor, pending list,
    /// point joints, banks and sorting state.
    pub fn particles(&mut self) {
        let game = self.game;
        let objs = &game.objs;
        // Generators.
        let mut t = self.u32(sym("hsd_804D78FC"));
        let mut o = objs.generators.head;
        let mut gen_t: HashMap<u32, u32> = HashMap::new();
        let mut gen_o: HashMap<GeneratorId, u32> = HashMap::new();
        let mut gens = Vec::new();
        let mut i = 0;
        loop {
            match (o, t) {
                (None, 0) => break,
                (Some(g), tg) if tg != 0 => {
                    gen_t.insert(tg, i);
                    gen_o.insert(g, i);
                    self.maps.generators.insert(tg, g);
                    gens.push((g, tg));
                    o = objs.generators[g].next;
                    t = self.u32(tg);
                }
                (og, tg) => {
                    self.diff(
                        "particles",
                        &format!("generator {i}"),
                        og,
                        format!("{tg:#x}"),
                    );
                    break;
                }
            }
            i += 1;
        }
        let gt = |a: u32| {
            if a == 0 {
                u32::MAX
            } else {
                gen_t.get(&a).copied().unwrap_or(u32::MAX - 1)
            }
        };
        let go = |g: Option<GeneratorId>| {
            g.map_or(u32::MAX, |g| gen_o.get(&g).copied().unwrap_or(u32::MAX - 1))
        };
        let mut srt_t: HashMap<u32, usize> = HashMap::new();
        let mut srt_o: HashMap<AppSrtId, usize> = HashMap::new();
        let mut srts: Vec<(AppSrtId, u32)> = Vec::new();
        let mut srt_label = |c: &mut Self, path: &str, o: Option<AppSrtId>, t: u32| -> usize {
            match (o, t) {
                (None, 0) => usize::MAX,
                (Some(o), t) if t != 0 => {
                    let next = srts.len();
                    let lt = *srt_t.entry(t).or_insert(next);
                    let lo = *srt_o.entry(o).or_insert(next);
                    if lt != lo {
                        c.diff(path, "app-SRT sharing", lo, lt);
                    }
                    if lt == next && lo == next {
                        srts.push((o, t));
                    }
                    lt
                }
                (o, t) => {
                    c.diff(path, "app-SRT", o, format!("{t:#x}"));
                    usize::MAX
                }
            }
        };
        for (i, &(g, tg)) in gens.iter().enumerate() {
            let path = format!("generator {i}");
            let o = objs.generators[g].clone();
            let theirs_list = self.u32(tg + 0x20);
            // A list in a file its scene let go of compares by where it was.
            use melee_sim::game::mirror::Cx;
            self.compare_data(&format!("{path} cmd list"), o.cmd_list, theirs_list);
            self.handle_jobj(&format!("{path} jobj"), o.jobj, self.u32(tg + 0x10));
            let ours = vec![
                o.kind,
                o.random.to_bits(),
                o.count.to_bits(),
                u32::from(o.gen_life),
                u32::from(o.ty),
                u32::from(o.bank),
                u32::from(o.link_no),
                u32::from(o.tex_group),
                u32::from(o.idnum),
                u32::from(o.life),
                o.num_child,
                o.userfunc.map_or(0, |f| f.0),
                u32::from(o.rect_flag),
            ];
            let theirs = vec![
                self.u32(tg + 4),
                self.u32(tg + 8),
                self.u32(tg + 0xC),
                u32::from(self.u16(tg + 0x14)),
                u32::from(self.u16(tg + 0x16)),
                u32::from(self.u8(tg + 0x18)),
                u32::from(self.u8(tg + 0x19)),
                u32::from(self.u8(tg + 0x1A)),
                u32::from(self.u16(tg + 0x1C)),
                u32::from(self.u16(tg + 0x1E)),
                self.u32(tg + 0x50),
                self.u32(tg + 0x58),
                u32::from(self.u16(tg + 0x90)),
            ];
            self.check(&path, "fields", ours, theirs);
            let mut ours: Vec<u32> = [o.pos.x, o.pos.y, o.pos.z, o.vel.x, o.vel.y, o.vel.z]
                .iter()
                .chain(&[o.grav, o.fric, o.size, o.radius, o.angle])
                .chain(&o.aux)
                .map(|v| v.to_bits())
                .collect();
            ours.push(0);
            let mut theirs: Vec<u32> = (0..11).map(|k| self.u32(tg + 0x24 + 4 * k)).collect();
            theirs.extend((0..12).map(|k| self.u32(tg + 0x60 + 4 * k)));
            theirs.push(self.u32(tg + 0x5C));
            self.check(&path, "floats", ours, theirs);
            let ts = self.u32(tg + 0x54);
            srt_label(self, &path, o.appsrt, ts);
        }
        // Particles.
        let mut where_t: HashMap<u32, u32> = HashMap::new();
        let mut where_o: HashMap<ParticleId, u32> = HashMap::new();
        for link in 0..particle::LINKS {
            let mut t = self.u32(sym("hsd_804D0908") + 4 * link as u32);
            let mut o = objs.particles.links[link];
            let mut i = 0u32;
            loop {
                let path = format!("link {link} particle {i}");
                match (o, t) {
                    (None, 0) => break,
                    (Some(p), tp) if tp != 0 => {
                        where_t.insert(tp, (link as u32) << 20 | i);
                        where_o.insert(p, (link as u32) << 20 | i);
                        let o_ = objs.particles[p].clone();
                        self.particle(&path, &o_, tp, &gt, &go);
                        let ts = self.u32(tp + 0x8C);
                        srt_label(self, &path, o_.appsrt, ts);
                        o = o_.next;
                        t = self.u32(tp);
                    }
                    (op, tp) => {
                        self.diff(&path, "particle", op, format!("{tp:#x}"));
                        break;
                    }
                }
                i += 1;
            }
        }
        // App-SRTs.
        for (i, &(s, t)) in srts.iter().enumerate() {
            let path = format!("app-SRT {i}");
            let o = &objs.app_srts[s];
            let mut ours = vec![go(o.gp)];
            ours.extend(
                [
                    o.translate.x,
                    o.translate.y,
                    o.translate.z,
                    o.rot.x,
                    o.rot.y,
                    o.rot.z,
                    o.rot.w,
                    o.scale.x,
                    o.scale.y,
                    o.scale.z,
                ]
                .map(f32::to_bits),
            );
            ours.extend([
                u32::from(o.status),
                u32::from(o.frame_num),
                u32::from(o.used_count),
            ]);
            ours.extend(o.mmtx.m.as_flattened().iter().map(|v| v.to_bits()));
            ours.extend(o.view.m.as_flattened().iter().map(|v| v.to_bits()));
            ours.extend(o.view_scale.map(f32::to_bits));
            ours.extend([u32::from(o.idnum), u32::from(o.xa2), 0, 0]);
            let mut theirs = vec![gt(self.u32(t + 4))];
            theirs.extend((0..10).map(|k| self.u32(t + 8 + 4 * k)));
            theirs.extend([
                u32::from(self.u8(t + 0x30)),
                u32::from(self.u8(t + 0x31)),
                u32::from(self.u16(t + 0x32)),
            ]);
            theirs.extend((0..26).map(|k| self.u32(t + 0x34 + 4 * k)));
            theirs.extend([
                u32::from(self.u16(t + 0xA0)),
                u32::from(self.u8(t + 0xA2)),
                self.u32(t),
                self.u32(t + 0x9C),
            ]);
            self.check(&path, "fields", ours, theirs);
        }
        // Counts, the cursor, the pending list and the hook.
        let ps = &objs.particles;
        let gs = &objs.generators;
        let ss = &objs.app_srts;
        let used = |c: &Self, d: &str| c.u32(sym(d) + 8);
        self.check(
            "particles",
            "live and peak particles, generators, family ids, app-SRTs",
            [
                u32::from(ps.live),
                u32::from(ps.peak),
                ps.pool.used(),
                u32::from(gs.live),
                u32::from(gs.peak),
                gs.pool.used(),
                u32::from(gs.idnum),
                u32::from(ss.live),
                u32::from(ss.peak),
                ss.pool.used(),
            ],
            [
                u32::from(self.u16(sym("hsd_804D78E2"))),
                u32::from(self.u16(sym("numPeakParticles"))),
                used(self, "hsd_804D0F60"),
                u32::from(self.u16(sym("hsd_804D78E0"))),
                u32::from(self.u16(sym("hsd_804D78DA"))),
                used(self, "hsd_804D0F90"),
                u32::from(self.u16(sym("lbl_804D6368"))),
                u32::from(self.u16(sym("hsd_804D78DE"))),
                u32::from(self.u16(sym("hsd_804D78D8"))),
                used(self, "HSD_PSAppSrt_804D10B0"),
            ],
        );
        self.check(
            "particles",
            "generator cursor",
            go(gs.cursor),
            gt(self.u32(sym("hsd_804D78F8"))),
        );
        self.check(
            "particles",
            "generator hook",
            gs.on_create.map_or(0, |h| h.0),
            self.u32(sym("hsd_804D7900")),
        );
        let mut pending_t = Vec::new();
        let mut n = self.u32(sym("hsd_804D78F4"));
        while n != 0 {
            pending_t.push(gt(self.u32(n + 4)));
            n = self.u32(n);
        }
        let mut pending_o = Vec::new();
        let mut n = gs.pending;
        while let Some(id) = n {
            pending_o.push(go(Some(gs.nodes[id.0].data)));
            n = gs.nodes[id.0].next;
        }
        self.check("particles", "generators to orient", pending_o, pending_t);
        for slot in 0..particle::POINT_JOBJS {
            let t = self.u32(sym("hsd_804D08E8") + 4 * slot as u32);
            self.handle_jobj(
                &format!("particles point joint {slot}"),
                ps.point_jobjs[slot],
                t,
            );
        }
        // Banks.
        for b in 0..particle::BANKS as u32 {
            let tex = self.u32(sym("psTexGroupArray") + 4 * b);
            let ptclref = self.u32(sym("ptclref_804D0E5C") + 4 * b);
            let count = self.u32(sym("psCmdListArray") + 4 * b) as i32;
            // A bank a scene before loaded stays set up after its file is freed: its
            // textures' pointer then names where the file was.
            let tex_gone = ps.banks[b as usize].and_then(|k| {
                (self.data.try_addr(k.tex).is_none())
                    .then(|| self.past.and_then(|p| p.gone_addr(k.tex)))
                    .flatten()
            });
            let theirs = (tex != 0).then(|| {
                let t = if tex_gone.is_some() {
                    None
                } else {
                    self.data.get(tex - 4)
                };
                (t, tex, ptclref, count)
            });
            let ours = ps.banks[b as usize].map(|k| {
                let table = self
                    .data
                    .try_addr(k.table)
                    .or_else(|| self.past.and_then(|p| p.gone_addr(k.table)))
                    .unwrap_or_else(|| panic!("particle bank {b}'s table isn't loaded"));
                let (t, addr) = match tex_gone {
                    Some(a) => (None, a + 4),
                    None => (Some(k.tex), tex),
                };
                (t, addr, table.wrapping_sub(4 * k.first as u32), k.count)
            });
            self.check("particles", &format!("bank {b}"), ours, theirs);
        }
        // The display pass's sorting state. A link's cached first particle without `TEX_EDGE`
        // is read only in the display pass that sorted the link; between passes it can name a
        // particle freed since, which the import leaves out: such a stale entry counts as none
        // on either side.
        let d = &objs.psdisp;
        let at = |w: &HashMap<u32, u32>, a: u32| {
            if a == 0 {
                u32::MAX
            } else {
                w.get(&a).copied().unwrap_or(u32::MAX)
            }
        };
        let mut theirs = vec![u32::from(self.u8(sym("psFrameNum")))];
        theirs.extend((0..16).map(|i| u32::from(self.u8(sym("HSD_PSDisp_8040C360") + i))));
        theirs.extend((0..16).map(|i| at(&where_t, self.u32(sym("particle_list") + 4 * i))));
        let mut ours = vec![u32::from(d.frame)];
        ours.extend(d.sorted.map(u32::from));
        ours.extend(
            d.non_edge
                .map(|p| p.map_or(u32::MAX, |p| where_o.get(&p).copied().unwrap_or(u32::MAX))),
        );
        self.check("particles", "display sorting", ours, theirs);
    }

    fn particle(
        &mut self,
        path: &str,
        p: &Particle,
        t: u32,
        gt: &dyn Fn(u32) -> u32,
        go: &dyn Fn(Option<GeneratorId>) -> u32,
    ) {
        use melee_sim::game::mirror::Cx;
        self.compare_data(&format!("{path} cmd list"), p.cmd_list, self.u32(t + 0x20));
        let r16 = |o| u32::from(self.u16(t + o));
        let r8 = |o| u32::from(self.u8(t + o));
        let rgba = |o| self.u32(t + o);
        let c = |c: Rgba| u32::from_be_bytes([c.r, c.g, c.b, c.a]);
        let mut theirs = vec![
            self.u32(t + 4),
            r8(8),
            r8(9),
            r8(0xA),
            r8(0xB),
            r16(0xC),
            r16(0xE),
            r16(0x10),
            rgba(0x12),
            rgba(0x16),
            r16(0x1A),
            r8(0x1C),
            r8(0x1D),
            r16(0x1E),
            r16(0x24),
            r16(0x26),
            r16(0x28),
            r16(0x2A),
        ];
        theirs.extend((0..10).map(|k| self.u32(t + 0x2C + 4 * k)));
        theirs.extend([
            r16(0x54),
            r8(0x56),
            r8(0x57),
            r8(0x58),
            r8(0x59),
            r16(0x5A),
            r16(0x5C),
            r16(0x5E),
            self.u32(t + 0x60),
            self.u32(t + 0x64),
            r16(0x68),
            r16(0x6A),
            rgba(0x6C),
            rgba(0x70),
            r16(0x74),
            r16(0x76),
            r16(0x78),
        ]);
        theirs.extend((0x7A..0x84).map(r8));
        theirs.push(self.u32(t + 0x84));
        theirs.push(gt(self.u32(t + 0x88)));
        theirs.push(self.u32(t + 0x90));
        theirs.push(self.u32(t + 0x94));
        let mut ours = vec![
            p.kind,
            u32::from(p.bank),
            u32::from(p.tex_group),
            u32::from(p.pose_num),
            u32::from(p.pal_num),
            u32::from(p.size_count),
            u32::from(p.prim_col_count),
            u32::from(p.env_col_count),
            c(p.prim_col),
            c(p.env_col),
            u32::from(p.cmd_wait),
            u32::from(p.loop_count),
            u32::from(p.link_no),
            u32::from(p.idnum),
            u32::from(p.cmd_ptr),
            u32::from(p.cmd_mark_ptr),
            u32::from(p.cmd_loop_ptr),
            u32::from(p.life),
        ];
        ours.extend(
            [
                p.vel.x, p.vel.y, p.vel.z, p.grav, p.fric, p.pos.x, p.pos.y, p.pos.z, p.size,
                p.rotate,
            ]
            .map(f32::to_bits),
        );
        ours.extend([
            u32::from(p.a_cmp_count),
            u32::from(p.a_cmp_mode),
            u32::from(p.a_cmp_param1),
            u32::from(p.a_cmp_param2),
            u32::from(p.p_jobj_ofs),
            u32::from(p.mat_col_count),
            u32::from(p.amb_col_count),
            u32::from(p.rotate_count),
            p.size_target.to_bits(),
            p.rotate_target.to_bits(),
            u32::from(p.prim_col_remain),
            u32::from(p.env_col_remain),
            c(p.prim_col_target),
            c(p.env_col_target),
            u32::from(p.mat_col_remain),
            u32::from(p.amb_col_remain),
            u32::from(p.a_cmp_remain),
        ]);
        ours.extend(
            [
                p.a_cmp_param1_target,
                p.a_cmp_param2_target,
                p.mat_rgb,
                p.mat_a,
                p.amb_rgb,
                p.amb_a,
                p.mat_rgb_target,
                p.mat_a_target,
                p.amb_rgb_target,
                p.amb_a_target,
            ]
            .map(u32::from),
        );
        ours.push(p.trail.to_bits());
        ours.push(go(p.generator));
        ours.push(u32::from(p.userdata.is_some()));
        ours.push(u32::from(p.callback.is_some()));
        self.check(path, "fields", ours, theirs);
    }
}
