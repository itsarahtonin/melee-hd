//! The items' `evt_unk` callbacks (`ItemLogic::evt_unk`), which only a fighter's unload runs
//! (`it_8026B7F8`, from `Fighter_Unload_8006DABC`): each item forgets the fighter going away.
//! No VS match unloads a fighter; Training's computer players leaving do, and the
//! single-player modes' players.

use super::kinds::{
    climbersstring, fflower, flipper, foxlaser, hassam, hitodeman, linkbomb, linkboomerang,
    linkhookshot, marumine, mewtwodisable, msbomb, peachturnip, seakchain, spycloak, star,
};
use super::{item, item_mut, lib26b1};
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

/// The items' process link.
const P_LINK_ITEM: u8 = 9;

/// Every item lets the fighter `fighter` go, `it_8026B7F8`: its kind's `evt_unk`, then the
/// item destroyed if it's a fighter's own (`x13`) that fighter owned before the callback.
/// The next item is the freed GObj's next, as the original reads it after the destroy.
#[doc(alias = "it_8026B7F8")]
pub fn fighter_unloaded(r: &mut Running, fighter: GObjId) {
    let mut cur = r.game.world.first_in(P_LINK_ITEM);
    while let Some(g) = cur {
        let ip = item(r.game, g);
        let owner = ip.owner;
        if let Some(f) = super::item::logic(r.game, g).evt_unk {
            run(r, g, Some(fighter), f);
        }
        if item(r.game, g).xDC8_word.flags.x13 && owner == Some(fighter) {
            super::item::destroy(r, g);
        }
        cur = r.game.world.gobj(g).next;
    }
}

/// Runs an item's `evt_unk` with the GObj going away.
pub fn run(r: &mut Running, gobj: GObjId, other: Option<GObjId>, f: CodeRef) {
    match f.0 {
        0x8029_0918 => msbomb::forget(r, gobj, other),
        0x8029_1BC0 => flipper::forget(r, gobj, other),
        0x8029_3640 => fflower::forget(r, gobj, other),
        a if FORGET_ONLY.iter().any(|&(x, _)| x == a) => forget(r, gobj, other),
        0x8028_4834 => star::forget(r, gobj, other),
        0x8029_5EB4 => spycloak::forget(r, gobj, other),
        0x8029_CCF8 => foxlaser::forget(r, gobj, other),
        0x8029_FD84 => linkbomb::evt_unk(r, gobj, other),
        0x802A_23CC => linkboomerang::evt_unk(r, gobj, other),
        0x802A_7D40 => linkhookshot::evt_unk(r, gobj, other),
        0x802B_2528 => pikachu_thunder(r, gobj, other),
        0x802B_54E4 => samus_bomb(r, gobj, other),
        0x802B_7104 => samus_missile(r, gobj, other),
        0x802B_AEA0 => samus_grapple(r, gobj, other),
        0x802B_D10C => seakchain::forget(r, gobj, other),
        0x802B_DA08 => peachturnip::evt_unk(r, gobj, other),
        0x802C_210C => climbers_ice(r, gobj, other),
        0x802C_3AA4 => climbersstring::forget(r, gobj, other),
        0x802C_4CD8 => mewtwodisable::evt_unk(r, gobj, other),
        0x802C_DC60 => hassam::evt_unk(r, gobj, other),
        0x802D_0AB0 => marumine::evt_unk(r, gobj, other),
        0x802D_43B0 => hitodeman::evt_unk(r, gobj, other),
        a => unreachable!("{a:#010x} is no item kind's evt_unk"),
    }
}

/// The GObj forgotten as every item does (`it_8026B894`): the `evt_unk` of each kind in
/// [`FORGET_ONLY`]; those aliased here are the ones the fighter unload's sessions reach
/// (docs/oracle.md, sessions), the rest held until a check reaches them.
#[doc(alias = "itHeart_Logic8_EvtUnk")]
#[doc(alias = "itBat_Logic11_EvtUnk")]
#[doc(alias = "itParasol_Logic13_EvtUnk")]
#[doc(alias = "itLGun_Logic16_EvtUnk")]
#[doc(alias = "itSScope_Logic21_EvtUnk")]
#[doc(alias = "itStarRod_Logic22_EvtUnk")]
#[doc(alias = "itLipstick_Logic23_EvtUnk")]
#[doc(alias = "itHarisen_Logic24_EvtUnk")]
#[doc(alias = "itWStar_Logic30_EvtUnk")]
fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// Pikachu's Thunder forgets its thrower too, `itPikachuThunder_Logic39_EvtUnk`.
// Ports `itPikachuThunder_Logic39_EvtUnk`; aliased once a check reaches it.
fn pikachu_thunder(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    let v = item_mut(r.game, gobj).xDD4_itemVar.pikachu_thunder();
    if v.x38.get() == other {
        v.x38.set(None);
    }
    lib26b1::forget(r.game, gobj, other);
}

/// Samus's bomb forgets its owner whoever goes, `itSamusBomb_Logic50_EvtUnk`.
// Ports `itSamusBomb_Logic50_EvtUnk`; aliased once a check reaches it.
fn samus_bomb(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .samus_bomb()
        .owner
        .set(None);
}

/// Samus's missile forgets its owner if it's the one going, `it_2725_Logic52_EvtUnk`.
// Ports `it_2725_Logic52_EvtUnk`; aliased once a check reaches it.
fn samus_missile(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
    let v = item_mut(r.game, gobj).xDD4_itemVar.samus_missile();
    if v.owner.get() == other {
        v.owner.set(None);
    }
}

/// Samus's grapple beam forgets its fighter if it's the one going,
/// `itSamusGrapple_Logic53_EvtUnk`.
// Ports `itSamusGrapple_Logic53_EvtUnk`; aliased once a check reaches it.
fn samus_grapple(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
    let v = item_mut(r.game, gobj).xDD4_itemVar.samus_grapple();
    if v.x8.get() == other {
        v.x8.set(None);
    }
}

/// The Ice Climbers' ice forgets its climber too, `itClimbersIce_Logic90_EvtUnk`.
// Ports `itClimbersIce_Logic90_EvtUnk`; aliased once a check reaches it.
fn climbers_ice(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    let v = item_mut(r.game, gobj).xDD4_itemVar.climbers_ice();
    if v.x0.get() == other {
        v.x0.set(None);
    }
    lib26b1::forget(r.game, gobj, other);
}

/// The kinds whose `evt_unk` only forgets the GObj as every item does (`it_8026B894`), by
/// address: ssbm-rs's port of each is that call alone (Ness's yo-yo's, `it_2725_Logic59_EvtUnk`,
/// also reads its owner and does nothing with it).
// Ports each name here not aliased on `forget` or its kind's own function; aliased once a
// check reaches it.
pub const FORGET_ONLY: &[(u32, &str)] = &[
    (0x8027_D650, "itCapsule_Logic0_EvtUnk"),
    (0x8028_1144, "itBombhei_Logic6_EvtUnk"),
    (0x8028_3AC4, "itDosei_Logic7_EvtUnk"),
    (0x8028_4194, "itHeart_Logic8_EvtUnk"),
    (0x8028_4624, "itTomato_Logic9_EvtUnk"),
    (0x8028_4D34, "itBat_Logic11_EvtUnk"),
    (0x8028_6068, "itSword_Logic12_EvtUnk"),
    (0x8028_7084, "itBox_Logic1_EvtUnk"),
    (0x8028_8C68, "itTaru_Logic2_EvtUnk"),
    (0x8028_96AC, "itEgg_Logic3_EvtUnk"),
    (0x8028_B06C, "itKusudama_Logic4_EvtUnk"),
    (0x8028_B8B8, "itParasol_Logic13_EvtUnk"),
    (0x8028_CFC0, "itGShell_Logic14_EvtUnk"),
    (0x8028_E718, "itRShell_Logic15_EvtUnk"),
    (0x8028_EB68, "itLGun_Logic16_EvtUnk"),
    (0x8028_F8C4, "itFreeze_Logic17_EvtUnk"),
    (0x8028_FE70, "itFoods_Logic18_EvtUnk"),
    (0x8029_0918, "itMSBomb_Logic19_EvtUnk"),
    (0x8029_1BC0, "itFlipper_EvtUnk"),
    (0x8029_2340, "itSScope_Logic21_EvtUnk"),
    (0x8029_27C8, "itStarRod_Logic22_EvtUnk"),
    (0x8029_2D28, "itHarisen_Logic24_EvtUnk"),
    (0x8029_3640, "itFFlower_Logic25_EvtUnk"),
    (0x8029_39E8, "itKinoko_Logic26_EvtUnk"),
    (0x8029_3D74, "itDKinoko_Logic27_EvtUnk"),
    (0x8029_4344, "itHammer_Logic28_EvtUnk"),
    (0x8029_4A90, "itWStar_Logic30_EvtUnk"),
    (0x8029_53A8, "itRabbitC_Logic31_EvtUnk"),
    (0x8029_56EC, "itMetalB_Logic32_EvtUnk"),
    (0x8029_5C28, "itLipstick_Logic23_EvtUnk"),
    (0x8029_78BC, "itTaruCann_Logic5_EvtUnk"),
    (0x8029_8148, "itMball_EvtUnk"),
    (0x8029_88C4, "itLGunRay_Logic35_EvtUnk"),
    (0x8029_8DCC, "itStarRodStar_Logic36_EvtUnk"),
    (0x8029_93C0, "itSScopeBeam_Logic38_EvtUnk"),
    (0x8029_9C28, "itLGunBeam_Logic39_EvtUnk"),
    (0x8029_A0F4, "itHammerHead_Logic40_EvtUnk"),
    (0x8029_A728, "itLipstickSpore_Logic37_EvtUnk"),
    (0x8029_B0A8, "itFFlowerFlame_Logic41_EvtUnk"),
    (0x8029_B6D8, "itEvYoshiEgg_Logic42_EvtUnk"),
    (0x8029_BA98, "itMarioFireball_Logic87_EvtUnk"),
    (0x8029_C4B4, "it_8029C4B4"),
    (0x8029_D948, "it_8029D948"),
    (0x802A_A034, "itLinkArrow_Logic98_EvtUnk"),
    (0x802A_A474, "it_802AA474"),
    (0x802A_A7C4, "itNesspkfirepillar_Logic24_EvtUnk"),
    (0x802A_B3D0, "itNesspkflash_Logic102_EvtUnk"),
    (0x802A_C41C, "it_802AC41C"),
    (0x802A_D458, "itKoopaFlame_Logic111_EvtUnk"),
    (0x802A_D9FC, "itNessBat_Logic58_EvtUnk"),
    (0x802A_DBE4, "it_802ADBE4"),
    (0x802A_DDB0, "it_802ADDB0"),
    (0x802A_F184, "itFoxBlaster_Logic96_EvtUnk"),
    (0x802A_F920, "itLinkBow_Logic100_EvtUnk"),
    (0x802A_FD6C, "itNessPKFlashExplode_Logic103_EvtUnk"),
    (0x802B_1890, "itSeakNeedleThrown_Logic109_EvtUnk"),
    (0x802B_1C40, "itSeakNeedleHeld_Logic110_EvtUnk"),
    (0x802B_1DCC, "it_802B1DCC"),
    (0x802B_2870, "it_802B2870"),
    (0x802B_2FA8, "it_802B2FA8"),
    (0x802B_3348, "it_802B3348"),
    (0x802B_3EDC, "itPikachuTJoltGround_Logic106_EvtUnk"),
    (0x802B_4AA8, "itPikachuTJoltAir_Logic107_EvtUnk"),
    (0x802B_62B0, "itSamusChargeshot_Logic108_EvtUnk"),
    (0x802B_D30C, "itPeachExplode_Logic55_EvtUnk"),
    (0x802B_DDF8, "itPeachParasol_Logic60_EvtUnk"),
    (0x802B_E1F4, "itPeachToad_Logic91_EvtUnk"),
    (0x802B_E578, "itPeachToadSpore_Logic92_EvtUnk"),
    (0x802C_016C, "it_2725_Logic59_EvtUnk"),
    (0x802C_04F0, "itLuigiFireball_Logic89_EvtUnk"),
    (0x802C_1570, "itDrMarioPill_EvtUnk"),
    (0x802C_246C, "itClimbersBlizzard_EvtUnk"),
    (0x802C_4560, "itZeldaDinFire_Logic65_EvtUnk"),
    (0x802C_49C0, "itZeldaDinFireExplode_Logic66_EvtUnk"),
    (0x802C_61D4, "itMewtwoShadowball_Logic101_EvtUnk"),
    (0x802C_65C4, "itGamewatchGreenhouse_EvtUnk"),
    (0x802C_68D8, "itGameWatchManhole_Logic72_EvtUnk"),
    (0x802C_6C18, "itGamewatchFire_EvtUnk"),
    (0x802C_6F20, "itGameWatchParachute_Logic74_EvtUnk"),
    (0x802C_71EC, "itGameWatchTurtle_Logic75_EvtUnk"),
    (0x802C_74B8, "itGameWatchBreath_Logic76_EvtUnk"),
    (0x802C_7754, "itKirbyGameWatchChefPan_Logic113_EvtUnk"),
    (0x802C_7D40, "itGameWatchJudge_Logic77_EvtUnk"),
    (0x802C_8018, "itGameWatchPanic_Logic78_EvtUnk"),
    (0x802C_835C, "itGameWatchRescue_Logic81_EvtUnk"),
    (0x802C_8B08, "itGameWatchChef_Logic112_EvtUnk"),
    (0x802C_8F2C, "itCLinkMilk_Logic80_EvtUnk"),
    (0x802C_8FC4, "it_802C8FC4"),
    (0x802C_961C, "itChicorita_Logic1_EvtUnk"),
    (0x802C_9C80, "itChicoritaLeaf_Logic30_EvtUnk"),
    (0x802C_9E24, "it_802C9E24"),
    (0x802C_A538, "it_802CA538"),
    (0x802C_ADD0, "itKamex_Logic31_EvtUnk"),
    (0x802C_B0F8, "it_802CB0F8"),
    (0x802C_B778, "itMatadogas_Logic33_EvtUnk"),
    (0x802C_B940, "it_802CB940"),
    (0x802C_C584, "itLizardon_Logic37_EvtUnk"),
    (0x802C_C744, "itFire_Logic6_EvtUnk"),
    (0x802C_CB14, "it_802CCB14"),
    (0x802C_D000, "itFreezer_802CD000"),
    (0x802C_D4DC, "it_802CD4DC"),
    (0x802C_E7D0, "it_802CE7D0"),
    (0x802C_F0B4, "itUnknown_Logic38_EvtUnk"),
    (0x802C_F450, "it_802CF450"),
    (0x802C_F884, "it_802CF884"),
    (0x802C_FCB8, "it_802CFCB8"),
    (0x802D_0118, "itKireihana_Logic15_EvtUnk"),
    (0x802D_14D4, "itLugia_Logic17_EvtUnk"),
    (0x802D_23D4, "it_802D23D4"),
    (0x802D_25BC, "it_802D25BC"),
    (0x802D_2ED0, "it_802D2ED0"),
    (0x802D_3070, "itMetamon_Logic19_EvtUnk"),
    (0x802D_32DC, "it_802D32DC"),
    (0x802D_372C, "itTogepy_Logic21_EvtUnk"),
    (0x802D_3B6C, "it_802D3B6C"),
    (0x802D_3F4C, "it_802D3F4C"),
    (0x802D_4F58, "it_802D4F58"),
    (0x802D_56F0, "it_802D56F0"),
    (0x802D_5A9C, "itLucky_Logic44_EvtUnk"),
    (0x802D_5CD8, "it_802D5CD8"),
    (0x802D_61A8, "it_802D61A8"),
    (0x802D_6850, "it_802D6850"),
    (0x802D_6FB0, "it_802D6FB0"),
    (0x802D_73D0, "itOldKuri_Logic29_EvtUnk"),
    (0x802D_84D8, "it_802D84D8"),
    (0x802D_9A0C, "it_802D9A0C"),
    (0x802D_D7D0, "it_802DD7D0"),
    (0x802D_FED4, "it_802DFED4"),
    (0x802E_0468, "it_802E0468"),
    (0x802E_16D8, "it_802E16D8"),
    (0x802E_2450, "it_802E2450"),
    (0x802E_3784, "it_802E3784"),
    (0x802E_4A24, "it_802E4A24"),
    (0x802E_5AA4, "it_802E5AA4"),
    (0x802E_6ACC, "it_2E5A_Logic115_EvtUnk"),
    (0x802E_709C, "it_2E6A_Logic117_EvtUnk"),
    (0x802E_89B0, "itOctarockstone_802E89B0"),
    (0x802E_B5A8, "it_802EB5A8"),
    (0x802E_C850, "it_802EC850"),
    (0x802E_E1E0, "it_802EE1E0"),
    (0x802E_EF88, "it_802EEF88"),
    (0x802E_FA24, "it_802EFA24"),
    (0x802F_0320, "it_802F0320"),
    (0x802F_044C, "itMasterHandLaser_Logic84_EvtUnk"),
    (0x802F_0BC8, "itMasterHandBullet_Logic85_EvtUnk"),
    (0x802F_1094, "itCrazyHandBomb_Logic86_EvtUnk"),
    (0x802F_18F0, "itCoin_Logic116_EvtUnk"),
    (0x802F_2BDC, "it_802F2BDC"),
    (0x802F_3374, "it_27CF_Logic114_EvtUnk"),
];

#[cfg(test)]
mod tests {
    /// The kinds `run` dispatches to their own ports.
    const OWN: &[u32] = &[
        0x8028_4834,
        0x8029_0918,
        0x8029_1BC0,
        0x8029_3640,
        0x8029_5EB4,
        0x8029_CCF8,
        0x8029_FD84,
        0x802A_23CC,
        0x802A_7D40,
        0x802B_2528,
        0x802B_54E4,
        0x802B_7104,
        0x802B_AEA0,
        0x802B_D10C,
        0x802B_DA08,
        0x802C_210C,
        0x802C_3AA4,
        0x802C_4CD8,
        0x802C_DC60,
        0x802D_0AB0,
        0x802D_43B0,
    ];

    /// Every item kind's `evt_unk` is dispatched: none falls through `run`.
    #[test]
    fn every_evt_unk_is_dispatched() {
        let mut n = 0;
        for g in 0..4 {
            for l in super::super::tables::logic_group(g) {
                if let Some(f) = l.evt_unk {
                    let forget = super::FORGET_ONLY.iter().any(|&(a, _)| a == f.0);
                    assert!(
                        forget || OWN.contains(&f.0),
                        "{:#010x} isn't dispatched",
                        f.0
                    );
                    n += 1;
                }
            }
        }
        assert!(n > 0);
    }
}
