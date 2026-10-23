//! A VS match as a whole: started from its settings and stepped a frame at a time with the
//! controllers, the way the VS scene runs it (`gm_801A4014`: the scene's setup, its entry
//! `gm_Scene_Vs_OnEnter`, then `gm_801A4D34`'s loop of engine ticks).
//!
//! The match calls into fighters, items, effects, the HUD and audio through
//! [`code::call`](super::code::call) and the scheduler's process table; until those ports fill
//! their arms in, a match stops at the first unported function it reaches, saying which.

use melee_data::archive::{Archive, ArchiveId, Archives};

use super::gm::{self, scene, vs, vs_mode};
use super::handles::CodeRef;
use super::hud;
use super::mirror::{Cx, Mem, Mirror};
use super::playback::{self, Playback};
use super::slippi::{self, CodeError};
use super::state::{Event, Game, Rules, Running};
use super::structs::StartMeleeData;
use super::ucf;
use crate::audio::clock::DiscSpeed;
use crate::audio::dsp::DspModel;
use crate::input::controller::{PadData, Requests};
use crate::input::pad::PadStatus;

/// What a match starts from.
#[derive(melee_derive::CloneFrom, Debug, melee_derive::StateEq)]
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
    /// How fast the disc reads, which decides when the music stream's reads land between
    /// audio frames and so which voices are free: a recording keeps the pace it was made at,
    /// as ssbm-rs's runs read at Slippi's fast disc speed unless told `DISC_RATE`, and the
    /// player reads at a drive's, as ssbm-rs's window does (`docs/engine.md`).
    pub disc: DiscSpeed,
    /// UCF's pad buffer as the match starts (with `rules.ucf`).
    pub ucf: ucf::Pads,
    /// A Slippi replay drives the match through Slippi's playback codes
    /// ([`Match::step_replay`]).
    pub playback: bool,
    /// The addresses the original gives the match's fighters as it creates them, in creation
    /// order (`ft::Globals::address_seed`), where the frontend knows them: the oracle takes
    /// them from ssbm-rs's trace. Empty, the fighters' addresses are 0.
    pub fighter_addresses: Vec<u32>,
    /// How the scene's heap is laid out, for the allocator model (`hsd::heap`), which gives
    /// objects the addresses the original's allocators give them; none leaves it off.
    pub heap: Option<crate::hsd::heap::Layout>,
    /// What Slippi's playback codes allocate in the scene's heap for the replay's code list
    /// (`Playback/Core/RestoreGameInfo.asm`): the list, and the backup of the instructions
    /// its codes replace.
    pub playback_heap: Option<playback::CodeListHeap>,
    /// What the save data has unlocked, and whether the unlock-all codes run: the stage's
    /// music choice reads it (`gm_80164ABC`), and on some stages that decides whether it
    /// draws from the random seed (see engine.md). Slippi Dolphin runs the codes.
    pub unlocks: super::gm::Unlocks,
    /// Whether the match's start runs the boot's audio setup (`game::boot`) when what the
    /// boot leaves brought no audio timeline: the sound bank and the boot's banks loaded, and
    /// the timeline the audio frames run on. Without it the match runs without one: sounds
    /// start nothing and the match's bank loads go to the frontend as events.
    pub boot_audio: bool,
    /// Where the match's preload put the buffer Pokémon Stadium's transformation files load
    /// into (lbDvd's preload of entry 0x7D5): the preload cache, outside the simulation,
    /// places it; the stage keeps its address and its loads write there (a device command's
    /// destination). The oracle takes it from ssbm-rs's trace; else 0x80000000, as the
    /// other main-memory loads' destinations are.
    pub stadium_buffer: u32,
    /// Records the match as a Slippi replay (`game::record`), with what the recording reads
    /// from outside the match: presentation output, which changes nothing the match does.
    pub record: Option<super::record::Setup>,
}

/// What Slippi's playback hands the game at a replay's start (the Slippi EXI device's answer
/// to its game info request), from the replay's game start event.
#[derive(melee_derive::CloneFrom, Debug, melee_derive::StateEq)]
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

/// The Ice Climbers' `CharacterKind`.
const CKIND_ICE_CLIMBERS: u8 = 14;
/// `Gm_PKind`'s computer player.
const PKIND_CPU: u8 = 1;
const CKIND_MARTH: i8 = 9;

/// Whether the rules' item switch can spawn a Super Scope: items on (`it_8026D018`) with its
/// kind in the switch's mask, which the spawner, the containers and the Party Ball all follow.
fn scope_on(rules: &super::structs::StartMeleeRules) -> bool {
    rules.item_freq != -1 && rules.x20 >> super::it::kinds::S_SCOPE & 1 != 0
}

/// A player in [`Settings::vs`].
#[derive(Clone, Copy, Debug, melee_derive::StateEq)]
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
    /// An ordinary VS match: stock (`stocks` above 0) or timed, with a time limit of
    /// `minutes` (0 for none), on `stage` (`StKind`), up to four players, and otherwise the VS
    /// menus' rules of a new save with the item switch off. [`Settings::vs_with`] takes any
    /// menu rules.
    pub fn vs(stage: u16, players: &[Player], stocks: u8, minutes: u8) -> Settings {
        Settings::vs_items(
            stage,
            players,
            stocks,
            minutes,
            &vs_mode::GamePrefs::items_off(),
        )
    }

    /// [`Settings::vs`] with the item frequency and switch of `prefs`
    /// ([`GamePrefs::items`](vs_mode::GamePrefs::items)), as the player's launcher starts a match.
    pub fn vs_items(
        stage: u16,
        players: &[Player],
        stocks: u8,
        minutes: u8,
        prefs: &vs_mode::GamePrefs,
    ) -> Settings {
        let rules = vs_mode::GameRules::vs(stocks, minutes);
        Settings::vs_with(stage, players, &rules, prefs)
    }

    /// A VS match as the VS mode starts it (`gmVsMelee_EnterVs`) from the menus' `rules` and
    /// `prefs`: the VS mode's start data with what the character select screen sets for each
    /// player (character, kind, costume, team and CPU level) and the stage select's stage.
    ///
    /// With a computer player or an Ice Climber among the players the allocator model is on,
    /// laid out as a console's VS match from the menus lays out the heap
    /// (`Layout::vanilla_vs`): a computer player's DI with no knockback, Nana's too, reads its
    /// `Fighter*`'s address, which the model gives (docs/engine.md, docs/player.md). So it is
    /// when the item switch can spawn a Super Scope: its one-shot charge takes the scope's GObj
    /// address for a level (docs/melee-bugs.md). Otherwise it stays off, which costs nothing.
    pub fn vs_with(
        stage: u16,
        players: &[Player],
        rules: &vs_mode::GameRules,
        prefs: &vs_mode::GamePrefs,
    ) -> Settings {
        Settings::vs_special(stage, players, rules, prefs, None)
    }

    /// [`Settings::vs_with`] in a Special Melee mode (`special`), as that mode's VS entry
    /// starts the match ([`vs_mode::Special::enter_vs`]), in the mode's game mode; none is VS
    /// mode's.
    pub fn vs_special(
        stage: u16,
        players: &[Player],
        rules: &vs_mode::GameRules,
        prefs: &vs_mode::GamePrefs,
        special: Option<vs_mode::Special>,
    ) -> Settings {
        assert!(players.len() <= 4, "a VS match has up to four players");
        let mut vs = vs_mode::start_defaults();
        vs.rules.stkind = stage;
        for p in players {
            let d = &mut vs.players[usize::from(p.port)];
            d.ckind = p.character as i8;
            d.slot_type = p.kind;
            d.color = p.costume;
            d.team = p.team;
            d.cpu_level = p.cpu_level;
        }
        let start = match special {
            Some(m) => m.enter_vs(&mut vs, rules, prefs),
            None => vs_mode::enter_vs(&mut vs, rules, prefs),
        };
        let heap = players
            .iter()
            .any(|p| p.kind == PKIND_CPU || p.character == CKIND_ICE_CLIMBERS)
            || scope_on(&start.rules);
        Settings {
            start,
            seed: 0,
            game_mode: special.map_or(gm::GM_VS, vs_mode::Special::game_mode),
            rules: Rules::default(),
            // A player hears the match: the voices play through their samples, and the music
            // needs a drive's pace to keep playing.
            dsp: DspModel::Samples,
            disc: DiscSpeed::Drive,
            ucf: ucf::Pads::default(),
            playback: false,
            fighter_addresses: Vec::new(),
            heap: heap.then(crate::hsd::heap::Layout::vanilla_vs),
            playback_heap: None,
            unlocks: super::gm::Unlocks::slippi(),
            boot_audio: true,
            stadium_buffer: super::ft::data::MAIN_MEMORY,
            record: None,
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
        // Marth's trail colors read his costume a fixed distance before his attribute block,
        // wherever the heap put it (`game::player_codes`): a match with Marth is refused.
        if s.rules.player.marth_trail
            && s.start
                .players
                .iter()
                .any(|p| p.slot_type != 3 && p.ckind == CKIND_MARTH)
        {
            return Err(CodeError::Unknown {
                kind: 0xC2,
                addr: 0x8013_6510,
            });
        }
        s.ucf = ucf::Pads::from_codes(&codes).unwrap_or_default();
        s.playback = true;
        s.heap = Some(crate::hsd::heap::Layout::slippi_playback());
        s.playback_heap = Some(playback::CodeListHeap::of(&r.codes));
        Ok(s)
    }

    /// The settings a Slippi replay's game start records: its game info block (the match's
    /// `StartMeleeData` as the game held it, 0x138 bytes) and random seed. A replay doesn't
    /// say how its sound played; this takes the sample-accurate model, as Slippi Dolphin
    /// recorded it and ssbm-rs plays replays unless told `AX=0`, so the two compare.
    /// [`DspModel::Silent`] stays available by setting `dsp`. The disc reads at Slippi's fast
    /// disc speed, as ssbm-rs's traces of replays do; a drive's pace is `disc`.
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
            disc: DiscSpeed::Fast,
            fighter_addresses: Vec::new(),
            heap: None,
            playback_heap: None,
            unlocks: super::gm::Unlocks::slippi(),
            boot_audio: true,
            stadium_buffer: super::ft::data::MAIN_MEMORY,
            record: None,
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
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
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
#[derive(melee_derive::CloneFrom, Debug)]
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
        game.recorder = settings
            .record
            .clone()
            .map(|s| Box::new(super::record::Recorder::new(s)));
        if settings.playback {
            game.playback = Some(Playback {
                start_seed: settings.seed,
                code_list_heap: settings.playback_heap,
                ..Playback::default()
            });
            game.pads.playback = true;
        }
        game.ft.address_seed = settings.fighter_addresses.iter().copied().collect();
        game.gm.unlocks = settings.unlocks;
        game.gr.stadium.buffer_address = settings.stadium_buffer;
        // `main` turns the items' own spawning on (`db_EnableItemSpawns`; `gmMain_804D6594`
        // is false on a retail disc).
        game.it.db_menu.ItemSpawnsEnabled = 1;
        let mut data = Archives::default();
        boot(&mut game, &mut data);
        // Before the boot's audio setup reads the disc, and over a seeded boot state's.
        game.audio.disc = settings.disc;
        if settings.boot_audio && game.audio.clock.is_none() {
            super::boot::boot(&mut game, &mut data, settings, load);
        }
        // After the boot's state, which may bring its own audio library.
        game.audio.dsp = settings.dsp;
        if game.audio.clock.is_some() {
            // The sound banks the match's bank choice may load, for the device command queue
            // to read.
            for name in crate::lb::audio::SSM_NAMES {
                let file = load(&format!("audio/us/{name}"));
                if !game.audio.devcom.files.iter().any(|f| f.0 == file.entrynum) {
                    let id = data.add(Archive::raw(&file.bytes));
                    game.audio.devcom.files.push((file.entrynum, id));
                }
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
        // Pokémon Stadium's transformation files, which it loads during the match: disc data
        // the frontend loads with the match's archives, as the fighters' files are.
        let stadium_files: Vec<(&str, ArchiveId)> = if super::gr::data::stage_data(
            crate::lb::audio::ground_kind(i32::from(settings.start.rules.stkind)) as i32,
        )
        .grkind
            == super::gr::GR_KIND_PSTADIUM
        {
            super::gr::stadium::FILES
                .iter()
                .map(|&name| (name, data.add(archive(name))))
                .collect()
        } else {
            Vec::new()
        };
        for (i, &(_, id)) in stadium_files.iter().enumerate() {
            game.gr.stadium.files[i] = Some(id);
        }
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
                let file = load(name);
                let a = Archive::parse(&file.bytes).unwrap_or_else(|e| panic!("{name}: {e:?}"));
                game.ef.files[i] = Some(data.add(a));
                game.ft.data.files.entries.push((
                    (*name).to_owned(),
                    file.entrynum,
                    file.bytes.len() as u32,
                ));
            }
        }
        if settings.heap.is_some_and(|l| l.from_menus) {
            let (files, buffers) = preloaded_files(&settings.start);
            game.ft.data.files.preloaded = files;
            game.ft.data.files.preloaded_buffers = buffers;
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
        names.extend(stadium_files.iter().map(|&(n, id)| (n.to_owned(), id)));
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
        let lent = crate::hsd::heap::Lent::take(&mut m.game.objs.heap);
        let mut r = m.running();
        // The boot's last scene ends (its tick's end is where the boot state was taken), then
        // the VS mode's step before the match's scene (`gmVsMelee_EnterVs`, which a replay's
        // playback takes too): the announcer's bank.
        if r.game.audio.clock.is_some() {
            r.game.audio.end_scene_loop(r.data);
        }
        r.load_announcer();
        // The scene's heap, made anew (`lbHeap_80015900`, `HSD_CreateMainHeap`).
        if let Some(layout) = &settings.heap {
            crate::hsd::heap::start_scene_lent(layout);
        }
        // `preloadState`: the scene's text arena (0x4800 bytes for a match).
        r.game.sis.init(0x4800);
        crate::hsd::heap::with(|h| h.mem_alloc(0x4800));
        init_scene(&mut r);
        r.game.gm.on_frame = Some(CodeRef(vs::ON_FRAME));
        r.vs_on_enter(&settings.start, files);
        // The polls the pad alarm queued while the scene set up, which the timeline counted
        // (`HSD_PadRenewRawStatus` from the alarm): controllers at rest, as a replay's are.
        // The timeline's count starts from the boot state's queue, which the seeded pad library
        // already holds: only the alarms since are new polls.
        let polls = r.game.audio.clock.as_ref().map_or(0, |c| {
            c.pads.saturating_sub(u32::from(r.game.pads.lib.qcount))
        });
        for _ in 0..polls {
            r.game
                .pads
                .renew_raw(r.data, [PadStatus::default(); 4], false, false);
        }
        r.game.start_scene_loop();
        // The scene loop's prologue on the audio timeline: its pad queue's flush and a first
        // round without a render.
        if r.game.audio.clock.is_some() {
            r.game.audio.start_scene_loop(r.data);
        }
        lent.give_back(&mut m.game.objs.heap);
        // The song the stage started, for the device command queue to read: its first reads
        // finish in the scene's first ticks.
        let stem = m.game.audio.lb.stem;
        let len = stem.iter().position(|&c| c == 0).unwrap_or(stem.len());
        if m.game.audio.clock.is_some() && len > 1 {
            let path = std::str::from_utf8(&stem[1..len]).expect("a music path");
            let file = load(path);
            if !m
                .game
                .audio
                .devcom
                .files
                .iter()
                .any(|f| f.0 == file.entrynum)
            {
                let id = m.data.add(Archive::raw(&file.bytes));
                m.game.audio.devcom.files.push((file.entrynum, id));
            }
        }
        // Mushroom Kingdom's or Mushroom Kingdom II's fast song, which its last 20 seconds
        // start.
        if m.game.audio.clock.is_some()
            && matches!(
                m.game.stage_info.grkind,
                super::gr::GR_KIND_INISHIE1 | super::gr::GR_KIND_INISHIE2
            )
            && let Some(fast) = super::gr::inishie1::fast_song(m.game.stage_info.x98)
        {
            let name = crate::lb::audio::HPS_NAMES[fast as usize];
            let file = load(&format!("audio/{name}"));
            if !m
                .game
                .audio
                .devcom
                .files
                .iter()
                .any(|f| f.0 == file.entrynum)
            {
                let id = m.data.add(Archive::raw(&file.bytes));
                m.game.audio.devcom.files.push((file.entrynum, id));
            }
        }
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
        let lent = crate::hsd::heap::Lent::take(&mut self.game.objs.heap);
        let out = self.step_replay_tick_lent(source);
        lent.give_back(&mut self.game.objs.heap);
        out
    }

    fn step_replay_tick_lent(&mut self, source: &mut dyn playback::Source) -> (Frame, bool) {
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
        let lent = crate::hsd::heap::Lent::take(&mut self.game.objs.heap);
        scene::render_screen(&mut self.running());
        lent.give_back(&mut self.game.objs.heap);
        std::mem::take(&mut self.game.objs.display.draws)
    }

    /// Takes a frame's buffers back once the frontend is done with it, so later frames fill
    /// them rather than allocating new ones. Changes nothing the game reads.
    pub fn recycle(&mut self, mut frame: Frame) {
        frame.events.clear();
        frame.draws.clear();
        if self.game.events.is_empty() && self.game.events.capacity() < frame.events.capacity() {
            self.game.events = frame.events;
        }
        let draws = &mut self.game.objs.display.draws;
        if draws.is_empty() && draws.capacity() < frame.draws.capacity() {
            *draws = frame.draws;
        }
    }

    /// [`Match::step`] up to the frame's display pass, where ssbm-rs's state traces sample the
    /// original: the tick's frame without draws, and whether the pass is due, which
    /// [`Match::render`] runs.
    pub fn step_tick(&mut self, pads: PadData) -> (Frame, bool) {
        let lent = crate::hsd::heap::Lent::take(&mut self.game.objs.heap);
        let (t, render) = scene::step_tick(&mut self.running(), pads, false);
        lent.give_back(&mut self.game.objs.heap);
        let out = Frame {
            events: std::mem::take(&mut self.game.events),
            pads: t.pads,
            ended: t.ended,
            draws: Vec::new(),
        };
        (out, render)
    }

    /// One frame: the controllers' poll queued, then one engine tick.
    pub fn step(&mut self, pads: PadData) -> Frame {
        let lent = crate::hsd::heap::Lent::take(&mut self.game.objs.heap);
        let t = scene::step(&mut self.running(), pads, false);
        lent.give_back(&mut self.game.objs.heap);
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
    // `HSD_GObjInit`'s lists: the process links' heads and tails, the render links' heads and
    // tails, the process priorities' heads, the processes by link and priority, and the
    // functions' table.
    crate::hsd::heap::with(|h| {
        for size in [0x100, 0x100, 0x104, 0x104, 0x64, 0x1900, 0x14] {
            h.mem_alloc(size);
        }
    });
    let g = &mut *r.game;
    g.setup_scene_text(r.data);
    let gobj = g.world.create(14, 0, 0);
    g.world.setup_proc(gobj, CodeRef(vs::SCENE_PROC), 0);
    g.gm.scene_gobj = Some(gobj);
    g.reset_controller_map();
    // Slippi's `Common/AllocSceneBuffer.asm` (at `gm_801A4BD4+0xE0`, in its playback set):
    // a buffer for the scene, the last scene's freed with that scene's heap.
    if g.playback.is_some() {
        crate::hsd::heap::with(|h| h.mem_alloc(0x80));
    }
    g.lb_reset_sound_objects();
}

/// The files and buffers the VS mode's character and stage selects leave in the preload cache
/// for a match (`lbDvd_80017960` from the game cache they fill): the stage's archive and the
/// buffers its stage asks for (`Ground_801C06B8`: Fountain of Dreams' reflection, Pokémon
/// Stadium's screens and transformation buffer, by preload entry), and for each playing
/// slot's characters (`Player_80031CB0`, `ftData_800855C8`) their data file, the costume's
/// file, their effect file and their animation file.
pub fn preloaded_files(start: &StartMeleeData) -> (Vec<String>, Vec<i32>) {
    use super::ft::data::kind_tables;
    let mut out: Vec<String> = Vec::new();
    let mut add = |name: &str| {
        if !out.iter().any(|n| n == name) {
            out.push(name.to_owned());
        }
    };
    let grkind = crate::lb::audio::ground_kind(i32::from(start.rules.stkind)) as i32;
    if let Some(name) = super::gr::data::stage_data(grkind).file {
        add(name);
    }
    // `grIzumi_801CD2D4` and `grStadium_801D511C`.
    let buffers = match grkind {
        super::gr::GR_KIND_IZUMI => vec![2001],
        super::gr::GR_KIND_PSTADIUM => vec![0x7D2, 0x7D3, 0x7D4, 0x7D5],
        _ => Vec::new(),
    };
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
            if let Some(name) = t.dat_file {
                add(name);
            }
            let c = usize::from(p.color);
            let c = if c >= usize::from(t.costume_count) {
                0
            } else {
                c
            };
            if let Some(name) = t.costumes[c].disc_file() {
                add(&name);
            }
            if let Some(Some((name, _))) = super::ef::FILES.get(usize::from(t.effect_file)) {
                add(name);
            }
            if let Some(name) = t.anim_file {
                add(name);
            }
        }
    }
    (out, buffers)
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
                &t.costumes[c].disc_file().expect("a costume without a file"),
            );
            let name = t.anim_file.expect("a fighter kind without animations");
            if !anims.contains(&name) {
                anims.push(name);
            }
        }
    }
    // Kirby's copies of the other players' characters (`fn_8016E124`,
    // `ftKb_SpecialN_800EED50`): each kind's copy file, and for the kinds whose copy wears his
    // costume's colors, the file for each Kirby's costume.
    let kirby_costumes: Vec<usize> = start
        .players
        .iter()
        .filter(|p| {
            i32::from(p.slot_type) != super::pl::player::PKIND_NONE
                && i32::from(p.ckind) == super::pl::player::CKIND_KIRBY
        })
        .map(|p| usize::from(p.color))
        .collect();
    if !kirby_costumes.is_empty() {
        use super::ft::kirby::init::{COPY_FILES, copy_costume_files};
        for p in &start.players {
            if i32::from(p.slot_type) == super::pl::player::PKIND_NONE {
                continue;
            }
            let m = super::pl::player::mapping(i32::from(p.ckind));
            for kind in [m.internal_id, m.extra_internal_id] {
                if kind < 0 || i32::from(kind) == super::ft::kirby::KIND_KIRBY {
                    continue;
                }
                if let Some((file, _)) = COPY_FILES[kind as usize] {
                    add(files, data, file);
                }
                for &c in &kirby_costumes {
                    if let Some((file, _, _)) = copy_costume_files(kind as usize, c) {
                        add(files, data, &file);
                    }
                }
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
            // The copy into a fighter's buffer takes the size rounded up to 32 bytes
            // (`lbArq_80014BD0`), so the file's next bytes come along.
            let after = &bytes[off + size..(off + size).next_multiple_of(32).min(bytes.len())];
            let a = Archive::parse_in(&bytes[off..off + size], after)
                .unwrap_or_else(|e| panic!("{name}@{off:#x}: {e:?}"));
            file.anims.push((off as u32, data.add(a)));
            off = (off + size).next_multiple_of(32);
        }
        aram += (bytes.len() as u32).next_multiple_of(32);
        files.anim_files.push((name.to_owned(), file));
    }
}

#[cfg(test)]
mod tests {
    use super::{Player, Settings};

    fn player(character: u8, port: u8) -> Player {
        Player {
            character,
            costume: 0,
            port,
            kind: 0,
            cpu_level: 0,
            team: port,
        }
    }

    #[test]
    fn ice_climbers_turn_on_the_allocator_model() {
        let fox = player(2, 0);
        let ics = player(14, 1);
        assert_eq!(Settings::vs(32, &[fox, player(9, 1)], 4, 8).heap, None);
        assert_eq!(
            Settings::vs(32, &[fox, ics], 4, 8).heap,
            Some(crate::hsd::heap::Layout::vanilla_vs())
        );
    }

    #[test]
    fn a_super_scope_turns_on_the_allocator_model() {
        use super::vs_mode::{GamePrefs, GameRules};
        let players = [player(2, 0), player(9, 1)];
        let heap = |item_freq, item_mask| {
            let prefs = GamePrefs {
                item_freq,
                item_mask,
                ..Default::default()
            };
            Settings::vs_with(32, &players, &GameRules::default(), &prefs).heap
        };
        // The item switch's entry 24 is the Super Scope.
        let scope = 1 << 24;
        assert_eq!(heap(2, scope), Some(crate::hsd::heap::Layout::vanilla_vs()));
        assert_eq!(heap(2, !scope), None);
        assert_eq!(heap(0xFF, u64::MAX), None);
    }
}
