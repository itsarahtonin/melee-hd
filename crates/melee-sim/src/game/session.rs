//! A console session from power-on: the boot (`main`), then the game modes' scenes, each engine
//! tick taking the controllers the session reads. Everything the console's platform hands the
//! game is an explicit input ([`Setup`], [`Console`]): the memory card, the disc's pace, whether
//! the DSP mixes sound, the real-time clock, the video interface's DTV status, and every
//! `PADRead`'s controllers, in the order the game reads them.
//!
//! [`Session::boot`] runs `main` to the first scene loop and stops at the first function no
//! port covers yet, naming it, as a match does (`code::unported_call`). Each
//! [`Session::step_tick`] runs an engine tick of the scene, and when a scene has ended, first
//! the modes' loops to the next scene (`gm::machine`). A session taken up from a state (an
//! imported trace tick, [`Session::resume`]) plays on with [`Session::step_tick`].

use melee_data::archive::{Archive, Archives};

use super::gm::scene;
use super::state::{Game, Running};
use super::vs_match::{DiscFile, Frame};
use crate::audio::boot::{BootTimes, CONSOLE_BOOT_DELAY};
use crate::audio::clock::{DiscSpeed, TB_HZ};
use crate::audio::dsp::DspModel;
use crate::input::controller::PadData;
use crate::input::pad::{ERR_NONE, ERR_NOT_READY, ERR_TRANSFER};
use crate::lb::save::{CardDevice, CardLent};

/// What a session starts with besides its controllers: the console's setup.
#[derive(Clone, Debug)]
pub struct Setup {
    /// How fast the disc reads (`vs_match::Settings::disc`).
    pub disc: DiscSpeed,
    /// Which DSP model the voices follow (`vs_match::Settings::dsp`).
    pub dsp: DspModel,
    /// The memory card in slot A as the session found it (its raw image), or none. The game
    /// reads and writes it through the native save (`lb::save`); the session keeps it as the
    /// console's device.
    pub card: Option<Vec<u8>>,
    /// The real-time clock at power-on, in time base ticks from 2000-01-01 (the OS's epoch):
    /// what the boot sets the time base to (ssbm-rs's `Regs::tb_power_on`, `SSBMINP1`
    /// version 2's `power_on`). It is an offset only the code's reads of the time base add
    /// (`OSGetTime`, `OSGetTick`, and the alarms' grid, [`crate::audio::clock::Clock::tb_power_on`]):
    /// the timeline still counts from power-on, so it moves no hardware timing. It reaches
    /// gameplay: the boot seeds the random seed from the time base's low word (`OSGetTick`)
    /// and the title draws from it once for each second of the clock's minute
    /// (`gmTitle_801A165C`).
    pub power_on: u64,
    /// The video interface's DTV status (`VIGetDTVStatus`): whether the console can show
    /// progressive scan, which with B held at power-on (or the SRAM's setting) asks for it.
    pub dtv: bool,
    /// The SRAM's flags (`OSSram::flags`), which the boot reads: 0x04 stereo
    /// (`OSGetSoundMode`, the synth's mode) and 0x80 progressive scan
    /// (`OSGetProgressiveMode`). The SRAM's language isn't here: Melee's is the save's.
    pub sram_flags: u8,
    /// The card pulled out of slot A at the start of a video field and put back at the
    /// start of another, or never (ssbm-rs's `CARD_REMOVE=F[,G]`).
    pub card_pull: Option<(u64, Option<u64>)>,
    /// The developer setting that raises the debug level (milestone 5's task 19), none on a
    /// retail disc's console, the default. A session's own input, never a replay's.
    pub develop: Option<Develop>,
}

/// The debug level a developer setting raises `DbLevel` to, as ssbm-rs's runner does
/// (`--develop N`, `DBLEVEL=N`): from `db_InitScreenshot` on, after the boot's own
/// `gmMain_8015FDA4` set retail's level from the disc (no `/develop.ini`) and its USB host
/// check read it. The debug menus and the match's develop tools are reached past it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Develop {
    /// `DbLevel` (`DbLKind`: 0 Master, 1 NoDebugRom, 2 DebugDevelop, 3 DebugRom, 4 Develop).
    pub level: i32,
    /// Whether `/develop.ini` counts as found (`db_804D6B20`), as on a development disc and
    /// under ssbm-rs's `--develop`: at levels 3 and 4 the save's load then runs HAL's debug
    /// unlock-all (`gmMainLib_8015FA34`), which the game saves. `DBLEVEL=N` leaves it clear.
    pub ini: bool,
}

impl Develop {
    /// A recording's level (`SSBMINP1`'s flags, ssbm-rs's `--develop`), which marks the file.
    pub fn from_recording(level: Option<i32>) -> Option<Develop> {
        level.map(|level| Develop { level, ini: true })
    }
}

/// The SRAM flags of ssbm-rs's console, as Dolphin initializes them: stereo, setup done,
/// bit 5 (`SSBMINP1` version 1's).
pub const SRAM_FLAGS_DEFAULT: u8 = 0x2C;
/// `OSSram::flags`' stereo bit (`OSGetSoundMode`) and progressive scan bit
/// (`OSGetProgressiveMode`).
pub const SRAM_STEREO: u8 = 0x04;
pub const SRAM_PROGRESSIVE: u8 = 0x80;

impl Default for Setup {
    /// ssbm-rs's console with no card: Slippi's fast disc speed, the clock at 2000-01-01, no
    /// component cable, its SRAM's flags.
    fn default() -> Self {
        Setup {
            disc: DiscSpeed::default(),
            dsp: DspModel::default(),
            card: None,
            power_on: 0,
            dtv: false,
            sram_flags: SRAM_FLAGS_DEFAULT,
            card_pull: None,
            develop: None,
        }
    }
}

/// The console's card device for `setup`: a card in at power-on was seen by the OS's
/// probes at boot.
fn card_device(setup: &Setup) -> CardDevice {
    // The tenth of a second the OS's first probe at boot (in field 2) saw the card at, as
    // `__EXIProbe` counts them on the code's time base.
    let seen = setup.card.as_ref().map_or(0, |_| {
        crate::lb::save::probe_tenths(field_start(2), setup.power_on)
    });
    CardDevice {
        card: setup.card.as_ref().map(|image| {
            melee_save::Card::from_image(image.clone()).expect("a 59-block card's image")
        }),
        console: melee_save::Console::ssbm_rs(),
        waits: setup.card.is_some(),
        pull: setup
            .card_pull
            .map(|(out, back)| (field_start(out), back.map(field_start))),
        probe_start: seen,
        time_offset: setup.power_on,
        attached: false,
        // The OS's probes at boot read the card's ID as it settled.
        id_time: seen,
        extint_seen: 0,
        // Nothing before the first mount asks for the font encoding.
        font_encode_known: false,
    }
}

/// The start of video field `n` on the time base (ssbm-rs's `field_start`: NTSC's
/// 60000/1001 fields a second).
pub fn field_start(n: u64) -> u64 {
    (u128::from(n) * u128::from(TB_HZ) * 1001 / 60_000) as u64
}

/// What the game asks the console's platform for as it runs.
pub trait Console {
    /// The controllers of the next `PADRead`, in the order the game reads them: the boot's
    /// (`db_GetGameLaunchButtonState`) and each of the pad alarm's (`HSD_PadRenewRawStatus`).
    fn read_pads(&mut self) -> PadData;
    /// A disc file's entry number, none if the disc hasn't it (`DVDConvertPathToEntrynum`).
    fn entrynum(&mut self, path: &str) -> Option<i32>;
    /// A disc file's bytes and entry number, for the frontend's loads before the session
    /// runs.
    fn file(&mut self, path: &str) -> DiscFile;
    /// The bytes a disc read of a file brings past its end, the disc's next bytes up to its
    /// length rounded up to 32, which the heap's image takes as the file's block's tail (none
    /// where the console doesn't give them: unknown).
    fn file_after(&mut self, path: &str) -> Vec<u8> {
        let _ = path;
        Vec::new()
    }
    /// A disc file's entry number and size (`DVDFastOpen`'s), for the preload cache's reads of
    /// files the simulation only times.
    fn file_info(&mut self, path: &str) -> (i32, u32) {
        let f = self.file(path);
        (f.entrynum, f.bytes.len() as u32)
    }
    /// Gives back the last `n` reads [`Console::read_pads`] handed out, which the next reads
    /// hand out again ([`with_feed`]'s reads ahead that the game didn't take).
    fn unread(&mut self, n: usize) {
        assert!(n == 0, "this console can't give back {n} reads");
    }
}

/// How many of the console's reads [`with_feed`] takes ahead: more than a step or a scene's
/// entry reads.
const FEED_AHEAD: usize = 2048;

thread_local! {
    /// The console's next reads while a session step runs ([`with_feed`]), for the game's
    /// own `PADRead`s in its code ([`direct_read`]) as well as the step's polls.
    static FEED: std::cell::RefCell<Option<std::collections::VecDeque<PadData>>> =
        const { std::cell::RefCell::new(None) };
}

/// Runs `f` with the console's next reads taken ahead into the step's feed, `f`'s reads
/// (the closure it's handed, and the game's [`direct_read`]) taking them in order; the
/// reads left are given back to the console.
pub fn with_feed<R>(
    console: &mut dyn Console,
    f: impl FnOnce(&mut dyn FnMut() -> PadData) -> R,
) -> R {
    let ahead: std::collections::VecDeque<PadData> =
        (0..FEED_AHEAD).map(|_| console.read_pads()).collect();
    let outer = FEED.with(|c| c.borrow_mut().replace(ahead));
    assert!(outer.is_none(), "a session step's feed inside another's");
    let r = f(&mut direct_read);
    let left = FEED.with(|c| c.borrow_mut().take()).map_or(0, |q| q.len());
    console.unread(left);
    r
}

/// The controllers of the console's next `PADRead` within a session's step: the game's own
/// reads (`HSD_PadRenewStatus`) and the step's polls.
pub fn direct_read() -> PadData {
    FEED.with(|c| {
        c.borrow_mut()
            .as_mut()
            .expect("a PADRead outside a session's step isn't ported")
            .pop_front()
            .expect("a step read more than its feed holds")
    })
}

/// A running session: the game's state, the archives it has loaded, the console's setup and
/// its memory card.
#[derive(Debug)]
pub struct Session {
    pub game: Game,
    pub data: Archives,
    pub setup: Setup,
    /// The memory card in slot A and the console's card library's setting (its flash ID):
    /// the console's, lent to each step.
    pub card: Option<CardDevice>,
}

/// `DbLevel` before `main` sets it: `DbLKind_NoDebugRom`, as the executable's data holds it.
const DB_NO_DEBUG_ROM: i32 = 1;
/// `DbLKind_Master`, a retail disc's level.
const DB_MASTER: i32 = 0;

/// The disc files a session's ported scenes load, which the frontend reads before the session
/// starts: each scene's archives, and the preload cache's files (whose sizes it reads by).
pub const FILES: &[&str] = &[
    "LbRb.dat",
    "LbMcGame.usd",
    "NtMemAc.usd",
    "NtMsgWin.dat",
    "SdMsgBox.usd",
    "EfMnData.dat",
    "EfCoData.dat",
    "ItCo.usd",
    "IfAll.usd",
    "TyDatai.usd",
    "GmTtAll.usd",
    "NtProge.dat",
    "SdProge.usd",
    "LbMcSnap.usd",
    "MnMaAll.usd",
    "SdMenu.usd",
    // The menus in Japanese, which the language options change to.
    "MnMaAll.dat",
    "SdMenu.dat",
    "IfAll.dat",
    "NtMemAc.dat",
    "SdMsgBox.dat",
    "SdProge.dat",
    "SdToy.dat",
    "GmEvent.dat",
    "LbAd.dat",
    // The character and stage selects'.
    "MnSlChr.usd",
    "MnExtAll.usd",
    "SdSlChr.usd",
    "MnSlMap.usd",
    // The special messages' interface's.
    "IfPrize.usd",
    "SdPrize.usd",
    "IfPrize.dat",
    "SdPrize.dat",
    // The sound test debug mode's lists.
    "SmSt.dat",
    // The challenger approach's screen, in either language.
    "NtAppro.usd",
    "NtAppro.dat",
    // A match's (the attract loop's): `vs_match::Match::start`'s list.
    "LbRf.dat",
    "PdPm.dat",
    "GmPause.usd",
    "SdIntro.dat",
    "IfCoGet.dat",
    "LbBf.dat",
    // Camera Mode's: its strings and its menu's models.
    "SdVsCam.usd",
    "IfVsCam.usd",
    "SdVsCam.dat",
    "IfVsCam.dat",
    // The trophy screens' (the trophies' own models, which `TyDataf.dat` names, after).
    "TyDatai.dat",
    "TyDataf.dat",
    "TyMnView.usd",
    "TyMnView.dat",
    "TyMnBg.dat",
    "TyMnInfo.usd",
    "TyMnInfo.dat",
    "TyLight.dat",
    "TyStand.dat",
    "SdToy.usd",
    "SdToyExp.usd",
    "SdToyExp.dat",
    "TyMnFigp.usd",
    "TyMnFigp.dat",
    // The collection's: its scene and its categories' models.
    "TyMnDisp.usd",
    "TyMnDisp.dat",
    "TyQuesD.dat",
    "TyMycCmA.dat",
    "TyMycCmB.dat",
    "TyMycCmC.dat",
    "TyMycCmD.dat",
    "TyMycCmE.dat",
    "TyMycR1A.dat",
    "TyMycR1B.dat",
    "TyMycR1C.dat",
    "TyMycR1D.dat",
    "TyMycR1E.dat",
    "TyMycR2A.dat",
    "TyMycR2B.dat",
    "TyMycR2C.dat",
    "TyMycR2D.dat",
    "TyMycR2E.dat",
    "TyMapA.dat",
    "TyMapB.dat",
    "TyMapC.dat",
    "TyMapD.dat",
    "TyMapE.dat",
    "TySeriA.dat",
    "TySeriB.dat",
    "TySeriC.dat",
    "TySeriD.dat",
    "TySeriE.dat",
    "TyEtcA.dat",
    "TyEtcB.dat",
    "TyEtcC.dat",
    "TyEtcD.dat",
    "TyEtcE.dat",
    "TyPokeA.dat",
    "TyPokeB.dat",
    "TyPokeC.dat",
    "TyPokeD.dat",
    "TyPokeE.dat",
    "TyItemA.dat",
    "TyItemB.dat",
    "TyItemC.dat",
    "TyItemD.dat",
    "TyItemE.dat",
    "TyStandD.dat",
    // Tournament mode's: its scenes, its bracket's boxes and its strings.
    "GmTou1p.usd",
    "GmTou2p.usd",
    "GmTou3p.usd",
    "GmTou4p.usd",
    "TmBox.dat",
    "SdTou.usd",
];

/// The songs the ported scenes play (`lbAudioAx_80023F28`) but a match's stage's and the sound
/// test's, which [`Session::read_song`] reads as the stage or the test starts them: the
/// opening movie's, the menus' two, How to Play's, the Special Movie's and the lottery's, the
/// special messages' three, the results screen's fanfares (`ff_dk.hps` to `ff_yoshi.hps`,
/// each character's), which the screen's entry starts, Tournament's bracket's two and the
/// challenger approach's.
pub const SONGS: &[usize] = &[
    0x3E, 0x34, 0x36, 0x24, 0x52, 0x35, 0x43, 0x44, 0x45, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x13, 0x14,
    0x15, 0x16, 0x17, 0x18, 0x19, 0x1D, 0x5D, 0x5E, 0x48,
];

/// The movies the ported scenes play (`lbMthp`), read whole before the session starts.
pub const MOVIES: &[&str] = &[
    super::gm::opening::MOVIE,
    super::gm::howto::HOWTO_MOVIE,
    super::gm::howto::OMAKE_MOVIE,
];

/// A time base read's time on ssbm-rs's clock, and a hardware register's.
pub(crate) const TB_READ: u64 = 16;
pub(crate) const REG_READ: u64 = 8;

/// The sound bank, in the US language's directory (the disc has `usa.ini`).
const SOUND_BANK: &str = "audio/us/smash2.sem";
const SOUND_BANK_JP: &str = "audio/smash2.sem";

impl Session {
    /// The console at power-on: nothing of the game has run, and memory holds what the
    /// executable's data holds.
    pub fn new(setup: Setup) -> Session {
        let mut game = Game::default();
        game.gm.db_level = DB_NO_DEBUG_ROM;
        // What the executable's data holds that the comparison reads: the stage selected
        // (`selected_stage`, Fountain of Dreams) and the camera's default field of view
        // (`cm_803BCCA0`'s, 38 degrees).
        game.gr.stkind = 2;
        game.camera.fov = 38.0;
        game.audio.disc = setup.disc;
        game.audio.dsp = setup.dsp;
        // The heap's memory as the scenes leave it, for a match's leftovers.
        game.objs.heap.image = crate::hsd::image::Image::power_on();
        let card = Some(card_device(&setup));
        crate::lb::snap_write::forget_frame();
        Session {
            game,
            data: Archives::default(),
            setup,
            card,
        }
    }

    /// Takes up a session from a state: one imported from a trace tick's end, say. Its
    /// timeline takes the setup's clock at power-on.
    pub fn resume(mut game: Game, data: Archives, setup: Setup) -> Session {
        game.audio.power_on = setup.power_on;
        game.audio.mono = setup.sram_flags & SRAM_STEREO == 0;
        if let Some(c) = game.audio.clock.as_mut() {
            c.tb_power_on = setup.power_on;
        }
        let card = Some(card_device(&setup));
        Session {
            game,
            data,
            setup,
            card,
        }
    }

    /// Reads the files the boot and the ported scenes load, before the session runs.
    fn load_files(&mut self, console: &mut dyn Console) -> crate::lb::audio::SoundBank {
        let file = console.file(SOUND_BANK);
        let bank = crate::lb::audio::SoundBank {
            len: file.bytes.len() as u32,
            archive: self.data.add(Archive::raw(&file.bytes)),
        };
        // Japanese's, which the language options load.
        let file = console.file(SOUND_BANK_JP);
        let bank_jp = crate::lb::audio::SoundBank {
            len: file.bytes.len() as u32,
            archive: self.data.add(Archive::raw(&file.bytes)),
        };
        // The executable's data the dummy stage points into (the results screen's).
        self.game.gr.defaults = Some(
            self.data
                .add(Archive::parse(&super::gr::exe_data::FILE).expect("the executable's data")),
        );
        let audio = &mut self.game.audio;
        audio.lang_banks = [Some(bank_jp), Some(bank)];
        for dir in ["audio/us/", "audio/"] {
            for name in crate::lb::audio::SSM_NAMES {
                let file = console.file(&format!("{dir}{name}"));
                if !audio.devcom.files.iter().any(|f| f.0 == file.entrynum) {
                    let id = self.data.add(Archive::raw(&file.bytes));
                    audio.devcom.files.push((file.entrynum, id));
                }
            }
        }
        let mut sizes = Vec::new();
        for &name in FILES {
            let file = console.file(name);
            let len = file.bytes.len() as u32;
            let a = Archive::parse_in(&file.bytes, &console.file_after(name))
                .unwrap_or_else(|e| panic!("{name}: {e:?}"));
            let id = self.data.add(a);
            let files = &mut self.game.ft.data.files;
            files.entries.push((name.to_owned(), file.entrynum, len));
            files.archives.push((name.to_owned(), id));
            sizes.push((file.entrynum, len));
        }
        // The trophies' models, each a file `TyDataf.dat`'s tables name (`ToyModelFile`).
        let dataf = self.game.ft.data.files.archive("TyDataf.dat");
        let a = self.data.get(dataf);
        let mut models = Vec::new();
        for (table, n) in [
            ("tyModelFileTbl", super::ty::TROPHY_COUNT),
            ("tyModelFileUsTbl", 5),
        ] {
            let at = a.public(table).expect("TyDataf.dat's tables") as usize;
            for i in 0..n {
                let name = &a.data()[at + 0x54 * i + 4..at + 0x54 * i + 0x24];
                let len = name.iter().position(|&b| b == 0).unwrap_or(name.len());
                let name = String::from_utf8_lossy(&name[..len]).into_owned();
                if !models.contains(&name) {
                    models.push(name);
                }
            }
        }
        for name in models {
            let file = console.file(&name);
            let len = file.bytes.len() as u32;
            let a = Archive::parse_in(&file.bytes, &console.file_after(&name))
                .unwrap_or_else(|e| panic!("{name}: {e:?}"));
            let id = self.data.add(a);
            let files = &mut self.game.ft.data.files;
            files.entries.push((name.clone(), file.entrynum, len));
            files.archives.push((name, id));
            sizes.push((file.entrynum, len));
        }
        // The files a game cache can preload, by entry and size: the preload cache only times
        // their reads.
        let mut names = Vec::new();
        for name in crate::lb::dvd::game_cache_files() {
            let (entrynum, len) = console.file_info(&name);
            if !sizes.iter().any(|s| s.0 == entrynum) {
                sizes.push((entrynum, len));
            }
            names.push((name, entrynum));
        }
        self.game.audio.dvd = Some(crate::lb::dvd::Dvd {
            sizes,
            names,
            ..Default::default()
        });
        // The songs the ported scenes play, which the music stream reads.
        for &bgm in SONGS {
            let name = crate::lb::audio::HPS_NAMES[bgm];
            let file = console.file(&format!("audio/{name}"));
            if !self
                .game
                .audio
                .devcom
                .files
                .iter()
                .any(|f| f.0 == file.entrynum)
            {
                let id = self.data.add(Archive::raw(&file.bytes));
                self.game.audio.devcom.files.push((file.entrynum, id));
            }
        }
        // The effect files a match registers as it and its fighters ask for them
        // (`efAsync_LoadSync`): disc data, each read before it's needed.
        for (i, f) in super::ef::FILES.iter().enumerate() {
            if let Some((name, _)) = f {
                let loaded = self
                    .game
                    .ft
                    .data
                    .files
                    .archives
                    .iter()
                    .find(|a| a.0 == *name);
                let id = match loaded {
                    Some(&(_, id)) => id,
                    None => {
                        let file = console.file(name);
                        let a = Archive::parse_in(&file.bytes, &console.file_after(name))
                            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
                        let id = self.data.add(a);
                        let files = &mut self.game.ft.data.files;
                        files.entries.push((
                            (*name).to_owned(),
                            file.entrynum,
                            file.bytes.len() as u32,
                        ));
                        // Named, so the comparer finds it where the original loads it.
                        files.archives.push(((*name).to_owned(), id));
                        id
                    }
                };
                self.game.ef.files[i] = Some(id);
            }
        }
        // The movies, whose frames the movie player reads through the device command queue.
        for &name in MOVIES {
            let file = console.file(name);
            let id = self.data.add(Archive::raw(&file.bytes));
            self.game.audio.devcom.files.push((file.entrynum, id));
            let len = file.bytes.len() as u32;
            let files = &mut self.game.ft.data.files;
            files.entries.push((name.to_owned(), file.entrynum, len));
        }
        bank
    }

    /// The boot, `main`, to the first scene loop's start: the launch buttons, the debug
    /// level, HSD's setup, the random seed, the audio library, the pads' alarm, the game's
    /// libraries, the game's data reset, then the modes' loop to its first scene. It stops at
    /// the first function no port covers yet. `OSInit`, `VIInit`, `DVDInit`, `PADInit`,
    /// `CARDInit` and `OSInitAlarm` set up the console's own libraries, which the platform
    /// replaces: what they would read is the session's inputs.
    pub fn boot(&mut self, console: &mut dyn Console) {
        let g = &mut self.game;
        g.gm.launch_buttons = launch_buttons(console);
        g.gm.db_level = db_level(g.gm.db_level, console);
        // `OSGetConsoleSimulatedMemSize` is 24 MB on a retail console, which leaves the arena
        // alone; `arena_size` only goes into the boot's report. HSD's setup
        // (`HSD_SetInitParameter`, the frame buffers and the GX FIFO, `HSD_InitComponent`'s
        // heaps and libraries, `GXSetMisc`) is the platform's video and the allocator
        // model's: the OS heaps it makes are the model's layouts (`hsd::heap`).
        let bank = self.load_files(console);
        let g = &mut self.game;
        // `*HSD_RandSeedPtr = OSGetTick()`: the time base's low word, on the code's time base
        // (the clock at power-on and the time since).
        let card_in = self.card.as_ref().is_some_and(|c| c.card.is_some());
        let delay = if card_in { CONSOLE_BOOT_DELAY } else { 0 };
        let tb = self.setup.power_on.wrapping_add(BootTimes::SEED + delay);
        g.objs.rng.seed = tb as u32;
        g.audio.boot_delay = delay;
        g.audio.console = true;
        g.audio.power_on = self.setup.power_on;
        g.audio.mono = self.setup.sram_flags & SRAM_STEREO == 0;
        let card = CardLent::take(&mut self.card);
        let lent = crate::hsd::heap::Lent::take(&mut self.game.objs.heap);
        let mut r = Running {
            game: &mut self.game,
            data: &self.data,
        };
        let progressive = self.setup.sram_flags & SRAM_PROGRESSIVE != 0;
        let develop = self.setup.develop;
        r.boot_main(
            &bank,
            card_in,
            (self.setup.dtv, progressive),
            develop,
            console,
        );
        lent.give_back(&mut self.game.objs.heap);
        card.give_back(&mut self.card);
    }

    /// One engine tick of the running scene, its polls taking the controllers' next reads from
    /// `console`, up to the frame's display pass (where ssbm-rs's state traces sample the
    /// original): the tick's frame without draws, and whether the pass is due
    /// ([`Session::render`]). When the last tick ended its scene, the modes' loops first run
    /// to the next scene.
    pub fn step_tick(&mut self, console: &mut dyn Console) -> (Frame, bool) {
        let card = CardLent::take(&mut self.card);
        if self.game.scene.unk_C != 0 {
            self.enter_next_scene(console);
        }
        let lent = crate::hsd::heap::Lent::take(&mut self.game.objs.heap);
        let mut r = Running {
            game: &mut self.game,
            data: &self.data,
        };
        let (t, render) = with_feed(console, |read| {
            scene::step_session_tick(&mut r, read, false)
        });
        // The pad alarm's card probes in the tick, which nothing in it looked at since, as
        // the alarm's handler left them (`lb_8001C600`, `lbSnap_8001D2BC`).
        r.card_alarm_probes();
        lent.give_back(&mut self.game.objs.heap);
        card.give_back(&mut self.card);
        // A song a scene started mid-scene (the sound test's): its reads come after the tick.
        self.read_song(console);
        let out = Frame {
            events: std::mem::take(&mut self.game.events),
            pads: t.pads,
            ended: t.ended,
            draws: Vec::new(),
        };
        (out, render)
    }

    /// The modes' loops to the next scene (the card lent): its files read first when its
    /// entry loads a match's, then the pad alarm's reads meanwhile and the scene loop's start.
    fn enter_next_scene(&mut self, console: &mut dyn Console) {
        let lent = crate::hsd::heap::Lent::take(&mut self.game.objs.heap);
        with_feed(console, |_| {
            Running {
                game: &mut self.game,
                data: &self.data,
            }
            .next_scene()
        });
        lent.give_back(&mut self.game.objs.heap);
        if std::mem::take(&mut self.game.machine.files_wanted) {
            let start = Running {
                game: &mut self.game,
                data: &self.data,
            }
            .scene_start_data()
            .clone();
            self.read_match_files(&start, console);
            let lent = crate::hsd::heap::Lent::take(&mut self.game.objs.heap);
            with_feed(console, |_| {
                Running {
                    game: &mut self.game,
                    data: &self.data,
                }
                .next_scene()
            });
            lent.give_back(&mut self.game.objs.heap);
        }
        // The song the scene's entry started (a match's stage's, the results' fanfare).
        self.read_song(console);
        let lent = crate::hsd::heap::Lent::take(&mut self.game.objs.heap);
        let mut r = Running {
            game: &mut self.game,
            data: &self.data,
        };
        with_feed(console, |read| r.take_alarm_reads(read));
        r.begin_scene_loop();
        lent.give_back(&mut self.game.objs.heap);
    }

    /// Reads the files a match's scene loads (`files_wanted`): its fighters' (`PlCo.dat`, each
    /// character's data, costume and animations), its stage's and Pokémon Stadium's
    /// transformations', as `vs_match::Match::start` does.
    fn read_match_files(
        &mut self,
        start: &super::structs::StartMeleeData,
        console: &mut dyn Console,
    ) {
        super::vs_match::load_fighter_files(&mut self.game, &mut self.data, start, &mut |n| {
            console.file(n)
        });
        super::vs_match::load_results_files(&mut self.game, &mut self.data, start, &mut |n| {
            console.file(n)
        });
        let grkind = crate::lb::audio::ground_kind(i32::from(start.rules.stkind)) as i32;
        let mut names: Vec<&str> = Vec::new();
        if let Some(name) = super::gr::data::stage_data(grkind).file {
            names.push(name.trim_start_matches('/'));
        }
        if grkind == super::gr::GR_KIND_PSTADIUM {
            names.extend(super::gr::stadium::FILES);
        }
        for name in names {
            let files = &self.game.ft.data.files;
            if files.archives.iter().any(|a| a.0 == name) {
                continue;
            }
            let file = console.file(name);
            let a = Archive::parse_in(&file.bytes, &console.file_after(name))
                .unwrap_or_else(|e| panic!("{name}: {e:?}"));
            let id = self.data.add(a);
            let files = &mut self.game.ft.data.files;
            files.archives.push((name.to_owned(), id));
            files
                .entries
                .push((name.to_owned(), file.entrynum, file.bytes.len() as u32));
        }
        if grkind == super::gr::GR_KIND_PSTADIUM {
            for (i, name) in super::gr::stadium::FILES.iter().enumerate() {
                self.game.gr.stadium.files[i] = Some(self.game.ft.data.files.archive(name));
            }
        }
    }

    /// Reads the song the scene's entry or its last tick started (`cur_hps_stem`: a match's
    /// stage's, the sound test's) unless the music stream has it already: its first reads come
    /// in the ticks after, behind the disc's latency.
    fn read_song(&mut self, console: &mut dyn Console) {
        let stem = self.game.audio.lb.stem;
        let len = stem.iter().position(|&c| c == 0).unwrap_or(stem.len());
        if len <= 1 {
            return;
        }
        let path = std::str::from_utf8(&stem[1..len]).expect("a music path");
        let file = console.file(path);
        if !self
            .game
            .audio
            .devcom
            .files
            .iter()
            .any(|f| f.0 == file.entrynum)
        {
            let id = self.data.add(Archive::raw(&file.bytes));
            self.game.audio.devcom.files.push((file.entrynum, id));
        }
    }

    /// The frame's display pass after [`Session::step_tick`]: its draw commands.
    pub fn render(&mut self) -> Vec<crate::hsd::display::Draw> {
        // The pass's wait can take the pad alarm, whose handler probes the card.
        let card = CardLent::take(&mut self.card);
        let lent = crate::hsd::heap::Lent::take(&mut self.game.objs.heap);
        let mut r = Running {
            game: &mut self.game,
            data: &self.data,
        };
        // The pass's wait starts a save a process marked (`lb_8001CC84`).
        r.card_writer_kick_due();
        r.game.audio.render_start(r.data);
        r.card_writer_take();
        scene::render_screen(&mut r);
        lent.give_back(&mut self.game.objs.heap);
        card.give_back(&mut self.card);
        std::mem::take(&mut self.game.objs.display.draws)
    }
}

impl Running<'_> {
    /// `main` from the audio setup to the modes' loop's first scene.
    #[doc(alias = "main")]
    #[doc(alias = "gmMain_8015FDA0")]
    fn boot_main(
        &mut self,
        bank: &crate::lb::audio::SoundBank,
        card_in: bool,
        (dtv, progressive): (bool, bool),
        develop: Option<Develop>,
        console: &mut dyn Console,
    ) {
        // `lbAudioAx_8002838C`, then `lb_80019AAC(gmMain_8015FD24)`: the frame clocks and the
        // pads, whose alarm the timeline runs from the audio setup.
        let aram_next = self.game.audio.lb_boot(self.data);
        if let Some(c) = self.game.audio.clock.as_mut() {
            c.alarm_times = Some(Vec::new());
        }
        self.game.pacing = crate::lb::pacing::Pacing::new();
        // The VI callbacks (`gmMain_8015FDA0` does nothing, `HSD_VIDrawDoneXFB`) and
        // `HSD_VISetBlack` are the platform's video.
        let d = self.dvd_mut();
        d.memory.init(aram_next);
        let arena = (
            crate::hsd::heap::arena_lo(crate::hsd::heap::CONSOLE_HEAPS)
                + crate::hsd::heap::AUDIO_HEAP_SIZE,
            crate::hsd::heap::ARENA_HI,
        );
        let mem = std::mem::take(&mut d.memory);
        d.heaps.init(arena, &mem);
        d.memory = mem;
        d.init_cache();
        // `lbArq_80014D2C`: the ARAM request queue, which the timeline's ARAM loads stand for.
        self.game.card_tasks.init();
        self.game.card_game.init(card_in);
        self.game.snap.init([card_in, false]);
        self.init_main_lib();
        self.game.audio.movie.init();
        // `HSD_SisLib_803A6048(0xC000)`: the boot's text heap, in HSD's boot heap, which the
        // first scene's heap replaces.
        self.game.sis.init(0xC000);
        self.reset_game_data(bank);
        // The USB host check reads the level `gmMain_8015FDA4` set (retail's: no host), and
        // `db_InitScreenshot` clears the screenshot's state, which nothing has set. A developer
        // setting raises the level from here, where ssbm-rs's runner does.
        if let Some(d) = develop {
            assert!(
                (0..=4).contains(&d.level),
                "DbLevel {} isn't a DbLKind",
                d.level
            );
            self.game.gm.db_level = d.level;
            self.game.gm.develop_ini |= d.ini;
        }
        // The boot's report (`OSReport`) changes nothing but reads the clock
        // (`lbTime_GetTimeInSeconds`); `db_EnableItemSpawns` (`gmMain_804D6594` is false on
        // a retail disc), `db_ClearFPUExceptions`.
        self.game.audio.clock_read(self.data, TB_READ);
        self.game.it.db_menu.ItemSpawnsEnabled = 1;
        self.start_modes(dtv, progressive);
        self.next_scene();
        self.take_alarm_reads(&mut || console.read_pads());
        self.begin_scene_loop();
    }

    /// The pad alarm's firings so far in a scene's entry run their frames of rumble now,
    /// ahead of their reads ([`Self::take_alarm_reads`]): a rumble the entry starts begins
    /// after them, as the original's alarm handlers ran each in its time.
    pub fn alarm_rumble_catch_up(&mut self) {
        let Some(c) = self.game.audio.clock.as_ref() else {
            return;
        };
        let p = &mut self.game.pads;
        let due = c.pads.saturating_sub(u32::from(p.lib.qcount));
        for _ in p.rumble_ahead..due {
            p.rumble.interpret(self.data);
            p.rumble_ahead += 1;
        }
    }

    /// `HSD_PadRenewStatus` from the game's code (the trophy screens'): the pad alarm's reads
    /// the timeline counted meanwhile queued, then a `PADRead` of its own (the console's next
    /// read) queued after them (`HSD_PadRenewRawStatus(0)`), and the master, copy and game
    /// statuses renewed from the oldest.
    #[doc(alias = "HSD_PadRenewStatus")]
    pub fn pads_renew_status(&mut self) {
        // `PADRead`'s serial interface registers: three reads, as the alarm handler's.
        self.game.audio.clock_read(self.data, 3 * REG_READ);
        self.take_alarm_reads(&mut direct_read);
        let before = u32::from(self.game.pads.lib.qcount);
        let now = direct_read();
        let p = &mut self.game.pads;
        p.renew_raw(self.data, now, false, false);
        p.renew_master();
        p.renew_copy();
        p.renew_game();
        // Its own poll queued and the oldest taken: the polls the scene's loop can take are
        // one fewer when the queue was full (`qtype` 0 merges the newest into the oldest).
        let after = u32::from(self.game.pads.lib.qcount);
        if let Some(c) = self.game.audio.clock.as_mut() {
            c.pads = (c.pads + after).saturating_sub(before);
        }
    }

    /// The pad alarm's reads while the boot or a scene's preparation ran, which the
    /// timeline counted (`HSD_PadRenewRawStatus` from the alarm, a `PADRead` each), into the
    /// pad library's queue; the scene loop's start then keeps the newest.
    pub fn take_alarm_reads(&mut self, read: &mut dyn FnMut() -> PadData) {
        let Some(c) = self.game.audio.clock.as_ref() else {
            return;
        };
        // Each firing since the last tick is a read; the ones the last tick already read
        // are in the queue.
        let n = c.pads.saturating_sub(u32::from(self.game.pads.lib.qcount));
        for _ in 0..n {
            let now = read();
            self.game.pads.renew_raw(self.data, now, false, false);
        }
        let qcount = u32::from(self.game.pads.lib.qcount);
        if let Some(c) = &mut self.game.audio.clock {
            c.pads = c.pads.min(qcount);
        }
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
