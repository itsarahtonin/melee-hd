//! Computer players' state (`ftCo_0A01.c`, `ftcpuattack.c`): what a fighter's creation and
//! spawn set up for its AI. The AI itself isn't ported.

use gekko_fp as fp;

use super::super::handles::FighterId;
use super::super::mirror::{Cx, Mem, Mirror};
use super::super::state::Running;
use super::super::structs::Vec2;
use crate::fpu::{mem, reg};
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
        if fp.kind == KIND_NANA {
            c.xF9_b2 = true;
            c.kind = 6;
            c.x3C = 15.0;
        } else {
            c.kind = kind;
            c.x3C = 40.0;
        }
        c.level = level;
        c.x14 = x14;
        (c.x18, c.x1C, c.x20) = match c.kind {
            1 | 25 => (12, 12, 0),
            3 => (1, 1, 11),
            15 => (0, 0, 0),
            _ => (1, 1, 10),
        };
        c.x24 = 0x12C;
        c.x28 = 0;
        c.x2C = 5;
        c.x30 = 0;
        c.xF9_b0 = false;
        c.x38 = 5.0;
        c.x40 = 50.0;
        c.x44 = None;
        c.x48 = Default::default();
        c.x50 = None;
        c.x98 = fp.cur_pos;
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
        c.x54 = at.clone();
        c.x64 = at;
        c.x60 = 0;
        c.x74 = Vec2 { x: 0.0, y: 0.0 };
        c.x6C = Vec2 { x: 0.0, y: 0.0 };
        c.xF8_b0 = false;
        c.xF8_b12 = 0;
        let t = fp::fctiwz(fp::fmul(10.0, reg(rand)));
        c.x7C = t;
        c.x80 = t;
        c.x84 = 0;
        c.x90 = 0;
        c.x88 = 0;
        c.x8C = 0;
        c.csP = Default::default();
        c.command_duration = 0;
        c.write_pos = SelfRef(Some(0));
        c.xA4 = 0;
        c.xC8 = 0;
        c.xEC = 0;
        c.xF8_b5 = false;
        c.xF8_b6 = false;
        c.xF8_b7 = false;
        c.xF9_b1 = false;
        c.buttons = 0;
        c.lstick.x = 0;
        c.lstick.y = 0;
        c.cstick.x = 0;
        c.cstick.y = 0;
        c.rtrigger = 0;
        c.ltrigger = 0;
        c.xF9_b2 = false;
        c.xF9_b3 = false;
        c.xF9_b4 = false;
        c.xF9_b6 = false;
        c.xF9_b5 = false;
        c.xF9_b7 = false;
        c.xFA_b7 = false;
        c.xFB_b0 = false;
        c.x94 = 0;
        c.xFA_b1 = false;
        c.xFA_b2 = false;
        c.xFA_b34 = 1;
        c.xFA_b5 = false;
        c.xFA_b6 = false;
        c.x55C = 1.0;
        c.x560 = 1.0;
        c.x564 = mem(fp::fmuls(0.5, fp::fadds(reg(c.x55C), reg(c.x560))));
        c.x568 = 2.0;
        c.x444 = SelfRef(Some(5 * INPUT_SIZE));
        c.x448 = SelfRef(Some(0));
        // Nana's recorded inputs start where Popo stands, facing his way.
        let (pos, facing) = if fp.kind == KIND_NANA {
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
        for i in &mut c.xFC {
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
        c.x558 = if gravity < tiny && gravity > fp::fneg(tiny) {
            10.0
        } else {
            mem(fp::frsp(fp::fdiv(fp::fmuls(v, v), fp::fmul(2.0, gravity))))
        };
        c.half_width = 10.0;
        c.half_height = 10.0;
        c.x570 = 1.0;
        self.cpu_attack_timer(f);
        let fp = &mut self.game.fighters[f.0];
        fp.cpu.x56C = match fp.kind {
            3 | 5 => 8.5,
            31 => 17.0,
            11 => 1.0,
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
        c.x34 = fp::fctiwz(fp::fmadds(level, fp::fmadds(15.0, reg(rand), 15.0), 10.0));
        if c.kind == 7 {
            c.x34 /= 2;
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
        match self.game.stage_info.grkind {
            // Gr_Kind_BigBlue
            GR_BIG_BLUE => (33..=65).contains(&joint()) && joint() != 34,
            GR_INISHIE1 => (joint().wrapping_sub(20) as u32) <= 1,
            GR_CORNERIA => matches!(joint(), 0 | 1 | 2 | 5 | 6 | 7),
            GR_VENOM => (joint().wrapping_sub(3) as u32) <= 1,
            _ => false,
        }
    }

    /// Whether [`Running::excluded_floor`] calls the stage's own check, on the four stages
    /// that have one.
    pub(super) fn excluded_floor_asks_stage(&self) -> bool {
        matches!(
            self.game.stage_info.grkind,
            GR_BIG_BLUE | GR_INISHIE1 | GR_CORNERIA | GR_VENOM
        )
    }
}

const KIND_NANA: i32 = 11;

/// `Gr_Kind_BigBlue`, `Gr_Kind_Inishie1`, `Gr_Kind_Corneria`, `Gr_Kind_Venom`.
const GR_BIG_BLUE: i32 = 19;
const GR_INISHIE1: i32 = 24;
const GR_CORNERIA: i32 = 14;
const GR_VENOM: i32 = 15;
