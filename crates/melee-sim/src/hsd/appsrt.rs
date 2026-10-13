//! Particle transforms, HAL's app-SRT (`sysdolphin/baselib/psappsrt.c`): a scale, rotation and
//! translation that generators and particles share, by reference count, so an effect can move
//! with what spawned it.

use std::ops::{Index, IndexMut};

use super::Objects;
use super::generator::GeneratorId;
use super::particle::ParticleId;
use super::pool::Pool;
use crate::hsd::heap::Kind;
use crate::hsd::pool::kind as pool_kind;
use crate::mtx::Mtx34;
use crate::quat::Quat;
use crate::vec::Vec3;

/// An app-SRT's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AppSrtId(pub u32);

/// `PS_APPSTATUS_ONCE`: the display pass computes the matrix once, then holds it.
pub const ONCE: u8 = 1;
/// `PS_APPSTATUS_STILL`: the display pass keeps the matrix it has.
pub const STILL: u8 = 2;

/// `HSD_psAppSRT`. `next` stays null and `freefunc`, which nothing in the game sets, stays
/// null, so neither is kept.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq)]
pub struct AppSrt {
    /// The generator whose joint moves it, if any.
    pub gp: Option<GeneratorId>,
    pub translate: Vec3,
    /// Euler angles in x, y and z; `w` is never set.
    pub rot: Quat,
    pub scale: Vec3,
    pub status: u8,
    pub frame_num: u8,
    pub used_count: u16,
    /// The matrix `translate`, `rot` and `scale` make (`mmtx`).
    pub mmtx: Mtx34,
    /// The display pass's view matrix for it (`ssx` to `x90`) and its scale on the screen's
    /// axes (`x94`, `x98`).
    pub view: Mtx34,
    pub view_scale: [f32; 2],
    pub idnum: u16,
    /// Set for a generator's own transform, which the display pass draws unrotated (`xA2`).
    pub xa2: u8,
}

/// The app-SRT pool (`HSD_PSAppSrt_804D10B0`) and its counts.
#[derive(melee_derive::CloneFrom, Debug, Default)]
pub struct AppSrts {
    pub pool: Pool<AppSrt, { pool_kind(Kind::AppSrt) }>,
    /// Live app-SRTs (`hsd_804D78DE`) and the most at once (`hsd_804D78D8`).
    pub live: u16,
    pub peak: u16,
}

impl Index<AppSrtId> for AppSrts {
    type Output = AppSrt;
    fn index(&self, s: AppSrtId) -> &AppSrt {
        &self.pool[s.0]
    }
}

impl IndexMut<AppSrtId> for AppSrts {
    fn index_mut(&mut self, s: AppSrtId) -> &mut AppSrt {
        &mut self.pool[s.0]
    }
}

/// `HSD_psAppSRT`'s size: `psInitAppSRT`'s object size must cover it.
const SIZE: i32 = 0xA4;

/// Empties the pool, `psInitAppSRT`. The game passes the structure's size; a smaller one
/// would overlap objects in memory, which plain data can't hold.
#[doc(alias = "psInitAppSRT")]
pub fn init(objs: &mut Objects, _num: i32, size: i32) -> i32 {
    assert!(size >= SIZE, "psInitAppSRT: objects of {size:#x} bytes");
    objs.app_srts = AppSrts::default();
    0
}

/// A new app-SRT with an identity transform, `psAddGeneratorAppSRT`.
#[doc(alias = "psAddGeneratorAppSRT")]
pub fn add(objs: &mut Objects, status: i32, idnum: u16) -> AppSrtId {
    let s = &mut objs.app_srts;
    let id = AppSrtId(s.pool.alloc(AppSrt {
        used_count: 1,
        status: status as u8,
        scale: Vec3::new(1.0, 1.0, 1.0),
        idnum,
        ..AppSrt::default()
    }));
    s.live = s.live.wrapping_add(1);
    if s.live > s.peak {
        s.peak = s.live;
    }
    id
}

/// Gives a particle a new app-SRT, `psAddParticleAppSRT_begin`.
#[doc(alias = "psAddParticleAppSRT_begin")]
pub fn add_to_particle(objs: &mut Objects, p: ParticleId, status: i32) -> AppSrtId {
    let s = add(objs, status, objs.particles[p].idnum);
    objs.particles[p].appsrt = Some(s);
    s
}

/// Gives a generator a new app-SRT, `psAddGeneratorAppSRT_begin`.
#[doc(alias = "psAddGeneratorAppSRT_begin")]
pub fn add_to_generator(objs: &mut Objects, g: GeneratorId, status: i32) -> AppSrtId {
    let s = add(objs, status, objs.generators[g].idnum);
    objs.generators[g].appsrt = Some(s);
    s
}

/// Shares `s` with a particle that has none; returns its new use count, or -1,
/// `psAttachParticleAppSRT`.
#[doc(alias = "psAttachParticleAppSRT")]
pub fn attach_particle(objs: &mut Objects, p: ParticleId, s: Option<AppSrtId>) -> i32 {
    let Some(s) = s else { return -1 };
    if objs.particles[p].appsrt.is_some() {
        return -1;
    }
    objs.particles[p].appsrt = Some(s);
    let srt = &mut objs.app_srts[s];
    srt.used_count = srt.used_count.wrapping_add(1);
    i32::from(srt.used_count)
}

/// Shares `s` with a generator that has none, `psAttachGeneratorAppSRT`.
#[doc(alias = "psAttachGeneratorAppSRT")]
pub fn attach_generator(objs: &mut Objects, g: GeneratorId, s: Option<AppSrtId>) -> i32 {
    let Some(s) = s else { return -1 };
    if objs.generators[g].appsrt.is_some() {
        return -1;
    }
    objs.generators[g].appsrt = Some(s);
    let srt = &mut objs.app_srts[s];
    srt.used_count = srt.used_count.wrapping_add(1);
    i32::from(srt.used_count)
}

/// Drops one use of `s`, freeing it after the last; returns the uses left.
fn release(objs: &mut Objects, s: AppSrtId) -> i32 {
    let srt = &mut objs.app_srts[s];
    srt.used_count = srt.used_count.wrapping_sub(1);
    let count = i32::from(srt.used_count);
    if count == 0 {
        objs.app_srts.pool.free(s.0);
        objs.app_srts.live = objs.app_srts.live.wrapping_sub(1);
    }
    count
}

/// Takes a particle's app-SRT away; returns the uses left, or -1 if it had none,
/// `psRemoveParticleAppSRT`.
#[doc(alias = "psRemoveParticleAppSRT")]
pub fn remove_from_particle(objs: &mut Objects, p: ParticleId) -> i32 {
    let Some(s) = objs.particles[p].appsrt else {
        return -1;
    };
    let count = release(objs, s);
    objs.particles[p].appsrt = None;
    count
}

/// Takes a generator's app-SRT away, unbinding it from the generator first if it follows
/// that generator, `psRemoveGeneratorSRT`.
#[doc(alias = "psRemoveGeneratorSRT")]
pub fn remove_from_generator(objs: &mut Objects, g: GeneratorId) -> i32 {
    let Some(s) = objs.generators[g].appsrt else {
        return -1;
    };
    if objs.app_srts[s].gp == Some(g) {
        objs.app_srts[s].gp = None;
    }
    let count = release(objs, s);
    objs.generators[g].appsrt = None;
    count
}
