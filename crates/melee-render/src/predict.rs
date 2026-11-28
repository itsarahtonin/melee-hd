//! Extrapolate's prediction (docs/architecture.md, "Frame pacing and high refresh rates"): the
//! next tick run ahead of time on a copy of the state, its display pass kept as the far end of
//! the frames drawn until the real tick comes.
//!
//! After each real tick N, [`Predictor::predict`] restores its copy from the real state
//! (`clone_from`, reusing the copy's storage), runs one tick on it with the inputs the caller
//! predicts (the latest held, as rollback predicts) and keeps that tick's display pass in a
//! [`Capture`]. Frames until tick N+1 draw tick N live, its transforms blended toward the
//! capture ([`Predictor::blend`]), so what a frame shows (the HUD, colors, what exists) is
//! always the real tick's and only motion is predicted; at t = 0 a frame is tick N's own,
//! command for command.
//!
//! **No side effects.** The real state is only read. The copy's tick hands its sounds, rumble
//! and other events to nobody (its frame goes back to the copy unread), and the copy's mix log
//! is off: a clone starts without it and a restore leaves the copy's as it was
//! (`#[clone_from(keep)]` on `Audio::mix`), so the mixer never sees the predicted tick's
//! audio. The archives the copy shares are loaded data a tick only reads.
//!
//! **On a thread of its own.** Only the frames after the real tick's first need the prediction:
//! that first frame, at t = 0, is the tick's own and blends toward nothing. So the player and
//! the bench make it with [`Predictor::predict_beside`], on a thread kept for it, while the
//! calling thread draws that first frame from the real state; both only read the real state,
//! and the copy and the capture are the prediction's alone until it's done. The thread lives
//! as long as the predictor, so the simulation's memos (`melee_sim::memo`, kept per thread)
//! stay warm from tick to tick, and runs on the game's stack ([`STACK`]). The floating point
//! mode is the thread's too: the caller's `step` sets it, as on any thread a tick runs on.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::{Duration, Instant};

use melee_sim::game::state::Game;
use melee_sim::game::vs_match::{Frame, Match};

use crate::blend::{Blend, Capture};

/// What a prediction runs on: a match, or a console's session from power-on (the player's).
/// The copy is the real state cloned once; each prediction restores it from the real state,
/// reusing its storage.
pub trait Ahead: Clone {
    fn game(&self) -> &Game;
    fn game_mut(&mut self) -> &mut Game;
    /// Restores this copy from `real`.
    fn restore_from(&mut self, real: &Self);
    /// Takes a frame's buffers back.
    fn recycle(&mut self, frame: Frame);
}

impl Ahead for Match {
    fn game(&self) -> &Game {
        &self.game
    }

    fn game_mut(&mut self) -> &mut Game {
        &mut self.game
    }

    /// The archives are fixed once the match has started; only the state changes.
    fn restore_from(&mut self, real: &Self) {
        self.game.clone_from(&real.game);
    }

    fn recycle(&mut self, frame: Frame) {
        Match::recycle(self, frame);
    }
}

/// The stack the prediction's thread runs the predicted tick on: the game's, as the player's game
/// thread has it (the simulation recurses as the original does, through joint trees).
pub const STACK: usize = 512 << 20;

/// What a prediction took, by part.
#[derive(Clone, Copy, Debug, Default)]
pub struct Cost {
    /// Restoring the copy from the real state.
    pub clone: Duration,
    /// The predicted tick, its display pass included.
    pub tick: Duration,
    /// Keeping its display pass's transforms.
    pub capture: Duration,
}

impl Cost {
    pub fn total(&self) -> Duration {
        self.clone + self.tick + self.capture
    }
}

/// The copy of the state the next tick runs on, and what its display pass left.
pub struct Predictor<S: Ahead = Match> {
    copy: Option<S>,
    capture: Capture,
    /// The thread [`Predictor::predict_beside`] predicts on, made at its first prediction.
    thread: Option<rayon::ThreadPool>,
}

impl<S: Ahead> Default for Predictor<S> {
    fn default() -> Self {
        Predictor {
            copy: None,
            capture: Capture::default(),
            thread: None,
        }
    }
}

/// What [`Predictor::predict_beside`] did beside the caller's work.
#[derive(Clone, Copy, Debug, Default)]
pub struct Beside {
    /// How long the caller waited for the prediction after its own work was done: none when
    /// the prediction finished first.
    pub waited: Duration,
}

impl<S: Ahead> Predictor<S> {
    /// Predicts the tick after the one `real` has just run: restores the copy from `real`, runs
    /// `step` on it (one tick with the inputs predicted) and keeps its display pass. A tick
    /// without a display pass, or one that stops (unported code, which the real tick would
    /// reach next), leaves no prediction, and frames draw the real tick as it is.
    pub fn predict(
        &mut self,
        real: &S,
        step: impl FnOnce(&mut S) -> Frame,
    ) -> Result<Cost, String> {
        predict_into(&mut self.copy, &mut self.capture, real, step)
    }

    /// [`Predictor::predict`] on the prediction's own thread while `here` runs on this one;
    /// returns once both are done, with what `here` returned. `here` must not need the
    /// prediction: it's the real tick's first frame, drawn as the tick left the state. Both read
    /// `real`; nothing writes it until this returns.
    pub fn predict_beside<R>(
        &mut self,
        real: &S,
        step: impl FnOnce(&mut S) -> Frame + Send,
        here: impl FnOnce() -> R,
    ) -> (Result<Cost, String>, R, Beside)
    where
        S: Send + Sync,
    {
        let Predictor {
            copy,
            capture,
            thread,
        } = self;
        let pool = thread.get_or_insert_with(|| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(1)
                .stack_size(STACK)
                .thread_name(|_| "predict".to_owned())
                .build()
                .expect("the prediction's thread")
        });
        let mut made = None;
        let into = &mut made;
        let mut here_done = None;
        let r = pool.in_place_scope(|s| {
            s.spawn(move |_| *into = Some(predict_into(copy, capture, real, step)));
            let r = here();
            here_done = Some(Instant::now());
            r
        });
        let waited = here_done.map_or(Duration::ZERO, |t| t.elapsed());
        (made.expect("the prediction ran"), r, Beside { waited })
    }

    /// Forgets the prediction and the copy, for a new match (the thread stays).
    pub fn clear(&mut self) {
        self.copy = None;
        self.capture.clear();
    }

    /// Forgets the prediction, keeping the copy's storage: frames draw the real tick as it is.
    pub fn forget(&mut self) {
        self.capture.clear();
    }

    /// Whether a prediction is kept.
    pub fn is_filled(&self) -> bool {
        self.capture.is_filled()
    }

    /// The blend for a frame `t` of the way from the real tick (0) to the predicted one (1):
    /// the real tick is drawn live, so the blend runs from the prediction's capture back to it.
    pub fn blend(&self, t: f32) -> Blend<'_> {
        Blend {
            from: &self.capture,
            t: 1.0 - t.clamp(0.0, 1.0),
        }
    }

    /// The predicted tick's display pass, as kept.
    pub fn capture(&self) -> &Capture {
        &self.capture
    }

    /// The copy as the predicted tick left it.
    pub fn predicted(&self) -> Option<&S> {
        self.copy.as_ref()
    }
}

/// [`Predictor::predict`] on the predictor's copy and capture: restores the copy from `real`,
/// runs `step` on it and keeps its display pass.
fn predict_into<S: Ahead>(
    copy: &mut Option<S>,
    capture: &mut Capture,
    real: &S,
    step: impl FnOnce(&mut S) -> Frame,
) -> Result<Cost, String> {
    let t0 = Instant::now();
    match copy {
        Some(copy) => copy.restore_from(real),
        None => {
            let mut c = real.clone();
            c.game_mut().audio.mix = None;
            *copy = Some(c);
        }
    }
    let copy = copy.as_mut().expect("the copy");
    let t1 = Instant::now();
    let frame = match catch_unwind(AssertUnwindSafe(|| step(copy))) {
        Ok(f) => f,
        Err(p) => {
            capture.clear();
            // Half a tick's state: restored in full next time.
            let why = p
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                .unwrap_or_else(|| "a panic".to_owned());
            return Err(why);
        }
    };
    let t2 = Instant::now();
    if frame.draws.is_empty() {
        capture.clear();
    } else {
        capture.fill(copy.game(), &frame.draws);
    }
    // Its sounds, rumble and events go nowhere.
    copy.recycle(frame);
    let t3 = Instant::now();
    Ok(Cost {
        clone: t1 - t0,
        tick: t2 - t1,
        capture: t3 - t2,
    })
}
