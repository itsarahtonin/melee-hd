//! The crowd (`melee/sfx/crowdsfx.c`): its gasps, chants and cheers as fighters fly, recover
//! and near the blast zones. Its state machine moves on only when the crowd's chant has
//! stopped playing, so how long a sound plays (the audio driver's voices and the DSP model)
//! decides when a knockback starts a gasp, which fighter the crowd follows, and the bonus a
//! gasp awards (stat 0x5B).

// NaN takes the original's branches, which negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::code::Val;
use super::handles::CodeRef;
use super::state::Running;
use crate::fpu::reg;
use crate::hsd::gobj::GObjId;
use crate::lb::audio::SFX_NONE;

/// `crowdsfx`: the crowd's state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crowd {
    /// `crowdsfx_ptr` points at the state: the match set the crowd up.
    pub set_up: bool,
    /// `x0`, `x4`, `x8`: the last knockback the crowd reacted to (whose, the ticks since, how
    /// hard).
    pub last_spawn: u32,
    pub since: i32,
    pub last_kb: f32,
    /// `xC`: the fighter the crowd follows (its spawn number), 0 for none.
    pub focus: u32,
    /// `x10`: ticks since the last cheer, up to the config's `cheer_limit`.
    pub cheer: i32,
    /// `x14`: the followed fighter's chant.
    pub chant: i32,
    /// `x18`: chants so far, up to the config's `max_gasp_count`.
    pub chants: i32,
    /// `x1C`, `x20`: end the chanting with a cheer (and the big one).
    pub end: i32,
    pub big_end: i32,
    /// `x24`: fighters near the bottom blast zone.
    pub near_bottom: i32,
    /// `x28`, `x2C`: the cheer's and the chant's handles.
    pub cheer_handle: i32,
    pub chant_handle: i32,
}

impl Default for Crowd {
    fn default() -> Self {
        Crowd {
            set_up: false,
            last_spawn: 0,
            since: 0,
            last_kb: 0.0,
            focus: 0,
            cheer: 0,
            chant: 0,
            chants: 0,
            end: 0,
            big_end: 0,
            near_bottom: 0,
            cheer_handle: 0,
            chant_handle: 0,
        }
    }
}

/// The crowd's functions game code calls, by address.
pub mod addr {
    pub const SETUP: u32 = 0x8032_1900;
    pub const PROC: u32 = 0x8032_19AC;
    pub const END_CHANT: u32 = 0x8032_1C70;
    pub const REACT: u32 = 0x8032_201C;
    pub const KB_CATEGORY: u32 = 0x8032_2298;
    pub const ANGLE_SCALE: u32 = 0x8032_22EC;
    pub const TRANSFORM: u32 = 0x8032_2314;
    pub const KNOCKBACK: u32 = 0x8032_233C;
    pub const SIDE_BLAST: u32 = 0x8032_24DC;
    pub const RECOVERY: u32 = 0x8032_2598;
}

/// The crowd's GObj: classifier and process link.
const CLASS: u16 = 0x16;
const P_LINK: u8 = 0x17;
/// The fighters' process link.
const P_LINK_FIGHTER: u8 = 8;
/// `ftCo_MS_DeadDown` to `ftCo_MS_DeadUpFallHitCameraIce`.
const MS_DEAD: std::ops::RangeInclusive<i32> = 0..=10;

/// The crowd's entry points game code reaches through [`super::code::call`].
pub fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let int = |i: usize| args[i].int();
    let float = |i: usize| args[i].float();
    Some(match f {
        addr::SETUP => {
            r.crowd_setup();
            Val::Int(0)
        }
        addr::END_CHANT => {
            r.crowd_end_chant();
            Val::Int(0)
        }
        addr::REACT => Val::Int(i32::from(r.crowd_react(int(0) as u32, int(1)))),
        addr::KB_CATEGORY => Val::Int(r.crowd_kb_category(float(0))),
        addr::ANGLE_SCALE => Val::Float(r.crowd_angle_scale(float(0), float(1))),
        addr::TRANSFORM => {
            r.crowd_transform();
            Val::Int(0)
        }
        addr::KNOCKBACK => {
            r.crowd_knockback(int(0) as u32, int(1) as u32);
            Val::Int(0)
        }
        addr::SIDE_BLAST => Val::Int(i32::from(r.crowd_side_blast(
            int(0) as u32,
            float(1),
            float(2),
        ))),
        addr::RECOVERY => Val::Int(r.crowd_recovery(int(0) as u32, float(1))),
        _ => return None,
    })
}

/// `gCrowdConfig`'s fields, by offset.
mod cfg {
    pub const KB_LOW: u32 = 0x0;
    pub const KB_MID: u32 = 0x4;
    pub const KB_HIGH: u32 = 0x8;
    pub const ANGLE_MIN: u32 = 0xC;
    pub const ANGLE_MAX: u32 = 0x10;
    pub const ANGLE_MULT: u32 = 0x14;
    pub const REACT_TICKS: u32 = 0x18;
    pub const MIN_PERCENT: u32 = 0x1C;
    pub const CHEER_LIMIT: u32 = 0x20;
    pub const END_FROM: u32 = 0x24;
    pub const MAX_CHANTS: u32 = 0x28;
    pub const RECOVERY_HIGH: u32 = 0x30;
    pub const RECOVERY_MID: u32 = 0x34;
    pub const RECOVERY_LOW: u32 = 0x38;
    pub const NEAR_BOTTOM: u32 = 0x3C;
    pub const BOTTOM_OFFSET: u32 = 0x40;
}

impl Running<'_> {
    /// `gCrowdConfig`, from `PlCo.dat`'s common data.
    fn crowd_cfg(&self, offset: u32) -> melee_data::archive::DataRef {
        let base = self
            .game
            .ft
            .load_common
            .expect("PlCo.dat's common data isn't loaded");
        self.data
            .ptr(base.at(0x54))
            .expect("no crowd config")
            .at(offset)
    }

    fn cfg_i32(&self, offset: u32) -> i32 {
        self.data.u32(self.crowd_cfg(offset)) as i32
    }

    fn cfg_f32(&self, offset: u32) -> f64 {
        reg(self.data.f32(self.crowd_cfg(offset)))
    }

    /// The fighter with spawn number `spawn`, `ftLib_FindBySpawnNum`.
    fn fighter_by_spawn(&self, spawn: u32) -> Option<GObjId> {
        let mut cur = self.game.world.first_in(P_LINK_FIGHTER);
        while let Some(g) = cur {
            if self.game.fighter(g).x8_spawnNum as u32 == spawn {
                return Some(g);
            }
            cur = self.game.world.gobj(g).next;
        }
        None
    }

    /// The fighter's player and whether it's the second Ice Climber.
    fn fighter_slot(&self, g: GObjId) -> (i32, i32) {
        let fp = self.game.fighter(g);
        (i32::from(fp.player_idx), i32::from(fp.is_sub_fighter))
    }

    /// Sets up the crowd for a match, `sfx_setupCrowdSFX`: its GObj and process, and its
    /// state from scratch (`sfx_initCrowdSFX`).
    #[doc(alias = "sfx_setupCrowdSFX")]
    #[doc(alias = "sfx_initCrowdSFX")]
    pub fn crowd_setup(&mut self) {
        let g = self.game.world.create(CLASS, P_LINK, 0);
        self.game.world.setup_proc(g, CodeRef(addr::PROC), 0x13);
        let cheer = self.cfg_i32(cfg::CHEER_LIMIT);
        let chants = self.cfg_i32(cfg::MAX_CHANTS);
        self.game.crowd = Crowd {
            set_up: true,
            last_spawn: 0,
            since: 0x10000,
            last_kb: 0.0,
            focus: 0,
            cheer,
            chant: SFX_NONE,
            chants,
            end: 0,
            big_end: 0,
            near_bottom: 0,
            cheer_handle: -1,
            chant_handle: -1,
        };
    }

    /// The crowd's process, `fn_803219AC`: the chanting, then the fighters near the bottom.
    #[doc(alias = "fn_803219AC")]
    pub fn crowd_proc(&mut self) {
        let c = &mut self.game.crowd;
        if c.since < 0x10000 {
            c.since += 1;
        }
        self.crowd_chanting();
        self.crowd_bottom();
    }

    /// `un_80321A00`: once the chant has stopped, the next one, or the end of the chanting
    /// with a cheer; past the last, the ticks since the cheer.
    #[doc(alias = "un_80321A00")]
    fn crowd_chanting(&mut self) {
        let max = self.cfg_i32(cfg::MAX_CHANTS);
        if self.game.crowd.chants >= max {
            let limit = self.cfg_i32(cfg::CHEER_LIMIT);
            let c = &mut self.game.crowd;
            if c.cheer < limit {
                c.cheer += 1;
            }
            return;
        }
        if self.sfx_playing(self.game.crowd.chant_handle) {
            return;
        }
        let c = &mut self.game.crowd;
        c.chants += 1;
        if c.chants < max {
            if c.end != 0 {
                c.end = 0;
                c.cheer = 0;
                c.chants = max;
                self.crowd_stop_chant();
                if self.game.crowd.big_end != 0 {
                    self.crowd_cheer(0x144);
                    self.game.crowd.big_end = 0;
                }
            } else {
                let chant = c.chant;
                self.crowd_chant(chant);
            }
        } else {
            c.cheer = 0;
            self.crowd_stop_chant();
            self.crowd_cheer(0x140);
        }
    }

    /// `un_80321AF4`: counts the living fighters near the bottom blast zone; when as many as
    /// the config's count first are, the crowd gasps (at the followed fighter, if it isn't
    /// one of them).
    #[doc(alias = "un_80321AF4")]
    fn crowd_bottom(&mut self) {
        let old = self.game.crowd.near_bottom;
        self.game.crowd.near_bottom = 0;
        let line = fp::fadds(
            self.cfg_f32(cfg::BOTTOM_OFFSET),
            reg(self
                .game
                .geometry
                .as_ref()
                .expect("no collision geometry")
                .bounds[1]
                .bottom),
        );
        let mut followed = false;
        let mut cur = self.game.world.first_in(P_LINK_FIGHTER);
        while let Some(g) = cur {
            let fp = self.game.fighter(g);
            if !MS_DEAD.contains(&fp.motion_id) && !fp.is_sleeping {
                if reg(fp.cur_pos.y) < line {
                    self.game.crowd.near_bottom += 1;
                } else if self.game.crowd.focus == fp.x8_spawnNum as u32 {
                    followed = true;
                }
            }
            cur = self.game.world.gobj(g).next;
        }
        let n = self.cfg_i32(cfg::NEAR_BOTTOM);
        if old < n && self.game.crowd.near_bottom >= n {
            if followed {
                let focus = self.game.crowd.focus;
                self.crowd_react(focus, 3);
            } else {
                self.crowd_gasp(3);
            }
        }
    }

    /// `un_80321BF8`: a chant on channel 5 (`lbAudioAx_800240B4`).
    #[doc(alias = "un_80321BF8")]
    fn crowd_chant(&mut self, id: i32) {
        self.game.crowd.chant_handle = self.sfx_play_channel(id, 5);
    }

    /// `un_80321C28`: stops the chant if it plays.
    #[doc(alias = "un_80321C28")]
    fn crowd_stop_chant(&mut self) {
        let h = self.game.crowd.chant_handle;
        if self.sfx_playing(h) {
            self.sfx_stop(h);
        }
        self.game.crowd.chant_handle = -1;
    }

    /// `un_80321C70`: the chanting ends with a cheer after this chant, once enough chants
    /// have played.
    #[doc(alias = "un_80321C70")]
    pub fn crowd_end_chant(&mut self) {
        let (max, from) = (self.cfg_i32(cfg::MAX_CHANTS), self.cfg_i32(cfg::END_FROM));
        let c = &mut self.game.crowd;
        if c.chants >= max || c.chants < from {
            return;
        }
        c.end = 1;
    }

    /// `un_80321CA4`: a cheer on channel 6 (`lbAudioAx_8002411C`), cutting off the last.
    #[doc(alias = "un_80321CA4")]
    fn crowd_cheer(&mut self, id: i32) {
        self.crowd_stop_cheer();
        self.game.crowd.cheer_handle = self.sfx_play_channel(id, 6);
    }

    /// `un_80321CE8`: stops the cheer if it plays.
    #[doc(alias = "un_80321CE8")]
    fn crowd_stop_cheer(&mut self) {
        let h = self.game.crowd.cheer_handle;
        if self.sfx_playing(h) {
            self.sfx_stop(h);
        }
        self.game.crowd.cheer_handle = -1;
    }

    /// `un_80321D30`: a fighter sent flying: a hard enough hit may start the crowd following
    /// it; otherwise a cheer by how hard.
    #[doc(alias = "un_80321D30")]
    fn crowd_flying(&mut self, spawn: u32, kb: f64) {
        let cat = self.crowd_kb_category(kb);
        if cat >= 2 && self.crowd_follow(spawn, kb) {
            self.crowd_stop_cheer();
            return;
        }
        match cat {
            3 => self.crowd_cheer(0x140),
            2 => self.crowd_cheer(0x141),
            1 => self.crowd_cheer(0x142),
            _ => {}
        }
        if spawn != 0 && self.game.crowd.focus == spawn {
            self.crowd_end_chant();
        }
    }

    /// `un_80321EBC`: the crowd starts following a fighter hit hard at high percent, chanting
    /// its name, once the last cheer is long enough ago, and awards its player bonus 0x5B.
    #[doc(alias = "un_80321EBC")]
    fn crowd_follow(&mut self, spawn: u32, kb: f64) -> bool {
        let Some(g) = self.fighter_by_spawn(spawn) else {
            return false;
        };
        let (slot, sub) = self.fighter_slot(g);
        if self.game.player_kind(slot, sub) == 1 {
            return false;
        }
        let percent = fp::fctiwz(reg(self.game.fighter(g).dmg.x1830_percent));
        if percent < self.cfg_i32(cfg::MIN_PERCENT)
            || self.game.crowd.cheer < self.cfg_i32(cfg::CHEER_LIMIT)
            || self.game.crowd.focus == spawn
        {
            return false;
        }
        let chant = self.fighter_chant(g);
        self.game.crowd.chant = chant;
        if chant == SFX_NONE {
            return false;
        }
        self.crowd_stop_chant();
        let id = if self.crowd_kb_category(kb) == 3 {
            0x140
        } else {
            0x141
        };
        self.crowd_chant(id);
        let c = &mut self.game.crowd;
        c.focus = spawn;
        c.chants = 0;
        let (slot, _) = self.fighter_slot(g);
        // `pl_8003FDA0`: bonus 0x5B.
        assert!((0..6).contains(&slot), "player slot {slot}");
        self.game.players[slot as usize].stale_moves.x904[0x5B] = 1;
        true
    }

    /// The crowd's chant for a fighter, `ftLib_8008746C`: the Ice Climbers' second and later
    /// costumes have one of their own.
    fn fighter_chant(&self, g: GObjId) -> i32 {
        let fp = self.game.fighter(g);
        // `ftCommon_80080144`: Popo or Nana.
        if (fp.kind == 0xA || fp.kind == 0xB) && fp.costume_id >= 2 {
            return 0x1FBD1;
        }
        let sfx = self
            .data
            .ptr(fp.ft_data.expect("a fighter without data").at(0x4C))
            .expect("a fighter without sounds");
        self.data.u32(sfx.at(0x34)) as i32
    }

    /// `un_8032201C`: a gasp by category, the chanting ending if it's at the followed
    /// fighter, and a count toward the fighter's player's bonus 0x5C.
    #[doc(alias = "un_8032201C")]
    pub fn crowd_react(&mut self, spawn: u32, cat: i32) -> bool {
        match cat {
            3 => self.crowd_cheer(0x13D),
            2 => self.crowd_cheer(0x13E),
            1 => self.crowd_cheer(0x13F),
            0 => return false,
            _ => {}
        }
        if spawn != 0 {
            if self.game.crowd.focus == spawn {
                self.crowd_end_chant();
            }
            let g = self
                .fighter_by_spawn(spawn)
                .expect("the crowd's reaction names no fighter");
            let (slot, _) = self.fighter_slot(g);
            // `pl_8003FDC8`.
            let p = &mut self.game.players[slot as usize];
            let n = p.ext_u32(0xD70);
            p.set_ext_u32(0xD70, n.wrapping_add(1));
        }
        true
    }

    /// `un_80322178`: a gasp by category.
    #[doc(alias = "un_80322178")]
    fn crowd_gasp(&mut self, cat: i32) {
        match cat {
            3 => self.crowd_cheer(0x13D),
            2 => self.crowd_cheer(0x13E),
            1 => self.crowd_cheer(0x13F),
            _ => {}
        }
    }

    /// `un_80322298`: how hard a knockback is, 0 to 3.
    #[doc(alias = "un_80322298")]
    pub fn crowd_kb_category(&self, kb: f64) -> i32 {
        if kb >= self.cfg_f32(cfg::KB_HIGH) {
            3
        } else if kb >= self.cfg_f32(cfg::KB_MID) {
            2
        } else if kb >= self.cfg_f32(cfg::KB_LOW) {
            1
        } else {
            0
        }
    }

    /// `un_803222EC`: a knockback scaled up within the config's angle range.
    #[doc(alias = "un_803222EC")]
    pub fn crowd_angle_scale(&self, mag: f64, angle: f64) -> f64 {
        if !(angle > self.cfg_f32(cfg::ANGLE_MIN)) || !(angle < self.cfg_f32(cfg::ANGLE_MAX)) {
            return mag;
        }
        fp::fmuls(mag, self.cfg_f32(cfg::ANGLE_MULT))
    }

    /// `un_80322314`: a fighter transformed (Zelda and Sheik): the chanting ends with the big
    /// cheer.
    #[doc(alias = "un_80322314")]
    pub fn crowd_transform(&mut self) {
        let max = self.cfg_i32(cfg::MAX_CHANTS);
        let c = &mut self.game.crowd;
        if c.chants >= max {
            return;
        }
        c.end = 1;
        c.big_end = 1;
    }

    /// `un_8032233C`: `victim` sent flying by `attacker`'s hit: the crowd reacts by how hard
    /// (the harder of a quick second hit's).
    #[doc(alias = "un_8032233C")]
    pub fn crowd_knockback(&mut self, victim: u32, attacker: u32) {
        let g = self.fighter_by_spawn(victim);
        let a = self
            .fighter_by_spawn(attacker)
            .expect("ftLib_GetCrowdKnockback of no fighter");
        let kb = reg(self.game.fighter(a).dmg.x18A4_knockbackMagnitude);
        let cat = self.crowd_kb_category(kb);
        if cat == 0 {
            return;
        }
        let victim_kb = g.map(|g| reg(self.game.fighter(g).dmg.x18A4_knockbackMagnitude));
        if victim_kb.is_some_and(|k| k >= 3.0) {
            self.crowd_flying(victim, kb);
        } else if self.game.crowd.last_spawn == victim
            && fp::frsp(f64::from(self.game.crowd.since)) < self.cfg_f32(cfg::REACT_TICKS)
        {
            let last = reg(self.game.crowd.last_kb);
            self.crowd_flying(victim, if kb > last { kb } else { last });
        } else {
            match cat {
                3 => self.crowd_cheer(0x144),
                2 => self.crowd_cheer(0x145),
                _ => self.crowd_cheer(0x146),
            }
            if cat == 3 || (cat == 2 && self.game.crowd.focus == attacker) {
                self.crowd_end_chant();
            }
        }
        let c = &mut self.game.crowd;
        c.since = 0;
        c.last_spawn = victim;
        c.last_kb = crate::fpu::mem(kb);
    }

    /// `un_803224DC`: a fighter flying near a side blast zone makes the crowd react.
    #[doc(alias = "un_803224DC")]
    pub fn crowd_side_blast(&mut self, spawn: u32, x: f64, kb: f64) -> bool {
        let cat = self.crowd_kb_category(kb);
        if self.near_side_blast_zone(crate::fpu::mem(x)) {
            return self.crowd_react(spawn, cat);
        }
        false
    }

    /// `un_80322598`: a fighter recovering from below the stage: the crowd reacts by how far
    /// below. Returns what the original leaves in r3: 0, or the reaction's result.
    #[doc(alias = "un_80322598")]
    pub fn crowd_recovery(&mut self, spawn: u32, y: f64) -> i32 {
        let bottom = reg(self
            .game
            .geometry
            .as_ref()
            .expect("no collision geometry")
            .bounds[1]
            .bottom);
        if y >= bottom || y < fp::fadds(self.cfg_f32(cfg::RECOVERY_LOW), bottom) {
            return 0;
        }
        let cat = if y > fp::fadds(self.cfg_f32(cfg::RECOVERY_HIGH), bottom) {
            3
        } else if y > fp::fadds(self.cfg_f32(cfg::RECOVERY_MID), bottom) {
            2
        } else {
            1
        };
        i32::from(self.crowd_react(spawn, cat))
    }
}
