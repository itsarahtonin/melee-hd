//! HAL's pad library (`sysdolphin/baselib/controller.c`): a queue of raw polls, and three
//! views of the controllers built from it each frame. The master status clamps, scales and
//! derives directions from the raw values; the copy and game statuses follow the master and
//! track their own presses, releases and key repeat.
//!
//! The original polls the hardware inside `HSD_PadRenewRawStatus`; here the poll's result is
//! an argument, and the requests it would make of the hardware (resetting a port, driving a
//! rumble motor) come back.

use gekko_fp as fp;
use melee_data::archive::Archives;

use super::pad::{ERR_NO_CONTROLLER, ERR_NONE, ERR_TRANSFER, PadStatus};
use super::rumble::{Motor, Rumble};
use crate::fpu::{mem, reg};
use crate::math;

/// `HSD_PadData`: one poll of the four ports.
pub type PadData = [PadStatus; 4];

/// `HSD_PAD_*`: the button bits of a [`Status`]. The low bits are the SDK's.
pub const BUTTON_LEFT: u32 = 0x0001;
pub const BUTTON_RIGHT: u32 = 0x0002;
pub const BUTTON_DOWN: u32 = 0x0004;
pub const BUTTON_UP: u32 = 0x0008;
pub const BUTTON_A: u32 = 0x0100;
/// The directions [`Controllers::renew_master`] derives from the control stick and C-stick.
pub const STICK_UP: u32 = 0x1_0000;
pub const STICK_DOWN: u32 = 0x2_0000;
pub const STICK_LEFT: u32 = 0x4_0000;
pub const STICK_RIGHT: u32 = 0x8_0000;
pub const SUBSTICK_UP: u32 = 0x10_0000;
pub const SUBSTICK_DOWN: u32 = 0x20_0000;
pub const SUBSTICK_LEFT: u32 = 0x40_0000;
pub const SUBSTICK_RIGHT: u32 = 0x80_0000;

const VERTICAL: u32 = BUTTON_DOWN | BUTTON_UP;
const HORIZONTAL: u32 = BUTTON_LEFT | BUTTON_RIGHT;

/// `HSD_PadStatus`: one controller as the game reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Status {
    pub button: u32,
    pub last_button: u32,
    /// Pressed this frame.
    pub trigger: u32,
    /// Pressed this frame, then every `repeat_interval` frames once held `repeat_start`.
    pub repeat: u32,
    /// Released this frame.
    pub release: u32,
    pub repeat_count: i32,
    pub stick_x: i8,
    pub stick_y: i8,
    pub sub_stick_x: i8,
    pub sub_stick_y: i8,
    pub analog_l: u8,
    pub analog_r: u8,
    pub analog_a: u8,
    pub analog_b: u8,
    pub nml_stick_x: f32,
    pub nml_stick_y: f32,
    pub nml_sub_stick_x: f32,
    pub nml_sub_stick_y: f32,
    pub nml_analog_l: f32,
    pub nml_analog_r: f32,
    pub nml_analog_a: f32,
    pub nml_analog_b: f32,
    /// The D-pad axis that won last (1 vertical, 2 horizontal) when `PadLib::cross_dir` is 3.
    pub cross_dir: u8,
    pub err: i8,
}

impl Status {
    /// `default_status_data`.
    pub const DEFAULT: Status = Status {
        button: 0,
        last_button: 0,
        trigger: 0,
        repeat: 0,
        release: 0,
        repeat_count: 0,
        stick_x: 0,
        stick_y: 0,
        sub_stick_x: 0,
        sub_stick_y: 0,
        analog_l: 0,
        analog_r: 0,
        analog_a: 0,
        analog_b: 0,
        nml_stick_x: 0.0,
        nml_stick_y: 0.0,
        nml_sub_stick_x: 0.0,
        nml_sub_stick_y: 0.0,
        nml_analog_l: 0.0,
        nml_analog_r: 0.0,
        nml_analog_a: 0.0,
        nml_analog_b: 0.0,
        cross_dir: 1,
        err: 0,
    };

    /// `HSD_PadCopyStatusFields`: the inputs, not the edges.
    fn copy_inputs(&mut self, src: &Status) {
        self.button = src.button;
        self.stick_x = src.stick_x;
        self.stick_y = src.stick_y;
        self.sub_stick_x = src.sub_stick_x;
        self.sub_stick_y = src.sub_stick_y;
        self.analog_l = src.analog_l;
        self.analog_r = src.analog_r;
        self.analog_a = src.analog_a;
        self.analog_b = src.analog_b;
        self.nml_stick_x = src.nml_stick_x;
        self.nml_stick_y = src.nml_stick_y;
        self.nml_sub_stick_x = src.nml_sub_stick_x;
        self.nml_sub_stick_y = src.nml_sub_stick_y;
        self.nml_analog_l = src.nml_analog_l;
        self.nml_analog_r = src.nml_analog_r;
        self.nml_analog_a = src.nml_analog_a;
        self.nml_analog_b = src.nml_analog_b;
    }

    /// `HSD_PadClearStatusFields`.
    fn clear_inputs(&mut self) {
        let zero = mem(fp::frsp(0.0));
        self.button = 0;
        self.sub_stick_y = 0;
        self.sub_stick_x = 0;
        self.stick_y = 0;
        self.stick_x = 0;
        self.analog_b = 0;
        self.analog_a = 0;
        self.analog_r = 0;
        self.analog_l = 0;
        self.nml_sub_stick_y = zero;
        self.nml_sub_stick_x = zero;
        self.nml_stick_y = zero;
        self.nml_stick_x = zero;
        self.nml_analog_b = zero;
        self.nml_analog_a = zero;
        self.nml_analog_r = zero;
        self.nml_analog_l = zero;
    }

    /// Presses, releases and key repeat against last frame's buttons.
    fn update_edges(&mut self, repeat_start: i32, repeat_interval: i32) {
        let changed = self.last_button ^ self.button;
        self.trigger = self.button & changed;
        self.release = self.last_button & changed;
        if changed != 0 {
            self.repeat = self.trigger;
            self.repeat_count = repeat_start;
        } else {
            self.repeat_count = self.repeat_count.wrapping_sub(1);
            if self.repeat_count != 0 {
                self.repeat = 0;
            } else {
                self.repeat = self.button;
                self.repeat_count = repeat_interval;
            }
        }
    }

    /// The copy and game statuses' renewal from the master.
    fn follow(&mut self, master: &Status, lib: &PadLib) {
        self.last_button = self.button;
        self.err = master.err;
        if self.err == ERR_NONE {
            self.copy_inputs(master);
        } else {
            self.clear_inputs();
        }
        self.update_edges(lib.repeat_start, lib.repeat_interval);
    }
}

/// What `HSD_PadFlushQueue` does with the polls not yet read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub enum Flush {
    /// Fold every queued poll's buttons into the newest, so no press is lost.
    Merge,
    /// Drop them all.
    Throwaway,
    /// Keep only the newest.
    Leave1,
    /// Nothing.
    Terminate,
}

/// `PadLibData` without its rumble state, which is [`Controllers::rumble`].
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
pub struct PadLib {
    pub qnum: u8,
    pub qread: u8,
    pub qwrite: u8,
    pub qcount: u8,
    /// When the queue is full: 0 merges the oldest poll's buttons into the next, 1 drops the
    /// oldest, 2 drops the new poll.
    pub qtype: u8,
    /// `qnum` polls.
    pub queue: Vec<PadData>,
    pub repeat_start: i32,
    pub repeat_interval: i32,
    /// 0 derives stick directions (`STICK_*`, `SUBSTICK_*`).
    pub adc_type: i8,
    /// The stick distance from which a direction counts.
    pub adc_th: i8,
    /// How far, in radians, each direction's quarter overlaps its neighbours.
    pub adc_angle: f32,
    /// 0 clamps both sticks to a circle.
    pub clamp_stick_type: u8,
    /// 1 removes the dead zone from what's left (sticks) or subtracts the minimum (analogs).
    pub clamp_stick_shift: u8,
    pub clamp_stick_max: i8,
    pub clamp_stick_min: i8,
    pub clamp_analog_lr_shift: u8,
    pub clamp_analog_lr_max: u8,
    pub clamp_analog_lr_min: u8,
    pub clamp_analog_ab_shift: u8,
    pub clamp_analog_ab_max: u8,
    pub clamp_analog_ab_min: u8,
    /// The values the normalized fields divide by.
    pub scale_stick: i8,
    pub scale_analog_lr: u8,
    pub scale_analog_ab: u8,
    /// How the D-pad resolves both axes held at once: 0 keeps both, 1 prefers vertical, 2
    /// horizontal, 3 the axis pressed first.
    pub cross_dir: u8,
    pub reset_switch_status: u8,
    pub reset_switch: u8,
}

impl PadLib {
    /// `default_libinfo_data`, with a queue of `qnum` empty polls.
    fn new(qnum: u8) -> PadLib {
        PadLib {
            qnum,
            qread: 0,
            qwrite: 0,
            qcount: 0,
            qtype: 0,
            queue: vec![[PadStatus::default(); 4]; usize::from(qnum)],
            repeat_start: 45,
            repeat_interval: 8,
            adc_type: 0,
            adc_th: 30,
            adc_angle: 0.0,
            clamp_stick_type: 0,
            clamp_stick_shift: 0,
            clamp_stick_max: 127,
            clamp_stick_min: 0,
            clamp_analog_lr_shift: 0,
            clamp_analog_lr_max: 255,
            clamp_analog_lr_min: 0,
            clamp_analog_ab_shift: 0,
            clamp_analog_ab_max: 255,
            clamp_analog_ab_min: 0,
            scale_stick: 127,
            scale_analog_lr: 255,
            scale_analog_ab: 255,
            cross_dir: 0,
            reset_switch_status: 0,
            reset_switch: 0,
        }
    }

    /// `HSD_PadRawQueueShift`.
    fn shift(&self, at: u8) -> u8 {
        ((i32::from(at) + 1) % i32::from(self.qnum)) as u8
    }

    /// `HSD_PadClamp`.
    fn clamp(&self, s: &mut Status) {
        if self.clamp_stick_type == 0 {
            let (shift, min, max) = (
                self.clamp_stick_shift,
                self.clamp_stick_min,
                self.clamp_stick_max,
            );
            (s.stick_x, s.stick_y) = clamp_stick(s.stick_x, s.stick_y, shift, min, max);
            (s.sub_stick_x, s.sub_stick_y) =
                clamp_stick(s.sub_stick_x, s.sub_stick_y, shift, min, max);
        }
        let lr = (
            self.clamp_analog_lr_shift,
            self.clamp_analog_lr_min,
            self.clamp_analog_lr_max,
        );
        let ab = (
            self.clamp_analog_ab_shift,
            self.clamp_analog_ab_min,
            self.clamp_analog_ab_max,
        );
        s.analog_l = clamp_analog(s.analog_l, lr.0, lr.1, lr.2);
        s.analog_r = clamp_analog(s.analog_r, lr.0, lr.1, lr.2);
        s.analog_a = clamp_analog(s.analog_a, ab.0, ab.1, ab.2);
        s.analog_b = clamp_analog(s.analog_b, ab.0, ab.1, ab.2);
    }

    /// `HSD_PadADConvert`.
    fn convert_directions(&self, s: &mut Status) {
        if self.adc_type == 0 {
            let stick = [STICK_UP, STICK_DOWN, STICK_LEFT, STICK_RIGHT];
            let substick = [SUBSTICK_UP, SUBSTICK_DOWN, SUBSTICK_LEFT, SUBSTICK_RIGHT];
            s.button = self.directions(s.button, s.stick_x, s.stick_y, stick);
            s.button = self.directions(s.button, s.sub_stick_x, s.sub_stick_y, substick);
        }
    }

    /// `HSD_PadADConvertCheck1`: `button` with the directions (up, down, left, right) the
    /// stick at (x, y) points in, if it's at least `adc_th` from the centre. Neighbouring
    /// quarters overlap by `adc_angle`.
    #[doc(alias = "HSD_PadADConvertCheck1")]
    pub fn directions(&self, button: u32, x: i8, y: i8, dirs: [u32; 4]) -> u32 {
        use std::f64::consts::{FRAC_PI_2, FRAC_PI_4};
        const THREE_PI_4: f64 = 2.356194490192345;
        let [up, down, left, right] = dirs;
        let mut button = button;
        let r = sqrt(fp::fmadds(int(x), int(x), fp::fmuls(int(y), int(y))));
        let a = if fp::fabs(f64::from(x)) == 0.0 {
            fp::frsp(if y >= 0 { FRAC_PI_2 } else { -FRAC_PI_2 })
        } else {
            reg(math::atan2(mem(int(y)), mem(int(x))))
        };
        let ha = fp::fmuls(0.5, reg(self.adc_angle));
        if r < int(self.adc_th) {
            return button;
        }
        if a < fp::fadd(-THREE_PI_4, ha) {
            button |= left;
        }
        if a >= fp::fsub(-THREE_PI_4, ha) && a <= fp::fadd(-FRAC_PI_4, ha) {
            button |= down;
        }
        if a > fp::fsub(-FRAC_PI_4, ha) && a < fp::fadd(FRAC_PI_4, ha) {
            button |= right;
        }
        if a >= fp::fsub(FRAC_PI_4, ha) && a <= fp::fadd(THREE_PI_4, ha) {
            button |= up;
        }
        if a > fp::fsub(THREE_PI_4, ha) {
            button |= left;
        }
        button
    }

    /// `HSD_PadScale`.
    fn scale(&self, s: &mut Status) {
        let stick = |v: i8| mem(fp::fdivs(int(v), int(self.scale_stick)));
        let analog = |v: u8, by: u8| mem(fp::fdivs(uint(v), uint(by)));
        s.nml_stick_x = stick(s.stick_x);
        s.nml_stick_y = stick(s.stick_y);
        s.nml_sub_stick_x = stick(s.sub_stick_x);
        s.nml_sub_stick_y = stick(s.sub_stick_y);
        s.nml_analog_l = analog(s.analog_l, self.scale_analog_lr);
        s.nml_analog_r = analog(s.analog_r, self.scale_analog_lr);
        s.nml_analog_a = analog(s.analog_a, self.scale_analog_ab);
        s.nml_analog_b = analog(s.analog_b, self.scale_analog_ab);
    }

    /// `HSD_PadCrossDir`: drops one D-pad axis when both are held, as `cross_dir` says.
    #[doc(alias = "HSD_PadCrossDir")]
    pub fn cross_dir(&self, s: &mut Status) {
        match self.cross_dir {
            1 => {
                if s.button & VERTICAL != 0 {
                    s.button &= !HORIZONTAL;
                }
            }
            2 => {
                if s.button & HORIZONTAL != 0 {
                    s.button &= !VERTICAL;
                }
            }
            3 => {
                if s.button & VERTICAL != 0 {
                    if s.button & HORIZONTAL != 0 {
                        if s.cross_dir == 1 {
                            s.button &= !HORIZONTAL;
                        } else {
                            s.button &= !VERTICAL;
                        }
                    } else {
                        s.cross_dir = 1;
                    }
                } else if s.button & HORIZONTAL != 0 {
                    s.cross_dir = 2;
                }
            }
            _ => {}
        }
    }
}

/// An integer converted to a float as C's `(f32)` does: exactly, then rounded to single.
fn int(v: i8) -> f64 {
    fp::frsp(f64::from(v))
}

fn uint(v: u8) -> f64 {
    fp::frsp(f64::from(v))
}

/// The inline `sqrtf` (math.h) on a register value.
fn sqrt(x: f64) -> f64 {
    reg(math::sqrt(mem(x)))
}

/// `HSD_PadClampCheck1`: an analog value below `min` is 0, above `max` is `max`; with
/// `shift` 1 the result starts from `min`.
#[doc(alias = "HSD_PadClampCheck1")]
pub fn clamp_analog(val: u8, shift: u8, min: u8, max: u8) -> u8 {
    if val < min {
        return 0;
    }
    let val = if val > max { max } else { val };
    if shift != 1 {
        return val;
    }
    val.wrapping_sub(min)
}

/// `HSD_PadClampCheck3`: a stick inside radius `min` is centred, one beyond `max` is pulled
/// back onto the circle; with `shift` 1 the dead zone is then taken off the radius.
#[doc(alias = "HSD_PadClampCheck3")]
pub fn clamp_stick(x: i8, y: i8, shift: u8, min: i8, max: i8) -> (i8, i8) {
    let len = |x: i8, y: i8| {
        sqrt(fp::fadds(
            fp::fmuls(int(x), int(x)),
            fp::fmuls(int(y), int(y)),
        ))
    };
    let (mut x, mut y) = (x, y);
    let mut r = len(x, y);
    if r < int(min) {
        return (0, 0);
    }
    if r > int(max) {
        x = fp::fctiwz(fp::fdivs(fp::fmuls(int(x), int(max)), r)) as i8;
        y = fp::fctiwz(fp::fdivs(fp::fmuls(int(y), int(max)), r)) as i8;
        r = len(x, y);
    }
    if shift == 1 && r > reg(1.0e-10) {
        x = fp::fctiwz(fp::fsubs(int(x), fp::fdivs(fp::fmuls(int(x), int(min)), r))) as i8;
        y = fp::fctiwz(fp::fsubs(int(y), fp::fdivs(fp::fmuls(int(y), int(min)), r))) as i8;
    }
    (x, y)
}

/// What a poll asks of the hardware.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Requests {
    /// The ports to reset (`PADReset`'s mask, bit 31 for port 0): those that reported no
    /// controller.
    pub reset: u32,
    /// The new state of each port's rumble motor, where it changed (`PADControlMotor`).
    pub motors: [Option<Motor>; 4],
}

/// The pad library: the raw queue, rumble and the master, copy and game statuses
/// (`HSD_PadLibData`, `HSD_Rumble_804C22E0`, `HSD_PadMasterStatus`, `HSD_PadCopyStatus`,
/// `HSD_PadGameStatus`).
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
pub struct Controllers {
    /// Slippi's playback codes patch the library (`HSD_PadRenewRawStatus+0x8C`,
    /// `HSD_PadRenewMasterStatus+0x38`): a poll goes to the queue's read position, and the
    /// master statuses renew from it every tick, whether or not one was queued.
    pub playback: bool,
    pub lib: PadLib,
    pub rumble: Rumble,
    /// Polls of a scene's entry whose frame of rumble ran before the entry finished (a
    /// rumble started in it: [`Controllers::rumble_ahead`]), which their queued reads then
    /// skip.
    pub rumble_ahead: u32,
    pub master: [Status; 4],
    pub copy: [Status; 4],
    pub game: [Status; 4],
}

impl Default for Controllers {
    /// The library as Melee sets it up, [`Controllers::melee`].
    fn default() -> Self {
        Controllers::melee()
    }
}

impl Controllers {
    /// `HSD_PadInit`: the library's defaults with a queue of `qnum` polls and `nb_list`
    /// rumble nodes. The original also initializes the SDK's PAD library, which the input
    /// layer does.
    #[doc(alias = "HSD_PadInit")]
    pub fn new(qnum: u8, nb_list: u16) -> Controllers {
        Controllers {
            playback: false,
            lib: PadLib::new(qnum),
            rumble: Rumble::new(nb_list),
            rumble_ahead: 0,
            master: [Status::DEFAULT; 4],
            copy: [Status::DEFAULT; 4],
            game: [Status::DEFAULT; 4],
        }
    }

    /// The library as Melee sets it up (`gmMain_8015FD24`): five queued polls, twelve rumble
    /// nodes, sticks clamped to radius 80 with the dead zone taken off, triggers to 140, and both scaled to
    /// 1. Its `PADSetSpec(5)` picks the decoder [`read`](super::pad::read) always uses.
    #[doc(alias = "gmMain_8015FD24")]
    pub fn melee() -> Controllers {
        let mut c = Controllers::new(5, 12);
        let lib = &mut c.lib;
        lib.clamp_stick_type = 0;
        lib.clamp_stick_shift = 1;
        lib.clamp_stick_max = 80;
        lib.clamp_stick_min = 0;
        lib.scale_stick = 80;
        lib.clamp_analog_lr_shift = 1;
        lib.clamp_analog_lr_max = 140;
        lib.clamp_analog_lr_min = 0;
        lib.scale_analog_lr = 140;
        c
    }

    /// `HSD_PadGetRawQueueCount`: polls waiting in the queue.
    #[doc(alias = "HSD_PadGetRawQueueCount")]
    pub fn raw_queue_count(&self) -> u8 {
        self.lib.qcount
    }

    /// `HSD_PadGetResetSwitch`: the console's reset button was pressed and released.
    #[doc(alias = "HSD_PadGetResetSwitch")]
    pub fn reset_switch(&self) -> bool {
        self.lib.reset_switch != 0
    }

    /// `HSD_PadRenewRawStatus`: runs a frame of rumble, then queues one poll of the four
    /// ports (`now`, what `PADRead` reported: see [`read`](super::pad::read)) and follows the
    /// console's reset button (`reset_pressed`, `OSGetResetSwitchState`). With `err_check`, a
    /// poll in which every port failed is dropped. `data` holds the rumble scripts.
    #[doc(alias = "HSD_PadRenewRawStatus")]
    pub fn renew_raw(
        &mut self,
        data: &Archives,
        now: PadData,
        err_check: bool,
        reset_pressed: bool,
    ) -> Requests {
        let motors = if self.rumble_ahead > 0 {
            self.rumble_ahead -= 1;
            [None; 4]
        } else {
            self.rumble.interpret(data)
        };
        let mut now = now;
        if err_check && now.iter().all(|s| s.err != ERR_NONE) {
            return Requests { reset: 0, motors };
        }
        let p = &mut self.lib;
        let qwrite = usize::from(if self.playback { p.qread } else { p.qwrite });
        let mut skip = false;
        if p.qcount == p.qnum {
            match p.qtype {
                0 => {
                    p.qread = p.shift(p.qread);
                    let qread = usize::from(p.qread);
                    if p.qnum != 1 {
                        let src = p.queue[qwrite];
                        merge(&src, &mut p.queue[qread]);
                    } else {
                        let older = p.queue[qread];
                        for (n, o) in now.iter_mut().zip(&older) {
                            n.button |= o.button;
                        }
                    }
                }
                1 => p.qread = p.shift(p.qread),
                2 => skip = true,
                _ => {}
            }
        } else {
            p.qcount += 1;
        }
        if !skip {
            p.queue[qwrite] = now;
            p.qwrite = p.shift(p.qwrite);
        }
        let reset = now
            .iter()
            .enumerate()
            .filter(|(_, s)| s.err == ERR_NO_CONTROLLER)
            .fold(0, |m, (i, _)| m | 0x8000_0000 >> i);
        if reset_pressed {
            p.reset_switch_status = 1;
        } else if p.reset_switch_status != 0 {
            p.reset_switch = 1;
            p.reset_switch_status = 0;
        }
        Requests { reset, motors }
    }

    /// `HSD_PadFlushQueue`.
    #[doc(alias = "HSD_PadFlushQueue")]
    pub fn flush_queue(&mut self, how: Flush) {
        let p = &mut self.lib;
        match how {
            Flush::Merge => {
                while p.qcount > 1 {
                    let from = usize::from(p.qread);
                    p.qread = p.shift(p.qread);
                    let src = p.queue[from];
                    merge(&src, &mut p.queue[usize::from(p.qread)]);
                    p.qcount -= 1;
                }
            }
            Flush::Throwaway => {
                p.qread = p.qwrite;
                p.qcount = 0;
            }
            Flush::Leave1 => {
                if p.qcount > 1 {
                    p.qread = if p.qwrite != 0 {
                        p.qwrite - 1
                    } else {
                        p.qnum.wrapping_sub(1)
                    };
                    p.qcount = 1;
                }
            }
            Flush::Terminate => {}
        }
    }

    /// `HSD_PadRenewMasterStatus`: takes the oldest queued poll, if any, into the master
    /// statuses: clamped, with stick directions, scaled, D-pad resolved. A port whose
    /// transfer failed keeps last frame's inputs; one with no status reads as released.
    #[doc(alias = "HSD_PadRenewMasterStatus")]
    pub fn renew_master(&mut self) {
        let p = &mut self.lib;
        if p.qcount == 0 {
            if !self.playback {
                return;
            }
            p.qcount = 1;
        }
        let poll = p.queue[usize::from(p.qread)];
        p.qread = p.shift(p.qread);
        p.qcount -= 1;
        let p = &self.lib;
        for (mp, q) in self.master.iter_mut().zip(&poll) {
            mp.last_button = mp.button;
            mp.err = q.err;
            if mp.err == ERR_NONE {
                mp.button = u32::from(q.button);
                mp.stick_x = q.stick_x;
                mp.stick_y = q.stick_y;
                mp.sub_stick_x = q.substick_x;
                mp.sub_stick_y = q.substick_y;
                mp.analog_l = q.trigger_left;
                mp.analog_r = q.trigger_right;
                mp.analog_a = q.analog_a;
                mp.analog_b = q.analog_b;
                p.clamp(mp);
                p.convert_directions(mp);
                p.scale(mp);
                p.cross_dir(mp);
            } else if mp.err == ERR_TRANSFER {
                mp.err = ERR_NONE;
            } else {
                mp.clear_inputs();
            }
            mp.update_edges(p.repeat_start, p.repeat_interval);
        }
    }

    /// `HSD_PadRenewCopyStatus`.
    #[doc(alias = "HSD_PadRenewCopyStatus")]
    pub fn renew_copy(&mut self) {
        for (cp, mp) in self.copy.iter_mut().zip(&self.master) {
            cp.follow(mp, &self.lib);
        }
    }

    /// `HSD_PadRenewGameStatus`.
    #[doc(alias = "HSD_PadRenewGameStatus")]
    pub fn renew_game(&mut self) {
        for (gs, mp) in self.game.iter_mut().zip(&self.master) {
            gs.follow(mp, &self.lib);
        }
    }

    /// `HSD_PadRenewStatus`: runs rumble, queues a poll and renews every status from the
    /// oldest, as [`Controllers::renew_raw`] does.
    #[doc(alias = "HSD_PadRenewStatus")]
    pub fn renew(&mut self, data: &Archives, now: PadData, reset_pressed: bool) -> Requests {
        let requests = self.renew_raw(data, now, false, reset_pressed);
        self.renew_master();
        self.renew_copy();
        self.renew_game();
        requests
    }

    /// `HSD_PadReset`: stops every rumble script, sets every port's motor to brake when idle,
    /// drops the queued polls and forgets the reset button. The original also recalibrates
    /// every port, which is the input layer's job.
    #[doc(alias = "HSD_PadReset")]
    pub fn reset(&mut self) {
        self.rumble.remove_all();
        for port in 0..4 {
            self.rumble.off(port);
        }
        self.flush_queue(Flush::Throwaway);
        self.lib.reset_switch = 0;
    }
}

/// `HSD_PadRawMerge` with `src2 == dst`: ORs one poll's buttons into another's.
fn merge(src: &PadData, dst: &mut PadData) {
    for (d, s) in dst.iter_mut().zip(src) {
        d.button |= s.button;
    }
}
