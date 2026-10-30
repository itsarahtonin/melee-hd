//! `melee-audio-check --session INPUTS AUDIO.wav [--card FILE] [--ticks N] [--window SECONDS]
//! [--out MIX.wav]`: plays a recorded session (`SSBMINP1`, [the oracle](../../../docs/oracle.md#sessions))
//! natively from power-on with its console's setup and reads, mixes its audio with
//! melee-audio from the boot's audio setup on, and compares the samples with ssbm-rs's
//! recording of the same session's playback (`AUDIO_OUT=AUDIO.wav ssbm-run ... --inputs
//! INPUTS`), as the replay mode compares a match's. The session runs to its last read or to
//! `--ticks`, or to the first function it reaches that isn't ported.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use melee_audio::Mixer;
use melee_data::disc::Disc;
use melee_sim::game::inputs::Recording;
use melee_sim::game::session::{Session, Setup};
use melee_sim::game::vs_match::DiscFile;
use melee_sim::input::controller::PadData;

use super::{Sample, compare, read_wav, write_wav};

/// The recording's reads, handed out in order (past the end, the last again), and the disc.
struct Console<'a> {
    reads: &'a [PadData],
    next: usize,
    disc: &'a Disc,
}

impl melee_sim::game::session::Console for Console<'_> {
    fn read_pads(&mut self) -> PadData {
        let pads = self.reads[self.next.min(self.reads.len() - 1)];
        self.next += 1;
        pads
    }

    fn unread(&mut self, n: usize) {
        self.next -= n;
    }

    fn entrynum(&mut self, path: &str) -> Option<i32> {
        self.disc.entrynum(path.trim_start_matches('/')).ok()
    }

    fn file_info(&mut self, path: &str) -> (i32, u32) {
        let path = path.trim_start_matches('/');
        self.disc
            .file_info(path)
            .unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    fn file(&mut self, path: &str) -> DiscFile {
        let path = path.trim_start_matches('/');
        DiscFile {
            entrynum: self
                .disc
                .entrynum(path)
                .unwrap_or_else(|e| panic!("{path}: {e}")),
            bytes: self
                .disc
                .read_file(path)
                .unwrap_or_else(|e| panic!("{path}: {e}")),
        }
    }
}

fn panic_text(p: &Box<dyn std::any::Any + Send>) -> String {
    p.downcast_ref::<String>()
        .cloned()
        .or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_else(|| "a panic".to_owned())
}

pub fn run(args: &[String]) -> Result<bool, String> {
    let mut pos = Vec::new();
    let (mut ticks, mut window, mut out) = (u64::MAX, 1.0f64, None);
    let (mut inputs, mut card_path) = (None, None);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--session" => inputs = Some(value()?.clone()),
            "--card" => card_path = Some(value()?.clone()),
            "--ticks" => ticks = value()?.parse().map_err(|_| "--ticks N")?,
            "--window" => window = value()?.parse().map_err(|_| "--window SECONDS")?,
            "--out" => out = Some(value()?.clone()),
            _ => pos.push(a.as_str()),
        }
    }
    let (Some(inputs), [wav_path]) = (inputs, &pos[..]) else {
        return Err("usage: melee-audio-check --session INPUTS AUDIO.wav [--card FILE] [--ticks N] [--window SECONDS] [--out MIX.wav]".into());
    };
    let rec = Recording::read(Path::new(&inputs)).map_err(|e| e.to_string())?;
    if rec.reads.is_empty() {
        return Err(format!("{inputs}: no reads"));
    }
    let card = match card_path.map(Into::into).or(rec.card.clone()) {
        Some(p) => Some(std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?),
        None => None,
    };
    let setup = Setup {
        disc: rec.disc().map_err(|e| format!("{inputs}: {e}"))?,
        dsp: rec.dsp(),
        card,
        power_on: rec.power_on,
        dtv: rec.dtv,
        sram_flags: rec.sram_flags,
        card_pull: None,
        card_b: None,
        card_b_pull: None,
        develop: rec.developer(),
    };
    let disc_path = std::env::var("SSBM_DISC").map_err(|_| "SSBM_DISC names the disc")?;
    let disc = Disc::open(&disc_path).map_err(|e| format!("{disc_path}: {e}"))?;
    let theirs = read_wav(wav_path)?;
    let mut console = Console {
        reads: &rec.reads,
        next: 0,
        disc: &disc,
    };
    let mut s = Session::new(setup);
    let mut mixer = Mixer::new();
    mixer.start(&mut s.game.audio, None);
    let mut ours: Vec<Sample> = Vec::new();
    let mut ran = 0u64;
    let stop = catch_unwind(AssertUnwindSafe(|| {
        s.boot(&mut console);
        mixer.mix(&mut s.game.audio, &s.data, &mut ours);
        while ran < ticks && console.next < rec.reads.len() {
            let (_, render) = s.step_tick(&mut console);
            if render {
                s.render();
            }
            mixer.mix(&mut s.game.audio, &s.data, &mut ours);
            ran += 1;
        }
    }));
    match stop {
        Ok(()) => println!("{ran} ticks, to read {}", console.next),
        Err(p) => println!("{ran} ticks; stopped: {}", panic_text(&p)),
    }
    let st = &mixer.stats;
    println!(
        "mixed {} frames ({:.2} s); drifted voice frames {}, ARAM writes from memory {}, from missing files {} {:?}",
        st.frames,
        ours.len() as f64 / super::RATE as f64,
        st.drift,
        st.memory_writes,
        st.missing_files,
        st.missing
            .iter()
            .map(|&e| disc.path_of(e).unwrap_or_else(|_| format!("entry {e}")))
            .collect::<Vec<_>>()
    );
    if let Some(path) = &out {
        write_wav(path, &ours).map_err(|e| format!("{path}: {e}"))?;
    }
    compare(&ours, &theirs, window)
}
