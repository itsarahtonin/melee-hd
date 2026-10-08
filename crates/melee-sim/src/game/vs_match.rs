//! A VS match as a whole: started from its settings and stepped a frame at a time with the
//! controllers, the way the VS scene runs it (`gm_801A4014`: the scene's setup, its entry
//! `gm_Scene_Vs_OnEnter`, then `gm_801A4D34`'s loop of engine ticks).
//!
//! The match calls into fighters, items, effects, the HUD and audio through
//! [`code::call`](super::code::call) and the scheduler's process table; until those ports fill
//! their arms in, a match stops at the first unported function it reaches, saying which.

use melee_data::archive::{Archive, ArchiveId, Archives};

use super::gm::{self, scene, vs};
use super::handles::CodeRef;
use super::hud;
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
    /// The addresses the original gives the match's fighters as it creates them, in creation
    /// order (`ft::Globals::address_seed`), where the frontend knows them: the oracle takes
    /// them from ssbm-rs's trace. Empty, the fighters' addresses are 0.
    pub fighter_addresses: Vec<u32>,
    /// What the save data has unlocked, and whether the unlock-all codes run: the stage's
    /// music choice reads it (`gm_80164ABC`), and on some stages that decides whether it
    /// draws from the random seed (see engine.md). Slippi Dolphin runs the codes.
    pub unlocks: super::gm::Unlocks,
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
            fighter_addresses: Vec::new(),
            unlocks: super::gm::Unlocks::slippi(),
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
    /// say how its sound played; this takes the sample-accurate model, as Slippi Dolphin
    /// recorded it and ssbm-rs plays replays unless told `AX=0`, so the two compare.
    /// [`DspModel::Silent`] stays available by setting `dsp`.
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
            dsp: DspModel::Samples,
            fighter_addresses: Vec::new(),
            unlocks: super::gm::Unlocks::slippi(),
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
    /// The display pass's draw commands, for the presentation layer.
    pub draws: Vec<crate::hsd::display::Draw>,
}

/// A disc file as the frontend reads it: its entry number in the disc's file table and its
/// bytes.
pub struct DiscFile {
    pub entrynum: i32,
    pub bytes: Vec<u8>,
}

/// A running VS match: the game's state and the archives it has loaded.
#[derive(Clone, Debug)]
pub struct Match {
    pub game: Game,
    pub data: Archives,
    /// The archives the match's start loaded, by file name (an animation's as
    /// `FILE@OFFSET`), for tools that find them in the original's memory.
    pub files: Vec<(String, ArchiveId)>,
}

impl Match {
    /// Starts a match: the boot's pad and frame-clock setup, the VS scene's setup
    /// (`gm_801A4BD4`) and entry (`gm_Scene_Vs_OnEnter`), loading the files the match's own
    /// code loads and its fighters need with `load` (a disc file's bytes, by name). The scene
    /// loop is then ready for [`Match::step`].
    pub fn start(settings: &Settings, load: &mut dyn FnMut(&str) -> DiscFile) -> Match {
        Match::start_from(settings, load, &mut |_, _| {})
    }

    /// [`Match::start`] with `boot` setting what the boot and the menus leave before the
    /// match's scene starts (the oracle seeds it from ssbm-rs's trace until the boot and the
    /// menus are ported): it runs on the new state, before anything of the scene's, and may
    /// add the archives the boot loaded.
    pub fn start_from(
        settings: &Settings,
        load: &mut dyn FnMut(&str) -> DiscFile,
        boot: &mut dyn FnMut(&mut Game, &mut Archives),
    ) -> Match {
        let mut game = Game {
            game_mode: settings.game_mode,
            rules: settings.rules.clone(),
            pacing: crate::lb::pacing::Pacing::new(),
            ..Game::default()
        };
        // The stage's code reads the same game mode (`gm_GetCurrentGameMode`).
        game.gr.game_mode = settings.game_mode;
        game.objs.rng.seed = settings.seed;
        game.ucf = settings.ucf;
        if settings.playback {
            game.playback = Some(Playback {
                start_seed: settings.seed,
                ..Playback::default()
            });
            game.pads.playback = true;
        }
        game.ft.address_seed = settings.fighter_addresses.iter().copied().collect();
        game.gm.unlocks = settings.unlocks;
        let mut data = Archives::default();
        boot(&mut game, &mut data);
        // After the boot's state, which may bring its own audio library.
        game.audio.dsp = settings.dsp;
        if game.audio.clock.is_some() {
            // The sound banks the match's bank choice may load, for the device command queue
            // to read.
            for name in crate::lb::audio::SSM_NAMES {
                let file = load(&format!("audio/us/{name}"));
                let id = data.add(Archive::raw(&file.bytes));
                game.audio.devcom.files.push((file.entrynum, id));
            }
        }
        // `lb_80014534`: the rumble scripts, then the match's own files in the order its
        // setup loads them, each with its disc entry for the loads the audio timeline times.
        let entries = &mut game.ft.data.files.entries;
        let mut archive = |name: &str| {
            let file = load(name);
            entries.push((name.to_owned(), file.entrynum, file.bytes.len() as u32));
            Archive::parse(&file.bytes).unwrap_or_else(|e| panic!("{name}: {e:?}"))
        };
        let rumble = data.add(archive("LbRb.dat"));
        let table = data.get(rumble).public("lbRumbleData");
        let files = vs::MatchArchives {
            // `lbRefract_800222A4`'s refraction table.
            lb_rf: data.add(archive("LbRf.dat")),
            pdpm: data.add(archive("PdPm.dat")),
            gm_pause: data.add(archive("GmPause.usd")),
            // `ifAll_802F390C`'s: `IfAll`, the names' font, the coin counter.
            hud: hud::all::HudArchives {
                if_all: data.add(archive("IfAll.usd")),
                sd_intro: data.add(archive("SdIntro.dat")),
                if_coget: data.add(archive("IfCoGet.dat")),
            },
            // `lbBgFlash_Init`'s.
            lb_bf: data.add(archive("LbBf.dat")),
            // `Item_80266F70`'s.
            it_co: data.add(archive("ItCo.usd")),
            // `Stage_802251E8`'s stage archive.
            stage: {
                let grkind = crate::lb::audio::ground_kind(i32::from(settings.start.rules.stkind));
                super::gr::data::stage_data(grkind as i32)
                    .file
                    .map(|name| data.add(archive(name)))
            },
        };
        // The trophies' data the stage load reads in (`Toy_803124BC`), for its disc load's
        // timing.
        let ty = load(super::gm::TROPHY_DATA_FILE);
        entries.push((
            super::gm::TROPHY_DATA_FILE.to_owned(),
            ty.entrynum,
            ty.bytes.len() as u32,
        ));
        load_fighter_files(&mut game, &mut data, &settings.start, load);
        // The effect files `efAsync_LoadSync` takes as the match and its fighters ask for
        // them: disc data, each read before it's needed. The common and menu effects
        // (`EfCoData.dat`, `EfMnData.dat`) are among them: the scene preloads those, and the
        // match's setup registers them (`efAsync_LoadSync(0)` and `(0x1F)`).
        for (i, f) in super::ef::FILES.iter().enumerate() {
            if let Some((name, _)) = f {
                let a =
                    Archive::parse(&load(name).bytes).unwrap_or_else(|e| panic!("{name}: {e:?}"));
                game.ef.files[i] = Some(data.add(a));
            }
        }
        let mut names: Vec<(String, ArchiveId)> = [
            ("LbRb.dat", rumble),
            ("LbRf.dat", files.lb_rf),
            ("PdPm.dat", files.pdpm),
            ("GmPause.usd", files.gm_pause),
            ("IfAll.usd", files.hud.if_all),
            ("SdIntro.dat", files.hud.sd_intro),
            ("IfCoGet.dat", files.hud.if_coget),
            ("LbBf.dat", files.lb_bf),
            ("ItCo.usd", files.it_co),
        ]
        .into_iter()
        .map(|(n, id)| (n.to_owned(), id))
        .collect();
        let grkind = crate::lb::audio::ground_kind(i32::from(settings.start.rules.stkind));
        if let (Some(name), Some(id)) =
            (super::gr::data::stage_data(grkind as i32).file, files.stage)
        {
            // The stage table's paths start at the disc's root.
            names.push((name.trim_start_matches('/').to_owned(), id));
        }
        let ft_files = &game.ft.data.files;
        names.extend(ft_files.archives.iter().cloned());
        for (name, file) in &ft_files.anim_files {
            names.extend(
                file.anims
                    .iter()
                    .map(|&(off, id)| (format!("{name}@{off:#x}"), id)),
            );
        }
        for (f, id) in super::ef::FILES.iter().zip(&game.ef.files) {
            if let (Some((name, _)), Some(id)) = (f, id) {
                names.push(((*name).to_owned(), *id));
            }
        }
        let mut m = Match {
            game,
            data,
            files: names,
        };
        m.game.rumble_data = table.map(|offset| melee_data::archive::DataRef {
            archive: rumble,
            offset,
        });
        let mut r = m.running();
        // `preloadState`: the scene's text arena (0x4800 bytes for a match).
        r.game.sis.init(0x4800);
        init_scene(&mut r);
        r.game.gm.on_frame = Some(CodeRef(vs::ON_FRAME));
        r.vs_on_enter(&settings.start, files);
        r.game.start_scene_loop();
        m
    }

    /// Takes up a match from a state: one imported from a capture of the original, say, at
    /// the start of an engine tick.
    pub fn resume(game: Game, data: Archives) -> Match {
        Match {
            game,
            data,
            files: Vec::new(),
        }
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
        let (mut out, render) = self.step_replay_tick(source);
        if render {
            out.draws = self.render();
        }
        out
    }

    /// [`Match::step_replay`] up to the frame's display pass, where ssbm-rs's state traces
    /// sample the original: whether the pass is due, which [`Match::render`] runs.
    pub fn step_replay_tick(&mut self, source: &mut dyn playback::Source) -> (Frame, bool) {
        let answers = Playback::answers(&self.game, source);
        self.game
            .playback
            .as_mut()
            .expect("a replay's match")
            .answers = Some(answers);
        let (t, render) = scene::step_tick(&mut self.running(), [PadStatus::default(); 4], false);
        self.game
            .playback
            .as_mut()
            .expect("a replay's match")
            .answers = None;
        let out = Frame {
            events: std::mem::take(&mut self.game.events),
            pads: t.pads,
            ended: t.ended,
            draws: Vec::new(),
        };
        (out, render)
    }

    /// The frame's display pass after [`Match::step_replay_tick`]: its draw commands.
    pub fn render(&mut self) -> Vec<crate::hsd::display::Draw> {
        scene::render_screen(&mut self.running());
        std::mem::take(&mut self.game.objs.display.draws)
    }

    /// One frame: the controllers' poll queued, then one engine tick.
    pub fn step(&mut self, pads: PadData) -> Frame {
        let t = scene::step(&mut self.running(), pads, false);
        Frame {
            events: std::mem::take(&mut self.game.events),
            pads: t.pads,
            ended: t.ended,
            draws: std::mem::take(&mut self.game.objs.display.draws),
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

/// Where the fighters' animation files start in ARAM (the ARAM heap's start, where the debug
/// VS mode's loads put the first one).
const ARAM_ANIM_BASE: u32 = 0x0062_9EA0;

/// Loads the files the match's fighters need, as the character select's preloading does
/// before the match: `PlCo.dat`, and for each playing slot's characters their data file, the
/// costume's file and the animation file (to ARAM, one archive per animation). The ARAM
/// addresses are laid out one after another from [`ARAM_ANIM_BASE`] in slot order, as the
/// debug VS mode's loads lay them out; a preloaded match's could differ (see
/// `docs/engine.md`), and only their identity matters to the simulation.
pub fn load_fighter_files(
    game: &mut Game,
    data: &mut Archives,
    start: &StartMeleeData,
    load: &mut dyn FnMut(&str) -> DiscFile,
) {
    use super::ft::data::{LoadedFiles, kind_tables};
    let mut add = |files: &mut LoadedFiles, data: &mut Archives, name: &str| {
        if !files.archives.iter().any(|(n, _)| n == name) {
            let file = load(name);
            let a = Archive::parse(&file.bytes).unwrap_or_else(|e| panic!("{name}: {e:?}"));
            files.archives.push((name.to_owned(), data.add(a)));
            files
                .entries
                .push((name.to_owned(), file.entrynum, file.bytes.len() as u32));
        }
    };
    let files = &mut game.ft.data.files;
    add(files, data, "PlCo.dat");
    let mut anims: Vec<&'static str> = Vec::new();
    for p in &start.players {
        if i32::from(p.slot_type) == super::pl::player::PKIND_NONE {
            continue;
        }
        let m = super::pl::player::mapping(i32::from(p.ckind));
        for kind in [m.internal_id, m.extra_internal_id] {
            if kind < 0 {
                continue;
            }
            let t = kind_tables(i32::from(kind));
            add(
                files,
                data,
                t.dat_file.expect("a fighter kind without a data file"),
            );
            let c = usize::from(p.color);
            let c = if c >= usize::from(t.costume_count) {
                0
            } else {
                c
            };
            add(
                files,
                data,
                t.costumes[c].file.expect("a costume without a file"),
            );
            let name = t.anim_file.expect("a fighter kind without animations");
            if !anims.contains(&name) {
                anims.push(name);
            }
        }
    }
    let mut aram = ARAM_ANIM_BASE;
    for name in anims {
        let DiscFile { entrynum, bytes } = load(name);
        files
            .entries
            .push((name.to_owned(), entrynum, bytes.len() as u32));
        let mut file = super::ft::AnimFile {
            aram,
            anims: Vec::new(),
        };
        // One archive per animation, each starting at a 32-byte boundary.
        let mut off = 0usize;
        let word = |i: usize| u32::from_be_bytes(bytes[i..i + 4].try_into().unwrap()) as usize;
        while off + 0x20 <= bytes.len() {
            let size = word(off);
            if size < 0x20 || off + size > bytes.len() {
                break;
            }
            let a = Archive::parse(&bytes[off..off + size])
                .unwrap_or_else(|e| panic!("{name}@{off:#x}: {e:?}"));
            file.anims.push((off as u32, data.add(a)));
            off = (off + size).next_multiple_of(32);
        }
        aram += (bytes.len() as u32).next_multiple_of(32);
        files.anim_files.push((name.to_owned(), file));
    }
}
