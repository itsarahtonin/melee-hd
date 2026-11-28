// SPDX-License-Identifier: GPL-3.0-only
//
// Tests for melee-hd's port of HayBox's Melee mode (see haybox.rs for its origin and authors).
// Distributed under the GNU General Public License version 3 only.

//! The Melee mode checked against HayBox's own numbers: each coordinate its source states (the
//! comments of `Melee20Button.cpp`, as Melee's stick values times 10000, so 7375 is 0.7375, 59
//! of the report's units of 0.0125) for every modifier, shield, C-button and B combination in
//! every quadrant, and the SOCD sequences of HayBox's README.

use super::*;
use Button::*;

/// HayBox's published coordinates (its comments' values times 10000) as the report's units.
fn units(v: i32) -> i32 {
    assert_eq!(v % 125, 0, "{v} isn't a whole number of units");
    v / 125
}

fn press(buttons: &[Button]) -> Inputs {
    buttons
        .iter()
        .fold(Inputs::default(), |i, &b| i.with(b, true))
}

/// The stick's offsets from the centre, as signed units.
fn stick(o: &Output) -> (i32, i32) {
    (i32::from(o.stick_x) - 128, i32::from(o.stick_y) - 128)
}

fn c_stick(o: &Output) -> (i32, i32) {
    (i32::from(o.c_x) - 128, i32::from(o.c_y) - 128)
}

/// One report from a fresh mode (nothing held before).
fn once(buttons: &[Button]) -> Output {
    Melee20Button::default().update(press(buttons))
}

/// The four quadrants' directions: the buttons and the signs they give.
const QUADRANTS: [([Button; 2], (i32, i32)); 4] = [
    ([Lf1, Rf4], (1, 1)),
    ([Lf3, Rf4], (-1, 1)),
    ([Lf3, Lf2], (-1, -1)),
    ([Lf1, Lf2], (1, -1)),
];

/// Every line of `UpdateAnalogOutputs` that gives the main stick a diagonal coordinate, with
/// the buttons that select it: (modifier and other buttons, which quadrants, HayBox's x y).
#[test]
fn every_published_diagonal() {
    let up = [true, true, false, false];
    let down = [false, false, true, true];
    let all = [true; 4];
    type Row = (&'static [Button], [bool; 4], (i32, i32));
    let table: &[Row] = &[
        // q1/2 = 7000 7000, and q3/4 without a shield button.
        (&[], all, (7000, 7000)),
        // L, R, LS, and MS + q3/4 = 7000 6875
        (&[Lf4], down, (7000, 6875)),
        (&[Rf5], down, (7000, 6875)),
        (&[Rf7], down, (7000, 6875)),
        (&[Rf8], down, (7000, 6875)),
        (&[Lf4], up, (7000, 7000)),
        // MX + L, R, LS, and MS + q1/2/3/4 = 6375 3750
        (&[Lt1, Lf4], all, (6375, 3750)),
        (&[Lt1, Rf5], all, (6375, 3750)),
        (&[Lt1, Rf7], all, (6375, 3750)),
        (&[Lt1, Rf8], all, (6375, 3750)),
        // Mod X's up B angles.
        (&[Lt1], all, (7375, 3125)),
        (&[Lt1, Rt2], all, (7000, 3625)),
        (&[Lt1, Rt3], all, (7875, 4875)),
        (&[Lt1, Rt4], all, (7000, 5125)),
        (&[Lt1, Rt5], all, (6125, 5250)),
        (&[Lt1, Rf1], all, (9125, 3875)),
        (&[Lt1, Rf1, Rt2], all, (8750, 4500)),
        (&[Lt1, Rf1, Rt3], all, (8500, 5250)),
        (&[Lt1, Rf1, Rt4], all, (7375, 5375)),
        (&[Lt1, Rf1, Rt5], all, (6375, 5375)),
        // MY + L, R, LS, and MS + q1/2 = 4750 8750, q3/4 = 5000 8500
        (&[Lt2, Lf4], up, (4750, 8750)),
        (&[Lt2, Rf8], up, (4750, 8750)),
        (&[Lt2, Lf4], down, (5000, 8500)),
        (&[Lt2, Rf7], down, (5000, 8500)),
        // Mod Y's up B angles.
        (&[Lt2], all, (3125, 7375)),
        (&[Lt2, Rt2], all, (3625, 7000)),
        (&[Lt2, Rt3], all, (4875, 7875)),
        (&[Lt2, Rt4], all, (5125, 7000)),
        (&[Lt2, Rt5], all, (6375, 7625)),
        (&[Lt2, Rf1], all, (3875, 9125)),
        (&[Lt2, Rf1, Rt2], all, (4500, 8750)),
        (&[Lt2, Rf1, Rt3], all, (5250, 8500)),
        (&[Lt2, Rf1, Rt4], all, (5875, 8000)),
        (&[Lt2, Rf1, Rt5], all, (5875, 7125)),
        // The turnaround neutral B nerf over a shielded Mod Y diagonal: x full.
        (&[Lt2, Rf1, Lf4], up, (10000, 8750)),
        (&[Lt2, Rf1, Lf4], down, (10000, 8500)),
    ];
    for (extra, quadrants, (x, y)) in table {
        for (q, (dirs, (sx, sy))) in QUADRANTS.iter().enumerate() {
            if !quadrants[q] {
                continue;
            }
            let held: Vec<Button> = dirs.iter().chain(extra.iter()).copied().collect();
            let o = once(&held);
            // A full 1.0 is 80 units, which the stick's byte holds.
            let want = (sx * units(*x).min(80), sy * units(*y));
            assert_eq!(stick(&o), want, "{held:?}");
        }
    }
}

/// The cardinals: 1.0 (48 and 208, the Melee mode's stick range), Mod X's 6625 and 5375, Mod
/// Y's 3375 and 7375, with or without a shield button ("even if shield is held").
#[test]
fn every_published_cardinal() {
    let table: &[(&[Button], i32, i32)] = &[
        (&[], 10000, 10000),
        (&[Lt1], 6625, 5375),
        (&[Lt1, Lf4], 6625, 5375),
        (&[Lt1, Rf7], 6625, 5375),
        (&[Lt2], 3375, 7375),
        (&[Lt2, Rf5], 3375, 7375),
        (&[Lt2, Rf8], 3375, 7375),
    ];
    for (extra, x, y) in table {
        for (dir, sign, horizontal) in [
            (Lf1, 1, true),
            (Lf3, -1, true),
            (Rf4, 1, false),
            (Lf2, -1, false),
        ] {
            let held: Vec<Button> = std::iter::once(dir).chain(extra.iter().copied()).collect();
            let want = if horizontal {
                (sign * units(*x).min(80), 0)
            } else {
                (0, sign * units(*y).min(80))
            };
            assert_eq!(stick(&once(&held)), want, "{held:?}");
        }
    }
    assert_eq!((once(&[Lf1]).stick_x, once(&[Lf3]).stick_x), (208, 48));
    // Mod Y with B: the turnaround neutral B nerf gives a full horizontal.
    assert_eq!(stick(&once(&[Lt2, Rf1, Lf3])), (-80, 0));
    assert_eq!(stick(&once(&[Lt2, Rf1, Rf4])), (0, units(7375)));
}

/// The C-stick: cardinals full, Mod X's angled forward smash (8500 5250, its vertical from the
/// main stick's), the diagonal 5250 8500 over every other C-stick modifier, and nothing in the
/// D-pad layer.
#[test]
fn every_published_c_stick_coordinate() {
    for (c, want) in [
        (Rt5, (80, 0)),
        (Rt3, (-80, 0)),
        (Rt4, (0, 80)),
        (Rt2, (0, -80)),
    ] {
        assert_eq!(c_stick(&once(&[c])), want);
        assert_eq!(c_stick(&once(&[c, Lt2])), want);
    }
    // Angled forward smash: Mod X with C-left or C-right, tilted as the main stick is.
    for (dirs, sy) in [(&[][..], 0), (&[Rf4][..], 1), (&[Lf2][..], -1)] {
        for (c, sx) in [(Rt5, 1), (Rt3, -1)] {
            let held: Vec<Button> = dirs.iter().copied().chain([Lt1, c]).collect();
            assert_eq!(
                c_stick(&once(&held)),
                (sx * units(8500), sy * units(5250)),
                "{held:?}"
            );
        }
    }
    // Mod X with C-up or C-down alone: a full vertical.
    assert_eq!(c_stick(&once(&[Lt1, Rt4])), (0, 80));
    // The diagonal, over angled forward smash too.
    for (cs, (sx, sy)) in [
        ([Rt5, Rt4], (1, 1)),
        ([Rt3, Rt4], (-1, 1)),
        ([Rt3, Rt2], (-1, -1)),
        ([Rt5, Rt2], (1, -1)),
    ] {
        for extra in [&[][..], &[Lt1][..], &[Lt1, Rf4][..], &[Lt2][..]] {
            let held: Vec<Button> = cs.iter().chain(extra.iter()).copied().collect();
            assert_eq!(
                c_stick(&once(&held)),
                (sx * units(5250), sy * units(8500)),
                "{held:?}"
            );
        }
    }
    // The D-pad layer turns the C-stick off and its buttons into the D-pad.
    let o = once(&[Lt1, Lt2, Rt4, Rt5]);
    assert_eq!(c_stick(&o), (0, 0));
    assert!(o.dpad_up && o.dpad_right && !o.dpad_down && !o.dpad_left);
    let o = once(&[Lt1, Lt2, Rt2, Rt3]);
    assert!(o.dpad_down && o.dpad_left && !o.dpad_up && !o.dpad_right);
    // Without both modifiers, the C-buttons press no D-pad.
    let o = once(&[Lt1, Rt4]);
    assert!(!o.dpad_up);
    // MB3 and MB2 press D-pad left and right in any layer.
    let o = once(&[Mb3, Mb2]);
    assert!(o.dpad_left && o.dpad_right && !o.dpad_up && !o.dpad_down);
}

/// The buttons, the triggers' analog values (light shield 49, mid shield 94, L and R 140 with
/// their click) and Start.
#[test]
fn buttons_and_triggers() {
    let o = once(&[Rt1, Rf1, Rf2, Rf6, Rf3, Mb1]);
    assert!(o.a && o.b && o.x && o.y && o.z && o.start);
    assert!(!o.l && !o.r && o.l_analog == 0 && o.r_analog == 0);
    let o = once(&[Rf7]);
    assert_eq!((o.r, o.r_analog, o.l_analog), (false, 49, 0));
    let o = once(&[Rf8]);
    assert_eq!((o.r, o.r_analog), (false, 94));
    // Mid shield after light shield in the source: 94 with both.
    assert_eq!(once(&[Rf7, Rf8]).r_analog, 94);
    let o = once(&[Rf5, Rf7]);
    assert_eq!((o.r, o.r_analog), (true, 140));
    let o = once(&[Lf4]);
    assert_eq!((o.l, o.l_analog, o.r_analog), (true, 140, 0));
    // Nothing held: the report's rest.
    assert_eq!(once(&[]), Output::default());
}

/// The crouch-walk option select and a custom air dodge, HayBox's two coordinate options.
#[test]
fn melee_options() {
    let cw = MeleeOptions {
        crouch_walk_os: true,
        ..MeleeOptions::default()
    };
    let o = Melee20Button::new(cw).update(press(&[Lf1, Lf2]));
    assert_eq!(stick(&o), (units(7000), -units(6875)));
    let o = Melee20Button::new(cw).update(press(&[Lf1, Rf4]));
    assert_eq!(stick(&o), (units(7000), units(7000)));
    let ad = MeleeOptions {
        custom_airdodge: Some((48, 36)),
        ..MeleeOptions::default()
    };
    let o = Melee20Button::new(ad).update(press(&[Lt1, Lf4, Lf3, Lf2]));
    assert_eq!(stick(&o), (-48, -36));
    let off = MeleeOptions {
        disable_ledgedash_socd_override: true,
        ..MeleeOptions::default()
    };
    let mut m = Melee20Button::new(off);
    m.update(press(&[Lf3, Lt1]));
    assert_eq!(stick(&m.update(press(&[Lf3, Lf1, Lt1]))), (units(6625), 0));
}

/// The SOCD sequences HayBox's README gives for its resolutions, on left and right.
#[test]
fn socd_sequences() {
    // Each step: the buttons held, then the stick's x (in units) for each resolution.
    type Step = (&'static [Button], [i32; 5]);
    let steps: &[Step] = &[
        // Left, then left and right: the second wins under both priorities; neutral gives
        // nothing; each fixed priority its own.
        (&[Lf3], [-80, -80, -80, -80, -80]),
        (&[Lf3, Lf1], [80, 80, 0, -80, 80]),
        // Releasing right: 2IP gives back left; without reactivation, neutral.
        (&[Lf3], [-80, 0, -80, -80, -80]),
        // Still left: no reactivation until left is pressed again.
        (&[Lf3], [-80, 0, -80, -80, -80]),
        (&[], [0, 0, 0, 0, 0]),
        (&[Lf3], [-80, -80, -80, -80, -80]),
        // Right first, then left: left wins; releasing left, right comes back only with 2IP.
        (&[], [0, 0, 0, 0, 0]),
        (&[Lf1], [80, 80, 80, 80, 80]),
        (&[Lf1, Lf3], [-80, -80, 0, -80, 80]),
        (&[Lf1], [80, 0, 80, 80, 80]),
        // Released and pressed again, right reactivates.
        (&[], [0, 0, 0, 0, 0]),
        (&[Lf1], [80, 80, 80, 80, 80]),
    ];
    let kinds = [
        SocdType::SecondInputPriority,
        SocdType::SecondInputPriorityNoReactivation,
        SocdType::Neutral,
        SocdType::Dir1Priority,
        SocdType::Dir2Priority,
    ];
    let off = MeleeOptions {
        // The ledgedash override would hide the resolution while both are held.
        disable_ledgedash_socd_override: true,
        ..MeleeOptions::default()
    };
    for (k, kind) in kinds.into_iter().enumerate() {
        let mut m = Melee20Button::with_socd(off, kind);
        for (n, (held, want)) in steps.iter().enumerate() {
            let o = m.update(press(held));
            assert_eq!(stick(&o).0, want[k], "{kind:?}, step {n}: {held:?}");
        }
    }
}

/// The Melee mode's default resolution on every pair, and the ledgedash override: left and
/// right held with no vertical give a full horizontal toward the one winning, over Mod X and
/// Mod Y.
#[test]
fn the_melee_modes_socd() {
    let mut m = Melee20Button::default();
    m.update(press(&[Rf4]));
    // Up, then down: down; down released: neutral, not up, until up is pressed again.
    assert_eq!(stick(&m.update(press(&[Rf4, Lf2]))).1, -80);
    assert_eq!(stick(&m.update(press(&[Rf4]))).1, 0);
    assert_eq!(stick(&m.update(press(&[Rf4]))).1, 0);
    m.update(press(&[]));
    assert_eq!(stick(&m.update(press(&[Rf4]))).1, 80);
    // Up released while down is held: down stays.
    assert_eq!(stick(&m.update(press(&[Rf4, Lf2]))).1, -80);
    assert_eq!(stick(&m.update(press(&[Lf2]))).1, -80);
    // The C-stick's pairs.
    let mut m = Melee20Button::default();
    m.update(press(&[Rt3]));
    assert_eq!(c_stick(&m.update(press(&[Rt3, Rt5]))), (80, 0));
    assert_eq!(c_stick(&m.update(press(&[Rt3]))), (0, 0));
    m.update(press(&[Rt4]));
    assert_eq!(c_stick(&m.update(press(&[Rt4, Rt2]))), (0, -80));
    // The ledgedash override, with either modifier, toward the direction pressed second.
    for modifier in [None, Some(Lt1), Some(Lt2)] {
        let mut m = Melee20Button::default();
        let with = |b: &[Button]| -> Vec<Button> { b.iter().copied().chain(modifier).collect() };
        m.update(press(&with(&[Lf1])));
        assert_eq!(stick(&m.update(press(&with(&[Lf1, Lf3])))), (-80, 0));
        // With a vertical held, the modifiers' coordinates stand.
        let o = m.update(press(&with(&[Lf1, Lf3, Rf4])));
        let x = match modifier {
            None => units(7000),
            Some(Lt1) => units(7375),
            _ => units(3125),
        };
        assert_eq!(stick(&o).0, -x, "{modifier:?}");
    }
    // Reset forgets the memory: after it, left and right held are a new press of both
    // (neither, the override's direction 0), where without it right would still win.
    let mut kept = Melee20Button::default();
    kept.update(press(&[Lf3]));
    kept.update(press(&[Lf3, Lf1]));
    let mut reset = kept.clone();
    reset.reset();
    assert_eq!(stick(&kept.update(press(&[Lf3, Lf1]))).0, 80);
    assert_eq!(stick(&reset.update(press(&[Lf3, Lf1]))).0, 0);
}

/// The published table as a model of its own (one lookup per modifier, in the order HayBox's
/// source lets each override the last), against the port on every combination of directions,
/// modifiers, shield buttons, B and C-buttons, without SOCD conflicts.
#[test]
fn every_combination_matches_the_table() {
    // Mod X and Mod Y's up B angles, by C-button (none, down, left, up, right), then with B.
    const UPB_X: [(i32, i32); 5] = [
        (7375, 3125),
        (7000, 3625),
        (7875, 4875),
        (7000, 5125),
        (6125, 5250),
    ];
    const UPB_X_B: [(i32, i32); 5] = [
        (9125, 3875),
        (8750, 4500),
        (8500, 5250),
        (7375, 5375),
        (6375, 5375),
    ];
    const UPB_Y: [(i32, i32); 5] = [
        (3125, 7375),
        (3625, 7000),
        (4875, 7875),
        (5125, 7000),
        (6375, 7625),
    ];
    const UPB_Y_B: [(i32, i32); 5] = [
        (3875, 9125),
        (4500, 8750),
        (5250, 8500),
        (5875, 8000),
        (5875, 7125),
    ];
    let dir = [None, Some(-1), Some(1)];
    let mut checked = 0;
    for x in dir {
        for y in dir {
            for cx in dir {
                for cy in dir {
                    for bits in 0..(1u32 << 8) {
                        let bit = |n: u32| bits >> n & 1 == 1;
                        let (mx, my, b) = (bit(0), bit(1), bit(2));
                        let shields = [(Lf4, bit(3)), (Rf5, bit(4)), (Rf7, bit(5)), (Rf8, bit(6))];
                        let crouch_walk = bit(7);
                        let mut held = Vec::new();
                        let pick = |v: Option<i32>, neg: Button, pos: Button| match v {
                            Some(-1) => Some(neg),
                            Some(_) => Some(pos),
                            None => None,
                        };
                        held.extend(pick(x, Lf3, Lf1));
                        held.extend(pick(y, Lf2, Rf4));
                        held.extend(pick(cx, Rt3, Rt5));
                        held.extend(pick(cy, Rt2, Rt4));
                        held.extend(mx.then_some(Lt1));
                        held.extend(my.then_some(Lt2));
                        held.extend(b.then_some(Rf1));
                        held.extend(shields.iter().filter(|s| s.1).map(|s| s.0));
                        let options = MeleeOptions {
                            crouch_walk_os: crouch_walk,
                            ..MeleeOptions::default()
                        };
                        let o = Melee20Button::new(options).update(press(&held));

                        let shield = shields.iter().any(|s| s.1);
                        let (sx, sy) = (x.unwrap_or(0), y.unwrap_or(0));
                        let (scx, scy) = (cx.unwrap_or(0), cy.unwrap_or(0));
                        // The C-button that picks an up B angle: the last in the source wins.
                        let c = match (cx, cy) {
                            (Some(1), _) => 4,
                            (_, Some(1)) => 3,
                            (Some(-1), _) => 2,
                            (_, Some(-1)) => 1,
                            _ => 0,
                        };
                        let u = |(a, b): (i32, i32)| (units(a), units(b));
                        // The main stick, as unsigned magnitudes.
                        let (mut ax, mut ay) = match (x, y) {
                            (Some(_), Some(_)) => {
                                let mut v = if sy < 0 && (shield || crouch_walk) {
                                    u((7000, 6875))
                                } else {
                                    u((7000, 7000))
                                };
                                if mx {
                                    v = if shield {
                                        u((6375, 3750))
                                    } else if b {
                                        u(UPB_X_B[c])
                                    } else {
                                        u(UPB_X[c])
                                    };
                                }
                                if my {
                                    v = if shield {
                                        let v = if sy > 0 { (4750, 8750) } else { (5000, 8500) };
                                        // The turnaround nerf follows the shield's angle.
                                        (if b { 80 } else { units(v.0) }, units(v.1))
                                    } else if b {
                                        u(UPB_Y_B[c])
                                    } else {
                                        u(UPB_Y[c])
                                    };
                                }
                                v
                            }
                            (Some(_), None) => {
                                let mut v = 80;
                                if mx {
                                    v = units(6625);
                                }
                                if my {
                                    v = if b { 80 } else { units(3375) };
                                }
                                (v, 0)
                            }
                            (None, Some(_)) => {
                                let mut v = 80;
                                if mx {
                                    v = units(5375);
                                }
                                if my {
                                    v = units(7375);
                                }
                                (0, v)
                            }
                            (None, None) => (0, 0),
                        };
                        if x.is_none() {
                            ax = 0;
                        }
                        if y.is_none() {
                            ay = 0;
                        }
                        // The C-stick.
                        let mut c_stick_want = (scx * 80, scy * 80);
                        if mx && scx != 0 {
                            c_stick_want = (scx * units(8500), sy * units(5250));
                        }
                        if scx != 0 && scy != 0 {
                            c_stick_want = (scx * units(5250), scy * units(8500));
                        }
                        if mx && my {
                            c_stick_want = (0, 0);
                        }
                        assert_eq!(stick(&o), (sx * ax, sy * ay), "{held:?} cw {crouch_walk}");
                        assert_eq!(c_stick(&o), c_stick_want, "{held:?}");
                        let r = if bit(4) {
                            140
                        } else if bit(6) {
                            94
                        } else if bit(5) {
                            49
                        } else {
                            0
                        };
                        assert_eq!(o.r_analog, r, "{held:?}");
                        assert_eq!(o.l_analog, if bit(3) { 140 } else { 0 }, "{held:?}");
                        assert_eq!((o.l, o.r, o.b), (bit(3), bit(4), b), "{held:?}");
                        checked += 1;
                    }
                }
            }
        }
    }
    assert_eq!(checked, 81 * 256);
}

/// Every one of the 2^22 ways to hold the buttons from rest gives a coordinate the published
/// table holds, and triggers HayBox sends.
#[test]
fn every_report_is_a_published_value() {
    let mut magnitudes: Vec<i32> = [
        7000, 6875, 6375, 3750, 7375, 3125, 3625, 7875, 4875, 5125, 6125, 5250, 9125, 3875, 8750,
        4500, 8500, 5375, 4750, 5000, 6625, 3375, 7625, 5875, 8000, 7125,
    ]
    .into_iter()
    .map(units)
    .collect();
    magnitudes.extend([0, 80]);
    for bits in 0..(1u32 << 22) {
        let mut i = Inputs::default();
        for (n, b) in Button::ALL.into_iter().enumerate() {
            i.set(b, bits >> n & 1 == 1);
        }
        let o = Melee20Button::default().update(i);
        let (x, y) = stick(&o);
        let (cx, cy) = c_stick(&o);
        for v in [x, y, cx, cy] {
            assert!(magnitudes.contains(&v.abs()), "{bits:#x}: {v}");
        }
        assert!([0, 49, 94, 140].contains(&o.r_analog));
        assert!([0, 140].contains(&o.l_analog));
        assert_eq!(o.l, o.l_analog == 140);
    }
}
