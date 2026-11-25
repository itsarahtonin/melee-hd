//! What Mr. Game & Watch's items share: their variables (`itGamewatch_ItemVars`, with
//! `itGamewatchchef_ItemVars`' index and `itGamewatchrescue_ItemVars`' owner after the
//! attributes' pointer), the spawn of the items he holds (`Item_InitSpawnPosition`,
//! `Item_InitSpawnCommonFields` and `Item_AttachGameWatchArticle`), their colors from
//! his (`it_8027CE64`), and the hooks of those that come and go with a move ([`Held`]: the
//! aerials' parachute, turtle and Sparky, Oil Panic's oil, and Kirby's copy's pan).

use melee_data::archive::DataRef;

use super::super::item::{animate, change, change_state, destroy, pick_up, spawn_air};
use super::super::{Var, item, item_mut, lib26b1, set_flag};
use crate::game::ft::FighterKind;
use crate::game::ft::common::GA_GROUND;
use crate::game::it::ItKind;
use crate::game::mirror::{Cx, Mem, Mirror};
use crate::game::state::Running;
use crate::game::structs::{_GXColor, SpawnItem};
use crate::hsd::gobj::GObjId;
use crate::hsd::tobj::Color;
use crate::vec::Vec3;

/// The variables of his items but the trampoline: the attributes' pointer `it_8027CE64`
/// keeps (`gamewatch.attr`) and the sausage's index (`gamewatchchef.x4`).
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
pub struct GameWatchVars {
    /// +0x0
    pub attr: Var<Option<DataRef>>,
    /// +0x4
    #[doc(alias("x4"))]
    pub sausage: Var<i32>,
}

impl Default for GameWatchVars {
    fn default() -> Self {
        GameWatchVars {
            attr: Var::new(None),
            sausage: Var::new(0),
        }
    }
}

impl GameWatchVars {
    /// Marks every field stale, as a new item finds its variables.
    pub fn mark_stale(&mut self) {
        self.attr.mark_stale();
        self.sausage.mark_stale();
    }
}

impl Mirror for GameWatchVars {
    const SIZE: u32 = 0x8;

    fn import(m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
        GameWatchVars {
            attr: Var::new(cx.import_data(m.u32(at))),
            sausage: Var::new(m.u32(at + 4) as i32),
        }
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        let p = |n: &str| format!("{path}.{n}");
        if self.attr.live(&p("attr"), cx) {
            cx.compare_data(&p("attr"), *self.attr.peek(), m.u32(at));
        }
        if self.sausage.live(&p("x4"), cx) && *self.sausage.peek() != m.u32(at + 4) as i32 {
            cx.mismatch(
                &p("x4"),
                format!("{:?}", self.sausage.peek()),
                format!("{:?}", m.u32(at + 4) as i32),
            );
        }
    }
}

/// The trampoline's variables: the attributes' pointer and its fighter
/// (`gamewatchrescue.xDD8`).
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
pub struct GameWatchRescueVars {
    /// +0x0
    pub attr: Var<Option<DataRef>>,
    /// +0x4
    pub parent: Var<Option<GObjId>>,
}

impl Default for GameWatchRescueVars {
    fn default() -> Self {
        GameWatchRescueVars {
            attr: Var::new(None),
            parent: Var::new(None),
        }
    }
}

impl GameWatchRescueVars {
    /// Marks every field stale, as a new item finds its variables.
    pub fn mark_stale(&mut self) {
        self.attr.mark_stale();
        self.parent.mark_stale();
    }
}

impl Mirror for GameWatchRescueVars {
    const SIZE: u32 = 0x8;

    fn import(m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
        GameWatchRescueVars {
            attr: Var::new(cx.import_data(m.u32(at))),
            parent: Var::new(cx.import_handle("GObjId", m.u32(at + 4)).map(GObjId)),
        }
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        let p = |n: &str| format!("{path}.{n}");
        if self.attr.live(&p("attr"), cx) {
            cx.compare_data(&p("attr"), *self.attr.peek(), m.u32(at));
        }
        if self.parent.live(&p("parent"), cx) {
            let ours = self.parent.peek().map(|g| g.0);
            cx.compare_handle(&p("parent"), "GObjId", ours, m.u32(at + 4));
        }
    }
}

/// A spawn's position (`Item_InitSpawnPosition`): `pos`, flattened onto the stage's plane
/// if `on_plane`.
pub(super) fn spawn_position(spawn: &mut SpawnItem, pos: Vec3, on_plane: bool) {
    spawn.prev_pos = pos;
    if on_plane {
        spawn.prev_pos.z = 0.0;
    }
    spawn.pos = spawn.prev_pos;
}

/// A spawn's common fields (`Item_InitSpawnCommonFields`).
pub(super) fn spawn_common(
    spawn: &mut SpawnItem,
    parent: Option<GObjId>,
    facing_dir: f32,
    initial_collision: bool,
) {
    spawn.facing_dir = facing_dir;
    spawn.damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.owner = parent;
    spawn.maker = spawn.owner;
    if initial_collision {
        spawn.flags[0] |= 0x80;
    } else {
        spawn.flags[0] &= !0x80;
    }
    spawn.ignore_id = 0;
}

/// An item he holds at his part `part` (the spawns of the sprayer, the manhole, the torch,
/// the parachute, the turtle, Sparky and the oil): from `pos`, with a first collision, taken
/// in hand and colored as he is (`Item_AttachGameWatchArticle`).
pub(super) fn spawn_held(
    r: &mut Running,
    kind: i32,
    parent: GObjId,
    pos: Vec3,
    part: i32,
    facing_dir: f32,
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    spawn_position(&mut spawn, pos, false);
    spawn_common(&mut spawn, Some(parent), facing_dir, true);
    let gobj = spawn_air(r, &mut spawn)?;
    let attr = first_attr(r, gobj);
    attach(r, gobj, parent, part);
    colors(r, gobj, parent, attr);
    Some(gobj)
}

/// The first word of the item's special attributes (`*xC4_article_data->x4_specialAttributes`).
pub(super) fn first_attr(r: &Running, gobj: GObjId) -> Option<DataRef> {
    let attrs = super::super::item::article(item(r.game, gobj))
        .special(r.data)
        .expect("a Game & Watch item without attributes");
    r.data.ptr(attrs)
}

/// Taken in hand (`Item_AttachToParent`: `Item_8026AB54`, `db_80225DD8`).
pub(super) fn attach(r: &mut Running, gobj: GObjId, parent: GObjId, part: i32) {
    pick_up(r, gobj, parent, part);
    super::super::item::debug_owner_bubbles(r.game, gobj, Some(parent));
}

fn color(c: _GXColor) -> Color {
    Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}

/// His items' colors from his, `it_8027CE64`: the model's diffuse color, the outline, the
/// owner's team tint, the size by the owner's scale, and the attributes' pointer kept.
#[doc(alias = "it_8027CE64")]
pub(super) fn colors(r: &mut Running, gobj: GObjId, fighter: GObjId, attr: Option<DataRef>) {
    set_flag(&mut item_mut(r.game, gobj).draw_flags, 3, true);
    let c = game_watch_color(r, fighter);
    let root = super::super::jobj_of(r.game, gobj);
    crate::game::ft::gamewatch::init::set_diffuse_all(&mut r.game.objs, root, color(c));
    let outline = game_watch_outline(r, fighter);
    item_mut(r.game, gobj).outline.color = outline;
    let owner = item(r.game, gobj)
        .owner
        .expect("ftLib_GetSubColor: no owner");
    let sub = sub_color(r, owner);
    let ip = item_mut(r.game, gobj);
    ip.tinted = u8::from(sub.is_some());
    if let Some(s) = sub {
        ip.tint = s;
    }
    super::super::lib2725::scale_by_owner(r, gobj);
    if item(r.game, gobj).kind == ItKind::GameAndWatchTrampoline {
        item_mut(r.game, gobj)
            .vars
            .game_watch_rescue()
            .attr
            .set(attr);
    } else {
        vars(r, gobj).attr.set(attr);
    }
}

/// `ftLib_GetGameWatchColor`: his costume's color, or Kirby's copy's
/// (`ftKb_Init_800EEB00`).
#[doc(alias = "ftLib_GetGameWatchColor")]
fn game_watch_color(r: &Running, fighter: GObjId) -> _GXColor {
    if r.game.ft_kind(fighter) == FighterKind::GameWatch {
        r.gw_color(fighter)
    } else {
        r.kb_gw_color(4)
    }
}

/// `ftLib_GetGameWatchOutlineColor`: his, or Kirby's copy's (`ftKb_Init_800EEB1C`).
#[doc(alias = "ftLib_GetGameWatchOutlineColor")]
fn game_watch_outline(r: &Running, fighter: GObjId) -> _GXColor {
    if r.game.ft_kind(fighter) == FighterKind::GameWatch {
        r.gw_outline_color(fighter)
    } else {
        r.kb_gw_color(8)
    }
}

/// The fighter's team tint, if it has one, `ftLib_GetSubColor`.
#[doc(alias = "ftLib_GetSubColor")]
fn sub_color(r: &Running, fighter: GObjId) -> Option<_GXColor> {
    let sub = r.game.fighter(fighter).sub_color;
    if sub == 0 {
        return None;
    }
    let at = r.common().at(0x6DC + 4 * u32::from(sub - 1));
    Some(_GXColor {
        r: r.data.u8(at),
        g: r.data.u8(at.at(1)),
        b: r.data.u8(at.at(2)),
        a: r.data.u8(at.at(3)),
    })
}

/// The item's variables as his items' (all but the trampoline).
pub(super) fn vars<'r>(r: &'r mut Running<'_>, gobj: GObjId) -> &'r mut GameWatchVars {
    item_mut(r.game, gobj).vars.game_watch()
}

/// The item pauses in hitlag (`it_8026B724`).
pub(super) fn hitlag_start(r: &mut Running, gobj: GObjId) {
    super::seakchain::hitlag_start(r, gobj);
}

/// The item runs again (`it_8026B73C`).
pub(super) fn hitlag_end(r: &mut Running, gobj: GObjId) {
    super::seakchain::hitlag_end(r, gobj);
}

/// The item's outline lists (`it_266F_ItemVars` at the attributes' pointer its variables
/// keep): the bones the outline passes hide (a count and their indices), then those they show.
pub fn outline_lists(
    r: &mut Running,
    gobj: GObjId,
) -> ((u16, Option<DataRef>), (u16, Option<DataRef>)) {
    let attr = if item(r.game, gobj).kind == ItKind::GameAndWatchTrampoline {
        item_mut(r.game, gobj).vars.game_watch_rescue().attr.get()
    } else {
        vars(r, gobj).attr.get()
    };
    let a = attr.expect("an outlined item without its lists");
    let d = r.data;
    (
        (d.u16(a), d.ptr(a.at(4))),
        (d.u16(a.at(8)), d.ptr(a.at(0xC))),
    )
}

/// What his items that come and go with a move (the aerials' parachute, turtle and Sparky,
/// `itgamewatchparachute.c`, `itgamewatchturtle.c`, `itgamewatchbreath.c`; Oil Panic's oil,
/// `itgamewatchpanic.c`) and Kirby's copy's pan (`itkirbygamewatchchefpan.c`) differ by. Each
/// shared function below carries every original's `#[doc(alias)]`; each kind's unit keeps its
/// addresses, its `run` tables and its settings.
pub struct Held {
    pub kind: ItKind,
    /// Its owner forgets it (the move's `SetFlag`; Kirby's `ftKb_SpecialNGw_8010D074`).
    pub forget: fn(&mut Running, GObjId),
    /// Whether its owner has left the move (the move's `ItemCheck*Remove`; Kirby's
    /// `ftKb_SpecialNGw_8010D160`).
    pub owner_done: fn(&Running, GObjId) -> bool,
    /// The script variables its pickup clears first.
    pub clears: Clears,
    /// Whether it's taken into its owner's ground or air, in state 0 or 1 by it (the oil);
    /// else into state 0.
    pub owner_air: bool,
    /// Whether its landing animation hides it (the parachute).
    pub hides_on_land: bool,
    /// The animation frame it's gone on (the parachute's 30th).
    pub gone_at: Option<f32>,
}

/// The script variables a held item's pickup clears.
pub enum Clears {
    /// None (the pan).
    None,
    /// `x40` (the aerials' items).
    First,
    /// `x44`, then `x40` (the oil).
    Both,
}

/// The held item at `parent`'s part `part` (`it_802C6C38`, `it_802C6F40`, `it_802C720C`,
/// `it_802C7D60`, `it_802C74D8`).
#[doc(alias = "it_802C6C38")]
#[doc(alias = "it_802C6F40")]
#[doc(alias = "it_802C720C")]
#[doc(alias = "it_802C7D60")]
#[doc(alias = "it_802C74D8")]
pub fn held_spawn(
    r: &mut Running,
    s: &Held,
    parent: GObjId,
    pos: Vec3,
    part: i32,
    facing_dir: f32,
) -> Option<GObjId> {
    spawn_held(r, s.kind.id(), parent, pos, part, facing_dir)
}

/// Gone: its owner forgets it (`itGameWatchParachute_Logic74_Destroyed`,
/// `itGameWatchTurtle_Logic75_Destroyed`, `itGameWatchBreath_Logic76_Destroyed`,
/// `itGameWatchPanic_Logic78_Destroyed`, `itKirbyGameWatchChefPan_Logic113_Destroyed`).
#[doc(alias = "itGameWatchParachute_Logic74_Destroyed")]
#[doc(alias = "itGameWatchTurtle_Logic75_Destroyed")]
#[doc(alias = "itGameWatchBreath_Logic76_Destroyed")]
#[doc(alias = "itGameWatchPanic_Logic78_Destroyed")]
#[doc(alias = "itKirbyGameWatchChefPan_Logic113_Destroyed")]
pub fn held_destroyed(r: &mut Running, gobj: GObjId, s: &Held) {
    if let Some(owner) = item(r.game, gobj).owner {
        (s.forget)(r, owner);
    }
}

/// Taken away (`it_802C6D6C`, `it_802C7074`, `it_802C7340`, `it_802C7E94`, `it_802C760C`).
#[doc(alias = "it_802C6D6C")]
#[doc(alias = "it_802C7074")]
#[doc(alias = "it_802C7340")]
#[doc(alias = "it_802C7E94")]
#[doc(alias = "it_802C760C")]
pub fn held_remove(r: &mut Running, gobj: GObjId, s: &Held) {
    held_destroyed(r, gobj, s);
    destroy(r, gobj);
}

/// Paused (`it_802C6DB8`, `it_802C70C0`, `it_802C738C`, `it_802C7EE0`, `it_802C7658`).
#[doc(alias = "it_802C6DB8")]
#[doc(alias = "it_802C70C0")]
#[doc(alias = "it_802C738C")]
#[doc(alias = "it_802C7EE0")]
#[doc(alias = "it_802C7658")]
pub fn held_hitlag_start(r: &mut Running, gobj: GObjId) {
    hitlag_start(r, gobj);
}

/// Running again (`it_802C6DD8`, `it_802C70E0`, `it_802C73AC`, `it_802C7F00`, `it_802C7678`).
#[doc(alias = "it_802C6DD8")]
#[doc(alias = "it_802C70E0")]
#[doc(alias = "it_802C73AC")]
#[doc(alias = "it_802C7F00")]
#[doc(alias = "it_802C7678")]
pub fn held_hitlag_end(r: &mut Running, gobj: GObjId) {
    hitlag_end(r, gobj);
}

/// Taken in hand, animating: its script variables cleared, then with an owner its first
/// state, or the oil's ground or air state as its owner is
/// (`itGameWatchParachute_Logic74_PickedUp`, `itGameWatchTurtle_Logic75_PickedUp`,
/// `itGameWatchBreath_Logic76_PickedUp`, `itGameWatchPanic_Logic78_PickedUp`,
/// `itKirbyGameWatchChefPan_Logic113_PickedUp`).
#[doc(alias = "itGameWatchParachute_Logic74_PickedUp")]
#[doc(alias = "itGameWatchTurtle_Logic75_PickedUp")]
#[doc(alias = "itGameWatchBreath_Logic76_PickedUp")]
#[doc(alias = "itGameWatchPanic_Logic78_PickedUp")]
#[doc(alias = "itKirbyGameWatchChefPan_Logic113_PickedUp")]
pub fn held_picked_up(r: &mut Running, gobj: GObjId, s: &Held) {
    let ip = item_mut(r.game, gobj);
    match s.clears {
        Clears::None => {}
        Clears::First => ip.cmd_var0 = 0,
        Clears::Both => {
            ip.cmd_var1 = 0;
            ip.cmd_var0 = 0;
        }
    }
    let Some(owner) = ip.owner else { return };
    let state = if s.owner_air {
        let ga = r.game.ft_ground_air(owner);
        item_mut(r.game, gobj).ground_or_air = ga;
        if ga == GA_GROUND { 0 } else { 1 }
    } else {
        0
    };
    change_state(r, gobj, state, change::ANIM_UPDATE);
    animate(r, gobj);
}

/// Its landing animation, the parachute hidden first (`it_802C6E50`, `it_802C7158`,
/// `it_802C7424`).
#[doc(alias = "it_802C6E50")]
#[doc(alias = "it_802C7158")]
#[doc(alias = "it_802C7424")]
pub fn held_land(r: &mut Running, gobj: GObjId, s: &Held) {
    if s.hides_on_land {
        lib26b1::hide(r.game, gobj);
    }
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// Gone on the parachute's 30th frame, or once its owner has left the move (or it has none)
/// (`itGamewatchparachute_UnkMotion1_Anim`, `itGamewatchturtle_UnkMotion1_Anim`,
/// `itGamewatchbreath_UnkMotion1_Anim`, `itGamewatchpanic_UnkMotion1_Anim`,
/// `itKirbygamewatchchefpan_UnkMotion0_Anim`).
#[doc(alias = "itGamewatchparachute_UnkMotion1_Anim")]
#[doc(alias = "itGamewatchturtle_UnkMotion1_Anim")]
#[doc(alias = "itGamewatchbreath_UnkMotion1_Anim")]
#[doc(alias = "itGamewatchpanic_UnkMotion1_Anim")]
#[doc(alias = "itKirbygamewatchchefpan_UnkMotion0_Anim")]
pub fn held_anim(r: &mut Running, gobj: GObjId, s: &Held) -> bool {
    if let Some(frame) = s.gone_at
        && item(r.game, gobj).anim_frame == frame
    {
        held_destroyed(r, gobj, s);
        return true;
    }
    let go = match item(r.game, gobj).owner {
        Some(owner) => (s.owner_done)(r, owner),
        None => true,
    };
    if go {
        held_destroyed(r, gobj, s);
    }
    go
}
