//! The fighter's own action-script commands (`ftAction_803C06E8`, opcodes 10 on): hitboxes,
//! flags the action's callbacks read, color and texture animations, rumble and sounds.
//! Commands that spawn effects or play sounds, or reach items, aren't ported.

use gekko_fp as fp;

use super::super::code::unported_call;
use super::super::handles::FighterId;
use super::super::state::{Event, Running};
use super::colanim::Overlay;
use super::common::{FIGHTER_P_LINK, GA_AIR};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::{aobj, tobj};
use crate::lb::command::Script;

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

impl Running<'_> {
    /// The script a command runs in: the action's (`x3E4_fighterCmdScript`) or a color
    /// overlay's.
    fn cmd_script(&mut self, f: FighterId, which: Option<Overlay>) -> &mut Script {
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

    fn cmd_word_in(&self, f: FighterId, which: Option<Overlay>, n: u32) -> u32 {
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
            7 => return self.sound_command(f, None),
            _ => match COMMANDS.get(index as usize) {
                Some(name) => unported_call(name),
                None => panic!("fighter command {} doesn't exist", index + 10),
            },
        }
        self.cmd_skip(f, 1);
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

    /// One of the character's smash sounds, at random (`ft_800889F4`, not ported).
    fn cmd_smash_sound(&mut self, _gobj: GObjId, f: FighterId, _w: u32) {
        let fp = &self.game.fighters[f.0];
        let sfx = self
            .data
            .ptr(fp.ft_data.expect("a fighter without data").at(0x4C))
            .expect("ftData's sounds");
        if self.data.ptr(sfx).is_some() {
            unported_call("ft_800889F4");
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
    fn cmd_airborne(&mut self, _gobj: GObjId, f: FighterId, w: u32) {
        match bits(w, 6, 26) {
            0 => unported_call("ftCommon_8007D7FC"),
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

    /// Shows one of a model's variants (`ftParts_80074B0C`; changing it isn't ported).
    fn cmd_model_variant(&mut self, _gobj: GObjId, f: FighterId, w: u32) {
        let (i, v) = (sbits(w, 6, 7), sbits(w, 13, 19));
        if v != i32::from(self.game.fighters[f.0].x5F4_arr[i as usize].idx) {
            unported_call("ftParts_80074D7C");
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
            31 => unported_call("ftAction_8007283C"),
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
    /// volume and panning. Stopping one of the fighter's tracks is ported; playing isn't.
    fn sound_command(&mut self, f: FighterId, which: Option<Overlay>) {
        let behavior = bits(self.cmd_word_in(f, which, 0), 6, 8);
        self.cmd_script(f, which).skip(1);
        match behavior {
            0..=6 => {
                self.cmd_script(f, which).skip(1);
                const PLAY: [&str; 7] = [
                    "ft_PlaySFX",
                    "ft_80088478",
                    "ft_800881D8",
                    "ft_80088510",
                    "ft_800885A8",
                    "ft_80088640",
                    "ft_80088328",
                ];
                unported_call(PLAY[behavior as usize]);
            }
            10..=15 => {
                self.cmd_script(f, which).skip(1);
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
        self.cmd_script(f, which).skip(1);
    }

    /// Runs a fighter command from a color overlay's script (opcodes 21 to 23): `ft_800BFF34`'s
    /// effect, sound and vibration commands (`ftCo_803C6AD0`), or with `alt` `ft_800BFF70`'s,
    /// which step over them (`ftCo_803C6ADC`).
    pub(super) fn overlay_command(&mut self, gobj: GObjId, which: Overlay, i: u32, alt: bool) {
        let f = self.game.fighter_id(gobj);
        match (alt, i) {
            (false, 0) => unported_call("ftAction_80071028"),
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
    fn start_rumble(&mut self, f: FighterId, kind: u32, frames: u32) {
        let fp = &self.game.fighters[f.0];
        if self
            .game
            .rumble_enabled(i32::from(fp.player_idx), i32::from(fp.is_sub_fighter))
            && !fp.is_sleeping
            && !fp.stamina_dead
        {
            self.game.events.push(Event::RumbleAdd {
                port: fp.pad_port,
                id: kind as i32 + 2,
                kind: kind as i32,
                frames: if frames != 0 { frames as i32 } else { -2 },
            });
        }
    }

    /// Takes the fighter into the air as falling off something, `ftCommon_8007D5D4`.
    #[doc(alias = "ftCommon_8007D5D4")]
    pub(super) fn go_airborne(&mut self, f: FighterId) {
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
    fn go_airborne_jumped(&mut self, f: FighterId) {
        let fp = &self.game.fighters[f.0];
        if fp.x2227_b0 && fp.x1968_jumpsUsed <= 1 {
            unported_call("pl_8003FC44");
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
    fn texture_anim(&mut self, gobj: GObjId, i: i32, frame: f32) {
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
