//! The players' damage displays (`melee/if/ifstatus.c`): each player's percentage as three
//! digit joints and a percent sign (texture-animated to their digits, coloured by damage,
//! shaken by hits, blown apart when the player is KO'd) with a character mark beside it.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::gm::vs::{hud_color, player_color, stock_icon_frame};
use super::super::handles::CodeRef;
use super::super::pl::player::{PKIND_CPU, PKIND_HUMAN, PKIND_NONE};
use super::super::state::{Game, OBJ_JOBJ, Obj, Running};
use super::super::structs::IfDamageState;
use super::{Draw, HIDDEN};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::{Objects, aobj, tobj};
use crate::lb::anim;

/// The damage display's processes and render callbacks.
pub const PROC_DAMAGE: u32 = 0x802F_5B48;
pub const PROC_DIGITS: u32 = 0x802F_4EDC;
pub const RENDER_DIGITS: u32 = 0x802F_5DE0;
pub const RENDER_MARK: u32 = 0x802F_5E50;

/// The joints of a display: `IfDamagePlace`.
const HUNDREDS: usize = 0;
const TENS: usize = 1;
const ONES: usize = 2;
const PERCENT: usize = 3;

/// The colours a percentage fades between (`ifStatus_804D57A8`, `ifStatus_804D57AC`).
const LOW: [u8; 3] = [0xFF, 0xFF, 0xFF];
const HIGH: [u8; 3] = [0x50, 0x00, 0x00];

/// The digit spacing a "1" saves.
const ONE_NARROWING: f64 = 0.506_900_012_493_133_5;

/// The texture animation of a damage number model's material animation joint, its first
/// child and `nexts` siblings on (`matanims[0]->child->next...->matanim->texanim`).
fn digit_texanim(g: &Game, data: &melee_data::archive::Archives, nexts: u32) -> Option<DataRef> {
    let list = g.hud.status.damage_num_matanims?;
    let mut j = data.ptr(data.ptr(list)?)?;
    for _ in 0..nexts {
        j = data.ptr(j.at(4))?;
    }
    let matanim = data.ptr(j.at(8))?;
    data.ptr(matanim.at(8))
}

/// A joint's first display object's material's first texture.
fn joint_tobj(objs: &Objects, j: JObjId) -> Option<crate::hsd::tobj::TObjId> {
    let d = objs.jobjs[j].dobj().expect("jobj->u.dobj");
    let m = objs.dobjs[d].mobj.expect("dobj->mobj");
    objs.mobjs[m].tobj
}

/// Sets a joint's first material's diffuse colour.
fn set_diffuse(objs: &mut Objects, j: JObjId, c: [u8; 3]) {
    let d = objs.jobjs[j].dobj().expect("jobj->u.dobj");
    let m = objs.dobjs[d].mobj.expect("dobj->mobj");
    let mat = objs.mobjs[m].mat.as_mut().expect("mobj->mat");
    mat.diffuse.r = c[0];
    mat.diffuse.g = c[1];
    mat.diffuse.b = c[2];
}

impl Game {
    /// `ifStatus_GetHUDInfo`: a player's damage display state.
    #[doc(alias = "ifStatus_GetHUDInfo")]
    pub fn damage_display(&self, i: usize) -> &IfDamageState {
        &self.hud.status.players[i]
    }

    /// The display whose digits' GObj is `gobj`.
    fn display_of(&self, gobj: GObjId) -> Option<usize> {
        (0..6).find(|&i| self.hud.status.players[i].HUD_parent_entity == Some(gobj))
    }

    /// Hides every display's digits, `ifStatus_802F6898`.
    #[doc(alias = "ifStatus_802F6898")]
    pub fn hide_damage_digits(&mut self) {
        for p in &mut self.hud.status.players {
            p.flags.hide_all_digits = true;
        }
    }

    /// Shows them, `ifStatus_802F68F0`.
    #[doc(alias = "ifStatus_802F68F0")]
    pub fn show_damage_digits(&mut self) {
        for p in &mut self.hud.status.players {
            p.flags.hide_all_digits = false;
        }
    }

    /// Blows a display's digits apart (once), the KO's shatter, `ifStatus_802F6948`; with
    /// the rules' `x2_7`, the display then goes after 200 frames.
    #[doc(alias = "ifStatus_802F6948")]
    pub fn explode_damage_display(&mut self, slot: i32) {
        let fade = self.vs.start.x2_7;
        let p = &mut self.hud.status.players[slot as usize];
        if !p.flags.explode_animation {
            p.flags.explode_animation = true;
            p.flags.randomize_velocity = true;
            if fade {
                p.unk9 = 1;
            }
        }
    }

    /// The digits' render callback, `ifStatus_802F5DE0`: drawn unless hidden.
    #[doc(alias = "ifStatus_802F5DE0")]
    pub fn damage_digits_render(&self, gobj: GObjId, pass: i32) -> Option<Draw> {
        let i = self.display_of(gobj).expect("a damage display's GObj");
        (!self.hud.status.players[i].flags.hide_all_digits).then_some(Draw::JObjs(gobj, pass))
    }

    /// The mark's render callback, `ifStatus_802F5E50`.
    #[doc(alias = "ifStatus_802F5E50")]
    pub fn damage_mark_render(&self, gobj: GObjId, pass: i32) -> Option<Draw> {
        let i = (0..6)
            .find(|&i| self.hud.status.players[i].next == Some(gobj))
            .expect("a damage mark's GObj");
        (!self.hud.status.players[i].flags.hide_all_digits).then_some(Draw::JObjs(gobj, pass))
    }
}

/// The child `n` siblings into `node`'s children, `ifStatus_802F6194`.
#[doc(alias = "ifStatus_802F6194")]
pub fn nth_child(jobjs: &jobj::JObjs, node: Option<JObjId>, n: i32) -> Option<JObjId> {
    let node = node?;
    if n < 0 {
        return None;
    }
    let mut cur = jobjs[node].child;
    let mut i = 0;
    while i < n
        && let Some(c) = cur
    {
        cur = jobjs[c].next;
        i += 1;
    }
    cur
}

impl Running<'_> {
    /// A digit joint's texture set to `digit` and held there.
    fn set_digit(&mut self, j: Option<JObjId>, texanim: Option<DataRef>, digit: i32) {
        let objs = &mut self.game.objs;
        let t = joint_tobj(objs, j.expect("a digit joint"));
        tobj::add_anim_all(objs, self.data, t, texanim);
        // The original keeps the digit in a `u8`.
        let digit = digit as u8;
        tobj::req_anim_all(objs, t, mem(fp::fmuls(2.0, fp::frsp(f64::from(digit)))));
        let a = t.and_then(|t| objs.tobjs[t].aobj);
        aobj::set_rate(&mut objs.aobjs, a, 0.0);
    }

    /// The percent sign's texture: HP's or percent's (the inline `ifStatus_InitPercentSign`).
    fn init_percent_sign(&mut self, i: usize) {
        let Some(j) = self.game.hud.status.players[i].jobjs[PERCENT] else {
            return;
        };
        let texanim = digit_texanim(self.game, self.data, 3);
        let stamina = self
            .game
            .player_more_flags_bit2(i32::from(self.game.hud.status.players[i].player_slot as i8));
        let objs = &mut self.game.objs;
        let t = joint_tobj(objs, j);
        tobj::add_anim_all(objs, self.data, t, texanim);
        tobj::req_anim_all(objs, t, if stamina { 1.0 } else { 0.0 });
        let a = t.and_then(|t| objs.tobjs[t].aobj);
        aobj::set_rate(&mut objs.aobjs, a, 0.0);
        tobj::anim(objs, self.data, t);
    }

    /// The three digits' textures for the display's percentage.
    fn set_digits(&mut self, i: usize, hide_tens: bool) {
        let texanim = digit_texanim(self.game, self.data, 0);
        let p = &self.game.hud.status.players[i];
        let d = i32::from(p.damage_percent);
        let js = p.jobjs;
        self.set_digit(js[ONES], texanim, d % 10);
        self.set_digit(js[TENS], texanim, d % 100 / 10);
        if hide_tens {
            let jobjs = &mut self.game.objs.jobjs;
            if d % 1000 / 100 == 0 && d % 100 / 10 == 0 {
                jobj::set_flags_all(jobjs, js[TENS], HIDDEN);
            } else {
                jobj::clear_flags_all(jobjs, js[TENS], HIDDEN);
            }
        }
        self.set_digit(js[HUNDREDS], texanim, d % 1000 / 100);
    }

    /// A blown-apart display's digits fly, the first frame picking their velocities,
    /// `ifStatus_PercentOnDeathAnimationThink`.
    #[doc(alias = "ifStatus_PercentOnDeathAnimationThink")]
    pub fn damage_digits_fly(&mut self, i: usize) {
        let g = &mut *self.game;
        if g.hud.status.players[i].flags.randomize_velocity {
            for k in 0..4 {
                let r = reg(g.objs.rng.draw_f32());
                let a = fp::fmuls(0.608_299_970_626_831, r);
                let b = 0.304_100_006_818_771_36;
                let x = if g.hud.status_flip != 0 {
                    g.hud.status_flip = 0;
                    fp::fsubs(fp::fneg(a), b)
                } else {
                    g.hud.status_flip = 1;
                    fp::fadds(a, b)
                };
                let r = reg(g.objs.rng.draw_f32());
                let p = &mut g.hud.status.players[i];
                p.velocity_x[k] = mem(x);
                p.velocity_y[k] = mem(fp::fmadds(
                    0.810_999_989_509_582_5,
                    r,
                    1.216_500_043_869_018_6,
                ));
            }
            g.hud.status.players[i].flags.randomize_velocity = false;
            return;
        }
        for k in 0..4 {
            let p = &g.hud.status.players[i];
            let j = p.jobjs[k].expect("a digit joint");
            let (vx, vy) = (p.velocity_x[k], p.velocity_y[k]);
            let jobjs = &mut g.objs.jobjs;
            let tx = jobjs[j].translate.x;
            if f32::from_bits(tx.to_bits() & !0x8000_0000) < 100.0 {
                jobj::set_srt(jobjs, j, |o| {
                    o.translate.x = mem(fp::fadds(reg(o.translate.x), reg(vx)))
                });
            }
            if reg(jobjs[j].translate.y) > -100.0 {
                jobj::set_srt(jobjs, j, |o| {
                    o.translate.y = mem(fp::fadds(reg(o.translate.y), reg(vy)))
                });
                let p = &mut g.hud.status.players[i];
                p.velocity_y[k] = mem(fp::fsubs(reg(vy), 0.202_800_005_674_362_18));
            }
        }
    }

    /// Shakes a display's digits about their places while its shake lasts, harder for a
    /// bigger hit, `ifStatus_802F4B84`.
    #[doc(alias = "ifStatus_802F4B84")]
    pub fn shake_damage_digits(&mut self, i: usize) {
        let g = &mut *self.game;
        let frames = g.hud.status.players[i].frames_of_shake_remaining;
        if frames == 0 {
            return;
        }
        if frames == 1 {
            for k in 0..4 {
                let p = &g.hud.status.players[i];
                let (j, x, y) = (
                    p.jobjs[k].expect("a digit joint"),
                    p.translation_x[k],
                    p.translation_y[k],
                );
                jobj::set_srt(&mut g.objs.jobjs, j, |o| o.translate.x = x);
                jobj::set_srt(&mut g.objs.jobjs, j, |o| o.translate.y = y);
            }
            g.hud.status.players[i].frames_of_shake_remaining = 0;
            return;
        }
        let hit = g.hud.status.players[i].damage_from_last_attack;
        let mut mag = fp::fmuls(0.101_400_002_837_181_09, fp::frsp(f64::from(hit)));
        if mag < 0.101_400_002_837_181_09 {
            mag = 0.101_400_002_837_181_09;
        } else if mag > 1.520_699_977_874_755_9 {
            mag = 1.520_699_977_874_755_9;
        }
        for k in 0..4 {
            let r = reg(g.objs.rng.draw_f32());
            let mut ox = fp::fmuls(mag, fp::fmuls(2.0, fp::fsubs(r, 0.5)));
            let r = reg(g.objs.rng.draw_f32());
            let mut oy = fp::fmuls(mag, fp::fmuls(2.0, fp::fsubs(r, 0.5)));
            let away = 0.202_800_005_674_362_18;
            ox = if ox < 0.0 {
                fp::fsubs(ox, away)
            } else {
                fp::fadds(ox, away)
            };
            oy = if oy < 0.0 {
                fp::fsubs(oy, away)
            } else {
                fp::fadds(oy, away)
            };
            let p = &g.hud.status.players[i];
            let j = p.jobjs[k].expect("a digit joint");
            let x = mem(fp::fadds(reg(p.translation_x[k]), ox));
            let y = mem(fp::fadds(reg(p.translation_y[k]), oy));
            jobj::set_srt(&mut g.objs.jobjs, j, |o| o.translate.x = x);
            jobj::set_srt(&mut g.objs.jobjs, j, |o| o.translate.y = y);
        }
        let p = &mut g.hud.status.players[i];
        p.frames_of_shake_remaining = p.frames_of_shake_remaining.wrapping_sub(1);
    }

    /// The hundreds digit hidden below 100, the digits' colour from the damage (HP fades
    /// the other way), and while the damage animation plays, the digits' places kept (the
    /// inline `ifStatus_UpdateDamageDisplay`).
    fn update_damage_display(&mut self, i: usize, root: JObjId) {
        let g = &mut *self.game;
        let p = &g.hud.status.players[i];
        let d = p.damage_percent;
        let js = p.jobjs;
        if i32::from(d) % 1000 / 100 == 0 {
            jobj::set_flags_all(&mut g.objs.jobjs, js[HUNDREDS], HIDDEN);
        } else {
            jobj::clear_flags_all(&mut g.objs.jobjs, js[HUNDREDS], HIDDEN);
        }
        if p.old_damage != d {
            let stamina = g.player_more_flags_bit2(i32::from(p.player_slot as i8));
            let factor = if stamina {
                let c = d.clamp(0, 100);
                fp::fsubs(1.0, fp::fdivs(fp::frsp(f64::from(c)), 100.0))
            } else {
                let c = d.clamp(0, 300);
                fp::fdivs(fp::frsp(f64::from(c)), 300.0)
            };
            let ch = |k: usize| {
                let span = fp::frsp(f64::from(i32::from(HIGH[k]) - i32::from(LOW[k])));
                fp::fctiwz(fp::fmadds(factor, span, fp::frsp(f64::from(LOW[k])))) as i8 as u8
            };
            let c = [ch(0), ch(1), ch(2)];
            for k in [HUNDREDS, TENS, ONES, PERCENT] {
                set_diffuse(&mut g.objs, js[k].expect("a digit joint"), c);
            }
        }
        if anim::any_animated(&g.objs, Some(root)) {
            for k in 0..4 {
                let j = js[k].expect("a digit joint");
                let t = g.objs.jobjs[j].translate;
                let p = &mut g.hud.status.players[i];
                p.translation_x[k] = t.x;
                p.translation_y[k] = t.y;
            }
        }
    }

    /// The digits' process, `ifStatus_802F4EDC`: a blown-apart display flies; otherwise a
    /// fresh hit restarts the damage animation, the digits follow the percentage, a "1"
    /// narrows its place, and a hit shakes them. Five or more players squeeze the display.
    #[doc(alias = "ifStatus_802F4EDC")]
    pub fn damage_digits_proc(&mut self, gobj: GObjId) {
        let i = self
            .game
            .display_of(gobj)
            .expect("the original reads a null display");
        let Some(Obj::JObj(root)) = self.game.world.gobj(gobj).obj else {
            panic!("a damage display without its joints");
        };
        if self.game.hud.status.players[i].flags.explode_animation {
            self.damage_digits_fly(i);
            return;
        }
        let animated = anim::any_animated(&self.game.objs, Some(root));
        let p = &self.game.hud.status.players[i];
        if p.flags.unk10 && (p.flags.animation_status_id != 1 || !animated) {
            self.game.hud.status.players[i].flags.animation_status_id = 1;
            let s = &self.game.hud.status;
            let (an, mat, shape) = (
                s.damage_num_anims,
                s.damage_num_matanims,
                s.damage_num_shapeanims,
            );
            let objs = &mut self.game.objs;
            jobj::remove_anim(objs, Some(root));
            anim::add_anim_index(objs, self.data, Some(root), 1, an, mat, shape);
            jobj::req_anim_all(objs, Some(root), 0.0);
            self.init_percent_sign(i);
            stop_anim_type(&mut self.game.objs, self.data, root, 1, 0x400);
            self.set_digits(i, false);
        }
        jobj::anim_all(
            &mut self.game.objs,
            self.data,
            &mut crate::game::code::ParticleHooks,
            Some(root),
        );
        self.set_digits(i, true);
        self.update_damage_display(i, root);
        let g = &mut *self.game;
        let p = &g.hud.status.players[i];
        let d = i32::from(p.damage_percent);
        let narrow = |c: bool| if c { ONE_NARROWING } else { 0.0 };
        let ones = narrow(d % 10 == 1);
        let tens = narrow(d % 100 / 10 == 1);
        let js = p.jobjs;
        let (tx_pct, tx_tens, tx_hund) = (
            p.translation_x[PERCENT],
            p.translation_x[TENS],
            p.translation_x[HUNDREDS],
        );
        let jobjs = &mut g.objs.jobjs;
        let x = mem(fp::fsubs(reg(tx_pct), ones));
        jobj::set_srt(jobjs, js[PERCENT].expect("a digit"), |o| o.translate.x = x);
        let mut off = mem(ones);
        off = mem(fp::fadds(reg(off), tens));
        let x = mem(fp::fadds(reg(tx_tens), reg(off)));
        jobj::set_srt(jobjs, js[TENS].expect("a digit"), |o| o.translate.x = x);
        let hundreds = narrow(d % 1000 / 100 == 1);
        off = mem(fp::fadds(reg(off), fp::fadds(tens, hundreds)));
        let x = mem(fp::fadds(reg(tx_hund), reg(off)));
        jobj::set_srt(jobjs, js[HUNDREDS].expect("a digit"), |o| o.translate.x = x);
        if p.flags.force_digit_shake {
            g.hud.status.players[i].frames_of_shake_remaining = 10;
        }
        self.shake_damage_digits(i);
        if self.game.hud.status_count >= 5 {
            self.game.objs.jobjs[root].scale.x = 0.65;
        }
    }

    /// The display's damage process, `ifStatus_802F5B48`: a faded display goes after 200
    /// frames; otherwise the percentage (or HP) is read, capped at 999, and a change marks a
    /// hit to shake for or to animate.
    #[doc(alias = "ifStatus_802F5B48")]
    pub fn damage_proc(&mut self, gobj: GObjId) {
        let i = self
            .game
            .display_of(gobj)
            .expect("the original reads a null display");
        let p = &mut self.game.hud.status.players[i];
        if p.unk9 != 0 {
            p.unk9 = p.unk9.wrapping_add(1);
            if p.unk9 > 0xC8 {
                let slot = p.player_slot;
                self.free_damage_display(slot);
                return;
            }
        }
        let slot = i32::from(self.game.hud.status.players[i].player_slot as i8);
        if self.game.player_entity(slot).is_none() {
            return;
        }
        let stamina = self.game.player_more_flags_bit2(slot);
        let value = if stamina {
            self.game.player_remaining_hp(slot)
        } else {
            self.game.player_damage(slot)
        };
        let explode = stamina && self.game.vs.state.fighters[slot as usize].x4[0] & 0x04 != 0;
        let p = &mut self.game.hud.status.players[i];
        p.old_damage = p.damage_percent;
        p.damage_percent = (value as i16).clamp(0, 0x3E7);
        let (old, now) = (p.old_damage, p.damage_percent);
        let (fell, rose) = (old != -1 && now < old, old != -1 && now > old);
        let (shake, animate) = if stamina { (fell, rose) } else { (rose, fell) };
        p.flags.unk10 = animate;
        p.flags.force_digit_shake = shake;
        if shake {
            p.damage_from_last_attack = (i32::from(old) - i32::from(now)).unsigned_abs() as u8;
        }
        if stamina && old == 0 && explode {
            self.game.explode_damage_display(slot);
        }
    }

    /// The display's digits, made the first time, posed and placed, `ifStatus_802F5EC0`.
    #[doc(alias = "ifStatus_802F5EC0")]
    pub fn create_damage_digits(&mut self, i: usize, place: i32) -> Option<GObjId> {
        let data = self.data;
        let g = &mut *self.game;
        let root = match g.hud.status.players[i].HUD_parent_entity {
            None => {
                let o = g.world.create(0xE, 0xF, 0);
                let j = jobj::load_joint(&mut g.objs, data, g.hud.status.damage_num_joint)
                    .expect("the damage display's model");
                g.world.set_obj(o, OBJ_JOBJ, Obj::JObj(j));
                g.world.setup_gx_link(o, CodeRef(RENDER_DIGITS), 0xB, 0);
                g.world.setup_proc(o, CodeRef(PROC_DAMAGE), 0x11);
                g.world.setup_proc(o, CodeRef(PROC_DIGITS), 0x11);
                g.hud.status.players[i].HUD_parent_entity = Some(o);
                j
            }
            Some(o) => match g.world.gobj(o).obj {
                Some(Obj::JObj(j)) => j,
                _ => panic!("a damage display without its joints"),
            },
        };
        g.hud.status.players[i].flags.animation_status_id = 0;
        let s = &g.hud.status;
        let (an, mat, shape) = (
            s.damage_num_anims,
            s.damage_num_matanims,
            s.damage_num_shapeanims,
        );
        jobj::remove_anim(&mut g.objs, Some(root));
        anim::add_anim_index(&mut g.objs, data, Some(root), 0, an, mat, shape);
        jobj::req_anim_all(&mut g.objs, Some(root), 0.0);
        jobj::anim_all(
            &mut g.objs,
            data,
            &mut crate::game::code::ParticleHooks,
            Some(root),
        );
        let pos = g.player_hud_position(place);
        jobj::set_translate(&mut g.objs.jobjs, root, pos);
        for k in 0..4 {
            let j = nth_child(&g.objs.jobjs, Some(root), k as i32);
            let t = g.objs.jobjs[j.expect("a digit joint")].translate;
            let p = &mut g.hud.status.players[i];
            p.jobjs[k] = j;
            p.translation_x[k] = t.x;
            p.translation_y[k] = t.y;
        }
        self.init_percent_sign(i);
        let o = self.game.hud.status.players[i]
            .HUD_parent_entity
            .expect("the digits");
        self.damage_proc(o);
        let p = &mut self.game.hud.status.players[i];
        p.old_damage = i16::from(p.damage_percent == 0);
        self.damage_digits_proc(o);
        self.game.hud.status.players[i].HUD_parent_entity
    }

    /// The display's character mark, made the first time, posed on the character's icon,
    /// placed and coloured for the player, `ifStatus_802F61FC`.
    #[doc(alias = "ifStatus_802F61FC")]
    pub fn create_damage_mark(&mut self, i: usize, place: i32) -> Option<GObjId> {
        let data = self.data;
        let idx = place as u8;
        let g = &mut *self.game;
        let mut chara = g.player_ckind(i32::from(idx));
        let root = match g.hud.status.players[i].next {
            None => {
                let o = g.world.create(0xE, 0xF, 0);
                let j = jobj::load_joint(&mut g.objs, data, g.hud.status.damage_mark_joint)
                    .expect("the damage mark's model");
                g.world.set_obj(o, OBJ_JOBJ, Obj::JObj(j));
                g.world.setup_gx_link(o, CodeRef(RENDER_MARK), 0xB, 0);
                g.hud.status.players[i].next = Some(o);
                j
            }
            Some(o) => match g.world.gobj(o).obj {
                Some(Obj::JObj(j)) => j,
                _ => panic!("a damage mark without its joints"),
            },
        };
        let child = g.objs.jobjs[root].child.expect("jobj->child");
        let t = joint_tobj(&g.objs, child);
        let s = &g.hud.status;
        let (an, mat, shape) = (
            s.damage_mark_anims,
            s.damage_mark_matanims,
            s.damage_mark_shapeanims,
        );
        anim::add_anim_index(&mut g.objs, data, Some(root), 0, an, mat, shape);
        // Master Hand, Giga Bowser and Crazy Hand show the male wireframe's icon.
        if chara == 0x1A || (chara.wrapping_sub(0x1D) as u32) <= 1 {
            chara = 0x1B;
        }
        let frame = fp::fadds(0.5, stock_icon_frame(chara, 0, 0));
        let objs = &mut g.objs;
        tobj::req_anim_all(objs, t, mem(frame));
        let a = t.and_then(|t| objs.tobjs[t].aobj);
        aobj::set_rate(&mut objs.aobjs, a, 0.1);
        tobj::anim(objs, data, t);
        let pos = g.player_hud_position(i32::from(idx));
        jobj::set_translate(&mut g.objs.jobjs, root, pos);
        jobj::set_srt(&mut g.objs.jobjs, root, |o| {
            o.translate.x = mem(fp::fadds(reg(o.translate.x), 0.25))
        });
        let s = i32::from(idx);
        let c = hud_color(player_color(
            g.player_port(s) as u8,
            g.player_team(s) as u8,
            u8::from(g.vs_is_teams()),
            g.player_pkind(s) as u8,
        ));
        set_diffuse(&mut g.objs, child, [c[0], c[1], c[2]]);
        g.hud.status.players[i].next
    }

    /// A playing slot's display (if the rules show damage and the layout has room): reset,
    /// its mark and digits made, the score if shown, and its stock icons, `ifStatus_802F6508`.
    #[doc(alias = "ifStatus_802F6508")]
    pub fn show_damage_display(&mut self, slot: i32) {
        let g = &mut *self.game;
        if g.player_pkind(slot) == PKIND_NONE
            || i32::from(g.hud.status_count) <= slot
            || !g.vs.start.x2_6
        {
            return;
        }
        let i = slot as u8 as usize;
        let p = &mut g.hud.status.players[i];
        p.damage_percent = -1;
        p.old_damage = -1;
        p.frames_of_shake_remaining = 0;
        p.flags.explode_animation = false;
        p.flags.randomize_velocity = false;
        p.flags.force_digit_shake = false;
        p.flags.unk10 = false;
        p.player_slot = slot as u8;
        p.unk9 = 0;
        self.create_damage_mark(i, i as i32);
        self.create_damage_digits(i, i as i32);
        let rules = self.game.vs.start.clone();
        if rules.x3_0 && self.game.vs_x6() == 0 {
            self.create_score(slot);
        }
        let mode = if rules.match_kind == 1 || rules.x4_2 {
            0
        } else if rules.match_kind == 2 {
            2
        } else {
            1
        };
        self.create_stock_icons(slot as u8, mode);
    }

    /// Lays the displays out for `count` players and shows each slot's,
    /// `ifStatus_802F665C`.
    #[doc(alias = "ifStatus_802F665C")]
    pub fn show_damage_displays(&mut self, count: i32) {
        self.place_player_displays(count);
        self.game.hud.status_count = count as u8;
        for i in 0..6 {
            self.show_damage_display(i);
        }
    }

    /// Takes the damage models from `IfAll.dat` and clears the displays,
    /// `ifStatus_802F66A4`. The original clears them only when its caller's `r30` isn't
    /// zero; its one caller, `ifAll_802F390C`, holds the scene's light description there.
    #[doc(alias = "ifStatus_802F66A4")]
    pub fn load_damage_models(&mut self) {
        let data = self.data;
        let a = self.game.hud.archive.expect("IfAll.dat");
        let sym = |name: &str| {
            let at = data
                .get(a)
                .public(name)
                .unwrap_or_else(|| panic!("IfAll.dat's {name}"));
            data.ptr(DataRef {
                archive: a,
                offset: at,
            })
            .expect("a model list")
        };
        let num = sym("DmgNum_scene_models");
        let mrk = sym("DmgMrk_scene_models");
        let s = &mut self.game.hud.status;
        let f = |m: DataRef, o: u32| data.ptr(m.at(o));
        s.damage_num_joint = f(num, 0);
        s.damage_num_anims = f(num, 4);
        s.damage_num_matanims = f(num, 8);
        s.damage_num_shapeanims = f(num, 0xC);
        s.damage_mark_joint = f(mrk, 0);
        s.damage_mark_anims = f(mrk, 4);
        s.damage_mark_matanims = f(mrk, 8);
        s.damage_mark_shapeanims = f(mrk, 0xC);
        self.game.hud.status_count = 0;
        self.game.hud.status.players = Default::default();
    }

    /// Frees a slot's display and stock icons, `ifStatus_802F6788`.
    #[doc(alias = "ifStatus_802F6788")]
    pub fn free_damage_display(&mut self, slot: u8) {
        let i = usize::from(slot);
        if let Some(o) = self.game.hud.status.players[i].HUD_parent_entity.take() {
            gobj::free(self, o);
        }
        if let Some(o) = self.game.hud.status.players[i].next.take() {
            gobj::free(self, o);
        }
        self.free_stock_icons(i32::from(slot));
    }

    /// Frees every display, `ifStatus_802F6804`.
    #[doc(alias = "ifStatus_802F6804")]
    pub fn free_damage_displays(&mut self) {
        for i in 0..6 {
            self.free_damage_display(i);
        }
    }

    /// A player is KO'd (the inline `ifStatus_TriggerStockLoss`): the scene remembers who,
    /// the display blows apart, a stock match announces a player out, and outside stock
    /// matches the rules' `x2_5` shows the KO's mark (`mark`, an `if_2F72` function).
    fn stock_loss(&mut self, slot: i32, mark: u32) {
        self.game.vs.state.unk_D = slot as u8;
        self.game.explode_damage_display(slot);
        let k = self.game.player_pkind(slot);
        if self.game.vs.start.is_stock
            && (k == PKIND_HUMAN || k == PKIND_CPU)
            && self.game.player_stocks(slot) == 0
        {
            self.vs_announce_entry(slot, k as u8);
        }
        let s = &self.game.vs.start;
        if s.match_kind != 1 && s.x2_5 {
            self.ko_mark(mark, slot);
        }
    }

    /// A player KO'd by another, `ifStatus_802F69C0`: outside stock matches, the KO-er's
    /// mark first.
    #[doc(alias = "ifStatus_802F69C0")]
    pub fn damage_display_ko(&mut self, slot: i32, by: i32) {
        let s = &self.game.vs.start;
        if s.match_kind != 1 && s.x2_5 {
            self.ko_mark(super::announce::MARK_7C30, by);
        }
        self.stock_loss(slot, super::announce::MARK_7BB4);
    }

    /// `ifStatus_802F6AF8`: a team KO.
    #[doc(alias = "ifStatus_802F6AF8")]
    pub fn damage_display_team_ko(&mut self, slot: i32) {
        self.stock_loss(slot, super::announce::MARK_7AF8);
    }

    /// `ifStatus_802F6C04`: a self-destruct.
    #[doc(alias = "ifStatus_802F6C04")]
    pub fn damage_display_self_destruct(&mut self, slot: i32) {
        self.stock_loss(slot, super::announce::MARK_7C30);
    }

    /// `ifStatus_802F6D10`: a fall with no one to credit.
    #[doc(alias = "ifStatus_802F6D10")]
    pub fn damage_display_fall(&mut self, slot: i32) {
        self.stock_loss(slot, super::announce::MARK_7D08);
    }

    /// `ifStatus_802F6E1C`: the display again, after a respawn.
    #[doc(alias = "ifStatus_802F6E1C")]
    pub fn damage_display_respawn(&mut self, slot: i32) {
        self.show_damage_display(slot);
    }

    /// Clears the processes' second flag on a slot's display GObjs and stock icons,
    /// `ifStatus_802F6E3C`.
    #[doc(alias = "ifStatus_802F6E3C")]
    pub fn pause_damage_display(&mut self, slot: i32) {
        let p = &self.game.hud.status.players[slot as usize];
        let (a, b) = (p.HUD_parent_entity, p.next);
        for o in [a, b].into_iter().flatten() {
            self.game.world.clear_flags_2(o);
        }
        self.stock_icons_flags(slot);
    }
}

/// Stops every FObj of a texture animation in a joint tree (`mask` naming the objects) whose
/// type is `ty` (255 any), `mn_8022F3D8` through `mn_8022F360`.
#[doc(alias = "mn_8022F3D8")]
#[doc(alias = "mn_8022F360")]
pub fn stop_anim_type(
    objs: &mut Objects,
    data: &melee_data::archive::Archives,
    root: JObjId,
    ty: u8,
    mask: u32,
) {
    let mut aobjs = Vec::new();
    jobj::foreach_anim(objs, root, mask, &mut |_, a| aobjs.push(a));
    for a in aobjs {
        let mut f = objs.aobjs[a].fobj;
        while let Some(id) = f {
            if objs.fobjs[id.0].obj_type == ty || ty == 0xFF {
                crate::hsd::fobj::stop_anim(objs, data, Some(id), 0.0, None);
            }
            f = objs.fobjs[id.0].next;
        }
    }
}
