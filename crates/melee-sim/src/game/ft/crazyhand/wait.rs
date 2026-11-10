//! Crazy Hand's waits (`ftcrazyhandwait10.c`) and glides (`ftcrazyhandtagcancel.c`): the
//! entrance's start, hovering in place, choosing the next attack (from his table at random,
//! or a human's buttons on port 4), and gliding to where an attack starts.

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::masterhand::u;
use super::super::masterhand::wait::{set_mv_vec, set_u_vec, u_vec};
use super::{addr, attr, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `ftCh_Init_803D4878`: the attacks he chooses from (`states`, standing for the attacks, not
/// his states), the table's rows as runs of them (`x5C`: the first and how many, through
/// `x48`, the identity) and each row's next rows (`x68`, five a row).
const ATTACKS: [i32; 18] = [
    ms::ENTRY,
    ms::DAMAGE2,
    ms::SWEEP_LOOP,
    ms::SLAP,
    ms::WALK_LOOP,
    ms::DRILL,
    ms::ROCK_CRUSH_UP,
    ms::ROCK_CRUSH_DOWN,
    ms::BACK_DISAPPEAR,
    ms::WAIT1_1,
    ms::GRAB,
    ms::POKE1,
    ms::FINGER_BEAM_START,
    ms::BACK_AIRPLANE2,
    ms::BACK_AIRPLANE3,
    ms::SQUEEZING1,
    ms::SQUEEZE,
    ms::THROW,
];
const ROWS: [(u8, u8); 6] = [(0, 4), (4, 4), (8, 3), (11, 2), (13, 2), (15, 3)];
const NEXT_ROWS: [u8; 32] = [
    1, 2, 3, 4, 5, 0, 2, 3, 4, 5, 0, 1, 3, 4, 5, 0, 1, 2, 4, 5, 0, 1, 2, 3, 5, 0, 1, 2, 3, 4, 0, 0,
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

/// An attack's start: gliding to a point first, or at once.
enum Start {
    Glide(u32, Vec3),
    Now(u32),
    With(u32, u32),
}

impl Running<'_> {
    /// His start in a match, `ftCh_Init_80155FCC`: the entrance, or waiting where he is in
    /// Event 50.
    #[doc(alias = "ftCh_Init_80155FCC")]
    pub fn ch_start(&mut self, gobj: GObjId) {
        if super::super::super::gr::stage::stkind(self.game) == ST_KIND_EVENT_50 {
            self.ch_start_in_place(gobj);
        } else {
            self.ch_entry_enter(gobj);
        }
    }

    /// Back to his place for a wait (`u.x2240_pos` kept while waiting).
    fn ch_wait_pos(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.motion_id == ms::WAIT1_0 || fp.motion_id == ms::WAIT2_0 {
            fp.cur_pos = u_vec(fp, u::WAIT_POS);
        } else {
            let p = fp.cur_pos;
            set_u_vec(fp, u::WAIT_POS, p);
        }
    }

    /// Enters wait `to`, keeping the frame from its moving form `from`.
    fn ch_change_wait(&mut self, gobj: GObjId, to: i32, from: i32) {
        self.ch_wait_pos(gobj);
        let fp = self.game.fighter(gobj);
        let frame = if fp.u_i32(u::WAIT) == from {
            fp.cur_anim_frame
        } else {
            0.0
        };
        self.change_motion_state(gobj, to, 0, frame, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).set_u_i32(u::WAIT, to);
    }

    /// Hovers (`Wait1_0`), from the wait he was in, `ftCh_Init_80156018`.
    #[doc(alias = "ftCh_Init_80156018")]
    pub fn ch_enter_wait1(&mut self, gobj: GObjId) {
        self.ch_change_wait(gobj, ms::WAIT1_0, ms::TAG_CANCEL);
    }

    /// Hovers (`Wait2_0`), `ftCh_Init_801560D8`.
    // Ports `ftCh_Init_801560D8`; aliased once a check reaches it.
    pub fn ch_enter_wait2(&mut self, gobj: GObjId) {
        self.ch_change_wait(gobj, ms::WAIT2_0, ms::WAIT2_1);
    }

    /// Hovers in the wait his last glide matches, the cry's flag cleared,
    /// `ftCh_Init_80156198`.
    #[doc(alias = "ftCh_Init_80156198")]
    pub fn ch_enter_wait(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[1] = 0;
        if fp.u_i32(u::WAIT) == ms::WAIT2_1 {
            self.ch_enter_wait2(gobj);
        } else {
            self.ch_enter_wait1(gobj);
        }
    }

    /// Event 50's start, `ftCh_Init_80156310`: at his place, waiting, his state set to the
    /// entrance first so the wait takes his place.
    #[doc(alias = "ftCh_Init_80156310")]
    pub fn ch_start_in_place(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (x, y) = (self.ch_da_f32(f, attr::X18), self.ch_da_f32(f, attr::X1C));
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[1] = 0;
        fp.set_u_i32(u::WAIT, ms::WAIT1_0);
        fp.motion_id = ms::TAG_FAIL;
        fp.cur_pos.x = x;
        fp.cur_pos.y = y;
        self.ch_enter_wait1(gobj);
    }

    /// Hovering, `ftCh_Wait1_0_Anim`: a cry on the script's flag; a human's buttons choose
    /// the next attack; a computer's timer runs out and draws one from the table.
    #[doc(alias = "ftCh_Wait1_0_Anim")]
    // NaN takes the original's branch (`--x223C < 0` fails).
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    pub fn ch_wait_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let slot = i32::from(self.game.fighters[f.0].player_idx);
        let kind = self.game.player_pkind(slot);
        if self.game.fighters[f.0].cmd_vars[1] != 0 {
            self.boss_cry(gobj);
            self.game.fighters[f.0].cmd_vars[1] = 0;
        }
        if kind == 0 {
            self.ch_human_attack(gobj);
            self.ch_wait_loop(gobj);
            return;
        }
        let fp = &mut self.game.fighters[f.0];
        let t = mem(fp::fsubs(reg(fp.u_f32(u::TIMER)), 1.0));
        fp.set_u_f32(u::TIMER, t);
        if !(reg(t) < 0.0) {
            self.ch_wait_loop(gobj);
            return;
        }
        self.ch_reset_timer(gobj);
        let row = if self.game.fighters[f.0].u_i32(u::SINCE_WALK) == self.ch_da_i32(f, attr::XC) {
            self.game.fighters[f.0].set_u_i32(u::SINCE_WALK, 0);
            2
        } else {
            let n = if self.boss_mh_beaten() { 4 } else { 5 };
            let r = self.game.objs.rng.below(n) as u32;
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
        if let Some(start) = self.ch_attack_start(gobj, attack) {
            self.ch_start_attack(gobj, start);
        }
    }

    /// Where attack `attack` starts (the table's and the human's), drawing the poke's point
    /// from the seed; `None` for the slam's point, which starts nothing, and anything else.
    fn ch_attack_start(&mut self, gobj: GObjId, attack: i32) -> Option<Start> {
        let f = self.game.fighter_id(gobj);
        let pt = |r: &Self, x, y| r.ch_da_point(f, x, y);
        Some(match attack {
            ms::DAMAGE2 => {
                let mut v = self.floor_left0();
                v.y = self.ch_da_f32(f, attr::X24);
                Start::Glide(addr::FN_80157080, v)
            }
            ms::ENTRY => {
                let mut v = self.floor_left0();
                v.y = self.ch_da_f32(f, attr::X20);
                Start::Glide(addr::FN_80156F6C, v)
            }
            ms::SWEEP_LOOP => Start::Glide(addr::FN_8015746C, pt(self, attr::X2C, attr::X30)),
            ms::SLAP => Start::Glide(addr::FN_8015755C, pt(self, attr::X34, attr::X38)),
            ms::WALK_LOOP => Start::Glide(addr::FN_801578E8, pt(self, attr::X6C, attr::X70)),
            ms::DRILL => Start::Glide(addr::FN_80157C50, pt(self, attr::X80, attr::X84)),
            ms::ROCK_CRUSH_UP => Start::Now(addr::INIT_80157DF8),
            ms::ROCK_CRUSH_DOWN => Start::Glide(addr::FN_80158144, pt(self, attr::X4C, attr::X50)),
            ms::POKE1 => Start::Glide(addr::FN_801582D8, self.ch_poke_start(gobj)),
            ms::FINGER_BEAM_START => {
                Start::Glide(addr::FN_801587B0, pt(self, attr::X98, attr::X9C))
            }
            ms::BACK_AIRPLANE2 | ms::BACK_AIRPLANE3 => Start::Now(addr::INIT_80158B3C),
            ms::BACK_DISAPPEAR => Start::With(addr::INIT_801597F0, addr::FN_80159908),
            ms::WAIT1_1 => Start::With(addr::INIT_801597F0, addr::FN_80159AA4),
            ms::GRAB => Start::Now(addr::INIT_80159F40),
            ms::SQUEEZING1 => Start::Glide(addr::FN_8015AAC8, pt(self, attr::X104, attr::X108)),
            ms::SQUEEZE => Start::Glide(addr::FN_8015AAC8, pt(self, attr::X10C, attr::X110)),
            ms::THROW => Start::Glide(addr::FN_8015AAC8, pt(self, attr::X114, attr::X118)),
            _ => return None,
        })
    }

    fn ch_start_attack(&mut self, gobj: GObjId, start: Start) {
        match start {
            Start::Glide(cb, v) => self.ch_glide_to(gobj, cb, v),
            Start::Now(a) => self.ch_dispatch(gobj, a),
            Start::With(a, cb) => {
                if !super::run_with(self, gobj, a, cb) {
                    super::super::super::code::unported_call(&format!(
                        "Crazy Hand's function {a:#010x}"
                    ));
                }
            }
        }
    }

    /// Where the poke starts: `x88` and up to `x90`, `x94` either way, at random.
    fn ch_poke_start(&mut self, gobj: GObjId) -> Vec3 {
        let f = self.game.fighter_id(gobj);
        let (c, d) = (
            self.ch_da_point(f, attr::X88, attr::X8C),
            self.ch_da_point(f, attr::X90, attr::X94),
        );
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

    /// The left end of the stage's first floor (`mpFloorGetLeft(0)`).
    pub(crate) fn floor_left0(&self) -> Vec3 {
        let g = self.game.geometry.as_ref().expect("no collision geometry");
        g.floor_left(0)
    }

    /// The wait's animation looping when it ends.
    fn ch_wait_loop(&mut self, gobj: GObjId) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        match self.game.fighter(gobj).motion_id {
            ms::WAIT1_0 => self.ch_enter_wait2(gobj),
            ms::WAIT2_0 => self.ch_enter_wait(gobj),
            _ => {}
        }
    }

    /// A human's attack (port 4's master status, a button and a direction),
    /// `ftCh_Init_80156AD8`.
    // Ports `ftCh_Init_80156AD8`; aliased once a check reaches it.
    pub fn ch_human_attack(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let b = self.game.pads.master[3].button;
        let on = |m: u32| b & m != 0;
        let next = |r: &mut Self, s: i32| r.game.fighters[f.0].set_u_i32(u::NEXT_ATTACK, s);
        let (l, rr, a, bb, y, z) = (
            on(PAD_L),
            on(PAD_R),
            on(PAD_A),
            on(PAD_B),
            on(PAD_Y),
            on(PAD_Z),
        );
        let (up, right, down, left) = (
            on(PAD_DPAD_UP),
            on(PAD_DPAD_RIGHT),
            on(PAD_DPAD_DOWN),
            on(PAD_DPAD_LEFT),
        );
        let attack = if l && up {
            ms::DAMAGE2
        } else if l && right {
            ms::ENTRY
        } else if l && down {
            ms::SWEEP_LOOP
        } else if l && left {
            ms::SLAP
        } else if rr && up {
            ms::WALK_LOOP
        } else if rr && right {
            ms::DRILL
        } else if rr && down {
            ms::ROCK_CRUSH_UP
        } else if rr && left {
            ms::ROCK_CRUSH_DOWN
        } else if bb && up {
            ms::POKE1
        } else if bb && right {
            ms::FINGER_BEAM_START
        } else if z && up {
            next(self, ms::BACK_AIRPLANE2);
            ms::BACK_AIRPLANE2
        } else if z && right {
            next(self, ms::BACK_AIRPLANE3);
            ms::BACK_AIRPLANE3
        } else if a && up {
            ms::BACK_DISAPPEAR
        } else if a && right {
            ms::WAIT1_1
        } else if a && down {
            ms::GRAB
        } else if y && up {
            next(self, ms::SQUEEZING1);
            ms::SQUEEZING1
        } else if y && right {
            next(self, ms::SQUEEZE);
            ms::SQUEEZE
        } else if y && down {
            next(self, ms::THROW);
            ms::THROW
        } else if y && left {
            // Unlike the table's, the human's slam glides to its point (`x11C`, `x120`)
            // and waits for Master Hand.
            next(self, ms::SLAM);
            let v = self.ch_da_point(f, attr::X11C, attr::X120);
            self.ch_glide_to(gobj, addr::FN_8015AAC8, v);
            return;
        } else {
            return;
        };
        if let Some(start) = self.ch_attack_start(gobj, attack) {
            self.ch_start_attack(gobj, start);
        }
    }

    /// Glides in the first wait's moving form (`0x184`), `ftCh_GrabUnk1_8015B8FC`.
    #[doc(alias = "ftCh_GrabUnk1_8015B8FC")]
    pub fn ch_enter_glide1(&mut self, gobj: GObjId) {
        self.ch_enter_glide(gobj, ms::TAG_CANCEL, ms::WAIT1_0);
    }

    /// Glides in the second's (`0x185`), `ftCh_GrabUnk1_8015B998`.
    // Ports `ftCh_GrabUnk1_8015B998`; aliased once a check reaches it.
    pub fn ch_enter_glide2(&mut self, gobj: GObjId) {
        self.ch_enter_glide(gobj, ms::WAIT2_1, ms::WAIT2_0);
    }

    /// Enters glide `to`, keeping the frame from `to` or its still form `from`.
    fn ch_enter_glide(&mut self, gobj: GObjId, to: i32, from: i32) {
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

    /// Glides to `pos`, then runs `cb`, `ftCh_GrabUnk1_8015BA34`.
    #[doc(alias = "ftCh_GrabUnk1_8015BA34")]
    pub fn ch_glide_to(&mut self, gobj: GObjId, cb: u32, pos: Vec3) {
        if self.game.fighter(gobj).u_i32(u::WAIT) == ms::WAIT2_0 {
            self.ch_enter_glide2(gobj);
        } else {
            self.ch_enter_glide1(gobj);
        }
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(mv::X4, cb as i32);
        set_mv_vec(fp, mv::TARGET, pos);
    }

    /// `ftCh_TagCancel_Anim`: the first glide's animation again when it ends (from either
    /// glide).
    #[doc(alias = "ftCh_TagCancel_Anim")]
    pub fn ch_glide_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.game
                .fighter_mut(gobj)
                .set_u_i32(u::WAIT, ms::TAG_CANCEL);
            self.ch_enter(gobj, ms::TAG_CANCEL);
        }
    }

    /// `ftCh_TagCancel_Coll`: there, he stops and runs the glide's callback.
    #[doc(alias = "ftCh_TagCancel_Coll")]
    pub fn ch_glide_coll(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if reg(fp.mv_f32(mv::DISTANCE)) == 0.0 {
            fp.self_vel = Vec3::new(0.0, 0.0, 0.0);
            let cb = fp.mv_i32(mv::X4) as u32;
            if cb != 0 {
                super::super::super::code::run_fighter_cb(self, gobj, CodeRef(cb));
            }
        }
    }

    /// Back to his place and the wait, `ftCh_GrabUnk1_8015BC88`.
    #[doc(alias = "ftCh_GrabUnk1_8015BC88")]
    pub fn ch_return(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let pos = self.ch_da_point(f, attr::X18, attr::X1C);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::HOLDING, 0);
        fp.set_u_i32(u::WAIT, ms::TAG_CANCEL);
        self.ch_glide_to(gobj, addr::INIT_80156198, pos);
    }

    /// One of his functions by address that takes his GObj, stopping by name where it isn't
    /// ported.
    pub(crate) fn ch_dispatch(&mut self, gobj: GObjId, a: u32) {
        if !super::run(self, gobj, a) {
            super::super::super::code::unported_call(&format!("Crazy Hand's function {a:#010x}"));
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::INIT_80155FCC => g.ch_start(gobj),
        addr::INIT_80156018 => g.ch_enter_wait1(gobj),
        addr::INIT_801560D8 => g.ch_enter_wait2(gobj),
        addr::INIT_80156198 => g.ch_enter_wait(gobj),
        addr::INIT_80156310 => g.ch_start_in_place(gobj),
        addr::WAIT1_0_ANIM => g.ch_wait_anim(gobj),
        addr::INIT_80156AD8 => g.ch_human_attack(gobj),
        addr::GRAB_UNK1_8015B8FC => g.ch_enter_glide1(gobj),
        addr::GRAB_UNK1_8015B998 => g.ch_enter_glide2(gobj),
        addr::TAG_CANCEL_ANIM => g.ch_glide_anim(gobj),
        addr::TAG_CANCEL_COLL => g.ch_glide_coll(gobj),
        addr::GRAB_UNK1_8015BC88 => g.ch_return(gobj),
        _ => return false,
    }
    true
}
