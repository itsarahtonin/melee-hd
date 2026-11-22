//! The item kinds' own code, a module per kind or family; the kinds themselves and what names
//! them are the registry's ([`super::registry`]).

pub mod arwinglaser;
pub mod bat;
#[doc(alias("kyasarin"))]
pub mod birdo;
#[doc(alias("kyasarinegg"))]
pub mod birdo_egg;
pub mod bombhei;
pub mod capsule;
pub mod cerebi;
pub mod chargedshot;
pub mod chicorita;
pub mod climbersblizzard;
pub mod climbersice;
pub mod climbersstring;
pub mod clinkmilk;
pub mod coin;
pub mod crate_box;
pub mod crazyhandbomb;
pub mod dosei;
pub mod drmariopill;
pub mod egg;
pub mod entei;
pub mod evyoshiegg;
pub mod fflower;
pub mod fire;
pub mod flipper;
pub mod flower;
pub mod foods;
pub mod foxblaster;
pub mod foxillusion;
pub mod foxlaser;
pub mod freeze;
pub mod freezer;
pub mod fushigibana;
pub mod gamewatch;
pub mod gamewatchbreath;
pub mod gamewatchchef;
pub mod gamewatchfire;
pub mod gamewatchgreenhouse;
pub mod gamewatchjudge;
pub mod gamewatchmanhole;
pub mod gamewatchpanic;
pub mod gamewatchparachute;
pub mod gamewatchrescue;
pub mod gamewatchturtle;
#[doc(alias("oldkuri"))]
pub mod goomba;
pub mod greatfoxlaser;
pub mod gshell;
pub mod hammer;
pub mod hammerhead;
pub mod harisen;
pub mod hassam;
pub mod heart;
pub mod held;
pub mod hinoarashi;
pub mod hitodeman;
pub mod houou;
pub mod kabigon;
pub mod kamex;
pub mod kinoko;
pub mod kirbycopystar;
pub mod kirbycutterbeam;
pub mod kirbygamewatchchefpan;
pub mod kirbyhammer;
pub mod kirbyinhale;
pub mod kirbyspitstar;
pub mod kireihana;
#[doc(alias("klap"))]
pub mod klap_trap;
#[doc(alias("patapata"))]
pub mod koopa_paratroopa;
#[doc(alias("nokonoko"))]
pub mod koopa_troopa;
pub mod koopaflame;
pub mod kusudama;
pub mod legendary;
pub mod lgun;
pub mod lgunbeam;
pub mod lgunray;
#[doc(alias("likelike"))]
pub mod like_like;
pub mod linkarrow;
pub mod linkbomb;
pub mod linkboomerang;
pub mod linkbow;
pub mod linkhookshot;
pub mod lipstick;
pub mod lipstickspore;
pub mod lizardon;
pub mod lucky;
pub mod lugia;
pub mod luigifireball;
pub mod maril;
pub mod mariocape;
pub mod mariofireball;
pub mod marumine;
pub mod masterhandbullet;
pub mod masterhandlaser;
pub mod matadogas;
pub mod mato;
pub mod mball;
pub mod metalb;
pub mod metamon;
pub mod mew;
pub mod mewtwodisable;
pub mod mewtwoshadowball;
pub mod msbomb;
pub mod nessbat;
pub mod nesspkfire;
pub mod nesspkfirepillar;
pub mod nesspkflash;
pub mod nesspkflashexplode;
pub mod nesspkthunder;
pub mod nesspkthundertrail;
pub mod nessyoyo;
pub mod octarockstone;
#[doc(alias("octarock"))]
pub mod octorok;
pub mod parasol;
pub mod peachexplode;
pub mod peachparasol;
pub mod peachtoad;
pub mod peachtoadspore;
pub mod peachturnip;
pub mod pikachuthunder;
pub mod pikachutjoltair;
pub mod pikachutjoltground;
pub mod pippi;
#[doc(alias("whitebea"))]
pub mod polar_bear;
pub mod porygon2;
pub mod rabbitc;
pub mod raikou;
#[doc(alias("leadead"))]
pub mod redead;
pub mod rshell;
pub mod samusbomb;
pub mod samuschargeshot;
pub mod samusgrapple;
pub mod samusmissile;
pub mod scball;
pub mod seakchain;
pub mod seakneedleheld;
pub mod seakneedlethrown;
pub mod seakvanish;
pub mod shell;
#[doc(alias("heiho"))]
pub mod shy_guy;
pub mod sonans;
pub mod spycloak;
pub mod sscope;
pub mod sscopebeam;
pub mod star;
pub mod starrod;
pub mod starrodstar;
pub mod steeredshot;
pub mod suikun;
pub mod sword;
pub mod taru;
pub mod tarucann;
pub mod tether;
pub mod thunder;
#[doc(alias("tincle"))]
pub mod tingle;
pub mod togepy;
pub mod tomato;
pub mod tools;
#[doc(alias("oldottosea"))]
pub mod topi;
pub mod tosakinto;
pub mod trophycoin;
pub mod unknown;
pub mod whispyapple;
pub mod wstar;
pub mod yaku;
pub mod yoshiegglay;
pub mod yoshieggthrow;
pub mod yoshistar;
pub mod yoshitongue;
pub mod zeldadinfire;
pub mod zeldadinfireexplode;
pub mod zgshell;
pub mod zrshell;

use super::item::{attr_of, common_data};
use super::{data, item, lib2725};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

/// Gravity from the item's attributes (`it_80272860` with its fall speed and limit), the
/// common items' falling states.
pub fn gravity(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, data::attr::FALL_SPEED);
    let max = a.f32(r.data, data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

/// Gravity and the spin it slows (`Item_ApplyFallingPhysics`), a thrown item's states.
pub fn falling_phys(r: &mut Running, gobj: GObjId) {
    gravity(r, gobj);
    let x68 = common_data(r.game).f32(r.data, data::common::X68);
    lib2725::clamp_fall(r.game, gobj, x68);
}
