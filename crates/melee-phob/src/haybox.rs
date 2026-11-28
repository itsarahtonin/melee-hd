// SPDX-License-Identifier: GPL-3.0-only
//
// Ported from HayBox (https://github.com/JonnyHaystack/HayBox) at 2cffcd23 (firmware 3.1.0):
// `src/modes/Melee20Button.cpp` and `include/modes/Melee20Button.hpp`, the SOCD resolution of
// `src/core/socd.cpp` and `src/core/InputMode.cpp`, `ControllerMode::UpdateDirections` of
// `src/core/ControllerMode.cpp`, the input and output state of `include/core/state.hpp`, and the
// Melee mode's defaults in `HAL/pico/include/config_defaults.hpp`. HayBox is by Jonathan Haylett
// (JonnyHaystack); these files' history also credits Sean Thomas, Mannie Festo and Grammy da goat.
// HayBox is licensed under the GNU General Public License version 3. This port is part of
// melee-hd and is distributed under the same license, GPL-3.0-only.

//! HayBox's Melee mode (`Melee20Button`), the B0XX's 20-button layout: which of the box's
//! buttons are held in, the GameCube report out, with its SOCD resolution (second input
//! priority without reactivation on each pair of opposite directions, the Melee mode's
//! default), its two modifiers' coordinates, the light and mid shield buttons and the D-pad
//! layer. melee-hd plays a keyboard through it (the "Box layout (B0XX)").
//!
//! The coordinates are the GameCube report's bytes (128 the centre, one unit 0.0125 of Melee's
//! stick range, so 80 above the centre is 1.0): [`Melee20Button::update`] returns them as the
//! box sends them through a GameCube cable, which the caller turns into a `PADStatus` as the
//! PAD library would from a controller whose origin is the centre.
//!
//! Not ported: HayBox's other modes, its button remapping (melee-hd's bindings choose the keys),
//! the Nunchuk, and the communication backends.

/// One of the box's buttons, by HayBox's name for its place: the left hand's fingers (LF) and
/// thumbs (LT), the middle buttons (MB), the right hand's thumbs (RT) and fingers (RF). The
/// Melee mode reads these 20.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Button {
    Lf1,
    Lf2,
    Lf3,
    Lf4,
    Lt1,
    Lt2,
    Mb1,
    Mb2,
    Mb3,
    Rt1,
    Rt2,
    Rt3,
    Rt4,
    Rt5,
    Rf1,
    Rf2,
    Rf3,
    Rf4,
    Rf5,
    Rf6,
    Rf7,
    Rf8,
}

impl Button {
    /// Every button the Melee mode reads, in the order a box's layout lists them.
    pub const ALL: [Button; 22] = [
        Button::Lf4,
        Button::Lf3,
        Button::Lf2,
        Button::Lf1,
        Button::Lt1,
        Button::Lt2,
        Button::Mb1,
        Button::Mb3,
        Button::Mb2,
        Button::Rt1,
        Button::Rt2,
        Button::Rt3,
        Button::Rt4,
        Button::Rt5,
        Button::Rf1,
        Button::Rf2,
        Button::Rf3,
        Button::Rf4,
        Button::Rf5,
        Button::Rf6,
        Button::Rf7,
        Button::Rf8,
    ];

    /// What the Melee mode does with it.
    pub fn function(self) -> &'static str {
        match self {
            Button::Lf1 => "Right",
            Button::Lf2 => "Down",
            Button::Lf3 => "Left",
            Button::Lf4 => "L",
            Button::Lt1 => "Mod X",
            Button::Lt2 => "Mod Y",
            Button::Mb1 => "Start",
            Button::Mb2 => "D-pad right",
            Button::Mb3 => "D-pad left",
            Button::Rt1 => "A",
            Button::Rt2 => "C-down",
            Button::Rt3 => "C-left",
            Button::Rt4 => "C-up",
            Button::Rt5 => "C-right",
            Button::Rf1 => "B",
            Button::Rf2 => "X",
            Button::Rf3 => "Z",
            Button::Rf4 => "Up",
            Button::Rf5 => "R",
            Button::Rf6 => "Y",
            Button::Rf7 => "Light shield",
            Button::Rf8 => "Mid shield",
        }
    }

    fn bit(self) -> u32 {
        1 << self as u32
    }
}

/// The buttons held in (HayBox's `InputState`, the 20 the Melee mode reads).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Inputs(u32);

impl Inputs {
    pub fn with(mut self, b: Button, held: bool) -> Inputs {
        self.set(b, held);
        self
    }

    pub fn set(&mut self, b: Button, held: bool) {
        if held {
            self.0 |= b.bit();
        } else {
            self.0 &= !b.bit();
        }
    }

    pub fn get(self, b: Button) -> bool {
        self.0 & b.bit() != 0
    }
}

/// How a pair of opposite directions held together resolves (`SocdType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SocdType {
    /// Both give neither.
    Neutral,
    /// The one pressed second wins; releasing it gives back the first.
    SecondInputPriority,
    /// The one pressed second wins; releasing it gives neither, until the first is pressed
    /// again (the Melee mode's default).
    SecondInputPriorityNoReactivation,
    /// The first of the pair always wins.
    Dir1Priority,
    /// The second of the pair always wins.
    Dir2Priority,
}

/// One pair's memory (`socd::SocdState`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct SocdState {
    was_dir1: bool,
    was_dir2: bool,
    lock_dir1: bool,
    lock_dir2: bool,
}

/// `socd::second_input_priority_no_reactivation`.
fn second_input_priority_no_reactivation(
    inputs: &mut Inputs,
    dir1: Button,
    dir2: Button,
    s: &mut SocdState,
) {
    let mut result_dir1 = false;
    let mut result_dir2 = false;
    let dir1_pressed = inputs.get(dir1);
    let dir2_pressed = inputs.get(dir2);
    if dir1_pressed && dir2_pressed {
        if s.was_dir2 {
            result_dir1 = true;
            result_dir2 = false;
            s.lock_dir2 = true;
        }
        if s.was_dir1 {
            result_dir1 = false;
            result_dir2 = true;
            s.lock_dir1 = true;
        }
    }
    if !dir1_pressed && dir2_pressed && !s.lock_dir2 {
        result_dir1 = false;
        result_dir2 = true;
        s.was_dir2 = true;
        s.was_dir1 = false;
        s.lock_dir1 = false;
    }
    if dir1_pressed && !dir2_pressed && !s.lock_dir1 {
        result_dir1 = true;
        result_dir2 = false;
        s.was_dir1 = true;
        s.was_dir2 = false;
        s.lock_dir2 = false;
    }
    if !dir1_pressed && !dir2_pressed {
        s.was_dir2 = false;
        s.was_dir1 = false;
        s.lock_dir1 = false;
        s.lock_dir2 = false;
    }
    inputs.set(dir1, result_dir1);
    inputs.set(dir2, result_dir2);
}

/// `socd::second_input_priority`.
fn second_input_priority(inputs: &mut Inputs, dir1: Button, dir2: Button, s: &mut SocdState) {
    let mut result_dir1 = false;
    let mut result_dir2 = false;
    let dir1_pressed = inputs.get(dir1);
    let dir2_pressed = inputs.get(dir2);
    if dir1_pressed && s.was_dir2 {
        result_dir1 = true;
        result_dir2 = false;
    }
    if dir2_pressed && s.was_dir1 {
        result_dir1 = false;
        result_dir2 = true;
    }
    if !dir1_pressed && dir2_pressed {
        result_dir1 = false;
        result_dir2 = true;
        s.was_dir2 = true;
        s.was_dir1 = false;
    }
    if dir1_pressed && !dir2_pressed {
        result_dir1 = true;
        result_dir2 = false;
        s.was_dir1 = true;
        s.was_dir2 = false;
    }
    inputs.set(dir1, result_dir1);
    inputs.set(dir2, result_dir2);
}

/// `socd::neutral`.
fn neutral(inputs: &mut Inputs, dir1: Button, dir2: Button) {
    if inputs.get(dir1) && inputs.get(dir2) {
        inputs.set(dir1, false);
        inputs.set(dir2, false);
    }
}

/// `socd::dir1_priority`.
fn dir1_priority(inputs: &mut Inputs, dir1: Button, dir2: Button) {
    if inputs.get(dir1) && inputs.get(dir2) {
        inputs.set(dir2, false);
    }
}

/// The Melee mode's options (`MeleeOptions`), HayBox's defaults by default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MeleeOptions {
    /// Down and forward with no shield held give 7000 6875, the crouch-walk option select,
    /// instead of 7000 7000.
    pub crouch_walk_os: bool,
    /// Left and right together with no vertical direction no longer give a full horizontal.
    pub disable_ledgedash_socd_override: bool,
    /// Mod X's air dodge coordinate in place of the B0XX's 6375 3750 (units of 0.0125).
    pub custom_airdodge: Option<(u8, u8)>,
}

/// The GameCube report the box sends (HayBox's `OutputState`, the parts a GameCube cable
/// carries): the sticks and triggers as the report's bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Output {
    pub a: bool,
    pub b: bool,
    pub x: bool,
    pub y: bool,
    /// `buttonR`, which the GameCube backend sends as Z.
    pub z: bool,
    pub l: bool,
    pub r: bool,
    pub start: bool,
    pub dpad_up: bool,
    pub dpad_down: bool,
    pub dpad_left: bool,
    pub dpad_right: bool,
    pub stick_x: u8,
    pub stick_y: u8,
    pub c_x: u8,
    pub c_y: u8,
    pub l_analog: u8,
    pub r_analog: u8,
}

impl Default for Output {
    /// `OutputState`'s initial values: nothing pressed, the sticks centred.
    fn default() -> Output {
        Output {
            a: false,
            b: false,
            x: false,
            y: false,
            z: false,
            l: false,
            r: false,
            start: false,
            dpad_up: false,
            dpad_down: false,
            dpad_left: false,
            dpad_right: false,
            stick_x: 128,
            stick_y: 128,
            c_x: 128,
            c_y: 128,
            l_analog: 0,
            r_analog: 0,
        }
    }
}

/// The HayBox commit this module is ported from (firmware 3.1.0).
pub const UPSTREAM_COMMIT: &str = "2cffcd239800500aad7098cdb7c6ac55eccf5ea9";

const ANALOG_STICK_MIN: u8 = 48;
const ANALOG_STICK_NEUTRAL: u8 = 128;
const ANALOG_STICK_MAX: u8 = 208;

/// `StickDirections`: the directions held, after SOCD resolution.
#[derive(Clone, Copy, Debug, Default)]
struct Directions {
    horizontal: bool,
    vertical: bool,
    diagonal: bool,
    x: i32,
    y: i32,
    cx: i32,
    cy: i32,
}

/// `128 + (direction * value)` as HayBox computes it, into the report's byte.
fn at(direction: i32, value: i32) -> u8 {
    (128 + direction * value) as u8
}

/// HayBox's Melee mode, with the SOCD memory it keeps from one report to the next.
#[derive(Clone, Debug)]
pub struct Melee20Button {
    options: MeleeOptions,
    /// Each SOCD pair and its resolution: left and right, down and up, C-left and C-right,
    /// C-down and C-up (the Melee mode's `socd_pairs` in `config_defaults.hpp`).
    pairs: [(Button, Button, SocdType); 4],
    states: [SocdState; 4],
    /// Left and right both held before SOCD resolution (`_horizontal_socd`).
    horizontal_socd: bool,
}

impl Default for Melee20Button {
    fn default() -> Self {
        Melee20Button::new(MeleeOptions::default())
    }
}

impl Melee20Button {
    /// The Melee mode as HayBox sets it up by default: each pair second input priority without
    /// reactivation.
    pub fn new(options: MeleeOptions) -> Melee20Button {
        Melee20Button::with_socd(options, SocdType::SecondInputPriorityNoReactivation)
    }

    /// The Melee mode with every pair resolved as `socd`.
    pub fn with_socd(options: MeleeOptions, socd: SocdType) -> Melee20Button {
        Melee20Button {
            options,
            pairs: [
                (Button::Lf3, Button::Lf1, socd),
                (Button::Lf2, Button::Rf4, socd),
                (Button::Rt3, Button::Rt5, socd),
                (Button::Rt2, Button::Rt4, socd),
            ],
            states: [SocdState::default(); 4],
            horizontal_socd: false,
        }
    }

    /// Forgets what the SOCD resolution remembers (the box unplugged and back).
    pub fn reset(&mut self) {
        self.states = [SocdState::default(); 4];
        self.horizontal_socd = false;
    }

    /// `ControllerMode::UpdateOutputs` for one report: the buttons held in, the report out.
    pub fn update(&mut self, inputs: Inputs) -> Output {
        let mut inputs = inputs;
        self.handle_socd(&mut inputs);
        let mut outputs = Output::default();
        update_digital_outputs(&inputs, &mut outputs);
        self.update_analog_outputs(&inputs, &mut outputs);
        outputs
    }

    /// `Melee20Button::HandleSocd`, then `InputMode::HandleSocd`.
    fn handle_socd(&mut self, inputs: &mut Inputs) {
        self.horizontal_socd = inputs.get(Button::Lf3) && inputs.get(Button::Lf1);
        for (i, &(dir1, dir2, kind)) in self.pairs.iter().enumerate() {
            let s = &mut self.states[i];
            match kind {
                SocdType::Neutral => neutral(inputs, dir1, dir2),
                SocdType::SecondInputPriority => second_input_priority(inputs, dir1, dir2, s),
                SocdType::SecondInputPriorityNoReactivation => {
                    second_input_priority_no_reactivation(inputs, dir1, dir2, s)
                }
                SocdType::Dir1Priority => dir1_priority(inputs, dir1, dir2),
                SocdType::Dir2Priority => dir1_priority(inputs, dir2, dir1),
            }
        }
    }

    /// `Melee20Button::UpdateAnalogOutputs`.
    fn update_analog_outputs(&self, inputs: &Inputs, outputs: &mut Output) {
        let i = |b: Button| inputs.get(b);
        // Coordinate calculations to make modifier handling simpler.
        let directions = update_directions(
            i(Button::Lf3), // Left
            i(Button::Lf1), // Right
            i(Button::Lf2), // Down
            i(Button::Rf4), // Up
            i(Button::Rt3), // C-Left
            i(Button::Rt5), // C-Right
            i(Button::Rt2), // C-Down
            i(Button::Rt4), // C-Up
            outputs,
        );
        let d = directions;

        let shield_button_pressed =
            i(Button::Lf4) || i(Button::Rf5) || i(Button::Rf7) || i(Button::Rf8);
        if d.diagonal {
            // q1/2 = 7000 7000
            outputs.stick_x = at(d.x, 56);
            outputs.stick_y = at(d.y, 56);
            // L, R, LS, and MS + q3/4 = 7000 6875 (For vanilla shield drop. Gives 44.5 degree
            // wavedash). Also used as default q3/4 diagonal if crouch walk option select is
            // enabled.
            if d.y == -1 && (shield_button_pressed || self.options.crouch_walk_os) {
                outputs.stick_x = at(d.x, 56);
                outputs.stick_y = at(d.y, 55);
            }
        }

        // Mod X
        if i(Button::Lt1) {
            // MX + Horizontal (even if shield is held) = 6625 = 53
            if d.horizontal {
                outputs.stick_x = at(d.x, 53);
            }
            // MX + Vertical (even if shield is held) = 5375 = 43
            if d.vertical {
                outputs.stick_y = at(d.y, 43);
            }
            if d.diagonal && shield_button_pressed {
                // Use custom airdodge angle if set, otherwise B0XX standard default.
                if let Some((x, y)) = self.options.custom_airdodge {
                    outputs.stick_x = at(d.x, i32::from(x));
                    outputs.stick_y = at(d.y, i32::from(y));
                } else {
                    // MX + L, R, LS, and MS + q1/2/3/4 = 6375 3750 = 51 30
                    outputs.stick_x = at(d.x, 51);
                    outputs.stick_y = at(d.y, 30);
                }
            }

            // Up B angles
            if d.diagonal && !shield_button_pressed {
                // 22.9638 - 7375 3125 = 59 25
                outputs.stick_x = at(d.x, 59);
                outputs.stick_y = at(d.y, 25);
                // 27.37104 - 7000 3625 (27.38) = 56 29
                if i(Button::Rt2) {
                    outputs.stick_x = at(d.x, 56);
                    outputs.stick_y = at(d.y, 29);
                }
                // 31.77828 - 7875 4875 (31.76) = 63 39
                if i(Button::Rt3) {
                    outputs.stick_x = at(d.x, 63);
                    outputs.stick_y = at(d.y, 39);
                }
                // 36.18552 - 7000 5125 (36.21) = 56 41
                if i(Button::Rt4) {
                    outputs.stick_x = at(d.x, 56);
                    outputs.stick_y = at(d.y, 41);
                }
                // 40.59276 - 6125 5250 (40.6) = 49 42
                if i(Button::Rt5) {
                    outputs.stick_x = at(d.x, 49);
                    outputs.stick_y = at(d.y, 42);
                }

                // Extended Up B Angles
                if i(Button::Rf1) {
                    // 22.9638 - 9125 3875 (23.0) = 73 31
                    outputs.stick_x = at(d.x, 73);
                    outputs.stick_y = at(d.y, 31);
                    // 27.37104 - 8750 4500 (27.2) = 70 36
                    if i(Button::Rt2) {
                        outputs.stick_x = at(d.x, 70);
                        outputs.stick_y = at(d.y, 36);
                    }
                    // 31.77828 - 8500 5250 (31.7) = 68 42
                    if i(Button::Rt3) {
                        outputs.stick_x = at(d.x, 68);
                        outputs.stick_y = at(d.y, 42);
                    }
                    // 36.18552 - 7375 5375 (36.1) = 59 43
                    if i(Button::Rt4) {
                        outputs.stick_x = at(d.x, 59);
                        outputs.stick_y = at(d.y, 43);
                    }
                    // 40.59276 - 6375 5375 (40.1) = 51 43
                    if i(Button::Rt5) {
                        outputs.stick_x = at(d.x, 51);
                        outputs.stick_y = at(d.y, 43);
                    }
                }
            }

            // Angled fsmash
            if d.cx != 0 {
                // 8500 5250 = 68 42
                outputs.c_x = at(d.cx, 68);
                outputs.c_y = at(d.y, 42);
            }
        }

        // Mod Y
        if i(Button::Lt2) {
            // MY + Horizontal (even if shield is held) = 3375 = 27
            if d.horizontal {
                outputs.stick_x = at(d.x, 27);
            }
            // MY + Vertical (even if shield is held) = 7375 = 59
            if d.vertical {
                outputs.stick_y = at(d.y, 59);
            }
            if d.diagonal && shield_button_pressed {
                // MY + L, R, LS, and MS + q1/2 = 4750 8750 = 38 70
                outputs.stick_x = at(d.x, 38);
                outputs.stick_y = at(d.y, 70);
                // MY + L, R, LS, and MS + q3/4 = 5000 8500 = 40 68
                if d.y == -1 {
                    outputs.stick_x = at(d.x, 40);
                    outputs.stick_y = at(d.y, 68);
                }
            }

            // Turnaround neutral B nerf
            if i(Button::Rf1) {
                outputs.stick_x = at(d.x, 80);
            }

            // Up B angles
            if d.diagonal && !shield_button_pressed {
                // 67.0362 - 3125 7375 = 25 59
                outputs.stick_x = at(d.x, 25);
                outputs.stick_y = at(d.y, 59);
                // 62.62896 - 3625 7000 (62.62) = 29 56
                if i(Button::Rt2) {
                    outputs.stick_x = at(d.x, 29);
                    outputs.stick_y = at(d.y, 56);
                }
                // 58.22172 - 4875 7875 (58.24) = 39 63
                if i(Button::Rt3) {
                    outputs.stick_x = at(d.x, 39);
                    outputs.stick_y = at(d.y, 63);
                }
                // 53.81448 - 5125 7000 (53.79) = 41 56
                if i(Button::Rt4) {
                    outputs.stick_x = at(d.x, 41);
                    outputs.stick_y = at(d.y, 56);
                }
                // 49.40724 - 6375 7625 (50.10) = 51 61
                if i(Button::Rt5) {
                    outputs.stick_x = at(d.x, 51);
                    outputs.stick_y = at(d.y, 61);
                }

                // Extended Up B Angles
                if i(Button::Rf1) {
                    // 67.0362 - 3875 9125 = 31 73
                    outputs.stick_x = at(d.x, 31);
                    outputs.stick_y = at(d.y, 73);
                    // 62.62896 - 4500 8750 (62.8) = 36 70
                    if i(Button::Rt2) {
                        outputs.stick_x = at(d.x, 36);
                        outputs.stick_y = at(d.y, 70);
                    }
                    // 58.22172 - 5250 8500 (58.3) = 42 68
                    if i(Button::Rt3) {
                        outputs.stick_x = at(d.x, 42);
                        outputs.stick_y = at(d.y, 68);
                    }
                    // 53.81448 - 5875 8000 (53.7) = 47 64
                    if i(Button::Rt4) {
                        outputs.stick_x = at(d.x, 47);
                        outputs.stick_y = at(d.y, 64);
                    }
                    // 49.40724 - 5875 7125 (50.49) = 47 57
                    if i(Button::Rt5) {
                        outputs.stick_x = at(d.x, 47);
                        outputs.stick_y = at(d.y, 57);
                    }
                }
            }
        }

        // C-stick ASDI Slideoff angle overrides any other C-stick modifiers (such as angled
        // fsmash).
        if d.cx != 0 && d.cy != 0 {
            // 5250 8500 = 42 68
            outputs.c_x = at(d.cx, 42);
            outputs.c_y = at(d.cy, 68);
        }

        // Horizontal SOCD overrides X-axis modifiers (for ledgedash maximum jump trajectory).
        if !self.options.disable_ledgedash_socd_override && self.horizontal_socd && !d.vertical {
            outputs.stick_x = at(d.x, 80);
        }

        if i(Button::Rf7) {
            outputs.r_analog = 49;
        }
        if i(Button::Rf8) {
            outputs.r_analog = 94;
        }

        if outputs.l {
            outputs.l_analog = 140;
        }
        if outputs.r {
            outputs.r_analog = 140;
        }

        // Shut off C-stick when using D-Pad layer.
        if i(Button::Lt1) && i(Button::Lt2) {
            outputs.c_x = 128;
            outputs.c_y = 128;
        }
    }
}

/// `Melee20Button::UpdateDigitalOutputs` (with no Nunchuk).
fn update_digital_outputs(inputs: &Inputs, outputs: &mut Output) {
    let i = |b: Button| inputs.get(b);
    outputs.a = i(Button::Rt1);
    outputs.b = i(Button::Rf1);
    outputs.x = i(Button::Rf2);
    outputs.y = i(Button::Rf6);
    outputs.z = i(Button::Rf3);
    outputs.l = i(Button::Lf4);
    outputs.r = i(Button::Rf5);
    outputs.start = i(Button::Mb1);

    // Activate D-Pad layer by holding Mod X + Mod Y.
    if i(Button::Lt1) && i(Button::Lt2) {
        outputs.dpad_up = i(Button::Rt4);
        outputs.dpad_down = i(Button::Rt2);
        outputs.dpad_left = i(Button::Rt3);
        outputs.dpad_right = i(Button::Rt5);
    }

    if i(Button::Mb3) {
        outputs.dpad_left = true;
    }
    if i(Button::Mb2) {
        outputs.dpad_right = true;
    }
}

/// `ControllerMode::UpdateDirections` with the Melee mode's stick range (48, 128, 208).
#[allow(clippy::too_many_arguments)]
fn update_directions(
    ls_left: bool,
    ls_right: bool,
    ls_down: bool,
    ls_up: bool,
    rs_left: bool,
    rs_right: bool,
    rs_down: bool,
    rs_up: bool,
    outputs: &mut Output,
) -> Directions {
    let mut d = Directions::default();

    outputs.stick_x = ANALOG_STICK_NEUTRAL;
    outputs.stick_y = ANALOG_STICK_NEUTRAL;
    outputs.c_x = ANALOG_STICK_NEUTRAL;
    outputs.c_y = ANALOG_STICK_NEUTRAL;

    // Coordinate calculations to make modifier handling simpler.
    if ls_left || ls_right {
        d.horizontal = true;
        if ls_left {
            d.x = -1;
            outputs.stick_x = ANALOG_STICK_MIN;
        } else {
            d.x = 1;
            outputs.stick_x = ANALOG_STICK_MAX;
        }
    }
    if ls_down || ls_up {
        d.vertical = true;
        if ls_down {
            d.y = -1;
            outputs.stick_y = ANALOG_STICK_MIN;
        } else {
            d.y = 1;
            outputs.stick_y = ANALOG_STICK_MAX;
        }
    }
    if d.horizontal && d.vertical {
        d.diagonal = true;
    }

    if rs_left || rs_right {
        if rs_left {
            d.cx = -1;
            outputs.c_x = ANALOG_STICK_MIN;
        } else {
            d.cx = 1;
            outputs.c_x = ANALOG_STICK_MAX;
        }
    }
    if rs_down || rs_up {
        if rs_down {
            d.cy = -1;
            outputs.c_y = ANALOG_STICK_MIN;
        } else {
            d.cy = 1;
            outputs.c_y = ANALOG_STICK_MAX;
        }
    }
    d
}

#[cfg(test)]
mod tests;
