//! The match's announcements and KO marks (`melee/if/if_2F6E.c`, `if_2F72.c`): GAME!, TIME!,
//! Ready GO! and the rest as animated models with the announcer's lines timed to their
//! frames, calling the match back as they start and end; and the +1/-1 marks a KO puts by a
//! player's display in a match that isn't for stocks.

use melee_data::archive::DataRef;

use super::super::code::{self, Val};
use super::super::handles::CodeRef;
use super::super::state::{Game, OBJ_JOBJ, Obj, Running};
use super::{Draw, JOBJ_RENDER};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj;
use crate::lb::anim;

/// The announcements' processes: `if_802F73C4` frees its model when the animation ends,
/// `if_802F74D0` holds the last frame.
pub const ONCE: u32 = 0x802F_73C4;
pub const WAIT: u32 = 0x802F_74D0;

/// The KO marks' processes and render callback.
pub const MARK_FADE: u32 = 0x802F_75D4;
pub const MARK_FADE_2: u32 = 0x802F_7670;
pub const MARK_RENDER: u32 = 0x802F_770C;
pub const MARK_SPAWN_2: u32 = 0x802F_7994;

/// The KO marks a display shows (`if_2F72`'s callbacks for `ifStatus_TriggerStockLoss`).
pub const MARK_7AF8: u32 = 0x802F_7AF8;
pub const MARK_7BB4: u32 = 0x802F_7BB4;
pub const MARK_7C30: u32 = 0x802F_7C30;
pub const MARK_7D08: u32 = 0x802F_7D08;

/// `MatchOutcome`s the GAME! announcement picks by.
const OUTCOME_TIMEOUT: u8 = 1;
const OUTCOME_1P_BONUS_STAGE_END: u8 = 6;

impl Running<'_> {
    /// Shows announcement `kind` (with announcer lines `sfx1` at its own frame and `sfx2`
    /// at `frame2`, and the match's callbacks for its start and end), replacing any of the
    /// same kind; kind 8 calls both callbacks at once and plays the lines,
    /// `ifStatus_802F6EA4`.
    #[doc(alias = "ifStatus_802F6EA4")]
    pub fn announce(
        &mut self,
        kind: i32,
        sfx1: i32,
        sfx2: i32,
        frame2: i32,
        on_start: Option<CodeRef>,
        on_end: Option<CodeRef>,
    ) {
        if kind == 8 {
            if let Some(f) = on_start {
                code::run_hud_callback(self, f, -1);
            }
            if let Some(f) = on_end {
                code::run_hud_callback(self, f, -1);
            }
            if sfx1 >= 0 {
                self.sfx_play(sfx1, 0x7F, 0x40);
            }
            // The original tests the first line again before the second.
            if sfx1 >= 0 {
                self.sfx_play(sfx2, 0x7F, 0x40);
            }
            return;
        }
        let data = self.data;
        let k = kind as usize;
        let e = &mut self.game.hud.announcements[k];
        e.x20 = sfx1;
        e.x24 = sfx2;
        e.x11 = frame2 as u8;
        if let Some(old) = e.x0 {
            gobj::free(self, old);
        }
        let g = &mut *self.game;
        let e = &g.hud.announcements[k];
        let model = e.x14.expect("an announcement's model");
        let proc = e.x8;
        let o = g.world.create(0xE, 0xE, 0);
        let j = jobj::load_joint(&mut g.objs, data, data.ptr(model)).expect("a model");
        crate::lb::joint::set_mobj_flags(&mut g.objs, j, 0x0800_0000);
        g.world.set_obj(o, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(o, CodeRef(JOBJ_RENDER), 0xB, 0);
        if let Some(p) = proc {
            g.world.setup_proc(o, p, 0);
        }
        anim::add_model_anim(&mut g.objs, data, Some(j), 0, model);
        jobj::req_anim_all(&mut g.objs, Some(j), 0.0);
        jobj::anim_all(
            &mut g.objs,
            data,
            &mut crate::game::code::ParticleHooks,
            Some(j),
        );
        let e = &mut g.hud.announcements[k];
        e.x12.x0 = false;
        e.x12.x1 = false;
        e.x12.x2 = false;
        e.x0 = Some(o);
        e.x18 = on_start;
        e.x1C = on_end;
    }

    /// The match's end announcement, `ifStatus_802F7034`: the rules' own, TIME!, a bonus
    /// stage's or a single-player stage's ending, or GAME!, with the scene's announcer
    /// lines.
    #[doc(alias = "ifStatus_802F7034")]
    pub fn announce_match_end(&mut self, on_start: Option<CodeRef>) {
        let s = &self.game.vs.state;
        let a = if s.unk_10 != 0 {
            s.unk_10 as i32 - 1
        } else {
            -1
        };
        let b = if s.unk_14 != 0 {
            s.unk_14 as i32 - 1
        } else {
            -1
        };
        let c = if s.unk_18 != 0 { s.unk_18 as i32 } else { 0 };
        let kind = if s.unk_B != 0 {
            i32::from(s.unk_B) - 1
        } else if s.match_result == OUTCOME_TIMEOUT {
            0
        } else if self.game.vs.start.x5_1 {
            if s.match_result == OUTCOME_1P_BONUS_STAGE_END {
                7
            } else {
                6
            }
        } else {
            5
        };
        self.announce(kind, a, b, c, on_start, None);
    }

    /// The announcements' models from `IfAll.dat`'s `ScInfCnt_scene_models`, none shown,
    /// `ifStatus_802F7134`.
    #[doc(alias = "ifStatus_802F7134")]
    pub fn load_announcements(&mut self) {
        let data = self.data;
        let a = self.game.hud.archive.expect("IfAll.dat");
        for e in &mut self.game.hud.announcements {
            e.x0 = None;
            e.x4 = 0;
        }
        let at = data
            .get(a)
            .public("ScInfCnt_scene_models")
            .expect("IfAll.dat's ScInfCnt_scene_models");
        let models = DataRef {
            archive: a,
            offset: at,
        };
        for (i, e) in self.game.hud.announcements.iter_mut().enumerate() {
            e.x14 = data.ptr(models.at(4 * i as u32));
        }
    }

    /// Frees the announcements shown, `ifStatus_802F7220`.
    #[doc(alias = "ifStatus_802F7220")]
    pub fn free_announcements(&mut self) {
        for i in 0..8 {
            if let Some(o) = self.game.hud.announcements[i].x0.take() {
                gobj::free(self, o);
            }
        }
    }

    /// Animates an announcement, playing its lines as their frames come; whether it still
    /// plays, `fn_802F7288`.
    #[doc(alias = "fn_802F7288")]
    pub fn announcement_step(&mut self, gobj: GObjId, k: usize) -> bool {
        let Some(Obj::JObj(j)) = self.game.world.gobj(gobj).obj else {
            panic!("an announcement without its model");
        };
        let frame = anim::current_frame(&self.game.objs, Some(j));
        jobj::anim_all(
            &mut self.game.objs,
            self.data,
            &mut crate::game::code::ParticleHooks,
            Some(j),
        );
        let e = self.game.hud.announcements[k].clone();
        let frame = crate::fpu::reg(frame);
        if !e.x12.x0 && frame >= gekko_fp::frsp(f64::from(e.x10)) {
            if e.x20 >= 0 {
                self.sfx_play(e.x20, 127, 64);
                self.game.hud.announcements[k].x12.x0 = true;
            } else if e.xC >= 0 {
                self.sfx_play(e.xC, 127, 64);
                self.game.hud.announcements[k].x12.x0 = true;
            }
        }
        if !e.x12.x1 && frame >= gekko_fp::frsp(f64::from(e.x11)) && e.x24 >= 0 {
            self.sfx_play(e.x24, 127, 64);
            self.game.hud.announcements[k].x12.x1 = true;
        }
        anim::any_animated(&self.game.objs, Some(j))
    }

    /// The announcement whose model is `gobj`.
    fn announcement_of(&self, gobj: GObjId) -> Option<usize> {
        (0..8).find(|&i| self.game.hud.announcements[i].x0 == Some(gobj))
    }

    /// The start callback the first frame, then the step; at the end, the end callback and
    /// (`free`) the model freed or (else) the end callback forgotten.
    fn announcement_proc(&mut self, gobj: GObjId, free: bool) {
        let Some(k) = self.announcement_of(gobj) else {
            return;
        };
        if !self.game.hud.announcements[k].x12.x2 {
            if let Some(f) = self.game.hud.announcements[k].x18 {
                code::run_hud_callback(self, f, k as i32);
            }
            self.game.hud.announcements[k].x12.x2 = true;
        }
        if !self.announcement_step(gobj, k) {
            if let Some(f) = self.game.hud.announcements[k].x1C {
                code::run_hud_callback(self, f, k as i32);
            }
            if free {
                self.game.hud.announcements[k].x0 = None;
                gobj::free(self, gobj);
            } else {
                self.game.hud.announcements[k].x1C = None;
            }
        }
    }

    /// An announcement that goes when its animation ends, `if_802F73C4`.
    #[doc(alias = "if_802F73C4")]
    pub fn announcement_once(&mut self, gobj: GObjId) {
        self.announcement_proc(gobj, true);
    }

    /// An announcement that stays on its last frame, `if_802F74D0`.
    #[doc(alias = "if_802F74D0")]
    pub fn announcement_wait(&mut self, gobj: GObjId) {
        self.announcement_proc(gobj, false);
    }

    /// A KO mark's model for `slot`, replacing `old`, posed as animation `anim` at the
    /// slot's display, `fn_802F77F8`; none without the models.
    #[doc(alias = "fn_802F77F8")]
    pub fn create_ko_mark(&mut self, old: Option<GObjId>, slot: u8, anim_i: u16) -> Option<GObjId> {
        let data = self.data;
        let models = self.game.hud.ko_marks.scene_models?;
        if let Some(o) = old {
            gobj::free(self, o);
        }
        let g = &mut *self.game;
        let o = g.world.create(14, 15, 0);
        let model = data.ptr(models).expect("the KO mark's model");
        let j = jobj::load_joint(&mut g.objs, data, data.ptr(model)).expect("a model");
        g.world.set_obj(o, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(o, CodeRef(MARK_RENDER), 11, 0);
        super::super::gm::vs::add_model_anim(&mut g.objs, data, Some(j), model, u32::from(anim_i));
        jobj::req_anim_all(&mut g.objs, Some(j), 0.0);
        jobj::anim_all(
            &mut g.objs,
            data,
            &mut crate::game::code::ParticleHooks,
            Some(j),
        );
        let pos = g.player_hud_position(i32::from(slot));
        jobj::set_translate(&mut g.objs.jobjs, j, pos);
        Some(o)
    }

    /// A slot's first KO mark with animation `anim` and process `proc`.
    fn spawn_ko_mark(&mut self, slot: i32, anim_i: u16, proc: u32) {
        let idx = usize::from(slot as u8);
        assert!(
            idx < 6,
            "a KO mark for slot {idx}, past the table, isn't ported"
        );
        let old = self.game.hud.ko_marks.slots[idx].gobj;
        let g = self.create_ko_mark(old, slot as u8, anim_i);
        self.game.hud.ko_marks.slots[idx].gobj = g;
        if let Some(g) = g {
            self.game.world.setup_proc(g, CodeRef(proc), 0x11);
        }
    }

    /// `if_802F7AF8`: marks for the slot (-1) and for its last attacker (+1... anim 2).
    #[doc(alias = "if_802F7AF8")]
    pub fn ko_mark_pair(&mut self, slot: i32) {
        let other = self.game.player_last_attacker(slot);
        self.spawn_ko_mark(slot, 1, MARK_FADE);
        self.spawn_ko_mark(other, 2, MARK_FADE);
    }

    /// `if_802F7BB4`.
    #[doc(alias = "if_802F7BB4")]
    pub fn ko_mark_lost(&mut self, slot: i32) {
        self.spawn_ko_mark(slot, 1, MARK_FADE);
    }

    /// `if_802F7C30`: by the self-destruct penalty, a mark of animation 0 (-2) or 1 (-1).
    #[doc(alias = "if_802F7C30")]
    pub fn ko_mark_penalty(&mut self, slot: i32) {
        match self.game.vs_sd_penalty() {
            -2 => self.spawn_ko_mark(slot, 0, MARK_FADE),
            -1 => self.spawn_ko_mark(slot, 1, MARK_FADE),
            _ => {}
        }
    }

    /// `if_802F7D08`: likewise, the penalties' marks spawning a second.
    #[doc(alias = "if_802F7D08")]
    pub fn ko_mark_fall(&mut self, slot: i32) {
        match self.game.vs_sd_penalty() {
            -2 => self.spawn_ko_mark(slot, 0, MARK_SPAWN_2),
            -1 => self.spawn_ko_mark(slot, 1, MARK_SPAWN_2),
            _ => self.spawn_ko_mark(slot, 1, MARK_FADE),
        }
    }

    /// Runs a KO mark callback by its address (the `if_2F72` function a display passes).
    pub fn ko_mark(&mut self, f: u32, slot: i32) {
        match f {
            MARK_7AF8 => self.ko_mark_pair(slot),
            MARK_7BB4 => self.ko_mark_lost(slot),
            MARK_7C30 => self.ko_mark_penalty(slot),
            MARK_7D08 => self.ko_mark_fall(slot),
            other => code::unported_call(&format!("KO mark {other:#010x}")),
        }
    }

    /// A KO mark that goes when its animation ends, `fn_802F75D4` (first marks) and
    /// `fn_802F7670` (second).
    fn ko_mark_fade(&mut self, gobj: GObjId, second: bool) {
        let Some(Obj::JObj(j)) = self.game.world.gobj(gobj).obj else {
            panic!("a KO mark without its model");
        };
        if !anim::any_animated(&self.game.objs, Some(j)) {
            let slots = &mut self.game.hud.ko_marks.slots;
            if let Some(s) = slots.iter_mut().find(|s| {
                if second {
                    s.gobj2 == Some(gobj)
                } else {
                    s.gobj == Some(gobj)
                }
            }) {
                if second {
                    s.gobj2 = None;
                } else {
                    s.gobj = None;
                }
            }
            gobj::free(self, gobj);
        } else {
            jobj::anim_all(
                &mut self.game.objs,
                self.data,
                &mut crate::game::code::ParticleHooks,
                Some(j),
            );
        }
    }

    #[doc(alias = "fn_802F75D4")]
    pub fn ko_mark_proc(&mut self, gobj: GObjId) {
        self.ko_mark_fade(gobj, false);
    }

    #[doc(alias = "fn_802F7670")]
    pub fn ko_mark_proc_2(&mut self, gobj: GObjId) {
        self.ko_mark_fade(gobj, true);
    }

    /// A first KO mark that spawns the second past frame 12, `fn_802F7994`.
    #[doc(alias = "fn_802F7994")]
    pub fn ko_mark_proc_spawning(&mut self, gobj: GObjId) {
        let Some(Obj::JObj(j)) = self.game.world.gobj(gobj).obj else {
            panic!("a KO mark without its model");
        };
        let frame = anim::current_frame(&self.game.objs, Some(j));
        let Some(slot) = self.game.ko_mark_slot(gobj) else {
            return;
        };
        if crate::fpu::reg(frame) > 12.0 && self.game.hud.ko_marks.slots[slot].gobj2.is_none() {
            let g = self.create_ko_mark(None, slot as u8, 1);
            self.game.hud.ko_marks.slots[slot].gobj2 = g;
            if let Some(g) = g {
                self.game.world.setup_proc(g, CodeRef(MARK_FADE_2), 0x11);
            }
        }
        if !anim::any_animated(&self.game.objs, Some(j)) {
            self.game.hud.ko_marks.slots[slot].gobj = None;
            gobj::free(self, gobj);
        } else {
            jobj::anim_all(
                &mut self.game.objs,
                self.data,
                &mut crate::game::code::ParticleHooks,
                Some(j),
            );
        }
    }

    /// The KO marks' models from `IfAll.dat`'s `ScInfStc_scene_models`, `if_802F7E24`.
    #[doc(alias = "if_802F7E24")]
    pub fn load_ko_marks(&mut self) {
        let a = self.game.hud.archive.expect("IfAll.dat");
        self.game.hud.ko_marks = Default::default();
        let at = self.data.get(a).public("ScInfStc_scene_models");
        self.game.hud.ko_marks.scene_models = at.map(|offset| DataRef { archive: a, offset });
    }

    /// Frees the KO marks, `if_802F7E7C`.
    #[doc(alias = "if_802F7E7C")]
    pub fn free_ko_marks(&mut self) {
        for i in 0..6 {
            let s = self.game.hud.ko_marks.slots[i].clone();
            if let Some(o) = s.gobj {
                gobj::free(self, o);
            }
            if let Some(o) = s.gobj2 {
                gobj::free(self, o);
            }
        }
        self.game.hud.ko_marks = Default::default();
    }
}

impl Game {
    /// The slot whose first KO mark is `gobj` (the inline `GetSlot`).
    fn ko_mark_slot(&self, gobj: GObjId) -> Option<usize> {
        (0..6).find(|&i| self.hud.ko_marks.slots[i].gobj == Some(gobj))
    }

    /// The KO marks' render callback, `fn_802F770C`: drawn unless the slot's display hides
    /// its digits. A second mark, on no slot's first, reads the record before the displays',
    /// which isn't ported.
    #[doc(alias = "fn_802F770C")]
    pub fn ko_mark_render(&self, gobj: GObjId, pass: i32) -> Option<Draw> {
        let slot = self
            .ko_mark_slot(gobj)
            .unwrap_or_else(|| code::unported_call("fn_802F770C on a second KO mark"));
        (!self.hud.status.players[slot].flags.hide_all_digits).then_some(Draw::JObjs(gobj, pass))
    }
}

/// `if_802F7AF8`'s other slot as [`code::call`] passes it.
pub fn slot_val(s: i32) -> Val {
    Val::Int(s)
}
