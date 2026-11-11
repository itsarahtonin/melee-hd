//! Scans every file on the disc for what its data can make the game run, the evidence behind
//! `ledger/dead.txt`'s disc-data proofs: a handler that runs only for a command no file holds
//! can't run.
//!
//! `SSBM_DISC=... cargo run --release -p melee-disc-scan -- fighter-commands [OPCODE...]`
//! walks every action script of every fighter file (each `Pl*.dat`'s `ftData*` root: its
//! animations, `xC`, and its demo animations, `x14`, the two tables `Fighter::x24` is set
//! from) command by command, through its subroutines and gotos, and counts the scripts that
//! hold each of the fighter's own opcodes (10 to 58, `ftAction_803C06E8`). With opcodes it
//! lists each script that holds them and the earliest frame each is reached at.
//!
//! `item-hitboxes` walks the state scripts of every character's own items (each fighter file's
//! `ftData::x48` articles) and prints each hitbox command with the bits the clank and absorb
//! tests read; `item-commands [OPCODE...]` counts every item script command on the disc (every
//! article's states and the stages' `ALDYakuAll` scripts). `item-colanims` walks the items' color animations (`ItCo`'s `itPublicData`
//! `x14`) and names any owner's command (21 and up) in them; `colanims FILE ROOT FIELD` any
//! other table (the fighters': `PlCo.dat ftLoadCommonData 0x18`, and `0x1C`). `robjs` counts
//! the joints' and camera positions' reference objects by type and lists any expression;
//! `stage-colanims` walks the stages' color animation scripts for owner's commands. `words
//! FILE OFFSET COUNT` prints a file's words, pointers marked.

use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;

use melee_data::archive::Archive;
use melee_data::disc::Disc;

/// Each fighter command's length in words (`ftAction_803C0870`), from opcode 10.
const FIGHTER_COMMAND_WORDS: [u32; 49] = [
    5, 5, 1, 1, 1, 1, 1, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 3, 1, 1, 1, 7, 4, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 3, 3, 2, 1, 4,
];

/// `Fighter_WaitAnimData`'s size; its script is at `xC`.
const ANIM_ENTRY: u32 = 0x18;

/// One command found in a script.
struct Found {
    op: u32,
    word: u32,
    file: String,
    root: String,
    table: &'static str,
    entry: u32,
    anim: String,
    offset: u32,
    frame: u32,
}

/// What a script walk found: each owner command's opcode and where, and the commands the
/// walk couldn't decode.
#[derive(Default)]
struct Walk {
    commands: Vec<(u32, u32, u32)>,
    bad: Vec<(u32, u32)>,
}

/// Walks a command script from `start` through its subroutines and gotos (`Command_Execute`'s
/// codes 0 to 9), each command once, with the earliest frame the script reaches it at (the
/// timers before it on the way, loops taken once); `words` gives an owner command's length
/// from its opcode and first word.
fn walk(a: &Archive, start: u32, words: &dyn Fn(u32, u32) -> Option<u32>) -> Walk {
    let mut out = Walk::default();
    let mut seen = BTreeSet::new();
    let mut todo = vec![(start, 0u32)];
    let len = a.data().len() as u32;
    while let Some((mut at, mut frame)) = todo.pop() {
        loop {
            if at + 4 > len || !seen.insert(at) {
                break;
            }
            let w = a.u32(at);
            let op = w >> 26;
            let arg = w & 0x3FF_FFFF;
            match op {
                0 | 6 => break,
                1 => {
                    frame = frame.saturating_add(arg);
                    at += 4;
                }
                2 => {
                    frame = frame.max(arg);
                    at += 4;
                }
                3 | 4 | 8 | 9 => at += 4,
                5 | 7 => {
                    // A null target ends the script: the interpreter stops on a null cursor.
                    let Some(target) = a.ptr(at + 4) else {
                        if a.u32(at + 4) != 0 {
                            out.bad.push((at, op));
                        }
                        break;
                    };
                    if op == 5 {
                        todo.push((target, frame));
                        at += 8;
                    } else {
                        at = target;
                    }
                }
                _ => match words(op, w) {
                    Some(n) => {
                        out.commands.push((op, at, frame));
                        at += 4 * n;
                    }
                    None => {
                        out.bad.push((at, op));
                        break;
                    }
                },
            }
        }
    }
    out
}

/// The offsets something in the archive points at or names: where its objects start.
fn object_starts(a: &Archive) -> BTreeSet<u32> {
    let mut s: BTreeSet<u32> = a
        .relocs()
        .iter()
        .filter(|&&r| r % 4 == 0)
        .map(|&r| a.u32(r))
        .collect();
    s.extend(a.publics().map(|(_, o)| o));
    s
}

fn c_string(a: &Archive, at: u32) -> String {
    let d = &a.data()[at as usize..];
    let end = d.iter().position(|&c| c == 0).unwrap_or(d.len());
    String::from_utf8_lossy(&d[..end]).into_owned()
}

/// Every script of every fighter file's animation tables.
fn fighter_commands(disc: &Disc, files: &[String]) -> (Vec<Found>, Vec<String>, u32) {
    let mut found = Vec::new();
    let mut problems = Vec::new();
    let mut scripts = 0;
    let words = |op: u32, _: u32| {
        op.checked_sub(10)
            .and_then(|i| FIGHTER_COMMAND_WORDS.get(i as usize))
            .copied()
    };
    for path in files {
        let name = path.rsplit('/').next().unwrap_or(path);
        if !(name.starts_with("Pl") && name.ends_with(".dat")) {
            continue;
        }
        let Ok(bytes) = disc.read_file(path) else {
            continue;
        };
        let Ok(a) = Archive::parse(&bytes) else {
            continue;
        };
        let roots: Vec<(String, u32)> = a
            .publics()
            .filter(|(n, _)| n.starts_with("ftData"))
            .map(|(n, o)| (n.to_owned(), o))
            .collect();
        if roots.is_empty() {
            continue;
        }
        let starts = object_starts(&a);
        for (root, at) in roots {
            for (table, field) in [("anims", 0xC), ("demo", 0x14)] {
                let Some(t) = a.ptr(at + field) else { continue };
                let end = starts
                    .range(t + 1..)
                    .next()
                    .copied()
                    .unwrap_or(a.data().len() as u32);
                problems.push(format!(
                    "table: {name} {root} {table} {} entries",
                    (end - t) / ANIM_ENTRY
                ));
                for entry in 0..(end - t) / ANIM_ENTRY {
                    let e = t + entry * ANIM_ENTRY;
                    let Some(script) = a.ptr(e + 0xC) else {
                        continue;
                    };
                    scripts += 1;
                    let anim = a.ptr(e).map(|s| c_string(&a, s)).unwrap_or_default();
                    let w = walk(&a, script, &words);
                    for (at, op) in w.bad {
                        problems.push(format!(
                            "{name} {root} {table}[{entry}] {anim}: opcode {op} at {at:#x} isn't a command"
                        ));
                    }
                    for (op, offset, frame) in w.commands {
                        found.push(Found {
                            op,
                            word: a.u32(offset),
                            file: name.to_owned(),
                            root: root.clone(),
                            table,
                            entry,
                            anim: anim.clone(),
                            offset,
                            frame,
                        });
                    }
                }
            }
        }
    }
    (found, problems, scripts)
}

/// An item command's length in words (`it_803F22A8`, opcodes 10 to 25): the effect's 5, the
/// hitbox's 6, the sound's 3 for a sound played or stopped (`it_8027978C`'s 0 to 2, 10, 11)
/// and 2 otherwise, 1 for the rest.
fn item_command_words(op: u32, w: u32) -> Option<u32> {
    match op {
        10 => Some(5),
        11 => Some(6),
        16 => Some(if matches!((w >> 18) & 0xFF, 0..=2 | 10 | 11) {
            3
        } else {
            2
        }),
        12..=25 => Some(1),
        _ => None,
    }
}

/// Every hitbox command (item command 11, `it_802790C0`) in the state scripts of each fighter
/// file's own items (`ftData::x48`, the articles each character registers on load with
/// `it_8026B3F8`): its element and the flags the clank and absorb tests read.
fn item_hitboxes(disc: &Disc, files: &[String]) {
    for path in files {
        let name = path.rsplit('/').next().unwrap_or(path);
        if !(name.starts_with("Pl") && name.ends_with(".dat")) {
            continue;
        }
        let Ok(bytes) = disc.read_file(path) else {
            continue;
        };
        let Ok(a) = Archive::parse(&bytes) else {
            continue;
        };
        let starts = object_starts(&a);
        let bound = |t: u32| {
            starts
                .range(t + 1..)
                .next()
                .copied()
                .unwrap_or(a.data().len() as u32)
        };
        let roots: Vec<(String, u32)> = a
            .publics()
            .filter(|(n, _)| n.starts_with("ftData"))
            .map(|(n, o)| (n.to_owned(), o))
            .collect();
        for (root, at) in roots {
            // Kirby's hat files (`PlKbCp*.dat`) are `KirbyHatStruct`s whose five `hat_dynamics`
            // words, from 0xC, are the copies' articles; a fighter's list is `ftData::x48`.
            let (list, count) = if name.starts_with("PlKbCp") {
                (at + 0xC, 5)
            } else {
                let Some(list) = a.ptr(at + 0x48) else {
                    continue;
                };
                (list, (bound(list) - list) / 4)
            };
            println!(
                "{name} {root} at {at:#x}: attributes x4 at {:#x}",
                a.ptr(at + 4).unwrap_or(0)
            );
            for i in 0..count {
                let Some(article) = a.ptr(list + 4 * i) else {
                    continue;
                };
                let Some(states) = a.ptr(article + 0xC) else {
                    println!("{name} {root} article {i}: no states");
                    continue;
                };
                let n = (bound(states) - states) / 0x10;
                let mut boxes = 0;
                for st in 0..n {
                    let Some(script) = a.ptr(states + 0x10 * st + 0xC) else {
                        continue;
                    };
                    let w = walk(&a, script, &item_command_words);
                    for (at, op) in w.bad {
                        println!(
                            "{name} {root} article {i} state {st}: opcode {op} at {at:#x} isn't a command"
                        );
                    }
                    for (op, off, frame) in w.commands {
                        if op != 11 {
                            continue;
                        }
                        boxes += 1;
                        let (w4, w5) = (a.u32(off + 16), a.u32(off + 20));
                        println!(
                            "{name} {root} article {i} state {st} frame {frame}: hitbox at {off:#x}, element {}, clank (x40_b0) {}, x41_b7 {}, x42_b0 {}, x42_b4 {} ({w4:#010x} {w5:#010x})",
                            (w4 >> 18) & 0x1F,
                            (w4 >> 17) & 1,
                            (w5 >> 20) & 1,
                            (w5 >> 19) & 1,
                            (w5 >> 15) & 1,
                        );
                    }
                }
                println!("{name} {root} article {i}: {n} states, {boxes} hitbox commands");
            }
        }
    }
}

/// A color animation command's length in words (`lb_803BA248`, opcodes 11 to 20); 10 ends the
/// script. The owner's commands (21 and up) are the fighters' `ftCo_803C6AD0` ones (the
/// fighter commands 10, 17 and 57: 5, 3 and 1 words); an item's are reported undecoded.
fn color_command_words(op: u32, _: u32) -> Option<u32> {
    match op {
        11 | 12 | 16 | 17 | 20 => Some(1),
        13 | 14 | 15 | 18 | 19 => Some(2),
        _ => None,
    }
}

fn fighter_color_command_words(op: u32, w: u32) -> Option<u32> {
    match op {
        21 => Some(5),
        22 => Some(3),
        23 => Some(1),
        _ => color_command_words(op, w),
    }
}

/// Every command of a color animation table (8 bytes an entry: the script, then its priority),
/// the word at `field` of the public `root` in each disc file named `file` or starting with
/// it, and the owner's commands (21 and up) among them: the items' (`ItCo`, `itPublicData`
/// 0x14, which `fn_80279B30` runs through `it_804D51C8`) and the fighters' (`PlCo.dat`,
/// `ftLoadCommonData` 0x18 and the Cloak's 0x1C, run through `ft_800BFF34` and
/// `ft_800BFF70`).
fn colanims(disc: &Disc, files: &[String], file: &str, root: &str, field: u32) {
    let words: &dyn Fn(u32, u32) -> Option<u32> = if file.starts_with("Pl") {
        &fighter_color_command_words
    } else {
        &color_command_words
    };
    for path in files {
        let name = path.rsplit('/').next().unwrap_or(path);
        if !name.starts_with(file) {
            continue;
        }
        let Ok(bytes) = disc.read_file(path) else {
            continue;
        };
        let Ok(a) = Archive::parse(&bytes) else {
            continue;
        };
        let Some(at) = a.public(root) else {
            continue;
        };
        let Some(t) = a.ptr(at + field) else {
            continue;
        };
        let starts = object_starts(&a);
        let end = starts
            .range(t + 1..)
            .next()
            .copied()
            .unwrap_or(a.data().len() as u32);
        let n = (end - t) / 8;
        println!("{name} {root} {field:#x}: {n} color animations at {t:#x}");
        let mut owner_ops = BTreeSet::new();
        for i in 0..n {
            let Some(script) = a.ptr(t + 8 * i) else {
                println!("  {i}: no script");
                continue;
            };
            let w = walk(&a, script, words);
            let mut ops: Vec<u32> = w.commands.iter().map(|c| c.0).collect();
            ops.sort_unstable();
            ops.dedup();
            let mut owner: Vec<String> = w
                .commands
                .iter()
                .filter(|c| c.0 >= 21)
                .map(|c| format!("{} at {:#x}", c.0, c.1))
                .collect();
            owner.extend(
                w.bad
                    .iter()
                    .filter(|(_, op)| *op != 10)
                    .map(|(at, op)| format!("{op} at {at:#x} (undecoded)")),
            );
            owner_ops.extend(ops.iter().copied().filter(|&o| o >= 21));
            owner_ops.extend(w.bad.iter().map(|b| b.1).filter(|&o| o != 10));
            println!(
                "  {i}: script at {script:#x}, flag byte {:#04x}, commands {ops:?}, owner's: {}",
                a.u8(t + 8 * i + 5),
                if owner.is_empty() {
                    "none".to_owned()
                } else {
                    owner.join(", ")
                }
            );
        }
        println!("{name}: owner's commands in the table: {owner_ops:?}");
    }
}

/// Every item article on the disc, as (where, its offset): the item common file's tables
/// (`ItCo`'s `itPublicData`: 0x4 the common items', 0x8 the table the kinds 43 to 47 take,
/// 0xC the Pokemon's; 0x10's words are no articles), each fighter file's own items (`ftData::x48`) and Kirby's
/// hats' (`hat_dynamics`), and every stage file's `itemdata` list (`{kind, article}` pairs, as
/// `Ground_801C0800`'s setup reads them).
fn articles_of(name: &str, a: &Archive) -> Vec<(String, u32)> {
    let starts = object_starts(a);
    let bound = |t: u32| {
        starts
            .range(t + 1..)
            .next()
            .copied()
            .unwrap_or(a.data().len() as u32)
    };
    let list = |label: &str, t: u32, n: u32| -> Vec<(String, u32)> {
        (0..n)
            .filter_map(|i| a.ptr(t + 4 * i).map(|art| (format!("{label}[{i}]"), art)))
            .collect()
    };
    let mut out = Vec::new();
    if name.starts_with("ItCo")
        && let Some(root) = a.public("itPublicData")
    {
        for (label, field) in [("common", 4), ("x8", 8), ("pokemon", 0xC), ("x10", 0x10)] {
            if let Some(t) = a.ptr(root + field) {
                out.extend(list(label, t, (bound(t) - t) / 4));
            }
        }
    }
    for (root, at) in a.publics() {
        if root.starts_with("ftData") && name.starts_with("Pl") {
            if name.starts_with("PlKbCp") {
                out.extend(list(&format!("{root} hat_dynamics"), at + 0xC, 5));
            } else if let Some(t) = a.ptr(at + 0x48) {
                out.extend(list(&format!("{root} x48"), t, (bound(t) - t) / 4));
            }
        }
        if root == "itemdata" {
            let mut i = 0;
            while let Some(pair) = a.ptr(at + 4 * i) {
                if let Some(art) = a.ptr(pair + 4) {
                    out.push((format!("itemdata[{i}] kind {}", a.u32(pair)), art));
                }
                i += 1;
            }
        }
    }
    // Only words that look like an article: a state table and a model.
    out.retain(|&(_, art)| a.ptr(art + 0xC).is_some() && a.ptr(art + 0x10).is_some());
    out
}

/// Counts every item command in every item state script on the disc (the articles of
/// `articles_of`, each state description's script, and the stages' `ALDYakuAll` scripts, which
/// `Ground_801C0800` puts in the random stage item's descriptions), and lists where the
/// opcodes in `want` are.
fn item_commands(disc: &Disc, files: &[String], want: &BTreeSet<u32>) {
    let mut counts: BTreeMap<u32, u32> = BTreeMap::new();
    let (mut n_articles, mut n_scripts) = (0, 0);
    let mut hits = Vec::new();
    for path in files {
        let name = path.rsplit('/').next().unwrap_or(path);
        if !name.ends_with(".dat") && !name.ends_with(".usd") {
            continue;
        }
        let Ok(bytes) = disc.read_file(path) else {
            continue;
        };
        let Ok(a) = Archive::parse(&bytes) else {
            continue;
        };
        let starts = object_starts(&a);
        let mut scripts: Vec<(String, u32)> = Vec::new();
        let arts = articles_of(name, &a);
        if !arts.is_empty() && want.contains(&0) {
            println!("{name}: {} articles", arts.len());
        }
        for (label, art) in arts {
            n_articles += 1;
            let states = a.ptr(art + 0xC).expect("checked");
            let end = starts
                .range(states + 1..)
                .next()
                .copied()
                .unwrap_or(a.data().len() as u32);
            for st in 0..(end - states) / 0x10 {
                if let Some(script) = a.ptr(states + 0x10 * st + 0xC) {
                    scripts.push((format!("{label} state {st}"), script));
                }
            }
        }
        if let Some(at) = a.public("ALDYakuAll") {
            let mut i = 0;
            while i < 64 {
                match a.ptr(at + 4 * i) {
                    Some(script) => scripts.push((format!("ALDYakuAll[{i}]"), script)),
                    None if i > 0 => break,
                    None => {}
                }
                i += 1;
            }
        }
        for (label, script) in scripts {
            n_scripts += 1;
            let w = walk(&a, script, &item_command_words);
            for (at, op) in w.bad {
                println!("not decoded: {name} {label}: opcode {op} at {at:#x}");
            }
            for (op, off, frame) in w.commands {
                *counts.entry(op).or_default() += 1;
                if want.contains(&op) {
                    hits.push(format!(
                        "  {op}: {name} {label} at {off:#x} ({:#010x}), frame {frame}",
                        a.u32(off)
                    ));
                }
            }
        }
    }
    println!("{n_articles} item articles, {n_scripts} scripts (their states' and ALDYakuAll's)");
    println!("opcode: commands");
    for op in 10..26 {
        let n = counts.get(&op).copied().unwrap_or(0);
        println!(
            "  {op}: {n}{}",
            if n == 0 { "  none on the disc" } else { "" }
        );
    }
    for h in hits {
        println!("{h}");
    }
}

/// The stages' color animations: the scripts `grMaterial_801C9604` starts on a stage's
/// pieces, each a word of the stage file's `yakumono_param` (grbattle.c's
/// `bg_curr_color_overlay` and `bg_prev_color_overlay`, grcastle.c's `x114`, grcorneria.c's
/// `x84`, grkongo.c's `unk84`, grlast.c's `x0` to `xC`, grmutecity.c's `x0` and `x4`,
/// groldkongo.c's `x6C`, grshrineroute.c's `x0` to `xC`, grvenom.c's `x38`), with their
/// commands and any owner's command (21 and up), which `grMaterial_801C9698` hands to
/// `fn_801C9664`.
fn stage_colanims(disc: &Disc, files: &[String]) {
    const SCRIPTS: [(&str, &[u32]); 9] = [
        ("GrNBa.dat", &[0x0, 0x4]),
        ("GrCs.dat", &[0x114]),
        ("GrCn.dat", &[0x84]),
        ("GrKg.dat", &[0x84]),
        ("GrNLa.dat", &[0x0, 0x4, 0x8, 0xC]),
        ("GrMc.dat", &[0x0, 0x4]),
        ("GrOk.dat", &[0x6C]),
        ("GrNSr.dat", &[0x0, 0x4, 0x8, 0xC]),
        ("GrVe.dat", &[0x38]),
    ];
    let mut owner_ops = BTreeSet::new();
    for (file, fields) in SCRIPTS {
        let Some(path) = files.iter().find(|p| p.rsplit('/').next() == Some(file)) else {
            println!("{file}: not on the disc");
            continue;
        };
        let bytes = disc.read_file(path).expect("the stage file");
        let a = Archive::parse(&bytes).expect("a DAT archive");
        let Some(root) = a.public("yakumono_param") else {
            println!("{file}: no yakumono_param");
            continue;
        };
        for &field in fields {
            let Some(script) = a.ptr(root + field) else {
                println!("{file} yakumono_param {field:#x}: no script");
                continue;
            };
            let w = walk(&a, script, &color_command_words);
            let mut ops: Vec<u32> = w.commands.iter().map(|c| c.0).collect();
            ops.extend(w.bad.iter().map(|b| b.1));
            ops.sort_unstable();
            ops.dedup();
            let owner: Vec<String> = w
                .bad
                .iter()
                .filter(|(_, op)| *op != 10)
                .map(|(at, op)| format!("{op} at {at:#x}"))
                .collect();
            owner_ops.extend(w.bad.iter().map(|b| b.1).filter(|&o| o != 10));
            println!(
                "{file} yakumono_param {field:#x}: script at {script:#x}, commands {ops:?} \
                 (10 ends it), owner's: {}",
                if owner.is_empty() {
                    "none".to_owned()
                } else {
                    owner.join(", ")
                }
            );
        }
    }
    println!("owner's commands in the stages' scripts: {owner_ops:?}");
}

/// Every reference object (`HSD_RObjDesc`) on the disc, by type. `HSD_RObjLoadDesc` takes them
/// only from a joint's descriptor (`HSD_JObjDesc::robjdesc`, +0x3C, in `JObjLoad`) and a camera
/// position's (`HSD_WObjDesc::robjdesc`, +0x10, in `WObjLoad` and `HSD_WObjInit`), so this
/// finds every object shaped like either (where something points or a symbol names: +0x3C a
/// pointer in an object of 0x40 bytes or more with no pointer in its vectors; +0x10 a pointer
/// in one of 0x14 or more with none in its position) and follows each chain (`next`, +0). An
/// expression (type 0, `REFTYPE_EXP`) is the only kind `expLoadDesc` gives `dummy_func`, when
/// its descriptor's function is null, which in a file it always is.
fn robjs(disc: &Disc, files: &[String]) {
    const TYPES: [&str; 5] = ["expression", "joint", "limit", "bytecode", "IK hint"];
    let mut counts = [[0u32; 5]; 2];
    let (mut joints, mut wobjs, mut bad, mut archives) = (0u32, 0u32, 0u32, 0u32);
    for path in files {
        let Ok(bytes) = disc.read_file(path) else {
            continue;
        };
        let Ok(a) = Archive::parse(&bytes) else {
            continue;
        };
        archives += 1;
        let starts = object_starts(&a);
        let size = |t: u32| {
            starts
                .range(t + 1..)
                .next()
                .copied()
                .unwrap_or(a.data().len() as u32)
                - t
        };
        let null_or_ptr = |o: u32| a.u32(o) == 0 || a.is_pointer(o);
        let no_ptrs = |from: u32, to: u32| (from..to).step_by(4).all(|o| !a.is_pointer(o));
        let mut heads = BTreeMap::new();
        for &s in &starts {
            if size(s) >= 0x40
                && a.is_pointer(s + 0x3C)
                && null_or_ptr(s)
                && null_or_ptr(s + 8)
                && null_or_ptr(s + 0xC)
                && no_ptrs(s + 0x14, s + 0x38)
            {
                joints += 1;
                heads.insert(a.u32(s + 0x3C), 0);
            } else if size(s) >= 0x14
                && a.is_pointer(s + 0x10)
                && null_or_ptr(s)
                && no_ptrs(s + 4, s + 0x10)
            {
                wobjs += 1;
                heads.entry(a.u32(s + 0x10)).or_insert(1);
            }
        }
        let mut seen = BTreeSet::new();
        for (h, from) in heads {
            let mut r = Some(h);
            while let Some(at) = r {
                if !seen.insert(at) || at as usize + 0xC > a.data().len() {
                    break;
                }
                let flags = a.u32(at + 4);
                let ty = ((flags >> 28) & 7) as usize;
                let u = a.u32(at + 8);
                let valid = !a.is_pointer(at + 4)
                    && null_or_ptr(at)
                    && ty < 5
                    && (ty == 2) != a.is_pointer(at + 8)
                    && match ty {
                        // An expression updates the joint property its low bits name, from
                        // an `HSD_ExpDesc` (a function, null in a file, and its values).
                        0 => a.u32(u) == 0 && null_or_ptr(u + 4),
                        3 => a.is_pointer(u),
                        _ => true,
                    };
                if !valid {
                    bad += 1;
                    break;
                }
                counts[from][ty] += 1;
                if ty == 0 {
                    println!(
                        "{path} {at:#x} (from a {}): an expression, flags {flags:#010x}, its \
                         desc {u:#x}, values {:#x}",
                        ["joint", "camera position"][from],
                        a.u32(u + 4)
                    );
                }
                r = a.ptr(at);
            }
        }
    }
    println!(
        "{archives} archives, {joints} joint descs with a reference list, {wobjs} camera \
         position descs with one"
    );
    for (ty, name) in TYPES.iter().enumerate() {
        println!(
            "type {ty} ({name}): {} from joints, {} from camera positions",
            counts[0][ty], counts[1][ty]
        );
    }
    println!("{bad} chains end in something not shaped like a reference");
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else {
        eprintln!("usage: melee-disc-scan fighter-commands [OPCODE...]");
        return ExitCode::FAILURE;
    };
    let disc_path = std::env::var("SSBM_DISC").expect("SSBM_DISC names the disc image");
    let disc = Disc::open(&disc_path).expect("the disc opens");
    let files = disc.files().expect("the disc's files");
    match cmd.as_str() {
        "fighter-commands" => {
            let want: BTreeSet<u32> = args[1..]
                .iter()
                .map(|s| s.parse().expect("an opcode"))
                .collect();
            let (found, problems, scripts) = fighter_commands(&disc, &files);
            let files: BTreeSet<&str> = found.iter().map(|f| f.file.as_str()).collect();
            println!(
                "{scripts} scripts in {} fighter files ({})",
                files.len(),
                files.iter().copied().collect::<Vec<_>>().join(" ")
            );
            for p in &problems {
                match p.strip_prefix("table: ") {
                    Some(t) => println!("{t}"),
                    None => println!("not decoded: {p}"),
                }
            }
            let mut by_op: BTreeMap<u32, BTreeSet<(&str, &str, &str, u32)>> = BTreeMap::new();
            for f in &found {
                by_op
                    .entry(f.op)
                    .or_default()
                    .insert((&f.file, &f.root, f.table, f.entry));
            }
            println!("opcode: scripts holding it (commands)");
            for op in 10..10 + FIGHTER_COMMAND_WORDS.len() as u32 {
                let n = found.iter().filter(|f| f.op == op).count();
                let s = by_op.get(&op).map_or(0, BTreeSet::len);
                println!(
                    "  {op}: {s} ({n}){}",
                    if s == 0 { "  none on the disc" } else { "" }
                );
            }
            for op in want {
                println!("opcode {op}:");
                for f in found.iter().filter(|f| f.op == op) {
                    println!(
                        "  {} {} {}[{}] {} at {:#x} ({:#010x}), frame {}",
                        f.file, f.root, f.table, f.entry, f.anim, f.offset, f.word, f.frame
                    );
                }
            }
        }
        "item-hitboxes" => item_hitboxes(&disc, &files),
        "item-commands" => {
            let want: BTreeSet<u32> = args[1..]
                .iter()
                .map(|s| s.parse().expect("an opcode"))
                .collect();
            item_commands(&disc, &files, &want);
        }
        "item-colanims" => colanims(&disc, &files, "ItCo", "itPublicData", 0x14),
        "robjs" => robjs(&disc, &files),
        "stage-colanims" => stage_colanims(&disc, &files),
        "colanims" => {
            // `colanims FILE ROOT FIELD`: any color animation table.
            let field = u32::from_str_radix(args[3].trim_start_matches("0x"), 16).expect("a field");
            colanims(&disc, &files, &args[1], &args[2], field);
        }
        "publics" => {
            // `publics PATTERN`: the public symbols containing PATTERN in every file.
            for path in &files {
                let Ok(bytes) = disc.read_file(path) else {
                    continue;
                };
                let Ok(a) = Archive::parse(&bytes) else {
                    continue;
                };
                for (n, o) in a.publics() {
                    if n.contains(args[1].as_str()) {
                        println!("{path} {n} {o:#x}");
                    }
                }
            }
        }
        "words" => {
            // `words FILE OFFSET COUNT`: a file's data words, pointers marked, for checking a
            // scan's finding by hand.
            let bytes = disc.read_file(&args[1]).expect("the file");
            let a = Archive::parse(&bytes).expect("a DAT archive");
            let at = u32::from_str_radix(args[2].trim_start_matches("0x"), 16).expect("an offset");
            let n: u32 = args[3].parse().expect("a count");
            for i in 0..n {
                let o = at + 4 * i;
                let p = if a.is_pointer(o) { " (pointer)" } else { "" };
                println!("{o:#07x}: {:#010x}{p}", a.u32(o));
            }
        }
        other => {
            eprintln!("unknown scan {other}");
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}
