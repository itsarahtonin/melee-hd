// SPDX-License-Identifier: GPL-3.0-only

//! melee-phob against PhobGCC-SW's own source (see the crate's docs): every float bit for bit,
//! every byte equal, on random and edge inputs and over long sequences through the filters.

#[cfg(not(phob_oracle))]
#[test]
fn matches_upstream() {
    melee_local_data::skip!(
        "phob-check: PHOBGCC_SW names no checkout of PhobGCC-SW at b4f175ea; skipped"
    );
}

#[cfg(phob_oracle)]
mod oracle {
    use std::f32::consts::PI;

    use melee_phob::calibrate::{Calibration, measure};
    use melee_phob::filter::recompute_gains;
    use melee_phob::profile::StickCal;
    use melee_phob::settings::{FilterGains, GAINS, Settings, WhichStick};
    use melee_phob::stick::{
        CAL_ORDER, CAL_POINTS, NOTCH_ANGLE_DEFAULTS, NOTCHES, StickParams, angle_on_sphere,
        calc_stick_values, legalize_notches, notch_remap, params_from,
    };
    use melee_phob::sticks::Sticks;
    use melee_phob::triggers;
    use phob_check::ffi;

    /// splitmix64.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^ (z >> 31)
        }
        fn f(&mut self) -> f32 {
            (self.next() >> 40) as f32 / (1u64 << 24) as f32
        }
        fn range(&mut self, lo: f32, hi: f32) -> f32 {
            lo + (hi - lo) * self.f()
        }
        fn int(&mut self, lo: i32, hi: i32) -> i32 {
            lo + (self.next() % (hi - lo + 1) as u64) as i32
        }
        fn chance(&mut self, p: f32) -> bool {
            self.f() < p
        }
    }

    fn same(a: &[f32], b: &[f32], what: &str) {
        for (i, (x, y)) in a.iter().zip(b).enumerate() {
            assert_eq!(
                x.to_bits(),
                y.to_bits(),
                "{what}[{i}]: ours {x}, upstream's {y}"
            );
        }
    }

    fn flat(p: &StickParams) -> Vec<f32> {
        let mut v = Vec::with_capacity(88);
        v.extend(p.fit_coeffs_x);
        v.extend(p.fit_coeffs_y);
        for a in &p.affine_coeffs {
            v.extend(a);
        }
        v.extend(p.boundary_angles);
        v
    }

    fn upstream_params(stick: i32) -> Vec<f32> {
        let mut v = vec![0f32; 88];
        unsafe { ffi::phob_get_params(stick, v.as_mut_ptr()) };
        v
    }

    fn random_settings(r: &mut Rng) -> Settings {
        let mut s = Settings::default();
        for (_, v, range) in s.fields() {
            *v = r.int(*range.start(), *range.end());
        }
        // Often the defaults, where players are.
        if r.chance(0.3) {
            s = Settings {
                l_mode: s.l_mode,
                r_mode: s.r_mode,
                ..Settings::default()
            };
        }
        s
    }

    fn set_settings(s: &Settings) {
        let mut s = *s;
        let v: Vec<i32> = s.fields().into_iter().map(|(_, v, _)| *v).collect();
        unsafe { ffi::phob_set_controls(v.as_ptr()) };
    }

    /// A stock stick's calibration points as the adapter would give them (bytes over 256):
    /// centres jittering about the middle, the notches at an uneven reach and angle, and each
    /// modder notch measured or left at the centre.
    fn random_points(r: &mut Rng) -> ([f32; CAL_POINTS], [f32; CAL_POINTS]) {
        let (cx, cy) = (r.range(0.45, 0.55), r.range(0.45, 0.55));
        let byte = |v: f32| (v * 256.0).round().clamp(0.0, 255.0) / 256.0;
        let mut x = [0f32; CAL_POINTS];
        let mut y = [0f32; CAL_POINTS];
        let skew = r.range(-0.1, 0.1);
        for n in 0..NOTCHES {
            x[n * 2] = byte(cx + r.range(-0.01, 0.01));
            y[n * 2] = byte(cy + r.range(-0.01, 0.01));
            let measured = n % 2 == 0 || r.chance(0.5);
            if measured {
                let a = NOTCH_ANGLE_DEFAULTS[n] + skew + r.range(-0.08, 0.08);
                let reach = r.range(0.24, 0.40);
                x[n * 2 + 1] = byte(cx + reach * a.cos());
                y[n * 2 + 1] = byte(cy + reach * a.sin());
            } else {
                x[n * 2 + 1] = x[n * 2];
                y[n * 2 + 1] = y[n * 2];
            }
        }
        (x, y)
    }

    fn random_angles(r: &mut Rng) -> [f32; NOTCHES] {
        let mut a = NOTCH_ANGLE_DEFAULTS;
        for (i, v) in a.iter_mut().enumerate() {
            if i % 2 == 1 || r.chance(0.3) {
                *v += r.range(-0.1, 0.1);
            }
        }
        a
    }

    /// Both sides' parameters for a stick from the same stored calibration; they must agree.
    fn calibrate_both(stick: i32, cal: &StickCal) -> StickParams {
        let mut p = StickParams::default();
        let status = params_from(&cal.points_x, &cal.points_y, &cal.notch_angles, &mut p);
        let mut st = [0i32; NOTCHES];
        unsafe {
            ffi::phob_params_from(
                stick,
                cal.points_x.as_ptr(),
                cal.points_y.as_ptr(),
                cal.notch_angles.as_ptr(),
                st.as_mut_ptr(),
            )
        };
        same(&flat(&p), &upstream_params(stick), "params");
        assert_eq!(status.map(|s| s as i32), st, "notch statuses");
        p
    }

    fn random_cal(r: &mut Rng) -> StickCal {
        let (points_x, points_y) = random_points(r);
        StickCal {
            points_x,
            points_y,
            notch_angles: random_angles(r),
        }
    }

    fn pure_functions(r: &mut Rng) {
        for _ in 0..20_000 {
            let a = r.range(-0.5, 7.0);
            let mut out = [0f32; 2];
            unsafe { ffi::phob_calc_stick_values(a, out.as_mut_ptr()) };
            let (x, y) = calc_stick_values(a);
            same(&[x, y], &out, "calcStickValues");
            let (x, y) = (r.range(-130.0, 130.0), r.range(-130.0, 130.0));
            let up = unsafe { ffi::phob_angle_on_sphere(x, y) };
            same(&[angle_on_sphere(x, y)], &[up], "angleOnSphere");
        }
        for _ in 0..5_000 {
            let mut px = [0f64; 5];
            let mut py = [0f64; 5];
            for i in 0..5 {
                px[i] = r.range(0.0, 1.0) as f64;
                py[i] = r.range(0.0, 255.0) as f64;
            }
            let mut ours = [0f64; 4];
            let mut up = [0f64; 4];
            melee_phob::curve_fit::fit_curve(3, &px, &py, 4, &mut ours).unwrap();
            assert_eq!(
                unsafe { ffi::phob_fit_curve(px.as_ptr(), py.as_ptr(), up.as_mut_ptr()) },
                0
            );
            assert_eq!(
                ours.map(f64::to_bits),
                up.map(f64::to_bits),
                "fitCurve {px:?}"
            );
        }
        for _ in 0..3_000 {
            let mut measured = NOTCH_ANGLE_DEFAULTS;
            for m in &mut measured {
                *m += r.range(-0.15, 0.15);
            }
            let mut angles = measured;
            for a in &mut angles {
                *a += r.range(-0.4, 0.4);
            }
            let step = r.int(32, 43);
            let mut up = angles;
            legalize_notches(step, &measured, &mut angles);
            unsafe { ffi::phob_legalize_notches(step, measured.as_ptr(), up.as_mut_ptr()) };
            same(&angles, &up, "legalizeNotches");
        }
        for _ in 0..300 {
            let s = random_settings(r);
            set_settings(&s);
            let cal = random_cal(r);
            let stick = r.int(0, 1);
            let p = calibrate_both(stick, &cal);
            let which = if stick == 0 {
                WhichStick::A
            } else {
                WhichStick::C
            };
            for _ in 0..200 {
                let (x, y) = if r.chance(0.2) {
                    // Near a cardinal at the rim, where snapping decides.
                    let a = r.int(0, 3) as f32 * PI / 2.0;
                    let rad = r.range(70.0, 110.0);
                    (
                        rad * a.cos() + r.range(-9.0, 9.0),
                        rad * a.sin() + r.range(-9.0, 9.0),
                    )
                } else {
                    (r.range(-140.0, 140.0), r.range(-140.0, 140.0))
                };
                let step = if r.chance(0.8) { -1 } else { r.int(0, 43) };
                let mut out = [0f32; 2];
                unsafe { ffi::phob_notch_remap(x, y, stick, step, out.as_mut_ptr()) };
                let (ox, oy) = notch_remap(x, y, &p, step, &s, which);
                same(&[ox, oy], &out, "notchRemap");
            }
        }
    }

    fn gains(r: &mut Rng) {
        for _ in 0..2_000 {
            let s = random_settings(r);
            set_settings(&s);
            let mut g = GAINS;
            let n = recompute_gains(&s, &mut g, 1.0);
            let mut up = [0f32; 13];
            unsafe { ffi::phob_norm_gains(up.as_mut_ptr()) };
            let FilterGains {
                max_stick,
                x_vel_decay,
                y_vel_decay,
                x_vel_pos_factor,
                y_vel_pos_factor,
                x_vel_damp,
                y_vel_damp,
                vel_thresh,
                accel_thresh,
                x_smoothing,
                y_smoothing,
                cx_smoothing,
                cy_smoothing,
            } = n;
            let ours = [
                max_stick,
                x_vel_decay,
                y_vel_decay,
                x_vel_pos_factor,
                y_vel_pos_factor,
                x_vel_damp,
                y_vel_damp,
                vel_thresh,
                accel_thresh,
                x_smoothing,
                y_smoothing,
                cx_smoothing,
                cy_smoothing,
            ];
            same(&ours, &up, &format!("normGains {s:?}"));
        }
    }

    /// A physical stick: where it is (from the centre, in full-scale units) and how it moves.
    struct Hand {
        pos: [f32; 2],
        vel: [f32; 2],
        target: [f32; 2],
        spring: bool,
        hold: u32,
    }

    impl Hand {
        /// The next millisecond: a random walk, a slow move, a hold at a notch, or a flick
        /// released to spring back through the centre (the snapback the filter is for).
        fn step(&mut self, r: &mut Rng) {
            if self.hold == 0 {
                self.hold = r.int(5, 200) as u32;
                let kind = r.int(0, 3);
                let a = r.range(0.0, 2.0 * PI);
                let reach = r.range(0.0, 0.42);
                self.target = [reach * a.cos(), reach * a.sin()];
                self.spring = kind == 3;
                if self.spring {
                    // Flick to the rim, then let go.
                    self.pos = [0.38 * a.cos(), 0.38 * a.sin()];
                    self.target = [0.0, 0.0];
                    self.vel = [0.0, 0.0];
                } else if kind == 0 {
                    self.pos = self.target;
                }
            }
            self.hold -= 1;
            for k in 0..2 {
                if self.spring {
                    // A lightly damped spring: overshoots the centre a few times.
                    self.vel[k] += -0.08 * self.pos[k] - 0.05 * self.vel[k];
                    self.pos[k] += self.vel[k];
                } else {
                    self.pos[k] += 0.15 * (self.target[k] - self.pos[k]) + r.range(-0.002, 0.002);
                }
            }
        }

        fn bytes(&self, centre: [f32; 2]) -> [u8; 2] {
            [0, 1].map(|k| {
                ((centre[k] + self.pos[k]) * 256.0)
                    .round()
                    .clamp(0.0, 255.0) as u8
            })
        }
    }

    fn pipeline(r: &mut Rng) {
        unsafe { ffi::phob_reset() };
        let mut ours = Sticks::default();
        let mut a = StickParams::default();
        let mut c = StickParams::default();
        let mut hands = [0, 1].map(|_| Hand {
            pos: [0.0; 2],
            vel: [0.0; 2],
            target: [0.0; 2],
            spring: false,
            hold: 0,
        });
        let mut s = Settings::default();
        let mut samples = 0u64;
        for segment in 0..60 {
            if segment > 0 {
                s = random_settings(r);
            }
            set_settings(&s);
            // The first segment runs uncalibrated (the raw output), as a fresh controller.
            if segment > 0 {
                if r.chance(0.9) {
                    a = calibrate_both(0, &random_cal(r));
                }
                if r.chance(0.9) {
                    c = calibrate_both(1, &random_cal(r));
                }
            }
            let mut g = GAINS;
            let n = recompute_gains(&s, &mut g, 1.0);
            let centre = [0.5, 0.5];
            for _ in 0..3_000 {
                let mut raw = [0u8; 4];
                for (k, h) in hands.iter_mut().enumerate() {
                    h.step(r);
                    let b = h.bytes(centre);
                    raw[k * 2] = b[0];
                    raw[k * 2 + 1] = b[1];
                }
                if r.chance(0.002) {
                    // Edges: the bytes' extremes.
                    raw = [0, 255, 255, 0].map(|v| if r.chance(0.5) { v } else { 128 });
                }
                let read_a = !r.chance(0.02);
                let read_c = !r.chance(0.02);
                let step = if r.chance(0.01) { r.int(0, 43) } else { -1 };
                let raw_mode = r.chance(0.005);
                let mut up = [0u8; 4];
                let mut up_raw = [0f32; 12];
                unsafe {
                    ffi::phob_read_sticks(
                        raw.as_ptr(),
                        read_a as i32,
                        read_c as i32,
                        step,
                        raw_mode as i32,
                        up.as_mut_ptr(),
                        up_raw.as_mut_ptr(),
                    )
                };
                let got = ours.read(
                    raw.map(|v| v as f32 / 256.0),
                    read_a,
                    read_c,
                    &s,
                    &n,
                    &a,
                    &c,
                    step,
                    raw_mode,
                );
                let mut flat = Vec::with_capacity(12);
                flat.extend(got.raw);
                flat.extend(got.linearized);
                flat.extend(got.unfiltered);
                same(&flat, &up_raw, &format!("RawStick, sample {samples}"));
                assert_eq!(ours.out, up, "sticks sent, sample {samples}, {s:?}");
                samples += 1;
            }
        }
        eprintln!("phob-check: {samples} samples through readSticks agree");
    }

    fn buttons(r: &mut Rng) {
        for _ in 0..50_000 {
            let s = random_settings(r);
            set_settings(&s);
            let mut b = (r.next() & 0x1f7f) as u16;
            if r.chance(0.05) {
                b |= triggers::L | triggers::R | triggers::A | triggers::START;
            }
            let sliders = [r.int(0, 255), r.int(0, 255)]
                .map(|v| if v < 20 && v % 2 == 0 { v / 4 } else { v });
            let mut up_t = [0u8; 2];
            let up =
                unsafe { ffi::phob_buttons(b as i32, sliders[0], sliders[1], up_t.as_mut_ptr()) };
            let (ob, ot) = triggers::buttons(&s, b, sliders.map(|v| v as f32));
            assert_eq!(
                (ob as i32, ot),
                (up, up_t),
                "buttons {b:#x} sliders {sliders:?} {s:?}"
            );
        }
    }

    fn compare_cal(cal: &Calibration, p: &StickParams, stick: i32, what: &str) {
        let mut su = [0i32; 2];
        let mut fl = [0f32; 96];
        let mut st = [0i32; 16];
        unsafe { ffi::phob_cal_get(su.as_mut_ptr(), fl.as_mut_ptr(), st.as_mut_ptr()) };
        assert_eq!([cal.step, cal.undo_ok as i32], su, "{what}: step and undo");
        same(&cal.points_x, &fl[..32], what);
        same(&cal.points_y, &fl[32..64], what);
        same(&cal.notch_angles, &fl[64..80], what);
        same(&cal.measured, &fl[80..96], what);
        assert_eq!(cal.status.map(|s| s as i32), st, "{what}: statuses");
        same(&flat(p), &upstream_params(stick), what);
    }

    fn calibrations(r: &mut Rng) {
        let mut stored = [StickCal::default(), StickCal::default()];
        let mut ever = [false; 2];
        let mut flows = 0;
        while flows < 60 {
            let s = random_settings(r);
            set_settings(&s);
            let stick = r.int(0, 1);
            let which = if stick == 0 {
                WhichStick::A
            } else {
                WhichStick::C
            };
            let k = stick as usize;
            if !ever[k] || r.chance(0.3) {
                stored[k] = random_cal(r);
                ever[k] = true;
                calibrate_both(stick, &stored[k]);
            }
            let st = stored[k];
            unsafe {
                ffi::phob_cal_store(
                    stick,
                    st.points_x.as_ptr(),
                    st.points_y.as_ptr(),
                    st.notch_angles.as_ptr(),
                );
                ffi::phob_cal_start(stick)
            };
            let mut p = StickParams::default();
            let mut q = vec![0f32; 88];
            unsafe { ffi::phob_get_params(stick, q.as_mut_ptr()) };
            // Start from the parameters both sides have now.
            p.fit_coeffs_x.copy_from_slice(&q[..4]);
            p.fit_coeffs_y.copy_from_slice(&q[4..8]);
            for (i, a) in p.affine_coeffs.iter_mut().enumerate() {
                a.copy_from_slice(&q[8 + i * 4..12 + i * 4]);
            }
            p.boundary_angles.copy_from_slice(&q[72..88]);
            let mut cal = Calibration::start(which, st);
            let (px, py) = random_points(r);
            let mut events = 0;
            while !cal.finished() {
                events += 1;
                assert!(events < 2_000, "a calibration that never ends");
                let what = format!("flow {flows}, event {events}, step {}", cal.step);
                let roll = r.f();
                if roll < 0.08 {
                    if cal.undo_ok {
                        cal.undo();
                    }
                    unsafe { ffi::phob_cal_undo() };
                } else if roll < 0.10 && cal.measuring() {
                    cal.skip(&s, &mut p);
                    unsafe { ffi::phob_cal_skip() };
                } else if roll < 0.45 && !cal.measuring() {
                    let dt = if r.chance(0.5) { 1.0 } else { 8.0 };
                    let (cw, ccw, reset) = (r.chance(0.5), r.chance(0.5), r.chance(0.1));
                    for _ in 0..r.int(1, 40) {
                        cal.adjust(dt, cw, ccw, reset, &mut p);
                        unsafe { ffi::phob_cal_adjust(dt, cw as i32, ccw as i32, reset as i32) };
                    }
                } else {
                    let (x, y) = if cal.measuring() {
                        let i = CAL_ORDER[cal.step as usize];
                        (px[i], py[i])
                    } else {
                        (0.5, 0.5)
                    };
                    let noise = |r: &mut Rng, v: f32| {
                        [0u8; 128].map(|_| ((v * 256.0) as i32 + r.int(-1, 1)).clamp(0, 255) as u8)
                    };
                    let xs = noise(r, x);
                    let ys = noise(r, y);
                    let done = cal.advance((measure(&xs), measure(&ys)), &s, &mut p);
                    unsafe { ffi::phob_cal_advance(xs.as_ptr(), ys.as_ptr()) };
                    if let Some(done) = done {
                        stored[k] = done;
                    }
                }
                compare_cal(&cal, &p, stick, &what);
            }
            flows += 1;
        }
    }

    #[test]
    fn matches_upstream() {
        // Upstream keeps its state in globals and statics, so one test runs every part, the
        // pipeline first while the sticks are still uncalibrated as at power-on.
        let mut r = Rng(0x5eed_0f_9c0b);
        pipeline(&mut r);
        pure_functions(&mut r);
        gains(&mut r);
        buttons(&mut r);
        calibrations(&mut r);
    }
}
