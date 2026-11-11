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
/// timers before it on the way, loops taken once); `words` gives an owner command's length.
fn walk(a: &Archive, start: u32, words: &dyn Fn(u32) -> Option<u32>) -> Walk {
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
                _ => match words(op) {
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
    let words = |op: u32| {
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
