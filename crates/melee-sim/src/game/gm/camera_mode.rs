//! Camera Mode (game mode 0xA, `melee/gm/gmcameramode.c`): a VS match whose fourth controller
//! is a camera, after a screen that checks the memory cards for room (`gm_1A33.c`'s scene),
//! with the camera's menu over the match (`gmcamera.c`): Z takes a snapshot (`cmsnap.c`'s copy
//! of the frame), which the menu offers to save to a card ([`crate::lb::snap_write`]).

use melee_data::archive::{ArchiveId, DataRef};

use super::super::handles::CodeRef;
use super::super::state::{OBJ_JOBJ, Obj, Running};
use super::machine::GM_MENU;
use super::vs_mode;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::sis::{SisPtr, TextId};
use crate::vec::Vec3;

/// `GM_CAMERA_MODE`.
pub const GM_CAMERA_MODE: u8 = 0x0A;
/// `GS_CAMERA_VS`, the card check's scene.
pub const GS_CAMERA_VS: u8 = 0x2C;
/// `GmVsMode_Camera`, Camera Mode's entry of the modes table.
const TABLE: usize = 3;
/// `VS_CAMERA`, the character select's match type for Camera Mode.
const VS_CAMERA: u8 = 1;

/// The preload cache's raw buffers Camera Mode wants (`gm_801B23F0`): the frame's copy, the
/// snapshot's data and the slots' lists, by entry number.
pub const SNAP_COPY: i32 = 2006;
pub const SNAP_DATA: i32 = 2007;
pub const SNAP_SLOTS: i32 = 2008;

/// The card check's scene: `gm_Scene_CameraVs_OnEnter`, `gm_Scene_CameraVs_OnFrame`.
pub const ON_ENTER: CodeRef = CodeRef(0x801A_3634);
pub const ON_FRAME: CodeRef = CodeRef(0x801A_34FC);

/// The match's hooks Camera Mode sets (`gm_PrepCameraModeVSScene`): the pause and unpause
/// camera (`gm_80165268`), the pauser (`gm_CameraModeVSGetPauser`), the match start
/// (`gmCamera_801A31FC`) and the frame's start and end (`gmCamera_801A3098`,
/// `gmCamera_801A30E4`).
pub const PAUSE_CAMERA: u32 = 0x8016_5268;
pub const GET_PAUSER: u32 = 0x8016_BE80;
pub const MATCH_START: u32 = 0x801A_31FC;
pub const FRAME_START: u32 = 0x801A_3098;
pub const FRAME_END: u32 = 0x801A_30E4;
/// The menu's model's process, `fn_801A31D8`.
pub const MODEL_PROC: u32 = 0x801A_31D8;
/// The frame's copy's render callback, `cmSnap_80031640`.
pub const SNAP_RENDER: u32 = 0x8003_1640;

/// `HSD_GObj_JObjCallback`.
const JOBJ_RENDER: u32 = 0x8039_1070;
/// `JOBJ_HIDDEN`.
const HIDDEN: u32 = jobj::HIDDEN;
/// `HSD_PAD_*`, the buttons the screens read.
const PAD_LEFT: u64 = 0x1;
const PAD_RIGHT: u64 = 0x2;
const PAD_Z: u64 = 0x10;
const PAD_A: u64 = 0x100;
const PAD_B: u64 = 0x200;
const PAD_START: u64 = 0x1000;
const PAD_STICK_LEFT: u64 = 0x4_0000;
const PAD_STICK_RIGHT: u64 = 0x8_0000;
/// `PAD_LR_START`: L, R and Start held together, as the controller map reports it.
const PAD_LR_START: u64 = super::super::mn::main::PAD_LR_START;
/// The menus' sounds (`lbAudioAx_80024030`).
const SFX_BACK: i32 = 0;
const SFX_FORWARD: i32 = 1;
const SFX_MOVE: i32 = 2;
/// The camera's controller, the fourth (`HSD_PadCopyStatus[3]`).
const CAMERA_PORT: usize = 3;
/// The menu's font.
const FONT: i32 = 3;

/// `gmCamera_803DA758`: the card check's texts' places and sizes (slot A's, slot B's: size,
/// then place; the bottom line's size and place).
const CHECK_LAYOUT: [f32; 12] = [
    0.7, 0.7, 168.0, 160.0, 0.7, 0.7, 168.0, 192.0, 0.7, 0.7, 0.0, 256.0,
];
/// `gmCamera_803DA630`: the save menu's texts' (slot A's, slot B's, the blocks' line).
const MENU_LAYOUT: [f32; 12] = [
    0.6, 0.6, 40.0, 416.0, 0.6, 0.6, 340.0, 416.0, 0.6, 0.6, 40.0, 44.0,
];

/// What a slot's listing found, for the texts (`gmCameraUnkStruct4`): the card's result,
/// its free blocks and files (as the last listing that answered left them), and what the
/// screen makes of it (`xC`: 0 or 1 room on this slot, 2 not enough blocks, 3 no file
/// entries, 4 a card that can't be used, 5 the wrong encoding).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct CamSlot {
    pub result: i32,
    pub free_blocks: i32,
    pub free_files: i32,
    pub verdict: i32,
}

/// `gmCamera_VsCamUiState`: the camera's menu in the match.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct CamUi {
    /// `IfVsCam` (`ifvscam`).
    pub archive: Option<ArchiveId>,
    /// `x4`: the info's model (`IfCameraInfo_Top`), `x8`: the menu's (`IfCamera_Top`).
    pub info: Option<JObjId>,
    pub model: Option<JObjId>,
    /// `xC`: the menu's state ([`STATES`]).
    pub state: i32,
    /// `x10`: frames since the snapshot was asked for.
    pub frames: i32,
    /// `x14`: whether the info shows (B toggles it).
    pub show_info: i32,
    /// `x18`: yes (0) or no (1) on the save's question.
    pub choice: i32,
    /// `snap_image`: the frame's copy, once taken.
    pub snap_image: u32,
    /// `x20`: the blocks the snapshot takes.
    pub blocks: i32,
    pub slots: [CamSlot; 2],
    /// `x44`: the better slot's verdict.
    pub verdict: i32,
    /// `x48`: slot A's, slot B's and the blocks' texts.
    pub texts: [Option<TextId>; 3],
    /// `x54`: the font's canvas.
    pub canvas: i32,
}

/// `cmsnap_data`: the frame's copy's progress (`CmSnapStatus`: 0 asleep, 1 asked, 2 copied,
/// 3 and 4 waiting, 5 done) and its image (`HSD_ImageDesc`, 640 by 480 RGB565 in the preload
/// cache's buffer).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct CmSnap {
    pub status: i32,
    pub image: u32,
}

/// Camera Mode's statics: the card check's exit (`gm_804D68C8`'s first word: 0 on to the
/// selects, 1 back, 2 L, R and Start), the check's texts (`gmCamera_VsCameraTextLayout`), the
/// match's menu and the frame's copy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct CameraMode {
    pub check_exit: u32,
    pub check_texts: [Option<TextId>; 3],
    pub ui: CamUi,
    pub snap: CmSnap,
}

/// The menu's states (`gmCamera_VsCamStateTable`): whether the info shows in it, the model's
/// parts it shows (bits 0 to 15), what entering it does and what each frame does.
#[derive(Clone, Copy)]
struct CamState {
    info: bool,
    parts: u16,
    enter: Option<fn(&mut Running)>,
    update: Option<fn(&mut Running)>,
}

const STATES: [CamState; 9] = [
    CamState {
        info: true,
        parts: 0,
        enter: Some(|r| r.cam_resume()),
        update: Some(|r| r.cam_watch()),
    },
    CamState {
        info: false,
        parts: 4,
        enter: Some(|r| r.cam_shoot()),
        update: Some(|r| r.cam_wait_copy()),
    },
    CamState {
        info: false,
        parts: 2,
        enter: Some(|r| r.cam_offer()),
        update: Some(|r| r.cam_offer_update()),
    },
    CamState {
        info: false,
        parts: 0x54,
        enter: None,
        update: Some(|r| r.cam_back_on_b()),
    },
    CamState {
        info: false,
        parts: 0x34,
        enter: None,
        update: Some(|r| r.cam_back_on_b()),
    },
    CamState {
        info: false,
        parts: 0x94,
        enter: None,
        update: Some(|r| r.cam_back_on_b()),
    },
    CamState {
        info: false,
        parts: 0x1204,
        enter: Some(|r| r.cam_ask()),
        update: Some(|r| r.cam_ask_update()),
    },
    CamState {
        info: false,
        parts: 0x1604,
        enter: Some(|r| r.cam_save()),
        update: Some(|r| r.cam_save_update()),
    },
    CamState {
        info: false,
        parts: 0x1A14,
        enter: None,
        update: Some(|r| r.cam_back_on_b()),
    },
];

/// The font's string table's digits (`HSD_SisLib_804D1124[3][2]`, two bytes a digit) written
/// at `dst` for `value` (at most 9999), with a terminator; where the terminator went.
/// `gmCamera_801A2224`.
#[doc(alias = "gmCamera_801A2224")]
fn write_number(r: &mut Running, dst: SisPtr, value: u32) -> SisPtr {
    let digits = r
        .game
        .sis
        .table_entry(r.data, FONT as usize, 2)
        .expect("the digits' glyphs");
    let step = |p: SisPtr, n: u32| match p {
        SisPtr::Arena(a) => SisPtr::Arena(a + n),
        SisPtr::Data(d) => SisPtr::Data(d.at(n)),
        SisPtr::Static(a) => SisPtr::Static(a + n),
    };
    let mut d = dst;
    let mut put = |r: &mut Running, digit: u32| {
        let s = &mut r.game.sis;
        let hi = s.sis_byte(r.data, step(digits, 2 * digit));
        let lo = s.sis_byte(r.data, step(digits, 2 * digit + 1));
        s.set_sis_byte(d, hi);
        s.set_sis_byte(step(d, 1), lo);
        d = step(d, 2);
    };
    let mut value = value.min(9999);
    let mut shown = 0;
    if value >= 1000 {
        put(r, value / 1000);
        value %= 1000;
        shown = 1;
    }
    if value >= 100 || shown != 0 {
        put(r, value / 100);
        value %= 100;
        shown += 1;
    }
    if value >= 10 || shown != 0 {
        put(r, value / 10);
        value %= 10;
    }
    put(r, value);
    r.game.sis.set_sis_byte(d, 0);
    d
}

impl Running<'_> {
    fn cam(&mut self) -> &mut CameraMode {
        &mut self.game.camera_mode
    }

    // --- The mode's states (`gmcameramode.c`).

    /// The card check's entry, `gm_801B24B4`: the preload cache set up for VS with Camera
    /// Mode's game cache (the frame's copy and the snapshot's buffers), the card's work areas,
    /// and the snapshot's and slots' buffers handed to `lbsnap`.
    #[doc(alias = "gm_801B24B4")]
    pub fn camera_enter_check(&mut self) {
        self.camera_preload();
        self.card_alloc_work_area();
        self.camera_snap_buffers();
    }

    /// `lbDvd_SetupVsPreloadCache`, then the game cache marked Camera Mode's and the cache
    /// refreshed (`lbDvd_80018254`).
    fn camera_preload(&mut self) {
        self.setup_vs_preload_cache();
        self.dvd_mut().cache.scene.game_cache.mode_kind = GM_CAMERA_MODE;
        self.preload_refresh();
    }

    /// `lbSnap_8001E218` with the preload cache's buffers (`lbDvd_GetPreloadedArchive` of
    /// 2008, then 2007, as the machine code takes them).
    fn camera_snap_buffers(&mut self) {
        let slot = self.take_cached(SNAP_SLOTS).expect("the slots' buffer");
        let snap = self.take_cached(SNAP_DATA).expect("the snapshot's buffer");
        self.snap_set_buffers(snap, slot);
    }

    /// The card check's exit, `gm_801B2510`: back to the main menu on B or L, R and Start.
    #[doc(alias = "gm_801B2510")]
    pub fn camera_exit_check(&mut self) {
        if matches!(self.game.camera_mode.check_exit, 1 | 2) {
            self.change_game_mode_after_scene(GM_MENU);
        }
    }

    /// The character select's entry, `gm_801B254C`: its data from Camera Mode's table entry
    /// with match type 1, and the preload cache as the check's entry sets it.
    #[doc(alias = "gm_801B254C")]
    pub fn camera_enter_css(&mut self) {
        let vs = self.vs_table(TABLE).clone();
        let css = &mut self.game.vs_melee.css;
        css.match_type = VS_CAMERA;
        css.ko_counts_null = false;
        css.vs.clone_from(&vs);
        self.camera_preload();
    }

    /// The character select's exit, `gm_801B25D4`: back to the main menu on L, R and Start,
    /// else the table entry from the screen's and the six players' characters' sound banks.
    #[doc(alias = "gm_801B25D4")]
    pub fn camera_exit_css(&mut self) {
        if self.game.vs_melee.css.pending_scene_change == 2 {
            self.change_game_mode_after_scene(GM_MENU);
            return;
        }
        let vs = self.game.vs_melee.css.vs.clone();
        self.vs_table(TABLE).clone_from(&vs);
        let mut mask = 0u64;
        for p in &vs.start.players[..6] {
            mask |= crate::lb::audio::character_banks(i32::from(p.ckind));
        }
        self.game.audio.lb.unwant_banks(20);
        self.game.audio.lb.want_banks(4, mask);
        self.game.audio.lb_start_bank_loads(self.data);
    }

    /// The stage select's entry, `gm_801B26AC`.
    #[doc(alias = "gm_801B26AC")]
    pub fn camera_enter_sss(&mut self) {
        let vs = self.vs_table(TABLE).clone();
        self.game.vs_melee.sss.vs.clone_from(&vs);
        self.sss_stage_rule();
    }

    /// The stage select's exit, `gm_801B2704`: back to the character select (state 1) when no
    /// stage was picked.
    #[doc(alias = "gm_801B2704")]
    pub fn camera_exit_sss(&mut self) {
        self.vs_melee_exit_sss(TABLE, 1);
    }

    /// The match's entry, `gm_PrepCameraModeVSScene`: the saved rules applied to the table
    /// entry, a time match without a timer (the entry's timer flag cleared too) whose pause
    /// belongs to the camera's controller and whose frames run the camera's menu, every
    /// player marked (`xD_b3`), the sub-colours, rumble and announcer, the card's work areas
    /// and the snapshot's buffers.
    #[doc(alias = "gm_PrepCameraModeVSScene")]
    pub fn camera_enter_vs(&mut self) {
        let (rules, prefs) = super::attract::saved_rules(&self.game.game_data);
        let mut vs = self.vs_table(TABLE).start.clone();
        vs_mode::apply_menu_rules(&mut vs, &rules, &prefs);
        let mut start = crate::game::structs::StartMeleeData::default();
        start.rules.clone_from(&vs.rules);
        let r = &mut start.rules;
        r.match_kind = 0;
        vs.rules.x4_4 = false;
        r.timer_enabled = false;
        r.x5_0 = true;
        r.x1_2 = true;
        r.x1_3 = true;
        r.x2_5 = false;
        r.x2_6 = false;
        r.x3_1 = false;
        r.x4_0 = false;
        r.on_unpause_override = Some(CodeRef(PAUSE_CAMERA));
        r.on_pause_override = Some(CodeRef(PAUSE_CAMERA));
        r.check_for_pauser_override = Some(CodeRef(GET_PAUSER));
        r.on_match_start = Some(CodeRef(MATCH_START));
        r.on_frame_start = Some(CodeRef(FRAME_START));
        r.on_frame_end = Some(CodeRef(FRAME_END));
        r.xD = 1;
        r.disable_pausing = false;
        for (p, v) in start.players.iter_mut().zip(&vs.players) {
            p.clone_from(v);
            p.xD_b3 = true;
        }
        vs_mode::setup_sub_colors(&mut start);
        let tags = self.nametag_rumbles();
        vs_mode::load_rumble_enabled_with(&mut start, &prefs, &tags);
        self.vs_table(TABLE).start.clone_from(&vs);
        self.game.vs_melee.start.clone_from(&start);
        self.load_announcer();
        self.card_alloc_work_area();
        self.camera_snap_buffers();
    }

    /// The match's exit, `gm_801B2AF8`: the match's records (`gm_80168638`, `gm_80168710`,
    /// `gm_8016247C`, the KO counts), then back to the character select.
    #[doc(alias = "gm_801B2AF8")]
    pub fn camera_exit_vs(&mut self) {
        let me = self.game.vs_melee.exit.match_end.clone();
        self.results_handicaps(&me);
        self.results_draw_winner(&me, TABLE);
        self.results_add_coins(super::results_exit::human_kos(&me));
        super::results_exit::update_ko_counts(&mut self.game.vs_melee.ko_counts, &me);
        self.set_next_game_mode_state_id(1);
    }

    // --- The card check's scene (`gm_1A33.c`).

    /// `gm_Scene_CameraVs_OnEnter`: the font's strings and canvas (`gmCamera_801A2650`), no
    /// texts yet, then the slots' texts.
    #[doc(alias = "gm_Scene_CameraVs_OnEnter")]
    pub fn camera_check_on_enter(&mut self) {
        self.camera_load_font();
        self.cam().check_texts = [None; 3];
        self.camera_check_texts();
    }

    /// `gm_Scene_CameraVs_OnExit`: nothing.
    #[doc(alias = "gm_Scene_CameraVs_OnExit")]
    pub fn camera_check_on_exit(&mut self) {}

    /// `gmCamera_801A2650`: two blocks a snapshot for now, the menu's strings (`SdVsCam`) as
    /// font 3, and its canvas.
    #[doc(alias = "gmCamera_801A2650")]
    fn camera_load_font(&mut self) {
        self.cam().ui.blocks = 2;
        let name = if self.saved_us() {
            "SdVsCam.usd"
        } else {
            "SdVsCam.dat"
        };
        self.load_font_named(FONT, name, "SIS_VsCameraData");
        self.cam().ui.canvas = self.add_canvas(FONT, None, 9, 13, 0, 14, 0, 11);
    }

    /// `gm_Scene_CameraVs_OnFrame`: a card in or out of a slot lists both again with their
    /// texts; else L, R and Start back to the main menu (2), Start or A on to the selects
    /// (0), B back (1).
    #[doc(alias = "gm_Scene_CameraVs_OnFrame")]
    #[doc(alias = "mn_8022F218")]
    #[doc(alias = "mn_8022F268")]
    pub fn camera_check_on_frame(&mut self) {
        if self.snap_changed(0) != 0 || self.snap_changed(1) != 0 {
            for i in 0..3 {
                if let Some(t) = self.cam().check_texts[i].take() {
                    self.destroy_text(t);
                }
            }
            self.camera_check_texts();
        } else if self.game.buttons_triggered(4) & PAD_LR_START != 0 {
            self.lb_audio_call(SFX_BACK);
            self.game.game_data.x1850.force_main_menu = 1;
            self.cam().check_exit = 2;
            self.game.end_scene();
        } else if self.game.buttons_triggered(4) & (PAD_START | PAD_A) != 0 {
            self.lb_audio_call(SFX_FORWARD);
            self.cam().check_exit = 0;
            self.game.end_scene();
        } else if self.game.buttons_triggered(4) & PAD_B != 0 {
            self.lb_audio_call(SFX_BACK);
            self.cam().check_exit = 1;
            self.game.end_scene();
        }
    }

    /// `gmCamera_801A33BC`: both slots listed, their texts, and the line under them: the
    /// blocks free on the slot with room, or why neither has.
    #[doc(alias = "gmCamera_801A33BC")]
    #[doc(alias = "gmCamera_801A2640")]
    fn camera_check_texts(&mut self) {
        self.camera_list_slots();
        let l = CHECK_LAYOUT;
        let a = self.camera_slot_text(0, l[2], l[3], l[0], l[1]);
        self.cam().check_texts[0] = Some(a);
        let b = self.camera_slot_text(1, l[6], l[7], l[4], l[5]);
        self.cam().check_texts[1] = Some(b);
        let (verdict, blocks) = self.camera_verdict();
        let canvas = self.cam().ui.canvas;
        let t = self.create_text(FONT, canvas, Vec3::new(l[10], l[11], 0.0), 914.2857, 64.0);
        self.cam().check_texts[2] = Some(t);
        let x = self.game.sis.text_mut(t);
        x.default_alignment = 1;
        x.default_kerning = 1;
        x.default_fitting = 1;
        x.font_size = [l[8], l[9]];
        self.game.sis.reset_text(self.data, t, 0x15);
        let font = FONT as usize;
        match verdict {
            0 | 1 => {
                let at = self.game.sis.copy_table_string(self.data, font, 0x15, 0x11);
                write_number(self, at, blocks as u32);
                self.game
                    .sis
                    .append_table_string(self.data, font, 0x15, 0x12);
            }
            2 => {
                self.game.sis.copy_table_string(self.data, font, 0x15, 0x13);
            }
            _ => {
                self.game.sis.copy_table_string(self.data, font, 0x15, 0x14);
            }
        }
    }

    /// `gmCamera_801A25C8`: each slot listed (`lbSnap_8001D40C`), with its free blocks and
    /// files when it answered.
    #[doc(alias = "gmCamera_801A25C8")]
    fn camera_list_slots(&mut self) {
        for i in 0..2 {
            let r = self.snap_list(i);
            self.cam().ui.slots[i].result = r;
            if r == 0 {
                let blocks = self.snap_free_blocks(i);
                let files = self.snap_free_files(i);
                let s = &mut self.cam().ui.slots[i];
                s.free_blocks = blocks;
                s.free_files = files;
            }
        }
    }

    /// `gmCamera_801A253C`: the better slot's verdict, and the most free blocks of a slot
    /// with room (slot A's when its verdict is 1, slot B's when 0).
    #[doc(alias = "gmCamera_801A253C")]
    fn camera_verdict(&mut self) -> (i32, i32) {
        let ui = &mut self.cam().ui;
        ui.verdict = ui.slots[0].verdict.min(ui.slots[1].verdict);
        let a = if ui.slots[0].verdict == 1 {
            ui.slots[0].free_blocks
        } else {
            0
        };
        let b = if ui.slots[1].verdict == 0 {
            ui.slots[1].free_blocks
        } else {
            0
        };
        (ui.verdict, a.max(b))
    }

    /// `gmCamera_801A2334`: slot `chan`'s text at (`x`, `y`) in size (`w`, `h`): its blocks
    /// free when a snapshot fits, else why not, with the slot's verdict.
    #[doc(alias = "gmCamera_801A2334")]
    fn camera_slot_text(&mut self, chan: usize, x: f32, y: f32, w: f32, h: f32) -> TextId {
        let canvas = self.cam().ui.canvas;
        let t = self.create_text(FONT, canvas, Vec3::new(x, y, 0.0), 640.0, 32.0);
        let s = self.game.sis.text_mut(t);
        s.font_size = [w, h];
        s.default_kerning = 1;
        let font = FONT as usize;
        let (group, digits) = if chan == 0 {
            (6, self.game.sis.copy_table_string(self.data, font, 6, 8))
        } else {
            (7, self.game.sis.copy_table_string(self.data, font, 7, 9))
        };
        let slot = self.cam().ui.slots[chan];
        let (verdict, label) = match slot.result {
            0 if slot.free_files == 0 => (3, 0xC),
            0 if slot.free_blocks == 0 => (2, 0xC),
            0 => {
                let v = if slot.free_blocks >= self.cam().ui.blocks {
                    i32::from(chan == 0)
                } else {
                    2
                };
                write_number(self, digits, slot.free_blocks as u32);
                (v, if slot.free_blocks != 1 { 0xB } else { 0xA })
            }
            15 => (5, 0x10),
            9 => (4, 0xD),
            12 => (4, 0xE),
            _ => (4, 0xF),
        };
        self.cam().ui.slots[chan].verdict = verdict;
        self.game
            .sis
            .append_table_string(self.data, font, group, label);
        self.game.sis.reset_text(self.data, t, group);
        t
    }

    // --- The match's menu (`gmcamera.c`).

    /// The match's start, `gmCamera_801A31FC`: the frame's copy set up (`cmSnap_800316B4`),
    /// the menu in its first state with the info showing, `IfVsCam`'s info model and the
    /// menu's model (hidden, animated by its process), and the font and canvas.
    #[doc(alias = "gmCamera_801A31FC")]
    pub fn camera_match_start(&mut self) {
        self.cm_snap_setup();
        self.cam().ui.show_info = 1;
        self.camera_set_state(0);
        let name = if self.saved_us() {
            "IfVsCam.usd"
        } else {
            "IfVsCam.dat"
        };
        let archive = self.load_archive(name);
        self.cam().ui.archive = Some(archive);
        let data = self.data;
        let a = data.get(archive);
        let at = |sym: &str| {
            let offset = a.public(sym).unwrap_or_else(|| panic!("IfVsCam's {sym}"));
            DataRef { archive, offset }
        };
        let info_desc = at("IfCameraInfo_Top_model_set");
        let model_desc = at("IfCamera_Top_model_set");
        let g = &mut *self.game;
        let gobj = g.world.create(0xE, 0x10, 0);
        let info = jobj::load_joint(&mut g.objs, data, data.ptr(info_desc));
        g.camera_mode.ui.info = info;
        g.world
            .set_obj(gobj, OBJ_JOBJ, Obj::JObj(info.expect("the info's model")));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_RENDER), 0xB, 0);
        let gobj = g.world.create(0xE, 0x10, 0);
        let model = jobj::load_joint(&mut g.objs, data, data.ptr(model_desc));
        g.camera_mode.ui.model = model;
        g.world
            .set_obj(gobj, OBJ_JOBJ, Obj::JObj(model.expect("the menu's model")));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_RENDER), 0xB, 0);
        super::vs::add_model_anim(&mut g.objs, data, model, model_desc, 0);
        jobj::req_anim_all(&mut g.objs, model, 0.0);
        jobj::anim_all(
            &mut g.objs,
            data,
            &mut crate::game::code::ParticleHooks,
            model,
        );
        jobj::set_flags_all(&mut g.objs.jobjs, model, HIDDEN);
        g.world.setup_proc(gobj, CodeRef(MODEL_PROC), 0);
        // `gmCamera_801A2650`'s body, inline.
        self.camera_load_font();
        self.cam().ui.texts = [None; 3];
    }

    /// The menu's model's process, `fn_801A31D8`: its animations a frame on.
    #[doc(alias = "fn_801A31D8")]
    pub fn camera_model_proc(&mut self, gobj: GObjId) {
        let j = match self.game.world.gobj(gobj).obj {
            Some(Obj::JObj(j)) => Some(j),
            _ => None,
        };
        jobj::anim_all(
            &mut self.game.objs,
            self.data,
            &mut crate::game::code::ParticleHooks,
            j,
        );
    }

    /// `gmCamera_801A3048`: the menu goes to state `i`, entering it.
    #[doc(alias = "gmCamera_801A3048")]
    fn camera_set_state(&mut self, i: i32) {
        self.cam().ui.state = i;
        if let Some(f) = STATES[i as usize].enter {
            f(self);
        }
    }

    /// The frame's start, `gmCamera_801A3098`: the menu's state's update.
    #[doc(alias = "gmCamera_801A3098")]
    pub fn camera_frame_start(&mut self) {
        let i = self.cam().ui.state as usize;
        if let Some(f) = STATES[i].update {
            f(self);
        }
    }

    /// The frame's end, `gmCamera_801A30E4`: the info shown in a state that shows it while B
    /// hasn't hidden it and nobody pauses, the menu's model hidden but for the state's parts.
    #[doc(alias = "gmCamera_801A30E4")]
    pub fn camera_frame_end(&mut self) {
        let ui = self.game.camera_mode.ui;
        let st = STATES[ui.state as usize];
        let paused = self.game.db_pause_flag(1);
        let j = &mut self.game.objs.jobjs;
        if st.info && ui.show_info != 0 && !paused {
            jobj::clear_flags_all(j, ui.info, HIDDEN);
        } else {
            jobj::set_flags_all(j, ui.info, HIDDEN);
        }
        jobj::set_flags_all(j, ui.model, HIDDEN);
        for i in 0..16 {
            if st.parts & (1 << i) != 0 {
                let part = super::pause::tree_nodes(j, ui.model, &[i])[0];
                jobj::clear_flags_all(j, part, HIDDEN);
            }
        }
    }

    /// The menu's texts freed, if it has them.
    fn camera_free_texts(&mut self) {
        if self.cam().ui.texts[0].is_some() {
            for i in 0..3 {
                if let Some(t) = self.cam().ui.texts[i].take() {
                    self.destroy_text(t);
                }
            }
        }
    }

    /// State 0's entry, `gmCamera_801A26C0`: after a snapshot, the match goes on (the pause
    /// screen again if a player had paused, else the music, the HUD and rumble), and the
    /// menu's texts go.
    #[doc(alias = "gmCamera_801A26C0")]
    fn cam_resume(&mut self) {
        if self.game.db_pause_flag(3) {
            self.game.clear_db_pause_flag(3);
            if self.game.db_pause_flag(1) {
                let pauser = i32::from(self.game.vs.state.pauser);
                let g = &mut *self.game;
                super::pause::show(&mut g.objs, self.data, &mut g.gm.pause, pauser, 1);
            } else {
                self.lb_audio_hold(false);
                self.show_hud_parts();
                self.game.pads.rumble.unpause_all();
            }
            let s = &mut self.game.vs.state;
            s.hud_enabled = 1;
            s.unk_3 = 0;
        }
        self.camera_free_texts();
    }

    /// State 0's frame, `gmCamera_801A2798`: B on the camera's controller shows or hides the
    /// info, Z takes a snapshot.
    #[doc(alias = "gmCamera_801A2798")]
    fn cam_watch(&mut self) {
        let trigger = self.game.pads.copy[CAMERA_PORT].trigger;
        if trigger & PAD_B as u32 != 0 {
            let ui = &mut self.cam().ui;
            ui.show_info = (ui.show_info + 1) % 2;
            return;
        }
        if trigger & PAD_Z as u32 != 0 {
            self.lb_audio_call(6);
            self.camera_set_state(1);
        }
    }

    /// State 1's entry, `gmCamera_801A2800`: the shutter's animation, the match frozen (the
    /// pause screen hidden if a player had paused, else the music, the HUD and rumble), and
    /// the frame's copy asked for.
    #[doc(alias = "gmCamera_801A2800")]
    fn cam_shoot(&mut self) {
        self.cam().ui.frames = 0;
        let model = self.game.camera_mode.ui.model;
        let part = super::pause::tree_nodes(&self.game.objs.jobjs, model, &[2])[0];
        jobj::req_anim_all(&mut self.game.objs, part, 0.0);
        self.game.set_db_pause_flag(3);
        if self.game.db_pause_flag(1) {
            let pauser = i32::from(self.game.vs.state.pauser);
            self.game.hide_pause(pauser);
        } else {
            self.lb_audio_hold(true);
            self.hide_hud_parts();
            self.game.pads.rumble.pause_all();
        }
        let s = &mut self.game.vs.state;
        s.hud_enabled = 0;
        s.unk_3 = 1;
        self.cm_snap_request();
    }

    /// State 1's frame, `gmCamera_801A28AC`: from the third frame, once the copy is done, the
    /// snapshot made from it (`lbSnap_8001DC0C`): its blocks, and the save's offer; or, when
    /// it couldn't be made, back to watching.
    #[doc(alias = "gmCamera_801A28AC")]
    fn cam_wait_copy(&mut self) {
        self.cam().ui.frames += 1;
        if self.cam().ui.frames > 2 {
            let image = self.cm_snap_take();
            self.cam().ui.snap_image = image;
            if image != 0 {
                if self.snap_make(image) != 0 {
                    let blocks = self.snap_blocks();
                    self.cam().ui.blocks = blocks;
                    self.camera_set_state(2);
                } else {
                    self.camera_set_state(0);
                }
            }
        }
    }

    /// State 2's entry, `gmCamera_801A292C`: the slots listed again, the line with the
    /// snapshot's blocks and each slot's text.
    #[doc(alias = "gmCamera_801A292C")]
    fn cam_offer(&mut self) {
        self.camera_free_texts();
        self.camera_list_slots();
        let l = MENU_LAYOUT;
        let canvas = self.cam().ui.canvas;
        let t = self.create_text(FONT, canvas, Vec3::new(l[10], l[11], 0.0), 640.0, 32.0);
        let s = self.game.sis.text_mut(t);
        s.font_size = [l[8], l[9]];
        s.default_kerning = 1;
        let font = FONT as usize;
        let at = self.game.sis.copy_table_string(self.data, font, 3, 4);
        let blocks = self.cam().ui.blocks;
        write_number(self, at, blocks as u32);
        self.game.sis.append_table_string(self.data, font, 3, 5);
        self.game.sis.reset_text(self.data, t, 3);
        self.cam().ui.texts[2] = Some(t);
        let a = self.camera_slot_text(0, l[2], l[3], l[0], l[1]);
        self.cam().ui.texts[0] = Some(a);
        let b = self.camera_slot_text(1, l[6], l[7], l[4], l[5]);
        self.cam().ui.texts[1] = Some(b);
        let ui = &mut self.cam().ui;
        ui.verdict = ui.slots[0].verdict.min(ui.slots[1].verdict);
    }

    /// State 2's frame, `gmCamera_801A2AAC`: a card in or out lists the slots again; A asks
    /// whether to save when a slot has room, else says why not; B goes back to watching.
    #[doc(alias = "gmCamera_801A2AAC")]
    fn cam_offer_update(&mut self) {
        let trigger = u64::from(self.game.pads.copy[CAMERA_PORT].trigger);
        if self.snap_changed(0) != 0 || self.snap_changed(1) != 0 {
            self.camera_set_state(2);
        } else if trigger & PAD_A != 0 {
            self.lb_audio_call(SFX_FORWARD);
            let v = self.cam().ui.verdict;
            if matches!(v, 0 | 1) {
                self.camera_set_state(6);
                return;
            }
            self.lb_audio_call(3);
            match v {
                5 => self.camera_set_state(3),
                2 | 3 => self.camera_set_state(4),
                _ => self.camera_set_state(5),
            }
        } else if trigger & PAD_B != 0 {
            self.lb_audio_call(SFX_BACK);
            self.camera_set_state(0);
        }
    }

    /// States 3, 4, 5 and 8's frame, `gmCamera_801A2BB0`: B goes back to watching.
    #[doc(alias = "gmCamera_801A2BB0")]
    fn cam_back_on_b(&mut self) {
        if u64::from(self.game.pads.copy[CAMERA_PORT].trigger) & PAD_B != 0 {
            self.lb_audio_call(SFX_BACK);
            self.camera_set_state(0);
        }
    }

    /// The question's cursor at yes (left) or no (right): its part moved there.
    fn camera_place_choice(&mut self) {
        let x = if self.cam().ui.choice != 0 { 5.0 } else { -5.0 };
        let model = self.game.camera_mode.ui.model;
        let j = &mut self.game.objs.jobjs;
        if let Some(c) = super::pause::tree_nodes(j, model, &[0xC])[0] {
            jobj::set_srt(j, c, |o| o.translate.x = x);
        }
    }

    /// State 6's entry, `gmCamera_801A2BF0`: the question's slot shown (frame 1 for slot A,
    /// 2 for B, its texture animations stopped there), the cursor on yes.
    #[doc(alias = "gmCamera_801A2BF0")]
    fn cam_ask(&mut self) {
        let model = self.game.camera_mode.ui.model;
        let part = super::pause::tree_nodes(&self.game.objs.jobjs, model, &[9])[0];
        let frame = if self.cam().ui.verdict == 1 { 1.0 } else { 2.0 };
        jobj::req_anim_all(&mut self.game.objs, part, frame);
        jobj::anim_all(
            &mut self.game.objs,
            self.data,
            &mut crate::game::code::ParticleHooks,
            part,
        );
        if let Some(j) = part {
            let data = self.data;
            jobj::foreach_anim(
                &mut self.game.objs,
                j,
                jobj::type_mask::TOBJ,
                &mut |objs, a| {
                    crate::hsd::aobj::stop_anim(objs, data, Some(a), None);
                },
            );
        }
        self.cam().ui.choice = 0;
        self.camera_place_choice();
    }

    /// State 6's frame, `gmCamera_801A2D44`: a card in or out goes back to the offer; A or
    /// Start saves on yes and goes back to watching on no; B goes back; left and right move
    /// the cursor.
    #[doc(alias = "gmCamera_801A2D44")]
    fn cam_ask_update(&mut self) {
        if self.snap_changed(0) != 0 || self.snap_changed(1) != 0 {
            self.camera_set_state(2);
            return;
        }
        let trigger = u64::from(self.game.pads.copy[CAMERA_PORT].trigger);
        if trigger & (PAD_A | PAD_START) != 0 {
            if self.cam().ui.choice == 0 {
                self.lb_audio_call(SFX_FORWARD);
                self.camera_set_state(7);
            } else {
                self.lb_audio_call(SFX_BACK);
                self.camera_set_state(0);
            }
            return;
        }
        if trigger & PAD_B != 0 {
            self.lb_audio_call(SFX_BACK);
            self.camera_set_state(0);
            return;
        }
        if trigger & (PAD_LEFT | PAD_STICK_LEFT) != 0 && self.cam().ui.choice != 0 {
            self.lb_audio_call(SFX_MOVE);
            self.cam().ui.choice = 0;
            self.camera_place_choice();
            return;
        }
        if trigger & (PAD_RIGHT | PAD_STICK_RIGHT) != 0 && self.cam().ui.choice != 1 {
            self.lb_audio_call(SFX_MOVE);
            self.cam().ui.choice = 1;
            self.camera_place_choice();
        }
    }

    /// State 7's entry, `gmCamera_801A2FBC`: the snapshot's save started on slot B when only
    /// it has room (verdict 0), else on slot A (`lbSnap_8001DF6C`).
    #[doc(alias = "gmCamera_801A2FBC")]
    fn cam_save(&mut self) {
        let chan = if self.cam().ui.verdict == 0 { 1 } else { 0 };
        self.snap_save(chan);
    }

    /// State 7's frame, `gmCamera_801A2FFC`: the save's next step; done, back to watching,
    /// or on a failure the failure's sound and its message.
    #[doc(alias = "gmCamera_801A2FFC")]
    fn cam_save_update(&mut self) {
        let r = self.snap_save_step();
        if r != crate::lb::save::result::BUSY {
            if r == 0 {
                self.camera_set_state(0);
            } else {
                self.lb_audio_call(3);
                self.camera_set_state(8);
            }
        }
    }

    // --- The frame's copy (`melee/cm/cmsnap.c`).

    /// `cmSnap_800316B4`: asleep, no image, its render callback on a GObj of its own
    /// (class 0x12, link 0x12, the cameras' link at priority 4), and its image the preload
    /// cache's 640 by 480 RGB565 buffer (`lb_800121FC`).
    #[doc(alias = "cmSnap_800316B4")]
    fn cm_snap_setup(&mut self) {
        self.cam().snap = CmSnap::default();
        let g = &mut *self.game;
        let gobj = g.world.create(0x12, 0x12, 0);
        g.world
            .setup_gx_link_max(gobj, CodeRef(SNAP_RENDER), 4, false);
        let image = self
            .take_cached(SNAP_COPY)
            .expect("the frame's copy's buffer");
        self.cam().snap.image = image;
    }

    /// `cmSnap_800315C8`: the copy asked for (it must be asleep).
    #[doc(alias = "cmSnap_800315C8")]
    fn cm_snap_request(&mut self) {
        assert_eq!(
            self.cam().snap.status,
            0,
            "the original's assertion failed: cmsnap.c:0x55"
        );
        self.cam().snap.status = 1;
    }

    /// `cmSnap_80031618`: the copy's image once it's done (asleep again), else 0.
    #[doc(alias = "cmSnap_80031618")]
    fn cm_snap_take(&mut self) -> u32 {
        let s = &mut self.cam().snap;
        if s.status != 5 {
            return 0;
        }
        s.status = 0;
        s.image
    }

    /// The copy's render callback, `cmSnap_80031640`: asked, the frame copied into the image
    /// (`lb_800122C8`); then three display passes on to done.
    #[doc(alias = "cmSnap_80031640")]
    pub fn cm_snap_render(&mut self) {
        match self.cam().snap.status {
            1 => {
                self.cm_snap_copy();
                self.cam().snap.status = 2;
            }
            2..=4 => self.cam().snap.status += 1,
            _ => {}
        }
    }
}

impl crate::game::state::Game {
    /// `gm_CameraModeVSGetPauser`: the first controller pressing Start that the camera's
    /// (the fourth) or a player's is, or -1.
    #[doc(alias = "gm_CameraModeVSGetPauser")]
    pub fn camera_pauser(&self) -> i32 {
        self.assert_retail();
        for port in 0..4 {
            let p = &self.pads.copy[port];
            if p.err != 0 || p.trigger & PAD_START as u32 == 0 {
                continue;
            }
            if port == CAMERA_PORT {
                return port as i32;
            }
            for slot in 0..6 {
                if port as i32 == self.player_port(slot)
                    && self.player_pkind(slot) != super::super::pl::player::PKIND_NONE
                {
                    return port as i32;
                }
            }
        }
        -1
    }
}

/// The game modes' functions the match calls by address for Camera Mode.
pub fn call(
    r: &mut Running,
    f: u32,
    _args: &[super::super::code::Val],
) -> Option<super::super::code::Val> {
    use super::super::code::Val;
    match f {
        PAUSE_CAMERA => super::super::gm::pause_camera(r),
        GET_PAUSER => return Some(Val::Int(r.game.camera_pauser())),
        MATCH_START => r.camera_match_start(),
        FRAME_START => r.camera_frame_start(),
        FRAME_END => r.camera_frame_end(),
        _ => return None,
    }
    Some(Val::None)
}
