// SPDX-License-Identifier: GPL-3.0-only
//
// phob-check's test oracle, the calls the tests make (see head.cpp). Everything here only sets
// up upstream's inputs and calls upstream's functions, in the order its RP2040 main loop does.

FilterGains _normGains;

static float g_tempX[32];
static float g_tempY[32];
static NotchStatus g_status[16];
static float g_notchAngles[16];
static float g_measured[16];
static int g_step = -1;
static WhichStick g_which = ASTICK;
static bool g_undoCal = false;

static StickParams &params(int stick) { return stick == 0 ? _aStickParams : _cStickParams; }

static void putParams(const StickParams &p, float *out) {
	memcpy(out, p.fitCoeffsX, 16);
	memcpy(out + 4, p.fitCoeffsY, 16);
	memcpy(out + 8, p.affineCoeffs, 256);
	memcpy(out + 72, p.boundaryAngles, 64);
}

extern "C" {

// `initializeButtons`' sticks.
void phob_reset() {
	_btn.Ax = _intOrigin;
	_btn.Ay = _intOrigin;
	_btn.Cx = _intOrigin;
	_btn.Cy = _intOrigin;
}

// The 18 settings in melee-phob's `Settings::fields` order, then `recomputeGains`.
void phob_set_controls(const int *v) {
	_controls.xSnapback = v[0];
	_controls.ySnapback = v[1];
	_controls.axSmoothing = v[2];
	_controls.aySmoothing = v[3];
	_controls.cxSmoothing = v[4];
	_controls.cySmoothing = v[5];
	_controls.axWaveshaping = v[6];
	_controls.ayWaveshaping = v[7];
	_controls.cxWaveshaping = v[8];
	_controls.cyWaveshaping = v[9];
	_controls.astickCardinalSnapping = v[10];
	_controls.cstickCardinalSnapping = v[11];
	_controls.astickAnalogScaler = v[12];
	_controls.cstickAnalogScaler = v[13];
	_controls.lConfig = v[14];
	_controls.rConfig = v[15];
	_controls.lTriggerOffset = v[16];
	_controls.rTriggerOffset = v[17];
	recomputeGains(_controls, _gains, _normGains);
}

void phob_norm_gains(float *out) { memcpy(out, &_normGains, sizeof(FilterGains)); }

void phob_get_params(int stick, float *out) { putParams(params(stick), out); }

// `readEEPROM`'s derivation of a stick's parameters from stored points and angles.
void phob_params_from(int stick, const float *px, const float *py, const float *angles, int *status) {
	float x[32], y[32], a[16];
	memcpy(x, px, sizeof x);
	memcpy(y, py, sizeof y);
	memcpy(a, angles, sizeof a);
	float cleanedX[17], cleanedY[17], notchX[17], notchY[17];
	NotchStatus st[16];
	cleanCalPoints(x, y, a, cleanedX, cleanedY, notchX, notchY, st);
	linearizeCal(cleanedX, cleanedY, cleanedX, cleanedY, params(stick));
	notchCalibrate(cleanedX, cleanedY, notchX, notchY, _noOfNotches, params(stick));
	for (int i = 0; i < 16; i++) status[i] = st[i];
}

void phob_calc_stick_values(float angle, float *out) { calcStickValues(angle, out, out + 1); }

float phob_angle_on_sphere(float x, float y) {
	float a;
	angleOnSphere(x, y, a);
	return a;
}

void phob_notch_remap(float x, float y, int stick, int calStep, float *out) {
	notchRemap(x, y, out, out + 1, _noOfNotches, params(stick), calStep, _controls, stick == 0 ? ASTICK : CSTICK);
}

int phob_fit_curve(const double *px, const double *py, double *coeffs) {
	double x[5], y[5];
	memcpy(x, px, sizeof x);
	memcpy(y, py, sizeof y);
	return fitCurve(_fitOrder, 5, x, y, _fitOrder + 1, coeffs);
}

void phob_legalize_notches(int step, const float *measured, float *angles) {
	float m[16];
	NotchStatus st[16];
	memcpy(m, measured, sizeof m);
	legalizeNotches(step, m, angles, st);
}

// One sample of `readSticks`, the sticks' bytes as the controller sent them.
void phob_read_sticks(const uint8_t *raw, int readA, int readC, int calStep, int currentlyRaw, uint8_t *out, float *rawOut) {
	for (int k = 0; k < 4; k++) {
		g_vals[k][0] = raw[k] * 16;
		g_idx[k] = 0;
	}
	readSticks(readA, readC, _btn, _pinList, _raw, _hardware, _controls, _normGains, _aStickParams, _cStickParams, _dT, calStep, currentlyRaw);
	out[0] = _btn.Ax;
	out[1] = _btn.Ay;
	out[2] = _btn.Cx;
	out[3] = _btn.Cy;
	memcpy(rawOut, &_raw, sizeof(RawStick));
}

static void toHardware(int b, Buttons &h) {
	h.Dl = (b & 0x0001) != 0;
	h.Dr = (b & 0x0002) != 0;
	h.Dd = (b & 0x0004) != 0;
	h.Du = (b & 0x0008) != 0;
	h.Z = (b & 0x0010) != 0;
	h.R = (b & 0x0020) != 0;
	h.L = (b & 0x0040) != 0;
	h.A = (b & 0x0100) != 0;
	h.B = (b & 0x0200) != 0;
	h.X = (b & 0x0400) != 0;
	h.Y = (b & 0x0800) != 0;
	h.S = (b & 0x1000) != 0;
}

static int fromButtons(const Buttons &h) {
	return h.Dl * 0x0001 | h.Dr * 0x0002 | h.Dd * 0x0004 | h.Du * 0x0008 | h.Z * 0x0010 | h.R * 0x0020 | h.L * 0x0040 |
		   h.A * 0x0100 | h.B * 0x0200 | h.X * 0x0400 | h.Y * 0x0800 | h.S * 0x1000;
}

// `processButtons`' button and trigger output for PADStatus buttons and the sliders' bytes.
int phob_buttons(int buttons, int la, int ra, uint8_t *triggers) {
	toHardware(buttons, _hardware);
	g_trig[0] = la;
	g_trig[1] = ra;
	phob_trigger_block(_btn, _hardware, _controls, _pinList);
	triggers[0] = _btn.La;
	triggers[1] = _btn.Ra;
	return fromButtons(_btn);
}

// Calibration, as the RP2040 main loop and `processButtons` drive it.
void phob_cal_store(int stick, const float *px, const float *py, const float *angles) {
	setPointsSetting(px, stick == 0 ? ASTICK : CSTICK, XAXIS);
	setPointsSetting(py, stick == 0 ? ASTICK : CSTICK, YAXIS);
	setNotchAnglesSetting(angles, stick == 0 ? ASTICK : CSTICK);
}

// AXY+L or AXY+R, with the statics as melee-phob's `Calibration::start` makes them (none of them
// reaches a result before `applyCalFromPoints` sets it).
void phob_cal_start(int stick) {
	g_which = stick == 0 ? ASTICK : CSTICK;
	g_step = 0;
	g_undoCal = false;
	getNotchAnglesSetting(g_notchAngles, g_which);
	for (int i = 0; i < 16; i++) {
		g_measured[i] = 0;
		g_status[i] = _notchStatusDefaults[i];
	}
	memset(g_tempX, 0, sizeof g_tempX);
	memset(g_tempY, 0, sizeof g_tempY);
}

// An advance, the stick's 128 readings of each axis given as bytes.
void phob_cal_advance(const uint8_t *xs, const uint8_t *ys) {
	int kx = g_which == ASTICK ? 0 : 2;
	for (int i = 0; i < 128; i++) {
		g_vals[kx][i] = xs[i] * 16;
		g_vals[kx + 1][i] = ys[i] * 16;
	}
	g_idx[kx] = 0;
	g_idx[kx + 1] = 0;
	calibrationAdvance(_controls, g_step, g_which, g_tempX, g_tempY, g_undoCal, g_notchAngles, g_status, g_measured, _aStickParams, _cStickParams);
}

void phob_cal_undo() {
	if (g_undoCal) calibrationUndo(g_step, g_which, g_status);
}

void phob_cal_skip() {
	if (g_step >= 0 && g_step < 32)
		calibrationSkipMeasurement(g_step, g_which, g_tempX, g_tempY, g_status, g_notchAngles, g_measured, _aStickParams, _cStickParams, _controls);
}

// One loop's notch adjustment (main.cpp), X, Y and B as given. `adjustNotch` also shows the target
// on the other stick's output; that display isn't ported, so the output is put back.
void phob_cal_adjust(float dT, int cw, int ccw, int reset) {
	if (g_step < _noOfCalibrationPoints) return;
	Buttons saved = _btn;
	_hardware.X = cw;
	_hardware.Y = ccw;
	_btn.B = reset;
	adjustNotch(g_step, dT, g_which, g_measured, g_notchAngles, g_status, _btn, _hardware);
	if (_hardware.Y || _hardware.X || (_btn.B)) {
		float cleanedPointsX[_noOfNotches + 1];
		float cleanedPointsY[_noOfNotches + 1];
		float notchPointsX[_noOfNotches + 1];
		float notchPointsY[_noOfNotches + 1];
		cleanCalPoints(g_tempX, g_tempY, g_notchAngles, cleanedPointsX, cleanedPointsY, notchPointsX, notchPointsY, g_status);
		linearizeCal(cleanedPointsX, cleanedPointsY, cleanedPointsX, cleanedPointsY, params(g_which == ASTICK ? 0 : 1));
		notchCalibrate(cleanedPointsX, cleanedPointsY, notchPointsX, notchPointsY, _noOfNotches, params(g_which == ASTICK ? 0 : 1));
	}
	_btn = saved;
}

// The calibration's state: step, undo, then the points (64), notch angles (16), measured angles
// (16) and statuses (16).
void phob_cal_get(int *stepUndo, float *floats, int *status) {
	stepUndo[0] = g_step;
	stepUndo[1] = g_undoCal;
	memcpy(floats, g_tempX, 128);
	memcpy(floats + 32, g_tempY, 128);
	memcpy(floats + 64, g_notchAngles, 64);
	memcpy(floats + 80, g_measured, 64);
	for (int i = 0; i < 16; i++) status[i] = g_status[i];
}

}
