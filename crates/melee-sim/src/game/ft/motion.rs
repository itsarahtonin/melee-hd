//! Changing a fighter's action state (`Fighter_ChangeMotionState`, `fighter.c`): the state's
//! flags say which of the last state's effects carry over; everything else is reset, the
//! state's animation is loaded from ARAM and started, its command script begins, and its
//! callbacks take over.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::code::{run_fighter_cb, unported_call};
use super::super::handles::FighterId;
use super::super::pl::player::PKIND_DEMO;
use super::super::state::Running;
use super::super::structs::Fighter;
use super::anim::x594_bit;
use super::common::GA_GROUND;
use super::{MotionState, motion_table};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, USE_QUATERNION};

/// `FighterKind`s the state change treats apart.
const KIND_PEACH: i32 = 9;
pub(super) const KIND_NANA: i32 = 11;
const KIND_POPO: i32 = 10;
const KIND_LUIGI: i32 = 17;

/// The state change's flags (`MotionFlags`), each keeping one effect of the last state.
pub mod flags {
    pub const KEEP_FAST_FALL: u32 = 1 << 0;
    pub const KEEP_GFX: u32 = 1 << 1;
    pub const KEEP_COL_ANIM_HIT_STATUS: u32 = 1 << 2;
    pub const SKIP_HIT: u32 = 1 << 3;
    pub const SKIP_MODEL: u32 = 1 << 4;
    pub const SKIP_ANIM_VEL: u32 = 1 << 5;
    pub const SKIP_HIT_STUN_START: u32 = 1 << 6;
    pub const SKIP_MAT_ANIM: u32 = 1 << 7;
    pub const SKIP_THROW_EXCEPTION: u32 = 1 << 8;
    pub const KEEP_SFX: u32 = 1 << 9;
    pub const SKIP_PARASOL: u32 = 1 << 10;
    pub const SKIP_RUMBLE: u32 = 1 << 11;
    pub const SKIP_COL_ANIM: u32 = 1 << 12;
    pub const KEEP_ACCESSORY: u32 = 1 << 13;
    pub const UPDATE_CMD: u32 = 1 << 14;
    pub const SKIP_NAMETAG_VIS: u32 = 1 << 15;
    pub const KEEP_COL_ANIM_PART_HIT_STATUS: u32 = 1 << 16;
    pub const KEEP_SWORD_TRAIL: u32 = 1 << 17;
    pub const SKIP_ITEM_VIS: u32 = 1 << 18;
    pub const UNK_19: u32 = 1 << 19;
    pub const UNK_UPDATE_PHYS: u32 = 1 << 20;
    pub const FREEZE_STATE: u32 = 1 << 21;
    pub const SKIP_MODEL_PART_VIS: u32 = 1 << 22;
    pub const SKIP_METAL_B: u32 = 1 << 23;
    pub const UNK_24: u32 = 1 << 24;
    pub const SKIP_ATTACK_COUNT: u32 = 1 << 25;
    pub const SKIP_MODEL_FLAGS: u32 = 1 << 26;
    pub const UNK_27: u32 = 1 << 27;
    pub const SKIP_HIT_STUN: u32 = 1 << 28;
    pub const SKIP_ANIM: u32 = 1 << 29;
}

/// `Fighter_WaitAnimData`'s size: an animation's name, place in the animation file, size,
/// command script, flags and (once loaded) ARAM address.
const ANIM_ENTRY_SIZE: u32 = 0x18;

impl Running<'_> {
    /// The fighter's action state `msid`: a common one, or from `x18` on, its character's.
    pub fn motion_state(fp: &Fighter, msid: i32) -> &'static MotionState {
        if msid >= fp.x18 {
            &motion_table(fp.x20_actionStateList.0)[(msid - fp.x18) as usize]
        } else {
            &motion_table(fp.x1C_actionStateList.0)[msid as usize]
        }
    }

    /// Animation `anim`'s entry in the fighter's animation table (`x24`), `ftData_80085FD4`.
    /// Nana outside demos takes Popo's entry (`gFtDataList[Ft_Kind_Popo]->xC`) where hers
    /// has no animation in ARAM.
    #[doc(alias = "ftData_80085FD4")]
    pub(super) fn anim_entry(&self, f: FighterId, anim: i32) -> DataRef {
        let fp = &self.game.fighters[f.0];
        let own = fp
            .x24
            .expect("a fighter without animations")
            .at(ANIM_ENTRY_SIZE * anim as u32);
        if fp.kind == KIND_NANA
            && self.game.player_pkind(i32::from(fp.player_idx)) != PKIND_DEMO
            && self.anim_aram(fp, own) == 0
        {
            let popo =
                self.game.ft.data.data_list[KIND_POPO as usize].expect("Popo's data isn't loaded");
            return self
                .data
                .ptr(popo.at(0xC))
                .expect("Popo's ftData::xC")
                .at(ANIM_ENTRY_SIZE * anim as u32);
        }
        own
    }

    /// The kind whose animation table `entry` is in: the fighter's, or Popo's for an entry
    /// Nana took from his ([`Running::anim_entry`]).
    pub(super) fn anim_entry_kind(&self, kind: i32, entry: DataRef) -> i32 {
        let popo = self.game.ft.data.data_list[KIND_POPO as usize].map(|d| d.archive);
        if kind == KIND_NANA && popo == Some(entry.archive) {
            KIND_POPO
        } else {
            kind
        }
    }

    /// Where an animation of the fighter is in ARAM (`Fighter_WaitAnimData::x14`): its
    /// character's animation file plus the animation's offset in it, as `ftData_80085A14`
    /// wrote it when the file was copied there; for an entry with no data, the file's word.
    pub(super) fn anim_aram(&self, fp: &Fighter, entry: DataRef) -> u32 {
        if self.data.u32(entry.at(8)) == 0 {
            return self.data.u32(entry.at(0x14));
        }
        let kind = self.anim_entry_kind(fp.kind, entry);
        let file = self.game.ft.anim_files[kind as usize]
            .as_ref()
            .expect("the character's animation file isn't in ARAM");
        file.aram.wrapping_add(self.data.u32(entry.at(4)))
    }

    /// The fighter whose animation buffer Nana may copy from rather than read ARAM,
    /// `ftData_80086060`: outside demos, her player's current fighter (Popo).
    // Ports `ftData_80086060`; aliased once a check reaches it.
    pub(super) fn anim_share_source(&self, f: FighterId) -> Option<FighterId> {
        let fp = &self.game.fighters[f.0];
        let slot = i32::from(fp.player_idx);
        if fp.kind == KIND_NANA && self.game.player_pkind(slot) != PKIND_DEMO {
            return self
                .game
                .player_entity_at(slot, 0)
                .map(|g| self.game.fighter_id(g));
        }
        None
    }

    /// Loads animation `anim` of fighter `src` into the fighter's animation buffer unless it's
    /// there already, `ftData_80085CD8`: the buffer then holds that animation's archive, and
    /// `x590` its figatree.
    #[doc(alias = "ftData_80085CD8")]
    pub(super) fn load_anim(&mut self, f: FighterId, src: FighterId, anim: i32) {
        if anim as u32 >= self.game.fighters[src.0].x58C {
            return;
        }
        let entry = self.anim_entry(src, anim);
        let aram = self.anim_aram(&self.game.fighters[src.0], entry);
        if aram == self.game.fighters[f.0].x5A4 {
            return;
        }
        let tree = if aram != 0 {
            // Nana copies the archive from Popo's buffer when it holds it (`memcpy` and
            // `lbArchiveRelocate`, untimed).
            let shared = self
                .anim_share_source(f)
                .is_some_and(|p| self.game.fighters[p.0].x5A4 == aram);
            if !shared {
                if aram >= 0x8000_0000 {
                    unported_call("ftData_80085CD8 from main memory");
                }
                // `lbArq_80014BD0` copies the archive from ARAM, which takes time on
                // ssbm-rs's timeline; `HSD_ArchiveParse` relocates it.
                let len = (self.data.u32(entry.at(8)) + 31) & !31;
                self.game.audio.aram_load(self.data, len);
            }
            let kind = self.anim_entry_kind(self.game.fighters[src.0].kind, entry);
            Some(self.anim_tree(kind, entry))
        } else {
            None
        };
        let fp = &mut self.game.fighters[f.0];
        fp.x590 = tree;
        fp.x5A4 = aram;
    }

    /// The figatree of a kind's animation table entry, as copying its archive from ARAM
    /// (`lbArq_80014BD0`) and relocating it (`HSD_ArchiveParse`) finds it.
    pub(super) fn anim_tree(&self, kind: i32, entry: DataRef) -> DataRef {
        let file = self.game.ft.anim_files[kind as usize]
            .as_ref()
            .expect("the character's animation file isn't in ARAM");
        let off = self.data.u32(entry.at(4));
        let &(_, id) = file
            .anims
            .iter()
            .find(|&&(o, _)| o == off)
            .unwrap_or_else(|| panic!("no animation at {off:#x} of character {kind}'s file"));
        let mut buf = [0u8; 128];
        let name = self.c_string(
            self.data.ptr(entry).expect("an animation without a name"),
            &mut buf,
        );
        let archive = self.data.get(id);
        let offset = archive
            .public(name)
            .unwrap_or_else(|| panic!("{name} isn't in its animation's archive"));
        DataRef {
            archive: id,
            offset,
        }
    }

    /// The NUL-terminated string at `at`, read into `buf` (a symbol name: ASCII, and shorter).
    fn c_string<'b>(&self, at: DataRef, buf: &'b mut [u8]) -> &'b str {
        let mut i = 0;
        loop {
            let b = self.data.u8(at.at(i as u32));
            if b == 0 {
                break;
            }
            assert!(
                i < buf.len(),
                "a symbol name longer than {} bytes",
                buf.len()
            );
            buf[i] = b;
            i += 1;
        }
        std::str::from_utf8(&buf[..i]).expect("an ASCII symbol name")
    }

    /// Stops the fighter's rumble of kind `id` if its controller rumbles, `ftCommon_8007ECD4`.
    #[doc(alias = "ftCommon_8007ECD4")]
    pub(super) fn stop_rumble(&mut self, f: FighterId, id: i32) {
        let fp = &self.game.fighters[f.0];
        if self
            .game
            .rumble_enabled(i32::from(fp.player_idx), i32::from(fp.is_sub_fighter))
        {
            // `HSD_PadRumbleRemoveId`: the pad library's rumble is simulation state, and its
            // motors reach the frontend through the poll's requests.
            let port = fp.pad_port;
            self.game.pads.rumble.remove_id(port, (id + 2) as u32);
        }
    }

    /// Stops the sound on one of the fighter's tracks (`lbAudioAx_80023870` with the key-off
    /// id): `base` plus two per player, plus one for the second Ice Climber.
    pub(super) fn key_off(&mut self, f: FighterId, base: i32) {
        let fp = &self.game.fighters[f.0];
        let track = base + 2 * i32::from(fp.player_idx) + i32::from(fp.is_sub_fighter);
        self.sfx_play_track(crate::lb::audio::SFX_KEY_OFF, 0, 0x40, track);
    }

    /// Clears the fighter's hitboxes, `ftColl_8007AFF8`.
    #[doc(alias = "ftColl_8007AFF8")]
    pub(super) fn clear_hitboxes(&mut self, f: FighterId) {
        let fp = &mut self.game.fighters[f.0];
        for h in &mut fp.x914 {
            crate::lb::hitcapsule::disable(h);
        }
        fp.x2219_b3 = false;
    }

    /// Sets every hurtbox's state, `ftColl_8007B0C0`.
    #[doc(alias = "ftColl_8007B0C0")]
    pub(super) fn set_hurtbox_state(&mut self, f: FighterId, state: i32) {
        let fp = &mut self.game.fighters[f.0];
        for i in 0..fp.hurt_capsules_len as usize {
            let h = &mut fp.hurt_capsules[i].capsule;
            h.state = state;
            h.skip_update_pos = false;
        }
        fp.x221A_b5 = state != 0;
    }

    /// Puts the hurtboxes back as the character's data describes them, `ftColl_8007B4E0`.
    #[doc(alias = "ftColl_8007B4E0")]
    fn reset_hurtboxes(&mut self, f: FighterId) {
        let fp = &self.game.fighters[f.0];
        let x30 = self
            .data
            .ptr(fp.ft_data.expect("a fighter without data").at(0x30))
            .expect("ftData's hurtboxes");
        let inits = self.data.ptr(x30.at(4)).expect("hurtbox descriptions");
        let v = |at: DataRef| crate::vec::Vec3 {
            x: self.data.f32(at),
            y: self.data.f32(at.at(4)),
            z: self.data.f32(at.at(8)),
        };
        for i in 0..fp.hurt_capsules_len as u32 {
            let init = inits.at(0x28 * i);
            let bone_idx = self.data.u32(init) as i32;
            let joint = self.bone(f, bone_idx).joint;
            let (height, grabbable) = (
                self.data.u32(init.at(4)) as i32,
                self.data.u32(init.at(8)) as i32,
            );
            let (a, b, scale) = (
                v(init.at(0xC)),
                v(init.at(0x18)),
                self.data.f32(init.at(0x24)),
            );
            let h = &mut self.game.fighters[f.0].hurt_capsules[i as usize];
            h.capsule.bone_idx = bone_idx;
            h.height = height;
            h.is_grabbable = grabbable;
            h.capsule.state = 0;
            h.capsule.bone = joint;
            h.capsule.a_offset = a;
            h.capsule.b_offset = b;
            h.capsule.scale = scale;
            h.capsule.skip_update_pos = false;
        }
        self.game.fighters[f.0].x221A_b6 = false;
    }

    /// Shows each model's previous variant again, `ftParts_80074A8C`.
    #[doc(alias = "ftParts_80074A8C")]
    pub(super) fn restore_models(&mut self, f: FighterId) {
        let fp = &mut self.game.fighters[f.0];
        for i in 0..fp.x5AC.model_num as usize {
            fp.x5F4_arr[i].idx = fp.x5F4_arr[i].prev;
        }
        fp.x221D_b2 = false;
    }

    /// Ends the parts' extra animations (`x8B0`'s slots), letting the parts animate again,
    /// `ftAnim_80070F28`.
    #[doc(alias = "ftAnim_80070F28")]
    fn end_part_anims(&mut self, f: FighterId) {
        for i in 0..5 {
            if self.game.fighters[f.0].x8B0[i].x11 == -1 {
                continue;
            }
            let fp = &self.game.fighters[f.0];
            let x1c = self
                .data
                .ptr(fp.ft_data.expect("a fighter without data").at(0x1C))
                .expect("ftData's part animations");
            let set = self
                .data
                .ptr(x1c.at(4 * i as u32))
                .expect("a part animation");
            let n = u32::from(self.data.u16(set.at(2)));
            let list = self.data.ptr(set.at(4));
            let parts = fp.parts.expect("a fighter without parts");
            for j in 0..n {
                let part = self.data.u8(list.expect("part animation parts").at(j));
                self.game.fighter_parts[parts.0][part as usize].x8[0] &= !0x04;
            }
            self.game.fighters[f.0].x8B0[i].x11 = -1;
        }
    }

    /// Re-applies the parts' extra animations still playing, `ftAnim_80070E74`.
    #[doc(alias = "ftAnim_80070E74")]
    fn reapply_part_anims(&mut self, f: FighterId) {
        let gobj = self.game.fighters[f.0].gobj.expect("a fighter's GObj");
        for i in 0..5 {
            let anim = self.game.fighters[f.0].x8B0[i].x10;
            if anim != -1 {
                self.apply_part_anim(gobj, i as i32, i32::from(anim), 0.0);
            }
        }
    }

    /// Restarts the costume's texture animations, `ftAnim_80070654`: each from its first
    /// frame, stepped once (`ftAnim_800705E0`). Kirby also restarts his copied ability's
    /// (`ftData_UnkCallbackPairs0`); not ported.
    #[doc(alias = "ftAnim_80070654")]
    #[doc(alias = "ftAnim_800705E0")]
    fn restart_costume_anims(&mut self, f: FighterId) {
        let list = self.game.fighters[f.0].tobj_list.clone();
        for i in 0..list.n_costume_tobjs as usize {
            let t = list.costume_tobjs[i].expect("a costume without its texture");
            let a = self.game.objs.tobjs[t].aobj;
            let objs = &mut self.game.objs;
            crate::hsd::aobj::req_anim(&mut objs.aobjs, &mut objs.fobjs, a, 0.0);
            crate::hsd::tobj::anim(objs, self.data, Some(t));
        }
        if self.game.fighters[f.0].kind == 4 {
            unported_call("ftKb_Init_UnkCallbackPairs0_0");
        }
        self.game.fighters[f.0].x221E_b7 = false;
    }

    /// Sets the fighter's hit status (`x1988`: 0 normal, 1 invincible, 2 intangible) and the
    /// color animation showing it, `ftColl_8007B62C`.
    #[doc(alias = "ftColl_8007B62C")]
    pub(super) fn set_hit_status(&mut self, gobj: GObjId, status: i32) {
        self.game.fighter_mut(gobj).x1988 = status;
        let id = match status {
            0 => 1,
            1 => 3,
            2 => 2,
            _ => return,
        };
        self.start_color_anim(gobj, id, 0);
    }

    /// `ftCo_800C8B2C`: a color animation, unless `x2226_b6` holds them off.
    #[doc(alias = "ftCo_800C8B2C")]
    pub(super) fn start_color_anim_unless_held(&mut self, gobj: GObjId, id: i32) -> bool {
        !self.game.fighter(gobj).x2226_b6 && self.start_color_anim(gobj, id, 0)
    }

    /// Records the action's attack (`move_id`), drawing a new attack instance for a new move
    /// or a repeat of move 1, `ft_800890D0`.
    #[doc(alias = "ft_800890D0")]
    #[doc(alias = "plStale_IncrementAttackInstance")]
    fn set_attack_id(&mut self, f: FighterId, move_id: u32) {
        if move_id == 1 || move_id != self.game.fighters[f.0].x2068_attackID as u32 {
            let before = self.game.pl.stale_attack_instance;
            self.game.pl.stale_attack_instance = match before.wrapping_add(1) {
                0 => 1,
                n => n,
            };
            let fp = &mut self.game.fighters[f.0];
            fp.x2068_attackID = move_id as i32;
            fp.x206C_attack_instance = before;
        }
    }

    /// Takes the action's attack flags (`x2070`) and resets the attack's record (`x2074`),
    /// `ft_800895E0`.
    #[doc(alias = "ft_800895E0")]
    #[doc(alias = "plAttack_80037B08")]
    #[doc(alias = "ft_80089768")]
    fn set_attack_flags(&mut self, f: FighterId, flags: u32) {
        let mut val = flags.to_be_bytes();
        let fp = &self.game.fighters[f.0];
        if val[3] == 0 || val[3] != fp.x2070[3] {
            let before = self.game.pl.attack_instance;
            self.game.pl.attack_instance = match before.wrapping_add(1) {
                0 => 1,
                n => n,
            };
            self.game.fighters[f.0].x2074.x2088 = before;
        }
        let fp = &self.game.fighters[f.0];
        if fp.kind == KIND_LUIGI && val[3] == 113 {
            val = 0x0024_0063u32.to_be_bytes();
        }
        // Throwing an adventure enemy counts as its own move.
        if val[3] == 98
            && let Some(item) = fp.item_gobj
            && crate::game::it::coll::is_enemy(self.game, item)
        {
            val = 0x0044_003Du32.to_be_bytes();
        }
        self.game.fighters[f.0].x2070 = val;
        self.reset_attack_record(f);
    }

    /// Starts the fighter's attack record over (`x2074`), `ft_80089460` (`ft_80089768`'s
    /// clearing): no hits, the percent it started at, and the conditions it began under
    /// (metal, a partner, invincible, and the like).
    #[doc(alias = "ft_80089460")]
    pub(crate) fn reset_attack_record(&mut self, f: FighterId) {
        let fp = &mut self.game.fighters[f.0];
        let r = &mut fp.x2074;
        r.x2074_vec.x = 0;
        r.x2074_vec.y = 0;
        r.x207C.x = 0.0;
        r.x207C.y = 6;
        r.x10[0] = 0;
        r.x10[1] &= 0x07;
        r.x2074_vec.x = fp::fctiwz(reg(fp.dmg.x1830_percent));
        let bits = [
            (0x40, fp.is_metal),
            (0x20, fp.x197C.is_some()),
            (0x10, fp.x221D_b6),
            (0x08, fp.x2226_b4),
            (0x04, fp.x2220_b5),
            (0x02, fp.x2220_b6),
        ];
        for (mask, set) in bits {
            if set {
                fp.x2074.x10[0] |= mask;
            }
        }
    }

    /// A new instance of the same attack, `ft_800892A0`: a new attack instance, the move kept.
    #[doc(alias = "ft_800892A0")]
    pub(crate) fn renew_attack_instance(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let before = self.game.pl.stale_attack_instance;
        self.game.pl.stale_attack_instance = match before.wrapping_add(1) {
            0 => 1,
            n => n,
        };
        self.game.fighters[f.0].x206C_attack_instance = before;
    }

    /// A looping attack's next hit counts anew (`x21EC`): `onPkPc21EC` (Pikachu's and
    /// Pichu's jab) and the down tilt's `callUnk`.
    // Ports `onPkPc21EC`; aliased once a check reaches it.
    #[doc(alias = "callUnk")]
    pub fn attack_counts_anew(&mut self, gobj: GObjId) {
        self.renew_attack_instance(gobj);
        self.renew_attack_record(gobj);
    }

    /// A new record of the same attack, `ft_80089824`: the record started over, a new record
    /// number, and the attack counted in the player's statistics again.
    #[doc(alias = "ft_80089824")]
    pub(crate) fn renew_attack_record(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.reset_attack_record(f);
        let n = self.game.next_attack_record();
        self.game.fighters[f.0].x2074.x2088 = n;
        self.count_attack(gobj, [0; 4]);
    }

    /// A thrown fighter's hitbox takes its thrower, `ftColl_8007B8CC`: its owner, team and
    /// player.
    #[doc(alias = "ftColl_8007B8CC")]
    pub(crate) fn thrown_hitbox_owner(&mut self, f: FighterId, thrower: GObjId) {
        let tf = self.game.fighter(thrower);
        let (team, slot) = (tf.team, tf.player_idx);
        let fp = &mut self.game.fighters[f.0];
        fp.x1064_thrownHitbox.x134 = crate::lb::hitcapsule::HitOwner::Thrower(thrower);
        fp.x119C_teamUnk = team;
        fp.grabber_unk1 = slot;
    }

    /// Changes the fighter's action state to `msid`, `Fighter_ChangeMotionState`. `start` is
    /// the animation frame to start from, `speed` its rate, `blend` the frames to blend into
    /// it over (0: the animation's own; -1: none); `flags` ([`flags`]) keep effects of the
    /// last state.
    #[doc(alias = "Fighter_ChangeMotionState")]
    #[allow(clippy::too_many_arguments)]
    pub fn change_motion_state(
        &mut self,
        gobj: GObjId,
        msid: i32,
        flags: u32,
        start: f32,
        speed: f32,
        blend: f32,
        other: Option<GObjId>,
    ) {
        use flags::*;
        let f = self.game.fighter_id(gobj);
        let root = self.root_jobj(gobj);
        {
            let fp = &mut self.game.fighters[f.0];
            fp.motion_id = msid;
            fp.facing_dir1 = fp.facing_dir;
            let pos = fp.cur_pos;
            jobj::set_translate(&mut self.game.objs.jobjs, root, pos);
        }
        // `efAsync_QueueFlush`.
        if self.game.fighters[f.0].x60C.is_some() {
            let gobj = self.game.fighters[f.0].gobj;
            let mut head = self.game.fighters[f.0].x60C;
            crate::game::ef::flush(self, gobj, &mut head);
            self.game.fighters[f.0].x60C = head;
        }
        if flags & SKIP_HIT == 0 {
            if self.game.fighters[f.0].x2219_b3 {
                self.clear_hitboxes(f);
            }
            // `ft_8007C114`.
            if self.game.fighters[f.0].x2219_b4 {
                if self.holding_hammer(gobj) {
                    unported_call("ftCo_800C555C (the hammer)");
                }
                self.game.fighters[f.0].x2219_b4 = false;
            }
        }
        if flags & SKIP_THROW_EXCEPTION == 0 {
            self.game.fighters[f.0].x1064_thrownHitbox.x134 = Default::default();
        }
        if flags & KEEP_COL_ANIM_HIT_STATUS == 0 {
            if self.game.fighters[f.0].x1988 != 0 {
                self.set_hit_status(gobj, 0);
            }
            if self.game.fighters[f.0].x221A_b5 {
                self.set_hurtbox_state(f, 0);
            }
        }
        if self.game.fighters[f.0].x221A_b6 {
            self.reset_hurtboxes(f);
        }
        if flags & SKIP_MODEL == 0 && self.game.fighters[f.0].x221D_b2 {
            self.restore_models(f);
        }
        if flags & SKIP_MAT_ANIM == 0 && self.game.fighters[f.0].x221E_b7 {
            self.restart_costume_anims(f);
        }
        if flags & SKIP_PARASOL == 0 {
            self.game.fighters[f.0].x2221_b4 = false;
            let status = self.parasol_status(gobj);
            if status != -1 && status != 6 {
                self.parasol_step(gobj, 6, 0.0);
            }
        }
        self.end_part_anims(f);
        self.reapply_part_anims(f);
        self.stop_rumble(f, 7);
        self.stop_rumble(f, 8);
        self.stop_rumble(f, 0x24);
        if flags & SKIP_RUMBLE == 0 {
            self.stop_rumble(f, 1);
            self.stop_rumble(f, 0x19);
        }
        if flags & KEEP_COL_ANIM_PART_HIT_STATUS == 0 && self.game.fighters[f.0].x2221_b1 {
            // `ftColl_8007B6EC`.
            let fp = &mut self.game.fighters[f.0];
            fp.x2221_b0 = false;
            if fp.x1990 == 0 {
                fp.x198C = i32::from(fp.x1994 != 0);
                if self.color_anim_id(gobj) == 9 {
                    self.end_color_anim(gobj, 9);
                }
            }
            // `ftColl_8007B760`.
            let x134 = self.common().i32(0x134);
            let fp = &mut self.game.fighters[f.0];
            if x134 > fp.x1990 {
                fp.x1990 = x134;
            }
            fp.x198C = 2;
            self.start_color_anim(gobj, 9, 0);
            self.game.fighters[f.0].x2221_b1 = false;
        }
        let x4cc = self.common().u32(0x4CC) as u16;
        {
            let fp = &mut self.game.fighters[f.0];
            // `ftCommon_8007E2F4`.
            fp.x1A6A = 0;
            if fp.dmg.x18F4 != 0 {
                fp.dmg.x18F4 = 0;
                fp.x2220_b4 = false;
            }
            if flags & UNK_19 == 0 {
                fp.x2222_b2 = false;
            }
            if flags & SKIP_METAL_B == 0 {
                fp.x2223_b4 = false;
            }
            if flags & UNK_27 == 0 {
                fp.x2227_b2 = false;
            }
            if flags & SKIP_HIT_STUN == 0 && fp.x221C_b6 {
                fp.x221C_b6 = false;
                fp.x2098 = x4cc;
            }
            fp.is_sleeping = false;
            fp.x2219_b1 = false;
            fp.x2219_b2 = false;
            fp.dmg.x182c_behavior = 1.0;
            fp.dmg.kb_applied = 0.0;
            fp.dmg.x18A8 = 0.0;
            fp.dmg.armor1 = 0.0;
            fp.dmg.x18a0 = 0.0;
            fp.x221A_b7 = false;
            fp.x221B.x221B_b0 = false;
            fp.x221C_b3 = false;
            fp.shield_unk0 = 0.0;
            fp.shield_unk1 = 0.0;
            fp.x221D_b5 = false;
            fp.reflecting = false;
            fp.x2218_b6 = false;
            fp.x221C_b4 = false;
            fp.x1A6A = 0;
            fp.x221D_b7 = false;
            fp.invisible = false;
            fp.x221E_b1 = false;
            fp.x221E_b2 = false;
            fp.x221F_b1 = false;
            fp.x221E_b5 = false;
            fp.x221E_b6 = false;
            fp.x2220_b3 = false;
            fp.x2220_b7 = false;
            fp.x209C = 0;
            fp.x2223_b0 = false;
            fp.x2222_b3 = false;
            fp.x2224_b5 = false;
            fp.x2225_b1 = false;
            fp.x2225_b4 = false;
            // `mpClearFloorSkip`.
            fp.coll_data.floor_skip = -1;
            // `ftCo_800DEEA8`.
            fp.smash_attrs.state = 0;
            fp.smash_attrs.x2138_smashSinceHitbox = -1.0;
            fp.x2224_b4 = false;
        }
        if flags & SKIP_ITEM_VIS == 0 {
            self.game.fighters[f.0].x221E_b3 = true;
        } else if !self.game.fighters[f.0].x221E_b3 {
            self.item_invisible_hook(gobj);
        }
        {
            let fp = &mut self.game.fighters[f.0];
            if flags & SKIP_MODEL_PART_VIS == 0 {
                fp.x221E_b4 = true;
            }
            if flags & SKIP_MODEL_FLAGS == 0 {
                fp.x2225_b2 = true;
            }
            if flags & KEEP_FAST_FALL == 0 {
                fp.fall_fast = false;
            }
        }
        if flags & SKIP_COL_ANIM == 0 {
            self.reset_x488(gobj);
        }
        if flags & KEEP_GFX == 0 && self.game.fighters[f.0].x2219_b0 {
            self.destroy_effects(gobj);
        }
        if flags & KEEP_ACCESSORY == 0
            && let Some(acc) = self.game.fighters[f.0].x20A0_accessory
        {
            jobj::remove_all(&mut self.game.objs, Some(acc));
            self.game.fighters[f.0].x20A0_accessory = None;
        }
        if self.game.fighters[f.0].ground_or_air == GA_GROUND {
            let fp = &mut self.game.fighters[f.0];
            if fp.kind == KIND_PEACH {
                fp.u[..4].copy_from_slice(&1u32.to_be_bytes());
            }
            fp.x2221_b5 = false;
            fp.x2221_b7 = true;
            fp.x2221_b6 = true;
            fp.x2224_b1 = false;
            fp.x2227_b1 = false;
            fp.x213C = -1;
            let (slot, sub) = (i32::from(fp.player_idx), i32::from(fp.is_sub_fighter));
            if fp.x2227_b4 {
                self.pl_8003fe1c(slot, sub);
                self.game.fighters[f.0].x2227_b4 = false;
            }
            self.game.fighters[f.0].x2227_b5 = false;
            let x2140 = self.game.fighters[f.0].x2140;
            self.lower_xcf8(slot, sub, x2140);
            let fp = &mut self.game.fighters[f.0];
            fp.x2140 = 0.0;
            fp.used_tether = false;
            fp.x2180 = 6;
        }
        if !(14..=17).contains(&msid) {
            self.game.fighters[f.0].hitlag_mul = 0.0;
        }
        if flags & KEEP_SFX == 0 {
            // `ft_80088884`, `ft_800888E0`, `ft_800887CC`.
            self.key_off(f, 66);
            self.game.fighters[f.0].x2150 = -1;
            self.key_off(f, 78);
            self.game.fighters[f.0].x2154 = -1;
            self.key_off(f, 42);
            self.game.fighters[f.0].x2148 = -1;
        }
        {
            let fp = &mut self.game.fighters[f.0];
            if flags & KEEP_SWORD_TRAIL == 0 {
                fp.x2100 = -1;
            }
            if flags & SKIP_NAMETAG_VIS == 0 {
                fp.x209A = 0;
            }
            fp.x2222_b7 = false;
            fp.x100 = if flags & UNK_UPDATE_PHYS != 0 {
                0.0
            } else {
                -1.0
            };
            if flags & UNK_24 == 0 {
                fp.x221C_u16_y = 0;
            }
            fp.lstick_angle = 0.0;
        }
        self.game.part_set_rot_x(f, 0, 0.0);
        let facing = reg(self.game.fighters[f.0].facing_dir);
        let y = mem(fp::frsp(fp::fmul(core::f64::consts::FRAC_PI_2, facing)));
        self.game.part_set_rot_y(f, 0, y);
        self.game.part_set_rot_z(f, 0, 0.0);
        let ms = Self::motion_state(&self.game.fighters[f.0], msid);
        if self.game.fighters[f.0].ground_or_air == GA_GROUND
            && flags & SKIP_HIT_STUN_START == 0
            && ms.x8 & 0x0040_0000 != 0
            && self.game.fighters[f.0].dmg.x18C8 == -1
        {
            let x814 = self.common().i32(0x814);
            self.game.fighters[f.0].dmg.x18C8 = if x814 > 0 { x814 } else { 1 };
        }
        let prev_attack = self.game.fighters[f.0].x2070;
        self.set_attack_id(f, ms.x8 >> 24);
        self.set_attack_flags(f, ms.flags);
        self.game.fighters[f.0].x2225_b3 = ms.x8 & 0x0080_0000 != 0;
        if self.game.fighters[f.0].x2226_b4 {
            if self.game.fighters[f.0].x2070[1] & 0x04 != 0 {
                self.start_color_anim_unless_held(gobj, 0x7E);
            }
            if self.game.fighters[f.0].x2070[1] & 0x02 != 0 {
                self.start_color_anim_unless_held(gobj, 0x7F);
            }
        }
        if let Some(cb) = self.game.fighters[f.0].x21EC {
            run_fighter_cb(self, gobj, cb);
            self.game.fighters[f.0].x21EC = None;
        }
        if flags & SKIP_ATTACK_COUNT == 0 {
            self.count_attack(gobj, prev_attack);
        }
        let mut speed = speed;
        let fp = &mut self.game.fighters[f.0];
        fp.anim_id = ms.anim_id;
        fp.frame_speed_mul = speed;
        fp.x8A0_unk = speed;
        fp.cur_anim_frame = mem(fp::fsubs(reg(start), reg(fp.frame_speed_mul)));
        fp.x898_unk = 0.0;
        let anim_flags = x594_bit(fp, 0) || x594_bit(fp, 5);
        if fp.anim_id != -1 {
            let bone_index =
                ((u32::from(u16::from_be_bytes([fp.x594[2], fp.x594[3]])) >> 6) & 7) as i32;
            if flags & FREEZE_STATE != 0 {
                fp.x2223_b0 = true;
                fp.x104 = 0x14;
                fp.frame_speed_mul = 0.0;
                speed = 0.0;
            }
            let anim = fp.anim_id;
            // With `other`, the other fighter's animation (a thrown fighter plays its
            // thrower's).
            let src = other.map_or(f, |o| self.game.fighter_id(o));
            let sp = &self.game.fighters[src.0];
            let entry = sp
                .x24
                .expect("a fighter without animations")
                .at(ANIM_ENTRY_SIZE * anim as u32);
            let pair = sp
                .x28
                .expect("a fighter without animation pairs")
                .at(2 * anim as u32);
            let fp = &mut self.game.fighters[f.0];
            fp.x594 = self.data.u32(entry.at(0x10)).to_be_bytes();
            self.dynamics_for_anim(f, pair);
            if flags & SKIP_ANIM == 0 {
                self.load_anim(f, src, anim);
                if let Some(o) = other {
                    self.thrown_hitbox_owner(f, o);
                }
                let script = self.data.ptr(entry.at(0xC));
                let s = &mut self.game.fighters[f.0].x3E4_fighterCmdScript.script;
                s.cursor = script;
                s.loop_count = 0;
                let pair0 = self.data.u8(pair);
                let blend_frames = if blend == -1.0 {
                    0.0
                } else if blend != 0.0 {
                    blend
                } else {
                    mem(fp::frsp(f64::from(pair0)))
                };
                let has_tree = self.game.fighters[f.0].x590.is_some();
                if start != 0.0 {
                    if has_tree {
                        let from = fp::fsubs(reg(start), reg(speed));
                        self.start_anim(gobj, from, reg(speed), reg(blend_frames));
                    }
                    self.anim_joints(gobj);
                    let fp = &mut self.game.fighters[f.0];
                    if x594_bit(fp, 0) {
                        fp.x6B0 = Default::default();
                        fp.x6A4_transNOffset = Default::default();
                        fp.x698 = fp.x68C_transNPos;
                    }
                    if x594_bit(fp, 5) {
                        fp.x6E4 = Default::default();
                        fp.x6D8 = Default::default();
                        fp.x6CC = fp.x6C0;
                    }
                    fp.x3E4_fighterCmdScript
                        .script
                        .set_timer_f32(mem(fp::fneg(reg(start))));
                } else {
                    if has_tree {
                        self.start_anim(gobj, reg(start), reg(speed), reg(blend_frames));
                    }
                    self.game.fighters[f.0]
                        .x3E4_fighterCmdScript
                        .script
                        .set_timer_f32(0.0);
                }
                self.anim_joints(gobj);
                if bone_index != 0 && pair0 != 0 {
                    let bone = self.bone(f, bone_index);
                    let (j, j2) = (
                        bone.joint.expect("a part without a joint"),
                        bone.x4_jobj2.expect("a part without a second joint"),
                    );
                    let (t, q) = (
                        self.game.objs.jobjs[j2].translate,
                        self.game.objs.jobjs[j2].rotate,
                    );
                    let jobjs = &mut self.game.objs.jobjs;
                    jobj::set_translate(jobjs, j, t);
                    // `ftParts_JObjSetRotation`.
                    jobj::set_rotation(jobjs, j, q);
                    jobj::clear_flags(jobjs, Some(j), USE_QUATERNION);
                }
                let fp = &mut self.game.fighters[f.0];
                let grounded = fp.ground_or_air == GA_GROUND;
                if x594_bit(fp, 0) {
                    if start == 0.0 {
                        fp.x6B0 = Default::default();
                        fp.x6A4_transNOffset = Default::default();
                        fp.x698 = fp.x68C_transNPos;
                    } else if flags & SKIP_ANIM_VEL == 0 && grounded {
                        let v = mem(fp::fmuls(reg(fp.x6A4_transNOffset.z), reg(fp.facing_dir)));
                        fp.self_vel.x = v;
                        fp.gr_vel = v;
                    }
                }
                if x594_bit(fp, 5) {
                    if start == 0.0 {
                        fp.x6E4 = Default::default();
                        fp.x6D8 = Default::default();
                        fp.x6CC = fp.x6C0;
                    } else if flags & SKIP_ANIM_VEL == 0 && grounded {
                        let v = mem(fp::fmuls(reg(fp.x6D8.z), reg(fp.facing_dir)));
                        fp.self_vel.x = v;
                        fp.gr_vel = v;
                    }
                }
                if flags & UPDATE_CMD != 0 {
                    self.skip_commands(gobj);
                } else if start != 0.0 {
                    self.catch_up_commands(gobj);
                } else {
                    self.color_anim_update(gobj);
                    self.run_commands(gobj);
                }
            } else {
                self.game.fighters[f.0].anim_id = -1;
            }
        }
        if self.game.fighters[f.0].anim_id == -1 {
            self.game.fighters[f.0].x594 = [0; 4];
            self.remove_anim(Some(root));
            let skel = self.game.fighters[f.0].x8AC_animSkeleton;
            self.remove_anim(skel);
            let fp = &mut self.game.fighters[f.0];
            fp.x3E4_fighterCmdScript.script.cursor = None;
            fp.x8A4_animBlendFrames = 0.0;
            fp.x8A8_anim_frame = 0.0;
        }
        let fp = &mut self.game.fighters[f.0];
        if anim_flags && !x594_bit(fp, 0) {
            // `ftCommon_ClampGroundVel`.
            let max = reg(fp.co_attrs.dash_max_velocity);
            let v = reg(fp.gr_vel);
            if v < fp::fneg(max) {
                fp.gr_vel = mem(fp::fneg(max));
            } else if v > max {
                fp.gr_vel = mem(max);
            }
        }
        fp.anim_cb = ms.anim_cb;
        fp.input_cb = ms.input_cb;
        fp.phys_cb = ms.phys_cb;
        fp.coll_cb = ms.coll_cb;
        fp.cam_cb = ms.cam_cb;
        fp.accessory1_cb = None;
        fp.accessory4_cb = None;
        fp.deal_dmg_cb = None;
        fp.shield_hit_cb = None;
        fp.reflect_hit_cb = None;
        fp.hitlag_cb = None;
        fp.x21CC = None;
        fp.post_hitlag_cb = None;
        fp.pre_hitlag_cb = None;
        fp.take_dmg_cb = None;
        fp.take_dmg_2_cb = None;
        fp.hurtbox_detect_cb = None;
        fp.x21F8 = None;
        fp.death2_cb = None;
    }
}
