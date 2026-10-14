//! The match's sound: the match starts with the boot's audio setup (melee-sim's `game::boot`),
//! which the mixer sees from its first frame; each tick's audio frames are then mixed
//! (`melee_audio::Mixer`) and queued on the output device.

use std::sync::Mutex;

use melee_audio::{Mixer, Output, OutputOptions, OutputStats};
use melee_sim::game::vs_match::{DiscFile, Match, Settings};

/// How the output device is opened (`--audio-exclusive`).
static OPTIONS: Mutex<OutputOptions> = Mutex::new(OutputOptions { exclusive: false });

/// Opens the matches' output devices this way from now on.
pub fn set_output(options: OutputOptions) {
    *OPTIONS.lock().unwrap() = options;
}

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

    /// What the output device has measured, while it plays.
    pub fn stats(&self) -> Option<OutputStats> {
        self.output.as_ref().map(Output::stats)
    }
}

/// Starts the match, with its sound played on the default device when `play`.
pub fn start(
    settings: &Settings,
    play: bool,
    load: &mut dyn FnMut(&str) -> DiscFile,
) -> Result<(Match, Option<Sound>), String> {
    let mut mixer = Mixer::new();
    // The mix log turned on before the boot's audio setup runs, so the mixer sees its ARAM
    // writes and frames.
    let mut m = Match::start_from(settings, load, &mut |game, _| {
        mixer.start(&mut game.audio, None);
    });
    if m.game.audio.clock.is_none() {
        eprintln!("no audio timeline: the match runs silent");
        m.game.audio.mix = None;
        return Ok((m, None));
    }
    let output = if play {
        match Output::open_with(*OPTIONS.lock().unwrap()) {
            Ok(o) => {
                eprintln!("sound: {} Hz, {}", o.rate, o.describe);
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
