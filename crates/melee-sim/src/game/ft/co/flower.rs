//! The Lip's Stick's flower on a fighter's head (`ftcommon.c`): a spore's hit plants it, or
//! grows one already there; while it lasts the fighter takes a little damage every few
//! frames and mashes to shake it off sooner; when its time runs out it falls.

use gekko_fp as fp;

use super::super::super::it::{kinds::flower, release};
use super::super::super::state::Running;
use super::{PAD_A, PAD_B, PAD_LR, PAD_XY};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `ftCommonData`: the flower's frames per point of damage (x704), the most it lasts (x700),
/// the frames between its damage (x6F8), a mash's worth (x6FC) and its growth (x708 to
/// x710).
mod cd {
    pub const LONGEST: u32 = 0x700;
    pub const FRAMES_PER_DAMAGE: u32 = 0x704;
    pub const TICK: u32 = 0x6F8;
    pub const MASH: u32 = 0x6FC;
    pub const SIZE_BASE: u32 = 0x708;
    pub const SIZE_MAX: u32 = 0x70C;
    pub const SIZE_PER_DAMAGE: u32 = 0x710;
}

impl Running<'_> {
    /// A spore's hit of `damage` on the fighter, `ftCommon_8007FC7C`: the flower already
    /// there lasts longer and grows; else one grows on the head; and the sound.
    #[doc(alias = "ftCommon_8007FC7C")]
    pub fn flower_hit(&mut self, gobj: GObjId, damage: f64) {
        let f = self.game.fighter_id(gobj);
        let c = self.common();
        let frames = fp::fctiwz(fp::fmuls(damage, reg(c.f32(cd::FRAMES_PER_DAMAGE))));
        let longest = c.i32(cd::LONGEST);
        let fp = &mut self.game.fighters[f.0];
        if fp.x1980.is_some() {
            fp.x2018 = fp.x2018.wrapping_add(frames);
            if fp.x2018 > longest {
                fp.x2018 = longest;
            }
            let grown = fp.x2024;
            fp.x2024 = fp::fctiwz(fp::fadds(fp::frsp(f64::from(grown)), damage));
            self.flower_shape(gobj);
        } else {
            let (pos, facing) = (fp.cur_pos, fp.facing_dir);
            let fp = &self.game.fighters[f.0];
            let part = i32::from(self.data.u8(self.ft_data_x8(fp).at(0x12)));
            if let Some(it) = flower::grow(self, gobj, pos, part, facing) {
                self.flower_planted(gobj, it, frames, damage);
                self.flower_shape(gobj);
            }
        }
        self.fighter_play_sfx(f, 0x11F, 0x7F, 0x40);
    }

    /// The flower's size and stem for the damage that grew it, `ftCommon_8007FDA0`: the
    /// character's offset (`co_attrs.x130`) shrunk by the growth, the petals by it.
    #[doc(alias = "ftCommon_8007FDA0")]
    pub fn flower_shape(&mut self, gobj: GObjId) {
        let c = self.common();
        let fp = self.game.fighter(gobj);
        // `my_fminf`.
        let grown = fp::fmadds(
            reg(c.f32(cd::SIZE_PER_DAMAGE)),
            fp::frsp(f64::from(fp.x2024)),
            reg(c.f32(cd::SIZE_BASE)),
        );
        let max = reg(c.f32(cd::SIZE_MAX));
        let size = if grown > max { max } else { grown };
        let inv = fp::fdivs(1.0, size);
        let o = fp.co_attrs.x130;
        let offset = Vec3::new(
            mem(fp::fmuls(reg(o.x), inv)),
            mem(fp::fmuls(reg(o.y), inv)),
            mem(fp::fmuls(reg(o.z), inv)),
        );
        let petals = mem(fp::fmuls(
            reg(fp.x34_scale.y),
            fp::fmuls(size, reg(fp.co_attrs.x13C)),
        ));
        // `it_8029A8F4` and `it_8029A89C` do nothing without a flower.
        if let Some(it) = fp.x1980 {
            flower::set_offset(self, it, offset);
            flower::set_size(self, it, petals);
        }
    }

    /// A flower newly on the head, `ftCommon_8007FE84`: its time, the damage's tick and its
    /// growth, the rumble, and the character's model for it (`ftData_UnkMotionStates1`).
    #[doc(alias = "ftCommon_8007FE84")]
    pub fn flower_planted(&mut self, gobj: GObjId, it: GObjId, frames: i32, damage: f64) {
        let f = self.game.fighter_id(gobj);
        let tick = self.common().i32(cd::TICK);
        let fp = &mut self.game.fighters[f.0];
        fp.x1980 = Some(it);
        fp.x2018 = frames;
        fp.x201C = tick;
        fp.x2024 = fp::fctiwz(damage);
        fp.x2021 = 0;
        fp.x2020 = 0;
        self.start_rumble(f, 2, 0);
        self.worn_item_on(gobj);
    }

    /// The flower gone from the head, `ftCommon_8007FF74`: the rumble stops and the player's
    /// record notes it.
    #[doc(alias = "ftCommon_8007FF74")]
    pub fn flower_gone(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].x1980 = None;
        self.stop_rumble(f, 2);
        let fp = &self.game.fighters[f.0];
        let (slot, sub) = (i32::from(fp.player_idx), fp.is_sub_fighter);
        self.flower_gone_record(slot, sub);
    }

    /// The player's record of a flower gone (`xCAC`), but for Nana, `pl_80040460`.
    #[doc(alias = "pl_80040460")]
    pub fn flower_gone_record(&mut self, slot: i32, sub: bool) {
        if !sub {
            self.game.player_stale_moves(slot).xCAC = 6;
        }
    }

    /// The flower falls when its time runs out (`Fighter_8006A1BC`'s worn items' countdown):
    /// the model back, dropped where it is, and gone from the head.
    pub fn flower_falls(&mut self, gobj: GObjId) {
        self.worn_item_off(gobj);
        let it = self
            .game
            .fighter(gobj)
            .x1980
            .expect("a falling flower with no flower");
        release::drop(self, it, Vec3::new(0.0, 0.0, 0.0), 0.0);
        self.flower_gone(gobj);
    }

    /// Mashing the flower off, `ftCommon_8007FFD8`: a button, or the stick pushed a new way,
    /// takes `amount` off its time; the player's record counts the frame and whether it
    /// mashed (`pl_800402D0`). Whether it mashed.
    #[doc(alias = "ftCommon_8007FFD8")]
    pub fn flower_mash(&mut self, gobj: GObjId, amount: f32) -> bool {
        let push = reg(self.common().f32(0x308));
        let fp = self.game.fighter_mut(gobj);
        let mut mashed = false;
        let buttons = PAD_A | PAD_B | PAD_XY | PAD_LR;
        if fp.input.pressed_buttons & buttons != 0 {
            fp.x2018 = fp::fctiwz(fp::fsubs(fp::frsp(f64::from(fp.x2018)), reg(amount)));
            mashed = true;
        }
        let (b0, b1) = (fp.x2020, fp.x2021);
        let (sx, sy) = (reg(fp.input.lstick[0].x), reg(fp.input.lstick[0].y));
        if sx < fp::fneg(push) {
            fp.x2020 = -1;
        }
        if sx > push {
            fp.x2020 = 1;
        }
        if sy < fp::fneg(push) {
            fp.x2021 = -1;
        }
        if sy > push {
            fp.x2021 = 1;
        }
        if b0 != fp.x2020 || b1 != fp.x2021 {
            fp.x2018 = fp::fctiwz(fp::fsubs(fp::frsp(f64::from(fp.x2018)), reg(amount)));
            mashed = true;
        }
        if !fp.is_sub_fighter {
            let slot = i32::from(fp.player_idx);
            let t = self.game.player_stale_moves(slot);
            t.xCEC = t.xCEC.wrapping_add(1);
            if mashed {
                t.xCF0 = t.xCF0.wrapping_add(1);
            }
        }
        mashed
    }

    /// `ftCommonData` x6FC, a mash's worth against the flower (an integer, as a float).
    pub fn flower_mash_amount(&self) -> f32 {
        mem(fp::frsp(f64::from(self.common().i32(cd::MASH))))
    }
}
