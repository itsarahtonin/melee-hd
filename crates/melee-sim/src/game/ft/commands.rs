//! The fighter's own action-script commands (`ftAction_803C06E8`, opcodes 10 on): hitboxes,
//! flags the action's callbacks read, color and texture animations, rumble and sounds; the
//! effect commands are `effect.rs`'s. Commands that reach items, and a few others, aren't
//! ported.

use gekko_fp as fp;

use super::super::code::unported_call;
use super::super::handles::FighterId;
use super::super::state::Running;
use super::colanim::Overlay;
use super::common::{FIGHTER_P_LINK, GA_AIR};
use super::sound::Track;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::{aobj, tobj};
use crate::lb::command::Script;
use crate::lb::hitcapsule;

/// The commands by name (`ftAction_803C06E8`), from opcode 10.
const COMMANDS: [&str; 49] = [
    "ftAction_80071028",
    "ftAction_8007121C",
    "ftAction_8007162C",
    "ftAction_8007169C",
    "ftAction_80071708",
    "ftAction_80071784",
    "ftAction_800717D8",
    "ftAction_80071B50",
    "ftAction_80071CCC",
    "ftAction_80071820",
    "ftAction_800718A4",
    "ftAction_80071908",
    "ftAction_8007192C",
    "ftAction_80071950",
    "ftAction_80071974",
    "ftAction_80071998",
    "ftAction_80071A14",
    "ftAction_80071A58",
    "ftAction_80071A9C",
    "ftAction_80071AE8",
    "ftAction_80071B28",
    "ftAction_80071D40",
    "ftAction_80071D94",
    "ftAction_80071DCC",
    "ftAction_80071E04",
    "ftAction_80071F34",
    "ftAction_80071F78",
    "ftAction_80071FA0",
    "ftAction_80071FC8",
    "ftAction_80072320",
    "ftAction_800726F4",
    "ftAction_800727C8",
    "ftAction_80072894",
    "ftAction_800728F8",
    "ftAction_8007297C",
    "ftAction_800729D4",
    "ftAction_80072A5C",
    "ftAction_80072ABC",
    "ftAction_80072B14",
    "ftAction_80072B3C",
    "ftAction_80072B94",
    "ftAction_80072BF4",
    "ftAction_80072C6C",
    "ftAction_80072CB0",
    "ftAction_80072CD8",
    "ftAction_80072E4C",
    "ftAction_80073008",
    "ftAction_800730B8",
    "ftAction_80073118",
];

/// `n` bits of a command word from bit `at` (0 the most significant).
fn bits(w: u32, at: u32, n: u32) -> u32 {
    (w >> (32 - at - n)) & (u32::MAX >> (32 - n))
}

/// [`bits`], sign-extended.
fn sbits(w: u32, at: u32, n: u32) -> i32 {
    ((w << at) as i32) >> (32 - n)
}

/// An unsigned command field as a float: `(f32)` of the integer.
fn ufloat(v: u32) -> f64 {
    fp::frsp(f64::from(v))
}

/// A signed command field as a float.
fn sfloat(v: i32) -> f64 {
    fp::frsp(f64::from(v))
}

/// `0.003906f`, the scale fixed-point command fields take.
const FIXED: f32 = 0.003906;

/// `SmashState_Release`.
const SMASH_RELEASE: i32 = 3;

impl Running<'_> {
    /// The script a command runs in: the action's (`x3E4_fighterCmdScript`) or a color
    /// overlay's.
    pub(super) fn cmd_script(&mut self, f: FighterId, which: Option<Overlay>) -> &mut Script {
        let fp = &mut self.game.fighters[f.0];
        match which {
            None => &mut fp.x3E4_fighterCmdScript.script,
            Some(Overlay::X408) => &mut fp.x408.script,
            Some(Overlay::X488) => &mut fp.x488.script,
            Some(Overlay::X508) => &mut fp.x508.script,
        }
    }

    fn cmd_word(&self, f: FighterId, n: u32) -> u32 {
        self.cmd_word_in(f, None, n)
    }

    pub(super) fn cmd_word_in(&self, f: FighterId, which: Option<Overlay>, n: u32) -> u32 {
        let fp = &self.game.fighters[f.0];
        let s = match which {
            None => &fp.x3E4_fighterCmdScript.script,
            Some(Overlay::X408) => &fp.x408.script,
            Some(Overlay::X488) => &fp.x488.script,
            Some(Overlay::X508) => &fp.x508.script,
        };
        s.word_at(self.data, n)
    }

    /// Steps the script past `n` words (`NEXT_CMD`, `SKIP_CMD`).
    fn cmd_skip(&mut self, f: FighterId, n: u32) {
        self.cmd_script(f, None).skip(n);
    }

    /// The hitbox a command names; the original indexes past the four with no check.
    fn cmd_hitbox(&mut self, f: FighterId, i: u32) -> &mut super::super::structs::HitCapsule {
        assert!(i < 4, "hitbox {i} of a command");
        &mut self.game.fighters[f.0].x914[i as usize]
    }

    /// Runs fighter command `index` (opcode `index + 10`, `ftAction_803C06E8`).
    pub(super) fn fighter_command(&mut self, gobj: GObjId, index: u32) {
        let f = self.game.fighter_id(gobj);
        let w = self.cmd_word(f, 0);
        match index {
            3 => self.cmd_hitbox_scale(gobj, f, w),
            4 => self.cmd_hitbox_flags(gobj, f, w),
            5 => self.cmd_clear_hitbox(gobj, f, w),
            6 => self.cmd_clear_hitboxes(gobj, f, w),
            8 => self.cmd_smash_sound(gobj, f, w),
            9 => self.cmd_set_var(gobj, f, w),
            10 => self.cmd_throw_flag_3_4(gobj, f, w),
            11 => self.cmd_throw_flag_1(gobj, f, w),
            12 => self.cmd_throw_flag_2(gobj, f, w),
            13 => self.cmd_allow_interrupt(gobj, f, w),
            14 => self.cmd_throw_flag_0(gobj, f, w),
            15 => self.cmd_airborne(gobj, f, w),
            16 => self.cmd_hit_status(gobj, f, w),
            17 => self.cmd_hurtbox_state(gobj, f, w),
            18 => self.cmd_bone_hurtbox_state(gobj, f, w),
            19 => self.cmd_jab_combo(gobj, f, w),
            20 => self.cmd_jab_rapid(gobj, f, w),
            21 => self.cmd_model_variant(gobj, f, w),
            22 => self.cmd_restore_models(gobj, f, w),
            23 => self.cmd_hide_models(gobj, f, w),
            25 => self.cmd_item_visible(gobj, f, w),
            26 => self.cmd_article_visible(gobj, f, w),
            27 => self.cmd_fighter_visible(gobj, f, w),
            30 => self.cmd_texture_anim(gobj, f, w),
            31 => self.cmd_part_anim(gobj, w, true),
            33 => self.cmd_rumble(gobj, f, w),
            34 => self.cmd_stop_rumble(gobj, f, w),
            35 => self.cmd_sword(gobj, f, w),
            36 => self.cmd_color_anim(gobj, f, w),
            37 => self.cmd_end_color_anim(gobj, f, w),
            38 => self.cmd_x221e_b1(gobj, f, w),
            39 => self.cmd_sword_trail(gobj, f, w),
            42 => self.cmd_x221c_flags(gobj, f, w),
            43 => self.cmd_x2225_b2(gobj, f, w),
            46 => self.cmd_smash_charge(gobj, f, w),
            47 => self.cmd_vibrate(gobj, f, w),
            // Commands of more than one word step over them themselves.
            0 => return self.cmd_effect(gobj, f, None),
            1 => return self.cmd_create_hitbox(gobj, f),
            7 => return self.sound_command(f, None),
            28 => return self.cmd_random_sfx(gobj, f),
            24 => return self.cmd_throw_hitbox(gobj, f),
            44 => return self.cmd_floor_sound(gobj, f),
            45 => return self.cmd_footstep(gobj, f),
            _ => match COMMANDS.get(index as usize) {
                Some(name) => unported_call(name),
                None => panic!("fighter command {} doesn't exist", index + 10),
            },
        }
        self.cmd_skip(f, 1);
    }

    /// The hitbox command, `ftAction_8007121C`: hitbox `id` of a group, on a bone, with its
    /// damage, size, offset, angle, knockback, element, shield damage, sounds and whom it
    /// hits; an unused group's box starts fresh. A box that skips thrown fighters isn't made
    /// while the fighter throws none (`ftAction_800715EC` steps over it). Five words.
    #[doc(alias = "ftAction_8007121C")]
    fn cmd_create_hitbox(&mut self, gobj: GObjId, f: FighterId) {
        let w: [u32; 6] = std::array::from_fn(|i| self.cmd_word(f, i as u32));
        let thrown = self.game.fighters[f.0].x1064_thrownHitbox.x134.is_set();
        if bits(w[3], 28, 1) != 0 && !thrown {
            self.cmd_skip(f, 5);
            self.smash_since_hitbox(f);
            return;
        }
        let id = bits(w[0], 6, 3) as usize;
        let group = bits(w[0], 9, 3);
        let fp = &mut self.game.fighters[f.0];
        let h = &fp.x914[id];
        if h.state == hitcapsule::DISABLED || h.x4 != group {
            fp.x914[id].x4 = group;
            fp.x914[id].state = hitcapsule::ENABLED;
            fp.x2219_b3 = true;
            // `ftColl_800768A0`: a box of a group already out shares its victims.
            let src = (0..4).find(|&i| {
                i != id && fp.x914[i].state != hitcapsule::DISABLED && fp.x914[i].x4 == group
            });
            match src {
                Some(i) => {
                    let s = fp.x914[i].clone();
                    hitcapsule::copy_victims(&s, &mut fp.x914[id]);
                }
                None => hitcapsule::clear_victims(&mut fp.x914[id]),
            }
        }
        let bone = bits(w[0], 13, 8) as i32;
        let bone = if bits(w[0], 21, 1) != 0 {
            self.bone_index(&self.game.fighters[f.0], bone)
        } else {
            bone
        };
        let j = self.bone_joint(gobj, bone);
        self.game.fighters[f.0].x914[id].jobj = j;
        self.hitbox_damage(f, id, false, bits(w[0], 22, 10));
        let fixed = |v: f64| mem(fp::fmuls(reg(FIXED), v));
        let h = &mut self.game.fighters[f.0].x914[id];
        h.scale = fixed(ufloat(bits(w[1], 0, 16)));
        h.b_offset.x = fixed(sfloat(sbits(w[1], 16, 16)));
        h.b_offset.y = fixed(sfloat(sbits(w[2], 0, 16)));
        h.b_offset.z = fixed(sfloat(sbits(w[2], 16, 16)));
        self.hitbox_angle(gobj, f, id, false, bits(w[3], 0, 9) as i32);
        let h = &mut self.game.fighters[f.0].x914[id];
        h.x24 = bits(w[3], 9, 9);
        h.x28 = bits(w[3], 18, 9);
        h.x43_b0 = bits(w[3], 27, 1) != 0;
        h.x43_b1 = bits(w[3], 29, 1) != 0;
        h.x40_b0 = bits(w[3], 30, 1) != 0;
        h.x40_b1 = bits(w[3], 31, 1) != 0;
        h.x2C = bits(w[4], 0, 9);
        h.element = bits(w[4], 9, 5);
        h.x34 = sbits(w[4], 14, 8);
        h.sfx_severity = bits(w[4], 22, 3) as i32;
        h.sfx_kind = bits(w[4], 25, 5) as i32;
        h.x40_b2 = bits(w[4], 31, 1) != 0;
        h.x40_b3 = bits(w[4], 30, 1) != 0;
        h.x42_b5 = true;
        h.x42_b7 = true;
        h.x41_b4 = false;
        h.x41_b6 = false;
        h.x41_b5 = false;
        h.x42_b0 = false;
        h.x42_b4 = false;
        h.x41_b7 = false;
        // `x134.hit_grabbed_victim_only`, from the word after the command's five.
        if bits(w[5], 12, 1) != 0 {
            h.x134.set_grabbed_victim_only(true);
        } else {
            h.x134.set_grabbed_victim_only(false);
        }
        h.x42_b1 = true;
        h.x42_b2 = false;
        h.x43_b2 = false;
        self.cmd_skip(f, 5);
        let late = self
            .game
            .world
            .current_proc()
            .is_some_and(|p| self.game.world.procs[p.0].s_link > 9);
        if late {
            self.hitbox_follow(gobj, id);
        }
        self.smash_since_hitbox(f);
    }

    /// A hitbox's damage, `ftColl_8007ABD0`: scaled by the fighter's height and a released
    /// smash's charge, its hit count from that, then weakened by how stale the move is.
    #[doc(alias = "ftColl_8007ABD0")]
    fn hitbox_damage(&mut self, f: FighterId, id: usize, throw: bool, damage: u32) {
        let fp = &self.game.fighters[f.0];
        let mut dmg = fp::frsp(f64::from(damage));
        if fp.x34_scale.y != 1.0 {
            let p = super::common_ptr(self.game, self.data, super::SCALE_PARAMS)
                .expect("no fighter scale parameters");
            dmg = crate::game::it::owner::y_scaled(
                dmg,
                reg(fp.x34_scale.y),
                reg(self.data.f32(p.at(4))),
            );
        }
        let dmg = Self::smash_damage(&self.game.fighters[f.0], dmg);
        let fp = &self.game.fighters[f.0];
        let attack = fp.x2068_attackID;
        let stale = crate::game::it::owner::stale_damage(self.game, self.data, f, attack, dmg);
        let fp = &mut self.game.fighters[f.0];
        let h = if throw {
            &mut fp.xDF4[id]
        } else {
            &mut fp.x914[id]
        };
        h.unk_count = fp::fctiwz(dmg) as u32;
        h.damage = mem(stale);
    }

    /// A throw's hitbox, `ftAction_80071E04`: one of the throw's two (`xDF4`), its damage
    /// (`ftColl_8007ABD0`), angle (`ftColl_8007AC9C`), knockback growth, fixed and base
    /// knockback, element and sound. Three words.
    #[doc(alias = "ftAction_80071E04")]
    fn cmd_throw_hitbox(&mut self, gobj: GObjId, f: FighterId) {
        let w: [u32; 3] = std::array::from_fn(|i| self.cmd_word(f, i as u32));
        let id = bits(w[0], 6, 3) as usize;
        assert!(id < 2, "throw hitbox {id} of a command");
        self.hitbox_damage(f, id, true, bits(w[0], 9, 23));
        self.hitbox_angle(gobj, f, id, true, bits(w[1], 0, 9) as i32);
        let h = &mut self.game.fighters[f.0].xDF4[id];
        h.x24 = bits(w[1], 9, 9);
        h.x28 = bits(w[1], 18, 9);
        h.x2C = bits(w[2], 0, 9);
        h.element = bits(w[2], 9, 4);
        h.sfx_severity = bits(w[2], 13, 3) as i32;
        h.sfx_kind = bits(w[2], 16, 4) as i32;
        self.cmd_skip(f, 3);
    }

    /// A released smash attack's damage, `ftCo_800DEEB8`: more the longer it charged.
    #[doc(alias = "ftCo_800DEEB8")]
    pub fn smash_damage(fp: &super::super::structs::Fighter, damage: f64) -> f64 {
        let a = &fp.smash_attrs;
        if a.state != SMASH_RELEASE {
            return damage;
        }
        fp::fmuls(
            damage,
            fp::fmadds(
                fp::fsubs(reg(a.x2120_damageMul), 1.0),
                fp::fdivs(reg(a.x2118_frames), reg(a.x211C_holdFrame)),
                1.0,
            ),
        )
    }

    /// A hitbox's angle, `ftColl_8007AC9C`: a spike's angle (`ftColl_8007AC68`) counts once
    /// in the player's statistics (`pl_80037ECC`).
    #[doc(alias = "ftColl_8007AC9C")]
    #[doc(alias = "ftColl_8007AC68")]
    #[doc(alias = "pl_80037ECC")]
    fn hitbox_angle(&mut self, _gobj: GObjId, f: FighterId, id: usize, throw: bool, angle: i32) {
        let fp = &mut self.game.fighters[f.0];
        if throw {
            fp.xDF4[id].kb_angle = angle;
        } else {
            fp.x914[id].kb_angle = angle;
        }
        let a = angle as u32;
        let c = self.common();
        let (lo, hi) = (c.u32(0x7E8), c.u32(0x7EC));
        if a != 361 && lo <= a && a <= hi {
            let fp = &mut self.game.fighters[f.0];
            if fp.x2074.x10[0] & 0x80 == 0 {
                fp.x2074.x10[0] |= 0x80;
                let slot = i32::from(fp.player_idx);
                let stats = self.game.player_action_stats(slot);
                stats.attacks.x1A8 = stats.attacks.x1A8.wrapping_add(1);
            }
        }
    }

    /// `ftCommon_80080484`: a smash attack's charge counts from its first hitbox.
    #[doc(alias = "ftCommon_80080484")]
    fn smash_since_hitbox(&mut self, f: FighterId) {
        let a = &mut self.game.fighters[f.0].smash_attrs;
        if a.x2138_smashSinceHitbox == -1.0 {
            a.x2138_smashSinceHitbox = 0.0;
        }
    }

    /// A hitbox's size.
    fn cmd_hitbox_scale(&mut self, _gobj: GObjId, f: FighterId, w: u32) {
        let v = mem(fp::fmuls(reg(FIXED), ufloat(bits(w, 9, 23))));
        self.cmd_hitbox(f, bits(w, 6, 3)).scale = v;
    }

    /// A hitbox's flags (`x42_b5`, `x42_b7`).
    fn cmd_hitbox_flags(&mut self, _gobj: GObjId, f: FighterId, w: u32) {
        let (ty, v) = (bits(w, 30, 1), bits(w, 31, 1) != 0);
        let h = self.cmd_hitbox(f, bits(w, 6, 24));
        match ty {
            0 => h.x42_b5 = v,
            _ => h.x42_b7 = v,
        }
    }

    /// Clears one hitbox (`ftColl_8007AFC8`).
    fn cmd_clear_hitbox(&mut self, _gobj: GObjId, f: FighterId, w: u32) {
        self.cmd_hitbox(f, bits(w, 6, 26)).state = 0;
    }

    /// Clears every hitbox.
    fn cmd_clear_hitboxes(&mut self, _gobj: GObjId, f: FighterId, _w: u32) {
        self.clear_hitboxes(f);
    }

    /// One of the character's smash sounds, at random (`ft_800889F4`).
    fn cmd_smash_sound(&mut self, _gobj: GObjId, f: FighterId, _w: u32) {
        let fp = &self.game.fighters[f.0];
        let sfx = self
            .data
            .ptr(fp.ft_data.expect("a fighter without data").at(0x4C))
            .expect("ftData's sounds");
        if let Some(smash) = self.data.ptr(sfx) {
            let n = self.data.u32(smash) as i32;
            let ids = self.data.ptr(smash.at(4)).expect("the smash sounds' ids");
            self.fighter_random_voice_in(f, ids, n);
        }
    }

    /// Sets one of `cmd_vars`.
    fn cmd_set_var(&mut self, _gobj: GObjId, f: FighterId, w: u32) {
        self.game.fighters[f.0].cmd_vars[bits(w, 6, 2) as usize] = bits(w, 8, 24);
    }

    /// Throw flag 3 (noting the timer) or 4.
    fn cmd_throw_flag_3_4(&mut self, _gobj: GObjId, f: FighterId, w: u32) {
        let fp = &mut self.game.fighters[f.0];
        match bits(w, 6, 26) {
            0 => {
                fp.x2210[0] |= 0x10;
                fp.cmd_timer = fp.x3E4_fighterCmdScript.script.timer_f32();
            }
            1 => fp.x2210[0] |= 0x08,
            _ => {}
        }
    }

    /// Throw flag 1.
    fn cmd_throw_flag_1(&mut self, _gobj: GObjId, f: FighterId, _w: u32) {
        self.game.fighters[f.0].x2210[0] |= 0x40;
    }

    /// Throw flag 2.
    fn cmd_throw_flag_2(&mut self, _gobj: GObjId, f: FighterId, _w: u32) {
        self.game.fighters[f.0].x2210[0] |= 0x20;
    }

    /// The action may be interrupted.
    fn cmd_allow_interrupt(&mut self, _gobj: GObjId, f: FighterId, _w: u32) {
        self.game.fighters[f.0].allow_interrupt = true;
    }

    /// Throw flag 0.
    fn cmd_throw_flag_0(&mut self, _gobj: GObjId, f: FighterId, _w: u32) {
        self.game.fighters[f.0].x2210[0] |= 0x80;
    }

    /// Lands, or leaves the ground as falling or as jumped.
    fn cmd_airborne(&mut self, gobj: GObjId, f: FighterId, w: u32) {
        match bits(w, 6, 26) {
            0 => self.land(gobj),
            1 => self.go_airborne(f),
            2 => self.go_airborne_jumped(f),
            _ => {}
        }
    }

    /// The fighter's hit status.
    fn cmd_hit_status(&mut self, gobj: GObjId, _f: FighterId, w: u32) {
        self.set_hit_status(gobj, bits(w, 6, 26) as i32);
    }

    /// Every hurtbox's state.
    fn cmd_hurtbox_state(&mut self, _gobj: GObjId, f: FighterId, w: u32) {
        self.set_hurtbox_state(f, bits(w, 6, 26) as i32);
    }

    /// One bone's hurtbox state.
    fn cmd_bone_hurtbox_state(&mut self, _gobj: GObjId, f: FighterId, w: u32) {
        self.set_bone_hurtbox_state(f, bits(w, 6, 8) as i32, bits(w, 14, 18) as i32);
    }

    /// The jab may continue into the next.
    fn cmd_jab_combo(&mut self, _gobj: GObjId, f: FighterId, w: u32) {
        let fp = &mut self.game.fighters[f.0];
        if bits(w, 6, 26) == 0 || fp.x197C.is_some() {
            fp.x2218_b1 = true;
        }
    }

    /// The jab may turn rapid.
    fn cmd_jab_rapid(&mut self, _gobj: GObjId, f: FighterId, w: u32) {
        self.game.fighters[f.0].x2218_b2 = bits(w, 6, 26) & 1 != 0;
    }

    /// Shows one of a model's variants.
    fn cmd_model_variant(&mut self, _gobj: GObjId, f: FighterId, w: u32) {
        let (i, v) = (sbits(w, 6, 7), sbits(w, 13, 19));
        self.set_model_variant(f, i as usize, v);
    }

    /// A model's variant chosen, `ftParts_80074B0C`: a new one marks the models changed and
    /// hides the first visibility group's parts until they are shown again.
    #[doc(alias = "ftParts_80074B0C")]
    fn set_model_variant(&mut self, f: FighterId, i: usize, v: i32) {
        let fp = &mut self.game.fighters[f.0];
        if v != i32::from(fp.x5F4_arr[i].idx) {
            fp.x5F4_arr[i].idx = v as i8;
            fp.x221D_b2 = true;
            let list = fp.dobj_list.data;
            self.hide_parts_vis(f, 0, list);
        }
    }

    /// Shows each model's previous variant.
    fn cmd_restore_models(&mut self, _gobj: GObjId, f: FighterId, _w: u32) {
        self.restore_models(f);
    }

    /// Hides every model variant.
    fn cmd_hide_models(&mut self, _gobj: GObjId, f: FighterId, _w: u32) {
        self.hide_models(f);
    }

    /// Shows or hides the held item (`ftCommon_8007F5CC`; with an item, not ported).
    #[doc(alias = "ftAction_80071F34")]
    fn cmd_item_visible(&mut self, _gobj: GObjId, f: FighterId, w: u32) {
        let v = bits(w, 6, 26) & 1 != 0;
        let fp = &self.game.fighters[f.0];
        if fp.item_gobj.is_some() && fp.x221E_b3 != v {
            unported_call("ftCommon_8007F5CC with an item");
        }
        self.game.fighters[f.0].x221E_b3 = v;
    }

    /// The character's articles' visibility (`x221E_b4`).
    fn cmd_article_visible(&mut self, _gobj: GObjId, f: FighterId, w: u32) {
        self.game.fighters[f.0].x221E_b4 = bits(w, 6, 26) & 1 != 0;
    }

    /// The fighter's visibility (`x221E_b5`).
    fn cmd_fighter_visible(&mut self, _gobj: GObjId, f: FighterId, w: u32) {
        self.game.fighters[f.0].x221E_b5 = bits(w, 6, 26) & 1 != 0;
    }

    /// One or two costume textures' animation frame.
    #[doc(alias = "ftAction_800726F4")]
    fn cmd_texture_anim(&mut self, gobj: GObjId, _f: FighterId, w: u32) {
        let frame = mem(sfloat(sbits(w, 21, 11)));
        self.texture_anim(gobj, sbits(w, 7, 7), frame);
        if bits(w, 6, 1) != 0 {
            self.texture_anim(gobj, sbits(w, 14, 7), frame);
        }
    }

    /// Plays a part animation (`ftAction_800727C8`), over the command's frames unless a
    /// state started partway in catches up on it (`ftAction_8007283C`, at once).
    #[doc(alias = "ftAction_800727C8")]
    #[doc(alias = "ftAction_8007283C")]
    fn cmd_part_anim(&mut self, gobj: GObjId, w: u32, blend: bool) {
        let frames = if blend {
            mem(fp::frsp(f64::from(bits(w, 20, 12))))
        } else {
            0.0
        };
        self.apply_part_anim(gobj, sbits(w, 6, 7), sbits(w, 13, 7), frames);
    }

    /// Rumbles the fighter's controller, or every fighter's (`ftCommon_8007EC30`).
    fn cmd_rumble(&mut self, _gobj: GObjId, f: FighterId, w: u32) {
        let (kind, frames) = (bits(w, 7, 12), bits(w, 19, 13));
        if bits(w, 6, 1) != 0 {
            let mut cur = self.game.world.first_in(FIGHTER_P_LINK);
            while let Some(g) = cur {
                cur = self.game.world.gobjs[g.0].next;
                let other = self.game.fighter_id(g);
                self.start_rumble(other, kind, frames);
            }
        } else {
            self.start_rumble(f, kind, frames);
        }
    }

    /// Stops one kind of the controller's rumble.
    fn cmd_stop_rumble(&mut self, _gobj: GObjId, f: FighterId, w: u32) {
        self.stop_rumble(f, bits(w, 6, 26) as i32);
    }

    /// A held sword's trail or flash (`ftCommon_8007EEC8`, `ftCommon_8007EF5C`; with one, not ported).
    fn cmd_sword(&mut self, _gobj: GObjId, f: FighterId, _w: u32) {
        if self.game.fighters[f.0].item_gobj.is_some() {
            unported_call("ftAction_800729D4 with an item");
        }
    }

    /// Starts a color animation.
    fn cmd_color_anim(&mut self, gobj: GObjId, _f: FighterId, w: u32) {
        self.start_color_anim(gobj, bits(w, 6, 8) as i32, bits(w, 14, 18) as i32);
    }

    /// Ends a color animation.
    fn cmd_end_color_anim(&mut self, gobj: GObjId, _f: FighterId, w: u32) {
        self.end_color_anim(gobj, bits(w, 6, 8) as i32);
    }

    /// Flag `x221E_b1`.
    fn cmd_x221e_b1(&mut self, _gobj: GObjId, f: FighterId, w: u32) {
        self.game.fighters[f.0].x221E_b1 = bits(w, 6, 26) & 1 != 0;
    }

    /// The sword trail's setting (`x2100`, `x2101_bits_8`).
    fn cmd_sword_trail(&mut self, _gobj: GObjId, f: FighterId, w: u32) {
        let fp = &mut self.game.fighters[f.0];
        fp.x2100 = sbits(w, 7, 25) as i8;
        fp.x2101_bits_8 = sbits(w, 6, 1) & 1 != 0;
    }

    /// Flags `x221C_u16_y`, the root's lean kept unless bit 2 (`ft_8008A1B8`).
    #[doc(alias = "ftAction_80072C6C")]
    fn cmd_x221c_flags(&mut self, _gobj: GObjId, f: FighterId, w: u32) {
        let flags = bits(w, 6, 26);
        self.game.fighters[f.0].x221C_u16_y = (flags & 7) as u8;
        if flags & 4 == 0 {
            self.game.part_set_rot_x(f, 0, 0.0);
        }
    }

    /// Flag `x2225_b2`.
    fn cmd_x2225_b2(&mut self, _gobj: GObjId, f: FighterId, w: u32) {
        self.game.fighters[f.0].x2225_b2 = bits(w, 6, 26) & 1 != 0;
    }

    /// A smash attack's charge window (`ftCo_800DEE84`): its frames, damage scale and color.
    fn cmd_smash_charge(&mut self, _gobj: GObjId, f: FighterId, w: u32) {
        let frames = mem(ufloat(bits(w, 6, 10)));
        let rate = mem(ufloat(bits(w, 16, 16)));
        let mul = mem(fp::fmuls(reg(FIXED), reg(rate)));
        self.cmd_skip(f, 1);
        let color = mem(ufloat(bits(self.cmd_word(f, 0), 0, 8)));
        let a = &mut self.game.fighters[f.0].smash_attrs;
        a.state = 1;
        a.x2118_frames = 0.0;
        a.x211C_holdFrame = frames;
        a.x2120_damageMul = mul;
        a.x2128 = fp::fctiwz(reg(color));
    }

    /// The fighter's vibration (`ftCo_800C8B60`).
    fn cmd_vibrate(&mut self, _gobj: GObjId, f: FighterId, w: u32) {
        let fp = &mut self.game.fighters[f.0];
        fp.x2226_b5 = bits(w, 6, 1) != 0;
        fp.smash_attrs.x2134_vibrateFrame = bits(w, 7, 8) as u8;
    }

    /// Runs fighter command `index` as a state started partway in does (`ftAction_803C07AC`):
    /// the commands that matter beyond their frame run; the rest are stepped over.
    pub(super) fn catch_up_command(&mut self, gobj: GObjId, index: u32) {
        let f = self.game.fighter_id(gobj);
        match index {
            0 => self.skip_800711dc(f),
            1 => self.skip_800715ec(f),
            2 => self.skip_8007168c(f),
            3 => self.skip_800716f8(f),
            4 => self.skip_80071774(f),
            5 => self.skip_800717c8(f),
            6 => self.skip_80071810(f),
            7 => self.skip_80071ca4(f),
            8 => self.skip_80071d30(f),
            24 => self.skip_80071f0c(f),
            28 => self.skip_800722c8(f),
            29 => self.skip_800726c0(f),
            33 => self.skip_8007296c(f),
            34 => self.skip_800729c4(f),
            35 => self.skip_80072a4c(f),
            36 => self.skip_80072aac(f),
            37 => self.skip_80072b04(f),
            39 => self.skip_80072b84(f),
            40 => self.skip_80072be4(f),
            41 => self.skip_80072c5c(f),
            44 => self.skip_80072e24(f),
            45 => self.skip_80072fe0(f),
            46 => self.skip_8007309c(f),
            47 => self.skip_80073108(f),
            48 => self.skip_8007320c(f),
            31 => {
                let w = self.cmd_word(f, 0);
                self.cmd_part_anim(gobj, w, false);
                self.cmd_skip(f, 1);
            }
            _ if index < 49 => self.fighter_command(gobj, index),
            _ => panic!("fighter command {} doesn't exist", index + 10),
        }
    }

    fn skip_800711dc(&mut self, f: FighterId) {
        self.cmd_skip(f, 5);
    }

    #[doc(alias = "ftAction_800715EC")]
    fn skip_800715ec(&mut self, f: FighterId) {
        self.cmd_skip(f, 5);
    }

    fn skip_8007168c(&mut self, f: FighterId) {
        self.cmd_skip(f, 1);
    }

    fn skip_800716f8(&mut self, f: FighterId) {
        self.cmd_skip(f, 1);
    }

    fn skip_80071774(&mut self, f: FighterId) {
        self.cmd_skip(f, 1);
    }

    fn skip_800717c8(&mut self, f: FighterId) {
        self.cmd_skip(f, 1);
    }

    fn skip_80071810(&mut self, f: FighterId) {
        self.cmd_skip(f, 1);
    }

    #[doc(alias = "ftAction_80071CA4")]
    fn skip_80071ca4(&mut self, f: FighterId) {
        self.cmd_skip(f, 3);
    }

    fn skip_80071d30(&mut self, f: FighterId) {
        self.cmd_skip(f, 1);
    }

    fn skip_80071f0c(&mut self, f: FighterId) {
        self.cmd_skip(f, 3);
    }

    fn skip_800722c8(&mut self, f: FighterId) {
        self.cmd_skip(f, 7);
    }

    fn skip_800726c0(&mut self, f: FighterId) {
        self.cmd_skip(f, 4);
    }

    #[doc(alias = "ftAction_8007296C")]
    fn skip_8007296c(&mut self, f: FighterId) {
        self.cmd_skip(f, 1);
    }

    fn skip_800729c4(&mut self, f: FighterId) {
        self.cmd_skip(f, 1);
    }

    fn skip_80072a4c(&mut self, f: FighterId) {
        self.cmd_skip(f, 1);
    }

    #[doc(alias = "ftAction_80072AAC")]
    fn skip_80072aac(&mut self, f: FighterId) {
        self.cmd_skip(f, 1);
    }

    fn skip_80072b04(&mut self, f: FighterId) {
        self.cmd_skip(f, 1);
    }

    fn skip_80072b84(&mut self, f: FighterId) {
        self.cmd_skip(f, 1);
    }

    fn skip_80072be4(&mut self, f: FighterId) {
        self.cmd_skip(f, 1);
    }

    fn skip_80072c5c(&mut self, f: FighterId) {
        self.cmd_skip(f, 1);
    }

    fn skip_80072e24(&mut self, f: FighterId) {
        self.cmd_skip(f, 3);
    }

    fn skip_80072fe0(&mut self, f: FighterId) {
        self.cmd_skip(f, 3);
    }

    fn skip_8007309c(&mut self, f: FighterId) {
        self.cmd_skip(f, 2);
    }

    fn skip_80073108(&mut self, f: FighterId) {
        self.cmd_skip(f, 1);
    }

    fn skip_8007320c(&mut self, f: FighterId) {
        self.cmd_skip(f, 4);
    }

    /// The sound command, `ftAction_80071B50`: a behavior, then (for a sound) its id and
    /// volume and panning.
    fn sound_command(&mut self, f: FighterId, which: Option<Overlay>) {
        // Three words, or two for a behavior that does nothing; a sound's id and volume in
        // the second and third.
        let w0 = self.cmd_word_in(f, which, 0);
        let behavior = bits(w0, 6, 8);
        let words = if matches!(behavior, 0..=6) {
            [
                w0,
                self.cmd_word_in(f, which, 1),
                self.cmd_word_in(f, which, 2),
            ]
        } else {
            [w0, 0, 0]
        };
        let len = if matches!(behavior, 0..=6 | 10..=15) {
            3
        } else {
            2
        };
        self.cmd_script(f, which).skip(len);
        self.play_sound_words(f, words);
    }

    /// The sound command of the action's script (`ftAction_80071B50`).
    pub(super) fn sound_command_main(&mut self, f: FighterId) {
        self.sound_command(f, None);
    }

    /// What a sound command's three words ask: a sound on the track the first names, or
    /// a track stopped.
    pub(super) fn play_sound_words(&mut self, f: FighterId, words: [u32; 3]) {
        let behavior = bits(words[0], 6, 8);
        match behavior {
            0..=6 => {
                let sfx = words[1] as i32;
                let w = words[2];
                let (vol, pan) = (bits(w, 16, 8) as u8, bits(w, 24, 8) as u8);
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
                    b => self.fighter_play_track(f, TRACKS[b as usize - 1], sfx, vol, pan),
                }
            }
            10..=15 => {
                let fp = &mut self.game.fighters[f.0];
                let (base, slot) = match behavior {
                    10 => (0x36, &mut fp.x214C),
                    11 => (0x1E, &mut fp.x2144),
                    12 => (0x42, &mut fp.x2150),
                    13 => (0x4E, &mut fp.x2154),
                    14 => (0x5A, &mut fp.x2158),
                    _ => (0x2A, &mut fp.x2148),
                };
                *slot = -1;
                self.key_off(f, base);
            }
            _ => {}
        }
    }

    /// The action's script (for the commands ported elsewhere).
    pub(super) fn cmd_script_main(&mut self, f: FighterId) -> &mut Script {
        self.cmd_script(f, None)
    }

    /// Word `n` of the action script's current command.
    pub(super) fn cmd_word_main(&self, f: FighterId, n: u32) -> u32 {
        self.cmd_word(f, n)
    }

    /// Runs a fighter command from a color overlay's script (opcodes 21 to 23): `ft_800BFF34`'s
    /// effect, sound and vibration commands (`ftCo_803C6AD0`), or with `alt` `ft_800BFF70`'s,
    /// which step over them (`ftCo_803C6ADC`).
    pub(super) fn overlay_command(&mut self, gobj: GObjId, which: Overlay, i: u32, alt: bool) {
        let f = self.game.fighter_id(gobj);
        match (alt, i) {
            (false, 0) => self.cmd_effect(gobj, f, Some(which)),
            (false, 1) => self.sound_command(f, Some(which)),
            (false, 2) => {
                // `ftAction_800730B8`.
                let w = self.cmd_word_in(f, Some(which), 0);
                let fp = &mut self.game.fighters[f.0];
                fp.x2226_b5 = bits(w, 6, 1) != 0;
                fp.smash_attrs.x2134_vibrateFrame = bits(w, 7, 8) as u8;
                self.cmd_script(f, Some(which)).skip(1);
            }
            (true, 0) => self.cmd_script(f, Some(which)).skip(5),
            (true, 1) => self.cmd_script(f, Some(which)).skip(3),
            (true, 2) => self.cmd_script(f, Some(which)).skip(1),
            _ => panic!("color overlay command {} doesn't exist", i + 21),
        }
    }

    /// Starts the fighter's rumble of kind `kind` for `frames` (0: the pattern's own) if its
    /// controller rumbles and the fighter is awake and alive, `ftCommon_8007EBAC`.
    #[doc(alias = "ftCommon_8007EBAC")]
    pub(super) fn start_rumble(&mut self, f: FighterId, kind: u32, frames: u32) {
        let fp = &self.game.fighters[f.0];
        if self
            .game
            .rumble_enabled(i32::from(fp.player_idx), i32::from(fp.is_sub_fighter))
            && !fp.is_sleeping
            && !fp.stamina_dead
        {
            let port = fp.pad_port;
            self.rumble_start(port, kind as i32 + 2, kind as i32, frames as i32);
        }
    }

    /// Starts rumble pattern `kind` on `port` with `id`, for `frames` (0: the pattern's own),
    /// with the priority `LbRb.dat`'s table gives it, `lb_80014574`. The pad library's rumble
    /// is simulation state; its motors reach the frontend through the poll's requests.
    #[doc(alias = "lb_80014574")]
    pub fn rumble_start(&mut self, port: u8, id: i32, kind: i32, frames: i32) {
        use crate::input::rumble::Script;
        let table = self.game.rumble_data.expect("LbRb.dat's lbRumbleData");
        let entry = table.at(8 * kind as u32);
        let script = self.data.ptr(entry).expect("a rumble pattern's script");
        let pri = self.data.u8(entry.at(4));
        let frame = if frames != 0 { frames } else { -2 };
        self.game
            .pads
            .rumble
            .add(port, id as u32, frame, pri, Script::Data(script));
    }

    /// Takes the fighter into the air as falling off something, `ftCommon_8007D5D4`.
    #[doc(alias = "ftCommon_8007D5D4")]
    pub(crate) fn go_airborne(&mut self, f: FighterId) {
        let fp = &mut self.game.fighters[f.0];
        fp.ground_or_air = GA_AIR;
        fp.gr_vel = 0.0;
        fp.x98_atk_shield_kb.z = 0.0;
        fp.cur_pos.z = 0.0;
        fp.x74_self_accel.y = 0.0;
        fp.x1968_jumpsUsed = 1;
        fp.ecb_lock = 10;
        fp.coll_data.x130_flags |= COLL_LOCKED;
    }

    /// Takes the fighter into the air with its jumps used, `ftCommon_8007D60C`.
    #[doc(alias = "ftCommon_8007D60C")]
    pub(super) fn go_airborne_jumped(&mut self, f: FighterId) {
        let fp = &self.game.fighters[f.0];
        if fp.x2227_b0 && fp.x1968_jumpsUsed <= 1 {
            let (slot, sub) = (i32::from(fp.player_idx), i32::from(fp.is_sub_fighter));
            self.game.record_jump_left(slot, sub);
        }
        let fp = &mut self.game.fighters[f.0];
        fp.ground_or_air = GA_AIR;
        fp.gr_vel = 0.0;
        fp.x74_self_accel.y = 0.0;
        fp.x1968_jumpsUsed = fp.co_attrs.max_jumps as u8;
        fp.ecb_lock = 5;
        fp.coll_data.x130_flags |= COLL_LOCKED;
    }

    /// Sets the state of the hurtbox on bone `bone`, `ftColl_8007B128`.
    fn set_bone_hurtbox_state(&mut self, f: FighterId, bone: i32, state: i32) {
        let fp = &mut self.game.fighters[f.0];
        for i in 0..fp.hurt_capsules_len as usize {
            if fp.hurt_capsules[i].capsule.bone_idx == bone {
                fp.hurt_capsules[i].capsule.state = state;
                if state != 0 {
                    fp.x221A_b5 = true;
                }
                return;
            }
        }
        panic!("fighter hurtbox on bone {bone} doesn't exist");
    }

    /// Hides every model variant, `ftParts_80074ACC`.
    fn hide_models(&mut self, f: FighterId) {
        let fp = &mut self.game.fighters[f.0];
        for i in 0..fp.x5AC.model_num as usize {
            fp.x5F4_arr[i].idx = -1;
        }
        fp.x221D_b2 = false;
    }

    /// Sets a costume texture's animation to `frame` and steps it, `ftAnim_800704F0`
    /// (`ftAnim_80070458`); Kirby's copied ability follows (not ported).
    #[doc(alias = "ftAnim_800704F0")]
    #[doc(alias = "ftAnim_80070458")]
    pub(crate) fn texture_anim(&mut self, gobj: GObjId, i: i32, frame: f32) {
        let f = self.game.fighter_id(gobj);
        let list = &self.game.fighters[f.0].tobj_list;
        assert!(
            (i as u32) < list.n_costume_tobjs,
            "texture no exist! {} {i}",
            self.game.fighters[f.0].player_idx
        );
        let t = list.costume_tobjs[i as usize].expect("a costume without its texture");
        let a = self.game.objs.tobjs[t].aobj;
        let objs = &mut self.game.objs;
        aobj::req_anim(&mut objs.aobjs, &mut objs.fobjs, a, frame);
        tobj::anim(objs, self.data, Some(t));
        if self.game.fighters[f.0].kind == 4 {
            unported_call("ftKb_Init_UnkCallbackPairs0_1");
        }
        self.game.fighters[f.0].x221E_b7 = true;
    }
}

/// `CollData_X130_Locked`: the ECB keeps its shape.
const COLL_LOCKED: u32 = 1 << 4;
