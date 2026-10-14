//! The items' loaded data: an item kind's `Article` (its attributes, states, model and
//! dynamics) and `ItCo.dat`'s `ItemCommonData`, read in place from the archives.

use melee_data::archive::{Archives, DataRef};

use crate::game::structs::{_itECB, Vec2};

/// An `Article`: what an item kind's data holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Article(pub DataRef);

impl Article {
    /// Its common attributes (`x0_common_attr`, an `ItemAttr`).
    pub fn attr(self, data: &Archives) -> DataRef {
        data.ptr(self.0).expect("an article without attributes")
    }

    /// The kind's own attributes (`x4_specialAttributes`).
    pub fn special(self, data: &Archives) -> Option<DataRef> {
        data.ptr(self.0.at(4))
    }

    /// Its hurtbox bones (`x8_hurtbones`, an `ItHurtBoneList`).
    pub fn hurtbones(self, data: &Archives) -> Option<DataRef> {
        data.ptr(self.0.at(8))
    }

    /// Its state descriptions (`xC_itemStates`, an `ItemStateArray`).
    pub fn states(self, data: &Archives) -> Option<DataRef> {
        data.ptr(self.0.at(0xC))
    }

    /// Its model (`x10_modelDesc`, an `ItemModelDesc`).
    pub fn model(self, data: &Archives) -> ModelDesc {
        ModelDesc(
            data.ptr(self.0.at(0x10))
                .expect("an article without a model"),
        )
    }

    /// Its dynamics (`x14_dynamics`, an `ItemDynamics`).
    pub fn dynamics(self, data: &Archives) -> Option<DataRef> {
        data.ptr(self.0.at(0x14))
    }
}

/// An `ItemModelDesc`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelDesc(pub DataRef);

impl ModelDesc {
    pub fn joint(self, data: &Archives) -> Option<DataRef> {
        data.ptr(self.0)
    }

    pub fn bone_count(self, data: &Archives) -> u32 {
        data.u32(self.0.at(4))
    }

    pub fn bone_attach_id(self, data: &Archives) -> i32 {
        data.u32(self.0.at(8)) as i32
    }

    pub fn bit_field(self, data: &Archives) -> u8 {
        data.u8(self.0.at(0xC))
    }
}

/// An `ItemAttr`: an item kind's common attributes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attr(pub DataRef);

impl Attr {
    pub fn is_heavy(self, data: &Archives) -> bool {
        data.u8(self.0) & 0x80 != 0
    }

    /// `x0_78`: how a fighter uses the item (0 thrown, 2 swung, 3 shot, ...).
    pub fn x0_78(self, data: &Archives) -> u8 {
        (data.u8(self.0) >> 3) & 0xF
    }

    /// `x1_1`: the axis the item spins about.
    pub fn x1_1(self, data: &Archives) -> u8 {
        data.u8(self.0.at(1)) >> 6
    }

    pub fn x1_3(self, data: &Archives) -> bool {
        data.u8(self.0.at(1)) & 0x20 != 0
    }

    pub fn x1_4(self, data: &Archives) -> bool {
        data.u8(self.0.at(1)) & 0x10 != 0
    }

    pub fn x1_5(self, data: &Archives) -> bool {
        data.u8(self.0.at(1)) & 0x08 != 0
    }

    /// `x1_67_cam_kind`: whether and how the camera follows the item.
    pub fn cam_kind(self, data: &Archives) -> u8 {
        data.u8(self.0.at(1)) >> 1 & 3
    }

    pub fn f32(self, data: &Archives, off: u32) -> f32 {
        data.f32(self.0.at(off))
    }

    pub fn i32(self, data: &Archives, off: u32) -> i32 {
        data.u32(self.0.at(off)) as i32
    }

    /// An `itECB` at `off` (`x20`, `x40`).
    pub fn ecb(self, data: &Archives, off: u32) -> _itECB {
        let f = |o| self.f32(data, off + o);
        _itECB {
            top: f(0),
            bottom: f(4),
            right: f(8),
            left: f(0xC),
        }
    }

    pub fn vec2(self, data: &Archives, off: u32) -> Vec2 {
        Vec2 {
            x: self.f32(data, off),
            y: self.f32(data, off + 4),
        }
    }
}

/// `ItemAttr` field offsets.
pub mod attr {
    /// `x4_throw_speed_mul`.
    pub const THROW_SPEED_MUL: u32 = 0x4;
    pub const SPIN_SPEED: u32 = 0xC;
    pub const FALL_SPEED: u32 = 0x10;
    pub const FALL_SPEED_MAX: u32 = 0x14;
    pub const DAMAGE_MUL: u32 = 0x1C;
    pub const ECB: u32 = 0x20;
    pub const X30: u32 = 0x30;
    pub const GRAB_RANGE: u32 = 0x38;
    pub const X40: u32 = 0x40;
    /// `x50`: how fast it slides down slopes.
    pub const X50: u32 = 0x50;
    pub const X54: u32 = 0x54;
    /// `x58`: how much speed it keeps bouncing off walls; `x5C`: the speed it stops below.
    pub const X58: u32 = 0x58;
    pub const X5C: u32 = 0x5C;
    /// `x18`: the upward speed something knocked out of a container gets.
    pub const X18: u32 = 0x18;
    pub const SCALE: u32 = 0x60;
    pub const DESTROY_GFX: u32 = 0x64;
    pub const X68: u32 = 0x68;
    pub const X6C: u32 = 0x6C;
    pub const X70: u32 = 0x70;
    pub const X74: u32 = 0x74;
    pub const DESTROY_SFX: u32 = 0x78;
    pub const X7C: u32 = 0x7C;
    pub const X80: u32 = 0x80;
}

/// `ItCo.dat`'s `ItemCommonData` (`it_804D6D28`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Common(pub DataRef);

impl Common {
    pub fn u32(self, data: &Archives, off: u32) -> u32 {
        data.u32(self.0.at(off))
    }

    pub fn f32(self, data: &Archives, off: u32) -> f32 {
        data.f32(self.0.at(off))
    }

    pub fn i32(self, data: &Archives, off: u32) -> i32 {
        data.u32(self.0.at(off)) as i32
    }
}

/// `ItemCommonData` field offsets.
pub mod common {
    pub const X2C: u32 = 0x2C;
    pub const LIFETIME: u32 = 0x30;
    pub const X34: u32 = 0x34;
    pub const X38: u32 = 0x38;
    pub const X44: u32 = 0x44;
    pub const X48: u32 = 0x48;
    pub const X54: u32 = 0x54;
    pub const X58: u32 = 0x58;
    pub const X5C: u32 = 0x5C;
    pub const X60: u32 = 0x60;
    pub const X70: u32 = 0x70;
    pub const X4C: u32 = 0x4C;
    pub const X68: u32 = 0x68;
    pub const X6C: u32 = 0x6C;
    pub const X74: u32 = 0x74;
    pub const X7C: u32 = 0x7C;
    /// `x80_float`: per hold kind.
    pub const X80: u32 = 0x80;
    pub const XB4: u32 = 0xB4;
    pub const XB8: u32 = 0xB8;
    pub const XBC: u32 = 0xBC;
    pub const XC8: u32 = 0xC8;
    pub const XCC: u32 = 0xCC;
    pub const XD4: u32 = 0xD4;
    pub const XD8: u32 = 0xD8;
    pub const XC0: u32 = 0xC0;
    pub const XC4: u32 = 0xC4;
    pub const UNK_DEGREES: u32 = 0xE0;
    pub const XE8: u32 = 0xE8;
    pub const XF0: u32 = 0xF0;
    pub const XF4: u32 = 0xF4;
    pub const XF8: u32 = 0xF8;
    pub const XFC: u32 = 0xFC;
    pub const X124: u32 = 0x124;
    pub const X128: u32 = 0x128;
    pub const X14C: u32 = 0x14C;
    pub const X150: u32 = 0x150;
    pub const X154: u32 = 0x154;
    pub const X158: u32 = 0x158;
}
