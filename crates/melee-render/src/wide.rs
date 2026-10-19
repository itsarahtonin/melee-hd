//! Widescreen's fighters in the added sides (docs/player.md, "Resolution, aspect ratio and
//! pipelines"). The game decides who draws with its own 73:60 camera: a fighter past that view
//! (with a margin) is marked off screen and left out (`ftLib_UpdateScreenVisibility`), and the
//! magnifier shows a bubble for it. A widescreen picture sees more to each side, so such a
//! fighter vanished there and only its bubble showed. Slippi's widescreen code draws it anyway;
//! this does the same for the picture, without touching the match.
//!
//! Drawing writes state (joint, envelope and texture matrices brought up to date, model flags,
//! a joint's particles on its first display), so the match's own display pass can't draw them:
//! its state would depend on the aspect. Instead [`Wide`] keeps a copy of the state the pass
//! starts from (`clone_from`, reusing the copy's storage) and, when the match's pass marked a
//! fighter off screen or the magnifier showed one, runs the same pass on the copy with the
//! widened view ([`melee_sim::hsd::display::Display::wide`]): a fighter the game leaves out
//! but the widened view reaches is drawn. The frame is then drawn from the copy and its
//! draws. The match is only read, so its state, draws and
//! sound are the same at every aspect ratio.

use std::panic::{AssertUnwindSafe, catch_unwind};

use melee_sim::game::state::Game;
use melee_sim::game::vs_match::Match;
use melee_sim::hsd::display::Draw;

use crate::frame::GAME_ASPECT;

/// The copy a widescreen picture's frames are drawn from when they differ from the match's.
#[derive(Default)]
pub struct Wide {
    /// How many times the game's width the picture shows (`frame::widen`'s factor); none
    /// without widescreen.
    k: Option<f32>,
    /// The state the latest display pass started from, then as the widened pass left it.
    copy: Option<Match>,
    /// Whether the latest frame is drawn from the copy.
    shown: bool,
    /// Why a widened pass stopped, until taken.
    stopped: Option<String>,
}

impl Wide {
    /// Draws for a picture shown at `widescreen` (an aspect ratio wider than the game's
    /// 73:60, as `Hsd::widescreen` holds it), or as the match draws without one.
    pub fn set(&mut self, widescreen: Option<f32>) {
        self.k = widescreen
            .map(|aspect| aspect / GAME_ASPECT)
            .filter(|&k| k > 1.0);
        if self.k.is_none() {
            self.copy = None;
            self.shown = false;
        }
    }

    /// Whether a picture this wide differs from the game's view.
    pub fn is_on(&self) -> bool {
        self.k.is_some()
    }

    /// Before the match's display pass: keeps the state it starts from.
    pub fn before(&mut self, real: &Match) {
        self.shown = false;
        let Some(k) = self.k else { return };
        match &mut self.copy {
            Some(copy) => copy.game.clone_from(&real.game),
            // A clone starts without the mix log and the widening (`#[clone_from(keep)]`).
            None => self.copy = Some(real.clone()),
        }
        self.copy.as_mut().expect("the copy").game.objs.display.wide = Some(k);
    }

    /// After the match's display pass drew `draws`: the draws to show. When the pass marked a
    /// fighter off screen, or the magnifier showed one the pass before marked, the pass runs
    /// again on the copy with the widened view, and its draws are shown (frames then draw
    /// from [`Wide::game`]); the match's draws are kept for the copy's next pass to fill.
    pub fn after(&mut self, real: &Match, draws: Vec<Draw>) -> Vec<Draw> {
        let Some(copy) = &mut self.copy else {
            return draws;
        };
        let marked = |g: &Game| g.fighters.iter().any(|(_, f)| f.x221F_b0);
        if self.k.is_none() || !(marked(&real.game) || marked(&copy.game)) {
            return draws;
        }
        match catch_unwind(AssertUnwindSafe(|| copy.render())) {
            Ok(wide) => {
                let mut kept = draws;
                kept.clear();
                copy.game.objs.display.draws = kept;
                self.shown = true;
                wide
            }
            Err(p) => {
                // A fighter the match left out can reach code no port covers yet: the frame
                // shows the match's draws, and the copy is restored before the next pass.
                let why = p
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                    .unwrap_or_else(|| "a panic".to_owned());
                self.stopped.get_or_insert(why);
                draws
            }
        }
    }

    /// Whether the latest frame's draws are the copy's.
    pub fn is_shown(&self) -> bool {
        self.shown
    }

    /// The state the latest frame's draws are drawn from: the copy if they're its, else the
    /// match's.
    pub fn game<'a>(&'a self, real: &'a Game) -> &'a Game {
        match &self.copy {
            Some(copy) if self.shown => &copy.game,
            _ => real,
        }
    }

    /// Why a widened pass stopped, once.
    pub fn take_stopped(&mut self) -> Option<String> {
        self.stopped.take()
    }
}
