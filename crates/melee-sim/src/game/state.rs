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
    /// A main menu list's user data (`MainMenuData`), by its block's address.
    MainMenuList(u32),
    /// The main menu panel's user data (`MainMenuPanelData`), by its block's address.
    MainMenuPanel(u32),
    /// The movies' list's user data, by its block's address.
    GalleryList(u32),
    /// A movie's cursor's user data, by its block's address.
    GalleryCursor(u32),
    /// The rules' list's user data (`struct mn_802307F8_t`), by its block's address.
    RulesList(u32),
    /// The extra rules' list's user data (`MenuRulesPlusData`), by its block's address.
    RulePlusList(u32),
    /// The item switch's user data (`MnItemSwData`), by its block's address.
    ItemSwitch(u32),
    /// The random stage switch's user data (`MnStageSwData`), by its block's address.
    StageSwitch(u32),
    /// The names' list's user data (`MnName_GObj`'s 0x44 bytes), by its block's address.
    NameList(u32),
    /// The new name's keyboard's user data (`NameNewEntry`), by its block's address.
    NameNew(u32),
    /// A glyph's variants' user data (`GlyphVariantEntry`), by its block's address.
    GlyphVariants(u32),
    /// The select screens' blocks: a hand, a coin and a name tag by door
    /// (`mnCharSel_804A0BC0`, `mnCharSel_804A0BD0`, `mnCharSel_803F0E8C`), and the stage
    /// select's name, preview and hover frame by block.
    CssCursor(u8),
    CssToken(u8),
    CssTag(u8),
    SssName(u32),
    SssPreview(u32),
    SssHover(u32),
    /// An Options screen's `Menu` user data (sound, screen, language), by its block's address.
    OptionMenu(u32),
    /// The rumble screen's user data (`MnVibrationData`), by its block's address.
    Vibration(u32),
    /// The erasing screen's user data (`MnDataDelGObjUserData`), by its block's address.
    DataDel(u32),
    /// The special messages' user data (`MnInfoData`), by its block's address.
    Info(u32),
    /// The other records' user data (`MnCountData`), by its block's address.
    Count(u32),
    /// The VS records' user data (`Diagram`), by its block's address.
    Diagram(u32),
    /// A VS records detail's user data (`mnDiagram_PopupData`), by its block's address.
    DiagramPopup(u32),
    /// The VS records' statistics' user data (`Diagram2`), by its block's address.
    Diagram2(u32),
    /// The VS records' rankings' user data (`Diagram3`), by its block's address.
    Diagram3(u32),
    /// The sound test's user data (`soundtest_user_data`), by its block's address.
    SoundTest(u32),
    /// The background flash's overlay ([`crate::lb::bgflash::BgFlash::user`]).
    BgFlash,
    /// A trophy screen's block (0x58 bytes of `HSD_MemAlloc`, freed with `HSD_Free`), by its
    /// address ([`super::ty::lottery::Lottery::user`]).
    Toy(u32),
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

    fn user_data_remover(u: &UserData) -> Option<u32> {
        Some(match u {
            // `Fighter_Unload_8006DABC`, `Item_OnUserDataRemove`, the links' `it_802A2474`,
            // grlib's `mem_free`, `efLib_remove_user_data`, `lbAudioAx_ObjFree`,
            // `lbBgFlash_Free`.
            UserData::Fighter(_) => 0x8006_DABC,
            UserData::Item(_) => 0x8026_AE10,
            UserData::ItemLink(_) => 0x802A_2474,
            UserData::Ground(_) => 0x801C_0478,
            UserData::Effect(_) => 0x8005_BB40,
            UserData::Sound(_) => 0x8002_63B4,
            UserData::BgFlash => 0x8002_19E4,
            // The menus' and two stages' blocks, freed with `HSD_Free` (their
            // `GObj_InitUserData` calls).
            UserData::CssCursor(_)
            | UserData::CssToken(_)
            | UserData::CssTag(_)
            | UserData::SssName(_)
            | UserData::SssPreview(_)
            | UserData::SssHover(_)
            | UserData::GalleryList(_)
            | UserData::GalleryCursor(_)
            | UserData::RulesList(_)
            | UserData::RulePlusList(_)
            | UserData::ItemSwitch(_)
            | UserData::StageSwitch(_)
            | UserData::NameList(_)
            | UserData::NameNew(_)
            | UserData::OptionMenu(_)
            | UserData::Vibration(_)
            | UserData::DataDel(_)
            | UserData::StadiumScreen(_)
            | UserData::IzumiReflection => 0x8037_F1B0,
            // The main menu's lists, panel and player tags (`mn_8022EB04`), a glyph's
            // variants (`fn_8023D0F8`).
            UserData::MainMenuList(_) | UserData::MainMenuPanel(_) | UserData::NameTagSlot(_) => {
                0x8022_EB04
            }
            UserData::GlyphVariants(_) => 0x8023_D0F8,
            // `Toy_RemoveUserData`.
            UserData::Toy(_) => crate::game::ty::lottery::TOY_REMOVE_USER_DATA,
            _ => return None,
        })
    }
}

/// What the simulation hands the presentation: sounds and controller rumble, which the
/// original passes to the audio and pad drivers mid-tick.
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
pub enum Event {
    /// The save's list wrote the card: the save file as the card now holds it, a `.gci`
    /// (its directory entry, then its blocks), for the frontend to keep (milestone 5's design
    /// calls: the frontend writes it whole; a tick never writes it itself).
    SaveWritten(Vec<u8>),
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
    /// Where the magnifier's bubbles but the first copy their pixels (`image_descs`'
    /// buffers), where the allocator model gives them: the heap's image takes them as a render
    /// target once a bubble is copied ([`crate::hsd::image::Src::RenderTarget`]).
    pub magnify_buffers: [(u32, u32); 6],
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
    /// card's data, as the boot resets them and the memory card loads them. Boxed: inline, its
    /// 0x10A30 bytes made a `Game` half again as large, and every `Game` moved or copied on the
    /// stack with it.
    pub game_data: Box<super::structs::gmm_x0>,
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
    /// The attract match's start data and camera (`gmopeningmode.c`, `gm_181A.c`).
    pub attract: super::gm::attract::Attract,
    /// How to Play's and the Special Movie's sprites (`gmhowto.c`, `gmomake15.c`).
    pub movies: super::gm::howto::Movies,
    /// The main menu's mode's enter and exit data (`gmmenumode.c`).
    pub menu_mode: super::gm::menu::MenuMode,
    /// The main menu's statics (`mnmain.c`) and its lists' user data.
    pub menu: super::mn::main::MainMenu,
    /// The Data menu's movies' statics and user data (`mngallery.c`).
    pub gallery: super::mn::gallery::Gallery,
    /// The sound test's statics and user data (`mnsoundtest.c`).
    pub sound_test: super::mn::sound_test::SoundTest,
    /// The VS menu's rules' and names' statics and user data (`mnmainrule.c` to
    /// `mnnamenew.c`).
    pub vs_menus: super::mn::rules::RulesMenus,
    /// The character select's statics (`mncharsel.c`), the stage select's (`mnstagesel.c`)
    /// and VS mode's scene data (`gmvsmelee.c`).
    pub charsel: super::mn::charsel::CharSel,
    pub stagesel: super::mn::stagesel::StageSel,
    pub vs_melee: super::gm::vs_melee::VsMelee,
    /// The results screen's statics (`gmresult.c`, `gm_1798.c`).
    pub results: super::gm::results_scene::Results,
    /// The Options menus' statics and user data (`mnvibration.c`, `mnsound.c`,
    /// `mndeflicker.c`, `mnlanguage.c`, `mndatadel.c`).
    pub options: super::mn::options::Options,
    /// The Data menu's screens' statics and user data (`mninfo.c` and the others
    /// [`super::mn::data`] lists).
    pub data_menus: super::mn::data::DataMenus,
    /// The special messages' interface's statics and its queue of notices (`ifprize.c`,
    /// `if_Scene_Prize_EnterData`, `gm_8049E558`).
    pub prize: super::hud::prize::Prize,

    /// The progressive scan prompt's statics (`gmprogressive.c`, `gmprogressivemode.c`).
    pub prog_scan: super::gm::progressive::ProgScan,
    /// The challenger approach's screen's statics and its data (`gmapproach.c`'s
    /// `gm_80480D98`, `gmVsMelee_ApproachData`).
    pub approach: super::gm::approach::Approach,
    /// The game's side of the memory card (`lbcardgame.c`).
    pub card_game: crate::lb::cardgame::CardGame,
    /// What the game keeps of the card's task lists (`lbcardnew.c`).
    pub card_tasks: crate::lb::save::CardTasks,
    /// Camera Mode's snapshots' state (`lbsnap.c`).
    pub snap: crate::lb::snap::Snap,
    /// Camera Mode's statics: the card check, the match's menu and the frame's copy
    /// (`gm_1A33.c`, `gmcamera.c`, `cmsnap.c`).
    pub camera_mode: super::gm::camera_mode::CameraMode,
    /// The snapshot being written (`lbsnap.c`'s writing side).
    pub snap_write: crate::lb::snap_write::SnapWrite,
    /// The modes' settings that live outside the game's data: the training mode's saved
    /// players (`gm_80473814`), the Home-Run Contest's (`gmHomeRun_VsModeData`) and the
    /// trophy gallery's (`Toy_804A284C`, in [`Game::toys`]).
    pub modes: super::gm::mode_init::ModeStatics,
    /// The trophies' statics (`toy.c`, `tylist.c`).
    pub toys: super::ty::Toys,
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
                let (game, data) = (&*self.game, self.data);
                crate::hsd::export::with_image(|img| super::export::ground(game, data, img, g));
                crate::hsd::heap::mem_free(self.game.grounds.addr(g.0));
                self.game.grounds.free(g.0);
            }
            UserData::Item(i) => {
                // The block holds the item as it was when it goes back (docs/engine.md, a
                // session's heap leftovers).
                let (game, data) = (&*self.game, self.data);
                crate::hsd::export::with_image(|img| super::export::item(game, data, img, i));
                super::it::item::free_user_data(self.game, i);
            }
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
            // `mn_8022EB04`: `HSD_Free`.
            UserData::MainMenuList(addr) => {
                crate::hsd::heap::mem_free(addr);
                self.game.menu.lists.remove(&addr);
            }
            UserData::MainMenuPanel(addr) => {
                crate::hsd::heap::mem_free(addr);
                self.game.menu.panels.remove(&addr);
            }
            // `HSD_Free`.
            UserData::GalleryList(addr) => {
                crate::hsd::heap::mem_free(addr);
                self.game.gallery.data.remove(&addr);
            }
            UserData::GalleryCursor(addr) => {
                crate::hsd::heap::mem_free(addr);
                self.game.gallery.cursors.remove(&addr);
            }
            UserData::OptionMenu(addr) => {
                crate::hsd::heap::mem_free(addr);
                self.game.options.menus.remove(&addr);
            }
            UserData::Info(addr) => {
                crate::hsd::heap::mem_free(addr);
                self.game.data_menus.info.data.remove(&addr);
            }
            UserData::Count(addr) => {
                crate::hsd::heap::mem_free(addr);
                let c = &mut self.game.data_menus.count;
                let d = c.data.remove(&addr);
                c.left = Some((addr, d.and_then(|d| d.title)));
            }
            UserData::Diagram(addr) => {
                crate::hsd::heap::mem_free(addr);
                self.game.data_menus.diagram.data.remove(&addr);
            }
            UserData::DiagramPopup(addr) => self.diagram_popup_cleanup(addr),
            UserData::Diagram2(addr) => self.d2_free(addr),
            UserData::Diagram3(addr) => self.d3_free(addr),

            UserData::Vibration(addr) => {
                crate::hsd::heap::mem_free(addr);
                let v = &mut self.game.options.vibration;
                let d = v.data.remove(&addr);
                v.left = Some((addr, d.and_then(|d| d.title)));
            }
            UserData::DataDel(addr) => {
                crate::hsd::heap::mem_free(addr);
                self.game.options.datadel.data.remove(&addr);
            }
            // `Toy_RemoveUserData`: `HSD_Free`.
            UserData::Toy(addr) => {
                crate::hsd::heap::mem_free(addr);
                self.game.toys.lottery.user.remove(&addr);
            }
            UserData::SoundTest(addr) => {
                crate::hsd::heap::mem_free(addr);
                self.game.sound_test.data.remove(&addr);
            }
            // `HSD_Free`.
            UserData::CssCursor(i) => {
                let c = &mut self.game.charsel.cursors[usize::from(i)];
                let addr = c.take().expect("a hand's block").addr;
                crate::hsd::heap::mem_free(addr);
                self.game.charsel.left_hands[usize::from(i)] = addr;
            }
            UserData::CssToken(i) => {
                let c = self.game.charsel.tokens[usize::from(i)]
                    .take()
                    .expect("a coin's block");
                crate::hsd::heap::mem_free(c.addr);
                self.game.charsel.left_tokens[usize::from(i)] = Some((c.addr, c.held_by));
            }
            UserData::CssTag(i) => {
                let c = &mut self.game.charsel.tags[usize::from(i)];
                let addr = c.take().expect("a tag's block").addr;
                crate::hsd::heap::mem_free(addr);
                self.game.charsel.left_tags[usize::from(i)] = addr;
            }
            UserData::SssName(addr) => {
                crate::hsd::heap::mem_free(addr);
                self.game.stagesel.names.remove(&addr);
            }
            UserData::SssPreview(addr) => {
                crate::hsd::heap::mem_free(addr);
                self.game.stagesel.previews.remove(&addr);
            }
            UserData::SssHover(addr) => {
                crate::hsd::heap::mem_free(addr);
                self.game.stagesel.hovers.remove(&addr);
            }
            // `HSD_Free`.
            UserData::StadiumScreen(which) => {
                let s = self.game.gr.stadium.screens[usize::from(which)].as_ref();
                crate::hsd::heap::mem_free(s.map_or(0, |s| s.addr));
            }
            UserData::IzumiReflection => {
                let r = self.game.gr.izumi.reflection.as_ref();
                crate::hsd::heap::mem_free(r.map_or(0, |r| r.addr));
            }
            UserData::RulesList(_)
            | UserData::RulePlusList(_)
            | UserData::ItemSwitch(_)
            | UserData::StageSwitch(_)
            | UserData::NameList(_)
            | UserData::NameNew(_)
            | UserData::GlyphVariants(_) => {
                self.rules_release(data);
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
            Obj::LObj(l) => crate::hsd::lobj::remove_all(&mut self.game.objs, Some(l)),
        }
    }
}
