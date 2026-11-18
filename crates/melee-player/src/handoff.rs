//! The Slippi ruleset's live matches in the player (`docs/slippi-ruleset.md`, task 6). Under the
//! ruleset melee-sim's session runs each VS match playback can rebuild as a live match of its
//! own (`game::live`), with its own game and archives, whose ids repeat the menus' (both count
//! from the first archive each loaded). A frame builder and renderer keep what they draw from
//! by those ids from frame to frame (the address space's archives, the decoded textures, HSD's
//! and GX's state caches), so the menus' pair can't draw the match, nor the match's the menus
//! after it: the live match draws with a pair of its own, made fresh at the handoff as the
//! Recordings tab makes one for a replay, and the menus' pair is set aside untouched across it
//! and taken back at the handback, as the menus' session is
//! (`melee_sim::game::session::Session::in_live_match`).

use melee_render::frame::Frames;

/// What a renderer was made for: its scale, its EFB's width and its samples a pixel. A renderer
/// set aside misses the changes the player made meanwhile, which it takes when it comes back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    pub scale: u32,
    pub efb_width: u32,
    pub msaa: u32,
}

/// The menus' frame builder and renderer while a live match draws with its own.
pub struct Handoff<R> {
    menus: Option<(Frames, R, Shape)>,
}

impl<R> Default for Handoff<R> {
    fn default() -> Self {
        Handoff { menus: None }
    }
}

/// How the session moved since the last [`Handoff::follow`].
pub enum Moved<R> {
    /// Into or out of nothing: the same frame builder and renderer draw on.
    Stayed,
    /// Into a live match: a fresh pair draws it, the menus' set aside.
    Off,
    /// Out of one: the menus' pair is back, made for this shape, and the match's renderer is
    /// let go (the caller keeps its pipelines).
    Back { shape: Shape, renderer: R },
}

impl<R> Handoff<R> {
    /// Whether a live match's pair is drawing, the menus' set aside.
    pub fn live(&self) -> bool {
        self.menus.is_some()
    }

    /// Follows the session after a tick: `live` is whether it runs a live match now. At the
    /// handoff the menus' frame builder and renderer (`frames`, `renderer`, made as `shape`)
    /// go aside and `fresh` makes the match's; at the handback they come back.
    pub fn follow(
        &mut self,
        live: bool,
        frames: &mut Frames,
        renderer: &mut R,
        shape: Shape,
        fresh: impl FnOnce() -> (Frames, R),
    ) -> Moved<R> {
        match (live, self.menus.take()) {
            (true, None) => {
                let (f, r) = fresh();
                let menus = (
                    std::mem::replace(frames, f),
                    std::mem::replace(renderer, r),
                    shape,
                );
                self.menus = Some(menus);
                Moved::Off
            }
            (false, Some((f, r, shape))) => {
                *frames = f;
                let renderer = std::mem::replace(renderer, r);
                Moved::Back { shape, renderer }
            }
            (_, menus) => {
                self.menus = menus;
                Moved::Stayed
            }
        }
    }
}
