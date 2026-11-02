//! Event Match (`melee/gm/gmevent.c`): the 51 events' table (`GmEvent.dat`'s
//! `sqEventInitDataLevelTbl`, indexed by the event's place in the list), the list's order of
//! names (`gm_803DF918`), and the event mode's getters.

use melee_data::archive::DataRef;

use super::super::state::Running;
use crate::hsd::gobj::{self, GObjId};

/// `GM_EVENT`.
pub const GM_EVENT: u8 = 0x2B;

/// The events (`0x33`).
pub const EVENTS: u8 = 0x33;

/// `gm_803DF918`: each place in the list's name and description (the menus' strings
/// `0x154 + 2n` and `0x155 + 2n`).
pub const NAME_ORDER: [u8; EVENTS as usize] = [
    0, 17, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 36, 15, 16, 1, 18, 19, 20, 21, 26, 23, 24, 25,
    22, 27, 28, 29, 30, 31, 32, 33, 47, 35, 14, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 34, 48, 49,
    50,
];

/// An event's entry in the table (`gm_804D6900_t`): its kind (0 a match, 1 a bonus match
/// against a crowd, 2 a series of rounds), its players' count (`flags`' top three bits),
/// `x4` (the event's own parameters), its rules (`evinit`), its crowd (`evbonus`), its rounds
/// (`evstage_table`) and its players' setups.
#[derive(Clone, Copy, Debug)]
pub struct Entry(pub DataRef);

impl Entry {
    pub fn kind(self, r: &Running) -> u8 {
        r.data.u8(self.0)
    }

    pub fn players(self, r: &Running) -> u8 {
        r.data.u8(self.0.at(1)) >> 5 & 7
    }

    pub fn x4(self, r: &Running) -> Option<DataRef> {
        r.data.ptr(self.0.at(4))
    }

    pub fn evinit(self, r: &Running) -> DataRef {
        r.data.ptr(self.0.at(8)).expect("an event's rules")
    }

    pub fn evbonus(self, r: &Running) -> Option<DataRef> {
        r.data.ptr(self.0.at(0xC))
    }

    pub fn stages(self, r: &Running) -> Option<DataRef> {
        r.data.ptr(self.0.at(0x10))
    }

    pub fn player(self, r: &Running, i: usize) -> Option<DataRef> {
        r.data.ptr(self.0.at(0x14 + 4 * i as u32))
    }
}

impl Running<'_> {
    /// `gm_804D6900[0]`: the events' table, which the menus' entry loads (`gm_801BA8FC`).
    pub fn event_table(&self) -> DataRef {
        self.game
            .menu
            .event_levels
            .expect("GmEvent.dat's sqEventInitDataLevelTbl")
    }

    /// The `n`-th event's entry, none where the table has a null.
    pub fn event_entry(&self, n: u8) -> Option<Entry> {
        self.data
            .ptr(self.event_table().at(4 * u32::from(n)))
            .map(Entry)
    }

    /// `gm_801BEB74`: the event chosen.
    #[doc(alias = "gm_801BEB74")]
    pub fn set_event(&mut self, n: u8) {
        self.game.game_data.vs.unk_530.unk_535 = n;
    }

    /// `gm_801BEB80`: the event chosen last.
    #[doc(alias = "gm_801BEB80")]
    pub fn event(&self) -> u8 {
        self.game.game_data.vs.unk_530.unk_535
    }

    /// `gm_801BEB8C`: whether event `n` is scored by its time (`evinit`'s `x1_0`), not KOs.
    #[doc(alias = "gm_801BEB8C")]
    pub fn event_timed(&self, n: u8) -> bool {
        let e = self.event_entry(n).expect("an event's entry");
        self.data.u8(e.evinit(self).at(1)) & 0x80 != 0
    }

    /// `gm_801BEBF8`: the character event `name`'s first player plays, none where its place's
    /// entry is null: the place in the list of the event named `name`, its entry's first
    /// player's character.
    #[doc(alias = "gm_801BEBF8")]
    pub fn event_character(&self, name: u8) -> u8 {
        let i = event_place(name);
        match self.event_entry(i) {
            None => 0x21,
            Some(e) => {
                let p = e.player(self, 0).expect("an event's first player");
                self.data.u8(p)
            }
        }
    }
}

/// `gm_801BEBA8`: the name of the event at place `n` in the list.
#[doc(alias = "gm_801BEBA8")]
pub fn event_name(n: u8) -> u8 {
    NAME_ORDER[usize::from(n)]
}

/// `gm_801BEBC0`: the place in the list of the event named `name`, 0x33 for none.
#[doc(alias = "gm_801BEBC0")]
pub fn event_place(name: u8) -> u8 {
    NAME_ORDER
        .iter()
        .position(|&v| v == name)
        .map_or(EVENTS, |i| i as u8)
}

/// `onEnterVs`'s players' setups from the table (`gm_801BAB40_src`, 0x1C bytes): the
/// character, slot type, stocks, costume, spawn point, sub-colour, team, `xB`, flags
/// (metal, invisible and four others), computer kind and level, damage, hit points and the
/// attack, defence and model ratios.
#[derive(Clone, Copy, Debug)]
pub struct PlayerSrc(pub DataRef);

impl PlayerSrc {
    pub fn ckind(self, r: &Running) -> i8 {
        r.data.u8(self.0) as i8
    }

    pub fn color(self, r: &Running) -> u8 {
        r.data.u8(self.0.at(3))
    }

    pub fn team(self, r: &Running) -> u8 {
        r.data.u8(self.0.at(6))
    }
}

/// `gmevent.c`'s statics: the select's data (`css_data`), the match's enter data
/// (`vs_enter_data`) and its exit data (`vs_exit_data[0]`; the second of the two nothing
/// uses).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct EventMode {
    pub css: super::vs_melee::CssData,
    pub start: super::super::structs::StartMeleeData,
    pub exit: super::match_exit::MatchExitInfo,
    /// Where the copy of `GmEvent.dat` the mode loaded last has its data (0 for the menus'
    /// copy), which the comparer takes `gm_804D6900` by.
    pub table_at: u32,
}

/// The match's start hook, `fn_801BBFE8`.
pub const ON_MATCH_START: u32 = 0x801B_BFE8;

/// The events' processes by the event's place (`gm_803DF94C`'s `x0`): the stock and time
/// events' (`gm_801BC754`), the coins' (`gm_801BC9E8`) and each event's own.
pub const PROCS: [u32; EVENTS as usize] = [
    0x801B_C754,
    0x801B_C754,
    0x801B_C754,
    0x801B_C754,
    0x801B_C9E8,
    0x801B_C754,
    0x801B_C754,
    0x801B_C754,
    0x801B_CAF0,
    0x801B_CF20,
    0x801B_CF40,
    0x801B_C754,
    0x801B_D028,
    0x801B_C754,
    0x801B_C754,
    0x801B_C754,
    0x801B_D164,
    0x801B_C754,
    0x801B_D30C,
    0x801B_D44C,
    0x801B_D46C,
    0x801B_D658,
    0x801B_C754,
    0x801B_C754,
    0x801B_D7FC,
    0x801B_C754,
    0x801B_C754,
    0x801B_C754,
    0x801B_D93C,
    0x801B_DAD4,
    0x801B_C754,
    0x801B_DAF4,
    0x801B_DC08,
    0x801B_C754,
    0x801B_C754,
    0x801B_DE94,
    0x801B_C754,
    0x801B_C754,
    0x801B_C754,
    0x801B_E37C,
    0x801B_C754,
    0x801B_C754,
    0x801B_C754,
    0x801B_E39C,
    0x801B_C754,
    0x801B_C754,
    0x801B_C754,
    0x801B_DD44,
    0x801B_E618,
    0x801B_E638,
    0x801B_C754,
];

/// The next costume after `c`, the fifth going back to the first (`gm_GetNextColor`).
fn next_color(c: u8) -> u8 {
    if c <= 2 { c + 1 } else { 0 }
}

impl Running<'_> {
    /// `gm_801BA8FC`: `GmEvent.dat` loaded again for its table.
    fn event_load_table(&mut self) {
        let a = self.load_archive("GmEvent.dat");
        let at = self.data.get(a).public("sqEventInitDataLevelTbl");
        self.game.menu.event_levels = at.map(|offset| DataRef { archive: a, offset });
        // Where this copy's data is: the menus' copy stays where it was, and a copy loaded
        // between two scenes is gone again before any tick's end shows it.
        let file = crate::hsd::heap::with(|h| h.last_archive.0).unwrap_or(0);
        self.game.event.table_at = if file != 0 { file + 0x20 } else { 0 };
    }

    /// `gm_801BA938`: the event's players `lo` to `hi` in the game cache (a crowd's Kirbys
    /// without their copy's hat), the stage too when `stage`, and their sound banks.
    #[doc(alias = "gm_801BA938")]
    fn event_cache(&mut self, lo: usize, hi: usize, stage: bool) {
        let ev = self.game.game_data.vs.unk_530.clone();
        let gc = &mut self.dvd_mut().cache.scene.game_cache;
        for i in lo..hi {
            let c = ev.x4C[i];
            if c != 0x21 {
                gc.entries[i].char_id = i32::from(c);
                gc.entries[i].color = ev.x50[i];
                if ev.x45 != 0 && i > 0 && c == 4 {
                    gc.entries[i].x5 = 0;
                }
            }
        }
        if stage {
            gc.stkind = ev.x48;
        }
        self.preload_refresh();
        let mut mask = 0;
        for i in lo..hi {
            if ev.x4C[i] != 0x21 {
                mask |= crate::lb::audio::character_banks(i32::from(ev.x4C[i]));
            }
        }
        mask |= crate::lb::audio::stage_banks(ev.x48);
        let lb = &mut self.game.audio.lb;
        lb.unwant_banks(0x1C);
        lb.want_banks(0xC, mask);
        self.game.audio.lb_start_bank_loads(self.data);
    }

    /// `onEnterCss`: the event's select for its first player, the screen's port the 1P
    /// lists' entry's; with its first player to choose, the other players in the cache.
    #[doc(alias = "onEnterCss")]
    pub fn event_enter_css(&mut self) {
        let e = self.game.game_data.vs.unk_530.clone();
        super::setup_players::css_one_player(
            &mut self.game.event.css,
            0xE,
            e.x2,
            0,
            e.x3 as i8,
            e.nametag,
            0,
            e.x6,
        );
        if e.x44 == 1 {
            self.event_cache(1, 4, true);
        }
    }

    /// `onExitCss`: back to the menu, or the character chosen (the random opponents drawn
    /// again).
    #[doc(alias = "onExitCss")]
    pub fn event_exit_css(&mut self) {
        if self.game.event.css.pending_scene_change == 2 {
            self.change_game_mode_after_scene(super::machine::GM_MENU);
            return;
        }
        let (ckind, _, color, nametag, _) =
            super::setup_players::css_one_player_choice(&self.game.event.css);
        let e = &mut self.game.game_data.vs.unk_530;
        e.x2 = ckind;
        e.x3 = color;
        e.nametag = nametag;
        e.x8 = -1;
        e.x9 = -1;
        e.xA = -1;
    }

    /// `gm_801BAB40`: a player's block from the event's setup.
    #[doc(alias = "gm_801BAB40")]
    pub fn event_player(&self, src: DataRef) -> super::super::structs::PlayerInitData {
        let d = self.data;
        let u8_ = |o| d.u8(src.at(o));
        let flags = u8_(8);
        let mut p = super::vs_mode::player_defaults();
        p.ckind = u8_(0) as i8;
        p.slot_type = u8_(1);
        p.stocks = u8_(2) as i8;
        p.color = u8_(3);
        p.slot = 0;
        p.spawn_pos = u8_(4) as i8;
        p.sub_color = u8_(5);
        p.handicap = 9;
        p.team = u8_(6);
        p.nametag = 0x78;
        p.xB = u8_(7);
        p.rumble_enabled = false;
        p.xC_b1 = flags & 0x80 != 0;
        p.vs_metal = flags & 0x40 != 0;
        p.xC_b3 = flags & 0x20 != 0;
        p.vs_invisible = flags & 0x10 != 0;
        p.xC_b6 = flags & 0x08 != 0;
        p.xC_b7 = flags & 0x04 != 0;
        p.xD_b1 = false;
        p.xD_b2 = false;
        p.xD_b4 = false;
        p.cpu_kind = u8_(9);
        p.cpu_level = u8_(0xA);
        p.damage = 0;
        p.damage1 = d.u16(src.at(0xC));
        p.hp = d.u16(src.at(0xE));
        p.attack_ratio = d.f32(src.at(0x10));
        p.defense_ratio = d.f32(src.at(0x14));
        p.model_scale = d.f32(src.at(0x18));
        p
    }

    /// `gm_801BAC9C`: a random opponent from the event's list (`x4`, ending at 0x21), none of
    /// the first `n` players' characters.
    #[doc(alias = "gm_801BAC9C")]
    fn event_random_opponent(
        &mut self,
        start: &super::super::structs::StartMeleeData,
        n: usize,
    ) -> u8 {
        let e = self.event_entry(self.event()).expect("the event's entry");
        let list = e.x4(self).expect("the event's characters");
        let mut chars = [0u8; 0x21];
        let mut count = 0;
        for i in 0..0x21 {
            let c = self.data.u8(list.at(i));
            if c == 0x21 {
                break;
            }
            if !start.players[..n].iter().any(|p| p.ckind == c as i8) {
                chars[count] = c;
                count += 1;
            }
        }
        let k = self.game.objs.rng.below(count as i32);
        chars[k as usize]
    }
}

impl Running<'_> {
    /// `gm_Mode_Event_OnLoad`: the table loaded again, the event's run cleared, the 1P
    /// lists' port, the players' characters as the event has them (`gm_801BBB64`), and the
    /// select skipped for an event whose first player's character is set.
    #[doc(alias = "gm_Mode_Event_OnLoad")]
    pub fn event_on_load(&mut self) {
        self.event_load_table();
        let port = self.one_player_port();
        let e = &mut self.game.game_data.vs.unk_530;
        e.x0 = 0;
        e.x1 = 0;
        e.x6 = port;
        e.x7 = 0;
        e.xB_0 = false;
        e.xB_1 = false;
        e.xB_2 = false;
        e.xB_3 = false;
        e.xB_4 = false;
        e.xB_5 = false;
        e.xB_6 = false;
        e.xC = 0;
        e.x10 = 0;
        e.x14 = 0;
        e.x18 = 0;
        e.x1C = 1.0;
        e.x20 = 0;
        e.x24 = 0;
        e.x28 = 0;
        e.x2C = 0;
        e.x30 = 0;
        e.x34 = 0;
        e.x38 = 0x21;
        e.x3C = 0;
        e.x40 = 0;
        self.event_players();
        let n = self.event();
        let e = self.event_entry(n).expect("the event's entry");
        let p0 = e.player(self, 0).expect("the event's first player");
        if self.data.u8(p0) != 0x21 {
            self.set_game_mode_state_id(1);
        }
    }

    /// `gm_801BBB64`: how the event's files load (`x44`: 0 its first player's character
    /// set, 1 to choose; 2 to 4 a series' rounds), its stage, and its players' characters
    /// and costumes (a crowd's and a round's opponent's from their own setups).
    #[doc(alias = "gm_801BBB64")]
    fn event_players(&mut self) {
        let idx = self.event();
        let e = self.event_entry(idx).expect("the event's entry");
        let p0 = e.player(self, 0).expect("the event's first player");
        let x44 = if self.data.u8(p0) as i8 != 0x21 { 0 } else { 1 };
        let stkind = i32::from(self.data.u16(e.evinit(self).at(6)));
        let mut chars = [0x21i8; 4];
        let mut colors = [0u8; 4];
        for i in 0..4 {
            if let Some(p) = e.player(self, i) {
                chars[i] = self.data.u8(p) as i8;
                colors[i] = self.data.u8(p.at(3));
            }
        }
        let mut x45 = 0;
        if e.kind(self) == 1 {
            let b = e.evbonus(self).expect("the event's crowd");
            chars[1] = self.data.u8(b) as i8;
            colors[1] = if self.data.u8(b.at(5)) == 1 {
                self.data.u8(b.at(6))
            } else {
                0xFF
            };
            x45 = u8::from(self.data.u8(b.at(0x17)) == 1);
        }
        let ev = &mut self.game.game_data.vs.unk_530;
        ev.x44 = x44;
        ev.x48 = stkind;
        ev.x4C = chars;
        ev.x50 = colors;
        ev.x45 = x45;
        let round = ev.x20;
        match idx {
            9 | 19 | 29 | 39 | 48 => {
                let r = self.event_round(e, round as usize);
                let ev = &mut self.game.game_data.vs.unk_530;
                ev.x44 = if round == 0 { 3 } else { 4 };
                ev.x4C[1] = self.data.u8(r) as i8;
                ev.x50[1] = self.data.u8(r.at(3));
            }
            35 if round <= 1 => {
                let picks: &[(usize, usize)] = if round == 0 {
                    &[(1, 0), (2, 2), (3, 3)]
                } else {
                    &[(1, 1), (2, 4)]
                };
                let v: Vec<(usize, i8, u8)> = picks
                    .iter()
                    .map(|&(slot, k)| {
                        let r = self.event_round(e, k);
                        (slot, self.data.u8(r) as i8, self.data.u8(r.at(3)))
                    })
                    .collect();
                let ev = &mut self.game.game_data.vs.unk_530;
                ev.x44 = if round == 0 { 2 } else { 4 };
                for (slot, c, col) in v {
                    ev.x4C[slot] = c;
                    ev.x50[slot] = col;
                }
            }
            43 => {
                let x4 = e.x4(self).expect("the event's parameters");
                let p = self.data.ptr(x4.at(4)).expect("the event's third player");
                let (c, col) = (self.data.u8(p) as i8, self.data.u8(p.at(3)));
                let ev = &mut self.game.game_data.vs.unk_530;
                ev.x4C[2] = c;
                ev.x50[2] = col;
            }
            _ => {}
        }
    }

    /// A series' round `k`'s opponent's setup (`evstage_table->entries[k]`).
    fn event_round(&self, e: Entry, k: usize) -> DataRef {
        let s = e.stages(self).expect("the event's rounds");
        self.data
            .ptr(s.at(0x10 + 4 * k as u32))
            .expect("a round's opponent")
    }
}

impl Running<'_> {
    /// `onEnterVs`: the table loaded again; the match's rules from the event's (`evinit`),
    /// its start hook (`fn_801BBFE8`); a series' stage and time for its round; the event's
    /// players from their setups (the first the chosen character where the event leaves it
    /// open, the random opponents kept for the event's retries, costumes kept apart from the
    /// first player's); a crowd's setup; the files as the event loads them (`gm_801BA938`),
    /// the rumble and the announcer's bank.
    #[doc(alias = "onEnterVs")]
    pub fn event_enter_vs(&mut self) {
        self.event_load_table();
        let level = self.event();
        let e = self.event_entry(level).expect("the event's entry");
        let ei = e.evinit(self);
        let d = self.data;
        let b0 = d.u8(ei);
        let b1 = d.u8(ei.at(1));
        let mut start = std::mem::take(&mut self.game.event.start);
        start.rules = super::vs_mode::rules_defaults();
        let r = &mut start.rules;
        r.match_kind = b0 >> 5;
        r.x0_3 = b0 >> 2 & 7;
        r.timer_enabled = b0 & 2 != 0;
        r.timer_counts_up = b0 & 1 != 0;
        r.x1_0 = true;
        r.x1_1 = false;
        r.x1_2 = false;
        r.x1_3 = false;
        r.timer_shows_hours = false;
        r.friendly_fire = b1 & 0x40 != 0;
        r.x2_2 = false;
        r.single_button = false;
        r.disable_pausing = false;
        r.x2_5 = b1 & 0x20 != 0;
        r.x3_1 = true;
        r.x3_2 = true;
        r.x3_3 = true;
        r.x3_4 = true;
        r.x3_5 = true;
        r.x3_7 = false;
        r.x4_2 = false;
        r.x4_3 = true;
        r.x4_4 = false;
        r.x4_6 = false;
        r.x4_7 = false;
        r.x5_0 = true;
        r.x5_1 = false;
        r.x5_2 = b1 & 0x10 != 0;
        r.x5_3 = b1 & 0x08 != 0;
        r.x7 = 0;
        r.is_teams = d.u8(ei.at(2));
        r.x9 = 0;
        r.xA = 0;
        r.item_freq = d.u8(ei.at(3)) as i8;
        r.sd_penalty = d.u8(ei.at(4)) as i8;
        r.xD = 0x6E;
        r.stkind = d.u16(ei.at(6));
        r.time_limit = d.u32(ei.at(8));
        r.x14 = 0;
        r.x18 = 0;
        r.x20 = u64::from(d.u32(ei.at(0x10))) << 32 | u64::from(d.u32(ei.at(0x14)));
        r.x28 = d.u32(ei.at(0x18)) as i32;
        r.x30 = d.f32(ei.at(0x1C));
        r.game_speed = d.f32(ei.at(0x20));
        r.on_match_start = Some(super::super::handles::CodeRef(ON_MATCH_START));
        let counts_up = r.timer_counts_up;
        let quake = d.f32(ei.at(0x24));
        let ev = &mut self.game.game_data.vs.unk_530;
        if counts_up {
            ev.xB_0 = true;
        }
        if b1 & 0x80 != 0 {
            ev.xB_6 = true;
        }
        if quake != 1.0 {
            ev.x1C = quake;
        }
        if e.kind(self) == 2 {
            super::super::code::unported_call("gm_8016A92C (an event's series of rounds)");
        }
        for p in &mut start.players {
            p.slot_type = 3;
        }
        let count = usize::from(e.players(self));
        let mut i = 0;
        while i < count {
            while e.player(self, i).is_none() {
                i += 1;
            }
            let src = e.player(self, i).expect("an event's player");
            start.players[i] = self.event_player(src);
            if i == 0 {
                self.event_first_player(&mut start);
            } else {
                self.event_other_player(&mut start, i, PlayerSrc(src));
            }
            i += 1;
        }
        if level == 0x2B {
            let ev = &mut self.game.game_data.vs.unk_530;
            if ev.x4C[0] == ev.x4C[2] && ev.x50[0] == ev.x50[2] {
                ev.x50[2] = next_color(ev.x50[2]);
            }
        }
        if level == 0x31 {
            for (i, dir) in [(1, 1), (2, -1)] {
                let p = &mut start.players[i];
                p.spawn_dir = dir;
                p.xD_b0 = true;
                p.xD_b2 = true;
            }
        }
        if e.kind(self) == 1 {
            super::super::code::unported_call("gm_8016A22C (an event's crowd)");
        }
        let ev = self.game.game_data.vs.unk_530.clone();
        match ev.x44 {
            0 | 1 => self.event_cache(0, 4, true),
            2 => {
                let gc = &mut self.dvd_mut().cache.scene.game_cache;
                gc.entries[0].char_id = i32::from(ev.x4C[0]);
                gc.entries[0].color = ev.x50[0];
                self.preload_refresh();
                self.wait_preload_class(0xC7);
                self.event_cache(1, 4, true);
            }
            3 => {
                self.wait_preload_class(0xC7);
                self.event_cache(1, 4, true);
            }
            4 => {
                self.dvd_mut().keep_class();
                let gc = &mut self.dvd_mut().cache.scene.game_cache;
                gc.entries[0].char_id = i32::from(ev.x4C[0]);
                gc.entries[0].color = ev.x50[0];
                self.preload_refresh();
                self.wait_preload_stale_released(4);
                self.event_cache(1, 4, true);
            }
            _ => {}
        }
        let (_, prefs) = super::attract::saved_rules(&self.game.game_data);
        let tags = self.nametag_rumbles();
        super::vs_mode::load_rumble_enabled_with(&mut start, &prefs, &tags);
        self.game.event.start = start;
        self.load_announcer_bank();
    }

    /// `onEnterVs`'s first player: its port, the chosen character where the event leaves it
    /// open, its rumble, and the event's note of its team, character and costume.
    fn event_first_player(&mut self, start: &mut super::super::structs::StartMeleeData) {
        let ev = self.game.game_data.vs.unk_530.clone();
        super::setup_players::set_slot(&mut start.players[0], i32::from(ev.x6));
        let p = &mut start.players[0];
        let team = p.team;
        if p.ckind == 0x21 {
            p.ckind = ev.x2;
            p.color = ev.x3;
            p.nametag = ev.nametag;
        }
        let (c, col, tag) = (p.ckind, p.color, p.nametag);
        let ev = &mut self.game.game_data.vs.unk_530;
        ev.x7 = team;
        ev.x0 = c as u8;
        ev.x4C[0] = c;
        ev.x1 = col;
        ev.x50[0] = col;
        let port = usize::from(ev.x6);
        let (_, prefs) = super::attract::saved_rules(&self.game.game_data);
        let tags = self.nametag_rumbles();
        start.players[0].rumble_enabled = super::vs_mode::rumble_enabled(&prefs, port, tag, &tags);
    }

    /// `onEnterVs`'s other players: on the first player's team where their setup has team 0,
    /// a random character where it has none (kept in `x8` to `xA` for the event's retries),
    /// and a costume apart from the first player's.
    fn event_other_player(
        &mut self,
        start: &mut super::super::structs::StartMeleeData,
        i: usize,
        src: PlayerSrc,
    ) {
        let p0 = start.players[0].clone();
        if src.team(self) == 0 {
            start.players[i].team = p0.team;
            start.players[i].xD_b1 = true;
        }
        if src.ckind(self) == 0x21 {
            let ev = &self.game.game_data.vs.unk_530;
            let kept = match i {
                1 => ev.x8,
                2 => ev.x9,
                3 => ev.xA,
                _ => super::super::code::unported_call("an event's random fifth player"),
            };
            if kept == -1 {
                let c = self.event_random_opponent(start, i) as i8;
                start.players[i].ckind = c;
                let ev = &mut self.game.game_data.vs.unk_530;
                match i {
                    1 => ev.x8 = c,
                    2 => ev.x9 = c,
                    _ => ev.xA = c,
                }
            } else {
                start.players[i].ckind = kept;
            }
        }
        let p = &mut start.players[i];
        if p.ckind == p0.ckind && p.color == p0.color {
            p.color = next_color(p.color);
        }
        // Sheik's opponent Zelda keeps apart from her costume as the same character does.
        if p0.ckind == 0x13 && p.ckind == 0x12 && p.color == p0.color {
            p.color = next_color(p.color);
        }
        if i < 4 {
            let (c, col) = (p.ckind, p.color);
            let ev = &mut self.game.game_data.vs.unk_530;
            ev.x4C[i] = c;
            ev.x50[i] = col;
        }
    }
}

impl Running<'_> {
    /// `onExitVs`: the single-player match's setup undone (`gm_8016A164`); a retry plays the
    /// event again from its first round; no contest goes back to the menu; else the match's
    /// coins and time added up, a series' next round, the event's best kept with its clear,
    /// the coins, the play time, the unlocks by the event (a character, the stage after every
    /// event, its trophy), the trophies and messages, and a challenger or the menu.
    #[doc(alias = "onExitVs")]
    #[doc(alias = "gm_80168940")]
    #[doc(alias = "gm_801732D8")]
    #[doc(alias = "gm_8017335C")]
    #[doc(alias = "gm_80173D3C")]
    pub fn event_exit_vs(&mut self) {
        let level = self.event();
        self.one_player_match_undo();
        let me = self.game.event.exit.match_end.clone();
        if me.outcome == super::vs::OUTCOME_RETRY {
            let ev = &mut self.game.game_data.vs.unk_530;
            let save = ev.x20 != 0 && level != 0x31;
            ev.xB_5 = false;
            ev.x20 = 0;
            ev.x24 = 0;
            ev.x28 = 0;
            ev.x2C = 0;
            ev.x30 = 0;
            ev.x34 = 0;
            ev.x38 = 0x21;
            ev.x3C = 0;
            ev.x40 = 0;
            self.event_players();
            if save {
                let (c, col) = (
                    self.game.game_data.vs.unk_530.x0,
                    self.game.game_data.vs.unk_530.x1,
                );
                self.dvd_mut().keep_class();
                let gc = &mut self.dvd_mut().cache.scene.game_cache;
                gc.entries[0].char_id = i32::from(c as i8);
                gc.entries[0].color = col;
                self.preload_refresh();
                self.wait_preload_stale_released(4);
            }
            self.set_next_game_mode_state_id(1);
            return;
        }
        if me.outcome == super::vs::OUTCOME_NO_CONTEST {
            self.change_game_mode_after_scene(super::machine::GM_MENU);
            return;
        }
        let p0 = &me.player_standings[0];
        let coins = if p0.pkind == 0 { i32::from(p0.xE) } else { 0 };
        let ev = &mut self.game.game_data.vs.unk_530;
        ev.x3C = ev.x3C.wrapping_add(coins);
        ev.x40 = ev.x40.wrapping_add(me.frame_count as i32);
        if ev.xB_4 && ev.xB_2 {
            ev.x24 = i32::from(p0.stocks);
            ev.x28 = p0.percent as i32;
            ev.xB_2 = false;
            ev.xB_5 = false;
            ev.x20 += 1;
            self.event_players();
            self.set_next_game_mode_state_id(1);
            return;
        }
        let cleared = ev.xB_1;
        if cleared {
            let cur = ev.xC;
            let timed = ev.xB_6;
            let mut best = self.event_score(i32::from(level));
            let better = if timed {
                best == 0 || (cur as u32) < best as u32
            } else {
                (cur as u32) > best as u32
            };
            if better {
                best = cur;
                self.set_event_score(i32::from(level), best);
            }
            self.game.game_data.thing.save_data.x1A68 |= 1i64 << level;
        }
        let (coins, frames) = {
            let ev = &self.game.game_data.vs.unk_530;
            (ev.x3C, ev.x40)
        };
        self.results_add_coins(coins);
        let s = &mut self.game.game_data.thing.save_data;
        s.x1A3C = (s.x1A3C as u32).wrapping_add(frames as u32 / 60) as i32;
        let place = event_place(level);
        let kind = if cleared {
            if !self.game.gm.unlocks.ckind_unlocked(0x19) && place == 0x1C {
                0x19
            } else if !self.game.gm.unlocks.ckind_unlocked(0x18) && place == 0xE {
                0x18
            } else {
                0x21
            }
        } else {
            0x21
        };
        if level == event_place(0x32) && me.player_standings[0].stocks == 3 {
            self.game.game_data.thing.save_data.x1B3C = 1;
        }
        if cleared {
            let all = (0..i32::from(EVENTS)).all(|i| self.event_cleared(i));
            if all && !self.stage_unlocked(0x20) {
                self.unlock_stage(0x20);
            }
            // `lbl_803D6450`: the trophies of events 3, 14, 26, 45, 47 and 51 (event 3's
            // English one on this disc).
            const TROPHIES: [(u8, u8, u16); 7] = [
                (0x02, 0x00, 0x67),
                (0x02, 0x01, 0x67),
                (0x0D, 0x02, 0xC0),
                (0x19, 0x02, 0x92),
                (0x2C, 0x02, 0xBB),
                (0x2E, 0x02, 0xDD),
                (0x32, 0x02, 0xBF),
            ];
            // `lbLang_GetLanguageSetting`: 1, English.
            if let Some(&(_, _, t)) = TROPHIES
                .iter()
                .find(|&&(n, l, _)| (l == 2 || l == 1) && event_place(n) == event_place(level))
            {
                self.give_record_trophy(i32::from(t));
            }
        }
        self.trophy_unlocks();
        let _ = self.special_unlocks(0x10);
        let ev = self.game.game_data.vs.unk_530.clone();
        if kind != 0x21 {
            self.init_challenger(
                ev.x0,
                ev.x1,
                ev.x6,
                ev.nametag,
                kind,
                super::machine::GM_MENU,
            );
            self.change_game_mode_after_scene(super::machine::GM_CHALLENGER_APPROACH);
            return;
        }
        if !self.challenger_check(1, ev.x6) {
            self.change_game_mode_after_scene(super::machine::GM_MENU);
        }
    }

    /// `gm_8016A164`: the single-player match's setup undone: not single-player, the
    /// crowd's counts and spawning cleared, no teams, and the players past the first
    /// gone from their slots.
    #[doc(alias = "gm_8016A164")]
    #[doc(alias = "fn_8016B138")]
    pub fn one_player_match_undo(&mut self) {
        self.game.vs.state.is_singleplayer = 0;
        self.game.gm.spawn_flags &= !3;
        self.game.vs.start.is_teams = 0;
        for i in 1..6 {
            self.game.set_player_pkind(i, 3);
            self.game.unset_player_flags_bit1(i);
        }
    }

    /// `fn_801BBFE8`, the match's start hook: the event's own setup (`gm_801BC00C`).
    #[doc(alias = "fn_801BBFE8")]
    pub fn event_match_start(&mut self) {
        let _ = super::super::mn::common::game_mode(self);
        self.event_match_setup();
    }

    /// `gm_801BC00C`: the event's run's counters cleared, its rounds' fighters' costumes
    /// loaded and their timers, the trophies' costumes loaded for the rounds, the flash
    /// dimmed in event 37, the quake's scale, and the event's process.
    #[doc(alias = "gm_801BC00C")]
    fn event_match_setup(&mut self) {
        let idx = self.event();
        let ev = &mut self.game.game_data.vs.unk_530;
        ev.xB_1 = false;
        ev.x10 = 0;
        ev.x14 = 0;
        ev.x18 = 0;
        if matches!(
            idx,
            4 | 9 | 12 | 15 | 19 | 21 | 23 | 27 | 29 | 35 | 39 | 43 | 44 | 48
        ) {
            self.event_load_table();
        }
        if matches!(idx, 9 | 19 | 29 | 35 | 39 | 43 | 48) {
            super::super::code::unported_call(&format!("gm_801BC00C for event {}", idx + 1));
        }
        if idx == 0x24 {
            self.game.bgflash.set_scale(0.2);
        }
        let q = self.game.game_data.vs.unk_530.x1C;
        super::super::camera::set_quake_scale(&mut self.game.camera.cam, q);
        let w = &mut self.game.world;
        let g = w.create(0xF, 0x11, 0);
        w.setup_proc(
            g,
            super::super::handles::CodeRef(PROCS[usize::from(idx)]),
            0x15,
        );
    }
}

/// The processes [`PROCS`] holds.
const PROC_STOCKS: u32 = 0x801B_C754;
const PROC_COINS: u32 = 0x801B_C9E8;
const PROC_SURVIVE: u32 = 0x801B_CF40;
const PROC_KOS_OF_3: u32 = 0x801B_D164;
const PROC_KEEP_2: u32 = 0x801B_D30C;
const PROC_KO_2: u32 = 0x801B_D7FC;
const PROC_KO_3_KEEP_2: u32 = 0x801B_D93C;
const PROC_GOAL: u32 = 0x801B_DC08;
const PROC_KO_3_4: u32 = 0x801B_DD44;

impl Running<'_> {
    /// Whether the match's time ran out (`gm_801BC754`'s and the others' inline): never for
    /// a timer that counts up, else with the timer on at its last frame of second 0.
    fn event_time_up(&self) -> bool {
        if self.game.game_data.vs.unk_530.xB_0 {
            return false;
        }
        self.game.vs.start.timer_enabled
            && self.game.vs_timer_seconds() == 0
            && self.game.vs_timer_frames() == 0x3B
    }

    /// `gm_801BC4F4`: the event cleared: its score (the match's time for a timed event, at
    /// most 59:59.99 and from the series' start for event 50, else the KOs of the players
    /// past the first, event 32's those of the second less the first's falls; capped at
    /// -1, which every count is under as unsigned), whether it's a new best, the match
    /// ended with the success's announcement (a new record's sound and rumble), and the
    /// process gone.
    #[doc(alias = "gm_801BC4F4")]
    fn event_success(&mut self, gobj: GObjId) {
        let level = self.event();
        self.game.game_data.vs.unk_530.xB_1 = true;
        let best = self.event_score(i32::from(level)) as u32;
        let ev = self.game.game_data.vs.unk_530.clone();
        let record = if ev.xB_6 {
            let mut t = self.game.vs_frame_count().wrapping_add(ev.x34 as u32);
            if level == 0x31 {
                t = ev.x34 as u32;
            }
            if t > 0x34BBF {
                t = 0x34BBF;
            }
            self.game.game_data.vs.unk_530.xC = t as i32;
            best == 0 || t < best
        } else {
            let mut kos: u32 = 0;
            for i in 1..6 {
                if self.game.player_pkind(i) != 3 {
                    kos = kos.wrapping_add(self.game.player_kos(0, i));
                }
            }
            if level == 0x1F {
                super::super::code::unported_call("pl_8003FBFC (event 32's falls)");
            }
            // `if (var_r27 > -1)`: an unsigned compare with 0xFFFFFFFF, never true.
            self.game.game_data.vs.unk_530.xC = kos as i32;
            kos > best
        };
        self.player_80036844(0, 1);
        self.game.audio.lb.master = 0x7F;
        self.game.set_game_speed(1.0);
        self.game.vs_set_end_kind(2);
        if record {
            self.game.vs.state.unk_10 = 0x9C40 + 1;
            self.game.vs.state.unk_14 = 0x144 + 1;
        } else {
            self.game.vs.state.unk_14 = 0x145 + 1;
        }
        self.game.vs_terminate();
        gobj::free(self, gobj);
    }

    /// `failEvent`: the event failed: the match ended with the failure's announcement and
    /// sound, its end frozen 40 frames, the process gone.
    fn event_fail(&mut self, gobj: GObjId) {
        self.game.game_data.vs.unk_530.xB_1 = false;
        self.game.audio.lb.master = 0x7F;
        self.game.set_game_speed(1.0);
        self.game.vs_set_end_kind(6);
        self.game.vs.state.unk_14 = 0x148 + 1;
        self.game.vs.state.unk_18 = 0x28;
        self.game.vs_terminate();
        gobj::free(self, gobj);
    }

    /// `gm_801BC754`, the stock and time events' process: in a stock match, cleared when no
    /// opponent has a stock left (nor a crowd's player waits), failed when the first player
    /// has none or the time runs out; in a time match (or a coin one), at the time's end
    /// cleared if the first player's team won alone, else failed.
    #[doc(alias = "gm_801BC754")]
    fn event_stocks_proc(&mut self, gobj: GObjId) {
        match self.game.vs.start.match_kind {
            1 => {
                let mut count = 0;
                for i in 1..6 {
                    if self.game.player_pkind(i) != 3 && self.game.player_stocks(i) != 0 {
                        count += 1;
                    }
                }
                if self.game.game_data.vs.unk_530.xB_3 {
                    super::super::code::unported_call("gm_80169384 (an event's crowd)");
                }
                if count == 0 {
                    self.event_success(gobj);
                    return;
                }
                if self.game.player_p1_stocks() <= 0 {
                    self.event_fail(gobj);
                    return;
                }
                if self.event_time_up() {
                    self.event_fail(gobj);
                }
            }
            0 | 2 if self.event_time_up() => {
                if u32::from(self.game.game_data.vs.unk_530.x7) == self.event_winning_team() {
                    self.event_success(gobj);
                    return;
                }
                self.event_fail(gobj);
            }
            _ => {}
        }
    }

    /// `gm_801BC488`: the team that won the match alone so far (`gm_80166378` on a copy of
    /// the match's end), else 4.
    #[doc(alias = "gm_801BC488")]
    fn event_winning_team(&mut self) -> u32 {
        let _ = self;
        super::super::code::unported_call("gm_801BC488 (an event's time match's winner)");
    }

    /// `gm_801BC9E8`, the coin event's: cleared at the event's coins, failed when the time
    /// runs out.
    #[doc(alias = "gm_801BC9E8")]
    fn event_coins_proc(&mut self, gobj: GObjId) {
        let e = self.event_entry(self.event()).expect("the event's entry");
        let x4 = e.x4(self).expect("the event's coins");
        let target = self.data.u32(x4);
        if self.game.player_coins(0) as u32 >= target {
            self.event_success(gobj);
            return;
        }
        if self.event_time_up() {
            self.event_fail(gobj);
        }
    }

    /// `gm_801BCF40`: survive to the time's end: failed with no stock left.
    #[doc(alias = "gm_801BCF40")]
    fn event_survive_proc(&mut self, gobj: GObjId) {
        if self.game.player_p1_stocks() <= 0 {
            self.event_fail(gobj);
            return;
        }
        if self.event_time_up() {
            self.event_success(gobj);
        }
    }

    /// `gm_801BD164`: once the third player is out, cleared if the first player KO'd it
    /// more than the second did, else failed; failed with no stock left or at the time's end.
    #[doc(alias = "gm_801BD164")]
    fn event_kos_of_3_proc(&mut self, gobj: GObjId) {
        if self.game.player_stocks(2) <= 0 {
            let theirs = self.game.player_kos(1, 2) as i32;
            if self.game.player_kos(0, 2) as i32 > theirs {
                self.event_success(gobj);
            } else {
                self.event_fail(gobj);
            }
        }
        if self.game.player_p1_stocks() <= 0 {
            self.event_fail(gobj);
            return;
        }
        if self.event_time_up() {
            self.event_fail(gobj);
        }
    }

    /// `gm_801BD30C`: keep the second player in to the time's end.
    #[doc(alias = "gm_801BD30C")]
    fn event_keep_2_proc(&mut self, gobj: GObjId) {
        if self.game.player_stocks(1) <= 0 || self.game.player_p1_stocks() <= 0 {
            self.event_fail(gobj);
            return;
        }
        if self.event_time_up() {
            self.event_success(gobj);
        }
    }

    /// `gm_801BD7FC`: KO the second player before the time's end.
    #[doc(alias = "gm_801BD7FC")]
    fn event_ko_2_proc(&mut self, gobj: GObjId) {
        if self.game.player_stocks(1) <= 0 {
            self.event_success(gobj);
            return;
        }
        if self.game.player_p1_stocks() <= 0 || self.event_time_up() {
            self.event_fail(gobj);
        }
    }

    /// `gm_801BD93C`: KO the third player while the second stays in.
    #[doc(alias = "gm_801BD93C")]
    fn event_ko_3_keep_2_proc(&mut self, gobj: GObjId) {
        if self.game.player_stocks(2) <= 0 {
            self.event_success(gobj);
            return;
        }
        if self.game.player_stocks(1) <= 0
            || self.game.player_p1_stocks() <= 0
            || self.event_time_up()
        {
            self.event_fail(gobj);
        }
    }

    /// `gm_801BDC08`: reach the stage's goal (`Ground_801C1D84`).
    #[doc(alias = "gm_801BDC08")]
    fn event_goal_proc(&mut self, gobj: GObjId) {
        if super::super::gr::ground::flags(self.game, 0x330) != 0 {
            self.event_success(gobj);
            return;
        }
        if self.game.player_p1_stocks() <= 0 || self.event_time_up() {
            self.event_fail(gobj);
        }
    }

    /// `gm_801BDD44`: KO the third and fourth players.
    #[doc(alias = "gm_801BDD44")]
    fn event_ko_3_4_proc(&mut self, gobj: GObjId) {
        if self.game.player_stocks(2) <= 0 && self.game.player_stocks(3) <= 0 {
            self.event_success(gobj);
            return;
        }
        if self.game.player_p1_stocks() <= 0 || self.event_time_up() {
            self.event_fail(gobj);
        }
    }
}

/// Runs an event's process.
pub fn run_proc(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PROC_STOCKS => r.event_stocks_proc(gobj),
        PROC_COINS => r.event_coins_proc(gobj),
        PROC_SURVIVE => r.event_survive_proc(gobj),
        PROC_KOS_OF_3 => r.event_kos_of_3_proc(gobj),
        PROC_KEEP_2 => r.event_keep_2_proc(gobj),
        PROC_KO_2 => r.event_ko_2_proc(gobj),
        PROC_KO_3_KEEP_2 => r.event_ko_3_keep_2_proc(gobj),
        PROC_GOAL => r.event_goal_proc(gobj),
        PROC_KO_3_4 => r.event_ko_3_4_proc(gobj),
        a if PROCS.contains(&a) => {
            super::super::code::unported_call(&format!("the event process {a:#x}"))
        }
        _ => return false,
    }
    true
}
