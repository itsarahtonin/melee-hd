//! The challenger approach (`melee/gm/gmapproach.c`, `gm_1BFA.c`, and VS mode's own exits in
//! `gmmovieend.c`): after a VS match whose records earn a locked character (the decisions in
//! `gmVsMelee_ExitResults`), VS mode's states run the "A new foe has appeared!" screen, a
//! one-stock match against the challenger as a computer player on its own stage, and, when
//! the human wins, the character unlocked; any notice waiting goes through the prize screen
//! (`ifprize`) before the character select.

use melee_data::archive::{ArchiveId, DataRef};

use super::super::handles::CodeRef;
use super::super::render::{CAMERA_CALLBACK, JOBJ_CALLBACK, LOBJ_CALLBACK};
use super::super::state::{OBJ_CAMERA, OBJ_JOBJ, OBJ_LIGHT, Obj, Running};
use super::results_scene::canceled;
use super::setup_players::{setup_cpu, setup_human};
use super::vs_mode::{NO_NAMETAG, all_player_defaults, load_rumble_enabled_with, rules_defaults};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::{aobj, tobj};

/// `gm_Scene_Approach_OnEnter`, `gm_Scene_Approach_OnFrame`, `gm_Scene_Approach_OnExit`'s
/// scene and `fn_801AD920`, the screen's model's process.
pub const ON_ENTER: CodeRef = CodeRef(0x801A_DCE4);
pub const ON_FRAME: CodeRef = CodeRef(0x801A_DC88);
pub const PROC: u32 = 0x801A_D920;

/// The screen's song (`lbAudioAx_80023F28`'s 0x48).
pub const SONG: i32 = 0x48;

/// `MatchKind_Stock`, `CpuKind_4`.
const MATCH_STOCK: u8 = 1;
const CPU_KIND: u8 = 4;

/// `HSD_PAD_A | HSD_PAD_START`.
const PAD_A_START: u32 = 0x1100;

/// The ticks before A or Start ends the screen (`gm_80480D98.xC` reaches 0xB4).
const WAIT: u16 = 0xB4;

/// The challengers' own characters and the frame of the screen's name texture each shows
/// (`fn_801AD920`'s switch; `gm_Scene_Approach_OnEnter` takes any other as Ganondorf, 25).
const NAME_FRAMES: [(i32, i32); 11] = [
    (3, 1),
    (7, 2),
    (9, 3),
    (10, 4),
    (15, 5),
    (20, 6),
    (21, 7),
    (22, 8),
    (23, 9),
    (24, 10),
    (25, 11),
];

/// `lbl_803B78C8`'s stage kinds, by unlock index: each challenger's stage (the single-player
/// variants of `StKind` past `St_Kind_Last`).
const CHALLENGER_STAGES: [u16; 11] = [
    0x116, 0x114, 0x115, 0x113, 0x112, 0x11A, 0x119, 0x117, 0x11C, 0x11B, 0x118,
];

/// `St_Kind_Unk328`, no stage.
const NO_STAGE: u16 = 0x148;

/// `gm_80480D98`, the screen's statics, and `gmVsMelee_ApproachData`, its enter and exit
/// data.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Approach {
    /// `x0`: `NtAppro`'s archive, and its two blocks in the scene's heap (its bytes, its
    /// `HSD_Archive`).
    pub archive: Option<ArchiveId>,
    pub blocks: (u32, u32),
    /// `x4`: the challenger's character.
    pub ckind: i32,
    /// `x8`, `xA`: the model's two animations' frames.
    pub frame: u16,
    pub frame2: u16,
    /// `xC`: the screen's ticks.
    pub ticks: u16,
    /// `xE`: the port whose A or Start ends the screen.
    pub port: u8,
    /// `gmVsMelee_ApproachData`: the challenger's character and the human's slot.
    pub data: [u8; 2],
}

/// `gm_GetChallengerStKind(gm_CKindToSelKind(ckind))`: the challenger's stage (`0x148` for
/// a character with none).
#[doc(alias = "gm_GetChallengerStKind")]
pub fn challenger_stage(ckind: u8) -> u16 {
    super::super::gm::unlock_index(ckind).map_or(NO_STAGE, |i| CHALLENGER_STAGES[i as usize])
}

impl Running<'_> {
    /// `gm_ModeState_Approach_OnEnter`: the screen's data, the challenger's character and the
    /// human's slot; the card's archive and the save marked (`lbCardGame_SetupArchive`).
    #[doc(alias = "gm_ModeState_Approach_OnEnter")]
    #[doc(alias = "gm_GetChallengerData")]
    pub fn approach_state_on_enter(&mut self) {
        let c = self.game.gm.challenger;
        self.game.approach.data = [c.cpu_ckind, c.human_slot];
        // `lbCardGame_SetupArchive` before a scene's entry, whose waits start the save.
        self.card_alloc_work_area();
        self.card_game_load_archive(0);
        self.card_game_save_changes();
    }

    /// `gm_DecideChallengerCpuLevel`: the challenger's level, its base less its step for
    /// each match lost to it since power-on (`gmMainLib_8015DB6C`), within 0 and 9.
    #[doc(alias = "gm_DecideChallengerCpuLevel")]
    #[doc(alias = "gmMainLib_8015DB6C")]
    fn challenger_level(&self, ckind: u8) -> u8 {
        let idx = super::super::gm::unlock_index(ckind).expect("a challenger") as u8;
        let (_, base, step, _) = super::results_exit::CHALLENGERS
            .iter()
            .copied()
            .find(|c| c.0 == idx)
            .expect("the challenger's row");
        let lost = self.game.game_data.x39[usize::from(idx)];
        let level = i32::from(base) - i32::from(step) * i32::from(lost);
        level.clamp(0, 9) as u8
    }

    /// `gm_ModeState_ApproachVs_OnEnter`: the match's start data: the rules' defaults, a
    /// one-stock match with no timer on the challenger's stage, the human on its slot and
    /// name tag against the challenger as a computer player of its level, and the humans'
    /// rumble.
    #[doc(alias = "gm_ModeState_ApproachVs_OnEnter")]
    pub fn approach_vs_on_enter(&mut self) {
        let c = self.game.gm.challenger;
        let level = self.challenger_level(c.cpu_ckind);
        let (_, prefs) = super::attract::saved_rules(&self.game.game_data);
        let tags = self.nametag_rumbles();
        let start = &mut self.game.vs_melee.start;
        start.rules = rules_defaults();
        all_player_defaults(&mut start.players);
        start.rules.timer_enabled = false;
        start.rules.match_kind = MATCH_STOCK;
        start.rules.x0_3 = 2;
        start.rules.stkind = challenger_stage(c.cpu_ckind);
        setup_human(
            &mut start.players[0],
            c.human_ckind,
            c.human_color,
            1,
            c.human_slot,
        );
        setup_cpu(&mut start.players[1], c.cpu_ckind, 0, 1, 1);
        start.players[0].nametag = c.human_nametag;
        start.players[1].nametag = NO_NAMETAG;
        start.players[1].cpu_kind = CPU_KIND;
        start.players[1].cpu_level = level;
        load_rumble_enabled_with(start, &prefs, &tags);
    }

    /// `gm_ModeState_ApproachVs_OnExit` (VS mode's and the Special Melee modes'): the match's
    /// seconds to the single-player time and its coins; the challenger unlocked when the human
    /// won with a stock left, else one more loss to it; the trophies and special messages the
    /// save has earned; and the character select next unless a notice waits (then the prize
    /// state, the next).
    #[doc(alias = "gm_ModeState_ApproachVs_OnExit")]
    #[doc(alias = "gm_80162968")]
    #[doc(alias = "gmMainLib_GetSingleplayerTime")]
    #[doc(alias = "gmMainLib_8015DB2C")]
    pub fn approach_vs_on_exit(&mut self) {
        self.approach_vs_exit(None);
    }

    /// The approach match's exit, VS mode's (`then` none: the character select or the prize
    /// state) or the challenger approach mode's (`onExitVs`: the mode the challenger came
    /// from, unless a notice waits).
    fn approach_vs_exit(&mut self, then: Option<u8>) {
        let end = self.game.vs_melee.exit.match_end.clone();
        let c = self.game.gm.challenger;
        let s = &mut self.game.game_data.thing.save_data;
        s.x1A3C = (s.x1A3C as u32).wrapping_add(end.frame_count / 60) as i32;
        let p = &end.player_standings[0];
        self.results_add_coins(i32::from(p.xE));
        if !canceled(end.outcome) && p.stocks != 0 {
            self.unlock_character(c.cpu_ckind);
        } else {
            let idx = super::super::gm::unlock_index(c.cpu_ckind).expect("a challenger");
            let lost = &mut self.game.game_data.x39[idx as usize];
            *lost = super::results_exit::add_u8_saturating(u32::from(*lost), 1) as u8;
        }
        self.trophy_unlocks();
        self.special_unlocks(0x100);
        if !self.notice_waits() {
            match then {
                None => self.set_next_game_mode_state_id(0),
                Some(mode) => {
                    self.set_pending_game_mode(mode);
                    self.set_new_game_mode_pending();
                }
            }
        }
    }

    /// `onExitVs` (the challenger approach mode's): as VS mode's, but the mode the challenger
    /// came from next.
    // Ports `onExitVs@801BFBA8`; aliased once a check reaches it (the single-player modes'
    // challengers, milestone 6).
    pub fn challenger_approach_vs_on_exit(&mut self) {
        let mode = self.game.gm.challenger.curr_mode;
        self.approach_vs_exit(Some(mode));
    }

    /// `gm_ModeState_Prize_OnExit` (VS mode's): the character select next.
    #[doc(alias = "gm_ModeState_Prize_OnExit")]
    pub fn vs_prize_state_on_exit(&mut self) {
        self.set_next_game_mode_state_id(0);
    }

    /// `gm_UnlockCKind`: an unlockable character unlocked in the save, its special message
    /// announced.
    #[doc(alias = "gm_UnlockCKind")]
    pub fn unlock_character(&mut self, ckind: u8) {
        let Some(idx) = super::super::gm::unlock_index(ckind) else {
            return;
        };
        let msg = super::erase::character_message(idx as i32);
        self.note_message(u32::from(msg));
        let s = &mut self.game.game_data.thing.save_data;
        s.unlocked_characters |= 1 << idx;
    }

    /// `gm_801ADB04`: `NtAppro`'s scene in the saved language: its camera, its light, and its
    /// model with its animations and process.
    #[doc(alias = "gm_801ADB04")]
    fn approach_setup(&mut self) {
        let us = self.game.game_data.saved_language() == super::main_lib::LANG_US;
        let name = if us { "NtAppro.usd" } else { "NtAppro.dat" };
        let (a, file, archive) = self.load_archive_blocks(name);
        let at = self
            .data
            .get(a)
            .public("ScNtcApproach_scene_data")
            .expect("NtAppro's scene");
        let scene = DataRef {
            archive: a,
            offset: at,
        };
        let ap = &mut self.game.approach;
        ap.archive = Some(a);
        ap.blocks = (file, archive);
        let data = self.data;
        let g = &mut *self.game;
        let cam = g.world.create(0x13, 0x14, 0);
        let cameras = data.ptr(scene.at(4)).expect("the scene's cameras");
        let c =
            crate::hsd::cobj::load_desc(&mut g.objs, data, data.ptr(cameras)).expect("a camera");
        g.world.set_obj(cam, OBJ_CAMERA, Obj::CObj(c));
        g.world
            .setup_gx_link_max(cam, CodeRef(CAMERA_CALLBACK), 0, false);
        g.world.gobj_mut(cam).gxlink_prios = 0x4801;
        let light = g.world.create(0xB, 0xF, 0);
        let lights = data.ptr(scene.at(8)).expect("the scene's lights");
        let l = crate::game::gr::setup::load_light_list(g, data, lights);
        g.world
            .set_obj(light, OBJ_LIGHT, Obj::LObj(l.expect("a light")));
        g.world.setup_gx_link(light, CodeRef(LOBJ_CALLBACK), 0, 0);
        let gobj = g.world.create(0xE, 0xF, 0);
        let models = data.ptr(scene).expect("the scene's models");
        let m = data.ptr(models).expect("a model");
        let j = jobj::load_joint(&mut g.objs, data, data.ptr(m)).expect("the screen's model");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 0xB, 0);
        super::vs::add_model_anim(&mut g.objs, data, Some(j), m, 0);
        jobj::req_anim_all(&mut g.objs, Some(j), 0.0);
        jobj::anim_all(
            &mut g.objs,
            data,
            &mut super::super::code::ParticleHooks,
            Some(j),
        );
        g.world.setup_proc(gobj, CodeRef(PROC), 1);
    }

    /// `fn_801AD920`, the model's process: its first child's animation loops over 301
    /// frames, the next two play once to frame 300, the third's texture shows the
    /// challenger's name (held), and the tree animates.
    #[doc(alias = "fn_801AD920")]
    pub fn approach_proc(&mut self, gobj: GObjId) {
        let j = match self.game.world.gobj(gobj).obj {
            Some(Obj::JObj(j)) => j,
            other => panic!("the approach's model GObj holds {other:?}"),
        };
        let objs = &mut self.game.objs;
        let first = objs.jobjs[j].child;
        let second = first.and_then(|c| objs.jobjs[c].next);
        let third = second.and_then(|c| objs.jobjs[c].next);
        let ap = &mut self.game.approach;
        if ap.frame <= 0x12C {
            ap.frame += 1;
        } else {
            ap.frame = 0;
        }
        if ap.frame2 < 0x12C {
            ap.frame2 += 1;
        }
        let (f1, f2) = (f32::from(ap.frame), f32::from(ap.frame2));
        let name = NAME_FRAMES
            .iter()
            .find(|&&(k, _)| k == ap.ckind)
            .map_or(0, |&(_, f)| f);
        jobj::req_anim_all(objs, first, f1);
        jobj::req_anim_all(objs, second, f2);
        jobj::req_anim_all(objs, third, f2);
        let t = name_tobj(objs, third.expect("the name's joint"));
        tobj::req_anim_all(objs, t, name as f32);
        let a = t.and_then(|t| objs.tobjs[t].aobj);
        aobj::set_rate(&mut objs.aobjs, a, 0.0);
        jobj::anim_all(
            objs,
            self.data,
            &mut super::super::code::ParticleHooks,
            Some(j),
        );
    }

    /// `gm_Scene_Approach_OnFrame`: from the screen's 181st tick, A or Start on the human's
    /// port ends it.
    #[doc(alias = "gm_Scene_Approach_OnFrame")]
    pub fn approach_on_frame(&mut self) {
        let ap = &mut self.game.approach;
        let t = ap.ticks;
        ap.ticks = ap.ticks.wrapping_add(1);
        if t >= WAIT {
            let port = usize::from(self.game.approach.port);
            if self.game.pads.copy[port].trigger & PAD_A_START != 0 {
                self.game.end_scene();
            }
        }
    }

    /// `gm_Scene_Approach_OnEnter`: the scene, the challenger (any not a challenger's own
    /// taken as Ganondorf) and the human's port from the enter data, and the screen's song.
    #[doc(alias = "gm_Scene_Approach_OnEnter")]
    pub fn approach_on_enter(&mut self) {
        self.approach_setup();
        let [ckind, port] = self.game.approach.data;
        let ckind = i32::from(ckind as i8);
        let ap = &mut self.game.approach;
        ap.ckind = if NAME_FRAMES.iter().any(|&(k, _)| k == ckind) {
            ckind
        } else {
            25
        };
        ap.port = port;
        ap.frame = 0;
        ap.frame2 = 0;
        ap.ticks = 0;
        self.lb_play_music(SONG);
    }

    /// `gm_Scene_Approach_OnExit`: the archive let go, the music stopped.
    #[doc(alias = "gm_Scene_Approach_OnExit")]
    pub fn approach_on_exit(&mut self) {
        let (file, archive) = self.game.approach.blocks;
        self.free_archive_blocks(file, archive);
        self.lb_audio_music_stop();
    }
}

/// A joint's first display object's material's first texture (`jobj->u.dobj->mobj->tobj`).
fn name_tobj(objs: &crate::hsd::Objects, j: JObjId) -> Option<tobj::TObjId> {
    let d = objs.jobjs[j].dobj().expect("jobj->u.dobj");
    let m = objs.dobjs[d].mobj.expect("dobj->mobj");
    objs.mobjs[m].tobj
}
