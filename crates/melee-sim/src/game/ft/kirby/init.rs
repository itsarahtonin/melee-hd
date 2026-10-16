//! Kirby's character callbacks (`ftkirby.c`): creation, spawn and attributes, the copies'
//! files a match loads for him, the item hooks and the knockback hooks.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::super::code::{Val, unported_call};
use super::super::super::handles::FighterId;
use super::super::super::state::Running;
use super::super::KIRBY_COSTUME_KINDS;
use super::super::data::MAIN_MEMORY;
use super::super::fox::ext;
use super::{ITEMS, KIND_KIRBY, addr, attr, u};
use super::super::super::ef::{Arg, SpawnKind};
use crate::fpu::reg;
use crate::vec::Vec3;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::mtx::Mtx34;

/// The bone his copies' hats sit on (`fp->parts[6]`).
const PART_HAT_BONE: i32 = 6;

/// `ftKb_Init_803CA9D0`: each kind's copy file and its data's symbol, by kind.
pub const COPY_FILES: [Option<(&str, &str)>; 33] = [
    Some(("PlKbCpMr.dat", "ftDataKirbyCopyMario")),
    Some(("PlKbCpFx.dat", "ftDataKirbyCopyFox")),
    Some(("PlKbCpCa.dat", "ftDataKirbyCopyCaptain")),
    Some(("PlKbCpDk.dat", "ftDataKirbyCopyDonkey")),
    None,
    Some(("PlKbCpKp.dat", "ftDataKirbyCopyKoopa")),
    Some(("PlKbCpLk.dat", "ftDataKirbyCopyLink")),
    Some(("PlKbCpSk.dat", "ftDataKirbyCopySeak")),
    Some(("PlKbCpNs.dat", "ftDataKirbyCopyNess")),
    Some(("PlKbCpPe.dat", "ftDataKirbyCopyPeach")),
    Some(("PlKbCpPp.dat", "ftDataKirbyCopyPopo")),
    None,
    Some(("PlKbCpPk.dat", "ftDataKirbyCopyPikachu")),
    Some(("PlKbCpSs.dat", "ftDataKirbyCopySamus")),
    Some(("PlKbCpYs.dat", "ftDataKirbyCopyYoshi")),
    Some(("PlKbCpPr.dat", "ftDataKirbyCopyPurin")),
    Some(("PlKbCpMt.dat", "ftDataKirbyCopyMewtwo")),
    Some(("PlKbCpLg.dat", "ftDataKirbyCopyLuigi")),
    Some(("PlKbCpMs.dat", "ftDataKirbyCopyMars")),
    Some(("PlKbCpZd.dat", "ftDataKirbyCopyZelda")),
    Some(("PlKbCpCl.dat", "ftDataKirbyCopyClink")),
    Some(("PlKbCpDr.dat", "ftDataKirbyCopyDrmario")),
    Some(("PlKbCpFc.dat", "ftDataKirbyCopyFalco")),
    Some(("PlKbCpPc.dat", "ftDataKirbyCopyPichu")),
    Some(("PlKbCpGw.dat", "ftDataKirbyCopyGamewatch")),
    Some(("PlKbCpGn.dat", "ftDataKirbyCopyGanon")),
    Some(("PlKbCpFe.dat", "ftDataKirbyCopyEmblem")),
    None,
    None,
    None,
    None,
    None,
    None,
];

/// The costume letters of Kirby's six costumes, in his costumes' order.
const COSTUMES: [&str; 6] = ["Nr", "Ye", "Bu", "Re", "Gr", "Wh"];

/// `ftKb_Init_803CB3E8`'s entries: the copies that dress Kirby in his own costume's colors
/// (Donkey Kong's, Jigglypuff's, Mewtwo's, Falco's and Mr. Game & Watch's), as each kind's
/// letters in the files' and the models' names; Mr. Game & Watch's six are his first one's.
const COPY_COSTUME_NAMES: [&str; 5] = ["Dk", "Pr", "Mt", "Fc", "Gw"];

/// `ftKb_Init_803CB46C`: each kind's copy effect file (255 for none), by kind.
const COPY_EFFECTS: [u8; 33] = [
    32, 33, 38, 39, 255, 41, 35, 21, 42, 45, 46, 46, 36, 34, 40, 43, 44, 37, 20, 21, 35, 32, 33,
    36, 255, 47, 48, 255, 255, 255, 255, 255, 255,
];

/// A copy costume's file, model and material animation (`ftKb_Init_803CACB0` and the others):
/// for `kind` (one of [`KIRBY_COSTUME_KINDS`]) and Kirby's costume `costume`.
pub fn copy_costume_files(kind: usize, costume: usize) -> Option<(String, String, String)> {
    let i = KIRBY_COSTUME_KINDS.iter().position(|&k| k == kind)?;
    let k = COPY_COSTUME_NAMES[i];
    let (c, sfx) = match (k, costume) {
        ("Gw", _) | (_, 0) => ("Nr", String::new()),
        (_, c) => (COSTUMES[c], COSTUMES[c].to_owned()),
    };
    Some((
        format!("PlKb{c}Cp{k}.dat"),
        format!("PlyKirby{k}{sfx}_Share_joint"),
        format!("PlyKirby{k}{sfx}_Share_matanim_joint"),
    ))
}

impl Running<'_> {
    /// Kirby's creation, `ftKb_Init_OnLoad`: his attributes, which are also his multi-jump
    /// attributes (`x2D0`), the player's flag in his hat's, and his four items registered.
    #[doc(alias = "ftKb_Init_OnLoad")]
    pub fn kb_on_load(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let d = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without data");
        let items = self.data.ptr(d.at(0x48)).expect("ftData::x48_items");
        self.push_attrs(f, attr::SIZE);
        let slot = i32::from(self.game.fighters[f.0].player_idx);
        let flag = self.game.player_flags_ae_bit1(slot);
        let fp = &mut self.game.fighters[f.0];
        fp.can_multijump = true;
        fp.x2D0 = fp.dat_attrs;
        fp.u[u::HAT_FLAGS] = (fp.u[u::HAT_FLAGS] & 0x7F) | (u8::from(flag) << 7);
        for (i, kind) in ITEMS.into_iter().enumerate() {
            let article = self.data.ptr(items.at(4 * i as u32));
            super::super::super::it::item::set_char_article(self.game, article, kind);
        }
    }

    /// Kirby's spawn, `ftKb_Init_OnDeath`: his first model variants, no hat or copy (the
    /// copy-star's number drawn anew), and the copy the player carries over, if any.
    #[doc(alias = "ftKb_Init_OnDeath")]
    pub fn kb_on_death(&mut self, gobj: GObjId) {
        self.game.set_part_vis(gobj, 0, 0);
        self.game.set_part_vis(gobj, 1, 0);
        let n = self.game.objs.rng.below(5) + 1;
        let fp = self.game.fighter_mut(gobj);
        fp.u.set_gobj_at(u::HAT_ITEM, None);
        fp.set_u_i32(u::HAT_X4, n);
        fp.u.set_jobj_at(u::HAT_JOBJ, None);
        fp.set_u_i32(u::HAT_KIND, KIND_KIRBY);
        fp.u.set_dobj_list_at(u::HAT_DOBJS, None);
        fp.set_u_i32(u::BUMP_TIMER, 0);
        fp.set_u_i32(u::X64, 0);
        let slot = i32::from(fp.player_idx);
        if self.game.player_flags_bit1(slot) && self.game.player_unk4d(slot) != KIND_KIRBY {
            unported_call("ftKb_SpecialN_800F1BAC (a copy carried over)");
        }
    }

    /// Kirby's neutral special on the ground, `ftKb_SpecialN_Enter`: his copy's
    /// (`ftKb_Init_803C9DD0`), else Inhale.
    #[doc(alias = "ftKb_SpecialN_Enter")]
    pub fn kb_special_n_enter(&mut self, gobj: GObjId) {
        match self.game.fighter(gobj).u_i32(u::HAT_KIND) {
            4 | 11 | 27..=30 | 32 => self.kb_n_enter(gobj),
            13 => self.kb_ss_enter(gobj),
            1 | 22 => self.kb_fx_enter(gobj),
            k => unported_call(&format!("the neutral special of copy kind {k}")),
        }
    }

    /// In the air, `ftKb_SpecialAirN_Enter` (`ftKb_Init_803C9E54`).
    #[doc(alias = "ftKb_SpecialAirN_Enter")]
    pub fn kb_special_air_n_enter(&mut self, gobj: GObjId) {
        match self.game.fighter(gobj).u_i32(u::HAT_KIND) {
            4 | 11 | 27..=30 | 32 => self.kb_n_air_enter(gobj),
            13 => self.kb_ss_air_enter(gobj),
            1 | 22 => self.kb_fx_air_enter(gobj),
            k => unported_call(&format!("the aerial neutral special of copy kind {k}")),
        }
    }

    /// His cleanup when KO'd, `ftKb_Init_800EE74C`: the hammer, the lean, the stone, an
    /// inhaled item and his copy's own put away, his copy lost, and no more of it.
    #[doc(alias = "ftKb_Init_800EE74C")]
    pub fn kb_death_cleanup(&mut self, gobj: GObjId) {
        self.kb_remove_hammer(gobj);
        self.kb_clear_lean(gobj);
        self.kb_stone_cleanup(gobj);
        self.kb_release_item(gobj);
        self.kb_copy_cleanup(gobj);
        self.kb_lose_copy(gobj, false);
        self.game.fighter_mut(gobj).death3_cb = None;
    }

    /// His cleanup when hit, `ftKb_Init_800EE7B8`: the same but his copy kept
    /// (`ftKb_SpecialN_800F1A8C`).
    #[doc(alias = "ftKb_Init_800EE7B8")]
    pub fn kb_damage_cleanup(&mut self, gobj: GObjId) {
        self.kb_remove_hammer(gobj);
        self.kb_clear_lean(gobj);
        self.kb_stone_cleanup(gobj);
        self.kb_release_item(gobj);
        self.kb_copy_hit_cleanup(gobj);
        self.game.fighter_mut(gobj).death1_cb = None;
    }

    /// `ftKb_Init_LoadSpecialAttrs`.
    #[doc(alias = "ftKb_Init_LoadSpecialAttrs")]
    pub fn kb_load_special_attrs(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.copy_attrs(f, attr::SIZE);
    }

    /// A kind's copy loaded for a match with Kirby in his costume `costume`,
    /// `ftKb_SpecialN_800EED50`: its data (`ft_80459B88`), the copy costume's model and
    /// material animation for the kinds that have them (`ftKb_Init_803C9FC8`), and its
    /// effects. Nothing for Kirby himself or no kind.
    #[doc(alias = "ftKb_SpecialN_800EED50")]
    pub fn kb_load_copy(&mut self, kind: i32, costume: i32) {
        if kind == -1 || kind == KIND_KIRBY {
            return;
        }
        let k = kind as usize;
        if let Some((file, name)) = COPY_FILES[k]
            && self.game.ft.kirby.copies[k].is_none()
        {
            self.disc_load(file, MAIN_MEMORY);
            let d = self.public_data(file, name);
            self.game.ft.kirby.copies[k] = Some(d);
        }
        if let Some(i) = KIRBY_COSTUME_KINDS.iter().position(|&c| c == k) {
            let c = costume as usize;
            if self.game.ft.kirby.models[i][c].0.is_none() {
                let (file, joint, matanim) =
                    copy_costume_files(k, c).expect("a copy costume's files");
                self.disc_load(&file, MAIN_MEMORY);
                let j = self.public_data(&file, &joint);
                let m = self.public_data(&file, &matanim);
                self.game.ft.kirby.models[i][c] = (Some(j), Some(m));
            }
        }
        super::super::super::ef::load_sync(self, i32::from(COPY_EFFECTS[k]));
    }

    /// The public symbol `name` of the loaded file `file`; the original asserts it's there.
    fn public_data(&self, file: &str, name: &str) -> DataRef {
        let archive = self.game.ft.data.files.archive(file);
        let offset = self
            .data
            .get(archive)
            .public(name)
            .unwrap_or_else(|| panic!("the original's assertion failed: Cannot find symbol {name}."));
        DataRef { archive, offset }
    }

    /// Picking up an item, `ftKb_Init_OnItemPickup`: the second model's hand takes the hold
    /// pose of the item's kind, unless the item is heavy.
    // Ports `ftKb_Init_OnItemPickup`; aliased once a check reaches it.
    pub fn kb_on_item_pickup(&mut self, gobj: GObjId, flag: bool) {
        let item = self
            .game
            .fighter(gobj)
            .item_gobj
            .expect("picking up no item");
        if super::super::super::it::lib26b1::is_heavy(self.game, self.data, item) {
            return;
        }
        let kind = self
            .ext(ext::IT_GET_HOLD_KIND, &[Val::GObj(Some(item))])
            .int();
        let pose = match kind {
            1 => Some(0),
            3 => Some(1),
            4 => Some(2),
            _ => None,
        };
        if let Some(p) = pose {
            self.ext(
                ext::FTANIM_80070FB4,
                &[Val::GObj(Some(gobj)), Val::Int(1), Val::Int(p)],
            );
        }
        if flag {
            self.ext(ext::FTANIM_80070C48, &[Val::GObj(Some(gobj)), Val::Int(1)]);
        }
    }

    /// Hiding the held item, `ftKb_Init_OnItemInvisible`, unless it's heavy.
    // Ports `ftKb_Init_OnItemInvisible`; aliased once a check reaches it.
    pub fn kb_on_item_invisible(&mut self, gobj: GObjId) {
        let item = self.game.fighter(gobj).item_gobj.expect("no item held");
        if !super::super::super::it::lib26b1::is_heavy(self.game, self.data, item) {
            self.ext(ext::FTANIM_80070CC4, &[Val::GObj(Some(gobj)), Val::Int(1)]);
        }
    }

    /// Showing the held item again, `ftKb_Init_OnItemVisible` (`Fighter_OnItemVisible(gobj,
    /// 1)`).
    // Ports `ftKb_Init_OnItemVisible`; aliased once a check reaches it.
    pub fn kb_on_item_visible(&mut self, gobj: GObjId) {
        let Some(item) = self.game.fighter(gobj).item_gobj else {
            return;
        };
        if !super::super::super::it::lib26b1::is_heavy(self.game, self.data, item) {
            self.ext(ext::FTANIM_80070C48, &[Val::GObj(Some(gobj)), Val::Int(1)]);
        }
    }

    /// Dropping the item, `ftKb_Init_OnItemDrop` (`Fighter_OnItemDrop(gobj, flag, 1, 1)`).
    // Ports `ftKb_Init_OnItemDrop`; aliased once a check reaches it.
    pub fn kb_on_item_drop(&mut self, gobj: GObjId, flag: bool) {
        self.ext(
            ext::FTANIM_80070FB4,
            &[Val::GObj(Some(gobj)), Val::Int(1), Val::Int(-1)],
        );
        if flag {
            self.ext(ext::FTANIM_80070CC4, &[Val::GObj(Some(gobj)), Val::Int(1)]);
        }
    }

    /// A charged copy's glow (the color animations' hook), `ftKb_Init_UnkMotionStates4`:
    /// Donkey Kong's full punch, Samus's full shot, Mewtwo's full ball and Sheik's full needles.
    #[doc(alias = "ftKb_Init_UnkMotionStates4")]
    pub fn kb_charged_glow(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let full_f =
            |word: usize, at: usize| fp::frsp(f64::from(fp.u_i32(word))) == reg(self.kb_attr_f32(f, at));
        let colanim = match fp.u_i32(u::HAT_KIND) {
            3 => (fp.u_i32(u::DK_SWINGS) == self.kb_attr_i32(f, attr::DK_SWINGS_TO_FULL))
                .then_some(58),
            13 => full_f(u::SS_CHARGE, attr::SS_CHARGE_TIME).then_some(54),
            16 => full_f(u::MT_CHARGE, attr::MT_CHARGE_TIME).then_some(93),
            7 => (fp.u_i32(u::SK_NEEDLES) == 6).then_some(87),
            _ => None,
        };
        if let Some(id) = colanim {
            self.start_color_anim(gobj, id, 0);
        }
    }

    /// Each frame before the state's, `ftKb_Init_UnkMotionStates3` (`ftData_UnkMotionStates3`):
    /// Bowser's copied breath refills.
    #[doc(alias = "ftKb_Init_UnkMotionStates3")]
    pub fn kb_unk_motion_states3(&mut self, gobj: GObjId) {
        if matches!(self.game.fighter(gobj).u_i32(u::HAT_KIND), 5 | 31) {
            unported_call("ftKb_SpecialNKp_800FA7D4 (Bowser's copied breath)");
        }
    }

    /// His bump effects' timer after the collision callback, `ftKb_SpecialN_800F1D24`: once
    /// it runs out, an effect where he meets a wall, ceiling or floor anew; five more frames
    /// whenever he touches one.
    #[doc(alias = "ftKb_SpecialN_800F1D24")]
    pub fn kb_bump_timer(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if fp.kind != KIND_KIRBY {
            return;
        }
        let t = fp.u_i32(u::BUMP_TIMER);
        if t != 0 {
            self.game.fighter_mut(gobj).set_u_i32(u::BUMP_TIMER, t - 1);
        } else {
            self.kb_bump_effects(gobj);
        }
        let fp = self.game.fighter_mut(gobj);
        if fp.coll_data.env_flags & (0x800 | 0x20 | 0x4000 | 0x10000) != 0 {
            fp.set_u_i32(u::BUMP_TIMER, 5);
        }
    }

    /// The effect where he meets a wall, ceiling or floor he didn't touch the frame before,
    /// `ftKb_SpecialN_800F1DAC`: at the side of his collision box that meets it.
    #[doc(alias = "ftKb_SpecialN_800F1DAC")]
    pub fn kb_bump_effects(&mut self, gobj: GObjId) {
        let c = &self.game.fighter(gobj).coll_data;
        let (env, prev) = (c.env_flags, c.prev_env_flags);
        let pos = if env & 0x800 != 0 && prev & 0xFC0 == 0 {
            Vec3::new(c.ecb.left.x, c.ecb.left.y, 0.0)
        } else if env & 0x20 != 0 && prev & 0x3F == 0 {
            Vec3::new(c.ecb.right.x, c.ecb.right.y, 0.0)
        } else if env & 0x4000 != 0 && prev & 0x6000 == 0 {
            Vec3::new(0.0, c.ecb.top.y, 0.0)
        } else if env & 0x10000 != 0 && prev & 0x18000 == 0 {
            Vec3::new(0.0, c.ecb.bottom.y, 0.0)
        } else {
            return;
        };
        self.kb_bump_effect_at(gobj, pos);
    }

    /// The bump effect at `pos` in his top joint's space (effect 0x49E).
    fn kb_bump_effect_at(&mut self, gobj: GObjId, pos: Vec3) {
        let j = self.bone_joint(gobj, 0);
        self.fighter_effect(gobj, SpawnKind::PosOffset, 0x49E, j, &[Arg::Vec(pos)]);
    }

    /// Kirby's effect where he bounces off a wall, ceiling or floor, `ftKb_SpecialN_800F1F1C`.
    #[doc(alias = "ftKb_SpecialN_800F1F1C")]
    pub fn kirby_bump_effect(&mut self, gobj: GObjId, pos: Vec3) {
        if self.game.fighter(gobj).kind == KIND_KIRBY {
            self.kb_bump_effect_at(gobj, pos);
        }
    }

    /// His copy's hat drawn on his head's matrix, `ftKb_UnkMtxFunc0` (his per-kind drawing):
    /// nothing without a hat model or with it hidden (`x2225_b2`).
    #[doc(alias = "ftKb_UnkMtxFunc0")]
    pub fn kb_draw_hat(&mut self, gobj: GObjId, vmtx: Option<Mtx34>, pass: i32) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let Some(hat) = fp.u.jobj_at(u::HAT_JOBJ) else {
            return;
        };
        if !fp.x2225_b2 {
            return;
        }
        // `HSD_JObjGetMtxPtr`, then `HSD_JObjCopyMtx`.
        let head = self.bone_joint(gobj, PART_HAT_BONE);
        jobj::setup_matrix(&mut self.game.objs, self.data, head);
        let head = head.expect("a fighter without its sixth bone");
        let m = self.game.objs.jobjs[head].mtx;
        let jobjs = &mut self.game.objs.jobjs;
        jobjs[hat].mtx = m;
        jobjs[hat].flags |= jobj::USER_DEF_MTX | jobj::MTX_INDEP_PARENT | jobj::MTX_INDEP_SRT;
        jobj::set_mtx_dirty(jobjs, Some(hat));
        self.draw_fighter_tree(f, Some(hat), vmtx, pass);
    }

    /// His copy's costume's texture animations restarted with his own,
    /// `ftKb_Init_UnkCallbackPairs0_0`: only a copy that dresses him has them.
    #[doc(alias = "ftKb_Init_UnkCallbackPairs0_0")]
    pub fn kb_hat_restart_texture_anims(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if fp.u.dobj_list_at(u::HAT_DOBJS).is_some() && fp.u.jobj_at(u::HAT_JOBJ).is_none() {
            let f = self.game.fighter_id(gobj);
            self.kb_hat_textures_restart(f);
        }
    }

    /// His copy's costume's texture `i` set to `frame` with his own,
    /// `ftKb_Init_UnkCallbackPairs0_1`.
    #[doc(alias = "ftKb_Init_UnkCallbackPairs0_1")]
    pub fn kb_hat_texture_anim(&mut self, gobj: GObjId, i: i32, frame: f32) {
        let fp = self.game.fighter(gobj);
        if fp.u.dobj_list_at(u::HAT_DOBJS).is_some() && fp.u.jobj_at(u::HAT_JOBJ).is_none() {
            let f = self.game.fighter_id(gobj);
            self.kb_hat_texture_set(f, i, frame);
        }
    }

    /// Entering knockback, `ftKb_Init_OnKnockbackEnter` (`Fighter_OnKnockbackEnter(gobj,
    /// 1)`): the hurt textures.
    #[doc(alias = "ftKb_Init_OnKnockbackEnter")]
    pub fn kb_on_knockback_enter(&mut self, gobj: GObjId) {
        self.texture_anim(gobj, 1, 3.0);
        self.texture_anim(gobj, 0, 3.0);
    }

    /// Leaving knockback, `ftKb_Init_OnKnockbackExit`.
    #[doc(alias = "ftKb_Init_OnKnockbackExit")]
    pub fn kb_on_knockback_exit(&mut self, gobj: GObjId) {
        self.texture_anim(gobj, 1, 0.0);
        self.texture_anim(gobj, 0, 0.0);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ON_ITEM_INVISIBLE => g.kb_on_item_invisible(gobj),
        addr::ON_ITEM_VISIBLE => g.kb_on_item_visible(gobj),
        addr::SPECIAL_N_ENTER => g.kb_special_n_enter(gobj),
        addr::SPECIAL_AIR_N_ENTER => g.kb_special_air_n_enter(gobj),
        addr::DEATH3 => g.kb_death_cleanup(gobj),
        addr::DEATH1 => g.kb_damage_cleanup(gobj),
        addr::UNK_MOTION_STATES_4 => g.kb_charged_glow(gobj),
        addr::ON_KNOCKBACK_ENTER => g.kb_on_knockback_enter(gobj),
        addr::ON_KNOCKBACK_EXIT => g.kb_on_knockback_exit(gobj),
        0x800E_F9BC => g.kb_hat_restart_texture_anims(gobj),
        0x800F_04F8 => g.kb_samus_hat_on(gobj),
        0x800F_05A8 => g.kb_samus_hat_off(gobj),
        0x800F_130C => g.kb_falco_hat_on(gobj),
        0x800F_13F0 => g.kb_falco_hat_off(gobj),
        0x800F_19AC => g.kb_copy_cleanup(gobj),
        0x800F_1A8C => g.kb_copy_hit_cleanup(gobj),
        0x800F_1B6C => g.kb_unk_motion_states3(gobj),
        0x800F_1D24 => g.kb_bump_timer(gobj),
        0x800F_1DAC => g.kb_bump_effects(gobj),
        0x800F_1F68 => g.kb_attack_dash_enter(gobj),
        _ => return false,
    }
    true
}

pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let gobj = |i: usize| args[i].gobj().expect("a null fighter");
    Some(match f {
        addr::ON_ITEM_PICKUP => {
            r.kb_on_item_pickup(gobj(0), args[1].int() != 0);
            Val::None
        }
        addr::ON_ITEM_DROP => {
            r.kb_on_item_drop(gobj(0), args[1].int() != 0);
            Val::None
        }
        addr::LOAD_COPY => {
            r.kb_load_copy(args[0].int(), args[1].int());
            Val::None
        }
        _ => return None,
    })
}
