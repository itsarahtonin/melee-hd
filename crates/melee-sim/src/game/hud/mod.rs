//! The in-match HUD and the text objects a match creates (`melee/if`): the damage
//! percentages and stock icons, the timer, the Ready/GO!/GAME! announcements, the name tags
//! and the rest of what a VS match puts on screen, and the debug text screens every scene
//! sets up. Their GObjs, joints, animations and timers live in the game's state and run in
//! the scheduler's lists, so they are simulation; only their drawing is presentation, and it
//! becomes draw commands ([`Draw`]).

pub mod all;
pub mod announce;
pub mod debug_card;
pub mod debug_items;
pub mod debug_menu;
pub mod debug_sound;
pub mod debug_tables;
pub mod devtext;
pub mod magnify;
pub mod nametag;
pub mod prize;
pub mod small;
pub mod status;
pub mod stock;
pub mod text;
pub mod time;
pub mod types;

use melee_data::archive::{ArchiveId, DataRef};

use super::handles::CodeRef;
use super::structs::_GXColor;
use crate::hsd::gobj::GObjId;
use types::{
    Announcement, CharsLeftTally, DamageDisplays, HudLayout, HudStocks, HudTimer, JoinerTally,
    KoMarks, OffscreenArrows, ScoreDisplays, StockUserData,
};

/// `JOBJ_HIDDEN`.
pub const HIDDEN: u32 = 1 << 4;
/// `HSD_GObj_JObjCallback`, the render callback that draws a GObj's joints.
pub const JOBJ_RENDER: u32 = 0x8039_1070;
/// `HSD_GObj_803910D8`, a camera GObj's render callback.
pub const CAMERA_RENDER: u32 = 0x8039_10D8;
/// `HSD_GObj_LObjCallback`, a light GObj's render callback.
pub const LIGHT_RENDER: u32 = 0x8039_1044;

/// What a HUD render callback asks the display pass to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub enum Draw {
    /// `HSD_GObj_JObjCallback`: draw the GObj's joints in this pass.
    JObjs(GObjId, i32),
    /// `HSD_CObjSetCurrent` on the GObj's camera, then `HSD_GObj_80390ED0` with this render
    /// mask, then `HSD_CObjEndCurrent`.
    Camera(GObjId, u32),
}

/// A `DynamicModelDesc` copied into a static (`ifTime_match_timer_models`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct ModelDesc {
    pub joint: Option<DataRef>,
    pub anims: Option<DataRef>,
    pub matanims: Option<DataRef>,
    pub shapeanims: Option<DataRef>,
}

/// The HUD's globals, one field per unit's statics.
#[derive(melee_derive::CloneFrom, Debug, melee_derive::StateEq)]
pub struct Hud {
    /// The debug text screens (`textdraw.c`, `textlib.c`).
    pub(in crate::game::hud) devtext: devtext::DevTexts,
    /// The debug menus' variables (`if_2FFC.c`'s `un_803FA258` and the words their tables
    /// name).
    pub(in crate::game::hud) debug: debug_menu::DebugVars,
    /// `ifAll_804A0FD8`: the HUD's camera and light and the places its parts go.
    pub(in crate::game::hud) all: HudLayout,
    /// `ifall.c`'s `hidden`.
    pub(in crate::game::hud) hidden: bool,
    /// `IfAll.dat` (`ifAll_GetArchive`'s static).
    pub(in crate::game::hud) archive: Option<ArchiveId>,
    /// `ifStatus_HudInfo`: each player's damage display.
    pub(in crate::game::hud) status: DamageDisplays,
    /// `ifStatus_804D6D60`: how many players the displays are laid out for.
    pub(in crate::game::hud) status_count: u8,
    /// `ifStatus_804D6D61`: which way the next exploding digit flies.
    pub(in crate::game::hud) status_flip: i8,
    /// `ifStatus_803F9628`: the announcements (GAME!, TIME!, Ready GO! and the rest).
    pub(in crate::game::hud) announcements: [Announcement; 8],
    /// `scinfstc_models`: the marks a KO puts by a player's display.
    pub(in crate::game::hud) ko_marks: KoMarks,
    /// `ifTime_data` and `ifTime_match_timer_models`.
    pub(in crate::game::hud) time: HudTimer,
    pub(in crate::game::hud) timer_models: ModelDesc,
    /// `ifStock_804A1378` and `ifStock_804A157C`: each player's stock icons.
    pub(in crate::game::hud) stock: HudStocks,
    pub(in crate::game::hud) stock_users: [StockUserData; 6],
    /// `ifStock_804A1774`, `ifStock_804A1A8C` and `ifStock_804A1ACC`: the single-player
    /// tallies.
    pub(in crate::game::hud) stock_1774: JoinerTally,
    pub(in crate::game::hud) stock_1a8c: [Option<GObjId>; 16],
    pub(in crate::game::hud) stock_1acc: CharsLeftTally,
    /// The players' tags.
    pub(in crate::game::hud) nametags: nametag::NameTags,
    /// The hazard sign.
    pub(in crate::game::hud) hazard: small::Hazard,
    /// The coin counter.
    pub(in crate::game::hud) coins: small::Coins,
    /// `un_804A1F10` and `un_804D6D90`: the off-screen arrows, and whether they're hidden.
    pub(in crate::game::hud) arrows: OffscreenArrows,
    pub(in crate::game::hud) arrows_hidden: i32,
    /// `un_804A1F58`: the scores.
    pub(in crate::game::hud) scores: ScoreDisplays,
}

impl Default for Hud {
    fn default() -> Self {
        let e = |proc: u32, sfx: i32, frame: u8| Announcement {
            proc: Some(CodeRef(proc)),
            default_sfx: sfx,
            sfx_frame: frame,
            ..Announcement::default()
        };
        let (wait, once) = (announce::WAIT, announce::ONCE);
        Hud {
            devtext: Default::default(),
            debug: Default::default(),
            all: Default::default(),
            hidden: false,
            archive: None,
            status: Default::default(),
            status_count: 0,
            status_flip: 0,
            announcements: [
                e(wait, 0x7C860, 8),
                e(once, 0xC351, 0),
                e(wait, 0x7C85E, 0),
                e(once, 0x7C85D, 0),
                e(once, 0x7C857, 10),
                e(wait, 0x7C855, 8),
                e(wait, 0x9C48, 0),
                e(wait, 0x9C46, 0),
            ],
            ko_marks: Default::default(),
            time: Default::default(),
            timer_models: Default::default(),
            stock: Default::default(),
            stock_users: Default::default(),
            stock_1774: Default::default(),
            stock_1a8c: [None; 16],
            stock_1acc: Default::default(),
            nametags: Default::default(),
            hazard: Default::default(),
            coins: Default::default(),
            arrows: Default::default(),
            arrows_hidden: 0,
            scores: Default::default(),
        }
    }
}

/// What the console's HUD carries from the matches before into a live match's
/// (`live::Carried`): the off-screen arrows' colors and players (`un_802FE260` resets which
/// are shown, not these; an arrow shows only once `un_802FD928` colors it).
#[derive(Clone, Debug)]
pub struct Carried {
    arrow_colors: [_GXColor; 4],
    arrow_players: [u8; 4],
}

impl Hud {
    /// The debug text screens (`textdraw.c`, `textlib.c`), which `db` and the toys write to.
    #[inline]
    pub fn devtext(&self) -> &devtext::DevTexts {
        &self.devtext
    }

    /// The debug text screens, to write ([`Hud::devtext`]).
    #[inline]
    pub fn devtext_mut(&mut self) -> &mut devtext::DevTexts {
        &mut self.devtext
    }

    /// The address of name tag `slot`'s GObj's user data on the console (the heap's image and
    /// the user data's lookup by address).
    #[inline]
    pub fn nametag_user_data_addr(&self, slot: u8) -> u32 {
        self.nametags.user_data_addrs[usize::from(slot)]
    }

    /// What the HUD carries now ([`Carried`]).
    pub fn carried(&self) -> Carried {
        Carried {
            arrow_colors: self.arrows.colors.clone(),
            arrow_players: self.arrows.players,
        }
    }

    /// A live match takes what the console's HUD carried (`live`'s `Carried::put`).
    pub fn take_carried(&mut self, at: &Carried) {
        self.arrows.colors.clone_from(&at.arrow_colors);
        self.arrows.players = at.arrow_players;
    }

    /// The menus' session, taking back a live match's scene, takes the HUD's values the next
    /// match's setup leaves (`live`'s `Game::take_match_statics`): the off-screen arrows'
    /// slots and sound (`un_802FE260` resets which are shown), the announcements' callbacks,
    /// lines and marks; their models and GObjs stay the menus'.
    pub fn take_match(&mut self, live: &Hud) {
        let (a, b) = (&mut self.arrows, &live.arrows);
        (a.shown, a.sides, a.silent, a.sounding, a.sound_timer) =
            (b.shown, b.sides, b.silent, b.sounding, b.sound_timer);
        for (e, l) in self.announcements.iter_mut().zip(&live.announcements) {
            let (gobj, model) = (e.gobj, e.model);
            e.clone_from(l);
            (e.gobj, e.model) = (gobj, model);
        }
    }

    /// The HUD's state laid open, for the oracle's tools (the comparer), leaf-check and tests;
    /// game code goes through the HUD's functions.
    pub fn raw_parts(&self) -> Parts<'_> {
        Parts {
            devtext: &self.devtext,
            debug: &self.debug,
            all: &self.all,
            hidden: &self.hidden,
            archive: &self.archive,
            status: &self.status,
            status_count: &self.status_count,
            status_flip: &self.status_flip,
            announcements: &self.announcements,
            ko_marks: &self.ko_marks,
            time: &self.time,
            timer_models: &self.timer_models,
            stock: &self.stock,
            stock_users: &self.stock_users,
            stock_1774: &self.stock_1774,
            stock_1a8c: &self.stock_1a8c,
            stock_1acc: &self.stock_1acc,
            nametags: &self.nametags,
            hazard: &self.hazard,
            coins: &self.coins,
            arrows: &self.arrows,
            arrows_hidden: &self.arrows_hidden,
            scores: &self.scores,
        }
    }

    /// The HUD's state laid open to write, for the oracle's importer, leaf-check and tests
    /// ([`Hud::raw_parts`]).
    pub fn raw_parts_mut(&mut self) -> PartsMut<'_> {
        PartsMut {
            devtext: &mut self.devtext,
            debug: &mut self.debug,
            all: &mut self.all,
            hidden: &mut self.hidden,
            archive: &mut self.archive,
            status: &mut self.status,
            status_count: &mut self.status_count,
            status_flip: &mut self.status_flip,
            announcements: &mut self.announcements,
            ko_marks: &mut self.ko_marks,
            time: &mut self.time,
            timer_models: &mut self.timer_models,
            stock: &mut self.stock,
            stock_users: &mut self.stock_users,
            stock_1774: &mut self.stock_1774,
            stock_1a8c: &mut self.stock_1a8c,
            stock_1acc: &mut self.stock_1acc,
            nametags: &mut self.nametags,
            hazard: &mut self.hazard,
            coins: &mut self.coins,
            arrows: &mut self.arrows,
            arrows_hidden: &mut self.arrows_hidden,
            scores: &mut self.scores,
        }
    }
}

/// [`Hud::raw_parts`]: each of [`Hud`]'s fields, as its docs say.
pub struct Parts<'a> {
    pub devtext: &'a devtext::DevTexts,
    pub debug: &'a debug_menu::DebugVars,
    pub all: &'a HudLayout,
    pub hidden: &'a bool,
    pub archive: &'a Option<ArchiveId>,
    pub status: &'a DamageDisplays,
    pub status_count: &'a u8,
    pub status_flip: &'a i8,
    pub announcements: &'a [Announcement; 8],
    pub ko_marks: &'a KoMarks,
    pub time: &'a HudTimer,
    pub timer_models: &'a ModelDesc,
    pub stock: &'a HudStocks,
    pub stock_users: &'a [StockUserData; 6],
    pub stock_1774: &'a JoinerTally,
    pub stock_1a8c: &'a [Option<GObjId>; 16],
    pub stock_1acc: &'a CharsLeftTally,
    pub nametags: &'a nametag::NameTags,
    pub hazard: &'a small::Hazard,
    pub coins: &'a small::Coins,
    pub arrows: &'a OffscreenArrows,
    pub arrows_hidden: &'a i32,
    pub scores: &'a ScoreDisplays,
}

/// [`Hud::raw_parts_mut`].
pub struct PartsMut<'a> {
    pub devtext: &'a mut devtext::DevTexts,
    pub debug: &'a mut debug_menu::DebugVars,
    pub all: &'a mut HudLayout,
    pub hidden: &'a mut bool,
    pub archive: &'a mut Option<ArchiveId>,
    pub status: &'a mut DamageDisplays,
    pub status_count: &'a mut u8,
    pub status_flip: &'a mut i8,
    pub announcements: &'a mut [Announcement; 8],
    pub ko_marks: &'a mut KoMarks,
    pub time: &'a mut HudTimer,
    pub timer_models: &'a mut ModelDesc,
    pub stock: &'a mut HudStocks,
    pub stock_users: &'a mut [StockUserData; 6],
    pub stock_1774: &'a mut JoinerTally,
    pub stock_1a8c: &'a mut [Option<GObjId>; 16],
    pub stock_1acc: &'a mut CharsLeftTally,
    pub nametags: &'a mut nametag::NameTags,
    pub hazard: &'a mut small::Hazard,
    pub coins: &'a mut small::Coins,
    pub arrows: &'a mut OffscreenArrows,
    pub arrows_hidden: &'a mut i32,
    pub scores: &'a mut ScoreDisplays,
}

/// Runs a HUD GObj's process, if `proc` is one; whether it was.
#[melee_callbacks::callbacks(Proc)]
pub fn run_proc(r: &mut super::state::Running, gobj: GObjId, proc: u32) -> bool {
    match proc {
        status::PROC_DAMAGE => r.damage_proc(gobj),
        status::PROC_DIGITS => r.damage_digits_proc(gobj),
        announce::ONCE => r.announcement_once(gobj),
        announce::WAIT => r.announcement_wait(gobj),
        announce::MARK_FADE => r.ko_mark_proc(gobj),
        announce::MARK_FADE_2 => r.ko_mark_proc_2(gobj),
        announce::MARK_SPAWN_2 => r.ko_mark_proc_spawning(gobj),
        time::PROC_TIMER => r.timer_proc(gobj),
        time::PROC_COUNTDOWN => r.countdown_proc(gobj),
        stock::PROC_ICONS => r.stock_proc(gobj),
        stock::PROC_TALLY => r.stock_tally_proc(gobj),
        stock::PROC_TALLY_2 => r.stock_tally_proc_2(gobj),
        stock::PROC_TALLY_PLACE => r.tally_place_proc(gobj),
        stock::PROC_JOINERS_PLACE => r.joiners_place_proc(gobj),
        stock::PROC_TARGETS => r.target_icons_proc(gobj),
        stock::PROC_COUNTER => r.crowd_counter_proc(gobj),
        nametag::PROC => r.nametag_proc(gobj),
        small::HAZARD_PROC => r.hazard_proc(gobj),
        small::COIN_PROC => r.coin_proc(gobj),
        small::ARROW_PROC => r.arrow_proc(gobj),
        small::SCORE_PROC => r.score_proc(gobj),
        _ => return false,
    }
    true
}
