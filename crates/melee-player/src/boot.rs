//! The boot state a match starts from when the player has one: what the boot and the menus
//! leave in the original's memory (the audio library's banks, voices and timeline among it),
//! which a match started from its settings alone lacks (`docs/engine.md`). Until the boot is
//! ported, it comes from an ssbm-rs state trace with memory (`STATE_TRACE_MEMORY`, version
//! 5), seeded as the oracle seeds it (`tools/oracle/src/boot.rs`).

use melee_data::archive::Archives;
use melee_sim::game::playback::FIRST_FRAME;
use melee_sim::game::state::Game;
use melee_sim::game::vs_match::Settings;

#[allow(dead_code)]
#[path = "../../../tools/oracle/src/trace.rs"]
mod trace;

#[allow(dead_code)]
#[path = "../../../tools/oracle/src/boot.rs"]
mod seeding;

use melee_capture::import::sym;

/// Memory at the end of the tick before the match's scene, and what the trace says of the
/// match's first tick.
pub struct Boot {
    pub mem1: Vec<u8>,
    /// The time base at the boot state's tick.
    pub tb: u64,
    /// The trace's tick that ran the match's first.
    pub first_tick: u32,
    /// The fighters' addresses, in creation order (`Settings::fighter_addresses`).
    fighter_addresses: Vec<u32>,
}

/// Whether the trace's image is at the VS scene's first tick: the scene loop has counted one
/// tick, with playback's frame count at the replay's first frame.
fn is_first_tick(image: &trace::Image) -> bool {
    image.u32(sym("gm_80479D58")) == 1 && image.u32(0x804D_6CF4) as i32 == FIRST_FRAME
}

impl Boot {
    /// Reads the trace up to the match's first tick.
    pub fn from_trace(path: &str) -> Result<Boot, String> {
        let mut reader = trace::Reader::open(path);
        if reader.version < 5 {
            return Err(format!("{path}: the boot state needs a version 5 trace"));
        }
        let mut image = trace::Image::default();
        loop {
            let before = (image.mem1.clone(), image.tb);
            let Some(t) = reader.tick() else {
                return Err(format!(
                    "{path}: no tick runs the match's first, or no memory in the trace"
                ));
            };
            image.apply(&t);
            if image.seen && is_first_tick(&image) {
                let (mem1, tb) = before;
                let tb = tb.ok_or(format!("{path}: no time base at the boot state"))?;
                let mut fighter_addresses = Vec::new();
                for slot in 0..6 {
                    let player = sym("player_slots") + 0xE90 * slot;
                    for which in 0..2 {
                        let gobj = image.u32(player + 0xB0 + 4 * which);
                        if gobj != 0 {
                            fighter_addresses.push(image.u32(gobj + 0x2C));
                        }
                    }
                }
                return Ok(Boot {
                    mem1,
                    tb,
                    first_tick: t.number,
                    fighter_addresses,
                });
            }
        }
    }

    /// The settings the boot state implies: the save data's unlocks it holds, and the
    /// fighters' addresses from the trace.
    pub fn apply(&self, settings: &mut Settings) {
        settings.fighter_addresses = self.fighter_addresses.clone();
        settings.unlocks = melee_sim::game::gm::Unlocks::from_memory(&|a| {
            let i = (a - 0x8000_0000) as usize;
            u32::from_be_bytes(self.mem1[i..i + 4].try_into().unwrap())
        });
    }

    /// Seeds a new match's state with the boot state (`Match::start_from`'s `boot`).
    pub fn seed(&self, game: &mut Game, data: &mut Archives) {
        seeding::seed(game, data, &self.mem1, self.tb);
    }

    /// The boot state's memory, for the mixer.
    pub fn memory(&self) -> melee_audio::BootMemory<'_> {
        melee_audio::BootMemory {
            mem1: &self.mem1,
            sym: &sym,
        }
    }
}
