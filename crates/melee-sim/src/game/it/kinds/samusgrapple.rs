//! Samus's grapple beam, `melee/it/kinds/itsamusgrapple.c`: a [tether](super::tether) from her
//! arm cannon (her `ThrowN` bone), the item's own model at the tip, every link's model
//! animating. The states, the spawn, the hooks and the transitions are the tether's, shared
//! with Link's hookshot; this file keeps the beam's addresses, its [`GRAPPLE`], its links and
//! their physics: the tip leads and the links follow one by one, each swinging at random and
//! held between the two link lengths from the one before. The button sequence (D-pad up, down,
//! up, then A, [`sequence`]) gives a ground beam that swings a hitbox and homes on the nearest
//! foe with Z, twice as long, or an air beam whose hang Z lengthens.
//!
//! The beam's variables (`itSamusGrapple_ItemVars`) keep the tether's (its tip, `x0`, the last
//! link made; the link at the hand, `x4`; its fighter, `x8`; the tip's model, `xC`; the frame's
//! work, `unk_10`; `x14`), the links made (`x15`) and the sequence's beam (`x16`); the links
//! point toward the tip through `prev` and toward the hand through `next`.
//!
//! The beam's spawn writes its lengths for her height into its attributes in the character's
//! loaded data (`x34` to `x58`); they're state ([`Globals::grapple_lengths`]) until the file
//! is read from the disc again ([`Globals::file_read`]).

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use crate::mp::coll::GroundOrAir;
use gekko_fp as fp;

use super::super::item::article;
use super::super::link::{self, apply_vel, direction, link as lk, link_mut, move_coll, place_coll};
use super::super::{item, item_mut, jobj_of};
use super::tether::{self, FighterAttr, Hand, Lead, Tether};
use crate::fpu::{mem, reg};
use crate::game::code::unported_call;
use crate::game::handles::{CodeRef, ItemLinkId};
use crate::game::state::{Obj, Running, UserData};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
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
#[doc(alias("EVT_UNK"))]
pub const FORGET: u32 = 0x802B_AEA0;

/// `itSamusGrappleAttributes` offsets.
mod attr {
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
    pub const N: u32 = 0x34;
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

impl Lengths {
    /// The file's own words where `attrs` is: what a fresh read of it holds until a spawn
    /// writes over them.
    pub fn from_file(data: &melee_data::archive::Archives, attrs: DataRef) -> Lengths {
        Lengths {
            attrs: Some(attrs),
            n: data.u32(attrs.at(attr::N)) as i32,
            words: core::array::from_fn(|i| data.f32(attrs.at(attr::LINK + 4 * i as u32))),
        }
    }
}

/// A tether's lengths written over its attributes from `at` (the count, then the words) into
/// the heap's image where it holds the file (docs/engine.md, the loaded files' in-place edits).
pub(super) fn image_lengths(at: DataRef, n: i32, words: &[f32]) {
    if !crate::hsd::heap::image_on() {
        return;
    }
    let mut bytes = (n as u32).to_be_bytes().to_vec();
    for w in words {
        bytes.extend(w.to_bits().to_be_bytes());
    }
    crate::hsd::heap::with(|h| h.image.file_bytes(at, &bytes));
}

/// `ftCo_MS_Catch`, `ftCo_MS_CatchDash` and `ftSs_MS_SpecialAirLw`'s slot `0x165`, the air
/// grab, and its latch: the states the beam lives through.
const MS_CATCH: i32 = 0xD4;
const MS_CATCH_DASH: i32 = 0xD6;
const MS_AIR_CATCH: i32 = 0x165;
const MS_AIR_CATCH_HIT: i32 = 0x166;
/// The links' process link.
const LINK_P_LINK: u8 = 0xA;
/// `FtPart_ThrowN`, where the beam comes from (the bone of that index).
const PART_THROW_N: i32 = 51;
/// `HSD_PAD_A`, the button sequence's (`x2240`) inputs: `HSD_PAD_DPADUP`, `_DPADDOWN`.
const PAD_A: u32 = 0x100;
const PAD_Z: u32 = 0x40;

/// The grapple beam's tether.
pub static GRAPPLE: Tether = Tether {
    vars: |v| {
        let v = v.samus_grapple();
        tether::Vars {
            tip: &mut v.tip,
            hand: &mut v.hand,
            fighter: &mut v.fighter,
            tip_model: &mut v.tip_model,
            frame: &mut v.frame,
            done: &mut v.done,
        }
    },
    slot: crate::game::ft::samus::u::GRAPPLE,
    hooks: [FT_FRAME, FT_DEATH, FT_LINE_UP],
    states: [MS_CATCH, MS_CATCH_DASH, MS_AIR_CATCH, MS_AIR_CATCH_HIT],
    hand: Hand::Bone(PART_THROW_N),
    ahead: None,
    attr: |r, gobj, off| mem(Attrs::of(r, gobj).f(r, off)),
    shot_speed: attr::SHOT_SPEED,
    retract: attr::RETRACT,
    retract_caught: attr::RETRACT_FAST,
    hang_frames: FighterAttr::Character(crate::game::ft::samus::attr::TETHER_HANG),
    landing_lag: crate::game::ft::samus::attr::TETHER_LANDING_LAG,
    make_links,
    lead: |r, gobj, f| {
        let t = tether::tip(r, gobj, &GRAPPLE);
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
        let mut pos = tether::point(r, &GRAPPLE, t);
        let a = Attrs::of(r, gobj);
        let lead = match lead(r, t, &mut pos, a, f, slot) {
            1 => Lead::Wall,
            2 => Lead::Surface,
            3 => Lead::AllOut,
            _ => Lead::Going,
        };
        (lead, pos)
    },
    bounce: |r, gobj, f| {
        let t = tether::tip(r, gobj, &GRAPPLE);
        let slot = rebuilt_slot(r, t, "fn_802B8384 with the links' joint not rebuilt");
        let mut pos = tether::point(r, &GRAPPLE, t);
        let a = Attrs::of(r, gobj);
        let lead = match fall_back(r, t, &mut pos, a, f, slot) {
            1 => Lead::Wall,
            2 => Lead::Surface,
            3 => Lead::AllOut,
            _ => Lead::Going,
        };
        (lead, pos)
    },
    fall: |r, gobj, f| {
        let h = tether::hand(r, gobj, &GRAPPLE);
        let slot = rebuilt_slot(r, h, "fn_802B8524 with the links' joint not rebuilt");
        let mut pos = tether::point(r, &GRAPPLE, h);
        let a = Attrs::of(r, gobj);
        hang_links(r, h, &mut pos, a, f, slot);
        pos
    },
    reel: |r, gobj, h, pos, speed| wind_in(r, h, pos, Attrs::of(r, gobj), speed),
    reel_caught: |r, gobj, h, pos, speed| wind_in_chain(r, h, pos, Attrs::of(r, gobj), speed),
    pay_out: |r, gobj, t, pos| pay_out(r, t, pos, Attrs::of(r, gobj)),
    climb: |r, gobj, h, t, pos, speed| climb(r, h, t, pos, Attrs::of(r, gobj), speed),
    hold_on: |r, gobj, h, t, pos| hold_on(r, h, t, pos, Attrs::of(r, gobj)),
    follow: |r, gobj, t, pos, f| before_attach(r, t, pos, Attrs::of(r, gobj), f),
    line_up_held: None,
    sequence: Some(sequence),
    animate: Some(animate),
    bounces_off_surfaces: true,
    air_retract_doubles: true,
    lines_up_on_hang: false,
    on_spawn: Some(on_spawn),
    after_links: Some(after_links),
    stale_ignore_id: true,
    on_hang: Some(on_hang),
};

/// Runs one of the beam's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    let t = &GRAPPLE;
    match addr {
        SPAWNED => tether::set_frame(r, gobj, t, None),
        PUT_AWAY => tether::put_away(r, gobj, t),
        PHYS_0 => tether::set_frame(r, gobj, t, Some(FOLDED)),
        PHYS_1 => tether::set_frame(r, gobj, t, Some(SHOT)),
        PHYS_2 => tether::set_frame(r, gobj, t, Some(BOUNCED)),
        PHYS_3 => tether::set_frame(r, gobj, t, Some(FALLING)),
        PHYS_4 => tether::set_frame(r, gobj, t, Some(RETRACT)),
        PHYS_5 => tether::set_frame(r, gobj, t, Some(RETRACT_FAST)),
        PHYS_6 => tether::set_frame(r, gobj, t, Some(LATCHED)),
        PHYS_7 => tether::set_frame(r, gobj, t, Some(PULL)),
        PHYS_8 => tether::set_frame(r, gobj, t, Some(HANG)),
        FOLDED => tether::folded(r, gobj, t),
        SHOT => tether::shot(r, gobj, t),
        BOUNCED => tether::bounced(r, gobj, t),
        FALLING => tether::falling(r, gobj, t),
        RETRACT => tether::retract(r, gobj, t),
        RETRACT_FAST => tether::retract_caught(r, gobj, t),
        LATCHED => tether::latched(r, gobj, t),
        PULL => tether::pull(r, gobj, t),
        HANG => tether::hang(r, gobj, t),
        PICKED_UP => tether::picked_up(r, gobj),
        TO_FALL => tether::to_fall(r, gobj, t),
        TO_BOUNCE => tether::to_bounce(r, gobj, t),
        TO_RETRACT => tether::to_retract(r, gobj),
        TO_RETRACT_FAST => tether::to_retract_caught(r, gobj, t),
        TO_LATCH => tether::to_latch(r, gobj),
        TO_PULL => tether::to_pull(r, gobj),
        TO_HANG => tether::to_hang(r, gobj, t),
        _ => return false,
    }
    true
}

/// The spawn's button sequence (`x16`): through it (`x2240` 4) a ground beam on the ground
/// (1), else an air beam (2); before it, none, and the sequence starts again.
fn on_spawn(r: &mut Running, beam: GObjId, owner: GObjId) {
    let fp = r.game.fighter(owner);
    let hanging = if fp.u[0x14] >= 4 {
        if fp.ground_or_air == GroundOrAir::Ground {
            1
        } else {
            2
        }
    } else {
        0
    };
    if hanging == 0 {
        r.game.fighter_mut(owner).u[0x14] = 0;
    }
    item_mut(r.game, beam)
        .vars
        .samus_grapple()
        .hanging
        .set(hanging);
}

/// The beam's own animations, after its links.
fn after_links(r: &mut Running, beam: GObjId) {
    let a = Attrs::of(r, beam);
    let [an, mat, shape] = attr::BEAM_ANIMS;
    let (shape, mat, an) = (a.anim(r, shape), a.anim(r, mat), a.anim(r, an));
    let root = jobj_of(r.game, beam).expect("a grapple beam without a model");
    let child = r.game.objs.jobjs[root].child;
    jobj::add_anim_all(&mut r.game.objs, r.data, child, an, mat, shape);
    jobj::req_anim_all(&mut r.game.objs, child, 0.0);
}

/// Hanging: the sequence's beam (`x16`) with Z held lengthens her hang a frame.
fn on_hang(r: &mut Running, gobj: GObjId, f: GObjId) {
    let hanging = item_mut(r.game, gobj).vars.samus_grapple().hanging.get();
    let fp = r.game.fighter_mut(f);
    if hanging != 0 && fp.input.held_buttons[0] & PAD_Z != 0 {
        let v = fp::fadds(reg(fp.mv_f32(tether::MV_HANG)), 1.0);
        fp.set_mv_f32(tether::MV_HANG, v);
    }
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
    if item_mut(r.game, gobj).vars.samus_grapple().hanging.get() == 1 {
        n = n.wrapping_mul(2);
    }
    r.game.it.grapple_lengths = Lengths {
        attrs: Some(a.0),
        n,
        words: w,
    };
    image_lengths(a.0.at(attr::N), n, &w);
    item_mut(r.game, gobj)
        .vars
        .samus_grapple()
        .links_made
        .set(n as u8);
    let mut prev_link: Option<ItemLinkId> = None;
    let (mut head, mut tail, mut result) = (None, None, None);
    for i in 0..n {
        let g = r.game.world.create(link::CLASS, LINK_P_LINK, 0);
        let l = link::alloc(r.game, r.data);
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
        k.out = false;
        k.touching = false;
        k.sounded = false;
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
        link_mut(r.game, l).wall_line = -1;
        prev_link = Some(l);
    }
    let v = item_mut(r.game, gobj).vars.samus_grapple();
    v.tip.set(tail);
    v.hand.set(head);
    result
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
    let mut cur = Some(tether::hand(r, gobj, &GRAPPLE));
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
    /// `it_802B99A0`'s and `it_802B9CE8`'s, when their callers (`fn_802B8384`,
    /// `fn_802B8524`) have just rebuilt the links' joint's matrix: the slot is the top word of
    /// `HSD_JObjMakeMatrix`'s frame under `HSD_JObjSetupMatrixSub`'s, its save of `r31`, which
    /// holds the joint's address. The joint is her `ThrowN` bone, so the bit is that address's
    /// bit 19.
    SavedR31(u32),
    /// [`SwingSlot::SavedR31`]'s frame without the allocator model (the Slippi ruleset's live
    /// matches and the player's playback of their recordings, design call 8 of
    /// `docs/slippi-ruleset.md`): the joint's address isn't known, and its bit 19 depends on
    /// where playback's heap put the joint (which follows from the match's files and the
    /// objects made before it), so no fixed address in the heap's range stands in for it as
    /// the Super Scope's does. Reading it stops by name.
    Unmodeled(&'static str),
    /// Not followed, named by the frame: reading it stops.
    Unknown(&'static str),
}

/// The slot `fn_802B8384` and `fn_802B8524` leave for their swing, from the links' joint `l`
/// holds as `samus_grapple_setup_pos` is about to rebuild its matrix: a dirty joint with no
/// IK role and no constraints runs `HSD_JObjSetupMatrixSub`, whose last call at that depth
/// is the matrix's (`HSD_JObjMakeMatrix`).
fn rebuilt_slot(r: &Running, l: ItemLinkId, frame: &'static str) -> SwingSlot {
    match lk(r.game, l).jobj {
        Some(j)
            if r.game.objs.jobjs[j].mtx_is_dirty()
                && r.game.objs.jobjs[j].flags & jobj::JOINT_MASK == 0
                && r.game.objs.jobjs[j].robj.is_none() =>
        {
            match r.game.objs.jobjs.pool.addr(j.0) {
                0 => SwingSlot::Unmodeled(frame),
                a => SwingSlot::SavedR31(a),
            }
        }
        _ => SwingSlot::Unknown(frame),
    }
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
    if fp.ground_or_air != GroundOrAir::Ground
        || item_mut(r.game, beam).vars.samus_grapple().hanging.get() != 1
    {
        return;
    }
    r.fighter_clear_hitboxes(f);
    if r.game.fighter(f).input.pressed_buttons & PAD_A != 0 {
        let sixth = match slot {
            // 0.0's high word: no bit set.
            SwingSlot::SavedF14 => 0,
            // The address's bytes: `x134_4` is its bit 19.
            SwingSlot::SavedR31(addr) => addr,
            SwingSlot::Unknown(frame) => unported_call(&format!(
                "it_802B7160: the swing's grabbed-victim-only bit, a stack byte {frame} leaves"
            )),
            SwingSlot::Unmodeled(frame) => unported_call(&format!(
                "it_802B7160 from {}: the swing's grabbed-victim-only bit, bit 19 of the ThrowN joint's address, without the allocator model,",
                frame.split(" with ").next().unwrap_or(frame)
            )),
        };
        let [a, b, c, d, e] = SWING_HITBOX;
        let fid = r.game.fighter_id(f);
        swing_hitbox(r, f, fid, [a, b, c, d, e, sixth]);
        let v = item_mut(r.game, beam).vars.samus_grapple();
        let n = v.hanging.get();
        v.hanging.set(n + 1);
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
        k.out = true;
    }
    ground_swing(r, f, slot);
    let next = lk(r.game, l).next;
    let beam = r.game.fighter(f).u.gobj_at(0x10).expect("the beam");
    if r.game.fighter(f).input.held_buttons[0] & PAD_Z != 0
        && item_mut(r.game, beam).vars.samus_grapple().hanging.get() == 1
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
            link_mut(r.game, l).wall_line = w;
            1
        } else if result & 0xFC0 != 0 {
            let w = lk(r.game, l).coll_data.right_facing_wall.index;
            link_mut(r.game, l).wall_line = w;
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
        if lk(r.game, n).out {
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
                link_mut(r.game, n).out = true;
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
pub fn fall_back(
    r: &mut Running,
    l: ItemLinkId,
    pos: &mut Vec3,
    a: Attrs,
    f: GObjId,
    slot: SwingSlot,
) -> i32 {
    ground_swing(r, f, slot);
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
        if lk(r.game, n).out {
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
                link_mut(r.game, n).out = true;
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
pub fn hang_links(
    r: &mut Running,
    l: ItemLinkId,
    pos: &mut Vec3,
    a: Attrs,
    f: GObjId,
    slot: SwingSlot,
) {
    ground_swing(r, f, slot);
    let mut cur = l;
    let mut prev = lk(r.game, cur).prev;
    while let Some(p) = prev {
        if lk(r.game, cur).out {
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
        if lk(r.game, n).out {
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
                link_mut(r.game, n).out = true;
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
    let (line, at) = (k.wall_line, k.pos);
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
        if lk(r.game, cur).out {
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
        link_mut(r.game, cur).out = false;
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
    let (next, cur, rest) = link::retract_chain(r, l, *pos, speed, a.f(r, attr::LINK));
    chase(r, cur, *pos, a, rest);
    next.is_none()
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
        if lk(r.game, tail).out {
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
        link_mut(r.game, tail).out = false;
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
        if !lk(r.game, c).out {
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
        if lk(r.game, cur).out {
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
        if !lk(r.game, c).out {
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
#[doc(alias = "it_802BA760")]
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
        if lk(r.game, n).out {
            let at = lk(r.game, cur).pos;
            if direction(lk(r.game, n).pos, at, &mut dir) > max {
                hold_to(r, n, at, dir, max);
            }
            move_coll(r.game, n);
        } else {
            let at = lk(r.game, cur).pos;
            if direction(*pos, at, &mut dir) > max {
                hold_to(r, n, at, dir, max);
                link_mut(r.game, n).out = true;
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
