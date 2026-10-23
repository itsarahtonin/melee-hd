//! The items-on audit (docs/player.md, "Items toggle: what's left"): what a match with the
//! item switch on can reach beyond an items-off match, and what of it isn't ported, held to
//! the port ledger and the code as the stages' audits in [`super::roster`] are. The player's
//! launcher offers an item switch kind only once no gap that kind reaches is left for the
//! chosen stage and characters ([`missing_with_switch`]). Presentation data only: nothing in
//! the simulation reads it.

/// Something an items-on match reaches that no port covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemGap {
    /// What it is, as the player would say so.
    pub what: &'static str,
    /// The ledger's functions and units, and the stops' text, it stands for.
    pub covers: &'static [&'static str],
    /// The item switch's kinds (`ItemKind`, 0 to 0x22) any of which reaches it: none, any
    /// items-on match.
    pub kinds: &'static [i32],
    /// The characters (`CharacterKind`) one of which must play: none, any match.
    pub with: &'static [u8],
    /// The stages (`StKind`) it happens on: none, any.
    pub stages: &'static [u16],
}

/// The gaps an items-on match with these item switch kinds, characters and stage reaches.
pub fn missing_with(kinds: &[i32], characters: &[u8], stage: u16) -> Vec<&'static ItemGap> {
    let any = |set: &[i32], of: &[i32]| set.is_empty() || set.iter().any(|k| of.contains(k));
    (MISSING.iter())
        .filter(|g| !kinds.is_empty() && any(g.kinds, kinds))
        .filter(|g| g.with.is_empty() || g.with.iter().any(|c| characters.contains(c)))
        .filter(|g| g.stages.is_empty() || g.stages.contains(&stage))
        .collect()
}

/// The kinds the random spawner draws from (`It_Kind_Capsule` to `It_Kind_M_Ball`).
pub const RANDOM_KINDS: i32 = 35;

/// The random kinds an items-on match from the VS menus always has on: those the item switch
/// doesn't list (`vs_mode::ITEM_SWITCH_KINDS`: the capsule, box, barrel and egg), which the
/// menus leave as the rules' defaults set them.
pub fn unswitched_kinds() -> Vec<i32> {
    use super::gm::vs_mode::{ITEM_SWITCH_KINDS, NO_ITEM_KIND};
    (0..RANDOM_KINDS)
        .filter(|&k| !ITEM_SWITCH_KINDS.contains(&(k as u8)) || k as u8 == NO_ITEM_KIND)
        .collect()
}

/// The gaps an items-on match from the VS menus reaches with the item switch's `kinds` on, the
/// kinds it can't turn off ([`unswitched_kinds`]) with them; none with the switch all off.
pub fn missing_with_switch(kinds: &[i32], characters: &[u8], stage: u16) -> Vec<&'static ItemGap> {
    if kinds.is_empty() {
        return Vec::new();
    }
    let mut all = unswitched_kinds();
    all.extend_from_slice(kinds);
    missing_with(&all, characters, stage)
}

/// The ledger's units the audit holds to the code: every item unit with functions not yet
/// accounted for, and the fighter units only items reach.
pub const UNITS: &[&str] = &[
    "melee/it/it_26B1",
    "melee/it/it_2725",
    "melee/it/it_279C",
    "melee/it/itanimlist",
    "melee/it/itcoll",
    "melee/it/itdraw",
    "melee/it/item",
    "melee/it/itgroundcoll",
    "melee/it/ithitbox",
    "melee/it/itmaterial",
    "melee/it/kinds/it_2ADA",
    "melee/it/kinds/it_2E5A",
    "melee/it/kinds/it_2F28",
    "melee/it/kinds/itarwinglaser",
    "melee/it/kinds/itbat",
    "melee/it/kinds/itcapsule",
    "melee/it/kinds/itcerebi",
    "melee/it/kinds/itclimbersblizzard",
    "melee/it/kinds/itclimbersice",
    "melee/it/kinds/itclimbersstring",
    "melee/it/kinds/itcoin",
    "melee/it/kinds/itcrazyhandbomb",
    "melee/it/kinds/itdrmariopill",
    "melee/it/kinds/itegg",
    "melee/it/kinds/itevyoshiegg",
    "melee/it/kinds/itfflower",
    "melee/it/kinds/itfflowerflame",
    "melee/it/kinds/itflipper",
    "melee/it/kinds/itfoods",
    "melee/it/kinds/itfoxblaster",
    "melee/it/kinds/itfoxillusion",
    "melee/it/kinds/itfoxlaser",
    "melee/it/kinds/itfreeze",
    "melee/it/kinds/itgamewatchbreath",
    "melee/it/kinds/itgamewatchchef",
    "melee/it/kinds/itgamewatchfire",
    "melee/it/kinds/itgamewatchgreenhouse",
    "melee/it/kinds/itgamewatchjudge",
    "melee/it/kinds/itgamewatchmanhole",
    "melee/it/kinds/itgamewatchpanic",
    "melee/it/kinds/itgamewatchparachute",
    "melee/it/kinds/itgamewatchrescue",
    "melee/it/kinds/itgamewatchturtle",
    "melee/it/kinds/itgreatfoxlaser",
    "melee/it/kinds/itharisen",
    "melee/it/kinds/itheart",
    "melee/it/kinds/itheiho",
    "melee/it/kinds/itkirbycutterbeam",
    "melee/it/kinds/itkirbygamewatchchefpan",
    "melee/it/kinds/itkirbyhammer",
    "melee/it/kinds/itkirbyyoshispecialn",
    "melee/it/kinds/itkusudama",
    "melee/it/kinds/itkyasarin",
    "melee/it/kinds/itkyasarinegg",
    "melee/it/kinds/itleadead",
    "melee/it/kinds/itlgun",
    "melee/it/kinds/itlgunbeam",
    "melee/it/kinds/itlgunray",
    "melee/it/kinds/itlikelike",
    "melee/it/kinds/itlipstick",
    "melee/it/kinds/itlipstickspore",
    "melee/it/kinds/itlucky",
    "melee/it/kinds/itluigifireball",
    "melee/it/kinds/itmariocape",
    "melee/it/kinds/itmariofireball",
    "melee/it/kinds/itmasterhandbullet",
    "melee/it/kinds/itmasterhandlaser",
    "melee/it/kinds/itmato",
    "melee/it/kinds/itmball",
    "melee/it/kinds/itmetamon",
    "melee/it/kinds/itmsbomb",
    "melee/it/kinds/itnessbat",
    "melee/it/kinds/itnesspkfire",
    "melee/it/kinds/itnesspkfirepillar",
    "melee/it/kinds/itnesspkflash",
    "melee/it/kinds/itnesspkflashexplode",
    "melee/it/kinds/itnesspkthunderball",
    "melee/it/kinds/itnesspkthundertrail",
    "melee/it/kinds/itnessyoyo",
    "melee/it/kinds/itnokonoko",
    "melee/it/kinds/itparasol",
    "melee/it/kinds/itpatapata",
    "melee/it/kinds/itpikachuthunder",
    "melee/it/kinds/itpikachutjoltair",
    "melee/it/kinds/itpikachutjoltground",
    "melee/it/kinds/itrabbitc",
    "melee/it/kinds/itsamusbomb",
    "melee/it/kinds/itsamuschargeshot",
    "melee/it/kinds/itsamusgrapple",
    "melee/it/kinds/itsamusmissile",
    "melee/it/kinds/itsscope",
    "melee/it/kinds/itsscopebeam",
    "melee/it/kinds/itstar",
    "melee/it/kinds/itstarrod",
    "melee/it/kinds/itstarrodstar",
    "melee/it/kinds/itsword",
    "melee/it/kinds/ittarucann",
    "melee/it/kinds/ittincle",
    "melee/it/kinds/ittomato",
    "melee/it/kinds/itwhitebea",
    "melee/it/kinds/itwstar",
    "melee/it/kinds/ityoshiegglay",
    "melee/it/kinds/ityoshieggthrow",
    "melee/it/kinds/ityoshistar",
    "melee/it/kinds/itzeldadinfire",
    "melee/it/kinds/itzeldadinfireexplode",
    "melee/it/kinds/itzgshell",
    "melee/it/kinds/itzrshell",
    "melee/ft/kinds/ftCommon/ftCo_Barrel",
    "melee/ft/kinds/ftCommon/ftCo_WarpStar",
    "melee/ft/ftCo_800C78B0",
    "melee/ft/ftCo_800C7590",
    "melee/ft/ftCo_800C7434",
    "melee/ft/ft_0BEC",
    "melee/ft/ft_0BF0",
];

/// melee-sim's item modules (`game/<module>.rs`), each of whose stops the audit names.
pub const MODULES: &[&str] = &[
    "it/anim",
    "it/coll",
    "it/data",
    "it/draw",
    "it/drop",
    "it/effect",
    "it/ground",
    "it/hitbox",
    "it/item",
    "it/kinds/arwinglaser",
    "it/kinds/bat",
    "it/kinds/bombhei",
    "it/kinds/capsule",
    "it/kinds/chicorita",
    "it/kinds/climbersblizzard",
    "it/kinds/climbersice",
    "it/kinds/climbersstring",
    "it/kinds/clinkmilk",
    "it/kinds/coin",
    "it/kinds/crate_box",
    "it/kinds/dosei",
    "it/kinds/drmariopill",
    "it/kinds/egg",
    "it/kinds/entei",
    "it/kinds/fflower",
    "it/kinds/fire",
    "it/kinds/flipper",
    "it/kinds/flower",
    "it/kinds/foods",
    "it/kinds/foxblaster",
    "it/kinds/foxillusion",
    "it/kinds/foxlaser",
    "it/kinds/freeze",
    "it/kinds/freezer",
    "it/kinds/fushigibana",
    "it/kinds/gamewatch",
    "it/kinds/gamewatchbreath",
    "it/kinds/gamewatchchef",
    "it/kinds/gamewatchfire",
    "it/kinds/gamewatchgreenhouse",
    "it/kinds/gamewatchjudge",
    "it/kinds/gamewatchmanhole",
    "it/kinds/gamewatchpanic",
    "it/kinds/gamewatchparachute",
    "it/kinds/gamewatchrescue",
    "it/kinds/gamewatchturtle",
    "it/kinds/greatfoxlaser",
    "it/kinds/gshell",
    "it/kinds/hammer",
    "it/kinds/hammerhead",
    "it/kinds/harisen",
    "it/kinds/hassam",
    "it/kinds/heart",
    "it/kinds/heiho",
    "it/kinds/hinoarashi",
    "it/kinds/hitodeman",
    "it/kinds/houou",
    "it/kinds/kabigon",
    "it/kinds/kamex",
    "it/kinds/kinoko",
    "it/kinds/kirbycopystar",
    "it/kinds/kirbycutterbeam",
    "it/kinds/kirbygamewatchchefpan",
    "it/kinds/kirbyhammer",
    "it/kinds/kirbyinhale",
    "it/kinds/kirbyspitstar",
    "it/kinds/kireihana",
    "it/kinds/klap",
    "it/kinds/koopaflame",
    "it/kinds/kusudama",
    "it/kinds/kyasarin",
    "it/kinds/kyasarinegg",
    "it/kinds/leadead",
    "it/kinds/lgun",
    "it/kinds/lgunbeam",
    "it/kinds/lgunray",
    "it/kinds/linkarrow",
    "it/kinds/linkbomb",
    "it/kinds/linkboomerang",
    "it/kinds/linkbow",
    "it/kinds/linkhookshot",
    "it/kinds/lipstick",
    "it/kinds/lipstickspore",
    "it/kinds/lizardon",
    "it/kinds/lucky",
    "it/kinds/lugia",
    "it/kinds/luigifireball",
    "it/kinds/maril",
    "it/kinds/mariocape",
    "it/kinds/mariofireball",
    "it/kinds/marumine",
    "it/kinds/matadogas",
    "it/kinds/mball",
    "it/kinds/metalb",
    "it/kinds/mew",
    "it/kinds/mewtwodisable",
    "it/kinds/mewtwoshadowball",
    "it/kinds/mod",
    "it/kinds/msbomb",
    "it/kinds/nessbat",
    "it/kinds/nesspkfire",
    "it/kinds/nesspkfirepillar",
    "it/kinds/nesspkflash",
    "it/kinds/nesspkflashexplode",
    "it/kinds/nesspkthunder",
    "it/kinds/nesspkthundertrail",
    "it/kinds/nessyoyo",
    "it/kinds/octarock",
    "it/kinds/octarockstone",
    "it/kinds/oldkuri",
    "it/kinds/oldottosea",
    "it/kinds/parasol",
    "it/kinds/peachexplode",
    "it/kinds/peachparasol",
    "it/kinds/peachtoad",
    "it/kinds/peachtoadspore",
    "it/kinds/peachturnip",
    "it/kinds/pikachuthunder",
    "it/kinds/pikachutjoltair",
    "it/kinds/pikachutjoltground",
    "it/kinds/pippi",
    "it/kinds/porygon2",
    "it/kinds/rabbitc",
    "it/kinds/raikou",
    "it/kinds/rshell",
    "it/kinds/samusbomb",
    "it/kinds/samuschargeshot",
    "it/kinds/samusgrapple",
    "it/kinds/samusmissile",
    "it/kinds/scball",
    "it/kinds/seakchain",
    "it/kinds/seakneedleheld",
    "it/kinds/seakneedlethrown",
    "it/kinds/seakvanish",
    "it/kinds/sonans",
    "it/kinds/spycloak",
    "it/kinds/sscope",
    "it/kinds/sscopebeam",
    "it/kinds/star",
    "it/kinds/starrod",
    "it/kinds/starrodstar",
    "it/kinds/suikun",
    "it/kinds/sword",
    "it/kinds/taru",
    "it/kinds/tarucann",
    "it/kinds/thunder",
    "it/kinds/tincle",
    "it/kinds/togepy",
    "it/kinds/tomato",
    "it/kinds/tools",
    "it/kinds/tosakinto",
    "it/kinds/unknown",
    "it/kinds/whispyapple",
    "it/kinds/wstar",
    "it/kinds/yaku",
    "it/kinds/yoshiegglay",
    "it/kinds/yoshieggthrow",
    "it/kinds/yoshistar",
    "it/kinds/yoshitongue",
    "it/kinds/zeldadinfire",
    "it/kinds/zeldadinfireexplode",
    "it/lib26b1",
    "it/lib2725",
    "it/lib279c",
    "it/link",
    "it/maplib",
    "it/mod",
    "it/owner",
    "it/pokemon/mod",
    "it/pokemon/vars",
    "it/release",
    "it/spawn",
    "it/tables",
    "it/tables_gen",
    "it/vars",
    "it/zako",
];

const ADVENTURE: &str = "an adventure enemy (kinds 0xD0 to 0xDC), which only a stage's enemy spawner or adventure stage code makes; no roster stage configures one";
const BONUS: &str =
    "the bonus match's tally (pl_80039450), which a stock or time match doesn't run";
const BOSSES: &str = "Master Hand's and Crazy Hand's items, which only those fighters spawn";
const COINS: &str = "coins: the single-player coin (trophy stages and a 1P enemy's drop, it_8027CC88 checks gm_IsCurrently1PMode) and coin matches' (it_8026F8B4 checks gm_8016B0B4)";
const COLANIM: &str = "the items' own color script commands 21 and 22, which none of ItCo.usd's six item color animations uses";
const DEBUG: &str = "the debug menu's item spawn and the items' debug drawings";
const DEMO: &str = "the demo fighters' items and states (ftDemo_CreateFighter's motion table): results and intros, never a match";
const CEREBI: &str = "Celebi, whose draw (it_8027A4D4) asks gm_80165084 whether the card records every Pokemon trophy; with Slippi's codes, the player's unlocks (Unlocks::slippi), the records' test answers with the index it is asked about (blr at 0x8015D94C), so record 0 reads as missing and Celebi never comes out";
const DITTO: &str = "Ditto, whose weight in the Poke Ball's table is 0 (ItCo.usd): only an event list, training or the debug menu picks it";
const DROP_BY_KIND: &str = "a spawn by kind of a box, Flipper, food, Home-Run Bat or bob-omb (it_8026BE84): the bob-omb rain of a sudden death, which the player doesn't play, Home-Run and the All-Star rest area (Big Blue's flying platforms make only the five kinds ported)";
const EVENT: &str =
    "an event match's items (the event Yoshi egg, the events' heart and tomato slots)";
const EVT_UNK: &str =
    "an item's evt_unk, which only a fighter's unload at the scene's end runs (it_8026B7F8)";
const ONE_PLAYER: &str = "the single-player modes' records (Classic, Adventure, All-Star)";
const POKEMON_MODES: &str =
    "training's and an event's Pokemon draws, which it_8027AB64 takes only in those modes";
const SCENES: &str = "other scenes' item setup (intros, cutscenes, the trophy fall)";
const YAKU: &str =
    "a stage item on neither a joint nor a point, which no stage makes (the original hangs)";
const ZAKO: &str = "the stage enemy spawner (grZakoGenerator), which no roster stage configures";
const ATTACKER: &str = "an item hit by an item keeps the attacking item's owner (xCF0_itemGObj) only when it's a fighter (it_80270E30's case 2), and nothing else writes it, so it_8027236C takes a fighter's team or none";
const THROWER: &str = "a thrown fighter's hitbox word is written only by ftColl_8007B8CC, whose callers pass a fighter (Fighter_ChangeMotionState's thrower, Kirby's inhale, the barrel cannon after ftLib_IsFighter), and cleared to none";
const GRABBERS: &str = "of the items that grab fighters (it_80274F28's callers) a VS match makes only the barrel cannon, whose callback is ftCo_Barrel_Enter (Like Like's are adventure only), and its owner is a fighter or none: it spawns without a parent and only fighters pick it up, throw it, hit it (it_8027236C) or stand on it";
const GIGA_BOWSER: &str =
    "Giga Bowser's motion table's callback (ftgkoopa.c), a fighter no VS roster has";
const LIKE_LIKE: &str = "the Like Like's held fighter's death callback, which only its capture (ftCo_800C78B0) sets: an adventure enemy";
const PROBE_BELOW: &str = "it_802759DC probes the floor mpLib_8005199C_Floor found under the item, one whose span holds the item's x, which mpLib_8004DD90_Floor then finds at that x and writes the height of";
const ONE_PLAYER_TROPHY: &str = "a beaten enemy's trophy, which it_8027CC88 rolls only when gm_IsCurrently1PMode says a single-player mode is on";
const ENEMY_KINDS: &str = "an enemy of another kind: in a VS match only a container (the monsters' table, it_8026CF04: Goomba, ReDead, Octorok, Topi) and the offered stages' own enemies (Shy Guys, Tingle, Klap Traps) spawn one; the rest come from the stage enemy spawner or adventure stages";
const ENEMY_BONUS: &str = "a bonus other than an enemy's KO: gm_8016F2F8's only caller in melee-hd is pl_8004049C, whose bonuses for the enemies a VS match spawns are all transcribed";
const WALK_ENDS: &str = "it_802EA804's walks start from the ReDead lifted 0.0001 onto the line it last stood on (mpLib_8004DD90_Floor), so the first line's length to its end is positive unless that lift is lost to rounding (2,048 units or more from the origin) and its sideways distance squares to 0 (closer than 4e-23), which no stage's lines give. A walk reaches its end only on a line neither a wall nor a ceiling, so on a floor (every line has one kind: the data's, or the one mpJointUpdateDynamics gives a dynamic line); the walk of 0 goes back and ends on its first line with positive length: from a floor, there with its normal; from a wall (a dynamic line tilted into one) the wall's length added to f27, 0 there (PROC_F27), passes the walk's limit of 0 and stops it short, as it stops the walk ahead, which then doesn't read the normals";

/// Functions, units (for every function of one not named on its own) and stops no items-on
/// match from the player reaches (VS mode, the roster's characters and offered stages, no
/// sudden death), each with why.
pub const UNREACHED: &[(&str, &str)] = &[
    ("it_8026B7F8", EVT_UNK),
    ("it_2E5A_Logic115_EvtUnk", EVT_UNK),
    ("it_802F2BDC", EVT_UNK),
    ("itBat_Logic11_EvtUnk", EVT_UNK),
    ("itCapsule_Logic0_EvtUnk", EVT_UNK),
    ("it_802D3F4C", EVT_UNK),
    ("itCoin_Logic116_EvtUnk", EVT_UNK),
    ("itCrazyHandBomb_Logic86_EvtUnk", EVT_UNK),
    ("itEgg_Logic3_EvtUnk", EVT_UNK),
    ("itEvYoshiEgg_Logic42_EvtUnk", EVT_UNK),
    ("itFFlower_Logic25_EvtUnk", EVT_UNK),
    ("itFFlowerFlame_Logic41_EvtUnk", EVT_UNK),
    ("itFlipper_EvtUnk", EVT_UNK),
    ("itFoods_Logic18_EvtUnk", EVT_UNK),
    ("itFreeze_Logic17_EvtUnk", EVT_UNK),
    ("it_802EB5A8", EVT_UNK),
    ("itHarisen_Logic24_EvtUnk", EVT_UNK),
    ("itHeart_Logic8_EvtUnk", EVT_UNK),
    ("it_802D9A0C", EVT_UNK),
    ("it_802EE1E0", EVT_UNK),
    ("it_802F0320", EVT_UNK),
    ("itLGun_Logic16_EvtUnk", EVT_UNK),
    ("itLGunBeam_Logic39_EvtUnk", EVT_UNK),
    ("itLGunRay_Logic35_EvtUnk", EVT_UNK),
    ("itLipstick_Logic23_EvtUnk", EVT_UNK),
    ("itLipstickSpore_Logic37_EvtUnk", EVT_UNK),
    ("itLucky_Logic44_EvtUnk", EVT_UNK),
    ("itMasterHandBullet_Logic85_EvtUnk", EVT_UNK),
    ("itMasterHandLaser_Logic84_EvtUnk", EVT_UNK),
    ("itMball_EvtUnk", EVT_UNK),
    ("itMetamon_Logic19_EvtUnk", EVT_UNK),
    ("itMSBomb_Logic19_EvtUnk", EVT_UNK),
    ("it_802DD7D0", EVT_UNK),
    ("itParasol_Logic13_EvtUnk", EVT_UNK),
    ("it_802E16D8", EVT_UNK),
    ("itSScope_Logic21_EvtUnk", EVT_UNK),
    ("itSScopeBeam_Logic38_EvtUnk", EVT_UNK),
    ("itStarRod_Logic22_EvtUnk", EVT_UNK),
    ("itStarRodStar_Logic36_EvtUnk", EVT_UNK),
    ("itTaruCann_Logic5_EvtUnk", EVT_UNK),
    ("it_802EC850", EVT_UNK),
    ("it_802EE1E0", EVT_UNK),
    ("it_802F0320", EVT_UNK),
    ("itTomato_Logic9_EvtUnk", EVT_UNK),
    ("it_802E4A24", EVT_UNK),
    ("itWStar_Logic30_EvtUnk", EVT_UNK),
    ("it_802DFED4", EVT_UNK),
    ("it_802E0468", EVT_UNK),
    ("it_80275210", ADVENTURE),
    ("it_80271534", ADVENTURE),
    ("melee/it/kinds/itlikelike", ADVENTURE),
    ("melee/it/kinds/itmato", ADVENTURE),
    ("melee/it/kinds/itnokonoko", ADVENTURE),
    ("melee/it/kinds/itpatapata", ADVENTURE),
    ("melee/it/kinds/itwhitebea", ADVENTURE),
    ("melee/it/kinds/itzgshell", ADVENTURE),
    ("melee/it/kinds/itzrshell", ADVENTURE),
    ("melee/it/kinds/itevyoshiegg", EVENT),
    ("it_8026C3FC", BOSSES),
    ("it_8026C42C", BOSSES),
    ("melee/it/ithitbox", BOSSES),
    ("melee/it/kinds/itcrazyhandbomb", BOSSES),
    ("melee/it/kinds/itmasterhandbullet", BOSSES),
    ("melee/it/kinds/itmasterhandlaser", BOSSES),
    ("it_802E614C", COINS),
    ("melee/it/kinds/itcoin", COINS),
    ("melee/it/kinds/itmetamon", DITTO),
    ("it_8026BE84", DROP_BY_KIND),
    ("it_80284854", DROP_BY_KIND),
    ("itFlipper_Spawn", DROP_BY_KIND),
    ("it_80283AE4", DROP_BY_KIND),
    ("it_802841B4", DROP_BY_KIND),
    ("it_8026ECE0", DEBUG),
    ("Item_80266F3C", DEBUG),
    ("Item_80266FA8", SCENES),
    ("it_8026C1E8", ZAKO),
    ("it_802EA9FC", ZAKO),
    ("it_802DC4BC", ZAKO),
    ("it_802E16F8", ZAKO),
    ("melee/it/it_279C", POKEMON_MODES),
    ("it_80279AF0", COLANIM),
    ("it_80279B10", COLANIM),
    ("it_8026B7D8", BONUS),
    ("it_8026B7E0", BONUS),
    ("melee/ft/ftCo_800C78B0", ADVENTURE),
    ("melee/ft/ft_0BEC", DEMO),
    ("melee/ft/ft_0BF0", DEMO),
    ("it_8027A9B8 (an event", POKEMON_MODES),
    ("it_8027A780 (training", POKEMON_MODES),
    ("fn_8017DF28", ONE_PLAYER),
    ("gm_80473A18", EVENT),
    ("ftCo_800BF228", DEMO),
    ("it_2E6A_UnkMotion19_Phys", YAKU),
    ("ftLib_CountFighters (Ditto's check)", DITTO),
    ("melee/it/kinds/itcerebi", CEREBI),
    ("grZakoGenerator_801CAC14 (a coin)", COINS),
    ("grZakoGenerator_801CACB8 (a coin)", COINS),
    (
        "grZakoGenerator_801CAC14 (an enemy from a spawner slot)",
        ZAKO,
    ),
    (
        "grZakoGenerator_801CACB8 (an enemy from a spawner slot)",
        ZAKO,
    ),
    ("ftLib_GetTeam on an item", ATTACKER),
    (
        "a thrown fighter's hitbox word that isn't a thrower",
        THROWER,
    ),
    (
        "it_802701BC with an item owner that isn't a fighter",
        GRABBERS,
    ),
    ("(a grabbed fighter's callback)", GRABBERS),
    (
        "it_8027CC88 (a 1P mode's trophy from an enemy)",
        ONE_PLAYER_TROPHY,
    ),
    ("ftCo_800C7434", DEMO),
    ("ftCo_800C74AC", GIGA_BOWSER),
    ("fn_800C7890", LIKE_LIKE),
    (
        "it_802759DC (a height the floor probe left unwritten)",
        PROBE_BELOW,
    ),
    ("it_8027B5B0 (an enemy of kind", ENEMY_KINDS),
    ("pl_8004049C (an enemy of kind", ENEMY_KINDS),
    (
        "gm_8016F2F8 (a bonus besides the enemies' KOs)",
        ENEMY_BONUS,
    ),
    (
        "it_802EA804 (a floor normal the first walk left unwritten)",
        WALK_ENDS,
    ),
    (
        "it_802EA804 (a floor normal the walk left unwritten)",
        WALK_ENDS,
    ),
];

/// Ported code an items-on match reaches whose ledger alias waits for a check (a `// Ports`
/// comment, or inline in a port): it doesn't stop a match.
pub const UNCHECKED: &[&str] = &[
    "melee/it/it_26B1",
    "melee/it/it_2725",
    "melee/it/itanimlist",
    "it_8026FE68",
    "melee/it/item",
    "it_80275870",
    "melee/it/itmaterial",
    "melee/it/kinds/itkirbyyoshispecialn",
    "melee/it/kinds/itlucky",
    "itStar_UnkMotion0_Phys",
    "it_80284E10",
    "melee/it/kinds/ittarucann",
    "melee/ft/kinds/ftCommon/ftCo_Barrel",
    "melee/ft/kinds/ftCommon/ftCo_WarpStar",
];

/// What an items-on match reaches that isn't ported, task by task: nothing left.
pub const MISSING: &[ItemGap] = &[];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::fighter_audit::tests::held;
    use crate::game::roster::tests::{game_modules, ledger, stops};

    /// The items-on audit is held to the ledger and the code as the stages' audits are: every
    /// function of its units is ported, replaced or dead, or the audit names it (by itself, or
    /// by its unit when no entry names it alone) as unreached, a fighter's, ported awaiting a
    /// check, or covered by a gap; every stop in the item modules is unreached, a fighter's or
    /// a gap; every item unit with unported functions is held; and every name still names
    /// something unported or a stop. So a port that lands has to update the audit.
    #[test]
    fn items_audit_reaches_nothing_unaccounted() {
        let ledger = ledger();
        let every_stop: Vec<String> = game_modules().iter().flat_map(|m| stops(m)).collect();
        let mut keys: Vec<(&str, &str)> = Vec::new();
        keys.extend(UNREACHED.iter().map(|(k, _)| (*k, "UNREACHED")));
        keys.extend(UNCHECKED.iter().map(|k| (*k, "UNCHECKED")));
        for g in MISSING {
            keys.extend(g.covers.iter().map(|k| (*k, "MISSING")));
        }
        let mut used = vec![false; keys.len()];
        // The characters' own items are the fighter audit's (`fighter_audit`).
        let (fighter_units, fighter_modules) = held();
        for (name, unit, status) in &ledger {
            if status != "unaccounted" || fighter_units.contains(unit) {
                continue;
            }
            let held = UNITS.contains(&unit.as_str());
            assert!(
                held || !unit.starts_with("melee/it/"),
                "{unit} has unported functions ({name}), and the items audit doesn't hold it"
            );
            if !held {
                continue;
            }
            let named = |key: &str| -> Vec<usize> {
                (0..keys.len()).filter(|&i| keys[i].0 == key).collect()
            };
            let mut hits = named(name);
            if hits.is_empty() {
                hits = named(unit);
            }
            assert!(
                !hits.is_empty(),
                "{name} ({unit}) isn't ported, and the items audit doesn't say why an items-on match can't reach it"
            );
            for i in hits {
                used[i] = true;
            }
        }
        for m in MODULES.iter().filter(|m| !fighter_modules.contains(**m)) {
            for stop in stops(m) {
                let hits: Vec<usize> = (0..keys.len())
                    .filter(|&i| keys[i].1 != "UNCHECKED" && stop.contains(keys[i].0))
                    .collect();
                assert!(
                    !hits.is_empty(),
                    "game/{m}.rs stops, and the items audit doesn't say why an items-on match can't reach it: {stop}"
                );
                for i in hits {
                    used[i] = true;
                }
            }
        }
        let mut unused = Vec::new();
        for (i, &(key, list)) in keys.iter().enumerate() {
            // A gap may also name a stop outside the item modules, or a fighter's function.
            let elsewhere = list == "MISSING"
                && (every_stop.iter().any(|t| t.contains(key))
                    || (ledger.iter())
                        .any(|(n, u, st)| st == "unaccounted" && (n == key || u == key)));
            if !used[i] && !elsewhere {
                unused.push(format!(
                    "the items audit's {list} names {key}, which no longer stops or is unported"
                ));
            }
        }
        assert!(unused.is_empty(), "{}", unused.join("\n"));
        let modules = game_modules();
        for m in modules.iter().filter(|m| m.starts_with("it/")) {
            assert!(
                MODULES.contains(&m.as_str()),
                "game/{m}.rs isn't among the items audit's modules"
            );
        }
        for m in MODULES {
            assert!(
                modules.iter().any(|n| n == m),
                "the items audit's module {m} is gone"
            );
        }
    }

    /// The gaps a match reaches follow its item switch, characters and stage.
    #[test]
    fn items_gaps_follow_the_match() {
        assert!(missing_with(&[], &[2, 9], 31).is_empty());
        let balls = missing_with(&[0x22], &[2, 9], 31);
        assert!(
            (balls.iter()).all(|g| g.kinds.is_empty() || g.kinds.contains(&0x22)),
            "a Poké Ball match reaches only its own gaps and every match's"
        );
        // The Ice Climbers' Nana reaches nothing unported with items.
        assert!(missing_with(&[0], &[14], 31).is_empty());
        // A gap with item kinds follows them: a match with one of its kinds (and its characters
        // and stage) reaches it, one with another kind doesn't.
        for g in MISSING.iter().filter(|g| !g.kinds.is_empty()) {
            let chars = if g.with.is_empty() {
                vec![2]
            } else {
                g.with.to_vec()
            };
            let stage = g.stages.first().copied().unwrap_or(31);
            assert!(missing_with(&g.kinds[..1], &chars, stage).contains(&g));
            let other = (0..0x23)
                .find(|k| !g.kinds.contains(k))
                .expect("another kind");
            assert!(!missing_with(&[other], &chars, stage).contains(&g));
        }
    }

    /// A match from the menus has the containers the item switch doesn't list on with any kind
    /// it does, so it reaches their gaps too.
    #[test]
    fn the_switch_brings_its_containers() {
        assert_eq!(unswitched_kinds(), vec![0, 1, 2, 3]);
        assert!(missing_with_switch(&[], &[2, 9], 31).is_empty());
        for kind in [0x06, 0x22] {
            let gaps = missing_with_switch(&[kind], &[2, 9], 31);
            for g in MISSING {
                let reached = (g.kinds.is_empty() || g.kinds.iter().any(|k| *k == kind || *k < 4))
                    && g.with.is_empty()
                    && g.stages.is_empty();
                assert_eq!(gaps.contains(&g), reached, "{kind:#x}: {}", g.what);
            }
        }
    }
}
