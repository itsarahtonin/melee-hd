//! The match's sound: with a boot state (`--boot-trace`, see [`crate::boot`]) the match starts
//! seeded with the boot's audio library, banks and timeline, so its voices play; each tick's
//! audio frames are mixed (`melee_audio::Mixer`) and queued on the output device. Without a
//! boot state the match runs silent: the boot's audio setup isn't ported in melee-sim, so a
//! match started from its settings alone has no banks and no timeline (`docs/engine.md`).

use melee_audio::{Mixer, Output};
use melee_sim::game::vs_match::{DiscFile, Match, Settings};

use crate::boot::Boot;

/// Mixes each tick's audio frames and plays them.
pub struct Sound {
    mixer: Mixer,
    output: Option<Output>,
    samples: Vec<[i16; 2]>,
}

impl Sound {
    /// Mixes the frames the tick ran and queues them; never waits.
    pub fn tick(&mut self, m: &mut Match) {
        self.samples.clear();
        self.mixer
            .mix(&mut m.game.audio, &m.data, &mut self.samples);
        if let Some(out) = &self.output {
            out.push(&self.samples);
        }
    }
}

/// Starts the match, from the boot state in the trace at `boot_trace` when there is one (with
/// sound, played on the default device when `play`), else from its settings alone (silent).
/// A replay's match also takes its fighters' addresses from the trace.
pub fn start(
    settings: &mut Settings,
    replay: bool,
    boot_trace: Option<&str>,
    play: bool,
    load: &mut dyn FnMut(&str) -> DiscFile,
) -> Result<(Match, Option<Sound>), String> {
    let Some(path) = boot_trace else {
        eprintln!("no boot state (--boot-trace): the match runs silent");
        return Ok((Match::start(settings, load), None));
    };
    let boot = Boot::from_trace(path)?;
    eprintln!("boot state: {path} at tick {}", boot.first_tick - 1);
    let addresses = settings.fighter_addresses.clone();
    boot.apply(settings);
    if !replay {
        settings.fighter_addresses = addresses;
    }
    let mut mixer = Mixer::new();
    let m = Match::start_from(settings, load, &mut |game, data| {
        boot.seed(game, data);
        mixer.start(&mut game.audio, Some(&boot.memory()));
    });
    let output = if play {
        match Output::open() {
            Ok(o) => {
                eprintln!(
                    "sound: {} Hz, {}",
                    o.rate,
                    o.buffer.map_or("the device's buffer".into(), |n| format!(
                        "{n}-sample buffers"
                    ))
                );
                Some(o)
            }
            Err(e) => {
                eprintln!("no sound: {e}");
                None
            }
        }
    } else {
        None
    };
    let sound = Sound {
        mixer,
        output,
        samples: Vec::new(),
    };
    Ok((m, Some(sound)))
}
