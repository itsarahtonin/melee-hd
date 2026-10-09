//! Items appearing on their own, `melee/it/itspawn.c`: the match's weighted tables of which
//! kinds appear (built from the stage's counts and the rules' item switches), and the process
//! that drops one somewhere on the stage whenever its timer runs out.

use gekko_fp as fp;
use melee_data::archive::Archives;

use super::data::common;
use super::item::common_data;
use super::kinds::{KURIBOH, M_BALL};
use super::{P_LINK_ITEM, item};
use crate::fpu::reg;
use crate::game::state::{Game, Running};
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `ItemPickTable`: kinds and their cumulative weights, for a weighted draw. The arrays are
/// `HSD_MemAlloc` blocks; `None` is a null pointer.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq)]
pub struct PickTable {
    /// How many kinds the draw picks from.
    pub size: u8,
    /// The kinds (`x4`, bytes).
    pub kinds: Option<Vec<u8>>,
    /// The weights' total (`x8`).
    pub total: u16,
    /// Each kind's weight's start, cumulative (`xC`).
    pub weights: Option<Vec<u16>>,
}

/// `RandomItemSpawner` (`it_804A0E30`): the process's countdown and table.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq)]
pub struct Spawner {
    /// Ticks until the next item (`x0`).
    pub timer: i32,
    pub table: PickTable,
    pub x14: i32,
    /// The rules' item switches, by kind (`x18`).
    pub mask: u64,
}

/// The spawn tables: the process's, the one without the containers (`it_804A0E50`) and the
/// Pokémon-like monsters' (`it_804A0E60`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq)]
pub struct Tables {
    pub spawner: Spawner,
    pub no_containers: PickTable,
    pub monsters: PickTable,
}

/// `It_Kind_L_Gun_Ray`: the kinds below it are the ones that appear on their own.
const RANDOM_KINDS: i32 = 35;
/// `It_Kind_BombHei`: the first kind that isn't a container.
const FIRST_NON_CONTAINER: i32 = 6;

/// The item GObj of kind `kind` latest in the list, `it_80272828`.
#[doc(alias = "it_80272828")]
pub fn find_kind(game: &Game, kind: i32) -> Option<GObjId> {
    let mut found = None;
    let mut cur = game.world.first_in(P_LINK_ITEM);
    while let Some(g) = cur {
        if item(game, g).kind == kind {
            found = Some(g);
        }
        cur = game.world.gobjs[g.0].next;
    }
    found
}

/// Which kinds have an item out, a bit per kind in eight words, `it_8026C47C`.
#[doc(alias = "it_8026C47C")]
pub fn kinds_out(game: &Game) -> [i32; 8] {
    let mut words = [0i32; 8];
    let (mut w, mut bit) = (0, 0);
    for kind in 0..238 {
        if find_kind(game, kind).is_some() {
            words[w] |= 1 << bit;
        }
        bit += 1;
        if (kind + 1) & 31 == 0 {
            bit = 0;
            w += 1;
        }
    }
    words
}

/// The index whose weight range holds `val`, by bisection, `bisectValue`.
#[doc(alias = "bisectValue")]
pub fn bisect(val: i32, weights: &[u16], lo: i32, hi: i32) -> i32 {
    if lo == hi - 1 {
        return lo;
    }
    let mid = (lo + hi) / 2;
    if i32::from(weights[mid as usize]) > val {
        return bisect(val, weights, lo, mid);
    }
    // Past the last weight the original reads the block's uninitialized slack, but with
    // `mid + 1 == hi` both answers are `mid`.
    match weights.get(mid as usize + 1) {
        Some(&w) if i32::from(w) <= val => bisect(val, weights, mid, hi),
        _ => mid,
    }
}

/// A weighted draw from a table, `it_8026C65C`.
#[doc(alias = "it_8026C65C")]
pub fn pick(game: &mut Game, table: &PickTable) -> i32 {
    let val = game.objs.rng.below(i32::from(table.total));
    let i = bisect(
        val,
        table.weights.as_deref().expect("a table without weights"),
        0,
        i32::from(table.size),
    );
    i32::from(table.kinds.as_deref().expect("a table without kinds")[i as usize])
}

/// Whether no more Poké Balls may appear (as many are out as allowed, or the rules don't
/// have them), `it_8026C704`.
#[doc(alias = "it_8026C704")]
pub fn poke_balls_full(game: &Game) -> bool {
    game.it.counts.x1C >= game.it.counts.x20 || !kind_on(game, M_BALL)
}

/// Draws a kind from a table, `it_8026C75C`; -1 for none. While no more Poké Balls may
/// appear, a table ending in one draws without it.
#[doc(alias = "it_8026C75C")]
pub fn pick_kind(game: &mut Game, which: fn(&mut Game) -> &mut PickTable) -> i32 {
    let full = poke_balls_full(game);
    let t = which(game);
    if t.total == 0 {
        return -1;
    }
    let mut saved = None;
    if full {
        let last = usize::from(t.size).wrapping_sub(1);
        let kinds = t.kinds.as_deref().expect("a table without kinds");
        if i32::from(kinds[last]) == M_BALL {
            if (i32::from(t.size) - 1) < 1 {
                return -1;
            }
            saved = Some(t.total);
            t.total = t.weights.as_deref().expect("a table without weights")[last];
            t.size = t.size.wrapping_sub(1);
        }
    }
    let table = which(game).clone();
    let kind = pick(game, &table);
    match saved {
        Some(total) => {
            let t = which(game);
            t.total = total;
            t.size = t.size.wrapping_add(1);
            if kind == M_BALL { -1 } else { kind }
        }
        None => kind,
    }
}

/// The process that drops random items, `fn_8026C88C`: when its countdown runs out, an item
/// of a drawn kind somewhere on the stage, then a new countdown from the common data's range
/// for the item frequency, scaled by the stage's.
#[doc(alias = "fn_8026C88C")]
pub fn proc_spawn(r: &mut Running, _gobj: GObjId) {
    if r.game.it.db_menu.ItemSpawnsEnabled == 0 {
        return;
    }
    let sp = &mut r.game.it.spawn.spawner;
    sp.timer = sp.timer.wrapping_sub(1);
    if sp.timer != 0 {
        return;
    }
    let kind = pick_kind(r.game, |g| &mut g.it.spawn.spawner.table);
    if kind != -1
        && let Some(pos) = spawn_point(r.game, r.data)
    {
        let facing_dir = super::owner::facing_for(r.game, r.data, pos);
        let mut spawn = SpawnItem {
            kind,
            prev_pos: pos,
            pos,
            facing_dir,
            ..SpawnItem::default()
        };
        // The rest of the original's stack copy is never read.
        spawn.x44_flag[0] |= 0x80;
        if let Some(g) = super::item::spawn_air(r, &mut spawn) {
            crate::game::ef::spawn(r, 0x420, Some(g), &[crate::game::ef::Arg::Vec(pos)]);
            super::lib2725::count_spawned(r.game);
        }
    }
    reset_timer(r.game, r.data);
}

/// A new countdown, `it_8026D018`'s and `fn_8026C88C`'s inline: a random time in the common
/// data's range for the item frequency, scaled by the stage's frequency.
fn reset_timer(game: &mut Game, data: &Archives) {
    let freq = game.vs_item_freq();
    let c = common_data(game);
    let lo = c.i32(data, common::XFC + 8 * freq as u32);
    let hi = c.i32(data, common::XFC + 8 * freq as u32 + 4);
    let f = f64::from(game.objs.rng.draw_f32());
    let diff = fp::frsp(f64::from(hi.wrapping_sub(lo)));
    let t = fp::fctiwz(fp::fmadds(diff, f, fp::frsp(f64::from(lo))));
    game.it.spawn.spawner.timer = t;
    let scale = crate::game::gr::ground::item_freq_scale(game, data, game.gr.stkind);
    let t = fp::fmuls(fp::frsp(f64::from(t)), scale);
    game.it.spawn.spawner.timer = fp::fctiwz(t);
}

/// The weights' total of the kinds the mask has on from `first`, `it_8026CA4C`.
#[doc(alias = "it_8026CA4C")]
pub fn total(counts: &[i32; 35], mask: u64, first: i32, weight: f32) -> u16 {
    let mut mask = mask;
    let mut sum = 0i32;
    for i in first..RANDOM_KINDS {
        if mask & 1 != 0 {
            sum = sum.wrapping_add(weighted(weight, counts[i as usize]));
        }
        mask >>= 1;
    }
    sum as u16
}

/// `weight * count + 0.99`, truncated.
fn weighted(weight: f32, count: i32) -> i32 {
    fp::fctiwz(fp::fmadds(
        reg(weight),
        fp::frsp(f64::from(count)),
        f64::from(0.99_f32),
    ))
}

/// A table of the kinds the mask has on from `first` that the stage has, `it_8026CB9C`
/// (from 0) and `it_8026CD50` (from the first non-container).
#[doc(alias = "it_8026CB9C")]
#[doc(alias = "it_8026CD50")]
pub fn build(table: &mut PickTable, counts: &[i32; 35], mask: u64, first: i32, weight: f32) {
    let on = |mask: u64| {
        let mut mask = mask;
        (first..RANDOM_KINDS).filter(move |&i| {
            let on = mask & 1 != 0 && counts[i as usize] != 0;
            mask >>= 1;
            on
        })
    };
    let n = on(mask).count();
    table.size = n as u8;
    let mut kinds = Vec::with_capacity(n);
    let mut weights = Vec::with_capacity(n);
    let mut cumulative = 0i32;
    for i in on(mask) {
        kinds.push(i as u8);
        weights.push(cumulative as u16);
        cumulative = fp::fctiwz(fp::fadds(
            fp::frsp(f64::from(cumulative)),
            fp::fmadds(
                reg(weight),
                fp::frsp(f64::from(counts[i as usize])),
                f64::from(0.99_f32),
            ),
        ));
    }
    table.kinds = Some(kinds);
    table.weights = Some(weights);
}

/// The monsters' table from the common data's weights, `it_8026CF04`.
#[doc(alias = "it_8026CF04")]
pub fn build_monsters(game: &mut Game, data: &Archives) {
    let c = common_data(game);
    let w: [i32; 4] = std::array::from_fn(|i| c.i32(data, common::X128 + 4 * i as u32));
    let sum = w.iter().fold(0i32, |s, &x| s.wrapping_add(x));
    if sum == 0 {
        return;
    }
    let t = &mut game.it.spawn.monsters;
    t.total = sum as u16;
    t.size = 4;
    let mut cumulative = 0u32;
    let mut weights = Vec::with_capacity(4);
    for &x in &w {
        weights.push(cumulative as u16);
        cumulative = cumulative.wrapping_add(x as u32);
    }
    t.kinds = Some((0..4).map(|i| KURIBOH as u8 + i).collect());
    t.weights = Some(weights);
}

/// The stage's item counts by kind (`Ground_801C2AD8`, `stage_info.xA0` on).
#[doc(alias = "Ground_801C2AD8")]
pub fn stage_counts(game: &Game) -> [i32; 35] {
    let si = &game.stage_info;
    std::array::from_fn(|i| match i {
        0 => si.xA0,
        _ => {
            let at = 4 * (i - 1);
            i32::from_be_bytes(si.xA4_pad[at..at + 4].try_into().unwrap())
        }
    })
}

/// Sets up the match's random items, `it_8026D018`: the tables, and the process with its
/// first countdown, unless the rules have none (or rain bob-ombs).
#[doc(alias = "it_8026D018")]
pub fn setup(r: &mut Running) {
    let game = &mut *r.game;
    if game.vs_x6() != 0 || game.vs_item_freq() == -1 {
        return;
    }
    let mask = game.vs.start.x20;
    game.it.spawn.spawner.mask = mask;
    let counts = stage_counts(game);
    let weight = game.vs_unk34();
    if mask == 0 || game.vs_item_freq() == -1 {
        return;
    }
    let t = total(&counts, mask, 0, weight);
    game.it.spawn.spawner.table.total = t;
    if t == 0 {
        return;
    }
    build(&mut game.it.spawn.spawner.table, &counts, mask, 0, weight);
    let mask = mask >> FIRST_NON_CONTAINER;
    let t = total(&counts, mask, FIRST_NON_CONTAINER, weight);
    game.it.spawn.no_containers.total = t;
    if t != 0 {
        build(
            &mut game.it.spawn.no_containers,
            &counts,
            mask,
            FIRST_NON_CONTAINER,
            weight,
        );
    }
    build_monsters(game, r.data);
    let g = game.world.create(5, 7, 0);
    game.world
        .setup_proc(g, crate::game::handles::CodeRef(PROC_SPAWN), 0);
    reset_timer(game, r.data);
}

/// [`proc_spawn`]'s address, as a process.
pub const PROC_SPAWN: u32 = 0x8026_C88C;

/// Drops an item of `kind` at `pos`, if fewer are out than allowed, `it_8026D258`.
#[doc(alias = "it_8026D258")]
pub fn drop_at(r: &mut Running, pos: Vec3, kind: i32) -> bool {
    if r.game.it.counts.x60 >= r.game.it.counts.x64 {
        return false;
    }
    let prev_pos = Vec3::new(pos.x, pos.y, 0.0);
    let facing_dir = super::owner::facing_for(r.game, r.data, prev_pos);
    let mut spawn = SpawnItem {
        kind,
        prev_pos,
        pos: prev_pos,
        facing_dir,
        ..SpawnItem::default()
    };
    spawn.x44_flag[0] |= 0x80;
    super::item::spawn_ground_marked(r, &mut spawn);
    true
}

/// Whether the rules have kind `kind` on, `it_8026D324`.
#[doc(alias = "it_8026D324")]
pub fn kind_on(game: &Game, kind: i32) -> bool {
    let mask = game.it.spawn.spawner.mask;
    if mask == 0 || game.vs_item_freq() == -1 {
        return false;
    }
    mask >> kind & 1 != 0
}

/// Whether any healing item is on, `it_8026D3CC`.
#[doc(alias = "it_8026D3CC")]
pub fn healing_on(game: &Game) -> bool {
    kind_on(game, 8) | kind_on(game, 9) | kind_on(game, 18)
}

/// A random spot on the stage where an item can stand, `it_8026CB3C`.
#[doc(alias = "it_8026CB3C")]
pub fn spawn_point(game: &mut Game, data: &Archives) -> Option<Vec3> {
    let mut p = crate::game::gr::stage::random_item_point(game, data)?;
    p.z = 0.0;
    let squeezed = super::maplib::with_stage_coll(game, data, |c| c.squeezed_at(p));
    (!squeezed).then_some(p)
}
