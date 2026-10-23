//! A console session from power-on: the boot (`main`), then the game modes' scenes, each engine
//! tick taking the controllers the session reads. Everything the console's platform hands the
//! game is an explicit input ([`Setup`], [`Console`]): the memory card, the disc's pace, whether
//! the DSP mixes sound, the real-time clock, and every `PADRead`'s controllers, in the order the
//! game reads them.
//!
//! Only the boot's first steps are ported: [`Session::boot`] stops at the first function no
//! port covers yet, naming it, as a match does (`code::unported_call`). A session taken up from
//! a state (an imported trace tick, [`Session::resume`]) plays on with [`Session::step_tick`].

use melee_data::archive::Archives;

use super::code;
use super::gm::scene;
use super::state::{Game, Running};
use super::vs_match::Frame;
use crate::audio::clock::DiscSpeed;
use crate::audio::dsp::DspModel;
use crate::input::controller::PadData;
use crate::input::pad::{ERR_NONE, ERR_NOT_READY, ERR_TRANSFER};

/// What a session starts with besides its controllers: the console's setup.
#[derive(Clone, Debug, Default)]
pub struct Setup {
    /// How fast the disc reads (`vs_match::Settings::disc`).
    pub disc: DiscSpeed,
    /// Which DSP model the voices follow (`vs_match::Settings::dsp`).
    pub dsp: DspModel,
    /// The memory card in slot A as the session found it (its raw image), or none. The card
    /// library's setup (`lbCardNew_Init`) reads it; until that is ported nothing does.
    pub card: Option<Vec<u8>>,
    /// The real-time clock at power-on, in seconds since 2000-01-01 (the OS's epoch); it then
    /// runs with the time base. It reaches gameplay: the boot seeds the random seed from the
    /// time base (`OSGetTick`) and the title draws from it once for each second of the clock's
    /// minute (`gmTitle_801A165C`). Nothing ported reads it yet.
    pub clock: u64,
}

/// What the game asks the console's platform for as it runs.
pub trait Console {
    /// The controllers of the next `PADRead`, in the order the game reads them: the boot's
    /// (`db_GetGameLaunchButtonState`) and each of the pad alarm's (`HSD_PadRenewRawStatus`).
    fn read_pads(&mut self) -> PadData;
    /// A disc file's entry number, none if the disc hasn't it (`DVDConvertPathToEntrynum`).
    fn entrynum(&mut self, path: &str) -> Option<i32>;
}

/// A running session: the game's state, the archives it has loaded, and the console's setup.
#[derive(Debug)]
pub struct Session {
    pub game: Game,
    pub data: Archives,
    pub setup: Setup,
}

/// `DbLevel` before `main` sets it: `DbLKind_NoDebugRom`, as the executable's data holds it.
const DB_NO_DEBUG_ROM: i32 = 1;
/// `DbLKind_Master`, a retail disc's level.
const DB_MASTER: i32 = 0;

impl Session {
    /// The console at power-on: nothing of the game has run, and memory holds what the
    /// executable's data holds.
    pub fn new(setup: Setup) -> Session {
        let mut game = Game::default();
        game.gm.db_level = DB_NO_DEBUG_ROM;
        game.audio.disc = setup.disc;
        game.audio.dsp = setup.dsp;
        Session {
            game,
            data: Archives::default(),
            setup,
        }
    }

    /// Takes up a session from a state: one imported from a trace tick's end, say.
    pub fn resume(game: Game, data: Archives, setup: Setup) -> Session {
        Session { game, data, setup }
    }

    /// The boot, `main`, as far as it is ported: it stops at the first function no port covers
    /// yet. `OSInit`, `VIInit`, `DVDInit`, `PADInit`, `CARDInit` and `OSInitAlarm` set up the
    /// console's own libraries, which the platform replaces: what they would read is the
    /// session's inputs.
    pub fn boot(&mut self, console: &mut dyn Console) {
        let g = &mut self.game;
        g.gm.launch_buttons = launch_buttons(console);
        g.gm.db_level = db_level(g.gm.db_level, console);
        // `OSGetConsoleSimulatedMemSize` is 24 MB on a retail console, which leaves the arena
        // alone; `arena_size` only goes into the boot's report.
        code::unported_call("HSD_SetInitParameter")
    }

    /// One engine tick of the running scene, its polls taking the controllers' next reads from
    /// `console`, up to the frame's display pass (where ssbm-rs's state traces sample the
    /// original): the tick's frame without draws, and whether the pass is due
    /// ([`Session::render`]).
    pub fn step_tick(&mut self, console: &mut dyn Console) -> (Frame, bool) {
        let lent = crate::hsd::heap::Lent::take(&mut self.game.objs.heap);
        let mut r = Running {
            game: &mut self.game,
            data: &self.data,
        };
        let (t, render) = scene::step_session_tick(&mut r, &mut || console.read_pads(), false);
        lent.give_back(&mut self.game.objs.heap);
        let out = Frame {
            events: std::mem::take(&mut self.game.events),
            pads: t.pads,
            ended: t.ended,
            draws: Vec::new(),
        };
        (out, render)
    }

    /// The frame's display pass after [`Session::step_tick`]: its draw commands.
    pub fn render(&mut self) -> Vec<crate::hsd::display::Draw> {
        let lent = crate::hsd::heap::Lent::take(&mut self.game.objs.heap);
        scene::render_screen(&mut Running {
            game: &mut self.game,
            data: &self.data,
        });
        lent.give_back(&mut self.game.objs.heap);
        std::mem::take(&mut self.game.objs.display.draws)
    }
}

/// The buttons held as the game starts, `db_GetGameLaunchButtonState`: the first port with a
/// controller, read until no port is busy (each read after a retrace, which the platform
/// times). Holding B asks for the progressive scan prompt (`gm_801A4510`).
#[doc(alias = "db_GetGameLaunchButtonState")]
fn launch_buttons(console: &mut dyn Console) -> u16 {
    let status = loop {
        let status = console.read_pads();
        if !status
            .iter()
            .any(|s| s.err == ERR_NOT_READY || s.err == ERR_TRANSFER)
        {
            break status;
        }
    };
    status
        .iter()
        .find(|s| s.err == ERR_NONE)
        .map_or(0, |s| s.button)
}

/// The debug level, `gmMain_8015FDA4`: a disc with `/develop.ini` is a development one, whose
/// levels the launch buttons pick; a retail disc runs at `DbLKind_Master`.
#[doc(alias = "gmMain_8015FDA4")]
fn db_level(level: i32, console: &mut dyn Console) -> i32 {
    if console.entrynum("/develop.ini").is_some() {
        panic!("a development disc (/develop.ini): its debug levels aren't ported");
    }
    assert_eq!(level, DB_NO_DEBUG_ROM, "DbLevel == DbLKind_NoDebugRom");
    DB_MASTER
}
