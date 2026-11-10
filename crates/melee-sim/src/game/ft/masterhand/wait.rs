//! Master Hand's waits (`ftmasterhandwait10.c`, `ftmasterhandwait12.c`): the entrance's
//! start, hovering in place, choosing the next attack (from his table at random, or a
//! human's buttons on port 3), and gliding to where an attack starts.

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::{KIND_CRAZY_HAND, addr, attr, ms, mv, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `ftMh_Init_803D40D0`: the attacks he chooses from (`states`), the table's rows as runs of
/// them (`x48`: the first and how many) and each row's next rows (`x54`, five a row).
const ATTACKS: [i32; 14] = [
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
const ROWS: [(u8, u8); 6] = [(0, 3), (3, 4), (7, 3), (10, 2), (12, 2), (0, 0)];
const NEXT_ROWS: [u8; 30] = [
    1, 2, 3, 4, 0, 0, 2, 3, 4, 0, 0, 1, 3, 4, 0, 0, 1, 2, 4, 0, 0, 1, 2, 3, 0, 0, 0, 0, 0, 0,
];

/// `HSD_PAD_*` of the master status.
const PAD_DPAD_LEFT: u32 = 0x1;
const PAD_DPAD_RIGHT: u32 = 0x2;
const PAD_DPAD_DOWN: u32 = 0x4;
const PAD_DPAD_UP: u32 = 0x8;
const PAD_Z: u32 = 0x10;
const PAD_R: u32 = 0x20;
const PAD_L: u32 = 0x40;
const PAD_A: u32 = 0x100;
const PAD_B: u32 = 0x200;
const PAD_Y: u32 = 0x800;

/// `Stage_80225194`'s stage kind of Event 50 (the hands' event), where he skips his
/// entrance.
const ST_KIND_EVENT_50: i32 = 251;

impl Running<'_> {
    /// His start in a match, `ftMh_MS_341_8014FE10`: the entrance, or waiting where he is in
    /// Event 50.
    #[doc(alias = "ftMh_MS_341_8014FE10")]
    pub fn mh_start(&mut self, gobj: GObjId) {
        if super::super::super::gr::stage::stkind(self.game) == ST_KIND_EVENT_50 {
            self.mh_start_in_place(gobj);
        } else {
            self.mh_entry_enter(gobj);
        }
    }

    /// `ftMh_MS_341_8014FE58`: nothing.
    #[doc(alias = "ftMh_MS_341_8014FE58")]
    pub fn mh_nop(&mut self, _gobj: GObjId) {}

    /// Back to his place for a wait (`u.x2240_pos` kept while waiting).
    fn mh_wait_pos(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.motion_id == ms::WAIT1_0 || fp.motion_id == ms::WAIT2_0 {
            fp.cur_pos = u_vec(fp, u::WAIT_POS);
        } else {
            let p = fp.cur_pos;
            set_u_vec(fp, u::WAIT_POS, p);
        }
    }

    /// Hovers (`Wait1_0`), from the wait he was in, `ftMh_MS_341_8014FE5C`.
    #[doc(alias = "ftMh_MS_341_8014FE5C")]
    pub fn mh_enter_wait1(&mut self, gobj: GObjId) {
        self.mh_wait_pos(gobj);
        self.mh_change_wait(gobj, ms::WAIT1_0, ms::WAIT1_2);
    }

    /// Hovers (`Wait2_0`): `ftMh_MS_341_8014FF1C`'s body, which the original runs only as
    /// [`Self::mh_enter_wait`]'s inline copy (its own call is dead, ledger/dead.txt).
    pub fn mh_enter_wait2(&mut self, gobj: GObjId) {
        self.mh_wait_pos(gobj);
        self.mh_change_wait(gobj, ms::WAIT2_0, ms::WAIT2_1);
    }

    /// Enters wait `to`, keeping the frame from its moving form `from`.
    fn mh_change_wait(&mut self, gobj: GObjId, to: i32, from: i32) {
        let fp = self.game.fighter(gobj);
        let frame = if fp.u_i32(u::WAIT) == from {
            fp.cur_anim_frame
        } else {
            0.0
        };
        self.change_motion_state(gobj, to, 0, frame, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).set_u_i32(u::WAIT, to);
    }

    /// Hovers in the wait his last moving one matches, `ftMh_MS_341_8014FFDC`.
    #[doc(alias = "ftMh_MS_341_8014FFDC")]
    pub fn mh_enter_wait(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).u_i32(u::WAIT) == ms::WAIT2_1 {
            self.mh_enter_wait2(gobj);
        } else {
            self.mh_enter_wait1(gobj);
        }
    }

    /// Event 50's start, `ifStage251`: at his place, waiting, his state set to the entrance
    /// first so the wait takes his place.
    #[doc(alias = "ifStage251")]
    pub fn mh_start_in_place(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let pos = self.mh_da_vec2(f, attr::X30);
        let fp = &mut self.game.fighters[f.0];
        fp.set_u_i32(u::WAIT, ms::WAIT1_0);
        fp.motion_id = ms::ENTRY;
        fp.cur_pos.x = pos.x;
        fp.cur_pos.y = pos.y;
        self.mh_enter_wait1(gobj);
    }

    /// Hovering, `ftMh_Wait1_0_Anim`: a human's buttons choose the next attack; a computer's
    /// timer runs out and draws one from the table (or joins Crazy Hand's tag attack).
    #[doc(alias = "ftMh_Wait1_0_Anim")]
    // NaN takes the original's branch (`--x223C < 0` fails).
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    pub fn mh_wait_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let slot = i32::from(self.game.fighters[f.0].player_idx);
        if self.game.player_pkind(slot) == 0 {
            self.mh_human_attack(gobj);
            self.mh_wait_loop(gobj);
            return;
        }
        let fp = &mut self.game.fighters[f.0];
        let t = mem(fp::fsubs(reg(fp.u_f32(u::TIMER)), 1.0));
        fp.set_u_f32(u::TIMER, t);
        if !(reg(t) < 0.0) {
            self.mh_wait_loop(gobj);
            return;
        }
        self.mh_reset_timer(gobj);
        if self.boss_motion_id(KIND_CRAZY_HAND) == super::super::crazyhand::ms::TAG_ROCK_PAPER {
            self.mh_tag_attack(gobj);
            return;
        }
        let fp = &mut self.game.fighters[f.0];
        let row = if fp.u_i32(u::SINCE_WALK) == self.mh_da_i32(f, attr::X24) {
            self.game.fighters[f.0].set_u_i32(u::SINCE_WALK, 0);
            2
        } else {
            let r = self.game.objs.rng.below(5) as u32;
            let fp = &mut self.game.fighters[f.0];
            let i = (fp.u_i32(u::ROW) as u32).wrapping_mul(5).wrapping_add(r);
            let next = i32::from(NEXT_ROWS[i as usize]);
            if next == 2 {
                fp.set_u_i32(u::SINCE_WALK, 0);
            } else {
                let n = fp.u_i32(u::SINCE_WALK).wrapping_add(1);
                fp.set_u_i32(u::SINCE_WALK, n);
            }
            next
        };
        let (first, count) = ROWS[row as usize];
        let pick = i32::from(first) + self.game.objs.rng.below(i32::from(count));
        let mut attack = ATTACKS[pick as usize];
        let fp = &mut self.game.fighters[f.0];
        fp.set_u_i32(u::ROW, row);
        fp.set_u_i32(u::NEXT_ATTACK, attack);
        if fp.input_disabled {
            attack = ms::WAIT1_0;
        }
        self.mh_start_attack(gobj, attack);
    }

    /// The wait's animation looping when it ends.
    fn mh_wait_loop(&mut self, gobj: GObjId) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        match self.game.fighter(gobj).motion_id {
            ms::WAIT1_0 => {
                self.mh_wait_pos(gobj);
                self.mh_change_wait(gobj, ms::WAIT2_0, ms::WAIT2_1);
            }
            ms::WAIT2_0 => {
                if self.game.fighter(gobj).u_i32(u::WAIT) == ms::WAIT2_1 {
                    self.mh_enter_wait2(gobj);
                } else {
                    self.mh_enter_wait1(gobj);
                }
            }
            _ => {}
        }
    }

    /// Starts the attack `attack` drew (the table's states stand for the attacks): most glide
    /// to their start first.
    fn mh_start_attack(&mut self, gobj: GObjId, attack: i32) {
        let f = self.game.fighter_id(gobj);
        let da = |r: &Self, off| r.mh_da_vec2(f, off);
        match attack {
            ms::SWEEP_LOOP => {
                let mut v = self.floor_right0();
                v.y = self.mh_da_f32(f, attr::X38);
                self.mh_glide_to(gobj, addr::MS_344_80151828, v);
            }
            ms::SLAP => {
                let mut v = self.floor_right0();
                v.y = self.mh_da_f32(f, attr::X50 + 4);
                self.mh_glide_to(gobj, addr::MS_348_80151BB8, v);
            }
            ms::WALK_LOOP => {
                let v = da(self, attr::X40);
                self.mh_glide_to(gobj, addr::MS_353_80151DC4, v);
            }
            ms::DRILL => {
                let v = da(self, attr::X88);
                self.mh_glide_to(gobj, addr::MS_352_801521DC, v);
            }
            ms::ROCK_CRUSH_UP => {
                let v = da(self, attr::XA8);
                self.mh_glide_to(gobj, addr::MS_354_80152370, v);
            }
            ms::PAPER_CRUSH => {
                let v = da(self, attr::XBC);
                self.mh_glide_to(gobj, addr::MS_357_801526D8, v);
            }
            ms::POKE1 => self.mh_dispatch(gobj, addr::MS_358_80152880),
            ms::BACK_AIRPLANE1 => self.mh_dispatch(gobj, addr::MS_366_80153820),
            ms::BACK_PUNCH => self.mh_back_attack(gobj, addr::MS_369_80153B90),
            ms::BACK_CRUSH => self.mh_back_attack(gobj, addr::MS_370_80153D2C),
            ms::FINGER_BEAM_LOOP => {
                let v = self.mh_laser_start(f);
                self.mh_glide_to(gobj, addr::MS_359_80152BCC, v);
            }
            ms::FINGER_GUN2 => self.mh_dispatch(gobj, addr::MS_363_801530A4),
            ms::THROW | ms::SLAM => self.mh_dispatch(gobj, addr::MS_372_801542E0),
            _ => {}
        }
    }

    /// Where the finger beams start: `xC4` and up to `xCC` either way, at random.
    fn mh_laser_start(&mut self, f: super::super::super::handles::FighterId) -> Vec3 {
        let (c, d) = (self.mh_da_vec2(f, attr::XC4), self.mh_da_vec2(f, attr::XCC));
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
            ch_ms::SQUEEZING1 => (addr::MS_380_80155194, attr::X124),
            ch_ms::SQUEEZE => (addr::MS_382_801552F8, attr::X12C),
            ch_ms::THROW => (addr::MS_383_80155484, attr::X134),
            // The original sets the point for `Slam` and starts nothing.
            _ => return,
        };
        let v = self.mh_da_vec2(f, off);
        self.mh_glide_to(gobj, cb, v);
    }

    /// A human's attack (port 3's master status, a button and a direction),
    /// `ftMh_MS_341_80150894`.
    #[doc(alias = "ftMh_MS_341_80150894")]
    pub fn mh_human_attack(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let b = self.game.pads.master[2].button;
        let on = |m: u32| b & m != 0;
        let da = |r: &Self, off| r.mh_da_vec2(f, off);
        let l = on(PAD_L);
        if l && on(PAD_DPAD_UP) {
            let mut v = self.floor_right0();
            v.y = self.mh_da_f32(f, attr::X38);
            self.mh_glide_to(gobj, addr::MS_344_80151828, v);
        } else if l && on(PAD_DPAD_RIGHT) {
            let mut v = self.floor_right0();
            v.y = self.mh_da_f32(f, attr::X50 + 4);
            self.mh_glide_to(gobj, addr::MS_348_80151BB8, v);
        } else if l && on(PAD_DPAD_DOWN) {
            let v = da(self, attr::X40);
            self.mh_glide_to(gobj, addr::MS_353_80151DC4, v);
        } else if on(PAD_R) && on(PAD_DPAD_UP) {
            let v = da(self, attr::X88);
            self.mh_glide_to(gobj, addr::MS_352_801521DC, v);
        } else if on(PAD_R) && on(PAD_DPAD_RIGHT) {
            let v = da(self, attr::XA8);
            self.mh_glide_to(gobj, addr::MS_354_80152370, v);
        } else if on(PAD_R) && on(PAD_DPAD_DOWN) {
            let v = da(self, attr::XBC);
            self.mh_glide_to(gobj, addr::MS_357_801526D8, v);
        } else if on(PAD_R) && on(PAD_DPAD_LEFT) {
            self.mh_dispatch(gobj, addr::MS_358_80152880);
        } else if on(PAD_A) && on(PAD_DPAD_UP) {
            self.mh_dispatch(gobj, addr::MS_366_80153820);
        } else if on(PAD_A) && on(PAD_DPAD_RIGHT) {
            self.mh_back_attack(gobj, addr::MS_369_80153B90);
        } else if on(PAD_A) && on(PAD_DPAD_DOWN) {
            self.mh_back_attack(gobj, addr::MS_370_80153D2C);
        } else if on(PAD_B) && on(PAD_DPAD_UP) {
            let v = self.mh_laser_start(f);
            self.mh_glide_to(gobj, addr::MS_359_80152BCC, v);
        } else if on(PAD_B) && on(PAD_DPAD_RIGHT) {
            self.mh_dispatch(gobj, addr::MS_363_801530A4);
        } else if on(PAD_Z) && on(PAD_DPAD_UP) {
            self.game.fighters[f.0].set_u_i32(u::NEXT_ATTACK, ms::THROW);
            self.mh_dispatch(gobj, addr::MS_372_801542E0);
        } else if on(PAD_Z) && on(PAD_DPAD_RIGHT) {
            self.game.fighters[f.0].set_u_i32(u::NEXT_ATTACK, ms::SLAM);
            self.mh_dispatch(gobj, addr::MS_372_801542E0);
        } else if on(PAD_Y) && on(PAD_DPAD_UP) {
            self.mh_tag_attack(gobj);
        }
    }

    /// Glides in the first wait's moving form (`Wait1_2`), `ftMh_MS_389_80150C8C`.
    #[doc(alias = "ftMh_MS_389_80150C8C")]
    pub fn mh_enter_glide1(&mut self, gobj: GObjId) {
        self.mh_enter_glide(gobj, ms::WAIT1_2, ms::WAIT1_0);
    }

    /// Glides in the second's (`Wait2_1`), `ftMh_MS_389_80150D28`.
    #[doc(alias = "ftMh_MS_389_80150D28")]
    pub fn mh_enter_glide2(&mut self, gobj: GObjId) {
        self.mh_enter_glide(gobj, ms::WAIT2_1, ms::WAIT2_0);
    }

    /// Enters glide `to`, keeping the frame from `to` or its still form `from`.
    fn mh_enter_glide(&mut self, gobj: GObjId, to: i32, from: i32) {
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

    /// Glides to `pos`, then runs `cb`, `ftMh_MS_389_80150DC4`.
    #[doc(alias = "ftMh_MS_389_80150DC4")]
    pub fn mh_glide_to(&mut self, gobj: GObjId, cb: u32, pos: Vec3) {
        if self.game.fighter(gobj).u_i32(u::WAIT) == ms::WAIT2_0 {
            self.mh_enter_glide2(gobj);
        } else {
            self.mh_enter_glide1(gobj);
        }
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(mv::X4, cb as i32);
        set_mv_vec(fp, mv::TARGET, pos);
    }

    /// `ftMh_Wait1_2_Anim`: the glide's animation again when it ends.
    #[doc(alias = "ftMh_Wait1_2_Anim")]
    pub fn mh_glide_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.game.fighter_mut(gobj).set_u_i32(u::WAIT, ms::WAIT1_2);
            self.change_motion_state(gobj, ms::WAIT1_2, 0, 0.0, 1.0, 0.0, None);
            self.anim_update(gobj);
        }
    }

    /// `ftMh_Wait1_2_Phys`: the animation's motion and the glide toward its point
    /// (`ftMh_UpdateBossMotion`).
    #[doc(alias = "ftMh_Wait1_2_Phys")]
    pub fn mh_glide_phys(&mut self, gobj: GObjId) {
        self.mh_update_motion(gobj);
    }

    /// `ftMh_UpdateBossMotion`: `ft_80085134` and `ftBossLib_8015BE40` toward `mv.xC`.
    pub(crate) fn mh_update_motion(&mut self, gobj: GObjId) {
        self.air_anim_motion(gobj);
        let f = self.game.fighter_id(gobj);
        let target = mv_vec(&self.game.fighters[f.0], mv::TARGET);
        let (near, rate) = (self.mh_da_f32(f, attr::X2C), self.mh_da_f32(f, attr::X28));
        let d = self.boss_glide(gobj, target, near, rate);
        self.game.fighters[f.0].set_mv_f32(mv::DISTANCE, reg(d));
    }

    /// `ftMh_Wait1_2_Coll`: there, he stops and runs the glide's callback.
    #[doc(alias = "ftMh_Wait1_2_Coll")]
    pub fn mh_glide_coll(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if reg(fp.mv_f32(mv::DISTANCE)) == 0.0 {
            fp.self_vel = Vec3::new(0.0, 0.0, 0.0);
            let cb = fp.mv_i32(mv::X4) as u32;
            if cb != 0 {
                super::super::super::code::run_fighter_cb(self, gobj, CodeRef(cb));
            }
        }
    }

    /// Back to his place and the wait, `ftMh_MS_389_80151018`.
    #[doc(alias = "ftMh_MS_389_80151018")]
    pub fn mh_return(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let pos = self.mh_da_vec2(f, attr::X30);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::HOLDING, 0);
        fp.set_u_i32(u::WAIT, ms::WAIT2_1);
        self.mh_glide_to(gobj, addr::MS_341_8014FFDC, pos);
    }

    /// One of his functions by address that takes his GObj, stopping by name where it isn't
    /// ported.
    pub(crate) fn mh_dispatch(&mut self, gobj: GObjId, a: u32) {
        if !super::run(self, gobj, a) {
            super::super::super::code::unported_call(&format!("Master Hand's function {a:#010x}"));
        }
    }

    /// `ftMh_MS_371_801541C8` with its callback.
    fn mh_back_attack(&mut self, gobj: GObjId, cb: u32) {
        if !super::run_with(self, gobj, addr::MS_371_801541C8, cb) {
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

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::MS_341_8014FE10 => g.mh_start(gobj),
        addr::MS_341_8014FE58 => g.mh_nop(gobj),
        addr::MS_341_8014FE5C => g.mh_enter_wait1(gobj),
        addr::MS_341_8014FF1C => g.mh_enter_wait2(gobj),
        addr::MS_341_8014FFDC => g.mh_enter_wait(gobj),
        addr::IF_STAGE251 => g.mh_start_in_place(gobj),
        addr::WAIT1_0_ANIM => g.mh_wait_anim(gobj),
        addr::MS_341_80150894 => g.mh_human_attack(gobj),
        addr::MS_389_80150C8C => g.mh_enter_glide1(gobj),
        addr::MS_389_80150D28 => g.mh_enter_glide2(gobj),
        addr::WAIT1_2_ANIM => g.mh_glide_anim(gobj),
        addr::WAIT1_2_PHYS => g.mh_glide_phys(gobj),
        addr::WAIT1_2_COLL => g.mh_glide_coll(gobj),
        addr::MS_389_80151018 => g.mh_return(gobj),
        _ => return false,
    }
    true
}
