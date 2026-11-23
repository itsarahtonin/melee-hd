//! The oracle's entry points into a running session, for its mirrors of ssbm-rs's runner hooks
//! (`melee-oracle session --hooks`, [the oracle](../../../../docs/oracle.md#sessions)): ssbm-rs's
//! lockstep campaign started most of its runs past the title (`--mode`), with each match drawn
//! from a seed (`--matches`, `MATCH_*`) and its humans made computer players (`CPU_PLAYERS`),
//! writes its runner makes at a function's entry. A session recorded from such a run plays back
//! only with the same writes at the same points, which the oracle's hook makes from these.
//!
//! Test-only: the simulation never installs a hook or sets a mutation, and the player and a
//! recording can't; only `melee-oracle` does. A run-time switch, not a cargo feature (the
//! refactor's task 26), so the oracle, the checks and the player link one build of the
//! simulation: the first setter called in the process turns the reads on, and until then each
//! entry point and mutation read is one relaxed load of a static. Each entry point is named for
//! the original function whose entry ssbm-rs's hook runs at.

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};

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

/// Whether any thread has installed a hook or set a mutation: never in the player.
static ON: AtomicBool = AtomicBool::new(false);

fn turn_on() {
    ON.store(true, Ordering::Relaxed);
}

#[inline]
fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

thread_local! {
    static HOOK: Cell<Option<Hook>> = const { Cell::new(None) };
    static NO_DYNAMICS_FIX: Cell<bool> = const { Cell::new(false) };
    static NO_HANDBACK_STATICS: Cell<bool> = const { Cell::new(false) };
    static FRESH_COUNTERS: Cell<bool> = const { Cell::new(false) };
}

/// Leaves a live match's counters as playback's boot starts them (`live::Counters`) on this
/// thread, where it takes the console's: the oracle's mutation (`melee-oracle session --mutate
/// fresh-counters`), to see its comparison find them.
pub fn set_fresh_counters(on: bool) {
    turn_on();
    FRESH_COUNTERS.with(|n| n.set(on));
}

/// Whether [`set_fresh_counters`] left them fresh.
#[inline]
pub(crate) fn fresh_counters() -> bool {
    on() && FRESH_COUNTERS.with(Cell::get)
}

/// Leaves out the statics a live match's handback takes back (`Game::take_match_statics`) on
/// this thread: the oracle's mutation (`melee-oracle session --mutate no-handback-statics`),
/// to see its comparison past the handback find them.
pub fn set_handback_statics_off(off: bool) {
    turn_on();
    NO_HANDBACK_STATICS.with(|n| n.set(off));
}

/// Whether [`set_handback_statics_off`] left them out.
#[inline]
pub(crate) fn handback_statics_off() -> bool {
    on() && NO_HANDBACK_STATICS.with(Cell::get)
}

/// Turns Slippi's DynamicsFix off on this thread (`Game::dynamics_fix`): the oracle's mutation
/// (`melee-oracle session --mutate no-dynamics-fix`), to see what its comparison finds
/// without the code.
pub fn set_dynamics_fix_off(off: bool) {
    turn_on();
    NO_DYNAMICS_FIX.with(|n| n.set(off));
}

/// Whether [`set_dynamics_fix_off`] turned DynamicsFix off.
#[inline]
pub(crate) fn dynamics_fix_off() -> bool {
    on() && NO_DYNAMICS_FIX.with(Cell::get)
}

/// Runs `hook` at every entry point on this thread from now on (none with `None`).
pub fn install(hook: Option<Hook>) {
    turn_on();
    HOOK.with(|h| h.set(hook));
}

/// The installed hook's run at `at`, if one is installed.
pub(crate) fn reach(game: &mut Game, at: Entry) {
    if !on() {
        return;
    }
    if let Some(hook) = HOOK.with(Cell::get) {
        hook(game, at);
    }
}

/// The start data a match's scene in `scene_mode` takes (the mode's state's enter data, which
/// `fn_8016E730`'s argument points at), as the scene's entry reads it.
pub fn scene_enter_data(game: &mut Game, scene_mode: u8) -> &mut StartMeleeData {
    super::gm::attract::scene_enter_data(game, scene_mode)
}
