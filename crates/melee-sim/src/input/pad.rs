//! The SDK's PAD library (`dolphin/pad`): what turns a controller's status response into a
//! `PADStatus`, centred on the origin the controller reported, and the octagon clamp of
//! `PADClamp`.
//!
//! Polling, probing, resets, origin reads and rumble talk to the serial interface. melee-hd's
//! input layer replaces them (`ledger/replaced.txt`) and hands in, per port and poll, what the
//! hardware reported: a [`Sample`], the controller's raw origin and its SI type.

/// `PADStatus`: one controller's state as the SDK reports it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct PadStatus {
    pub button: u16,
    pub stick_x: i8,
    pub stick_y: i8,
    pub substick_x: i8,
    pub substick_y: i8,
    pub trigger_left: u8,
    pub trigger_right: u8,
    pub analog_a: u8,
    pub analog_b: u8,
    pub err: i8,
}

/// `PAD_ERR_*`.
pub const ERR_NONE: i8 = 0;
pub const ERR_NO_CONTROLLER: i8 = -1;
pub const ERR_NOT_READY: i8 = -2;
pub const ERR_TRANSFER: i8 = -3;

/// `PAD_BUTTON_*` and `PAD_TRIGGER_*`.
pub const BUTTON_LEFT: u16 = 0x0001;
pub const BUTTON_RIGHT: u16 = 0x0002;
pub const BUTTON_DOWN: u16 = 0x0004;
pub const BUTTON_UP: u16 = 0x0008;
pub const TRIGGER_Z: u16 = 0x0010;
pub const TRIGGER_R: u16 = 0x0020;
pub const TRIGGER_L: u16 = 0x0040;
pub const BUTTON_A: u16 = 0x0100;
pub const BUTTON_B: u16 = 0x0200;
pub const BUTTON_X: u16 = 0x0400;
pub const BUTTON_Y: u16 = 0x0800;
pub const BUTTON_START: u16 = 0x1000;
/// The controller asks for its origin to be read again.
pub const ORIGIN_REQUEST: u16 = 0x2000;
/// Set in every response; `PADRead` clears it.
pub const USE_ORIGIN: u16 = 0x0080;
/// `PAD_ALL`: the bits of a response's first word that are buttons.
const ALL: u32 = 0x3FFF;

impl PadStatus {
    /// What `PADRead` reports for a port that gave no status: every input cleared
    /// (`memset(status, 0, 10)`) and the error.
    pub const fn error(err: i8) -> PadStatus {
        PadStatus {
            button: 0,
            stick_x: 0,
            stick_y: 0,
            substick_x: 0,
            substick_y: 0,
            trigger_left: 0,
            trigger_right: 0,
            analog_a: 0,
            analog_b: 0,
            err,
        }
    }

    /// `SPEC2_MakeStatus`, the decoder `PADSetSpec(5)` selects: unpacks a status response in
    /// analog mode 3 (the only one Melee uses: `PADSetAnalogMode` isn't linked) and subtracts
    /// the origin from the sticks and triggers. `err` is left as it was.
    #[doc(alias = "SPEC2_MakeStatus")]
    pub fn decode(&mut self, data: [u32; 2], origin: &PadStatus) {
        self.button = ((data[0] >> 16) & ALL) as u16;
        self.stick_x = (data[0] >> 8) as i8;
        self.stick_y = data[0] as i8;
        self.substick_x = (data[1] >> 24) as i8;
        self.substick_y = (data[1] >> 16) as i8;
        self.trigger_left = (data[1] >> 8) as u8;
        self.trigger_right = data[1] as u8;
        self.analog_a = 0;
        self.analog_b = 0;
        self.stick_x = recentre(self.stick_x);
        self.stick_y = recentre(self.stick_y);
        self.substick_x = recentre(self.substick_x);
        self.substick_y = recentre(self.substick_y);
        self.stick_x = clamp_s8(self.stick_x, origin.stick_x);
        self.stick_y = clamp_s8(self.stick_y, origin.stick_y);
        self.substick_x = clamp_s8(self.substick_x, origin.substick_x);
        self.substick_y = clamp_s8(self.substick_y, origin.substick_y);
        self.trigger_left = clamp_u8(self.trigger_left, origin.trigger_left);
        self.trigger_right = clamp_u8(self.trigger_right, origin.trigger_right);
    }
}

/// A stick byte the hardware centres on 128, as a signed offset.
fn recentre(v: i8) -> i8 {
    i32::from(v).wrapping_sub(128) as i8
}

/// `ClampS8`: the axis minus its origin, kept from wrapping past the far end.
fn clamp_s8(var: i8, org: i8) -> i8 {
    let mut var = var;
    if org > 0 {
        let min = (-128 + i32::from(org)) as i8;
        if var < min {
            var = min;
        }
    } else if org < 0 {
        let max = (127 + i32::from(org)) as i8;
        if max < var {
            var = max;
        }
    }
    i32::from(var).wrapping_sub(i32::from(org)) as i8
}

/// `ClampU8`: the trigger minus its origin, at least 0.
fn clamp_u8(var: u8, org: u8) -> u8 {
    var.max(org) - org
}

/// What a port reported to one poll: the cases `PADRead` tells apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub enum Sample {
    /// No controller, or one that stopped answering (`PAD_ERR_NO_CONTROLLER`).
    Disconnected,
    /// Being reset or probed (`PAD_ERR_NOT_READY`).
    NotReady,
    /// The channel was busy or the transfer failed (`PAD_ERR_TRANSFER`).
    Transfer,
    /// Just connected, its type still being checked: all inputs clear, no error.
    Waiting,
    /// A status response's two words.
    Response([u32; 2]),
}

/// One port's result from a poll.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct Read {
    pub status: PadStatus,
    /// The controller asked for its origin to be read again: the input layer reads it and
    /// passes it through [`update_origin`] before the next poll.
    pub read_origin: bool,
}

/// `PADRead` for one port, once the hardware has answered: decodes a response against the
/// port's origin (from [`update_origin`]) and sets the error the HSD library acts on.
pub fn read(sample: Sample, origin: &PadStatus) -> Read {
    let error = |err| Read {
        status: PadStatus::error(err),
        read_origin: false,
    };
    match sample {
        Sample::Disconnected => error(ERR_NO_CONTROLLER),
        Sample::NotReady => error(ERR_NOT_READY),
        Sample::Transfer => error(ERR_TRANSFER),
        Sample::Waiting => error(ERR_NONE),
        Sample::Response(data) if data[0] & 0x8000_0000 != 0 => error(ERR_TRANSFER),
        Sample::Response(data) => {
            let mut status = PadStatus::default();
            status.decode(data, origin);
            if status.button & ORIGIN_REQUEST != 0 {
                Read {
                    status: PadStatus::error(ERR_TRANSFER),
                    read_origin: true,
                }
            } else {
                status.err = ERR_NONE;
                status.button &= !USE_ORIGIN;
                Read {
                    status,
                    read_origin: false,
                }
            }
        }
    }
}

/// `UpdateOrigin` in analog mode 3: centres the origin a controller reported (`raw`, the ten
/// bytes the origin command returns). `si_type` is what `SIGetType` reports for the port: a
/// standard controller (`0x0900....`) whose stick X origin sits far right gets it zeroed, as
/// `XPatchBits` (never cleared in Melee) asks.
#[doc(alias = "UpdateOrigin")]
pub fn update_origin(raw: &PadStatus, si_type: u32) -> PadStatus {
    let mut origin = *raw;
    origin.stick_x = recentre(origin.stick_x);
    origin.stick_y = recentre(origin.stick_y);
    origin.substick_x = recentre(origin.substick_x);
    origin.substick_y = recentre(origin.substick_y);
    if origin.stick_x > 0x40 && si_type & 0xFFFF_0000 == 0x0900_0000 {
        origin.stick_x = 0;
    }
    origin
}

/// One stick's octagon in `ClampRegion`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct StickRegion {
    /// The dead zone's radius on each axis.
    pub min: i8,
    /// The furthest reach along an axis.
    pub max: i8,
    /// The furthest reach along a diagonal, per axis.
    pub xy: i8,
}

/// `ClampRegion`'s triggers: dead below `TRIGGER_MIN`, full at `TRIGGER_MAX`.
const TRIGGER_MIN: u8 = 30;
const TRIGGER_MAX: u8 = 180;
/// `ClampRegion`'s sticks.
pub const STICK: StickRegion = StickRegion {
    min: 15,
    max: 72,
    xy: 40,
};
pub const SUBSTICK: StickRegion = StickRegion {
    min: 15,
    max: 59,
    xy: 31,
};

/// `ClampStick`: removes the dead zone and pulls the point inside the region's octagon.
/// `xy` and `max` are positive in every region the SDK passes.
#[doc(alias = "ClampStick")]
pub fn clamp_stick(x: i8, y: i8, region: StickRegion) -> (i8, i8) {
    let (mut x, mut y) = (i32::from(x), i32::from(y));
    let (min, max, xy) = (
        i32::from(region.min),
        i32::from(region.max),
        i32::from(region.xy),
    );
    let sign_x = if x >= 0 {
        1
    } else {
        x = -x;
        -1
    };
    let sign_y = if y >= 0 {
        1
    } else {
        y = -y;
        -1
    };
    x = if x <= min { 0 } else { x - min };
    y = if y <= min { 0 } else { y - min };
    if x == 0 && y == 0 {
        return (0, 0);
    }
    let d = if xy * y <= xy * x {
        xy * x + (max - xy) * y
    } else {
        xy * y + (max - xy) * x
    };
    if xy * max < d {
        x = i32::from((xy * max * x / d) as i8);
        y = i32::from((xy * max * y / d) as i8);
    }
    ((sign_x * x) as i8, (sign_y * y) as i8)
}

/// `ClampTrigger`.
fn clamp_trigger(t: u8) -> u8 {
    if t <= TRIGGER_MIN {
        return 0;
    }
    t.min(TRIGGER_MAX) - TRIGGER_MIN
}

/// `PADClamp`: clamps the sticks and triggers of every port that reported a status.
#[doc(alias = "PADClamp")]
pub fn clamp(statuses: &mut [PadStatus; 4]) {
    for s in statuses.iter_mut().filter(|s| s.err == ERR_NONE) {
        (s.stick_x, s.stick_y) = clamp_stick(s.stick_x, s.stick_y, STICK);
        (s.substick_x, s.substick_y) = clamp_stick(s.substick_x, s.substick_y, SUBSTICK);
        s.trigger_left = clamp_trigger(s.trigger_left);
        s.trigger_right = clamp_trigger(s.trigger_right);
    }
}
