//! The match's sound: each tick's audio frames are mixed (`melee_audio::Mixer`) and queued on
//! the output device. A match runs silent when it has no audio timeline (`docs/player.md`).

use melee_audio::{Mixer, Output};
use melee_sim::game::vs_match::{DiscFile, Match, Settings};

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

/// Starts the match, with its sound played on the default device when `play`.
pub fn start(
    settings: &Settings,
    play: bool,
    load: &mut dyn FnMut(&str) -> DiscFile,
) -> Result<(Match, Option<Sound>), String> {
    let mut mixer = Mixer::new();
    let mut timeline = false;
    let m = Match::start_from(settings, load, &mut |game, _| {
        timeline = game.audio.clock.is_some();
        if timeline {
            mixer.start(&mut game.audio, None);
        }
    });
    if !timeline {
        eprintln!("no audio timeline: the match runs silent");
        return Ok((m, None));
    }
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
