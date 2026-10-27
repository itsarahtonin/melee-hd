//! The game's side of the memory card (`melee/lb/lbcardgame.c`): the save's file
//! (`SuperSmashBros0110290334`, the card data's eight logical files), its comment and icon,
//! the card's status as the game tracks it, the override that sends the game to the memory
//! card scene when the card needs the player (`lbCardGame_DecideGameMode`), the save's
//! writer each frame (`lb_8001CC84`), and the card indicator scenes draw while it writes.

use melee_data::archive::DataRef;

use super::super::game::handles::CodeRef;
use super::super::game::state::{OBJ_CAMERA, OBJ_JOBJ, Obj, Running};
use super::save::result;
use crate::hsd::{cobj, jobj};

/// `LbCardStatus`: 0 the card holds the save as the game knows it; 1 the card was swapped
/// (the probe changed) since; 2 a load failed; 3 the player chose to go on without saving
/// with a card in; 4 the same with none in.
pub const STATUS_OK: i32 = 0;
pub const STATUS_SWAPPED: i32 = 1;
pub const STATUS_FAILED: i32 = 2;
pub const STATUS_NO_SAVE: i32 = 3;
pub const STATUS_NO_CARD: i32 = 4;

/// The card indicator's camera's render callback (`gobj0_RenderFunc`) and its model's
/// process (`gobj1_Proc`).
pub const INDICATOR_RENDER: u32 = 0x8001_CEE4;
pub const INDICATOR_PROC: u32 = 0x8001_CEC0;

/// `HSD_GOBJ_CLASS_MEMCARD`, `HSD_GOBJ_PLINK_MEMCARD`.
const GOBJ_CLASS: u16 = 24;
const GOBJ_PLINK: u8 = 61;

/// `lbcardgame.c`'s `state`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct CardGame {
    /// Whether a card was in slot A at the pad alarm's last probe (`CARDProbe(0)`).
    pub probe_status: bool,
    /// The probe changed, or a task list cleared it (`unk_status`, the lists' `status_out`).
    pub unk_status: bool,
    pub card_status: i32,
    /// The save changed and waits to be written.
    pub dirty: bool,
    /// The writer's state: 0 idle, 1 a write running.
    pub x10: i32,
    pub failed: bool,
    pub enable: bool,
    /// The save's comment as last made (`lb_8001C658`).
    pub comment: [u8; 0x40],
    /// `LbMcGame`'s `MemCardIconData`: the three banners by unlock state and the icon.
    pub icon_data: Option<DataRef>,
    pub jobj_translate_idx: i32,
    /// `NtMemAc`'s `ScNtcCommon_scene_data`, the indicator's camera and model.
    pub scene_data: Option<DataRef>,
    /// `lbCardGame_Init` has pointed the save's manifest at the card data (`manifest`'s
    /// `data`), which only it writes.
    pub manifest_set: bool,
}

impl Default for CardGame {
    fn default() -> Self {
        CardGame {
            probe_status: false,
            unk_status: false,
            card_status: STATUS_OK,
            dirty: false,
            x10: 0,
            failed: false,
            enable: false,
            comment: [0; 0x40],
            icon_data: None,
            jobj_translate_idx: 0,
            scene_data: None,
            manifest_set: false,
        }
    }
}

/// `jobj_translate`: where the indicator stands.
const INDICATOR_TRANSLATE: [(f32, f32); 1] = [(21.5, 16.5)];

impl CardGame {
    /// `lbCardGame_Reset`, each scene's preparation: the archives forgotten, the save off
    /// until they load again, nothing to write.
    #[doc(alias = "lbCardGame_Reset")]
    pub fn reset(&mut self) {
        self.icon_data = None;
        self.scene_data = None;
        self.enable = false;
        self.dirty = false;
        self.x10 = 0;
        self.failed = false;
    }

    /// `lbCardGame_Init` at boot: the probe as it stands, the card as the game knows it,
    /// and the manifest pointed at the card data.
    #[doc(alias = "lbCardGame_Init")]
    pub fn init(&mut self, card_in: bool) {
        self.probe_status = card_in;
        self.unk_status = false;
        self.card_status = STATUS_OK;
        self.reset();
        self.manifest_set = true;
    }

    /// `lbCardGame_SetCardStatus`.
    // Ports `lbCardGame_SetCardStatus`; aliased once a check reaches it.
    pub fn set_card_status(&mut self, status: i32) {
        self.card_status = status;
    }
}

impl Running<'_> {
    /// `lbCardGame_DecideGameMode`, every mode's override once the boot is done: the memory
    /// card mode when the card needs the player, else none (`GM_COUNT`).
    #[doc(alias = "lbCardGame_DecideGameMode")]
    pub fn card_decide_game_mode(&mut self) -> u8 {
        let status = self.update_card_status();
        if status != STATUS_OK && status != STATUS_NO_CARD {
            super::super::game::gm::machine::GM_MEMCARD
        } else {
            super::super::game::gm::machine::GM_COUNT
        }
    }

    /// `updateCardStatus`: the card's status moved on by a swap the probe saw
    /// (`unk_status`), and a player who went on without saving followed as the card comes
    /// and goes.
    #[doc(alias = "updateCardStatus")]
    pub fn update_card_status(&mut self) -> i32 {
        self.card_alarm_probes();
        let swapped = std::mem::take(&mut self.game.card_game.unk_status);
        let status = self.game.card_game.card_status;
        let next = match status {
            STATUS_OK if swapped => STATUS_SWAPPED,
            STATUS_NO_SAVE if self.card_probe() != result::READY => STATUS_NO_CARD,
            STATUS_NO_CARD if swapped && self.card_probe() == result::READY => STATUS_NO_SAVE,
            s => s,
        };
        self.game.card_game.card_status = next;
        next
    }

    /// `lb_8001CBBC`: the save loaded, unless the card isn't as the game knows it; a load
    /// that fails (but for damage the game reads past) marks the status. Its result.
    #[doc(alias = "lb_8001CBBC")]
    pub fn card_game_load(&mut self) -> i32 {
        if self.update_card_status() != STATUS_OK {
            return result::INVALID;
        }
        let r = self.card_load();
        if r != result::READY && r != result::DAMAGED {
            self.game.card_game.card_status = STATUS_FAILED;
        }
        r
    }

    /// `lbCardGame_LoadArchive`: the icons and the indicator's scene (`LbMcGame`,
    /// `NtMemAc`), once a scene, and the save on.
    #[doc(alias = "lbCardGame_LoadArchive")]
    pub fn card_game_load_archive(&mut self, translate_idx: i32) {
        if self.game.card_game.icon_data.is_some() {
            return;
        }
        // `LbMcGame.` takes the disc's language (English), `NtMemAc` the saved one
        // (`lbFileGetFullName`).
        let icons = self.load_archive("LbMcGame.usd");
        let us = self.game.game_data.saved_language() == crate::game::gm::main_lib::LANG_US;
        let scene = self.load_archive(if us { "NtMemAc.usd" } else { "NtMemAc.dat" });
        let sym = |r: &Running, a, name: &str| {
            let offset = r
                .data
                .get(a)
                .public(name)
                .unwrap_or_else(|| panic!("{name} in its archive"));
            DataRef { archive: a, offset }
        };
        let icon_data = sym(self, icons, "MemCardIconData");
        let scene_data = sym(self, scene, "ScNtcCommon_scene_data");
        let c = &mut self.game.card_game;
        c.icon_data = Some(icon_data);
        c.scene_data = Some(scene_data);
        c.jobj_translate_idx = translate_idx;
        c.enable = true;
    }

    /// `lbCardGame_InitScene`, as each scene's loop starts: when the scene loaded the
    /// indicator's archive, its camera (drawing only while the save writes) and its turning
    /// model.
    #[doc(alias = "lbCardGame_InitScene")]
    pub fn card_game_init_scene(&mut self) {
        let Some(scene) = self.game.card_game.scene_data else {
            return;
        };
        let data = self.data;
        let g = &mut *self.game;
        let camera = g.world.create(GOBJ_CLASS, GOBJ_PLINK, 0);
        let cameras = data.ptr(scene.at(4)).expect("the scene's cameras");
        let c = cobj::load_desc(&mut g.objs, data, data.ptr(cameras)).expect("a camera");
        g.world.set_obj(camera, OBJ_CAMERA, Obj::CObj(c));
        g.world
            .setup_gx_link_max(camera, CodeRef(INDICATOR_RENDER), 14, false);
        g.world.gobj_mut(camera).gxlink_prios = 1 << 19;
        let model_gobj = g.world.create(GOBJ_CLASS, GOBJ_PLINK, 0);
        let models = data.ptr(scene).expect("the scene's models");
        let model = data.ptr(models).expect("a model");
        let j = jobj::load_joint(&mut g.objs, data, data.ptr(model)).expect("the indicator");
        let (x, y) = INDICATOR_TRANSLATE[g.card_game.jobj_translate_idx as usize];
        jobj::set_srt(&mut g.objs.jobjs, j, |o| {
            o.translate.x = x;
            o.translate.y = y;
        });
        g.world.set_obj(model_gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(
            model_gobj,
            CodeRef(super::super::game::render::JOBJ_CALLBACK),
            19,
            0,
        );
        g.world.setup_proc(model_gobj, CodeRef(INDICATOR_PROC), 0);
        super::super::game::gm::vs::add_model_anim(&mut g.objs, data, Some(j), model, 0);
        jobj::req_anim_all(&mut g.objs, Some(j), 0.0);
        jobj::anim_all(
            &mut g.objs,
            data,
            &mut super::super::game::code::ParticleHooks,
            Some(j),
        );
    }

    /// `gobj1_Proc`, the indicator model's process: its animation.
    #[doc(alias = "gobj1_Proc")]
    pub fn card_indicator_proc(&mut self, gobj: crate::hsd::gobj::GObjId) {
        let Some(Obj::JObj(j)) = self.game.world.gobj(gobj).obj else {
            panic!("the card indicator without its model");
        };
        jobj::anim_all(
            &mut self.game.objs,
            self.data,
            &mut super::super::game::code::ParticleHooks,
            Some(j),
        );
    }

    /// `gobj0_RenderFunc`, the indicator's camera: it draws while the save writes.
    #[doc(alias = "gobj0_RenderFunc")]
    pub fn card_indicator_render(&mut self, gobj: crate::hsd::gobj::GObjId) {
        if self.game.card_game.x10 == 1 {
            self.render_camera(gobj);
        }
    }

    /// `lb_8001CDB4`, after each scene: the writer runs until the save is written, its
    /// steps waited for here (each `lbCardNew_CompleteNextTask`'s wait point).
    #[doc(alias = "lb_8001CDB4")]
    pub fn card_game_flush(&mut self) {
        loop {
            self.card_writer_take();
            self.card_writer_kick_due();
            let c = &self.game.card_game;
            if !c.dirty && c.x10 == 0 {
                return;
            }
            let now = self.game.audio.clock.as_ref().map_or(0, |c| c.now);
            match self.game.audio.card_writer.wait(now) {
                Some(work) => self.game.audio.clock_card_work(self.data, work),
                None => unreachable!("a save writer with nothing to run"),
            }
        }
    }

    /// `lb_8001CC84`'s first state as the next of the game's waits runs it, once the save is
    /// marked to write: the mark taken, the card's status brought up to date, and the save's
    /// list (`lb_8001BE30`, through `dont_inline_helper`) handed to the waits with the card
    /// data, the banner the unlocks pick (`getCurrentIcon`) and the icons. Nothing reads what
    /// it sets before that wait but the pad alarm's card probes, which a card that stays in
    /// leaves alone ([`Running::card_alarm_probes`]).
    #[doc(alias = "lb_8001CC84")]
    #[doc(alias = "lb_8001BE30")]
    #[doc(alias = "getCurrentIcon")]
    fn card_writer_kick(&mut self) {
        let c = &mut self.game.card_game;
        if c.x10 != 0 || !c.dirty {
            return;
        }
        c.dirty = false;
        if c.card_status != 0 {
            return;
        }
        if self.update_card_status() != 0 {
            self.game.card_game.failed = true;
            return;
        }
        let icons = self.game.card_game.icon_data.expect("the card's icons");
        let idx = if self.holds_every_trophy() {
            2
        } else if self.game.gm.unlocks.all_characters() {
            1
        } else {
            0
        };
        let data = self.data;
        let bytes = |i: u32, n: usize| -> Vec<u8> {
            let p = data.ptr(icons.at(4 * i)).expect("an icon image");
            (0..n as u32).map(|k| data.u8(p.at(k))).collect()
        };
        let req = super::save::SaveRequest {
            data: super::save::card_data_bytes(&self.game.game_data.thing),
            banner: bytes(idx, 0x1800),
            icons: bytes(3, 0x600),
        };
        self.game.audio.card_writer.start = Some(req);
        self.game.card_game.x10 = 1;
    }

    /// What the save's writer did at the waits since the game last looked: the comment its
    /// list made, and the list's end (`lbCardNew_CompleteNextTask` no longer busy): the
    /// writer idle, the list's failure noted (`fn_8001CC30`, its `on_finished`, marks the card
    /// failed too), the files' results, the card's status taken by its mount, and the card
    /// unmounted.
    #[doc(alias = "fn_8001CC30")]
    #[doc(alias = "lb_8001C658")]
    pub fn card_writer_take(&mut self) {
        let w = &mut self.game.audio.card_writer;
        if let Some(comment) = w.comment.take() {
            self.game.card_game.comment = comment;
        }
        if std::mem::take(&mut w.errors_due) {
            self.game.card_tasks.file_errors = w.file_errors;
        }
        let Some((r, card_in)) = w.take_done() else {
            return;
        };
        self.game.card_tasks.file_errors = w.file_errors;
        let c = &mut self.game.card_game;
        if card_in {
            c.unk_status = false;
        }
        if r != 0 {
            c.failed = true;
            c.card_status = STATUS_FAILED;
        }
        c.x10 = 0;
        // The save written: its file for the frontend (a replayed tick's events are dropped).
        if r == 0
            && let Some(gci) = super::save::lent_save_gci()
        {
            self.game
                .events
                .push(crate::game::state::Event::SaveWritten(gci));
        }
    }

    /// `lbCardGame_SaveChanges` where one of the game's waits follows at once (a scene's
    /// entry): the save marked to write and the writer started as that wait runs it.
    pub fn card_game_save_changes(&mut self) {
        self.card_game_mark_changes();
        self.card_writer_kick();
    }

    /// The writer started by the game's next wait (`lb_8001CC84`'s first state) if the save
    /// is marked to write and nothing writes: the display pass's (`Session::render`) after a
    /// process saved, or the scene's end (`lb_8001CDB4`).
    pub fn card_writer_kick_due(&mut self) {
        let c = &self.game.card_game;
        if c.dirty && c.x10 == 0 {
            self.card_writer_kick();
        }
    }

    /// `lbCardGame_SaveChanges`: the power-on time added to the save, and the save marked
    /// to write, which the game's next wait takes up ([`Self::card_writer_kick_due`]).
    pub fn card_game_mark_changes(&mut self) {
        assert!(self.game.card_game.enable, "_p(enable)");
        // `gmMainLib_8015FC74`'s `lbTime_GetTimeInSeconds`: the clock's read.
        let now = self.session_seconds();
        self.game
            .audio
            .clock_read(self.data, crate::game::session::TB_READ);
        let add = self.game.main_lib.take_power_time(now);
        // `gm_GetPowerTime`.
        let t = &mut self.game.game_data.thing.save_data.x1A54;
        *t = t.wrapping_add(add);
        self.game.card_game.dirty = true;
    }
}
