//! The game's state: HSD's objects and scheduler, and Melee's own objects on top.
//!
//! The original's function pointers (GObj processes, render callbacks, the callbacks fighters
//! and items keep) are [`CodeRef`]s, the original function's address. [`code`](super::code)
//! dispatches them to their ports.

use melee_data::archive::Archives;

use super::camera::CameraState;
use super::gr;
use super::handles::{CodeRef, FighterId, GroundId, ItemId};
use super::structs::{
    _StaticPlayer, ControllerMap, DynamicsData, Fighter, FighterBone, Ground, Item, StageInfo,
    VsSceneController, gm_80479D58_t, ifMagnify,
};
use super::{ft, gm, pl};
use crate::hsd::Objects;
use crate::hsd::cobj::CObjId;
use crate::hsd::fog::FogId;
use crate::hsd::gobj::{GObjId, Host, Kinds, World};
use crate::hsd::jobj::JObjId;
use crate::hsd::lobj::LObjId;
use crate::hsd::pool::Pool;

/// What a GObj's user data is (`user_data`, by its `user_data_kind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserData {
    Fighter(FighterId),
    Item(ItemId),
    /// A stage object (user data kind 3).
    Ground(GroundId),
    /// An effect (user data kind 8).
    Effect(super::handles::EffectId),
    /// User data of a kind no port models yet (a sound's, an effect's), by its address in a
    /// captured state: code that reads it stops.
    Raw(super::handles::RawPtr),
}

/// The GObj object kinds as Melee's `HSD_GObj_80391260` numbers them, after the game's own
/// kind 0 (`HSD_GObj_CameraKind` and friends).
pub const OBJ_CAMERA: u8 = 1;
pub const OBJ_LIGHT: u8 = 2;
pub const OBJ_JOBJ: u8 = 3;
pub const OBJ_FOG: u8 = 4;

/// The user data kind of stage objects.
pub const USER_DATA_GROUND: u8 = 3;

/// Gameplay toggles (`docs/architecture.md`, rulesets).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rules {
    /// The Slippi ruleset's online determinism fixes from Slippi's playback codes: the stage
    /// objects' user data is zeroed when allocated (`Ground_GetStageGObj`), and Final
    /// Destination's background cycle keeps the random seed it draws from to itself
    /// (`grLast_8021AAB0`).
    pub slippi: bool,
}

/// What a GObj's HSD object is (`hsd_obj`, by its `obj_kind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Obj {
    JObj(JObjId),
    CObj(CObjId),
    /// A light list's first light.
    LObj(LObjId),
    Fog(FogId),
}

/// The game's types for the scheduler.
#[derive(Clone, Debug)]
pub struct GameKinds;

impl Kinds for GameKinds {
    type Proc = CodeRef;
    type Render = CodeRef;
    type UserData = UserData;
    type Obj = Obj;
}

/// What the simulation hands the presentation: sounds and controller rumble, which the
/// original passes to the audio and pad drivers mid-tick.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// `HSD_AudioSFXKeyOffTrack`: the sound on a track stops.
    SfxKeyOffTrack(i32),
    /// `HSD_PadRumbleRemoveId`: a controller's rumble of one kind stops.
    RumbleRemove { port: u8, id: i32 },
    /// `HSD_PadRumbleAdd` (through `lb_80014574`): a controller rumbles with pattern `kind`
    /// for `frames` (-2: the pattern's length).
    RumbleAdd {
        port: u8,
        id: i32,
        kind: i32,
        frames: i32,
    },
    /// A sound effect plays (`lbAudioAx_800237A8`: id, volume, pan).
    Sfx { id: i32, volume: u8, pan: u8 },
    /// A sound plays on a voice group (`lbAudioAx_80023870`: id, volume, pan, group).
    SfxGroup {
        id: i32,
        volume: u8,
        pan: u8,
        group: i32,
    },
    /// An audio-driver call the frontend acts on (the original function's address and its
    /// integer arguments): the crowd, the announcer, music changes.
    Audio { func: u32, args: [i32; 4] },
}

/// Everything a tick reads and writes.
#[derive(Clone, Debug, Default)]
pub struct Game {
    pub world: World<GameKinds>,
    pub objs: Objects,
    pub fighters: Pool<Fighter>,
    /// The items (`Item`, the item GObjs' user data, from `item_alloc_data`).
    pub items: Pool<Item>,
    /// The item code's other globals.
    pub it: super::it::Globals,
    /// The effects library's globals.
    pub ef: super::ef::Globals,
    /// The stage objects (`Ground`, the stage GObjs' user data).
    pub grounds: Pool<Ground>,
    pub stage_info: StageInfo,
    /// The stage's collision geometry, once `mpLibLoad` has loaded it.
    pub geometry: Option<gr::Geometry>,
    /// The stage code's other globals.
    pub gr: gr::Globals,
    /// The game camera (`game_camera`), its camera boxes and the rest of its state.
    pub camera: CameraState,
    /// The player slots (`player_slots`).
    pub players: [_StaticPlayer; 6],
    /// The current game mode (`state_machine.routing.curr_mode`, a `GameModeKind`).
    pub game_mode: u8,
    /// The pad library: the poll queue, rumble and the master, copy and game statuses
    /// (`HSD_PadCopyStatus` is what the frame's game code reads).
    pub pads: crate::input::controller::Controllers,
    /// The scenes' button maps (`controller_map`; its fifth entry merges every port).
    pub controller_map: ControllerMap,
    pub rules: Rules,
    /// Fighters' parts tables (`Fighter::parts`), one entry per bone of the character.
    pub fighter_parts: Pool<Vec<FighterBone>>,
    /// Fighters' copies of their character's attributes (`Fighter::dat_attrs`): the
    /// `fighter_dat_attrs_alloc_data` block's big-endian bytes, whose layout each character
    /// defines.
    pub fighter_attrs: Pool<Vec<u8>>,
    /// The links of fighters' dynamic bone chains (`BoneDynamicsDesc::dyn_desc`), each a
    /// joint's physics state.
    pub fighter_dynamics: Pool<DynamicsData>,
    /// The fighter code's globals.
    pub ft: ft::Globals,
    /// The player library's globals.
    pub pl: pl::Globals,
    /// The off-screen magnifier (`ifMagnify_804A1DE0`): which players are off screen, which
    /// deals them damage over time.
    pub magnify: ifMagnify,
    /// mpcoll's statics: the last pass's walls and correction, and the floor filter.
    pub coll: crate::mp::coll::CollStatics,
    /// The scene loop's state (`gm_80479D58`): its tick counts, the debug pause flags and
    /// the pause mask the scheduler reads.
    pub scene: gm_80479D58_t,
    /// How engine ticks fall on frames (`lb_804329F0`).
    pub pacing: crate::lb::pacing::Pacing,
    /// The audio driver's voices, the synth and AX voices under them, and the timeline that
    /// runs their frames.
    pub audio: crate::audio::Audio,
    /// The VS match's controller (`gmvs.c`'s `controller`): its rules, timer, pause state,
    /// per-fighter respawn state and results.
    pub vs: VsSceneController,
    /// The game-mode layer's other globals.
    pub gm: gm::Globals,
    /// What the tick asked of the audio and pad drivers since the frontend last took them.
    pub events: Vec<Event>,
}

/// The game as the scheduler's host for one call: the state and the game data it reads.
pub struct Running<'a> {
    pub game: &'a mut Game,
    pub data: &'a Archives,
}

impl Host<GameKinds> for Running<'_> {
    fn world(&mut self) -> &mut World<GameKinds> {
        &mut self.game.world
    }

    fn invoke(&mut self, gobj: GObjId, proc: CodeRef) {
        super::code::run_proc(self, gobj, proc);
    }

    fn remove_user_data(&mut self, _gobj: GObjId, _kind: u8, data: UserData) {
        match data {
            // `mem_free`: the block goes back to the heap.
            UserData::Ground(g) => self.game.grounds.free(g.0),
            UserData::Item(i) => super::it::item::free_user_data(self.game, i),
            UserData::Effect(e) => super::ef::free_user_data(self.game, e),
            other => panic!("removing {other:?} isn't ported"),
        }
    }

    fn remove_obj(&mut self, _kind: u8, obj: Obj) {
        match obj {
            Obj::JObj(j) => crate::hsd::jobj::remove_all(&mut self.game.objs, Some(j)),
            Obj::Fog(f) => crate::hsd::fog::unref(&mut self.game.objs, Some(f)),
            other => panic!("releasing a GObj's {other:?} needs its port"),
        }
    }
}
