//! The oracle's entry points into a running session, for its mirrors of ssbm-rs's runner hooks
//! (`melee-oracle session --hooks`, [the oracle](../../../../docs/oracle.md#sessions)): ssbm-rs's
//! lockstep campaign started most of its runs past the title (`--mode`), with each match drawn
//! from a seed (`--matches`, `MATCH_*`) and its humans made computer players (`CPU_PLAYERS`),
//! writes its runner makes at a function's entry. A session recorded from such a run plays back
//! only with the same writes at the same points, which the oracle's hook makes from these.
//!
//! Test-only, behind the feature `oracle-hooks`, which only `melee-oracle` turns on: the
//! simulation never installs a hook, and the player and a recording can't. Each entry point is
//! named for the original function whose entry ssbm-rs's hook runs at.

use std::cell::Cell;

use super::state::Game;
use super::structs::StartMeleeData;

/// Where a hook runs: a function's entry, before its first instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    /// `runGameMode`'s, the mode it starts in `game.machine.routing.curr_mode` (its argument,
    /// which `gm_801A4B60` read from there).
    RunGameMode,
    /// `onEnterDebugVs`'s, the debug VS mode's state's entry.
    OnEnterDebugVs,
    /// `gm_LoadAnnouncer`'s.
    LoadAnnouncer,
    /// `fn_8016E730`'s, from a match's scene entry (`gm_Scene_Vs_OnEnter`,
    /// `gm_Scene_SuddenDeath_OnEnter`, `gm_Scene_Training_OnEnter`) in the scene mode given: its
    /// argument is that mode's enter data ([`scene_enter_data`]).
    MatchStart { scene_mode: u8 },
}

/// A hook: what the oracle writes at an entry point.
pub type Hook = fn(&mut Game, Entry);

thread_local! {
    static HOOK: Cell<Option<Hook>> = const { Cell::new(None) };
}

/// Runs `hook` at every entry point on this thread from now on (none with `None`).
pub fn install(hook: Option<Hook>) {
    HOOK.with(|h| h.set(hook));
}

/// The installed hook's run at `at`, if one is installed.
pub(crate) fn reach(game: &mut Game, at: Entry) {
    if let Some(hook) = HOOK.with(Cell::get) {
        hook(game, at);
    }
}

/// The start data a match's scene in `scene_mode` takes (the mode's state's enter data, which
/// `fn_8016E730`'s argument points at), as the scene's entry reads it.
pub fn scene_enter_data(game: &mut Game, scene_mode: u8) -> &mut StartMeleeData {
    super::gm::attract::scene_enter_data(game, scene_mode)
}
