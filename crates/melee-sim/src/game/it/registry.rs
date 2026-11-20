//! The item registry: one [`ItemKind`] entry per item kind ([`ItKind`], the game's own number),
//! holding what names the kind: its name, its module's callbacks for the callback table (the
//! `#[callbacks]` dispatch functions its logic table's and state tables' callbacks are in) and
//! the member of the item variables' union its code uses. Its tables are the executable's
//! ([`super::tables`]), indexed by the number and read through the enum ([`ItKind::logic`],
//! [`ItKind::render`]).
//!
//! The game numbers its items once (`ItemKind`, `It_Kind_*`: the common items, the monsters and
//! the characters' items, the Pokémon and their items, the stages' items), so there is one enum.
//! Every kind has an entry, since every kind has its tables in the executable; a kind no port
//! runs has no dispatch functions.
//!
//! A new item's port writes its module (its callbacks' `#[callbacks]` dispatch, `run_event` and
//! `run_pred`) and fills its entry here; nothing else lists items. The state keeps the game's
//! numbers (`Item::kind`), so it and the oracle are unchanged; code reads them through the enum
//! (`item.kind == ItKind::Capsule`, `match item.item_kind() { ItKind::Heart => .. }`,
//! `ItKind::Capsule.id()`).

use super::super::code::{Entries, kind};
use super::kinds::{
    arwinglaser, bat, bombhei, capsule, cerebi, chicorita, climbersblizzard, climbersice,
    climbersstring, clinkmilk, coin, crate_box, crazyhandbomb, dosei, drmariopill, egg, entei,
    evyoshiegg, fflower, fire, flipper, flower, foods, foxblaster, foxillusion, foxlaser, freeze,
    freezer, fushigibana, gamewatchbreath, gamewatchchef, gamewatchfire, gamewatchgreenhouse,
    gamewatchjudge, gamewatchmanhole, gamewatchpanic, gamewatchparachute, gamewatchrescue,
    gamewatchturtle, greatfoxlaser, gshell, hammer, hammerhead, harisen, hassam, heart, heiho,
    hinoarashi, hitodeman, houou, kabigon, kamex, kinoko, kirbycopystar, kirbycutterbeam,
    kirbygamewatchchefpan, kirbyhammer, kirbyinhale, kirbyspitstar, kireihana, klap, koopaflame,
    kusudama, kyasarin, kyasarinegg, leadead, lgun, lgunbeam, lgunray, likelike, linkarrow,
    linkbomb, linkboomerang, linkbow, linkhookshot, lipstick, lipstickspore, lizardon, lucky,
    lugia, luigifireball, maril, mariocape, mariofireball, marumine, masterhandbullet,
    masterhandlaser, matadogas, mato, mball, metalb, metamon, mew, mewtwodisable, mewtwoshadowball,
    msbomb, nessbat, nesspkfire, nesspkfirepillar, nesspkflash, nesspkflashexplode, nesspkthunder,
    nesspkthundertrail, nessyoyo, nokonoko, octarock, octarockstone, oldkuri, oldottosea, parasol,
    patapata, peachexplode, peachparasol, peachtoad, peachtoadspore, peachturnip, pikachuthunder,
    pikachutjoltair, pikachutjoltground, pippi, porygon2, rabbitc, raikou, rshell, samusbomb,
    samuschargeshot, samusgrapple, samusmissile, scball, seakchain, seakneedleheld,
    seakneedlethrown, seakvanish, sonans, spycloak, sscope, sscopebeam, star, starrod, starrodstar,
    suikun, sword, taru, tarucann, thunder, tincle, togepy, tomato, tools, tosakinto, trophycoin,
    unknown, whispyapple, whitebea, wstar, yaku, yoshiegglay, yoshieggthrow, yoshistar,
    yoshitongue, zeldadinfire, zeldadinfireexplode, zgshell,
};
use super::tables::{self, ItemLogic};
use super::{VarsKind, pokemon, zako};
use crate::game::handles::CodeRef;

/// An item kind (`ItemKind`, `It_Kind_*`): the game's own numbers, which the state, the
/// replays and the executable's tables use; the index of [`ITEMS`].
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ItKind {
    /// `It_Kind_Capsule`: the Capsule.
    Capsule = 0,
    /// `It_Kind_Box`: the Crate.
    Box = 1,
    /// `It_Kind_Taru`: the Barrel.
    Taru = 2,
    /// `It_Kind_Egg`: the Egg.
    Egg = 3,
    /// `It_Kind_Kusudama`: the Party Ball.
    Kusudama = 4,
    /// `It_Kind_TaruCann`: the Barrel Cannon.
    TaruCann = 5,
    /// `It_Kind_BombHei`: the Bob-omb.
    BombHei = 6,
    /// `It_Kind_Dosei`: Mr. Saturn.
    Dosei = 7,
    /// `It_Kind_Heart`: the Heart Container.
    Heart = 8,
    /// `It_Kind_Tomato`: the Maxim Tomato.
    Tomato = 9,
    /// `It_Kind_Star`: the Starman.
    Star = 10,
    /// `It_Kind_Bat`: the Home-Run Bat.
    Bat = 11,
    /// `It_Kind_Sword`: the Beam Sword.
    Sword = 12,
    /// `It_Kind_Parasol`: the Parasol.
    Parasol = 13,
    /// `It_Kind_G_Shell`: the Green Shell.
    GShell = 14,
    /// `It_Kind_R_Shell`: the Red Shell.
    RShell = 15,
    /// `It_Kind_L_Gun`: the Ray Gun.
    LGun = 16,
    /// `It_Kind_Freeze`: the Freezie.
    Freeze = 17,
    /// `It_Kind_Foods`: the food.
    Foods = 18,
    /// `It_Kind_MSBomb`: the Motion-Sensor Bomb.
    MSBomb = 19,
    /// `It_Kind_Flipper`: the Flipper.
    Flipper = 20,
    /// `It_Kind_S_Scope`: the Super Scope.
    SScope = 21,
    /// `It_Kind_StarRod`: the Star Rod.
    StarRod = 22,
    /// `It_Kind_LipStick`: the Lip's Stick.
    LipStick = 23,
    /// `It_Kind_Harisen`: the Fan.
    Harisen = 24,
    /// `It_Kind_F_Flower`: the Fire Flower.
    FFlower = 25,
    /// `It_Kind_Kinoko`: the Super Mushroom.
    Kinoko = 26,
    /// `It_Kind_DKinoko`: the Poison Mushroom.
    DKinoko = 27,
    /// `It_Kind_Hammer`: the Hammer.
    Hammer = 28,
    /// `It_Kind_WStar`: the Warp Star.
    WStar = 29,
    /// `It_Kind_ScBall`: the Screw Attack.
    ScBall = 30,
    /// `It_Kind_RabbitC`: the Bunny Hood.
    RabbitC = 31,
    /// `It_Kind_MetalB`: the Metal Box.
    MetalB = 32,
    /// `It_Kind_Spycloak`: the Cloaking Device.
    Spycloak = 33,
    /// `It_Kind_M_Ball`: the Poké Ball.
    MBall = 34,
    /// `It_Kind_L_Gun_Ray`: the Ray Gun's shot.
    LGunRay = 35,
    /// `It_Kind_StarRod_Star`: the Star Rod's star.
    StarRodStar = 36,
    /// `It_Kind_LipStick_Spore`: the Lip's Stick's spore.
    LipStickSpore = 37,
    /// `It_Kind_S_Scope_Beam`: the Super Scope's shot.
    SScopeBeam = 38,
    /// `It_Kind_L_Gun_Beam`: the Fire Flower's flame.
    LGunBeam = 39,
    /// `It_Kind_Hammer_Head`: the Hammer's head.
    HammerHead = 40,
    /// `It_Kind_F_Flower_Flame`: the flower the Lip's Stick plants.
    FFlowerFlame = 41,
    /// `It_Kind_EvYoshiEgg`: Event 13's egg.
    EvYoshiEgg = 42,
    /// `It_Kind_Kuriboh`: the Goomba.
    Kuriboh = 43,
    /// `It_Kind_Leadead`: the ReDead.
    Leadead = 44,
    /// `It_Kind_Octarock`: the Octorok.
    Octarock = 45,
    /// `It_Kind_Ottosea`: the Ottosea.
    Ottosea = 46,
    /// `It_Kind_Octarock_Stone`: the Octorok's stone.
    OctarockStone = 47,
    /// `It_Kind_Mario_Fire`: Mario's fireball.
    MarioFire = 48,
    /// `It_Kind_DrMario_Vitamin`: Dr. Mario's pill.
    DrMarioVitamin = 49,
    /// `It_Kind_Kirby_CBeam`: Kirby's Final Cutter wave.
    KirbyCBeam = 50,
    /// `It_Kind_Kirby_Hammer`: Kirby's hammer.
    KirbyHammer = 51,
    /// `It_Kind_Unk1`: the star of Kirby's lost copy.
    Unk1 = 52,
    /// `It_Kind_Unk2`: the star Kirby spits.
    Unk2 = 53,
    /// `It_Kind_Fox_Laser`: Fox's laser.
    FoxLaser = 54,
    /// `It_Kind_Falco_Laser`: Falco's laser.
    FalcoLaser = 55,
    /// `It_Kind_Fox_Illusion`: Fox's Illusion.
    FoxIllusion = 56,
    /// `It_Kind_Falco_Phantasm`: Falco's Phantasm.
    FalcoPhantasm = 57,
    /// `It_Kind_Link_Bomb`: Link's bomb.
    LinkBomb = 58,
    /// `It_Kind_CLink_Bomb`: Young Link's bomb.
    CLinkBomb = 59,
    /// `It_Kind_Link_Boomerang`: Link's boomerang.
    LinkBoomerang = 60,
    /// `It_Kind_CLink_Boomerang`: Young Link's boomerang.
    CLinkBoomerang = 61,
    /// `It_Kind_Link_HShot`: Link's hookshot.
    LinkHShot = 62,
    /// `It_Kind_CLink_HShot`: Young Link's hookshot.
    CLinkHShot = 63,
    /// `It_Kind_Link_Arrow`: Link's arrow.
    LinkArrow = 64,
    /// `It_Kind_CLink_Arrow`: Young Link's fire arrow.
    CLinkArrow = 65,
    /// `It_Kind_Ness_PKFire`: Ness's PK Fire.
    NessPKFire = 66,
    /// `It_Kind_Ness_PKFire_Flame`: PK Fire's pillar.
    NessPKFireFlame = 67,
    /// `It_Kind_Ness_PKFlush`: Ness's PK Flash, charging.
    NessPKFlush = 68,
    /// `It_Kind_Ness_PKThunder`: Ness's PK Thunder.
    NessPKThunder = 69,
    /// `It_Kind_Ness_PKThunder1`: PK Thunder's first trail.
    NessPKThunder1 = 70,
    /// `It_Kind_Ness_PKThunder2`: PK Thunder's second trail.
    NessPKThunder2 = 71,
    /// `It_Kind_Ness_PKThunder3`: PK Thunder's third trail.
    NessPKThunder3 = 72,
    /// `It_Kind_Ness_PKThunder4`: PK Thunder's fourth trail.
    NessPKThunder4 = 73,
    /// `It_Kind_Fox_Blaster`: Fox's blaster.
    FoxBlaster = 74,
    /// `It_Kind_Falco_Blaster`: Falco's blaster.
    FalcoBlaster = 75,
    /// `It_Kind_Link_Bow`: Link's bow.
    LinkBow = 76,
    /// `It_Kind_CLink_Bow`: Young Link's bow.
    CLinkBow = 77,
    /// `It_Kind_Ness_PKFlush_Explode`: PK Flash's explosion.
    NessPKFlushExplode = 78,
    /// `It_Kind_Seak_NeedleThrow`: Sheik's thrown needle.
    SeakNeedleThrow = 79,
    /// `It_Kind_Seak_NeedleHeld`: Sheik's held needle.
    SeakNeedleHeld = 80,
    /// `It_Kind_Pikachu_Thunder`: Pikachu's Thunder.
    PikachuThunder = 81,
    /// `It_Kind_Pichu_Thunder`: Pichu's Thunder.
    PichuThunder = 82,
    /// `It_Kind_Mario_Cape`: Mario's cape.
    MarioCape = 83,
    /// `It_Kind_DrMario_Sheet`: Dr. Mario's sheet.
    DrMarioSheet = 84,
    /// `It_Kind_Seak_Vanish`: Sheik's Vanish smoke.
    SeakVanish = 85,
    /// `It_Kind_Yoshi_EggThrow`: Yoshi's thrown egg.
    YoshiEggThrow = 86,
    /// `It_Kind_Yoshi_EggLay`: the egg Yoshi lays.
    YoshiEggLay = 87,
    /// `It_Kind_Yoshi_Star`: Yoshi's ground pound's star.
    YoshiStar = 88,
    /// `It_Kind_Pikachu_TJolt_Ground`: Pikachu's Thunder Jolt on the ground.
    PikachuTJoltGround = 89,
    /// `It_Kind_Pikachu_TJolt_Air`: Pikachu's Thunder Jolt in the air.
    PikachuTJoltAir = 90,
    /// `It_Kind_Pichu_TJolt_Ground`: Pichu's Thunder Jolt on the ground.
    PichuTJoltGround = 91,
    /// `It_Kind_Pichu_TJolt_Air`: Pichu's Thunder Jolt in the air.
    PichuTJoltAir = 92,
    /// `It_Kind_Samus_Bomb`: Samus's bomb.
    SamusBomb = 93,
    /// `It_Kind_Samus_Charge`: Samus's Charge Shot.
    SamusCharge = 94,
    /// `It_Kind_Samus_Missile`: Samus's missile.
    SamusMissile = 95,
    /// `It_Kind_Samus_GBeam`: Samus's grapple beam.
    SamusGBeam = 96,
    /// `It_Kind_Seak_Chain`: Sheik's chain.
    SeakChain = 97,
    /// `It_Kind_Peach_Explode`: Peach Bomber's explosion.
    PeachExplode = 98,
    /// `It_Kind_Peach_Turnip`: Peach's turnip.
    PeachTurnip = 99,
    /// `It_Kind_Koopa_Flame`: Bowser's flame.
    KoopaFlame = 100,
    /// `It_Kind_Ness_Bat`: Ness's bat.
    NessBat = 101,
    /// `It_Kind_Ness_Yoyo`: Ness's yo-yo.
    NessYoyo = 102,
    /// `It_Kind_Peach_Parasol`: Peach's parasol.
    PeachParasol = 103,
    /// `It_Kind_Peach_Toad`: Peach's Toad.
    PeachToad = 104,
    /// `It_Kind_Luigi_Fire`: Luigi's fireball.
    LuigiFire = 105,
    /// `It_Kind_IceClimber_Ice`: the Ice Climbers' ice.
    IceClimberIce = 106,
    /// `It_Kind_IceClimber_Blizzard`: the Ice Climbers' Blizzard.
    IceClimberBlizzard = 107,
    /// `It_Kind_Zelda_DinFire`: Zelda's Din's Fire.
    ZeldaDinFire = 108,
    /// `It_Kind_Zelda_DinFire_Explode`: Din's Fire's explosion.
    ZeldaDinFireExplode = 109,
    /// `It_Kind_Mewtwo_Disable`: Mewtwo's Disable.
    MewtwoDisable = 110,
    /// `It_Kind_Peach_ToadSpore`: Toad's spores.
    PeachToadSpore = 111,
    /// `It_Kind_Mewtwo_ShadowBall`: Mewtwo's Shadow Ball.
    MewtwoShadowBall = 112,
    /// `It_Kind_IceClimber_GumStrings`: the Ice Climbers' belay string.
    IceClimberGumStrings = 113,
    /// `It_Kind_GameWatch_Greenhouse`: Mr. Game & Watch's Greenhouse spray.
    GameWatchGreenhouse = 114,
    /// `It_Kind_GameWatch_Manhole`: Mr. Game & Watch's manhole.
    GameWatchManhole = 115,
    /// `It_Kind_GameWatch_Fire`: Mr. Game & Watch's torch.
    GameWatchFire = 116,
    /// `It_Kind_GameWatch_Parachute`: Mr. Game & Watch's parachute.
    GameWatchParachute = 117,
    /// `It_Kind_GameWatch_Turtle`: Mr. Game & Watch's turtle.
    GameWatchTurtle = 118,
    /// `It_Kind_GameWatch_Breath`: Mr. Game & Watch's Sparky.
    GameWatchBreath = 119,
    /// `It_Kind_GameWatch_Judge`: Mr. Game & Watch's Judge.
    GameWatchJudge = 120,
    /// `It_Kind_GameWatch_Panic`: Mr. Game & Watch's Oil Panic.
    GameWatchPanic = 121,
    /// `It_Kind_GameWatch_Chef`: Mr. Game & Watch's sausage.
    GameWatchChef = 122,
    /// `It_Kind_CLink_Milk`: Young Link's milk.
    CLinkMilk = 123,
    /// `It_Kind_GameWatch_Rescue`: Mr. Game & Watch's Fire rescue.
    GameWatchRescue = 124,
    /// `It_Kind_MasterHand_Laser`: Master Hand's laser.
    MasterHandLaser = 125,
    /// `It_Kind_MasterHand_Bullet`: Master Hand's bullet.
    MasterHandBullet = 126,
    /// `It_Kind_CrazyHand_Laser`: Crazy Hand's laser.
    CrazyHandLaser = 127,
    /// `It_Kind_CrazyHand_Bullet`: Crazy Hand's bullet.
    CrazyHandBullet = 128,
    /// `It_Kind_CrazyHand_Bomb`: Crazy Hand's bomb.
    CrazyHandBomb = 129,
    /// `It_Kind_Kirby_MarioFire`: Kirby's copy of Mario's fireball.
    KirbyMarioFire = 130,
    /// `It_Kind_Kirby_DrMarioVitamin`: Kirby's copy of Dr. Mario's pill.
    KirbyDrMarioVitamin = 131,
    /// `It_Kind_Kirby_LuigiFire`: Kirby's copy of Luigi's fireball.
    KirbyLuigiFire = 132,
    /// `It_Kind_Kirby_IceClimberIce`: Kirby's copy of the Ice Climbers' ice.
    KirbyIceClimberIce = 133,
    /// `It_Kind_Kirby_PeachToad`: Kirby's copy of Peach's Toad.
    KirbyPeachToad = 134,
    /// `It_Kind_Kirby_PeachToadSpore`: Kirby's copy of Toad's spores.
    KirbyPeachToadSpore = 135,
    /// `It_Kind_Kirby_FoxLaser`: Kirby's copy of Fox's laser.
    KirbyFoxLaser = 136,
    /// `It_Kind_Kirby_FalcoLaser`: Kirby's copy of Falco's laser.
    KirbyFalcoLaser = 137,
    /// `It_Kind_Kirby_FoxBlaster`: Kirby's copy of Fox's blaster.
    KirbyFoxBlaster = 138,
    /// `It_Kind_Kirby_FalcoBlaster`: Kirby's copy of Falco's blaster.
    KirbyFalcoBlaster = 139,
    /// `It_Kind_Kirby_LinkArrow`: Kirby's copy of Link's arrow.
    KirbyLinkArrow = 140,
    /// `It_Kind_Kirby_CLinkArrow`: Kirby's copy of Young Link's arrow.
    KirbyCLinkArrow = 141,
    /// `It_Kind_Kirby_LinkBow`: Kirby's copy of Link's bow.
    KirbyLinkBow = 142,
    /// `It_Kind_Kirby_CLinkBow`: Kirby's copy of Young Link's bow.
    KirbyCLinkBow = 143,
    /// `It_Kind_Kirby_MewtwoShadowBall`: Kirby's copy of Mewtwo's Shadow Ball.
    KirbyMewtwoShadowBall = 144,
    /// `It_Kind_Kirby_NessPKFlush`: Kirby's copy of PK Flash.
    KirbyNessPKFlush = 145,
    /// `It_Kind_Kirby_NessPKFlush_Explode`: Kirby's copy of PK Flash's explosion.
    KirbyNessPKFlushExplode = 146,
    /// `It_Kind_Kirby_PikachuTJolt_Ground`: Kirby's copy of Pikachu's Thunder Jolt on the ground.
    KirbyPikachuTJoltGround = 147,
    /// `It_Kind_Kirby_PikachuTJolt_Air`: Kirby's copy of Pikachu's Thunder Jolt in the air.
    KirbyPikachuTJoltAir = 148,
    /// `It_Kind_Kirby_PichuTJolt_Ground`: Kirby's copy of Pichu's Thunder Jolt on the ground.
    KirbyPichuTJoltGround = 149,
    /// `It_Kind_Kirby_PichuTJolt_Air`: Kirby's copy of Pichu's Thunder Jolt in the air.
    KirbyPichuTJoltAir = 150,
    /// `It_Kind_Kirby_SamusCharge`: Kirby's copy of Samus's Charge Shot.
    KirbySamusCharge = 151,
    /// `It_Kind_Kirby_SeakNeedleThrow`: Kirby's copy of Sheik's thrown needle.
    KirbySeakNeedleThrow = 152,
    /// `It_Kind_Kirby_SeakNeedleHeld`: Kirby's copy of Sheik's held needle.
    KirbySeakNeedleHeld = 153,
    /// `It_Kind_Kirby_KoopaFlame`: Kirby's copy of Bowser's flame.
    KirbyKoopaFlame = 154,
    /// `It_Kind_Kirby_GameWatchChef`: Kirby's copy of Mr. Game & Watch's sausage.
    KirbyGameWatchChef = 155,
    /// `It_Kind_Kirby_GameWatchChefPan`: Kirby's copy of Mr. Game & Watch's pan.
    KirbyGameWatchChefPan = 156,
    /// `It_Kind_Kirby_YoshiEggLay`: Kirby's copy of Yoshi's laid egg.
    KirbyYoshiEggLay = 157,
    /// `It_Kind_Unk4`: the single-player coin.
    Unk4 = 158,
    /// `It_Kind_Coin`: the trophy coin.
    Coin = 159,
    /// The stages' hurtbox items (`It_PKind_Random` in the decompilation, the number the Poké
    /// Ball's random Pokémon would take; every use here is the stages' items).
    StageHurtbox = 160,
    /// `It_PKind_Tosakinto`: Goldeen.
    Tosakinto = 161,
    /// `It_PKind_Chicorita`: Chikorita.
    Chicorita = 162,
    /// `It_PKind_Kabigon`: Snorlax.
    Kabigon = 163,
    /// `It_PKind_Kamex`: Blastoise.
    Kamex = 164,
    /// `It_PKind_Matadogas`: Weezing.
    Matadogas = 165,
    /// `It_PKind_Lizardon`: Charizard.
    Lizardon = 166,
    /// `It_PKind_Fire`: Moltres.
    Fire = 167,
    /// `It_PKind_Thunder`: Zapdos.
    Thunder = 168,
    /// `It_PKind_Freezer`: Articuno.
    Freezer = 169,
    /// `It_PKind_Sonans`: Wobbuffet.
    Sonans = 170,
    /// `It_PKind_Hassam`: Scizor.
    Hassam = 171,
    /// `It_PKind_Unknown`: Unown.
    Unknown = 172,
    /// `It_PKind_Entei`: Entei.
    Entei = 173,
    /// `It_PKind_Raikou`: Raikou.
    Raikou = 174,
    /// `It_PKind_Suikun`: Suicune.
    Suikun = 175,
    /// `It_PKind_Kireihana`: Bellossom.
    Kireihana = 176,
    /// `It_PKind_Marumine`: Electrode.
    Marumine = 177,
    /// `It_PKind_Lugia`: Lugia.
    Lugia = 178,
    /// `It_PKind_Houou`: Ho-Oh.
    Houou = 179,
    /// `It_PKind_Metamon`: Ditto.
    Metamon = 180,
    /// `It_PKind_Pippi`: Clefairy.
    Pippi = 181,
    /// `It_PKind_Togepy`: Togepi.
    Togepy = 182,
    /// `It_PKind_Mew`: Mew.
    Mew = 183,
    /// `It_PKind_Cerebi`: Celebi.
    Cerebi = 184,
    /// `It_PKind_Hitodeman`: Staryu.
    Hitodeman = 185,
    /// `It_PKind_Lucky`: Chansey.
    Lucky = 186,
    /// `It_PKind_Porygon2`: Porygon2.
    Porygon2 = 187,
    /// `It_PKind_Hinoarashi`: Cyndaquil.
    Hinoarashi = 188,
    /// `It_PKind_Maril`: Marill.
    Maril = 189,
    /// `It_PKind_Fushigibana`: Venusaur.
    Fushigibana = 190,
    /// `It_Kind_Chicorita_Leaf`: Chikorita's leaf.
    ChicoritaLeaf = 191,
    /// `It_Kind_Kamex_HydroPump`: Blastoise's Hydro Pump.
    KamexHydroPump = 192,
    /// `It_Kind_Matadogas_Gas1`: Weezing's first gas.
    MatadogasGas1 = 193,
    /// `It_Kind_Matadogas_Gas2`: Weezing's second gas.
    MatadogasGas2 = 194,
    /// `It_Kind_Lizardon_Flame1`: Charizard's first flame.
    LizardonFlame1 = 195,
    /// `It_Kind_Lizardon_Flame2`: Charizard's second flame.
    LizardonFlame2 = 196,
    /// `It_Kind_Lizardon_Flame3`: Charizard's third flame.
    LizardonFlame3 = 197,
    /// `It_Kind_Lizardon_Flame4`: Charizard's fourth flame.
    LizardonFlame4 = 198,
    /// `It_Kind_Unknown_Swarm`: Unown's little Unown.
    UnknownSwarm = 199,
    /// `It_Kind_Lugia_Aeroblast`: Lugia's Aeroblast.
    LugiaAeroblast = 200,
    /// `It_Kind_Lugia_Aeroblast2`: Lugia's second Aeroblast.
    LugiaAeroblast2 = 201,
    /// `It_Kind_Lugia_Aeroblast3`: Lugia's third Aeroblast.
    LugiaAeroblast3 = 202,
    /// `It_Kind_Houou_SacredFire`: Ho-Oh's Sacred Fire.
    HououSacredFire = 203,
    /// `It_Kind_Hitodeman_Star`: Staryu's star.
    HitodemanStar = 204,
    /// `It_Kind_Lucky_Egg`: Chansey's egg.
    LuckyEgg = 205,
    /// `It_Kind_Hinoarashi_Flame`: Cyndaquil's flame.
    HinoarashiFlame = 206,
    /// `It_Kind_Pokemon_Unk`: a Pokémon's item of unknown use.
    PokemonUnk = 207,
    /// `It_Kind_Old_Kuri`: the old Goomba.
    OldKuri = 208,
    /// `It_Kind_Mato`: Target Test's targets.
    Mato = 209,
    /// `It_Kind_Heiho`: Yoshi's Story's Shy Guys.
    Heiho = 210,
    /// `It_Kind_Nokonoko`: the Koopa Troopa.
    Nokonoko = 211,
    /// `It_Kind_Patapata`: the Koopa Paratroopa.
    Patapata = 212,
    /// `It_Kind_Likelike`: the Like Like.
    Likelike = 213,
    /// `It_Kind_Old_Lead`: the old ReDead (unused).
    OldLead = 214,
    /// `It_Kind_Old_Octa`: the old Octorok (unused).
    OldOcta = 215,
    /// `It_Kind_Old_Otto`: the old Ottosea.
    OldOtto = 216,
    /// `It_Kind_Whitebea`: the Polar Bear.
    Whitebea = 217,
    /// `It_Kind_Klap`: Kongo Jungle's Klap Trap.
    Klap = 218,
    /// `It_Kind_ZGShell`: the Koopas' green shell.
    ZGShell = 219,
    /// `It_Kind_ZRShell`: the Koopas' red shell.
    ZRShell = 220,
    /// `It_Kind_Tincle`: Great Bay's Tingle.
    Tincle = 221,
    /// `It_Kind_Invalid1`: unused.
    Invalid1 = 222,
    /// `It_Kind_Invalid2`: unused.
    Invalid2 = 223,
    /// `It_Kind_Invalid3`: unused.
    Invalid3 = 224,
    /// `It_Kind_WhispyApple`: Green Greens' apple.
    WhispyApple = 225,
    /// `It_Kind_WhispyHealApple`: Green Greens' healing apple.
    WhispyHealApple = 226,
    /// `It_Kind_Invalid4`: unused.
    Invalid4 = 227,
    /// `It_Kind_Invalid5`: unused.
    Invalid5 = 228,
    /// `It_Kind_Invalid6`: unused.
    Invalid6 = 229,
    /// `It_Kind_Tools`: Flat Zone's tools.
    Tools = 230,
    /// `It_Kind_Invalid7`: unused.
    Invalid7 = 231,
    /// `It_Kind_Invalid8`: unused.
    Invalid8 = 232,
    /// `It_Kind_Kyasarin`: Birdo.
    Kyasarin = 233,
    /// `It_Kind_Arwing_Laser`: Corneria's Arwings' laser.
    ArwingLaser = 234,
    /// `It_Kind_GreatFox_Laser`: the Great Fox's laser.
    GreatFoxLaser = 235,
    /// `It_Kind_Kyasarin_Egg`: Birdo's egg.
    KyasarinEgg = 236,
}

impl ItKind {
    /// How many there are.
    pub const COUNT: usize = 237;

    /// Every item kind, by number.
    pub const ALL: [ItKind; Self::COUNT] = [
        ItKind::Capsule,
        ItKind::Box,
        ItKind::Taru,
        ItKind::Egg,
        ItKind::Kusudama,
        ItKind::TaruCann,
        ItKind::BombHei,
        ItKind::Dosei,
        ItKind::Heart,
        ItKind::Tomato,
        ItKind::Star,
        ItKind::Bat,
        ItKind::Sword,
        ItKind::Parasol,
        ItKind::GShell,
        ItKind::RShell,
        ItKind::LGun,
        ItKind::Freeze,
        ItKind::Foods,
        ItKind::MSBomb,
        ItKind::Flipper,
        ItKind::SScope,
        ItKind::StarRod,
        ItKind::LipStick,
        ItKind::Harisen,
        ItKind::FFlower,
        ItKind::Kinoko,
        ItKind::DKinoko,
        ItKind::Hammer,
        ItKind::WStar,
        ItKind::ScBall,
        ItKind::RabbitC,
        ItKind::MetalB,
        ItKind::Spycloak,
        ItKind::MBall,
        ItKind::LGunRay,
        ItKind::StarRodStar,
        ItKind::LipStickSpore,
        ItKind::SScopeBeam,
        ItKind::LGunBeam,
        ItKind::HammerHead,
        ItKind::FFlowerFlame,
        ItKind::EvYoshiEgg,
        ItKind::Kuriboh,
        ItKind::Leadead,
        ItKind::Octarock,
        ItKind::Ottosea,
        ItKind::OctarockStone,
        ItKind::MarioFire,
        ItKind::DrMarioVitamin,
        ItKind::KirbyCBeam,
        ItKind::KirbyHammer,
        ItKind::Unk1,
        ItKind::Unk2,
        ItKind::FoxLaser,
        ItKind::FalcoLaser,
        ItKind::FoxIllusion,
        ItKind::FalcoPhantasm,
        ItKind::LinkBomb,
        ItKind::CLinkBomb,
        ItKind::LinkBoomerang,
        ItKind::CLinkBoomerang,
        ItKind::LinkHShot,
        ItKind::CLinkHShot,
        ItKind::LinkArrow,
        ItKind::CLinkArrow,
        ItKind::NessPKFire,
        ItKind::NessPKFireFlame,
        ItKind::NessPKFlush,
        ItKind::NessPKThunder,
        ItKind::NessPKThunder1,
        ItKind::NessPKThunder2,
        ItKind::NessPKThunder3,
        ItKind::NessPKThunder4,
        ItKind::FoxBlaster,
        ItKind::FalcoBlaster,
        ItKind::LinkBow,
        ItKind::CLinkBow,
        ItKind::NessPKFlushExplode,
        ItKind::SeakNeedleThrow,
        ItKind::SeakNeedleHeld,
        ItKind::PikachuThunder,
        ItKind::PichuThunder,
        ItKind::MarioCape,
        ItKind::DrMarioSheet,
        ItKind::SeakVanish,
        ItKind::YoshiEggThrow,
        ItKind::YoshiEggLay,
        ItKind::YoshiStar,
        ItKind::PikachuTJoltGround,
        ItKind::PikachuTJoltAir,
        ItKind::PichuTJoltGround,
        ItKind::PichuTJoltAir,
        ItKind::SamusBomb,
        ItKind::SamusCharge,
        ItKind::SamusMissile,
        ItKind::SamusGBeam,
        ItKind::SeakChain,
        ItKind::PeachExplode,
        ItKind::PeachTurnip,
        ItKind::KoopaFlame,
        ItKind::NessBat,
        ItKind::NessYoyo,
        ItKind::PeachParasol,
        ItKind::PeachToad,
        ItKind::LuigiFire,
        ItKind::IceClimberIce,
        ItKind::IceClimberBlizzard,
        ItKind::ZeldaDinFire,
        ItKind::ZeldaDinFireExplode,
        ItKind::MewtwoDisable,
        ItKind::PeachToadSpore,
        ItKind::MewtwoShadowBall,
        ItKind::IceClimberGumStrings,
        ItKind::GameWatchGreenhouse,
        ItKind::GameWatchManhole,
        ItKind::GameWatchFire,
        ItKind::GameWatchParachute,
        ItKind::GameWatchTurtle,
        ItKind::GameWatchBreath,
        ItKind::GameWatchJudge,
        ItKind::GameWatchPanic,
        ItKind::GameWatchChef,
        ItKind::CLinkMilk,
        ItKind::GameWatchRescue,
        ItKind::MasterHandLaser,
        ItKind::MasterHandBullet,
        ItKind::CrazyHandLaser,
        ItKind::CrazyHandBullet,
        ItKind::CrazyHandBomb,
        ItKind::KirbyMarioFire,
        ItKind::KirbyDrMarioVitamin,
        ItKind::KirbyLuigiFire,
        ItKind::KirbyIceClimberIce,
        ItKind::KirbyPeachToad,
        ItKind::KirbyPeachToadSpore,
        ItKind::KirbyFoxLaser,
        ItKind::KirbyFalcoLaser,
        ItKind::KirbyFoxBlaster,
        ItKind::KirbyFalcoBlaster,
        ItKind::KirbyLinkArrow,
        ItKind::KirbyCLinkArrow,
        ItKind::KirbyLinkBow,
        ItKind::KirbyCLinkBow,
        ItKind::KirbyMewtwoShadowBall,
        ItKind::KirbyNessPKFlush,
        ItKind::KirbyNessPKFlushExplode,
        ItKind::KirbyPikachuTJoltGround,
        ItKind::KirbyPikachuTJoltAir,
        ItKind::KirbyPichuTJoltGround,
        ItKind::KirbyPichuTJoltAir,
        ItKind::KirbySamusCharge,
        ItKind::KirbySeakNeedleThrow,
        ItKind::KirbySeakNeedleHeld,
        ItKind::KirbyKoopaFlame,
        ItKind::KirbyGameWatchChef,
        ItKind::KirbyGameWatchChefPan,
        ItKind::KirbyYoshiEggLay,
        ItKind::Unk4,
        ItKind::Coin,
        ItKind::StageHurtbox,
        ItKind::Tosakinto,
        ItKind::Chicorita,
        ItKind::Kabigon,
        ItKind::Kamex,
        ItKind::Matadogas,
        ItKind::Lizardon,
        ItKind::Fire,
        ItKind::Thunder,
        ItKind::Freezer,
        ItKind::Sonans,
        ItKind::Hassam,
        ItKind::Unknown,
        ItKind::Entei,
        ItKind::Raikou,
        ItKind::Suikun,
        ItKind::Kireihana,
        ItKind::Marumine,
        ItKind::Lugia,
        ItKind::Houou,
        ItKind::Metamon,
        ItKind::Pippi,
        ItKind::Togepy,
        ItKind::Mew,
        ItKind::Cerebi,
        ItKind::Hitodeman,
        ItKind::Lucky,
        ItKind::Porygon2,
        ItKind::Hinoarashi,
        ItKind::Maril,
        ItKind::Fushigibana,
        ItKind::ChicoritaLeaf,
        ItKind::KamexHydroPump,
        ItKind::MatadogasGas1,
        ItKind::MatadogasGas2,
        ItKind::LizardonFlame1,
        ItKind::LizardonFlame2,
        ItKind::LizardonFlame3,
        ItKind::LizardonFlame4,
        ItKind::UnknownSwarm,
        ItKind::LugiaAeroblast,
        ItKind::LugiaAeroblast2,
        ItKind::LugiaAeroblast3,
        ItKind::HououSacredFire,
        ItKind::HitodemanStar,
        ItKind::LuckyEgg,
        ItKind::HinoarashiFlame,
        ItKind::PokemonUnk,
        ItKind::OldKuri,
        ItKind::Mato,
        ItKind::Heiho,
        ItKind::Nokonoko,
        ItKind::Patapata,
        ItKind::Likelike,
        ItKind::OldLead,
        ItKind::OldOcta,
        ItKind::OldOtto,
        ItKind::Whitebea,
        ItKind::Klap,
        ItKind::ZGShell,
        ItKind::ZRShell,
        ItKind::Tincle,
        ItKind::Invalid1,
        ItKind::Invalid2,
        ItKind::Invalid3,
        ItKind::WhispyApple,
        ItKind::WhispyHealApple,
        ItKind::Invalid4,
        ItKind::Invalid5,
        ItKind::Invalid6,
        ItKind::Tools,
        ItKind::Invalid7,
        ItKind::Invalid8,
        ItKind::Kyasarin,
        ItKind::ArwingLaser,
        ItKind::GreatFoxLaser,
        ItKind::KyasarinEgg,
    ];

    /// One past the common items (`It_Common_End`): the random spawner draws the kinds below.
    pub const COMMON_END: ItKind = ItKind::LGunRay;

    /// The first Pokémon (`It_PKind_Start`), and the first kind of the Pokémon's tables.
    pub const FIRST_POKEMON: ItKind = ItKind::Tosakinto;

    /// One past the last Pokémon (`It_PKind_Terminate`), the first of their items.
    pub const POKEMON_END: ItKind = ItKind::ChicoritaLeaf;

    /// The kind the item code takes for none chosen (`It_Kind_Unselected`).
    pub const UNSELECTED: i32 = -1;

    /// The item kind numbered `id`, if the game has one.
    pub const fn from_id(id: i32) -> Option<ItKind> {
        if id >= 0 && (id as usize) < Self::COUNT {
            Some(Self::ALL[id as usize])
        } else {
            None
        }
    }

    /// The kind numbered `id`, a number the game holds as an item's kind: stops if it isn't one.
    pub fn of(id: i32) -> ItKind {
        Self::from_id(id).unwrap_or_else(|| panic!("{id} is no item kind"))
    }

    /// Its number, as the state and the game's tables hold it.
    pub const fn id(self) -> i32 {
        self as i32
    }

    /// Its registry entry.
    pub fn entry(self) -> &'static ItemKind {
        &ITEMS[self as usize]
    }

    /// Its logic table (`it_803F14C4` and its siblings, by `Item_80267978`'s split).
    pub fn logic(self) -> &'static ItemLogic {
        let (g, i) = tables::group(self.id());
        &tables::logic_group(g)[i]
    }

    /// Its render function (`it_803F1418` and its siblings).
    pub fn render(self) -> CodeRef {
        tables::render_func(self.id())
    }

    /// Whether it's one of the Poké Ball's Pokémon.
    pub fn is_pokemon(self) -> bool {
        (Self::FIRST_POKEMON..Self::POKEMON_END).contains(&self)
    }
}

impl PartialEq<ItKind> for i32 {
    fn eq(&self, k: &ItKind) -> bool {
        *self == k.id()
    }
}

impl PartialEq<i32> for ItKind {
    fn eq(&self, id: &i32) -> bool {
        self.id() == *id
    }
}

impl PartialOrd<ItKind> for i32 {
    fn partial_cmp(&self, k: &ItKind) -> Option<std::cmp::Ordering> {
        self.partial_cmp(&k.id())
    }
}

impl super::super::structs::Item {
    /// The item's kind (`kind`), which the game only ever sets to one of its own.
    pub fn item_kind(&self) -> ItKind {
        ItKind::of(self.kind)
    }
}

/// One item kind.
#[derive(Clone, Copy)]
pub struct ItemKind {
    pub kind: ItKind,
    /// Its name.
    pub name: &'static str,
    /// Its module's `#[callbacks(Proc)]` dispatch: the processes and events its logic table and
    /// state tables name, and the processes those start.
    pub events: &'static [fn(&mut Entries<kind::Proc>)],
    /// Its module's `#[callbacks(Pred)]` dispatch: the callbacks of its tables that answer.
    pub preds: &'static [fn(&mut Entries<kind::Pred>)],
    /// Which member of the item variables' union (`Item_ItemVars`) its code uses.
    pub vars: VarsKind,
}

impl std::fmt::Debug for ItemKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ItemKind")
            .field("kind", &self.kind)
            .field("name", &self.name)
            .field("vars", &self.vars)
            .finish_non_exhaustive()
    }
}

impl PartialEq for ItemKind {
    fn eq(&self, o: &Self) -> bool {
        self.kind == o.kind
    }
}

/// The entry of the kind numbered `kind`; stops if the game has no such kind.
pub fn item(kind: i32) -> &'static ItemKind {
    match ItKind::from_id(kind) {
        Some(k) => k.entry(),
        None => panic!("{kind} is no item kind"),
    }
}

/// Which member of the variables' union an item of `kind` uses (bytes for a number that's no
/// kind, as a capture reads it).
pub fn vars_kind(kind: i32) -> VarsKind {
    ItKind::from_id(kind).map_or(VarsKind::Bytes, |k| k.entry().vars)
}

/// The item callbacks no kind's tables name: the states Kirby's inhale and Yoshi's tongue give
/// any item they hold (`it_803F9450` and its siblings), and the effects' hitlag hooks the
/// Pokémon set.
const HELD_EVENTS: [fn(&mut Entries<kind::Proc>); 3] = [
    kirbyinhale::run_event,
    yoshitongue::run_event,
    pokemon::run_event,
];
const HELD_PREDS: [fn(&mut Entries<kind::Pred>); 2] =
    [kirbyinhale::run_pred, yoshitongue::run_pred];

/// The items' entries in the callback table: each kind's dispatch functions (a module several
/// kinds share adds its entries once; the table takes the same arm twice as one).
pub fn register(events: &mut Entries<kind::Proc>, preds: &mut Entries<kind::Pred>) {
    for e in &ITEMS {
        for f in e.events {
            f(events);
        }
        for f in e.preds {
            f(preds);
        }
    }
    for f in HELD_EVENTS {
        f(events);
    }
    for f in HELD_PREDS {
        f(preds);
    }
}

/// Every item kind, by number.
pub static ITEMS: [ItemKind; ItKind::COUNT] = [
    ItemKind {
        kind: ItKind::Capsule,
        name: "Capsule",
        events: &[capsule::run_event],
        preds: &[capsule::run_pred],
        vars: VarsKind::Capsule,
    },
    ItemKind {
        kind: ItKind::Box,
        name: "Crate",
        events: &[crate_box::run_event],
        preds: &[crate_box::run_pred],
        vars: VarsKind::Box,
    },
    ItemKind {
        kind: ItKind::Taru,
        name: "Barrel",
        events: &[taru::run_event],
        preds: &[taru::run_pred],
        vars: VarsKind::Taru,
    },
    ItemKind {
        kind: ItKind::Egg,
        name: "Egg",
        events: &[egg::run_event],
        preds: &[egg::run_pred],
        vars: VarsKind::Egg,
    },
    ItemKind {
        kind: ItKind::Kusudama,
        name: "Party Ball",
        events: &[kusudama::run_event],
        preds: &[kusudama::run_pred],
        vars: VarsKind::Kusudama,
    },
    ItemKind {
        kind: ItKind::TaruCann,
        name: "Barrel Cannon",
        events: &[tarucann::run_event],
        preds: &[tarucann::run_pred],
        vars: VarsKind::TaruCann,
    },
    ItemKind {
        kind: ItKind::BombHei,
        name: "Bob-omb",
        events: &[bombhei::run_event],
        preds: &[bombhei::run_pred],
        vars: VarsKind::BombHei,
    },
    ItemKind {
        kind: ItKind::Dosei,
        name: "Mr. Saturn",
        events: &[dosei::run_event],
        preds: &[dosei::run_pred],
        vars: VarsKind::Dosei,
    },
    ItemKind {
        kind: ItKind::Heart,
        name: "Heart Container",
        events: &[heart::run_event],
        preds: &[heart::run_pred],
        vars: VarsKind::Heart,
    },
    ItemKind {
        kind: ItKind::Tomato,
        name: "Maxim Tomato",
        events: &[tomato::run_event],
        preds: &[tomato::run_pred],
        vars: VarsKind::Tomato,
    },
    ItemKind {
        kind: ItKind::Star,
        name: "Starman",
        events: &[star::run_event],
        preds: &[star::run_pred],
        vars: VarsKind::Star,
    },
    ItemKind {
        kind: ItKind::Bat,
        name: "Home-Run Bat",
        events: &[bat::run_event],
        preds: &[bat::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::Sword,
        name: "Beam Sword",
        events: &[sword::run_event],
        preds: &[sword::run_pred],
        vars: VarsKind::Sword,
    },
    ItemKind {
        kind: ItKind::Parasol,
        name: "Parasol",
        events: &[parasol::run_event],
        preds: &[parasol::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::GShell,
        name: "Green Shell",
        events: &[gshell::run_event],
        preds: &[gshell::run_pred],
        vars: VarsKind::GShell,
    },
    ItemKind {
        kind: ItKind::RShell,
        name: "Red Shell",
        events: &[rshell::run_event],
        preds: &[rshell::run_pred],
        vars: VarsKind::RShell,
    },
    ItemKind {
        kind: ItKind::LGun,
        name: "Ray Gun",
        events: &[lgun::run_event],
        preds: &[lgun::run_pred],
        vars: VarsKind::LGun,
    },
    ItemKind {
        kind: ItKind::Freeze,
        name: "Freezie",
        events: &[freeze::run_event],
        preds: &[freeze::run_pred],
        vars: VarsKind::Freeze,
    },
    ItemKind {
        kind: ItKind::Foods,
        name: "Food",
        events: &[foods::run_event],
        preds: &[foods::run_pred],
        vars: VarsKind::Foods,
    },
    ItemKind {
        kind: ItKind::MSBomb,
        name: "Motion-Sensor Bomb",
        events: &[msbomb::run_event],
        preds: &[msbomb::run_pred],
        vars: VarsKind::MsBomb,
    },
    ItemKind {
        kind: ItKind::Flipper,
        name: "Flipper",
        events: &[flipper::run_event],
        preds: &[flipper::run_pred],
        vars: VarsKind::Flipper,
    },
    ItemKind {
        kind: ItKind::SScope,
        name: "Super Scope",
        events: &[sscope::run_event],
        preds: &[sscope::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::StarRod,
        name: "Star Rod",
        events: &[starrod::run_event],
        preds: &[starrod::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::LipStick,
        name: "Lip's Stick",
        events: &[lipstick::run_event],
        preds: &[lipstick::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::Harisen,
        name: "Fan",
        events: &[harisen::run_event],
        preds: &[harisen::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::FFlower,
        name: "Fire Flower",
        events: &[fflower::run_event],
        preds: &[fflower::run_pred],
        vars: VarsKind::FFlower,
    },
    ItemKind {
        kind: ItKind::Kinoko,
        name: "Super Mushroom",
        events: &[kinoko::run_event],
        preds: &[kinoko::run_pred],
        vars: VarsKind::Kinoko,
    },
    ItemKind {
        kind: ItKind::DKinoko,
        name: "Poison Mushroom",
        events: &[kinoko::run_event],
        preds: &[kinoko::run_pred],
        vars: VarsKind::Kinoko,
    },
    ItemKind {
        kind: ItKind::Hammer,
        name: "Hammer",
        events: &[hammer::run_event],
        preds: &[hammer::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::WStar,
        name: "Warp Star",
        events: &[wstar::run_event],
        preds: &[wstar::run_pred],
        vars: VarsKind::Wstar,
    },
    ItemKind {
        kind: ItKind::ScBall,
        name: "Screw Attack",
        events: &[rabbitc::run_event, scball::run_event],
        preds: &[rabbitc::run_pred, scball::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::RabbitC,
        name: "Bunny Hood",
        events: &[rabbitc::run_event],
        preds: &[rabbitc::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::MetalB,
        name: "Metal Box",
        events: &[metalb::run_event],
        preds: &[metalb::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::Spycloak,
        name: "Cloaking Device",
        events: &[spycloak::run_event],
        preds: &[spycloak::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::MBall,
        name: "Poké Ball",
        events: &[mball::run_event],
        preds: &[mball::run_pred],
        vars: VarsKind::MBall,
    },
    ItemKind {
        kind: ItKind::LGunRay,
        name: "Ray Gun's shot",
        events: &[lgunray::run_event],
        preds: &[lgunray::run_pred],
        vars: VarsKind::FoxLaser,
    },
    ItemKind {
        kind: ItKind::StarRodStar,
        name: "Star Rod's star",
        events: &[starrodstar::run_event],
        preds: &[starrodstar::run_pred],
        vars: VarsKind::StarRodStar,
    },
    ItemKind {
        kind: ItKind::LipStickSpore,
        name: "Lip's Stick's spore",
        events: &[lipstickspore::run_event],
        preds: &[lipstickspore::run_pred],
        vars: VarsKind::LipstickSpore,
    },
    ItemKind {
        kind: ItKind::SScopeBeam,
        name: "Super Scope's shot",
        events: &[sscopebeam::run_event],
        preds: &[sscopebeam::run_pred],
        vars: VarsKind::ScopeBeam,
    },
    ItemKind {
        kind: ItKind::LGunBeam,
        name: "Fire Flower's flame",
        events: &[lgunbeam::run_event],
        preds: &[lgunbeam::run_pred],
        vars: VarsKind::LGunBeam,
    },
    ItemKind {
        kind: ItKind::HammerHead,
        name: "Hammer's head",
        events: &[hammerhead::run_event],
        preds: &[hammerhead::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::FFlowerFlame,
        name: "Flower the Lip's Stick plants",
        events: &[flower::run_event],
        preds: &[flower::run_pred],
        vars: VarsKind::FFlowerFlame,
    },
    ItemKind {
        kind: ItKind::EvYoshiEgg,
        name: "Event 13's egg",
        events: &[evyoshiegg::run_event],
        preds: &[evyoshiegg::run_pred],
        vars: VarsKind::Egg,
    },
    ItemKind {
        kind: ItKind::Kuriboh,
        name: "Goomba",
        events: &[oldkuri::run_event, zako::run_event],
        preds: &[oldkuri::run_pred],
        vars: VarsKind::Zako,
    },
    ItemKind {
        kind: ItKind::Leadead,
        name: "ReDead",
        events: &[leadead::run_event],
        preds: &[leadead::run_pred],
        vars: VarsKind::Zako,
    },
    ItemKind {
        kind: ItKind::Octarock,
        name: "Octorok",
        events: &[octarock::run_event, zako::run_event],
        preds: &[octarock::run_pred],
        vars: VarsKind::Zako,
    },
    ItemKind {
        kind: ItKind::Ottosea,
        name: "Ottosea",
        events: &[oldottosea::run_event],
        preds: &[oldottosea::run_pred],
        vars: VarsKind::Zako,
    },
    ItemKind {
        kind: ItKind::OctarockStone,
        name: "Octorok's stone",
        events: &[octarockstone::run_event],
        preds: &[octarockstone::run_pred],
        vars: VarsKind::Zako,
    },
    ItemKind {
        kind: ItKind::MarioFire,
        name: "Mario's fireball",
        events: &[mariofireball::run_event],
        preds: &[mariofireball::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::DrMarioVitamin,
        name: "Dr. Mario's pill",
        events: &[drmariopill::run_event],
        preds: &[drmariopill::run_pred],
        vars: VarsKind::DrMarioPill,
    },
    ItemKind {
        kind: ItKind::KirbyCBeam,
        name: "Kirby's Final Cutter wave",
        events: &[kirbycutterbeam::run_event],
        preds: &[kirbycutterbeam::run_pred],
        vars: VarsKind::KirbyCutterBeam,
    },
    ItemKind {
        kind: ItKind::KirbyHammer,
        name: "Kirby's hammer",
        events: &[kirbyhammer::run_event],
        preds: &[],
        vars: VarsKind::KirbyHammer,
    },
    ItemKind {
        kind: ItKind::Unk1,
        name: "Star of Kirby's lost copy",
        events: &[kirbycopystar::run_event],
        preds: &[kirbycopystar::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::Unk2,
        name: "Star Kirby spits",
        events: &[kirbyspitstar::run_event],
        preds: &[kirbyspitstar::run_pred],
        vars: VarsKind::KirbySpitStar,
    },
    ItemKind {
        kind: ItKind::FoxLaser,
        name: "Fox's laser",
        events: &[foxlaser::run_event],
        preds: &[foxlaser::run_pred],
        vars: VarsKind::FoxLaser,
    },
    ItemKind {
        kind: ItKind::FalcoLaser,
        name: "Falco's laser",
        events: &[foxlaser::run_event],
        preds: &[foxlaser::run_pred],
        vars: VarsKind::FoxLaser,
    },
    ItemKind {
        kind: ItKind::FoxIllusion,
        name: "Fox's Illusion",
        events: &[foxillusion::run_event],
        preds: &[foxillusion::run_pred],
        vars: VarsKind::FoxIllusion,
    },
    ItemKind {
        kind: ItKind::FalcoPhantasm,
        name: "Falco's Phantasm",
        events: &[foxillusion::run_event],
        preds: &[foxillusion::run_pred],
        vars: VarsKind::FoxIllusion,
    },
    ItemKind {
        kind: ItKind::LinkBomb,
        name: "Link's bomb",
        events: &[linkbomb::run_event],
        preds: &[linkbomb::run_pred],
        vars: VarsKind::LinkBomb,
    },
    ItemKind {
        kind: ItKind::CLinkBomb,
        name: "Young Link's bomb",
        events: &[linkbomb::run_event],
        preds: &[linkbomb::run_pred],
        vars: VarsKind::LinkBomb,
    },
    ItemKind {
        kind: ItKind::LinkBoomerang,
        name: "Link's boomerang",
        events: &[linkboomerang::run_event],
        preds: &[linkboomerang::run_pred],
        vars: VarsKind::LinkBoomerang,
    },
    ItemKind {
        kind: ItKind::CLinkBoomerang,
        name: "Young Link's boomerang",
        events: &[linkboomerang::run_event],
        preds: &[linkboomerang::run_pred],
        vars: VarsKind::LinkBoomerang,
    },
    ItemKind {
        kind: ItKind::LinkHShot,
        name: "Link's hookshot",
        events: &[linkhookshot::run_event],
        preds: &[linkhookshot::run_pred],
        vars: VarsKind::LinkHookshot,
    },
    ItemKind {
        kind: ItKind::CLinkHShot,
        name: "Young Link's hookshot",
        events: &[linkhookshot::run_event],
        preds: &[linkhookshot::run_pred],
        vars: VarsKind::LinkHookshot,
    },
    ItemKind {
        kind: ItKind::LinkArrow,
        name: "Link's arrow",
        events: &[linkarrow::run_event],
        preds: &[linkarrow::run_pred],
        vars: VarsKind::LinkArrow,
    },
    ItemKind {
        kind: ItKind::CLinkArrow,
        name: "Young Link's fire arrow",
        events: &[linkarrow::run_event],
        preds: &[linkarrow::run_pred],
        vars: VarsKind::LinkArrow,
    },
    ItemKind {
        kind: ItKind::NessPKFire,
        name: "Ness's PK Fire",
        events: &[nesspkfire::run_event],
        preds: &[nesspkfire::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::NessPKFireFlame,
        name: "PK Fire's pillar",
        events: &[nesspkfirepillar::run_event],
        preds: &[nesspkfirepillar::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::NessPKFlush,
        name: "Ness's PK Flash, charging",
        events: &[nesspkflash::run_event],
        preds: &[nesspkflash::run_pred],
        vars: VarsKind::PKFlush,
    },
    ItemKind {
        kind: ItKind::NessPKThunder,
        name: "Ness's PK Thunder",
        events: &[nesspkthunder::run_event],
        preds: &[nesspkthunder::run_pred],
        vars: VarsKind::PKThunder,
    },
    ItemKind {
        kind: ItKind::NessPKThunder1,
        name: "PK Thunder's first trail",
        events: &[nesspkthundertrail::run_event],
        preds: &[nesspkthundertrail::run_pred],
        vars: VarsKind::NessPKThunderTrail,
    },
    ItemKind {
        kind: ItKind::NessPKThunder2,
        name: "PK Thunder's second trail",
        events: &[nesspkthundertrail::run_event],
        preds: &[nesspkthundertrail::run_pred],
        vars: VarsKind::NessPKThunderTrail,
    },
    ItemKind {
        kind: ItKind::NessPKThunder3,
        name: "PK Thunder's third trail",
        events: &[nesspkthundertrail::run_event],
        preds: &[nesspkthundertrail::run_pred],
        vars: VarsKind::NessPKThunderTrail,
    },
    ItemKind {
        kind: ItKind::NessPKThunder4,
        name: "PK Thunder's fourth trail",
        events: &[nesspkthundertrail::run_event],
        preds: &[nesspkthundertrail::run_pred],
        vars: VarsKind::NessPKThunderTrail,
    },
    ItemKind {
        kind: ItKind::FoxBlaster,
        name: "Fox's blaster",
        events: &[foxblaster::run_event],
        preds: &[foxblaster::run_pred],
        vars: VarsKind::FoxBlaster,
    },
    ItemKind {
        kind: ItKind::FalcoBlaster,
        name: "Falco's blaster",
        events: &[foxblaster::run_event],
        preds: &[foxblaster::run_pred],
        vars: VarsKind::FoxBlaster,
    },
    ItemKind {
        kind: ItKind::LinkBow,
        name: "Link's bow",
        events: &[linkbow::run_event],
        preds: &[linkbow::run_pred],
        vars: VarsKind::LinkBow,
    },
    ItemKind {
        kind: ItKind::CLinkBow,
        name: "Young Link's bow",
        events: &[linkbow::run_event],
        preds: &[linkbow::run_pred],
        vars: VarsKind::LinkBow,
    },
    ItemKind {
        kind: ItKind::NessPKFlushExplode,
        name: "PK Flash's explosion",
        events: &[nesspkflashexplode::run_event],
        preds: &[nesspkflashexplode::run_pred],
        vars: VarsKind::PKFlushExplode,
    },
    ItemKind {
        kind: ItKind::SeakNeedleThrow,
        name: "Sheik's thrown needle",
        events: &[seakneedlethrown::run_event],
        preds: &[seakneedlethrown::run_pred],
        vars: VarsKind::SeakNeedleThrown,
    },
    ItemKind {
        kind: ItKind::SeakNeedleHeld,
        name: "Sheik's held needle",
        events: &[seakneedleheld::run_event],
        preds: &[seakneedleheld::run_pred],
        vars: VarsKind::SeakNeedleHeld,
    },
    ItemKind {
        kind: ItKind::PikachuThunder,
        name: "Pikachu's Thunder",
        events: &[pikachuthunder::run_event],
        preds: &[pikachuthunder::run_pred],
        vars: VarsKind::PikachuThunder,
    },
    ItemKind {
        kind: ItKind::PichuThunder,
        name: "Pichu's Thunder",
        events: &[pikachuthunder::run_event],
        preds: &[pikachuthunder::run_pred],
        vars: VarsKind::PikachuThunder,
    },
    ItemKind {
        kind: ItKind::MarioCape,
        name: "Mario's cape",
        events: &[mariocape::run_event],
        preds: &[mariocape::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::DrMarioSheet,
        name: "Dr. Mario's sheet",
        events: &[mariocape::run_event],
        preds: &[mariocape::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::SeakVanish,
        name: "Sheik's Vanish smoke",
        events: &[seakvanish::run_event],
        preds: &[seakvanish::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::YoshiEggThrow,
        name: "Yoshi's thrown egg",
        events: &[yoshieggthrow::run_event],
        preds: &[yoshieggthrow::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::YoshiEggLay,
        name: "Egg Yoshi lays",
        events: &[yoshiegglay::run_event],
        preds: &[yoshiegglay::run_pred],
        vars: VarsKind::YoshiEggLay,
    },
    ItemKind {
        kind: ItKind::YoshiStar,
        name: "Yoshi's ground pound's star",
        events: &[yoshistar::run_event],
        preds: &[yoshistar::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::PikachuTJoltGround,
        name: "Pikachu's Thunder Jolt on the ground",
        events: &[pikachutjoltground::run_event],
        preds: &[pikachutjoltground::run_pred],
        vars: VarsKind::PikachuJoltGround,
    },
    ItemKind {
        kind: ItKind::PikachuTJoltAir,
        name: "Pikachu's Thunder Jolt in the air",
        events: &[pikachutjoltair::run_event],
        preds: &[pikachutjoltair::run_pred],
        vars: VarsKind::PikachuJoltAir,
    },
    ItemKind {
        kind: ItKind::PichuTJoltGround,
        name: "Pichu's Thunder Jolt on the ground",
        events: &[pikachutjoltground::run_event],
        preds: &[pikachutjoltground::run_pred],
        vars: VarsKind::PikachuJoltGround,
    },
    ItemKind {
        kind: ItKind::PichuTJoltAir,
        name: "Pichu's Thunder Jolt in the air",
        events: &[pikachutjoltair::run_event],
        preds: &[pikachutjoltair::run_pred],
        vars: VarsKind::PikachuJoltAir,
    },
    ItemKind {
        kind: ItKind::SamusBomb,
        name: "Samus's bomb",
        events: &[samusbomb::run_event],
        preds: &[samusbomb::run_pred],
        vars: VarsKind::SamusBomb,
    },
    ItemKind {
        kind: ItKind::SamusCharge,
        name: "Samus's Charge Shot",
        events: &[samuschargeshot::run_event],
        preds: &[samuschargeshot::run_pred],
        vars: VarsKind::SamusChargeshot,
    },
    ItemKind {
        kind: ItKind::SamusMissile,
        name: "Samus's missile",
        events: &[samusmissile::run_event],
        preds: &[samusmissile::run_pred],
        vars: VarsKind::SamusMissile,
    },
    ItemKind {
        kind: ItKind::SamusGBeam,
        name: "Samus's grapple beam",
        events: &[samusgrapple::run_event],
        preds: &[],
        vars: VarsKind::SamusGrapple,
    },
    ItemKind {
        kind: ItKind::SeakChain,
        name: "Sheik's chain",
        events: &[seakchain::run_event],
        preds: &[seakchain::run_pred],
        vars: VarsKind::SeakChain,
    },
    ItemKind {
        kind: ItKind::PeachExplode,
        name: "Peach Bomber's explosion",
        events: &[peachexplode::run_event],
        preds: &[peachexplode::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::PeachTurnip,
        name: "Peach's turnip",
        events: &[peachturnip::run_event],
        preds: &[peachturnip::run_pred],
        vars: VarsKind::PeachTurnip,
    },
    ItemKind {
        kind: ItKind::KoopaFlame,
        name: "Bowser's flame",
        events: &[koopaflame::run_event],
        preds: &[koopaflame::run_pred],
        vars: VarsKind::KoopaFlame,
    },
    ItemKind {
        kind: ItKind::NessBat,
        name: "Ness's bat",
        events: &[nessbat::run_event],
        preds: &[nessbat::run_pred],
        vars: VarsKind::NessBat,
    },
    ItemKind {
        kind: ItKind::NessYoyo,
        name: "Ness's yo-yo",
        events: &[nessyoyo::run_event],
        preds: &[nessyoyo::run_pred],
        vars: VarsKind::NessYoyo,
    },
    ItemKind {
        kind: ItKind::PeachParasol,
        name: "Peach's parasol",
        events: &[peachparasol::run_event],
        preds: &[peachparasol::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::PeachToad,
        name: "Peach's Toad",
        events: &[peachtoad::run_event],
        preds: &[peachtoad::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::LuigiFire,
        name: "Luigi's fireball",
        events: &[luigifireball::run_event],
        preds: &[luigifireball::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::IceClimberIce,
        name: "Ice Climbers' ice",
        events: &[climbersice::run_event],
        preds: &[climbersice::run_pred],
        vars: VarsKind::ClimbersIce,
    },
    ItemKind {
        kind: ItKind::IceClimberBlizzard,
        name: "Ice Climbers' Blizzard",
        events: &[climbersblizzard::run_event],
        preds: &[climbersblizzard::run_pred],
        vars: VarsKind::ClimbersBlizzard,
    },
    ItemKind {
        kind: ItKind::ZeldaDinFire,
        name: "Zelda's Din's Fire",
        events: &[zeldadinfire::run_event],
        preds: &[zeldadinfire::run_pred],
        vars: VarsKind::ZeldaDinFire,
    },
    ItemKind {
        kind: ItKind::ZeldaDinFireExplode,
        name: "Din's Fire's explosion",
        events: &[zeldadinfireexplode::run_event],
        preds: &[zeldadinfireexplode::run_pred],
        vars: VarsKind::ZeldaDinFireExplode,
    },
    ItemKind {
        kind: ItKind::MewtwoDisable,
        name: "Mewtwo's Disable",
        events: &[mewtwodisable::run_event],
        preds: &[mewtwodisable::run_pred],
        vars: VarsKind::MDisable,
    },
    ItemKind {
        kind: ItKind::PeachToadSpore,
        name: "Toad's spores",
        events: &[peachtoadspore::run_event],
        preds: &[peachtoadspore::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::MewtwoShadowBall,
        name: "Mewtwo's Shadow Ball",
        events: &[mewtwoshadowball::run_event],
        preds: &[mewtwoshadowball::run_pred],
        vars: VarsKind::MewtwoShadowball,
    },
    ItemKind {
        kind: ItKind::IceClimberGumStrings,
        name: "Ice Climbers' belay string",
        events: &[climbersstring::run_event],
        preds: &[climbersstring::run_pred],
        vars: VarsKind::ClimbersString,
    },
    ItemKind {
        kind: ItKind::GameWatchGreenhouse,
        name: "Mr. Game & Watch's Greenhouse spray",
        events: &[gamewatchgreenhouse::run_event],
        preds: &[gamewatchgreenhouse::run_pred],
        vars: VarsKind::GameWatch,
    },
    ItemKind {
        kind: ItKind::GameWatchManhole,
        name: "Mr. Game & Watch's manhole",
        events: &[gamewatchmanhole::run_event],
        preds: &[gamewatchmanhole::run_pred],
        vars: VarsKind::GameWatch,
    },
    ItemKind {
        kind: ItKind::GameWatchFire,
        name: "Mr. Game & Watch's torch",
        events: &[gamewatchfire::run_event],
        preds: &[gamewatchfire::run_pred],
        vars: VarsKind::GameWatch,
    },
    ItemKind {
        kind: ItKind::GameWatchParachute,
        name: "Mr. Game & Watch's parachute",
        events: &[gamewatchparachute::run_event],
        preds: &[gamewatchparachute::run_pred],
        vars: VarsKind::GameWatch,
    },
    ItemKind {
        kind: ItKind::GameWatchTurtle,
        name: "Mr. Game & Watch's turtle",
        events: &[gamewatchturtle::run_event],
        preds: &[gamewatchturtle::run_pred],
        vars: VarsKind::GameWatch,
    },
    ItemKind {
        kind: ItKind::GameWatchBreath,
        name: "Mr. Game & Watch's Sparky",
        events: &[gamewatchbreath::run_event],
        preds: &[gamewatchbreath::run_pred],
        vars: VarsKind::GameWatch,
    },
    ItemKind {
        kind: ItKind::GameWatchJudge,
        name: "Mr. Game & Watch's Judge",
        events: &[gamewatchjudge::run_event],
        preds: &[gamewatchjudge::run_pred],
        vars: VarsKind::GameWatch,
    },
    ItemKind {
        kind: ItKind::GameWatchPanic,
        name: "Mr. Game & Watch's Oil Panic",
        events: &[gamewatchpanic::run_event],
        preds: &[gamewatchpanic::run_pred],
        vars: VarsKind::GameWatch,
    },
    ItemKind {
        kind: ItKind::GameWatchChef,
        name: "Mr. Game & Watch's sausage",
        events: &[gamewatchchef::run_event],
        preds: &[gamewatchchef::run_pred],
        vars: VarsKind::GameWatch,
    },
    ItemKind {
        kind: ItKind::CLinkMilk,
        name: "Young Link's milk",
        events: &[clinkmilk::run_event],
        preds: &[clinkmilk::run_pred],
        vars: VarsKind::CLinkMilk,
    },
    ItemKind {
        kind: ItKind::GameWatchRescue,
        name: "Mr. Game & Watch's Fire rescue",
        events: &[gamewatchrescue::run_event],
        preds: &[gamewatchrescue::run_pred],
        vars: VarsKind::GameWatchRescue,
    },
    ItemKind {
        kind: ItKind::MasterHandLaser,
        name: "Master Hand's laser",
        events: &[masterhandlaser::run_event],
        preds: &[masterhandlaser::run_pred],
        vars: VarsKind::MasterHandLaser,
    },
    ItemKind {
        kind: ItKind::MasterHandBullet,
        name: "Master Hand's bullet",
        events: &[masterhandbullet::run_event],
        preds: &[masterhandbullet::run_pred],
        vars: VarsKind::MasterHandBullet,
    },
    ItemKind {
        kind: ItKind::CrazyHandLaser,
        name: "Crazy Hand's laser",
        events: &[masterhandlaser::run_event],
        preds: &[masterhandlaser::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::CrazyHandBullet,
        name: "Crazy Hand's bullet",
        events: &[masterhandbullet::run_event],
        preds: &[masterhandbullet::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::CrazyHandBomb,
        name: "Crazy Hand's bomb",
        events: &[crazyhandbomb::run_event],
        preds: &[crazyhandbomb::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::KirbyMarioFire,
        name: "Kirby's copy of Mario's fireball",
        events: &[mariofireball::run_event],
        preds: &[mariofireball::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::KirbyDrMarioVitamin,
        name: "Kirby's copy of Dr. Mario's pill",
        events: &[drmariopill::run_event],
        preds: &[drmariopill::run_pred],
        vars: VarsKind::DrMarioPill,
    },
    ItemKind {
        kind: ItKind::KirbyLuigiFire,
        name: "Kirby's copy of Luigi's fireball",
        events: &[luigifireball::run_event],
        preds: &[luigifireball::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::KirbyIceClimberIce,
        name: "Kirby's copy of the Ice Climbers' ice",
        events: &[climbersice::run_event],
        preds: &[climbersice::run_pred],
        vars: VarsKind::ClimbersIce,
    },
    ItemKind {
        kind: ItKind::KirbyPeachToad,
        name: "Kirby's copy of Peach's Toad",
        events: &[peachtoad::run_event],
        preds: &[peachtoad::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::KirbyPeachToadSpore,
        name: "Kirby's copy of Toad's spores",
        events: &[peachtoadspore::run_event],
        preds: &[peachtoadspore::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::KirbyFoxLaser,
        name: "Kirby's copy of Fox's laser",
        events: &[foxlaser::run_event],
        preds: &[foxlaser::run_pred],
        vars: VarsKind::FoxLaser,
    },
    ItemKind {
        kind: ItKind::KirbyFalcoLaser,
        name: "Kirby's copy of Falco's laser",
        events: &[foxlaser::run_event],
        preds: &[foxlaser::run_pred],
        vars: VarsKind::FoxLaser,
    },
    ItemKind {
        kind: ItKind::KirbyFoxBlaster,
        name: "Kirby's copy of Fox's blaster",
        events: &[foxblaster::run_event],
        preds: &[foxblaster::run_pred],
        vars: VarsKind::FoxBlaster,
    },
    ItemKind {
        kind: ItKind::KirbyFalcoBlaster,
        name: "Kirby's copy of Falco's blaster",
        events: &[foxblaster::run_event],
        preds: &[foxblaster::run_pred],
        vars: VarsKind::FoxBlaster,
    },
    ItemKind {
        kind: ItKind::KirbyLinkArrow,
        name: "Kirby's copy of Link's arrow",
        events: &[linkarrow::run_event],
        preds: &[linkarrow::run_pred],
        vars: VarsKind::LinkArrow,
    },
    ItemKind {
        kind: ItKind::KirbyCLinkArrow,
        name: "Kirby's copy of Young Link's arrow",
        events: &[linkarrow::run_event],
        preds: &[linkarrow::run_pred],
        vars: VarsKind::LinkArrow,
    },
    ItemKind {
        kind: ItKind::KirbyLinkBow,
        name: "Kirby's copy of Link's bow",
        events: &[linkbow::run_event],
        preds: &[linkbow::run_pred],
        vars: VarsKind::LinkBow,
    },
    ItemKind {
        kind: ItKind::KirbyCLinkBow,
        name: "Kirby's copy of Young Link's bow",
        events: &[linkbow::run_event],
        preds: &[linkbow::run_pred],
        vars: VarsKind::LinkBow,
    },
    ItemKind {
        kind: ItKind::KirbyMewtwoShadowBall,
        name: "Kirby's copy of Mewtwo's Shadow Ball",
        events: &[mewtwoshadowball::run_event],
        preds: &[mewtwoshadowball::run_pred],
        vars: VarsKind::MewtwoShadowball,
    },
    ItemKind {
        kind: ItKind::KirbyNessPKFlush,
        name: "Kirby's copy of PK Flash",
        events: &[nesspkflash::run_event],
        preds: &[nesspkflash::run_pred],
        vars: VarsKind::PKFlush,
    },
    ItemKind {
        kind: ItKind::KirbyNessPKFlushExplode,
        name: "Kirby's copy of PK Flash's explosion",
        events: &[nesspkflashexplode::run_event],
        preds: &[nesspkflashexplode::run_pred],
        vars: VarsKind::PKFlushExplode,
    },
    ItemKind {
        kind: ItKind::KirbyPikachuTJoltGround,
        name: "Kirby's copy of Pikachu's Thunder Jolt on the ground",
        events: &[pikachutjoltground::run_event],
        preds: &[pikachutjoltground::run_pred],
        vars: VarsKind::PikachuJoltGround,
    },
    ItemKind {
        kind: ItKind::KirbyPikachuTJoltAir,
        name: "Kirby's copy of Pikachu's Thunder Jolt in the air",
        events: &[pikachutjoltair::run_event],
        preds: &[pikachutjoltair::run_pred],
        vars: VarsKind::PikachuJoltAir,
    },
    ItemKind {
        kind: ItKind::KirbyPichuTJoltGround,
        name: "Kirby's copy of Pichu's Thunder Jolt on the ground",
        events: &[pikachutjoltground::run_event],
        preds: &[pikachutjoltground::run_pred],
        vars: VarsKind::PikachuJoltGround,
    },
    ItemKind {
        kind: ItKind::KirbyPichuTJoltAir,
        name: "Kirby's copy of Pichu's Thunder Jolt in the air",
        events: &[pikachutjoltair::run_event],
        preds: &[pikachutjoltair::run_pred],
        vars: VarsKind::PikachuJoltAir,
    },
    ItemKind {
        kind: ItKind::KirbySamusCharge,
        name: "Kirby's copy of Samus's Charge Shot",
        events: &[samuschargeshot::run_event],
        preds: &[samuschargeshot::run_pred],
        vars: VarsKind::SamusChargeshot,
    },
    ItemKind {
        kind: ItKind::KirbySeakNeedleThrow,
        name: "Kirby's copy of Sheik's thrown needle",
        events: &[seakneedlethrown::run_event],
        preds: &[seakneedlethrown::run_pred],
        vars: VarsKind::SeakNeedleThrown,
    },
    ItemKind {
        kind: ItKind::KirbySeakNeedleHeld,
        name: "Kirby's copy of Sheik's held needle",
        events: &[seakneedleheld::run_event],
        preds: &[seakneedleheld::run_pred],
        vars: VarsKind::SeakNeedleHeld,
    },
    ItemKind {
        kind: ItKind::KirbyKoopaFlame,
        name: "Kirby's copy of Bowser's flame",
        events: &[koopaflame::run_event],
        preds: &[koopaflame::run_pred],
        vars: VarsKind::KoopaFlame,
    },
    ItemKind {
        kind: ItKind::KirbyGameWatchChef,
        name: "Kirby's copy of Mr. Game & Watch's sausage",
        events: &[gamewatchchef::run_event],
        preds: &[gamewatchchef::run_pred],
        vars: VarsKind::GameWatch,
    },
    ItemKind {
        kind: ItKind::KirbyGameWatchChefPan,
        name: "Kirby's copy of Mr. Game & Watch's pan",
        events: &[kirbygamewatchchefpan::run_event],
        preds: &[kirbygamewatchchefpan::run_pred],
        vars: VarsKind::GameWatch,
    },
    ItemKind {
        kind: ItKind::KirbyYoshiEggLay,
        name: "Kirby's copy of Yoshi's laid egg",
        events: &[yoshiegglay::run_event],
        preds: &[yoshiegglay::run_pred],
        vars: VarsKind::YoshiEggLay,
    },
    ItemKind {
        kind: ItKind::Unk4,
        name: "Single-player coin",
        events: &[coin::run_event],
        preds: &[coin::run_pred],
        vars: VarsKind::Coin,
    },
    ItemKind {
        kind: ItKind::Coin,
        name: "Trophy coin",
        events: &[trophycoin::run_event],
        preds: &[trophycoin::run_pred],
        vars: VarsKind::TrophyCoin,
    },
    ItemKind {
        kind: ItKind::StageHurtbox,
        name: "Stages' hurtbox items",
        events: &[yaku::run_event],
        preds: &[yaku::run_pred],
        vars: VarsKind::Yaku,
    },
    ItemKind {
        kind: ItKind::Tosakinto,
        name: "Goldeen",
        events: &[tosakinto::run_event],
        preds: &[tosakinto::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Chicorita,
        name: "Chikorita",
        events: &[chicorita::run_event],
        preds: &[chicorita::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Kabigon,
        name: "Snorlax",
        events: &[kabigon::run_event],
        preds: &[kabigon::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Kamex,
        name: "Blastoise",
        events: &[kamex::run_event],
        preds: &[kamex::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Matadogas,
        name: "Weezing",
        events: &[matadogas::run_event],
        preds: &[matadogas::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Lizardon,
        name: "Charizard",
        events: &[lizardon::run_event],
        preds: &[lizardon::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Fire,
        name: "Moltres",
        events: &[fire::run_event],
        preds: &[fire::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Thunder,
        name: "Zapdos",
        events: &[thunder::run_event],
        preds: &[thunder::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Freezer,
        name: "Articuno",
        events: &[freezer::run_event],
        preds: &[freezer::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Sonans,
        name: "Wobbuffet",
        events: &[sonans::run_event],
        preds: &[sonans::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Hassam,
        name: "Scizor",
        events: &[hassam::run_event],
        preds: &[hassam::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Unknown,
        name: "Unown",
        events: &[unknown::run_event],
        preds: &[unknown::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Entei,
        name: "Entei",
        events: &[entei::run_event],
        preds: &[entei::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Raikou,
        name: "Raikou",
        events: &[raikou::run_event],
        preds: &[raikou::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Suikun,
        name: "Suicune",
        events: &[suikun::run_event],
        preds: &[suikun::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Kireihana,
        name: "Bellossom",
        events: &[kireihana::run_event],
        preds: &[kireihana::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Marumine,
        name: "Electrode",
        events: &[marumine::run_event],
        preds: &[marumine::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Lugia,
        name: "Lugia",
        events: &[lugia::run_event],
        preds: &[lugia::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Houou,
        name: "Ho-Oh",
        events: &[houou::run_event],
        preds: &[houou::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Metamon,
        name: "Ditto",
        events: &[metamon::run_event],
        preds: &[metamon::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Pippi,
        name: "Clefairy",
        events: &[pippi::run_event],
        preds: &[pippi::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Togepy,
        name: "Togepi",
        events: &[togepy::run_event],
        preds: &[togepy::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Mew,
        name: "Mew",
        events: &[mew::run_event],
        preds: &[mew::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Cerebi,
        name: "Celebi",
        events: &[cerebi::run_event],
        preds: &[cerebi::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Hitodeman,
        name: "Staryu",
        events: &[hitodeman::run_event],
        preds: &[hitodeman::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Lucky,
        name: "Chansey",
        events: &[lucky::run_event],
        preds: &[lucky::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Porygon2,
        name: "Porygon2",
        events: &[porygon2::run_event],
        preds: &[porygon2::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Hinoarashi,
        name: "Cyndaquil",
        events: &[hinoarashi::run_event],
        preds: &[hinoarashi::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Maril,
        name: "Marill",
        events: &[maril::run_event],
        preds: &[maril::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Fushigibana,
        name: "Venusaur",
        events: &[fushigibana::run_event],
        preds: &[fushigibana::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::ChicoritaLeaf,
        name: "Chikorita's leaf",
        events: &[chicorita::run_event],
        preds: &[chicorita::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::KamexHydroPump,
        name: "Blastoise's Hydro Pump",
        events: &[kamex::run_event],
        preds: &[kamex::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::MatadogasGas1,
        name: "Weezing's first gas",
        events: &[matadogas::run_event],
        preds: &[matadogas::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::MatadogasGas2,
        name: "Weezing's second gas",
        events: &[matadogas::run_event],
        preds: &[matadogas::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::LizardonFlame1,
        name: "Charizard's first flame",
        events: &[lizardon::run_event],
        preds: &[lizardon::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::LizardonFlame2,
        name: "Charizard's second flame",
        events: &[lizardon::run_event],
        preds: &[lizardon::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::LizardonFlame3,
        name: "Charizard's third flame",
        events: &[lizardon::run_event],
        preds: &[lizardon::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::LizardonFlame4,
        name: "Charizard's fourth flame",
        events: &[lizardon::run_event],
        preds: &[lizardon::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::UnknownSwarm,
        name: "Unown's little Unown",
        events: &[unknown::run_event],
        preds: &[unknown::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::LugiaAeroblast,
        name: "Lugia's Aeroblast",
        events: &[lugia::run_event],
        preds: &[lugia::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::LugiaAeroblast2,
        name: "Lugia's second Aeroblast",
        events: &[lugia::run_event],
        preds: &[lugia::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::LugiaAeroblast3,
        name: "Lugia's third Aeroblast",
        events: &[lugia::run_event],
        preds: &[lugia::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::HououSacredFire,
        name: "Ho-Oh's Sacred Fire",
        events: &[houou::run_event],
        preds: &[houou::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::HitodemanStar,
        name: "Staryu's star",
        events: &[hitodeman::run_event],
        preds: &[hitodeman::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::LuckyEgg,
        name: "Chansey's egg",
        events: &[lucky::run_event],
        preds: &[lucky::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::HinoarashiFlame,
        name: "Cyndaquil's flame",
        events: &[maril::run_event],
        preds: &[maril::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::PokemonUnk,
        name: "A Pokémon's item of unknown use",
        events: &[],
        preds: &[],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::OldKuri,
        name: "Old Goomba",
        events: &[oldkuri::run_event, zako::run_event],
        preds: &[oldkuri::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::Mato,
        name: "Target Test's targets",
        events: &[mato::run_event, zako::run_event],
        preds: &[mato::run_pred],
        vars: VarsKind::Zako,
    },
    ItemKind {
        kind: ItKind::Heiho,
        name: "Yoshi's Story's Shy Guys",
        events: &[heiho::run_event, zako::run_event],
        preds: &[heiho::run_pred],
        vars: VarsKind::Heiho,
    },
    ItemKind {
        kind: ItKind::Nokonoko,
        name: "Koopa Troopa",
        events: &[nokonoko::run_event, zako::run_event],
        preds: &[nokonoko::run_pred],
        vars: VarsKind::Zako,
    },
    ItemKind {
        kind: ItKind::Patapata,
        name: "Koopa Paratroopa",
        events: &[patapata::run_event, zako::run_event],
        preds: &[patapata::run_pred],
        vars: VarsKind::Zako,
    },
    ItemKind {
        kind: ItKind::Likelike,
        name: "Like Like",
        events: &[likelike::run_event],
        preds: &[likelike::run_pred],
        vars: VarsKind::Zako,
    },
    ItemKind {
        kind: ItKind::OldLead,
        name: "Old ReDead (unused)",
        events: &[],
        preds: &[],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::OldOcta,
        name: "Old Octorok (unused)",
        events: &[],
        preds: &[],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::OldOtto,
        name: "Old Ottosea",
        events: &[oldottosea::run_event],
        preds: &[oldottosea::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::Whitebea,
        name: "Polar Bear",
        events: &[zako::run_event, whitebea::run_event],
        preds: &[whitebea::run_pred],
        vars: VarsKind::Zako,
    },
    ItemKind {
        kind: ItKind::Klap,
        name: "Kongo Jungle's Klap Trap",
        events: &[klap::run_event],
        preds: &[klap::run_pred],
        vars: VarsKind::Klap,
    },
    ItemKind {
        kind: ItKind::ZGShell,
        name: "Koopas' green shell",
        events: &[zgshell::run_event],
        preds: &[zgshell::run_pred],
        vars: VarsKind::Zako,
    },
    ItemKind {
        kind: ItKind::ZRShell,
        name: "Koopas' red shell",
        events: &[zgshell::run_event],
        preds: &[zgshell::run_pred],
        vars: VarsKind::Zako,
    },
    ItemKind {
        kind: ItKind::Tincle,
        name: "Great Bay's Tingle",
        events: &[tincle::run_event],
        preds: &[tincle::run_pred],
        vars: VarsKind::Tincle,
    },
    ItemKind {
        kind: ItKind::Invalid1,
        name: "Unused",
        events: &[],
        preds: &[],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::Invalid2,
        name: "Unused",
        events: &[],
        preds: &[],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::Invalid3,
        name: "Unused",
        events: &[],
        preds: &[],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::WhispyApple,
        name: "Green Greens' apple",
        events: &[whispyapple::run_event],
        preds: &[whispyapple::run_pred],
        vars: VarsKind::WhispyApple,
    },
    ItemKind {
        kind: ItKind::WhispyHealApple,
        name: "Green Greens' healing apple",
        events: &[whispyapple::run_event],
        preds: &[whispyapple::run_pred],
        vars: VarsKind::WhispyApple,
    },
    ItemKind {
        kind: ItKind::Invalid4,
        name: "Unused",
        events: &[],
        preds: &[],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::Invalid5,
        name: "Unused",
        events: &[],
        preds: &[],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::Invalid6,
        name: "Unused",
        events: &[],
        preds: &[],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::Tools,
        name: "Flat Zone's tools",
        events: &[tools::run_event],
        preds: &[tools::run_pred],
        vars: VarsKind::Tools,
    },
    ItemKind {
        kind: ItKind::Invalid7,
        name: "Unused",
        events: &[],
        preds: &[],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::Invalid8,
        name: "Unused",
        events: &[],
        preds: &[],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::Kyasarin,
        name: "Birdo",
        events: &[kyasarin::run_event],
        preds: &[kyasarin::run_pred],
        vars: VarsKind::Kyasarin,
    },
    ItemKind {
        kind: ItKind::ArwingLaser,
        name: "Corneria's Arwings' laser",
        events: &[arwinglaser::run_event],
        preds: &[arwinglaser::run_pred],
        vars: VarsKind::ArwingLaser,
    },
    ItemKind {
        kind: ItKind::GreatFoxLaser,
        name: "Great Fox's laser",
        events: &[greatfoxlaser::run_event],
        preds: &[greatfoxlaser::run_pred],
        vars: VarsKind::GreatFoxLaser,
    },
    ItemKind {
        kind: ItKind::KyasarinEgg,
        name: "Birdo's egg",
        events: &[kyasarinegg::run_event],
        preds: &[kyasarinegg::run_pred],
        vars: VarsKind::KyasarinEgg,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// The addresses an entry's dispatch functions add.
    fn own(e: &ItemKind) -> BTreeSet<u32> {
        let mut ev = Entries::<kind::Proc>::new();
        let mut pr = Entries::<kind::Pred>::new();
        for f in e.events {
            f(&mut ev);
        }
        for f in e.preds {
            f(&mut pr);
        }
        (ev.addresses().into_iter().chain(pr.addresses()))
            .map(|(a, _)| a)
            .collect()
    }

    /// The callbacks a kind's logic table and its state table name.
    fn tables_name(k: ItKind) -> Vec<u32> {
        let l = k.logic();
        let mut a: Vec<u32> = [
            l.spawned,
            l.destroyed,
            l.picked_up,
            l.dropped,
            l.thrown,
            l.dmg_dealt,
            l.dmg_received,
            l.entered_air,
            l.reflected,
            l.clanked,
            l.absorbed,
            l.shield_bounced,
            l.hit_shield,
            l.evt_unk,
        ]
        .iter()
        .flatten()
        .map(|c| c.0)
        .collect();
        if let Some((_, states)) = tables::STATE_TABLES.iter().find(|(at, _)| *at == l.states) {
            for s in states.iter() {
                a.extend(
                    [s.animated, s.physics_updated, s.collided]
                        .iter()
                        .flatten()
                        .map(|c| c.0),
                );
            }
        }
        a
    }

    /// Each entry sits at its kind's index, and each kind's number is its index.
    #[test]
    fn the_registry_is_indexed_by_item_kind() {
        for (i, k) in ItKind::ALL.iter().enumerate() {
            assert_eq!(k.id(), i as i32);
            assert_eq!(ItKind::from_id(i as i32), Some(*k));
            assert_eq!(ITEMS[i].kind, *k, "{}", ITEMS[i].name);
        }
        assert_eq!(ItKind::from_id(-1), None);
        assert_eq!(ItKind::from_id(ItKind::COUNT as i32), None);
        assert_eq!(ItKind::ALL.iter().filter(|k| k.is_pokemon()).count(), 30);
        assert!(
            -1 < ItKind::Capsule && 0 == ItKind::Capsule && ItKind::Box == 1 && 2 > ItKind::Box
        );
    }

    /// Every callback a kind's tables name that an item module ports is in the kind's own
    /// entry's modules, and an entry with modules runs some of its tables' callbacks: the
    /// entries name the code their kinds run (two entries' modules swapped fail here).
    #[test]
    fn each_kinds_callbacks_are_in_its_entry() {
        let mut ported = BTreeSet::new();
        for e in &ITEMS {
            ported.extend(own(e));
        }
        let mut wrong = Vec::new();
        for e in &ITEMS {
            let mine = own(e);
            let named = tables_name(e.kind);
            for a in named
                .iter()
                .filter(|a| ported.contains(a) && !mine.contains(a))
            {
                wrong.push(format!(
                    "{:?}: {a:#010x} isn't in its entry's modules",
                    e.kind
                ));
            }
            if !mine.is_empty() && !named.iter().any(|a| mine.contains(a)) {
                wrong.push(format!(
                    "{:?}: its modules run none of its tables' callbacks",
                    e.kind
                ));
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }

    /// Each entry's callbacks are in the callback table.
    #[test]
    fn item_callbacks_are_in_the_table() {
        let t = super::super::super::code::table();
        for e in &ITEMS {
            for a in own(e) {
                assert!(
                    t.proc.get(a).is_some() || t.pred.get(a).is_some(),
                    "{}: {a:#010x}",
                    e.name
                );
            }
        }
    }
}
