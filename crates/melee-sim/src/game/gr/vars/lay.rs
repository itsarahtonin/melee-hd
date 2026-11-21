//! The stage parts' typed members laid out for the heap's image (docs/engine.md, a session's
//! heap leftovers): each field at its offset from the union's start (`Ground::u`, +0xC4), as the
//! comparer reads it ([`GroundVars`]'s `Mirror`). A member's bytes past its fields, and the
//! bits of a byte no field names, are what the block held: the layout leaves them as the
//! caller's [`Lay`] has them.

use super::*;
use crate::game::mirror::LayCx;
use crate::hsd::export::Lay;

/// A word that is 0 where no allocator model runs (a spawn configuration's or a block's
/// address): unknown then.
fn addr(l: &mut Lay, at: u32, w: u32) {
    if w != 0 {
        l.put_u32(at, w);
    } else {
        l.forget(at, 4);
    }
}

fn f(l: &mut Lay, at: u32, v: f32) {
    l.put_u32(at, v.to_bits());
}

fn fs(l: &mut Lay, at: u32, v: &[f32]) {
    for (i, &x) in v.iter().enumerate() {
        f(l, at + 4 * i as u32, x);
    }
}

fn h16(l: &mut Lay, at: u32, v: &[i16]) {
    for (i, &x) in v.iter().enumerate() {
        l.put_u16(at + 2 * i as u32, x as u16);
    }
}

fn w32(l: &mut Lay, at: u32, v: &[i32]) {
    for (i, &x) in v.iter().enumerate() {
        l.put_u32(at + 4 * i as u32, x as u32);
    }
}

fn gobj(l: &mut Lay, cx: &dyn LayCx, at: u32, g: Option<GObjId>) {
    l.put_word(at, cx.handle("GObjId", g.map(|g| g.0)));
}

fn jobj(l: &mut Lay, cx: &dyn LayCx, at: u32, j: Option<crate::hsd::jobj::JObjId>) {
    l.put_word(at, cx.handle("JObjId", j.map(|j| j.0)));
}

fn data(l: &mut Lay, cx: &dyn LayCx, at: u32, d: Option<melee_data::archive::DataRef>) {
    l.put_word(at, cx.data(d));
}

/// The two flags at the top of the first byte (`StadiumBits`).
fn bits(l: &mut Lay, at: u32, b: StadiumBits) {
    l.put_bits(at, 0, 1, u32::from(b.b0));
    l.put_bits(at, 1, 1, u32::from(b.b1));
}

impl GroundVars {
    /// Lays the typed member out at `a` (the union's start): whether melee-sim lays this one
    /// out (a member it keeps as bytes is the caller's).
    pub fn lay(&self, l: &mut Lay, a: u32, cx: &dyn LayCx) -> bool {
        match self {
            GroundVars::Bytes(_) => return false,
            GroundVars::Map(v) => {
                l.put_u32(a, v.word());
                f(l, a + 4, v.cycle_timer);
                for (i, &g) in v.lv_gobj.iter().enumerate() {
                    gobj(l, cx, a + 8 + 4 * i as u32, g);
                }
                fs(
                    l,
                    a + 0x20,
                    &[
                        v.blend_r,
                        v.blend_g,
                        v.blend_b,
                        v.blend_unused,
                        v.fog_r,
                        v.fog_g,
                        v.fog_b,
                        v.fog_unused,
                        v.blend_frames,
                    ],
                );
                for (i, &g) in v.symbol.iter().enumerate() {
                    gobj(l, cx, a + 0x44 + 4 * i as u32, g);
                }
                l.put_bytes(a + 0x5C, &v.pad_5c);
                l.put_u32(a + 0x6C, v.chikuwa.0);
                l.put_u32(a + 0x70, v.vanish.0);
                l.put_bytes(a + 0x74, &v.pad_74);
            }
            GroundVars::Last(v) => {
                fs(
                    l,
                    a,
                    &[
                        v.angle_x, v.angle_y, v.speed_x, v.speed_y, v.push_x, v.push_y, v.fade,
                    ],
                );
                l.put_word(a + 0x1C, cx.handle("GeneratorId", v.generator.map(|g| g.0)));
            }
            GroundVars::StadiumDisplay(v) => {
                bits(l, a, v.bits);
                l.put_word(a + 4, cx.handle("TObjId", v.tobj.map(|t| t.0)));
                l.put_word(a + 8, cx.handle("MObjId", v.mobj.map(|t| t.0)));
                data(l, cx, a + 0xC, v.image);
                gobj(l, cx, a + 0x10, v.text);
                gobj(l, cx, a + 0x14, v.feed);
                gobj(l, cx, a + 0x18, v.zoom);
                l.put_u32(a + 0x1C, v.timer as u32);
                h16(
                    l,
                    a + 0x20,
                    &[
                        v.mode,
                        v.shown,
                        v.applied,
                        v.prev_mode,
                        v.unwritten,
                        v.player,
                        v.slot_type,
                        v.count,
                    ],
                );
                l.put_word(a + 0x30, cx.handle("CameraBoxId", v.subject.map(|s| s.0)));
                l.put_bits(a + 0x34, 0, 1, u32::from(v.rendered));
                l.put_bits(a + 0x34, 1, 1, u32::from(v.unread_bit));
                l.put_bits(a + 0x34, 2, 1, u32::from(v.shows_count));
            }
            GroundVars::Stadium(v) => {
                bits(l, a, v.bits);
                l.put_u32(a + 4, v.size);
                l.put_u32(a + 8, v.buffer);
                l.put_u32(
                    a + 0xC,
                    v.archive
                        .map_or(0, |s| ARCHIVE_TABLE + ARCHIVE_ENTRY * u32::from(s)),
                );
                f(l, a + 0x10, v.path_len);
                l.put_u32(a + 0x14, v.timer as u32);
                h16(l, a + 0x18, &[v.state, v.kind, v.prev, v.prev2]);
                gobj(l, cx, a + 0x20, v.current);
                gobj(l, cx, a + 0x24, v.next);
                l.put_u32(a + 0x28, v.preload_kind as u32);
                l.put_u8(a + 0x2C, v.preload_out);
            }
            GroundVars::StadiumBits(v) => bits(l, a, *v),
            GroundVars::StadiumPiece(v) => {
                bits(l, a, v.bits);
                l.put_word(a + 4, cx.handle("GeneratorId", v.generator.map(|g| g.0)));
                for (i, &g) in v.blades.iter().enumerate() {
                    gobj(l, cx, a + 8 + 4 * i as u32, g);
                }
                jobj(l, cx, a + 0x10, v.turning);
            }
            GroundVars::Izumi(v) => {
                l.put_word(a, cx.handle("TObjId", v.water_tobj.map(|o| o.0)));
                gobj(l, cx, a + 4, v.reflection_camera);
                gobj(l, cx, a + 8, v.reflections);
                jobj(l, cx, a + 0xC, v.platform_jobj);
                jobj(l, cx, a + 0x10, v.platform_jobj2);
            }
            GroundVars::Izumi2(v) => {
                jobj(l, cx, a, v.reflection);
                jobj(l, cx, a + 4, v.reflection2);
            }
            GroundVars::Izumi3(v) => {
                h16(l, a, &[v.state, v.timer, v.coll_joint, v.unwritten]);
                jobj(l, cx, a + 8, v.column);
                fs(
                    l,
                    a + 0xC,
                    &[v.height, v.target_height, v.column_height, v.home_height],
                );
            }
            GroundVars::OldPupupu(v) => w32(l, a, &v.words()),
            GroundVars::OldPupupu2(v) => h16(l, a, &[v.flight_wait, v.unwritten]),
            GroundVars::OldYoshiCloud(v) => {
                for (i, c) in v.cloud.iter().enumerate() {
                    let b = a + 0x14 * i as u32;
                    l.put_u32(b, c.bits);
                    jobj(l, cx, b + 4, c.jobj);
                    fs(l, b + 8, &[c.rest_y, c.sunk, c.sink_speed]);
                }
            }
            GroundVars::OldYoshiGuest(v) => h16(l, a, &[v.wait, v.part]),
            GroundVars::OldKongo(v) => {
                h16(
                    l,
                    a,
                    &[
                        v.turn,
                        v.hold,
                        v.swing_cycle,
                        v.hold_timer,
                        v.turn_timer,
                        v.swing_timer,
                        v.hit_timer,
                    ],
                );
                gobj(l, cx, a + 0x10, v.keep);
                fs(
                    l,
                    a + 0x14,
                    &[
                        v.heading,
                        v.angle,
                        v.turn_accel,
                        v.turn_speed,
                        v.unread,
                        v.swing_speed,
                    ],
                );
            }
            GroundVars::OldKongoBirds(v) => l.put_u32(a, v.wait as u32),
            GroundVars::Garden(v) => w32(l, a, &[v.anim, v.runs_left]),
            GroundVars::Garden2(v) => {
                gobj(l, cx, a, v.item);
                l.put_u8(a + 4, v.state as u8);
                l.put_u32(a + 8, v.count as u32);
            }
            GroundVars::Kongo(v) => {
                fs(l, a, &[v.left_angle, v.left_speed]);
                jobj(l, cx, a + 8, v.left_post);
                jobj(l, cx, a + 0xC, v.left_post2);
                fs(l, a + 0x10, &[v.right_angle, v.right_speed]);
                jobj(l, cx, a + 0x18, v.right_post);
                jobj(l, cx, a + 0x1C, v.right_post2);
                h16(l, a + 0x20, &[v.event_wait, v.last_event]);
            }
            GroundVars::KongoBarrel(v) => {
                h16(
                    l,
                    a,
                    &[
                        v.turn,
                        v.hold,
                        v.swing_cycle,
                        v.hold_timer,
                        v.turn_timer,
                        v.swing_timer,
                    ],
                );
                gobj(l, cx, a + 0xC, v.keep);
                fs(
                    l,
                    a + 0x10,
                    &[
                        v.heading,
                        v.angle,
                        v.turn_accel,
                        v.turn_speed,
                        v.unread,
                        v.swing_speed,
                    ],
                );
            }
            GroundVars::KongoPlatform(v) => {
                h16(
                    l,
                    a,
                    &[v.state, v.landings, v.landings_last, v.ticks_afloat],
                );
                fs(l, a + 8, &[v.across, v.unread_min, v.unread_max]);
            }
            GroundVars::KongoLog(v) => f(l, a + 8, v.across),
            GroundVars::KongoPath(v) => {
                data(l, cx, a, v.spline);
                fs(l, a + 4, &[v.unread, v.along, v.unread2]);
                fs(l, a + 0x14, &[v.highest, v.deepest]);
            }
            GroundVars::Pura(v) => h16(l, a, &[v.color_from, v.color_to, v.fade_tick]),
            GroundVars::PuraFloat(v) => {
                h16(l, a, &[v.part]);
                l.put_bytes(a + 2, &v.pad);
                jobj(l, cx, a + 4, v.stage_jobj);
            }
            GroundVars::PuraStage(v) => {
                for i in 0..25 {
                    jobj(l, cx, a + 4 * i as u32, v.jobjs[i]);
                    l.put_word(
                        a + 0x64 + 4 * i as u32,
                        cx.handle("CameraBoxId", v.subjects[i].map(|b| b.0)),
                    );
                }
            }
            GroundVars::RCruise(v) => {
                l.put_u8(a, v.flags);
                // The wind's slot in the pool `WIND_POOL` names: its address in the wind
                // sources' block where the allocator model gives it (unknown where none runs).
                l.put_word(a + 4, cx.handle("WindSource", v.wind.map(u32::from)));
                f(l, a + 8, v.steer_step);
                w32(l, a + 0xC, &[v.gust, v.waiting]);
                fs(
                    l,
                    a + 0x14,
                    &[
                        v.unread,
                        v.tilt,
                        v.tilt_dir,
                        v.tilt_speed,
                        v.pull_left,
                        v.pull_right,
                    ],
                );
                w32(
                    l,
                    a + 0x2C,
                    &[v.tilt_state, v.touches_last, v.touches, v.ease_wait],
                );
                addr(l, a + 0x6C, v.entries_addr);
                addr(l, a + 0x70, v.vanish_addr);
            }
            GroundVars::RCruiseBlocks(v) => {
                for (i, e) in v.blocks.iter().enumerate() {
                    let b = a + 0x3C + 0x10 * i as u32;
                    l.put_u8(b, e.state);
                    l.put_u8(b + 1, e.flags);
                    l.put_u16(b + 2, e.coll_joint as u16);
                    w32(l, b + 4, &[e.ticks, e.touches]);
                    jobj(l, cx, b + 0xC, e.jobj);
                }
            }
            GroundVars::Corneria(v) => {
                l.put_bytes(a, &[v.flags, v.unwritten, v.unwritten2, v.unwritten3]);
                addr(l, a + 4, v.splash_config);
                addr(l, a + 8, v.splash_config2);
                fs(
                    l,
                    a + 0xC,
                    &[v.sink_depth, v.base_x, v.base_y, v.offset_x, v.offset_y],
                );
                fs(
                    l,
                    a + 0x20,
                    &[
                        v.last_drift.x,
                        v.last_drift.y,
                        v.last_drift.z,
                        v.drift_speed_y,
                        v.drift_accel_x,
                        v.drift_accel_y,
                        v.sink_speed,
                    ],
                );
                w32(
                    l,
                    a + 0x3C,
                    &[
                        v.sink_ticks,
                        v.drift_ticks,
                        v.guns_state as i32,
                        v.guns_wait,
                        v.shot_wait,
                    ],
                );
                f(l, a + 0x50, v.guns_damage);
                l.put_bytes(
                    a + 0x54,
                    &[v.shots_left as u8, v.shot_kind, v.shot_side, v.unwritten4],
                );
                l.put_u32(a + 0x58, v.unwritten5);
                gobj(l, cx, a + 0x5C, v.left_cannon);
                gobj(l, cx, a + 0x60, v.right_cannon);
                gobj(l, cx, a + 0x64, v.copy);
                jobj(l, cx, a + 0x68, v.deck);
            }
            GroundVars::CorneriaCity(v) => {
                l.put_u8(a, v.waiting);
                l.put_u8(a + 1, v.made_next);
                l.put_bits(a + 2, 0, 1, u32::from(v.still));
            }
            GroundVars::CorneriaCity8(v) => {
                l.put_bytes(a, &[v.waiting, v.made_next, v.cleared]);
                for (k, &w) in v.cleared_words.iter().enumerate() {
                    l.put_u32(a + 4 + 4 * k as u32, w);
                }
                l.put_bits(a + 0x1C, 0, 1, u32::from(v.still));
            }
            _ => return self.lay_rest(l, a, cx),
        }
        true
    }
}

fn v3(l: &mut Lay, at: u32, v: &crate::vec::Vec3) {
    fs(l, at, &[v.x, v.y, v.z]);
}

/// Brinstar's acid's state (`grZebes_AcidState`, 0x24).
fn acid(l: &mut Lay, cx: &dyn LayCx, at: u32, s: &AcidState) {
    l.put_u8(at, s.state);
    l.put_u8(at + 1, s.next_state);
    l.put_u16(at + 2, s.timer as u16);
    fs(l, at + 4, &[s.rest_x, s.swing, s.swing_speed, s.damage]);
    jobj(l, cx, at + 0x14, s.jobj);
    jobj(l, cx, at + 0x18, s.jobj2);
    gobj(l, cx, at + 0x1C, s.item);
    l.put_u16(at + 0x20, s.anim_part as u16);
}

fn dynamics(l: &mut Lay, cx: &dyn LayCx, at: u32, d: &crate::game::structs::DynamicsDesc) {
    l.put_word(at, cx.handle("DynamicsId", d.data.map(|x| x.0)));
    l.put_u32(at + 4, d.count);
    v3(l, at + 8, &d.pos);
}

impl GroundVars {
    /// [`GroundVars::lay`]'s second half.
    fn lay_rest(&self, l: &mut Lay, a: u32, cx: &dyn LayCx) -> bool {
        match self {
            GroundVars::ArwingPath(v) => {
                l.put_u32(a + 4, v.slot);
                w32(l, a + 0x10, &[v.anim_pending, v.sound_state]);
            }
            GroundVars::Arwing(v) => {
                l.put_bits(a, 0, 1, u32::from(v.b0));
                l.put_u32(a + 4, v.slot as u32);
                l.put_u32(a + 0x10, v.cleared as u32);
                gobj(l, cx, a + 0x18, v.coll);
                for (k, &g) in v.articles.iter().enumerate() {
                    if v.disabled && k < 2 {
                        l.put_u32(a + 0x1C + 4 * k as u32, u32::MAX);
                    } else {
                        gobj(l, cx, a + 0x1C + 4 * k as u32, g);
                    }
                }
                w32(
                    l,
                    a + 0x2C,
                    &[v.ticks_flown, v.roll_state, v.roll_wait, v.fires],
                );
                if let Some(x100) = v.next_gun {
                    l.put_u32(a + 0x3C, x100 as u32);
                }
            }
            GroundVars::ArwingColl(v) => {
                l.put_u32(a + 4, v.slot);
                l.put_u32(a + 0xC, v.cleared);
                f(l, a + 0x18, v.roll);
                v3(l, a + 0x1C, &v.pos);
            }
            GroundVars::StarFoxTalk(v) => {
                l.put_u32(a, v.talk);
                w32(l, a + 4, &[v.next_line, v.wait]);
            }
            GroundVars::SmashTaunt(v) => {
                h16(l, a, &[v.state, v.timer, v.line, v.sis_data_idx]);
                l.put_u32(a + 8, v.sound_id as u32);
                jobj(l, cx, a + 0xC, v.jobj0);
                jobj(l, cx, a + 0x10, v.jobj1);
                jobj(l, cx, a + 0x14, v.jobj2);
                h16(l, a + 0x18, &[v.joint_idx0, v.joint_idx1, v.joint_idx2]);
                l.put_word(a + 0x20, cx.handle("TextId", v.text.map(|t| t.0)));
            }
            GroundVars::VenomPlatform(v) => {
                gobj(l, cx, a, v.target);
                l.put_u32(a + 4, v.timer as u32);
                jobj(l, cx, a + 8, v.upper);
                jobj(l, cx, a + 0xC, v.lower);
            }
            GroundVars::VenomShip(v) => {
                for (k, &j) in v.jobjs.iter().enumerate() {
                    jobj(l, cx, a + 4 * k as u32, j);
                }
                l.put_u16(a + 0x1C, v.bits);
                f(l, a + 0x20, v.frame);
            }
            GroundVars::Castle(v) => {
                for (i, &g) in v.switches.iter().enumerate() {
                    gobj(l, cx, a + 4 * i as u32, g);
                }
                h16(
                    l,
                    a + 0xC,
                    &[v.switch_slot, v.switch_wait, v.bill_wait, v.recent_at],
                );
                h16(l, a + 0x14, &v.recent);
                l.put_u8(a + 0x1A, v.flags);
                for (i, d) in v.dynamics.iter().enumerate() {
                    dynamics(l, cx, a + 0x1C + 0x14 * i as u32, d);
                }
            }
            GroundVars::CastleLifts(v) => {
                for (i, p) in v.plat.iter().enumerate() {
                    let b = a + 0x10 * i as u32;
                    jobj(l, cx, b, p.jobj);
                    f(l, b + 4, p.pos);
                    h16(l, b + 8, &[p.state, p.timer]);
                    f(l, b + 0xC, p.wind);
                }
            }
            GroundVars::CastleBlaster(v) => {
                l.put_u32(a, v.anim);
                h16(l, a + 4, &[v.timer]);
            }
            GroundVars::CastleBlast(v) => {
                gobj(l, cx, a, v.item);
                l.put_word(a + 4, cx.handle("CameraBoxId", v.subject.map(|b| b.0)));
            }
            GroundVars::CastleBill(v) => {
                l.put_u8(a, v.flags);
                h16(l, a + 2, &[v.path, v.linger, v.delay]);
                gobj(l, cx, a + 8, v.blaster);
                gobj(l, cx, a + 0xC, v.item);
                gobj(l, cx, a + 0x10, v.stage);
                l.put_word(a + 0x14, cx.handle("CameraBoxId", v.subject.map(|b| b.0)));
            }
            GroundVars::CastleBlocks(v) => {
                h16(l, a, &[v.state]);
                h16(l, a + 4, &[v.flicker]);
                l.put_u32(a + 8, v.timer as u32);
                for i in 0..5 {
                    let o = 4 * i as u32;
                    jobj(l, cx, a + 0xC + o, v.jobjs[i]);
                    jobj(l, cx, a + 0x20 + o, v.effect_a[i]);
                    jobj(l, cx, a + 0x34 + o, v.effect_b[i]);
                    gobj(l, cx, a + 0x48 + o, v.items[i]);
                }
                w32(l, a + 0x5C, &v.joints);
                l.put_bytes(a + 0x70, &v.bump);
                l.put_bytes(a + 0x75, &v.step);
                fs(l, a + 0x7C, &v.base_y);
            }
            GroundVars::CastleSwitch(v) => {
                h16(l, a, &[v.state]);
                h16(l, a + 4, &[v.blink, v.delay, v.count]);
                gobj(l, cx, a + 0xC, v.blocks);
                jobj(l, cx, a + 0x10, v.anchor);
                gobj(l, cx, a + 0x14, v.item);
            }
            GroundVars::RCruiseFlag(v) => dynamics(l, cx, a, &v.chain),
            GroundVars::Inishie1(v) => {
                l.put_u16(a, v.flags);
                h16(
                    l,
                    a + 2,
                    &[
                        v.hatena_wait_a,
                        v.hatena_wait_b,
                        v.last_hatena_a,
                        v.last_hatena_b,
                    ],
                );
                w32(l, a + 0xC, &[v.unwritten, v.unwritten2, v.hatena_cooldown]);
                addr(l, a + 0x18, v.blocks_addr);
                fs(l, a + 0x1C, &[v.lift_weight_a, v.lift_weight_b]);
                h16(
                    l,
                    a + 0x24,
                    &[
                        v.lift_touches_a,
                        v.lift_touches_b,
                        v.lift_wait,
                        v.lift_state,
                    ],
                );
                fs(
                    l,
                    a + 0x2C,
                    &[v.lift_offset, v.lift_speed, v.unwritten3, v.lift_rest_y],
                );
                jobj(l, cx, a + 0x3C, v.platform_a);
                jobj(l, cx, a + 0x40, v.platform_b);
                jobj(l, cx, a + 0x44, v.arm_a);
                jobj(l, cx, a + 0x48, v.arm_b);
            }
            GroundVars::Inishie1Hatena(v) => {
                jobj(l, cx, a, v.brick_jobj);
                w32(l, a + 4, &[v.blink, v.hidden]);
            }
            // The flags' bits the stage writes (0x80 to 0x08) and the log's (0x80), with the
            // rest as the block held them.
            GroundVars::Inishie2(v) => {
                l.put_u8(a, v.flags);
                h16(
                    l,
                    a + 2,
                    &[v.log_wait_a, v.log_wait_b, v.birdo_wait, v.pidgit_wait],
                );
            }
            GroundVars::Inishie2Log(v) => {
                l.put_u8(a, v.flags);
                h16(l, a + 2, &[v.wait, v.log]);
                gobj(l, cx, a + 8, v.sender);
            }
            GroundVars::Inishie2Birdo(v) => {
                gobj(l, cx, a, v.birdo);
                gobj(l, cx, a + 4, v.sender);
            }
            GroundVars::Inishie2Pidgit(v) => {
                gobj(l, cx, a, v.sender);
                l.put_u8(a + 4, v.flags);
                h16(l, a + 6, &[v.turns]);
                v3(l, a + 8, &v.anim_step);
                v3(l, a + 0x14, &v.place);
            }
            GroundVars::IceMtScroll(v) => {
                h16(
                    l,
                    a,
                    &[
                        v.under,
                        v.upper,
                        v.phase,
                        v.delay,
                        v.lerp_count,
                        v.burst_count,
                        v.idx,
                    ],
                );
                f(l, a + 0x10, v.cur);
                h16(l, a + 0x14, &[v.waiting, v.unread]);
                h16(l, a + 0x18, &v.speed_rests);
                h16(l, a + 0x30, &v.field_rests);
            }
            GroundVars::IceMtClimb(v) => {
                h16(
                    l,
                    a,
                    &[
                        v.under,
                        v.upper,
                        v.phase,
                        v.delay,
                        v.lerp_count,
                        v.burst_count,
                        v.idx,
                    ],
                );
                f(l, a + 0x10, v.cur);
                for k in 0..5 {
                    l.put_bits(a + 0x14, k, 1, u32::from(v.flags & (0x80 >> k) != 0));
                }
                h16(
                    l,
                    a + 0x16,
                    &[v.push_ticks, v.unread, v.speed_cursor, v.field_cursor],
                );
                f(l, a + 0x20, v.push_speed);
            }
            GroundVars::IceMtField(v) => {
                let (n, slots) = icemt_field_layout(v.map);
                l.put_bits(a, 0, 1, u32::from(v.flags & IceMtFieldVars::B0 != 0));
                l.put_bits(a, 1, 1, u32::from(v.flags & IceMtFieldVars::B1 != 0));
                for (k, &j) in v.parts.iter().enumerate() {
                    jobj(l, cx, a + 4 + 4 * k as u32, j);
                }
                for (k, &g) in v.items.iter().take(n).enumerate() {
                    gobj(l, cx, a + 0x34 + 4 * k as u32, g);
                }
                for (s, &off) in v.slots.iter().zip(slots) {
                    h16(l, a + off, &s.0);
                }
            }
            GroundVars::IceMtBg(v) => f(l, a, v.speed),
            GroundVars::Scroll(v) => {
                l.put_u8(a, v.flags);
                v3(l, a + 4, &v.center);
                v3(l, a + 0x10, &v.offset);
                v3(l, a + 0x1C, &v.step);
                gobj(l, cx, a + 0x28, v.anim_gobj);
                jobj(l, cx, a + 0x2C, v.int_jobj);
                jobj(l, cx, a + 0x30, v.cam_jobj);
                jobj(l, cx, a + 0x34, v.ctr_jobj);
                for (k, &j) in v.scenery.iter().enumerate() {
                    jobj(l, cx, a + 0x38 + 4 * k as u32, j);
                }
                jobj(l, cx, a + 0x44, v.scenery2);
            }
            GroundVars::MuteCity(v) => {
                h16(l, a, &[v.script_cursor, v.script_wait]);
                gobj(l, cx, a + 4, v.platform);
                gobj(l, cx, a + 8, v.cars);
                l.put_u8(a + 0xC, v.flags);
                h16(l, a + 0xE, &[v.cleared]);
                fs(l, a + 0x10, &[v.left_end, v.right_end]);
                jobj(l, cx, a + 0x18, v.spline_jobj);
                jobj(l, cx, a + 0x1C, v.spline_jobj2);
                v3(l, a + 0x20, &v.track_start);
                v3(l, a + 0x2C, &v.track_end);
                for (k, &j) in v.shadow_jobjs.iter().enumerate() {
                    jobj(l, cx, a + 0x38 + 4 * k as u32, j);
                }
                l.put_word(a + 0x4C, cx.handle("LObjId", v.light.map(|o| o.0)));
                fs(l, a + 0x50, &v.bounds);
            }
            GroundVars::MuteCity2(v) => {
                l.put_u8(a, v.flags);
                jobj(l, cx, a + 4, v.sway_jobj);
                fs(l, a + 8, &[v.sway_target, v.sway_scale]);
                for (k, c) in v.saved_colors.iter().enumerate() {
                    l.put_bytes(a + 0x10 + 4 * k as u32, c);
                }
            }
            GroundVars::MuteCityCars(v) => h16(l, a, &[v.horn_wait]),
            GroundVars::BigBlueManager(v) => {
                l.put_u32(a, v.flags);
                for (k, &e) in v.event.iter().enumerate() {
                    l.put_u32(a + 4 + 4 * k as u32, e);
                }
                for (k, &j) in v.jobjs.iter().enumerate() {
                    jobj(l, cx, a + 0x10 + 4 * k as u32, j);
                }
                match v.event_extra {
                    EventExtra::Joint(j) => jobj(l, cx, a + 0x1C, Some(j)),
                    EventExtra::Left(w) => l.put_u32(a + 0x1C, w),
                }
                for (k, d) in v.data.iter().enumerate() {
                    let b = a + 0x20 + 0x54 * k as u32;
                    l.put_bytes(b, &[d.index, d.state, d.dir as u8, d.unwritten]);
                    l.put_u32(b + 4, d.timer as u32);
                    f(l, b + 8, d.height);
                    v3(l, b + 0xC, &d.targets);
                    v3(l, b + 0x18, &d.swing);
                    fs(l, b + 0x24, &[d.push_left, d.push_right]);
                    w32(l, b + 0x2C, &[d.landings, d.landings_last, d.dip_stage]);
                    v3(l, b + 0x38, &d.pos);
                    v3(l, b + 0x44, &d.speed);
                    gobj(l, cx, b + 0x50, d.item);
                }
            }
            GroundVars::BigBlueCars(v) => {
                l.put_u32(a, v.flags);
                addr(l, a + 4, v.collision_jobjs_addr);
                addr(l, a + 8, v.ranks_addr);
                h16(l, a + 0xC, &[v.spawn_timer]);
                for (k, c) in v.cars.iter().enumerate() {
                    let b = a + 0x10 + 0x40 * k as u32;
                    l.put_u16(b, c.status);
                    l.put_u8(b + 2, c.unwritten as u8);
                    l.put_u8(b + 3, c.unwritten2);
                    fs(
                        l,
                        b + 4,
                        &[c.target, c.delta, c.pos.x, c.pos.y, c.pos.z, c.alpha],
                    );
                    l.put_u32(b + 0x1C, c.threshold as u32);
                    fs(
                        l,
                        b + 0x20,
                        &[
                            c.gravity,
                            c.height,
                            c.velocity,
                            c.accel,
                            c.rotation,
                            c.amplitude,
                            c.angular_velocity,
                            c.unwritten3,
                        ],
                    );
                }
            }
            GroundVars::BigBlueRoad(v) => {
                l.put_u32(a, v.flags);
                v3(l, a + 4, &v.position);
                v3(l, a + 0x10, &v.previous_position);
                v3(l, a + 0x1C, &v.drift);
                f(l, a + 0x28, v.lateral_drift);
                h16(l, a + 0x2C, &[v.direction]);
                f(l, a + 0x34, v.rotation);
            }
            GroundVars::BigBlueFlyer(v) => {
                l.put_u8(a, v.state);
                l.put_u32(a + 4, v.timer as u32);
                fs(l, a + 8, &[v.target_turn, v.target_y, v.unwritten, v.speed]);
            }
            GroundVars::KinokoRoute(v) => {
                for (i, e) in v.entries.iter().enumerate() {
                    let b = a + 0x10 * i as u32;
                    v3(l, b, &e.pos);
                    jobj(l, cx, b + 0xC, e.jobj);
                }
            }
            GroundVars::KinokoRoute2(v) => {
                l.put_u8(a, u8::from(v.flags_0) << 7 | v.flags_rest);
                l.put_u8(a + 1, v.pad_01);
                h16(l, a + 2, &[v.phase, v.spawn_idx, v.zone_idx, v.cam_timer]);
                l.put_bytes(a + 0xA, &v.pad_0a);
                v3(l, a + 0xC, &v.reb0_pos);
            }
            GroundVars::ShrineRoute(v) => {
                let hs = [
                    v.state,
                    v.taken,
                    v.point,
                    v.touched,
                    v.both_touched,
                    v.cam_timer,
                ];
                h16(l, a, &hs.map(|x| x as i16));
                if let Some(x) = v.way_out {
                    l.put_u16(a + 0xC, x);
                }
                gobj(l, cx, a + 0x10, v.portal);
                for (i, p) in v.platforms.iter().enumerate() {
                    let e = a + 0x14 + 0x10 * i as u32;
                    if p.jobj.is_some() {
                        v3(l, e, &p.offset);
                    }
                    jobj(l, cx, e + 0xC, p.jobj);
                }
                for (i, &s) in v.symbols.iter().enumerate() {
                    gobj(l, cx, a + 0x44 + 4 * i as u32, s);
                }
            }
            GroundVars::ShrineRoute3(v) => {
                jobj(l, cx, a, v.spin);
                fs(l, a + 4, &[v.sway_x, v.sway_y, v.push_x, v.push_y]);
                if v.spin.is_some() {
                    fs(l, a + 0x14, &[v.spin_x, v.spin_y]);
                }
            }
            GroundVars::BigBlueRoute(v) => {
                gobj(l, cx, a, v.cars);
                h16(l, a + 4, &[v.zone]);
                l.put_bytes(a + 6, &v.pad_ca);
                v3(l, a + 8, &v.reb0_pos);
                for (i, t) in v.tracks.iter().enumerate() {
                    let e = a + 0x14 + 0x10 * i as u32;
                    if t.jobj.is_some() {
                        v3(l, e, &t.offset);
                    }
                    jobj(l, cx, e + 0xC, t.jobj);
                }
            }
            GroundVars::BigBlueRouteCars(v) => {
                l.put_u32(a, v.flags);
                addr(l, a + 4, v.cars_addr);
                data(l, cx, a + 8, v.air);
                data(l, cx, a + 0xC, v.left);
                data(l, cx, a + 0x10, v.right);
                v3(l, a + 0x14, &v.air_start);
                v3(l, a + 0x20, &v.air_end);
                v3(l, a + 0x2C, &v.road_start);
                v3(l, a + 0x38, &v.road_end);
                h16(l, a + 0x44, &[v.out, v.wait]);
            }

            GroundVars::PushOn(v) => {
                gobj(l, cx, a, v.gobj);
                for i in 0..v.count.clamp(0, 20) as usize {
                    let at = a + 4 * i as u32;
                    l.put_word(at + 4, cx.handle("LObjId", v.lobjs[i].map(|o| o.0)));
                    l.put_u32(at + 0x54, v.lobj_flags[i]);
                }
                l.put_u32(a + 0xA4, v.count as u32);
                l.put_word(a + 0xA8, cx.handle("LObjId", v.point_light.map(|o| o.0)));
                l.put_word(a + 0xAC, cx.handle("LObjId", v.spot_light.map(|o| o.0)));
            }

            GroundVars::FigureGet(v) => {
                w32(
                    l,
                    a,
                    &[v.coin_wait, v.coins_out, v.coins_landed, v.coins_gone],
                );
                w32(l, a + 0x10, &v.coin_trophies);
                w32(l, a + 0x1C, &v.landed_trophies);
                for i in 0..3 {
                    gobj(l, cx, a + 0x28 + 4 * i as u32, v.landed_coins[i]);
                    gobj(l, cx, a + 0x34 + 4 * i as u32, v.coins[i]);
                }
            }

            GroundVars::HomeRun(v) => {
                addr(l, a, v.parts_addr);
                addr(l, a + 4, v.back_addr);
                l.put_word(a + 8, cx.handle("TextId", v.text.map(|t| t.0)));
                jobj(l, cx, a + 0xC, v.jobj);
                gobj(l, cx, a + 0x10, v.text_gobj);
                for (i, &g) in v.bg.iter().enumerate() {
                    gobj(l, cx, a + 0x14 + 4 * i as u32, g);
                }
                l.put_u8(a + 0x24, u8::from(v.landed) << 7 | v.landed_rest);
            }
            GroundVars::HomeRunPart(v) => {
                l.put_u16(a, v.index);
                l.put_u16(a + 2, v.label);
                f(l, a + 0xC, v.x);
            }
            GroundVars::HomeRunLabel(v) => {
                l.put_u16(a, v.index);
                l.put_u16(a + 2, v.label);
                l.put_word(a + 4, cx.handle("TextId", v.text.map(|t| t.0)));
                jobj(l, cx, a + 8, v.jobj);
                f(l, a + 0xC, v.x);
            }
            GroundVars::BigBlueRelay(v) => {
                l.put_u32(a, v.state_dir);
                w32(l, a + 4, &[v.arrive_timer, v.side_timer, v.rise_timer]);
                fs(l, a + 0x10, &[v.height_offset, v.unwritten, v.target_y]);
                v3(l, a + 0x1C, &v.velocity);
                f(l, a + 0x28, v.unread);
            }
            GroundVars::Yorster(v) => {
                l.put_u32(a, v.waiting as u32);
                for (i, e) in v.blocks.iter().enumerate() {
                    let b = a + 4 + 0x20 * i as u32;
                    l.put_u8(b, e.index as u8);
                    l.put_u8(b + 1, e.state);
                    fs(l, b + 4, &[e.damage, e.landing_speed]);
                    w32(l, b + 0xC, &[e.timer, e.flip_start, e.part]);
                    jobj(l, cx, b + 0x18, e.jobj);
                    gobj(l, cx, b + 0x1C, e.item);
                }
            }
            GroundVars::StoryShyGuys(v) => {
                l.put_u8(a, v.count as u8);
                l.put_u8(a + 1, v.pattern as u8);
                l.put_bytes(a + 2, &v.pad);
                l.put_u32(a + 4, v.timer as u32);
            }
            GroundVars::StoryRandall(v) => {
                h16(l, a, &[v.timer]);
                l.put_bytes(a + 2, &v.pad);
                jobj(l, cx, a + 4, v.jobj);
            }
            GroundVars::Greens(v) => {
                l.put_u32(a, v.flags);
                addr(l, a + 4, v.grid_addr);
                addr(l, a + 8, v.blocks_addr);
                l.put_u32(a + 0xC, v.wait as u32);
            }
            GroundVars::Greens2(v) => w32(
                l,
                a,
                &[
                    v.cycle,
                    v.state,
                    v.step,
                    v.count,
                    v.state_new,
                    v.facing,
                    v.wind,
                    v.ticks,
                    v.apples_left,
                    v.apple_side,
                ],
            ),
            GroundVars::Onett(v) => {
                for (i, w) in v.awnings.iter().enumerate() {
                    let b = a + 0x1C * i as u32;
                    jobj(l, cx, b, w.jobj);
                    fs(
                        l,
                        b + 4,
                        &[w.initial_y, w.accumulator, w.velocity, w.initial],
                    );
                    h16(
                        l,
                        b + 0x14,
                        &[w.counter, w.counter_prev, w.cooldown, w.flag],
                    );
                }
                l.put_u32(a + 0x38, v.timer as u32);
                l.put_word(a + 0x3C, cx.handle("GeneratorId", v.smoke.map(|g| g.0)));
                l.put_word(a + 0x40, cx.handle("CameraBoxId", v.subject.map(|b| b.0)));
            }
            GroundVars::OnettBuilding(v) => {
                h16(l, a, &[v.state, v.next_state]);
                w32(l, a + 4, &[v.hit_count, v.frame]);
                l.put_u32(a + 0xC, v.timer);
            }
            GroundVars::OnettCar(v) => {
                l.put_bits(a, 0, 1, v.flags >> 31);
                for (k, &j) in v.car_jobjs.iter().enumerate() {
                    jobj(l, cx, a + 4 + 4 * k as u32, j);
                }
                for (k, &g) in v.car_items.iter().enumerate() {
                    gobj(l, cx, a + 0x18 + 4 * k as u32, g);
                }
                for (k, &j) in v.car_jobjs2.iter().enumerate() {
                    jobj(l, cx, a + 0x2C + 4 * k as u32, j);
                }
                l.put_bytes(a + 0x40, &[v.curr_car as u8, v.state_a]);
                l.put_u32(a + 0x44, v.timer_a as u32);
                l.put_u32(a + 0x4C, v.sub_state_a as u32);
                f(l, a + 0x50, v.car_speed);
                l.put_bytes(a + 0x54, &[v.next_car as u8, v.state_b]);
                l.put_u32(a + 0x58, v.timer_b as u32);
                l.put_u32(a + 0x60, v.sub_state_b as u32);
                f(l, a + 0x64, v.speed_b);
            }
            GroundVars::Kraid(v) => {
                l.put_bytes(a, &[v.state as u8, v.rumbling as u8]);
                fs(l, a + 4, &[v.turn_dir, v.turn_speed, v.turn_step]);
                l.put_u32(a + 0x10, v.turn_delay as u32);
            }
            GroundVars::Kraid2(v) => {
                l.put_bytes(
                    a,
                    &[
                        v.state,
                        v.anim,
                        v.last_place,
                        v.leftmost,
                        v.rightmost,
                        v.turned_rock,
                    ]
                    .map(|x| x as u8),
                );
                f(l, a + 8, v.swipe_side);
                l.put_u32(a + 0xC, v.ticks as u32);
                jobj(l, cx, a + 0x10, v.hand);
                jobj(l, cx, a + 0x14, v.hand2);
            }
            GroundVars::Flatzone(v) => {
                l.put_bytes(a, &[v.waiting, v.last_event, v.event_before, v.may_start]);
                h16(l, a + 4, &[v.timer]);
            }
            GroundVars::FlatzoneOil(v) => {
                l.put_u8(a, v.started);
                f(l, a + 4, v.side);
                l.put_u32(a + 8, super::super::flatzone::area_addr(v.area));
                w32(l, a + 0xC, &[v.state, v.timer]);
            }
            GroundVars::FlatzonePlatforms(v) => {
                l.put_bytes(
                    a,
                    &[
                        v.waiting,
                        v.anim,
                        v.last_anim,
                        v.set,
                        v.last_set,
                        v.row,
                        v.last_row,
                    ],
                );
                h16(l, a + 8, &[v.timer]);
            }
            GroundVars::FlatzoneEvent(v) => l.put_u8(a, v.started),
            GroundVars::FlatzoneTools(v) => {
                l.put_u8(a, v.started);
                l.put_u32(a + 4, v.timer as u32);
            }
            GroundVars::Fourside(v) => {
                jobj(l, cx, a, v.skyline);
                jobj(l, cx, a + 4, v.skyline2);
                jobj(l, cx, a + 8, v.back);
            }
            GroundVars::FoursideCrane(v) => {
                l.put_bytes(a, &[v.state, v.flags]);
                l.put_u32(a + 4, v.ticks as u32);
                fs(
                    l,
                    a + 8,
                    &[v.rest_y, v.height, v.target, v.start_speed, v.speed, v.pull],
                );
            }
            GroundVars::FoursideUfo(v) => {
                l.put_bytes(a, &[v.state, v.building, v.waits, v.sound_state]);
                w32(l, a + 4, &[v.ticks, v.on_roof]);
                l.put_word(a + 0xC, cx.handle("CameraBoxId", v.subject.map(|b| b.0)));
            }
            GroundVars::Fourside2(v) => {
                l.put_bytes(a, &[v.state, v.sound_played]);
                w32(l, a + 4, &[v.ticks, v.on_roof]);
            }
            GroundVars::GreatBay(v) => {
                l.put_bytes(a, &[v.state, v.bits]);
                h16(l, a + 2, &[v.ticks]);
                l.put_word(a + 4, cx.handle("GeneratorId", v.splash.map(|g| g.0)));
                f(l, a + 8, v.shell_height);
            }
            GroundVars::GreatBay2(v) => {
                for (i, &g) in v.gobjs.iter().enumerate() {
                    gobj(l, cx, a + 4 * i as u32, g);
                }
                h16(l, a + 0x10, &[v.moon_wait]);
                l.put_u8(a + 0x12, v.moon_flags);
            }
            GroundVars::GreatBay3(v) => {
                v3(l, a, &v.translation);
                fs(
                    l,
                    a + 0xC,
                    &[v.slide_push, v.sink_push, v.tilt, v.slide, v.sink],
                );
                jobj(l, cx, a + 0x20, v.jobj);
            }
            GroundVars::GreatBay4(v) => {
                w32(
                    l,
                    a,
                    &[
                        v.state,
                        v.last_state,
                        v.popped,
                        v.splashed,
                        v.ledge_hang,
                        v.standing,
                        v.standing_last,
                    ],
                );
                f(l, a + 0x1C, v.weight);
                v3(l, a + 0x20, &v.place);
                gobj(l, cx, a + 0x2C, v.item);
            }
            GroundVars::ZebesCreature(v) => {
                l.put_u8(a, v.flags);
                jobj(l, cx, a + 4, v.stored_jobj);
                h16(l, a + 8, &[v.next_anim, v.wait]);
            }
            GroundVars::ZebesBack(v) => h16(l, a, &[v.state]),
            GroundVars::ZebesBubble(v) => {
                gobj(l, cx, a, v.item);
                l.put_u32(a + 4, v.bubble as u32);
            }
            GroundVars::ZebesIsland(v) => {
                acid(l, cx, a, &v.acid);
                l.put_u32(a + 0x24, v.last_state as u32);
                addr(l, a + 0x28, v.spawn_config);
            }
            GroundVars::ZebesStage(v) => {
                h16(l, a, &[v.platform_state, v.platform_wait]);
                acid(l, cx, a + 4, &v.acid);
                l.put_u32(a + 0x28, v.last_state);
                gobj(l, cx, a + 0x2C, v.island);
                h16(
                    l,
                    a + 0x30,
                    &[v.bubble_wait, v.regrowths, v.pop_wait, v.pop_wait_low],
                );
                addr(l, a + 0x38, v.spawn_config);
                gobj(l, cx, a + 0x3C, v.platform_item);
            }
            GroundVars::ZebesLava(v) => {
                h16(l, a, &[v.entry, v.wait]);
                l.put_u32(a + 4, v.state);
                fs(l, a + 8, &[v.top_speed, v.speed, v.target, v.level]);
                l.put_word(a + 0x18, cx.handle("LObjId", v.light.map(|o| o.0)));
            }
            GroundVars::BattleBg(v) => w32(l, a, &[v.state, v.curr, v.prev, v.timer]),
            _ => return false,
        }
        true
    }
}

impl GroundVars {
    /// The blocks of the main heap a typed member keeps its tables in (Rainbow Cruise's planks
    /// and vanishing parts, Mushroom Kingdom's ? blocks, Green Greens' grid and blocks, Big
    /// Blue's cars' collision joints and ranks), laid out where their addresses are known:
    /// each entry the member holds, the rest of a block what it held.
    pub fn lay_blocks(&self, cx: &dyn LayCx) -> Vec<Lay> {
        let mut lays = Vec::new();
        match self {
            GroundVars::RCruise(v) => {
                if v.entries_addr != 0 {
                    let mut l = Lay::new(v.entries_addr, 0x18 * v.entries.len() as u32);
                    for (i, e) in v.entries.iter().enumerate() {
                        let b = v.entries_addr + 0x18 * i as u32;
                        l.put_bytes(b, &[e.state, e.flags]);
                        h16(&mut l, b + 2, &[e.coll_joint]);
                        w32(&mut l, b + 4, &[e.ticks, e.touches]);
                        fs(&mut l, b + 0xC, &[e.rest_y, e.fall_speed]);
                        jobj(&mut l, cx, b + 0x14, e.jobj);
                    }
                    lays.push(l);
                }
                if v.vanish_addr != 0 {
                    let mut l = Lay::new(v.vanish_addr, 8 * v.vanish.len() as u32);
                    for (i, e) in v.vanish.iter().enumerate() {
                        let b = v.vanish_addr + 8 * i as u32;
                        h16(&mut l, b, &[e.state, e.unwritten]);
                        jobj(&mut l, cx, b + 4, e.jobj);
                    }
                    lays.push(l);
                }
            }
            GroundVars::HomeRun(v) => {
                for (list, at) in [(&v.parts, v.parts_addr), (&v.back, v.back_addr)] {
                    if at != 0 {
                        let mut l = Lay::new(at, 4 * list.len() as u32);
                        for (i, &g) in list.iter().enumerate() {
                            gobj(&mut l, cx, at + 4 * i as u32, g);
                        }
                        lays.push(l);
                    }
                }
            }
            GroundVars::Inishie1(v) if v.blocks_addr != 0 => {
                let mut l = Lay::new(v.blocks_addr, 0x24 * v.blocks.len() as u32);
                for (i, e) in v.blocks.iter().enumerate() {
                    let b = v.blocks_addr + 0x24 * i as u32;
                    h16(&mut l, b, &[e.status, e.state, e.count, e.unwritten]);
                    fs(&mut l, b + 8, &[e.rest_y, e.bump, e.bump_speed]);
                    jobj(&mut l, cx, b + 0x14, e.jobj2);
                    gobj(&mut l, cx, b + 0x18, e.hatena_gobj);
                    gobj(&mut l, cx, b + 0x1C, e.item_gobj);
                    h16(&mut l, b + 0x20, &[e.back_wait, e.blink]);
                }
                lays.push(l);
            }
            GroundVars::Greens(v) => {
                if v.grid_addr != 0 && v.grid.len() == 30 {
                    let mut l = Lay::new(v.grid_addr, 0xC * 30);
                    for (i, c) in v.grid.iter().enumerate() {
                        v3(&mut l, v.grid_addr + 0xC * i as u32, c);
                    }
                    lays.push(l);
                }
                if v.blocks_addr != 0 && v.blocks.len() == 30 {
                    let mut l = Lay::new(v.blocks_addr, 0x20 * 30);
                    for (i, e) in v.blocks.iter().enumerate() {
                        let b = v.blocks_addr + 0x20 * i as u32;
                        l.put_u32(b, e.bits);
                        fs(&mut l, b + 4, &[e.fall_speed, e.height]);
                        gobj(&mut l, cx, b + 0xC, e.gobj);
                        gobj(&mut l, cx, b + 0x10, e.item);
                        jobj(&mut l, cx, b + 0x14, e.jobj);
                        l.put_u32(b + 0x18, e.coll_joint as u32);
                        gobj(&mut l, cx, b + 0x1C, e.breaker);
                    }
                    lays.push(l);
                }
            }
            GroundVars::BigBlueRouteCars(v) if v.cars_addr != 0 => {
                let mut l = Lay::new(v.cars_addr, 0x2C * v.cars.len() as u32);
                for (i, c) in v.cars.iter().enumerate() {
                    let b = v.cars_addr + 0x2C * i as u32;
                    l.put_bytes(b, &[c.flags, 0, 0, 0]);
                    fs(
                        &mut l,
                        b + 4,
                        &[
                            c.road_pos,
                            c.road_speed,
                            c.lane,
                            c.target_lane,
                            c.air_pos,
                            c.air_speed,
                            c.roll,
                            c.target_roll,
                            c.line_change_start,
                        ],
                    );
                    gobj(&mut l, cx, b + 0x28, c.item);
                }
                lays.push(l);
            }
            GroundVars::BigBlueCars(v) => {
                if v.collision_jobjs_addr != 0 {
                    let a = v.collision_jobjs_addr;
                    let mut l = Lay::new(a, 4 * v.collision_jobjs.len() as u32);
                    for (k, &j) in v.collision_jobjs.iter().enumerate() {
                        jobj(&mut l, cx, a + 4 * k as u32, j);
                    }
                    lays.push(l);
                }
                if v.ranks_addr != 0 {
                    let mut l = Lay::new(v.ranks_addr, v.ranks.len() as u32);
                    l.put_bytes(v.ranks_addr, &v.ranks);
                    lays.push(l);
                }
            }
            _ => {}
        }
        lays
    }
}
