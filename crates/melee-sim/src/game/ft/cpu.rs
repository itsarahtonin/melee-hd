//! Computer players' state (`ftCo_0A01.c`, `ftcpuattack.c`): what a fighter's creation and
//! spawn set up for its AI. The AI itself isn't ported.

use gekko_fp as fp;

use super::super::handles::FighterId;
use super::super::mirror::{Cx, Mem, Mirror};
use super::super::state::Running;
use super::super::structs::Vec2;
use crate::fpu::{mem, reg};
use crate::game::ft::CpuKind;
use crate::game::ft::FighterKind;
use crate::game::gr::GrKind;
use crate::vec::Vec3;

/// A pointer a computer player's state keeps into itself (`CpuFighter`): the field at
/// `FIELD` points into the field at `TARGET`, as a byte offset from that field's start, or
/// none for a null pointer. Unlike the address, the offset doesn't depend on where the
/// fighter is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct SelfRef<const TARGET: u32, const FIELD: u32>(pub Option<u32>);

/// `CpuFighter::x444` and `x448`, into `xFC`; `csP` and `write_pos`, into `buffer`.
pub type CpuInputRef444 = SelfRef<0xFC, 0x444>;
pub type CpuInputRef448 = SelfRef<0xFC, 0x448>;
pub type CpuCsP = SelfRef<0x454, 0x450>;
pub type CpuWritePos = SelfRef<0x454, 0x554>;

impl<const TARGET: u32, const FIELD: u32> Mirror for SelfRef<TARGET, FIELD> {
    const SIZE: u32 = 4;

    fn import(m: &Mem, at: u32, _: &mut dyn Cx) -> Self {
        let p = m.u32(at);
        SelfRef((p != 0).then(|| p.wrapping_sub(at - FIELD + TARGET)))
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        let theirs = Self::import(m, at, cx);
        if *self != theirs {
            cx.mismatch(path, format!("{:?}", self.0), format!("{:?}", theirs.0));
        }
    }
}

impl<const TARGET: u32, const FIELD: u32> crate::game::mirror::LayOut for SelfRef<TARGET, FIELD> {
    fn lay(&self, l: &mut crate::hsd::export::Lay, at: u32, _: &dyn crate::game::mirror::LayCx) {
        l.put_u32(
            at,
            self.0.map_or(0, |o| o.wrapping_add(at - FIELD + TARGET)),
        );
    }
}

/// `Fighter_x1A88_xFC_t`'s size: one of the 30 recorded inputs.
const INPUT_SIZE: u32 = 0x1C;

impl Running<'_> {
    /// A computer player's state for its spawn, `ftCo_800A101C`: its kind and level, timers,
    /// where it stands (the floor below it unless that floor is one some stages exclude), its
    /// recorded inputs and buffers cleared, and its reach.
    #[doc(alias = "ftCo_800A101C")]
    pub fn init_cpu(&mut self, f: FighterId, kind: i32, level: i32, x14: i32) {
        let fp = &mut self.game.fighters[f.0];
        let c = &mut fp.cpu;
        if fp.kind == FighterKind::Nana {
            c.acts_alone = true;
            c.kind = CpuKind::Nana.id();
            c.walk_far = 15.0;
        } else {
            c.kind = kind;
            c.walk_far = 40.0;
        }
        c.level = level;
        c.spawn_arg = x14;
        (c.behavior, c.default_behavior, c.alt_behavior) = match CpuKind::from_id(c.kind) {
            Some(CpuKind::Walk | CpuKind::EnemyAgain) => (12, 12, 0),
            Some(CpuKind::Jump) => (1, 1, 11),
            Some(CpuKind::NoAct) => (0, 0, 0),
            _ => (1, 1, 10),
        };
        c.unread_24 = 0x12C;
        c.unread_28 = 0;
        c.item_worth_min = 5;
        c.retarget_timer = 0;
        c.keeps_target = false;
        c.destination_reach = 5.0;
        c.keep_away_distance = 50.0;
        c.target = None;
        c.last_target = Default::default();
        c.enemy_item = None;
        c.spawn_pos = fp.cur_pos;
        let pos = fp.cur_pos;
        // `ftCo_800A0FB0`: the floor below, up to 1000 down, unless `ftCo_800A1B38` excludes it.
        let ay = mem(fp::fadds(10.0, reg(pos.y)));
        let by = mem(fp::fsubs(reg(pos.y), 1000.0));
        let hit = self
            .game
            .geometry
            .as_mut()
            .expect("no collision geometry")
            .check_floor(pos.x, ay, pos.x, by, 0.0, -1, -1, -1, None);
        let hit = hit.filter(|h| !self.excluded_floor(h.line));
        let at = match hit {
            Some(h) => Vec2 {
                x: h.pos.x,
                y: h.pos.y,
            },
            None => Vec2 { x: pos.x, y: pos.y },
        };
        let rand = self.game.objs.rng.draw_f32();
        let fp = &mut self.game.fighters[f.0];
        let c = &mut fp.cpu;
        c.destination = at.clone();
        c.home = at;
        c.destination_timer = 0;
        c.attack_reach_1 = Vec2 { x: 0.0, y: 0.0 };
        c.attack_reach_0 = Vec2 { x: 0.0, y: 0.0 };
        c.fear_target = false;
        c.threat_kind = 0;
        let t = fp::fctiwz(fp::fmul(10.0, reg(rand)));
        c.frame_counter = t;
        c.hold_back_counter = t;
        c.stage_read_count = 0;
        c.ranged_count = 0;
        c.stand_timer = 0;
        c.stand_kind = 0;
        c.script_cursor = Default::default();
        c.command_duration = 0;
        c.write_pos = SelfRef(Some(0));
        c.command = 0;
        c.allowed_count = 0;
        c.ruled_out_count = 0;
        c.coin_flip = false;
        c.near_target = false;
        c.attacking = false;
        c.fleeing = false;
        c.buttons = 0;
        c.lstick.x = 0;
        c.lstick.y = 0;
        c.cstick.x = 0;
        c.cstick.y = 0;
        c.rtrigger = 0;
        c.ltrigger = 0;
        c.acts_alone = false;
        c.may_jump_attack = false;
        c.may_air_attack = false;
        c.may_shield = false;
        c.may_ranged_attack = false;
        c.may_pick_up = false;
        c.follows_popo = false;
        c.nana_recovering = false;
        c.hold_choice = 0;
        c.tech_shield = false;
        c.ledge_picked = false;
        c.human_target_rule = 1;
        c.drifting_back = false;
        c.off_stage = false;
        c.hurt_behind = 1.0;
        c.hurt_ahead = 1.0;
        c.hurt_middle = mem(fp::fmuls(
            0.5,
            fp::fadds(reg(c.hurt_behind), reg(c.hurt_ahead)),
        ));
        c.hurt_top = 2.0;
        c.partner_input_write = SelfRef(Some(5 * INPUT_SIZE));
        c.partner_input_read = SelfRef(Some(0));
        // Nana's recorded inputs start where Popo stands, facing his way.
        let (pos, facing) = if fp.kind == FighterKind::Nana {
            let popo = self
                .cpu_partner(f)
                .expect("Nana's computer state without Popo (ftCo_800A589C)");
            let p = &self.game.fighters[popo.0];
            (p.cur_pos, p.facing_dir)
        } else {
            (Vec3::new(0.0, 0.0, 0.0), 1.0)
        };
        let fp = &mut self.game.fighters[f.0];
        let c = &mut fp.cpu;
        for i in &mut c.partner_inputs {
            i.x0 = 0;
            i.lstick.x = 0;
            i.lstick.y = 0;
            i.x4 = 0;
            i.x5 = 0;
            i.cur_pos = pos;
            i.facing_dir = facing;
        }
        let gravity = reg(fp.co_attrs.gravity);
        let v = fp::fmuls(
            reg(fp.co_attrs.jump_v_initial_velocity),
            reg(fp.co_attrs.air_jump_v_multiplier),
        );
        let tiny = reg(1e-5);
        let c = &mut fp.cpu;
        c.jump_height = if gravity < tiny && gravity > fp::fneg(tiny) {
            10.0
        } else {
            mem(fp::frsp(fp::fdiv(fp::fmuls(v, v), fp::fmul(2.0, gravity))))
        };
        c.half_width = 10.0;
        c.half_height = 10.0;
        c.reach_scale = 1.0;
        self.cpu_attack_timer(f);
        let fp = &mut self.game.fighters[f.0];
        fp.cpu.reach = match fp.fighter_kind() {
            FighterKind::Donkey | FighterKind::Koopa => 8.5,
            FighterKind::GKoopa => 17.0,
            FighterKind::Nana => 1.0,
            _ => 3.5,
        };
    }

    /// The computer player's first attack delay, by its level and a random draw,
    /// `ftCo_800B9704`.
    #[doc(alias = "ftCo_800B9704")]
    pub(super) fn cpu_attack_timer(&mut self, f: FighterId) {
        let rand = self.game.objs.rng.draw_f32();
        let c = &mut self.game.fighters[f.0].cpu;
        let level = fp::frsp(f64::from(10i32.wrapping_sub(c.level)));
        c.ranged_delay = fp::fctiwz(fp::fmadds(level, fp::fmadds(15.0, reg(rand), 15.0), 10.0));
        if c.kind == CpuKind::Defensive {
            c.ranged_delay /= 2;
        }
    }

    /// Whether a floor is one of the stage pieces computer players don't count as standing
    /// ground (Big Blue's cars, Peach's Castle's, Corneria's Arwings, Venom's), `ftCo_800A1B38`.
    #[doc(alias = "ftCo_800A1B38")]
    #[doc(alias = "grBigBlue_801EF844")]
    #[doc(alias = "grInishie1_801FCAAC")]
    #[doc(alias = "grCorneria_801E2D90")]
    #[doc(alias = "grVenom_80206D10")]
    pub(super) fn excluded_floor(&self, line: i32) -> bool {
        if line == -1 {
            return false;
        }
        let joint = || {
            self.game
                .geometry
                .as_ref()
                .expect("no collision geometry")
                .joint_from_line(line)
        };
        match self.game.stage_info.ground_kind() {
            // Gr_Kind_BigBlue
            GrKind::BigBlue => (33..=65).contains(&joint()) && joint() != 34,
            GrKind::Inishie1 => (joint().wrapping_sub(20) as u32) <= 1,
            GrKind::Corneria => matches!(joint(), 0 | 1 | 2 | 5 | 6 | 7),
            GrKind::Venom => (joint().wrapping_sub(3) as u32) <= 1,
            _ => false,
        }
    }

    /// Whether [`Running::excluded_floor`] calls the stage's own check, on the four stages
    /// that have one.
    pub(super) fn excluded_floor_asks_stage(&self) -> bool {
        matches!(
            self.game.stage_info.ground_kind(),
            GrKind::BigBlue | GrKind::Inishie1 | GrKind::Corneria | GrKind::Venom
        )
    }

    /// The r5 [`Running::excluded_floor`] leaves for a floor found (`line` not -1) on a stage
    /// it asks: Big Blue's check (`grBigBlue_801EF844`) ends with the last of its chain of
    /// flags, 1 for joints 33 and 35 to 41, else 0 (Melee's bug, docs/melee-bugs.md);
    /// Mushroom Kingdom's (`grInishie1_801FCAAC`) and Venom's (`grVenom_80206D10`) with what
    /// `mpJointFromLine` left there, the joints' array's address (the allocator model's);
    /// Corneria's (`grCorneria_801E2D90`) with its chain of tests' third flag, whether the
    /// floor's joint is 0, 1, 2 or 5. The address is unfollowed without the allocator model.
    pub(super) fn excluded_floor_r5(&self, line: i32) -> super::cpu_ai::Leftover {
        let geo = self.game.geometry.as_ref().expect("no collision geometry");
        match self.game.stage_info.ground_kind() {
            GrKind::BigBlue => {
                let joint = geo.joint_from_line(line);
                super::cpu_ai::Leftover::Word(u32::from(joint == 33 || (35..=41).contains(&joint)))
            }
            GrKind::Corneria => super::cpu_ai::Leftover::Word(u32::from(matches!(
                geo.joint_from_line(line),
                0 | 1 | 2 | 5
            ))),
            GrKind::Inishie1 | GrKind::Venom if geo.joints_addr() != 0 => {
                super::cpu_ai::Leftover::Word(geo.joints_addr())
            }
            _ => super::cpu_ai::Leftover::Unknown("ftCo_800A1B38 (a stage's excluded floors)"),
        }
    }
}
