//! UCF, the Slippi ruleset's controller fixes: what each code does at the place it injects,
//! ported from its machine code. Replays carry one of two versions ([`Ucf`]).
//!
//! UCF 0.84 (`External/UCF 0.84/UCF` in slippi-ssbm-asm): the pad buffer (`UCF Pad Buffer +
//! 1.0 Cardinals.asm`, in `Fighter_procInput`) keeps each port's last four raw stick positions
//! in the code's own words, which the other codes read to tell a fast flick from a slow one; it
//! also snaps sticks pushed almost straight to their cardinal. SDI (`UCF SDI.asm`, in
//! `ftCo_Damage_OnEveryHitlag`) lets a fast flick count even when the stick was already held.
//! The codes in the common action states (dashback, the shield drops, the down-smash out of
//! crouch fix, shield SDI and tumble) belong with those states' ports, which read [`Pads`] the
//! same way; `docs/oracle.md` lists them.
//!
//! UCF 0.8 (`External/UCF 0.8/Logic`): dashback, shield drop and tumble only, at the same
//! places as 0.84's. It keeps no buffer: its dashback and tumble read the raw stick from the
//! pad library's queue, the poll the tick took and the one two polls before it.

#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::state::Running;
use crate::fpu::reg;
use crate::hsd::gobj::GObjId;

/// Which version of UCF a match runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Ucf {
    #[default]
    Off,
    /// UCF 0.8: Slippi 3.14's and 3.15's, and some 3.16 replays'.
    V0_8,
    /// UCF 0.84: Slippi 3.16's, 3.17's and 3.19's.
    V0_84,
}

/// One port's raw stick history (12 bytes of the pad buffer's data).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pad {
    /// The control stick's raw x and y, one entry per frame, written round `index`.
    pub history: [[i8; 2]; 4],
    /// The entry written last.
    pub index: u8,
    /// How many frames the stick has been flicked down out of the dead zone (`UCF SDI` and
    /// the pad buffer count it).
    pub flicked: u8,
    /// The code's two unused bytes.
    pub unused: [u8; 2],
}

/// The pad buffer's data: the four ports' histories, in the code's words after its first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pads(pub [Pad; 4]);

impl Pads {
    /// The data as the code's twelve words hold it.
    pub fn from_code_words(words: &[u32]) -> Pads {
        assert_eq!(words.len(), 12, "UCF's pad buffer is 12 words");
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_be_bytes()).collect();
        Pads::from_bytes(&bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Pads {
        Pads(core::array::from_fn(|p| {
            let b = &bytes[12 * p..12 * p + 12];
            Pad {
                history: core::array::from_fn(|i| [b[2 * i] as i8, b[2 * i + 1] as i8]),
                index: b[8],
                flicked: b[9],
                unused: [b[10], b[11]],
            }
        }))
    }

    pub fn to_bytes(&self) -> [u8; 48] {
        let mut out = [0; 48];
        for (p, pad) in self.0.iter().enumerate() {
            let b = &mut out[12 * p..12 * p + 12];
            for (i, [x, y]) in pad.history.iter().enumerate() {
                b[2 * i] = *x as u8;
                b[2 * i + 1] = *y as u8;
            }
            b[8] = pad.index;
            b[9] = pad.flicked;
            b[10..12].copy_from_slice(&pad.unused);
        }
        out
    }
}

impl Pad {
    /// The raw stick's travel since two frames ago, squared, per axis.
    fn travel(&self) -> (i32, i32) {
        let now = self.history[usize::from(self.index)];
        let before = self.history[usize::from(self.index.wrapping_sub(2) & 3)];
        let d = |a: usize| i32::from(now[a]) - i32::from(before[a]);
        (d(0) * d(0), d(1) * d(1))
    }
}

/// The constants in the pad buffer's words: how far down the stick must be for the flick
/// count, and the scale and bias that turn a stick axis into whole units.
const DOWN: f32 = -0.609_375;
const BIAS: f32 = 0.000_1;
const UNITS: f32 = 80.0;

/// A stick axis in whole units of 1/80, as the codes convert it: `|v| * 80 - 0.0001`,
/// rounded toward zero.
pub(crate) fn units(v: f32) -> i32 {
    fp::fctiwz(fp::fmsubs(fp::fabs(reg(v)), reg(UNITS), reg(BIAS)))
}

/// 1.0 cardinals: a raw stick pushed at least 80 along one axis and within 6 of the other's
/// centre snaps to that axis at full strength.
fn cardinal(x: i8, y: i8, out: &mut super::structs::Vec2) {
    let within = |v: i8, r: u8| (v as u8).wrapping_add(r) <= 2 * r;
    let full = |v: i8| f32::from_bits(((i32::from(v) as u32) & 0x8000_0000) ^ 0x3F80_0000);
    if !within(x, 79) {
        if !within(y, 6) {
            return;
        }
        out.y = 0.0;
        out.x = full(x);
    } else if !within(y, 79) && within(x, 6) {
        out.x = 0.0;
        out.y = full(y);
    }
}

impl Running<'_> {
    /// UCF's pad buffer and 1.0 cardinals (`Fighter_procInput+0x750`): a human fighter's port
    /// records the raw stick of the poll this tick took, its sticks snap to their cardinals
    /// (except Zelda's in action 349), and the down-flick count follows the stick.
    ///
    /// The Gecko code at `0x8006B460`.
    pub fn ucf_pad_buffer(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.is_cpu_controlled(&self.game.fighters[f.0]) {
            return;
        }
        let lib = &self.game.pads.lib;
        let slot = if lib.qread == 0 { 4 } else { lib.qread - 1 };
        let fp = &mut self.game.fighters[f.0];
        let port = usize::from(fp.pad_port);
        let poll = lib.queue[usize::from(slot)][port];
        let pad = &mut self.game.ucf.0[port];
        pad.index = pad.index.wrapping_add(1) & 3;
        pad.history[usize::from(pad.index)] = [poll.stick_x, poll.stick_y];
        if !(fp.kind == 0x13 && fp.motion_id == 349) {
            cardinal(poll.stick_x, poll.stick_y, &mut fp.input.lstick[0]);
            cardinal(poll.substick_x, poll.substick_y, &mut fp.input.cstick[0]);
        }
        let mut flicked = 0;
        if !(reg(fp.input.lstick[0].y) > reg(DOWN)) {
            let (x, y) = (
                units(fp.input.lstick[0].x) + 2,
                units(fp.input.lstick[0].y) + 2,
            );
            if x.wrapping_mul(x).wrapping_add(y.wrapping_mul(y)) > 6400 {
                if pad.flicked != 0 {
                    flicked = pad.flicked.wrapping_add(1);
                } else if fp.active_timer.lstick.y <= 1 && pad.travel().1 > 1936 {
                    flicked = 1;
                }
            }
        }
        pad.flicked = flicked;
    }

    /// UCF's dashback (`ftCo_Turn_IASA+0x50`, where the turn's first frames flip the facing),
    /// in the match's version.
    ///
    /// The Gecko code at `0x800C9A44`; the caller has flipped the facing already.
    pub fn ucf_dashback(&mut self, gobj: GObjId) {
        match self.game.rules.ucf {
            Ucf::Off => {}
            Ucf::V0_8 => self.ucf08_dashback(gobj),
            Ucf::V0_84 => self.ucf084_dashback(gobj),
        }
    }

    /// UCF 0.84's dashback: on the turn's frame 2, a smash the way the turn faces, fresh this
    /// frame or the last, whose raw stick travelled more than 75 along x since two frames ago,
    /// turns the fighter at once, so the rest of the interrupt dashes. Not for Nana; with
    /// another fighter in the slot, the code also turns that one's CPU stick.
    fn ucf084_dashback(&mut self, gobj: GObjId) {
        let t = reg(self.common().f32(0x3C));
        let fp = self.game.fighter(gobj);
        if fp.is_sub_fighter || fp.cur_anim_frame.to_bits() != 0x4000_0000 {
            return;
        }
        if fp::fmuls(reg(fp.facing_dir), reg(fp.input.lstick[0].x)) < t {
            return;
        }
        if fp.active_timer.lstick.x > 1 {
            return;
        }
        if self.game.ucf.0[usize::from(fp.pad_port)].travel().0 <= 5625 {
            return;
        }
        let slot = i32::from(fp.player_idx);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(0, 1);
        fp.set_mv_i32(0x18, 1);
        // The code then turns the CPU stick of the slot's other fighter (Nana's, meant), any
        // other fighter's too (a sleeping Zelda or Sheik): its stick fully the turned way (127,
        // or -128 facing left).
        if let Some(other) = self.game.player_entity_at(slot, 1) {
            let facing = self.game.fighter(gobj).facing_dir;
            self.turn_partner_cpu_stick(other, facing, (0x7F + (facing.to_bits() >> 31)) as u8);
        }
    }

    /// UCF 0.8's dashback: on the turn's frame 2, the stick pushed past the dash threshold `t`
    /// either way, fresh this frame or the last, whose raw x moved more than 75 between the
    /// poll two before this tick's and this tick's, turns the fighter at once. Not for Nana; for
    /// Popo, the code also turns Nana's CPU stick.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    fn ucf08_dashback(&mut self, gobj: GObjId) {
        const KIND_POPO: i32 = 0x0A;
        let t = reg(self.common().f32(0x3C));
        let fp = self.game.fighter(gobj);
        // NaN takes the code's unordered compares: on past the stick, out at the frame.
        if fp.cur_anim_frame.to_bits() != 0x4000_0000
            || fp::fabs(reg(fp.input.lstick[0].x)) < t
            || fp.active_timer.lstick.x >= 2
            || fp.is_sub_fighter
            || self.ucf08_travel_x(fp.pad_port) <= 5625
        {
            return;
        }
        let (slot, kind) = (i32::from(fp.player_idx), fp.kind);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(0, 1);
        fp.set_mv_i32(0x18, 1);
        if kind != KIND_POPO {
            return;
        }
        if let Some(other) = self.game.player_entity_at(slot, 1) {
            let facing = self.game.fighter(gobj).facing_dir;
            let x = if reg(facing) > 0.0 { 0x7F } else { 0x80 };
            self.turn_partner_cpu_stick(other, facing, x);
        }
    }

    /// The dashbacks' turn of the slot's other fighter: through its CPU block's input pointer
    /// (`x1A88.x444`, an entry of its inputs, `xFC`), the entry's facing and its stick's x byte.
    fn turn_partner_cpu_stick(&mut self, other: GObjId, facing: f32, x: u8) {
        let o = self.game.fighter_mut(other);
        let Some(at) = o.cpu.x444.0 else {
            super::code::unported_call("UCF's dashback through a null CPU input pointer")
        };
        assert!(
            at % 0x1C == 0 && at / 0x1C < 30,
            "UCF's dashback: the CPU input pointer is off its entries"
        );
        let e = &mut o.cpu.xFC[(at / 0x1C) as usize];
        e.facing_dir = facing;
        e.lstick.x = x as i8;
    }

    /// UCF 0.8's raw stick travel along x: the square of how far the control stick's raw x
    /// moved from the poll two before the one this tick took to that one, both read from the
    /// pad library's queue as the code reads it (`gmMain_8046B108`, five polls round `qread`).
    fn ucf08_travel_x(&self, port: u8) -> i32 {
        let lib = &self.game.pads.lib;
        let x = |index: i32| {
            let mut i = index - 1;
            if i < 0 {
                i += 5;
            }
            i32::from(lib.queue[i as usize][usize::from(port)].stick_x)
        };
        let q = i32::from(lib.qread);
        let d = x(q).wrapping_sub(x(q - 2));
        d.wrapping_mul(d)
    }

    /// UCF's tumble wiggle (`ftCo_DamageFall_IASA+0xCC`, in place of the stick's timer against
    /// `ftCommonData` x214), in the match's version, with the stick past the wiggle threshold
    /// `t`: whether the push counts as a flick.
    ///
    /// The Gecko code at `0x800908F4`.
    pub fn ucf_tumble(&self, gobj: GObjId, t: f64) -> bool {
        match self.game.rules.ucf {
            Ucf::Off => unreachable!("UCF's tumble without UCF"),
            Ucf::V0_8 => self.ucf08_tumble(gobj, t),
            Ucf::V0_84 => self.ucf084_tumble(gobj, t),
        }
    }

    /// UCF 0.84's tumble wiggle: a fresh push (timer 0) leaves the tumble and an old one (past
    /// 1) doesn't; on the push's second frame (timer 1) it leaves only if the stick was inside
    /// the threshold the frame before and the raw stick travelled more than 75 along x since two
    /// frames ago.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    fn ucf084_tumble(&self, gobj: GObjId, t: f64) -> bool {
        let fp = self.game.fighter(gobj);
        match fp.active_timer.lstick.x {
            0 => true,
            1 => {
                // NaN goes on to the travel, as the code's unordered compare does.
                !(fp::fabs(reg(fp.input.lstick[1].x)) >= t)
                    && self.game.ucf.0[usize::from(fp.pad_port)].travel().0 > 5625
            }
            _ => false,
        }
    }

    /// UCF 0.8's tumble wiggle: as 0.84's, but the push's second frame needs the stick inside
    /// the threshold the frame before (NaN isn't) and its raw x moved more than 75 between the
    /// poll two before this tick's and this tick's.
    fn ucf08_tumble(&self, gobj: GObjId, t: f64) -> bool {
        let fp = self.game.fighter(gobj);
        match fp.active_timer.lstick.x {
            0 => true,
            1 => fp::fabs(reg(fp.input.lstick[1].x)) < t && self.ucf08_travel_x(fp.pad_port) > 5625,
            _ => false,
        }
    }

    /// UCF's shield SDI (`ftCo_80093240+0x54`, past the stick's timer against `ftCommonData`
    /// x4B4): a stick held too long still shield-SDIs if it wasn't held before (sticky
    /// timer of 1 or less), was inside the threshold `min` on x the frame before, and the raw
    /// stick travelled more than 62 along x since two frames ago.
    ///
    /// The Gecko code at `0x80093294`.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    pub fn ucf_shield_sdi(&self, gobj: GObjId, min: f64) -> bool {
        let fp = self.game.fighter(gobj);
        // The code compares the last frame's x itself, not its magnitude; NaN goes on.
        fp.active_sticky.lstick.x <= 1
            && !(reg(fp.input.lstick[1].x) >= min)
            && self.game.ucf.0[usize::from(fp.pad_port)].travel().0 > 3844
    }

    /// UCF's SDI (`ftCo_Damage_OnEveryHitlag+0x5C`): with the stick held past the SDI window
    /// on its y axis, a flick still counts if the stick left the dead zone this frame and the
    /// raw stick travelled more than 62 since two frames ago, unless both axes were already
    /// held.
    ///
    /// The Gecko code at `0x8008E54C`.
    pub fn ucf_sdi(&self, gobj: GObjId, min: f64) -> bool {
        let fp = self.game.fighter(gobj);
        if fp.active_sticky.lstick.x > 1 && fp.active_sticky.lstick.y > 1 {
            return false;
        }
        let (x, y) = (reg(fp.input.lstick[1].x), reg(fp.input.lstick[1].y));
        let before = fp::fmadds(x, x, fp::fmuls(y, y));
        if fp::fmuls(min, min) <= before {
            return false;
        }
        let (tx, ty) = self.game.ucf.0[usize::from(fp.pad_port)].travel();
        tx.wrapping_add(ty) > 3844
    }
}
