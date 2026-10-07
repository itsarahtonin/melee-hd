//! A VS match as a whole: started from its settings and stepped a frame at a time with the
//! controllers, the way the VS scene runs it (`gm_801A4014`: the scene's setup, its entry
//! `gm_Scene_Vs_OnEnter`, then `gm_801A4D34`'s loop of engine ticks).
//!
//! The match calls into fighters, items, effects, the HUD and audio through
//! [`code::call`](super::code::call) and the scheduler's process table; until those ports fill
//! their arms in, a match stops at the first unported function it reaches, saying which.

use melee_data::archive::{Archive, Archives};

use super::code;
use super::gm::{self, scene, vs};
use super::handles::CodeRef;
use super::mirror::{Cx, Mem, Mirror};
use super::state::{Event, Game, Rules, Running};
use super::structs::StartMeleeData;
use crate::input::controller::{PadData, Requests};

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
        }
    }

    /// The settings a Slippi replay's game start records: its game info block (the match's
    /// `StartMeleeData` as the game held it, 0x138 bytes) and random seed.
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

/// The functions the scene's setup calls that no port covers yet, by address.
mod addr {
    /// `un_802FF78C`: the debug text's GObj.
    pub const UN_802FF78C: u32 = 0x802F_F78C;
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
        let mut data = Archives::default();
        // `lb_80014534`: the rumble scripts, then the match's own files in the order its
        // setup loads them.
        let _rumble = data.add(load("LbRb.dat"));
        let files = vs::MatchArchives {
            pdpm: data.add(load("PdPm.dat")),
            gm_pause: data.add(load("GmPause.usd")),
        };
        let mut m = Match { game, data };
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
    code::call(r, addr::UN_802FF78C, &[]);
    let g = &mut *r.game;
    let gobj = g.world.create(14, 0, 0);
    g.world.setup_proc(gobj, CodeRef(vs::SCENE_PROC), 0);
    g.gm.scene_gobj = Some(gobj);
    g.reset_controller_map();
    // `lbAudioAx_8002835C`: the audio driver's scene change.
    g.audio(0x8002_835C, [0; 4]);
}
