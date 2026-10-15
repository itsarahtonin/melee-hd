//! The VS mode's match setup: the start data's defaults (`gm_SetupRulesDefaults`,
//! `gm_SetupPlayerDefaults`), the VS menus' rules applied to it (`gm_80167BC8`) and the
//! match's entry (`gmVsMelee_EnterVs`), which [`Settings::vs`](super::super::vs_match::Settings::vs)
//! runs on the choices the character and stage select screens would have left.

use gekko_fp as fp;

use crate::fpu::{mem, reg};
use crate::game::code::unported_call;
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

/// The item kind of each of the item switch's entries (`lbl_803B7844`); `0x23` has no kind.
const ITEM_SWITCH_KINDS: [u8; 32] = [
    0x06, 0x0B, 0x07, 0x19, 0x14, 0x12, 0x11, 0x0E, 0x1C, 0x18, 0x08, 0x1A, 0x04, 0x10, 0x17, 0x22,
    0x20, 0x13, 0x09, 0x21, 0x0D, 0x0F, 0x1F, 0x16, 0x15, 0x1E, 0x0A, 0x0C, 0x23, 0x05, 0x1D, 0x1B,
];

/// Each handicap's attack and defense ratios (`lbl_803B7930`), handicap 1 first.
const HANDICAP_RATIOS: [(f32, f32); 9] = [
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

/// The VS menus' rules and the item switch applied to `start`, `gm_80167BC8`.
#[doc(alias = "gm_80167BC8")]
pub fn apply_menu_rules(start: &mut StartMeleeData, rules: &GameRules, prefs: &GamePrefs) {
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
    for p in &mut start.players {
        p.stocks = rules.stock_count as i8;
        match rules.handicap {
            0 => {
                p.attack_ratio = 1.0;
                p.defense_ratio = 1.0;
            }
            // By name tag (`gmMainLib_8015CE44`): the save's name tags aren't modeled.
            1 => unported_call("gmMainLib_8015CE44"),
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
        if kind != 0x23 {
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
/// without a name tag. The save's name tags aren't modeled.
#[doc(alias = "gm_RumbleEnabledForPlayer")]
fn rumble_enabled(prefs: &GamePrefs, port: usize, nametag: u8) -> bool {
    if nametag != NO_NAMETAG {
        unported_call("GetPersistentNameData");
    }
    prefs.rumble_enabled[port]
}

/// Each human player's rumble from the preferences, `gm_LoadRumbleEnabled`.
#[doc(alias = "gm_LoadRumbleEnabled")]
pub fn load_rumble_enabled(start: &mut StartMeleeData, prefs: &GamePrefs) {
    for (i, p) in start.players.iter_mut().enumerate() {
        p.rumble_enabled = if i32::from(p.slot_type) == PKIND_HUMAN && i < 4 {
            let port = if p.slot == 0 {
                i
            } else {
                usize::from(p.slot) - 1
            };
            rumble_enabled(prefs, port, p.nametag)
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
#[doc(alias = "gmVsMelee_EnterVs")]
pub fn enter_vs(vs: &mut StartMeleeData, rules: &GameRules, prefs: &GamePrefs) -> StartMeleeData {
    apply_menu_rules(vs, rules, prefs);
    let mut start = StartMeleeData::default();
    start.rules.clone_from(&vs.rules);
    if start.rules.match_kind == 1 {
        start.rules.is_stock = true;
    }
    start.rules.is_vs = true;
    start.players.clone_from(&vs.players);
    setup_sub_colors(&mut start);
    load_rumble_enabled(&mut start, prefs);
    start
}
