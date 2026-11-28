//! What Interpolate's and Extrapolate's frames draw ([`crate::pacing`]): the latest tick's draws
//! with its transforms blended toward the tick before or the next predicted.
//!
//! **A tick that draws nothing holds the image shown.** About every 1001 ticks Melee runs a
//! tick without its display pass (its alarm runs at 60 Hz, its video at 60000/1001 Hz), and
//! with high refresh off nothing new is presented for it, as on the console. A frame names
//! objects by handle and reads them live (world matrices, materials, particles, what exists),
//! and the tick moved the state on without a display pass, so the tick before's draws on the
//! new state are neither image: matrices the tick's logic recomputed sit beside ones only the
//! display pass would have, animations and particles have moved, and handles may be freed. So
//! the paced frames hold too until a tick draws again. Interpolate then blends from the last
//! tick that drew (its transforms are kept until another tick draws) to the new one, and
//! Extrapolate's prediction of a tick that draws nothing is no prediction, so the tick before
//! it is drawn as it is and held through it, Off's image.
//!
//! **A tick that loads holds the image shown too** ([`crate::jit::Hold`]): its frame is the
//! one drawn after the load, which shows only once the load's length has passed, and it's drawn
//! as it is then, never blended with the tick before the load.
//!
//! **Extrapolate's prediction waits for the frame that needs it.** A tick only marks its
//! prediction to make ([`Paced::will_predict`]). The tick's first frame drawn at t = 0 is the
//! tick's own and blends toward nothing, so the prediction runs on a thread of its own beside it
//! ([`Paced::beside`], `Predictor::predict_beside`); a frame at a later t that finds it still to
//! make makes it first ([`Paced::predict`]), as the tick did before. A tick no frame drew after
//! (a hold, the panel opened) is never predicted.

use std::time::Instant;

use melee_render::blend::{Blend, Capture};
use melee_render::predict::{Cost, Predictor};
use melee_sim::game::state::Game;
use melee_sim::game::vs_match::Frame;
use melee_sim::hsd::display::Draw;

use crate::pacing::Mode;

/// The four controllers a tick reads.
pub type Pads = [melee_sim::input::pad::PadStatus; 4];

/// The ticks the paced frames draw from.
#[derive(Default)]
pub struct Paced {
    /// The latest tick's frame and when the tick was due.
    latest: Option<(Frame, Instant)>,
    /// With Interpolate: the transforms of the tick that drew before the latest that did (A)
    /// and of that latest (B), kept for the next tick that draws.
    before: Capture,
    after: Capture,
    /// With Extrapolate: the next tick, predicted.
    pub predictor: Predictor<crate::power::Run>,
    /// With Extrapolate: the latest tick's prediction still to make, with the controllers the
    /// next tick will see.
    pending: Option<Pads>,
}

impl Paced {
    /// Forgets every tick, for a new match or mode.
    pub fn clear(&mut self) {
        self.latest = None;
        self.before.clear();
        self.after.clear();
        self.predictor.clear();
        self.pending = None;
    }

    /// Takes in a tick's frame, due at `due`, as `game` was left by it; returns the frame it
    /// replaced, to recycle. Extrapolate's prediction is the caller's.
    pub fn ran(&mut self, mode: Mode, game: &Game, f: Frame, due: Instant) -> Option<Frame> {
        if mode == Mode::Interpolate && !f.draws.is_empty() {
            // B is this tick; A, the last that drew, whose transforms were kept then.
            std::mem::swap(&mut self.before, &mut self.after);
            self.after.fill(game, &f.draws);
        }
        self.latest.replace((f, due)).map(|(old, _)| old)
    }

    /// The next tick loads (a long tick, [`crate::jit::Hold`]): Interpolate's frames don't blend
    /// across the load, from the scene before it or the state before it, to what the load
    /// brought. The next tick that draws is drawn as it is (as Off draws it) until the one after.
    pub fn loading(&mut self) {
        self.after.clear();
    }

    /// With Extrapolate, after a tick: the next tick is to be predicted with `pads`, by the first
    /// frame drawn after it ([`Paced::beside`] or [`Paced::predict`]). The prediction before it
    /// was the tick's own and is forgotten. After a tick that drew nothing the frames hold, so
    /// nothing is to be predicted.
    pub fn will_predict(&mut self, pads: Pads) {
        self.predictor.forget();
        self.pending = self
            .latest
            .as_ref()
            .is_some_and(|(f, _)| !f.draws.is_empty())
            .then_some(pads);
    }

    /// Whether the latest tick's prediction is still to make.
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Makes the latest tick's prediction now, if it's still to make, by `predict`
    /// (`Predictor::predict` with the controllers given).
    pub fn predict(
        &mut self,
        predict: impl FnOnce(&mut Predictor<crate::power::Run>, Pads) -> Result<Cost, String>,
    ) -> Option<Result<Cost, String>> {
        let pads = self.pending.take()?;
        Some(predict(&mut self.predictor, pads))
    }

    /// The tick's first frame at t = 0 with its prediction still to make: the latest tick's
    /// draws, drawn as they are (the frame blends toward nothing), and the predictor with the
    /// controllers, for the prediction beside the frame. None when nothing is to be predicted.
    pub fn beside(&mut self) -> Option<(&[Draw], &mut Predictor<crate::power::Run>, Pads)> {
        let pads = self.pending.take()?;
        let (f, _) = self.latest.as_ref()?;
        Some((&f.draws, &mut self.predictor, pads))
    }

    /// When the latest tick was due.
    pub fn due(&self) -> Option<Instant> {
        self.latest.as_ref().map(|(_, due)| *due)
    }

    /// What a frame `t` of the way from the tick before (Interpolate) or to the next tick
    /// (Extrapolate) draws: the latest tick's draws, drawn live, and the blend; none, holding
    /// the image shown, when the latest tick drew nothing.
    pub fn frame(&self, mode: Mode, t: f32) -> Option<(&[Draw], Blend<'_>)> {
        let (f, _) = self.latest.as_ref()?;
        if f.draws.is_empty() {
            return None;
        }
        let blend = match mode {
            Mode::Extrapolate => self.predictor.blend(t),
            _ => Blend {
                from: &self.before,
                t,
            },
        };
        Some((&f.draws, blend))
    }
}
