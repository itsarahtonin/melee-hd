//! The in-match HUD and the text objects a match creates (`melee/if`): the damage
//! percentages and stock icons, the timer, the Ready/GO!/GAME! announcements, the name tags
//! and the rest of what a VS match puts on screen, and the debug text screens every scene
//! sets up. Their GObjs, joints, animations and timers live in the game's state and run in
//! the scheduler's lists, so they are simulation; only their drawing is presentation, and it
//! becomes draw commands.

pub mod devtext;

/// The HUD's globals, one field per unit's statics.
#[derive(Clone, Debug, Default)]
pub struct Hud {
    /// The debug text screens (`textdraw.c`, `textlib.c`).
    pub devtext: devtext::DevTexts,
}
