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
    VarsKind, battle, bigblue, bigblueroute, castle, corneria, figure, figureget, flatzone,
    fourside, garden, greatbay, greens, heal, homerun, icemt, inishie1, inishie2, izumi,
    kinokoroute, kongo, kraid, last, mutecity, oldkongo, oldpupupu, oldyoshi, onett, pura, pushon,
    rcruise, shrine, shrineroute, stadium, story, target, teststage, venom, yorster, zebes,
    zebesroute,
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
        if i >= first && i < first + target::TARGETS.len() {
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
            setup: |r| target::on_init(r, $i),
            load: target::on_load,
            start: target::on_start,
            demo: Some(|r, _| target::on_demo_init(r)),
            procs: Some(target::run),
            renders: None,
            shadow: Some(|_, _, _| target::check_shadow_render()),
            touch_line: Some(|r, line| target::line_hazard(r.game, r.data, line)),
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
        setup: teststage::on_init,
        load: teststage::on_load,
        start: teststage::on_start,
        demo: Some(|r, _| teststage::on_demo_init(r)),
        procs: Some(teststage::run),
        renders: None,
        shadow: Some(|_, _, _| teststage::check_shadow_render()),
        touch_line: Some(|_, line| teststage::on_touch_line(line)),
        vars: bytes,
        vs: None,
        keys: &[],
        audit: None,
    }),
    Some(Stage {
        kind: GrKind::Castle,
        name: "Princess Peach's Castle",
        data: &data::CASTLE,
        setup: castle::on_init,
        load: castle::on_load,
        start: castle::on_start,
        demo: Some(|r, _| castle::on_demo_init(r)),
        procs: Some(castle::run),
        renders: Some(castle::run_render),
        shadow: Some(|r, pos, j| castle::check_shadow_render(r.game, r.data, pos, Some(j))),
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
            modules: &["gr/castle", "it/kinds/yaku"],
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
        setup: rcruise::on_init,
        load: rcruise::on_load,
        start: rcruise::on_start,
        demo: Some(|r, _| rcruise::on_demo_init(r)),
        procs: Some(rcruise::run),
        renders: Some(rcruise::run_render),
        shadow: Some(|_, _, _| rcruise::check_shadow_render()),
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
            modules: &["gr/rcruise"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::Kongo,
        name: "Kongo Jungle",
        data: &data::KONGO,
        setup: kongo::on_init,
        load: kongo::on_load,
        start: kongo::on_start,
        demo: Some(|r, _| kongo::on_demo_init(r)),
        procs: Some(kongo::run),
        renders: None,
        shadow: Some(|r, pos, j| kongo::check_shadow_render(r.game, r.data, pos, Some(j))),
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
            modules: &["gr/kongo", "it/kinds/klap"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::Garden,
        name: "Jungle Japes",
        data: &data::GARDEN,
        setup: garden::on_init,
        load: garden::on_load,
        start: garden::on_start,
        demo: Some(|r, _| garden::on_demo_init(r)),
        procs: Some(garden::run),
        renders: None,
        shadow: Some(|r, pos, j| garden::check_shadow_render(r.game, r.data, pos, Some(j))),
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
            modules: &["gr/garden", "it/kinds/yaku"],
            unreached: &[YAKU_UNANCHORED],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::GreatBay,
        name: "Great Bay",
        data: &data::GREATBAY,
        setup: greatbay::on_init,
        load: greatbay::on_load,
        start: greatbay::on_start,
        demo: Some(|r, _| greatbay::on_demo_init(r)),
        procs: Some(greatbay::run),
        renders: None,
        shadow: Some(|r, pos, j| greatbay::check_shadow_render(r.game, r.data, pos, Some(j))),
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
            modules: &["gr/greatbay", "it/kinds/tincle"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::Shrine,
        name: "Hyrule Temple",
        data: &data::SHRINE,
        setup: shrine::on_init,
        load: shrine::on_load,
        start: shrine::on_start,
        demo: Some(|r, _| shrine::on_demo_init(r)),
        procs: Some(shrine::run),
        renders: None,
        shadow: Some(|r, pos, j| shrine::check_shadow_render(r.game, r.data, pos, Some(j))),
        touch_line: Some(|_, _| None),
        vars: bytes,
        vs: Some(StKind::Shrine),
        keys: &["ht", "hyrule-temple"],
        audit: Some(Audit {
            units: &["melee/gr/grshrine"],
            modules: &["gr/shrine"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::Zebes,
        name: "Brinstar",
        data: &data::ZEBES,
        setup: zebes::on_init,
        load: zebes::on_load,
        start: zebes::on_start,
        demo: Some(|r, _| zebes::on_demo_init(r)),
        procs: Some(zebes::run),
        renders: None,
        shadow: Some(|_, _, _| zebes::check_shadow_render()),
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
            modules: &["gr/zebes", "it/kinds/yaku"],
            unreached: &[YAKU_UNANCHORED],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::Kraid,
        name: "Brinstar Depths",
        data: &data::KRAID,
        setup: kraid::on_init,
        load: kraid::on_load,
        start: kraid::on_start,
        demo: Some(|r, _| kraid::on_demo_init(r)),
        procs: Some(kraid::run),
        renders: None,
        shadow: Some(|_, _, _| kraid::check_shadow_render()),
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
            modules: &["gr/kraid"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::Story,
        name: "Yoshi's Story",
        data: &data::STORY,
        setup: story::on_init,
        load: story::on_load,
        start: story::on_start,
        demo: Some(|r, _| story::on_demo_init(r)),
        procs: Some(story::run),
        renders: None,
        shadow: Some(|r, pos, j| story::check_shadow_render(r.game, r.data, pos, Some(j))),
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
            modules: &["gr/story", "it/kinds/heiho"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::Yorster,
        name: "Yoshi's Island",
        data: &data::YORSTER,
        setup: yorster::on_init,
        load: yorster::on_load,
        start: yorster::on_start,
        demo: Some(|r, _| yorster::on_demo_init(r)),
        procs: Some(yorster::run),
        renders: None,
        shadow: Some(|_, _, _| yorster::check_shadow_render()),
        touch_line: Some(|_, _| None),
        vars: |map_id| match map_id {
            1 => VarsKind::Yorster,
            _ => VarsKind::Bytes,
        },
        vs: Some(StKind::Yorster),
        keys: &["yi", "yoshis-island"],
        audit: Some(Audit {
            units: &["melee/gr/gryorster", "melee/it/kinds/ityaku"],
            modules: &["gr/yorster", "it/kinds/yaku"],
            unreached: &[YAKU_UNANCHORED],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::Izumi,
        name: "Fountain of Dreams",
        data: &data::IZUMI,
        setup: izumi::on_init,
        load: izumi::on_load,
        start: izumi::on_start,
        demo: Some(|r, _| izumi::on_demo_init(r)),
        procs: Some(izumi::run),
        renders: Some(izumi::run_render),
        shadow: Some(|r, pos, j| izumi::check_shadow_render(r.game, r.data, pos, Some(j))),
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
            modules: &["gr/izumi"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::Greens,
        name: "Green Greens",
        data: &data::GREENS,
        setup: greens::on_init,
        load: greens::on_load,
        start: greens::on_start,
        demo: Some(|r, _| greens::on_demo_init(r)),
        procs: Some(greens::run),
        renders: None,
        shadow: Some(|_, _, _| greens::check_shadow_render()),
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
            modules: &["gr/greens", "it/kinds/whispyapple", "it/kinds/yaku"],
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
        setup: stadium::on_init,
        load: stadium::on_load,
        start: stadium::on_start,
        demo: Some(|r, _| stadium::on_demo_init(r)),
        procs: Some(stadium::run),
        renders: Some(stadium::run_render),
        shadow: Some(|r, pos, j| stadium::check_shadow_render(r.game, r.data, pos, Some(j))),
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
            modules: &["gr/stadium"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::Pura,
        name: "Poké Floats",
        data: &data::PURA,
        setup: pura::on_init,
        load: pura::on_load,
        start: pura::on_start,
        demo: Some(|r, _| pura::on_demo_init(r)),
        procs: Some(pura::run),
        renders: Some(pura::run_render),
        shadow: Some(|_, _, _| pura::check_shadow_render()),
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
            modules: &["gr/pura"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::MuteCity,
        name: "Mute City",
        data: &data::MUTECITY,
        setup: mutecity::on_init,
        load: mutecity::on_load,
        start: mutecity::on_start,
        demo: Some(|r, _| mutecity::on_demo_init(r)),
        procs: Some(mutecity::run),
        renders: None,
        shadow: Some(|r, pos, j| mutecity::check_shadow_render(r.game, pos, Some(j))),
        touch_line: Some(|r, line| mutecity::line_hazard(r.game, r.data, line)),
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
            modules: &["gr/mutecity", "gr/fzerocar", "it/kinds/yaku"],
            unreached: &[YAKU_UNANCHORED],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::BigBlue,
        name: "Big Blue",
        data: &data::BIGBLUE,
        setup: bigblue::on_init,
        load: bigblue::on_load,
        start: bigblue::on_start,
        demo: Some(|r, _| bigblue::on_demo_init(r)),
        procs: Some(bigblue::run),
        renders: Some(bigblue::run_render),
        shadow: Some(|r, pos, j| bigblue::check_shadow_render(r.game, r.data, pos, Some(j))),
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
            modules: &["gr/bigblue", "gr/fzerocar"],
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
        setup: icemt::on_init,
        load: icemt::on_load,
        start: icemt::on_start,
        demo: Some(|r, _| icemt::on_demo_init(r)),
        procs: Some(icemt::run),
        renders: None,
        shadow: Some(|_, _, _| icemt::check_shadow_render()),
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
            modules: &["gr/icemt", "it/kinds/yaku"],
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
        setup: inishie1::on_init,
        load: inishie1::on_load,
        start: inishie1::on_start,
        demo: Some(|r, _| inishie1::on_demo_init(r)),
        procs: Some(inishie1::run),
        renders: None,
        shadow: Some(|_, _, _| inishie1::check_shadow_render()),
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
            modules: &["gr/inishie1", "it/kinds/yaku"],
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
        setup: inishie2::on_init,
        load: inishie2::on_load,
        start: inishie2::on_start,
        demo: Some(|r, _| inishie2::on_demo_init(r)),
        procs: Some(inishie2::run),
        renders: None,
        shadow: Some(|_, _, _| inishie2::check_shadow_render()),
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
            modules: &["gr/inishie2", "it/kinds/kyasarin", "it/kinds/kyasarinegg"],
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
        setup: flatzone::on_init,
        load: flatzone::on_load,
        start: flatzone::on_start,
        demo: Some(|r, _| flatzone::on_demo_init(r)),
        procs: Some(flatzone::run),
        renders: None,
        shadow: Some(|_, _, _| flatzone::check_shadow_render()),
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
            modules: &["gr/flatzone", "it/kinds/tools"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::OldPupupu,
        name: "Dream Land",
        data: &data::OLDPUPUPU,
        setup: oldpupupu::on_init,
        load: oldpupupu::on_load,
        start: oldpupupu::on_start,
        demo: Some(|r, _| oldpupupu::on_demo_init(r)),
        procs: Some(oldpupupu::run),
        renders: None,
        shadow: Some(|r, pos, j| oldpupupu::check_shadow_render(r.game, r.data, pos, Some(j))),
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
            modules: &["gr/oldpupupu"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::OldYoshi,
        name: "Yoshi's Island N64",
        data: &data::OLDYOSHI,
        setup: oldyoshi::on_init,
        load: oldyoshi::on_load,
        start: oldyoshi::on_start,
        demo: Some(|r, _| oldyoshi::on_demo_init(r)),
        procs: Some(oldyoshi::run),
        renders: None,
        shadow: Some(|r, pos, j| oldyoshi::check_shadow_render(r.game, r.data, pos, Some(j))),
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
            modules: &["gr/oldyoshi"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::OldKongo,
        name: "Kongo Jungle N64",
        data: &data::OLDKONGO,
        setup: oldkongo::on_init,
        load: oldkongo::on_load,
        start: oldkongo::on_start,
        demo: Some(|r, _| oldkongo::on_demo_init(r)),
        procs: Some(oldkongo::run),
        renders: None,
        shadow: Some(|r, pos, j| oldkongo::check_shadow_render(r.game, r.data, pos, Some(j))),
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
            modules: &["gr/oldkongo"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::KinokoRoute,
        name: "Mushroom Kingdom (Adventure)",
        data: &data::KINOKOROUTE,
        setup: kinokoroute::on_init,
        load: kinokoroute::on_load,
        start: kinokoroute::on_start,
        demo: Some(|r, _| kinokoroute::on_demo_init(r)),
        procs: Some(kinokoroute::run),
        renders: None,
        shadow: Some(|r, pos, j| kinokoroute::check_shadow_render(r.game, r.data, pos, Some(j))),
        touch_line: Some(|_, line| kinokoroute::on_touch_line(line)),
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
        setup: shrineroute::on_init,
        load: shrineroute::on_load,
        start: shrineroute::on_start,
        demo: Some(|r, _| shrineroute::on_demo_init(r)),
        procs: Some(shrineroute::run),
        renders: None,
        shadow: Some(|r, pos, j| shrineroute::check_shadow_render(r.game, r.data, pos, Some(j))),
        touch_line: Some(|_, line| shrineroute::on_touch_line(line)),
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
        setup: zebesroute::on_init,
        load: zebesroute::on_load,
        start: zebesroute::on_start,
        demo: Some(|r, _| zebesroute::on_demo_init(r)),
        procs: Some(zebesroute::run),
        renders: None,
        shadow: Some(|_, _, _| zebesroute::check_shadow_render()),
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
        setup: bigblueroute::on_init,
        load: bigblueroute::on_load,
        start: bigblueroute::on_start,
        demo: Some(|r, _| bigblueroute::on_demo_init(r)),
        procs: Some(bigblueroute::run),
        renders: None,
        shadow: Some(|r, pos, j| bigblueroute::check_shadow_render(r.game, r.data, pos, Some(j))),
        touch_line: Some(|_, line| bigblueroute::on_touch_line(line)),
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
        setup: teststage::on_init,
        load: teststage::on_load,
        start: teststage::on_start,
        demo: Some(|r, _| teststage::on_demo_init(r)),
        procs: Some(teststage::run),
        renders: None,
        shadow: Some(|_, _, _| teststage::check_shadow_render()),
        touch_line: Some(|_, line| teststage::on_touch_line(line)),
        vars: bytes,
        vs: None,
        keys: &[],
        audit: None,
    }),
    Some(Stage {
        kind: GrKind::Battle,
        name: "Battlefield",
        data: &data::BATTLE,
        setup: battle::on_init,
        load: battle::on_load,
        start: battle::on_start,
        demo: Some(|r, _| battle::on_demo_init(r.game)),
        procs: Some(battle::run),
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
            modules: &["gr/battle"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::Last,
        name: "Final Destination",
        data: &data::LAST,
        setup: last::on_init,
        load: last::on_load,
        start: last::on_start,
        demo: Some(last::on_demo_init),
        procs: Some(last::run),
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
            modules: &["gr/last"],
            unreached: &[],
            missing: &[],
        }),
    }),
    Some(Stage {
        kind: GrKind::FigureGet,
        name: "Snag the Trophies",
        data: &data::FIGUREGET,
        setup: figureget::on_init,
        load: figureget::on_load,
        start: figureget::on_start,
        demo: Some(|r, _| figureget::on_demo_init(r)),
        procs: Some(figureget::run),
        renders: None,
        shadow: Some(|_, _, _| figureget::check_shadow_render()),
        touch_line: Some(|_, line| figureget::on_touch_line(line)),
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
        setup: pushon::on_init,
        load: pushon::on_load,
        start: pushon::on_start,
        demo: Some(|r, _| pushon::on_demo_init(r)),
        procs: Some(pushon::run),
        renders: None,
        shadow: Some(|_, _, _| pushon::check_shadow_render()),
        touch_line: Some(|r, line| pushon::line_hazard(r.game, r.data, line)),
        vars: |map_id| match map_id {
            1 => VarsKind::Map,
            2 => VarsKind::PushOn,
            _ => VarsKind::Bytes,
        },
        vs: None,
        keys: &[],
        audit: None,
    }),
    target!(TMario, target::TMARIO_TARGET.data, "Target Test (Mario)", 0),
    target!(
        TCaptain,
        target::TCAPTAIN_TARGET.data,
        "Target Test (Captain Falcon)",
        1
    ),
    target!(
        TClink,
        target::TCLINK_TARGET.data,
        "Target Test (Young Link)",
        2
    ),
    target!(
        TDonkey,
        target::TDONKEY_TARGET.data,
        "Target Test (Donkey Kong)",
        3
    ),
    target!(
        TDrmario,
        target::TDRMARIO_TARGET.data,
        "Target Test (Dr. Mario)",
        4
    ),
    target!(TFalco, target::TFALCO_TARGET.data, "Target Test (Falco)", 5),
    target!(TFox, target::TFOX_TARGET.data, "Target Test (Fox)", 6),
    target!(
        TIceclimber,
        target::TICECLIMBER_TARGET.data,
        "Target Test (the Ice Climbers)",
        7
    ),
    target!(TKirby, target::TKIRBY_TARGET.data, "Target Test (Kirby)", 8),
    target!(
        TKoopa,
        target::TKOOPA_TARGET.data,
        "Target Test (Bowser)",
        9
    ),
    target!(TLink, target::TLINK_TARGET.data, "Target Test (Link)", 10),
    target!(
        TLuigi,
        target::TLUIGI_TARGET.data,
        "Target Test (Luigi)",
        11
    ),
    target!(TMars, target::TMARS_TARGET.data, "Target Test (Marth)", 12),
    target!(
        TMewtwo,
        target::TMEWTWO_TARGET.data,
        "Target Test (Mewtwo)",
        13
    ),
    target!(TNess, target::TNESS_TARGET.data, "Target Test (Ness)", 14),
    target!(
        TPeach,
        target::TPEACH_TARGET.data,
        "Target Test (Peach)",
        15
    ),
    target!(
        TPichu,
        target::TPICHU_TARGET.data,
        "Target Test (Pichu)",
        16
    ),
    target!(
        TPikachu,
        target::TPIKACHU_TARGET.data,
        "Target Test (Pikachu)",
        17
    ),
    target!(
        TPurin,
        target::TPURIN_TARGET.data,
        "Target Test (Jigglypuff)",
        18
    ),
    target!(
        TSamus,
        target::TSAMUS_TARGET.data,
        "Target Test (Samus)",
        19
    ),
    target!(TSeak, target::TSEAK_TARGET.data, "Target Test (Sheik)", 20),
    target!(
        TYoshi,
        target::TYOSHI_TARGET.data,
        "Target Test (Yoshi)",
        21
    ),
    target!(
        TZelda,
        target::TZELDA_TARGET.data,
        "Target Test (Zelda)",
        22
    ),
    target!(
        TGamewatch,
        target::TGAMEWATCH_TARGET.data,
        "Target Test (Mr. Game & Watch)",
        23
    ),
    target!(
        TEmblem,
        target::TEMBLEM_TARGET.data,
        "Target Test (Roy)",
        24
    ),
    target!(
        TGanon,
        target::TGANON_TARGET.data,
        "Target Test (Ganondorf)",
        25
    ),
    Some(Stage {
        kind: GrKind::Heal,
        name: "All-Star's rest area",
        data: &data::HEAL,
        setup: heal::on_init,
        load: nothing,
        start: heal::on_start,
        demo: Some(|_, _| heal::empty()),
        procs: Some(heal::run),
        renders: None,
        shadow: Some(|_, _, _| heal::check_shadow_render()),
        touch_line: Some(|_, line| heal::on_touch_line(line)),
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
        setup: homerun::on_init,
        load: homerun::on_load,
        start: homerun::on_start,
        demo: Some(|r, _| homerun::on_demo_init(r)),
        procs: Some(homerun::run),
        renders: Some(homerun::run_render),
        shadow: Some(|r, pos, j| homerun::check_shadow_render(r.game, r.data, pos, Some(j))),
        touch_line: Some(|_, line| homerun::on_touch_line(line)),
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
        setup: |r| figure::on_init(r, 0),
        load: figure::on_load,
        start: figure::on_start,
        demo: Some(|r, _| figure::on_demo_init(r)),
        procs: Some(figure::run),
        renders: None,
        shadow: Some(|_, _, _| figure::check_shadow_render()),
        touch_line: Some(|_, line| figure::on_touch_line(line)),
        vars: bytes,
        vs: None,
        keys: &[],
        audit: None,
    }),
    Some(Stage {
        kind: GrKind::Figure2,
        name: "The events' second trophy stage",
        data: &data::FIGURE2,
        setup: |r| figure::on_init(r, 1),
        load: figure::on_load,
        start: figure::on_start,
        demo: Some(|r, _| figure::on_demo_init(r)),
        procs: Some(figure::run),
        renders: None,
        shadow: Some(|_, _, _| figure::check_shadow_render()),
        touch_line: Some(|_, line| figure::on_touch_line(line)),
        vars: bytes,
        vs: None,
        keys: &[],
        audit: None,
    }),
    Some(Stage {
        kind: GrKind::Figure3,
        name: "The events' third trophy stage",
        data: &data::FIGURE3,
        setup: |r| figure::on_init(r, 2),
        load: figure::on_load,
        start: figure::on_start,
        demo: Some(|r, _| figure::on_demo_init(r)),
        procs: Some(figure::run),
        renders: None,
        shadow: Some(|_, _, _| figure::check_shadow_render()),
        touch_line: Some(|_, line| figure::on_touch_line(line)),
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
