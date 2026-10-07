//! The match timer (`melee/if/iftime.c`): minutes, seconds and hundredths as texture-animated
//! digit joints, an hour column in long matches, and the 5-4-3-2-1 countdown model that
//! replaces the timer for a timed match's last five seconds.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::handles::CodeRef;
use super::super::state::{Game, OBJ_JOBJ, Obj, Running};
use super::{HIDDEN, JOBJ_RENDER, ModelDesc};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::{Objects, aobj};
use crate::lb::anim;

/// The timer's and the countdown's processes.
pub const PROC_TIMER: u32 = 0x802F_428C;
pub const PROC_COUNTDOWN: u32 = 0x802F_4194;

/// The timer's digit joints (`IFTIME_*`).
const HOUR_SEP: usize = 0;
const HOUR_TEN: usize = 2;
const HOUR_ONE: usize = 3;
const MINUTE_TEN: usize = 4;
const MINUTE_ONE: usize = 5;
const CENTI_TEN: usize = 6;
const CENTI_ONE: usize = 7;
const SECOND_TEN: usize = 8;
const SECOND_ONE: usize = 9;

/// Holds a digit joint's texture on frame `frame` (the inline `ifTime_SetDigit`).
fn set_digit(objs: &mut Objects, j: Option<JObjId>, frame: u32) {
    let j = j.expect("a timer digit");
    let d = objs.jobjs[j].dobj().expect("jobj->u.dobj");
    let m = objs.dobjs[d].mobj.expect("dobj->mobj");
    let t = objs.mobjs[m].tobj.expect("mobj->tobj");
    let a = objs.tobjs[t].aobj;
    aobj::req_anim(
        &mut objs.aobjs,
        &mut objs.fobjs,
        a,
        crate::fpu::mem(fp::frsp(f64::from(frame & 0xFF))),
    );
    aobj::set_rate(&mut objs.aobjs, a, 0.0);
}

impl Game {
    /// The timer's hundredths, counting down unless the timer counts up, `gm_8016AF0C`.
    #[doc(alias = "gm_8016AF0C")]
    pub fn vs_timer_centiseconds(&self) -> u16 {
        let f = fp::frsp(f64::from(self.vs.state.unk_2C));
        let t = fp::fctiwz(fp::fdivs(fp::fmuls(99.0, f), 59.0)) as u16;
        if self.vs_timer_counts_up() {
            t
        } else {
            99u16.wrapping_sub(t)
        }
    }

    /// The self-destruct penalty, `gm_8016AEC8`.
    #[doc(alias = "gm_8016AEC8")]
    pub fn vs_sd_penalty(&self) -> i32 {
        i32::from(self.vs.start.sd_penalty)
    }

    /// Sets the timer's digits for a time, `ifTime_SetTime` (`root` is the timer's model,
    /// whose own visibility the hour column takes).
    #[doc(alias = "ifTime_SetTime")]
    pub fn set_timer_digits(&mut self, root: Option<JObjId>, seconds: i32, centis: i32) {
        let d = self.hud.time.digits;
        let objs = &mut self.objs;
        let hours = if self.vs.start.timer_shows_hours {
            set_digit(objs, d[MINUTE_TEN], (seconds / 60 % 60 % 100 / 10) as u32);
            seconds / 60 / 60
        } else {
            set_digit(objs, d[MINUTE_TEN], (seconds / 60 % 100 / 10) as u32);
            0
        };
        set_digit(objs, d[MINUTE_ONE], (seconds / 60 % 60 % 10) as u32);
        set_digit(objs, d[SECOND_TEN], (seconds % 60 % 100 / 10) as u32);
        set_digit(objs, d[SECOND_ONE], (seconds % 60 % 10) as u32);
        let jobjs = &mut objs.jobjs;
        if hours != 0 {
            let flags = !jobj::get_flags(jobjs, root) & HIDDEN;
            jobj::clear_flags(jobjs, d[HOUR_SEP], flags);
            jobj::clear_flags(jobjs, d[HOUR_ONE], flags);
            set_digit(objs, d[HOUR_ONE], (hours % 10) as u32);
            let ten = hours % 100 / 10;
            let jobjs = &mut objs.jobjs;
            if ten > 0 {
                jobj::clear_flags(jobjs, d[HOUR_TEN], flags);
                set_digit(objs, d[HOUR_TEN], ten as u32);
            } else {
                jobj::set_flags(jobjs, d[HOUR_TEN], flags);
            }
        } else {
            jobj::set_flags(jobjs, d[HOUR_ONE], HIDDEN);
            jobj::set_flags(jobjs, d[HOUR_TEN], HIDDEN);
            jobj::set_flags(jobjs, d[HOUR_SEP], HIDDEN);
        }
        set_digit(objs, d[CENTI_TEN], (centis % 100 / 10) as u32);
        set_digit(objs, d[CENTI_ONE], (centis % 10) as u32);
    }

    /// The seconds the countdown shows, `ifTime_GetCountdownSeconds`.
    #[doc(alias = "ifTime_GetCountdownSeconds")]
    pub fn countdown_seconds(&self) -> i32 {
        let centis = self.vs_timer_centiseconds();
        let seconds = self.vs_timer_seconds() as i32;
        let s = if centis == 0 {
            5 - seconds
        } else {
            4 - seconds
        };
        s.max(0)
    }

    /// `ifTime_Reset`.
    #[doc(alias = "ifTime_Reset")]
    pub fn reset_timers(&mut self) {
        self.hud.time = Default::default();
    }

    /// Whether the timer or the countdown is hidden, `ifTime_IsTimerHidden`.
    #[doc(alias = "ifTime_IsTimerHidden")]
    pub fn is_timer_hidden(&self) -> bool {
        let t = &self.hud.time;
        [t.match_timer, t.countdown_timer]
            .into_iter()
            .flatten()
            .any(|g| jobj::get_flags(&self.objs.jobjs, self.timer_root(g)) & HIDDEN != 0)
    }

    /// A timer GObj's model; the countdown's GObj has none until its last five seconds.
    fn timer_root(&self, g: GObjId) -> Option<JObjId> {
        match self.world.gobj(g).obj {
            Some(Obj::JObj(j)) => Some(j),
            _ => None,
        }
    }
}

impl Running<'_> {
    /// Hides the timer and the countdown, `ifTime_HideTimers`.
    #[doc(alias = "ifTime_HideTimers")]
    pub fn hide_timers(&mut self) {
        let t = self.game.hud.time.clone();
        for g in [t.match_timer, t.countdown_timer].into_iter().flatten() {
            let j = self.game.timer_root(g);
            jobj::set_flags_all(&mut self.game.objs.jobjs, j, HIDDEN);
        }
    }

    /// Shows them again, the timer's digits brought up to date, `ifTime_ShowTimers`.
    #[doc(alias = "ifTime_ShowTimers")]
    pub fn show_timers(&mut self) {
        let t = self.game.hud.time.clone();
        if let Some(g) = t.match_timer {
            let j = self.game.timer_root(g);
            jobj::clear_flags_all(&mut self.game.objs.jobjs, j, HIDDEN);
            let s = self.game.vs_timer_seconds() as i32;
            let c = i32::from(self.game.vs_timer_centiseconds());
            self.game.set_timer_digits(j, s, c);
        }
        if let Some(g) = t.countdown_timer {
            let j = self.game.timer_root(g);
            jobj::clear_flags_all(&mut self.game.objs.jobjs, j, HIDDEN);
        }
    }

    /// The countdown's process, `ifTime_UpdateCountdown`: a new second restarts the model
    /// on its animation.
    #[doc(alias = "ifTime_UpdateCountdown")]
    pub fn countdown_proc(&mut self, gobj: GObjId) {
        let data = self.data;
        let j = self.game.timer_root(gobj).expect("the countdown's model");
        let f = self.game.countdown_seconds();
        if f != i32::from(self.game.hud.time.countdown_seconds) {
            self.game.hud.time.countdown_seconds = f as u8;
            let model = self.countdown_model();
            let objs = &mut self.game.objs;
            jobj::remove_anim_all(objs, Some(j));
            anim::add_model_anim(objs, data, Some(j), u32::from(f as u8), model);
            jobj::req_anim_all(objs, Some(j), 0.0);
            jobj::anim_all(objs, data, &mut crate::game::code::ParticleHooks, Some(j));
        }
        jobj::anim_all(
            &mut self.game.objs,
            data,
            &mut crate::game::code::ParticleHooks,
            Some(j),
        );
    }

    /// The countdown's model (`countdown_timer_models[0]`).
    fn countdown_model(&self) -> DataRef {
        let list = self.game.hud.time.countdown_timer_models.expect("tdsce");
        self.data.ptr(list).expect("the countdown's model")
    }

    /// Frees the countdown, `ifTime_FreeCountdown`.
    #[doc(alias = "ifTime_FreeCountdown")]
    pub fn free_countdown(&mut self) {
        if let Some(g) = self.game.hud.time.countdown_timer.take() {
            gobj::free(self, g);
        }
    }

    /// The timer's process, `ifTime_UpdateTimers`: its digits, and at five seconds left on a
    /// counting-down timer, the countdown's model in its place.
    #[doc(alias = "ifTime_UpdateTimers")]
    pub fn timer_proc(&mut self, gobj: GObjId) {
        let data = self.data;
        let j = self.game.timer_root(gobj).expect("the timer's model");
        let seconds = self.game.vs_timer_seconds() as i32;
        let centis = i32::from(self.game.vs_timer_centiseconds());
        self.game.set_timer_digits(Some(j), seconds, centis);
        if !self.game.vs_timer_counts_up() && centis == 0 && seconds == 5 {
            let model = self.countdown_model();
            let g = &mut *self.game;
            let j2 = jobj::load_joint(&mut g.objs, data, data.ptr(model))
                .expect("the original's assertion failed: jobj dont't get (ifAddTimeDownModel)");
            let cd = g.hud.time.countdown_timer.expect("the countdown's GObj");
            g.world.set_obj(cd, OBJ_JOBJ, Obj::JObj(j2));
            g.world.setup_gx_link(cd, CodeRef(JOBJ_RENDER), 11, 0);
            let s = g.countdown_seconds();
            g.hud.time.countdown_seconds = s as u8;
            anim::add_model_anim(&mut g.objs, data, Some(j2), u32::from(s as u8), model);
            jobj::req_anim_all(&mut g.objs, Some(j2), 0.0);
            jobj::anim_all(
                &mut g.objs,
                data,
                &mut crate::game::code::ParticleHooks,
                Some(j2),
            );
            let pos = g.timer_position();
            jobj::set_translate(&mut g.objs.jobjs, j2, pos);
            g.world.setup_proc(cd, CodeRef(PROC_COUNTDOWN), 17);
            if let Some(t) = g.hud.time.match_timer.take() {
                gobj::free(self, t);
            }
        }
        jobj::anim_all(
            &mut self.game.objs,
            data,
            &mut crate::game::code::ParticleHooks,
            Some(j),
        );
    }

    /// Makes the timer if the match is timed, with an empty GObj for the countdown,
    /// `ifTime_CreateTimers` (its models through the inline `ifTime_LoadModels`).
    #[doc(alias = "ifTime_CreateTimers")]
    pub fn create_timers(&mut self) {
        let data = self.data;
        let rules = &self.game.vs.start;
        if !rules.x1_0 && !rules.timer_enabled {
            self.game.hud.time.match_timer = None;
            return;
        }
        let a = self.game.hud.archive.expect("IfAll.dat");
        let sym = |name: &str| {
            let at = data
                .get(a)
                .public(name)
                .unwrap_or_else(|| panic!("IfAll.dat's {name}"));
            DataRef {
                archive: a,
                offset: at,
            }
        };
        let tim = sym("ScInfTim_scene_models");
        let g = &mut *self.game;
        g.hud.time.countdown_timer_models = Some(sym("tdsce"));
        let Some(first) = data.ptr(tim) else {
            return;
        };
        let f = |o: u32| data.ptr(first.at(o));
        g.hud.timer_models = ModelDesc {
            joint: f(0),
            anims: f(4),
            matanims: f(8),
            shapeanims: f(0xC),
        };
        g.hud.time.countdown_timer = Some(g.world.create(0xE, 15, 0));
        let o = g.world.create(0xE, 16, 0);
        let m = g.hud.timer_models;
        let j = jobj::load_joint(&mut g.objs, data, m.joint)
            .expect("the original's assertion failed: jobj dont't get (ifAddTime)");
        g.world.set_obj(o, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(o, CodeRef(JOBJ_RENDER), 11, 0);
        anim::add_anim_index(
            &mut g.objs,
            data,
            Some(j),
            0,
            m.anims,
            m.matanims,
            m.shapeanims,
        );
        jobj::req_anim_all(&mut g.objs, Some(j), 0.0);
        g.world.setup_proc(o, CodeRef(PROC_TIMER), 17);
        let pos = g.timer_position();
        jobj::set_translate(&mut g.objs.jobjs, j, pos);
        let mut d = g.objs.jobjs[j].child;
        g.hud.time.digits[0] = d;
        for i in 1..10 {
            d = d.and_then(|x| g.objs.jobjs[x].next);
            g.hud.time.digits[i] = d;
        }
        g.hud.time.match_timer = Some(o);
    }

    /// Frees the timer and the countdown, `ifTime_FreeTimers`.
    #[doc(alias = "ifTime_FreeTimers")]
    pub fn free_timers(&mut self) {
        if let Some(g) = self.game.hud.time.match_timer.take() {
            gobj::free(self, g);
        }
        if let Some(g) = self.game.hud.time.countdown_timer.take() {
            gobj::free(self, g);
        }
    }
}
