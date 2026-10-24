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
use crate::hsd::heap::Kind;
use crate::hsd::jobj::JObjId;
use crate::hsd::lobj::LObjId;
use crate::hsd::pool::Pool;
use crate::hsd::pool::kind as pool_kind;

/// What a GObj's user data is (`user_data`, by its `user_data_kind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub enum UserData {
    Fighter(FighterId),
    Item(ItemId),
    /// A link of an item made of links (user data kind 6 on a GObj of class 7, `ItemLink`).
    ItemLink(super::handles::ItemLinkId),
    /// A stage object (user data kind 3).
    Ground(GroundId),
    /// A sound object (user data kind 23, [`crate::lb::sound_obj`]).
    Sound(super::handles::SoundObjId),
    /// An effect (user data kind 8).
    Effect(super::handles::EffectId),
    /// User data of a kind no port models yet (a sound's, an effect's), by its address in a
    /// captured state: code that reads it stops.
    Raw(super::handles::RawPtr),
    /// A text the GObj draws and frees (`HSD_SisLib_803A5A2C`'s).
    Text(crate::hsd::sis::TextId),
    /// A text canvas's camera, which its canvas forgets when freed (`fn_803A60EC`'s).
    SisCamera(GObjId),
    /// A player's stock icons (`ifStock_804A157C[player]`, set without a kind).
    StockIcons(u8),
    /// An off-screen arrow's slot (`un_804DDBA8[slot]`, set without a kind).
    ArrowSlot(u8),
    /// A player tag's slot (a heap byte `mn_8022EB04` frees).
    NameTagSlot(u8),
    /// The background flash's overlay ([`crate::lb::bgflash::BgFlash::user`]).
    BgFlash,
    /// A player's magnifier bubble (`&ifMagnify_804A1DE0.player[slot]`).
    MagnifyPlayer(u8),
    /// One of Pokémon Stadium's screens (user data kind 3, freed with `HSD_Free`): its index in
    /// [`super::gr::stadium::Globals::screens`].
    StadiumScreen(u8),
    /// Fountain of Dreams' reflection camera's (user data kind 3, freed with `HSD_Free`):
    /// [`super::gr::izumi::Globals::reflection`].
    IzumiReflection,
}

/// The GObj object kinds as Melee's `HSD_GObj_80391260` numbers them, after the game's own
/// kind 0 (`HSD_GObj_CameraKind` and friends).
pub const OBJ_CAMERA: u8 = 1;
pub const OBJ_LIGHT: u8 = 2;
pub const OBJ_JOBJ: u8 = 3;
pub const OBJ_FOG: u8 = 4;

/// The user data kind of stage objects.
pub const USER_DATA_GROUND: u8 = 3;

/// Gameplay toggles (`docs/architecture.md`, rulesets): the Slippi ruleset's codes, each rule
/// on or off ([`Rules::from_codes`] reads them from a replay; `game::slippi` lists them).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Rules {
    /// The online determinism fixes (`Common` and `Online/Core`): among those the ported code
    /// reaches, the stage objects' user data is zeroed when allocated (`Ground_GetStageGObj`),
    /// and Final Destination's background cycle keeps the random seed it draws from to itself
    /// (`grLast_8021AAB0`).
    pub slippi: bool,
    /// Which UCF (`game::ucf`).
    pub ucf: super::ucf::Ucf,
    /// Neutral spawn points.
    pub neutral_spawn: bool,
    /// Wobbling prevention.
    pub prevent_wobbling: bool,
    /// Off-screen damage from the stage's camera limits.
    pub offscreen_limits: bool,
    /// The Ice Climbers' freeze glitch fix.
    pub freeze_glitch_fix: bool,
    /// Which version of Slippi's codes the rules come from, where versions differ.
    pub codes: super::slippi::CodeSet,
    /// The codes players add to their own setups that the match runs
    /// (`game::player_codes`).
    pub player: super::player_codes::PlayerCodes,
}

/// Cosmetic toggles (`docs/architecture.md`, simulation and presentation), as Slippi's
/// optional codes are: each changes what the display pass draws, and with it state only the
/// pass reads, never gameplay (`crates/melee-player/src/cosmetic_tests.rs` checks it). The frontend sets them;
/// they're state, so a rollback restores them with the rest.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Cosmetic {
    /// Fighters the game camera marks off screen draw anyway, as Slippi's widescreen code
    /// draws them (`80086B24`, "Draw High Poly Models": `ftLib_UpdateScreenVisibility`'s branch
    /// that returns "don't draw" made a nop), so a widescreen picture's added sides show them.
    /// The mark (`x221F_b0`, `x2188`), its bubble damage and the magnifier stay the game's.
    pub draw_offscreen_fighters: bool,
}

/// What a GObj's HSD object is (`hsd_obj`, by its `obj_kind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub enum Obj {
    JObj(JObjId),
    CObj(CObjId),
    /// A light list's first light.
    LObj(LObjId),
    Fog(FogId),
    /// A chain of sprites, by its first (none until one is made).
    SObj(Option<crate::hsd::sobj::SObjId>),
}

/// The game's types for the scheduler.
#[derive(melee_derive::CloneFrom, Debug, melee_derive::StateEq)]
pub struct GameKinds;

impl Kinds for GameKinds {
    type Proc = CodeRef;
    type Render = CodeRef;
    type UserData = UserData;
    type Obj = Obj;
}

/// What the simulation hands the presentation: sounds and controller rumble, which the
/// original passes to the audio and pad drivers mid-tick.
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
pub enum Event {
    /// `HSD_AudioSFXKeyOffTrack`: the sound on a track stops.
    SfxKeyOffTrack(i32),
    /// `HSD_PadRumbleRemoveId`: a controller's rumble of one kind stops.
    RumbleRemove {
        port: u8,
        id: i32,
    },
    /// `HSD_PadRumbleAdd` (through `lb_80014574`): a controller rumbles with pattern `kind`
    /// for `frames` (-2: the pattern's length).
    RumbleAdd {
        port: u8,
        id: i32,
        kind: i32,
        frames: i32,
    },
    /// A sound starts (`HSD_AudioSFXStartParam`, through `lbAudioAx_800237A8` or `80023870`):
    /// its handle (-1 if none was free), id, the driver's volume and pan, and its track (0
    /// none). What plays is the audio state's: the voices' parameter blocks.
    SfxStart {
        handle: i32,
        id: i32,
        volume: u8,
        pan: u8,
        track: i32,
    },
    /// `HSD_AudioSFXKeyOff`: a sound stops.
    SfxKeyOff(i32),
    /// `HSD_AudioSFXKeyOffAll`: every sound stops.
    SfxKeyOffAll,
    /// A sound's pan, volume or pitch changes (`HSD_AudioSFXSetPan`, `SetVolumeEx`,
    /// `SetPitchFid`).
    SfxPan {
        handle: i32,
        pan: u8,
    },
    SfxVolume {
        handle: i32,
        volume: u8,
    },
    SfxPitch {
        handle: i32,
        cents: i16,
    },
    /// An audio-driver call the frontend acts on (the original function's address and its
    /// integer arguments): the crowd, the announcer, music changes.
    Audio {
        func: u32,
        args: [i32; 4],
    },
}

/// Everything a tick reads and writes.
#[derive(melee_derive::CloneFrom, Debug, Default, melee_derive::StateEq)]
pub struct Game {
    pub world: World<GameKinds>,
    pub objs: Objects,
    pub fighters: Pool<Fighter, { pool_kind(Kind::Fighter) }>,
    /// The items (`Item`, the item GObjs' user data, from `item_alloc_data`).
    pub items: Pool<Item, { pool_kind(Kind::Item) }>,
    /// The item code's other globals.
    pub it: super::it::Globals,
    /// The effects: efLib's and efAsync's globals and pools.
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
    pub cosmetic: Cosmetic,
    /// Fighters' parts tables (`Fighter::parts`), one entry per bone of the character.
    pub fighter_parts: Pool<Vec<FighterBone>, { pool_kind(Kind::FighterParts) }>,
    /// Fighters' copies of their character's attributes (`Fighter::dat_attrs`): the
    /// `fighter_dat_attrs_alloc_data` block's big-endian bytes, whose layout each character
    /// defines.
    pub fighter_attrs: Pool<Vec<u8>, { pool_kind(Kind::FighterAttrs) }>,
    /// Fighters' lists of their models' DObjs (`DObjList::data`): the main model's (up to
    /// 124) and the second model's (up to 32), the first `count` of each meaningful.
    pub fighter_dobj_lists: Pool<Vec<Option<crate::hsd::dobj::DObjId>>>,
    /// Fighters' animation buffers (`fighter_x59C_alloc_data`): the archives loaded into
    /// each, the last of which the fighter names (`x590`, `x598`), and what longer ones
    /// before it left past its end.
    pub fighter_anim_bufs: Pool<ft::anim_buf::AnimBuf, { pool_kind(Kind::FighterX59C) }>,
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
    /// The crowd's gasps, chants and cheers (`crowdsfx`).
    pub crowd: super::sfx::Crowd,
    /// The VS match's controller (`gmvs.c`'s `controller`): its rules, timer, pause state,
    /// per-fighter respawn state and results.
    pub vs: VsSceneController,
    /// The game-mode layer's other globals.
    pub gm: gm::Globals,
    /// The HUD and the debug text (`melee/if`).
    pub hud: super::hud::Hud,
    /// Refraction's textures and users (`lbrefract.c`).
    pub refract: crate::lb::refract::Refract,
    /// The background flash (`lbbgflash.c`, `lb_0219.c`).
    pub bgflash: crate::lb::bgflash::BgFlash,
    /// What the tick asked of the audio and pad drivers since the frontend last took them.
    pub events: Vec<Event>,
    /// Slippi replay playback's state, in a match a replay drives (`playback`).
    pub playback: Option<super::playback::Playback>,
    /// UCF's pad buffer (with [`Rules::ucf`]).
    pub ucf: super::ucf::Pads,
    /// What the players' codes keep outside the game's structs (with [`Rules::player`]).
    pub player_code_state: super::player_codes::State,
    /// The rumble patterns by kind (`lb_804D63C0`, `LbRb.dat`'s `lbRumbleData`).
    pub rumble_data: Option<melee_data::archive::DataRef>,
    /// The text system (SisLib).
    pub sis: crate::hsd::sis::SisLib,
    /// The audio library's sound objects (`lbl_80433710`).
    pub sound_objs: crate::lb::sound_obj::SoundObjs,
    /// The game's data (`gmMainLib_8045A6C0`): the modes' settings, the rules and the
    /// card's data, as the boot resets them and the memory card loads them.
    pub game_data: super::structs::gmm_x0,
    /// The power-on state (`gmMainLib_8046B0F0`) and the clock at power-on.
    pub main_lib: super::gm::main_lib::MainLib,
    /// The game modes' machine (`state_machine`) and where its loops stand.
    pub machine: super::gm::machine::Machine,
    /// The boot mode's statics (`gmboot.c`) and the opening mode's state to start at.
    pub boot_mode: super::gm::boot_mode::BootMode,
    /// The memory card scene's statics (`gmscmemcard.c`, `gm_1ADD.c`).
    pub memcard: super::gm::memcard::MemCard,
    /// The opening movie's scene's statics (`gmopening.c`).
    pub opening: super::gm::opening::Opening,
    /// The title's statics (`gmtitle.c`, `gmtitlemode.c`).
    pub title: super::gm::title::Title,
    /// The progressive scan prompt's statics (`gmprogressive.c`, `gmprogressivemode.c`).
    pub prog_scan: super::gm::progressive::ProgScan,
    /// The game's side of the memory card (`lbcardgame.c`).
    pub card_game: crate::lb::cardgame::CardGame,
    /// What the game keeps of the card's task lists (`lbcardnew.c`).
    pub card_tasks: crate::lb::save::CardTasks,
    /// Camera Mode's snapshots' state, as far as the boot sets it (`lbsnap.c`).
    pub snap: crate::lb::snap::Snap,
    /// The modes' settings that live outside the game's data: the training mode's saved
    /// players (`gm_80473814`), the Home-Run Contest's (`gmHomeRun_VsModeData`) and the
    /// trophy gallery's flag (`_Toy_804A26B8.x194`).
    pub modes: super::gm::mode_init::ModeStatics,
    /// Presentation output, not state: the Slippi replay the match makes, when the frontend
    /// turns it on (`record`). Simulated code never reads it; a clone starts without it and a
    /// restore leaves it alone.
    #[clone_from(keep)]
    pub recorder: Option<Box<super::record::Recorder>>,
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
        if self.game.recorder.is_some() {
            super::record::before_proc(self.game, gobj);
        }
        super::code::run_proc(self, gobj, proc);
    }

    fn remove_user_data(&mut self, _gobj: GObjId, _kind: u8, data: UserData) {
        match data {
            // `mem_free`: the block goes back to the heap.
            UserData::Ground(g) => {
                crate::hsd::heap::mem_free(self.game.grounds.addr(g.0));
                self.game.grounds.free(g.0);
            }
            UserData::Item(i) => super::it::item::free_user_data(self.game, i),
            // `it_802A2474`: the link goes back to `item_link_alloc_data`.
            UserData::ItemLink(l) => super::it::link::free(self.game, l),
            UserData::Effect(e) => super::ef::remove_user_data(self.game, e),
            UserData::Sound(s) => crate::lb::sound_obj::free(self.game, s),
            UserData::Text(t) => self.game.sis.free_text(t),
            UserData::SisCamera(g) => self.game.sis.forget_camera(g),
            // `mn_8022EB04`: the heap byte goes back.
            UserData::NameTagSlot(slot) => {
                let addr = self.game.hud.nametags.user_data_addrs[usize::from(slot)];
                crate::hsd::heap::mem_free(addr);
            }
            UserData::BgFlash => self.game.bg_flash_free(),
            // `HSD_Free`.
            UserData::StadiumScreen(which) => {
                let s = self.game.gr.stadium.screens[usize::from(which)].as_ref();
                crate::hsd::heap::mem_free(s.map_or(0, |s| s.addr));
            }
            UserData::IzumiReflection => {
                let r = self.game.gr.izumi.reflection.as_ref();
                crate::hsd::heap::mem_free(r.map_or(0, |r| r.addr));
            }
            // `ifMagnify_802FC3BC` does nothing.
            UserData::MagnifyPlayer(_) => {}
            UserData::StockIcons(_) | UserData::ArrowSlot(_) => {
                unreachable!("user data set without a kind isn't released")
            }
            other => panic!("removing {other:?} isn't ported"),
        }
    }

    fn remove_obj(&mut self, _kind: u8, obj: Obj) {
        match obj {
            Obj::JObj(j) => crate::hsd::jobj::remove_all(&mut self.game.objs, Some(j)),
            Obj::Fog(f) => crate::hsd::fog::unref(&mut self.game.objs, Some(f)),
            Obj::CObj(c) => crate::hsd::cobj::unref(&mut self.game.objs, Some(c)),
            Obj::SObj(s) => crate::hsd::sobj::free_all(&mut self.game.objs.sobjs, s),
            other => panic!("releasing a GObj's {other:?} needs its port"),
        }
    }
}
