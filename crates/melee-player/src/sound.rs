//! The match's sound: the match starts with the boot's audio setup (melee-sim's `game::boot`),
//! which the mixer sees from its first frame; each tick's audio frames are then mixed
//! (`melee_audio::Mixer`) and queued on the output device. A long tick's (a scene's load, which
//! runs the load's whole length on Melee's timeline) are queued to play whole, as the player
//! waits that long before its next tick (`jit::Scheduler::took`).

use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use melee_audio::{Mixer, Output, OutputOptions, OutputStats};
use melee_data::archive::Archives;
use melee_sim::game::session::{Session, Setup};
use melee_sim::game::vs_match::{DiscFile, Match, Settings};

/// How the output device is opened (`--audio-exclusive`, the settings panel's Audio).
static OPTIONS: Mutex<OutputOptions> = Mutex::new(OutputOptions {
    exclusive: false,
    device: None,
});
/// The volume, in percent.
static VOLUME: AtomicU32 = AtomicU32::new(100);

/// Opens the matches' output devices this way from now on.
pub fn set_output(options: OutputOptions) {
    *OPTIONS.lock().unwrap_or_else(|e| e.into_inner()) = options;
}

/// Plays the matches' sound at `percent` of its volume from now on.
pub fn set_volume(percent: u32) {
    VOLUME.store(percent.min(100), Ordering::Relaxed);
}

/// What the samples are multiplied by at `percent`: the square, which a listener hears as
/// about even steps (50% is a quarter, 12 dB down); 100% plays the mix as it is.
pub fn gain(percent: u32) -> f32 {
    let v = percent.min(100) as f32 / 100.0;
    v * v
}

fn options() -> OutputOptions {
    OPTIONS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Mixes each tick's audio frames and plays them.
pub struct Sound {
    mixer: Mixer,
    output: Option<Output>,
    samples: Vec<[i16; 2]>,
}

impl Sound {
    /// Mixes the frames the tick ran and queues them, to play whole when the tick was `long`;
    /// never waits.
    pub fn mix(&mut self, audio: &mut melee_sim::audio::Audio, data: &Archives, long: bool) {
        self.samples.clear();
        self.mixer.mix(audio, data, &mut self.samples);
        match &self.output {
            Some(out) if long => out.push_long(&self.samples),
            Some(out) => out.push(&self.samples),
            None => {}
        }
    }

    /// The samples the last tick mixed.
    pub fn samples(&self) -> &[[i16; 2]] {
        &self.samples
    }

    /// What the output device has measured, while it plays.
    pub fn stats(&self) -> Option<OutputStats> {
        self.output.as_ref().map(Output::stats)
    }

    /// Plays by the output's options and volume as they stand now (the device is opened again
    /// if they name another or open it another way). Only how the sound is heard changes: the
    /// mix, let alone the game, never sees it.
    pub fn configure(&self) {
        if let Some(out) = &self.output {
            out.configure(&options());
            out.set_volume(gain(VOLUME.load(Ordering::Relaxed)));
        }
    }
}

/// The output device when `play`, else none.
fn output(play: bool) -> Option<Output> {
    if !play {
        return None;
    }
    match Output::open_with(options()) {
        Ok(o) => {
            eprintln!("sound: {} Hz, {}", o.rate, o.describe);
            o.set_volume(gain(VOLUME.load(Ordering::Relaxed)));
            Some(o)
        }
        Err(e) => {
            eprintln!("no sound: {e}");
            None
        }
    }
}

/// The console at power-on, its sound mixed from the boot's audio setup on and played on the
/// output device when `play`: the boot is the caller's, the sound's first mix after it.
pub fn power_on(setup: Setup, play: bool) -> Result<(Session, Option<Sound>), String> {
    let mut s = Session::new(setup);
    let mut mixer = Mixer::new();
    mixer.start(&mut s.game.audio, None);
    let sound = Sound {
        mixer,
        output: output(play),
        samples: Vec::new(),
    };
    Ok((s, Some(sound)))
}

/// Starts the match, with its sound played on the output device when `play`.
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
    let sound = Sound {
        mixer,
        output: output(play),
        samples: Vec::new(),
    };
    Ok((m, Some(sound)))
}
