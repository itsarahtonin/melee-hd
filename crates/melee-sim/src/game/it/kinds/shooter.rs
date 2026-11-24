//! What the Poké Ball's shooting Pokémon share (Chikorita, `itchicorita.c`; Cyndaquil,
//! `ithinoarashi.c`; Staryu, `ithitodeman.c`; Blastoise, `itkamex.c`; Charizard,
//! `itlizardon.c`): the way out of the ball, the rounds of an attack, leaving, and their shots'
//! launch and life. Each function carries every original's `#[doc(alias)]` and takes what
//! differs from the kind's descriptor: a [`Release`] for the way out (all five), a [`Rounds`]
//! for the attack (Chikorita, Blastoise and Charizard), a [`Shot`] for a launch (Chikorita's
//! leaf, Blastoise's water, Staryu's star).
//!
//! Each kind's unit keeps its addresses, its `run` tables, its spawn and what only it does:
//! Chikorita's fall, Cyndaquil's flames, Staryu's chase, Blastoise's push back and Charizard's
//! breath.

use gekko_fp as fp;

use super::super::ground::{Event, air_touched_any, fall, ground_or_fall};
use super::super::item::{change, change_state, play_sfx, spawn_air};
use super::super::pokemon::{self, attr_f32, attr_i32, effect_hitlag};
use super::super::{item, item_mut, lib26b1, lib2725, maplib};
use crate::fpu::{mem, reg};
use crate::game::handles::CodeRef;
use crate::game::it::ItKind;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::lb::sound_obj::SoundParams;
use crate::vec::Vec3;

/// The rounds left (`x60` of the kind's variables, past the release's; -1 until the first).
pub const ROUNDS: usize = 0x60;

/// A Pokémon's way out of the ball.
pub struct Release {
    /// Its state falling and growing out of the ball.
    pub out: i32,
    /// Whether its states pause its effects in hitlag (`Item_SetEffectHitlagCallbacks`): all
    /// but Chikorita's.
    pub hitlag: bool,
    /// Whether it's posed at its first frame as it comes out (Cyndaquil, Charizard).
    pub posed: bool,
    /// What it does as it lands out of the ball.
    pub landed: Event,
    /// What it does when its fall out of the ball ends (Staryu stops and follows).
    pub fallen: Option<Event>,
}

/// A Pokémon's attack in rounds: Chikorita's leaves, Blastoise's shots, Charizard's breaths.
pub struct Rounds {
    pub release: Release,
    /// Its state standing between landing and its first round (Blastoise, Charizard; Chikorita
    /// fires as it lands).
    pub stand: Option<i32>,
    /// Its state in a round, and leaving.
    pub fire: i32,
    pub leave: i32,
    /// Its `on_accessory` in a round.
    pub accessory: u32,
    /// The first round's settings from its attributes: the count (`x4`, [`rounds_from_attrs`]),
    /// and Blastoise's push back.
    pub first_round: Event,
    /// Each round's start, after its state (Charizard's smoke and flame timers).
    pub on_round: Option<Event>,
    /// Each frame of a round and of leaving, first (Chikorita's bone).
    pub each_frame: Option<Event>,
    /// Whether it stops (its x speed to 0) at the end of its last round (Blastoise).
    pub stops: bool,
}

/// A Pokémon's shot: Chikorita's leaf, Blastoise's water, Staryu's star.
pub struct Shot {
    pub kind: ItKind,
    /// Its attributes' words: how far ahead of the Pokémon (along its facing) and above it the
    /// shot starts, and its speed its way.
    pub ahead: u32,
    pub up: u32,
    pub speed: u32,
    /// Whether it's spawned with `flags[0]`'s top bit (all but the star, which clears it).
    pub flagged: bool,
    /// Its depth from the Pokémon's facing and depth (Blastoise's cannons).
    pub depth: Option<Depth>,
    pub sound: ShotSound,
}

/// A shot's depth from the Pokémon's facing (in a register) and depth.
pub type Depth = fn(&mut Running, GObjId, f64, f32) -> f32;

/// The sound a shot makes when it's out.
pub enum ShotSound {
    /// The Pokémon's sound.
    Sfx(i32),
    /// A sound object panned its way, by the way it flies.
    Panned { right: i32, left: i32 },
}

/// Into the air out of the ball (`it_802C9A74`, `it_802D4B54`, `it_802CAA40`, `it_802CC0EC`;
/// Cyndaquil's `it_802D5E4C` runs it first): its out state, its effects' hitlag hooks, and
/// posed at its first frame (Charizard's and Cyndaquil's,
/// `Item_EnterAirStateWithHitlagAndStateDesc`).
#[doc(alias = "it_802C9A74")]
#[doc(alias = "it_802D4B54")]
#[doc(alias = "it_802CAA40")]
#[doc(alias = "it_802CC0EC")]
pub fn enter_air(r: &mut Running, gobj: GObjId, s: &Release) {
    maplib::enter_air(item_mut(r.game, gobj));
    change_state(r, gobj, s.out, change::ANIM_UPDATE);
    if s.hitlag {
        effect_hitlag(r.game, gobj);
    }
    if s.posed {
        super::drmariopill::set_pose(r, gobj, 0, 0.0);
    }
}

/// Growing out of the ball (`itChicorita_UnkMotion2_Anim`, `it_802D5EC8`,
/// `itHitodeman_UnkMotion2_Anim`, `itKamex_UnkMotion3_Anim`, `it_802CC160`).
#[doc(alias = "itChicorita_UnkMotion2_Anim")]
#[doc(alias = "it_802D5EC8")]
#[doc(alias = "itHitodeman_UnkMotion2_Anim")]
#[doc(alias = "itKamex_UnkMotion3_Anim")]
#[doc(alias = "it_802CC160")]
pub fn growing(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// Falling out of the ball, and what it does when the fall ends (`itChicorita_UnkMotion2_Phys`,
/// `it_802D5EEC`, `itHitodeman_UnkMotion2_Phys`, `itKamex_UnkMotion3_Phys`, `it_802CC184`).
#[doc(alias = "itChicorita_UnkMotion2_Phys")]
#[doc(alias = "it_802D5EEC")]
#[doc(alias = "itHitodeman_UnkMotion2_Phys")]
#[doc(alias = "itKamex_UnkMotion3_Phys")]
#[doc(alias = "it_802CC184")]
pub fn falling(r: &mut Running, gobj: GObjId, s: &Release) {
    if pokemon::fall(r, gobj)
        && let Some(f) = s.fallen
    {
        f(r, gobj);
    }
}

/// Lands out of the ball (`itChicorita_UnkMotion2_Coll`, `it_802D5F0C`,
/// `itHitodeman_UnkMotion2_Coll`, `itKamex_UnkMotion3_Coll`, `it_802CC1A4`).
#[doc(alias = "itChicorita_UnkMotion2_Coll")]
#[doc(alias = "it_802D5F0C")]
#[doc(alias = "itHitodeman_UnkMotion2_Coll")]
#[doc(alias = "itKamex_UnkMotion3_Coll")]
#[doc(alias = "it_802CC1A4")]
pub fn landing(r: &mut Running, gobj: GObjId, s: &Release) -> bool {
    pokemon::land(r, gobj, s.landed)
}

/// Nothing: landing in their states (`it_802C9618`, `it_802D43AC`, `it_802CA534`,
/// `it_802CB93C`), Staryu's shooting state's `on_accessory` (`it_802D4B50`), the leaf's and the
/// star's physics (`itChicoritaleaf_UnkMotion0_Phys`, `it_802D5044`).
#[doc(alias = "it_802C9618")]
#[doc(alias = "it_802D43AC")]
#[doc(alias = "it_802CA534")]
#[doc(alias = "it_802CB93C")]
#[doc(alias = "it_802D4B50")]
#[doc(alias = "itChicoritaleaf_UnkMotion0_Phys")]
#[doc(alias = "it_802D5044")]
pub fn nothing(_: &mut Running, _: GObjId) {}

/// Answers no: kept (the leaf's `itChicoritaLeaf_Logic30_HitShield`, the water's
/// `itKamex_Logic31_DmgDealt`), or a state with nothing to do (Staryu's `it_802D48A8` and
/// `itHitodeman_UnkMotion1_Coll`, the star's `it_802D5048`, Cyndaquil's launch,
/// `itHinoarashi_UnkMotion2_Anim`).
#[doc(alias = "itChicoritaLeaf_Logic30_HitShield")]
#[doc(alias = "itKamex_Logic31_DmgDealt")]
#[doc(alias = "it_802D48A8")]
#[doc(alias = "itHitodeman_UnkMotion1_Coll")]
#[doc(alias = "it_802D5048")]
#[doc(alias = "itHinoarashi_UnkMotion2_Anim")]
pub fn kept(_: &mut Running, _: GObjId) -> bool {
    false
}

/// Answers yes, gone: the water hitting a shield (`itKamex_Logic31_HitShield`), the star
/// absorbed or hitting one (`itHitodeman_Logic43_Absorbed`, `it_802D4F50`), a flame hitting one
/// or absorbed (`itLizardon_Logic37_HitShield`, `itLizardon_Logic37_Absorbed`).
#[doc(alias = "itKamex_Logic31_HitShield")]
#[doc(alias = "itHitodeman_Logic43_Absorbed")]
#[doc(alias = "it_802D4F50")]
#[doc(alias = "itLizardon_Logic37_HitShield")]
#[doc(alias = "itLizardon_Logic37_Absorbed")]
pub fn gone(_: &mut Running, _: GObjId) -> bool {
    true
}

/// Lands in the air, falls off the floor on it, doing nothing either way
/// (`itChicorita_UnkMotion0_Coll`, `_1_Coll`; `it_802CA654`, `itKamex_UnkMotion1_Coll`,
/// `_2_Coll`; `itLizardon_UnkMotion1_Coll`, `_2_Coll`, `_3_Coll`).
#[doc(alias = "itChicorita_UnkMotion0_Coll")]
#[doc(alias = "itChicorita_UnkMotion1_Coll")]
#[doc(alias = "it_802CA654")]
#[doc(alias = "itKamex_UnkMotion1_Coll")]
#[doc(alias = "itKamex_UnkMotion2_Coll")]
#[doc(alias = "itLizardon_UnkMotion1_Coll")]
#[doc(alias = "itLizardon_UnkMotion2_Coll")]
#[doc(alias = "itLizardon_UnkMotion3_Coll")]
pub fn stays(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).ground_or_air == 1 {
        fall(r, gobj, nothing);
    } else {
        ground_or_fall(r, gobj, nothing);
    }
    false
}

/// Falls by its gravity in the air (`it_802CA618`, `itKamex_UnkMotion2_Phys`,
/// `itLizardon_UnkMotion1_Phys`, `_2_Phys`, `_3_Phys`).
#[doc(alias = "it_802CA618")]
#[doc(alias = "itKamex_UnkMotion2_Phys")]
#[doc(alias = "itLizardon_UnkMotion1_Phys")]
#[doc(alias = "itLizardon_UnkMotion2_Phys")]
#[doc(alias = "itLizardon_UnkMotion3_Phys")]
pub fn gravity_in_air(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).ground_or_air == 1 {
        super::gravity(r, gobj);
    }
}

/// Lands out of the ball, still (`it_802C963C`, `it_802CA558`, `it_802CB960`): standing, or
/// with no stand (Chikorita) firing.
#[doc(alias = "it_802C963C")]
#[doc(alias = "it_802CA558")]
#[doc(alias = "it_802CB960")]
pub fn landed(r: &mut Running, gobj: GObjId, s: &Rounds) {
    lib2725::clear_velocity(r.game, gobj);
    match s.stand {
        Some(_) => stand(r, gobj, s),
        None => fire(r, gobj, s),
    }
}

/// Stands before its first round (Blastoise's `it_802CA58C`, Charizard's `it_802CB994`).
#[doc(alias = "it_802CA58C")]
#[doc(alias = "it_802CB994")]
pub fn stand(r: &mut Running, gobj: GObjId, s: &Rounds) {
    let state = s.stand.expect("a stand for a kind without one");
    change_state(r, gobj, state, change::ANIM_UPDATE);
    if s.release.hitlag {
        effect_hitlag(r.game, gobj);
    }
}

/// Standing: its first round when its animation ends (`it_802CA5D8`,
/// `itLizardon_UnkMotion1_Anim`).
#[doc(alias = "it_802CA5D8")]
#[doc(alias = "itLizardon_UnkMotion1_Anim")]
pub fn standing(r: &mut Running, gobj: GObjId, s: &Rounds) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        fire(r, gobj, s);
    }
    false
}

/// The rounds from its attributes (`x4`), the first time.
pub fn rounds_from_attrs(r: &mut Running, gobj: GObjId) {
    let n = attr_i32(r, gobj, 4);
    pokemon::vars(r.game, gobj).set_i32(ROUNDS, n);
}

/// A round, `it_802C9670`, `it_802CA6A0`, `it_802CBAA8` (and each one's inline copy in its
/// rounds' animation): the first time, its settings from its attributes; its round's state,
/// the round's own start, its effects' hitlag hooks, and its `on_accessory`.
#[doc(alias = "it_802C9670")]
#[doc(alias = "it_802CA6A0")]
#[doc(alias = "it_802CBAA8")]
pub fn fire(r: &mut Running, gobj: GObjId, s: &Rounds) {
    if pokemon::vars(r.game, gobj).i32(ROUNDS) == -1 {
        (s.first_round)(r, gobj);
    }
    change_state(r, gobj, s.fire, change::ANIM_UPDATE);
    if let Some(f) = s.on_round {
        f(r, gobj);
    }
    if s.release.hitlag {
        effect_hitlag(r.game, gobj);
    }
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(s.accessory));
}

/// The rounds (`itChicorita_UnkMotion0_Anim`, `itKamex_UnkMotion1_Anim`,
/// `itLizardon_UnkMotion2_Anim`): at each round's end another, or with none left (stopped, for
/// Blastoise) it leaves.
#[doc(alias = "itChicorita_UnkMotion0_Anim")]
#[doc(alias = "itKamex_UnkMotion1_Anim")]
#[doc(alias = "itLizardon_UnkMotion2_Anim")]
pub fn rounds(r: &mut Running, gobj: GObjId, s: &Rounds) -> bool {
    if let Some(f) = s.each_frame {
        f(r, gobj);
    }
    if !lib2725::is_animating(r.game, gobj) {
        let v = pokemon::vars(r.game, gobj);
        let n = v.i32(ROUNDS);
        if n <= 0 {
            v.set_i32(ROUNDS, 0);
            if s.stops {
                item_mut(r.game, gobj).vel.x = 0.0;
            }
            leave(r, gobj, s);
        } else {
            v.set_i32(ROUNDS, n.wrapping_sub(1));
            fire(r, gobj, s);
        }
    }
    false
}

/// Its `on_accessory` in a round (`it_802C989C`, `it_802CA8DC`): a shot when its script says
/// (`cmd_flags[0]`'s top bit), then `after` (Blastoise's push back), the flag cleared.
#[doc(alias = "it_802C989C")]
#[doc(alias = "it_802CA8DC")]
pub fn fire_when_told(r: &mut Running, gobj: GObjId, shoot: Event, after: Option<Event>) {
    if item(r.game, gobj).cmd_flags[0] & 0x80 != 0 {
        shoot(r, gobj);
        if let Some(f) = after {
            f(r, gobj);
        }
        item_mut(r.game, gobj).cmd_flags[0] &= !0x80;
    }
}

/// Leaves (`it_802C98E4`, `it_802CA938`, `it_802CBFE4`).
#[doc(alias = "it_802C98E4")]
#[doc(alias = "it_802CA938")]
#[doc(alias = "it_802CBFE4")]
pub fn leave(r: &mut Running, gobj: GObjId, s: &Rounds) {
    change_state(r, gobj, s.leave, change::ANIM_UPDATE);
    if s.release.hitlag {
        effect_hitlag(r.game, gobj);
    }
}

/// Leaving: gone when its animation ends (`itChicorita_UnkMotion1_Anim`,
/// `itKamex_UnkMotion2_Anim`, `itLizardon_UnkMotion3_Anim`).
#[doc(alias = "itChicorita_UnkMotion1_Anim")]
#[doc(alias = "itKamex_UnkMotion2_Anim")]
#[doc(alias = "itLizardon_UnkMotion3_Anim")]
pub fn leaving(r: &mut Running, gobj: GObjId, s: &Rounds) -> bool {
    if let Some(f) = s.each_frame {
        f(r, gobj);
    }
    !lib2725::is_animating(r.game, gobj)
}

/// Fires a shot (`it_802C9B20`, `it_802CAB10`, `it_802D4C74`): ahead of and above the Pokémon,
/// flying its way at its speed, owned as the Pokémon is and made by it; its sound when it's
/// out.
#[doc(alias = "it_802C9B20")]
#[doc(alias = "it_802CAB10")]
#[doc(alias = "it_802D4C74")]
pub fn shoot(r: &mut Running, gobj: GObjId, s: &Shot) {
    let (ahead, up, speed) = (
        attr_f32(r, gobj, s.ahead),
        attr_f32(r, gobj, s.up),
        attr_f32(r, gobj, s.speed),
    );
    let ip = item(r.game, gobj);
    let (pos, facing, owner) = (ip.pos, reg(ip.facing_dir), ip.owner);
    let mut prev = pos;
    prev.x = mem(fp::fmadds(reg(ahead), facing, reg(prev.x)));
    prev.y = mem(fp::fadds(reg(prev.y), reg(up)));
    if let Some(depth) = s.depth {
        prev.z = depth(r, gobj, facing, prev.z);
    }
    let mut spawn = SpawnItem {
        kind: s.kind.id(),
        prev_pos: prev,
        pos: lib26b1::ecb_center(r.game, gobj),
        facing_dir: mem(facing),
        damage: 0,
        vel: Vec3::new(mem(fp::fmuls(reg(speed), facing)), 0.0, 0.0),
        owner,
        maker: Some(gobj),
        ignore_id: 0,
        ..Default::default()
    };
    if s.flagged {
        spawn.flags[0] |= 0x80;
    } else {
        spawn.flags[0] &= !0x80;
    }
    if spawn_air(r, &mut spawn).is_none() {
        return;
    }
    match s.sound {
        ShotSound::Sfx(id) => play_sfx(r, gobj, id, 0x7F, 0x40),
        ShotSound::Panned { right, left } => {
            let sfx = if facing == 1.0 { right } else { left };
            r.lb_create_sound_object(&SoundParams {
                owner: gobj,
                behavior: 1,
                sfx_id: sfx,
                start_vol: 0x7F,
                end_vol: 0x7F,
                pan_left: 0,
                pan_right: 0x7F,
                end_frame: 0x3C,
                direction: 3.0,
                track: 0,
                voice: -1,
            });
        }
    }
}

/// A shot out (the leaf's `itChicoritaLeaf_Logic30_Spawned`, the water's
/// `it_2725_Logic31_Spawned`): its life from its attributes (`x0`), not spinning, flying
/// (`fly`), not pickable, then `after` (the leaf's hits not counted, the water at a fifth of
/// its size).
#[doc(alias = "itChicoritaLeaf_Logic30_Spawned")]
#[doc(alias = "it_2725_Logic31_Spawned")]
pub fn shot_out(r: &mut Running, gobj: GObjId, fly: Event, after: Event) {
    let life = attr_f32(r, gobj, 0);
    item_mut(r.game, gobj).life_timer = life;
    lib2725::stop_spin(r.game, r.data, gobj);
    fly(r, gobj);
    lib26b1::disable_pickup(r.game, gobj);
    after(r, gobj);
}

/// A shot's life (the leaf's `itChicoritaleaf_UnkMotion0_Anim`, a flame's `it_802CC650`,
/// `Item_TickLifetime`): gone once it's out, else one less.
#[doc(alias = "itChicoritaleaf_UnkMotion0_Anim")]
#[doc(alias = "it_802CC650")]
pub fn life_ticks(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    if reg(ip.life_timer) <= 0.0 {
        return true;
    }
    ip.life_timer = mem(fp::fsubs(reg(ip.life_timer), 1.0));
    false
}

/// A shot's life spent first (the water's `it_802CAE60`; the star's `it_802D4FFC` runs it):
/// one less, gone once it's out.
#[doc(alias = "it_802CAE60")]
pub fn life_spent(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    ip.life_timer = mem(fp::fsubs(reg(ip.life_timer), 1.0));
    reg(ip.life_timer) <= 0.0
}

/// A shot gone when it touches the stage (the leaf's `itChicoritaleaf_UnkMotion0_Coll`, the
/// water's `it_802CAFB4`, a flame's `it_802CC6C4`).
#[doc(alias = "itChicoritaleaf_UnkMotion0_Coll")]
#[doc(alias = "it_802CAFB4")]
#[doc(alias = "it_802CC6C4")]
pub fn gone_on_touch(r: &mut Running, gobj: GObjId) -> bool {
    air_touched_any(r, gobj)
}

/// A shot reflected back (the leaf's `itChicoritaLeaf_Logic30_Reflected`, a flame's
/// `itLizardon_Logic37_Reflected`, `it_80273030`).
#[doc(alias = "itChicoritaLeaf_Logic30_Reflected")]
#[doc(alias = "itLizardon_Logic37_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj)
}
