//! A fighter's spawn (`fighter.c`'s `Fighter_Spawn`): its state reset for a new stock, at its
//! player's position, with its attributes, collision, camera box, color and hurt capsules set
//! up and its computer player's state; and the entry state a match's fighters start in.

use gekko_fp as fp;

use super::super::code::unported_call;
use super::super::handles::FighterId;
use super::super::state::Running;
use super::super::structs::{Fighter, ftCollisionBox};
use crate::fpu::{copy, mem, reg};
use crate::game::gr::GrKind;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::mp::coll::Coll;
use crate::vec::Vec3;

/// `ftCo_MS_Entry`.
const MS_ENTRY: i32 = 322;

impl Running<'_> {
    /// Spawns the fighter, `Fighter_Spawn`.
    #[doc(alias = "ft_800890BC")]
    #[doc(alias = "ftCo_800C0074")]
    #[doc(alias = "Fighter_Spawn")]
    pub fn fighter_spawn(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.init_reset(f);
        let root = self.root_jobj(gobj);
        let pos = self.game.fighters[f.0].cur_pos;
        jobj::set_translate(&mut self.game.objs.jobjs, root, pos);
        self.calc_attributes(gobj);
        self.init_bury(f);
        self.game.clear_velocities(f);
        self.key_off_all(f);
        let fp = &mut self.game.fighters[f.0];
        fp.attack_id = 1;
        fp.attack_instance = 0;
        clear_hit_records(fp);
        self.init_coll(gobj);
        self.init_x1930(f);
        let fp = &self.game.fighters[f.0];
        if fp.smash_attrs.x2135 == -1
            && self.land_on_spawn(f)
            && !self.game.fighters[f.0].spawn_in_air
        {
            self.touch_ground(gobj);
        } else {
            self.go_airborne(f);
        }
        self.init_camera_box(f);
        let pos = self.game.fighters[f.0].cur_pos;
        jobj::set_translate(&mut self.game.objs.jobjs, root, pos);
        self.depth_scale_matrix(gobj);
        self.update_model_scale(gobj);
        let fp = &mut self.game.fighters[f.0];
        crate::lb::colanim::clear(&mut fp.secondary_overlay);
        crate::lb::colanim::clear(&mut fp.main_overlay);
        self.init_metal(gobj);
        self.init_clear(gobj);
        self.init_x2034(gobj);
        self.clear_hitboxes(f);
        self.set_hurtbox_state(f, 0);
        self.on_death(gobj);
        let slot = i32::from(self.game.fighters[f.0].player_idx);
        let (kind, level) = (
            self.game.player_cpu_type(slot),
            self.game.player_cpu_level(slot),
        );
        self.init_cpu(f, kind, level, 0);
        let mut head = self.game.fighters[f.0].effect_queue;
        crate::game::ef::queue_clear(self.game, &mut head);
        self.game.fighters[f.0].effect_queue = head;
        self.init_thrown_hitbox(gobj);
        self.init_stamina_markers(f);
    }

    /// Resets the fighter for a spawn at its player's position, `Fighter_UnkInitReset_80067C98`.
    #[doc(alias = "ftCo_800DEEA8")]
    #[doc(alias = "Fighter_UnkInitReset_80067C98")]
    pub(crate) fn init_reset(&mut self, f: FighterId) {
        let spawn = self.game.new_spawn_number();
        let slot = i32::from(self.game.fighters[f.0].player_idx);
        let coords = self.game.player_pos(slot);
        let facing = self.game.player_facing(slot);
        let damage = self.game.player_damage(slot);
        let shield = self.common().f32(0x260);
        let fp = &mut self.game.fighters[f.0];
        fp.spawn_num = spawn as i32;
        fp.facing_dir = facing;
        // `ftCommon_800804EC`: `x40` in the model's scale.
        let offset = fp::fmuls(reg(fp.platform_offset), reg(fp.scale.x));
        let x = mem(fp::fmadds(reg(fp.facing_dir), offset, reg(coords.x)));
        fp.cur_pos = Vec3::new(x, coords.y, coords.z);
        fp.prev_pos = fp.cur_pos;
        fp.facing_dir1 = fp.facing_dir;
        fp.scale.y = fp.scale.x;
        fp.giant = false;
        fp.tiny = false;
        fp.queued_grows = 0;
        fp.queued_shrinks = 0;
        fp.mushroom_frames = 0;
        fp.out_of_play = false;
        fp.blast_immune_state = false;
        fp.hitboxes_set = false;
        fp.hammer_hits_held = false;
        fp.hurtboxes_set = false;
        fp.hurtboxes_custom = false;
        fp.variants_hidden = false;
        fp.costume_anim_set = false;
        fp.camera_placed = false;
        fp.parasol_open = false;
        fp.parasol_checked = false;
        fp.parasol_blend = true;
        fp.parasol_from_jump = false;
        fp.alpha = 255;
        fp.pos_delta.z = 0.0;
        fp.pos_delta.y = 0.0;
        fp.pos_delta.x = 0.0;
        fp.cur_anim_frame = 0.0;
        fp.cmd_frame_offset = 0.0;
        fp.frame_speed_mul = 1.0;
        fp.freeze_rate_step = 1.0;
        fp.dmg.kb_applied = 0.0;
        fp.dmg.x18A4_knockbackMagnitude = 0.0;
        fp.dmg.x18A8 = 0.0;
        fp.dmg.x18ac_time_since_hit = -1;
        fp.dmg.armor0 = 0.0;
        fp.dmg.armor1 = 0.0;
        fp.hit_reaction = 0;
        fp.in_hitstun = false;
        fp.dmg.x18a0 = 0.0;
        fp.jumps_used = 0;
        fp.walljumps_used = 0;
        fp.hitlag_mul = 0.0;
        fp.ledge_cooldown = 0;
        fp.dmg.x1838_percentTemp = 0.0;
        fp.dmg.x183C_applied = 0;
        fp.dmg.x18C0 = 0;
        fp.dmg.x18c4_source_ply = 6;
        fp.dmg.x18C8 = -1;
        fp.dmg.x18F0 = 0;
        fp.dmg.x18CC = 0;
        fp.dmg.x18D0 = -10;
        fp.last_hit_by_sub = false;
        fp.damage_col_anim = false;
        fp.dmg.x18F4 = 0;
        fp.dmg.x18F8 = 1;
        fp.dmg.x18fa_model_shift_frames = 0;
        fp.dmg.x18FD = 0;
        fp.dmg.x18FC = 0;
        fp.dmg.x1834 = 0.0;
        fp.no_turn = false;
        fp.dmg.x1840 = 0;
        fp.in_hitlag = false;
        fp.was_in_hitlag = false;
        fp.hitlag_held = false;
        fp.pending_hitlag = false;
        fp.thaw_size = false;
        fp.dmg.x1954 = 0.0;
        fp.dmg.x1958 = 0.0;
        fp.allow_sdi = false;
        fp.dmg.x195c_hitlag_frames = 0.0;
        fp.in_hitstun_sdi = false;
        fp.hitlag_vibrate_mul = 1.0;
        fp.min_hitlag = 0.0;
        fp.dmg.x189C_unk_num_frames = 0.0;
        fp.buried = false;
        fp.turn_spin = false;
        fp.dmg.x1914 = 0;
        fp.dmg.int_value = 0;
        fp.dmg.x191C = 0.0;
        fp.dmg.x1924 = 0;
        fp.dmg.x1928 = 0.0;
        fp.stepped_on_item = false;
        fp.dmg.x1950 = 0;
        fp.dmg.x1948 = 0;
        fp.no_item_bounce = false;
        fp.nudge_vel.y = 0.0;
        fp.nudge_vel.x = 0.0;
        fp.follows_holder = false;
        fp.anim_frozen = false;
        fp.fall_fast = false;
        fp.effect_once = false;
        fp.accessory = None;
        fp.cmd_timer = 0.0;
        fp.item_gobj = None;
        fp.second_item = None;
        fp.item_visible = true;
        fp.special_item = None;
        fp.hit_status = 0;
        fp.timed_hit_status = 0;
        fp.warp_star_intangible = false;
        fp.intangible_frames = 0;
        fp.invincible_frames = 0;
        fp.starman = false;
        fp.state_flags.x221B_b5 = false;
        fp.victim_gobj = None;
        fp.partner = None;
        fp.state_flags.x221B_b6 = false;
        fp.target_item_gobj = None;
        fp.caught_item = Default::default();
        fp.state_flags.x221B_b7 = false;
        fp.held_hit_light = false;
        fp.leg_reach = 0;
        fp.hit_by = None;
        fp.hit_this_frame = false;
        fp.hammer_music = 0;
        fp.star_cheer = 0;
        fp.combo_attack_id = 0;
        fp.combo_count = 0;
        fp.combo_window = 0;
        fp.combo_push_frames = 0;
        fp.combo_victim = None;
        fp.shield_was_hit = false;
        fp.state_flags.x221B_b0 = false;
        fp.state_flags.x221B_b1 = false;
        fp.state_flags.x221B_b3 = false;
        fp.state_flags.x221B_b4 = false;
        fp.powershield_early = false;
        fp.powershield_late = false;
        fp.powershield = false;
        fp.shield_damage_taken = 0;
        fp.shield_hit_damage = 0;
        fp.lightshield_amount = 0.0;
        fp.shield_hit_by = None;
        fp.counter_min_hitlag = 0.0;
        fp.counter_item_min_hitlag = 0.0;
        fp.shield_hit_slot = 6;
        fp.shield_hit_by_sub = false;
        fp.reflecting = false;
        fp.reflect_kept = false;
        fp.reflect.x1A3C_damageOver = 0;
        fp.reflect.x1A2C_reflectHitDirection = 0.0;
        fp.absorbing = false;
        fp.absorb_any = false;
        fp.absorb.x1A40_absorbHitDirection = 0.0;
        fp.absorb.x1A44_damageTaken = 0;
        fp.absorb.x1A48_hitsTaken = 0;
        fp.lstick_angle = 0.0;
        fp.air_grab_timer = 0;
        fp.air_item_caught = false;
        fp.death2_cb = None;
        fp.afterimage_mode = -1;
        fp.afterimage_slot = 0;
        fp.accessory2_cb = None;
        fp.accessory3_cb = None;
        fp.death1_cb = None;
        fp.death3_cb = None;
        fp.articles_visible = true;
        fp.bunny_hood = None;
        fp.is_metal = false;
        fp.metal_timer = 0;
        fp.metal_health = 0;
        fp.alt_metal = false;
        fp.alt_metal_timer = 0;
        fp.alt_metal_health = 0;
        fp.flower = None;
        fp.wind_immune = false;
        fp.capture_timer = 0;
        fp.mash_shaking = false;
        fp.mash_shake_rows = 0;
        fp.mash_shake_row = 0;
        fp.wall_jump_input_timer = 254;
        fp.dmg.x1910 = 0;
        fp.map_coll_done = false;
        fp.hat_hidden = true;
        fp.hits_others_team = false;
        fp.dmg.x18BC = 0.0;
        fp.dmg.x18B8 = 0.0;
        fp.held_in_model = false;
        fp.hold_dy = 0.0;
        fp.dmg.x1908 = -1;
        fp.dmg.x190C = None;
        fp.tumble_early = false;
        fp.ledge_caught = -1;
        fp.special_air_used = false;
        fp.flat_floor_chains = false;
        fp.lowest_y = 0.0;
        fp.held_in_ice = false;
        fp.dead_holder_slot = 6;
        fp.coin_odd_frame = true;
        fp.dmg.x1830_percent = mem(fp::frsp(f64::from(damage)));
        fp.part_35_rot_x = -1.0;
        fp.throw_flags = [0; 4];
        fp.thrown_hitbox.x134 = Default::default();
        fp.sound_handle_4 = -1;
        fp.sound_handle_5 = -1;
        fp.sound_handle_6 = -1;
        fp.track_handle = -1;
        fp.sfx_handle = -1;
        fp.voice_handle = -1;
        fp.sound_handle_2 = -1;
        fp.sound_handle_3 = -1;
        fp.shield_health = shield;
        fp.trans_n_pos = Vec3::new(0.0, 0.0, 0.0);
        fp.trans_n_offset = Vec3::new(0.0, 0.0, 0.0);
        fp.part_35_pos = Vec3::new(0.0, 0.0, 0.0);
        fp.part_35_offset = Vec3::new(0.0, 0.0, 0.0);
        // `ftCo_800C88A0`.
        fp.cloak = false;
        fp.refracted = false;
        fp.smash_attrs.x2134_vibrateFrame = 0;
        fp.cloak_frames = 0;
        fp.cloak_look = false;
        fp.stamina_dead = false;
        fp.stamina_out = false;
        // `ftCo_800DEEA8`: no smash attack charging.
        fp.smash_attrs.state = 0;
        fp.voice_muted = fp.rules_voice_muted;
        fp.smash_attrs.x2138_smashSinceHitbox = -1.0;
    }

    /// The multi-jumps' rises (`Fighter_x2D0_t::x14`, `x28` of them) scaled by the fighter's
    /// height (`Fighter_804D6524` x28), `ftCo_800D0CBC`.
    #[doc(alias = "ftCo_800D0CBC")]
    pub fn scale_multi_jumps(&mut self, f: FighterId) {
        let fp = &self.game.fighters[f.0];
        let attrs = fp
            .multi_jump_attrs
            .expect("multi-jumps without their attributes");
        let word = |s: &Self, off: usize| {
            let b = &s.game.fighter_attrs[attrs.0][off..off + 4];
            u32::from_be_bytes([b[0], b[1], b[2], b[3]])
        };
        let count = word(self, 0x28) as i32;
        let mut scale = 1.0;
        let scale_y = reg(fp.scale.y);
        if scale != scale_y {
            let p = super::common_ptr(self.game, self.data, super::SCALE_PARAMS)
                .expect("no fighter scale parameters");
            let k = reg(self.data.f32(p.at(0x28)));
            scale = fp::fmuls(scale, super::super::it::owner::y_scaled(1.0, scale_y, k));
        }
        if fp.bunny_hood.is_some() {
            let t = super::common_ptr(self.game, self.data, super::powerup::BUNNY_PARAMS)
                .expect("no Bunny Hood parameters");
            scale = fp::fmuls(scale, reg(self.data.f32(t.at(0x14))));
        }
        if fp.is_metal {
            let t = super::common_ptr(self.game, self.data, super::powerup::METAL_PARAMS)
                .expect("no metal parameters");
            scale = fp::fmuls(scale, reg(self.data.f32(t.at(0x4))));
        }
        for i in 0..count.max(0) as usize {
            let off = 0x14 + 4 * i;
            let v = mem(fp::fmuls(reg(f32::from_bits(word(self, off))), scale));
            self.game.fighter_attrs[attrs.0][off..off + 4]
                .copy_from_slice(&v.to_bits().to_be_bytes());
        }
    }

    /// The fighter's attributes from its character's data, in its model's scale and changed
    /// by its items and rules, then its character's own, `ftCo_800D105C`.
    #[doc(alias = "ftCo_800D105C")]
    pub fn calc_attributes(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.copy_attributes(f);
        let fp = &self.game.fighters[f.0];
        if reg(fp.scale.y) != 1.0 {
            self.scale_attributes(f);
        }
        if self.game.fighters[f.0].bunny_hood.is_some() {
            let data = self.data;
            let t = super::common_ptr(self.game, data, super::powerup::BUNNY_PARAMS)
                .expect("no Bunny Hood parameters");
            let k = |o: u32| reg(data.f32(t.at(o)));
            let (k4, k8, kc, k10, k14, k1c, k18, k20, k24, k28, k2c, k30, k34, k38) = (
                k(0x4),
                k(0x8),
                k(0xC),
                k(0x10),
                k(0x14),
                k(0x1C),
                k(0x18),
                k(0x20),
                k(0x24),
                k(0x28),
                k(0x2C),
                k(0x30),
                k(0x34),
                k(0x38),
            );
            let a = &mut self.game.fighters[f.0].co_attrs;
            let mul = |v: &mut f32, k: f64| *v = mem(fp::fmuls(reg(*v), k));
            mul(&mut a.dash_accel_mul, k4);
            mul(&mut a.dash_accel_base, k8);
            mul(&mut a.dash_max_velocity, kc);
            mul(&mut a.jump_h_initial_velocity, k10);
            mul(&mut a.jump_v_initial_velocity, k14);
            mul(&mut a.jump_h_max_velocity, k1c);
            mul(&mut a.hop_v_initial_velocity, k18);
            mul(&mut a.gravity, k20);
            mul(&mut a.terminal_velocity, k24);
            mul(&mut a.fast_fall_velocity, k28);
            mul(&mut a.ledge_jump_horizontal_velocity, k2c);
            mul(&mut a.ledge_jump_vertical_velocity, k30);
            mul(&mut a.wall_jump_horizontal_velocity, k34);
            mul(&mut a.wall_jump_vertical_velocity, k38);
        }
        if self.game.fighters[f.0].is_metal {
            let data = self.data;
            let t = super::common_ptr(self.game, data, super::powerup::METAL_PARAMS)
                .expect("no metal parameters");
            let k = |o: u32| reg(data.f32(t.at(o)));
            let (k4, k8, kc, k10, k14, k18, k1c, k20) = (
                k(0x4),
                k(0x8),
                k(0xC),
                k(0x10),
                k(0x14),
                k(0x18),
                k(0x1C),
                k(0x20),
            );
            let a = &mut self.game.fighters[f.0].co_attrs;
            let mul = |v: &mut f32, k: f64| *v = mem(fp::fmuls(reg(*v), k));
            mul(&mut a.jump_v_initial_velocity, k4);
            mul(&mut a.hop_v_initial_velocity, k8);
            mul(&mut a.gravity, kc);
            mul(&mut a.terminal_velocity, k10);
            mul(&mut a.fast_fall_velocity, k14);
            mul(&mut a.weight, k18);
            mul(&mut a.ledge_jump_vertical_velocity, k1c);
            mul(&mut a.wall_jump_vertical_velocity, k20);
        }
        if self.game.fighters[f.0].attrs_scaled {
            let t = self.load_common(0x3C);
            let (g, w) = (reg(self.data.f32(t)), reg(self.data.f32(t.at(4))));
            let a = &mut self.game.fighters[f.0].co_attrs;
            a.gravity = mem(fp::fmuls(reg(a.gravity), g));
            a.weight = mem(fp::fmuls(reg(a.weight), w));
        }
        self.load_special_attrs(gobj);
        if self.game.fighters[f.0].multi_jump_attrs.is_some() {
            self.scale_multi_jumps(f);
        }
        if self.game.fighters[f.0].has_cargo {
            self.dk_scale_cargo_walk_rates(f);
        }
    }

    /// The fighter's burial timers for the stage, `ftCo_800C09B4`.
    #[doc(alias = "ftCo_800C09B4")]
    fn init_bury(&mut self, f: FighterId) {
        let grkind = self.game.stage_info.grkind;
        let fp = &mut self.game.fighters[f.0];
        fp.bury_grkind = grkind;
        fp.bury_timer_1 = 0;
        if matches!(
            GrKind::from_id(grkind),
            Some(
                GrKind::OldKongo
                    | GrKind::Kongo
                    | GrKind::ShrineRoute
                    | GrKind::Pushon
                    | GrKind::Zebes
            )
        ) {
            fp.bury_timer_2 = 0;
        }
    }

    /// Stops the sounds on all the fighter's tracks, and the two it holds,
    /// `ft_80088A50`.
    #[doc(alias = "ft_80088A50")]
    fn key_off_all(&mut self, f: FighterId) {
        let slot = i32::from(self.game.fighters[f.0].player_idx);
        let track = |base: i32| base + 2 * slot;
        for (bases, field) in [
            ([54, 55], 0x214C),
            ([30, 31], 0x2144),
            ([42, 43], 0x2148),
            ([66, 67], 0x2150),
            ([78, 79], 0x2154),
            ([90, 91], 0x2158),
            ([114, 115], 0x215C),
        ] {
            for b in bases {
                self.sfx_play_track(crate::lb::audio::SFX_KEY_OFF, 0, 0x40, track(b));
            }
            let fp = &mut self.game.fighters[f.0];
            match field {
                0x214C => fp.sound_handle_3 = -1,
                0x2144 => fp.voice_handle = -1,
                0x2148 => fp.sound_handle_2 = -1,
                0x2150 => fp.sound_handle_4 = -1,
                0x2154 => fp.sound_handle_5 = -1,
                0x2158 => fp.sound_handle_6 = -1,
                _ => fp.track_handle = -1,
            }
        }
        let fp = &mut self.game.fighters[f.0];
        fp.sfx_handle = -1;
        let (a, b) = (fp.star_cheer, fp.hammer_music);
        self.lb_audio_crowd_a_less(a);
        self.game.fighters[f.0].star_cheer = 0;
        self.lb_audio_crowd_b_less(b);
        let fp = &mut self.game.fighters[f.0];
        fp.hammer_music = 0;
        fp.star_cheer = 0;
    }

    /// The fighter's collision data, its ECB fitted around its bones and its ledge snap in
    /// its scale, `ft_80081B38`.
    #[doc(alias = "ft_80081B38")]
    fn init_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let parts = fp.parts.expect("a fighter without parts").0;
        let x44 = self
            .data
            .ptr(fp.ft_data.expect("a fighter without data").at(0x44))
            .expect("ftData::x44");
        let joint = |i: u32| {
            let b = self.data.u16(x44.at(2 * i)) as usize;
            self.game.fighter_parts[parts][b].joint
        };
        let joints = [joint(0), joint(1), joint(2), joint(3), joint(4), joint(5)];
        let root = self.game.fighter_parts[parts][0].joint;
        let scale = reg(fp.scale.y);
        let x124 = mem(fp::fmuls(reg(self.data.f32(x44.at(0xC))), scale));
        let snap = |o: u32| mem(fp::fmuls(reg(self.data.f32(x44.at(o))), scale));
        let (sx, sy, sh) = (snap(0x10), snap(0x14), snap(0x18));
        let weight = fp.co_attrs.weight;
        let x128 = mem(fp::fmuls(10.0, scale));
        let game = &mut *self.game;
        let fp = &mut game.fighters[f.0];
        fp.ecb_lock = 0;
        fp.coll_data.cur_pos = fp.cur_pos;
        fp.coll_data.last_pos = fp.coll_data.cur_pos;
        let landing = crate::game::gr::yoshis_island::Landing::of(game);
        let geometry = game.geometry.as_mut().expect("no collision geometry");
        let mut hooks = super::coll::ObjCollHooks {
            objs: &mut game.objs,
            data: self.data,
            landing,
            kicks: Default::default(),
            bumps: Default::default(),
            escape: Default::default(),
            climb: Default::default(),
            maze: Default::default(),
            // A fighter's pass touches no coin (`grFigureGet_80219B10` takes items alone).
            coins: Default::default(),
            grounds: &mut game.grounds,
            stage_maps: super::coll::StageMaps::of_parts(
                &game.world,
                &game.stage_info,
                &game.gr,
                &game.players,
            ),
            filter: None,
        };
        let mut coll = Coll {
            g: geometry,
            s: &mut game.coll,
            hooks: &mut hooks,
        };
        let cd = &mut game.fighters[f.0].coll_data;
        coll.init(cd);
        cd.x34_flags.b1234 = 1;
        cd.set_ecb_source_joints(Some(gobj), root, joints, x124);
        cd.set_ledge_snap(sx, sy, sh);
        cd.x50 = weight;
        if cd.ecb_source.kind == crate::mp::coll::ECB_SOURCE_JOBJ {
            cd.ecb_source.min_height = x128;
            cd.ecb_source.min_width = x128;
        }
    }

    /// `dmg.x1930` from the ECB's sides and the fighter's height, `ft_80081938`.
    #[doc(alias = "ft_80081938")]
    fn init_x1930(&mut self, f: FighterId) {
        let fp = &mut self.game.fighters[f.0];
        let p = fp.coll_data.cur_pos;
        let (lx, rx) = (fp.coll_data.ecb.left.x, fp.coll_data.ecb.right.x);
        let v = &mut fp.dmg.x1930;
        v.xC.x = mem(fp::fadds(reg(lx), reg(p.x)));
        v.xC.y = mem(fp::fadds(reg(rx), reg(p.x)));
        v.xC.z = mem(fp::fadds(reg(fp.cur_pos.y), reg(fp.co_attrs.x17C)));
        v.x0 = v.xC;
    }

    /// Whether the fighter has a floor within 10 below where it spawns, put on it if so,
    /// `ft_80082A68` (mpcoll's airborne pass from 10 above to 10 below).
    #[doc(alias = "ft_80082A68")]
    fn land_on_spawn(&mut self, f: FighterId) -> bool {
        let game = &mut *self.game;
        let landing = crate::game::gr::yoshis_island::Landing::of(game);
        let geometry = game.geometry.as_mut().expect("no collision geometry");
        let mut hooks = super::coll::ObjCollHooks {
            objs: &mut game.objs,
            data: self.data,
            landing,
            kicks: Default::default(),
            bumps: Default::default(),
            escape: Default::default(),
            climb: Default::default(),
            maze: Default::default(),
            // A fighter's pass touches no coin (`grFigureGet_80219B10` takes items alone).
            coins: Default::default(),
            grounds: &mut game.grounds,
            stage_maps: super::coll::StageMaps::of_parts(
                &game.world,
                &game.stage_info,
                &game.gr,
                &game.players,
            ),
            filter: None,
        };
        let mut coll = Coll {
            g: geometry,
            s: &mut game.coll,
            hooks: &mut hooks,
        };
        let fp = &mut game.fighters[f.0];
        let cd = &mut fp.coll_data;
        cd.cur_pos = fp.cur_pos;
        cd.last_pos = cd.cur_pos;
        cd.last_pos.y = mem(fp::fadds(reg(cd.last_pos.y), 10.0));
        cd.cur_pos.y = mem(fp::fsubs(reg(cd.cur_pos.y), 10.0));
        let landed = coll.air_frame(cd, 6, 0);
        if landed {
            fp.cur_pos = fp.coll_data.cur_pos;
        }
        let kicks = std::mem::take(&mut hooks.kicks);
        let bumps = std::mem::take(&mut hooks.bumps);
        let escape = std::mem::take(&mut hooks.escape);
        let climb = std::mem::take(&mut hooks.climb);
        let maze = std::mem::take(&mut hooks.maze);
        kicks.apply(game, self.data);
        escape.apply(game);
        climb.apply(game);
        maze.apply(game);
        bumps.apply(&mut Running {
            game,
            data: self.data,
        });
        landed
    }

    /// Fits the fighter's camera box to it, `ftCamera_80076064`.
    #[doc(alias = "ftCamera_80076064")]
    pub(crate) fn init_camera_box(&mut self, f: FighterId) {
        let fp = &self.game.fighters[f.0];
        let b = fp.camera_box.expect("a fighter without a camera box");
        let x3c = self
            .data
            .ptr(fp.ft_data.expect("a fighter without data").at(0x3C))
            .expect("ftData::x3C");
        let s = reg(fp.scale.y);
        let v = |o: u32| mem(fp::fmuls(reg(self.data.f32(x3c.at(o))), s));
        let (x0, y0, z0) = (v(0), v(4), v(8));
        let (xc, yc, zc) = (v(0xC), v(0x10), v(0x14));
        let zoom = reg(self.game.stage_info.cam_info.cam_fixed_zoom);
        let (facing, pos) = (fp.facing_dir, fp.cur_pos);
        let cb = self.game.camera.subject_mut(b);
        cb.state = 0;
        if facing == 1.0 {
            cb.target_ext.h.x = z0;
            cb.target_ext.h.y = mem(fp::fmuls(reg(y0), zoom));
            cb.facing_dir = 1.0;
        } else {
            cb.target_ext.h.x = mem(fp::fmuls(fp::fneg(reg(y0)), zoom));
            cb.target_ext.h.y = mem(fp::fneg(reg(z0)));
            cb.facing_dir = -1.0;
        }
        cb.target_ext.v.x = xc;
        cb.target_ext.v.y = yc;
        cb.target_ext.v.z = zc;
        cb.pos.x = pos.x;
        cb.pos.y = mem(fp::fadds(reg(pos.y), reg(x0)));
        cb.pos.z = pos.z;
        cb.bone_pos = cb.pos;
        self.game.camera.fit_extents(b);
    }

    /// Scales the fighter's model to its model scale (`x34_scale.z` across, if set),
    /// `Fighter_UpdateModelScale`.
    #[doc(alias = "Fighter_UpdateModelScale")]
    pub(crate) fn update_model_scale(&mut self, gobj: GObjId) {
        let root = self.root_jobj(gobj);
        let fp = self.game.fighter(gobj);
        let model = mem(Running::model_scale(fp));
        let x = if reg(fp.scale.z) != 1.0 {
            fp.scale.z
        } else {
            model
        };
        jobj::set_scale(&mut self.game.objs.jobjs, root, Vec3::new(x, model, model));
    }

    /// The fighter's metal state from its player's rules, `ftCo_800C8438`.
    #[doc(alias = "ftCo_800C8438")]
    fn init_metal(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.is_metal = fp.is_always_metal;
        fp.metal_timer = 0;
        fp.metal_health = 0;
        if fp.is_metal {
            let f = self.game.fighter_id(gobj);
            self.shine_parts_metal(f);
            self.calc_attributes(gobj);
            let scale = self.game.fighters[f.0].scale.y;
            self.scale_ledge_snap(gobj, scale);
        }
    }

    /// The fighter's clear state from its player's rules, and its color overlay reset,
    /// `ftCo_800C89A0`.
    #[doc(alias = "ftCo_800C89A0")]
    fn init_clear(&mut self, gobj: GObjId) {
        self.clear_cloak(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.cloak = fp.takes_damage_cloaked;
        if fp.cloak {
            self.start_cloak_look(gobj);
        }
        self.reset_cloak_overlay(gobj);
    }

    /// `x2034` and `x2038` from the common data when the player's rules set it,
    /// `ftCo_800C8FC4`.
    #[doc(alias = "ftCo_800C8FC4")]
    fn init_x2034(&mut self, gobj: GObjId) {
        let slot = i32::from(self.game.fighter(gobj).player_idx);
        if self.game.player_more_flags_bit0(slot) {
            unported_call("ftCo_800C8FC4 with its rule set");
        }
    }

    /// The fighter's thrown hitbox on its character's bone, `ft_8007C17C`.
    #[doc(alias = "ft_8007C17C")]
    fn init_thrown_hitbox(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let x34 = self
            .data
            .ptr(fp.ft_data.expect("a fighter without data").at(0x34))
            .expect("ftData::x34");
        let bone = self.data.u32(x34) as i32;
        let scale = copy(self.data.f32(x34.at(4)));
        let joint = self.bone(f, bone).joint;
        let fp = &mut self.game.fighters[f.0];
        let h = &mut fp.thrown_hitbox;
        h.x4 = 0;
        h.state = 1;
        // `lbColl_80008440`.
        for v in &mut h.victims_1 {
            v.victim = Default::default();
        }
        h.x44 = 0;
        for v in &mut h.victims_2 {
            v.victim = Default::default();
        }
        h.x45 = 0;
        h.jobj.set(joint);
        h.scale = scale;
        h.b_offset = Vec3::new(0.0, 0.0, 0.0);
        h.x43_b1 = false;
        h.x43_b2 = false;
        fp.thrown_hitbox_live = false;
        self.thrown_hitbox_follow(gobj);
    }

    /// The fighter's two `x1614` points on their bones, `ft_8007C630`.
    #[doc(alias = "ft_8007C630")]
    #[doc(alias("init_x1614"))]
    fn init_stamina_markers(&mut self, f: FighterId) {
        let fp = &self.game.fighters[f.0];
        let x38 = self
            .data
            .ptr(fp.ft_data.expect("a fighter without data").at(0x38))
            .expect("ftData::x38");
        for i in 0..2u32 {
            let e = x38.at(0x14 * i);
            let bone = self.data.u32(e) as i32;
            let joint = self.bone(f, bone).joint;
            let offset = crate::game::mirror::read_vec3(self.data, e.at(4));
            let x0 = copy(self.data.f32(e.at(0x10)));
            let v = crate::lb::joint::world_position(
                &mut self.game.objs,
                self.data,
                joint,
                Some(offset),
            );
            let p = &mut self.game.fighters[f.0].stamina_markers[i as usize];
            p.x4 = joint;
            p.x0 = x0;
            p.x8 = Vec3::new(v.x, v.y, 0.0);
            p.x14 = p.x8;
        }
    }

    /// The entry state a match's fighters start in, `ftCo_800C61B0`: hidden, intangible and
    /// still, flattened to the common data's height, for its player's entry delay.
    #[doc(alias = "ft_80084CB0")]
    #[doc(alias = "ftCo_800C61B0")]
    pub fn enter_entry(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let root = self.root_jobj(gobj);
        let slot = i32::from(self.game.fighters[f.0].player_idx);
        let timer = self.game.player_entry_delay(slot);
        let height = self.common().f32(0x6C4);
        let scale = self.game.objs.jobjs[root].scale;
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(0, timer);
        set_mv_vec(fp, 8, scale);
        let squashed = Vec3::new(scale.x, height, scale.z);
        set_mv_vec(fp, 0x14, squashed);
        jobj::set_scale(&mut self.game.objs.jobjs, root, squashed);
        let fp = &mut self.game.fighters[f.0];
        let y = fp.cur_pos.y;
        fp.mv[4..8].copy_from_slice(&y.to_bits().to_be_bytes());
        self.change_motion_state(gobj, MS_ENTRY, 0, 0.0, 1.0, 0.0, None);
        let fp = &mut self.game.fighters[f.0];
        fp.invisible = true;
        fp.out_of_play = true;
        fp.dead_flag = true;
        fp.entering = true;
        fp.dead = true;
        // `ft_80084CB0`: the ECB's box now.
        let e = &fp.coll_data.ecb;
        let b = ftCollisionBox {
            top: e.top.y,
            bottom: e.bottom.y,
            right: super::super::structs::Vec2 {
                x: e.right.x,
                y: e.right.y,
            },
            left: super::super::structs::Vec2 {
                x: e.left.x,
                y: e.left.y,
            },
        };
        set_mv_box(fp, 0x2C, &b);
    }
}

/// A vector of the state's variables, `off` bytes into `mv`, as its words.
fn set_mv_vec(fp: &mut Fighter, off: usize, v: Vec3) {
    for (i, x) in [v.x, v.y, v.z].into_iter().enumerate() {
        fp.mv[off + 4 * i..off + 4 * i + 4].copy_from_slice(&x.to_bits().to_be_bytes());
    }
}

/// An ECB box among the state's variables.
fn set_mv_box(fp: &mut Fighter, off: usize, b: &ftCollisionBox) {
    for (i, x) in [b.top, b.bottom, b.left.x, b.left.y, b.right.x, b.right.y]
        .into_iter()
        .enumerate()
    {
        fp.mv[off + 4 * i..off + 4 * i + 4].copy_from_slice(&x.to_bits().to_be_bytes());
    }
}

/// Clears the records of what the fighter last hit and was hit by, `ft_800892D4`.
#[doc(alias = "ft_800892D4")]
fn clear_hit_records(fp: &mut Fighter) {
    fp.dmg.x18d4 = [0; 4];
    fp.attack_flags = fp.dmg.x18d4;
    let r = &mut fp.attack_record;
    r.x2074_vec.x = 0;
    r.x2074_vec.y = 0;
    r.x207C.x = 0.0;
    r.x207C.y = 6;
    // `x2084`'s bits only: the first byte's eight and the second's top five.
    r.x10[0] = 0;
    r.x10[1] &= 0x07;
    r.x2088 = 0;
    let d = &mut fp.dmg.x18d8;
    d.x0 = 0;
    d.x4 = 0;
    d.kb_applied1 = 0.0;
    d.xC = 6;
    d.x10_b0 = false;
    d.x11_b3 = false;
    d.x10_b1 = false;
    d.x10_b2 = false;
    d.x10_b3 = false;
    d.x10_b4 = false;
    d.x10_b5 = false;
    d.x10_b6 = false;
    d.x10_b7 = false;
    d.x11_b0 = false;
    d.x11_b1 = false;
    d.x11_b2 = false;
    d.x11_b4 = false;
    fp.dmg.x18ec_instancehitby = 0;
}

impl super::super::state::Game {
    /// Zeroes the fighter's velocities and accelerations, `ftCommon_8007E2FC`.
    #[doc(alias = "ftCommon_8007E2FC")]
    pub fn clear_velocities(&mut self, f: FighterId) {
        let fp = &mut self.fighters[f.0];
        fp.ground_accel_1 = 0.0;
        fp.ground_accel_2 = 0.0;
        fp.anim_vel = Vec3::new(0.0, 0.0, 0.0);
        fp.gr_vel = 0.0;
        fp.self_vel = Vec3::new(0.0, 0.0, 0.0);
        fp.ground_kb_vel = 0.0;
        fp.kb_vel = Vec3::new(0.0, 0.0, 0.0);
        fp.ground_attacker_shield_kb_vel = 0.0;
        fp.atk_shield_kb_vel = Vec3::new(0.0, 0.0, 0.0);
        fp.frame_vel = Vec3::new(0.0, 0.0, 0.0);
    }
}
