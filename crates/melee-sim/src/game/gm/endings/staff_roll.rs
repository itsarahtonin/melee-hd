//! The staff roll (`gmstaffroll.c`, scene 0x2B): the credits' names, each a joint of
//! `GmStRoll.dat`'s name models flying past the camera with two texts drawn on it (the name and
//! the role, from `SdStRoll.dat`'s pages), sorted by depth each tick; and the shooting game over
//! them: the stick aims a cursor (the cursor's model at the stick's place, the stick's dead
//! zone 4), A fires (the gun's animation and a shot's sound), and a name under the cursor is hit
//! (its text, or for the first seven its title's material, turns red, a particle bursts on it
//! and the gun kicks), B pauses the names with the name's details shown (and A, B, Start or Z
//! resumes), Start runs the roll six times as fast, the details (six texts) show the last name
//! aimed at, and at tick 4741 the names' count of hits (`%d ＨＩＴ`). The roll's songs fade from
//! tick 4821, the background flashes at 4881 and the scene ends at 4941.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::super::handles::CodeRef;
use super::super::super::render::{FOG_CALLBACK, JOBJ_CALLBACK, LOBJ_CALLBACK};
use super::super::super::state::{OBJ_CAMERA, OBJ_FOG, OBJ_JOBJ, OBJ_LIGHT, Obj, Running};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::sis::TextId;
use crate::mtx::Mtx34;
use crate::vec::Vec3;

/// `gm_Scene_StaffRoll_OnEnter`, `gm_Scene_StaffRoll_OnFrame`.
pub const ON_ENTER: CodeRef = CodeRef(0x801A_C6D8);
pub const ON_FRAME: CodeRef = CodeRef(0x801A_A7C4);

/// The scene's processes and render callbacks.
pub const PROC_CAMERA: u32 = 0x801A_A7F8;
pub const RENDER_NAMES: u32 = 0x801A_A854;
pub const RENDER_DETAILS: u32 = 0x801A_AA28;
pub const PROC_LIGHT: u32 = 0x801A_AABC;
pub const PROC_BG: u32 = 0x801A_AB18;
pub const PROC_NAMES: u32 = 0x801A_AB74;
pub const PROC_MAIN: u32 = 0x801A_B200;
pub const PROC_FOG: u32 = 0x801A_C67C;

/// The names, `gm_803DBFD8`'s count.
pub const NAMES: usize = 198;

/// The roll's ticks: the details give way to the hits' count (`0x1285`), the songs fade from
/// `0x12D5`, the background flashes at `0x1311`, the scene ends at `0x134D`.
const RESULT_AT: i32 = 0x1285;
const FADE_AT: i32 = 0x12D5;
const FLASH_AT: i32 = 0x1311;
const END_AT: i32 = 0x134D;

/// A name's check (`StaffEntryData::check`), with its argument.
#[derive(Clone, Copy)]
enum Check {
    /// `gm_801AA644`: the save's feature bit 4 (`gmMainLib_8015EDD4`).
    Feature,
    /// `gm_801AA664`: the character unlocked.
    Character(u8),
    /// `gm_801AA688`: Marth or Roy unlocked.
    MarsOrEmblem,
    /// `gm_801AA6D8`: the trophy held.
    Trophy(u16),
    /// `gm_801AA6FC`: trophy 0x10B, 0x10A or 0x109 held.
    Trophies3,
    /// `gm_801AA774`: trophy 0x107 or 0x108 held.
    Trophies2,
}

/// `StaffEntryData`: the details' column (`x0`), the check, the page for a name whose check
/// fails (`alt_page`) and the name's widths in Japanese and English (`x11`).
#[derive(Clone, Copy)]
struct Entry {
    column: u8,
    check: Option<(Check, i32)>,
    width: [u8; 2],
}

const fn e(column: u8, w0: u8, w1: u8) -> Entry {
    Entry {
        column,
        check: None,
        width: [w0, w1],
    }
}

const fn c(column: u8, check: Check, page: i32, w0: u8, w1: u8) -> Entry {
    Entry {
        column,
        check: Some((check, page)),
        width: [w0, w1],
    }
}

use Check::*;

/// `gm_803DBFD8`. The first check-holding entry, `gm_801AA644`'s, has no `has_check`: it
/// changes only the details' pages.
const ENTRIES: [Entry; NAMES] = [
    e(2, 80, 80), e(3, 80, 80), e(2, 80, 80), e(2, 80, 80), e(3, 80, 80), e(2, 80, 80),
    e(2, 80, 80), e(1, 80, 116), e(1, 80, 95), e(1, 80, 102), e(2, 80, 80), e(2, 80, 102),
    e(1, 80, 95), e(2, 80, 123), e(1, 80, 116), e(1, 96, 116), e(2, 80, 102), e(2, 80, 123),
    e(2, 80, 123), e(1, 80, 123), e(3, 80, 130), e(1, 112, 102), e(2, 80, 123),
    Entry { column: 2, check: None, width: [80, 109] },
    e(1, 80, 123), e(1, 80, 123), e(2, 80, 109), e(1, 80, 123), e(1, 80, 123), e(1, 96, 88),
    e(1, 80, 102), e(3, 80, 123), e(1, 80, 102), e(1, 80, 109), e(1, 80, 95), e(1, 80, 95),
    e(1, 80, 116), e(1, 80, 80), e(1, 80, 95), e(1, 80, 95), e(1, 80, 102), e(1, 142, 104),
    e(2, 80, 88), e(2, 80, 116), e(1, 80, 88), e(1, 96, 81), e(1, 80, 81), e(1, 80, 102),
    e(1, 96, 102), e(1, 96, 95), e(1, 80, 88), e(1, 80, 88), e(1, 80, 102), e(1, 80, 88),
    e(1, 80, 109), e(1, 96, 95), e(1, 80, 95), e(1, 96, 95), e(1, 80, 102), e(1, 80, 130),
    e(1, 80, 102), e(1, 80, 116), e(1, 80, 130), e(1, 80, 80), e(1, 80, 95), e(2, 224, 102),
    e(1, 80, 95), e(1, 80, 116), e(1, 80, 81), e(1, 80, 102), e(1, 80, 102), e(2, 80, 95),
    e(2, 80, 102), e(1, 80, 95), e(1, 160, 109), e(1, 176, 116), e(1, 80, 109), e(1, 80, 95),
    e(1, 96, 95), e(1, 80, 116), e(1, 80, 81), e(1, 80, 88),
    c(1, Character(15), 2976, 96, 102), e(1, 128, 80), e(1, 80, 88),
    c(1, Character(9), 2980, 80, 123), c(1, Character(10), 2984, 80, 130), e(1, 80, 109),
    c(1, Character(23), 2988, 80, 88), c(1, Character(24), 2992, 128, 102),
    c(1, Character(21), 2996, 96, 109), c(1, Character(25), 3000, 80, 116), e(1, 80, 81),
    e(1, 80, 88), e(1, 80, 80), e(1, 80, 95), e(1, 96, 80), e(2, 204, 148), e(2, 144, 102),
    e(2, 128, 83), e(2, 112, 81), e(2, 144, 88), e(2, 128, 81), e(2, 112, 81), e(1, 80, 123),
    e(3, 80, 80), e(1, 80, 109), e(1, 80, 88), e(3, 80, 109), e(1, 80, 102), e(1, 128, 80),
    e(1, 80, 80), e(2, 80, 95), e(1, 80, 102), e(1, 80, 109), e(1, 80, 109), e(1, 80, 116),
    e(2, 80, 81), e(1, 80, 116), e(1, 80, 102), e(3, 80, 116), e(1, 80, 102), e(1, 80, 102),
    e(2, 80, 81), e(2, 80, 80), e(1, 80, 102), e(1, 80, 102), e(1, 80, 88), e(1, 80, 116),
    e(1, 80, 95), e(1, 96, 116), e(1, 80, 102), e(1, 80, 88), e(1, 80, 109), e(2, 80, 130),
    c(1, Character(25), 3004, 80, 116), c(2, MarsOrEmblem, 3008, 80, 102),
    c(2, MarsOrEmblem, 3012, 80, 116), c(2, MarsOrEmblem, 3016, 80, 123),
    c(2, MarsOrEmblem, 3020, 80, 95), c(1, MarsOrEmblem, 3024, 96, 123), e(1, 156, 146),
    e(1, 96, 102), e(1, 80, 130), e(1, 80, 81), e(2, 236, 111), e(1, 80, 88), e(1, 80, 88),
    e(1, 80, 102), e(1, 80, 109), e(2, 80, 102), e(2, 80, 102), e(2, 80, 95), e(2, 80, 80),
    e(2, 96, 109), e(2, 80, 102), e(2, 96, 88), e(2, 80, 95), e(3, 176, 130),
    c(1, Trophy(269), 3028, 80, 88), e(3, 192, 139), e(3, 192, 139), e(1, 192, 139),
    e(1, 176, 108), e(3, 176, 115), e(2, 176, 115), e(1, 80, 109), e(1, 96, 109),
    e(1, 80, 109), e(1, 80, 88), e(1, 96, 116), e(2, 80, 109), e(2, 176, 160), e(1, 80, 88),
    e(2, 80, 102), e(2, 80, 146), e(2, 210, 210), e(1, 80, 95), e(1, 80, 130), e(1, 80, 88),
    e(3, 80, 116), e(1, 80, 81), e(1, 80, 116), e(0, 38, 88), e(1, 102, 102), e(1, 123, 123),
    e(1, 80, 80), e(1, 102, 102), c(1, MarsOrEmblem, 3032, 169, 169),
    c(1, Trophies3, 3036, 110, 110), c(1, Trophy(272), 3040, 146, 146),
    c(1, Trophies2, 3044, 80, 80), e(1, 80, 80), e(0, 122, 140), e(0, 35, 35), e(0, 98, 148),
    e(0, 28, 28), c(0, MarsOrEmblem, 3048, 120, 120),
];

/// Entry 23's check (`gm_801AA644`, the save's feature bit 4) and its page, which only the
/// details read: its `has_check` is 0.
const ENTRY_23: (Check, i32) = (Feature, 2972);

/// `gm_803DBFD8_extra.x_positions`: the details' columns' x.
const COLUMNS: [f32; 4] = [-1.0, -1.79, -2.579_999_9, -3.579_999_9];

/// `gm_803DBFD8_extra.sfx_cues`: the roll's sounds by tick, to the end's mark.
const CUES: [(i32, i32); 45] = [
    (1, 500015), (2, 500005), (160, 500001), (220, 500005), (240, 500003), (580, 500002),
    (602, 500004), (660, 500001), (830, 500002), (880, 500003), (1030, 500001), (1050, 500003),
    (1200, 500004), (1250, 500002), (1380, 500003), (1480, 500004), (1670, 500001),
    (1900, 500005), (1902, 500001), (1960, 500004), (2000, 500003), (2200, 500004),
    (2350, 500001), (2000, 500004), (2400, 500002), (2650, 500003), (2730, 500000),
    (2900, 500000), (3100, 500000), (3300, 500000), (3465, 500002), (3490, 500001),
    (3500, 500002), (3545, 500002), (3650, 500001), (3680, 500005), (3745, 500001),
    (3900, 500005), (3950, 500003), (4180, 500008), (4309, 500009), (4517, 500008),
    (4735, 500009), (4741, 500012), (0x7FFF_FFFF, 0),
];

/// `gm_803DD0F0`: the details' camera, a perspective from (0, 0, 2.9) toward (0, 0, -80).
const DETAILS_CAMERA: crate::hsd::cobj::StaticDesc = crate::hsd::cobj::StaticDesc {
    flags: 0,
    projection_type: 1,
    viewport: [0, 640, 0, 480],
    scissor: [0, 640, 0, 480],
    eye: Vec3::new(0.0, 0.0, 2.9),
    interest: Vec3::new(0.0, 0.0, -80.0),
    roll: 0.0,
    near: 1.0,
    far: 10000.0,
    projection: [80.0, 1.216_667_1, 0.0, 0.0],
};

/// The colors: `gm_804D42B8` (clear), `gm_804D42BC` (black at 0x3C), `gm_804D42C0` (black),
/// `gm_804D42C4` (white), `gm_804D42C8` (a hit name's red) and `gm_804D42CC` (the aimed
/// name's cyan).
const CLEAR: [u8; 4] = [0, 0, 0, 0];
const DIM: [u8; 4] = [0, 0, 0, 0x3C];
const BLACK: [u8; 4] = [0, 0, 0, 0xFF];
const WHITE: [u8; 4] = [0xFF; 4];
const HIT: [u8; 4] = [0xFF, 0x64, 0x8C, 0xFF];
const AIMED: [u8; 4] = [0xB4, 0xFF, 0xFF, 0xFF];

/// `gm_803DD1C8`: the first seven names' titles, by their joint in the background model.
const TITLE_JOINTS: [i32; 7] = [0x10, 0x63, 0x62, 0x64, 0x5B, 0x5A, 0x59];

/// The pad buttons the roll reads.
const PAD_A: u32 = 0x100;
const PAD_B: u32 = 0x200;
const PAD_START: u32 = 0x1000;
const PAD_Z: u32 = 0x10;

/// A name's state (`staffInfo`'s entries): its two texts while it's shown, and its hits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Name {
    pub win: [Option<TextId>; 2],
    pub hits: i32,
}

/// A shown name's place in the depth order (`staffInfoSortBuf`'s entries): its index, its
/// leaf joint, and the joint's matrix in the names' camera's view. `x38` is never written.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Sorted {
    pub index: i32,
    pub jobj: Option<JObjId>,
    pub mtx: Mtx34,
}

/// The staff roll's statics.
#[derive(Clone, Copy, Debug, PartialEq, melee_derive::StateEq)]
pub struct StaffRoll {
    /// `staffInfo` and `staffInfoSortBuf`: their heap blocks, and their entries.
    pub info_addr: u32,
    pub sort_addr: u32,
    pub names: [Name; NAMES],
    pub sorted: [Sorted; NAMES],
    /// `gm_804D6800`: how many names are shown.
    pub shown: i32,
    /// `gm_804D6804`: the cursor's place.
    pub cursor: [f32; 2],
    /// `gm_804D680C`: the hits' count's text; `gm_80480D58`: the details' texts.
    pub tally: Option<TextId>,
    pub details: [Option<TextId>; 6],
    /// `gm_804D6810`: the details' column.
    pub column: i32,
    /// `gm_804D6814`: the roll's tick.
    pub tick: i32,
    /// `gm_804D6818`: paused (B).
    pub paused: i32,
    /// `gm_804D681C`: ticks the roll runs a tick (1, or 6 after Start).
    pub speed: i32,
    /// `gm_804D6820`: ticks after a resume before A fires again.
    pub resume_wait: i32,
    /// `gm_804D6824`, `gm_804D6828`: the gun's kick and its target.
    pub kick: f32,
    pub kick_target: f32,
    /// `gm_804D682C`: the background's model; `gm_804D6830`, `gm_804D6834`: the names' and the
    /// details' cameras; `gm_804D6838`: the fog.
    pub bg: Option<JObjId>,
    pub camera: Option<crate::hsd::cobj::CObjId>,
    pub details_camera: Option<crate::hsd::cobj::CObjId>,
    pub fog: Option<crate::hsd::fog::FogId>,
    /// `gm_804D6848`: the background dimmed (Start's speed); `gm_804D684C`: the next sound.
    pub dimmed: i32,
    pub cue: i32,
    /// `gm_803DD1E4`'s x, which the hit test writes: the last name's half width.
    pub half_width: f32,
}

impl Default for StaffRoll {
    fn default() -> Self {
        StaffRoll {
            info_addr: 0,
            sort_addr: 0,
            names: [Name::default(); NAMES],
            sorted: [Sorted::default(); NAMES],
            shown: 0,
            cursor: [0.0; 2],
            tally: None,
            details: [None; 6],
            column: 0,
            tick: 0,
            paused: 0,
            speed: 0,
            resume_wait: 0,
            kick: 0.0,
            kick_target: 0.0,
            bg: None,
            camera: None,
            details_camera: None,
            fog: None,
            dimmed: 0,
            cue: 0,
            half_width: 13.5,
        }
    }
}

impl Running<'_> {
    /// A name's check, `StaffEntryData::check` called with its argument.
    #[doc(alias = "gm_801AA644")]
    #[doc(alias = "gm_801AA664")]
    #[doc(alias = "gm_801AA688")]
    #[doc(alias = "gm_801AA6D8")]
    #[doc(alias = "gm_801AA6FC")]
    #[doc(alias = "gm_801AA774")]
    fn staff_check(&self, check: Check) -> bool {
        let u = &self.game.gm.unlocks;
        let held = |id: usize| self.game.trophy_count(id) != 0;
        match check {
            Feature => self.game.game_data.thing.save_data.x186C & 4 != 0,
            Character(k) => u.ckind_unlocked(k),
            MarsOrEmblem => u.ckind_unlocked(9) || u.ckind_unlocked(0x17),
            Trophy(id) => held(usize::from(id)),
            Trophies3 => held(0x10B) || held(0x10A) || held(0x109),
            Trophies2 => held(0x107) || held(0x108),
        }
    }

    /// Entry `i`'s check and page, if it has one (`check != NULL`), and whether `has_check`.
    fn staff_entry_check(&self, i: usize) -> (Option<(bool, i32)>, bool) {
        if i == 23 {
            let (k, page) = ENTRY_23;
            return (Some((self.staff_check(k), page)), false);
        }
        match ENTRIES[i].check {
            Some((k, page)) => (Some((self.staff_check(k), page)), true),
            None => (None, false),
        }
    }

    /// `gm_Scene_StaffRoll_OnFrame`: past the roll's end, the master volume back up, the music
    /// stopped and the scene's end.
    #[doc(alias = "gm_Scene_StaffRoll_OnFrame")]
    #[doc(alias = "lbAudioAx_80028B90")]
    pub fn staff_roll_on_frame(&mut self) {
        if self.game.endings.staff_roll.tick >= END_AT {
            self.game.audio.lb.master = 0x7F;
            self.lb_audio_music_stop();
            self.game.end_scene();
        }
    }

    /// The names' camera's process, `fn_801AA7F8`, and the light's, background's and fog's
    /// (`fn_801AAABC`, `fn_801AAB18`, `fn_801AC67C`): their animations once per tick the roll
    /// runs, unless paused.
    #[doc(alias = "fn_801AA7F8")]
    #[doc(alias = "fn_801AAABC")]
    #[doc(alias = "fn_801AAB18")]
    #[doc(alias = "fn_801AC67C")]
    pub fn staff_roll_anim(&mut self, gobj: GObjId) {
        let s = &self.game.endings.staff_roll;
        if s.paused != 0 {
            return;
        }
        let speed = s.speed;
        let obj = self.game.world.gobj(gobj).obj;
        for _ in 0..speed {
            match obj {
                Some(Obj::CObj(c)) => crate::hsd::cobj::anim(&mut self.game.objs, self.data, Some(c)),
                Some(Obj::LObj(l)) => {
                    crate::hsd::lobj::anim_all(&mut self.game.objs, self.data, Some(l))
                }
                Some(Obj::JObj(j)) => self.anim_all(j),
                Some(Obj::Fog(f)) => {
                    crate::hsd::fog::interpret_anim(&mut self.game.objs, self.data, Some(f))
                }
                ref other => panic!("a staff roll process on {other:?}"),
            }
        }
    }

    /// The names' camera's render callback, `fn_801AA854`: the screen erased to the fog's
    /// color, its links drawn, the fog off, then each shown name's two texts from the farthest
    /// on, each in the camera's view moved to its joint, if facing the camera.
    #[doc(alias = "fn_801AA854")]
    pub fn staff_roll_names_render(&mut self, gobj: GObjId) {
        let Some(Obj::CObj(c)) = self.game.world.gobj(gobj).obj else {
            panic!("the staff roll's camera holds no CObj");
        };
        if !crate::hsd::display::camera_begin(&mut self.game.objs, self.data, Some(c)) {
            return;
        }
        let fog = self.game.endings.staff_roll.fog.expect("the staff roll's fog");
        let [r, g, b, _] = self.game.objs.fogs[fog].color;
        let draws = &mut self.game.objs.display.draws;
        draws.push(crate::hsd::display::Draw::EraseColor([r, g, b, 0xFF]));
        draws.push(crate::hsd::display::Draw::Erase);
        self.render_links(gobj, 7);
        crate::hsd::display::fog_set(&mut self.game.objs, None);
        let saved = self.game.objs.cobjs[c].view_mtx.copied();
        let mut cur = self.game.world.gx_head[9];
        while let Some(g) = cur {
            assert!(
                Some(g) == self.game.world.gx_head[9],
                "the original's assertion failed: gobj == HSD_GObjGXLinkHead[Gm_GObj_GXLink_PlyCursor]"
            );
            let s = &self.game.endings.staff_roll;
            for i in (0..s.shown as usize).rev() {
                let s = &self.game.endings.staff_roll;
                let e = s.sorted[i];
                let name = s.names[e.index as usize];
                let [w0, w1] = name.win.map(|w| {
                    w.unwrap_or_else(|| {
                        panic!("the original's assertion failed: staffInfo[staffInfoSortBuf[i].index].win")
                    })
                });
                let j = e.jobj.expect("a shown name's joint");
                let jm = self.game.objs.jobjs[j].mtx;
                let view = self.game.objs.cobjs[c].view_mtx.concat(&jm);
                self.game.objs.cobjs[c].view_mtx = view;
                if reg(view.m[2][2]) >= 0.0 {
                    for t in [w0, w1] {
                        self.staff_roll_text(t, Some(view));
                    }
                }
                self.game.objs.cobjs[c].view_mtx = saved.copied();
            }
            cur = self.game.world.gobj(g).next_gx;
        }
        crate::hsd::display::camera_end(&mut self.game.objs);
    }

    /// A text drawn through its GObj (`HSD_SisLib_803A84BC(text->entity, 2)`), in `view`.
    fn staff_roll_text(&mut self, t: TextId, view: Option<Mtx34>) {
        let g = self
            .game
            .sis
            .text(t)
            .entity
            .expect("a staff roll text without its GObj");
        let n = self.game.objs.display.draws.len();
        self.render_text(g, 2);
        if let Some(v) = view
            && self.game.objs.display.draws.len() > n
            && let Some(crate::hsd::display::Draw::SisText(d)) =
                self.game.objs.display.draws.last_mut()
        {
            d.view = Some(v);
        }
    }

    /// The details' camera's render callback, `fn_801AAA28`: its links, then the details'
    /// texts and the hits' count.
    #[doc(alias = "fn_801AAA28")]
    pub fn staff_roll_details_render(&mut self, gobj: GObjId) {
        self.render_camera(gobj);
        let Some(Obj::CObj(c)) = self.game.world.gobj(gobj).obj else {
            panic!("the staff roll's details camera holds no CObj");
        };
        if crate::hsd::display::camera_begin(&mut self.game.objs, self.data, Some(c)) {
            let s = &self.game.endings.staff_roll;
            let (details, tally) = (s.details, s.tally);
            for t in details.into_iter().flatten() {
                self.staff_roll_text(t, None);
            }
            if let Some(t) = tally {
                self.staff_roll_text(t, None);
            }
        }
        crate::hsd::display::camera_end(&mut self.game.objs);
    }

    /// The names' process, `fn_801AAB74`: the names' animations once per tick the roll runs
    /// unless paused; each name (a child of the names' root, its leaf two joints down) that
    /// came into view gets its texts (its name's page in the saved language, the English for
    /// three in English settings, the check's page for a name whose check fails; red once
    /// hit), one that left loses them; the shown ones sorted from nearest to farthest by their
    /// depth in the camera's view.
    #[doc(alias = "fn_801AAB74")]
    pub fn staff_roll_names(&mut self, gobj: GObjId) {
        let Some(Obj::JObj(root)) = self.game.world.gobj(gobj).obj else {
            panic!("the staff roll's names hold no JObj");
        };
        let s = &self.game.endings.staff_roll;
        if s.paused == 0 {
            for _ in 0..s.speed {
                self.anim_all(root);
            }
        }
        let jp = self.game.gm.unlocks.is_jp();
        let setting_jp = super::super::super::gm::language_setting_is_jp();
        self.game.endings.staff_roll.shown = 0;
        let mut child = self.game.objs.jobjs[root].child;
        let mut i = 0usize;
        while let Some(ch) = child {
            let j = &self.game.objs.jobjs;
            let leaf = j[ch]
                .child
                .and_then(|x| j[x].child)
                .expect("a name's leaf joint");
            let hidden = j[leaf].flags & jobj::HIDDEN != 0;
            let (title_page, jp_page, en_page) = (15 * i as i32 + 2, 15 * i as i32 + 3, 15 * i as i32 + 4);
            let (chk, has) = self.staff_entry_check(i);
            let passes = !has || chk.is_some_and(|(ok, _)| ok);
            let shows = (passes || i == 0xC5) && (i != 0x5E || jp);
            let name = self.game.endings.staff_roll.names[i];
            if !hidden && name.win[0].is_none() && shows {
                let w0 = self.create_text(0, 0, Vec3::new(0.0, -3.3, 0.0), 0.0, 100.0);
                let tx = self.game.sis.text_mut(w0);
                tx.x4c = 1;
                tx.default_alignment = 1;
                tx.default_kerning = 1;
                tx.font_size = if jp && i < 0xB7 && i != 0xB0 {
                    [0.209, 0.209]
                } else {
                    [0.136_799_99, 0.209]
                };
                self.game.endings.staff_roll.names[i].win[0] = Some(w0);
                let english_three = matches!(i, 0xB7 | 0xC1 | 0xC3);
                let page = if has && chk.is_some_and(|(ok, _)| !ok) {
                    let (_, alt) = chk.unwrap();
                    if jp { alt } else { alt + 1 }
                } else if (!english_three && jp) || (english_three && setting_jp) {
                    jp_page
                } else {
                    en_page
                };
                self.game.sis.reset_text(self.data, w0, page);
                let w1 = self.create_text(0, 0, Vec3::new(0.0, -7.8, 0.0), 0.0, 100.0);
                let tx = self.game.sis.text_mut(w1);
                tx.x4c = 1;
                tx.default_alignment = 1;
                tx.default_kerning = 1;
                tx.font_size = [0.1, 0.1];
                self.game.endings.staff_roll.names[i].win[1] = Some(w1);
                if passes {
                    self.game.sis.reset_text(self.data, w1, title_page);
                }
                if name.hits >= 1 {
                    self.game.sis.text_mut(w0).active_color = Some(HIT);
                    self.game.sis.text_mut(w1).active_color = Some(HIT);
                }
            } else if hidden && let Some(w0) = name.win[0] {
                self.destroy_text(w0);
                let w1 = name.win[1].expect("a shown name's second text");
                self.destroy_text(w1);
                self.game.endings.staff_roll.names[i].win[0] = None;
            }
            if !hidden && shows {
                jobj::setup_matrix(&mut self.game.objs, self.data, Some(leaf));
                let s = &mut self.game.endings.staff_roll;
                let n = s.shown as usize;
                s.sorted[n].index = i as i32;
                s.sorted[n].jobj = Some(leaf);
                s.shown += 1;
            }
            child = self.game.objs.jobjs[ch].next;
            i += 1;
        }
        let cam = self.game.endings.staff_roll.camera.expect("the names' camera");
        crate::hsd::cobj::setup_viewing_mtx(&mut self.game.objs, self.data, cam);
        let view = self.game.objs.cobjs[cam].view_mtx;
        let n = self.game.endings.staff_roll.shown as usize;
        for k in 0..n {
            let j = self.game.endings.staff_roll.sorted[k]
                .jobj
                .expect("a shown name's joint");
            jobj::setup_matrix(&mut self.game.objs, self.data, Some(j));
            let m = self.game.objs.jobjs[j].mtx;
            self.game.endings.staff_roll.sorted[k].mtx = view.concat(&m);
        }
        let sorted = &mut self.game.endings.staff_roll.sorted;
        let mut top = n;
        while top > 1 {
            top -= 1;
            for k in 0..top {
                if reg(sorted[k].mtx.m[2][3]) < reg(sorted[k + 1].mtx.m[2][3]) {
                    sorted.swap(k, k + 1);
                }
            }
        }
    }

    /// Name `idx`'s color (`gm_801AB200_ptcl`): for the first seven their title's material's
    /// diffuse color in the background (the fourth display object's for the first, the
    /// second's for the third and fourth, else the first's), for the others their texts'.
    fn staff_roll_color(&mut self, idx: usize, color: [u8; 4]) {
        if idx < 7 {
            let bg = self.game.endings.staff_roll.bg;
            let [j] = super::super::pause::tree_nodes(
                &self.game.objs.jobjs,
                bg,
                &[TITLE_JOINTS[idx]],
            );
            let j = j.expect("a title's joint");
            let jobj::Contents::DObjs(d) = self.game.objs.jobjs[j].u else {
                panic!("a title joint without display objects");
            };
            let skip = match idx {
                0 => 3,
                2 | 3 => 1,
                _ => 0,
            };
            let mut d = d.expect("a title's display object");
            for _ in 0..skip {
                d = self.game.objs.dobjs[d].next.expect("a title's display object");
            }
            let m = self.game.objs.dobjs[d].mobj.expect("a title's material");
            crate::hsd::mobj::set_diffuse(&mut self.game.objs.mobjs, m, color);
        } else {
            let n = self.game.endings.staff_roll.names[idx];
            for w in n.win {
                let w = w.expect("a shown name's text");
                self.game.sis.text_mut(w).active_color = Some(color);
            }
        }
    }

    /// The roll's main process, `fn_801AB200`: the cursor from the stick, the shown names
    /// white (but those hit) and the cursor's place in the names' camera's view; before the
    /// result, the nearest shown name under the cursor aimed (cyan); then the buttons (A, B,
    /// Start, Z in that order): A fires the gun (unless paused or just resumed); A or B on an
    /// aimed name shows its details, and A also hits it (a particle at its joint, its hit
    /// counted, red, the gun's kick); the gun's kick settles; the roll's own animations and
    /// tick, the result at 4741, the fade from 4821 and the flash at 4881; Start's speed; B's
    /// pause and the resume; and the roll's sounds.
    #[doc(alias = "fn_801AB200")]
    #[doc(alias = "lbAudioAx_80028B6C")]
    pub fn staff_roll_main(&mut self, gobj: GObjId) {
        let Some(Obj::JObj(root)) = self.game.world.gobj(gobj).obj else {
            panic!("the staff roll's model holds no JObj");
        };
        let node = |r: &Self, i: i32| {
            super::super::pause::tree_nodes(&r.game.objs.jobjs, Some(root), &[i])[0]
                .expect("a staff roll joint")
        };
        let cursor = node(self, 7);
        let port = usize::from(self.game.endings.gover.port());
        let dead = |v: i8| {
            let v = i32::from(v);
            if v > 4 {
                v - 4
            } else if v < -4 {
                v + 4
            } else {
                0
            }
        };
        let sx = dead(self.game.pads.copy[port].stick_x);
        let sy = dead(self.game.pads.copy[port].stick_y);
        let vx = fp::frsp(f64::from(sx));
        let vy = fp::frsp(f64::from(sy));
        let k = reg(0.000_384_615_38);
        let rx = crate::math::sqrt(mem(fp::fmadds(vx, vx, 6400.0)));
        let vx = fp::fmuls(vx, fp::fmuls(k, reg(rx)));
        let ry = crate::math::sqrt(mem(fp::fmadds(vy, vy, 6400.0)));
        let mut vy = fp::fmuls(vy, fp::fmuls(k, reg(ry)));
        if vy < reg(-2.6) {
            vy = reg(-2.6);
        }
        if vy > reg(2.6) {
            vy = reg(2.6);
        }
        let s = &mut self.game.endings.staff_roll;
        s.cursor = [mem(vx), mem(vy)];
        let [cx, cy] = s.cursor;
        let jobjs = &mut self.game.objs.jobjs;
        jobj::set_srt(jobjs, cursor, |o| o.translate.x = cx);
        jobj::set_srt(jobjs, cursor, |o| o.translate.y = cy);
        let mut selected: i32 = -1;
        let mut hover = None;
        for i in 0..self.game.endings.staff_roll.shown as usize {
            let s = &self.game.endings.staff_roll;
            let idx = s.sorted[i].index as usize;
            if s.names[idx].hits < 1 {
                self.staff_roll_color(idx, WHITE);
            }
        }
        let k = reg(0.294_117_63);
        let s = &mut self.game.endings.staff_roll;
        s.cursor = s.cursor.map(|v| mem(fp::fmuls(reg(v), k)));
        let cam = s.camera;
        let fov = crate::hsd::cobj::fov(&self.game.objs.cobjs, cam);
        let half = fp::frsp(fp::fmul(0.008_726_646_471_282_037, reg(fov)));
        let t = crate::math::tan(mem(half));
        let scale = fp::frsp(fp::fmul(1.191_753_6, reg(t)));
        let s = &mut self.game.endings.staff_roll;
        s.cursor = s.cursor.map(|v| mem(fp::fmuls(reg(v), scale)));
        if s.tick < RESULT_AT {
            let us = usize::from(self.game.gm.unlocks.is_us());
            for i in 0..self.game.endings.staff_roll.shown as usize {
                let s = &self.game.endings.staff_roll;
                let (e, cursor) = (s.sorted[i], s.cursor);
                if !(reg(e.mtx.m[2][3]) < reg(-1.0)) {
                    continue;
                }
                let idx = e.index as usize;
                if idx == 0xB7 || idx >= 0xC0 {
                    continue;
                }
                hover = e.jobj;
                let width = fp::frsp(f64::from(ENTRIES[idx].width[us]));
                let half = mem(fp::fmuls(reg(0.168_75), width));
                self.game.endings.staff_roll.half_width = half;
                let corners = [
                    Vec3::new(-half, -4.5, 0.0),
                    Vec3::new(-half, 6.5, 0.0),
                    Vec3::new(half, 6.5, 0.0),
                    Vec3::new(half, -4.5, 0.0),
                ];
                let mut proj = [0.0f32; 8];
                let mut broke = false;
                for (j, &p) in corners.iter().enumerate() {
                    let v = e.mtx.transform(p);
                    if reg(v.z) >= reg(-1.0) {
                        broke = true;
                        break;
                    }
                    proj[2 * j] = mem(fp::fdivs(fp::fneg(reg(v.x)), reg(v.z)));
                    proj[2 * j + 1] = mem(fp::fdivs(fp::fneg(reg(v.y)), reg(v.z)));
                }
                if broke {
                    continue;
                }
                let [x0, x4] = cursor.map(reg);
                let p = proj.map(reg);
                let edge = |a: usize, b: usize| {
                    let l = fp::fmuls(fp::fsubs(p[b + 1], p[a + 1]), fp::fsubs(x0, p[a]));
                    let r = fp::fmuls(fp::fsubs(x4, p[a + 1]), fp::fsubs(p[b], p[a]));
                    l >= r
                };
                if edge(0, 2) && edge(2, 4) && edge(4, 6) && edge(6, 0) {
                    selected = idx as i32;
                    if self.game.endings.staff_roll.names[idx].hits < 1 {
                        self.staff_roll_color(idx, AIMED);
                        for w in self.game.endings.staff_roll.names[idx].win {
                            let w = w.expect("an aimed name's text");
                            let tx = self.game.sis.text_mut(w);
                            let mut c = tx.active_color.expect("an aimed name's color");
                            c[0] = 0xB4;
                            tx.active_color = Some(c);
                        }
                    }
                    break;
                }
            }
        }
        let trigger = self.game.pads.copy[port].trigger;
        let button = [PAD_A, PAD_B, PAD_START, PAD_Z]
            .into_iter()
            .find(|&b| trigger & b != 0)
            .unwrap_or(0);
        if button != 0 {
            let s = &self.game.endings.staff_roll;
            let (paused, wait) = (s.paused, s.resume_wait);
            if paused == 0 && button == PAD_A && wait == 0 {
                let gun = node(self, 5);
                jobj::req_anim(&mut self.game.objs, Some(gun), 0.0);
                self.sfx_play(0x7A12D, 0x7F, 0x40);
            }
            if selected != -1 && (button == PAD_B || (paused == 0 && button == PAD_A)) {
                self.staff_roll_details(root, selected as usize);
                if button == PAD_A && self.game.endings.staff_roll.resume_wait == 0 {
                    self.staff_roll_hit(selected as usize, hover.expect("the aimed name"));
                }
            }
        }
        let s = &mut self.game.endings.staff_roll;
        s.kick_target = mem(fp::fsubs(reg(s.kick_target), reg(0.017)));
        if reg(s.kick_target) < 0.0 {
            s.kick_target = 0.0;
        }
        let up = fp::fadds(reg(0.25), reg(s.kick));
        s.kick = if up < reg(s.kick_target) { mem(up) } else { s.kick_target };
        let turn = fp::frsp(fp::fsub(fp::fneg(reg(s.kick)), std::f64::consts::FRAC_PI_4));
        let gun_turn = node(self, 0x18);
        jobj::set_rotation_z(&mut self.game.objs.jobjs, gun_turn, mem(turn));
        let gun = node(self, 5);
        self.anim_all(gun);
        let pair = super::super::pause::tree_nodes(&self.game.objs.jobjs, Some(root), &[0x1C, 0x1A]);
        let (paused, speed) = {
            let s = &self.game.endings.staff_roll;
            (s.paused, s.speed)
        };
        if paused == 0 {
            for _ in 0..speed {
                let a = node(self, 2);
                self.anim_all(a);
                let b = node(self, 0x14);
                self.anim_all(b);
            }
            let s = &mut self.game.endings.staff_roll;
            s.tick += s.speed;
            if s.tick >= RESULT_AT && s.tally.is_none() {
                self.staff_roll_result(pair);
            }
            let s = &self.game.endings.staff_roll;
            let (tick, dimmed) = (s.tick, s.dimmed);
            if tick >= FADE_AT {
                let lb = &mut self.game.audio.lb;
                lb.master -= 1;
                if lb.master < 0 {
                    lb.master = 0;
                }
            }
            if tick == FLASH_AT {
                let from = if dimmed == 0 { CLEAR } else { DIM };
                self.game.bgflash.fade(gx(from), gx(BLACK), 0x3C);
                self.sfx_stop_all();
            }
        }
        let col = COLUMNS[self.game.endings.staff_roll.column as usize];
        let p0 = pair[0].expect("the details' column joint");
        jobj::set_srt(&mut self.game.objs.jobjs, p0, |o| o.translate.x = col);
        let s = &mut self.game.endings.staff_roll;
        if button == PAD_START {
            if s.speed == 1 {
                if s.tick < RESULT_AT {
                    s.speed = 6;
                    s.dimmed = 1;
                    self.game.bgflash.fade(gx(DIM), gx(DIM), 1);
                } else {
                    s.speed = 1;
                }
            } else if s.tick < RESULT_AT {
                s.speed = 1;
                s.dimmed = 0;
                self.game.bgflash.fade(gx(CLEAR), gx(CLEAR), 1);
            } else {
                s.speed = 1;
            }
        }
        for _ in 1..self.game.endings.staff_roll.speed {
            super::super::super::ef::eflib::particles_proc_main(self);
        }
        let s = &mut self.game.endings.staff_roll;
        if s.resume_wait != 0 {
            s.resume_wait -= 1;
        }
        if s.paused == 0 && button == PAD_B {
            if s.tick < RESULT_AT {
                s.paused = 1;
                let g = self
                    .game
                    .world
                    .find(8, 0xB)
                    .expect("the original's assertion failed: gobj");
                self.game.world.set_paused(g, true);
            }
        } else if s.paused == 1 && button != 0 && (button != PAD_B || selected == -1) {
            s.paused = 0;
            let g = self
                .game
                .world
                .find(8, 0xB)
                .expect("the original's assertion failed: gobj");
            self.game.world.set_paused(g, false);
            self.game.endings.staff_roll.resume_wait = 0x3C;
        }
        let s = &self.game.endings.staff_roll;
        let (at, sfx) = CUES[s.cue as usize];
        if s.tick >= at {
            self.sfx_play(sfx, 0x7F, 0x40);
            self.game.endings.staff_roll.cue += 1;
        }
    }

    /// The aimed name's details (`fn_801AB200`'s A or B): the details' joint's animation and
    /// a sound, then the six texts made afresh in the name's column: its name (grey) and its
    /// role (magenta) unless its check fails, and four lines of notes (orange): Luigi's and
    /// Dr. Mario's unlocks choose entry 75's first, a failed check its own page.
    fn staff_roll_details(&mut self, root: JObjId, sel: usize) {
        let [j] = super::super::pause::tree_nodes(&self.game.objs.jobjs, Some(root), &[8]);
        jobj::req_anim_all(&mut self.game.objs, j, 0.0);
        self.sfx_play(0x7A12A, 0x7F, 0x40);
        let (chk, has) = self.staff_entry_check(sel);
        let failed = chk.is_some_and(|(ok, _)| !ok);
        let alt = chk.map_or(0, |(_, p)| p);
        let base = sel as i32 * 0xF;
        let jp = self.game.gm.unlocks.is_jp();
        let column = i32::from(ENTRIES[sel].column);
        for k in 0..6usize {
            if let Some(t) = self.game.endings.staff_roll.details[k] {
                self.destroy_text(t);
            }
            self.game.endings.staff_roll.column = column;
            let x = COLUMNS[column as usize];
            let pos = match k {
                0 => Vec3::new(mem(fp::fadds(reg(1.45), reg(x))), 0.86, 0.0),
                1 => {
                    let o1 = if sel == 0xAC { 0.2 } else { 0.0 };
                    let o2 = if sel == 0xAF { 0.1 } else { 0.0 };
                    let v = fp::fadds(fp::fadds(reg(3.0), reg(x)), reg(o1));
                    Vec3::new(mem(fp::fadds(reg(o2), v)), 0.86, 0.0)
                }
                _ => {
                    let y = fp::fmadds(reg(0.19), fp::frsp(k as f64), reg(0.67));
                    Vec3::new(mem(fp::fadds(reg(1.45), reg(x))), mem(y), 0.0)
                }
            };
            let t = self.create_text(0, 0, pos, 0.0, 100.0);
            self.game.endings.staff_roll.details[k] = Some(t);
            let tx = self.game.sis.text_mut(t);
            tx.default_kerning = 1;
            tx.font_size = [0.003_76, 0.004_5];
            let shown = !failed || !has;
            match k {
                0 if shown => {
                    let tx = self.game.sis.text_mut(t);
                    tx.text_color[..3].copy_from_slice(&[0xC3, 0xC3, 0xC3]);
                    let page = if jp { base + 5 } else { base + 6 };
                    self.game.sis.reset_text(self.data, t, page);
                }
                1 if shown => {
                    let tx = self.game.sis.text_mut(t);
                    tx.text_color[..3].copy_from_slice(&[0xE8, 0x37, 0xE0]);
                    let page = if jp { base + 7 } else { base + 8 };
                    self.game.sis.reset_text(self.data, t, page);
                }
                0 | 1 => {}
                _ => {
                    let tx = self.game.sis.text_mut(t);
                    tx.text_color[..3].copy_from_slice(&[0xFF, 0xB4, 0]);
                    let page = if sel == 0x4B && k == 2 {
                        let u = &self.game.gm.unlocks;
                        0xBEC + i32::from(u.ckind_unlocked(7)) + 2 * i32::from(u.ckind_unlocked(0x16))
                    } else if k == 2 && failed {
                        if jp { alt + 2 } else { alt + 3 }
                    } else if jp {
                        base + 7 + k as i32
                    } else {
                        base + 0xB + k as i32
                    };
                    self.game.sis.reset_text(self.data, t, page);
                }
            }
        }
    }

    /// A hit (`fn_801AB200`'s A on an aimed name): effect 0x27's generator at the name's joint
    /// with an app-SRT 1.3 times its scale, the name's hit counted and colored red, and the
    /// gun's kick.
    fn staff_roll_hit(&mut self, sel: usize, hover: JObjId) {
        let objs = &mut self.game.objs;
        let g = crate::hsd::generator::spawn(
            objs,
            self.data,
            &mut super::super::super::code::ParticleHooks,
            0,
            0,
            0x27,
        )
        .expect("the staff roll's hit effect");
        let s = match objs.generators[g].appsrt {
            Some(s) => s,
            None => crate::hsd::appsrt::add_to_generator(objs, g, 1)
                .expect("the staff roll's hit's app-SRT"),
        };
        let j = &objs.jobjs[hover];
        let (t, sc) = (
            Vec3::new(j.mtx.m[0][3], j.mtx.m[1][3], j.mtx.m[2][3]),
            j.scale,
        );
        let k = reg(1.3);
        let a = &mut objs.app_srts[s];
        a.translate = t;
        a.scale = Vec3::new(
            mem(fp::fmuls(k, reg(sc.x))),
            mem(fp::fmuls(k, reg(sc.y))),
            mem(fp::fmuls(k, reg(sc.z))),
        );
        self.game.endings.staff_roll.names[sel].hits += 1;
        if self.game.endings.staff_roll.names[sel].hits >= 1 {
            self.staff_roll_color(sel, HIT);
        }
        let s = &mut self.game.endings.staff_roll;
        s.kick_target = mem(fp::fmadds(reg(0.5), reg(s.kick_target), reg(1.7)));
    }

    /// The result (`fn_801AB200` at 4741): the details' texts freed, the hits' count's text
    /// (`%d ＨＩＴ` in orange at (85, 130, -280)), the details' columns lowered, and the roll
    /// back to its own speed.
    fn staff_roll_result(&mut self, pair: [Option<JObjId>; 2]) {
        for k in 0..6 {
            if let Some(t) = self.game.endings.staff_roll.details[k] {
                self.destroy_text(t);
            }
            self.game.endings.staff_roll.details[k] = None;
        }
        self.game.endings.staff_roll.column = 0;
        let t = self.create_text_buffer(0, 0);
        self.game.endings.staff_roll.tally = Some(t);
        let tx = self.game.sis.text_mut(t);
        tx.default_kerning = 1;
        tx.pos = Vec3::new(85.0, 130.0, -280.0);
        let hits = self
            .game
            .endings
            .staff_roll
            .names
            .iter()
            .filter(|n| n.hits >= 1)
            .count();
        let mut s = format!("{hits} ").into_bytes();
        s.extend_from_slice(&[0x82, 0x67, 0x82, 0x68, 0x82, 0x73, 0]);
        let line = self.game.sis.add_string(t, 0.0, 0.0, Some(&s));
        self.game.sis.set_color(self.data, t, line, [0xFF, 0xB4, 0]);
        let jobjs = &mut self.game.objs.jobjs;
        for p in pair {
            let c = p.and_then(|p| jobjs[p].child).expect("a details' column's child");
            jobj::set_srt(jobjs, c, |o| o.translate.y = -0.4);
        }
        self.game.endings.staff_roll.speed = 1;
    }

    /// The files the staff roll reads, for the frontend: its scene and strings, and the
    /// character's two songs, one of which its entry draws.
    pub fn staff_roll_scene_files(&self) -> super::super::super::vi::SceneFiles {
        let mut f = super::super::super::vi::SceneFiles::default();
        f.names.push("GmStRoll.dat".into());
        f.names.push("SdStRoll.dat".into());
        let ckind = i32::from(self.game.endings.gover.ckind());
        for which in 0..2 {
            let song = crate::lb::audio::character_song(ckind, which);
            if !f.songs.contains(&(song as usize)) {
                f.songs.push(song as usize);
            }
        }
        f
    }

    /// `gm_Scene_StaffRoll_OnExit`: nothing.
    #[doc(alias = "gm_Scene_StaffRoll_OnExit")]
    pub fn staff_roll_on_exit(&mut self) {}

    /// `gm_Scene_StaffRoll_OnEnter`: the effects, the names' state and depth order in the
    /// heap, the names' strings (`SdStRoll.dat`) and their canvas, the roll's banks, its
    /// scene (`GmStRoll.dat`): the names' camera (animated), the details' camera, the light,
    /// the fog (animated), the background (the English logo's image in an English setting),
    /// the names' ten models under one root, and the cursor's and gun's model; the banks
    /// waited for, the character's song (its second one in four in five), the background
    /// flash, and the statics.
    #[doc(alias = "gm_Scene_StaffRoll_OnEnter")]
    pub fn staff_roll_on_enter(&mut self) {
        // The scene takes the files the preload cache holds (`lbArchive_80017040`): the
        // common effects.
        self.game.ft.data.files.from_cache = true;
        super::super::super::ef::eflib::init(self);
        super::super::super::ef::load_sync(self, 0);
        let info = crate::hsd::heap::mem_alloc(0x948);
        let sort = crate::hsd::heap::mem_alloc(0x2E68);
        self.load_font_named(0, "SdStRoll.dat", "SIS_StRollData");
        // `HSD_SisLib_803A611C(0, (HSD_GObj*) -1, ...)`: a canvas with no camera of its own.
        self.add_canvas(0, Some(GObjId(u32::MAX)), 9, 13, 0, 18, 0, 19);
        let a = &mut self.game.audio;
        a.lb.unwant_banks(28);
        a.lb.want_banks(16, 0x4_0000_0000_0000);
        a.lb_start_bank_loads(self.data);
        let ar = self.load_archive("GmStRoll.dat");
        let sym = |r: &Self, s: &str| DataRef {
            archive: ar,
            offset: r
                .data
                .get(ar)
                .public(s)
                .unwrap_or_else(|| panic!("{s} isn't in GmStRoll.dat")),
        };
        let scene = sym(self, "ScGamRegStaffroll_scene_data");
        let names = sym(self, "ScGamRegStaffrollNames_scene_modelset");
        let data = self.data;
        // The names' camera.
        let cams = data.ptr(scene.at(4)).expect("the scene's cameras");
        let desc = data.ptr(cams).expect("the scene's camera");
        let anim = data.ptr(cams.at(4)).and_then(|a| data.ptr(a));
        let g = &mut *self.game;
        let gobj = g.world.create(17, 19, 0);
        let c = crate::hsd::cobj::load_desc(&mut g.objs, data, Some(desc)).expect("a camera");
        g.endings.staff_roll.camera = Some(c);
        g.world.set_obj(gobj, OBJ_CAMERA, Obj::CObj(c));
        g.world
            .setup_gx_link_max(gobj, CodeRef(RENDER_NAMES), 5, false);
        g.world.gobj_mut(gobj).gxlink_prios = 0x189;
        crate::hsd::cobj::add_anim(&mut g.objs, data, Some(c), anim);
        crate::hsd::cobj::req_anim(&mut g.objs, Some(c), 0.0);
        g.world.setup_proc(gobj, CodeRef(PROC_CAMERA), 0);
        // The details' camera.
        let gobj = g.world.create(19, 20, 0);
        let c = crate::hsd::cobj::load_static_desc(&mut g.objs, data, &DETAILS_CAMERA);
        g.endings.staff_roll.details_camera = Some(c);
        g.world.set_obj(gobj, OBJ_CAMERA, Obj::CObj(c));
        g.world
            .setup_gx_link_max(gobj, CodeRef(RENDER_DETAILS), 8, false);
        g.world.gobj_mut(gobj).gxlink_prios = 1 << 11;
        // The light.
        let gobj = g.world.create(11, 3, 0);
        let lights = data.ptr(scene.at(8)).expect("the scene's lights");
        let l = super::super::super::gr::setup::load_light_list(g, data, lights).expect("a light");
        g.world.set_obj(gobj, OBJ_LIGHT, Obj::LObj(l));
        g.world.setup_gx_link(gobj, CodeRef(LOBJ_CALLBACK), 0, 0);
        crate::hsd::lobj::req_anim_all(&mut g.objs, Some(l), 0.0);
        g.world.setup_proc(gobj, CodeRef(PROC_LIGHT), 0);
        // The fog.
        let gobj = g.world.create(0xA, 3, 0);
        let fogs = data.ptr(scene.at(0xC)).expect("the scene's fogs");
        let desc = data.ptr(fogs).expect("the scene's fog");
        let anims = data.ptr(fogs.at(4)).expect("the fog's animations");
        let anim = data.ptr(anims).and_then(|a| data.ptr(a));
        let f = crate::hsd::fog::load_desc(&mut g.objs.fogs, data, desc);
        g.endings.staff_roll.fog = Some(f);
        g.world.set_obj(gobj, OBJ_FOG, Obj::Fog(f));
        g.world.setup_gx_link(gobj, CodeRef(FOG_CALLBACK), 3, 0);
        crate::hsd::fog::add_anim(&mut g.objs, data, Some(f), anim);
        crate::hsd::fog::req_anim(&mut g.objs, Some(f), 0.0);
        g.world.setup_proc(gobj, CodeRef(PROC_FOG), 0);
        // The background.
        let models = data.ptr(scene).expect("the scene's models");
        let model = |i: u32| data.ptr(models.at(4 * i)).expect("a scene model");
        let m1 = model(1);
        let gobj = g.world.create(3, 5, 0);
        let bg = jobj::load_joint(&mut g.objs, data, data.ptr(m1)).expect("the background");
        g.endings.staff_roll.bg = Some(bg);
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(bg));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 3, 0);
        super::super::vs::add_model_anim(&mut g.objs, data, Some(bg), m1, 0);
        jobj::req_anim_all(&mut g.objs, Some(bg), 0.0);
        if super::super::super::gm::language_setting_is_us() {
            let [a, b] = super::super::pause::tree_nodes(&g.objs.jobjs, Some(bg), &[103, 20]);
            let tobj = |g: &super::super::super::state::Game, j: Option<JObjId>| {
                let j = j.expect("the logo's joint");
                let jobj::Contents::DObjs(Some(d)) = g.objs.jobjs[j].u else {
                    panic!("the logo's joint without a display object");
                };
                let m = g.objs.dobjs[d].mobj.expect("the logo's material");
                g.objs.mobjs[m].tobj.expect("the logo's texture")
            };
            let (ta, tb) = (tobj(g, a), tobj(g, b));
            g.objs.tobjs[ta].imagedesc = g.objs.tobjs[tb].imagedesc;
        }
        g.world.setup_proc(gobj, CodeRef(PROC_BG), 0);
        // The names.
        let gobj = g.world.create(14, 15, 0);
        let first = data.ptr(names).expect("the names' first model");
        let nroot = jobj::load_joint(&mut g.objs, data, data.ptr(first)).expect("the names");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(nroot));
        g.world.setup_gx_link(gobj, CodeRef(0), 9, 0);
        super::super::vs::add_model_anim(&mut g.objs, data, Some(nroot), first, 0);
        jobj::req_anim_all(&mut g.objs, Some(nroot), 0.0);
        for i in 1..10u32 {
            let m = data.ptr(names.at(4 * i)).expect("a names' model");
            let t = jobj::load_joint(&mut g.objs, data, data.ptr(m)).expect("a names' model");
            super::super::vs::add_model_anim(&mut g.objs, data, Some(t), m, 0);
            jobj::req_anim_all(&mut g.objs, Some(t), 0.0);
            while let Some(ch) = g.objs.jobjs[t].child {
                jobj::reparent(&mut g.objs.jobjs, Some(ch), Some(nroot));
            }
        }
        g.world.setup_proc(gobj, CodeRef(PROC_NAMES), 1);
        // The cursor and the gun.
        let gobj = g.world.create(14, 15, 0);
        let m0 = model(0);
        let j = jobj::load_joint(&mut g.objs, data, data.ptr(m0)).expect("the cursor's model");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 11, 0);
        super::super::vs::add_model_anim(&mut g.objs, data, Some(j), m0, 0);
        let pair = super::super::pause::tree_nodes(&g.objs.jobjs, Some(j), &[2, 20]);
        jobj::req_anim_all(&mut g.objs, pair[0], 0.0);
        jobj::req_anim_all(&mut g.objs, pair[1], 0.0);
        g.world.setup_proc(gobj, CodeRef(PROC_MAIN), 13);
        self.game.audio.lb_wait_for_banks(self.data);
        let r = self.game.objs.rng.below(5);
        let ckind = self.game.endings.gover.ckind();
        let song = crate::lb::audio::character_song(i32::from(ckind), i32::from(r == 0));
        self.lb_play_music(song);
        self.bg_flash_create(10);
        self.game.endings.staff_roll = StaffRoll {
            info_addr: info,
            sort_addr: sort,
            sorted: self.game.endings.staff_roll.sorted,
            camera: self.game.endings.staff_roll.camera,
            details_camera: self.game.endings.staff_roll.details_camera,
            fog: self.game.endings.staff_roll.fog,
            bg: self.game.endings.staff_roll.bg,
            half_width: self.game.endings.staff_roll.half_width,
            speed: 1,
            ..StaffRoll::default()
        };
    }
}

/// A color as `lbBgFlash_800206D4` takes it.
fn gx(c: [u8; 4]) -> super::super::super::structs::_GXColor {
    super::super::super::structs::_GXColor {
        r: c[0],
        g: c[1],
        b: c[2],
        a: c[3],
    }
}
