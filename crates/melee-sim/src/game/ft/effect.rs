//! The effects a fighter's action script spawns (`ftCo_8009F834`, `ftCo_09F7.c`) and the
//! script commands that spawn them or play a sound picked at random (`ftaction.c`): the effect
//! command (`ftAction_80071028`), the random sound (`ftAction_80071FC8`) and the footstep
//! dust with its terrain's sound (`ftAction_80072E4C`).

use gekko_fp as fp;

use super::super::ef::{self, Arg, SpawnKind};
use super::super::handles::FighterId;
use super::super::state::Running;
use super::common::GA_GROUND;
use super::sound::Track;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// The part ids `ftCo_8009F834` reads from the character's data: the next of its five
/// footstep parts (`ftData::x54`, cycling with `x2220_b0`), and its own effect part
/// (`ftData_x8::x10`).
const PART_FOOTSTEP: i32 = 0x8D;
const PART_DATA: i32 = 0x8E;

/// `0.003906f`, the scale fixed-point command fields take.
const FIXED: f32 = 0.003906;

/// A field of `n` bits `at` bits from a command word's top.
fn bits(w: u32, at: u32, n: u32) -> u32 {
    (w >> (32 - at - n)) & (u32::MAX >> (32 - n))
}

impl Running<'_> {
    /// `offset` spread by `range` at random, each axis `offset + 2·range·(r - 0.5)` with a
    /// fresh `HSD_Randf`.
    fn spread(&mut self, offset: Vec3, range: Vec3) -> Vec3 {
        let mut v = offset;
        let r = reg(self.game.objs.rng.draw_f32());
        v.x = mem(fp::fmadds(
            fp::fmuls(2.0, reg(range.x)),
            fp::fsubs(r, 0.5),
            reg(v.x),
        ));
        let r = reg(self.game.objs.rng.draw_f32());
        v.y = mem(fp::fmadds(
            fp::fmuls(2.0, reg(range.y)),
            fp::fsubs(r, 0.5),
            reg(v.y),
        ));
        let r = reg(self.game.objs.rng.draw_f32());
        let z = fp::fmuls(2.0, reg(range.z));
        v.z = mem(fp::fmadds(z, fp::fsubs(r, 0.5), reg(v.z)));
        v
    }

    /// The floor's angle under a grounded fighter, else 0.
    fn floor_angle(&self, f: FighterId) -> f32 {
        let fp = &self.game.fighters[f.0];
        if fp.ground_or_air == GA_GROUND {
            let n = fp.coll_data.floor.normal;
            crate::math::atan2(mem(fp::fneg(reg(n.x))), n.y)
        } else {
            0.0
        }
    }

    /// Queues one of the fighter's effects (`efAsync_Spawn` with its queue).
    fn queue_effect(
        &mut self,
        gobj: GObjId,
        kind: SpawnKind,
        gfx_id: i32,
        part: i32,
        args: &[Arg],
    ) {
        let f = self.game.fighter_id(gobj);
        let j = self.bone_joint(gobj, part);
        let mut head = self.game.fighters[f.0].x60C;
        ef::queue(self, Some(gobj), &mut head, kind, gfx_id, j, args);
        self.game.fighters[f.0].x60C = head;
    }

    /// Spawns effect `gfx_id` on the fighter's `part` for its script, `ftCo_8009F834`: at
    /// `offset` spread by `range` at random, or as the effect takes it (on the joint, with
    /// the facing, the floor's angle or a size). `common` takes `part` as a common part id;
    /// `on_state` marks the effect to go when the state changes (`x2219_b0`).
    #[doc(alias = "ftCo_8009F834")]
    #[allow(clippy::too_many_arguments)]
    pub fn script_effect(
        &mut self,
        gobj: GObjId,
        gfx_id: i32,
        part: i32,
        common: bool,
        on_state: bool,
        offset: Vec3,
        range: Vec3,
    ) {
        let f = self.game.fighter_id(gobj);
        if on_state {
            self.game.fighters[f.0].x2219_b0 = true;
        }
        let ft_data = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without data");
        let part = if part == PART_FOOTSTEP {
            let parts = self
                .data
                .ptr(ft_data.at(0x54))
                .expect("ftData's footstep parts");
            let fp = &mut self.game.fighters[f.0];
            let p = self.data.u32(parts.at(4 * u32::from(fp.x2220_b0))) as i32;
            fp.x2220_b0 = fp.x2220_b0.wrapping_add(1);
            if fp.x2220_b0 >= 5 {
                fp.x2220_b0 = 0;
            }
            p
        } else if part == PART_DATA {
            let x8 = self.data.ptr(ft_data.at(8)).expect("ftData::x8");
            i32::from(self.data.u8(x8.at(0x10)))
        } else if common {
            self.bone_index(&self.game.fighters[f.0], part)
        } else {
            part
        };
        if gfx_id < 0x250 || gfx_id / 1000 == 0x1E {
            let at = self.spread(offset, range);
            self.queue_effect(gobj, SpawnKind::PosOffset, gfx_id, part, &[Arg::Vec(at)]);
            return;
        }
        let facing = self.game.fighters[f.0].facing_dir;
        match gfx_id {
            0x402 | 0x403 | 0x409 | 0x412 | 0x413 | 0x414 | 0x422 | 0x487 | 0x4D1 | 0x4E5
            | 0x4E6 | 0x4FE | 0x500 | 0x501 | 0x502 => {
                self.queue_effect(gobj, SpawnKind::Attach, gfx_id, part, &[]);
                return;
            }
            0x446 | 0x448 | 0x4E1 => {
                self.queue_effect(
                    gobj,
                    SpawnKind::AttachOffset,
                    gfx_id,
                    part,
                    &[Arg::Vec(offset)],
                );
                return;
            }
            0x415 | 0x41E | 0x428 | 0x429 | 0x42A => {
                // `&ftData::x0->x168`, in the character's data.
                let attrs = self.data.ptr(ft_data).expect("ftData's attributes");
                let v = self.data.f32(attrs.at(0x168));
                self.queue_effect(gobj, SpawnKind::AttachParam, gfx_id, part, &[Arg::Float(v)]);
                return;
            }
            0x495 => {
                self.queue_effect(
                    gobj,
                    SpawnKind::AttachParam,
                    gfx_id,
                    part,
                    &[Arg::Float(facing)],
                );
                return;
            }
            0x49D => {
                let turn = mem(fp::fmuls(
                    reg(0.017_453_292),
                    fp::fmuls(256.0, reg(range.x)),
                ));
                self.queue_effect(
                    gobj,
                    SpawnKind::PosOffsetParam2,
                    gfx_id,
                    part,
                    &[Arg::Vec(offset), Arg::Float(facing), Arg::Float(turn)],
                );
                return;
            }
            0x438 | 0x439 => {
                let fp = &self.game.fighters[f.0];
                let size = mem(fp::fmuls(reg(fp.x34_scale.y), reg(fp.co_attrs.x144)));
                self.queue_effect(
                    gobj,
                    SpawnKind::AttachParam,
                    0x438,
                    part,
                    &[Arg::Float(size)],
                );
                return;
            }
            0x423 | 0x424 | 0x4C1 | 0x4C2 | 0x4C4 | 0x4C5 => {
                let a = self.floor_angle(f);
                self.queue_effect(gobj, SpawnKind::AttachParam, gfx_id, part, &[Arg::Float(a)]);
                return;
            }
            _ => {}
        }
        let at = self.spread(offset, range);
        match gfx_id {
            0x3E8 => {
                let v = self.common().f32(0x520 + 0x44);
                self.queue_effect(
                    gobj,
                    SpawnKind::PosOffsetParam,
                    gfx_id,
                    part,
                    &[Arg::Vec(at), Arg::Float(v)],
                );
            }
            0x3E9..=0x3EC
            | 0x3F3
            | 0x3F4
            | 0x3F6
            | 0x3FA..=0x3FC
            | 0x405
            | 0x407
            | 0x40C..=0x411
            | 0x416
            | 0x41C
            | 0x41D
            | 0x41F
            | 0x421
            | 0x425
            | 0x426
            | 0x44B
            | 0x4E2..=0x4E4 => {
                self.queue_effect(gobj, SpawnKind::PosOffset, gfx_id, part, &[Arg::Vec(at)]);
            }
            0x404 | 0x406 => {
                let a = self.floor_angle(f);
                self.queue_effect(
                    gobj,
                    SpawnKind::PosOffsetParam,
                    gfx_id,
                    part,
                    &[Arg::Vec(at), Arg::Float(a)],
                );
            }
            0x513..=0x515 => {
                let quake = gfx_id - 0x513 + 2;
                self.queue_effect(gobj, SpawnKind::CameraShake, quake, part, &[Arg::Vec(at)]);
            }
            0x3ED | 0x3EF..=0x3F2 | 0x3F5 | 0x3FE | 0x400 | 0x401 | 0x4D9 => {
                self.queue_effect(
                    gobj,
                    SpawnKind::PosOffsetParam,
                    gfx_id,
                    part,
                    &[Arg::Vec(at), Arg::Float(facing)],
                );
            }
            0x3F7..=0x3F9 | 0x3FD | 0x3FF => {
                let a = self.floor_angle(f);
                self.queue_effect(
                    gobj,
                    SpawnKind::PosOffsetParam2,
                    gfx_id,
                    part,
                    &[Arg::Vec(at), Arg::Float(facing), Arg::Float(a)],
                );
            }
            // "no effect from animlist": the original reports it and spawns nothing.
            _ => {}
        }
    }

    /// The effect command, `ftAction_80071028`: an effect on a part at an offset spread by a
    /// range, unless the fighter is invisible (`ftAction_800711DC` steps over it). Five words.
    #[doc(alias = "ftAction_80071028")]
    pub(super) fn cmd_effect(&mut self, gobj: GObjId, f: FighterId) {
        if self.game.fighters[f.0].invisible {
            self.cmd_script_main(f).skip(5);
            return;
        }
        let w = self.cmd_word_main(f, 0);
        let bone = if bits(w, 16, 1) != 0 {
            let ft_data = self.game.fighters[f.0]
                .ft_data
                .expect("a fighter without data");
            let x8 = self.data.ptr(ft_data.at(8)).expect("ftData::x8");
            i32::from(self.data.u8(x8.at(0x12)))
        } else {
            bits(w, 6, 8) as i32
        };
        let common = bits(w, 14, 1) != 0;
        let on_state = bits(w, 15, 1) != 0;
        let w1 = self.cmd_word_main(f, 1);
        let gfx_id = bits(w1, 0, 16) as i32;
        let w2 = self.cmd_word_main(f, 2);
        let w3 = self.cmd_word_main(f, 3);
        let w4 = self.cmd_word_main(f, 4);
        let fixed = |v: f64| mem(fp::fmuls(reg(FIXED), v));
        let s16 = |v: u32| fp::frsp(f64::from(v as u16 as i16));
        let u16f = |v: u32| fp::frsp(f64::from(v as u16));
        let offset = Vec3::new(fixed(s16(w2 >> 16)), fixed(s16(w2)), fixed(s16(w3 >> 16)));
        let range = Vec3::new(fixed(u16f(w3)), fixed(u16f(w4 >> 16)), fixed(u16f(w4)));
        self.cmd_script_main(f).skip(5);
        self.script_effect(gobj, gfx_id, bone, common, on_state, offset, range);
    }

    /// The random sound command, `ftAction_80071FC8`: one of up to six sounds, picked with
    /// `HSD_Randi`, on the track the command names. Seven words.
    #[doc(alias = "ftAction_80071FC8")]
    pub(super) fn cmd_random_sfx(&mut self, _gobj: GObjId, f: FighterId) {
        let w = self.cmd_word_main(f, 0);
        let (vol, pan) = (bits(w, 6, 8) as u8, bits(w, 14, 8) as u8);
        let behavior = bits(w, 22, 4);
        let range = bits(w, 26, 6) as i32;
        let pick = self.game.objs.rng.below(range);
        let sfx = match pick {
            0..=5 => self.cmd_word_main(f, 1 + pick as u32) as i32,
            // The original plays what the register holds; no command picks past six.
            _ => panic!("ftAction_80071FC8: a random sound past its six ({pick})"),
        };
        self.cmd_script_main(f).skip(7);
        // The sound command's tracks (`ft_80088478`, `ft_800881D8`, `ft_80088510`,
        // `ft_800885A8`, `ft_80088640`, `ft_80088328`).
        const TRACKS: [Track; 6] = [
            Track::Effect,
            Track::Voice,
            Track::Effect2,
            Track::Effect3,
            Track::Effect4,
            Track::Voice2,
        ];
        match behavior {
            0 => self.fighter_play_sfx(f, sfx, vol, pan),
            1..=6 => self.fighter_play_track(f, TRACKS[behavior as usize - 1], sfx, vol, pan),
            _ => {}
        }
    }

    /// The floor's sound command, `ftAction_80072CD8`: on the ground, the floor's terrain picks
    /// the sound (`ft_80084BFC`, set 0) and its effect, at the model's foot (or its other
    /// one); where the terrain has none, the command's own sound plays. Three words, as the
    /// sound command it plays through.
    #[doc(alias = "ftAction_80072CD8")]
    pub(super) fn cmd_floor_sound(&mut self, gobj: GObjId, f: FighterId) {
        let w0 = self.cmd_word_main(f, 0);
        let mut play = 1;
        if let Some((s, p, gfx_id)) = self.floor_sounds(gobj, 0) {
            play = p;
            if s[0] != -1 {
                let w2 = self.cmd_word_main(f, 2);
                self.play_sound_words(f, [w0, s[0] as u32, w2]);
            }
            if gfx_id != -1 {
                let x8 = self.ft_data_x8(&self.game.fighters[f.0]);
                let part = self
                    .data
                    .u8(x8.at(if bits(w0, 14, 1) == 0 { 0x13 } else { 0x14 }));
                let zero = Vec3::new(0.0, 0.0, 0.0);
                self.script_effect(gobj, gfx_id, i32::from(part), false, false, zero, zero);
            }
        }
        if play != 0 {
            self.sound_command_main(f);
            return;
        }
        self.cmd_script_main(f).skip(3);
    }

    /// The footstep command, `ftAction_80072E4C`: the floor's terrain picks the step's sound
    /// (`ft_80084C38`, set 1) and its dust, the dust spawns, and the step rumbles the controller.
    /// Three words, as the sound command it plays through.
    #[doc(alias = "ftAction_80072E4C")]
    pub(super) fn cmd_footstep(&mut self, gobj: GObjId, f: FighterId) {
        let w0 = self.cmd_word_main(f, 0);
        let flag = bits(w0, 8, 8) & 1;
        let (mut sound, mut play, mut gfx_id) = (None, 1, -1);
        if let Some((s, p, g)) = self.floor_sounds(gobj, 1) {
            let s = s[0];
            sound = Some(s);
            play = p;
            gfx_id = g;
        }
        if let Some(s) = sound
            && flag == 0
            && s != -1
        {
            let w2 = self.cmd_word_main(f, 2);
            self.play_sound_words(f, [w0, s as u32, w2]);
        }
        if gfx_id == -1 {
            gfx_id = bits(w0, 16, 16) as i32;
        }
        let zero = Vec3::new(0.0, 0.0, 0.0);
        self.script_effect(gobj, gfx_id, 0, false, false, zero, zero);
        if play != 0 {
            self.fighter_play_sfx(f, 0x46, 0x7F, 0x40);
            if flag == 0 {
                self.sound_command_main(f);
            }
        }
        if play == 0 || flag != 0 {
            self.cmd_script_main(f).skip(3);
        }
        self.start_rumble(f, 0x16, 0);
    }
}
