//! The match setups' helpers of `melee/gm/gm_1B03.c` (the sub colors are `vs_mode`'s):
//! Sudden Death's start data from the match that tied (`gm_SetupSuddenDeath`), a human's
//! and a CPU's player blocks for the single-player modes' matches, and the select screen's
//! data for a one-player mode's screen (its player and its partner).

use super::super::structs::{MatchEnd, PlayerInitData, StartMeleeData};
use super::vs_melee::CssData;

/// `Gm_PKind_Human`, `Gm_PKind_Cpu`, `Gm_PKind_NA`.
const PKIND_HUMAN: u8 = 0;
const PKIND_CPU: u8 = 1;
const PKIND_NONE: u8 = 3;
/// `OUTCOME_TIMEOUT`.
const OUTCOME_TIMEOUT: u8 = 1;

/// Sudden Death's start data from the tied match's end, `gm_SetupSuddenDeath`: a stock match
/// with no timer, each player still in (not a big loser, with a stock left or a time-out's
/// stocks) back with one stock at 300%, Zelda or Sheik as the match left them; the others out.
#[doc(alias = "gm_SetupSuddenDeath")]
pub fn setup_sudden_death(start: &mut StartMeleeData, end: &MatchEnd) {
    let r = &mut start.rules;
    r.match_kind = 1;
    r.timer_enabled = false;
    r.x2_5 = false;
    for i in 0..6 {
        if start.players[i].slot_type == PKIND_NONE {
            continue;
        }
        let ps = &end.player_standings[i];
        // The inline `gm_801B0474_inline`: the stocks a stock match left (none counts as one
        // only when the player lasted the match), one otherwise.
        let left = if end.match_kind == 1 {
            if end.outcome == OUTCOME_TIMEOUT {
                i32::from(ps.stocks as u8)
            } else {
                let s = ps.stocks as u8;
                if ps.x28 < end.frame_count || s != 0 {
                    i32::from(s)
                } else {
                    1
                }
            }
        } else {
            1
        };
        let big_loser = if end.is_teams == 1 {
            end.team_standings[usize::from(ps.team)].is_big_loser
        } else {
            ps.is_big_loser
        };
        let p = &mut start.players[i];
        if big_loser == 0 && left > 0 {
            let mut c = ps.ckind;
            if c == 0x12 || c == 0x13 {
                c = if ps.ftkind == 7 { 0x13 } else { 0x12 };
            }
            p.ckind = c;
            p.stocks = 1;
            p.damage1 = 300;
        } else {
            p.slot_type = PKIND_NONE;
        }
    }
}

/// A player's port (`slot + 1`) and team (ports 3 and 4 swap teams), `gm_801B05F4`.
#[doc(alias = "gm_801B05F4")]
pub fn set_slot(player: &mut PlayerInitData, slot: i32) {
    player.slot = (slot + 1) as u8;
    let team = match slot {
        2 => 3,
        3 => 2,
        s => s,
    };
    player.team = team as u8;
}

/// A human player's block, `gm_SetupHumanPlayer`.
#[doc(alias = "gm_SetupHumanPlayer")]
pub fn setup_human(player: &mut PlayerInitData, ckind: u8, color: u8, stocks: u8, slot: u8) {
    player.slot_type = PKIND_HUMAN;
    player.ckind = ckind as i8;
    player.color = color;
    player.stocks = stocks as i8;
    set_slot(player, i32::from(slot));
}

/// A computer player's block, on team 4, `gm_SetupCpuPlayer`.
#[doc(alias = "gm_SetupCpuPlayer")]
pub fn setup_cpu(player: &mut PlayerInitData, ckind: u8, color: u8, stocks: u8, slot: u8) {
    player.slot_type = PKIND_CPU;
    player.ckind = ckind as i8;
    player.color = color;
    player.stocks = stocks as i8;
    set_slot(player, i32::from(slot));
    player.team = 4;
}

/// A one-player screen's data (`gm_801B06B0`): the VS data at its defaults, the match type,
/// the screen's port, and its player's character, stocks, costume, level and name tag (the
/// level and stocks the first block's too).
#[doc(alias = "gm_801B06B0")]
#[allow(clippy::too_many_arguments)]
pub fn css_one_player(
    css: &mut CssData,
    match_type: u8,
    ckind: i8,
    stocks: i8,
    color: i8,
    nametag: u8,
    level: u8,
    slot: u8,
) {
    let vs = &mut css.vs;
    vs.start = super::vs_mode::start_defaults();
    vs.loser = -1;
    vs.ordered_stage_index = -1;
    vs.winner = -1;
    css.match_type = match_type;
    css.unk_0x0 = u16::from(slot) + 1;
    let p = &mut css.vs.start.players[usize::from(slot)];
    p.ckind = ckind;
    p.stocks = stocks;
    p.color = color as u8;
    p.cpu_level = level;
    p.nametag = nametag;
    let p0 = &mut css.vs.start.players[0];
    p0.cpu_level = level;
    p0.stocks = stocks;
}

/// A one-player screen's player's choices (`gm_801B0730`): character, stocks, costume, name
/// tag and level.
#[doc(alias = "gm_801B0730")]
pub fn css_one_player_choice(css: &CssData) -> (i8, u8, u8, u8, u8) {
    let slot = usize::from(css.unk_0x0.wrapping_sub(1));
    let p = &css.vs.start.players[slot];
    (p.ckind, p.stocks as u8, p.color, p.nametag, p.cpu_level)
}

/// A one-player screen's partner's block (`gm_801B07B4`): the second block for `first` 0,
/// else the first.
#[doc(alias = "gm_801B07B4")]
pub fn css_set_partner(
    css: &mut CssData,
    ckind: i8,
    stocks: i8,
    color: i8,
    nametag: u8,
    level: u8,
    first: u8,
) {
    let i = usize::from(first == 0);
    let p = &mut css.vs.start.players[i];
    p.ckind = ckind;
    p.stocks = stocks;
    p.color = color as u8;
    p.cpu_level = level;
    p.nametag = nametag;
}

/// A one-player screen's partner's choices (`gm_801B07E8`): the other of the first two blocks
/// than the screen's port's.
#[doc(alias = "gm_801B07E8")]
pub fn css_partner_choice(css: &CssData) -> (i8, i8, i8, i8, u8) {
    let slot = usize::from(css.unk_0x0.wrapping_sub(1) == 0);
    let p = &css.vs.start.players[slot];
    (
        p.ckind,
        p.stocks,
        p.color as i8,
        p.nametag as i8,
        p.cpu_level,
    )
}
