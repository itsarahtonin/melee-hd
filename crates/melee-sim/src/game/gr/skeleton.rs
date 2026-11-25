//! The stages' skeleton: what Fourside, Great Bay, Icicle Mountain, Mushroom Kingdom I and II,
//! Mute City and Rainbow Cruise each wrote the same way, the same calls in the same order:
//! their objects' creation from their callbacks, their setup's frame (the parameters, the entry
//! points' bits, the first objects, the camera limits and blast zones), the hooks and object
//! callbacks that do nothing, the start that runs the enemy spawner without enemies, the setups
//! that only start their map's animation, and the parameters' reads.
//!
//! What differs is data: each stage's [`StageObjects`] (its callbacks by map id) and, for the six
//! whose setup has this shape, its [`Setup`] (its parameters' place, its bits, the objects it
//! makes first, and what it does besides before and after the measuring). The registry's
//! entries call these with the stage's data. Each original's `#[doc(alias)]` is on the item
//! here that ports it. Rainbow Cruise's setup is its own (`rainbow_cruise::on_init`): it uses
//! the objects it made before the measuring after it, so its creations aren't a list.

use melee_data::archive::{Archives, DataRef};

use super::super::state::{Game, Running};
use super::data::StageCallbacks;
use super::ground::ground_of;
use super::{Globals, anime, setup, zako};
use crate::fpu::reg;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// A stage's objects, as their creation reads them.
pub struct StageObjects {
    /// Their setups, processes and destroy callbacks by map id (`grXx_StageCallbacks`).
    pub callbacks: &'static [StageCallbacks],
    /// The map whose process runs at priority 5 rather than 4 (Great Bay's Tingle).
    pub late: Option<i32>,
}

/// A stage's setup (`on_init`), for the stages whose setup is the skeleton's ([`on_init`]).
pub struct Setup {
    /// Its objects.
    pub objects: &'static StageObjects,
    /// Where it keeps the stage's parameters (`yakumono_param`).
    pub param: fn(&mut Globals) -> &mut Option<DataRef>,
    /// `stage_info`'s entry point bits: whether the stage places its own entry points, and
    /// whether a respawn keeps the spawn point.
    pub own_entry_points: bool,
    pub keeps_spawn_point: bool,
    /// The objects it makes first, in order, from the setup's frame.
    pub maps: &'static [i32],
    /// What it does next, before the camera limits and blast zones are measured.
    pub then: Option<fn(&mut Running<'_>)>,
    /// What it does after they're measured.
    pub after: Option<fn(&mut Running<'_>)>,
}

/// Creates one of a stage's objects and sets it up (each stage's copy of `grLast_8021A7F4`,
/// [`setup::create_from_callbacks`]): its process at priority 4, or 5 for the stage's late map.
#[doc(alias = "grFourside_801F2DD0")]
#[doc(alias = "grGreatBay_801F4300")]
#[doc(alias = "setupStageCallbacks@801F71E8")]
#[doc(alias = "setupStageCallbacks@801FA9B4")]
#[doc(alias = "grInishie2_801FCCDC")]
#[doc(alias = "grMuteCity_801EFD0C")]
#[doc(alias = "grRCruise_801FF2C8")]
pub fn create(r: &mut Running, objects: &StageObjects, map_id: i32, frame: u32) -> Option<GObjId> {
    let cb = &objects.callbacks[map_id as usize];
    let priority = if objects.late == Some(map_id) { 5 } else { 4 };
    setup::create_from_callbacks_at(r, map_id, cb, frame, priority)
}

/// A stage's setup: its parameters, its entry point bits, the objects it makes first, what it
/// does next, the camera limits and blast zones, and what it does after (each stage's
/// [`Setup`] says).
#[doc(alias = "grFourside_801F2D10")]
#[doc(alias = "grGreatBay_801F4240")]
#[doc(alias = "grIceMt_801F686C")]
#[doc(alias = "grInishie1_801FA90C")]
#[doc(alias = "grInishie2_801FCBC4")]
#[doc(alias = "grMuteCity_801EFC6C")]
pub fn on_init(r: &mut Running, s: &Setup) {
    let param = super::ground::yakumono_param(r.game);
    *(s.param)(&mut r.game.gr) = param;
    r.game.stage_info.bits.own_entry_points = s.own_entry_points;
    r.game.stage_info.bits.keeps_spawn_point = s.keeps_spawn_point;
    for &map_id in s.maps {
        create(r, s.objects, map_id, setup::SETUP_FRAME);
    }
    if let Some(then) = s.then {
        then(r);
    }
    setup::measure_cam_bounds(r.game, r.data);
    setup::measure_blast_zones(r.game, r.data);
    if let Some(after) = s.after {
        after(r);
    }
}

/// The stages' hooks that do nothing: the demos' setups (only a demo runs them), the loads but
/// Icicle Mountain's, and Icicle Mountain's start.
#[doc(alias = "grFourside_801F2D0C")]
#[doc(alias = "grFourside_801F2DA0")]
#[doc(alias = "grGreatBay_801F423C")]
#[doc(alias = "grGreatBay_801F42D0")]
#[doc(alias = "grIceMt_801F6868")]
#[doc(alias = "grIceMt_801F71DC")]
#[doc(alias = "grInishie1_801FA908")]
#[doc(alias = "grInishie1_801FA984")]
#[doc(alias = "grInishie2_801FCBC0")]
#[doc(alias = "grInishie2_801FCCAC")]
#[doc(alias = "grMuteCity_801EFC68")]
#[doc(alias = "grMuteCity_801EFCDC")]
#[doc(alias = "grRCruise_801FF164")]
#[doc(alias = "grRCruise_801FF298")]
pub fn nothing(_r: &mut Running) {}

/// The stages' start but Icicle Mountain's: the enemy spawner, without enemies.
#[doc(alias = "grFourside_801F2DA4")]
#[doc(alias = "grGreatBay_801F42D4")]
#[doc(alias = "grInishie1_801FA988")]
#[doc(alias = "grInishie2_801FCCB0")]
#[doc(alias = "grMuteCity_801EFCE0")]
#[doc(alias = "grRCruise_801FF29C")]
pub fn start(r: &mut Running) {
    zako::start(r.game, r.data, None);
}

/// The objects' processes and destroy callbacks that do nothing (Icicle Mountain's: the
/// scenery's, the climbs' destroy callbacks and the background's).
#[doc(alias = "grFourside_801F2EF0")]
#[doc(alias = "grFourside_801F2EF4")]
#[doc(alias = "grFourside_801F2F2C")]
#[doc(alias = "grFourside_801F2F30")]
#[doc(alias = "grFourside_801F309C")]
#[doc(alias = "grFourside_801F314C")]
#[doc(alias = "grFourside_801F3150")]
#[doc(alias = "grFourside_801F37F8")]
#[doc(alias = "grFourside_801F3B6C")]
#[doc(alias = "grFourside_801F3F0C")]
#[doc(alias = "grGreatBay_801F4438")]
#[doc(alias = "grGreatBay_801F443C")]
#[doc(alias = "grGreatBay_801F44A8")]
#[doc(alias = "grGreatBay_801F44AC")]
#[doc(alias = "grGreatBay_801F4518")]
#[doc(alias = "grGreatBay_801F451C")]
#[doc(alias = "grGreatBay_801F4690")]
#[doc(alias = "grGreatBay_801F545C")]
#[doc(alias = "grGreatBay_801F5988")]
#[doc(alias = "grGreatBay_801F59F8")]
#[doc(alias = "grGreatBay_801F59FC")]
#[doc(alias = "grGreatBay_801F5D48")]
#[doc(alias = "stageGObj0_GObjProc@801F75E4")]
#[doc(alias = "stageGObj0_Callback3@801F75E8")]
#[doc(alias = "stageGObj9_Callback3")]
#[doc(alias = "stageGObj8_Callback3")]
// Ports Icicle Mountain's stageGObj10_Callback3; aliased once a check reaches it.
#[doc(alias = "grInishie1_801FAAD4")]
#[doc(alias = "grInishie1_801FAAD8")]
#[doc(alias = "grInishie1_801FABB0")]
#[doc(alias = "grInishie1_801FABFC")]
#[doc(alias = "grInishie1_801FAC00")]
#[doc(alias = "grInishie1_801FAD80")]
#[doc(alias = "grInishie2_801FCDFC")]
#[doc(alias = "grInishie2_801FCE00")]
#[doc(alias = "grInishie2_801FD014")]
#[doc(alias = "grInishie2_801FD364")]
#[doc(alias = "grInishie2_801FD740")]
#[doc(alias = "grInishie2_801FDFE4")]
#[doc(alias = "grMuteCity_801F01B0")]
#[doc(alias = "grMuteCity_801F040C")]
#[doc(alias = "grMuteCity_801F0444")]
#[doc(alias = "grMuteCity_801F0448")]
#[doc(alias = "grMuteCity_801F04B0")]
#[doc(alias = "grMuteCity_801F04B4")]
#[doc(alias = "grMuteCity_801F0948")]
#[doc(alias = "grRCruise_801FF3E8")]
#[doc(alias = "grRCruise_801FF3EC")]
#[doc(alias = "grRCruise_801FF43C")]
#[doc(alias = "grRCruise_801FF440")]
#[doc(alias = "grRCruise_801FF738")]
#[doc(alias = "grRCruise_801FF79C")]
#[doc(alias = "grRCruise_801FF7A0")]
#[doc(alias = "grRCruise_801FF920")]
#[doc(alias = "grRCruise_80200070")]
#[doc(alias = "grRCruise_8020045C")]
pub fn empty(_r: &mut Running, _gobj: GObjId) {}

/// The setups that only start their map's animation (the inline `Ground_StartMapAnim`):
/// Fourside's maps 0 and 2, Great Bay's map 0, both Mushroom Kingdoms' scenery, Mute City's maps
/// 36 and 37, Rainbow Cruise's background.
#[doc(alias = "stageGObj0_OnInit@801F2EBC")]
#[doc(alias = "stageGObj2_OnInit@801F2EF8")]
#[doc(alias = "stageGObj0_OnInit@801F4404")]
#[doc(alias = "stageGObj0_OnInit@801FAAA0")]
#[doc(alias = "stageGObj0_OnInit@801FCDC8")]
#[doc(alias = "stageGObj36_37_OnInit")]
#[doc(alias = "grRCruise_801FF3B4")]
pub fn animate(r: &mut Running, gobj: GObjId) {
    let m = map_id(r.game, gobj);
    anime::start_map(r.game, r.data, gobj, m, 0);
}

/// A stage object's map id.
pub fn map_id(game: &Game, gobj: GObjId) -> i32 {
    game.grounds[ground_of(game, gobj).0].map_id
}

/// A stage's parameters (`yakumono_param`), read by offset.
#[derive(Clone, Copy)]
pub struct Param(pub DataRef);

impl Param {
    /// The parameters the stage's setup kept, or the stage's panic naming them.
    pub fn of(param: Option<DataRef>, what: &str) -> Param {
        Param(param.expect(what))
    }
    pub fn f(self, data: &Archives, off: u32) -> f32 {
        data.f32(self.0.at(off))
    }
    pub fn i(self, data: &Archives, off: u32) -> i32 {
        data.u32(self.0.at(off)) as i32
    }
    pub fn h(self, data: &Archives, off: u32) -> i16 {
        data.u16(self.0.at(off)) as i16
    }
    pub fn hu(self, data: &Archives, off: u32) -> u16 {
        data.u16(self.0.at(off))
    }
    pub fn ptr(self, data: &Archives, off: u32) -> Option<DataRef> {
        data.ptr(self.0.at(off))
    }
    /// A float field taken as an integer (`fctiwz`).
    pub fn fi(self, data: &Archives, off: u32) -> i32 {
        gekko_fp::fctiwz(reg(self.f(data, off)))
    }
    /// Three floats from `off`.
    pub fn v(self, data: &Archives, off: u32) -> Vec3 {
        Vec3::new(
            self.f(data, off),
            self.f(data, off + 4),
            self.f(data, off + 8),
        )
    }
}
