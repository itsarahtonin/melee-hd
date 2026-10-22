//! The launcher's CPU players: each port human or a computer player at a level, 1 to 9, as VS
//! mode's character select sets them, offered as the launcher offers stages and items. A CPU,
//! level or stage is offered only once the CPU audit (`melee_sim::game::cpu_audit`) finds nothing
//! the choice brings into the match unported, and a match starts only when the audit finds
//! nothing at all, Nana's computer player included in every match with the Ice Climbers.

use melee_sim::game::cpu_audit::{self, Computer, CpuGap, Items};
use melee_sim::game::roster;
use melee_sim::game::vs_match::Player;

/// `Gm_PKind`: a human player, a computer player.
pub const HUMAN: u8 = 0;
pub const COMPUTER: u8 = 1;

/// The VS levels the character select screen's slider sets.
pub const LEVELS: std::ops::RangeInclusive<u8> = 1..=9;

/// The match's players from each port's character and costume (0xFF: none) and CPU level
/// (none: a human), as the character select screen leaves VS mode's start data: a computer
/// player is `Gm_PKind` computer at its level, and keeps the rest of its slot's defaults
/// (`gm_SetupPlayerDefaults`: computer kind 4, no name tag, which mnCharSel sets again when a
/// door turns to CPU), so VS mode's setup (`Settings::vs_with`) makes its block as a console's
/// menus do; a human keeps level 0.
pub fn players(ports: &[(u8, u8)], cpus: &[Option<u8>]) -> Vec<Player> {
    (ports.iter().enumerate())
        .filter(|(_, p)| p.0 != 0xFF)
        .map(|(i, &(character, costume))| {
            let level = cpus.get(i).copied().flatten();
            Player {
                character,
                costume,
                port: i as u8,
                kind: if level.is_some() { COMPUTER } else { HUMAN },
                cpu_level: level.unwrap_or(0),
                team: i as u8,
            }
        })
        .collect()
}

/// The VS computer players of ports with these characters and CPU levels.
pub fn computers(players: &[Option<u8>], cpus: &[Option<u8>]) -> Vec<Computer> {
    (players.iter().zip(cpus))
        .filter_map(|(p, l)| {
            Some(Computer {
                character: (*p)?,
                level: (*l)?,
            })
        })
        .collect()
}

/// The CPU audit's gaps a match reaches with these ports' characters and CPU levels on `stage`
/// with the item kinds it gets (none: items off): its computer players' and, with the Ice
/// Climbers, Nana's, whether Popo is human or not (`cpu_audit::missing_with_switch`).
pub fn gaps(
    players: &[Option<u8>],
    cpus: &[Option<u8>],
    stage: u16,
    kinds: &[u8],
) -> Vec<&'static CpuGap> {
    let characters: Vec<u8> = players.iter().flatten().copied().collect();
    let kinds: Vec<i32> = kinds.iter().map(|&k| i32::from(k)).collect();
    cpu_audit::missing_with_switch(&computers(players, cpus), &characters, stage, &kinds)
}

/// Why a match can't start for its computer players: what the CPU audit finds it reaches.
pub fn refusal(
    players: &[Option<u8>],
    cpus: &[Option<u8>],
    stage: u16,
    kinds: &[u8],
) -> Option<String> {
    let gaps = gaps(players, cpus, stage, kinds);
    if gaps.is_empty() {
        return None;
    }
    let who = if computers(players, cpus).is_empty() {
        "Nana, the Ice Climbers' computer partner,"
    } else {
        "A computer player"
    };
    let whats: Vec<&str> = gaps.iter().map(|g| g.what).collect();
    Some(format!(
        "{who} can stop mid-match: not ported yet: {}",
        whats.join("; ")
    ))
}

/// Whether the launcher offers a choice: offered, or the gaps it brings into the match and
/// why in a few words.
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Offered,
    Not(String, Vec<&'static CpuGap>),
}

impl Status {
    /// The reason in a few words.
    pub fn short(&self) -> &str {
        match self {
            Status::Offered => "",
            Status::Not(why, _) => why,
        }
    }

    /// The reason with what isn't ported.
    pub fn detail(&self) -> String {
        match self {
            Status::Offered => String::new(),
            Status::Not(why, gaps) => {
                let whats: Vec<&str> = gaps.iter().map(|g| g.what).collect();
                format!(
                    "{}: {}",
                    crate::settings::capitalized(why),
                    whats.join("; ")
                )
            }
        }
    }
}

/// Whether port `port` may be a computer player at `level` (none: human, always offered),
/// the other ports, the stage and the item kinds as they are: offered when it brings no gap the
/// match with the port human doesn't reach. A gap the match reaches anyway is the other
/// choices', and refuses the match under Start.
pub fn control_status(
    players: &[Option<u8>; 4],
    cpus: &[Option<u8>; 4],
    stage: u16,
    kinds: &[u8],
    port: usize,
    level: Option<u8>,
) -> Status {
    let Some(level) = level else {
        return Status::Offered;
    };
    let mut with = *cpus;
    with[port] = Some(level);
    let mut without = *cpus;
    without[port] = None;
    let base = gaps(players, &without, stage, kinds);
    let brought: Vec<&'static CpuGap> = (gaps(players, &with, stage, kinds).into_iter())
        .filter(|g| !base.contains(g))
        .collect();
    if brought.is_empty() {
        return Status::Offered;
    }
    let every = brought.iter().any(|g| {
        g.levels.is_empty()
            && g.cpu.is_empty()
            && g.with.is_empty()
            && g.stages.is_empty()
            && g.items == Items::Off
    });
    if every {
        return Status::Not("not ported yet".into(), brought);
    }
    let mut who = Vec::new();
    if brought.iter().any(|g| g.levels.contains(&level)) {
        who.push(format!("at level {level}"));
    }
    if let Some(c) = players[port]
        && brought.iter().any(|g| g.cpu.contains(&c))
    {
        let name = roster::character_by_kind(c).map_or("?", |c| c.name);
        who.push(format!("as {name}"));
    }
    let mut names: Vec<&str> = Vec::new();
    for g in &brought {
        for c in g.with.iter().filter(|c| players.contains(&Some(**c))) {
            let name = roster::character_by_kind(*c).map_or("?", |c| c.name);
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    if !names.is_empty() {
        who.push(format!("with {}", names.join(" or ")));
    }
    if brought.iter().any(|g| g.stages.contains(&stage)) {
        let name = roster::stage_by_kind(stage).map_or("this stage", |s| s.name);
        who.push(format!("on {name}"));
    }
    if brought.iter().any(|g| g.items != Items::Off) {
        who.push("with these items".into());
    }
    Status::Not(format!("not {} yet", who.join(", not ")), brought)
}

/// Whether the launcher offers `stage` for a match with these ports: offered when it brings
/// none of the CPU audit's stage gaps. Only asked with a computer player in the match: the
/// stage audits already refuse Nana's stage gaps with the Ice Climbers.
pub fn stage_status(
    players: &[Option<u8>; 4],
    cpus: &[Option<u8>; 4],
    stage: u16,
    kinds: &[u8],
) -> Status {
    let brought: Vec<&'static CpuGap> = (gaps(players, cpus, stage, kinds).into_iter())
        .filter(|g| !g.stages.is_empty())
        .collect();
    if brought.is_empty() {
        Status::Offered
    } else {
        Status::Not("not with CPU players yet".into(), brought)
    }
}

/// A port's control as the form names it.
pub fn control_name(level: Option<u8>) -> String {
    match level {
        None => "Human".into(),
        Some(l) => format!("CPU {l}"),
    }
}

/// A port's control as the settings folder keeps it.
pub fn save(level: Option<u8>) -> String {
    level.map_or_else(|| "off".into(), |l| l.to_string())
}

/// Takes back a port's control [`save`] wrote; none for a value it can't read.
pub fn restore(v: &str) -> Option<Option<u8>> {
    match v {
        "off" => Some(None),
        v => v.parse().ok().filter(|l| LEVELS.contains(l)).map(Some),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use melee_sim::game::gm::vs_mode::{self, GamePrefs, GameRules};
    use melee_sim::game::vs_match::Settings;

    /// A computer player's block is the character select screen's: `Gm_PKind` computer (1),
    /// computer kind 4, its level, no name tag, no rumble, and every other field a human's
    /// would have in the same slot, as `gm_SetupPlayerDefaults` and mnCharSel leave it.
    #[test]
    fn a_cpu_slot_is_the_menus() {
        let fd = roster::stage("fd").unwrap().kind;
        let (fox, marth) = (2, 9);
        for level in LEVELS {
            let ports = [(fox, 0), (marth, 0), (0xFF, 0), (fox, 1)];
            let cpus = [None, Some(level), None, Some(10 - level)];
            let p = players(&ports, &cpus);
            assert_eq!(p.len(), 3);
            let s = Settings::vs(fd, &p, 4, 8).start;
            let human = Settings::vs(fd, &players(&ports, &[None; 4]), 4, 8).start;
            for (port, cpu) in [(1, level), (3, 10 - level)] {
                let d = &s.players[port];
                assert_eq!(d.slot_type, COMPUTER);
                assert_eq!(d.cpu_kind, 4);
                assert_eq!(d.cpu_level, cpu);
                assert_eq!(d.nametag, vs_mode::player_defaults().nametag);
                assert!(!d.rumble_enabled);
                let mut h = human.players[port].clone();
                (h.slot_type, h.cpu_level, h.rumble_enabled) = (COMPUTER, cpu, false);
                assert_eq!(*d, h, "port {port}");
            }
            assert_eq!(s.players[0], human.players[0]);
            assert_eq!(s.players[0].slot_type, HUMAN);
            assert_eq!(s.players[2].slot_type, 3, "an empty slot");
        }
    }

    /// The launcher's computer players' blocks are the oracle's CPU matches' (`melee-bench`'s
    /// `cpu` set, recorded from the start data `player_block` makes as ssbm-rs played it), but
    /// for the spawn point, which the match picks for a menus' match (-1), and the team, which
    /// the launcher numbers by port. Skips without `SSBM_RS_LOCAL` or its replays.
    #[test]
    fn cpu_slots_match_the_oracles() {
        let Some(local) = std::env::var_os("SSBM_RS_LOCAL") else {
            eprintln!("skipped: SSBM_RS_LOCAL isn't set");
            return;
        };
        let dir = std::path::Path::new(&local).join("bench/oracle-m4");
        let Ok(entries) = std::fs::read_dir(&dir) else {
            eprintln!("skipped: no {}", dir.display());
            return;
        };
        let mut names: Vec<_> = (entries.flatten())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("cpu-") && n.ends_with(".slp"))
            .collect();
        names.sort();
        let mut slots = 0;
        for name in &names {
            let bytes = std::fs::read(dir.join(name)).unwrap();
            let replay = melee_slippi::Replay::parse(&bytes).unwrap();
            let rec = Settings::from_replay(&melee_slippi::start(&replay))
                .unwrap()
                .start;
            let mut ports = [(0xFF, 0); 4];
            let mut cpus = [None; 4];
            for (i, p) in rec.players.iter().take(4).enumerate() {
                if p.slot_type != 3 {
                    ports[i] = (p.ckind as u8, p.color);
                }
                if p.slot_type == COMPUTER {
                    assert_eq!(p.cpu_kind, 4, "{name}: a VS CPU");
                    cpus[i] = Some(p.cpu_level);
                }
            }
            let rules = GameRules::vs(rec.players[0].stocks as u8, 3);
            let ours = Settings::vs_with(
                rec.rules.stkind,
                &players(&ports, &cpus),
                &rules,
                &GamePrefs::items_off(),
            )
            .start;
            for i in (0..4).filter(|&i| cpus[i].is_some()) {
                let r = &rec.players[i];
                let mut o = ours.players[i].clone();
                (o.spawn_pos, o.team) = (r.spawn_pos, r.team);
                assert_eq!(&o, r, "{name}: port {}", i + 1);
                slots += 1;
            }
        }
        eprintln!(
            "{slots} CPU slots of {} oracle matches compared",
            names.len()
        );
    }

    /// No computer player and no Ice Climbers: the audit has nothing to say. Peach's pulled
    /// Beam Sword with the Ice Climbers, items off: Nana reaches it, human Popo too.
    #[test]
    fn nana_is_asked_in_every_ice_climbers_match() {
        let fd = roster::stage("fd").unwrap().kind;
        let (fox, marth, peach, ics) = (2, 9, 12, 14);
        let none = [None; 4];
        assert!(gaps(&[Some(fox), Some(marth), None, None], &none, fd, &[]).is_empty());
        let why = refusal(&[Some(ics), Some(peach), None, None], &none, fd, &[])
            .expect("Nana and Peach's sword");
        assert!(why.contains("Nana") && why.contains("Beam Sword"), "{why}");
        assert!(refusal(&[Some(ics), Some(fox), None, None], &none, fd, &[]).is_none());
    }

    /// A choice is turned off with the gaps it brings, named in a few words; Human always
    /// stays offered.
    #[test]
    fn choices_name_what_they_bring() {
        let fd = roster::stage("fd").unwrap().kind;
        let players = [Some(2), Some(9), None, None];
        let none = [None; 4];
        assert_eq!(
            control_status(&players, &none, fd, &[], 1, None),
            Status::Offered
        );
        for level in LEVELS {
            let s = control_status(&players, &none, fd, &[], 1, Some(level));
            let mut cpus = none;
            cpus[1] = Some(level);
            let reached = gaps(&players, &cpus, fd, &[]);
            match &s {
                Status::Offered => assert!(reached.is_empty()),
                Status::Not(why, brought) => {
                    assert!(!why.is_empty() && !brought.is_empty());
                    assert!(brought.iter().all(|g| reached.contains(g)));
                    assert!(s.detail().contains(brought[0].what));
                }
            }
        }
        // Peach in the match: the sword's gap comes with a CPU too, named by her.
        let with_peach = [Some(2), Some(12), None, None];
        if let Status::Not(why, brought) = control_status(&with_peach, &none, fd, &[], 0, Some(9))
            && brought.iter().all(|g| !g.with.is_empty())
        {
            assert!(why.contains("Peach"), "{why}");
        }
        assert_eq!(stage_status(&players, &none, fd, &[]), Status::Offered);
    }

    #[test]
    fn a_control_comes_back() {
        for level in [None, Some(1), Some(5), Some(9)] {
            assert_eq!(restore(&save(level)), Some(level));
        }
        for v in ["0", "10", "cpu", ""] {
            assert_eq!(restore(v), None, "{v}");
        }
    }
}
