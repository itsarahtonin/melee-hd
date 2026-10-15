//! What Mr. Game & Watch's items share: their variables (`itGamewatch_ItemVars`, with
//! `itGamewatchchef_ItemVars`' index and `itGamewatchrescue_ItemVars`' owner after the
//! attributes' pointer), the spawn of the items he holds (`Item_InitSpawnPosition`,
//! `Item_InitSpawnCommonFields` and `Item_AttachGameWatchArticle`), and their colors from
//! his (`it_8027CE64`).

use melee_data::archive::DataRef;

use super::super::item::{pick_up, spawn_air};
use super::super::{Var, item, item_mut, set_flag};
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
    pub x4: Var<i32>,
}

impl Default for GameWatchVars {
    fn default() -> Self {
        GameWatchVars {
            attr: Var::new(None),
            x4: Var::new(0),
        }
    }
}

impl GameWatchVars {
    /// Marks every field stale, as a new item finds its variables.
    pub fn mark_stale(&mut self) {
        self.attr.mark_stale();
        self.x4.mark_stale();
    }
}

impl Mirror for GameWatchVars {
    const SIZE: u32 = 0x8;

    fn import(m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
        GameWatchVars {
            attr: Var::new(cx.import_data(m.u32(at))),
            x4: Var::new(m.u32(at + 4) as i32),
        }
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        let p = |n: &str| format!("{path}.{n}");
        if self.attr.live(&p("attr"), cx) {
            cx.compare_data(&p("attr"), *self.attr.peek(), m.u32(at));
        }
        if self.x4.live(&p("x4"), cx) && *self.x4.peek() != m.u32(at + 4) as i32 {
            cx.mismatch(
                &p("x4"),
                format!("{:?}", self.x4.peek()),
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
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = parent;
    spawn.x4_parent_gobj2 = spawn.x0_parent_gobj;
    if initial_collision {
        spawn.x44_flag[0] |= 0x80;
    } else {
        spawn.x44_flag[0] &= !0x80;
    }
    spawn.x40 = 0;
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
    set_flag(&mut item_mut(r.game, gobj).xDCF_flag, 3, true);
    let c = game_watch_color(r, fighter);
    let root = super::super::jobj_of(r.game, gobj);
    crate::game::ft::gamewatch::init::set_diffuse_all(&mut r.game.objs, root, color(c));
    let outline = game_watch_outline(r, fighter);
    item_mut(r.game, gobj).xBC8.x0_unk = outline;
    let owner = item(r.game, gobj)
        .owner
        .expect("ftLib_GetSubColor: no owner");
    let sub = sub_color(r, owner);
    let ip = item_mut(r.game, gobj);
    ip.x5C8 = u8::from(sub.is_some());
    if let Some(s) = sub {
        ip.xBC4 = s;
    }
    super::super::lib2725::scale_by_owner(r, gobj);
    if item(r.game, gobj).kind == super::GAME_WATCH_RESCUE {
        item_mut(r.game, gobj)
            .xDD4_itemVar
            .game_watch_rescue()
            .attr
            .set(attr);
    } else {
        vars(r, gobj).attr.set(attr);
    }
}

/// `ftLib_GetGameWatchColor`: his costume's color (Kirby's copy's isn't ported).
#[doc(alias = "ftLib_GetGameWatchColor")]
fn game_watch_color(r: &Running, fighter: GObjId) -> _GXColor {
    assert_eq!(
        r.game.ft_kind(fighter),
        crate::game::ft::gamewatch::KIND_GAME_WATCH,
        "ftKb_Init_800EEB00 (Kirby's Chef) isn't ported"
    );
    r.gw_color(fighter)
}

/// `ftLib_GetGameWatchOutlineColor`.
#[doc(alias = "ftLib_GetGameWatchOutlineColor")]
fn game_watch_outline(r: &Running, fighter: GObjId) -> _GXColor {
    assert_eq!(
        r.game.ft_kind(fighter),
        crate::game::ft::gamewatch::KIND_GAME_WATCH,
        "ftKb_Init_800EEB1C (Kirby's Chef) isn't ported"
    );
    r.gw_outline_color(fighter)
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
    item_mut(r.game, gobj).xDD4_itemVar.game_watch()
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
    let attr = if item(r.game, gobj).kind == super::GAME_WATCH_RESCUE {
        item_mut(r.game, gobj)
            .xDD4_itemVar
            .game_watch_rescue()
            .attr
            .get()
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
