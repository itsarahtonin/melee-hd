//! UCF 0.84 (`External/UCF 0.84/UCF` in slippi-ssbm-asm), the Slippi ruleset's controller
//! fixes: what each code does at the place it injects, ported from its machine code.
//!
//! The pad buffer (`UCF Pad Buffer + 1.0 Cardinals.asm`, in `Fighter_procInput`) keeps each
//! port's last four raw stick positions in the code's own words, which the other codes read to
//! tell a fast flick from a slow one; it also snaps sticks pushed almost straight to their
//! cardinal. SDI (`UCF SDI.asm`, in `ftCo_Damage_OnEveryHitlag`) lets a fast flick count even
//! when the stick was already held. The codes in the common action states (dashback, the
//! shield drops, the down-smash out of crouch fix, shield SDI and tumble) belong with those
//! states' ports, which read [`Pads`] the same way; `docs/oracle.md` lists them.

#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::state::Running;
use crate::fpu::reg;
use crate::hsd::gobj::GObjId;

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

    /// UCF's dashback (`ftCo_Turn_IASA+0x50`, where the turn's first frames flip the facing):
    /// on the turn's frame 2, a smash the way the turn faces, fresh this frame or the last,
    /// whose raw stick travelled more than 75 along x since two frames ago, turns the fighter
    /// at once, so the rest of the interrupt dashes. Not for Nana; with one in the slot, the
    /// code also turns her CPU's stick (not ported).
    ///
    /// The Gecko code at `0x800C9A44`; the caller has flipped the facing already.
    pub fn ucf_dashback(&mut self, gobj: GObjId) {
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
        if self.game.player_entity_at(slot, 1).is_some() {
            super::code::unported_call("UCF's dashback for Nana's CPU stick");
        }
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
