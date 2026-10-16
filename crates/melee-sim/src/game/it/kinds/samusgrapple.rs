//! Samus's grapple beam, `melee/it/kinds/itsamusgrapple.c`: a string of links (`ItemLink`,
//! see [`super::super::link`]) from her arm cannon, the item's own model at the tip. Her grab
//! and dash grab (`ftCo_Catch`'s tether frames), her air grab (`ftCo_AirCatch`) and its hang
//! from a ledge drive it: shot out (state 1), the tip leads and the links follow one by one,
//! each held to the link length from the one before; bouncing back off the stage (state 2),
//! falling (state 3), drawn back in (states 4 and 5), latched on a ledge (state 6), pulling her
//! up (state 7) or hanging (state 8); state 0 is the beam folded in her hand. The item's states
//! keep no callbacks but physics, which only name the frame's work (`unk_10`); the fighter's
//! accessory hooks run it, then line the links' models up.
//!
//! The beam's variables (`itSamusGrapple_ItemVars`) keep its tip (`x0`, the last link made),
//! the link at the hand (`x4`), its fighter (`x8`) and the tip's model (`xC`); the links point
//! toward the tip through `prev` and toward the hand through `next`.
//!
//! The beam's spawn writes its lengths for her height into its attributes in the character's
//! loaded data (`x34` to `x58`); they're state ([`Globals::grapple_lengths`]).

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::item::{article, change, change_state, pick_up, spawn_air};
use super::super::link::{self, apply_vel, direction, link as lk, link_mut, move_coll, place_coll};
use super::super::{item, item_mut, jobj_of};
use crate::fpu::{mem, reg};
use crate::game::code::unported_call;
use crate::game::ef::{self, Arg};
use crate::game::handles::{CodeRef, ItemLinkId};
use crate::game::state::{Obj, Running, UserData};
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::{self as gobjs, GObjId};
use crate::hsd::jobj::{self, JObjId};
use crate::mtx::Mtx34;
use crate::vec::Vec3;
use melee_data::archive::DataRef;

pub const SPAWNED: u32 = 0x802B_7150;
pub const HITBOX: u32 = 0x802B_7160;
pub const LINK_MODEL: u32 = 0x802B_743C;
pub const MAKE_LINKS: u32 = 0x802B_75FC;
pub const PUT_AWAY: u32 = 0x802B_7B84;
pub const SPAWN: u32 = 0x802B_7C18;
pub const FOLDED: u32 = 0x802B_7E34;
pub const PHYS_0: u32 = 0x802B_8048;
pub const SHOT: u32 = 0x802B_805C;
pub const PHYS_1: u32 = 0x802B_8370;
pub const BOUNCED: u32 = 0x802B_8384;
pub const PHYS_2: u32 = 0x802B_8510;
pub const FALLING: u32 = 0x802B_8524;
pub const PHYS_3: u32 = 0x802B_8670;
pub const RETRACT: u32 = 0x802B_8684;
pub const PHYS_4: u32 = 0x802B_8800;
pub const RETRACT_FAST: u32 = 0x802B_8814;
pub const PHYS_5: u32 = 0x802B_8948;
pub const LATCHED: u32 = 0x802B_895C;
pub const PHYS_6: u32 = 0x802B_8B40;
pub const PULL: u32 = 0x802B_8B54;
pub const PHYS_7: u32 = 0x802B_8D24;
pub const HANG: u32 = 0x802B_8D38;
pub const PHYS_8: u32 = 0x802B_8FF8;
pub const PICKED_UP: u32 = 0x802B_A97C;
pub const TO_FALL: u32 = 0x802B_A9B8;
pub const TO_BOUNCE: u32 = 0x802B_AA08;
pub const TO_RETRACT: u32 = 0x802B_AA58;
pub const TO_RETRACT_FAST: u32 = 0x802B_AA94;
pub const TO_SHOT: u32 = 0x802B_AAE4;
pub const TO_LATCH: u32 = 0x802B_AB40;
pub const TO_PULL: u32 = 0x802B_AB7C;
pub const TO_HANG: u32 = 0x802B_ABB8;
/// The fighter's hooks while the beam is out: on death (`death1_cb`), each frame
/// (`accessory2_cb`), and after her animation (`accessory3_cb`).
pub const FT_DEATH: u32 = 0x802B_AC3C;
pub const FT_FRAME: u32 = 0x802B_AC80;
pub const FT_LINE_UP: u32 = 0x802B_ACC4;
pub const EVT_UNK: u32 = 0x802B_AEA0;

/// `itSamusGrappleAttributes` offsets.
mod attr {
    pub const BOUNCE: u32 = 0x0;
    pub const BIG_SHARE: u32 = 0x8;
    pub const COUNT: u32 = 0xC;
    pub const LENGTH: u32 = 0x10;
    pub const LENGTH_MIN: u32 = 0x14;
    pub const SPEED: u32 = 0x18;
    pub const GRAVITY: u32 = 0x1C;
    pub const X20: u32 = 0x20;
    pub const X24: u32 = 0x24;
    pub const X28: u32 = 0x28;
    pub const X2C: u32 = 0x2C;
    pub const FRICTION: u32 = 0x30;
    /// The derived lengths the spawn writes (state, [`super::Lengths`]).
    pub const LINK: u32 = 0x38;
    pub const LINK_MIN: u32 = 0x3C;
    pub const SHOT_SPEED: u32 = 0x40;
    pub const FALL: u32 = 0x44;
    pub const RETRACT: u32 = 0x4C;
    pub const RETRACT_FAST: u32 = 0x54;
    pub const DRAG: u32 = 0x58;
    pub const AIR_MUL: u32 = 0x5C;
    pub const GROUND_MUL: u32 = 0x60;
    pub const LINK_JOINTS: [u32; 3] = [0x64, 0x68, 0x6C];
    pub const TIP_JOINT: u32 = 0x70;
    /// Each link type's anim, matanim and shapeanim joints (pointers to pointers).
    pub const LINK_ANIMS: [[u32; 3]; 3] =
        [[0x74, 0x78, 0x7C], [0x80, 0x84, 0x88], [0x8C, 0x90, 0x94]];
    pub const TIP_ANIMS: [u32; 3] = [0x98, 0x9C, 0xA0];
    pub const BEAM_ANIMS: [u32; 3] = [0xA4, 0xA8, 0xAC];
}

/// The beam's lengths for her height, which its spawn writes over its attributes' `x34` to
/// `x58` in the character's loaded data, and the attributes they're in.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Lengths {
    pub attrs: Option<DataRef>,
    pub n: i32,
    pub words: [f32; 9],
}

/// `ftCo_MS_Catch`, `ftCo_MS_CatchDash` and `ftSs_MS_SpecialAirLw`'s slot `0x165`, the air
/// grab: the states the beam lives through.
const MS_CATCH: i32 = 0xD4;
const MS_CATCH_DASH: i32 = 0xD6;
const MS_AIR_CATCH: i32 = 0x165;
const MS_AIR_CATCH_HIT: i32 = 0x166;
/// The links' process link.
const LINK_P_LINK: u8 = 0xA;
/// `FtPart_ThrowN`, where the beam comes from, and part `0x8B`, which follows its tip.
const PART_THROW_N: i32 = 51;
const PART_TIP: usize = 0x8B;
/// The latch's sparks and sound effect, the hit spark.
const GFX_LATCH: i32 = 0x41C;
const GFX_LATCH_HIT: i32 = 0x3F1;
/// `HSD_PAD_A`, the button sequence's (`x2240`) inputs: `HSD_PAD_DPADUP`, `_DPADDOWN`.
const PAD_A: u32 = 0x100;
const PAD_Z: u32 = 0x40;

/// Runs one of the beam's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        PUT_AWAY => put_away(r, gobj),
        PHYS_0 => set_frame(r, gobj, FOLDED),
        PHYS_1 => set_frame(r, gobj, SHOT),
        PHYS_2 => set_frame(r, gobj, BOUNCED),
        PHYS_3 => set_frame(r, gobj, FALLING),
        PHYS_4 => set_frame(r, gobj, RETRACT),
        PHYS_5 => set_frame(r, gobj, RETRACT_FAST),
        PHYS_6 => set_frame(r, gobj, LATCHED),
        PHYS_7 => set_frame(r, gobj, PULL),
        PHYS_8 => set_frame(r, gobj, HANG),
        FOLDED => folded(r, gobj),
        SHOT => shot(r, gobj),
        BOUNCED => bounced(r, gobj),
        FALLING => falling(r, gobj),
        RETRACT => retract(r, gobj),
        RETRACT_FAST => retract_fast(r, gobj),
        LATCHED => latched(r, gobj),
        PULL => pull(r, gobj),
        HANG => hang(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        TO_FALL => to_fall(r, gobj),
        TO_BOUNCE => to_bounce(r, gobj),
        TO_RETRACT => to_retract(r, gobj),
        TO_RETRACT_FAST => to_retract_fast(r, gobj),
        TO_LATCH => to_latch(r, gobj),
        TO_PULL => to_pull(r, gobj),
        TO_HANG => to_hang(r, gobj),
        _ => return false,
    }
    true
}

/// The beam's attributes (`itSamusGrappleAttributes`), the derived lengths read from the
/// state where its spawn wrote them.
#[derive(Clone, Copy, melee_derive::StateEq)]
pub struct Attrs(DataRef);

impl Attrs {
    fn of(r: &Running, gobj: GObjId) -> Attrs {
        Attrs(
            article(item(r.game, gobj))
                .special(r.data)
                .expect("a grapple beam without attributes"),
        )
    }

    fn f(self, r: &Running, off: u32) -> f64 {
        let l = &r.game.it.grapple_lengths;
        if l.attrs == Some(self.0) && (attr::LINK..=attr::DRAG).contains(&off) {
            return reg(l.words[((off - attr::LINK) / 4) as usize]);
        }
        reg(r.data.f32(self.0.at(off)))
    }

    fn ptr(self, r: &Running, off: u32) -> Option<DataRef> {
        r.data.ptr(self.0.at(off))
    }

    /// An animation joint through its pointer to a pointer, or none.
    fn anim(self, r: &Running, off: u32) -> Option<DataRef> {
        self.ptr(r, off).and_then(|p| r.data.ptr(p))
    }
}

fn tip(r: &mut Running, gobj: GObjId) -> ItemLinkId {
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .samus_grapple()
        .x0
        .get()
        .expect("a grapple beam without its tip")
}

fn hand(r: &mut Running, gobj: GObjId) -> ItemLinkId {
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .samus_grapple()
        .x4
        .get()
        .expect("a grapple beam without its hand link")
}

/// The tip link's GObj.
pub fn tip_gobj(r: &mut Running, gobj: GObjId) -> GObjId {
    let t = tip(r, gobj);
    lk(r.game, t).gobj.expect("a link without its GObj")
}

/// A link GObj's model.
pub fn model_of(r: &Running, g: GObjId) -> Option<JObjId> {
    match r.game.world.gobj(g).obj {
        Some(Obj::JObj(j)) => Some(j),
        _ => None,
    }
}

/// The beam's shot speed for her height (its attributes' `x40` as its spawn wrote it).
pub fn shot_speed(r: &Running, gobj: GObjId) -> f32 {
    mem(Attrs::of(r, gobj).f(r, attr::SHOT_SPEED))
}

/// Whether the fighter is in a state the beam lives through (`samus_grapple_fighter_compare`).
fn holds_beam(motion: i32) -> bool {
    matches!(
        motion,
        MS_AIR_CATCH | MS_AIR_CATCH_HIT | MS_CATCH | MS_CATCH_DASH
    )
}

/// The owner's fighter.
fn owner_of(r: &Running, gobj: GObjId) -> GObjId {
    item(r.game, gobj)
        .owner
        .expect("a grapple beam without its owner")
}

/// A random swing of up to `amp` in the direction of `vel_y`, one time in ten
/// (`samus_grapple_calc_grav` and `it_802B9328_grav`): the double `0.9` and `0.0` as compared.
fn swing(r: &mut Running, vel_y: f32, amp: f64) -> f64 {
    if reg(r.game.objs.rng.draw_f32()) > 0.9 {
        if f64::from(vel_y) < 0.0 {
            fp::fnmsubs(amp, reg(r.game.objs.rng.draw_f32()), reg(vel_y))
        } else {
            fp::fmadds(amp, reg(r.game.objs.rng.draw_f32()), reg(vel_y))
        }
    } else {
        0.0
    }
}

/// A link's joint's world position, its matrix brought up to date
/// (`samus_grapple_setup_pos`).
fn joint_pos(r: &mut Running, l: ItemLinkId) -> Vec3 {
    let j = lk(r.game, l).jobj;
    jobj::setup_matrix(&mut r.game.objs, r.data, j);
    let m = r.game.objs.jobjs[j.expect("a link without its joint")].mtx;
    Vec3::new(m.m[0][3], m.m[1][3], m.m[2][3])
}

/// `itSamusGrapple_Logic53_Spawned`: no frame work yet.
#[doc(alias = "itSamusGrapple_Logic53_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .samus_grapple()
        .unk_10
        .set(None);
}

/// Names the frame's work, the states' physics (`itSamusgrapple_UnkMotion0_Phys` to `8`).
fn set_frame(r: &mut Running, gobj: GObjId, f: u32) {
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .samus_grapple()
        .unk_10
        .set(Some(CodeRef(f)));
}

/// A link's model, `it_802B743C`: the link type's joint, drawn by the links' render callback,
/// its animations from the type's.
#[doc(alias = "it_802B743C")]
pub fn link_model(r: &mut Running, gobj: GObjId, a: DataRef, kind: usize) {
    let a = Attrs(a);
    let desc = a.ptr(r, attr::LINK_JOINTS[kind]);
    let model = jobj::load_joint(&mut r.game.objs, r.data, desc).expect("a link's model");
    let k = super::super::item::jobj_kind(r.game);
    r.game.world.set_obj(gobj, k, Obj::JObj(model));
    r.game
        .world
        .setup_gx_link(gobj, CodeRef(link::RENDER), 6, 0);
    let [an, mat, shape] = attr::LINK_ANIMS[kind];
    let (shape, mat, an) = (a.anim(r, shape), a.anim(r, mat), a.anim(r, an));
    let child = r.game.objs.jobjs[model].child;
    jobj::add_anim_all(&mut r.game.objs, r.data, child, an, mat, shape);
    jobj::req_anim_all(&mut r.game.objs, child, 0.0);
}

/// The beam's links for a fighter of height `scale`, `it_802B75FC`: first its lengths, which
/// it writes over the attributes (the ground's or the air's share, `air`), twice the links
/// for the button sequence's ground beam (`x16` 1); then the links, each a GObj with its
/// model (the three types in turn, the tip its own), all at the joint `hand_joint`. Returns
/// the tip's model.
#[doc(alias = "it_802B75FC")]
pub fn make_links(
    r: &mut Running,
    gobj: GObjId,
    hand_joint: Option<JObjId>,
    air: bool,
    scale: f32,
) -> Option<JObjId> {
    let a = Attrs::of(r, gobj);
    let s = reg(scale);
    let coeff = a.f(r, if air { attr::GROUND_MUL } else { attr::AIR_MUL });
    let base = |r: &Running, off| reg(r.data.f32(a.0.at(off)));
    let link = mem(fp::fmuls(base(r, attr::LENGTH), s));
    let mut w = [0.0f32; 9];
    w[0] = link;
    w[1] = mem(fp::fmuls(base(r, attr::LENGTH_MIN), s));
    w[2] = mem(fp::fmuls(coeff, fp::fmuls(base(r, attr::SPEED), s)));
    w[3] = mem(fp::fmuls(base(r, attr::GRAVITY), s));
    w[4] = mem(fp::fmuls(coeff, fp::fmuls(base(r, attr::X20), s)));
    w[5] = mem(fp::fmuls(coeff, fp::fmuls(base(r, attr::X24), s)));
    w[6] = mem(fp::fmuls(coeff, fp::fmuls(base(r, attr::X28), s)));
    w[7] = mem(fp::fmuls(coeff, fp::fmuls(base(r, attr::X2C), s)));
    w[8] = mem(fp::fmuls(base(r, attr::FRICTION), s));
    let count = fp::frsp(f64::from(r.data.u32(a.0.at(attr::COUNT)) as i32));
    let share = base(r, attr::BIG_SHARE);
    let temp = if s > 1.0 {
        fp::fmadds(
            share,
            fp::fmuls(count, reg(link)),
            fp::fmuls(
                fp::fsubs(1.0, share),
                fp::fmuls(count, base(r, attr::LENGTH)),
            ),
        )
    } else {
        fp::fmuls(count, reg(link))
    };
    let mut n = fp::fctiwz(fp::fdivs(fp::fmuls(temp, coeff), reg(link)));
    if item_mut(r.game, gobj)
        .xDD4_itemVar
        .samus_grapple()
        .x16
        .get()
        == 1
    {
        n = n.wrapping_mul(2);
    }
    r.game.it.grapple_lengths = Lengths {
        attrs: Some(a.0),
        n,
        words: w,
    };
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .samus_grapple()
        .x15
        .set(n as u8);
    let mut prev_link: Option<ItemLinkId> = None;
    let (mut head, mut tail, mut result) = (None, None, None);
    for i in 0..n {
        let g = r.game.world.create(link::CLASS, LINK_P_LINK, 0);
        let l = link::alloc(r.game);
        r.game
            .world
            .init_user_data(g, link::USER_DATA_KIND, UserData::ItemLink(l));
        let size = if i == 0 {
            link_mut(r.game, l).next = None;
            head = Some(l);
            1.0
        } else if i == n - 1 {
            link_mut(r.game, prev_link.unwrap()).prev = Some(l);
            let k = link_mut(r.game, l);
            k.prev = None;
            k.next = prev_link;
            tail = Some(l);
            1.5
        } else {
            link_mut(r.game, prev_link.unwrap()).prev = Some(l);
            link_mut(r.game, l).next = prev_link;
            1.0
        };
        // `samus_grapple_init_link`.
        let k = link_mut(r.game, l);
        k.jobj = hand_joint;
        k.gobj = Some(g);
        k.vel = Vec3::new(0.0, 0.0, 0.0);
        k.pos = Vec3::new(0.0, 0.0, 0.0);
        k.x2C_b0 = false;
        k.x2C_b1 = false;
        k.x2C_b2 = false;
        link::init_coll_fixed(r, l, size);
        if i == n - 1 && i != 0 {
            // `samus_grapple_setup_tail`.
            let desc = a.ptr(r, attr::TIP_JOINT);
            let model = jobj::load_joint(&mut r.game.objs, r.data, desc).expect("the tip's model");
            let k = super::super::item::jobj_kind(r.game);
            r.game.world.set_obj(g, k, Obj::JObj(model));
            r.game.world.setup_gx_link(g, CodeRef(link::RENDER), 6, 0);
            let [an, mat, shape] = attr::TIP_ANIMS;
            let (an, mat, shape) = (a.anim(r, an), a.anim(r, mat), a.anim(r, shape));
            let child = r.game.objs.jobjs[model].child;
            jobj::add_anim_all(&mut r.game.objs, r.data, child, an, mat, shape);
            jobj::req_anim_all(&mut r.game.objs, child, 0.0);
            result = Some(model);
        } else {
            link_model(r, g, a.0, if i == 0 { 0 } else { (i % 3) as usize });
        }
        link_mut(r.game, l).x1CC = -1;
        prev_link = Some(l);
    }
    let v = item_mut(r.game, gobj).xDD4_itemVar.samus_grapple();
    v.x0.set(tail);
    v.x4.set(head);
    result
}

/// Puts the beam away, `it_802B7B84`: while its fighter is known, her beam and hooks are
/// cleared, its links' GObjs go, then the item.
#[doc(alias = "it_802B7B84")]
pub fn put_away(r: &mut Running, gobj: GObjId) {
    let Some(f) = item_mut(r.game, gobj).xDD4_itemVar.samus_grapple().x8.get() else {
        return;
    };
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .samus_grapple()
        .unk_10
        .set(None);
    let fp = r.game.fighter_mut(f);
    fp.u.set_gobj_at(0x10, None);
    fp.accessory2_cb = None;
    fp.death1_cb = None;
    fp.accessory3_cb = None;
    let mut cur = item_mut(r.game, gobj).xDD4_itemVar.samus_grapple().x0.get();
    while let Some(l) = cur {
        let g = lk(r.game, l).gobj.expect("a link without its GObj");
        cur = lk(r.game, l).next;
        gobjs::free(r, g);
    }
    super::super::item::destroy(r, gobj);
}

/// Makes the beam at `pos` for `owner`, `it_802B7C18`: only in the states that use it; the
/// button sequence's beam (`x2240` 4, `x16`) on the ground or in the air; its links from her
/// throw joint; the beam's own animations; her tip part following its tip; held at her
/// throw joint, folded.
#[doc(alias = "it_802B7C18")]
pub fn spawn(r: &mut Running, owner: GObjId, pos: Vec3, facing_dir: f32) -> Option<GObjId> {
    let motion = r.game.fighter(owner).motion_id;
    if !holds_beam(motion) {
        return None;
    }
    let mut spawn = SpawnItem {
        kind: super::SAMUS_GBEAM,
        ..Default::default()
    };
    // `Item_InitSpawnPosition(&spawn, pos, false)`.
    spawn.prev_pos = pos;
    spawn.pos = spawn.prev_pos;
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(owner);
    spawn.x4_parent_gobj2 = spawn.x0_parent_gobj;
    spawn.x44_flag[0] |= 0x80;
    let beam = spawn_air(r, &mut spawn)?;
    // The spawn never sets `x40`, a stack word: the beam's ignore id is stale.
    item_mut(r.game, beam).xAC4_ignoreItemID = super::super::IgnoreId(None);
    item_mut(r.game, beam)
        .xDD4_itemVar
        .samus_grapple()
        .x14
        .set(0);
    let fp = r.game.fighter(owner);
    let x16 = if fp.u[0x14] >= 4 {
        if fp.ground_or_air == crate::game::ft::common::GA_GROUND {
            1
        } else {
            2
        }
    } else {
        0
    };
    if x16 == 0 {
        r.game.fighter_mut(owner).u[0x14] = 0;
    }
    item_mut(r.game, beam)
        .xDD4_itemVar
        .samus_grapple()
        .x16
        .set(x16);
    let air = motion == MS_AIR_CATCH;
    let throw_n = r.bone_joint(owner, PART_THROW_N);
    let scale = r.game.fighter(owner).x34_scale.y;
    if make_links(r, beam, throw_n, air, scale).is_none() {
        super::super::item::destroy(r, beam);
        return None;
    }
    let a = Attrs::of(r, beam);
    let [an, mat, shape] = attr::BEAM_ANIMS;
    let (shape, mat, an) = (a.anim(r, shape), a.anim(r, mat), a.anim(r, an));
    let root = jobj_of(r.game, beam).expect("a grapple beam without a model");
    let child = r.game.objs.jobjs[root].child;
    jobj::add_anim_all(&mut r.game.objs, r.data, child, an, mat, shape);
    jobj::req_anim_all(&mut r.game.objs, child, 0.0);
    let t = tip(r, beam);
    let tip_model = match r.game.world.gobj(lk(r.game, t).gobj.unwrap()).obj {
        Some(Obj::JObj(j)) => Some(j),
        _ => None,
    };
    let v = item_mut(r.game, beam).xDD4_itemVar.samus_grapple();
    v.x8.set(Some(owner));
    v.xC.set(tip_model);
    // Past her bones, the table's last entry (`MAX_FT_PARTS` of them).
    let f = r.game.fighter_id(owner);
    let parts = r.game.fighters[f.0].parts.expect("a fighter without parts");
    let bones = &mut r.game.fighter_parts[parts.0];
    if bones.len() <= PART_TIP {
        bones.resize(PART_TIP + 1, Default::default());
    }
    bones[PART_TIP].joint = tip_model;
    pick_up(r, beam, owner, PART_THROW_N);
    stop_anim(r, beam);
    Some(beam)
}

/// The beam's (or Link's hookshot's) scale from its owner's, `it_802A2428` (the hookshot's
/// unit): it reads its owner as an item, whose `scl` is a fighter's height (`x34_scale.y`, at
/// the same offset), times its own attributes' scale.
#[doc(alias = "it_802A2428")]
pub(super) fn stop_anim(r: &mut Running, gobj: GObjId) {
    let owner = owner_of(r, gobj);
    let height = reg(r.game.fighter(owner).x34_scale.y);
    let a = super::super::item::attr_of(item(r.game, gobj));
    let s = mem(fp::fmuls(
        height,
        reg(a.f32(r.data, super::super::data::attr::SCALE)),
    ));
    item_mut(r.game, gobj).scl = s;
    let j = jobj_of(r.game, gobj).expect("a tether without a model");
    super::super::lib2725::set_uniform_scale(r.game, j, s);
}

/// The tip's model put on the tip link's joint's matrix (`fn_802B7E34_inline`).
fn tip_on_hand(r: &mut Running, gobj: GObjId) {
    let t = tip(r, gobj);
    let g = lk(r.game, t).gobj.expect("a link without its GObj");
    let model = match r.game.world.gobj(g).obj {
        Some(Obj::JObj(j)) => j,
        ref o => panic!("a link GObj holding {o:?}"),
    };
    let j = lk(r.game, t).jobj;
    jobj::setup_matrix(&mut r.game.objs, r.data, j);
    let m = r.game.objs.jobjs[j.expect("a link without its joint")]
        .mtx
        .concat(&Mtx34::IDENTITY);
    let jobjs = &mut r.game.objs.jobjs;
    jobjs[model].mtx = m;
    jobjs[model].flags |= 0x0380_0000;
    jobj::set_mtx_dirty(jobjs, Some(model));
}

/// The beam's models animate, the links' from the hand out (`samus_grapple_anim`).
fn animate(r: &mut Running, gobj: GObjId) {
    let root = jobj_of(r.game, gobj);
    jobj::anim_all(
        &mut r.game.objs,
        r.data,
        &mut crate::game::code::ParticleHooks,
        root,
    );
    let mut cur = Some(hand(r, gobj));
    while let Some(l) = cur {
        let g = lk(r.game, l).gobj.expect("a link without its GObj");
        let model = match r.game.world.gobj(g).obj {
            Some(Obj::JObj(j)) => Some(j),
            _ => None,
        };
        jobj::anim_all(
            &mut r.game.objs,
            r.data,
            &mut crate::game::code::ParticleHooks,
            model,
        );
        cur = lk(r.game, l).prev;
    }
}

/// The button sequence (`samus_grapple_state_sync`): D-pad up, down, up, then A.
fn sequence(r: &mut Running, f: GObjId) {
    let fp = r.game.fighter_mut(f);
    let held = fp.input.held_buttons[0];
    let step = match fp.u[0x14] {
        0 | 2 => held & 8 != 0,
        1 => held & 4 != 0,
        3 => fp.input.pressed_buttons & PAD_A != 0,
        _ => false,
    };
    if step {
        fp.u[0x14] = fp.u[0x14].wrapping_add(1);
    }
}

/// Folded in her hand (state 0), `fn_802B7E34`: the tip on her hand and the models
/// animating; out of the states that use it, put away.
#[doc(alias = "fn_802B7E34")]
pub fn folded(r: &mut Running, gobj: GObjId) {
    let f = owner_of(r, gobj);
    sequence(r, f);
    tip_on_hand(r, gobj);
    animate(r, gobj);
    if !holds_beam(r.game.fighter(f).motion_id) {
        put_away(r, gobj);
    }
}

/// Shot out (state 1), `fn_802B805C`: out of the states that use it with the first link not
/// out, put away; else the tip leads ([`lead`]): against a wall, the air grab latches on (the
/// latch's effects), else the tip bounces back off the wall or the floor, and her hit status
/// clears; all out, it falls.
#[doc(alias = "fn_802B805C")]
pub fn shot(r: &mut Running, gobj: GObjId) {
    let a = Attrs::of(r, gobj);
    let f = owner_of(r, gobj);
    sequence(r, f);
    let t = tip(r, gobj);
    let first_out = lk(r.game, t).next.is_some_and(|n| lk(r.game, n).x2C_b0);
    if !holds_beam(r.game.fighter(f).motion_id) && !first_out {
        put_away(r, gobj);
        return;
    }
    // `samus_grapple_setup_pos`: whether `HSD_JObjSetupMatrix` rebuilds the tip's matrix
    // under its parent decides what `it_802B9328`'s swing finds on the stack.
    let slot = match lk(r.game, t).jobj {
        Some(j)
            if r.game.objs.jobjs[j].mtx_is_dirty()
                && r.game.objs.jobjs[j].parent.is_some()
                && r.game.objs.jobjs[j]
                    .aobj
                    .and_then(|o| r.game.objs.aobjs[o].hsd_obj)
                    .is_none() =>
        {
            SwingSlot::SavedF14
        }
        _ => SwingSlot::Unknown("fn_802B805C with the tip's matrix not rebuilt under a parent"),
    };
    let mut pos = joint_pos(r, t);
    match lead(r, t, &mut pos, a, f, slot) {
        1 => {
            if r.game.fighter(f).motion_id == MS_AIR_CATCH {
                r.enter_air_catch_hit(f);
                to_latch(r, gobj);
                let t = tip(r, gobj);
                let at = lk(r.game, t).pos;
                let g = lk(r.game, t).gobj.expect("a link without its GObj");
                let facing = r.game.fighter(f).facing_dir;
                ef::spawn(r, GFX_LATCH, Some(g), &[Arg::Vec(at)]);
                ef::spawn(
                    r,
                    GFX_LATCH_HIT,
                    Some(g),
                    &[Arg::Vec(at), Arg::Float(facing)],
                );
                return;
            }
            let bounce = a.f(r, attr::BOUNCE);
            let k = link_mut(r.game, t);
            k.vel.x = mem(fp::fmuls(reg(k.vel.x), fp::fneg(bounce)));
            to_bounce(r, gobj);
            r.game.fighter_mut(f).x1A6A = 0;
        }
        2 => {
            let k = link_mut(r.game, t);
            let mut normal = k.coll_data.floor.normal;
            normal.normalize();
            k.vel.mirror_xy(normal);
            let bounce = a.f(r, attr::BOUNCE);
            let k = link_mut(r.game, t);
            k.vel.y = mem(fp::fmuls(reg(k.vel.y), bounce));
            to_bounce(r, gobj);
            r.game.fighter_mut(f).x1A6A = 0;
        }
        3 => {
            to_fall(r, gobj);
            r.game.fighter_mut(f).x1A6A = 0;
        }
        _ => {}
    }
    let scale = r.game.fighter(f).x34_scale.y;
    line_up(r, gobj, pos, scale);
    animate(r, gobj);
}

/// Bouncing back (state 2), `fn_802B8384`: the tip falls back ([`fall_back`]), bouncing off
/// walls and floors; all out, it falls.
#[doc(alias = "fn_802B8384")]
pub fn bounced(r: &mut Running, gobj: GObjId) {
    let a = Attrs::of(r, gobj);
    let f = owner_of(r, gobj);
    let t = tip(r, gobj);
    let mut pos = joint_pos(r, t);
    match fall_back(r, t, &mut pos, a, f) {
        1 => {
            let bounce = a.f(r, attr::BOUNCE);
            let k = link_mut(r.game, t);
            k.vel.x = mem(fp::fmuls(reg(k.vel.x), fp::fneg(bounce)));
        }
        2 => {
            let k = link_mut(r.game, t);
            let mut normal = k.coll_data.floor.normal;
            normal.normalize();
            k.vel.mirror_xy(normal);
            let bounce = a.f(r, attr::BOUNCE);
            let k = link_mut(r.game, t);
            k.vel.y = mem(fp::fmuls(reg(k.vel.y), bounce));
        }
        3 => to_fall(r, gobj),
        _ => {}
    }
    let scale = r.game.fighter(f).x34_scale.y;
    line_up(r, gobj, pos, scale);
    animate(r, gobj);
}

/// Falling (state 3), `fn_802B8524`: the links hang from the hand ([`hang_links`]); out of
/// the states that use it, drawn back in.
#[doc(alias = "fn_802B8524")]
pub fn falling(r: &mut Running, gobj: GObjId) {
    let a = Attrs::of(r, gobj);
    let f = owner_of(r, gobj);
    let h = hand(r, gobj);
    let mut pos = joint_pos(r, h);
    hang_links(r, h, &mut pos, a, f);
    let scale = r.game.fighter(f).x34_scale.y;
    line_up(r, gobj, pos, scale);
    animate(r, gobj);
    if !holds_beam(r.game.fighter(f).motion_id) {
        to_retract(r, gobj);
    }
}

/// Drawn in (state 4), `fn_802B8684`: the links wind in at the attributes' `x4C` (twice that
/// in the air); all in, it's put away out of the states that use it, else folds.
#[doc(alias = "fn_802B8684")]
pub fn retract(r: &mut Running, gobj: GObjId) {
    let a = Attrs::of(r, gobj);
    let f = owner_of(r, gobj);
    let h = hand(r, gobj);
    let mut pos = joint_pos(r, h);
    let speed = if r.game.fighter(f).ground_or_air == crate::game::ft::common::GA_GROUND {
        a.f(r, attr::RETRACT)
    } else {
        fp::frsp(fp::fmul(2.0, a.f(r, attr::RETRACT)))
    };
    if wind_in(r, h, &mut pos, a, speed) {
        if !holds_beam(r.game.fighter(f).motion_id) {
            put_away(r, gobj);
            return;
        }
        picked_up(r, gobj);
    }
    let scale = r.game.fighter(f).x34_scale.y;
    line_up(r, gobj, pos, scale);
    animate(r, gobj);
}

/// Drawn in fast (state 5), `fn_802B8814`: all in the frame before (`x14`), put away; else the
/// links come in at the attributes' `x54`.
#[doc(alias = "fn_802B8814")]
pub fn retract_fast(r: &mut Running, gobj: GObjId) {
    let a = Attrs::of(r, gobj);
    let f = owner_of(r, gobj);
    if item_mut(r.game, gobj)
        .xDD4_itemVar
        .samus_grapple()
        .x14
        .get()
        != 0
    {
        put_away(r, gobj);
        return;
    }
    let h = hand(r, gobj);
    let mut pos = joint_pos(r, h);
    let speed = a.f(r, attr::RETRACT_FAST);
    if wind_in_chain(r, h, &mut pos, a, speed) {
        item_mut(r.game, gobj)
            .xDD4_itemVar
            .samus_grapple()
            .x14
            .set(1);
        return;
    }
    let scale = r.game.fighter(f).x34_scale.y;
    line_up(r, gobj, pos, scale);
    animate(r, gobj);
}

/// Whether the stage cuts between the tip and the hand's point, or her and the tip
/// (`itGrappleCheckCollision`).
fn cut(r: &mut Running, tip: ItemLinkId, f: GObjId, pos: Vec3) -> bool {
    let at = lk(r.game, tip).pos;
    let me = r.game.fighter(f).cur_pos;
    let g = r.game.geometry.as_mut().expect("no collision geometry");
    g.check_all_remap(-1, -1, pos.x, pos.y, at.x, at.y)
        .is_some()
        || g.check_all_remap(-1, -1, me.x, me.y, at.x, at.y).is_some()
}

/// Latched on (state 6), `fn_802B895C`: if the stage cuts the beam she tumbles and it's put
/// away; once the links are all out it hangs; on the ground it's drawn in; A pulls her up.
#[doc(alias = "fn_802B895C")]
pub fn latched(r: &mut Running, gobj: GObjId) {
    let a = Attrs::of(r, gobj);
    let f = owner_of(r, gobj);
    let h = hand(r, gobj);
    let mut pos = joint_pos(r, h);
    let t = tip(r, gobj);
    if cut(r, t, f, pos) {
        r.enter_damage_fall(f);
        put_away(r, gobj);
        return;
    }
    if pay_out(r, t, &mut pos, a) {
        to_hang(r, gobj);
        return;
    }
    let scale = r.game.fighter(f).x34_scale.y;
    line_up(r, gobj, pos, scale);
    animate(r, gobj);
    if r.game.fighter(f).ground_or_air != crate::game::ft::common::GA_AIR {
        to_retract(r, gobj);
        return;
    }
    if r.game.fighter(f).input.pressed_buttons & PAD_A != 0 {
        to_pull(r, gobj);
    }
}

/// Pulling her up (state 7), `fn_802B8B54`: the links wind in at the attributes' `x4C`, she
/// keeps her place on the beam; all in, she catches the ledge if one is in reach and free,
/// else stops and lands helpless (`ftCo_8009B390`, her attributes' `xCC`), and it's put away;
/// on the ground it's drawn in.
#[doc(alias = "fn_802B8B54")]
pub fn pull(r: &mut Running, gobj: GObjId) {
    let a = Attrs::of(r, gobj);
    let f = owner_of(r, gobj);
    let h = hand(r, gobj);
    let mut pos = joint_pos(r, h);
    let fp = r.game.fighter(f);
    let dx = mem(fp::fsubs(reg(fp.cur_pos.x), reg(pos.x)));
    let dy = mem(fp::fsubs(reg(fp.cur_pos.y), reg(pos.y)));
    let t = tip(r, gobj);
    let speed = a.f(r, attr::RETRACT);
    let done = climb(r, h, t, &mut pos, a, speed);
    let fp = r.game.fighter_mut(f);
    fp.cur_pos.x = mem(fp::fadds(reg(pos.x), reg(dx)));
    fp.cur_pos.y = mem(fp::fadds(reg(pos.y), reg(dy)));
    if done {
        if r.ledge_in_reach(f) && !r.ledge_taken(f) {
            r.enter_cliff_catch(f);
        } else {
            r.game.fighter_mut(f).self_vel.x = 0.0;
            r.samus_tether_landing(f);
        }
        put_away(r, gobj);
        return;
    }
    let scale = r.game.fighter(f).x34_scale.y;
    line_up(r, gobj, pos, scale);
    animate(r, gobj);
    if r.game.fighter(f).ground_or_air != crate::game::ft::common::GA_AIR {
        to_retract(r, gobj);
    }
}

/// Hanging (state 8), `fn_802B8D38`: if the stage cuts the beam she tumbles and it's put
/// away; the links hold her to the beam, which moves her; Z held lengthens the button
/// sequence's hang; on the ground it's drawn in; A pulls her up; when her hang runs out she
/// tumbles and it's put away.
#[doc(alias = "fn_802B8D38")]
pub fn hang(r: &mut Running, gobj: GObjId) {
    let a = Attrs::of(r, gobj);
    let f = owner_of(r, gobj);
    let h = hand(r, gobj);
    let mut pos = joint_pos(r, h);
    let t = tip(r, gobj);
    if cut(r, t, f, pos) {
        r.enter_damage_fall(f);
        put_away(r, gobj);
        return;
    }
    let fp = r.game.fighter(f);
    let ox = mem(fp::fsubs(reg(fp.cur_pos.x), reg(pos.x)));
    let oy = mem(fp::fsubs(reg(fp.cur_pos.y), reg(pos.y)));
    let saved = fp.cur_pos;
    hold_on(r, h, t, &mut pos, a);
    let fp = r.game.fighter_mut(f);
    fp.cur_pos.x = mem(fp::fadds(reg(pos.x), reg(ox)));
    fp.cur_pos.y = mem(fp::fadds(reg(pos.y), reg(oy)));
    let vx = mem(fp::fadds(
        reg(fp.self_vel.x),
        fp::fsubs(reg(fp.cur_pos.x), reg(saved.x)),
    ));
    fp.self_vel.x = vx;
    fp.pos_delta.x = vx;
    let vy = mem(fp::fadds(
        reg(fp.self_vel.y),
        fp::fsubs(reg(fp.cur_pos.y), reg(saved.y)),
    ));
    fp.self_vel.y = vy;
    fp.pos_delta.y = vy;
    let scale = fp.x34_scale.y;
    line_up(r, gobj, pos, scale);
    animate(r, gobj);
    const HANG: usize = crate::game::ft::samus::mv::GRAPPLE_HANG;
    let x16 = item_mut(r.game, gobj)
        .xDD4_itemVar
        .samus_grapple()
        .x16
        .get();
    let fp = r.game.fighter_mut(f);
    if x16 != 0 && fp.input.held_buttons[0] & PAD_Z != 0 {
        let v = fp::fadds(reg(fp.mv_f32(HANG)), 1.0);
        fp.set_mv_f32(HANG, v);
    }
    if fp.ground_or_air != crate::game::ft::common::GA_AIR {
        to_retract(r, gobj);
        return;
    }
    if fp.input.pressed_buttons & PAD_A != 0 {
        to_pull(r, gobj);
        return;
    }
    let left = fp.mv_f32(HANG);
    fp.set_mv_f32(HANG, fp::fsubs(reg(left), 1.0));
    if left <= 0.0 {
        r.enter_damage_fall(f);
        put_away(r, gobj);
    }
}

/// Holds the links from `l` to the hand's point `pos`: `l` first to `dist` from it, then each
/// toward the tip falling with a random swing and held between the link lengths of the one
/// before, `it_802B900C`.
#[doc(alias = "it_802B900C")]
pub fn chase(r: &mut Running, l: ItemLinkId, pos: Vec3, a: Attrs, dist: f64) {
    hold_from(r, l, pos, a, dist, |r, p| {
        let vy = lk(r.game, p).vel.y;
        let s = swing(r, vy, fp::frsp(0.6));
        let k = link_mut(r.game, p);
        k.vel.y = mem(fp::fsubs(reg(k.vel.y), s));
    });
}

/// [`chase`] with the links falling by the attributes' `x44`, `it_802B91C4`.
#[doc(alias = "it_802B91C4")]
pub fn chase_falling(r: &mut Running, l: ItemLinkId, pos: Vec3, a: Attrs, dist: f64) {
    let g = a.f(r, attr::FALL);
    hold_from(r, l, pos, a, dist, |r, p| {
        let k = link_mut(r.game, p);
        k.vel.y = mem(fp::fsubs(reg(k.vel.y), g));
    });
}

/// The shared walk of [`chase`] and [`chase_falling`].
fn hold_from(
    r: &mut Running,
    l: ItemLinkId,
    pos: Vec3,
    a: Attrs,
    dist: f64,
    mut fall: impl FnMut(&mut Running, ItemLinkId),
) {
    let mut dir = Vec3::default();
    let mut cur = l;
    let mut prev = lk(r.game, cur).prev;
    direction(lk(r.game, cur).pos, pos, &mut dir);
    let k = link_mut(r.game, cur);
    k.pos.x = mem(fp::fmadds(reg(dir.x), dist, reg(pos.x)));
    k.pos.y = mem(fp::fmadds(reg(dir.y), dist, reg(pos.y)));
    k.pos.z = mem(fp::fmadds(reg(dir.z), dist, reg(pos.z)));
    let (max, min) = (a.f(r, attr::LINK), a.f(r, attr::LINK_MIN));
    while let Some(p) = prev {
        fall(r, p);
        apply_vel(r.game, p);
        move_coll(r.game, p);
        let at = lk(r.game, cur).pos;
        let d = direction(lk(r.game, p).pos, at, &mut dir);
        if d > max {
            hold_to(r, p, at, dir, max);
        } else if d < min {
            hold_to(r, p, at, dir, min);
        }
        cur = p;
        prev = lk(r.game, cur).prev;
    }
}

/// A link put `len` along `d` from `at`.
fn hold_to(r: &mut Running, l: ItemLinkId, at: Vec3, d: Vec3, len: f64) {
    let k = link_mut(r.game, l);
    k.pos.x = mem(fp::fmadds(reg(d.x), len, reg(at.x)));
    k.pos.y = mem(fp::fmadds(reg(d.y), len, reg(at.y)));
    k.pos.z = mem(fp::fmadds(reg(d.z), len, reg(at.z)));
}

/// `it_803B8660`: the button sequence's swing hitbox, five hitbox command words (hitbox 0 of
/// group 0 on bone 139, no damage, size 1200, angle 361, knockback growth 100).
const SWING_HITBOX: [u32; 5] = [0x2C04_5800, 0x04B0_0000, 0, 0xB499_0012, 0x0020_008A];

/// What the stack holds past the swing's hitbox words when its caller makes the swing
/// (`it_802B7160` reads the grabbed-victim-only bit from `x134_4`, byte 0x15 of the caller's
/// `itSamusGrapple_HitboxData`, which the caller never writes): the stale-stack rule
/// (docs/engine.md).
#[derive(Clone, Copy)]
pub enum SwingSlot {
    /// `it_802B9328`'s, when `fn_802B805C` has just rebuilt the tip's matrix under its parent
    /// (`HSD_JObjSetupMatrix`, its dirty joint's `HSD_JObjMakeMatrix` ending in
    /// `PSMTXConcat`): `PSMTXConcat`'s save of `f14` lands on the slot, and the game's code
    /// never leaves `f14` live at that depth, so the word is 0.0's.
    SavedF14,
    /// Not followed, named by the frame: reading it stops.
    Unknown(&'static str),
}

/// The ground beam's hitbox (the button sequence's, `x16` 1): with A on the ground she
/// swings it (`it_802B7160` with `it_803B8660`), and it turns into the swing (`x16` 2); her
/// hit status clears either way. The swing's grabbed-victim-only bit is what `slot` says the
/// caller's stack holds there.
fn ground_swing(r: &mut Running, f: GObjId, slot: SwingSlot) {
    let beam = r
        .game
        .fighter(f)
        .u
        .gobj_at(0x10)
        .expect("a grapple beam's fighter without it");
    let fp = r.game.fighter(f);
    if fp.ground_or_air != crate::game::ft::common::GA_GROUND
        || item_mut(r.game, beam)
            .xDD4_itemVar
            .samus_grapple()
            .x16
            .get()
            != 1
    {
        return;
    }
    r.fighter_clear_hitboxes(f);
    if r.game.fighter(f).input.pressed_buttons & PAD_A != 0 {
        let sixth = match slot {
            // 0.0's high word: no bit set.
            SwingSlot::SavedF14 => 0,
            SwingSlot::Unknown(frame) => unported_call(&format!(
                "it_802B7160: the swing's grabbed-victim-only bit, a stack byte {frame} leaves"
            )),
        };
        let [a, b, c, d, e] = SWING_HITBOX;
        let fid = r.game.fighter_id(f);
        swing_hitbox(r, f, fid, [a, b, c, d, e, sixth]);
        let v = item_mut(r.game, beam).xDD4_itemVar.samus_grapple();
        let n = v.x16.get();
        v.x16.set(n + 1);
    }
}

/// The fighter hitbox of a grapple swing, `it_802B7160`: a hitbox command's body
/// ([`Running::fighter_hitbox_from`]) on the words its caller hands it.
#[doc(alias = "it_802B7160")]
fn swing_hitbox(r: &mut Running, f: GObjId, fid: crate::game::handles::FighterId, w: [u32; 6]) {
    r.fighter_hitbox_from(f, fid, w);
}

/// The tip leads and the links follow, `it_802B9328`: first it attaches to her throw joint on
/// the frame her state says (her attributes' `xA0`, `xB0` or `xC0` by state); the ground
/// beam's swing; Z homes the ground beam on the nearest foe (`ftCo_800A4A40`); the tip flies
/// and meets the stage (1 a wall, the wall's line kept; 2 a floor or ceiling); each link out
/// follows held between the link lengths, and the next one leaves once the point is a link
/// length on. All out (3), the links chase the hand.
#[doc(alias = "it_802B9328")]
pub fn lead(
    r: &mut Running,
    l: ItemLinkId,
    pos: &mut Vec3,
    a: Attrs,
    f: GObjId,
    slot: SwingSlot,
) -> i32 {
    let fid = r.game.fighter_id(f);
    let frame = r.game.fighters[fid.0].mv_f32(0);
    let motion = r.game.fighters[fid.0].motion_id;
    let at = match motion {
        MS_CATCH => Some(crate::game::ft::samus::attr::CATCH_ATTACH),
        MS_CATCH_DASH => Some(crate::game::ft::samus::attr::CATCH_DASH_ATTACH),
        MS_AIR_CATCH => Some(crate::game::ft::samus::attr::AIR_CATCH_ATTACH),
        _ => None,
    };
    if let Some(off) = at
        && reg(frame) == fp::frsp(f64::from(r.samus_ext_attr_i32(fid, off)))
    {
        // `it_802B9328_attach`.
        let p = joint_pos(r, l);
        let k = link_mut(r.game, l);
        k.pos = p;
        k.coll_data.cur_pos = k.pos;
        k.coll_data.last_pos = k.coll_data.cur_pos;
        k.x2C_b0 = true;
    }
    ground_swing(r, f, slot);
    let next = lk(r.game, l).next;
    let beam = r.game.fighter(f).u.gobj_at(0x10).expect("the beam");
    if r.game.fighter(f).input.held_buttons[0] & PAD_Z != 0
        && item_mut(r.game, beam)
            .xDD4_itemVar
            .samus_grapple()
            .x16
            .get()
            == 1
        && let Some(t) = r.nearest_fighter(fid)
    {
        let tp = r.game.fighters[t.0].cur_pos;
        let k = lk(r.game, l);
        let sub = |a: f32, b: f32| mem(fp::fsubs(reg(a), reg(b)));
        let mut d = Vec3::new(sub(tp.x, k.pos.x), sub(tp.y, k.pos.y), sub(tp.z, k.pos.z));
        d.normalize();
        let speed = a.f(r, attr::SHOT_SPEED);
        let k = link_mut(r.game, l);
        k.vel.x = mem(fp::fmuls(reg(d.x), speed));
        k.vel.y = mem(fp::fmuls(reg(d.y), speed));
        k.vel.z = mem(fp::fmuls(reg(d.z), speed));
    }
    apply_vel(r.game, l);
    let mut result = link::coll_air(r, l);
    if result != 0 {
        result = if result & 0x3F != 0 {
            let w = lk(r.game, l).coll_data.left_facing_wall.index;
            link_mut(r.game, l).x1CC = w;
            1
        } else if result & 0xFC0 != 0 {
            let w = lk(r.game, l).coll_data.right_facing_wall.index;
            link_mut(r.game, l).x1CC = w;
            1
        } else {
            2
        };
    }
    let mut cur = l;
    let mut next = next;
    let (max, min) = (a.f(r, attr::LINK), a.f(r, attr::LINK_MIN));
    let mut dir = Vec3::default();
    while let Some(n) = next {
        if lk(r.game, n).x2C_b0 {
            let vy = lk(r.game, n).vel.y;
            let s = swing(r, vy, 1.0);
            let k = link_mut(r.game, n);
            k.vel.y = mem(fp::fsubs(reg(k.vel.y), s));
            k.pos.y = mem(fp::fadds(reg(k.pos.y), reg(k.vel.y)));
            let at = lk(r.game, cur).pos;
            let d = direction(lk(r.game, n).pos, at, &mut dir);
            if d > max {
                hold_to(r, n, at, dir, max);
            } else if d < min {
                hold_to(r, n, at, dir, min);
            }
            move_coll(r.game, n);
        } else {
            let at = lk(r.game, cur).pos;
            if direction(*pos, at, &mut dir) > max {
                hold_to(r, n, at, dir, max);
                link_mut(r.game, n).x2C_b0 = true;
                place_coll(r.game, n);
            } else {
                return result;
            }
        }
        cur = n;
        next = lk(r.game, cur).next;
    }
    chase(r, cur, *pos, a, max);
    3
}

/// The tip falls back with a random swing, sliding to a stop by the attributes' `x58`, and the
/// links follow as in [`lead`], `it_802B99A0`: 1 a wall, 2 a floor; all out (3), the links
/// chase the hand.
#[doc(alias = "it_802B99A0")]
pub fn fall_back(r: &mut Running, l: ItemLinkId, pos: &mut Vec3, a: Attrs, f: GObjId) -> i32 {
    ground_swing(r, f, SwingSlot::Unknown("it_802B99A0"));
    let next = lk(r.game, l).next;
    let vy = lk(r.game, l).vel.y;
    let s = swing(r, vy, fp::frsp(0.6));
    let drag = a.f(r, attr::DRAG);
    let k = link_mut(r.game, l);
    k.vel.y = mem(fp::fsubs(reg(k.vel.y), s));
    k.vel.x = mem(slide(reg(k.vel.x), drag));
    apply_vel(r.game, l);
    let mut result = link::coll_air(r, l);
    if result != 0 {
        result = if result & 0xFFF != 0 { 1 } else { 2 };
    }
    let mut cur = l;
    let mut next = next;
    let (max, min) = (a.f(r, attr::LINK), a.f(r, attr::LINK_MIN));
    let mut dir = Vec3::default();
    while let Some(n) = next {
        if lk(r.game, n).x2C_b0 {
            let vy = lk(r.game, n).vel.y;
            let s = swing(r, vy, fp::frsp(0.6));
            let k = link_mut(r.game, n);
            k.vel.y = mem(fp::fsubs(reg(k.vel.y), s));
            apply_vel(r.game, n);
            let at = lk(r.game, cur).pos;
            let d = direction(lk(r.game, n).pos, at, &mut dir);
            if d > max {
                hold_to(r, n, at, dir, max);
            } else if d < min {
                hold_to(r, n, at, dir, min);
            }
            move_coll(r.game, n);
        } else {
            let at = lk(r.game, cur).pos;
            if direction(*pos, at, &mut dir) > max {
                hold_to(r, n, at, dir, max);
                link_mut(r.game, n).x2C_b0 = true;
                place_coll(r.game, n);
            } else {
                return result;
            }
        }
        cur = n;
        next = lk(r.game, cur).next;
    }
    chase(r, cur, *pos, a, max);
    3
}

/// A speed slowed by `drag`, stopping inside it.
fn slide(v: f64, drag: f64) -> f64 {
    if v > drag {
        fp::fsubs(v, drag)
    } else if v < fp::fneg(drag) {
        fp::fadds(v, drag)
    } else {
        0.0
    }
}

/// The links hang from the hand, `it_802B9CE8`: the ground beam's swing; from the first link
/// out, each falls with a random swing, held within a link length of the hand's point or the
/// one before (and no closer than the shortest), the last sliding to a stop.
#[doc(alias = "it_802B9CE8")]
pub fn hang_links(r: &mut Running, l: ItemLinkId, pos: &mut Vec3, a: Attrs, f: GObjId) {
    ground_swing(r, f, SwingSlot::Unknown("it_802B9CE8"));
    let mut cur = l;
    let mut prev = lk(r.game, cur).prev;
    while let Some(p) = prev {
        if lk(r.game, cur).x2C_b0 {
            break;
        }
        cur = p;
        prev = lk(r.game, p).prev;
    }
    let vy = lk(r.game, cur).vel.y;
    let s = swing(r, vy, fp::frsp(0.6));
    let k = link_mut(r.game, cur);
    k.vel.y = mem(fp::fsubs(reg(k.vel.y), s));
    apply_vel(r.game, cur);
    let (max, min) = (a.f(r, attr::LINK), a.f(r, attr::LINK_MIN));
    let mut dir = Vec3::default();
    if direction(lk(r.game, cur).pos, *pos, &mut dir) > max {
        hold_to(r, cur, *pos, dir, max);
    }
    while let Some(p) = prev {
        let vy = lk(r.game, p).vel.y;
        let s = swing(r, vy, fp::frsp(0.6));
        let k = link_mut(r.game, p);
        k.vel.y = mem(fp::fsubs(reg(k.vel.y), s));
        apply_vel(r.game, p);
        let at = lk(r.game, cur).pos;
        let d = direction(lk(r.game, p).pos, at, &mut dir);
        if d > max {
            hold_to(r, p, at, dir, max);
        } else if d < min {
            hold_to(r, p, at, dir, min);
        }
        cur = p;
        prev = lk(r.game, p).prev;
    }
    let drag = a.f(r, attr::DRAG);
    let k = link_mut(r.game, cur);
    k.vel.x = mem(slide(reg(k.vel.x), drag));
}

/// The latched beam pays out, `it_802B9FD4`: the tip rides its line (`it_802A4454`), and the
/// links leave toward the hand's point and fall as in [`lead`]; true once all are out.
#[doc(alias = "it_802B9FD4")]
pub fn pay_out(r: &mut Running, l: ItemLinkId, pos: &mut Vec3, a: Attrs) -> bool {
    ride_line(r, l);
    let mut cur = l;
    let mut next = lk(r.game, cur).next;
    let max = a.f(r, attr::LINK);
    let mut dir = Vec3::default();
    while let Some(n) = next {
        if lk(r.game, n).x2C_b0 {
            let vy = lk(r.game, n).vel.y;
            let s = swing(r, vy, fp::frsp(0.6));
            let k = link_mut(r.game, n);
            k.vel.y = mem(fp::fsubs(reg(k.vel.y), s));
            apply_vel(r.game, n);
            let at = lk(r.game, cur).pos;
            if direction(lk(r.game, n).pos, at, &mut dir) > max {
                hold_to(r, n, at, dir, max);
            }
            move_coll(r.game, n);
        } else {
            let at = lk(r.game, cur).pos;
            if direction(*pos, at, &mut dir) > max {
                hold_to(r, n, at, dir, max);
                link_mut(r.game, n).x2C_b0 = true;
                place_coll(r.game, n);
            } else {
                return false;
            }
        }
        cur = n;
        next = lk(r.game, cur).next;
    }
    true
}

/// A link on a moving line moves with it, `it_802A4454`.
#[doc(alias = "it_802A4454")]
pub fn ride_line(r: &mut Running, l: ItemLinkId) {
    let k = lk(r.game, l);
    let (line, at) = (k.x1CC, k.pos);
    let g = r.game.geometry.as_ref().expect("no collision geometry");
    if !g.line_live(line) {
        return;
    }
    let Some(d) = g.line_speed(line, at) else {
        return;
    };
    let k = link_mut(r.game, l);
    k.pos.x = mem(fp::fadds(reg(k.pos.x), reg(d.x)));
    k.pos.y = mem(fp::fadds(reg(k.pos.y), reg(d.y)));
    k.pos.z = mem(fp::fadds(reg(k.pos.z), reg(d.z)));
}

/// Winds the links in by `speed`, `it_802BA194`: from the first link out, those within
/// `speed` of the hand's point go in; the rest chase it from what's left of a link length;
/// each link's speed eases toward how its collision moved (a double-precision 0.12 share).
/// True once all are in.
#[doc(alias = "it_802BA194")]
pub fn wind_in(r: &mut Running, l: ItemLinkId, pos: &mut Vec3, a: Attrs, speed: f64) -> bool {
    let mut cur = l;
    let mut next = lk(r.game, l).prev;
    let mut dir = Vec3::default();
    let mut d = 0.0;
    while let Some(n) = next {
        if lk(r.game, cur).x2C_b0 {
            d = direction(lk(r.game, cur).pos, *pos, &mut dir);
            break;
        }
        cur = n;
        next = lk(r.game, n).prev;
    }
    if next.is_none() {
        d = direction(lk(r.game, cur).pos, *pos, &mut dir);
    }
    while let Some(n) = next {
        if !(speed > d) {
            break;
        }
        link_mut(r.game, cur).x2C_b0 = false;
        d = direction(lk(r.game, n).pos, *pos, &mut dir);
        cur = n;
        next = lk(r.game, n).prev;
    }
    let mut rest = fp::fsubs(d, speed);
    let max = a.f(r, attr::LINK);
    if rest > max {
        rest = max;
    }
    chase(r, cur, *pos, a, rest);
    let done = next.is_none();
    let mut it = Some(l);
    while let Some(k) = it {
        let c = &mut link_mut(r.game, k);
        let dx = fp::fsubs(reg(c.coll_data.cur_pos.x), reg(c.coll_data.last_pos.x));
        let dz = fp::fsubs(reg(c.coll_data.cur_pos.z), reg(c.coll_data.last_pos.z));
        let ex = fp::fsubs(dx, reg(c.vel.x));
        let ez = fp::fsubs(dz, reg(c.vel.z));
        c.vel.x = mem(fp::frsp(fp::fmadd(0.12, ex, reg(c.vel.x))));
        c.vel.z = mem(fp::frsp(fp::fmadd(0.12, ez, reg(c.vel.z))));
        it = c.prev;
    }
    done
}

/// Winds the links in by `speed` without easing their speeds, `it_802BA2D8`
/// (`Item_RetractChain`, then [`chase`]); true once all are in.
#[doc(alias = "it_802BA2D8")]
pub fn wind_in_chain(r: &mut Running, l: ItemLinkId, pos: &mut Vec3, a: Attrs, speed: f64) -> bool {
    let (next, cur, rest) = retract_chain(r, l, *pos, speed, a.f(r, attr::LINK));
    chase(r, cur, *pos, a, rest);
    next.is_none()
}

/// `Item_RetractChain`: from the first link out, those within `speed` of `pos` go in; what's
/// left of a link length past the last.
fn retract_chain(
    r: &mut Running,
    l: ItemLinkId,
    pos: Vec3,
    speed: f64,
    max: f64,
) -> (Option<ItemLinkId>, ItemLinkId, f64) {
    let mut cur = l;
    let mut next = lk(r.game, l).prev;
    while let Some(n) = next {
        if lk(r.game, cur).x2C_b0 {
            break;
        }
        cur = n;
        next = lk(r.game, n).prev;
    }
    let mut dir = Vec3::default();
    let mut d = direction(lk(r.game, cur).pos, pos, &mut dir);
    while let Some(n) = next {
        if !(speed > d) {
            break;
        }
        link_mut(r.game, cur).x2C_b0 = false;
        d = direction(lk(r.game, n).pos, pos, &mut dir);
        cur = n;
        next = lk(r.game, n).prev;
    }
    let mut rest = fp::fsubs(d, speed);
    if rest > max {
        rest = max;
    }
    (next, cur, rest)
}

/// Climbing the beam, `it_802BA3BC`: the tip rides its line; from the hand's first link out,
/// those within `speed` of the hand's point go in and the rest fall toward the tip (the tip
/// kept where it is); from the tip, the links out hold a link length from each other, and the
/// hand's point within a link length of the last; true once only the tip is left.
#[doc(alias = "it_802BA3BC")]
pub fn climb(
    r: &mut Running,
    hand: ItemLinkId,
    tip: ItemLinkId,
    pos: &mut Vec3,
    a: Attrs,
    speed: f64,
) -> bool {
    ride_line(r, tip);
    let mut count = 0;
    let mut tail = hand;
    let mut next = lk(r.game, tail).prev;
    let saved = lk(r.game, tip).pos;
    while let Some(n) = next {
        if lk(r.game, tail).x2C_b0 {
            break;
        }
        tail = n;
        next = lk(r.game, n).prev;
    }
    let max = a.f(r, attr::LINK);
    chase_falling(r, tail, *pos, a, max);
    link_mut(r.game, tip).pos = saved;
    let mut dir = Vec3::default();
    let mut d = direction(lk(r.game, tail).pos, *pos, &mut dir);
    while let Some(n) = next {
        if !(speed > d) {
            break;
        }
        link_mut(r.game, tail).x2C_b0 = false;
        d = direction(lk(r.game, n).pos, *pos, &mut dir);
        tail = n;
        next = lk(r.game, n).prev;
    }
    let mut rest = fp::fsubs(d, speed);
    let max = a.f(r, attr::LINK);
    if rest > max {
        rest = max;
    }
    chase_falling(r, tail, *pos, a, rest);
    link_mut(r.game, tip).pos = saved;
    let mut head = tip;
    let mut cur = lk(r.game, head).next;
    while let Some(c) = cur {
        if !lk(r.game, c).x2C_b0 {
            break;
        }
        count += 1;
        let at = lk(r.game, head).pos;
        let max = a.f(r, attr::LINK);
        if direction(lk(r.game, c).pos, at, &mut dir) > max {
            hold_to(r, c, at, dir, max);
        }
        head = c;
        cur = lk(r.game, head).next;
    }
    let at = lk(r.game, head).pos;
    let max = a.f(r, attr::LINK);
    if direction(*pos, at, &mut dir) > max {
        pos.x = mem(fp::fmadds(reg(dir.x), max, reg(at.x)));
        pos.y = mem(fp::fmadds(reg(dir.y), max, reg(at.y)));
        pos.z = mem(fp::fmadds(reg(dir.z), max, reg(at.z)));
    }
    count == 0
}

/// Hanging on the beam, `it_802BA5DC`: the tip rides its line; from the hand's first link out
/// the links fall toward the tip (the tip kept where it is); from the tip the links out hold a
/// link length from each other until one is within it; the hand's point within a link length
/// of the last.
#[doc(alias = "it_802BA5DC")]
pub fn hold_on(r: &mut Running, hand: ItemLinkId, tip: ItemLinkId, pos: &mut Vec3, a: Attrs) {
    ride_line(r, tip);
    let mut cur = hand;
    let saved = lk(r.game, tip).pos;
    let mut next = lk(r.game, hand).prev;
    while let Some(n) = next {
        if lk(r.game, cur).x2C_b0 {
            break;
        }
        cur = n;
        next = lk(r.game, n).prev;
    }
    let max = a.f(r, attr::LINK);
    chase_falling(r, cur, *pos, a, max);
    let mut retracted = false;
    link_mut(r.game, tip).pos = saved;
    let mut dir = Vec3::default();
    let mut head = tip;
    let mut l = lk(r.game, head).next;
    while let Some(c) = l {
        if !lk(r.game, c).x2C_b0 {
            break;
        }
        if !retracted {
            let at = lk(r.game, head).pos;
            if direction(lk(r.game, c).pos, at, &mut dir) > max {
                hold_to(r, c, at, dir, max);
            } else {
                retracted = true;
            }
        }
        head = c;
        l = lk(r.game, c).next;
    }
    let at = lk(r.game, head).pos;
    if direction(*pos, at, &mut dir) > max {
        pos.x = mem(fp::fmadds(reg(dir.x), max, reg(at.x)));
        pos.y = mem(fp::fmadds(reg(dir.y), max, reg(at.y)));
        pos.z = mem(fp::fmadds(reg(dir.z), max, reg(at.z)));
    }
}

/// The beam before it attaches, `it_802BA760`: true until her state's attach frame (her
/// attributes' `xA0`, `xB0` or `xC0`); after it, the links out follow toward the hand's point
/// and the next leaves once it's a link length on; all out, they chase the hand.
// Ports `it_802BA760`; aliased once a check reaches it.
pub fn before_attach(r: &mut Running, l: ItemLinkId, pos: &mut Vec3, a: Attrs, f: GObjId) -> bool {
    let fid = r.game.fighter_id(f);
    let frame = reg(r.game.fighters[fid.0].mv_f32(0));
    let off = match r.game.fighters[fid.0].motion_id {
        MS_CATCH => Some(crate::game::ft::samus::attr::CATCH_ATTACH),
        MS_CATCH_DASH => Some(crate::game::ft::samus::attr::CATCH_DASH_ATTACH),
        MS_AIR_CATCH => Some(crate::game::ft::samus::attr::AIR_CATCH_ATTACH),
        _ => None,
    };
    if let Some(off) = off
        && frame < fp::frsp(f64::from(r.samus_ext_attr_i32(fid, off)))
    {
        return true;
    }
    let mut cur = l;
    let mut next = lk(r.game, l).next;
    let max = a.f(r, attr::LINK);
    let mut dir = Vec3::default();
    while let Some(n) = next {
        if lk(r.game, n).x2C_b0 {
            let at = lk(r.game, cur).pos;
            if direction(lk(r.game, n).pos, at, &mut dir) > max {
                hold_to(r, n, at, dir, max);
            }
            move_coll(r.game, n);
        } else {
            let at = lk(r.game, cur).pos;
            if direction(*pos, at, &mut dir) > max {
                hold_to(r, n, at, dir, max);
                link_mut(r.game, n).x2C_b0 = true;
                place_coll(r.game, n);
            } else {
                return false;
            }
        }
        cur = n;
        next = lk(r.game, cur).next;
    }
    chase(r, cur, *pos, a, max);
    false
}

/// Lines the links' models up between their neighbours at her height (`it_802A7168`, the
/// hookshot's unit).
fn line_up(r: &mut Running, gobj: GObjId, pos: Vec3, scale: f32) {
    let h = hand(r, gobj);
    link::line_up_models(r, h, pos, scale);
}

/// Folded, `itSamusGrapple_Logic53_PickedUp`: state 0.
#[doc(alias = "itSamusGrapple_Logic53_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    stop_anim(r, gobj);
}

/// Falling, `it_802BA9B8`: state 3, her hit status cleared.
#[doc(alias = "it_802BA9B8")]
pub fn to_fall(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 3, change::ANIM_UPDATE);
    let f = item_mut(r.game, gobj).xDD4_itemVar.samus_grapple().x8.get();
    r.fighter_clear_hitboxes(f.expect("a grapple beam without its fighter"));
    stop_anim(r, gobj);
}

/// Bouncing back, `it_802BAA08`: state 2, her hit status cleared.
#[doc(alias = "it_802BAA08")]
pub fn to_bounce(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    let f = item_mut(r.game, gobj).xDD4_itemVar.samus_grapple().x8.get();
    r.fighter_clear_hitboxes(f.expect("a grapple beam without its fighter"));
    stop_anim(r, gobj);
}

/// Drawn in, `it_802BAA58`: state 4.
#[doc(alias = "it_802BAA58")]
pub fn to_retract(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, change::ANIM_UPDATE);
    stop_anim(r, gobj);
}

/// Drawn in fast, `it_802BAA94`: state 5, her hit status cleared.
#[doc(alias = "it_802BAA94")]
pub fn to_retract_fast(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 5, change::ANIM_UPDATE);
    let f = item_mut(r.game, gobj).xDD4_itemVar.samus_grapple().x8.get();
    r.fighter_clear_hitboxes(f.expect("a grapple beam without its fighter"));
    stop_anim(r, gobj);
}

/// Shot out at `vel`, `it_802BAAE4`: the tip's speed, state 1.
#[doc(alias = "it_802BAAE4")]
pub fn shoot(r: &mut Running, gobj: GObjId, vel: Vec3) {
    let t = tip(r, gobj);
    link_mut(r.game, t).vel = vel;
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    stop_anim(r, gobj);
}

fn to_latch(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 6, change::ANIM_UPDATE);
    stop_anim(r, gobj);
}

/// Latched, `it_802BAB40`: state 6; pulling, `it_802BAB7C`: state 7.
#[doc(alias = "it_802BAB40")]
#[doc(alias = "it_802BAB7C")]
pub fn to_pull(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 7, change::ANIM_UPDATE);
    stop_anim(r, gobj);
}

/// Hanging, `it_802BABB8`: state 8, her hang's frames from her attributes' `xD0` (the
/// character's own).
#[doc(alias = "it_802BABB8")]
pub fn to_hang(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 8, change::ANIM_UPDATE);
    stop_anim(r, gobj);
    let f = owner_of(r, gobj);
    let fid = r.game.fighter_id(f);
    let frames = r.samus_ext_attr_i32(fid, crate::game::ft::samus::attr::TETHER_HANG);
    r.game.fighters[fid.0].set_mv_f32(
        crate::game::ft::samus::mv::GRAPPLE_HANG,
        fp::frsp(f64::from(frames)),
    );
}

/// Her death while the beam is out, `it_802BAC3C`: it's put away, or with none her hooks
/// clear.
#[doc(alias = "it_802BAC3C")]
pub fn ft_death(r: &mut Running, f: GObjId) {
    if let Some(beam) = r.game.fighter(f).u.gobj_at(0x10) {
        put_away(r, beam);
        return;
    }
    let fp = r.game.fighter_mut(f);
    fp.accessory2_cb = None;
    fp.death1_cb = None;
    fp.accessory3_cb = None;
}

/// Her frame with the beam out, `it_802BAC80`: its frame's work.
#[doc(alias = "it_802BAC80")]
pub fn ft_frame(r: &mut Running, f: GObjId) {
    let Some(beam) = r.game.fighter(f).u.gobj_at(0x10) else {
        return;
    };
    if let Some(cb) = item_mut(r.game, beam)
        .xDD4_itemVar
        .samus_grapple()
        .unk_10
        .get()
    {
        run_event(r, beam, cb.0);
    }
}

/// After her animation, `it_802BACC4`: before the beam attaches, its tip on her hand; after,
/// its links' models lined up.
#[doc(alias = "it_802BACC4")]
pub fn ft_line_up(r: &mut Running, f: GObjId) {
    let Some(beam) = r.game.fighter(f).u.gobj_at(0x10) else {
        return;
    };
    let a = Attrs::of(r, beam);
    let t = tip(r, beam);
    let mut pos = joint_pos(r, t);
    if before_attach(r, t, &mut pos, a, f) {
        tip_on_hand(r, beam);
    } else {
        let scale = r.game.fighter(f).x34_scale.y;
        line_up(r, beam, pos, scale);
    }
}

/// Forgets a GObj the beam points to, `itSamusGrapple_Logic53_EvtUnk` (`it_8026B894`): and its
/// fighter, if that.
// Ports `itSamusGrapple_Logic53_EvtUnk`; aliased once a check reaches it.
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    super::super::lib26b1::forget(r.game, gobj, other);
    let v = item_mut(r.game, gobj).xDD4_itemVar.samus_grapple();
    if v.x8.get() == other {
        v.x8.set(None);
    }
}
