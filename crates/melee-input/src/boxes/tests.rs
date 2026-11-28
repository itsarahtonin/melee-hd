//! A box's reports decoded to their exact bytes. With no recorded reports from a box, these
//! build HayBox's reports from its format (HayBox at 2cffcd23: `DInputBackend::SendReport` and
//! `TUGamepad`'s report for DInput, `XInputBackend::SendReport` for XInput) for every report
//! its Melee mode can send, take each through the system's and gilrs's arithmetic as their
//! sources do it (Windows Gaming Input's raw controller and gamepad, Linux's evdev, gilrs-core
//! 0.6.8's conversion and gilrs 0.11.2's `axis_value` and `btn_value`), and check the
//! `PADStatus` read back is the one the same report gives through a GameCube cable.

use super::*;
use melee_phob::haybox::{Button, Inputs, Melee20Button, Output};

/// gilrs's `axis_value` (gamepad.rs), without the vertical axes' turn, which the callers apply.
fn axis_value(min: i32, max: i32, val: i32) -> f32 {
    let mut range = max as f32 - min as f32;
    let mut val = val as f32 - min as f32;
    if let Some(i_range) = max.checked_sub(min)
        && i_range % 2 == 1
    {
        range += 1.0;
        val += 1.0;
    }
    val = val / range * 2.0 - 1.0;
    val.clamp(-1.0, 1.0)
}

/// gilrs's `btn_value`.
fn btn_value(min: i32, max: i32, val: i32) -> f32 {
    let range = max as f32 - min as f32;
    ((val as f32 - min as f32) / range).clamp(0.0, 1.0)
}

/// gilrs's turn of a vertical axis.
fn turn(v: f32) -> f32 {
    if v != 0.0 { -v } else { v }
}

/// HayBox's DInput report for a Melee mode report, as `DInputBackend::SendReport` fills
/// `TUGamepad`'s: report id 1; X, Y, Z, Rz, Rx, Ry as 16-bit values (each byte times 257, the
/// vertical axes flipped, the triggers plus one); the hat (TinyUSB's `GAMEPAD_HAT_*`, 0
/// centred, 1 up, then clockwise to 8 up-left); 16 buttons.
fn dinput_report(o: &Output) -> [u8; 16] {
    let mut buttons = 0u16;
    for (n, on) in [
        (0, o.b),
        (1, o.a),
        (2, o.y),
        (3, o.x),
        (4, o.z),
        (5, o.r),
        (7, o.l),
        (9, o.start),
    ] {
        if on {
            buttons |= 1 << n;
        }
    }
    let (l, r, d, u) = (o.dpad_left, o.dpad_right, o.dpad_down, o.dpad_up);
    // `TUGamepad::getHatPosition`.
    let hat: u8 = if r && !l {
        if d {
            4
        } else if u {
            2
        } else {
            3
        }
    } else if l && !r {
        if d {
            6
        } else if u {
            8
        } else {
            7
        }
    } else if d && !u {
        5
    } else if u && !d {
        1
    } else {
        0
    };
    let axes = [
        o.stick_x,
        255 - o.stick_y,
        o.l_analog + 1,
        o.r_analog + 1,
        o.c_x,
        255 - o.c_y,
    ];
    let mut out = [0u8; 16];
    out[0] = 1;
    for (i, a) in axes.iter().enumerate() {
        out[1 + 2 * i..3 + 2 * i].copy_from_slice(&(u16::from(*a) * 257).to_le_bytes());
    }
    out[13] = hat;
    out[14..16].copy_from_slice(&buttons.to_le_bytes());
    out
}

/// The report's HID values as the system's HID parser gives them: the six axes by usage in
/// DirectInput's order (X, Y, Z, Rx, Ry, Rz) over their logical range 0 to 65535, the hat as
/// (x, y) with up −1, the buttons.
fn parse(report: &[u8; 16]) -> ([u16; 6], (i32, i32), u16) {
    assert_eq!(report[0], 1);
    let field = |i: usize| u16::from_le_bytes([report[1 + 2 * i], report[2 + 2 * i]]);
    // The report's order is X, Y, Z, Rz, Rx, Ry.
    let axes = [field(0), field(1), field(2), field(4), field(5), field(3)];
    let hat = match report[13] {
        1 => (0, -1),
        2 => (1, -1),
        3 => (1, 0),
        4 => (1, 1),
        5 => (0, 1),
        6 => (-1, 1),
        7 => (-1, 0),
        8 => (-1, -1),
        _ => (0, 0),
    };
    (axes, hat, u16::from_le_bytes([report[14], report[15]]))
}

/// gilrs's values for the report on Windows: the raw controller's axes as 0 to 1 over the
/// logical range, which gilrs-core makes `(v × 65535 − 32768) as i32` with the range −32768 to
/// 32767; its default mapping (no SDL mapping lists these boxes) names axis 0 LeftStickX, 1
/// LeftStickY, 2 RightStickX, 3 and 4 the triggers (as buttons), 5 RightStickY, and the switch
/// DPadX and DPadY (gilrs-core sends its vertical negated). `changed_only` leaves out what
/// still holds the value the box connected with, as gilrs sees only changes.
fn wgi_values(report: &[u8; 16], changed_only: bool) -> Vec<(u32, Raw)> {
    let (axes, hat, buttons) = parse(report);
    let (rest_axes, _, _) = parse(&dinput_report(&Output::default()));
    let mut out = Vec::new();
    for (i, &a) in axes.iter().enumerate() {
        if changed_only && a == rest_axes[i] {
            continue;
        }
        let d = f64::from(a) / 65535.0;
        let val = (d * 65535.0 - 32768.0) as i32;
        let code = 1 << 16 | i as u32;
        out.push(match i {
            3 | 4 => (code, Raw::Travel(btn_value(-32768, 32767, val))),
            1 | 5 => {
                let v = turn(axis_value(-32768, 32767, val));
                (
                    code,
                    Raw::Axis {
                        value: v,
                        turned: v != 0.0,
                    },
                )
            }
            _ => (
                code,
                Raw::Axis {
                    value: axis_value(-32768, 32767, val),
                    turned: false,
                },
            ),
        });
    }
    // `direction_from_switch`: up is (0, 1), sent as y −1; DPadY turned by gilrs.
    let (hx, hy) = hat;
    if !changed_only || hx != 0 {
        out.push((
            2 << 16,
            Raw::Axis {
                value: axis_value(-1, 1, hx),
                turned: false,
            },
        ));
    }
    if !changed_only || hy != 0 {
        let v = turn(axis_value(-1, 1, hy));
        out.push((
            2 << 16 | 1,
            Raw::Axis {
                value: v,
                turned: v != 0.0,
            },
        ));
    }
    for n in 0..16 {
        let on = buttons >> n & 1 == 1;
        if changed_only && !on {
            continue;
        }
        out.push((n, Raw::Pressed(on)));
    }
    out
}

/// gilrs's values for the report on Linux: evdev's axes over the descriptor's logical range
/// (0 to 65535), ABS_X to ABS_RZ, the hat as ABS_HAT0X and ABS_HAT0Y, the buttons from
/// BTN_GAMEPAD (a gamepad's application); gilrs's default mapping turns ABS_Y, ABS_RY and
/// ABS_HAT0Y.
fn evdev_values(report: &[u8; 16]) -> Vec<(u32, Raw)> {
    let (axes, hat, buttons) = parse(report);
    let mut out = Vec::new();
    for (i, &a) in axes.iter().enumerate() {
        let v = axis_value(0, 65535, i32::from(a));
        let turned = matches!(i, 1 | 4);
        let v = if turned { turn(v) } else { v };
        out.push((
            3 << 16 | i as u32,
            Raw::Axis {
                value: v,
                turned: turned && v != 0.0,
            },
        ));
    }
    out.push((
        3 << 16 | 0x10,
        Raw::Axis {
            value: axis_value(-1, 1, hat.0),
            turned: false,
        },
    ));
    let v = turn(axis_value(-1, 1, hat.1));
    out.push((
        3 << 16 | 0x11,
        Raw::Axis {
            value: v,
            turned: v != 0.0,
        },
    ));
    for n in 0..16 {
        out.push((1 << 16 | (0x130 + n), Raw::Pressed(buttons >> n & 1 == 1)));
    }
    out
}

/// HayBox's XInput report, as `XInputBackend::SendReport` fills it: each stick
/// `(v − 128) × 65535 / 255 + 128` in C's integer arithmetic, each trigger 255 with its click
/// or else its analog value; then the values gilrs gives for it on Windows (the gamepad's
/// sticks as `v / scale`, gilrs-core times `i32::MAX`, the vertical ones negated there and
/// turned by gilrs; the triggers `t / 255` times `i32::MAX`, read as buttons over 0 to
/// `i32::MAX`). `scale` is 32767 or 32768: the reading is exact either way.
fn xinput(o: &Output, scale: f64) -> XInput {
    let stick = |v: u8| (i32::from(v) - 128) * 65535 / 255 + 128;
    let lt = if o.l { 255 } else { i32::from(o.l_analog) };
    let rt = if o.r { 255 } else { i32::from(o.r_analog) };
    let axis = |raw: i32, mult: f64| {
        let d = f64::from(raw) / scale;
        let val = (mult * d * f64::from(i32::MAX)) as i32;
        axis_value(i32::MIN, i32::MAX, val)
    };
    let trigger = |t: i32| {
        let val = (f64::from(t) / 255.0 * f64::from(i32::MAX)) as i32;
        btn_value(0, i32::MAX, val)
    };
    XInput {
        left: (
            axis(stick(o.stick_x), 1.0),
            turn(axis(stick(o.stick_y), -1.0)),
        ),
        right: (axis(stick(o.c_x), 1.0), turn(axis(stick(o.c_y), -1.0))),
        triggers: (trigger(lt), trigger(rt)),
        a: o.a,
        b: o.b,
        x: o.x,
        y: o.y,
        rb: o.z,
        start: o.start,
        back: false,
        guide: false,
        dpad: [o.dpad_up, o.dpad_down, o.dpad_left, o.dpad_right],
    }
}

/// The report HayBox's Melee mode sends from rest for every `step`th of the 2^22 ways to hold
/// its buttons.
fn reports(step: u32) -> impl Iterator<Item = Output> {
    (0..1u32 << 22).step_by(step as usize).map(|bits| {
        let mut i = Inputs::default();
        for (n, b) in Button::ALL.into_iter().enumerate() {
            i.set(b, bits >> n & 1 == 1);
        }
        Melee20Button::default().update(i)
    })
}

/// The hat's limit: a D-pad left and right held together arrive as neither.
fn through_hat(mut want: PadStatus) -> PadStatus {
    if want.button & LEFT != 0 && want.button & RIGHT != 0 {
        want.button &= !(LEFT | RIGHT);
    }
    if want.button & UP != 0 && want.button & DOWN != 0 {
        want.button &= !(UP | DOWN);
    }
    want
}

#[test]
fn dinput_reports_decode_to_their_bytes_on_windows() {
    let mut n = 0;
    for o in reports(7) {
        let report = dinput_report(&o);
        let want = through_hat(haybox_status(&o));
        for changed_only in [false, true] {
            let h = hid(Platform::Wgi, wgi_values(&report, changed_only));
            assert_eq!(h.haybox_status(), want, "{o:?} {changed_only}");
        }
        n += 1;
    }
    assert!(n > 500_000);
}

#[test]
fn dinput_reports_decode_to_their_bytes_on_linux() {
    for o in reports(13) {
        let report = dinput_report(&o);
        let h = hid(Platform::Evdev, evdev_values(&report));
        assert_eq!(h.haybox_status(), through_hat(haybox_status(&o)), "{o:?}");
    }
}

#[test]
fn xinput_reports_decode_to_their_bytes() {
    for o in reports(5) {
        let want = haybox_status(&o);
        for scale in [32767.0, 32768.0] {
            assert_eq!(xinput(&o, scale).haybox_status(), want, "{o:?}");
        }
    }
}

/// Every byte each of the report's values can hold reads back as itself, not only the Melee
/// mode's: the sticks, the triggers, both systems.
#[test]
fn every_byte_reads_back() {
    for v in 0..=255u8 {
        let o = Output {
            stick_x: v,
            stick_y: v,
            c_x: v,
            c_y: v,
            l_analog: v.min(254),
            r_analog: v.min(254),
            ..Output::default()
        };
        let want = haybox_status(&o);
        let report = dinput_report(&o);
        for h in [
            hid(Platform::Wgi, wgi_values(&report, true)),
            hid(Platform::Evdev, evdev_values(&report)),
        ] {
            assert_eq!(h.haybox_status(), want, "{v}");
        }
        // XInput carries the triggers' values below 255 as themselves.
        let o = Output {
            l_analog: v.min(254),
            r_analog: v.min(254),
            ..o
        };
        assert_eq!(
            xinput(&o, 32767.0).haybox_status(),
            haybox_status(&o),
            "{v}"
        );
    }
}

/// A box's report as the PAD library reads it through a GameCube cable: centred sticks read 0,
/// HayBox's 1.0 (208) reads 80, its full left (48) −80, the triggers as they are.
#[test]
fn the_cables_status() {
    let p = status(0, [128, 128, 128, 128], [0, 0]);
    assert_eq!(
        (p.stick_x, p.stick_y, p.substick_x, p.substick_y),
        (0, 0, 0, 0)
    );
    let p = status(A | L, [208, 48, 179, 77], [140, 49]);
    assert_eq!(
        (p.stick_x, p.stick_y, p.substick_x, p.substick_y),
        (80, -80, 51, -51)
    );
    assert_eq!(
        (p.trigger_left, p.trigger_right, p.button),
        (140, 49, A | L)
    );
    assert_eq!(status(0, [0, 255, 0, 0], [0, 0]).stick_x, -128);
}

#[test]
fn the_id_table() {
    // Every line names its source, and no ids repeat.
    for (i, m) in MODELS.iter().enumerate() {
        assert!(m.source.contains("HayBox 2cffcd23"), "{m:?}");
        assert!(
            !MODELS[..i]
                .iter()
                .any(|o| (o.vendor, o.product) == (m.vendor, m.product))
        );
        assert_eq!(model(m.vendor, m.product), Some(m));
    }
    assert_eq!(
        model(0x2e8a, 0x000a).map(|m| m.report),
        Some(Report::HayBoxDInput)
    );
    assert_eq!(
        model(0x0738, 0x4726).map(|m| m.report),
        Some(Report::HayBoxXInput)
    );
    // The GameCube adapter and an Xbox controller are no box.
    assert_eq!(model(0x057e, 0x0337), None);
    assert_eq!(model(0x045e, 0x028e), None);
}

#[test]
fn native_codes() {
    use HidInput::*;
    for (p, code, want) in [
        (Platform::Wgi, 0x0000_0007, Some(Button(7))),
        (Platform::Wgi, 0x0001_0005, Some(Axis(RZ_AXIS))),
        (Platform::Wgi, 0x0001_0006, None),
        (Platform::Wgi, 0x0002_0000, Some(HatX)),
        (Platform::Wgi, 0x0002_0001, Some(HatY)),
        (Platform::Evdev, 0x0003_0002, Some(Axis(Z_AXIS))),
        (Platform::Evdev, 0x0003_0011, Some(HatY)),
        (Platform::Evdev, 0x0001_0139, Some(Button(9))),
        (Platform::Evdev, 0x0001_0127, Some(Button(7))),
        (Platform::Evdev, 0x0001_02c1, Some(Button(17))),
        (Platform::Evdev, 0x0001_0110, None),
        (Platform::Other, 0x0001_0000, None),
    ] {
        assert_eq!(HidInput::of(p, code), want, "{code:#x}");
    }
}

/// A gamepad in "No deadzone, exact": each axis the nearest of 256 steps, centred.
#[test]
fn exact_sticks() {
    assert_eq!(exact_stick(0.0), 0);
    assert_eq!(exact_stick(1.0), 127);
    assert_eq!(exact_stick(-1.0), -128);
    // One step either side of the centre reads as one step, where gilrs's deadzone gives 0.
    let one = |b: i32| axis_value(0, 65535, b * 257);
    assert_eq!((exact_stick(one(129)), exact_stick(one(127))), (1, -1));
    for v in 0..=255u8 {
        let f = axis_value(0, 65535, i32::from(v) * 257);
        assert_eq!(exact_stick(f), i32::from(v) - 128);
    }
}

#[test]
fn box_layouts_round_trip_as_text() {
    let mut k = BoxKeys::default();
    k.set(Button::Mb2, Some("Digit6".into()));
    k.set(Button::Rt1, None);
    assert_eq!(BoxKeys::restore(&k.save()), Some(k.clone()));
    assert_eq!(BoxKeys::restore(""), Some(BoxKeys::default()));
    assert_eq!(BoxKeys::restore("lf9=key:KeyA"), None);
    assert_eq!(BoxKeys::restore("lf1=button:South"), None);
    assert!(k.uses_key("Digit6") && !k.uses_key("Space"));
    let i = k.inputs(|key| key == "KeyF" || key == "KeyV");
    assert!(i.get(Button::Lf1) && i.get(Button::Lt1) && !i.get(Button::Lf3));
    // Each default key once.
    let d = BoxKeys::default();
    let keys: Vec<&str> = Button::ALL.into_iter().filter_map(|b| d.get(b)).collect();
    for (n, key) in keys.iter().enumerate() {
        assert!(!keys[..n].contains(key), "{key}");
    }
    assert_eq!(keys.len(), 20);
}
