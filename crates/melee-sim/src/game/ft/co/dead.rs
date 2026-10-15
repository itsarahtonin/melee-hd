//! Leaving play: past a blast zone (`ft_0D31.c`), the deaths and the star and screen KOs.

use gekko_fp as fp;

use super::super::super::code::{run_fighter_cb, unported_call};
use super::super::super::ef::Arg;
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
/// The screen KO's: the approach's step and how far along it is (`walk.slow_anim_frame`,
/// `walk.middle_anim_frame`), the fighter's place in the game camera's view (`x50`) and the
/// fall's motion still to add to it (`x5C`).
const DEAD_STEP: usize = 0x8;
const DEAD_ALONG: usize = 0xC;
const DEAD_VIEW_POS: usize = 0x10;
const DEAD_VIEW_VEL: usize = 0x1C;
/// Under Slippi's FreezeDeadUpFallPhysics, the slide's vertical and depth speed (over the
/// approach's step and progress).
const DEAD_SLIDE_VEL_Y: usize = 0x8;
const DEAD_SLIDE_VEL_Z: usize = 0xC;

/// `fn_800D4DD4`, the screen KO's accessory, by address.
pub const DEAD_UP_FALL_ACCESSORY: u32 = 0x800D_4DD4;

/// `lbl_803B7500`: the fighter turned to face the screen.
const FACE_SCREEN: crate::quat::Quat = crate::quat::Quat {
    x: 0.0,
    y: std::f32::consts::PI,
    z: 0.0,
    w: 0.0,
};

/// The slot that stands for no one.
const NO_SLOT: i32 = 6;

/// `QuakeKind_Large`, the fighters' GObj list, and the wireframes' characters.
const QUAKE_LARGE: i32 = 4;
const P_LINK_FIGHTER: u8 = 8;
const CKIND_BOY: i32 = 0x1B;
const CKIND_GIRL: i32 = 0x1C;

/// The blast zone a fighter left by.
#[derive(Clone, Copy)]
enum Side {
    Left,
    Right,
    Down,
    Up,
}

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
    // Ports `pl_8004065C`; aliased once a check reaches it.
    // Ports `ftCo_800DD100`; aliased once a check reaches it.
    // Ports `pl_8003EA08`; aliased once a check reaches it.
    // Ports `pl_8003EA74`; aliased once a check reaches it.
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
        self.lb_stop_owned_sound_objects(gobj);
        let last = self.game.fighters[f.0].x2160;
        if last != -1 {
            self.lb_stop_owned_sound_object(gobj, last);
            self.game.fighters[f.0].x2160 = -1;
        }
        for t in TRACKS {
            let h = self.track_handle(f, t);
            if h != -1 && self.lb_stop_owned_sound_object(gobj, h) {
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
    #[doc(alias = "ftCo_DeadUpFall_Cam")]
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
            let slot = i32::from(self.game.fighter(gobj).player_idx);
            if let Some(nana) = self.game.player_entity_at(slot, 1) {
                self.remove_with_partner(nana, true);
            }
        }
        let fp = self.game.fighter(gobj);
        let (slot, sub) = (i32::from(fp.player_idx), fp.is_sub_fighter);
        self.fighter_out_of_play(slot, sub);
    }

    /// A fighter taken out of play with its partner, `ftCo_800D4F24` (Nana when Popo is
    /// out): unless asleep already, with `puff` the vanishing puff where it stands at its
    /// size (`ftData::x0->x168`), then it lets go of everything (`ftCo_800D331C`), takes
    /// the out-of-play flags and sleeps.
    // Ports `ftCo_800D4F24`; aliased once a check reaches it.
    pub fn remove_with_partner(&mut self, gobj: GObjId, puff: bool) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].is_sleeping {
            return;
        }
        if puff {
            let fp = &self.game.fighters[f.0];
            let ft_data = fp.ft_data.expect("a fighter without data");
            let attrs = self.data.ptr(ft_data).expect("ftData's attributes");
            let size = mem(fp::fmuls(
                reg(self.data.f32(attrs.at(0x168))),
                reg(fp.x34_scale.y),
            ));
            let pos = fp.cur_pos;
            super::super::super::ef::spawn(self, 0x43F, None, &[Arg::Vec(pos), Arg::Float(size)]);
        }
        self.leave_play(gobj);
        self.set_dead_flags(gobj);
        self.sleep(gobj);
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

    /// A KO past a blast zone, `ftCo_800D3680` (left), `ftCo_800D3950` (right),
    /// `ftCo_800D3BC8` (bottom) and `ftCo_800D3E40` (top, under stamina's or sudden
    /// death's rule): out of play (`enterDead`), its sound, and the KO's blast where it
    /// left, kept within the zones.
    // Ports `ftCo_800D3680`; aliased once a check reaches it.
    #[doc(alias = "ftCo_800D3950")]
    #[doc(alias = "ftCo_800D3BC8")]
    // Ports `ftCo_800D3E40`; aliased once a check reaches it.
    fn enter_dead_side(&mut self, gobj: GObjId, side: Side) {
        let (msid, sfx, angle, along_x) = match side {
            Side::Left => (ms::DEAD_LEFT, 0x88, -std::f32::consts::FRAC_PI_2, false),
            Side::Right => (ms::DEAD_RIGHT, 0x89, std::f32::consts::FRAC_PI_2, false),
            Side::Down => (ms::DEAD_DOWN, 0x61, 0.0, true),
            Side::Up => (ms::DEAD_UP, 0x61, std::f32::consts::PI, true),
        };
        self.enter_dead(gobj, msid);
        let f = self.game.fighter_id(gobj);
        self.play_sfx(gobj, sfx, 0x7F, 0x40);
        let id = self.fighter_sfx(f, sfx);
        self.lb_audio_queue(id);
        let mut pos = self.game.fighters[f.0].cur_pos;
        let g = &*self.game;
        let clamp = |v: &mut f32, lo: f32, hi: f32| {
            if *v > hi {
                *v = hi;
            }
            if *v < lo {
                *v = lo;
            }
        };
        if along_x {
            clamp(&mut pos.x, stage::blast_left(g), stage::blast_right(g));
        } else {
            clamp(&mut pos.y, stage::blast_bottom(g), stage::blast_top(g));
        }
        self.spawn_dead_effect(gobj, pos, angle);
        self.drop_coins(gobj);
    }

    pub fn enter_dead_right(&mut self, gobj: GObjId) {
        self.enter_dead_side(gobj, Side::Right);
    }
    pub fn enter_dead_left(&mut self, gobj: GObjId) {
        self.enter_dead_side(gobj, Side::Left);
    }
    pub fn enter_dead_down(&mut self, gobj: GObjId) {
        self.enter_dead_side(gobj, Side::Down);
    }
    pub fn enter_dead_up(&mut self, gobj: GObjId) {
        self.enter_dead_side(gobj, Side::Up);
    }

    /// Out of play past a zone (`enterDead`): what it lets go of, the state with its frames
    /// (`ftCommonData` x500), its flags, the last attacker noted, and the death itself
    /// (`processDeath`).
    fn enter_dead(&mut self, gobj: GObjId, msid: i32) {
        let f = self.game.fighter_id(gobj);
        self.leave_play(gobj);
        let frames = self.common().i32(0x500);
        self.game.fighters[f.0].set_mv_i32(DEAD_FRAMES, frames);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.set_dead_flags(gobj);
        let fp = &self.game.fighters[f.0];
        let (slot, sub) = (i32::from(fp.player_idx), fp.is_sub_fighter);
        self.record_ko_attacker(slot, sub);
        self.process_death(gobj);
    }

    /// A KO (`processDeath`): invisible, the screen's quake, the rumble (`ftCo_800D35FC`),
    /// the KO counted, its sounds stopped, then its two death cries (`ftCo_800D38B8`).
    #[doc(alias = "ftCo_800D35FC")]
    #[doc(alias = "ftCo_800D38B8")]
    fn process_death(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.invisible = true;
        fp.x221F_b1 = true;
        let pos = fp.cur_pos;
        super::super::super::camera::request_quake(self, QUAKE_LARGE, Some(pos));
        let c = self.common();
        let (mine, theirs) = (c.u32(0x4F8), c.u32(0x4FC));
        self.start_rumble(f, 5, mine);
        let mut cur = self.game.world.first_in(P_LINK_FIGHTER);
        while let Some(other) = cur {
            let o = self.game.fighter_id(other);
            if o != f && !self.game.fighters[o.0].is_sleeping {
                self.start_rumble(o, 6, theirs);
            }
            cur = self.game.world.gobjs[other.0].next;
        }
        self.count_ko(gobj);
        self.stop_fighter_sounds(gobj);
        for off in [0x4, 0x8] {
            let id = self.character_sfx(gobj, off);
            self.fighter_play_track(f, Track::Voice, id, 0x7F, 0x40);
            if !self.game.fighters[f.0].x2225_b6 {
                let id = self.fighter_sfx(f, id);
                self.lb_audio_queue(id);
            }
        }
    }

    /// The KO's blast at `pos` turned to `angle` (`spawnDeadEffect`): in the player's
    /// colors, the wireframes' own kind (`gm_801693BC`).
    #[doc(alias = "gm_801693BC")]
    fn spawn_dead_effect(&mut self, gobj: GObjId, pos: crate::vec::Vec3, angle: f32) {
        let slot = i32::from(self.game.fighter(gobj).player_idx);
        let i = self.game.player_unk45(slot) * 4;
        let rgb = |g: &Self, off: u32| {
            let t = g.common_ptr(off);
            let b = |k: u32| u32::from(g.data.u8(t.at(i + k)));
            (b(0) << 16) | (b(1) << 8) | b(2)
        };
        let (rgb1, rgb2) = (rgb(self, 0x48), rgb(self, 0x4C));
        if self.game.player_flags_bit1(slot) {
            unported_call("gm_801693BC for a slot that leaves");
        }
        let ckind = self.game.player_ckind(slot);
        let wireframe = (CKIND_BOY..=CKIND_GIRL).contains(&ckind);
        let kind = if wireframe { 0x42C } else { 0x42B };
        let scale = self.common().f32(0x4F4);
        super::super::super::ef::spawn(
            self,
            kind,
            Some(gobj),
            &[
                Arg::Vec(pos),
                Arg::Float(angle),
                Arg::Float(scale),
                Arg::Int(rgb1),
                Arg::Int(rgb2),
            ],
        );
    }

    /// `ftCo_DeadLeft_Anim` and the other zones' deaths: when the frames run out, out of
    /// play for now.
    // Ports `ftCo_DeadLeft_Anim`; aliased once a check reaches it.
    #[doc(alias = "ftCo_DeadRight_Anim")]
    #[doc(alias = "ftCo_DeadDown_Anim")]
    // Ports `ftCo_DeadUp_Anim`; aliased once a check reaches it.
    pub fn co_dead_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(DEAD_FRAMES, fp.mv_i32(DEAD_FRAMES).wrapping_sub(1));
        if fp.mv_i32(DEAD_FRAMES) == 0 {
            self.out_of_play(gobj);
        }
    }

    /// `ftCo_DeadLeft_Cam` and the other zones' deaths: nothing.
    // Ports `ftCo_DeadLeft_Cam`; aliased once a check reaches it.
    #[doc(alias = "ftCo_DeadRight_Cam")]
    #[doc(alias = "ftCo_DeadDown_Cam")]
    // Ports `ftCo_DeadUp_Cam`; aliased once a check reaches it.
    pub fn co_dead_cam(&mut self, _gobj: GObjId) {}
    /// The screen KO, `ftCo_800D4780`: the fighter falls toward the camera.
    #[doc(alias = "ftCo_800D4780")]
    pub fn enter_dead_up_fall(&mut self, gobj: GObjId) {
        self.enter_dead_up_screen(gobj, ms::DEAD_UP_FALL);
        self.game.fighter_mut(gobj).set_mv_i32(DEAD_FROZEN, 0);
    }

    /// A vector of the death states' variables, `off` bytes into `mv`.
    fn dead_vec(&self, gobj: GObjId, off: usize) -> crate::vec::Vec3 {
        let fp = self.game.fighter(gobj);
        crate::vec::Vec3::new(fp.mv_f32(off), fp.mv_f32(off + 4), fp.mv_f32(off + 8))
    }

    fn set_dead_vec(&mut self, gobj: GObjId, off: usize, v: crate::vec::Vec3) {
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_f32(off, reg(v.x));
        fp.set_mv_f32(off + 4, reg(v.y));
        fp.set_mv_f32(off + 8, reg(v.z));
    }

    /// The fighter turned to face the screen, unless its depth is scaled.
    fn face_screen(&mut self, gobj: GObjId) {
        let root = self.root_jobj(gobj);
        if self.game.fighter(gobj).x34_scale.z == 1.0 {
            crate::hsd::jobj::set_rotation(&mut self.game.objs.jobjs, root, FACE_SCREEN);
        }
    }

    /// The screen KO's start, `ftCo_800D4580`: out of play, the fighter placed in the game
    /// camera's view at `ftCommonData`'s start (the display pass puts it in the world each
    /// frame, `x2220_b7`), facing the screen, with the KO's flash.
    #[doc(alias = "ftCo_800D4580")]
    fn enter_dead_up_screen(&mut self, gobj: GObjId, msid: i32) {
        let c = self.common();
        let frames = c.i32(0x524);
        let start = crate::vec::Vec3::new(c.f32(0x538), c.f32(0x53C), c.f32(0x540));
        self.leave_play(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(DEAD_FRAMES, frames);
        fp.set_mv_i32(DEAD_PHASE, 0);
        self.set_dead_vec(gobj, DEAD_VIEW_POS, start);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_f32(DEAD_VIEW_VEL + 8, 0.0);
        fp.set_mv_f32(DEAD_VIEW_VEL + 4, 0.0);
        fp.set_mv_f32(DEAD_VIEW_VEL, 0.0);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).x2220_b7 = true;
        self.face_screen(gobj);
        self.set_dead_flags(gobj);
        self.stop_fighter_sounds(gobj);
        // `ftCommon_8007EFC0`.
        self.game.fighter_mut(gobj).x209A = 1;
        self.start_color_anim(gobj, 0x2B, 0);
        let fp = self.game.fighter(gobj);
        let (slot, sub) = (i32::from(fp.player_idx), fp.is_sub_fighter);
        self.record_ko_attacker(slot, sub);
    }

    /// The fighter hits the camera, `ftCo_800D481C`: the hit's state, facing the screen
    /// again, a rumble for it and one for everyone else awake, a quake, a scream and the
    /// accessory that stops it below the camera.
    #[doc(alias = "ftCo_800D481C")]
    fn dead_up_hit_camera(&mut self, gobj: GObjId, msid: i32) {
        use super::super::motion::flags::{SKIP_COL_ANIM, SKIP_RUMBLE};
        self.change_motion_state(gobj, msid, SKIP_RUMBLE | SKIP_COL_ANIM, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).x2220_b7 = true;
        self.face_screen(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.x2219_b1 = true;
        fp.x221E_b1 = true;
        fp.x221E_b2 = true;
        let c = self.common();
        let (mine, others) = (c.u32(0x4F8), c.u32(0x4FC));
        let f = self.game.fighter_id(gobj);
        self.start_rumble(f, 5, mine);
        let mut next = self.game.world.first_in(P_LINK_FIGHTER);
        while let Some(o) = next {
            next = self.game.world.gobj(o).next;
            let of = self.game.fighter_id(o);
            if of != f && !self.game.fighters[of.0].is_sleeping {
                self.start_rumble(of, 6, others);
            }
        }
        let pos = self.game.fighters[f.0].cur_pos;
        super::super::super::camera::request_quake(self, QUAKE_LARGE, Some(pos));
        // `ft_800889F4` with the character's camera-hit screams (`FtSFX::x20`).
        let ft_data = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without ftData");
        let sfx = self
            .data
            .ptr(ft_data.at(0x4C))
            .expect("ftData without sounds");
        if let Some(arr) = self.data.ptr(sfx.at(0x20)) {
            let n = self.data.u32(arr) as i32;
            let ids = self.data.ptr(arr.at(4)).expect("a voice set's ids");
            self.fighter_random_voice_in(f, ids, n);
        }
        self.drop_coins(gobj);
        self.game.fighters[f.0].accessory4_cb = Some(super::super::super::handles::CodeRef(
            DEAD_UP_FALL_ACCESSORY,
        ));
    }

    /// `ftCo_DeadUpFall_Anim`: the screen KO's phases. After its wait the fighter comes
    /// toward the camera; it hits it; it slides down it; out of sight below, the KO counts,
    /// with its sound, rumble and quake; then it's out of play.
    #[doc(alias = "ftCo_DeadUpFall_Anim")]
    pub fn co_dead_up_fall_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        if fp.mv_i32(DEAD_PHASE) == 1 {
            let along = fp::fadds(reg(fp.mv_f32(DEAD_ALONG)), reg(fp.mv_f32(DEAD_STEP)));
            fp.set_mv_f32(DEAD_ALONG, along);
            if fp.mv_i32(DEAD_FROZEN) != 0 {
                unported_call("ftCo_DeadUpFall_Anim frozen (its spin)");
            }
        }
        let fp = &mut self.game.fighters[f.0];
        if fp.mv_i32(DEAD_FRAMES) != 0 {
            fp.set_mv_i32(DEAD_FRAMES, fp.mv_i32(DEAD_FRAMES).wrapping_sub(1));
        }
        if fp.mv_i32(DEAD_FRAMES) != 0 {
            return;
        }
        let c = self.common();
        let fp = &mut self.game.fighters[f.0];
        match fp.mv_i32(DEAD_PHASE) {
            0 => {
                let n = c.i32(0x528);
                let step = fp::fdivs(1.0, fp::frsp(f64::from(n)));
                fp.set_mv_f32(DEAD_STEP, step);
                fp.set_mv_f32(DEAD_ALONG, reg(fp.mv_f32(DEAD_STEP)));
                fp.set_mv_i32(DEAD_FRAMES, n);
                fp.set_mv_i32(DEAD_PHASE, 1);
            }
            1 => {
                if fp.mv_i32(DEAD_FROZEN) != 0 {
                    self.ice_effect_again(gobj);
                }
                let fp = &self.game.fighters[f.0];
                let msid = if fp.x34_scale.z != 1.0 {
                    ms::DEAD_UP_FALL_HIT_CAMERA_FLAT
                } else {
                    ms::DEAD_UP_FALL_HIT_CAMERA
                };
                self.dead_up_hit_camera(gobj, msid);
                let frames = c.i32(0x52C);
                let fp = &mut self.game.fighters[f.0];
                fp.set_mv_i32(DEAD_FRAMES, frames);
                fp.set_mv_i32(DEAD_PHASE, 2);
            }
            2 => {
                fp.self_vel.y = c.f32(0x550);
                fp.self_vel.z = c.f32(0x55C);
                fp.set_mv_i32(DEAD_FRAMES, c.i32(0x530));
                fp.set_mv_i32(DEAD_PHASE, 3);
                if self.game.rules.slippi {
                    // Slippi's FreezeDeadUpFallPhysics (`800D4C1C`): the slide's speed is
                    // kept in the state's variables, not the fighter's.
                    let fp = &mut self.game.fighters[f.0];
                    fp.set_mv_f32(DEAD_SLIDE_VEL_Y, reg(c.f32(0x550)));
                    fp.set_mv_f32(DEAD_SLIDE_VEL_Z, reg(c.f32(0x55C)));
                    fp.self_vel = crate::vec::Vec3::new(0.0, 0.0, 0.0);
                }
            }
            3 => {
                self.stop_dead(gobj);
                if self.game.fighters[f.0].mv_i32(DEAD_FROZEN) != 0 {
                    self.destroy_effects(gobj);
                }
                let fp = &mut self.game.fighters[f.0];
                fp.x221F_b1 = true;
                fp.invisible = true;
                self.count_ko(gobj);
                self.stop_fighter_sounds(gobj);
                self.play_sfx(gobj, 0x61, 0x7F, 0x40);
                // `ft_8008805C`.
                let id = self.fighter_sfx(f, 0x61);
                self.lb_audio_queue(id);
                self.start_rumble(f, 0xD, 0);
                let pos = self.game.fighters[f.0].cur_pos;
                super::super::super::camera::request_quake(self, QUAKE_LARGE, Some(pos));
                let fp = &mut self.game.fighters[f.0];
                fp.set_mv_i32(DEAD_FRAMES, c.i32(0x534));
                fp.set_mv_i32(DEAD_PHASE, 4);
            }
            4 => self.out_of_play(gobj),
            _ => {}
        }
    }

    /// `ftCo_DeadUpFall_Phys`: the approach moves the fighter's place in the view from
    /// `ftCommonData`'s start to its end; sliding down, its own gravity moves it (unless
    /// the animation's rotation held it still this frame, `ftAnim_80070FD0`).
    #[doc(alias = "ftCo_DeadUpFall_Phys")]
    pub fn co_dead_up_fall_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let c = self.common();
        match self.game.fighters[f.0].mv_i32(DEAD_PHASE) {
            1 => {
                if self.game.fighters[f.0].x2222_b6 && !self.x100_rot_changed(f) {
                    return;
                }
                let a = crate::vec::Vec3::new(c.f32(0x538), c.f32(0x53C), c.f32(0x540));
                let b = crate::vec::Vec3::new(c.f32(0x544), c.f32(0x548), c.f32(0x54C));
                let t = self.game.fighters[f.0].mv_f32(DEAD_ALONG);
                self.set_dead_vec(gobj, DEAD_VIEW_POS, a.lerp(b, t));
            }
            3 => {
                let (g, term) = (reg(c.f32(0x554)), reg(c.f32(0x558)));
                let v = if self.game.rules.slippi {
                    // Slippi's FreezeDeadUpFallPhysics (`800D4D68`): the fall acts on the
                    // speed the state keeps, without `ftCommon_Fall`.
                    let fp = &mut self.game.fighters[f.0];
                    let mut vy = fp::fsub(reg(fp.mv_f32(DEAD_SLIDE_VEL_Y)), g);
                    let floor = fp::fneg(term);
                    // `bgt`: a NaN takes the floor too.
                    #[allow(clippy::neg_cmp_op_on_partial_ord)]
                    if !(vy > floor) {
                        vy = floor;
                    }
                    fp.set_mv_f32(DEAD_SLIDE_VEL_Y, vy);
                    let y = fp::fadds(
                        reg(fp.mv_f32(DEAD_VIEW_VEL + 4)),
                        reg(fp.mv_f32(DEAD_SLIDE_VEL_Y)),
                    );
                    fp.set_mv_f32(DEAD_VIEW_VEL + 4, y);
                    let z = fp::fadds(
                        reg(fp.mv_f32(DEAD_SLIDE_VEL_Z)),
                        reg(fp.mv_f32(DEAD_VIEW_VEL + 8)),
                    );
                    fp.set_mv_f32(DEAD_VIEW_VEL + 8, z);
                    self.dead_vec(gobj, DEAD_VIEW_VEL)
                } else {
                    Self::fall(&mut self.game.fighters[f.0], g, term);
                    let vel = self.game.fighters[f.0].self_vel;
                    let v = self.dead_vec(gobj, DEAD_VIEW_VEL) + vel;
                    self.set_dead_vec(gobj, DEAD_VIEW_VEL, v);
                    v
                };
                if self.game.fighters[f.0].x2222_b6 && !self.x100_rot_changed(f) {
                    return;
                }
                let p = self.dead_vec(gobj, DEAD_VIEW_POS) + v;
                self.set_dead_vec(gobj, DEAD_VIEW_POS, p);
                let fp = &mut self.game.fighters[f.0];
                fp.set_mv_f32(DEAD_VIEW_VEL + 8, 0.0);
                fp.set_mv_f32(DEAD_VIEW_VEL + 4, 0.0);
                fp.set_mv_f32(DEAD_VIEW_VEL, 0.0);
            }
            _ => {}
        }
    }

    /// `fn_800D4DD4`: sliding down the camera, the fighter stops once below its bounds.
    #[doc(alias = "fn_800D4DD4")]
    pub fn dead_up_fall_below_camera(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).mv_i32(DEAD_PHASE) == 3 {
            let bottom = stage::cam_bottom(self.game);
            if self.game.fighter(gobj).cur_pos.y < bottom {
                self.stop_dead(gobj);
            }
        }
    }

    /// Where the display pass puts a fighter positioned by the camera (`x2220_b7`,
    /// `ftDrawCommon_80080E18`'s inline): its place in the game camera's view (`x50`) through
    /// the camera's inverse view matrix (`Camera_800310B8`, rebuilt), as its position and its
    /// root joint's.
    pub fn place_by_camera(&mut self, gobj: GObjId) {
        let c = super::super::super::camera::cobj_copy_view(self.game, self.data);
        let m = crate::hsd::cobj::inv_viewing_mtx(&mut self.game.objs, self.data, c)
            .expect("the camera's inverse view matrix");
        let pos = m.transform(self.dead_vec(gobj, DEAD_VIEW_POS));
        // Slippi's FreezeDeadUpFallPhysics (`80080E80`) places only the model, leaving the
        // fighter's position where it was.
        if !self.game.rules.slippi {
            self.game.fighter_mut(gobj).cur_pos = pos;
        }
        let j = self.root_jobj(gobj);
        crate::hsd::jobj::set_translate(&mut self.game.objs.jobjs, j, pos);
    }
    pub fn enter_dead_up_fall_ice(&mut self, _gobj: GObjId) {
        unported_call("ftCo_800D47B8");
    }
    pub fn enter_dead_up_star_ice(&mut self, _gobj: GObjId) {
        unported_call("ftCo_800D41C4");
    }
}
