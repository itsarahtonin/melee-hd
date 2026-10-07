//! A VS match as a whole: started from its settings and stepped a frame at a time with the
//! controllers, the way the VS scene runs it (`gm_801A4014`: the scene's setup, its entry
//! `gm_Scene_Vs_OnEnter`, then `gm_801A4D34`'s loop of engine ticks).
//!
//! The match calls into fighters, items, effects, the HUD and audio through
//! [`code::call`](super::code::call) and the scheduler's process table; until those ports fill
//! their arms in, a match stops at the first unported function it reaches, saying which.

use melee_data::archive::{Archive, Archives};

use super::gm::{self, scene, vs};
use super::handles::CodeRef;
use super::mirror::{Cx, Mem, Mirror};
use super::playback::{self, Playback};
use super::slippi::{self, CodeError};
use super::state::{Event, Game, Rules, Running};
use super::structs::StartMeleeData;
use super::ucf;
use crate::audio::dsp::DspModel;
use crate::input::controller::{PadData, Requests};
use crate::input::pad::PadStatus;

/// What a match starts from.
#[derive(Clone, Debug)]
pub struct Settings {
    /// The match's rules and players (`StartMeleeData`): characters, costumes, ports, teams,
    /// stocks, timer, stage and the rest, as the character and stage selects leave them.
    pub start: StartMeleeData,
    /// The random seed when the match starts.
    pub seed: u32,
    /// The game mode the match runs in (`GameModeKind`): VS for an ordinary match.
    pub game_mode: u8,
    pub rules: Rules,
    /// Which DSP model the voices follow, which decides when sounds end and so the handles
    /// later sounds get: a recording keeps the one it was played with, as ssbm-rs's keep
    /// their sound flag.
    pub dsp: DspModel,
    /// UCF's pad buffer as the match starts (with `rules.ucf`).
    pub ucf: ucf::Pads,
    /// A Slippi replay drives the match through Slippi's playback codes
    /// ([`Match::step_replay`]).
    pub playback: bool,
}

/// What Slippi's playback hands the game at a replay's start (the Slippi EXI device's answer
/// to its game info request), from the replay's game start event.
#[derive(Clone, Debug)]
pub struct ReplayStart {
    /// The match's `StartMeleeData` as the replay recorded it (0x138 bytes), with the
    /// device's one change: a player who started the match as Sheik starts as Sheik.
    pub info: Vec<u8>,
    pub seed: u32,
    /// The players' nametags, eight characters each.
    pub nametags: [[u16; 8]; 4],
    /// The replay's code list as playback applies it: without the codes it leaves out, which
    /// leaves the codes that affect gameplay.
    pub codes: Vec<u8>,
}

/// A player in [`Settings::vs`].
#[derive(Clone, Copy, Debug)]
pub struct Player {
    /// `CharacterKind`.
    pub character: u8,
    pub costume: u8,
    /// The controller port, 0 to 3.
    pub port: u8,
    /// `Gm_PKind`: human (0) or computer (1).
    pub kind: u8,
    pub cpu_level: u8,
    pub team: u8,
}

impl Settings {
    /// An ordinary VS match: stock or timed, on `stage` (`StKind`), up to four players, the
    /// rules the VS menus default to otherwise.
    pub fn vs(stage: u16, players: &[Player], stocks: i8, time_limit: u32) -> Settings {
        assert!(players.len() <= 4, "a VS match has up to four players");
        let mut start = StartMeleeData::default();
        let r = &mut start.rules;
        r.match_kind = if stocks > 0 { 1 } else { 0 };
        r.timer_enabled = time_limit != 0;
        r.time_limit = time_limit;
        r.is_stock = stocks > 0;
        r.is_vs = true;
        r.x4_0 = true;
        r.stkind = stage;
        r.item_freq = -1;
        r.sd_penalty = -1;
        r.x2C = 1.0;
        r.x30 = 1.0;
        r.game_speed = 1.0;
        for (i, p) in start.players.iter_mut().enumerate() {
            p.slot_type = 3;
            p.spawn_pos = -1;
            p.attack_ratio = 1.0;
            p.defense_ratio = 1.0;
            p.model_scale = 1.0;
            p.cpu_kind = 4;
            p.nametag = 0x78;
            p.team = i as u8;
        }
        for p in players {
            let d = &mut start.players[usize::from(p.port)];
            d.ckind = p.character as i8;
            d.slot_type = p.kind;
            d.stocks = stocks;
            d.color = p.costume;
            d.slot = p.port + 1;
            d.team = p.team;
            d.cpu_level = p.cpu_level;
        }
        Settings {
            start,
            seed: 0,
            game_mode: gm::GM_VS,
            rules: Rules::default(),
            // A player hears the match: the voices play through their samples.
            dsp: DspModel::Samples,
            ucf: ucf::Pads::default(),
            playback: false,
        }
    }

    /// A replay's match as Slippi's playback codes set it up (`fn_8016E730+0x18`): the
    /// replay's `StartMeleeData` with its callbacks cleared (the codes zero 0x40 to 0x5C) and
    /// the empty nametags of its human players dropped, its seed, and the rules its code list
    /// makes. The replay's UCF toggles, PAL and Pokémon Stadium flags and player names go to
    /// places only codes these rules don't include read, and the HUD.
    pub fn from_replay(r: &ReplayStart) -> Result<Settings, CodeError> {
        let mut info = r.info.clone();
        info[0x40..0x5C].fill(0);
        for (i, tag) in r.nametags.iter().enumerate() {
            let p = 0x60 + 0x24 * i;
            if info[p + 1] == 0 && info[p + 10] != 0x78 && tag[0] >> 8 == 0 {
                info[p + 10] = 0x78;
            }
        }
        let codes = slippi::parse_codes(&r.codes);
        let mut s = Settings::from_game_info(&info, r.seed);
        // Playback boots into the debug VS mode, whose scene runs the match.
        s.game_mode = gm::GM_DEBUG_VS;
        s.rules = Rules::from_codes(&codes)?;
        s.ucf = ucf::Pads::from_codes(&codes).unwrap_or_default();
        s.playback = true;
        Ok(s)
    }

    /// The settings a Slippi replay's game start records: its game info block (the match's
    /// `StartMeleeData` as the game held it, 0x138 bytes) and random seed. A replay doesn't
    /// say how its sound played; this takes the silent model, as ssbm-rs plays replays
    /// unless told `AX=1`, so the two compare.
    pub fn from_game_info(info: &[u8], seed: u32) -> Settings {
        assert_eq!(
            info.len(),
            StartMeleeData::SIZE as usize,
            "a game info block"
        );
        let mem = Mem {
            bytes: info,
            base: 0,
        };
        let start = StartMeleeData::import(&mem, 0, &mut NoPointers);
        Settings {
            start,
            seed,
            game_mode: gm::GM_VS,
            rules: Rules::default(),
            ucf: ucf::Pads::default(),
            playback: false,
            dsp: DspModel::Silent,
        }
    }
}

/// The rules hold no pointers that a replay can carry; any there are dropped.
struct NoPointers;

impl Cx for NoPointers {
    fn import_handle(&mut self, _: &'static str, _: u32) -> Option<u32> {
        None
    }
    fn import_data(&mut self, _: u32) -> Option<melee_data::archive::DataRef> {
        None
    }
    fn compare_handle(&mut self, _: &str, _: &'static str, _: Option<u32>, _: u32) {}
    fn compare_data(&mut self, _: &str, _: Option<melee_data::archive::DataRef>, _: u32) {}
    fn mismatch(&mut self, _: &str, _: String, _: String) {}
}

/// What a frame hands the frontend.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    /// Sounds and the like, in the order the tick asked for them.
    pub events: Vec<Event>,
    /// What the pad library asks of the hardware (resets, rumble motors).
    pub pads: Requests,
    /// The scene asked to end: the match is over and its results would follow.
    pub ended: bool,
}

/// A running VS match: the game's state and the archives it has loaded.
#[derive(Clone, Debug)]
pub struct Match {
    pub game: Game,
    pub data: Archives,
}

impl Match {
    /// Starts a match: the boot's pad and frame-clock setup, the VS scene's setup
    /// (`gm_801A4BD4`) and entry (`gm_Scene_Vs_OnEnter`), loading the files the match's own
    /// code loads with `load` (by disc file name). The scene loop is then ready for
    /// [`Match::step`].
    pub fn start(settings: &Settings, load: &mut dyn FnMut(&str) -> Archive) -> Match {
        let mut game = Game {
            game_mode: settings.game_mode,
            rules: settings.rules.clone(),
            pacing: crate::lb::pacing::Pacing::new(),
            ..Game::default()
        };
        game.objs.rng.seed = settings.seed;
        game.ucf = settings.ucf;
        if settings.playback {
            game.playback = Some(Playback {
                start_seed: settings.seed,
                ..Playback::default()
            });
            game.pads.playback = true;
        }
        game.audio.dsp = settings.dsp;
        let mut data = Archives::default();
        // `lb_80014534`: the rumble scripts, then the match's own files in the order its
        // setup loads them.
        let rumble = data.add(load("LbRb.dat"));
        let table = data.get(rumble).public("lbRumbleData");
        let files = vs::MatchArchives {
            pdpm: data.add(load("PdPm.dat")),
            gm_pause: data.add(load("GmPause.usd")),
        };
        let mut m = Match { game, data };
        m.game.rumble_data = table.map(|offset| melee_data::archive::DataRef {
            archive: rumble,
            offset,
        });
        let mut r = m.running();
        init_scene(&mut r);
        r.game.gm.on_frame = Some(CodeRef(vs::ON_FRAME));
        r.vs_on_enter(&settings.start, files);
        r.game.start_scene_loop();
        m
    }

    /// Takes up a match from a state: one imported from a capture of the original, say, at
    /// the start of an engine tick.
    pub fn resume(game: Game, data: Archives) -> Match {
        Match { game, data }
    }

    fn running(&mut self) -> Running<'_> {
        Running {
            game: &mut self.game,
            data: &self.data,
        }
    }

    /// One frame of a replay's match: what the tick may ask the Slippi device, answered from
    /// `source`, and a poll of four connected controllers at rest, as a replay run's
    /// controllers report.
    pub fn step_replay(&mut self, source: &mut dyn playback::Source) -> Frame {
        let answers = Playback::answers(&self.game, source);
        self.game
            .playback
            .as_mut()
            .expect("a replay's match")
            .answers = Some(answers);
        let out = self.step([PadStatus::default(); 4]);
        self.game
            .playback
            .as_mut()
            .expect("a replay's match")
            .answers = None;
        out
    }

    /// One frame: the controllers' poll queued, then one engine tick.
    pub fn step(&mut self, pads: PadData) -> Frame {
        let t = scene::step(&mut self.running(), pads, false);
        Frame {
            events: std::mem::take(&mut self.game.events),
            pads: t.pads,
            ended: t.ended,
        }
    }
}

/// A scene's setup before its entry, `gm_801A4BD4`: the debug pause buttons, no hook before
/// the processes, the 60 Hz frame clock, an empty scheduler, the debug text, the scene's own
/// GObj with an empty process, and the button maps.
pub fn init_scene(r: &mut Running) {
    let g = &mut *r.game;
    g.default_db_pause_handlers();
    g.set_pre_gobj_proc(0);
    g.set_clock_mask(0);
    g.pacing
        .request(crate::lb::pacing::seconds_to_ticks(gekko_fp::fdivs(
            1.0,
            gekko_fp::frsp(60.0),
        )));
    g.world = crate::hsd::gobj::World::new();
    let g = &mut *r.game;
    g.setup_scene_text(r.data);
    let gobj = g.world.create(14, 0, 0);
    g.world.setup_proc(gobj, CodeRef(vs::SCENE_PROC), 0);
    g.gm.scene_gobj = Some(gobj);
    g.reset_controller_map();
    // `lbAudioAx_8002835C`: the audio driver's scene change.
    g.audio(0x8002_835C, [0; 4]);
}
