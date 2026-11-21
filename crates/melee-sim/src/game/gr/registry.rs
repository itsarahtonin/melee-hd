//! The stage registry: one [`Stage`] entry per ground kind ([`GrKind`], the original's
//! `stage_datas` index), holding everything that names the stage: its name, its data (the
//! executable's `StageData`: archive, objects' callbacks, collision joints), its hooks (setup,
//! load, start and demo), its objects' callbacks for the callback table, the variables its
//! objects use, and its roster line (the stage kind a VS match on it starts with, the command
//! line's names and its audit, [`roster`](super::super::roster)).
//!
//! The game numbers its stages twice, and both numbers are the game's own, so both are enums
//! here with those numbers: a [`GrKind`] says which stage's code and archive run (`stage_info`'s
//! `grkind`, 71 of them), a [`StKind`] which stage a mode starts (`StartMeleeRules`' stage, the
//! replays' and the stage select's number, 286 of them: the VS stages, then the single-player
//! and event variants). The game's `stage_id_map` takes a stage kind to its ground kind
//! ([`StKind::ground`]); several stage kinds share a ground kind.
//!
//! A new stage's port writes its module (its hooks and its objects' `#[callbacks]` dispatch) and
//! its data, and fills its entry here; nothing else lists stages. The state keeps the game's
//! numbers (`stage_info.grkind`, `gr.stkind`), so it and the oracle are unchanged; code reads
//! them through the enums ([`GrKind::from_id`], `stage_info.grkind == GrKind::Castle`).

use super::super::code::{Entries, kind};
use super::super::roster::Audit;
use super::super::state::Running;
use super::data::{self, StageData};
use super::{
    VarsKind, battlefield, big_blue, brinstar, brinstar_depths, brinstar_escape, corneria,
    dream_land, final_destination, flat_zone, fountain_of_dreams, fourside, fzero_race, great_bay,
    green_greens, home_run_contest, hyrule_temple, icicle_mountain, jungle_japes, kongo_jungle,
    kongo_jungle_64, mushroom_kingdom, mushroom_kingdom_2, mushroom_kingdom_route, mute_city,
    onett, peachs_castle, poke_floats, pokemon_stadium, race_to_the_finish, rainbow_cruise,
    rest_area, snag_the_trophies, target_test, test_stage, trophy_stages, underground_maze, venom,
    yoshis_island, yoshis_island_64, yoshis_story,
};
use crate::hsd::jobj::JObjId;
use crate::vec::Vec3;
use melee_data::archive::DataRef;

/// A ground kind (`GrKind`): which stage's code and archive run, the index of the
/// executable's `stage_datas` and of this registry ([`STAGES`]). The game's own numbers.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, melee_derive::StateEq)]
pub enum GrKind {
    /// `Gr_Kind_Unk00`: the dummy stage.
    Dummy = 0,
    /// `Gr_Kind_Test`: the test stage.
    Test = 1,
    /// `Gr_Kind_Castle`: Princess Peach's Castle.
    Castle = 2,
    /// `Gr_Kind_RCruise`: Rainbow Cruise.
    RCruise = 3,
    /// `Gr_Kind_Kongo`: Kongo Jungle.
    Kongo = 4,
    /// `Gr_Kind_Garden`: Jungle Japes.
    Garden = 5,
    /// `Gr_Kind_GreatBay`: Great Bay.
    GreatBay = 6,
    /// `Gr_Kind_Shrine`: Hyrule Temple.
    Shrine = 7,
    /// `Gr_Kind_Zebes`: Brinstar.
    Zebes = 8,
    /// `Gr_Kind_Kraid`: Brinstar Depths.
    Kraid = 9,
    /// `Gr_Kind_Story`: Yoshi's Story.
    Story = 10,
    /// `Gr_Kind_Yorster`: Yoshi's Island.
    Yorster = 11,
    /// `Gr_Kind_Izumi`: Fountain of Dreams.
    Izumi = 12,
    /// `Gr_Kind_Greens`: Green Greens.
    Greens = 13,
    /// `Gr_Kind_Corneria`: Corneria.
    Corneria = 14,
    /// `Gr_Kind_Venom`: Venom.
    Venom = 15,
    /// `Gr_Kind_PStadium`: Pokémon Stadium.
    PStadium = 16,
    /// `Gr_Kind_Pura`: Poké Floats.
    Pura = 17,
    /// `Gr_Kind_MuteCity`: Mute City.
    MuteCity = 18,
    /// `Gr_Kind_BigBlue`: Big Blue.
    BigBlue = 19,
    /// `Gr_Kind_Onett`: Onett.
    Onett = 20,
    /// `Gr_Kind_Fourside`: Fourside.
    Fourside = 21,
    /// `Gr_Kind_Icemt`: Icicle Mountain.
    Icemt = 22,
    /// `Gr_Kind_Unk23`: not ported.
    Unk23 = 23,
    /// `Gr_Kind_Inishie1`: Mushroom Kingdom.
    Inishie1 = 24,
    /// `Gr_Kind_Inishie2`: Mushroom Kingdom II.
    Inishie2 = 25,
    /// `Gr_Kind_Unk26`: not ported.
    Unk26 = 26,
    /// `Gr_Kind_Flatzone`: Flat Zone.
    Flatzone = 27,
    /// `Gr_Kind_OldPupupu`: Dream Land.
    OldPupupu = 28,
    /// `Gr_Kind_OldYoshi`: Yoshi's Island N64.
    OldYoshi = 29,
    /// `Gr_Kind_OldKongo`: Kongo Jungle N64.
    OldKongo = 30,
    /// `Gr_Kind_KinokoRoute`: Mushroom Kingdom (Adventure).
    KinokoRoute = 31,
    /// `Gr_Kind_ShrineRoute`: Underground Maze.
    ShrineRoute = 32,
    /// `Gr_Kind_ZebesRoute`: Escape from Brinstar.
    ZebesRoute = 33,
    /// `Gr_Kind_BigBlueRoute`: F-Zero Grand Prix.
    BigBlueRoute = 34,
    /// `Gr_Kind_Unk35`: the test stage (its second ground kind).
    Unk35 = 35,
    /// `Gr_Kind_Battle`: Battlefield.
    Battle = 36,
    /// `Gr_Kind_Last`: Final Destination.
    Last = 37,
    /// `Gr_Kind_FigureGet`: Snag the Trophies.
    FigureGet = 38,
    /// `Gr_Kind_Pushon`: Race to the Finish.
    Pushon = 39,
    /// `Gr_Kind_TMario`: Target Test (Mario).
    TMario = 40,
    /// `Gr_Kind_TCaptain`: Target Test (Captain Falcon).
    TCaptain = 41,
    /// `Gr_Kind_TClink`: Target Test (Young Link).
    TClink = 42,
    /// `Gr_Kind_TDonkey`: Target Test (Donkey Kong).
    TDonkey = 43,
    /// `Gr_Kind_TDrmario`: Target Test (Dr. Mario).
    TDrmario = 44,
    /// `Gr_Kind_TFalco`: Target Test (Falco).
    TFalco = 45,
    /// `Gr_Kind_TFox`: Target Test (Fox).
    TFox = 46,
    /// `Gr_Kind_TIceclimber`: Target Test (the Ice Climbers).
    TIceclimber = 47,
    /// `Gr_Kind_TKirby`: Target Test (Kirby).
    TKirby = 48,
    /// `Gr_Kind_TKoopa`: Target Test (Bowser).
    TKoopa = 49,
    /// `Gr_Kind_TLink`: Target Test (Link).
    TLink = 50,
    /// `Gr_Kind_TLuigi`: Target Test (Luigi).
    TLuigi = 51,
    /// `Gr_Kind_TMars`: Target Test (Marth).
    TMars = 52,
    /// `Gr_Kind_TMewtwo`: Target Test (Mewtwo).
    TMewtwo = 53,
    /// `Gr_Kind_TNess`: Target Test (Ness).
    TNess = 54,
    /// `Gr_Kind_TPeach`: Target Test (Peach).
    TPeach = 55,
    /// `Gr_Kind_TPichu`: Target Test (Pichu).
    TPichu = 56,
    /// `Gr_Kind_TPikachu`: Target Test (Pikachu).
    TPikachu = 57,
    /// `Gr_Kind_TPurin`: Target Test (Jigglypuff).
    TPurin = 58,
    /// `Gr_Kind_TSamus`: Target Test (Samus).
    TSamus = 59,
    /// `Gr_Kind_TSeak`: Target Test (Sheik).
    TSeak = 60,
    /// `Gr_Kind_TYoshi`: Target Test (Yoshi).
    TYoshi = 61,
    /// `Gr_Kind_TZelda`: Target Test (Zelda).
    TZelda = 62,
    /// `Gr_Kind_TGamewatch`: Target Test (Mr. Game & Watch).
    TGamewatch = 63,
    /// `Gr_Kind_TEmblem`: Target Test (Roy).
    TEmblem = 64,
    /// `Gr_Kind_TGanon`: Target Test (Ganondorf).
    TGanon = 65,
    /// `Gr_Kind_Heal`: All-Star's rest area.
    Heal = 66,
    /// `Gr_Kind_Homerun`: Home-Run Contest.
    Homerun = 67,
    /// `Gr_Kind_Figure1`: the events' first trophy stage.
    Figure1 = 68,
    /// `Gr_Kind_Figure2`: the events' second trophy stage.
    Figure2 = 69,
    /// `Gr_Kind_Figure3`: the events' third trophy stage.
    Figure3 = 70,
}

impl GrKind {
    /// How many there are (`stage_datas`' length).
    pub const COUNT: usize = 71;

    /// Every ground kind, by number.
    pub const ALL: [GrKind; Self::COUNT] = [
        GrKind::Dummy,
        GrKind::Test,
        GrKind::Castle,
        GrKind::RCruise,
        GrKind::Kongo,
        GrKind::Garden,
        GrKind::GreatBay,
        GrKind::Shrine,
        GrKind::Zebes,
        GrKind::Kraid,
        GrKind::Story,
        GrKind::Yorster,
        GrKind::Izumi,
        GrKind::Greens,
        GrKind::Corneria,
        GrKind::Venom,
        GrKind::PStadium,
        GrKind::Pura,
        GrKind::MuteCity,
        GrKind::BigBlue,
        GrKind::Onett,
        GrKind::Fourside,
        GrKind::Icemt,
        GrKind::Unk23,
        GrKind::Inishie1,
        GrKind::Inishie2,
        GrKind::Unk26,
        GrKind::Flatzone,
        GrKind::OldPupupu,
        GrKind::OldYoshi,
        GrKind::OldKongo,
        GrKind::KinokoRoute,
        GrKind::ShrineRoute,
        GrKind::ZebesRoute,
        GrKind::BigBlueRoute,
        GrKind::Unk35,
        GrKind::Battle,
        GrKind::Last,
        GrKind::FigureGet,
        GrKind::Pushon,
        GrKind::TMario,
        GrKind::TCaptain,
        GrKind::TClink,
        GrKind::TDonkey,
        GrKind::TDrmario,
        GrKind::TFalco,
        GrKind::TFox,
        GrKind::TIceclimber,
        GrKind::TKirby,
        GrKind::TKoopa,
        GrKind::TLink,
        GrKind::TLuigi,
        GrKind::TMars,
        GrKind::TMewtwo,
        GrKind::TNess,
        GrKind::TPeach,
        GrKind::TPichu,
        GrKind::TPikachu,
        GrKind::TPurin,
        GrKind::TSamus,
        GrKind::TSeak,
        GrKind::TYoshi,
        GrKind::TZelda,
        GrKind::TGamewatch,
        GrKind::TEmblem,
        GrKind::TGanon,
        GrKind::Heal,
        GrKind::Homerun,
        GrKind::Figure1,
        GrKind::Figure2,
        GrKind::Figure3,
    ];

    /// The first of Target Test's 26 stages, one per character, in the characters' order.
    pub const FIRST_TARGET: GrKind = GrKind::TMario;

    /// The ground kind numbered `id`, if the game has one.
    pub const fn from_id(id: i32) -> Option<GrKind> {
        if id >= 0 && (id as usize) < Self::COUNT {
            Some(Self::ALL[id as usize])
        } else {
            None
        }
    }

    /// Its number, as the state and the game's tables hold it.
    pub const fn id(self) -> i32 {
        self as i32
    }

    /// Its registry entry; stops if the stage isn't ported.
    pub fn stage(self) -> &'static Stage {
        stage(self.id())
    }

    /// Which of Target Test's stages it is (0 for Mario's), if it's one.
    pub const fn target_index(self) -> Option<usize> {
        let i = self as usize;
        let first = Self::FIRST_TARGET as usize;
        if i >= first && i < first + target_test::TARGETS.len() {
            Some(i - first)
        } else {
            None
        }
    }

    /// Target Test's stages, in order.
    pub fn targets() -> impl Iterator<Item = GrKind> {
        Self::ALL.into_iter().filter(|k| k.target_index().is_some())
    }
}

impl PartialEq<GrKind> for i32 {
    fn eq(&self, k: &GrKind) -> bool {
        *self == k.id()
    }
}

impl PartialEq<i32> for GrKind {
    fn eq(&self, id: &i32) -> bool {
        self.id() == *id
    }
}

impl super::super::structs::StageInfo {
    /// The stage's ground kind (`grkind`), which the game only ever sets to one of its own.
    pub fn ground_kind(&self) -> GrKind {
        GrKind::from_id(self.grkind)
            .unwrap_or_else(|| panic!("stage_info.grkind {} is no ground kind", self.grkind))
    }
}

/// A stage kind (`StKind`): which stage a mode starts, as `StartMeleeRules`, the replays and
/// the stage select number it; the game's `stage_id_map` takes it to its [`GrKind`]. The
/// game's own numbers. Only the decompilation's names and the kinds the code names are
/// here: the others (single-player and event variants) stay numbers.
#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StKind {
    /// `St_Kind_Dummy`.
    Dummy = 0,
    /// `St_Kind_Test`.
    Test = 1,
    /// `St_Kind_Izumi`.
    Izumi = 2,
    /// `St_Kind_PStadium`.
    PStadium = 3,
    /// `St_Kind_Castle`.
    Castle = 4,
    /// `St_Kind_Kongo`.
    Kongo = 5,
    /// `St_Kind_Zebes`.
    Zebes = 6,
    /// `St_Kind_Corneria`.
    Corneria = 7,
    /// `St_Kind_Story`.
    Story = 8,
    /// `St_Kind_Onett`.
    Onett = 9,
    /// `St_Kind_MuteCity`.
    MuteCity = 10,
    /// `St_Kind_RCruise`.
    RCruise = 11,
    /// `St_Kind_Garden`.
    Garden = 12,
    /// `St_Kind_GreatBay`.
    GreatBay = 13,
    /// `St_Kind_Shrine`.
    Shrine = 14,
    /// `St_Kind_Kraid`.
    Kraid = 15,
    /// `St_Kind_Yoster`.
    Yorster = 16,
    /// `St_Kind_Greens`.
    Greens = 17,
    /// `St_Kind_Fourside`.
    Fourside = 18,
    /// `St_Kind_Inishie1`.
    Inishie1 = 19,
    /// `St_Kind_Inishie2`.
    Inishie2 = 20,
    /// `St_Kind_Akaneia`.
    Akaneia = 21,
    /// `St_Kind_Venom`.
    Venom = 22,
    /// `St_Kind_Pura`.
    Pura = 23,
    /// `St_Kind_BigBlue`.
    BigBlue = 24,
    /// `St_Kind_Icemt`.
    Icemt = 25,
    /// `St_Kind_Icetop`.
    Icetop = 26,
    /// `St_Kind_Flatzone`.
    Flatzone = 27,
    /// `St_Kind_OldPupupu`.
    OldPupupu = 28,
    /// `St_Kind_OldYoshi`.
    OldYoshi = 29,
    /// `St_Kind_OldKongo`.
    OldKongo = 30,
    /// `St_Kind_Battle`.
    Battle = 31,
    /// `St_Kind_Last`.
    Last = 32,
    /// `St_Kind_BigBlueRoute`.
    BigBlueRoute = 73,
    /// `St_Kind_Unk84`: the Home-Run Contest's.
    HomeRun = 84,
    /// `St_Kind_Heal`.
    Heal = 85,
    /// Event 50's (Final Destination, with both hands).
    Event50 = 251,
    /// `St_Kind_Unk328`: none: the game cache names no stage.
    Unk328 = 328,
}

impl StKind {
    /// Its number, as `StartMeleeRules` and the replays hold it.
    pub const fn id(self) -> u16 {
        self as u16
    }

    /// The ground kind the game runs for it (`stage_id_map`, `Stage_8022519C`).
    pub fn ground(self) -> GrKind {
        GrKind::from_id(crate::lb::audio::ground_kind(i32::from(self.id())) as i32)
            .expect("stage_id_map gives a ground kind")
    }
}

impl PartialEq<StKind> for i32 {
    fn eq(&self, k: &StKind) -> bool {
        *self == i32::from(k.id())
    }
}

impl PartialEq<StKind> for u16 {
    fn eq(&self, k: &StKind) -> bool {
        *self == k.id()
    }
}

/// A stage's hook without arguments (`void (*)(void)`).
pub type Hook = fn(&mut Running<'_>);

/// One stage, by its ground kind.
#[derive(Clone, Copy)]
pub struct Stage {
    pub kind: GrKind,
    /// Its name (the roster's, which the player shows, for the VS stages).
    pub name: &'static str,
    /// The executable's `StageData`: its archive, its objects' callbacks by map id, its hooks'
    /// addresses and its collision joints.
    pub data: &'static StageData,
    /// The ports of `data`'s hooks: `on_init` (the stage's setup), `on_load`, `on_start`.
    pub setup: Hook,
    pub load: Hook,
    pub start: Hook,
    /// The port of `data.on_demo_init`, which takes the demo's number; none where it isn't
    /// ported.
    pub demo: Option<fn(&mut Running<'_>, i32)>,
    /// Its module's `#[callbacks(Proc)]` dispatch: its objects' setups, processes and destroy
    /// callbacks (`data.callbacks`) and the processes they start.
    pub procs: Option<fn(&mut Entries<kind::Proc>)>,
    /// Its module's `#[callbacks(Render)]` dispatch, for the stages that draw their own.
    pub renders: Option<fn(&mut Entries<kind::Render>)>,
    /// The port of `data.on_check_shadow_render`: whether a joint of the stage takes a
    /// fighter's shadow, the fighter at a position (`gr::display`); none for the dummy stage.
    pub shadow: Option<fn(&mut Running<'_>, Vec3, JObjId) -> bool>,
    /// The port of `data.on_touch_line`: the hit a collision line gives a fighter touching it
    /// (its `DynamicsDesc`), if any (`ft::attack`'s line hazards); none for the dummy stage.
    pub touch_line: Option<fn(&mut Running<'_>, i32) -> Option<DataRef>>,
    /// Which member of `GroundVars` an object of the stage uses, by its map id.
    pub vars: fn(i32) -> VarsKind,
    /// The stage kind a VS match on it starts with, for a stage the roster lists: the player's
    /// command line and launcher offer it once its audit finds nothing missing.
    pub vs: Option<StKind>,
    /// The names the command line takes, the first the usual one (a roster stage's).
    pub keys: &'static [&'static str],
    /// What a match on the stage can reach that no port covers, once someone has looked: none
    /// until then, and the stage isn't offered.
    pub audit: Option<Audit>,
}

impl std::fmt::Debug for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stage")
            .field("kind", &self.kind)
            .field("name", &self.name)
            .field("vs", &self.vs)
            .field("keys", &self.keys)
            .field("audit", &self.audit)
            .finish_non_exhaustive()
    }
}

impl PartialEq for Stage {
    fn eq(&self, o: &Self) -> bool {
        self.kind == o.kind
    }
}

impl Stage {
    /// The stage kind a VS match on it starts with, as `Settings::vs` takes it: a roster
    /// stage's.
    pub fn vs_kind(&self) -> u16 {
        self.vs
            .unwrap_or_else(|| panic!("{} isn't a roster stage", self.name))
            .id()
    }

    /// Its archive's name as the game names it (`lbFileGetFullName`): the five stages whose
    /// table gives no extension (Pokémon Stadium's, Onett's, Corneria's, Venom's and the
    /// Home-Run Contest's) take the saved language's, `.dat` with Japanese saved.
    pub fn file(&self, saved_us: bool) -> Option<&'static str> {
        let f = self.data.file?;
        Some(if saved_us {
            f
        } else {
            match f {
                "/GrPs.usd" => "/GrPs.dat",
                "/GrOt.usd" => "/GrOt.dat",
                "/GrCn.usd" => "/GrCn.dat",
                "/GrVe.usd" => "/GrVe.dat",
                "/GrHr.usd" => "/GrHr.dat",
                f => f,
            }
        })
    }
}

/// The ported stage numbered `grkind` (`stage_datas[grkind]`); stops if it isn't ported.
pub fn stage(grkind: i32) -> &'static Stage {
    match GrKind::from_id(grkind).and_then(|k| STAGES[k as usize].as_ref()) {
        Some(s) => s,
        None => unimplemented!("stage kind {grkind} isn't ported"),
    }
}

/// The ported stages, by ground kind.
pub fn ported() -> impl Iterator<Item = &'static Stage> {
    STAGES.iter().flatten()
}

/// Which member of `GroundVars` an object of the stage `grkind` uses (bytes for a stage that
/// isn't ported, as a capture of one reads it).
pub fn vars_kind(grkind: i32, map_id: i32) -> VarsKind {
    match GrKind::from_id(grkind).and_then(|k| STAGES[k as usize].as_ref()) {
        Some(s) => (s.vars)(map_id),
        None => VarsKind::Bytes,
    }
}

/// The stage whose data names the address `f` in `field`: the loaded stage (`grkind`)
/// when its data does, else the first that does (the state holds the original's address,
/// which only the loaded stage's setup writes).
pub fn by_address(grkind: i32, f: u32, field: fn(&StageData) -> u32) -> Option<&'static Stage> {
    match GrKind::from_id(grkind).and_then(|k| STAGES[k as usize].as_ref()) {
        Some(s) if field(s.data) == f => Some(s),
        _ => ported().find(|s| field(s.data) == f),
    }
}

/// Runs a stage's demo setup by its address (`StageData::on_demo_init`), with the demo's
/// number; `None` if no stage's data names the address.
pub fn run_demo(r: &mut Running, f: u32, arg: i32) -> Option<()> {
    let s = ported().find(|s| s.data.on_demo_init == f)?;
    (s.demo?)(r, arg);
    Some(())
}

/// A stage hook by its address in the callback table: the hook of the first stage whose data
/// names the address.
fn run_hook(r: &mut Running, f: u32) {
    for s in ported() {
        let d = s.data;
        let hook = if d.on_init == f {
            s.setup
        } else if d.on_load == f {
            s.load
        } else if d.on_start == f {
            s.start
        } else {
            continue;
        };
        return hook(r);
    }
    unreachable!("{f:#010x} is in the callback table as a stage hook")
}

/// The stages' entries in the callback table: each stage's hooks by their addresses in its
/// data, and its module's objects' callbacks and renders.
pub fn register(
    events: &mut Entries<kind::Event>,
    procs: &mut Entries<kind::Proc>,
    renders: &mut Entries<kind::Render>,
) {
    let site = concat!(file!(), ":", line!());
    for s in ported() {
        for a in [s.data.on_init, s.data.on_load, s.data.on_start] {
            events.add(a, run_hook, site);
        }
        if let Some(p) = s.procs {
            p(procs);
        }
        if let Some(r) = s.renders {
            r(renders);
        }
    }
}

/// The exemption the audits of stages with hurtbox items share.
const YAKU_UNANCHORED: (&str, &str) = (
    "it_2E6A_UnkMotion19_Phys",
    "a stage item on neither a joint nor a point, which no stage makes (the original hangs)",
);

/// A stage with no variables of its own in its objects.
fn bytes(_: i32) -> VarsKind {
    VarsKind::Bytes
}

/// Does nothing (`Ground_OnStart`, the dummy stage's hooks).
fn nothing(_: &mut Running) {}

/// One of Target Test's stages: the template's hooks and callbacks, with its index.
macro_rules! target {
    ($kind:ident, $data:expr, $name:expr, $i:expr) => {
        Some(Stage {
            kind: GrKind::$kind,
            name: $name,
            data: &$data,
            setup: |r| target_test::on_init(r, $i),
            load: target_test::on_load,
            start: target_test::on_start,
            demo: Some(|r, _| target_test::on_demo_init(r)),
            procs: Some(target_test::run),
            renders: None,
            shadow: Some(|_, _, _| target_test::check_shadow_render()),
            touch_line: Some(|r, line| target_test::line_hazard(r.game, r.data, line)),
            vars: bytes,
            vs: None,
            keys: &[],
            audit: None,
        })
    };
}

/// Every stage, by ground kind (`stage_datas`); none where the stage isn't ported.
pub static STAGES: [Option<Stage>; GrKind::COUNT] = [
    Some(Stage {
        kind: GrKind::Dummy,
        name: "The dummy stage",
        data: &data::DUMMY,
        setup: nothing,
        load: nothing,
        start: nothing,
        // `Ground_801BFFAC`: not ported.
        demo: None,
        procs: None,
        renders: None,
        shadow: None,
        touch_line: None,
        vars: bytes,
        vs: None,
        keys: &[],
        audit: None,
    }),
    Some(Stage {
        kind: GrKind::Test,
        name: "The test stage",
        data: &data::TEST,
        setup: test_stage::on_init,
        load: test_stage::on_load,
        start: test_stage::on_start,
        demo: Some(|r, _| test_stage::on_demo_init(r)),
        procs: Some(test_stage::run),
        renders: None,
        shadow: Some(|_, _, _| test_stage::check_shadow_render()),
        touch_line: Some(|_, line| test_stage::on_touch_line(line)),
        vars: bytes,
        vs: None,
        keys: &[],
        audit: None,
    }),
    Some(Stage {
        kind: GrKind::Castle,
        name: "Princess Peach's Castle",
        data: &data::CASTLE,
        setup: peachs_castle::on_init,
        load: peachs_castle::on_load,
        start: peachs_castle::on_start,
        demo: Some(|r, _| peachs_castle::on_demo_init(r)),
        procs: Some(peachs_castle::run),
        renders: Some(peachs_castle::run_render),
        shadow: Some(|r, pos, j| peachs_castle::check_shadow_render(r.game, r.data, pos, Some(j))),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            1 => VarsKind::CastleBlast,
            2 => VarsKind::CastleBlaster,
            3 => VarsKind::Castle,
            5 | 7 | 17 => VarsKind::CastleBlocks,
            6 => VarsKind::CastleLifts,
            8..=16 => VarsKind::CastleBill,
            18..=20 => VarsKind::CastleSwitch,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::Castle),
        keys: &["pc", "peachs-castle"],
        audit: Some(Audit {
            units: &["melee/gr/grcastle", "melee/it/kinds/ityaku"],
            modules: &["gr/peachs_castle", "it/kinds/yaku"],
            unreached: &[
                YAKU_UNANCHORED,
                (
                    "lb_800117F4",
                    "the flags' chains drawn as lines, which only the debug camera's flag on a stage object turns on (dbcamera.c)",
                ),
            ],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::RCruise,
        name: "Rainbow Cruise",
        data: &data::RCRUISE,
        setup: rainbow_cruise::on_init,
        load: rainbow_cruise::on_load,
        start: rainbow_cruise::on_start,
        demo: Some(|r, _| rainbow_cruise::on_demo_init(r)),
        procs: Some(rainbow_cruise::run),
        renders: Some(rainbow_cruise::run_render),
        shadow: Some(|_, _, _| rainbow_cruise::check_shadow_render()),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            1 => VarsKind::RCruise,
            3 => VarsKind::Scroll,
            4 => VarsKind::RCruiseBlocks,
            6 => VarsKind::RCruiseFlag,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::RCruise),
        keys: &["rc", "rainbow-cruise"],
        audit: Some(Audit {
            units: &["melee/gr/grrcruise"],
            modules: &["gr/rainbow_cruise"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::Kongo,
        name: "Kongo Jungle",
        data: &data::KONGO,
        setup: kongo_jungle::on_init,
        load: kongo_jungle::on_load,
        start: kongo_jungle::on_start,
        demo: Some(|r, _| kongo_jungle::on_demo_init(r)),
        procs: Some(kongo_jungle::run),
        renders: None,
        shadow: Some(|r, pos, j| kongo_jungle::check_shadow_render(r.game, r.data, pos, Some(j))),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            10 => VarsKind::Kongo,
            5 => VarsKind::KongoBarrel,
            1 | 2 => VarsKind::KongoPlatform,
            7..=9 => VarsKind::KongoLog,
            11 => VarsKind::KongoPath,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::Kongo),
        keys: &["kj", "kongo-jungle"],
        audit: Some(Audit {
            units: &[
                "melee/gr/grkongo",
                "melee/it/kinds/itklap",
                "melee/ft/kinds/ftCommon/ftCo_BarrelWait",
            ],
            modules: &["gr/kongo_jungle", "it/kinds/klap_trap"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::Garden,
        name: "Jungle Japes",
        data: &data::GARDEN,
        setup: jungle_japes::on_init,
        load: jungle_japes::on_load,
        start: jungle_japes::on_start,
        demo: Some(|r, _| jungle_japes::on_demo_init(r)),
        procs: Some(jungle_japes::run),
        renders: None,
        shadow: Some(|r, pos, j| jungle_japes::check_shadow_render(r.game, r.data, pos, Some(j))),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            1 => VarsKind::Garden,
            3 => VarsKind::Garden2,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::Garden),
        keys: &["jj", "jungle-japes"],
        audit: Some(Audit {
            units: &["melee/gr/grgarden", "melee/it/kinds/ityaku"],
            modules: &["gr/jungle_japes", "it/kinds/yaku"],
            unreached: &[YAKU_UNANCHORED],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::GreatBay,
        name: "Great Bay",
        data: &data::GREATBAY,
        setup: great_bay::on_init,
        load: great_bay::on_load,
        start: great_bay::on_start,
        demo: Some(|r, _| great_bay::on_demo_init(r)),
        procs: Some(great_bay::run),
        renders: None,
        shadow: Some(|r, pos, j| great_bay::check_shadow_render(r.game, r.data, pos, Some(j))),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            1 => VarsKind::GreatBay,
            2 => VarsKind::GreatBay3,
            9 => VarsKind::GreatBay2,
            10 => VarsKind::GreatBay4,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::GreatBay),
        keys: &["gb", "great-bay"],
        audit: Some(Audit {
            units: &["melee/gr/grgreatbay", "melee/it/kinds/ittincle"],
            modules: &["gr/great_bay", "it/kinds/tingle"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::Shrine,
        name: "Hyrule Temple",
        data: &data::SHRINE,
        setup: hyrule_temple::on_init,
        load: hyrule_temple::on_load,
        start: hyrule_temple::on_start,
        demo: Some(|r, _| hyrule_temple::on_demo_init(r)),
        procs: Some(hyrule_temple::run),
        renders: None,
        shadow: Some(|r, pos, j| hyrule_temple::check_shadow_render(r.game, r.data, pos, Some(j))),
        touch_line: Some(|_, _| None),
        vars: bytes,
        vs: Some(StKind::Shrine),
        keys: &["ht", "hyrule-temple"],
        audit: Some(Audit {
            units: &["melee/gr/grshrine"],
            modules: &["gr/hyrule_temple"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::Zebes,
        name: "Brinstar",
        data: &data::ZEBES,
        setup: brinstar::on_init,
        load: brinstar::on_load,
        start: brinstar::on_start,
        demo: Some(|r, _| brinstar::on_demo_init(r)),
        procs: Some(brinstar::run),
        renders: None,
        shadow: Some(|_, _, _| brinstar::check_shadow_render()),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            1 => VarsKind::ZebesBack,
            3 => VarsKind::ZebesBubble,
            4 => VarsKind::ZebesCreature,
            6 => VarsKind::ZebesStage,
            7 => VarsKind::ZebesIsland,
            8 => VarsKind::ZebesLava,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::Zebes),
        keys: &["br", "brinstar"],
        audit: Some(Audit {
            units: &["melee/gr/grzebes", "melee/it/kinds/ityaku"],
            modules: &["gr/brinstar", "it/kinds/yaku"],
            unreached: &[YAKU_UNANCHORED],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::Kraid,
        name: "Brinstar Depths",
        data: &data::KRAID,
        setup: brinstar_depths::on_init,
        load: brinstar_depths::on_load,
        start: brinstar_depths::on_start,
        demo: Some(|r, _| brinstar_depths::on_demo_init(r)),
        procs: Some(brinstar_depths::run),
        renders: None,
        shadow: Some(|_, _, _| brinstar_depths::check_shadow_render()),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            1 | 3 => VarsKind::Kraid,
            4 => VarsKind::Kraid2,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::Kraid),
        keys: &["bd", "brinstar-depths"],
        audit: Some(Audit {
            units: &["melee/gr/grkraid"],
            modules: &["gr/brinstar_depths"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::Story,
        name: "Yoshi's Story",
        data: &data::STORY,
        setup: yoshis_story::on_init,
        load: yoshis_story::on_load,
        start: yoshis_story::on_start,
        demo: Some(|r, _| yoshis_story::on_demo_init(r)),
        procs: Some(yoshis_story::run),
        renders: None,
        shadow: Some(|r, pos, j| yoshis_story::check_shadow_render(r.game, r.data, pos, Some(j))),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            2 => VarsKind::StoryRandall,
            3 => VarsKind::StoryShyGuys,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::Story),
        keys: &["ys", "yoshis-story"],
        audit: Some(Audit {
            units: &["melee/gr/grstory", "melee/it/kinds/itheiho"],
            modules: &["gr/yoshis_story", "it/kinds/shy_guy"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::Yorster,
        name: "Yoshi's Island",
        data: &data::YORSTER,
        setup: yoshis_island::on_init,
        load: yoshis_island::on_load,
        start: yoshis_island::on_start,
        demo: Some(|r, _| yoshis_island::on_demo_init(r)),
        procs: Some(yoshis_island::run),
        renders: None,
        shadow: Some(|_, _, _| yoshis_island::check_shadow_render()),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            1 => VarsKind::Yorster,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::Yorster),
        keys: &["yi", "yoshis-island"],
        audit: Some(Audit {
            units: &["melee/gr/gryorster", "melee/it/kinds/ityaku"],
            modules: &["gr/yoshis_island", "it/kinds/yaku"],
            unreached: &[YAKU_UNANCHORED],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::Izumi,
        name: "Fountain of Dreams",
        data: &data::IZUMI,
        setup: fountain_of_dreams::on_init,
        load: fountain_of_dreams::on_load,
        start: fountain_of_dreams::on_start,
        demo: Some(|r, _| fountain_of_dreams::on_demo_init(r)),
        procs: Some(fountain_of_dreams::run),
        renders: Some(fountain_of_dreams::run_render),
        shadow: Some(|r, pos, j| {
            fountain_of_dreams::check_shadow_render(r.game, r.data, pos, Some(j))
        }),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            2 => VarsKind::Izumi2,
            3 => VarsKind::Izumi,
            4 => VarsKind::Izumi3,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::Izumi),
        keys: &["fod", "fountain-of-dreams"],
        audit: Some(Audit {
            units: &["melee/gr/grizumi"],
            modules: &["gr/fountain_of_dreams"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::Greens,
        name: "Green Greens",
        data: &data::GREENS,
        setup: green_greens::on_init,
        load: green_greens::on_load,
        start: green_greens::on_start,
        demo: Some(|r, _| green_greens::on_demo_init(r)),
        procs: Some(green_greens::run),
        renders: None,
        shadow: Some(|_, _, _| green_greens::check_shadow_render()),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            5 => VarsKind::Greens2,
            6 => VarsKind::Greens,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::Greens),
        keys: &["gg", "green-greens"],
        audit: Some(Audit {
            units: &[
                "melee/gr/grgreens",
                "melee/it/kinds/itwhispyapple",
                "melee/it/kinds/ityaku",
            ],
            modules: &["gr/green_greens", "it/kinds/whispyapple", "it/kinds/yaku"],
            unreached: &[YAKU_UNANCHORED],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::Corneria,
        name: "Corneria",
        data: &data::CORNERIA,
        setup: corneria::on_init,
        load: corneria::on_load,
        start: corneria::on_start,
        demo: Some(|r, _| corneria::on_demo_init(r)),
        procs: Some(corneria::run),
        renders: Some(corneria::run_render),
        shadow: Some(|r, pos, j| corneria::check_shadow_render(r.game, r.data, pos, Some(j))),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            3 => VarsKind::Corneria,
            4 | 9 => VarsKind::CorneriaCity,
            8 => VarsKind::CorneriaCity8,
            1 => VarsKind::ArwingPath,
            2 | 10 => VarsKind::Arwing,
            13..=18 => VarsKind::ArwingColl,
            12 => VarsKind::StarFoxTalk,
            5 => VarsKind::SmashTaunt,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::Corneria),
        keys: &["cn", "corneria"],
        audit: Some(Audit {
            units: &[
                "melee/gr/grcorneria",
                "melee/it/kinds/itarwinglaser",
                "melee/it/kinds/itgreatfoxlaser",
                "melee/it/kinds/ityaku",
            ],
            modules: &[
                "gr/corneria",
                "it/kinds/arwinglaser",
                "it/kinds/greatfoxlaser",
                "it/kinds/yaku",
            ],
            unreached: &[YAKU_UNANCHORED],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::Venom,
        name: "Venom",
        data: &data::VENOM,
        setup: venom::on_init,
        load: venom::on_load,
        start: venom::on_start,
        demo: Some(|r, _| venom::on_demo_init(r)),
        procs: Some(venom::run),
        renders: None,
        shadow: Some(|r, pos, j| venom::check_shadow_render(r.game, r.data, pos, Some(j))),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            1 => VarsKind::SmashTaunt,
            2 => VarsKind::ArwingPath,
            3 | 6 => VarsKind::Arwing,
            5 => VarsKind::VenomPlatform,
            7 => VarsKind::VenomShip,
            8 => VarsKind::StarFoxTalk,
            10..=15 => VarsKind::ArwingColl,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::Venom),
        keys: &["ve", "venom"],
        audit: Some(Audit {
            units: &["melee/gr/grvenom", "melee/it/kinds/itarwinglaser"],
            modules: &["gr/venom", "it/kinds/arwinglaser"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::PStadium,
        name: "Pokémon Stadium",
        data: &data::STADIUM,
        setup: pokemon_stadium::on_init,
        load: pokemon_stadium::on_load,
        start: pokemon_stadium::on_start,
        demo: Some(|r, _| pokemon_stadium::on_demo_init(r)),
        procs: Some(pokemon_stadium::run),
        renders: Some(pokemon_stadium::run_render),
        shadow: Some(|r, pos, j| {
            pokemon_stadium::check_shadow_render(r.game, r.data, pos, Some(j))
        }),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            1 => VarsKind::StadiumDisplay,
            2 => VarsKind::Stadium,
            3..=5 => VarsKind::StadiumBits,
            6 | 9 => VarsKind::StadiumPiece,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::PStadium),
        keys: &["ps", "pokemon-stadium"],
        audit: Some(Audit {
            units: &["melee/gr/grpstadium"],
            modules: &["gr/pokemon_stadium"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::Pura,
        name: "Poké Floats",
        data: &data::PURA,
        setup: poke_floats::on_init,
        load: poke_floats::on_load,
        start: poke_floats::on_start,
        demo: Some(|r, _| poke_floats::on_demo_init(r)),
        procs: Some(poke_floats::run),
        renders: Some(poke_floats::run_render),
        shadow: Some(|_, _, _| poke_floats::check_shadow_render()),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            1 => VarsKind::Pura,
            2 | 3 | 5..=26 => VarsKind::PuraFloat,
            4 => VarsKind::PuraStage,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::Pura),
        keys: &["pf", "poke-floats"],
        audit: Some(Audit {
            units: &["melee/gr/grpura"],
            modules: &["gr/poke_floats"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::MuteCity,
        name: "Mute City",
        data: &data::MUTECITY,
        setup: mute_city::on_init,
        load: mute_city::on_load,
        start: mute_city::on_start,
        demo: Some(|r, _| mute_city::on_demo_init(r)),
        procs: Some(mute_city::run),
        renders: None,
        shadow: Some(|r, pos, j| mute_city::check_shadow_render(r.game, pos, Some(j))),
        touch_line: Some(|r, line| mute_city::line_hazard(r.game, r.data, line)),
        vars: |map_id| match map_id {
            30 => VarsKind::MuteCity,
            29 => VarsKind::MuteCity2,
            2 => VarsKind::MuteCityCars,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::MuteCity),
        keys: &["mc", "mute-city"],
        audit: Some(Audit {
            units: &[
                "melee/gr/grmutecity",
                "melee/gr/grfzerocar",
                "melee/it/kinds/ityaku",
            ],
            modules: &["gr/mute_city", "gr/fzerocar", "it/kinds/yaku"],
            unreached: &[YAKU_UNANCHORED],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::BigBlue,
        name: "Big Blue",
        data: &data::BIGBLUE,
        setup: big_blue::on_init,
        load: big_blue::on_load,
        start: big_blue::on_start,
        demo: Some(|r, _| big_blue::on_demo_init(r)),
        procs: Some(big_blue::run),
        renders: Some(big_blue::run_render),
        shadow: Some(|r, pos, j| big_blue::check_shadow_render(r.game, r.data, pos, Some(j))),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            32 => VarsKind::BigBlueManager,
            33 => VarsKind::BigBlueCars,
            34 => VarsKind::BigBlueRoad,
            35 => VarsKind::BigBlueFlyer,
            36 => VarsKind::BigBlueRelay,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::BigBlue),
        keys: &["bb", "big-blue"],
        audit: Some(Audit {
            units: &["melee/gr/grbigblue", "melee/gr/grfzerocar"],
            modules: &["gr/big_blue", "gr/fzerocar"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::Onett,
        name: "Onett",
        data: &data::ONETT,
        setup: onett::on_init,
        load: onett::on_load,
        start: onett::on_start,
        demo: Some(|r, _| onett::on_demo_init(r)),
        procs: Some(onett::run),
        renders: None,
        shadow: Some(|r, pos, j| onett::check_shadow_render(r.game, r.data, pos, Some(j))),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            3 => VarsKind::OnettCar,
            4 => VarsKind::OnettBuilding,
            5 => VarsKind::Onett,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::Onett),
        keys: &["on", "onett"],
        audit: Some(Audit {
            units: &["melee/gr/gronett", "melee/it/kinds/ityaku"],
            modules: &["gr/onett", "it/kinds/yaku"],
            unreached: &[YAKU_UNANCHORED],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::Fourside,
        name: "Fourside",
        data: &data::FOURSIDE,
        setup: fourside::on_init,
        load: fourside::on_load,
        start: fourside::on_start,
        demo: Some(|r, _| fourside::on_demo_init(r)),
        procs: Some(fourside::run),
        renders: Some(fourside::run_render),
        shadow: Some(|r, pos, j| fourside::check_shadow_render(r.game, r.data, pos, Some(j))),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            1 => VarsKind::FoursideCrane,
            3 => VarsKind::Fourside2,
            5 => VarsKind::FoursideUfo,
            6 => VarsKind::Fourside,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::Fourside),
        keys: &["fs", "fourside"],
        audit: Some(Audit {
            units: &["melee/gr/grfourside"],
            modules: &["gr/fourside"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::Icemt,
        name: "Icicle Mountain",
        data: &data::ICEMT,
        setup: icicle_mountain::on_init,
        load: icicle_mountain::on_load,
        start: icicle_mountain::on_start,
        demo: Some(|r, _| icicle_mountain::on_demo_init(r)),
        procs: Some(icicle_mountain::run),
        renders: None,
        shadow: Some(|_, _, _| icicle_mountain::check_shadow_render()),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            1..=6 => VarsKind::IceMtField(map_id as u8),
            8 => VarsKind::IceMtBg,
            9 => VarsKind::IceMtScroll,
            10 => VarsKind::IceMtClimb,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::Icemt),
        keys: &["im", "icicle-mountain"],
        audit: Some(Audit {
            units: &["melee/gr/gricemt", "melee/it/kinds/ityaku"],
            modules: &["gr/icicle_mountain", "it/kinds/yaku"],
            unreached: &[YAKU_UNANCHORED],
            missing: &[],
        }),
    }),
    // `Gr_Kind_Unk23`: not ported.
    None,
    Some(Stage {
        kind: GrKind::Inishie1,
        name: "Mushroom Kingdom",
        data: &data::INISHIE1,
        setup: mushroom_kingdom::on_init,
        load: mushroom_kingdom::on_load,
        start: mushroom_kingdom::on_start,
        demo: Some(|r, _| mushroom_kingdom::on_demo_init(r)),
        procs: Some(mushroom_kingdom::run),
        renders: None,
        shadow: Some(|_, _, _| mushroom_kingdom::check_shadow_render()),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            2 => VarsKind::Inishie1Hatena,
            3 => VarsKind::Inishie1,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::Inishie1),
        keys: &["mk", "mushroom-kingdom"],
        audit: Some(Audit {
            units: &["melee/gr/grinishie1", "melee/it/kinds/ityaku"],
            modules: &["gr/mushroom_kingdom", "it/kinds/yaku"],
            unreached: &[
                YAKU_UNANCHORED,
                (
                    "grinishie1.c:496",
                    "a block's ceiling callback on a joint not among the blocks': only the blocks' collision joints get it (the original hangs)",
                ),
                (
                    "grinishie1.c:541",
                    "a block whose joint the stage's model lacks: the stage's archive has every one (the original hangs)",
                ),
            ],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::Inishie2,
        name: "Mushroom Kingdom II",
        data: &data::INISHIE2,
        setup: mushroom_kingdom_2::on_init,
        load: mushroom_kingdom_2::on_load,
        start: mushroom_kingdom_2::on_start,
        demo: Some(|r, _| mushroom_kingdom_2::on_demo_init(r)),
        procs: Some(mushroom_kingdom_2::run),
        renders: None,
        shadow: Some(|_, _, _| mushroom_kingdom_2::check_shadow_render()),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            1 => VarsKind::Inishie2Pidgit,
            2 => VarsKind::Inishie2,
            3 | 4 => VarsKind::Inishie2Log,
            15 => VarsKind::Inishie2Birdo,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::Inishie2),
        keys: &["mk2", "mushroom-kingdom-2"],
        audit: Some(Audit {
            units: &[
                "melee/gr/grinishie2",
                "melee/it/kinds/itkyasarin",
                "melee/it/kinds/itkyasarinegg",
            ],
            modules: &[
                "gr/mushroom_kingdom_2",
                "it/kinds/birdo",
                "it/kinds/birdo_egg",
            ],
            unreached: &[],
            missing: &[],
        }),
    }),
    // `Gr_Kind_Unk26`: not ported.
    None,
    Some(Stage {
        kind: GrKind::Flatzone,
        name: "Flat Zone",
        data: &data::FLATZONE,
        setup: flat_zone::on_init,
        load: flat_zone::on_load,
        start: flat_zone::on_start,
        demo: Some(|r, _| flat_zone::on_demo_init(r)),
        procs: Some(flat_zone::run),
        renders: None,
        shadow: Some(|_, _, _| flat_zone::check_shadow_render()),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            2 => VarsKind::Flatzone,
            3 => VarsKind::FlatzoneOil,
            4 | 6 => VarsKind::FlatzoneEvent,
            5 => VarsKind::FlatzonePlatforms,
            8 => VarsKind::FlatzoneTools,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::Flatzone),
        keys: &["fz", "flat-zone"],
        audit: Some(Audit {
            units: &["melee/gr/grflatzone", "melee/it/kinds/ittools"],
            modules: &["gr/flat_zone", "it/kinds/tools"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::OldPupupu,
        name: "Dream Land",
        data: &data::OLDPUPUPU,
        setup: dream_land::on_init,
        load: dream_land::on_load,
        start: dream_land::on_start,
        demo: Some(|r, _| dream_land::on_demo_init(r)),
        procs: Some(dream_land::run),
        renders: None,
        shadow: Some(|r, pos, j| dream_land::check_shadow_render(r.game, r.data, pos, Some(j))),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            1 | 7 => VarsKind::OldPupupu,
            8 => VarsKind::OldPupupu2,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::OldPupupu),
        keys: &["dl", "dream-land"],
        audit: Some(Audit {
            units: &["melee/gr/groldpupupu"],
            modules: &["gr/dream_land"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::OldYoshi,
        name: "Yoshi's Island N64",
        data: &data::OLDYOSHI,
        setup: yoshis_island_64::on_init,
        load: yoshis_island_64::on_load,
        start: yoshis_island_64::on_start,
        demo: Some(|r, _| yoshis_island_64::on_demo_init(r)),
        procs: Some(yoshis_island_64::run),
        renders: None,
        shadow: Some(|r, pos, j| {
            yoshis_island_64::check_shadow_render(r.game, r.data, pos, Some(j))
        }),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            2 => VarsKind::OldYoshiCloud,
            3 => VarsKind::OldYoshiGuest,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::OldYoshi),
        keys: &["yi64", "yoshis-island-64"],
        audit: Some(Audit {
            units: &["melee/gr/groldyoshi"],
            modules: &["gr/yoshis_island_64"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::OldKongo,
        name: "Kongo Jungle N64",
        data: &data::OLDKONGO,
        setup: kongo_jungle_64::on_init,
        load: kongo_jungle_64::on_load,
        start: kongo_jungle_64::on_start,
        demo: Some(|r, _| kongo_jungle_64::on_demo_init(r)),
        procs: Some(kongo_jungle_64::run),
        renders: None,
        shadow: Some(|r, pos, j| {
            kongo_jungle_64::check_shadow_render(r.game, r.data, pos, Some(j))
        }),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            1 => VarsKind::OldKongo,
            2 => VarsKind::OldKongoBirds,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::OldKongo),
        keys: &["kj64", "kongo-jungle-64"],
        audit: Some(Audit {
            units: &[
                "melee/gr/groldkongo",
                "melee/ft/kinds/ftCommon/ftCo_BarrelWait",
            ],
            modules: &["gr/kongo_jungle_64"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::KinokoRoute,
        name: "Mushroom Kingdom (Adventure)",
        data: &data::KINOKOROUTE,
        setup: mushroom_kingdom_route::on_init,
        load: mushroom_kingdom_route::on_load,
        start: mushroom_kingdom_route::on_start,
        demo: Some(|r, _| mushroom_kingdom_route::on_demo_init(r)),
        procs: Some(mushroom_kingdom_route::run),
        renders: None,
        shadow: Some(|r, pos, j| {
            mushroom_kingdom_route::check_shadow_render(r.game, r.data, pos, Some(j))
        }),
        touch_line: Some(|_, line| mushroom_kingdom_route::on_touch_line(line)),
        vars: |map_id| match map_id {
            0 => VarsKind::KinokoRoute,
            3 => VarsKind::KinokoRoute2,
            _ => VarsKind::Bytes,
        },
        vs: None,
        keys: &[],
        audit: None,
    }),
    Some(Stage {
        kind: GrKind::ShrineRoute,
        name: "Underground Maze",
        data: &data::SHRINEROUTE,
        setup: underground_maze::on_init,
        load: underground_maze::on_load,
        start: underground_maze::on_start,
        demo: Some(|r, _| underground_maze::on_demo_init(r)),
        procs: Some(underground_maze::run),
        renders: None,
        shadow: Some(|r, pos, j| {
            underground_maze::check_shadow_render(r.game, r.data, pos, Some(j))
        }),
        touch_line: Some(|_, line| underground_maze::on_touch_line(line)),
        vars: |map_id| match map_id {
            4 => VarsKind::ShrineRoute,
            5 => VarsKind::ShrineRoute3,
            // The maze's lights' variables have the race's lights' layout.
            6 => VarsKind::PushOn,
            _ => VarsKind::Bytes,
        },
        vs: None,
        keys: &[],
        audit: None,
    }),
    Some(Stage {
        kind: GrKind::ZebesRoute,
        name: "Escape from Brinstar",
        data: &data::ZEBESROUTE,
        setup: brinstar_escape::on_init,
        load: brinstar_escape::on_load,
        start: brinstar_escape::on_start,
        demo: Some(|r, _| brinstar_escape::on_demo_init(r)),
        procs: Some(brinstar_escape::run),
        renders: None,
        shadow: Some(|_, _, _| brinstar_escape::check_shadow_render()),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            1 => VarsKind::ZebesBack,
            _ => VarsKind::Bytes,
        },
        vs: None,
        keys: &[],
        audit: None,
    }),
    Some(Stage {
        kind: GrKind::BigBlueRoute,
        name: "F-Zero Grand Prix",
        data: &data::BIGBLUEROUTE,
        setup: fzero_race::on_init,
        load: fzero_race::on_load,
        start: fzero_race::on_start,
        demo: Some(|r, _| fzero_race::on_demo_init(r)),
        procs: Some(fzero_race::run),
        renders: None,
        shadow: Some(|r, pos, j| fzero_race::check_shadow_render(r.game, r.data, pos, Some(j))),
        touch_line: Some(|_, line| fzero_race::on_touch_line(line)),
        vars: |map_id| match map_id {
            31 => VarsKind::BigBlueRoute,
            4 => VarsKind::BigBlueRouteCars,
            _ => VarsKind::Bytes,
        },
        vs: None,
        keys: &[],
        audit: None,
    }),
    Some(Stage {
        kind: GrKind::Unk35,
        name: "The test stage (its second ground kind)",
        data: &data::TEST,
        setup: test_stage::on_init,
        load: test_stage::on_load,
        start: test_stage::on_start,
        demo: Some(|r, _| test_stage::on_demo_init(r)),
        procs: Some(test_stage::run),
        renders: None,
        shadow: Some(|_, _, _| test_stage::check_shadow_render()),
        touch_line: Some(|_, line| test_stage::on_touch_line(line)),
        vars: bytes,
        vs: None,
        keys: &[],
        audit: None,
    }),
    Some(Stage {
        kind: GrKind::Battle,
        name: "Battlefield",
        data: &data::BATTLE,
        setup: battlefield::on_init,
        load: battlefield::on_load,
        start: battlefield::on_start,
        demo: Some(|r, _| battlefield::on_demo_init(r.game)),
        procs: Some(battlefield::run),
        renders: None,
        shadow: Some(|_, _, _| true),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            3 => VarsKind::BattleBg,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::Battle),
        keys: &["bf", "battlefield"],
        audit: Some(Audit {
            units: &["melee/gr/grbattle"],
            modules: &["gr/battlefield"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::Last,
        name: "Final Destination",
        data: &data::LAST,
        setup: final_destination::on_init,
        load: final_destination::on_load,
        start: final_destination::on_start,
        demo: Some(final_destination::on_demo_init),
        procs: Some(final_destination::run),
        renders: None,
        shadow: Some(|_, _, _| true),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            3 => VarsKind::Map,
            7 => VarsKind::Last,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::Last),
        keys: &["fd", "final-destination"],
        audit: Some(Audit {
            units: &["melee/gr/grlast"],
            modules: &["gr/final_destination"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::FigureGet,
        name: "Snag the Trophies",
        data: &data::FIGUREGET,
        setup: snag_the_trophies::on_init,
        load: snag_the_trophies::on_load,
        start: snag_the_trophies::on_start,
        demo: Some(|r, _| snag_the_trophies::on_demo_init(r)),
        procs: Some(snag_the_trophies::run),
        renders: None,
        shadow: Some(|_, _, _| snag_the_trophies::check_shadow_render()),
        touch_line: Some(|_, line| snag_the_trophies::on_touch_line(line)),
        vars: |map_id| match map_id {
            1 => VarsKind::FigureGet,
            _ => VarsKind::Bytes,
        },
        vs: None,
        keys: &[],
        audit: None,
    }),
    Some(Stage {
        kind: GrKind::Pushon,
        name: "Race to the Finish",
        data: &data::PUSHON,
        setup: race_to_the_finish::on_init,
        load: race_to_the_finish::on_load,
        start: race_to_the_finish::on_start,
        demo: Some(|r, _| race_to_the_finish::on_demo_init(r)),
        procs: Some(race_to_the_finish::run),
        renders: None,
        shadow: Some(|_, _, _| race_to_the_finish::check_shadow_render()),
        touch_line: Some(|r, line| race_to_the_finish::line_hazard(r.game, r.data, line)),
        vars: |map_id| match map_id {
            1 => VarsKind::Map,
            2 => VarsKind::PushOn,
            _ => VarsKind::Bytes,
        },
        vs: None,
        keys: &[],
        audit: None,
    }),
    target!(
        TMario,
        target_test::TMARIO_TARGET.data,
        "Target Test (Mario)",
        0
    ),
    target!(
        TCaptain,
        target_test::TCAPTAIN_TARGET.data,
        "Target Test (Captain Falcon)",
        1
    ),
    target!(
        TClink,
        target_test::TCLINK_TARGET.data,
        "Target Test (Young Link)",
        2
    ),
    target!(
        TDonkey,
        target_test::TDONKEY_TARGET.data,
        "Target Test (Donkey Kong)",
        3
    ),
    target!(
        TDrmario,
        target_test::TDRMARIO_TARGET.data,
        "Target Test (Dr. Mario)",
        4
    ),
    target!(
        TFalco,
        target_test::TFALCO_TARGET.data,
        "Target Test (Falco)",
        5
    ),
    target!(TFox, target_test::TFOX_TARGET.data, "Target Test (Fox)", 6),
    target!(
        TIceclimber,
        target_test::TICECLIMBER_TARGET.data,
        "Target Test (the Ice Climbers)",
        7
    ),
    target!(
        TKirby,
        target_test::TKIRBY_TARGET.data,
        "Target Test (Kirby)",
        8
    ),
    target!(
        TKoopa,
        target_test::TKOOPA_TARGET.data,
        "Target Test (Bowser)",
        9
    ),
    target!(
        TLink,
        target_test::TLINK_TARGET.data,
        "Target Test (Link)",
        10
    ),
    target!(
        TLuigi,
        target_test::TLUIGI_TARGET.data,
        "Target Test (Luigi)",
        11
    ),
    target!(
        TMars,
        target_test::TMARS_TARGET.data,
        "Target Test (Marth)",
        12
    ),
    target!(
        TMewtwo,
        target_test::TMEWTWO_TARGET.data,
        "Target Test (Mewtwo)",
        13
    ),
    target!(
        TNess,
        target_test::TNESS_TARGET.data,
        "Target Test (Ness)",
        14
    ),
    target!(
        TPeach,
        target_test::TPEACH_TARGET.data,
        "Target Test (Peach)",
        15
    ),
    target!(
        TPichu,
        target_test::TPICHU_TARGET.data,
        "Target Test (Pichu)",
        16
    ),
    target!(
        TPikachu,
        target_test::TPIKACHU_TARGET.data,
        "Target Test (Pikachu)",
        17
    ),
    target!(
        TPurin,
        target_test::TPURIN_TARGET.data,
        "Target Test (Jigglypuff)",
        18
    ),
    target!(
        TSamus,
        target_test::TSAMUS_TARGET.data,
        "Target Test (Samus)",
        19
    ),
    target!(
        TSeak,
        target_test::TSEAK_TARGET.data,
        "Target Test (Sheik)",
        20
    ),
    target!(
        TYoshi,
        target_test::TYOSHI_TARGET.data,
        "Target Test (Yoshi)",
        21
    ),
    target!(
        TZelda,
        target_test::TZELDA_TARGET.data,
        "Target Test (Zelda)",
        22
    ),
    target!(
        TGamewatch,
        target_test::TGAMEWATCH_TARGET.data,
        "Target Test (Mr. Game & Watch)",
        23
    ),
    target!(
        TEmblem,
        target_test::TEMBLEM_TARGET.data,
        "Target Test (Roy)",
        24
    ),
    target!(
        TGanon,
        target_test::TGANON_TARGET.data,
        "Target Test (Ganondorf)",
        25
    ),
    Some(Stage {
        kind: GrKind::Heal,
        name: "All-Star's rest area",
        data: &data::HEAL,
        setup: rest_area::on_init,
        load: nothing,
        start: rest_area::on_start,
        demo: Some(|_, _| rest_area::empty()),
        procs: Some(rest_area::run),
        renders: None,
        shadow: Some(|_, _, _| rest_area::check_shadow_render()),
        touch_line: Some(|_, line| rest_area::on_touch_line(line)),
        vars: |map_id| match map_id {
            1 => VarsKind::Map,
            _ => VarsKind::Bytes,
        },
        vs: None,
        keys: &[],
        audit: None,
    }),
    Some(Stage {
        kind: GrKind::Homerun,
        name: "Home-Run Contest",
        data: &data::HOMERUN,
        setup: home_run_contest::on_init,
        load: home_run_contest::on_load,
        start: home_run_contest::on_start,
        demo: Some(|r, _| home_run_contest::on_demo_init(r)),
        procs: Some(home_run_contest::run),
        renders: Some(home_run_contest::run_render),
        shadow: Some(|r, pos, j| {
            home_run_contest::check_shadow_render(r.game, r.data, pos, Some(j))
        }),
        touch_line: Some(|_, line| home_run_contest::on_touch_line(line)),
        vars: |map_id| match map_id {
            10 => VarsKind::HomeRun,
            1 | 4 | 6 | 8 => VarsKind::HomeRunPart,
            2 | 5 | 7 | 9 => VarsKind::HomeRunLabel,
            _ => VarsKind::Bytes,
        },
        vs: None,
        keys: &[],
        audit: None,
    }),
    Some(Stage {
        kind: GrKind::Figure1,
        name: "The events' first trophy stage",
        data: &data::FIGURE1,
        setup: |r| trophy_stages::on_init(r, 0),
        load: trophy_stages::on_load,
        start: trophy_stages::on_start,
        demo: Some(|r, _| trophy_stages::on_demo_init(r)),
        procs: Some(trophy_stages::run),
        renders: None,
        shadow: Some(|_, _, _| trophy_stages::check_shadow_render()),
        touch_line: Some(|_, line| trophy_stages::on_touch_line(line)),
        vars: bytes,
        vs: None,
        keys: &[],
        audit: None,
    }),
    Some(Stage {
        kind: GrKind::Figure2,
        name: "The events' second trophy stage",
        data: &data::FIGURE2,
        setup: |r| trophy_stages::on_init(r, 1),
        load: trophy_stages::on_load,
        start: trophy_stages::on_start,
        demo: Some(|r, _| trophy_stages::on_demo_init(r)),
        procs: Some(trophy_stages::run),
        renders: None,
        shadow: Some(|_, _, _| trophy_stages::check_shadow_render()),
        touch_line: Some(|_, line| trophy_stages::on_touch_line(line)),
        vars: bytes,
        vs: None,
        keys: &[],
        audit: None,
    }),
    Some(Stage {
        kind: GrKind::Figure3,
        name: "The events' third trophy stage",
        data: &data::FIGURE3,
        setup: |r| trophy_stages::on_init(r, 2),
        load: trophy_stages::on_load,
        start: trophy_stages::on_start,
        demo: Some(|r, _| trophy_stages::on_demo_init(r)),
        procs: Some(trophy_stages::run),
        renders: None,
        shadow: Some(|_, _, _| trophy_stages::check_shadow_render()),
        touch_line: Some(|_, line| trophy_stages::on_touch_line(line)),
        vars: bytes,
        vs: None,
        keys: &[],
        audit: None,
    }),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Each entry sits at its ground kind's index, and each ground kind's number is its index.
    #[test]
    fn the_registry_is_indexed_by_ground_kind() {
        for (i, k) in GrKind::ALL.iter().enumerate() {
            assert_eq!(k.id(), i as i32);
            assert_eq!(GrKind::from_id(i as i32), Some(*k));
        }
        assert_eq!(GrKind::from_id(-1), None);
        assert_eq!(GrKind::from_id(GrKind::COUNT as i32), None);
        for (i, s) in STAGES.iter().enumerate() {
            if let Some(s) = s {
                assert_eq!(s.kind as usize, i, "{}", s.name);
                assert!(
                    s.data.grkind == s.kind
                        || (s.kind == GrKind::Unk35 && s.data.grkind == GrKind::Test),
                    "{}: its data is {:?}'s",
                    s.name,
                    s.data.grkind
                );
            }
        }
        assert_eq!(GrKind::targets().count(), 26);
    }

    /// Each roster stage's stage kind runs its ground kind (`stage_id_map`), and no two roster
    /// lines share a stage kind.
    #[test]
    fn roster_stages_start_their_ground_kinds() {
        let mut seen = Vec::new();
        for s in ported() {
            if let Some(st) = s.vs {
                assert_eq!(st.ground(), s.kind, "{}", s.name);
                assert!(!seen.contains(&st), "{st:?} twice");
                seen.push(st);
            }
        }
        assert_eq!(seen.len(), 29);
    }

    /// Each stage but the dummy has its shadow check and line-touch ports, and each stage's
    /// addresses find their own entry.
    #[test]
    fn shadow_checks_and_line_hazards_are_wired() {
        for s in ported() {
            let dummy = s.kind == GrKind::Dummy;
            assert_eq!(s.shadow.is_none(), dummy, "{}", s.name);
            assert_eq!(s.touch_line.is_none(), dummy, "{}", s.name);
            if !dummy {
                let d = s.data;
                let by = |f, field| by_address(s.kind.id(), f, field).map(|t| t.data.grkind);
                assert_eq!(
                    by(d.on_check_shadow_render, |d| d.on_check_shadow_render),
                    Some(d.grkind)
                );
                assert_eq!(by(d.on_touch_line, |d| d.on_touch_line), Some(d.grkind));
            }
        }
    }

    /// Each stage's hook addresses are in the callback table (leaf-check calls them by address).
    #[test]
    fn hooks_are_found_by_their_addresses() {
        let t = super::super::super::code::table();
        for s in ported() {
            for a in [s.data.on_init, s.data.on_load, s.data.on_start] {
                assert!(t.event.get(a).is_some(), "{}: {a:#010x}", s.name);
            }
        }
    }
}
