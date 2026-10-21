//! The launcher's items: the item frequency and the item switch, as VS mode's Item Switch
//! screen sets them, offered kind by kind as the launcher offers stages. A kind is offered only
//! once the items audit (`melee_sim::game::items_audit`) finds nothing an items-on match with it
//! reaches unported, for the chosen stage and characters; a kind the choice makes unavailable is
//! off for that match, and the form says so. Items on with no kind offered is items off.

use melee_sim::game::gm::vs_mode::GamePrefs;
use melee_sim::game::items_audit::{self, ItemGap};
use melee_sim::game::roster;

/// An item switch kind (`ItemKind`), with its name and the settings file's key.
pub struct Kind {
    pub kind: u8,
    pub name: &'static str,
    pub key: &'static str,
}

/// The item switch's kinds, in kind order (the switch's own entries are
/// `vs_mode::ITEM_SWITCH_KINDS`).
pub const KINDS: &[Kind] = &[
    kind(0x04, "Party Ball", "party-ball"),
    kind(0x05, "Barrel Cannon", "barrel-cannon"),
    kind(0x06, "Bob-omb", "bob-omb"),
    kind(0x07, "Mr. Saturn", "mr-saturn"),
    kind(0x08, "Heart Container", "heart-container"),
    kind(0x09, "Maxim Tomato", "maxim-tomato"),
    kind(0x0A, "Starman", "starman"),
    kind(0x0B, "Home-Run Bat", "home-run-bat"),
    kind(0x0C, "Beam Sword", "beam-sword"),
    kind(0x0D, "Parasol", "parasol"),
    kind(0x0E, "Green Shell", "green-shell"),
    kind(0x0F, "Red Shell", "red-shell"),
    kind(0x10, "Ray Gun", "ray-gun"),
    kind(0x11, "Freezie", "freezie"),
    kind(0x12, "Food", "food"),
    kind(0x13, "Motion-Sensor Bomb", "motion-sensor-bomb"),
    kind(0x14, "Flipper", "flipper"),
    kind(0x15, "Super Scope", "super-scope"),
    kind(0x16, "Star Rod", "star-rod"),
    kind(0x17, "Lip's Stick", "lips-stick"),
    kind(0x18, "Fan", "fan"),
    kind(0x19, "Fire Flower", "fire-flower"),
    kind(0x1A, "Super Mushroom", "super-mushroom"),
    kind(0x1B, "Poison Mushroom", "poison-mushroom"),
    kind(0x1C, "Hammer", "hammer"),
    kind(0x1D, "Warp Star", "warp-star"),
    kind(0x1E, "Screw Attack", "screw-attack"),
    kind(0x1F, "Bunny Hood", "bunny-hood"),
    kind(0x20, "Metal Box", "metal-box"),
    kind(0x21, "Cloaking Device", "cloaking-device"),
    kind(0x22, "Poké Ball", "poke-ball"),
];

const fn kind(kind: u8, name: &'static str, key: &'static str) -> Kind {
    Kind { kind, name, key }
}

/// The item frequency of none (`GamePrefs::item_freq`).
pub const OFF: u8 = 0xFF;

/// The item frequencies (`GamePrefs::item_freq`), with their names and keys.
pub const FREQUENCIES: [(u8, &str, &str); 6] = [
    (OFF, "Off", "off"),
    (0, "Very low", "very-low"),
    (1, "Low", "low"),
    (2, "Medium", "medium"),
    (3, "High", "high"),
    (4, "Very high", "very-high"),
];

/// The launcher's item setting: the frequency and which kinds the switch has on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Switch {
    /// `GamePrefs::item_freq`: [`OFF`], or 0 to 4.
    pub freq: u8,
    /// Bit N for kind N.
    pub on: u64,
}

impl Default for Switch {
    /// Items off, the switch all on, as a new save has it.
    fn default() -> Self {
        Switch {
            freq: OFF,
            on: KINDS.iter().fold(0, |m, k| m | 1 << k.kind),
        }
    }
}

/// Whether the launcher offers a kind for a match.
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Offered,
    /// Something every match with the kind reaches isn't ported.
    NotPorted(Vec<&'static ItemGap>),
    /// Only the chosen characters or stage reach a gap: whom or where.
    NotWith(String, Vec<&'static ItemGap>),
}

impl Status {
    /// The reason in a few words.
    pub fn short(&self) -> String {
        match self {
            Status::Offered => String::new(),
            Status::NotPorted(_) => "not ported yet".into(),
            Status::NotWith(who, _) => format!("not {who}"),
        }
    }

    /// The reason with what isn't ported.
    pub fn detail(&self, kind: u8, characters: &[u8], stage: u16) -> String {
        let gaps = match self {
            Status::Offered => return String::new(),
            Status::NotPorted(g) | Status::NotWith(_, g) => g,
        };
        let whats: Vec<&str> = gaps.iter().map(|g| g.what).collect();
        let mut text = format!(
            "{}: {}",
            crate::settings::capitalized(&self.short()),
            whats.join("; ")
        );
        let alone = items_audit::missing_with(&[i32::from(kind)], characters, stage);
        if gaps.iter().any(|g| !alone.contains(g)) {
            text += " (through the capsules, boxes, barrels and eggs every items match has: the item switch can't turn them off)";
        }
        text
    }
}

/// Whether the launcher offers `kind` for a match on `stage` with `characters`
/// (`CharacterKind`s): offered when an items-on match from the menus with it on reaches no gap
/// (`items_audit::missing_with_switch`).
pub fn status(kind: u8, characters: &[u8], stage: u16) -> Status {
    let gaps = items_audit::missing_with_switch(&[i32::from(kind)], characters, stage);
    if gaps.is_empty() {
        return Status::Offered;
    }
    if gaps
        .iter()
        .any(|g| g.with.is_empty() && g.stages.is_empty())
    {
        return Status::NotPorted(gaps);
    }
    let mut names: Vec<&str> = Vec::new();
    for g in &gaps {
        for c in g.with.iter().filter(|c| characters.contains(c)) {
            let name = roster::character_by_kind(*c).map_or("?", |c| c.name);
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    let mut who = Vec::new();
    if !names.is_empty() {
        who.push(format!("with {}", names.join(" or ")));
    }
    if gaps.iter().any(|g| g.stages.contains(&stage)) {
        let name = roster::stage_by_kind(stage).map_or("this stage", |s| s.name);
        who.push(format!("on {name}"));
    }
    Status::NotWith(who.join(", not "), gaps)
}

/// What a match gets from the setting.
#[derive(Clone, Debug, PartialEq)]
pub struct ForMatch {
    /// The menus' preferences the match starts from.
    pub prefs: GamePrefs,
    /// The kinds the switch has on that the match gets; none: items off.
    pub kinds: Vec<u8>,
    /// The kinds the switch has on that aren't offered for the match, with why.
    pub dropped: Vec<(u8, Status)>,
}

impl Switch {
    pub fn is_on(&self, kind: u8) -> bool {
        self.on >> kind & 1 != 0
    }

    pub fn set(&mut self, kind: u8, on: bool) {
        if on {
            self.on |= 1 << kind;
        } else {
            self.on &= !(1 << kind);
        }
    }

    /// The match's items for `characters` on `stage`: the switch's kinds that are offered at
    /// the frequency, or items off when the frequency is off or none is left.
    pub fn for_match(&self, characters: &[u8], stage: u16) -> ForMatch {
        let mut out = ForMatch {
            prefs: GamePrefs::items_off(),
            kinds: Vec::new(),
            dropped: Vec::new(),
        };
        if self.freq == OFF {
            return out;
        }
        for k in KINDS.iter().filter(|k| self.is_on(k.kind)) {
            match status(k.kind, characters, stage) {
                Status::Offered => out.kinds.push(k.kind),
                s => out.dropped.push((k.kind, s)),
            }
        }
        let kinds: Vec<i32> = out.kinds.iter().map(|&k| i32::from(k)).collect();
        // Each kind is offered alone, and the gaps a set reaches are its kinds' together.
        if !kinds.is_empty()
            && items_audit::missing_with_switch(&kinds, characters, stage).is_empty()
        {
            out.prefs = GamePrefs::items(self.freq, |k| out.kinds.contains(&k));
        } else {
            out.kinds.clear();
        }
        out
    }

    /// What the form says about the match's items, when the setting gives it less than it asks
    /// for: items off for the match, or kinds the characters or stage turn off.
    pub fn note(&self, characters: &[u8], stage: u16) -> Option<String> {
        if self.freq == OFF {
            return None;
        }
        let m = self.for_match(characters, stage);
        if m.kinds.is_empty() && m.dropped.is_empty() {
            return Some("Items are off: the item switch has nothing on.".into());
        }
        // The kinds the characters or stage turn off, by whom or where.
        let mut reasons: Vec<(&str, Vec<&str>)> = Vec::new();
        for (kind, s) in &m.dropped {
            if let Status::NotWith(who, _) = s {
                let name = name(*kind);
                match reasons.iter_mut().find(|(r, _)| r == who) {
                    Some((_, names)) => names.push(name),
                    None => reasons.push((who, vec![name])),
                }
            }
        }
        if m.kinds.is_empty() {
            let who: Vec<&str> = reasons.iter().map(|(r, _)| *r).collect();
            return Some(if who.is_empty() {
                "Items are off: none of the items switched on is ported yet.".into()
            } else {
                format!(
                    "Items are off for this match: the items switched on aren't ported {} yet.",
                    who.join(" or ")
                )
            });
        }
        let these: Vec<String> = (reasons.iter())
            .map(|(who, names)| match names.len() {
                1..=4 => format!("{} (not {who})", names.join(", ")),
                n => format!("{n} of the items switched on (not {who})"),
            })
            .collect();
        (!these.is_empty()).then(|| format!("Off for this match: {}.", these.join("; ")))
    }

    /// The setting as the settings folder keeps it.
    pub fn save(&self) -> String {
        let freq = FREQUENCIES
            .iter()
            .find(|f| f.0 == self.freq)
            .map_or("off", |f| f.2);
        let off: Vec<&str> = (KINDS.iter())
            .filter(|k| !self.is_on(k.kind))
            .map(|k| k.key)
            .collect();
        format!("items = {freq}\nitem-switch-off = {}\n", off.join(", "))
    }

    /// Takes back a line [`Switch::save`] wrote; false for another line. A value it can't read
    /// leaves the setting as it is.
    pub fn restore(&mut self, key: &str, v: &str) -> bool {
        match key {
            "items" => {
                if let Some(f) = FREQUENCIES.iter().find(|f| f.2 == v) {
                    self.freq = f.0;
                }
            }
            "item-switch-off" => {
                let keys: Vec<&str> = v.split(',').map(str::trim).collect();
                if keys
                    .iter()
                    .all(|key| key.is_empty() || KINDS.iter().any(|k| k.key == *key))
                {
                    for k in KINDS {
                        self.set(k.kind, !keys.contains(&k.key));
                    }
                }
            }
            _ => return false,
        }
        true
    }

    /// The item switch's boxes: a kind not offered for the match is shown off and disabled,
    /// with why.
    pub fn show_switch(&mut self, ui: &mut egui::Ui, characters: &[u8], stage: u16) {
        ui.horizontal(|ui| {
            if ui.button("All on").clicked() {
                KINDS.iter().for_each(|k| self.set(k.kind, true));
            }
            if ui.button("All off").clicked() {
                KINDS.iter().for_each(|k| self.set(k.kind, false));
            }
        });
        egui::Grid::new("item-switch")
            .num_columns(2)
            .spacing([16.0, 4.0])
            .show(ui, |ui| {
                for (i, k) in KINDS.iter().enumerate() {
                    let s = status(k.kind, characters, stage);
                    if s == Status::Offered {
                        let mut on = self.is_on(k.kind);
                        if ui.checkbox(&mut on, k.name).changed() {
                            self.set(k.kind, on);
                        }
                    } else {
                        let label = format!("{}: {}", k.name, s.short());
                        ui.add_enabled(false, egui::Checkbox::new(&mut false, label))
                            .on_disabled_hover_text(s.detail(k.kind, characters, stage));
                    }
                    if i % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
    }
}

/// A kind's name.
pub fn name(kind: u8) -> &'static str {
    KINDS
        .iter()
        .find(|k| k.kind == kind)
        .map_or("?", |k| k.name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use melee_sim::game::gm::vs_mode::{ITEM_SWITCH_KINDS, NO_ITEM_KIND};
    use melee_sim::game::items_audit::{MISSING, RANDOM_KINDS};
    use melee_sim::game::roster::{CHARACTERS, offered_stages};
    use melee_sim::game::vs_match::{Player, Settings};

    fn players(characters: &[u8]) -> Vec<Player> {
        (characters.iter().enumerate())
            .map(|(i, &c)| Player {
                character: c,
                costume: characters[..i].iter().filter(|&&d| d == c).count() as u8,
                port: i as u8,
                kind: 0,
                cpu_level: 0,
                team: i as u8,
            })
            .collect()
    }

    /// The switch lists the item switch screen's kinds, each once.
    #[test]
    fn the_switch_lists_the_screens_kinds() {
        let mut screen: Vec<u8> = (ITEM_SWITCH_KINDS.iter().copied())
            .filter(|&k| k != NO_ITEM_KIND)
            .collect();
        screen.sort_unstable();
        let ours: Vec<u8> = KINDS.iter().map(|k| k.kind).collect();
        assert_eq!(ours, screen);
        for (i, k) in KINDS.iter().enumerate() {
            assert!(KINDS[..i].iter().all(|j| j.key != k.key), "{}", k.key);
        }
    }

    /// The launcher's rule, held to the audit as the stages' is (`roster`'s
    /// `audited_stages_reach_nothing_unported` and `offered_stages_are_complete`): on every
    /// stage it offers, with every character alone and every pair (an `ItemGap` is reached
    /// when any one of its characters plays, so larger sets reach only what their pairs do),
    /// the match the setting starts with the whole switch on at every frequency turns on, in
    /// its start data, no item kind that reaches a gap. Each kind offered is on, and every
    /// other kind of the switch off.
    #[test]
    fn offered_items_reach_nothing_unported() {
        let mut offered = vec![0usize; KINDS.len()];
        let mut matches = 0usize;
        let mut sets: Vec<Vec<u8>> = CHARACTERS.iter().map(|c| vec![c.kind]).collect();
        for a in CHARACTERS {
            for b in CHARACTERS {
                sets.push(vec![a.kind, b.kind]);
            }
        }
        for freq in FREQUENCIES.map(|f| f.0) {
            let all = Switch {
                freq,
                ..Switch::default()
            };
            for s in offered_stages() {
                for chars in &sets {
                    let m = all.for_match(chars, s.kind);
                    let settings = Settings::vs_items(s.kind, &players(chars), 4, 8, &m.prefs);
                    let r = &settings.start.rules;
                    if r.item_freq == -1 {
                        assert!(m.kinds.is_empty(), "{}: {chars:?}", s.name);
                        continue;
                    }
                    assert_eq!(i32::from(r.item_freq), i32::from(freq));
                    let on: Vec<i32> = (0..RANDOM_KINDS).filter(|k| r.x20 >> k & 1 != 0).collect();
                    let gaps = items_audit::missing_with(&on, chars, s.kind);
                    assert!(
                        gaps.is_empty(),
                        "{} with {chars:?} turns on kinds {on:?}, which reach: {:?}",
                        s.name,
                        gaps.iter().map(|g| g.what).collect::<Vec<_>>()
                    );
                    for (i, k) in KINDS.iter().enumerate() {
                        let is_on = on.contains(&i32::from(k.kind));
                        assert_eq!(is_on, m.kinds.contains(&k.kind), "{}", k.name);
                        if is_on && freq == 3 {
                            offered[i] += 1;
                        }
                    }
                    if freq == 3 {
                        matches += 1;
                    }
                }
            }
        }
        let names: Vec<String> = (KINDS.iter().zip(&offered))
            .filter(|(_, n)| **n > 0)
            .map(|(k, n)| format!("{} ({n})", k.name))
            .collect();
        eprintln!(
            "of {} stage and character choices, items play in {matches}; offered: {}",
            offered_stages().count() * sets.len(),
            if names.is_empty() {
                "none".into()
            } else {
                names.join(", ")
            }
        );
    }

    /// A kind a gap of every match reaches isn't ported; one only some characters reach is
    /// off with them, and the form names them.
    #[test]
    fn gaps_turn_kinds_off() {
        let fd = roster::stage("fd").unwrap().kind;
        let (fox, marth) = (2, 9);
        for k in KINDS {
            let s = status(k.kind, &[fox, marth], fd);
            let every = MISSING.iter().any(|g| {
                g.with.is_empty()
                    && g.stages.is_empty()
                    && (g.kinds.is_empty()
                        || g.kinds.iter().any(|&j| j == i32::from(k.kind) || j < 4))
            });
            assert_eq!(matches!(s, Status::NotPorted(_)), every, "{}", k.name);
            assert_eq!(s == Status::Offered, !every, "{}", k.name);
        }
        // A character's own gap.
        for g in MISSING
            .iter()
            .filter(|g| g.stages.is_empty() && !g.with.is_empty())
        {
            let c = g.with[0];
            let name = roster::character_by_kind(c).unwrap().name;
            for k in KINDS {
                let reached =
                    g.kinds.is_empty() || g.kinds.iter().any(|&j| j == i32::from(k.kind) || j < 4);
                let s = status(k.kind, &[fox, c], fd);
                if reached && status(k.kind, &[fox, marth], fd) == Status::Offered {
                    assert!(
                        matches!(&s, Status::NotWith(who, _) if who.contains(name)),
                        "{}: {s:?}",
                        k.name
                    );
                    let mut one = Switch { freq: 3, on: 0 };
                    one.set(k.kind, true);
                    let m = one.for_match(&[fox, c], fd);
                    assert!(m.kinds.is_empty() && m.prefs == GamePrefs::items_off());
                    let note = one.note(&[fox, c], fd).expect("a note");
                    assert!(note.contains(name), "{note}");
                }
                if reached {
                    assert_ne!(s, Status::Offered, "{} with {name}", k.name);
                }
            }
        }
    }

    /// Off, nothing on, or nothing offered all start the match with items off, as `Settings::vs`
    /// does.
    #[test]
    fn items_off_is_the_vs_default() {
        let fd = roster::stage("fd").unwrap().kind;
        let off = Switch::default();
        assert_eq!(off.freq, OFF);
        assert_eq!(off.for_match(&[2, 9], fd).prefs, GamePrefs::items_off());
        assert_eq!(off.note(&[2, 9], fd), None);
        let none = Switch { freq: 2, on: 0 };
        assert_eq!(none.for_match(&[2, 9], fd).prefs, GamePrefs::items_off());
        assert!(none.note(&[2, 9], fd).is_some());
        let p = players(&[2, 9]);
        assert_eq!(
            Settings::vs_items(fd, &p, 4, 8, &GamePrefs::items_off()).start,
            Settings::vs(fd, &p, 4, 8).start
        );
    }

    #[test]
    fn the_setting_comes_back() {
        let mut was = Switch {
            freq: 4,
            ..Switch::default()
        };
        was.set(0x15, false);
        was.set(0x22, false);
        let mut back = Switch::default();
        for line in was.save().lines() {
            let (k, v) = line.split_once('=').unwrap();
            assert!(back.restore(k.trim(), v.trim()));
        }
        assert_eq!(back, was);
        let mut kept = was;
        assert!(kept.restore("items", "sometimes"));
        assert!(kept.restore("item-switch-off", "bob-omb, nothing"));
        assert!(!kept.restore("stage", "fd"));
        assert_eq!(kept, was);
    }

    /// The switch's boxes draw for any choice, and a kind not offered keeps its place in the
    /// setting: shown off, it comes back on when the choice offers it again.
    #[test]
    fn the_switch_keeps_what_it_hides() {
        let ctx = egui::Context::default();
        let mut switch = Switch {
            freq: 3,
            ..Switch::default()
        };
        let fd = roster::stage("fd").unwrap().kind;
        for characters in [[2u8, 9], [2, 1], [4, 14]] {
            let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
                egui::CentralPanel::default_margins()
                    .show(ui, |ui| switch.show_switch(ui, &characters, fd));
            });
        }
        assert_eq!(
            switch,
            Switch {
                freq: 3,
                ..Switch::default()
            }
        );
    }
}
