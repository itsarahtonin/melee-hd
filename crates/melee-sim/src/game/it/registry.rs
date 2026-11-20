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
//! (`item.kind == ItKind::Capsule`, `match item.item_kind() { ItKind::HeartContainer => .. }`,
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
    #[doc(alias("Box"))]
    Crate = 1,
    /// `It_Kind_Taru`: the Barrel.
    #[doc(alias("Taru"))]
    Barrel = 2,
    /// `It_Kind_Egg`: the Egg.
    Egg = 3,
    /// `It_Kind_Kusudama`: the Party Ball.
    #[doc(alias("Kusudama"))]
    PartyBall = 4,
    /// `It_Kind_TaruCann`: the Barrel Cannon.
    #[doc(alias("TaruCann"))]
    BarrelCannon = 5,
    /// `It_Kind_BombHei`: the Bob-omb.
    #[doc(alias("BombHei"))]
    BobOmb = 6,
    /// `It_Kind_Dosei`: Mr. Saturn.
    #[doc(alias("Dosei"))]
    MrSaturn = 7,
    /// `It_Kind_Heart`: the Heart Container.
    #[doc(alias("Heart"))]
    HeartContainer = 8,
    /// `It_Kind_Tomato`: the Maxim Tomato.
    #[doc(alias("Tomato"))]
    MaximTomato = 9,
    /// `It_Kind_Star`: the Starman.
    #[doc(alias("Star"))]
    Starman = 10,
    /// `It_Kind_Bat`: the Home-Run Bat.
    #[doc(alias("Bat"))]
    HomeRunBat = 11,
    /// `It_Kind_Sword`: the Beam Sword.
    #[doc(alias("Sword"))]
    BeamSword = 12,
    /// `It_Kind_Parasol`: the Parasol.
    Parasol = 13,
    /// `It_Kind_G_Shell`: the Green Shell.
    #[doc(alias("GShell"))]
    GreenShell = 14,
    /// `It_Kind_R_Shell`: the Red Shell.
    #[doc(alias("RShell"))]
    RedShell = 15,
    /// `It_Kind_L_Gun`: the Ray Gun.
    #[doc(alias("LGun"))]
    RayGun = 16,
    /// `It_Kind_Freeze`: the Freezie.
    #[doc(alias("Freeze"))]
    Freezie = 17,
    /// `It_Kind_Foods`: the food.
    #[doc(alias("Foods"))]
    Food = 18,
    /// `It_Kind_MSBomb`: the Motion-Sensor Bomb.
    #[doc(alias("MSBomb"))]
    MotionSensorBomb = 19,
    /// `It_Kind_Flipper`: the Flipper.
    Flipper = 20,
    /// `It_Kind_S_Scope`: the Super Scope.
    #[doc(alias("SScope"))]
    SuperScope = 21,
    /// `It_Kind_StarRod`: the Star Rod.
    StarRod = 22,
    /// `It_Kind_LipStick`: the Lip's Stick.
    #[doc(alias("LipStick"))]
    LipsStick = 23,
    /// `It_Kind_Harisen`: the Fan.
    #[doc(alias("Harisen"))]
    Fan = 24,
    /// `It_Kind_F_Flower`: the Fire Flower.
    #[doc(alias("FFlower"))]
    FireFlower = 25,
    /// `It_Kind_Kinoko`: the Super Mushroom.
    #[doc(alias("Kinoko"))]
    SuperMushroom = 26,
    /// `It_Kind_DKinoko`: the Poison Mushroom.
    #[doc(alias("DKinoko"))]
    PoisonMushroom = 27,
    /// `It_Kind_Hammer`: the Hammer.
    Hammer = 28,
    /// `It_Kind_WStar`: the Warp Star.
    #[doc(alias("WStar"))]
    WarpStar = 29,
    /// `It_Kind_ScBall`: the Screw Attack.
    #[doc(alias("ScBall"))]
    ScrewAttack = 30,
    /// `It_Kind_RabbitC`: the Bunny Hood.
    #[doc(alias("RabbitC"))]
    BunnyHood = 31,
    /// `It_Kind_MetalB`: the Metal Box.
    #[doc(alias("MetalB"))]
    MetalBox = 32,
    /// `It_Kind_Spycloak`: the Cloaking Device.
    #[doc(alias("Spycloak"))]
    CloakingDevice = 33,
    /// `It_Kind_M_Ball`: the Poké Ball.
    #[doc(alias("MBall"))]
    PokeBall = 34,
    /// `It_Kind_L_Gun_Ray`: the Ray Gun's shot.
    #[doc(alias("LGunRay"))]
    RayGunShot = 35,
    /// `It_Kind_StarRod_Star`: the Star Rod's star.
    StarRodStar = 36,
    /// `It_Kind_LipStick_Spore`: the Lip's Stick's spore.
    #[doc(alias("LipStickSpore"))]
    LipsStickSpore = 37,
    /// `It_Kind_S_Scope_Beam`: the Super Scope's shot.
    #[doc(alias("SScopeBeam"))]
    SuperScopeShot = 38,
    /// `It_Kind_L_Gun_Beam`: the Fire Flower's flame.
    #[doc(alias("LGunBeam"))]
    FireFlowerFlame = 39,
    /// `It_Kind_Hammer_Head`: the Hammer's head.
    HammerHead = 40,
    /// `It_Kind_F_Flower_Flame`: the flower the Lip's Stick plants.
    #[doc(alias("FFlowerFlame"))]
    LipsStickFlower = 41,
    /// `It_Kind_EvYoshiEgg`: Event 13's egg.
    #[doc(alias("EvYoshiEgg"))]
    EventYoshiEgg = 42,
    /// `It_Kind_Kuriboh`: the Goomba.
    #[doc(alias("Kuriboh"))]
    Goomba = 43,
    /// `It_Kind_Leadead`: the ReDead.
    #[doc(alias("Leadead"))]
    ReDead = 44,
    /// `It_Kind_Octarock`: the Octorok.
    #[doc(alias("Octarock"))]
    Octorok = 45,
    /// `It_Kind_Ottosea`: Topi, the seal of Ice Climber.
    #[doc(alias("Ottosea"))]
    Topi = 46,
    /// `It_Kind_Octarock_Stone`: the Octorok's stone.
    #[doc(alias("OctarockStone"))]
    OctorokStone = 47,
    /// `It_Kind_Mario_Fire`: Mario's fireball.
    #[doc(alias("MarioFire"))]
    MarioFireball = 48,
    /// `It_Kind_DrMario_Vitamin`: Dr. Mario's pill.
    #[doc(alias("DrMarioVitamin"))]
    DrMarioMegavitamin = 49,
    /// `It_Kind_Kirby_CBeam`: Kirby's Final Cutter wave.
    #[doc(alias("KirbyCBeam"))]
    KirbyFinalCutterWave = 50,
    /// `It_Kind_Kirby_Hammer`: Kirby's hammer.
    KirbyHammer = 51,
    /// `It_Kind_Unk1`: the star of Kirby's lost copy.
    #[doc(alias("Unk1"))]
    KirbyCopyStar = 52,
    /// `It_Kind_Unk2`: the star Kirby spits.
    #[doc(alias("Unk2"))]
    KirbySpitStar = 53,
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
    #[doc(alias("CLinkBomb"))]
    YoungLinkBomb = 59,
    /// `It_Kind_Link_Boomerang`: Link's boomerang.
    LinkBoomerang = 60,
    /// `It_Kind_CLink_Boomerang`: Young Link's boomerang.
    #[doc(alias("CLinkBoomerang"))]
    YoungLinkBoomerang = 61,
    /// `It_Kind_Link_HShot`: Link's hookshot.
    #[doc(alias("LinkHShot"))]
    LinkHookshot = 62,
    /// `It_Kind_CLink_HShot`: Young Link's hookshot.
    #[doc(alias("CLinkHShot"))]
    YoungLinkHookshot = 63,
    /// `It_Kind_Link_Arrow`: Link's arrow.
    LinkArrow = 64,
    /// `It_Kind_CLink_Arrow`: Young Link's fire arrow.
    #[doc(alias("CLinkArrow"))]
    YoungLinkFireArrow = 65,
    /// `It_Kind_Ness_PKFire`: Ness's PK Fire.
    NessPKFire = 66,
    /// `It_Kind_Ness_PKFire_Flame`: PK Fire's pillar.
    #[doc(alias("NessPKFireFlame"))]
    NessPKFirePillar = 67,
    /// `It_Kind_Ness_PKFlush`: Ness's PK Flash, charging.
    #[doc(alias("NessPKFlush"))]
    NessPKFlash = 68,
    /// `It_Kind_Ness_PKThunder`: Ness's PK Thunder.
    NessPKThunder = 69,
    /// `It_Kind_Ness_PKThunder1`: PK Thunder's first trail.
    #[doc(alias("NessPKThunder1"))]
    NessPKThunderTrail1 = 70,
    /// `It_Kind_Ness_PKThunder2`: PK Thunder's second trail.
    #[doc(alias("NessPKThunder2"))]
    NessPKThunderTrail2 = 71,
    /// `It_Kind_Ness_PKThunder3`: PK Thunder's third trail.
    #[doc(alias("NessPKThunder3"))]
    NessPKThunderTrail3 = 72,
    /// `It_Kind_Ness_PKThunder4`: PK Thunder's fourth trail.
    #[doc(alias("NessPKThunder4"))]
    NessPKThunderTrail4 = 73,
    /// `It_Kind_Fox_Blaster`: Fox's blaster.
    FoxBlaster = 74,
    /// `It_Kind_Falco_Blaster`: Falco's blaster.
    FalcoBlaster = 75,
    /// `It_Kind_Link_Bow`: Link's bow.
    LinkBow = 76,
    /// `It_Kind_CLink_Bow`: Young Link's bow.
    #[doc(alias("CLinkBow"))]
    YoungLinkBow = 77,
    /// `It_Kind_Ness_PKFlush_Explode`: PK Flash's explosion.
    #[doc(alias("NessPKFlushExplode"))]
    NessPKFlashExplosion = 78,
    /// `It_Kind_Seak_NeedleThrow`: Sheik's thrown needle.
    #[doc(alias("SeakNeedleThrow"))]
    SheikNeedleThrown = 79,
    /// `It_Kind_Seak_NeedleHeld`: Sheik's held needle.
    #[doc(alias("SeakNeedleHeld"))]
    SheikNeedleHeld = 80,
    /// `It_Kind_Pikachu_Thunder`: Pikachu's Thunder.
    PikachuThunder = 81,
    /// `It_Kind_Pichu_Thunder`: Pichu's Thunder.
    PichuThunder = 82,
    /// `It_Kind_Mario_Cape`: Mario's cape.
    MarioCape = 83,
    /// `It_Kind_DrMario_Sheet`: Dr. Mario's sheet.
    #[doc(alias("DrMarioSheet"))]
    DrMarioSuperSheet = 84,
    /// `It_Kind_Seak_Vanish`: Sheik's Vanish smoke.
    #[doc(alias("SeakVanish"))]
    SheikVanish = 85,
    /// `It_Kind_Yoshi_EggThrow`: Yoshi's thrown egg.
    YoshiEggThrow = 86,
    /// `It_Kind_Yoshi_EggLay`: the egg Yoshi lays.
    YoshiEggLay = 87,
    /// `It_Kind_Yoshi_Star`: Yoshi's ground pound's star.
    YoshiStar = 88,
    /// `It_Kind_Pikachu_TJolt_Ground`: Pikachu's Thunder Jolt on the ground.
    #[doc(alias("PikachuTJoltGround"))]
    PikachuThunderJoltGround = 89,
    /// `It_Kind_Pikachu_TJolt_Air`: Pikachu's Thunder Jolt in the air.
    #[doc(alias("PikachuTJoltAir"))]
    PikachuThunderJoltAir = 90,
    /// `It_Kind_Pichu_TJolt_Ground`: Pichu's Thunder Jolt on the ground.
    #[doc(alias("PichuTJoltGround"))]
    PichuThunderJoltGround = 91,
    /// `It_Kind_Pichu_TJolt_Air`: Pichu's Thunder Jolt in the air.
    #[doc(alias("PichuTJoltAir"))]
    PichuThunderJoltAir = 92,
    /// `It_Kind_Samus_Bomb`: Samus's bomb.
    SamusBomb = 93,
    /// `It_Kind_Samus_Charge`: Samus's Charge Shot.
    #[doc(alias("SamusCharge"))]
    SamusChargeShot = 94,
    /// `It_Kind_Samus_Missile`: Samus's missile.
    SamusMissile = 95,
    /// `It_Kind_Samus_GBeam`: Samus's grapple beam.
    #[doc(alias("SamusGBeam"))]
    SamusGrappleBeam = 96,
    /// `It_Kind_Seak_Chain`: Sheik's chain.
    #[doc(alias("SeakChain"))]
    SheikChain = 97,
    /// `It_Kind_Peach_Explode`: Peach Bomber's explosion.
    #[doc(alias("PeachExplode"))]
    PeachBomberExplosion = 98,
    /// `It_Kind_Peach_Turnip`: Peach's turnip.
    PeachTurnip = 99,
    /// `It_Kind_Koopa_Flame`: Bowser's flame.
    #[doc(alias("KoopaFlame"))]
    BowserFlame = 100,
    /// `It_Kind_Ness_Bat`: Ness's bat.
    NessBat = 101,
    /// `It_Kind_Ness_Yoyo`: Ness's yo-yo.
    NessYoyo = 102,
    /// `It_Kind_Peach_Parasol`: Peach's parasol.
    PeachParasol = 103,
    /// `It_Kind_Peach_Toad`: Peach's Toad.
    PeachToad = 104,
    /// `It_Kind_Luigi_Fire`: Luigi's fireball.
    #[doc(alias("LuigiFire"))]
    LuigiFireball = 105,
    /// `It_Kind_IceClimber_Ice`: the Ice Climbers' ice.
    #[doc(alias("IceClimberIce"))]
    IceClimbersIceShot = 106,
    /// `It_Kind_IceClimber_Blizzard`: the Ice Climbers' Blizzard.
    #[doc(alias("IceClimberBlizzard"))]
    IceClimbersBlizzard = 107,
    /// `It_Kind_Zelda_DinFire`: Zelda's Din's Fire.
    #[doc(alias("ZeldaDinFire"))]
    ZeldaDinsFire = 108,
    /// `It_Kind_Zelda_DinFire_Explode`: Din's Fire's explosion.
    #[doc(alias("ZeldaDinFireExplode"))]
    ZeldaDinsFireExplosion = 109,
    /// `It_Kind_Mewtwo_Disable`: Mewtwo's Disable.
    MewtwoDisable = 110,
    /// `It_Kind_Peach_ToadSpore`: Toad's spores.
    PeachToadSpore = 111,
    /// `It_Kind_Mewtwo_ShadowBall`: Mewtwo's Shadow Ball.
    MewtwoShadowBall = 112,
    /// `It_Kind_IceClimber_GumStrings`: the Ice Climbers' belay string.
    #[doc(alias("IceClimberGumStrings"))]
    IceClimbersBelayRope = 113,
    /// `It_Kind_GameWatch_Greenhouse`: Mr. Game & Watch's Greenhouse spray.
    #[doc(alias("GameWatchGreenhouse"))]
    GameAndWatchSprayer = 114,
    /// `It_Kind_GameWatch_Manhole`: Mr. Game & Watch's manhole.
    #[doc(alias("GameWatchManhole"))]
    GameAndWatchManhole = 115,
    /// `It_Kind_GameWatch_Fire`: Mr. Game & Watch's torch.
    #[doc(alias("GameWatchFire"))]
    GameAndWatchTorch = 116,
    /// `It_Kind_GameWatch_Parachute`: Mr. Game & Watch's parachute.
    #[doc(alias("GameWatchParachute"))]
    GameAndWatchParachute = 117,
    /// `It_Kind_GameWatch_Turtle`: Mr. Game & Watch's turtle.
    #[doc(alias("GameWatchTurtle"))]
    GameAndWatchTurtle = 118,
    /// `It_Kind_GameWatch_Breath`: Mr. Game & Watch's Sparky.
    #[doc(alias("GameWatchBreath"))]
    GameAndWatchSparky = 119,
    /// `It_Kind_GameWatch_Judge`: Mr. Game & Watch's Judge.
    #[doc(alias("GameWatchJudge"))]
    GameAndWatchJudge = 120,
    /// `It_Kind_GameWatch_Panic`: Mr. Game & Watch's Oil Panic.
    #[doc(alias("GameWatchPanic"))]
    GameAndWatchOil = 121,
    /// `It_Kind_GameWatch_Chef`: Mr. Game & Watch's sausage.
    #[doc(alias("GameWatchChef"))]
    GameAndWatchSausage = 122,
    /// `It_Kind_CLink_Milk`: Young Link's milk.
    #[doc(alias("CLinkMilk"))]
    YoungLinkMilk = 123,
    /// `It_Kind_GameWatch_Rescue`: Mr. Game & Watch's Fire rescue.
    #[doc(alias("GameWatchRescue"))]
    GameAndWatchTrampoline = 124,
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
    #[doc(alias("KirbyMarioFire"))]
    KirbyMarioFireball = 130,
    /// `It_Kind_Kirby_DrMarioVitamin`: Kirby's copy of Dr. Mario's pill.
    #[doc(alias("KirbyDrMarioVitamin"))]
    KirbyDrMarioMegavitamin = 131,
    /// `It_Kind_Kirby_LuigiFire`: Kirby's copy of Luigi's fireball.
    #[doc(alias("KirbyLuigiFire"))]
    KirbyLuigiFireball = 132,
    /// `It_Kind_Kirby_IceClimberIce`: Kirby's copy of the Ice Climbers' ice.
    #[doc(alias("KirbyIceClimberIce"))]
    KirbyIceClimbersIceShot = 133,
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
    #[doc(alias("KirbyCLinkArrow"))]
    KirbyYoungLinkFireArrow = 141,
    /// `It_Kind_Kirby_LinkBow`: Kirby's copy of Link's bow.
    KirbyLinkBow = 142,
    /// `It_Kind_Kirby_CLinkBow`: Kirby's copy of Young Link's bow.
    #[doc(alias("KirbyCLinkBow"))]
    KirbyYoungLinkBow = 143,
    /// `It_Kind_Kirby_MewtwoShadowBall`: Kirby's copy of Mewtwo's Shadow Ball.
    KirbyMewtwoShadowBall = 144,
    /// `It_Kind_Kirby_NessPKFlush`: Kirby's copy of PK Flash.
    #[doc(alias("KirbyNessPKFlush"))]
    KirbyNessPKFlash = 145,
    /// `It_Kind_Kirby_NessPKFlush_Explode`: Kirby's copy of PK Flash's explosion.
    #[doc(alias("KirbyNessPKFlushExplode"))]
    KirbyNessPKFlashExplosion = 146,
    /// `It_Kind_Kirby_PikachuTJolt_Ground`: Kirby's copy of Pikachu's Thunder Jolt on the ground.
    #[doc(alias("KirbyPikachuTJoltGround"))]
    KirbyPikachuThunderJoltGround = 147,
    /// `It_Kind_Kirby_PikachuTJolt_Air`: Kirby's copy of Pikachu's Thunder Jolt in the air.
    #[doc(alias("KirbyPikachuTJoltAir"))]
    KirbyPikachuThunderJoltAir = 148,
    /// `It_Kind_Kirby_PichuTJolt_Ground`: Kirby's copy of Pichu's Thunder Jolt on the ground.
    #[doc(alias("KirbyPichuTJoltGround"))]
    KirbyPichuThunderJoltGround = 149,
    /// `It_Kind_Kirby_PichuTJolt_Air`: Kirby's copy of Pichu's Thunder Jolt in the air.
    #[doc(alias("KirbyPichuTJoltAir"))]
    KirbyPichuThunderJoltAir = 150,
    /// `It_Kind_Kirby_SamusCharge`: Kirby's copy of Samus's Charge Shot.
    #[doc(alias("KirbySamusCharge"))]
    KirbySamusChargeShot = 151,
    /// `It_Kind_Kirby_SeakNeedleThrow`: Kirby's copy of Sheik's thrown needle.
    #[doc(alias("KirbySeakNeedleThrow"))]
    KirbySheikNeedleThrown = 152,
    /// `It_Kind_Kirby_SeakNeedleHeld`: Kirby's copy of Sheik's held needle.
    #[doc(alias("KirbySeakNeedleHeld"))]
    KirbySheikNeedleHeld = 153,
    /// `It_Kind_Kirby_KoopaFlame`: Kirby's copy of Bowser's flame.
    #[doc(alias("KirbyKoopaFlame"))]
    KirbyBowserFlame = 154,
    /// `It_Kind_Kirby_GameWatchChef`: Kirby's copy of Mr. Game & Watch's sausage.
    #[doc(alias("KirbyGameWatchChef"))]
    KirbyGameAndWatchSausage = 155,
    /// `It_Kind_Kirby_GameWatchChefPan`: Kirby's copy of Mr. Game & Watch's pan.
    #[doc(alias("KirbyGameWatchChefPan"))]
    KirbyGameAndWatchPan = 156,
    /// `It_Kind_Kirby_YoshiEggLay`: Kirby's copy of Yoshi's laid egg.
    KirbyYoshiEggLay = 157,
    /// `It_Kind_Unk4`: a Coin match's coin.
    #[doc(alias("Unk4"))]
    VsCoin = 158,
    /// `It_Kind_Coin`: a trophy on its stand, as the single-player modes set them out.
    #[doc(alias("Coin"))]
    Trophy = 159,
    /// The stages' hurtbox items (`It_PKind_Random` in the decompilation, the number the Poké
    /// Ball's random Pokémon would take; every use here is the stages' items).
    StageHurtbox = 160,
    /// `It_PKind_Tosakinto`: Goldeen.
    #[doc(alias("Tosakinto"))]
    Goldeen = 161,
    /// `It_PKind_Chicorita`: Chikorita.
    #[doc(alias("Chicorita"))]
    Chikorita = 162,
    /// `It_PKind_Kabigon`: Snorlax.
    #[doc(alias("Kabigon"))]
    Snorlax = 163,
    /// `It_PKind_Kamex`: Blastoise.
    #[doc(alias("Kamex"))]
    Blastoise = 164,
    /// `It_PKind_Matadogas`: Weezing.
    #[doc(alias("Matadogas"))]
    Weezing = 165,
    /// `It_PKind_Lizardon`: Charizard.
    #[doc(alias("Lizardon"))]
    Charizard = 166,
    /// `It_PKind_Fire`: Moltres.
    #[doc(alias("Fire"))]
    Moltres = 167,
    /// `It_PKind_Thunder`: Zapdos.
    #[doc(alias("Thunder"))]
    Zapdos = 168,
    /// `It_PKind_Freezer`: Articuno.
    #[doc(alias("Freezer"))]
    Articuno = 169,
    /// `It_PKind_Sonans`: Wobbuffet.
    #[doc(alias("Sonans"))]
    Wobbuffet = 170,
    /// `It_PKind_Hassam`: Scizor.
    #[doc(alias("Hassam"))]
    Scizor = 171,
    /// `It_PKind_Unknown`: Unown.
    #[doc(alias("Unknown"))]
    Unown = 172,
    /// `It_PKind_Entei`: Entei.
    Entei = 173,
    /// `It_PKind_Raikou`: Raikou.
    Raikou = 174,
    /// `It_PKind_Suikun`: Suicune.
    #[doc(alias("Suikun"))]
    Suicune = 175,
    /// `It_PKind_Kireihana`: Bellossom.
    #[doc(alias("Kireihana"))]
    Bellossom = 176,
    /// `It_PKind_Marumine`: Electrode.
    #[doc(alias("Marumine"))]
    Electrode = 177,
    /// `It_PKind_Lugia`: Lugia.
    Lugia = 178,
    /// `It_PKind_Houou`: Ho-Oh.
    #[doc(alias("Houou"))]
    HoOh = 179,
    /// `It_PKind_Metamon`: Ditto.
    #[doc(alias("Metamon"))]
    Ditto = 180,
    /// `It_PKind_Pippi`: Clefairy.
    #[doc(alias("Pippi"))]
    Clefairy = 181,
    /// `It_PKind_Togepy`: Togepi.
    #[doc(alias("Togepy"))]
    Togepi = 182,
    /// `It_PKind_Mew`: Mew.
    Mew = 183,
    /// `It_PKind_Cerebi`: Celebi.
    #[doc(alias("Cerebi"))]
    Celebi = 184,
    /// `It_PKind_Hitodeman`: Staryu.
    #[doc(alias("Hitodeman"))]
    Staryu = 185,
    /// `It_PKind_Lucky`: Chansey.
    #[doc(alias("Lucky"))]
    Chansey = 186,
    /// `It_PKind_Porygon2`: Porygon2.
    Porygon2 = 187,
    /// `It_PKind_Hinoarashi`: Cyndaquil.
    #[doc(alias("Hinoarashi"))]
    Cyndaquil = 188,
    /// `It_PKind_Maril`: Marill.
    #[doc(alias("Maril"))]
    Marill = 189,
    /// `It_PKind_Fushigibana`: Venusaur.
    #[doc(alias("Fushigibana"))]
    Venusaur = 190,
    /// `It_Kind_Chicorita_Leaf`: Chikorita's leaf.
    #[doc(alias("ChicoritaLeaf"))]
    ChikoritaLeaf = 191,
    /// `It_Kind_Kamex_HydroPump`: Blastoise's Hydro Pump.
    #[doc(alias("KamexHydroPump"))]
    BlastoiseHydroPump = 192,
    /// `It_Kind_Matadogas_Gas1`: Weezing's first gas.
    #[doc(alias("MatadogasGas1"))]
    WeezingGas1 = 193,
    /// `It_Kind_Matadogas_Gas2`: Weezing's second gas.
    #[doc(alias("MatadogasGas2"))]
    WeezingGas2 = 194,
    /// `It_Kind_Lizardon_Flame1`: Charizard's first flame.
    #[doc(alias("LizardonFlame1"))]
    CharizardFlame1 = 195,
    /// `It_Kind_Lizardon_Flame2`: Charizard's second flame.
    #[doc(alias("LizardonFlame2"))]
    CharizardFlame2 = 196,
    /// `It_Kind_Lizardon_Flame3`: Charizard's third flame.
    #[doc(alias("LizardonFlame3"))]
    CharizardFlame3 = 197,
    /// `It_Kind_Lizardon_Flame4`: Charizard's fourth flame.
    #[doc(alias("LizardonFlame4"))]
    CharizardFlame4 = 198,
    /// `It_Kind_Unknown_Swarm`: Unown's little Unown.
    #[doc(alias("UnknownSwarm"))]
    UnownSwarm = 199,
    /// `It_Kind_Lugia_Aeroblast`: Lugia's Aeroblast.
    LugiaAeroblast = 200,
    /// `It_Kind_Lugia_Aeroblast2`: Lugia's second Aeroblast.
    LugiaAeroblast2 = 201,
    /// `It_Kind_Lugia_Aeroblast3`: Lugia's third Aeroblast.
    LugiaAeroblast3 = 202,
    /// `It_Kind_Houou_SacredFire`: Ho-Oh's Sacred Fire.
    #[doc(alias("HououSacredFire"))]
    HoOhSacredFire = 203,
    /// `It_Kind_Hitodeman_Star`: Staryu's star.
    #[doc(alias("HitodemanStar"))]
    StaryuStar = 204,
    /// `It_Kind_Lucky_Egg`: Chansey's egg.
    #[doc(alias("LuckyEgg"))]
    ChanseyEgg = 205,
    /// `It_Kind_Hinoarashi_Flame`: Cyndaquil's flame.
    #[doc(alias("HinoarashiFlame"))]
    CyndaquilFlame = 206,
    /// `It_Kind_Pokemon_Unk`: a Pokémon's item of unknown use.
    #[doc(alias("PokemonUnk"))]
    PokemonUnused = 207,
    /// `It_Kind_Old_Kuri`: the old Goomba.
    #[doc(alias("OldKuri"))]
    OldGoomba = 208,
    /// `It_Kind_Mato`: Target Test's targets.
    #[doc(alias("Mato"))]
    Target = 209,
    /// `It_Kind_Heiho`: Yoshi's Story's Shy Guys.
    #[doc(alias("Heiho"))]
    ShyGuy = 210,
    /// `It_Kind_Nokonoko`: the Koopa Troopa.
    #[doc(alias("Nokonoko"))]
    KoopaTroopa = 211,
    /// `It_Kind_Patapata`: the Koopa Paratroopa.
    #[doc(alias("Patapata"))]
    KoopaParatroopa = 212,
    /// `It_Kind_Likelike`: the Like Like.
    #[doc(alias("Likelike"))]
    LikeLike = 213,
    /// `It_Kind_Old_Lead`: the old ReDead (unused).
    #[doc(alias("OldLead"))]
    OldReDead = 214,
    /// `It_Kind_Old_Octa`: the old Octorok (unused).
    #[doc(alias("OldOcta"))]
    OldOctorok = 215,
    /// `It_Kind_Old_Otto`: the old Topi.
    #[doc(alias("OldOtto"))]
    OldTopi = 216,
    /// `It_Kind_Whitebea`: the Polar Bear.
    #[doc(alias("Whitebea"))]
    PolarBear = 217,
    /// `It_Kind_Klap`: Kongo Jungle's Klap Trap.
    #[doc(alias("Klap"))]
    KlapTrap = 218,
    /// `It_Kind_ZGShell`: the Koopas' green shell.
    #[doc(alias("ZGShell"))]
    KoopaGreenShell = 219,
    /// `It_Kind_ZRShell`: the Koopas' red shell.
    #[doc(alias("ZRShell"))]
    KoopaRedShell = 220,
    /// `It_Kind_Tincle`: Great Bay's Tingle.
    #[doc(alias("Tincle"))]
    Tingle = 221,
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
    #[doc(alias("Tools"))]
    FlatZoneTool = 230,
    /// `It_Kind_Invalid7`: unused.
    Invalid7 = 231,
    /// `It_Kind_Invalid8`: unused.
    Invalid8 = 232,
    /// `It_Kind_Kyasarin`: Birdo.
    #[doc(alias("Kyasarin"))]
    Birdo = 233,
    /// `It_Kind_Arwing_Laser`: Corneria's Arwings' laser.
    ArwingLaser = 234,
    /// `It_Kind_GreatFox_Laser`: the Great Fox's laser.
    GreatFoxLaser = 235,
    /// `It_Kind_Kyasarin_Egg`: Birdo's egg.
    #[doc(alias("KyasarinEgg"))]
    BirdoEgg = 236,
}

impl ItKind {
    /// How many there are.
    pub const COUNT: usize = 237;

    /// Every item kind, by number.
    pub const ALL: [ItKind; Self::COUNT] = [
        ItKind::Capsule,
        ItKind::Crate,
        ItKind::Barrel,
        ItKind::Egg,
        ItKind::PartyBall,
        ItKind::BarrelCannon,
        ItKind::BobOmb,
        ItKind::MrSaturn,
        ItKind::HeartContainer,
        ItKind::MaximTomato,
        ItKind::Starman,
        ItKind::HomeRunBat,
        ItKind::BeamSword,
        ItKind::Parasol,
        ItKind::GreenShell,
        ItKind::RedShell,
        ItKind::RayGun,
        ItKind::Freezie,
        ItKind::Food,
        ItKind::MotionSensorBomb,
        ItKind::Flipper,
        ItKind::SuperScope,
        ItKind::StarRod,
        ItKind::LipsStick,
        ItKind::Fan,
        ItKind::FireFlower,
        ItKind::SuperMushroom,
        ItKind::PoisonMushroom,
        ItKind::Hammer,
        ItKind::WarpStar,
        ItKind::ScrewAttack,
        ItKind::BunnyHood,
        ItKind::MetalBox,
        ItKind::CloakingDevice,
        ItKind::PokeBall,
        ItKind::RayGunShot,
        ItKind::StarRodStar,
        ItKind::LipsStickSpore,
        ItKind::SuperScopeShot,
        ItKind::FireFlowerFlame,
        ItKind::HammerHead,
        ItKind::LipsStickFlower,
        ItKind::EventYoshiEgg,
        ItKind::Goomba,
        ItKind::ReDead,
        ItKind::Octorok,
        ItKind::Topi,
        ItKind::OctorokStone,
        ItKind::MarioFireball,
        ItKind::DrMarioMegavitamin,
        ItKind::KirbyFinalCutterWave,
        ItKind::KirbyHammer,
        ItKind::KirbyCopyStar,
        ItKind::KirbySpitStar,
        ItKind::FoxLaser,
        ItKind::FalcoLaser,
        ItKind::FoxIllusion,
        ItKind::FalcoPhantasm,
        ItKind::LinkBomb,
        ItKind::YoungLinkBomb,
        ItKind::LinkBoomerang,
        ItKind::YoungLinkBoomerang,
        ItKind::LinkHookshot,
        ItKind::YoungLinkHookshot,
        ItKind::LinkArrow,
        ItKind::YoungLinkFireArrow,
        ItKind::NessPKFire,
        ItKind::NessPKFirePillar,
        ItKind::NessPKFlash,
        ItKind::NessPKThunder,
        ItKind::NessPKThunderTrail1,
        ItKind::NessPKThunderTrail2,
        ItKind::NessPKThunderTrail3,
        ItKind::NessPKThunderTrail4,
        ItKind::FoxBlaster,
        ItKind::FalcoBlaster,
        ItKind::LinkBow,
        ItKind::YoungLinkBow,
        ItKind::NessPKFlashExplosion,
        ItKind::SheikNeedleThrown,
        ItKind::SheikNeedleHeld,
        ItKind::PikachuThunder,
        ItKind::PichuThunder,
        ItKind::MarioCape,
        ItKind::DrMarioSuperSheet,
        ItKind::SheikVanish,
        ItKind::YoshiEggThrow,
        ItKind::YoshiEggLay,
        ItKind::YoshiStar,
        ItKind::PikachuThunderJoltGround,
        ItKind::PikachuThunderJoltAir,
        ItKind::PichuThunderJoltGround,
        ItKind::PichuThunderJoltAir,
        ItKind::SamusBomb,
        ItKind::SamusChargeShot,
        ItKind::SamusMissile,
        ItKind::SamusGrappleBeam,
        ItKind::SheikChain,
        ItKind::PeachBomberExplosion,
        ItKind::PeachTurnip,
        ItKind::BowserFlame,
        ItKind::NessBat,
        ItKind::NessYoyo,
        ItKind::PeachParasol,
        ItKind::PeachToad,
        ItKind::LuigiFireball,
        ItKind::IceClimbersIceShot,
        ItKind::IceClimbersBlizzard,
        ItKind::ZeldaDinsFire,
        ItKind::ZeldaDinsFireExplosion,
        ItKind::MewtwoDisable,
        ItKind::PeachToadSpore,
        ItKind::MewtwoShadowBall,
        ItKind::IceClimbersBelayRope,
        ItKind::GameAndWatchSprayer,
        ItKind::GameAndWatchManhole,
        ItKind::GameAndWatchTorch,
        ItKind::GameAndWatchParachute,
        ItKind::GameAndWatchTurtle,
        ItKind::GameAndWatchSparky,
        ItKind::GameAndWatchJudge,
        ItKind::GameAndWatchOil,
        ItKind::GameAndWatchSausage,
        ItKind::YoungLinkMilk,
        ItKind::GameAndWatchTrampoline,
        ItKind::MasterHandLaser,
        ItKind::MasterHandBullet,
        ItKind::CrazyHandLaser,
        ItKind::CrazyHandBullet,
        ItKind::CrazyHandBomb,
        ItKind::KirbyMarioFireball,
        ItKind::KirbyDrMarioMegavitamin,
        ItKind::KirbyLuigiFireball,
        ItKind::KirbyIceClimbersIceShot,
        ItKind::KirbyPeachToad,
        ItKind::KirbyPeachToadSpore,
        ItKind::KirbyFoxLaser,
        ItKind::KirbyFalcoLaser,
        ItKind::KirbyFoxBlaster,
        ItKind::KirbyFalcoBlaster,
        ItKind::KirbyLinkArrow,
        ItKind::KirbyYoungLinkFireArrow,
        ItKind::KirbyLinkBow,
        ItKind::KirbyYoungLinkBow,
        ItKind::KirbyMewtwoShadowBall,
        ItKind::KirbyNessPKFlash,
        ItKind::KirbyNessPKFlashExplosion,
        ItKind::KirbyPikachuThunderJoltGround,
        ItKind::KirbyPikachuThunderJoltAir,
        ItKind::KirbyPichuThunderJoltGround,
        ItKind::KirbyPichuThunderJoltAir,
        ItKind::KirbySamusChargeShot,
        ItKind::KirbySheikNeedleThrown,
        ItKind::KirbySheikNeedleHeld,
        ItKind::KirbyBowserFlame,
        ItKind::KirbyGameAndWatchSausage,
        ItKind::KirbyGameAndWatchPan,
        ItKind::KirbyYoshiEggLay,
        ItKind::VsCoin,
        ItKind::Trophy,
        ItKind::StageHurtbox,
        ItKind::Goldeen,
        ItKind::Chikorita,
        ItKind::Snorlax,
        ItKind::Blastoise,
        ItKind::Weezing,
        ItKind::Charizard,
        ItKind::Moltres,
        ItKind::Zapdos,
        ItKind::Articuno,
        ItKind::Wobbuffet,
        ItKind::Scizor,
        ItKind::Unown,
        ItKind::Entei,
        ItKind::Raikou,
        ItKind::Suicune,
        ItKind::Bellossom,
        ItKind::Electrode,
        ItKind::Lugia,
        ItKind::HoOh,
        ItKind::Ditto,
        ItKind::Clefairy,
        ItKind::Togepi,
        ItKind::Mew,
        ItKind::Celebi,
        ItKind::Staryu,
        ItKind::Chansey,
        ItKind::Porygon2,
        ItKind::Cyndaquil,
        ItKind::Marill,
        ItKind::Venusaur,
        ItKind::ChikoritaLeaf,
        ItKind::BlastoiseHydroPump,
        ItKind::WeezingGas1,
        ItKind::WeezingGas2,
        ItKind::CharizardFlame1,
        ItKind::CharizardFlame2,
        ItKind::CharizardFlame3,
        ItKind::CharizardFlame4,
        ItKind::UnownSwarm,
        ItKind::LugiaAeroblast,
        ItKind::LugiaAeroblast2,
        ItKind::LugiaAeroblast3,
        ItKind::HoOhSacredFire,
        ItKind::StaryuStar,
        ItKind::ChanseyEgg,
        ItKind::CyndaquilFlame,
        ItKind::PokemonUnused,
        ItKind::OldGoomba,
        ItKind::Target,
        ItKind::ShyGuy,
        ItKind::KoopaTroopa,
        ItKind::KoopaParatroopa,
        ItKind::LikeLike,
        ItKind::OldReDead,
        ItKind::OldOctorok,
        ItKind::OldTopi,
        ItKind::PolarBear,
        ItKind::KlapTrap,
        ItKind::KoopaGreenShell,
        ItKind::KoopaRedShell,
        ItKind::Tingle,
        ItKind::Invalid1,
        ItKind::Invalid2,
        ItKind::Invalid3,
        ItKind::WhispyApple,
        ItKind::WhispyHealApple,
        ItKind::Invalid4,
        ItKind::Invalid5,
        ItKind::Invalid6,
        ItKind::FlatZoneTool,
        ItKind::Invalid7,
        ItKind::Invalid8,
        ItKind::Birdo,
        ItKind::ArwingLaser,
        ItKind::GreatFoxLaser,
        ItKind::BirdoEgg,
    ];

    /// One past the common items (`It_Common_End`): the random spawner draws the kinds below.
    pub const COMMON_END: ItKind = ItKind::RayGunShot;

    /// The first Pokémon (`It_PKind_Start`), and the first kind of the Pokémon's tables.
    pub const FIRST_POKEMON: ItKind = ItKind::Goldeen;

    /// One past the last Pokémon (`It_PKind_Terminate`), the first of their items.
    pub const POKEMON_END: ItKind = ItKind::ChikoritaLeaf;

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
        kind: ItKind::Crate,
        name: "Crate",
        events: &[crate_box::run_event],
        preds: &[crate_box::run_pred],
        vars: VarsKind::Box,
    },
    ItemKind {
        kind: ItKind::Barrel,
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
        kind: ItKind::PartyBall,
        name: "Party Ball",
        events: &[kusudama::run_event],
        preds: &[kusudama::run_pred],
        vars: VarsKind::Kusudama,
    },
    ItemKind {
        kind: ItKind::BarrelCannon,
        name: "Barrel Cannon",
        events: &[tarucann::run_event],
        preds: &[tarucann::run_pred],
        vars: VarsKind::TaruCann,
    },
    ItemKind {
        kind: ItKind::BobOmb,
        name: "Bob-omb",
        events: &[bombhei::run_event],
        preds: &[bombhei::run_pred],
        vars: VarsKind::BombHei,
    },
    ItemKind {
        kind: ItKind::MrSaturn,
        name: "Mr. Saturn",
        events: &[dosei::run_event],
        preds: &[dosei::run_pred],
        vars: VarsKind::Dosei,
    },
    ItemKind {
        kind: ItKind::HeartContainer,
        name: "Heart Container",
        events: &[heart::run_event],
        preds: &[heart::run_pred],
        vars: VarsKind::Heart,
    },
    ItemKind {
        kind: ItKind::MaximTomato,
        name: "Maxim Tomato",
        events: &[tomato::run_event],
        preds: &[tomato::run_pred],
        vars: VarsKind::Tomato,
    },
    ItemKind {
        kind: ItKind::Starman,
        name: "Starman",
        events: &[star::run_event],
        preds: &[star::run_pred],
        vars: VarsKind::Star,
    },
    ItemKind {
        kind: ItKind::HomeRunBat,
        name: "Home-Run Bat",
        events: &[bat::run_event],
        preds: &[bat::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::BeamSword,
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
        kind: ItKind::GreenShell,
        name: "Green Shell",
        events: &[gshell::run_event],
        preds: &[gshell::run_pred],
        vars: VarsKind::GShell,
    },
    ItemKind {
        kind: ItKind::RedShell,
        name: "Red Shell",
        events: &[rshell::run_event],
        preds: &[rshell::run_pred],
        vars: VarsKind::RShell,
    },
    ItemKind {
        kind: ItKind::RayGun,
        name: "Ray Gun",
        events: &[lgun::run_event],
        preds: &[lgun::run_pred],
        vars: VarsKind::LGun,
    },
    ItemKind {
        kind: ItKind::Freezie,
        name: "Freezie",
        events: &[freeze::run_event],
        preds: &[freeze::run_pred],
        vars: VarsKind::Freeze,
    },
    ItemKind {
        kind: ItKind::Food,
        name: "Food",
        events: &[foods::run_event],
        preds: &[foods::run_pred],
        vars: VarsKind::Foods,
    },
    ItemKind {
        kind: ItKind::MotionSensorBomb,
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
        kind: ItKind::SuperScope,
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
        kind: ItKind::LipsStick,
        name: "Lip's Stick",
        events: &[lipstick::run_event],
        preds: &[lipstick::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::Fan,
        name: "Fan",
        events: &[harisen::run_event],
        preds: &[harisen::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::FireFlower,
        name: "Fire Flower",
        events: &[fflower::run_event],
        preds: &[fflower::run_pred],
        vars: VarsKind::FFlower,
    },
    ItemKind {
        kind: ItKind::SuperMushroom,
        name: "Super Mushroom",
        events: &[kinoko::run_event],
        preds: &[kinoko::run_pred],
        vars: VarsKind::Kinoko,
    },
    ItemKind {
        kind: ItKind::PoisonMushroom,
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
        kind: ItKind::WarpStar,
        name: "Warp Star",
        events: &[wstar::run_event],
        preds: &[wstar::run_pred],
        vars: VarsKind::Wstar,
    },
    ItemKind {
        kind: ItKind::ScrewAttack,
        name: "Screw Attack",
        events: &[rabbitc::run_event, scball::run_event],
        preds: &[rabbitc::run_pred, scball::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::BunnyHood,
        name: "Bunny Hood",
        events: &[rabbitc::run_event],
        preds: &[rabbitc::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::MetalBox,
        name: "Metal Box",
        events: &[metalb::run_event],
        preds: &[metalb::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::CloakingDevice,
        name: "Cloaking Device",
        events: &[spycloak::run_event],
        preds: &[spycloak::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::PokeBall,
        name: "Poké Ball",
        events: &[mball::run_event],
        preds: &[mball::run_pred],
        vars: VarsKind::MBall,
    },
    ItemKind {
        kind: ItKind::RayGunShot,
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
        kind: ItKind::LipsStickSpore,
        name: "Lip's Stick's spore",
        events: &[lipstickspore::run_event],
        preds: &[lipstickspore::run_pred],
        vars: VarsKind::LipstickSpore,
    },
    ItemKind {
        kind: ItKind::SuperScopeShot,
        name: "Super Scope's shot",
        events: &[sscopebeam::run_event],
        preds: &[sscopebeam::run_pred],
        vars: VarsKind::ScopeBeam,
    },
    ItemKind {
        kind: ItKind::FireFlowerFlame,
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
        kind: ItKind::LipsStickFlower,
        name: "Flower the Lip's Stick plants",
        events: &[flower::run_event],
        preds: &[flower::run_pred],
        vars: VarsKind::FFlowerFlame,
    },
    ItemKind {
        kind: ItKind::EventYoshiEgg,
        name: "Event 13's egg",
        events: &[evyoshiegg::run_event],
        preds: &[evyoshiegg::run_pred],
        vars: VarsKind::Egg,
    },
    ItemKind {
        kind: ItKind::Goomba,
        name: "Goomba",
        events: &[oldkuri::run_event, zako::run_event],
        preds: &[oldkuri::run_pred],
        vars: VarsKind::Zako,
    },
    ItemKind {
        kind: ItKind::ReDead,
        name: "ReDead",
        events: &[leadead::run_event],
        preds: &[leadead::run_pred],
        vars: VarsKind::Zako,
    },
    ItemKind {
        kind: ItKind::Octorok,
        name: "Octorok",
        events: &[octarock::run_event, zako::run_event],
        preds: &[octarock::run_pred],
        vars: VarsKind::Zako,
    },
    ItemKind {
        kind: ItKind::Topi,
        name: "Topi",
        events: &[oldottosea::run_event],
        preds: &[oldottosea::run_pred],
        vars: VarsKind::Zako,
    },
    ItemKind {
        kind: ItKind::OctorokStone,
        name: "Octorok's stone",
        events: &[octarockstone::run_event],
        preds: &[octarockstone::run_pred],
        vars: VarsKind::Zako,
    },
    ItemKind {
        kind: ItKind::MarioFireball,
        name: "Mario's fireball",
        events: &[mariofireball::run_event],
        preds: &[mariofireball::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::DrMarioMegavitamin,
        name: "Dr. Mario's pill",
        events: &[drmariopill::run_event],
        preds: &[drmariopill::run_pred],
        vars: VarsKind::DrMarioPill,
    },
    ItemKind {
        kind: ItKind::KirbyFinalCutterWave,
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
        kind: ItKind::KirbyCopyStar,
        name: "Star of Kirby's lost copy",
        events: &[kirbycopystar::run_event],
        preds: &[kirbycopystar::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::KirbySpitStar,
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
        kind: ItKind::YoungLinkBomb,
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
        kind: ItKind::YoungLinkBoomerang,
        name: "Young Link's boomerang",
        events: &[linkboomerang::run_event],
        preds: &[linkboomerang::run_pred],
        vars: VarsKind::LinkBoomerang,
    },
    ItemKind {
        kind: ItKind::LinkHookshot,
        name: "Link's hookshot",
        events: &[linkhookshot::run_event],
        preds: &[linkhookshot::run_pred],
        vars: VarsKind::LinkHookshot,
    },
    ItemKind {
        kind: ItKind::YoungLinkHookshot,
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
        kind: ItKind::YoungLinkFireArrow,
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
        kind: ItKind::NessPKFirePillar,
        name: "PK Fire's pillar",
        events: &[nesspkfirepillar::run_event],
        preds: &[nesspkfirepillar::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::NessPKFlash,
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
        kind: ItKind::NessPKThunderTrail1,
        name: "PK Thunder's first trail",
        events: &[nesspkthundertrail::run_event],
        preds: &[nesspkthundertrail::run_pred],
        vars: VarsKind::NessPKThunderTrail,
    },
    ItemKind {
        kind: ItKind::NessPKThunderTrail2,
        name: "PK Thunder's second trail",
        events: &[nesspkthundertrail::run_event],
        preds: &[nesspkthundertrail::run_pred],
        vars: VarsKind::NessPKThunderTrail,
    },
    ItemKind {
        kind: ItKind::NessPKThunderTrail3,
        name: "PK Thunder's third trail",
        events: &[nesspkthundertrail::run_event],
        preds: &[nesspkthundertrail::run_pred],
        vars: VarsKind::NessPKThunderTrail,
    },
    ItemKind {
        kind: ItKind::NessPKThunderTrail4,
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
        kind: ItKind::YoungLinkBow,
        name: "Young Link's bow",
        events: &[linkbow::run_event],
        preds: &[linkbow::run_pred],
        vars: VarsKind::LinkBow,
    },
    ItemKind {
        kind: ItKind::NessPKFlashExplosion,
        name: "PK Flash's explosion",
        events: &[nesspkflashexplode::run_event],
        preds: &[nesspkflashexplode::run_pred],
        vars: VarsKind::PKFlushExplode,
    },
    ItemKind {
        kind: ItKind::SheikNeedleThrown,
        name: "Sheik's thrown needle",
        events: &[seakneedlethrown::run_event],
        preds: &[seakneedlethrown::run_pred],
        vars: VarsKind::SeakNeedleThrown,
    },
    ItemKind {
        kind: ItKind::SheikNeedleHeld,
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
        kind: ItKind::DrMarioSuperSheet,
        name: "Dr. Mario's sheet",
        events: &[mariocape::run_event],
        preds: &[mariocape::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::SheikVanish,
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
        kind: ItKind::PikachuThunderJoltGround,
        name: "Pikachu's Thunder Jolt on the ground",
        events: &[pikachutjoltground::run_event],
        preds: &[pikachutjoltground::run_pred],
        vars: VarsKind::PikachuJoltGround,
    },
    ItemKind {
        kind: ItKind::PikachuThunderJoltAir,
        name: "Pikachu's Thunder Jolt in the air",
        events: &[pikachutjoltair::run_event],
        preds: &[pikachutjoltair::run_pred],
        vars: VarsKind::PikachuJoltAir,
    },
    ItemKind {
        kind: ItKind::PichuThunderJoltGround,
        name: "Pichu's Thunder Jolt on the ground",
        events: &[pikachutjoltground::run_event],
        preds: &[pikachutjoltground::run_pred],
        vars: VarsKind::PikachuJoltGround,
    },
    ItemKind {
        kind: ItKind::PichuThunderJoltAir,
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
        kind: ItKind::SamusChargeShot,
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
        kind: ItKind::SamusGrappleBeam,
        name: "Samus's grapple beam",
        events: &[samusgrapple::run_event],
        preds: &[],
        vars: VarsKind::SamusGrapple,
    },
    ItemKind {
        kind: ItKind::SheikChain,
        name: "Sheik's chain",
        events: &[seakchain::run_event],
        preds: &[seakchain::run_pred],
        vars: VarsKind::SeakChain,
    },
    ItemKind {
        kind: ItKind::PeachBomberExplosion,
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
        kind: ItKind::BowserFlame,
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
        kind: ItKind::LuigiFireball,
        name: "Luigi's fireball",
        events: &[luigifireball::run_event],
        preds: &[luigifireball::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::IceClimbersIceShot,
        name: "Ice Climbers' ice",
        events: &[climbersice::run_event],
        preds: &[climbersice::run_pred],
        vars: VarsKind::ClimbersIce,
    },
    ItemKind {
        kind: ItKind::IceClimbersBlizzard,
        name: "Ice Climbers' Blizzard",
        events: &[climbersblizzard::run_event],
        preds: &[climbersblizzard::run_pred],
        vars: VarsKind::ClimbersBlizzard,
    },
    ItemKind {
        kind: ItKind::ZeldaDinsFire,
        name: "Zelda's Din's Fire",
        events: &[zeldadinfire::run_event],
        preds: &[zeldadinfire::run_pred],
        vars: VarsKind::ZeldaDinFire,
    },
    ItemKind {
        kind: ItKind::ZeldaDinsFireExplosion,
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
        kind: ItKind::IceClimbersBelayRope,
        name: "Ice Climbers' belay string",
        events: &[climbersstring::run_event],
        preds: &[climbersstring::run_pred],
        vars: VarsKind::ClimbersString,
    },
    ItemKind {
        kind: ItKind::GameAndWatchSprayer,
        name: "Mr. Game & Watch's Greenhouse spray",
        events: &[gamewatchgreenhouse::run_event],
        preds: &[gamewatchgreenhouse::run_pred],
        vars: VarsKind::GameWatch,
    },
    ItemKind {
        kind: ItKind::GameAndWatchManhole,
        name: "Mr. Game & Watch's manhole",
        events: &[gamewatchmanhole::run_event],
        preds: &[gamewatchmanhole::run_pred],
        vars: VarsKind::GameWatch,
    },
    ItemKind {
        kind: ItKind::GameAndWatchTorch,
        name: "Mr. Game & Watch's torch",
        events: &[gamewatchfire::run_event],
        preds: &[gamewatchfire::run_pred],
        vars: VarsKind::GameWatch,
    },
    ItemKind {
        kind: ItKind::GameAndWatchParachute,
        name: "Mr. Game & Watch's parachute",
        events: &[gamewatchparachute::run_event],
        preds: &[gamewatchparachute::run_pred],
        vars: VarsKind::GameWatch,
    },
    ItemKind {
        kind: ItKind::GameAndWatchTurtle,
        name: "Mr. Game & Watch's turtle",
        events: &[gamewatchturtle::run_event],
        preds: &[gamewatchturtle::run_pred],
        vars: VarsKind::GameWatch,
    },
    ItemKind {
        kind: ItKind::GameAndWatchSparky,
        name: "Mr. Game & Watch's Sparky",
        events: &[gamewatchbreath::run_event],
        preds: &[gamewatchbreath::run_pred],
        vars: VarsKind::GameWatch,
    },
    ItemKind {
        kind: ItKind::GameAndWatchJudge,
        name: "Mr. Game & Watch's Judge",
        events: &[gamewatchjudge::run_event],
        preds: &[gamewatchjudge::run_pred],
        vars: VarsKind::GameWatch,
    },
    ItemKind {
        kind: ItKind::GameAndWatchOil,
        name: "Mr. Game & Watch's Oil Panic",
        events: &[gamewatchpanic::run_event],
        preds: &[gamewatchpanic::run_pred],
        vars: VarsKind::GameWatch,
    },
    ItemKind {
        kind: ItKind::GameAndWatchSausage,
        name: "Mr. Game & Watch's sausage",
        events: &[gamewatchchef::run_event],
        preds: &[gamewatchchef::run_pred],
        vars: VarsKind::GameWatch,
    },
    ItemKind {
        kind: ItKind::YoungLinkMilk,
        name: "Young Link's milk",
        events: &[clinkmilk::run_event],
        preds: &[clinkmilk::run_pred],
        vars: VarsKind::CLinkMilk,
    },
    ItemKind {
        kind: ItKind::GameAndWatchTrampoline,
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
        kind: ItKind::KirbyMarioFireball,
        name: "Kirby's copy of Mario's fireball",
        events: &[mariofireball::run_event],
        preds: &[mariofireball::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::KirbyDrMarioMegavitamin,
        name: "Kirby's copy of Dr. Mario's pill",
        events: &[drmariopill::run_event],
        preds: &[drmariopill::run_pred],
        vars: VarsKind::DrMarioPill,
    },
    ItemKind {
        kind: ItKind::KirbyLuigiFireball,
        name: "Kirby's copy of Luigi's fireball",
        events: &[luigifireball::run_event],
        preds: &[luigifireball::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::KirbyIceClimbersIceShot,
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
        kind: ItKind::KirbyYoungLinkFireArrow,
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
        kind: ItKind::KirbyYoungLinkBow,
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
        kind: ItKind::KirbyNessPKFlash,
        name: "Kirby's copy of PK Flash",
        events: &[nesspkflash::run_event],
        preds: &[nesspkflash::run_pred],
        vars: VarsKind::PKFlush,
    },
    ItemKind {
        kind: ItKind::KirbyNessPKFlashExplosion,
        name: "Kirby's copy of PK Flash's explosion",
        events: &[nesspkflashexplode::run_event],
        preds: &[nesspkflashexplode::run_pred],
        vars: VarsKind::PKFlushExplode,
    },
    ItemKind {
        kind: ItKind::KirbyPikachuThunderJoltGround,
        name: "Kirby's copy of Pikachu's Thunder Jolt on the ground",
        events: &[pikachutjoltground::run_event],
        preds: &[pikachutjoltground::run_pred],
        vars: VarsKind::PikachuJoltGround,
    },
    ItemKind {
        kind: ItKind::KirbyPikachuThunderJoltAir,
        name: "Kirby's copy of Pikachu's Thunder Jolt in the air",
        events: &[pikachutjoltair::run_event],
        preds: &[pikachutjoltair::run_pred],
        vars: VarsKind::PikachuJoltAir,
    },
    ItemKind {
        kind: ItKind::KirbyPichuThunderJoltGround,
        name: "Kirby's copy of Pichu's Thunder Jolt on the ground",
        events: &[pikachutjoltground::run_event],
        preds: &[pikachutjoltground::run_pred],
        vars: VarsKind::PikachuJoltGround,
    },
    ItemKind {
        kind: ItKind::KirbyPichuThunderJoltAir,
        name: "Kirby's copy of Pichu's Thunder Jolt in the air",
        events: &[pikachutjoltair::run_event],
        preds: &[pikachutjoltair::run_pred],
        vars: VarsKind::PikachuJoltAir,
    },
    ItemKind {
        kind: ItKind::KirbySamusChargeShot,
        name: "Kirby's copy of Samus's Charge Shot",
        events: &[samuschargeshot::run_event],
        preds: &[samuschargeshot::run_pred],
        vars: VarsKind::SamusChargeshot,
    },
    ItemKind {
        kind: ItKind::KirbySheikNeedleThrown,
        name: "Kirby's copy of Sheik's thrown needle",
        events: &[seakneedlethrown::run_event],
        preds: &[seakneedlethrown::run_pred],
        vars: VarsKind::SeakNeedleThrown,
    },
    ItemKind {
        kind: ItKind::KirbySheikNeedleHeld,
        name: "Kirby's copy of Sheik's held needle",
        events: &[seakneedleheld::run_event],
        preds: &[seakneedleheld::run_pred],
        vars: VarsKind::SeakNeedleHeld,
    },
    ItemKind {
        kind: ItKind::KirbyBowserFlame,
        name: "Kirby's copy of Bowser's flame",
        events: &[koopaflame::run_event],
        preds: &[koopaflame::run_pred],
        vars: VarsKind::KoopaFlame,
    },
    ItemKind {
        kind: ItKind::KirbyGameAndWatchSausage,
        name: "Kirby's copy of Mr. Game & Watch's sausage",
        events: &[gamewatchchef::run_event],
        preds: &[gamewatchchef::run_pred],
        vars: VarsKind::GameWatch,
    },
    ItemKind {
        kind: ItKind::KirbyGameAndWatchPan,
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
        kind: ItKind::VsCoin,
        name: "Coin match's coin",
        events: &[coin::run_event],
        preds: &[coin::run_pred],
        vars: VarsKind::Coin,
    },
    ItemKind {
        kind: ItKind::Trophy,
        name: "Trophy",
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
        kind: ItKind::Goldeen,
        name: "Goldeen",
        events: &[tosakinto::run_event],
        preds: &[tosakinto::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Chikorita,
        name: "Chikorita",
        events: &[chicorita::run_event],
        preds: &[chicorita::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Snorlax,
        name: "Snorlax",
        events: &[kabigon::run_event],
        preds: &[kabigon::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Blastoise,
        name: "Blastoise",
        events: &[kamex::run_event],
        preds: &[kamex::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Weezing,
        name: "Weezing",
        events: &[matadogas::run_event],
        preds: &[matadogas::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Charizard,
        name: "Charizard",
        events: &[lizardon::run_event],
        preds: &[lizardon::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Moltres,
        name: "Moltres",
        events: &[fire::run_event],
        preds: &[fire::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Zapdos,
        name: "Zapdos",
        events: &[thunder::run_event],
        preds: &[thunder::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Articuno,
        name: "Articuno",
        events: &[freezer::run_event],
        preds: &[freezer::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Wobbuffet,
        name: "Wobbuffet",
        events: &[sonans::run_event],
        preds: &[sonans::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Scizor,
        name: "Scizor",
        events: &[hassam::run_event],
        preds: &[hassam::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Unown,
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
        kind: ItKind::Suicune,
        name: "Suicune",
        events: &[suikun::run_event],
        preds: &[suikun::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Bellossom,
        name: "Bellossom",
        events: &[kireihana::run_event],
        preds: &[kireihana::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Electrode,
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
        kind: ItKind::HoOh,
        name: "Ho-Oh",
        events: &[houou::run_event],
        preds: &[houou::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Ditto,
        name: "Ditto",
        events: &[metamon::run_event],
        preds: &[metamon::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Clefairy,
        name: "Clefairy",
        events: &[pippi::run_event],
        preds: &[pippi::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Togepi,
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
        kind: ItKind::Celebi,
        name: "Celebi",
        events: &[cerebi::run_event],
        preds: &[cerebi::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Staryu,
        name: "Staryu",
        events: &[hitodeman::run_event],
        preds: &[hitodeman::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Chansey,
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
        kind: ItKind::Cyndaquil,
        name: "Cyndaquil",
        events: &[hinoarashi::run_event],
        preds: &[hinoarashi::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Marill,
        name: "Marill",
        events: &[maril::run_event],
        preds: &[maril::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::Venusaur,
        name: "Venusaur",
        events: &[fushigibana::run_event],
        preds: &[fushigibana::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::ChikoritaLeaf,
        name: "Chikorita's leaf",
        events: &[chicorita::run_event],
        preds: &[chicorita::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::BlastoiseHydroPump,
        name: "Blastoise's Hydro Pump",
        events: &[kamex::run_event],
        preds: &[kamex::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::WeezingGas1,
        name: "Weezing's first gas",
        events: &[matadogas::run_event],
        preds: &[matadogas::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::WeezingGas2,
        name: "Weezing's second gas",
        events: &[matadogas::run_event],
        preds: &[matadogas::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::CharizardFlame1,
        name: "Charizard's first flame",
        events: &[lizardon::run_event],
        preds: &[lizardon::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::CharizardFlame2,
        name: "Charizard's second flame",
        events: &[lizardon::run_event],
        preds: &[lizardon::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::CharizardFlame3,
        name: "Charizard's third flame",
        events: &[lizardon::run_event],
        preds: &[lizardon::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::CharizardFlame4,
        name: "Charizard's fourth flame",
        events: &[lizardon::run_event],
        preds: &[lizardon::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::UnownSwarm,
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
        kind: ItKind::HoOhSacredFire,
        name: "Ho-Oh's Sacred Fire",
        events: &[houou::run_event],
        preds: &[houou::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::StaryuStar,
        name: "Staryu's star",
        events: &[hitodeman::run_event],
        preds: &[hitodeman::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::ChanseyEgg,
        name: "Chansey's egg",
        events: &[lucky::run_event],
        preds: &[lucky::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::CyndaquilFlame,
        name: "Cyndaquil's flame",
        events: &[maril::run_event],
        preds: &[maril::run_pred],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::PokemonUnused,
        name: "A Pokémon's item of unknown use",
        events: &[],
        preds: &[],
        vars: VarsKind::Pokemon,
    },
    ItemKind {
        kind: ItKind::OldGoomba,
        name: "Old Goomba",
        events: &[oldkuri::run_event, zako::run_event],
        preds: &[oldkuri::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::Target,
        name: "Target Test's targets",
        events: &[mato::run_event, zako::run_event],
        preds: &[mato::run_pred],
        vars: VarsKind::Zako,
    },
    ItemKind {
        kind: ItKind::ShyGuy,
        name: "Yoshi's Story's Shy Guys",
        events: &[heiho::run_event, zako::run_event],
        preds: &[heiho::run_pred],
        vars: VarsKind::Heiho,
    },
    ItemKind {
        kind: ItKind::KoopaTroopa,
        name: "Koopa Troopa",
        events: &[nokonoko::run_event, zako::run_event],
        preds: &[nokonoko::run_pred],
        vars: VarsKind::Zako,
    },
    ItemKind {
        kind: ItKind::KoopaParatroopa,
        name: "Koopa Paratroopa",
        events: &[patapata::run_event, zako::run_event],
        preds: &[patapata::run_pred],
        vars: VarsKind::Zako,
    },
    ItemKind {
        kind: ItKind::LikeLike,
        name: "Like Like",
        events: &[likelike::run_event],
        preds: &[likelike::run_pred],
        vars: VarsKind::Zako,
    },
    ItemKind {
        kind: ItKind::OldReDead,
        name: "Old ReDead (unused)",
        events: &[],
        preds: &[],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::OldOctorok,
        name: "Old Octorok (unused)",
        events: &[],
        preds: &[],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::OldTopi,
        name: "Old Topi",
        events: &[oldottosea::run_event],
        preds: &[oldottosea::run_pred],
        vars: VarsKind::Bytes,
    },
    ItemKind {
        kind: ItKind::PolarBear,
        name: "Polar Bear",
        events: &[zako::run_event, whitebea::run_event],
        preds: &[whitebea::run_pred],
        vars: VarsKind::Zako,
    },
    ItemKind {
        kind: ItKind::KlapTrap,
        name: "Kongo Jungle's Klap Trap",
        events: &[klap::run_event],
        preds: &[klap::run_pred],
        vars: VarsKind::Klap,
    },
    ItemKind {
        kind: ItKind::KoopaGreenShell,
        name: "Koopas' green shell",
        events: &[zgshell::run_event],
        preds: &[zgshell::run_pred],
        vars: VarsKind::Zako,
    },
    ItemKind {
        kind: ItKind::KoopaRedShell,
        name: "Koopas' red shell",
        events: &[zgshell::run_event],
        preds: &[zgshell::run_pred],
        vars: VarsKind::Zako,
    },
    ItemKind {
        kind: ItKind::Tingle,
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
        kind: ItKind::FlatZoneTool,
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
        kind: ItKind::Birdo,
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
        kind: ItKind::BirdoEgg,
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
            l.forget,
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
            -1 < ItKind::Capsule && 0 == ItKind::Capsule && ItKind::Crate == 1 && 2 > ItKind::Crate
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
