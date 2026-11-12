//! `melee-oracle birdo-sweep INPUTS [--snaps N] [--snap-gap TICKS] [--x0 X] [--x1 X] [--dx D]
//! [--mirror] [--angles all|A,B,...]`: Birdo's flight once knocked off bouncing away (state 10,
//! `itkyasarin.c`) through melee-hd's own code on Mushroom Kingdom II's collision, to show how
//! many times she can touch the stage, where a single-player mode's trophy roll (`it_8027CC88`)
//! waits for her sixth (milestone 6's task 13; docs/oracle.md, "Training's trophy roll").
//!
//! INPUTS is a session that reaches a match on Mushroom Kingdom II with Birdo coming in (task
//! 13's `m6-training-gate/tg-base`: Training, nothing pressed in the match). The session plays
//! without its display pass to Birdo standing at her mark, then `--snaps` snapshots are taken
//! `--snap-gap` ticks apart (the logs and Pidgit's carpet elsewhere in each). From each, for
//! each launch angle (`--angles`: every nine-bit angle 0 to 511 and -1 for a hit with no
//! knockback by default), each way the hit faces her, and each x from `--x0` to `--x1` by `--dx`
//! at her height (negated with `--mirror`, the right side), she is moved there and given a hit
//! as the hit detection leaves one (`xCA0`, `xCAC_angle`, `xCC8_knockback`,
//! `xCCC_incDamageDirection`), which the item's reaction process hands to her damage callback
//! (`it_802EDDC0`) in a tick of the whole scene; the toward-screen draw is moved on until she
//! bounces away. Then her own processes run, the rest of the scene held, until she's gone or
//! flies away from the box round every line of the stage: her touches (`x14[1]`, a sixth
//! counted where she's gone inside the blast zones). Prints the most touches per angle and the
//! overall most.
//!
//! `melee-oracle birdo-sweep --hit-angles` lists the knockback angles the disc's hitboxes give:
//! every fighter's action scripts' hitbox commands and every character item's state scripts'
//! (each `Pl*.dat`'s `ftData`), which with the common items' and the Pokémon's (`ItCo.usd`, read
//! from a session's match with `INPUTS --item-angles`) are every launch a hit can give an item.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::ExitCode;

use melee_data::disc::Disc;
use melee_sim::game::it::{self, item as gi, kinds};
use melee_sim::game::session::{Session, Setup};
use melee_sim::game::state::{Game, Running};
use melee_sim::input::controller::PadData;
use melee_sim::input::pad::PadStatus;

use crate::session::{Console, Recording};

/// One tick of the session, its first controller held at `pad` (none: the recording's).
fn step(s: &mut Session, reads: &mut Vec<PadData>, disc: &Disc, next: &mut usize, pad: PadStatus) {
    if reads.len() < *next + 4 {
        let last = *reads.last().expect("reads");
        reads.resize(*next + 4, last);
    }
    for i in 0..4 {
        reads[*next + i][0] = pad;
    }
    let mut c = Console {
        reads,
        next: *next,
        disc,
        mutate: None,
    };
    let _ = s.step_tick(&mut c);
    *next = c.next;
}

/// Birdo's item: its GObj, place, state and touches so far.
fn birdo(g: &Game) -> Option<(melee_sim::hsd::gobj::GObjId, f32, f32, i32, i32)> {
    g.items
        .iter()
        .find(|(_, it)| it.kind == kinds::KYASARIN)
        .map(|(_, it)| {
            let touches = match &it.xDD4_itemVar.member {
                it::Member::Kyasarin(v) => *v.x14[1].peek(),
                _ => -1,
            };
            (
                it.entity.expect("Birdo's GObj"),
                it.pos.x,
                it.pos.y,
                it.msid,
                touches,
            )
        })
}

fn arg<T: std::str::FromStr>(args: &[&str], name: &str, d: T) -> T {
    args.iter()
        .position(|a| *a == name)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(d)
}

/// The angles in the scripts at `script` (hitbox commands, opcode 11: the angle in the fourth
/// word's top nine bits), read word by word to the end command.
fn script_angles(a: &melee_data::archive::Archive, script: u32, out: &mut BTreeSet<u32>) {
    let mut k = script;
    for _ in 0..400 {
        if k as usize + 20 > a.data().len() {
            return;
        }
        let w = a.u32(k);
        match w >> 26 {
            0 => return,
            11 => {
                out.insert(a.u32(k + 12) >> 23);
                k += 20;
            }
            _ => k += 4,
        }
    }
}

fn hit_angles() -> ExitCode {
    use melee_data::archive::Archive;
    let disc = Disc::open(std::env::var("SSBM_DISC").expect("SSBM_DISC")).expect("disc");
    let mut all = BTreeSet::new();
    for f in disc.files().expect("files") {
        let name = f.rsplit('/').next().unwrap_or(&f).to_string();
        if !(name.starts_with("Pl") && name.len() == 8 && name.ends_with(".dat")) {
            continue;
        }
        let bytes = disc.read_file(&f).expect("a fighter's file");
        let Ok(a) = Archive::parse(&bytes) else {
            continue;
        };
        let Some(d) = a
            .publics()
            .find(|(n, _)| n.starts_with("ftData"))
            .map(|(_, o)| o)
        else {
            continue;
        };
        let mut set = BTreeSet::new();
        if let Some(table) = a.ptr(d + 0xC) {
            for i in 0..500u32 {
                let e = table + 0x18 * i;
                if e as usize + 0x18 > a.data().len() {
                    break;
                }
                if let Some(sc) = a.ptr(e + 0xC) {
                    script_angles(&a, sc, &mut set);
                }
            }
        }
        if let Some(items) = a.ptr(d + 0x48) {
            for i in 0..24u32 {
                let Some(states) = a.ptr(items + 4 * i).and_then(|art| a.ptr(art + 0xC)) else {
                    continue;
                };
                for j in 0..48u32 {
                    if let Some(sc) = a.ptr(states + 0x10 * j + 0xC) {
                        script_angles(&a, sc, &mut set);
                    }
                }
            }
        }
        println!("{name}: {set:?}");
        all.extend(set);
    }
    println!("all: {all:?}");
    ExitCode::SUCCESS
}

pub fn birdo_sweep(args: &[&str]) -> ExitCode {
    if args.first() == Some(&"--hit-angles") {
        return hit_angles();
    }
    let Some(&inputs_path) = args.first() else {
        eprintln!("usage: melee-oracle birdo-sweep INPUTS [--item-angles] [--snaps N] ...");
        return ExitCode::from(2);
    };
    let rec = Recording::read(Path::new(inputs_path)).expect("the recording");
    let card = rec
        .card
        .clone()
        .map(|p| std::fs::read(p).expect("the card"));
    let setup = Setup {
        disc: rec.disc().expect("the disc's pace"),
        dsp: rec.dsp(),
        card,
        power_on: rec.power_on,
        dtv: rec.dtv,
        sram_flags: rec.sram_flags,
        card_pull: None,
        card_b: None,
        card_b_pull: None,
        develop: rec.developer(),
    };
    let disc = Disc::open(std::env::var("SSBM_DISC").expect("SSBM_DISC")).expect("the disc");
    let mut reads = rec.reads.clone();
    let mut s = Session::new(setup);
    let mut next = 0;
    {
        let mut c = Console {
            reads: &reads,
            next: 0,
            disc: &disc,
            mutate: None,
        };
        s.boot(&mut c);
        next = next.max(c.next);
    }
    // To the match, then the recording's controllers, still.
    loop {
        let p = reads[next.min(reads.len() - 1)][0];
        step(&mut s, &mut reads, &disc, &mut next, p);
        let g = &s.game;
        if g.fighters.used() > 0 && g.stage_info.grkind == 0x19 && g.player_entity(0).is_some() {
            break;
        }
        assert!(next < rec.reads.len(), "no match on Mushroom Kingdom II");
    }
    let still = {
        let mut p = reads[next][0];
        p.button = 0;
        p.stick_x = 0;
        p.stick_y = 0;
        p.substick_x = 0;
        p.substick_y = 0;
        p.trigger_left = 0;
        p.trigger_right = 0;
        p
    };
    if args.contains(&"--item-angles") {
        let mut all = BTreeSet::new();
        for kind in 0..kinds::OLD_KURI {
            let Some(a) = gi::article_of(&s.game, &s.data, kind) else {
                continue;
            };
            let Some(states) = s.data.ptr(a.at(0xC)) else {
                continue;
            };
            let mut set = BTreeSet::new();
            for i in 0..48u32 {
                let Some(mut k) = s.data.ptr(states.at(0x10 * i + 0xC)) else {
                    continue;
                };
                for _ in 0..300 {
                    let w = s.data.u32(k);
                    match w >> 26 {
                        0 => break,
                        11 => {
                            set.insert(s.data.u32(k.at(12)) >> 23);
                            k = k.at(20);
                        }
                        _ => k = k.at(4),
                    }
                }
            }
            if !set.is_empty() {
                println!("kind {kind}: {set:?}");
            }
            all.extend(set);
        }
        println!("all: {all:?}");
        return ExitCode::SUCCESS;
    }
    let snaps: u32 = arg(args, "--snaps", 3);
    let snap_gap: u32 = arg(args, "--snap-gap", 150);
    let x0: f32 = arg(args, "--x0", -179.4);
    let x1: f32 = arg(args, "--x1", -79.8);
    let dx: f32 = arg(args, "--dx", 0.4);
    let mirror = args.contains(&"--mirror");
    let angles: Vec<i32> = match args.iter().position(|a| *a == "--angles") {
        Some(i) if args.get(i + 1) != Some(&"all") => args[i + 1]
            .split(',')
            .map(|t| t.parse().expect("an angle"))
            .collect(),
        _ => (-1..512).collect(),
    };
    loop {
        step(&mut s, &mut reads, &disc, &mut next, still);
        if birdo(&s.game).is_some_and(|b| b.3 == 6) {
            break;
        }
    }
    let (left, right, top, bottom) = {
        use melee_sim::game::gr::stage::*;
        let g = &s.game;
        (blast_left(g), blast_right(g), blast_top(g), blast_bottom(g))
    };
    let mut snapshots = Vec::new();
    for _ in 0..snaps {
        snapshots.push((s.game.clone(), s.card.clone(), next));
        for _ in 0..snap_gap {
            step(&mut s, &mut reads, &disc, &mut next, still);
        }
        if !birdo(&s.game).is_some_and(|b| b.3 == 6 || b.3 == 7) {
            break;
        }
    }
    eprintln!("{} snapshots", snapshots.len());
    let mut overall = (0, 0, 0.0f32, 0.0f32, 0usize);
    let mut cases = 0u64;
    for &a in &angles {
        let mut best = (0, 0.0f32, 0.0f32, 0usize);
        for dir in [-1.0f32, 1.0] {
            for (si, (g0, c0, n0)) in snapshots.iter().enumerate() {
                let mut x = x0;
                while x <= x1 + 1e-3 {
                    let px = if mirror { -x } else { x };
                    let mut tries = 0;
                    let gobj = loop {
                        s.game.clone_from(g0);
                        s.card.clone_from(c0);
                        next = *n0;
                        for _ in 0..tries {
                            s.game.objs.rng.draw_f32();
                        }
                        let gobj = birdo(&s.game).expect("Birdo").0;
                        let ip = it::item_mut(&mut s.game, gobj);
                        ip.pos.x = px;
                        ip.xCA0 = 1000;
                        ip.xCAC_angle = a.max(0);
                        ip.xCC8_knockback = if a < 0 { 0.0 } else { 50.0 };
                        ip.xCCC_incDamageDirection = dir;
                        step(&mut s, &mut reads, &disc, &mut next, still);
                        if birdo(&s.game).is_some_and(|b| b.3 == 10) {
                            break gobj;
                        }
                        tries += 1;
                        assert!(tries < 200, "Birdo never bounces away");
                    };
                    let mut most = 0;
                    let mut last = (px, 0.0f32);
                    for _ in 0..600 {
                        for proc in [
                            gi::PROC_HITLAG,
                            gi::PROC_ANIM,
                            gi::PROC_PHYS,
                            gi::PROC_COLL,
                            gi::PROC_ACCESSORY,
                            gi::PROC_JUMPED_ON,
                            gi::PROC_GRAB,
                            gi::PROC_HIT,
                            gi::PROC_REACT,
                            gi::PROC_DYNAMICS,
                        ] {
                            if birdo(&s.game).is_none() {
                                break;
                            }
                            let lent = melee_sim::hsd::heap::Lent::take(&mut s.game.objs.heap);
                            {
                                let mut r = Running {
                                    game: &mut s.game,
                                    data: &s.data,
                                };
                                gi::run_proc(&mut r, gobj, proc);
                            }
                            lent.give_back(&mut s.game.objs.heap);
                        }
                        match birdo(&s.game) {
                            Some((_, bx, by, 10, n)) => {
                                most = most.max(n);
                                last = (bx, by);
                                let v = it::item(&s.game, gobj).x40_vel;
                                if (bx < -310.0 && v.x <= 0.0)
                                    || (bx > 310.0 && v.x >= 0.0)
                                    || (by > 65.0 && v.y >= 0.0)
                                    || (by < -185.0 && v.y <= 0.0)
                                {
                                    break;
                                }
                            }
                            Some(_) => break,
                            None => {
                                if last.0 > left + 10.0
                                    && last.0 < right - 10.0
                                    && last.1 > bottom + 10.0
                                    && last.1 < top - 10.0
                                {
                                    most = 6;
                                }
                                break;
                            }
                        }
                    }
                    cases += 1;
                    if most > best.0 {
                        best = (most, px, dir, si);
                    }
                    x += dx;
                }
            }
        }
        println!(
            "angle {a}: most touches {} (x {:.1}, facing {}, snapshot {})",
            best.0, best.1, best.2, best.3
        );
        if best.0 > overall.0 {
            overall = (best.0, a, best.1, best.2, best.3);
        }
    }
    println!(
        "{cases} cases; most touches {} (angle {}, x {:.1}, facing {}, snapshot {})",
        overall.0, overall.1, overall.2, overall.3, overall.4
    );
    ExitCode::SUCCESS
}
