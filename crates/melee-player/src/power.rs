//! The console from power-on (milestone 5's task 18): the player boots Melee as a console does,
//! through the memory card check, the opening movie and the title into the menus, and every
//! match starts from the game's own menus. melee-sim's `game::session` runs it; what the
//! console's platform hands the game is the player's: the controllers as they are when the
//! game reads them, the disc, the memory card made from the saves folder ([`crate::saves`]),
//! the host's local time at power-on as the real-time clock, no component cable, the SRAM's
//! stereo setting, a drive's pace and the DSP mixing sound.
//!
//! Every read the game takes is recorded ([`crate::session_rec`]), so a session played here is
//! one ssbm-rs and the oracle can play again.

use melee_data::disc::Disc;
use melee_render::predict::Ahead;
use melee_sim::audio::clock::{DiscSpeed, TB_HZ};
use melee_sim::audio::dsp::DspModel;
use melee_sim::game::session::{self, Session, Setup};
use melee_sim::game::state::Game;
use melee_sim::game::vs_match::{DiscFile, Frame, Match};
use melee_sim::input::controller::PadData;

use crate::session_rec::Writer;

/// What the player runs: a match (a replay's, or the command line's for developers), or the
/// console from power-on.
#[derive(Clone)]
pub enum Run {
    Match(Box<Match>),
    Console(Box<Session>),
}

impl Run {
    pub fn game(&self) -> &Game {
        match self {
            Run::Match(m) => &m.game,
            Run::Console(s) => &s.game,
        }
    }

    pub fn game_mut(&mut self) -> &mut Game {
        match self {
            Run::Match(m) => &mut m.game,
            Run::Console(s) => &mut s.game,
        }
    }

    pub fn data(&self) -> &melee_data::archive::Archives {
        match self {
            Run::Match(m) => &m.data,
            Run::Console(s) => &s.data,
        }
    }

    /// Takes a frame's buffers back.
    pub fn recycle(&mut self, f: Frame) {
        match self {
            Run::Match(m) => m.recycle(f),
            Run::Console(s) => {
                let g = &mut s.game;
                let (mut events, mut draws) = (f.events, f.draws);
                events.clear();
                draws.clear();
                if g.events.is_empty() && g.events.capacity() < events.capacity() {
                    g.events = events;
                }
                let d = &mut g.objs.display.draws;
                if d.is_empty() && d.capacity() < draws.capacity() {
                    *d = draws;
                }
            }
        }
    }
}

impl Ahead for Run {
    fn game(&self) -> &Game {
        Run::game(self)
    }

    fn game_mut(&mut self) -> &mut Game {
        Run::game_mut(self)
    }

    fn restore_from(&mut self, real: &Self) {
        match (self, real) {
            (Run::Match(c), Run::Match(r)) => (**c).restore_from(r),
            (Run::Console(c), Run::Console(r)) => {
                c.game.clone_from(&r.game);
                // The archives are shared: each a reference counted.
                c.data.clone_from(&r.data);
                c.setup.clone_from(&r.setup);
                c.card.clone_from(&r.card);
            }
            (c, r) => {
                *c = r.clone();
                c.game_mut().audio.mix = None;
            }
        }
    }

    fn recycle(&mut self, frame: Frame) {
        Run::recycle(self, frame);
    }
}

/// The console's clock at power-on: the host's local time, in time base ticks from 2000-01-01
/// (the OS's epoch), as a console's real-time clock holds the time its owner set.
pub fn power_on_clock() -> u64 {
    let [y, mo, d, h, mi, s] = crate::recording::local_now();
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |t| u64::from(t.subsec_millis()));
    let days = days_since_2000(y, mo, d);
    let seconds = days * 86_400 + u64::from(h * 3600 + mi * 60 + s);
    seconds * TB_HZ + ms * TB_HZ / 1000
}

/// Days from 2000-01-01 to a date of the Gregorian calendar (0 before it).
fn days_since_2000(y: u32, m: u32, d: u32) -> u64 {
    let days_from_civil = |y: i64, m: i64, d: i64| {
        let y = if m <= 2 { y - 1 } else { y };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe
    };
    let n = days_from_civil(i64::from(y), i64::from(m), i64::from(d)) - days_from_civil(2000, 1, 1);
    n.max(0) as u64
}

/// The console the player's sessions run on: a drive's pace, the DSP mixing, the card, the
/// clock at power-on, no component cable and the SRAM's stereo setting (ssbm-rs's defaults).
pub fn setup(card: Vec<u8>, power_on: u64) -> Setup {
    Setup {
        disc: DiscSpeed::Drive,
        dsp: DspModel::Samples,
        card: Some(card),
        power_on,
        dtv: false,
        sram_flags: session::SRAM_FLAGS_DEFAULT,
        card_pull: None,
    }
}

/// Where a step's reads come from: the controllers as they are now, or a recording's reads
/// (`--inputs`), the last again past its end as ssbm-rs's playback does.
pub enum Source<'a> {
    Now(PadData),
    Reads { reads: &'a [PadData], next: usize },
}

/// The console's platform while a step runs: every read takes the controllers from its
/// source, each read the game keeps recorded; the disc's files read as the game asks.
pub struct Live<'a> {
    source: Source<'a>,
    pub disc: &'a Disc,
    /// The reads handed out since the last record, of which the game may give some back.
    handed: usize,
}

impl<'a> Live<'a> {
    pub fn new(disc: &'a Disc, source: Source<'a>) -> Live<'a> {
        Live {
            source,
            disc,
            handed: 0,
        }
    }

    /// The reads the game kept since the last call, into the recording; the source's next
    /// read.
    pub fn record(&mut self, rec: Option<&mut Writer>) -> usize {
        let next = match self.source {
            Source::Now(_) => 0,
            Source::Reads { next, .. } => next,
        };
        if let Some(w) = rec {
            match self.source {
                Source::Now(p) => {
                    for _ in 0..self.handed {
                        w.read(&p, None);
                    }
                }
                Source::Reads { reads, next } => {
                    for i in next - self.handed..next {
                        w.read(&reads[i.min(reads.len() - 1)], None);
                    }
                }
            }
        }
        self.handed = 0;
        next
    }
}

impl session::Console for Live<'_> {
    fn read_pads(&mut self) -> PadData {
        self.handed += 1;
        match &mut self.source {
            Source::Now(p) => *p,
            Source::Reads { reads, next } => {
                let p = reads[(*next).min(reads.len() - 1)];
                *next += 1;
                p
            }
        }
    }

    fn unread(&mut self, n: usize) {
        self.handed -= n;
        if let Source::Reads { next, .. } = &mut self.source {
            *next -= n;
        }
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

    fn file_after(&mut self, path: &str) -> Vec<u8> {
        let path = path.trim_start_matches('/');
        self.disc
            .read_after(path)
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

/// Extrapolate's console: the controllers held, and no disc. A predicted tick that would read
/// a file (a scene's entry, a song started) stops, and the frames draw the real tick.
struct Held(PadData);

impl session::Console for Held {
    fn read_pads(&mut self) -> PadData {
        self.0
    }

    fn unread(&mut self, _: usize) {}

    fn entrynum(&mut self, path: &str) -> Option<i32> {
        panic!("the prediction asks the disc for {path}")
    }

    fn file(&mut self, path: &str) -> DiscFile {
        panic!("the prediction reads {path}")
    }
}

/// One tick of the console with `console`'s controllers, its display pass drawn when due.
pub fn step(s: &mut Session, console: &mut dyn session::Console) -> Frame {
    let (mut f, render) = s.step_tick(console);
    if render {
        f.draws = s.render();
        // The display pass's wait can write the card and start the save's events.
        f.events.append(&mut s.game.events);
    }
    f
}

/// Extrapolate's step on the copy: none (no draws) when the tick starts a new scene, whose
/// files only the real console reads, or takes a Camera Mode snapshot, whose frame is the real
/// tick's.
pub fn step_predicted(s: &mut Session, pads: PadData) -> Frame {
    let busy = s.game.scene.unk_C != 0 || s.game.camera_mode.snap.image != 0;
    if busy {
        return Frame {
            events: Vec::new(),
            pads: Default::default(),
            ended: false,
            draws: Vec::new(),
        };
    }
    step(s, &mut Held(pads))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clock_counts_from_2000() {
        assert_eq!(days_since_2000(2000, 1, 1), 0);
        assert_eq!(days_since_2000(2000, 3, 1), 60);
        assert_eq!(days_since_2001(), 366);
        assert_eq!(days_since_2000(2026, 10, 28), 9797);
    }

    fn days_since_2001() -> u64 {
        days_since_2000(2001, 1, 1)
    }
}
