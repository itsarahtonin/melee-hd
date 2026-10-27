//! The VS mode's match setup: the start data's defaults (`gm_SetupRulesDefaults`,
//! `gm_SetupPlayerDefaults`), the VS menus' rules applied to it (`gm_80167BC8`) and the
//! match's entry (`gmVsMelee_EnterVs`), which [`Settings::vs`](super::super::vs_match::Settings::vs)
//! runs on the choices the character and stage select screens would have left.

use gekko_fp as fp;

use crate::fpu::{mem, reg};
use crate::game::code::unported_call;
use crate::game::handles::CodeRef;
use crate::game::pl::player::{PKIND_HUMAN, PKIND_NONE};
use crate::game::structs::{PlayerInitData, StartMeleeData, StartMeleeRules};

/// `ChKind_None`.
pub const CHKIND_NONE: i8 = 0x21;
/// `GM_NAMETAG_COUNT`: no name tag.
pub const NO_NAMETAG: u8 = 120;

/// The VS menus' rules as the save holds them (`GameRules`).
#[derive(Clone, Debug, PartialEq, melee_derive::StateEq)]
pub struct GameRules {
    /// Time (0), stock (1), coin (2) or bonus (3).
    pub mode: u8,
    /// Minutes, for time, coin and bonus matches; 0 for no limit.
    pub time_limit: u8,
    pub stock_count: u8,
    /// Off (0), auto (1, by name tag) or fixed (2).
    pub handicap: u8,
    /// Tenths.
    pub damage_ratio: u8,
    /// A stock match's minutes; 0 for no limit.
    pub stock_time_limit: u8,
    pub friendly_fire: u8,
    pub pause: u8,
    pub score_display: u8,
    /// The self-destruct penalty (`gmMainLib_8015ED30`): 0 for -1, 1 for none, 2 for -2.
    pub sd_penalty: u8,
}

impl Default for GameRules {
    /// A new save's (`gmMainLib_DefaultGameRules`).
    fn default() -> Self {
        GameRules {
            mode: 0,
            time_limit: 2,
            stock_count: 3,
            handicap: 0,
            damage_ratio: 10,
            stock_time_limit: 0,
            friendly_fire: 0,
            pause: 1,
            score_display: 0,
            sd_penalty: 0,
        }
    }
}

/// The game's preferences as the save holds them (`GamePrefs`).
#[derive(Clone, Debug, PartialEq, melee_derive::StateEq)]
pub struct GamePrefs {
    /// The item switch's frequency; `0xFF` for none.
    pub item_freq: u8,
    /// The item switch's items, by the switch's order.
    pub item_mask: u64,
    pub rumble_enabled: [bool; 4],
}

impl Default for GamePrefs {
    /// A new save's (`gmMainLib_DefaultGamePrefs`).
    fn default() -> Self {
        GamePrefs {
            item_freq: 2,
            item_mask: u64::MAX,
            rumble_enabled: [true; 4],
        }
    }
}

impl GameRules {
    /// A new save's rules for a stock match of `stocks` (above 0) or a timed one, with a time
    /// limit of `minutes` (0 for none): the player's own matches.
    pub fn vs(stocks: u8, minutes: u8) -> GameRules {
        let mut rules = GameRules {
            stock_count: stocks,
            ..Default::default()
        };
        if stocks > 0 {
            rules.mode = 1;
            rules.stock_time_limit = minutes;
        } else {
            rules.mode = 0;
            rules.time_limit = minutes;
        }
        rules
    }
}

impl GamePrefs {
    /// A new save's preferences with the item frequency set to none.
    pub fn items_off() -> GamePrefs {
        GamePrefs {
            item_freq: 0xFF,
            ..Default::default()
        }
    }

    /// A new save's preferences with the item frequency `freq` (0 to 4, very low to very high;
    /// `0xFF` for none) and the item switch's entries on for the item kinds `on` takes, as the
    /// item switch screen leaves them. The entry with no kind stays on, as a new save has it.
    pub fn items(freq: u8, on: impl Fn(u8) -> bool) -> GamePrefs {
        let mut mask = u64::MAX;
        for (i, &kind) in ITEM_SWITCH_KINDS.iter().enumerate() {
            if kind != NO_ITEM_KIND && !on(kind) {
                mask &= !(1 << i);
            }
        }
        GamePrefs {
            item_freq: freq,
            item_mask: mask,
            ..Default::default()
        }
    }
}

/// The item switch's entry that has no item kind.
pub const NO_ITEM_KIND: u8 = 0x23;

/// The item kind of each of the item switch's entries (`lbl_803B7844`); `0x23` has no kind.
/// The kinds it doesn't list (the capsule, box, barrel and egg) stay as the rules' defaults
/// set them, on.
pub const ITEM_SWITCH_KINDS: [u8; 32] = [
    0x06, 0x0B, 0x07, 0x19, 0x14, 0x12, 0x11, 0x0E, 0x1C, 0x18, 0x08, 0x1A, 0x04, 0x10, 0x17, 0x22,
    0x20, 0x13, 0x09, 0x21, 0x0D, 0x0F, 0x1F, 0x16, 0x15, 0x1E, 0x0A, 0x0C, 0x23, 0x05, 0x1D, 0x1B,
];

/// Each handicap's attack and defense ratios (`lbl_803B7930`), handicap 1 first.
pub(crate) const HANDICAP_RATIOS: [(f32, f32); 9] = [
    (0.35, 2.857143),
    (0.4, 2.5),
    (0.46, 2.173913),
    (0.53, 1.8867925),
    (0.61, 1.6393442),
    (0.7, 1.4285715),
    (0.8, 1.25),
    (0.93, 1.0752689),
    (1.07, 0.9345794),
];

/// A player slot's defaults, `gm_SetupPlayerDefaults`.
#[doc(alias = "gm_SetupPlayerDefaults")]
pub fn player_defaults() -> PlayerInitData {
    PlayerInitData {
        ckind: CHKIND_NONE,
        slot_type: PKIND_NONE as u8,
        spawn_pos: -1,
        handicap: 9,
        nametag: NO_NAMETAG,
        xC_b1: true,
        cpu_kind: 4,
        attack_ratio: 1.0,
        defense_ratio: 1.0,
        model_scale: 1.0,
        ..PlayerInitData::default()
    }
}

/// The rules' defaults, `gm_SetupRulesDefaults`.
#[doc(alias = "gm_SetupRulesDefaults")]
pub fn rules_defaults() -> StartMeleeRules {
    StartMeleeRules {
        x0_3: 4,
        sd_penalty: 0,
        item_freq: 2,
        x20: u64::MAX,
        x28: 0,
        x3_1: true,
        x3_5: true,
        x3_4: true,
        x4_0: true,
        timer_shows_hours: false,
        x2_5: true,
        x2_6: true,
        x2C: 1.0,
        x30: 1.0,
        game_speed: 1.0,
        x4_6: true,
        x4_7: true,
        xD: 110,
        xA: 0,
        ..StartMeleeRules::default()
    }
}

/// A VS mode's start data as the game first sets it up, `gm_InitVsMode`'s.
#[doc(alias = "gm_InitVsMode")]
pub fn start_defaults() -> StartMeleeData {
    let mut start = StartMeleeData {
        rules: rules_defaults(),
        ..StartMeleeData::default()
    };
    for p in &mut start.players {
        *p = player_defaults();
    }
    start
}

/// The automatic handicaps `gmMainLib_8015CE44` gives: each port's own (without a name tag;
/// a port past the four has none) and each name tag's (`x1A2`).
#[derive(Clone, Debug, Default)]
pub struct AutoHandicaps {
    pub ports: [i8; 4],
    pub tags: Vec<i8>,
}

impl AutoHandicaps {
    /// The save's.
    pub fn of(g: &crate::game::structs::gmm_x0) -> AutoHandicaps {
        AutoHandicaps {
            ports: g.modes.nametags[..4].try_into().expect("four ports"),
            tags: g
                .thing
                .nametag_banks
                .iter()
                .flat_map(|b| b.inner.iter().map(|t| t.x1A2))
                .collect(),
        }
    }

    /// `gmMainLib_8015CE44(port, nametag)`'s handicap, none where it gives null.
    pub fn get(&self, port: usize, nametag: u8) -> Option<i8> {
        if nametag == NO_NAMETAG {
            self.ports.get(port).copied()
        } else {
            self.tags.get(usize::from(nametag)).copied()
        }
    }
}

/// The VS menus' rules and the item switch applied to `start`, `gm_80167BC8`.
pub fn apply_menu_rules(start: &mut StartMeleeData, rules: &GameRules, prefs: &GamePrefs) {
    apply_menu_rules_auto(start, rules, prefs, None);
}

/// [`apply_menu_rules`] with the automatic handicaps the save gives (`auto`; none where the
/// caller has no save, which stops at the automatic handicap).
#[doc(alias = "gm_80167BC8")]
pub fn apply_menu_rules_auto(
    start: &mut StartMeleeData,
    rules: &GameRules,
    prefs: &GamePrefs,
    auto: Option<&AutoHandicaps>,
) {
    let r = &mut start.rules;
    r.timer_enabled = false;
    let limit = match rules.mode {
        0..=3 => {
            r.match_kind = rules.mode;
            if rules.mode == 1 {
                rules.stock_time_limit
            } else {
                rules.time_limit
            }
        }
        _ => 0,
    };
    if rules.mode <= 3 && limit != 0 {
        r.timer_enabled = true;
        r.time_limit = u32::from(limit) * 60;
    }
    for (i, p) in start.players.iter_mut().enumerate() {
        p.stocks = rules.stock_count as i8;
        match rules.handicap {
            0 => {
                p.attack_ratio = 1.0;
                p.defense_ratio = 1.0;
            }
            // By port or name tag (`gmMainLib_8015CE44`), 5 where it gives none.
            1 => {
                let auto = auto.unwrap_or_else(|| {
                    unported_call("gmMainLib_8015CE44 (the automatic handicap without a save)")
                });
                let h = match auto.get(i, p.nametag) {
                    Some(h) => {
                        p.handicap = h;
                        h as u8
                    }
                    None => {
                        p.handicap = 5;
                        5
                    }
                };
                let (a, d) = HANDICAP_RATIOS[h.wrapping_sub(1) as usize];
                p.attack_ratio = a;
                p.defense_ratio = d;
            }
            2 => {
                let (a, d) = HANDICAP_RATIOS[(p.handicap as u8).wrapping_sub(1) as usize];
                p.attack_ratio = a;
                p.defense_ratio = d;
            }
            _ => {}
        }
    }
    let r = &mut start.rules;
    r.friendly_fire = rules.friendly_fire & 1 != 0;
    r.x30 = mem(fp::fmuls(reg(0.1), reg(f32::from(rules.damage_ratio))));
    r.item_freq = prefs.item_freq as i8;
    for (i, &kind) in ITEM_SWITCH_KINDS.iter().enumerate() {
        if kind != NO_ITEM_KIND {
            if prefs.item_mask & (1 << i) != 0 {
                r.x20 |= 1 << kind;
            } else {
                r.x20 &= !(1 << kind);
            }
        }
    }
    match rules.sd_penalty {
        1 => r.sd_penalty = 0,
        0 => r.sd_penalty = -1,
        2 => r.sd_penalty = -2,
        _ => {}
    }
    r.disable_pausing = rules.pause == 0;
    r.x3_0 = rules.score_display != 0 && rules.mode == 0;
}

/// Whether a player's pad rumbles (`gm_RumbleEnabledForPlayer`): its port's preference
/// without a name tag, else its name tag's (`tags`, each name tag's setting in the save).
#[doc(alias = "gm_RumbleEnabledForPlayer")]
pub fn rumble_enabled(prefs: &GamePrefs, port: usize, nametag: u8, tags: &[bool]) -> bool {
    if nametag != NO_NAMETAG {
        return *tags
            .get(usize::from(nametag))
            .unwrap_or_else(|| unported_call("GetPersistentNameData past the name tags"));
    }
    prefs.rumble_enabled[port]
}

/// Each human player's rumble from the preferences, `gm_LoadRumbleEnabled`.
#[doc(alias = "gm_LoadRumbleEnabled")]
pub fn load_rumble_enabled(start: &mut StartMeleeData, prefs: &GamePrefs) {
    load_rumble_enabled_with(start, prefs, &[]);
}

/// [`load_rumble_enabled`] with the save's name tags' settings.
pub fn load_rumble_enabled_with(start: &mut StartMeleeData, prefs: &GamePrefs, tags: &[bool]) {
    for (i, p) in start.players.iter_mut().enumerate() {
        p.rumble_enabled = if i32::from(p.slot_type) == PKIND_HUMAN && i < 4 {
            let port = if p.slot == 0 {
                i
            } else {
                usize::from(p.slot) - 1
            };
            rumble_enabled(prefs, port, p.nametag, tags)
        } else {
            false
        };
    }
}

/// Teammates playing the same character get different sub-colours, `gm_SetupSubColors`.
#[doc(alias = "gm_SetupSubColors")]
pub fn setup_sub_colors(start: &mut StartMeleeData) {
    if start.rules.is_teams != 1 {
        return;
    }
    let n = start.players.len();
    for i in 0..n {
        for j in 0..n {
            if i == j {
                continue;
            }
            let (a, b) = (&start.players[i], &start.players[j]);
            if a.team == b.team && a.ckind == b.ckind && a.sub_color == b.sub_color {
                start.players[j].sub_color = start.players[j].sub_color.wrapping_add(1);
            }
        }
    }
}

/// The match's start data from the VS mode's, `gmVsMelee_EnterVs` with no callbacks: the
/// menus' rules applied, the match marked a VS (and stock) match, the sub-colours and rumble
/// set. The announcer's load (`gm_LoadAnnouncer`) is the match's own, in `Match::start_from`.
pub fn enter_vs(vs: &mut StartMeleeData, rules: &GameRules, prefs: &GamePrefs) -> StartMeleeData {
    enter_vs_with(vs, rules, prefs, None, None)
}

/// A mode's change to the rules of its match (`gm_StartMeleeCallback`).
pub type StartCallback = fn(&mut StartMeleeData);
/// A mode's change to each player block of its match (`gm_PlayerInitCallback`).
pub type PlayerCallback = fn(&mut PlayerInitData);

/// `gmVsMelee_EnterVs` with a mode's callbacks: `start_cb` changes the rules once they're
/// marked, `player_cb` each of the six player blocks once copied.
#[doc(alias = "gmVsMelee_EnterVs")]
pub fn enter_vs_with(
    vs: &mut StartMeleeData,
    rules: &GameRules,
    prefs: &GamePrefs,
    start_cb: Option<StartCallback>,
    player_cb: Option<PlayerCallback>,
) -> StartMeleeData {
    enter_vs_full(vs, rules, prefs, start_cb, player_cb, &[], None)
}

/// VS mode's own entry from the menus, [`enter_vs`] with the save's name tags' rumble
/// settings (`tags`).
pub fn enter_vs_with_tags(
    vs: &mut StartMeleeData,
    rules: &GameRules,
    prefs: &GamePrefs,
    tags: &[bool],
    auto: &AutoHandicaps,
) -> StartMeleeData {
    enter_vs_full(vs, rules, prefs, None, None, tags, Some(auto))
}

/// `gmVsMelee_EnterVs` with a mode's callbacks and the save's name tags' rumble settings
/// (`tags`).
pub fn enter_vs_full(
    vs: &mut StartMeleeData,
    rules: &GameRules,
    prefs: &GamePrefs,
    start_cb: Option<StartCallback>,
    player_cb: Option<PlayerCallback>,
    tags: &[bool],
    auto: Option<&AutoHandicaps>,
) -> StartMeleeData {
    apply_menu_rules_auto(vs, rules, prefs, auto);
    let mut start = StartMeleeData::default();
    start.rules.clone_from(&vs.rules);
    if start.rules.match_kind == 1 {
        start.rules.is_stock = true;
    }
    start.rules.is_vs = true;
    if let Some(cb) = start_cb {
        cb(&mut start);
    }
    start.players.clone_from(&vs.players);
    if let Some(cb) = player_cb {
        for p in &mut start.players {
            cb(p);
        }
    }
    setup_sub_colors(&mut start);
    load_rumble_enabled_with(&mut start, prefs, tags);
    start
}

/// Every player slot's defaults, `gm_SetupAllPlayerDefaults`.
#[doc(alias = "gm_SetupAllPlayerDefaults")]
pub fn all_player_defaults(players: &mut [PlayerInitData]) {
    for p in players {
        *p = player_defaults();
    }
}

/// A Special Melee mode, as VS mode's Special Melee menu offers them (Camera Mode, which
/// takes snapshots, aside): each its own game mode (`GameModeKind`), whose VS entry changes
/// the start data before the match (`gmsupersudden.c` and the others).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Special {
    SuperSuddenDeath,
    Invisible,
    SlowMo,
    Lightning,
    Tiny,
    Giant,
    Stamina,
    SingleButton,
    FixedCamera,
}

impl Special {
    /// The modes, in the menu's order.
    pub const ALL: [Special; 9] = [
        Special::Stamina,
        Special::SuperSuddenDeath,
        Special::Giant,
        Special::Tiny,
        Special::Invisible,
        Special::SlowMo,
        Special::Lightning,
        Special::FixedCamera,
        Special::SingleButton,
    ];

    /// Its game mode.
    pub fn game_mode(self) -> u8 {
        match self {
            Special::SuperSuddenDeath => 0x10,
            Special::Invisible => 0x11,
            Special::SlowMo => 0x12,
            Special::Lightning => 0x13,
            Special::Tiny => 0x1D,
            Special::Giant => 0x1E,
            Special::Stamina => 0x1F,
            Special::FixedCamera => 0x2A,
            Special::SingleButton => 0x2C,
        }
    }

    /// Its name, as the menu shows it.
    pub fn name(self) -> &'static str {
        match self {
            Special::SuperSuddenDeath => "Super Sudden Death",
            Special::Invisible => "Invisible Melee",
            Special::SlowMo => "Slo-Mo Melee",
            Special::Lightning => "Lightning Melee",
            Special::Tiny => "Tiny Melee",
            Special::Giant => "Giant Melee",
            Special::Stamina => "Stamina Mode",
            Special::FixedCamera => "Fixed-Camera Mode",
            Special::SingleButton => "Single-Button Melee",
        }
    }

    /// Its command-line and settings name.
    pub fn key(self) -> &'static str {
        match self {
            Special::SuperSuddenDeath => "super-sudden-death",
            Special::Invisible => "invisible",
            Special::SlowMo => "slo-mo",
            Special::Lightning => "lightning",
            Special::Tiny => "tiny",
            Special::Giant => "giant",
            Special::Stamina => "stamina",
            Special::FixedCamera => "fixed-camera",
            Special::SingleButton => "single-button",
        }
    }

    /// The mode named `key` ([`Special::key`]).
    pub fn from_key(key: &str) -> Option<Special> {
        Special::ALL.into_iter().find(|m| m.key() == key)
    }

    /// The match's start data from the mode's VS data, as its VS entry makes it: VS mode's
    /// (`gmVsMelee_EnterVs`) with the mode's callbacks, or Stamina Mode's own.
    pub fn enter_vs(
        self,
        vs: &mut StartMeleeData,
        rules: &GameRules,
        prefs: &GamePrefs,
    ) -> StartMeleeData {
        let (start_cb, player_cb): (Option<StartCallback>, Option<PlayerCallback>) = match self {
            Special::SuperSuddenDeath => (None, Some(super_sudden_death_player)),
            Special::Invisible => (None, Some(invisible_player)),
            Special::SlowMo => (Some(slow_mo_rules), None),
            Special::Lightning => (Some(lightning_rules), None),
            Special::Tiny => (None, Some(tiny_player)),
            Special::Giant => (None, Some(giant_player)),
            Special::SingleButton => (Some(single_button_rules), None),
            Special::FixedCamera => (Some(fixed_camera_rules), None),
            Special::Stamina => return enter_stamina(vs, rules, prefs),
        };
        enter_vs_with(vs, rules, prefs, start_cb, player_cb)
    }
}

/// Super Sudden Death's players start (and come back) at 300%, `fn_801B8C5C`.
#[doc(alias = "fn_801B8C5C")]
pub fn super_sudden_death_player(p: &mut PlayerInitData) {
    p.damage1 = 300;
}

/// Invisible Melee's players are invisible, `initVsPlayer`.
#[doc(alias = "initVsPlayer")]
pub fn invisible_player(p: &mut PlayerInitData) {
    p.vs_invisible = true;
}

/// Slo-Mo Melee's game runs at half speed, `fn_801BA5B4`.
#[doc(alias = "fn_801BA5B4")]
pub fn slow_mo_rules(start: &mut StartMeleeData) {
    start.rules.game_speed = 0.5;
}

/// Lightning Melee's runs at 1.25 times, `fn_801BA7AC`.
#[doc(alias = "fn_801BA7AC")]
pub fn lightning_rules(start: &mut StartMeleeData) {
    start.rules.game_speed = 1.25;
}

/// Tiny Melee's players: about a third of the size, taking half the damage, `fn_801B8E50`.
#[doc(alias = "fn_801B8E50")]
pub fn tiny_player(p: &mut PlayerInitData) {
    p.model_scale = 0.35;
    p.defense_ratio = 0.5;
    p.attack_ratio = 1.0;
    p.xB = 1;
}

/// Giant Melee's players: nearly twice the size, dealing half again the damage,
/// `fn_801B9060`.
#[doc(alias = "fn_801B9060")]
pub fn giant_player(p: &mut PlayerInitData) {
    p.model_scale = 1.8;
    p.defense_ratio = 1.0;
    p.attack_ratio = 1.5;
    p.xB = 2;
}

/// Single-Button Melee's rules, `fn_801BA1B4`.
#[doc(alias = "fn_801BA1B4")]
pub fn single_button_rules(start: &mut StartMeleeData) {
    start.rules.single_button = true;
}

/// Fixed-Camera Mode's unpause hook fixes the camera (`gm_80165290`), `fn_801B9FB8`; the
/// match's start calls it too.
#[doc(alias = "fn_801B9FB8")]
pub fn fixed_camera_rules(start: &mut StartMeleeData) {
    start.rules.on_unpause_override = Some(CodeRef(super::FIXED_CAMERA));
}

/// Stamina Mode's VS entry: the menus' rules applied to its VS data ([`stamina_entry`]),
/// then the rumble.
pub fn enter_stamina(
    vs: &mut StartMeleeData,
    rules: &GameRules,
    prefs: &GamePrefs,
) -> StartMeleeData {
    apply_menu_rules(vs, rules, prefs);
    let mut start = stamina_entry(vs);
    load_rumble_enabled(&mut start, prefs);
    start
}

/// Stamina Mode's VS entry, `gm_801B931C` (between the menus' rules and the rumble): its VS
/// data's rules taken as they are (no VS or stock mark), a stock match with no timer whose
/// match start is the mode's (`fn_801B9850`); the first four players with one stock and 150
/// HP, shown as HP (`xC_b7`), the other two the defaults; the sub-colours set.
#[doc(alias = "gm_801B931C")]
pub fn stamina_entry(vs: &StartMeleeData) -> StartMeleeData {
    let mut start = StartMeleeData::default();
    start.rules.clone_from(&vs.rules);
    let r = &mut start.rules;
    r.x2_5 = false;
    r.timer_enabled = false;
    r.match_kind = 1;
    r.on_match_start = Some(CodeRef(super::stamina::ON_MATCH_START));
    r.x3_0 = false;
    all_player_defaults(&mut start.players);
    for (p, v) in start.players.iter_mut().zip(&vs.players).take(4) {
        p.clone_from(v);
        p.xC_b7 = true;
        p.stocks = 1;
        p.hp = 150;
    }
    setup_sub_colors(&mut start);
    start
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The item switch's entries reach the rules' kinds through `gm_80167BC8`: each kind the
    /// switch lists is on exactly when its entry is, and the kinds it doesn't list stay on.
    #[test]
    fn the_item_switch_sets_its_kinds() {
        for wanted in [
            vec![],
            vec![0x22],
            vec![0x06, 0x15, 0x04],
            (4..=0x22).collect(),
        ] {
            let prefs = GamePrefs::items(3, |k| wanted.contains(&k));
            let mut start = start_defaults();
            apply_menu_rules(&mut start, &GameRules::default(), &prefs);
            assert_eq!(start.rules.item_freq, 3);
            for kind in 0..64u8 {
                let listed = kind != NO_ITEM_KIND && ITEM_SWITCH_KINDS.contains(&kind);
                let on = start.rules.x20 >> kind & 1 != 0;
                assert_eq!(on, !listed || wanted.contains(&kind), "kind {kind:#x}");
            }
        }
        let all = GamePrefs::items(2, |_| true);
        assert_eq!(all, GamePrefs::default());
        assert_eq!(GamePrefs::items_off().item_freq, 0xFF);
    }
}
