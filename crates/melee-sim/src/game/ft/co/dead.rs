//! Leaving play: past a blast zone (`ft_0D31.c`), the deaths and the star and screen KOs.

use gekko_fp as fp;

use super::super::super::code::{run_fighter_cb, unported_call};
use super::super::super::gr::stage;
use super::super::super::it::item;
use super::super::super::state::Running;
use super::super::sound::Track;
use super::ids::ms;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// The death states' variables (`ftCommon_MotionVars_unk_deadup`, `unk_800D331C`): the
/// phase's frames left, the phase, whether it's frozen, and the state and attack it died in.
const DEAD_FRAMES: usize = 0x0;
const DEAD_PHASE: usize = 0x4;
const DEAD_FROZEN: usize = 0x28;
const DEAD_MOTION: usize = 0x2C;
const DEAD_ATTACK: usize = 0x30;

/// The slot that stands for no one.
const NO_SLOT: i32 = 6;

/// The order `ft_80088C5C` stops the fighter's tracks in.
const TRACKS: [Track; 7] = [
    Track::Effect,
    Track::Voice,
    Track::Voice2,
    Track::Effect2,
    Track::Effect3,
    Track::Effect4,
    Track::Effect5,
];

impl Running<'_> {
    /// What a fighter leaving play lets go of, `ftCo_800D331C`: a holder's victim or a held
    /// fighter's holder gets its bonus, the state's death hooks run, a grab ends, it stops,
    /// its item goes, its effects end, and its state and attack are kept for the stats.
    #[doc(alias = "ftCo_800D331C")]
    #[doc(alias = "pl_8004065C")]
    #[doc(alias = "ftCo_800DD100")]
    #[doc(alias = "pl_8003EA08")]
    #[doc(alias = "pl_8003EA74")]
    fn leave_play(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let (slot, sub) = (i32::from(fp.player_idx), fp.is_sub_fighter);
        if let Some(v) = fp.victim_gobj {
            if fp.x221B.x221B_b5 {
                let vf = self.game.fighter_mut(v);
                if !vf.is_sub_fighter {
                    vf.x2180 = slot;
                }
            } else if !sub {
                let vf = self.game.fighter(v);
                if !vf.is_sub_fighter {
                    let vs = i32::from(vf.player_idx);
                    self.count_bonus(vs, 0x78);
                }
            }
        }
        let holder = self.game.fighters[f.0].x2180;
        if holder != NO_SLOT {
            self.count_bonus(holder, 0x78);
        }
        if let Some(cb) = self.game.fighters[f.0].death2_cb {
            run_fighter_cb(self, gobj, cb);
        }
        if let Some(cb) = self.game.fighters[f.0].death3_cb {
            run_fighter_cb(self, gobj, cb);
        }
        if let Some(cb) = self.game.fighters[f.0].death1_cb {
            run_fighter_cb(self, gobj, cb);
        }
        // `ftCo_800DD100`: a grab ends, and the other fighter waits or falls.
        if let Some(v) = self.game.fighters[f.0].victim_gobj {
            if self.game.fighters[f.0].x221B.x221B_b5 {
                self.grab_release(gobj, v);
            } else {
                self.grab_release(v, gobj);
            }
            self.fall_or_wait(v);
        }
        self.stop_dead(gobj);
        if let Some(it) = self.game.fighters[f.0].item_gobj {
            item::destroy(self, it);
            self.game.fighters[f.0].item_gobj = None;
            if !sub {
                self.game.player_mut(slot).set_ext_u32(0xD6C, 6);
                self.game.player_mut(slot).set_ext_u32(0xD44, 0);
            }
        }
        let fp = &self.game.fighters[f.0];
        if fp.x197C.is_some() || fp.x1980.is_some() {
            unported_call("ftCo_800D331C with a worn item");
        }
        if fp.is_metal {
            unported_call("ftCo_800C8540 (metal's end)");
        }
        if fp.x2226_b4 {
            unported_call("ftCo_800C8A64");
        }
        self.destroy_effects(gobj);
        let fp = &mut self.game.fighters[f.0];
        let (m, attack) = (fp.motion_id, i32::from(fp.x2070[3]));
        fp.set_mv_i32(DEAD_MOTION, m);
        fp.set_mv_i32(DEAD_ATTACK, attack);
    }

    /// The flags of a fighter out of play (`setDeadFlags`): its crowd gasp stops
    /// (`ft_800880D8`), and it can't be hit or grabbed.
    #[doc(alias = "ft_800880D8")]
    fn set_dead_flags(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.x221D_b6 {
            let n = fp.x2164;
            fp.x2164 = 0;
            self.lb_audio_crowd_a_less(n);
            self.game.fighter_mut(gobj).x2004 = 0;
        }
        let fp = self.game.fighter_mut(gobj);
        fp.x2219_b1 = true;
        fp.x221E_b1 = true;
        fp.x221E_b2 = true;
    }

    /// The fighter's sounds stopped, `ft_80088C5C`: its sound objects, the last sound it
    /// played, each track's sound it owns, then every track of its slot.
    #[doc(alias = "ft_80088C5C")]
    pub fn stop_fighter_sounds(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.game.lb_stop_owned_sound_objects(gobj);
        let last = self.game.fighters[f.0].x2160;
        if last != -1 {
            self.game.lb_stop_owned_sound_object(gobj, last);
            self.game.fighters[f.0].x2160 = -1;
        }
        for t in TRACKS {
            let h = self.track_handle(f, t);
            if h != -1 && self.game.lb_stop_owned_sound_object(gobj, h) {
                self.fighter_stop_track(f, t);
            }
        }
        for t in TRACKS {
            self.fighter_key_off_tracks(f, t);
        }
    }

    /// The star KO, `ftCo_800D40B8`: out of play, the fighter flies off into the background
    /// with its scream.
    #[doc(alias = "ftCo_800D40B8")]
    pub fn enter_dead_up_star(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let frames = self.common().i32(0x504);
        self.leave_play(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(DEAD_FRAMES, frames);
        fp.set_mv_i32(DEAD_PHASE, 0);
        self.change_motion_state(gobj, ms::DEAD_UP_STAR, 0, 0.0, 1.0, 0.0, None);
        self.set_dead_flags(gobj);
        self.stop_fighter_sounds(gobj);
        // `ftCommon_8007EFC0`.
        self.game.fighters[f.0].x209A = 1;
        let scream = self.character_sfx(gobj, 0xC);
        self.fighter_play_track(f, Track::Voice, scream, 127, 64);
        let fp = &self.game.fighters[f.0];
        let (slot, sub) = (i32::from(fp.player_idx), fp.is_sub_fighter);
        self.record_ko_attacker(slot, sub);
        self.game.fighters[f.0].set_mv_i32(DEAD_FROZEN, 0);
    }

    /// `ftCo_DeadUpStar_Anim`: the star KO's phases. Its wait over, the fighter flies toward
    /// the background's top; there it twinkles and is out of the match; then it sleeps.
    #[doc(alias = "ftCo_DeadUpStar_Anim")]
    pub fn co_dead_up_star_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        if fp.mv_i32(DEAD_PHASE) == 1 && fp.mv_i32(DEAD_FROZEN) != 0 {
            unported_call("ftCo_DeadUpStar_Anim frozen");
        }
        let fp = &mut self.game.fighters[f.0];
        if fp.mv_i32(DEAD_FRAMES) != 0 {
            fp.set_mv_i32(DEAD_FRAMES, fp.mv_i32(DEAD_FRAMES).wrapping_sub(1));
        }
        if fp.mv_i32(DEAD_FRAMES) != 0 {
            return;
        }
        match fp.mv_i32(DEAD_PHASE) {
            0 => {
                let c = self.common();
                let (flight, depth, height) = (c.i32(0x508), c.f32(0x510), c.f32(0x514));
                let top = stage::cam_top(self.game);
                let fp = &mut self.game.fighters[f.0];
                let n = fp::frsp(f64::from(flight));
                fp.self_vel.y = mem(fp::fdivs(
                    fp::fmsubs(reg(height), reg(top), reg(fp.cur_pos.y)),
                    n,
                ));
                fp.self_vel.z = mem(fp::fdivs(reg(depth), n));
                fp.set_mv_i32(DEAD_FRAMES, flight);
                fp.set_mv_i32(DEAD_PHASE, 1);
            }
            1 => {
                self.stop_dead(gobj);
                if self.game.fighters[f.0].mv_i32(DEAD_FROZEN) != 0 {
                    self.destroy_effects(gobj);
                }
                let pos = self.game.fighters[f.0].cur_pos;
                self.spawn_effect_at(gobj, 0x42D, pos);
                self.drop_coins(gobj);
                let fp = &mut self.game.fighters[f.0];
                fp.x221F_b1 = true;
                fp.invisible = true;
                self.stop_fighter_sounds(gobj);
                self.play_sfx(gobj, 0x83, 0x7F, 0x40);
                // `ft_8008805C`.
                let id = self.fighter_sfx(f, 0x83);
                self.lb_audio_queue(id);
                self.count_ko(gobj);
                let twinkle = self.common().i32(0x50C);
                let fp = &mut self.game.fighters[f.0];
                fp.set_mv_i32(DEAD_FRAMES, twinkle);
                fp.set_mv_i32(DEAD_PHASE, 2);
            }
            2 => self.out_of_play(gobj),
            _ => {}
        }
    }

    /// The camera follows a fighter flying off the top, `ftCamera_80076320`: its box's x
    /// scaled from the camera's top to the blast zone's, its y at the zone.
    #[doc(alias = "ftCamera_80076320")]
    #[doc(alias = "ftCo_DeadUpStar_Cam")]
    pub fn dead_up_cam(&mut self, gobj: GObjId) {
        self.update_camera_box(gobj);
        let center = stage::cam_center(self.game);
        let (top, cam_top) = (stage::blast_top(self.game), stage::cam_top(self.game));
        let d = fp::fsubs(reg(top), reg(center.y));
        assert!(d != 0.0, "stGetPlyDeadUp() - center_pos.y != 0.0F");
        let c = fp::fsubs(reg(cam_top), reg(center.y));
        let b = self
            .game
            .fighter(gobj)
            .x890_cameraBox
            .expect("a fighter without a camera box");
        let cb = &mut self.game.camera.boxes[b];
        cb.pos.x = mem(fp::fdivs(fp::fmuls(reg(cb.pos.x), c), d));
        cb.pos.y = top;
    }

    /// A KO counted, `ftCo_800D34E0`: a fall, its attacks fresh again, a stock gone (the
    /// last's time noted), the KO for whoever hit it last or a self-destruct, the death's
    /// stats, and no damage.
    #[doc(alias = "ftCo_800D34E0")]
    fn count_ko(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        let (slot, sub) = (i32::from(fp.player_idx), fp.is_sub_fighter);
        let which = i32::from(sub);
        let falls = self.game.player_falls_at(slot, which);
        self.game
            .set_player_falls_at(slot, which, falls.wrapping_add(1));
        self.game.reset_stale_moves(slot);
        if (self.game.vs_is_stock() || self.game.vs_x4_2())
            && self.game.player_entity(slot) == Some(gobj)
        {
            self.game.player_lose_stock(slot);
        }
        if self.game.vs_is_stock() && self.game.player_stocks(slot) <= 0 {
            self.game.update_player_match_frame_count(slot, which);
        }
        let source = self.game.fighter(gobj).dmg.x18c4_source_ply;
        if source != NO_SLOT {
            self.player_count_ko(source, sub, slot);
        } else {
            self.player_count_self_destruct(slot, which);
        }
        let fp = self.game.fighter(gobj);
        let (motion, attack) = (fp.mv_i32(DEAD_MOTION), fp.mv_i32(DEAD_ATTACK));
        self.record_death(slot, sub, motion, attack);
        self.game.set_player_damage_at(slot, which, 0);
    }

    /// Coins dropped at a KO in a coin match, `ftCo_800D4E50` (not ported there).
    #[doc(alias = "ftCo_800D4E50")]
    fn drop_coins(&mut self, gobj: GObjId) {
        if self.game.vs_is_coins() && !self.game.fighter(gobj).is_sub_fighter {
            unported_call("ftCo_800D4E50 in a coin match");
        }
    }

    /// The fighter out of play for now, `ftCo_800BFD9C`: asleep, Nana with Popo, and the
    /// match told (`gm_80167320`).
    #[doc(alias = "ftCo_800BFD9C")]
    pub fn out_of_play(&mut self, gobj: GObjId) {
        self.sleep(gobj);
        if self.game.fighter(gobj).x2222_b5 {
            unported_call("ftCo_800BFD9C for Popo");
        }
        let fp = self.game.fighter(gobj);
        let (slot, sub) = (i32::from(fp.player_idx), fp.is_sub_fighter);
        self.fighter_out_of_play(slot, sub);
    }

    /// The last attacker's KO noted for a fighter leaving play, `pl_8003DF44`: unless no one,
    /// itself or a teammate hit it last.
    #[doc(alias = "pl_8003DF44")]
    fn record_ko_attacker(&mut self, slot: i32, sub: bool) {
        let me = self
            .game
            .player_entity_at(slot, i32::from(sub))
            .expect("a slot without its fighter");
        let last = self.game.ft_last_attacker(me);
        // `pl_CheckIfSameTeam`.
        let team =
            self.game.vs_is_teams() && self.game.player_team(last) == self.game.player_team(slot);
        if last == NO_SLOT || last == slot || team {
            return;
        }
        let t = self.bonus_param(0xB4);
        self.game.player_mut(last).set_ext_u32(0xD54, t);
    }

    pub fn enter_dead_right(&mut self, _gobj: GObjId) {
        unported_call("ftCo_800D3950");
    }
    pub fn enter_dead_left(&mut self, _gobj: GObjId) {
        unported_call("ftCo_800D3680");
    }
    pub fn enter_dead_down(&mut self, _gobj: GObjId) {
        unported_call("ftCo_800D3BC8");
    }
    pub fn enter_dead_up(&mut self, _gobj: GObjId) {
        unported_call("ftCo_800D3E40");
    }
    pub fn enter_dead_up_fall(&mut self, _gobj: GObjId) {
        unported_call("ftCo_800D4780");
    }
    pub fn enter_dead_up_fall_ice(&mut self, _gobj: GObjId) {
        unported_call("ftCo_800D47B8");
    }
    pub fn enter_dead_up_star_ice(&mut self, _gobj: GObjId) {
        unported_call("ftCo_800D41C4");
    }
}
