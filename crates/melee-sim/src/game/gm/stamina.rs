//! Stamina Mode's match (`melee/gm/gmstamina.c`): its match start (`fn_801B9850`) gives
//! the scene a process that slows the game for a moment when a player is knocked out and
//! ends the match 100 frames after one player or team is left.

use super::super::code::Val;
use super::super::handles::CodeRef;
use super::super::pl::player::PKIND_NONE;
use super::super::state::{Game, Running};
use super::GM_STAMINA_VS;

/// The match start the mode sets (`StartMeleeRules::on_match_start`).
pub const ON_MATCH_START: u32 = 0x801B_9850;
/// The mode's process, `fn_801B96E8`.
pub const PROC: u32 = 0x801B_96E8;

/// The mode's statics (`gm_804975F8`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Stamina {
    /// Frames since one player or team was left, up to the match's end at 100.
    pub done_counter: u16,
    /// Each port's player is out (or none plays there).
    pub eliminated: [bool; 4],
    /// Frames since each port's player was knocked out, slowing the game until 100.
    pub slomo_counter: [u16; 4],
}

impl Game {
    /// How many players, or teams in a team match, are left, `gm_801B9600`: a player with no
    /// stocks left is out. The mode's team flag is its VS mode data's
    /// (`gmMainLib_804D3EE0`'s Stamina entry), which the match's rules copy at its start.
    #[doc(alias = "gm_801B9600")]
    pub fn stamina_left(&mut self) -> i32 {
        let teams = self.vs.start.is_teams != 0;
        let mut count = 0;
        for i in 0..4 {
            if self.player_pkind(i) != PKIND_NONE && self.player_stocks(i) == 0 {
                self.gm.stamina.eliminated[i as usize] = true;
            }
            if self.gm.stamina.eliminated[i as usize] {
                continue;
            }
            if teams {
                let mut j = 0;
                while j < i {
                    if !self.gm.stamina.eliminated[j as usize]
                        && self.player_team(i) == self.player_team(j)
                    {
                        break;
                    }
                    j += 1;
                }
                if i == j {
                    count += 1;
                }
            } else {
                count += 1;
            }
        }
        count
    }

    /// A player is knocked out, `gm_801B97C4`: the first time, its match's last frame is
    /// noted and the game slows to 0.4 for 100 frames. A sub-fighter (Nana) changes nothing.
    #[doc(alias = "gm_801B97C4")]
    pub fn stamina_knocked_out(&mut self, slot: i32, sub: i32) {
        if slot < 4 && sub == 0 {
            if !self.gm.stamina.eliminated[slot as usize] {
                self.update_player_match_frame_count(slot, sub);
                self.gm.stamina.slomo_counter[slot as usize] = 1;
                self.set_game_speed(0.4);
            }
            self.gm.stamina.eliminated[slot as usize] = true;
        }
    }
}

impl Game {
    /// The mode's match start, `fn_801B9850`: nothing counted, the empty ports out, and the
    /// mode's process on a GObj of its own.
    #[doc(alias = "fn_801B9850")]
    pub fn stamina_match_start(&mut self) -> crate::hsd::gobj::GObjId {
        let g = self;
        g.gm.stamina.done_counter = 0;
        for i in 0..4 {
            g.gm.stamina.eliminated[i as usize] = g.player_pkind(i) == PKIND_NONE;
            g.gm.stamina.slomo_counter[i as usize] = 0;
        }
        let gobj = g.world.create(0xF, 0x11, 0);
        g.world.setup_proc(gobj, CodeRef(PROC), 0x15);
        gobj
    }

    /// The mode's process, `fn_801B96E8`: each knocked-out player's slowdown counts to 100,
    /// then the game's speed comes back; with one player or team left, 100 frames later the
    /// match ends with the mode's own ending (`gm_8016B33C(5)`).
    #[doc(alias = "fn_801B96E8")]
    pub fn stamina_proc(&mut self) {
        let g = self;
        for i in 0..4 {
            if g.player_pkind(i) == PKIND_NONE {
                continue;
            }
            let c = &mut g.gm.stamina.slomo_counter[i as usize];
            if *c > 0 && *c < 100 {
                *c += 1;
            } else if *c == 100 {
                *c = 0;
                g.set_game_speed(1.0);
            }
        }
        if g.stamina_left() <= 1 {
            let d = &mut g.gm.stamina.done_counter;
            *d = d.wrapping_add(1);
            if *d > 100 {
                g.set_game_speed(1.0);
                g.vs_set_end_kind(5);
                g.vs_terminate();
            }
        }
    }

    /// A fighter is knocked out of stamina (or a hand beaten), `gm_80167470`: Stamina Mode
    /// counts it; no other mode does anything.
    #[doc(alias = "gm_80167470")]
    pub fn fighter_lost(&mut self, slot: i32, sub: i32) {
        if self.game_mode == GM_STAMINA_VS {
            self.stamina_knocked_out(slot, sub);
        }
    }
}

/// The mode's functions the match calls by address: its match start.
pub fn call(r: &mut Running, f: u32, _args: &[Val]) -> Option<Val> {
    match f {
        ON_MATCH_START => {
            r.game.stamina_match_start();
        }
        _ => return None,
    }
    Some(Val::None)
}
