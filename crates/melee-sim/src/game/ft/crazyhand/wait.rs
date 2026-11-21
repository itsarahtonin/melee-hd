//! Crazy Hand's own part of the hands' waits (`ftcrazyhandwait10.c`; the rest is
//! `ft::masterhand::wait`): his attack table, his attacks' starts, and a human's buttons on
//! port 4.

use super::super::super::state::Running;
use super::super::masterhand::u;
use super::super::masterhand::wait::{
    PAD_A, PAD_B, PAD_DPAD_DOWN, PAD_DPAD_LEFT, PAD_DPAD_RIGHT, PAD_DPAD_UP, PAD_L, PAD_R, PAD_Y,
    PAD_Z,
};
use super::{HAND, addr, attr, ms};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `ftCh_Init_803D4878`: the attacks he chooses from (`states`, standing for the attacks, not
/// his states), the table's rows as runs of them (`x5C`: the first and how many, through
/// `x48`, the identity) and each row's next rows (`x68`, five a row).
pub(super) const ATTACKS: [i32; 18] = [
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
pub(super) const ROWS: [(u8, u8); 6] = [(0, 4), (4, 4), (8, 3), (11, 2), (13, 2), (15, 3)];
pub(super) const NEXT_ROWS: [u8; 32] = [
    1, 2, 3, 4, 5, 0, 2, 3, 4, 5, 0, 1, 3, 4, 5, 0, 1, 2, 4, 5, 0, 1, 2, 3, 5, 0, 1, 2, 3, 4, 0, 0,
];

/// An attack's start: gliding to a point first, or at once.
enum Start {
    Glide(u32, Vec3),
    Now(u32),
    With(u32, u32),
}

impl Running<'_> {
    /// Starts attack `attack` (the table's and the human's), if it starts anything.
    pub(crate) fn ch_attack(&mut self, gobj: GObjId, attack: i32) {
        if let Some(start) = self.ch_attack_start(gobj, attack) {
            self.ch_start_attack(gobj, start);
        }
    }

    /// Where attack `attack` starts (the table's and the human's), drawing the poke's point
    /// from the seed; `None` for the slam's point, which starts nothing, and anything else.
    fn ch_attack_start(&mut self, gobj: GObjId, attack: i32) -> Option<Start> {
        let f = self.game.fighter_id(gobj);
        let pt = |r: &Self, off| HAND.point(r, f, off);
        Some(match attack {
            ms::DAMAGE2 => {
                let mut v = self.floor_left0();
                v.y = HAND.f32(self, f, attr::X24);
                Start::Glide(addr::FN_80157080, v)
            }
            ms::ENTRY => {
                let mut v = self.floor_left0();
                v.y = HAND.f32(self, f, attr::X20);
                Start::Glide(addr::FN_80156F6C, v)
            }
            ms::SWEEP_LOOP => Start::Glide(addr::FN_8015746C, pt(self, attr::X2C)),
            ms::SLAP => Start::Glide(addr::FN_8015755C, pt(self, attr::X34)),
            ms::WALK_LOOP => Start::Glide(addr::FN_801578E8, pt(self, attr::X6C)),
            ms::DRILL => Start::Glide(addr::FN_80157C50, pt(self, attr::X80)),
            ms::ROCK_CRUSH_UP => Start::Now(addr::INIT_80157DF8),
            ms::ROCK_CRUSH_DOWN => Start::Glide(addr::FN_80158144, pt(self, attr::X4C)),
            // Where the poke starts: `x88` and up to `x90`, `x94` either way, at random.
            ms::POKE1 => Start::Glide(
                addr::FN_801582D8,
                self.hand_point_around(f, &HAND, attr::X88, attr::X90),
            ),
            ms::FINGER_BEAM_START => Start::Glide(addr::FN_801587B0, pt(self, attr::X98)),
            ms::BACK_AIRPLANE2 | ms::BACK_AIRPLANE3 => Start::Now(addr::INIT_80158B3C),
            ms::BACK_DISAPPEAR => Start::With(addr::INIT_801597F0, addr::FN_80159908),
            ms::WAIT1_1 => Start::With(addr::INIT_801597F0, addr::FN_80159AA4),
            ms::GRAB => Start::Now(addr::INIT_80159F40),
            ms::SQUEEZING1 => Start::Glide(addr::FN_8015AAC8, pt(self, attr::X104)),
            ms::SQUEEZE => Start::Glide(addr::FN_8015AAC8, pt(self, attr::X10C)),
            ms::THROW => Start::Glide(addr::FN_8015AAC8, pt(self, attr::X114)),
            _ => return None,
        })
    }

    fn ch_start_attack(&mut self, gobj: GObjId, start: Start) {
        match start {
            Start::Glide(cb, v) => self.hand_glide_to(gobj, &HAND, cb, v),
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

    /// The left end of the stage's first floor (`mpFloorGetLeft(0)`).
    pub(crate) fn floor_left0(&self) -> Vec3 {
        let g = self.game.geometry.as_ref().expect("no collision geometry");
        g.floor_left(0)
    }

    /// A human's attack (port 4's master status, a button and a direction),
    /// `ftCh_Init_80156AD8`.
    #[doc(alias = "ftCh_Init_80156AD8")]
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
            let v = HAND.point(self, f, attr::X11C);
            self.hand_glide_to(gobj, &HAND, addr::FN_8015AAC8, v);
            return;
        } else {
            return;
        };
        self.ch_attack(gobj, attack);
    }

    /// One of his functions by address that takes his GObj, stopping by name where it isn't
    /// ported.
    pub(crate) fn ch_dispatch(&mut self, gobj: GObjId, a: u32) {
        if !super::super::super::code::try_fighter_cb(self, gobj, a) {
            super::super::super::code::unported_call(&format!("Crazy Hand's function {a:#010x}"));
        }
    }
}

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::INIT_80155FCC => g.hand_start(gobj, &HAND),
        addr::INIT_80156018 => g.hand_enter_wait1(gobj, &HAND),
        addr::INIT_801560D8 => g.hand_enter_wait2(gobj, &HAND),
        addr::INIT_80156198 => g.hand_enter_wait(gobj, &HAND),
        addr::INIT_80156310 => g.hand_start_in_place(gobj, &HAND),
        addr::WAIT1_0_ANIM => g.hand_wait_anim(gobj, &HAND),
        addr::INIT_80156AD8 => g.ch_human_attack(gobj),
        addr::GRAB_UNK1_8015B8FC => g.hand_enter_glide1(gobj, &HAND),
        addr::GRAB_UNK1_8015B998 => g.hand_enter_glide2(gobj, &HAND),
        addr::TAG_CANCEL_ANIM => g.hand_glide_anim(gobj, &HAND),
        addr::TAG_CANCEL_COLL => g.hand_glide_coll(gobj),
        addr::GRAB_UNK1_8015BC88 => g.hand_return(gobj, &HAND),
        _ => return false,
    }
    true
}
