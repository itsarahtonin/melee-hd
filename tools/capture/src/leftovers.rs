//! Heap leftovers in a session from power-on: a block the scene's heap hands a new object
//! (a fighter, an item, a stage object) keeps the bytes an earlier scene left there in every
//! field its creation doesn't write, where melee-sim, whose heap holds no bytes, has the
//! field's zero. Such a field is unknown until written: while ours still holds its zero and
//! the original's still holds what the block held when the object appeared, the comparer
//! leaves it out, by a list of fields each proven never to reach gameplay or what players see
//! and hear before something writes it (`docs/engine.md`, "Uninitialized values"). Once
//! either side writes it, it compares as any field does.
//!
//! The record is per session: [`enable`] it before the first tick, [`end_tick`] after each.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};

/// A color overlay's fields its owner's creation leaves (`ColorOverlay`): the colors and
/// lights are read (by the material passes, the light and the background flash) only while
/// the overlay's color (`x7C_color_enable`) or light (`x7C_flag2`) is on, which creation
/// turns off and only `lb_80014014` and `lb_80013C18` turn on, writing them; a blend before
/// either computes steps that the enabling command zeroes and nothing adds while off. The
/// script's timer, loop index and return stack are written by every script's start and
/// push; `x24` and the other flag bits no code names.
macro_rules! overlay {
    ($at:literal) => {
        [
            concat!($at, ".script.timer"),
            concat!($at, ".script.loop_count"),
            concat!($at, ".script.ret[]"),
            concat!($at, ".x24"),
            concat!($at, ".x2C_hex."),
            concat!($at, ".x30_color_red"),
            concat!($at, ".x34_color_green"),
            concat!($at, ".x38_color_blue"),
            concat!($at, ".x3C_color_alpha"),
            concat!($at, ".x40_colorblend_red"),
            concat!($at, ".x44_colorblend_green"),
            concat!($at, ".x48_colorblend_blue"),
            concat!($at, ".x4C_colorblend_alpha"),
            concat!($at, ".x50_light_color."),
            concat!($at, ".x54_light_red"),
            concat!($at, ".x58_light_green"),
            concat!($at, ".x5C_light_blue"),
            concat!($at, ".x60_light_alpha"),
            concat!($at, ".x64_lightblend_red"),
            concat!($at, ".x68_lightblend_green"),
            concat!($at, ".x6C_lightblend_blue"),
            concat!($at, ".x70_lightblend_alpha"),
            concat!($at, ".x74_light_rot_x"),
            concat!($at, ".x78_light_rot_yz"),
            concat!($at, ".x7C_light_enable"),
            concat!($at, ".x7C_flag4"),
            concat!($at, ".x7C_flag5"),
            concat!($at, ".x7C_flag6"),
            concat!($at, ".x7C_flag7"),
            concat!($at, ".x7C_flag8"),
        ]
    };
}

const OVERLAYS: &[(&str, &[&str])] = &[
    ("fighter", &overlay!(".x408")),
    ("fighter", &overlay!(".x488")),
    ("fighter", &overlay!(".x508")),
    ("item", &overlay!(".x548_colorOverlay")),
    ("ground", &overlay!(".color_overlay")),
    ("flash", &overlay!(".x4")),
    // An item's command script: its start (`Item_80268E5C`) writes the timer and loop index.
    (
        "item",
        &[
            ".x524_cmd.script.timer",
            ".x524_cmd.script.loop_count",
            ".x524_cmd.script.ret[]",
        ],
    ),
];

/// The fields a block of each kind may hold leftovers in, as path patterns after the block's
/// own path (indices written `[]`; a pattern ending in `.` covers every field under it).
/// Each has its proof in `docs/engine.md`.
const FIELDS: &[(&str, &[&str])] = &[
    (
        "fighter",
        &[
            // Hitboxes: read only while on, and the command that turns one on writes what the
            // passes read; the throw boxes are left out of this (the grab breaks read the
            // second one, which a throw's script may not have set yet).
            ".x914[].",
            // Collision scratch: the contact point is the tests' out-parameter, the stick's x
            // is read by no code, `x13C` named by none.
            ".coll_data.contact",
            ".coll_data.lstick_x",
            ".coll_data.x13C",
            // Timers and callbacks read only under the flag set with them.
            ".bury_timer_2",
            ".grab_cb",
            ".grabbed_cb",
            ".x2194",
            ".x1A68",
            ".x2004",
            ".x2014",
            ".x201C",
            ".x2104",
            ".x104",
            ".x19B0",
            ".x2330.",
            ".x2338.",
            ".smash_attrs.",
            // Written by every display pass before anything reads them.
            ".x2188.",
            ".x2223_b2",
            ".x2223_b3",
            ".x2228_b0",
            // Read only by the outline pass of the fighters that write it.
            ".x610_color_rgba[].",
            // Named by no code.
            ".x107",
            ".x2022",
            ".x206E",
            ".x2102",
            ".x2103",
            ".x221F_b7",
            ".filler_x1674[]",
            ".filler_x2044[]",
            ".filler_x210C[]",
            // The dynamics' last link's axis, which nothing reads (docs/engine.md).
            ".dynamic_bone_sets[]@.desc.unk_54",
            // The damage record a hit writes before its readers run.
            ".dmg.facing_dir_1",
            ".dmg.x1854_collpos",
            ".dmg.x1880",
            ".dmg.x188c",
            ".dmg.x1894",
            ".dmg.x1900",
            ".dmg.x1904",
        ],
    ),
    (
        "item",
        &[
            // Item hitboxes: read only while on, and the script's command writes every bit
            // read (`x43_b3` to `x43_b7` no code names; `x134` only the grapple's reads).
            ".x5D4_hitboxes[].hit.",
            ".x378_itemColl.contact",
            ".x378_itemColl.lstick_x",
            ".x378_itemColl.x13C",
            // Written together with the flag (`xDD0_flag` bit 5) they're read under.
            ".grab_dealt",
            ".grabbed_for_victim",
            ".xD08",
            // A reflect's record, written by `ftColl_80077464` and copied on the reflect.
            ".xC74",
            ".xC78.",
            ".xC80.",
            ".xC88",
            ".xC8C",
            // Named by no code.
            ".x5CA",
            ".x5CB",
            ".xB69",
            ".xB6A",
            ".xB6B",
            ".xD0A",
            ".xD0B",
            ".xD8E",
            ".xDD2_flag",
            ".xDD3_flag",
        ],
    ),
    (
        "ground",
        &[
            // Named by no code (the low three bits are written at creation).
            ".x0",
            ".x11_flags.b3",
            ".x11_flags.b4",
            ".x11_flags.b5",
            ".x11_flags.b6",
            ".x11_flags.b7",
            ".u.izumi3.xCA",
            ".u.zebes5.xFA",
            // Kongo Jungle's barrel's wait: written as it takes its item (`grKongo_801D577C`'s
            // state 0, which its setup sets) before state 1 counts it down.
            ".u.kongo3.xCA",
            // Brinstar's rise wait and quake count: written by the step before the one
            // reading them (`grZebes_801D881C`'s states 1 and 2), from state 0, which the
            // stage's setup sets.
            ".u.zebes5.xC6",
            ".u.zebes5.xF6",
        ],
    ),
];

/// A session's leftovers: which blocks lived at the last comparison, and each unknown field's
/// value at the object's appearance.
#[derive(Default)]
pub struct Leftovers {
    prev: HashSet<(&'static str, u32)>,
    live: HashSet<(&'static str, u32)>,
    born: HashSet<(&'static str, u32)>,
    values: HashMap<(&'static str, u32, String), String>,
    /// Every field left out this session, by kind and pattern, and how many times.
    pub left_out: BTreeMap<(String, String), usize>,
    /// Whether to leave out any field (not only [`FIELDS`]'s): to survey what a session's
    /// blocks hold.
    pub survey: bool,
}

thread_local! {
    static LEFTOVERS: RefCell<Option<Leftovers>> = const { RefCell::new(None) };
}

/// Starts a session's record; `survey` leaves out every field a new block differs in.
pub fn enable(survey: bool) {
    LEFTOVERS.with(|l| {
        *l.borrow_mut() = Some(Leftovers {
            survey,
            ..Default::default()
        })
    });
}

/// Whether a session's record is on.
pub fn enabled() -> bool {
    LEFTOVERS.with(|l| l.borrow().is_some())
}

/// Ends a tick's comparison: the blocks met become the ones that lived.
pub fn end_tick() {
    LEFTOVERS.with(|l| {
        if let Some(l) = l.borrow_mut().as_mut() {
            l.prev = std::mem::take(&mut l.live);
            l.born.clear();
        }
    });
}

/// The fields left out this session, by kind and pattern, with their counts.
pub fn report() -> Vec<String> {
    LEFTOVERS.with(|l| {
        l.borrow().as_ref().map_or(Vec::new(), |l| {
            l.left_out
                .iter()
                .map(|((k, p), n)| format!("{k} {p}: {n}"))
                .collect()
        })
    })
}

/// A block of `kind` at the original's `addr` is being compared: new since the last tick's
/// comparison, its fields' leftovers are taken as they are.
pub(crate) fn begin(kind: &'static str, addr: u32) {
    LEFTOVERS.with(|l| {
        if let Some(l) = l.borrow_mut().as_mut() {
            l.live.insert((kind, addr));
            if !l.prev.contains(&(kind, addr)) && l.born.insert((kind, addr)) {
                l.values.retain(|(k, a, _), _| !(*k == kind && *a == addr));
            }
        }
    });
}

/// A field's path with its indices as `[]` (and a variant's number after `@` dropped).
fn pattern(field: &str) -> String {
    let mut out = String::with_capacity(field.len());
    let mut in_index = false;
    for c in field.chars() {
        match c {
            '[' => {
                in_index = true;
                out.push('[');
            }
            ']' => {
                in_index = false;
                out.push(']');
            }
            '@' => {
                in_index = true;
                out.push('@');
            }
            '.' if in_index => {
                in_index = false;
                out.push('.');
            }
            c if in_index && c.is_ascii_digit() => {}
            c => out.push(c),
        }
    }
    out
}

/// Whether a value as the mirror prints it is its type's zero.
fn is_zero(v: &str) -> bool {
    if v == "false" || v == "None" {
        return true;
    }
    let v = v.trim_start_matches("Some(").trim_end_matches(')');
    v.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .all(|c| c == '0' || c == 'x')
        && v.chars().any(|c| c == '0')
}

/// A value as the mirror prints it as a big-endian word: a number, or bytes (`[01, 23]`).
fn word(v: &str) -> Option<u64> {
    if let Some(bytes) = v.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
        return bytes.split(", ").try_fold(0u64, |w, b| {
            u8::from_str_radix(b, 16)
                .ok()
                .map(|b| w << 8 | u64::from(b))
        });
    }
    v.strip_prefix("0x")
        .map_or_else(|| v.parse().ok(), |h| u64::from_str_radix(h, 16).ok())
}

/// Words whose code writes and reads only the bits `mask` names, by kind and pattern: the
/// others keep the block's leftovers (`docs/engine.md`).
const MASKED: &[(&str, &str, u64)] = &[
    // An item's attack record's flags (`xDA4_word`, a copy of a fighter's
    // `x2074.x10.x2084`): `ft_80089768`'s clear writes `x2084_b0` to `x2085_b4`, the rest no
    // code names.
    ("item", ".xDA4_word", 0xFFF8_0000),
    // The fighter's own record (`Struct2074_x10`): its clears (`ft_800892D4`, `ft_80089460`)
    // write `x2084_b0` to `x2085_b4`; whole-word reads only copy it into an item (above), and
    // the stats read `x2084_b0` and `x2084_b7` (`pl_80038144`).
    ("fighter", ".x2074.x10", 0xFFF8_0000),
    // A fighter's hitbox's `x134` holds only its grabbed-victim-only bit (the top), which its
    // command writes (`ftAction_80071028`) and `ftColl` reads; only a thrown fighter's
    // `x1064_thrownHitbox` holds a thrower's pointer there.
    ("fighter", ".x914[].x134", 0x8000_0000),
    // An item's flag bytes: no code names `xDD0_flag`'s bit 2 or `xDD1_flag`'s six low bits.
    ("item", ".xDD0_flag", 0xDF),
    ("item", ".xDD1_flag", 0xC0),
    // Brinstar's creature's flag byte: code names only its top bit (`x0_b0`).
    ("ground", ".u.zebes.x0", 0x80),
];

/// The fields whose leftovers the original moves (copies whole with their block's
/// neighbours), left out wherever ours holds the zero: each never read unless written.
const MOVED: &[(&str, &[&str])] = &[(
    "fighter",
    // A hitbox's victim records: `lbColl_80008A5C` reads a record's timer (`x4`) only while
    // its victim is set, and `lbColl_80008688`/`lbColl_80008820` write both together.
    &[".x914[].victims_1[].x4", ".x914[].victims_2[].x4"],
)];

fn moves(kind: &str, pat: &str) -> bool {
    MOVED.iter().any(|(k, fields)| {
        *k == kind
            && fields
                .iter()
                .any(|f| pat == *f || (f.ends_with('.') && pat.starts_with(f)))
    })
}

fn allowed(kind: &str, pat: &str) -> bool {
    FIELDS.iter().chain(OVERLAYS).any(|(k, fields)| {
        *k == kind
            && fields
                .iter()
                .any(|f| pat == *f || (f.ends_with('.') && pat.starts_with(f)))
    })
}

/// A mismatch in block `kind` at `addr` (the field's path after the block's): whether it is a
/// leftover still unknown on both sides, which the comparer leaves out.
pub(crate) fn leave_out(
    kind: &'static str,
    addr: u32,
    field: &str,
    ours: &str,
    theirs: &str,
) -> bool {
    LEFTOVERS.with(|l| {
        let mut b = l.borrow_mut();
        let Some(l) = b.as_mut() else { return false };
        let pat = pattern(field);
        // A word code writes only some bits of: the rest are leftovers.
        // A word code writes only some bits of: the rest are leftovers. A difference in the
        // written bits is the field's own, unless the word is still unwritten (below).
        if let Some(&(_, _, mask)) = MASKED.iter().find(|(k, p, _)| *k == kind && *p == pat)
            && let (Some(o), Some(t)) = (word(ours), word(theirs))
            && (o ^ t) & mask == 0
        {
            *l.left_out
                .entry((format!("{kind} masked"), pat))
                .or_default() += 1;
            return true;
        }
        if !(l.survey || allowed(kind, &pat)) || !is_zero(ours) {
            return false;
        }
        // Unknown while the original's still holds what the block held when the object
        // appeared; a field the original moves its leftovers through (a hitbox copied whole
        // carries its victims') is listed as such, and left out wherever ours holds the zero.
        let key = (kind, addr, field.to_owned());
        let born = l.born.contains(&(kind, addr));
        let unchanged = if born {
            l.values.insert(key, theirs.to_owned());
            true
        } else {
            l.values.get(&key).is_some_and(|v| v == theirs)
        };
        let moved = moves(kind, &pat);
        let what = if unchanged { "unwritten" } else { "moved" };
        if unchanged || moved || l.survey {
            // A survey's report can name each field with its indices.
            let name = if l.survey && std::env::var_os("LEFTOVERS_INDICES").is_some() {
                field.to_owned()
            } else {
                pat
            };
            *l.left_out
                .entry((format!("{kind} {what}"), name))
                .or_default() += 1;
            return true;
        }
        false
    })
}
