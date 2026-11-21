//! The hands' waits (`ftmasterhandwait10.c`, `ftmasterhandwait12.c`, `ftcrazyhandwait10.c`,
//! `ftcrazyhandtagcancel.c`): the entrance's start, hovering in place, choosing the next attack
//! (from the hand's table at random, or a human's buttons), gliding to where an attack starts
//! and back to his place. Master Hand's own: his attacks' starts and a human's buttons on
//! port 3.

use gekko_fp as fp;

use super::super::super::handles::{CodeRef, FighterId};
use super::super::super::state::Running;
use super::{HAND, Hand, addr, attr, ms, mv, u};
use crate::fpu::{mem, reg};
use crate::game::ft::FighterKind;
use crate::game::gr::StKind;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `ftMh_Init_803D40D0`: the attacks he chooses from (`states`), the table's rows as runs of
/// them (`x48`: the first and how many) and each row's next rows (`x54`, five a row).
pub(super) const ATTACKS: [i32; 14] = [
    ms::SLAP,
    ms::SWEEP_LOOP,
    ms::WALK_LOOP,
    ms::ROCK_CRUSH_UP,
    ms::PAPER_CRUSH,
    ms::POKE1,
    ms::DRILL,
    ms::BACK_PUNCH,
    ms::BACK_CRUSH,
    ms::BACK_AIRPLANE1,
    ms::FINGER_BEAM_LOOP,
    ms::FINGER_GUN2,
    ms::THROW,
    ms::SLAM,
];
pub(super) const ROWS: [(u8, u8); 6] = [(0, 3), (3, 4), (7, 3), (10, 2), (12, 2), (0, 0)];
pub(super) const NEXT_ROWS: [u8; 30] = [
    1, 2, 3, 4, 0, 0, 2, 3, 4, 0, 0, 1, 3, 4, 0, 0, 1, 2, 4, 0, 0, 1, 2, 3, 0, 0, 0, 0, 0, 0,
];

/// `HSD_PAD_*` of the master status: the buttons a human hand's attacks take.
pub(crate) const PAD_DPAD_LEFT: u32 = 0x1;
pub(crate) const PAD_DPAD_RIGHT: u32 = 0x2;
pub(crate) const PAD_DPAD_DOWN: u32 = 0x4;
pub(crate) const PAD_DPAD_UP: u32 = 0x8;
pub(crate) const PAD_Z: u32 = 0x10;
pub(crate) const PAD_R: u32 = 0x20;
pub(crate) const PAD_L: u32 = 0x40;
pub(crate) const PAD_A: u32 = 0x100;
pub(crate) const PAD_B: u32 = 0x200;
pub(crate) const PAD_Y: u32 = 0x800;

impl Running<'_> {
    /// His start in a match, `ftMh_MS_341_8014FE10` and `ftCh_Init_80155FCC`: the entrance, or
    /// waiting where he is in Event 50.
    #[doc(alias = "ftMh_MS_341_8014FE10")]
    #[doc(alias = "ftCh_Init_80155FCC")]
    pub fn hand_start(&mut self, gobj: GObjId, h: &Hand) {
        if super::super::super::gr::stage::stkind(self.game) == StKind::Event50 {
            self.hand_start_in_place(gobj, h);
        } else {
            self.hand_entry_enter(gobj, h);
        }
    }

    /// Back to his place for a wait (`u.x2240_pos` kept while waiting).
    fn hand_wait_pos(&mut self, gobj: GObjId, h: &Hand) {
        let fp = self.game.fighter_mut(gobj);
        if fp.motion_id == h.wait1 || fp.motion_id == h.wait2 {
            fp.cur_pos = u_vec(fp, u::WAIT_POS);
        } else {
            let p = fp.cur_pos;
            set_u_vec(fp, u::WAIT_POS, p);
        }
    }

    /// Enters wait `to` at his place, keeping the frame from its moving form `from`.
    fn hand_change_wait(&mut self, gobj: GObjId, h: &Hand, to: i32, from: i32) {
        self.hand_wait_pos(gobj, h);
        let fp = self.game.fighter(gobj);
        let frame = if fp.u_i32(u::WAIT) == from {
            fp.cur_anim_frame
        } else {
            0.0
        };
        self.change_motion_state(gobj, to, 0, frame, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).set_u_i32(u::WAIT, to);
    }

    /// Hovers (`Wait1_0`), from the wait he was in, `ftMh_MS_341_8014FE5C` and
    /// `ftCh_Init_80156018`.
    #[doc(alias = "ftMh_MS_341_8014FE5C")]
    #[doc(alias = "ftCh_Init_80156018")]
    pub fn hand_enter_wait1(&mut self, gobj: GObjId, h: &Hand) {
        self.hand_change_wait(gobj, h, h.wait1, h.glide1);
    }

    /// Hovers (`Wait2_0`), `ftCh_Init_801560D8`; Master Hand's `ftMh_MS_341_8014FF1C`, which
    /// the original runs only as [`Self::hand_enter_wait`]'s inline copy (its own call is
    /// dead, ledger/dead.txt).
    #[doc(alias = "ftCh_Init_801560D8")]
    pub fn hand_enter_wait2(&mut self, gobj: GObjId, h: &Hand) {
        self.hand_change_wait(gobj, h, h.wait2, h.glide2);
    }

    /// Hovers in the wait his last glide matches, `ftMh_MS_341_8014FFDC` and
    /// `ftCh_Init_80156198` (Crazy Hand's clears the cry's flag first).
    #[doc(alias = "ftMh_MS_341_8014FFDC")]
    #[doc(alias = "ftCh_Init_80156198")]
    pub fn hand_enter_wait(&mut self, gobj: GObjId, h: &Hand) {
        let fp = self.game.fighter_mut(gobj);
        if h.cries_waiting {
            fp.cmd_vars[1] = 0;
        }
        if fp.u_i32(u::WAIT) == h.glide2 {
            self.hand_enter_wait2(gobj, h);
        } else {
            self.hand_enter_wait1(gobj, h);
        }
    }

    /// Event 50's start, `ifStage251` and `ftCh_Init_80156310`: at his place, waiting, his
    /// state set to the entrance first so the wait takes his place.
    #[doc(alias = "ifStage251")]
    #[doc(alias = "ftCh_Init_80156310")]
    pub fn hand_start_in_place(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        let pos = h.point(self, f, h.home);
        let fp = &mut self.game.fighters[f.0];
        if h.cries_waiting {
            fp.cmd_vars[1] = 0;
        }
        fp.set_u_i32(u::WAIT, h.wait1);
        fp.motion_id = h.entry;
        fp.cur_pos.x = pos.x;
        fp.cur_pos.y = pos.y;
        self.hand_enter_wait1(gobj, h);
    }

    /// Hovering, `ftMh_Wait1_0_Anim` and `ftCh_Wait1_0_Anim`: a human's buttons choose the next
    /// attack; a computer's timer runs out and draws one from the table (Master Hand joins
    /// Crazy Hand's tag attack instead when Crazy Hand waits at one).
    #[doc(alias = "ftMh_Wait1_0_Anim")]
    #[doc(alias = "ftCh_Wait1_0_Anim")]
    // NaN takes the original's branch (`--x223C < 0` fails).
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    pub fn hand_wait_anim(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        let slot = i32::from(self.game.fighters[f.0].player_idx);
        let kind = self.game.player_pkind(slot);
        if h.cries_waiting && self.game.fighters[f.0].cmd_vars[1] != 0 {
            self.boss_cry(gobj);
            self.game.fighters[f.0].cmd_vars[1] = 0;
        }
        if kind == 0 {
            (h.human_attack)(self, gobj);
            self.hand_wait_loop(gobj, h);
            return;
        }
        let fp = &mut self.game.fighters[f.0];
        let t = mem(fp::fsubs(reg(fp.u_f32(u::TIMER)), 1.0));
        fp.set_u_f32(u::TIMER, t);
        if !(reg(t) < 0.0) {
            self.hand_wait_loop(gobj, h);
            return;
        }
        self.hand_reset_timer(gobj, h);
        if h.tags_from_wait
            && self.boss_motion_id(FighterKind::CrazyHand.id())
                == super::super::crazyhand::ms::TAG_ROCK_PAPER
        {
            self.mh_tag_attack(gobj);
            return;
        }
        let row = if self.game.fighters[f.0].u_i32(u::SINCE_WALK) == h.i32(self, f, h.walk_every) {
            self.game.fighters[f.0].set_u_i32(u::SINCE_WALK, 0);
            2
        } else {
            let n = if h.fewer_rows_alone && self.boss_hand_beaten(Hand::of(h.partner)) {
                4
            } else {
                5
            };
            let r = self.game.objs.rng.below(n) as u32;
            let fp = &mut self.game.fighters[f.0];
            let i = (fp.u_i32(u::ROW) as u32).wrapping_mul(5).wrapping_add(r);
            let next = i32::from(h.next_rows[i as usize]);
            if next == 2 {
                fp.set_u_i32(u::SINCE_WALK, 0);
            } else {
                let n = fp.u_i32(u::SINCE_WALK).wrapping_add(1);
                fp.set_u_i32(u::SINCE_WALK, n);
            }
            next
        };
        let (first, count) = h.rows[row as usize];
        let pick = i32::from(first) + self.game.objs.rng.below(i32::from(count));
        let mut attack = h.attacks[pick as usize];
        let fp = &mut self.game.fighters[f.0];
        fp.set_u_i32(u::ROW, row);
        fp.set_u_i32(u::NEXT_ATTACK, attack);
        if fp.input_disabled {
            attack = h.wait1;
        }
        (h.start_attack)(self, gobj, attack);
    }

    /// The wait's animation looping when it ends: the first wait into the second, the second
    /// into the one his last glide matches.
    fn hand_wait_loop(&mut self, gobj: GObjId, h: &Hand) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let m = self.game.fighter(gobj).motion_id;
        if m == h.wait1 {
            self.hand_enter_wait2(gobj, h);
        } else if m == h.wait2 {
            self.hand_enter_wait(gobj, h);
        }
    }

    /// The next attack's timer from his computer level (`ftBossLib_8015BD24` with his base,
    /// high and low).
    pub(crate) fn hand_reset_timer(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let (level, speed) = (fp.cpu.level, fp.u_f32(u::TIMER_SPEED));
        let [base, hi, lo] = h.timer.map(|off| h.i32(self, f, off));
        let t = self.boss_attack_timer(level, speed, base, hi, lo);
        self.game.fighters[f.0].set_u_f32(u::TIMER, t);
    }

    /// Starts the attack `attack` drew (the table's states stand for the attacks): most glide
    /// to their start first.
    pub(crate) fn mh_start_attack(&mut self, gobj: GObjId, attack: i32) {
        let f = self.game.fighter_id(gobj);
        let da = |r: &Self, off| HAND.point(r, f, off);
        match attack {
            ms::SWEEP_LOOP => {
                let mut v = self.floor_right0();
                v.y = HAND.f32(self, f, attr::X38);
                self.hand_glide_to(gobj, &HAND, addr::WAIT_SWEEP_ENTER, v);
            }
            ms::SLAP => {
                let mut v = self.floor_right0();
                v.y = HAND.f32(self, f, attr::X50 + 4);
                self.hand_glide_to(gobj, &HAND, addr::SLAP_ENTER, v);
            }
            ms::WALK_LOOP => {
                let v = da(self, attr::X40);
                self.hand_glide_to(gobj, &HAND, addr::WALK2_ENTER, v);
            }
            ms::DRILL => {
                let v = da(self, attr::X88);
                self.hand_glide_to(gobj, &HAND, addr::COUNTDOWN_ENTER, v);
            }
            ms::ROCK_CRUSH_UP => {
                let v = da(self, attr::XA8);
                self.hand_glide_to(gobj, &HAND, addr::ROCK_CRUSH_UP_ENTER, v);
            }
            ms::PAPER_CRUSH => {
                let v = da(self, attr::XBC);
                self.hand_glide_to(gobj, &HAND, addr::WINDOW_ENTER, v);
            }
            ms::POKE1 => self.mh_dispatch(gobj, addr::CHASES_ENTER),
            ms::BACK_AIRPLANE1 => self.mh_dispatch(gobj, addr::BACK_AIRPLANE1_ENTER),
            ms::BACK_PUNCH => self.mh_back_attack(gobj, addr::RAM_ENTER),
            ms::BACK_CRUSH => self.mh_back_attack(gobj, addr::SHRINK_ENTER),
            ms::FINGER_BEAM_LOOP => {
                let v = self.hand_point_around(f, &HAND, attr::XC4, attr::XCC);
                self.hand_glide_to(gobj, &HAND, addr::BEAMS_WINDUP_ENTER, v);
            }
            ms::FINGER_GUN2 => self.mh_dispatch(gobj, addr::GUN_ENTER),
            ms::THROW | ms::SLAM => self.mh_dispatch(gobj, addr::REACH_ENTER),
            _ => {}
        }
    }

    /// A point at random about the point `center` of his attributes, up to the point
    /// `spread` either way: where Master Hand's finger beams and Crazy Hand's poke start.
    pub(crate) fn hand_point_around(
        &mut self,
        f: FighterId,
        h: &Hand,
        center: usize,
        spread: usize,
    ) -> Vec3 {
        let (c, d) = (h.point(self, f, center), h.point(self, f, spread));
        let rx = reg(self.game.objs.rng.draw_f32());
        let x = mem(fp::fsubs(
            fp::fmadds(2.0, fp::fmuls(reg(d.x), rx), reg(c.x)),
            reg(d.x),
        ));
        let ry = reg(self.game.objs.rng.draw_f32());
        let y = mem(fp::fsubs(
            fp::fmadds(2.0, fp::fmuls(reg(d.y), ry), reg(c.y)),
            reg(d.y),
        ));
        Vec3::new(x, y, 0.0)
    }

    /// The right end of the stage's first floor (`mpFloorGetRight(0)`).
    pub(crate) fn floor_right0(&self) -> Vec3 {
        let g = self.game.geometry.as_ref().expect("no collision geometry");
        g.floor_right(0)
    }

    /// Crazy Hand's tag attack's part for him, from Crazy Hand's next attack.
    fn mh_tag_attack(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        use super::super::crazyhand::ms as ch_ms;
        let (cb, off) = match self.boss_ch_next() {
            ch_ms::SQUEEZING1 => (addr::TAG_CRUSH_ENTER, attr::X124),
            ch_ms::SQUEEZE => (addr::TAG_APPLAUD_ENTER, attr::X12C),
            ch_ms::THROW => (addr::TAG_ROCK_PAPER_ENTER, attr::X134),
            // The original sets the point for `Slam` and starts nothing.
            _ => return,
        };
        let v = HAND.point(self, f, off);
        self.hand_glide_to(gobj, &HAND, cb, v);
    }

    /// A human's attack (port 3's master status, a button and a direction),
    /// `ftMh_MS_341_80150894`.
    #[doc(alias = "ftMh_MS_341_80150894")]
    pub fn mh_human_attack(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let b = self.game.pads.master[2].button;
        let on = |m: u32| b & m != 0;
        let da = |r: &Self, off| HAND.point(r, f, off);
        let l = on(PAD_L);
        if l && on(PAD_DPAD_UP) {
            let mut v = self.floor_right0();
            v.y = HAND.f32(self, f, attr::X38);
            self.hand_glide_to(gobj, &HAND, addr::WAIT_SWEEP_ENTER, v);
        } else if l && on(PAD_DPAD_RIGHT) {
            let mut v = self.floor_right0();
            v.y = HAND.f32(self, f, attr::X50 + 4);
            self.hand_glide_to(gobj, &HAND, addr::SLAP_ENTER, v);
        } else if l && on(PAD_DPAD_DOWN) {
            let v = da(self, attr::X40);
            self.hand_glide_to(gobj, &HAND, addr::WALK2_ENTER, v);
        } else if on(PAD_R) && on(PAD_DPAD_UP) {
            let v = da(self, attr::X88);
            self.hand_glide_to(gobj, &HAND, addr::COUNTDOWN_ENTER, v);
        } else if on(PAD_R) && on(PAD_DPAD_RIGHT) {
            let v = da(self, attr::XA8);
            self.hand_glide_to(gobj, &HAND, addr::ROCK_CRUSH_UP_ENTER, v);
        } else if on(PAD_R) && on(PAD_DPAD_DOWN) {
            let v = da(self, attr::XBC);
            self.hand_glide_to(gobj, &HAND, addr::WINDOW_ENTER, v);
        } else if on(PAD_R) && on(PAD_DPAD_LEFT) {
            self.mh_dispatch(gobj, addr::CHASES_ENTER);
        } else if on(PAD_A) && on(PAD_DPAD_UP) {
            self.mh_dispatch(gobj, addr::BACK_AIRPLANE1_ENTER);
        } else if on(PAD_A) && on(PAD_DPAD_RIGHT) {
            self.mh_back_attack(gobj, addr::RAM_ENTER);
        } else if on(PAD_A) && on(PAD_DPAD_DOWN) {
            self.mh_back_attack(gobj, addr::SHRINK_ENTER);
        } else if on(PAD_B) && on(PAD_DPAD_UP) {
            let v = self.hand_point_around(f, &HAND, attr::XC4, attr::XCC);
            self.hand_glide_to(gobj, &HAND, addr::BEAMS_WINDUP_ENTER, v);
        } else if on(PAD_B) && on(PAD_DPAD_RIGHT) {
            self.mh_dispatch(gobj, addr::GUN_ENTER);
        } else if on(PAD_Z) && on(PAD_DPAD_UP) {
            self.game.fighters[f.0].set_u_i32(u::NEXT_ATTACK, ms::THROW);
            self.mh_dispatch(gobj, addr::REACH_ENTER);
        } else if on(PAD_Z) && on(PAD_DPAD_RIGHT) {
            self.game.fighters[f.0].set_u_i32(u::NEXT_ATTACK, ms::SLAM);
            self.mh_dispatch(gobj, addr::REACH_ENTER);
        } else if on(PAD_Y) && on(PAD_DPAD_UP) {
            self.mh_tag_attack(gobj);
        }
    }

    /// Glides in the first wait's moving form, `ftMh_MS_389_80150C8C` and
    /// `ftCh_GrabUnk1_8015B8FC`.
    #[doc(alias = "ftMh_MS_389_80150C8C")]
    #[doc(alias = "ftCh_GrabUnk1_8015B8FC")]
    pub fn hand_enter_glide1(&mut self, gobj: GObjId, h: &Hand) {
        self.hand_enter_glide(gobj, h.glide1, h.wait1);
    }

    /// Glides in the second's, `ftMh_MS_389_80150D28` and `ftCh_GrabUnk1_8015B998`.
    #[doc(alias = "ftMh_MS_389_80150D28")]
    #[doc(alias = "ftCh_GrabUnk1_8015B998")]
    pub fn hand_enter_glide2(&mut self, gobj: GObjId, h: &Hand) {
        self.hand_enter_glide(gobj, h.glide2, h.wait2);
    }

    /// Enters glide `to`, keeping the frame from `to` or its still form `from`.
    fn hand_enter_glide(&mut self, gobj: GObjId, to: i32, from: i32) {
        let w = self.game.fighter(gobj).u_i32(u::WAIT);
        if w == from || w == to {
            let frame = self.game.fighter(gobj).cur_anim_frame;
            self.change_motion_state(gobj, to, 0, frame, 1.0, 0.0, None);
        } else {
            self.change_motion_state(gobj, to, 0, 0.0, 1.0, 0.0, None);
            self.anim_update(gobj);
        }
        self.game.fighter_mut(gobj).set_u_i32(u::WAIT, to);
    }

    /// Glides to `pos`, then runs `cb`, `ftMh_MS_389_80150DC4` and `ftCh_GrabUnk1_8015BA34`.
    #[doc(alias = "ftMh_MS_389_80150DC4")]
    #[doc(alias = "ftCh_GrabUnk1_8015BA34")]
    pub fn hand_glide_to(&mut self, gobj: GObjId, h: &Hand, cb: u32, pos: Vec3) {
        if self.game.fighter(gobj).u_i32(u::WAIT) == h.wait2 {
            self.hand_enter_glide2(gobj, h);
        } else {
            self.hand_enter_glide1(gobj, h);
        }
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(mv::X4, cb as i32);
        set_mv_vec(fp, mv::TARGET, pos);
    }

    /// `ftMh_Wait1_2_Anim` and `ftCh_TagCancel_Anim`: the first glide's animation again when it
    /// ends (from either glide).
    #[doc(alias = "ftMh_Wait1_2_Anim")]
    #[doc(alias = "ftCh_TagCancel_Anim")]
    pub fn hand_glide_anim(&mut self, gobj: GObjId, h: &Hand) {
        if !self.anim_frames_remaining(gobj) {
            self.game.fighter_mut(gobj).set_u_i32(u::WAIT, h.glide1);
            self.hand_enter(gobj, h.glide1);
        }
    }

    /// The animation's motion and the glide toward `mv.xC` (`ftMh_UpdateBossMotion`,
    /// `ftCh_UpdateBossMotion`: `ft_80085134` and `ftBossLib_8015BE40`): the glides', the
    /// grab's and Crazy Hand's slide's physics.
    #[doc(alias = "ftMh_Wait1_2_Phys")]
    #[doc(alias = "ftMh_Fail_Phys")]
    #[doc(alias = "ftMh_Cancel_Phys")]
    #[doc(alias = "ftMh_Squeezing_Phys")]
    #[doc(alias = "ftMh_Squeeze_Phys")]
    #[doc(alias = "ftCh_BackPunch_Phys")]
    #[doc(alias = "ftCh_FingerGun1_Phys")]
    #[doc(alias = "ftCh_BackAirplane1_Phys")]
    #[doc(alias = "ftCh_FingerGun2_Phys")]
    #[doc(alias = "ftCh_TagCancel_Phys")]
    #[doc(alias = "ftCh_Damage2_Phys")]
    pub fn hand_glide_motion(&mut self, gobj: GObjId, h: &Hand) {
        self.air_anim_motion(gobj);
        self.hand_glide_step(gobj, h);
    }

    /// The glide alone (`ftBossLib_8015BE40` toward `mv.xC`), the distance left into `mv.x18`:
    /// `ftMh_SweepLoop_Phys`.
    #[doc(alias = "ftMh_SweepLoop_Phys")]
    pub fn hand_glide_step(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        let target = mv_vec(&self.game.fighters[f.0], mv::TARGET);
        let (near, rate) = (h.f32(self, f, h.glide_near), h.f32(self, f, h.glide_rate));
        let d = self.boss_glide(gobj, target, near, rate);
        self.game.fighters[f.0].set_mv_f32(mv::DISTANCE, reg(d));
    }

    /// `ftMh_Wait1_2_Coll` and `ftCh_TagCancel_Coll`: there, he stops and runs the glide's
    /// callback.
    #[doc(alias = "ftMh_Wait1_2_Coll")]
    #[doc(alias = "ftCh_TagCancel_Coll")]
    pub fn hand_glide_coll(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if reg(fp.mv_f32(mv::DISTANCE)) == 0.0 {
            fp.self_vel = Vec3::new(0.0, 0.0, 0.0);
            let cb = fp.mv_i32(mv::X4) as u32;
            if cb != 0 {
                super::super::super::code::run_fighter_cb(self, gobj, CodeRef(cb));
            }
        }
    }

    /// Stopped where the glide ends: the grab's let-go and squeeze.
    #[doc(alias = "ftMh_Cancel_Coll")]
    #[doc(alias = "ftMh_Squeeze_Coll")]
    #[doc(alias = "ftCh_FingerGun1_Coll")]
    #[doc(alias = "ftCh_BackAirplane1_Coll")]
    pub fn hand_stop_at_target(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if reg(fp.mv_f32(mv::DISTANCE)) == 0.0 {
            fp.self_vel = Vec3::new(0.0, 0.0, 0.0);
        }
    }

    /// Back to his place and the wait, `ftMh_MS_389_80151018` and `ftCh_GrabUnk1_8015BC88`.
    #[doc(alias = "ftMh_MS_389_80151018")]
    #[doc(alias = "ftCh_GrabUnk1_8015BC88")]
    pub fn hand_return(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        let pos = h.point(self, f, h.home);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::HOLDING, 0);
        fp.set_u_i32(u::WAIT, h.return_mark);
        self.hand_glide_to(gobj, h, h.back_to_wait, pos);
    }

    /// One of his functions by address that takes his GObj, stopping by name where it isn't
    /// ported.
    pub(crate) fn mh_dispatch(&mut self, gobj: GObjId, a: u32) {
        if !super::super::super::code::try_fighter_cb(self, gobj, a) {
            super::super::super::code::unported_call(&format!("Master Hand's function {a:#010x}"));
        }
    }

    /// `ftMh_MS_371_801541C8` with its callback.
    fn mh_back_attack(&mut self, gobj: GObjId, cb: u32) {
        if !super::run_with(self, gobj, addr::VANISH_ENTER, cb) {
            super::super::super::code::unported_call("ftMh_MS_371_801541C8");
        }
    }
}

/// A point at `off` of his variables.
pub(crate) fn u_vec(fp: &super::super::super::structs::Fighter, off: usize) -> Vec3 {
    Vec3::new(fp.u_f32(off), fp.u_f32(off + 4), fp.u_f32(off + 8))
}

pub(crate) fn set_u_vec(fp: &mut super::super::super::structs::Fighter, off: usize, v: Vec3) {
    fp.set_u_f32(off, v.x);
    fp.set_u_f32(off + 4, v.y);
    fp.set_u_f32(off + 8, v.z);
}

/// A point at `off` of the state's variables.
pub(crate) fn mv_vec(fp: &super::super::super::structs::Fighter, off: usize) -> Vec3 {
    Vec3::new(fp.mv_f32(off), fp.mv_f32(off + 4), fp.mv_f32(off + 8))
}

pub(crate) fn set_mv_vec(fp: &mut super::super::super::structs::Fighter, off: usize, v: Vec3) {
    fp.set_mv_f32(off, reg(v.x));
    fp.set_mv_f32(off + 4, reg(v.y));
    fp.set_mv_f32(off + 8, reg(v.z));
}

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::START => g.hand_start(gobj, &HAND),
        addr::NOTHING => g.hand_nothing(gobj),
        addr::WAIT1_ENTER => g.hand_enter_wait1(gobj, &HAND),
        addr::WAIT2_ENTER => g.hand_enter_wait2(gobj, &HAND),
        addr::WAIT_ENTER => g.hand_enter_wait(gobj, &HAND),
        addr::IF_STAGE251 => g.hand_start_in_place(gobj, &HAND),
        addr::WAIT1_0_ANIM => g.hand_wait_anim(gobj, &HAND),
        addr::HUMAN_ATTACK => g.mh_human_attack(gobj),
        addr::GLIDE1_ENTER => g.hand_enter_glide1(gobj, &HAND),
        addr::GLIDE2_ENTER => g.hand_enter_glide2(gobj, &HAND),
        addr::WAIT1_2_ANIM => g.hand_glide_anim(gobj, &HAND),
        addr::WAIT1_2_PHYS => g.hand_glide_motion(gobj, &HAND),
        addr::WAIT1_2_COLL => g.hand_glide_coll(gobj),
        addr::RETURN => g.hand_return(gobj, &HAND),
        _ => return false,
    }
    true
}
