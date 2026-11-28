// SPDX-License-Identifier: GPL-3.0-only
//
// phob-check's test oracle: PhobGCC-SW's own source (https://github.com/PhobGCC/PhobGCC-SW, GPL
// version 3), built for the tests only. build.rs copies its headers and the functions it needs
// from `phobGCC.h` and `readHardware.h` verbatim between this file and api.cpp; what's here
// stands in for the board and for the RP2040 build's name lookup.

#define _USE_MATH_DEFINES
#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cstdlib>
#include <cstring>
using std::max;
using std::min;

template <class... T> inline void debug_print(T...) {}
template <class... T> inline void debug_println(T...) {}

// The RP2040 build includes <cmath> and <algorithm> (libstdc++) and no <stdlib.h> or <math.h>
// wrapper before PhobGCC's headers, so its unqualified `abs`, `pow`, `sqrt`, `fmin`, `fmax`,
// `round` and `fabs` are C's: `int abs(int)` and the double functions. The host's headers add
// float overloads to the global namespace; these names take them away again.
static inline int phob_abs(int v) { return std::abs(v); }
static inline double phob_pow(double a, double b) { return std::pow(a, b); }
static inline double phob_sqrt(double a) { return std::sqrt(a); }
static inline double phob_fmin(double a, double b) { return std::fmin(a, b); }
static inline double phob_fmax(double a, double b) { return std::fmax(a, b); }
static inline double phob_round(double a) { return std::round(a); }
static inline double phob_fabs(double a) { return std::fabs(a); }
#define abs phob_abs
#define pow phob_pow
#define sqrt phob_sqrt
#define fmin phob_fmin
#define fmax phob_fmax
#define round phob_round
#define fabs phob_fabs

#include "structsAndEnums.h"
#include "variables.h"

// The board. Each axis reads from a queue the API fills; `micros` steps a millisecond a call,
// so `readSticks` reads each axis once.
static int g_vals[4][128];
static int g_idx[4];
static int g_trig[2];
static int g_adc;
static uint32_t g_micros = 0;
uint32_t micros() { g_micros += 1000; return g_micros; }
void readADCScale(float &, float &) {}
int readAx(const Pins &) { return g_vals[0][g_idx[0]++ & 127]; }
int readAy(const Pins &) { return g_vals[1][g_idx[1]++ & 127]; }
int readCx(const Pins &) { return g_vals[2][g_idx[2]++ & 127]; }
int readCy(const Pins &) { return g_vals[3][g_idx[3]++ & 127]; }
#define _pinLadc 0
#define _pinRadc 1
static void adc_select_input(int i) { g_adc = i; }
static uint16_t adc_read() { return (uint16_t)(g_trig[g_adc] * 16); }
Pins _pinList{};

// Storage: each stick's calibration points and notch angles.
static float g_points[2][2][32];
static float g_angles[2][16];
void getPointsSetting(float points[], const WhichStick s, const WhichAxis a) { memcpy(points, g_points[s][a], sizeof(float) * 32); }
void setPointsSetting(const float points[], const WhichStick s, const WhichAxis a) { memcpy(g_points[s][a], points, sizeof(float) * 32); }
void getNotchAnglesSetting(float angles[], const WhichStick s) { memcpy(angles, g_angles[s], sizeof(float) * 16); }
void setNotchAnglesSetting(const float angles[], const WhichStick s) { memcpy(g_angles[s], angles, sizeof(float) * 16); }
void setAutoInitSetting(const int) {}

#include "filter.h"
#include "stick.h"
