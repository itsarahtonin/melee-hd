//! The results screen's frames (`gmresultplayer.c`): the model GObj's process that runs the
//! screen (`fn_80179350`): its sounds, the panel coming in once the winners have posed for
//! 160 frames (or at a human's Start after a cancelled match), the players' rows, each
//! player's statistics pages, which humans turn and scroll and computers page through on
//! their own, and the screen's end once every player is done.

use gekko_fp as fp;

use super::super::state::{Obj, Running};
use super::results_scene::{PKIND_HUMAN, canceled};
use crate::hsd::gobj::GObjId;

impl Running<'_> {
    /// The results screen's enter data cleared, `gm_80177724`.
    #[doc(alias = "gm_80177724")]
    pub fn results_enter_clear(&mut self) {
        self.game.vs_melee.results_enter = Default::default();
    }

    /// Whether the panel comes in, `fn_801791E4`: after a cancelled match a human's Start,
    /// else at the screen's 160th frame.
    #[doc(alias = "fn_801791E4")]
    fn results_panel_due(&self) -> bool {
        let me = self.results_end();
        if canceled(me.outcome) {
            for i in 0..4 {
                let s = &self.game.pads.copy[i];
                if me.player_standings[i].pkind == PKIND_HUMAN
                    && self.game.pads.master[i].err == 0
                    && s.trigger & 0x1000 != 0
                {
                    return true;
                }
            }
        }
        self.game.results.data.frames as u32 == 0xA0
    }

    /// The model GObj's process, `fn_80179350`: the players' statistics' spots, the first
    /// frame's pause (the enter data's), the sounds at frames 2, 154 and 162 (the crowd, the
    /// winner's name, the panel's), then by the screen's state: the panel's arrival, its
    /// animation's first 40 frames (the players' rows at its tenth), waiting for a button,
    /// the statistics, and the countdown to the screen's end; the frame counted.
    #[doc(alias = "fn_80179350")]
    pub fn results_model_proc(&mut self, gobj: GObjId) {
        for i in 0..6 {
            if let Some(j) = self.game.results.data.players[i].jobjs[12] {
                let p =
                    crate::lb::joint::world_position(&mut self.game.objs, self.data, Some(j), None);
                self.game.results.data.players[i].stats_position = p;
            }
        }
        let me = self.results_end().clone();
        let d = &self.game.results.data;
        let frames = d.frames as u32;
        if frames == 0 && d.x0_4 {
            self.game.set_db_pause_flag(0);
        }
        let cancel = canceled(me.outcome);
        match frames {
            0xA2 => {
                if !cancel {
                    self.sfx_play(0xB5, 0x7F, 0x40);
                }
            }
            2 => {
                if !cancel {
                    self.sfx_play(0xC355, 0x7F, 0x40);
                    self.sfx_play(0x144, 0x7F, 0x40);
                } else {
                    self.sfx_play(0x148, 0x7F, 0x40);
                }
            }
            0x9A => {
                if cancel {
                    self.sfx_play(0xC350, 0x7F, 0x40);
                } else {
                    let w = &me.player_standings[usize::from(self.game.results.data.x6)];
                    let (ck, fk, team) = (w.ckind, w.ftkind, w.team);
                    self.announce_winner(
                        i32::from(ck),
                        i32::from(fk),
                        i32::from(team),
                        me.is_teams == 1,
                    );
                }
            }
            _ => {}
        }
        if self.game.results.data.state < 1 {
            if self.results_panel_due() {
                self.results_panel_in(gobj);
            }
        } else {
            let j = match self.game.world.gobj(gobj).obj {
                Some(Obj::JObj(j)) => j,
                other => panic!("the results' model holds {other:?}"),
            };
            self.anim_all(j);
            match self.game.results.data.state {
                1 => {
                    let frame = crate::lb::anim::current_frame(&self.game.objs, Some(j));
                    if fp::frsp(f64::from(frame)) >= 10.0 && !self.game.results.data.x0_1 {
                        self.results_rows();
                        self.game.results.data.x0_1 = true;
                    }
                    if fp::frsp(f64::from(frame)) >= 40.0 {
                        crate::lb::anim::set_framerate(&mut self.game.objs, Some(j), 0.0);
                        self.game.results.data.state = 2;
                    }
                }
                2 => self.results_wait(gobj),
                3 => self.results_stats_frame(gobj),
                4 => {
                    let d = &mut self.game.results.data;
                    if d.x3 != 0 {
                        d.x3 -= 1;
                    } else {
                        self.game.end_scene();
                    }
                }
                _ => {}
            }
        }
        let d = &mut self.game.results.data;
        if (d.frames as u32) < u32::MAX {
            d.frames = d.frames.wrapping_add(1);
        }
    }

    /// The panel comes in, `fn_80178BB4`: not ported yet.
    fn results_panel_in(&mut self, _gobj: GObjId) {
        super::super::code::unported_call("fn_80178BB4 (the results' panel)");
    }

    /// The players' rows shown, `fn_80177748`: not ported yet.
    fn results_rows(&mut self) {
        super::super::code::unported_call("fn_80177748 (the results' rows)");
    }

    /// Waiting for a button, `fn_80177920`: not ported yet.
    fn results_wait(&mut self, _gobj: GObjId) {
        super::super::code::unported_call("fn_80177920 (the results' wait)");
    }

    /// The statistics' frame, `fn_80178050`: not ported yet.
    fn results_stats_frame(&mut self, _gobj: GObjId) {
        super::super::code::unported_call("fn_80178050 (the results' statistics)");
    }

    /// The announcer names the winner, `fn_80168E54`: not ported yet.
    fn announce_winner(&mut self, _ckind: i32, _ftkind: i32, _team: i32, _teams: bool) {
        super::super::code::unported_call("fn_80168E54 (the winner's name)");
    }
}
