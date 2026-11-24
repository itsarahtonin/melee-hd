//! The structs every match mode shares, hand-written ([design call 3's second
//! half](../../../../../docs/refactor.md#design-call-3s-second-half-the-owned-structs)): a
//! match's start data (`StartMeleeData`, its rules and six players), the match scene's state
//! (`VsSceneController`), what a match's end leaves for the results (`MatchEnd`), the scene
//! loop's counters and debug pause (`SceneLoop`), and the menus' controller map
//! (`ControllerMap`). Each mode's registry entry ([`super::registry::Mode`]) names where its
//! start data lives; the structs one mode keeps live beside its module (`gm::event`'s
//! `EventData`, `gm::one_player`'s `OnePlayerChoice`), and the game's data with the modes'
//! saved settings beside `gm::main_lib`. Each field's `#[at]` is its place in the original's
//! struct, which the comparer's mirror reads (`#[derive(Mirror)]`) and the layout check holds
//! to the decomp's (`game::mirror`); the fields keep the original's order, so the sync test's
//! hash reads them as the generated structs' did.

mod input;
mod match_end;
mod scene_loop;
mod start;
mod vs_scene;

pub use input::*;
pub use match_end::*;
pub use scene_loop::*;
pub use start::*;
pub use vs_scene::*;
