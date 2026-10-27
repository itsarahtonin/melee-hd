//! What the tournament's scenes share (`gmtoulib.c`): the bracket's boxes from `TmBox.dat`,
//! their lines drawn and their players' icons placed and moved, the camera that follows the
//! match, the names' texts, the random characters and stages, the scenes' cameras, lights,
//! texts and models, and the match's start data.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::super::handles::CodeRef;
use super::super::super::render::{CAMERA_CALLBACK, JOBJ_CALLBACK, LOBJ_CALLBACK};
use super::super::super::state::{OBJ_CAMERA, OBJ_JOBJ, OBJ_LIGHT, Obj, Running, UserData};
use super::super::super::structs::{MatchEnd, StartMeleeData};
use super::{GM_TOURNAMENT, TmData};
use crate::fpu::{mem, reg};
use crate::hsd::cobj;
use crate::hsd::display::{Draw, Quad};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::sis::TextId;
use crate::vec::Vec3;

/// The bracket's process (`fn_8018B090`) and its boxes' lines (`fn_8018E46C`).
pub const BRACKET_PROC: u32 = 0x8018_B090;
pub const BRACKET_RENDER: u32 = 0x8018_E46C;

/// `lbl_803D9D20.x0`: the entrants each of the bracket's 32 choices makes.
pub const ENTRANTS: [u8; 32] = [
    0x04, 0x06, 0x08, 0x0C, 0x10, 0x18, 0x20, 0x30, 0x40, 0x06, 0x09, 0x0C, 0x12, 0x1B, 0x08, 0x0C,
    0x10, 0x10, 0x14, 0x18, 0x1C, 0x20, 0x20, 0x24, 0x28, 0x2C, 0x30, 0x34, 0x38, 0x3C, 0x40, 0x40,
];

/// `lbl_803D9D20.x20`: each choice's boxes, as `TmBox.dat`'s tables list them one choice
/// after another.
const BOXES: [u8; 57] = [
    0x04, 0x07, 0x08, 0x0E, 0x10, 0x19, 0x20, 0x31, 0x3F, 0x04, 0x05, 0x08, 0x0A, 0x0E, 0x04, 0x05,
    0x06, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x10, 0x0D, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x17,
    0x06, 0x05, 0x04, 0x04, 0x06, 0x04, 0x05, 0x04, 0x05, 0x04, 0x04, 0x04, 0x05, 0x04, 0x04, 0x04,
    0x05, 0x06, 0x05, 0x04, 0x05, 0x05, 0x04, 0x04, 0x04,
];

/// `lbl_803D9D20.x59`: the icons' characters in the setup's grid order.
pub const ICON_ORDER: [u8; 25] = [
    0x00, 0x01, 0x02, 0x04, 0x05, 0x06, 0x08, 0x0B, 0x0D, 0x0E, 0x12, 0x11, 0x10, 0x0C, 0x03, 0x16,
    0x07, 0x0F, 0x0A, 0x09, 0x19, 0x15, 0x14, 0x18, 0x17,
];

/// `lbl_803D9E1C`: where the winner's icon goes on the bracket's last box, by choice.
const WINNER_AT: [[i16; 2]; 32] = [
    [0x140, 0x8C],
    [0x15E, 0x82],
    [0x140, 0x96],
    [0x140, 0x78],
    [0x140, 0xA0],
    [0x172, 0xA0],
    [0x140, 0x8C],
    [0x168, 0xEB],
    [0x140, 0xF0],
    [0x140, 0xC8],
    [0x140, 0xBE],
    [0x140, 0x96],
    [0x140, 0xAA],
    [0x140, 0x8C],
    [0x140, 0xB4],
    [0x140, 0xB4],
    [0x140, 0xA0],
    [0x140, 0x8C],
    [0x14E, 0x8C],
    [0x140, 0x8C],
    [0x140, 0xA0],
    [0x140, 0xAA],
    [0x140, 0x96],
    [0x140, 0xF0],
    [0x154, 0xF0],
    [0x13F, 0xF0],
    [0x13F, 0xF0],
    [0x13F, 0xF0],
    [0x147, 0xF0],
    [0x140, 0xF0],
    [0x140, 0xF0],
    [0x140, 0xF0],
];

/// `lbl_803B7C80`: the boxes' bounce, ten steps.
const BOUNCE: [i32; 10] = [30, -20, 15, -12, 10, -8, 6, -4, 2, 1];

/// `lbl_804DA67C` and its copies: the lines' yellow.
const YELLOW: [u8; 4] = [0xFF, 0xFF, 0x00, 0xFF];

/// `lbl_803D9DF4`, `lbl_803D9E08`: the bracket camera's eye and interest at rest.
const EYE: Vec3 = Vec3::new(320.0, -240.0, super::EYE_Z);
const INTEREST: Vec3 = Vec3::new(320.0, -240.0, 0.0);

/// `lbl_803B7CA8`: the bracket's camera.
const BRACKET_CAMERA: cobj::StaticDesc = cobj::StaticDesc {
    flags: 0,
    projection_type: 1,
    viewport: [0, 640, 0, 480],
    scissor: [0, 640, 0, 480],
    eye: EYE,
    interest: INTEREST,
    roll: 0.0,
    near: 0.1,
    far: 30000.0,
    projection: [60.0, f32::from_bits(0x3FAA_AAAA), 0.0, 0.0],
};

/// "HMN"'s and "CPU"'s names (`lbl_803D9EE8`, `lbl_803D9EF4`, `lbl_803D9F00`): Japanese
/// and English guests, and computers, three Shift-JIS characters and a space before the
/// number.
const GUEST_JP: [u8; 7] = [0x83, 0x51, 0x83, 0x58, 0x83, 0x67, 0x20];
const GUEST_US: [u8; 7] = [0x82, 0x67, 0x82, 0x6C, 0x82, 0x6D, 0x20];
const CPU: [u8; 7] = [0x82, 0x62, 0x82, 0x6F, 0x82, 0x74, 0x20];

/// A guest's or a computer's name (`fn_8018ECA8`'s and `fn_8018F00C`'s templates with the
/// number written in): the name and its number from 0 to 99. The templates are the
/// executable's own strings, which these writes change; what's left past the terminator
/// they write is read by nothing.
fn numbered(prefix: &[u8; 7], num: i32) -> Vec<u8> {
    let mut s = prefix.to_vec();
    if num >= 10 {
        s.push((num / 10 + 0x30) as u8);
        s.push((num % 10 + 0x30) as u8);
    } else {
        s.push((num + 0x30) as u8);
    }
    s
}

/// `(f32) i`.
pub(crate) fn f(i: i32) -> f64 {
    fp::frsp(f64::from(i))
}

/// `divw`: a division by zero gives -1 for a negative dividend and 0 else, as ssbm-rs's
/// `div_i32`.
pub(crate) fn divw(a: i32, b: i32) -> i32 {
    if b == 0 || (a == i32::MIN && b == -1) {
        if a < 0 { -1 } else { 0 }
    } else {
        a / b
    }
}

/// `mn_8022F410`: `x` a step of `dx` towards `target`, not past it; which way it moved.
#[doc(alias = "mn_8022F410")]
pub fn step_towards(x: &mut f32, target: f32, dx: f32) -> i32 {
    if *x == target {
        return 0;
    }
    if *x < target {
        let v = fp::fadds(reg(*x), reg(dx));
        *x = if v > reg(target) { target } else { mem(v) };
        1
    } else {
        let v = fp::fsubs(reg(*x), reg(dx));
        *x = if v < reg(target) { target } else { mem(v) };
        -1
    }
}

/// `mn_8022F470`: [`step_towards`] for integers.
#[doc(alias = "mn_8022F470")]
pub fn step_towards_int(x: &mut i32, target: i32, dx: i32) -> i32 {
    if *x == target {
        return 0;
    }
    if *x < target {
        *x = (*x).wrapping_add(dx).min(target);
        1
    } else {
        *x = (*x).wrapping_sub(dx).max(target);
        -1
    }
}

/// `fn_8018F6DC`: a character kind's icon (Sheik's and Zelda's after Zelda's own, Giga
/// Bowser's Bowser's).
#[doc(alias = "fn_8018F6DC")]
pub fn icon_of(ckind: i32) -> i32 {
    if ckind >= 0x13 {
        if ckind == 0x1D {
            return 5;
        }
        return ckind - 1;
    }
    ckind
}

/// `fn_8018F6FC`: an icon's character kind.
#[doc(alias = "fn_8018F6FC")]
pub fn ckind_of(icon: i32) -> i32 {
    if icon >= 0x13 {
        if icon == 0x1D {
            return 5;
        }
        return icon + 1;
    }
    icon
}

/// `fn_8018F71C`: an icon model's frame for an icon and color.
#[doc(alias = "fn_8018F71C")]
pub fn icon_frame(icon: i32, color: i32) -> f32 {
    mem(f(icon.wrapping_add(color.wrapping_mul(0x1E))))
}

/// `fn_8018F310`: where a character kind is in the grid's order, -1 if nowhere.
#[doc(alias = "fn_8018F310")]
pub fn grid_place(ckind: i32) -> i32 {
    ICON_ORDER
        .iter()
        .position(|&k| i32::from(k) == ckind)
        .map_or(-1, |i| i as i32)
}

/// `fn_8018F3BC`: the grid's place `i`'s character kind.
#[doc(alias = "fn_8018F3BC")]
pub fn grid_ckind(i: i32) -> i32 {
    i32::from(ICON_ORDER[i as usize])
}

/// `fn_8018F3D0`: a grid place's group (1 for the unlockable characters' places, 0 for the
/// first fourteen, 2 for the rest).
#[doc(alias = "fn_8018F3D0")]
pub fn grid_group(i: i32) -> i32 {
    if i == 0xE || (0x10..=0x13).contains(&i) || i == 0xA {
        return 1;
    }
    if i <= 0xD {
        return 0;
    }
    2
}

/// `fn_8018F5F0`: the tournament's strings' file.
#[doc(alias = "fn_8018F5F0")]
pub fn strings_file(us: bool) -> &'static str {
    if us { "SdTou.usd" } else { "SdTou.dat" }
}

/// `gm_8018F1B0`: whether a match's tie goes to Sudden Death (with `hmn_cpu_count` and the
/// players a match).
#[doc(alias = "gm_8018F1B0")]
pub fn tie_goes_on(tm: &TmData, me: &MatchEnd) -> bool {
    match tm.match_type() {
        0 => true,
        1 => match tm.hmn_cpu_count() {
            1 => true,
            2 => (tm.x30 == 3 && me.n_winners == 3) || (tm.x30 == 4 && me.n_winners >= 3),
            3 => me.n_winners == 4,
            _ => false,
        },
        2 => match tm.hmn_cpu_count() {
            1 => tm.x30 == me.n_winners,
            2 => tm.x30 == 3 || (tm.x30 == 4 && me.n_winners >= 3),
            3 => true,
            _ => false,
        },
        _ => false,
    }
}

/// The scene's models (`SceneDesc.models[i]`).
pub(crate) fn scene_model(r: &Running, scene: Option<DataRef>, i: u32) -> DataRef {
    let scene = scene.expect("a tournament scene");
    let models = r.data.ptr(scene).expect("the scene's models");
    r.data.ptr(models.at(4 * i)).expect("a scene model")
}

impl Running<'_> {
    /// `gm_8018F1B0` on the tournament's data.
    pub fn tou_tie_goes_on(&self, me: &MatchEnd) -> bool {
        tie_goes_on(&self.game.tou.tm, me)
    }

    /// `fn_8018A514`: the bracket's `count` boxes from `TmBox.dat`'s table for the choice
    /// (of up to 8 entrants, up to 13, or more), their lines `thickness` thick, nobody in
    /// them; the byes the small brackets' odd choices leave.
    #[doc(alias = "fn_8018A514")]
    pub fn tou_bracket_load(&mut self, count: i32, thickness: f32) {
        let region = if count < 9 {
            0
        } else if count >= 14 {
            2
        } else {
            1
        };
        let mut src = self.game.tou.boxes[region].expect("TmBox.dat's tables");
        let start = [0, 9, 14][region];
        for i in start..count {
            src = src.at(0x28 * u32::from(BOXES[i as usize]));
        }
        let n = usize::from(BOXES[count as usize]);
        let d = self.data;
        for i in 0..n {
            let s = src.at(0x28 * i as u32);
            let b = &mut self.game.tou.bracket[i];
            b.x0 = d.u8(s);
            b.x1 = d.u8(s.at(1));
            b.x2 = d.u8(s.at(2));
            b.x3 = d.u8(s.at(3));
            b.x4 = d.u8(s.at(4));
            b.x5 = d.u8(s.at(5));
            b.x6 = d.u8(s.at(6));
            b.x1c = thickness;
            b.xc = d.u32(s.at(8)) as i32;
            b.x14 = d.u32(s.at(0xC)) as i32;
            b.x10 = d.u32(s.at(0x10)) as i32;
            b.x18 = d.u32(s.at(0x14)) as i32;
            b.x20 = [
                d.u8(s.at(0x18)),
                d.u8(s.at(0x1A)),
                d.u8(s.at(0x1C)),
                d.u8(s.at(0x1E)),
            ];
            b.x24 = d.u8(s.at(0x19));
            b.x25 = d.u8(s.at(0x1B));
            b.x26 = d.u8(s.at(0x1D));
            b.x27 = d.u8(s.at(0x1F));
            b.x28 = d.u8(s.at(0x20));
            for k in 0..4 {
                b.slots[k].x52 = 9;
                b.slots[k].x32 = 0;
            }
            for k in 0..4 {
                b.slots[k].x30 = d.u8(s.at(0x21 + k as u32));
            }
        }
        if region == 0 {
            self.tou_bracket_byes(count);
        }
    }

    /// `gmTournament_SetBracketByes`: the byes' lines in the small brackets.
    fn tou_bracket_byes(&mut self, count: i32) {
        let b = &mut self.game.tou.bracket;
        match count {
            1 => b[5].slots[1].x32 = 1,
            3 => {
                b[10].slots[0].x32 = 1;
                b[11].slots[1].x32 = 1;
            }
            5 => b[23].slots[1].x32 = 1,
            7 => {
                b[46].slots[1].x32 = 1;
                b[47].slots[1].x32 = 1;
            }
            _ => {}
        }
    }

    /// `fn_8018A970`: a GObj for each box in the bracket, drawing its lines, the first one's
    /// with the bracket's process.
    #[doc(alias = "fn_8018A970")]
    pub fn tou_bracket_gobjs(&mut self, count: i32) {
        for i in 0..64 {
            if self.game.tou.bracket[i].x0 == 0 {
                continue;
            }
            let w = &mut self.game.world;
            let g = w.create(0xE, 0x1B, 0);
            w.gobj_mut(g).user_data = Some(UserData::TouBox(i as u8));
            if i == 0 {
                w.setup_proc(g, CodeRef(BRACKET_PROC), 0);
            }
            w.setup_gx_link(g, CodeRef(BRACKET_RENDER), 4, 1);
        }
        if count < 9 {
            self.tou_bracket_byes(count);
        }
    }

    /// `fn_8018AA74`: where player `slot`'s icon of box `i` sits (its corners in the box's
    /// rectangle by the box's kind and seats), and the icon moved there.
    #[doc(alias = "fn_8018AA74")]
    pub fn tou_place_icon(&mut self, j: JObjId, i: usize, slot: usize) {
        let entrants = self.game.tou.tm.x2e;
        let b = self.game.tou.bracket[i];
        let s = &mut self.game.tou.bracket[i].slots[slot];
        if b.x1 != 0 {
            let x3 = b.x3;
            if x3 == 0 && b.x4 != 0 {
                let si = slot as i32;
                let (x, y, y40) = match b.x4 {
                    1 => {
                        let y = b
                            .x10
                            .wrapping_add(b.x18)
                            .wrapping_sub(si.wrapping_mul(b.x18));
                        (b.xc, y, b.x10.wrapping_add(b.x18 / 2))
                    }
                    2 => match slot {
                        0 => (
                            b.xc.wrapping_add(0x2B),
                            b.x10.wrapping_add(b.x18),
                            b.x10.wrapping_add(b.x18 / 2),
                        ),
                        1 => (
                            b.xc.wrapping_add(b.x14),
                            b.x10.wrapping_add(b.x18),
                            b.x10.wrapping_add(b.x18 / 2),
                        ),
                        _ => (
                            b.xc.wrapping_add(b.x14 / 2),
                            b.x10,
                            b.x10.wrapping_add(b.x18 / 2),
                        ),
                    },
                    3 => match slot {
                        0 => (
                            b.xc,
                            b.x10.wrapping_add(b.x18),
                            b.x10.wrapping_add(b.x18).wrapping_sub(b.x18 / 3),
                        ),
                        1 => (
                            b.xc.wrapping_add(b.x14),
                            b.x10.wrapping_add(b.x18),
                            b.x10.wrapping_add(b.x18).wrapping_sub(b.x18 / 3),
                        ),
                        2 => (b.xc, b.x10, b.x10.wrapping_add(b.x18 / 3)),
                        _ => (
                            b.xc.wrapping_add(b.x14),
                            b.x10,
                            b.x10.wrapping_add(b.x18 / 3),
                        ),
                    },
                    _ => {
                        // Kinds past 3 write nothing.
                        (s.x34, s.x38, s.x40)
                    }
                };
                if b.x4 <= 3 {
                    s.x3c = x;
                    s.x44 = x;
                    s.x34 = x;
                    s.x48 = y;
                    s.x38 = y;
                    s.x40 = y40;
                }
            } else {
                let si = slot as i32;
                let x =
                    b.xc.wrapping_add(si.wrapping_mul(divw(b.x14, i32::from(x3))));
                s.x3c = x;
                s.x44 = x;
                s.x34 = x;
                let y = b
                    .x10
                    .wrapping_add(b.x18)
                    .wrapping_sub(b.x18.wrapping_mul(i32::from(b.x2)));
                s.x48 = y;
                s.x38 = y;
                s.x40 = b.x10.wrapping_add(b.x18.wrapping_mul(i32::from(b.x2)));
                if x3 == 1 {
                    let shift = match (entrants, i, slot) {
                        (6, 5, 1) => 0x46,
                        (0xC, 0xA, 0) | (0xC, 0xB, 1) => 0x3C,
                        (0x18, 0x17, 1) => 0x28,
                        (0x30, 0x2E, 1) => 0x1E,
                        (0x30, 0x2F, 1) => -0x1E,
                        _ => 0,
                    };
                    if shift != 0 {
                        let t = s.x48.wrapping_add(shift);
                        s.x48 = t;
                        s.x38 = t;
                    }
                }
            }
        }
        let (x44, x48) = (s.x44, s.x48);
        self.tou_set_tx(Some(j), mem(f(x44)));
        self.tou_set_ty(Some(j), mem(fp::fneg(f(x48))));
    }

    /// `HSD_JObjSetTranslateX`.
    pub(crate) fn tou_set_tx(&mut self, j: Option<JObjId>, v: f32) {
        super::super::super::mn::common::set_tx(self, j, v);
    }

    /// `HSD_JObjSetTranslateY`.
    pub(crate) fn tou_set_ty(&mut self, j: Option<JObjId>, v: f32) {
        super::super::super::mn::common::set_ty(self, j, v);
    }

    /// `HSD_JObjSetTranslateZ`.
    pub(crate) fn tou_set_tz(&mut self, j: Option<JObjId>, v: f32) {
        let j = j.expect("the original's assertion failed: jobj");
        jobj::set_srt(&mut self.game.objs.jobjs, j, |o| o.translate.z = v);
    }

    /// A GObj's joint tree.
    pub(crate) fn tou_jobj(&self, g: GObjId) -> JObjId {
        super::super::super::mn::common::jobj_of(self, g)
    }

    /// A box's player's icon's joint tree.
    fn tou_icon(&self, i: usize, slot: usize) -> JObjId {
        let g = self.game.tou.bracket[i].slots[slot]
            .x2c
            .expect("a bracket icon's GObj");
        self.tou_jobj(g)
    }

    /// `fn_8018B090`, the bracket's process: the boxes of the match being played sliding
    /// and bouncing as it comes up, the camera moving in on it and back out, and its
    /// winner's icon moving along the line to the next box.
    #[doc(alias = "fn_8018B090")]
    pub fn tou_bracket_proc(&mut self) {
        let idx = self.tou_current_box();
        let cur = self.game.tou.tm.cur_option;
        match cur {
            20 => {
                self.game.tou.slide = 0;
            }
            21 => {
                let b = self.game.tou.bracket[idx];
                if b.x18 != 0 {
                    for i in 0..4 {
                        if b.slots[i].x30 == 0 {
                            continue;
                        }
                        let j = self.tou_icon(idx, i);
                        let n = reg(0.3);
                        let c = f(self.game.tou.slide);
                        let x48 = f(b.slots[i].x48);
                        let fwd = fp::fnmadds(n, c, x48);
                        let rev = fp::fmsubs(n, c, x48);
                        let y = if b.x2 != 0 {
                            rev
                        } else {
                            match b.x4 {
                                1 => {
                                    if i == 0 {
                                        fwd
                                    } else {
                                        rev
                                    }
                                }
                                2 | 3 => {
                                    if i <= 1 {
                                        fwd
                                    } else {
                                        rev
                                    }
                                }
                                _ => fwd,
                            }
                        };
                        self.tou_set_ty(Some(j), mem(y));
                    }
                }
                self.game.tou.slide = self.game.tou.slide.wrapping_add(1);
            }
            22 | 23 => {
                if cur == 22 {
                    let b = self.game.tou.bracket[idx];
                    let h = b.x18;
                    if h != 0 {
                        let c = &mut self.game.tou.cam;
                        c.current = Vec3::new(320.0, -240.0, 0.0);
                        c.target.x = mem(f(b.xc.wrapping_add(b.x14 / 2)));
                        c.target.y = mem(fp::fneg(f(b.x10.wrapping_add(h / 2))));
                        c.target.z = -150.0;
                        let k = reg(0.033333);
                        let abs = |d: f64| if d < 0.0 { fp::fneg(d) } else { d };
                        let d = fp::fmuls(k, fp::fsubs(reg(c.target.x), reg(c.current.x)));
                        c.step.x = mem(fp::fadds(1.0, abs(d)));
                        let d = fp::fmuls(k, fp::fsubs(reg(c.target.y), reg(c.current.y)));
                        c.step.y = mem(fp::fadds(1.0, abs(d)));
                        let d = fp::fmuls(k, fp::fsubs(reg(c.target.z), reg(c.current.z)));
                        c.step.z = mem(fp::fadds(1.0, abs(d)));
                    }
                    self.game.tou.slide = 0;
                }
                let b = self.game.tou.bracket[idx];
                if b.x18 == 0 {
                    return;
                }
                let amount = self.game.tou.slide / 6;
                self.game.tou.slide = self.game.tou.slide.wrapping_add(1);
                let c = &mut self.game.tou.cam;
                // Each axis stepped twice where the original tests its first step's way.
                for _ in 0..2 {
                    step_towards(&mut c.current.x, c.target.x, c.step.x);
                }
                for _ in 0..2 {
                    step_towards(&mut c.current.y, c.target.y, c.step.y);
                }
                for _ in 0..2 {
                    step_towards(&mut c.current.z, c.target.z, c.step.z);
                }
                let cur = c.current;
                self.tou_camera_to(reg(cur.x), reg(cur.y), reg(cur.z));
                if b.x4 != 1 {
                    for k in 0..4 {
                        if b.slots[k].x30 == 0 {
                            continue;
                        }
                        let s = &mut self.game.tou.bracket[idx].slots[k];
                        let target = s.x40;
                        step_towards_int(&mut s.x48, target, amount);
                        let y = mem(fp::fneg(f(s.x48)));
                        let j = self.tou_icon(idx, k);
                        self.tou_set_ty(Some(j), y);
                    }
                    return;
                }
                for i in 0..4 {
                    let s = &mut self.game.tou.bracket[idx].slots[i];
                    if s.x30 != 0 {
                        s.x40 = s.x48;
                    }
                }
            }
            24 => {
                let b = self.game.tou.bracket[idx];
                if b.x18 == 0 {
                    return;
                }
                self.game.tou.slide = self.game.tou.slide.wrapping_add(1);
                let bounce = f(BOUNCE[(self.game.tou.slide % 10) as usize]);
                for k in 0..4 {
                    if b.slots[k].x30 == 0 {
                        continue;
                    }
                    let j = self.tou_icon(idx, k);
                    let x40 = f(b.slots[k].x40);
                    let n = reg(0.1);
                    let up = fp::fnmadds(n, bounce, x40);
                    let down = fp::fmsubs(n, bounce, x40);
                    let y = if b.x2 != 0 {
                        down
                    } else {
                        match b.x4 {
                            1 => {
                                if k == 0 {
                                    up
                                } else {
                                    down
                                }
                            }
                            2 | 3 => {
                                if k <= 1 {
                                    up
                                } else {
                                    down
                                }
                            }
                            _ => up,
                        }
                    };
                    self.tou_set_ty(Some(j), mem(y));
                }
            }
            25 => {
                for s in &mut self.game.tou.bracket[idx].slots {
                    if s.x30 != 0 {
                        s.x48 = s.x40;
                    }
                }
            }
            32 => {
                let b = self.game.tou.bracket[idx];
                if b.x18 != 0 {
                    self.tou_camera_to(
                        f(b.xc.wrapping_add(b.x14 / 2)),
                        fp::fneg(f(b.x10.wrapping_add(b.x18 / 2))),
                        -150.0,
                    );
                    for s in &mut self.game.tou.bracket[idx].slots {
                        if s.x4c == 0 {
                            s.x3c = b.xc.wrapping_add(b.x14 / 2);
                        } else {
                            std::mem::swap(&mut s.x40, &mut s.x38);
                        }
                    }
                } else {
                    self.tou_camera_rest();
                }
                self.game.tou.tm.cur_option = 0x21;
            }
            33 => {
                let b = self.game.tou.bracket[idx];
                if b.x18 == 0 {
                    self.game.tou.tm.cur_option = 0x22;
                    return;
                }
                let mut done = 0;
                for i in 0..4 {
                    let s = self.game.tou.bracket[idx].slots[i];
                    if s.x30 == 0 {
                        done += 1;
                        continue;
                    }
                    let t5 = s.x4c;
                    if t5 == 0 {
                        self.game.tou.seat = i as i32;
                        if b.x4 != 1 {
                            let s = &mut self.game.tou.bracket[idx].slots[i];
                            let target = s.x3c;
                            step_towards_int(&mut s.x44, target, 2);
                            if s.x44 == s.x3c {
                                done += 1;
                            }
                            let x = mem(f(s.x44));
                            let j = self.tou_icon(idx, i);
                            self.tou_set_tx(Some(j), x);
                        } else {
                            done += 1;
                        }
                    } else {
                        let s = &mut self.game.tou.bracket[idx].slots[i];
                        let target = s.x40;
                        let t5 = i32::from(t5);
                        step_towards_int(&mut s.x48, target, t5 + (t5 + 1) / 2);
                        if s.x48 == s.x40 {
                            done += 1;
                        }
                        let y = mem(fp::fneg(f(s.x48)));
                        let j = self.tou_icon(idx, i);
                        self.tou_set_ty(Some(j), y);
                    }
                }
                if done == 4 {
                    let e = self.game.tou.tm.entrants() as usize;
                    let seat = self.game.tou.seat as usize;
                    let s = &mut self.game.tou.bracket[idx].slots[seat];
                    s.x3c = i32::from(WINNER_AT[e][0]);
                    s.x40 = i32::from(WINNER_AT[e][1]);
                    self.game.tou.tm.cur_option = 0x22;
                }
            }
            34 | 35 => {
                let b = self.game.tou.bracket[idx];
                if b.x18 == 0 || self.game.tou.tm.x33 != 5 {
                    self.game.tou.slide = 0;
                    self.game.tou.tm.cur_option = 0x24;
                    return;
                }
                let seat = self.game.tou.seat as usize;
                let j = self.tou_icon(idx, seat);
                let s = &mut self.game.tou.bracket[idx].slots[seat];
                if cur == 34 {
                    let target = s.x3c;
                    step_towards_int(&mut s.x44, target, 2);
                    let (x, done) = (s.x44, s.x44 == s.x3c);
                    self.tou_set_tx(Some(j), mem(f(x)));
                    if done {
                        self.game.tou.tm.cur_option = 0x23;
                    }
                } else {
                    let target = s.x40;
                    step_towards_int(&mut s.x48, target, 2);
                    let (y, done) = (s.x48, s.x48 == s.x40);
                    self.tou_set_ty(Some(j), mem(fp::fneg(f(y))));
                    if done {
                        self.game.tou.tm.x33 = 6;
                    }
                }
            }
            36 => {
                let b = self.game.tou.bracket[idx];
                let h = b.x18;
                if h != 0 {
                    let c = &mut self.game.tou.cam;
                    c.current.x = mem(f(b.xc.wrapping_add(b.x14 / 2)));
                    c.current.y = mem(fp::fneg(f(b.x10.wrapping_add(h / 2))));
                    c.current.z = -150.0;
                    c.target = Vec3::new(320.0, -240.0, 0.0);
                    let k = reg(0.04);
                    let abs = |d: f64| if d < 0.0 { fp::fneg(d) } else { d };
                    c.step.x = mem(abs(fp::fmuls(
                        k,
                        fp::fsubs(reg(c.target.x), reg(c.current.x)),
                    )));
                    c.step.y = mem(abs(fp::fmuls(
                        k,
                        fp::fsubs(reg(c.target.y), reg(c.current.y)),
                    )));
                    c.step.z = mem(abs(fp::fmuls(
                        k,
                        fp::fsubs(reg(c.target.z), reg(c.current.z)),
                    )));
                }
                self.game.tou.tm.cur_option = 0x25;
            }
            37 => {
                if self.game.tou.bracket[idx].x18 == 0 {
                    self.game.tou.slide = 0x78;
                }
                if self.game.tou.slide < 0x78 {
                    self.game.tou.slide += 1;
                    let c = &mut self.game.tou.cam;
                    step_towards(&mut c.current.x, c.target.x, c.step.x);
                    step_towards(&mut c.current.y, c.target.y, c.step.y);
                    step_towards(&mut c.current.z, c.target.z, c.step.z);
                    let cur = c.current;
                    self.tou_camera_to(reg(cur.x), reg(cur.y), reg(cur.z));
                    return;
                }
                self.game.tou.slide = 0;
                self.tou_camera_rest();
                self.game.tou.tm.cur_option = 0x26;
            }
            38 => {
                if self.game.tou.bracket[idx].x18 == 0 {
                    self.game.tou.slide = 0x78;
                }
                if self.game.tou.slide < 0x78 {
                    self.game.tou.slide += 1;
                    return;
                }
                let b = &self.game.tou.bracket[idx];
                let seat = (0..4).find(|&i| b.slots[i].x4c == 0).unwrap_or(4);
                let src_i = self.tou_current_box();
                self.tou_winner_moves_on(src_i, seat);
                self.game.tou.tm.cur_option = 0x27;
            }
            _ => {}
        }
    }

    /// `fn_8018B090_inline5`: the winner of box `i` (seat `seat`) moved into the next box's
    /// seat, box `i` no longer the match being played.
    fn tou_winner_moves_on(&mut self, i: usize, seat: usize) {
        let b = self.game.tou.bracket[i];
        self.game.tou.bracket[i].x1 = 0;
        let (ne, ns) = (usize::from(b.x5), usize::from(b.x6));
        let from = b.slots[seat];
        let to = &mut self.game.tou.bracket[ne].slots[ns];
        to.x30 = 1;
        to.x50 = from.x50;
        to.x51 = from.x51;
        to.x52 = from.x52;
        to.x4d = from.x4d;
        to.x4e = from.x4e;
        to.x4f = from.x4f;
        let s = &mut self.game.tou.bracket[i].slots[seat];
        s.x30 = 0;
        s.x4e = 3;
    }

    /// `fn_8018E46C`, a box's GObj's render callback: no fog, the debug drawing set up
    /// (`hsd_80391A04(1, 1, 1)`), and the box's lines by its kind.
    #[doc(alias = "fn_8018E46C")]
    pub fn tou_box_render(&mut self, gobj: GObjId) {
        let Some(UserData::TouBox(i)) = self.game.world.gobj(gobj).user_data else {
            panic!("a bracket box's GObj without its box");
        };
        let b = self.game.tou.bracket[usize::from(i)];
        let r30 = if b.x2 != 0 {
            b.x10.wrapping_add(b.x18).wrapping_neg()
        } else {
            b.x10.wrapping_neg()
        };
        crate::hsd::display::fog_set(&mut self.game.objs, None);
        let c = self
            .game
            .objs
            .cobjs
            .current
            .expect("hsd_80391A04: no camera");
        cobj::viewing_mtx_copy(&mut self.game.objs, self.data, c);
        let x = b.xc.wrapping_sub(fp::fctiwz(fp::fmuls(0.5, reg(b.x1c))));
        let (y, w, h) = (b.x10.wrapping_neg(), b.x14, b.x18.wrapping_neg());
        let entrants = self.game.tou.tm.entrants();
        let mut q = Vec::new();
        match b.x3 {
            0 => lines_0(&b, x, y, w, h, &mut q),
            1 => lines_1(&b, entrants, x, y, w, h, r30, &mut q),
            2 => lines_2(&b, x, y, w, h, r30, &mut q),
            3 => lines_3(&b, x, y, w, h, r30, &mut q),
            _ => {}
        }
        let draws = &mut self.game.objs.display.draws;
        draws.extend(q.into_iter().map(Draw::Quad));
    }

    /// `fn_8018E618`: the bracket's GObjs and camera made anew (every GObj of process links
    /// 27 and 20 freed), the boxes loaded first if `reset`, and their GObjs.
    #[doc(alias = "fn_8018E618")]
    #[doc(alias = "gmTournament_InitBracket")]
    pub fn tou_bracket_make(&mut self, count: i32, thickness: f32, reset: bool) {
        while let Some(g) = self.game.world.first_in(27) {
            gobj::free(self, g);
        }
        while let Some(g) = self.game.world.first_in(20) {
            gobj::free(self, g);
        }
        if reset {
            for b in &mut self.game.tou.bracket {
                b.x0 = 0;
                for s in &mut b.slots {
                    s.x4e = 3;
                }
            }
        }
        let data = self.data;
        let g = &mut *self.game;
        let cam = g.world.create(9, 20, 1);
        let c = cobj::load_static_desc(&mut g.objs, data, &BRACKET_CAMERA);
        g.tou.cobj = Some(c);
        g.world.set_obj(cam, OBJ_CAMERA, Obj::CObj(c));
        g.world
            .setup_gx_link_max(cam, CodeRef(CAMERA_CALLBACK), 1, false);
        g.world.gobj_mut(cam).gxlink_prios = 0x10;
        if reset {
            self.tou_bracket_load(count, thickness);
        }
        self.tou_bracket_gobjs(count);
    }

    /// `fn_8018E85C`: an icon for each player in the bracket's boxes (`model`, at its
    /// character's and color's frame, larger in the box being played and in smaller
    /// brackets), placed by [`Running::tou_place_icon`] before the matches start and where
    /// they were after; with `fill`, each box's players taken from the entrants in seat
    /// order.
    #[doc(alias = "fn_8018E85C")]
    pub fn tou_bracket_icons(&mut self, model: DataRef, fill: bool) {
        let mut seat = 0u8;
        for i in 0..64 {
            if self.game.tou.bracket[i].x0 == 0 {
                continue;
            }
            for k in 0..4 {
                if self.game.tou.bracket[i].slots[k].x30 == 0 {
                    continue;
                }
                if fill {
                    let tm = &self.game.tou.tm;
                    let j = (0..64).find(|&j| tm.x37[j].xd == seat).unwrap_or(64);
                    let e = tm.x37.get(j).copied().unwrap_or_else(|| {
                        super::super::super::code::unported_call(
                            "fn_8018E85C past the entrants (no entrant at a bracket seat)",
                        )
                    });
                    let s = &mut self.game.tou.bracket[i].slots[k];
                    s.x50 = j as u8;
                    s.x4d = e.x3;
                    s.x4e = e.x0;
                    s.x4f = e.x7;
                    s.x51 = e.x1;
                    s.x52 = e.x2;
                    s.x54 = e.x9;
                    seat = seat.wrapping_add(1);
                }
                let data = self.data;
                let g = &mut *self.game;
                let gobj = g.world.create(0xE, 0x1B, 0);
                g.tou.bracket[i].slots[k].x2c = Some(gobj);
                let j = jobj::load_joint(&mut g.objs, data, data.ptr(model)).expect("an icon");
                g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
                g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 4, 2);
                super::super::vs::add_model_anim(&mut g.objs, data, Some(j), model, 0);
                let s = g.tou.bracket[i].slots[k];
                let frame = i32::from(s.x4d).wrapping_add(i32::from(s.x4f) * 0x1E);
                jobj::req_anim_all(&mut g.objs, Some(j), mem(f(frame)));
                super::super::super::mn::common::anim_all(self, Some(j));
                let mult = if i == self.tou_current_box() {
                    10.0
                } else {
                    7.0
                };
                let n = f(64 - i32::from(self.game.tou.tm.x2e));
                let pos = fp::fmadds(reg(0.0083), n, 1.0);
                let scale = mem(fp::fmuls(mult, pos));
                jobj::set_srt(&mut self.game.objs.jobjs, j, |o| o.scale.x = scale);
                let pos = fp::fmadds(reg(0.0083), n, 1.0);
                let scale = mem(fp::fmuls(mult, pos));
                jobj::set_srt(&mut self.game.objs.jobjs, j, |o| o.scale.y = scale);
                if self.game.tou.tm.cur_option < 0x1F {
                    self.tou_place_icon(j, i, k);
                } else {
                    let s = self.game.tou.bracket[i].slots[k];
                    self.tou_move(j, f(s.x44), fp::fneg(f(s.x48)), 666.0);
                }
            }
        }
    }

    /// `fn_8018EC48`: the GObjs of process links 25 to 28 and 18 to 21 freed
    /// (`mn_8022F138`).
    #[doc(alias = "fn_8018EC48")]
    pub fn tou_free_scene(&mut self) {
        self.free_gobj_links(0x19, 0x1C);
        self.free_gobj_links(0x12, 0x15);
    }

    /// `fn_8018EC7C`: the bracket's GObjs (process links 27 and 20) freed (`mn_8022F0F0`).
    #[doc(alias = "fn_8018EC7C")]
    pub fn tou_free_bracket(&mut self) {
        self.free_gobj_links(0x1B, 0x1B);
        self.free_gobj_links(0x14, 0x14);
    }

    /// `fn_8018ECA8`: a name added at (`x`, `y`) to text `x518[a]` (an entrant's name tag,
    /// or "HMN n" for `0x320` up and "CPU n" for `0x384` up in text `x518[b]`).
    #[doc(alias = "fn_8018ECA8")]
    pub fn tou_add_name(&mut self, name: i32, kind: i32, a: usize, x: f64, y: f64, b: usize) {
        let us = self.game.gm.unlocks.is_us();
        let text = |r: &Running, k: usize| -> TextId {
            r.game.tou.tm.x518[k].expect("a tournament name's text")
        };
        if kind == 0xFF {
            let s = self.game.name_text(name as u8);
            let t = text(self, a);
            self.game.sis.add_string(t, x, y, s.as_deref());
            return;
        }
        if name >= 0x320 {
            if name < 0x384 {
                let s = numbered(if us { &GUEST_US } else { &GUEST_JP }, name - 0x320);
                let t = text(self, a);
                self.game.sis.add_string(t, x, y, Some(&s));
                return;
            }
            if name >= 0x3E7 {
                return;
            }
            let s = numbered(&CPU, name - 0x384);
            let t = text(self, b);
            self.game.sis.add_string(t, x, y, Some(&s));
            return;
        }
        let k = match kind {
            0 => a,
            1 => b,
            _ => return,
        };
        let s = self.game.name_text(name as u8);
        let t = text(self, k);
        self.game.sis.add_string(t, x, y, s.as_deref());
    }

    /// `fn_8018F00C`: an entrant's name as the bracket shows it, at most 19 bytes.
    #[doc(alias = "fn_8018F00C")]
    pub fn tou_name(&mut self, name: i32) -> Vec<u8> {
        let us = self.game.gm.unlocks.is_us();
        if name >= 0x320 {
            if name < 0x384 {
                return numbered(if us { &GUEST_US } else { &GUEST_JP }, name - 0x320);
            }
            if name != 0x3E7 {
                return numbered(&CPU, name - 0x384);
            }
            // The buffer keeps what it held: the caller's cleared one.
            return Vec::new();
        }
        self.game.name_text(name as u8).unwrap_or_else(|| {
            super::super::super::code::unported_call(
                "fn_8018F00C on an empty name tag (strcpy from NULL)",
            )
        })
    }

    /// `fn_8018F410`: a random unlocked character's icon, none of the last three.
    #[doc(alias = "fn_8018F410")]
    pub fn tou_random_icon(&mut self) -> i32 {
        let t = &self.game.tou;
        let mut c;
        loop {
            c = self.game.objs.rng.below(0x19);
            let t = &self.game.tou;
            if t.unlocked[c as usize] != 0
                && t.recent[0] != c
                && t.recent[1] != c
                && t.recent[2] != c
            {
                break;
            }
        }
        let _ = t;
        let r = &mut self.game.tou.recent;
        r[2] = r[1];
        r[1] = r[0];
        r[0] = c;
        c
    }

    /// `fn_8018F4A0`: a random stage (`mnSelStageRandom`), which must be unlocked.
    #[doc(alias = "fn_8018F4A0")]
    pub fn tou_random_stage(&mut self) -> i32 {
        let s = self.random_stage();
        assert!(
            self.stage_unlocked(u16::from(s)),
            "This is impossible stage num from mnSelStageRandom() -> stage {s}"
        );
        i32::from(s)
    }

    /// `fn_8018F508`: how many players are left in the box being played (-1 for no box),
    /// and the last one's seat.
    #[doc(alias = "fn_8018F508")]
    pub fn tou_players_left(&self) -> (i32, Option<i32>) {
        let i = self.tou_current_box();
        let b = &self.game.tou.bracket[i.min(63)];
        if i >= 64 || b.x0 == 0 {
            if i >= 64 {
                super::super::super::code::unported_call("fn_8018F508 past the bracket");
            }
            return (-1, None);
        }
        let mut count = 0;
        let mut last = None;
        for (k, s) in b.slots.iter().enumerate() {
            if s.x4e != 3 {
                last = Some(k as i32);
                count += 1;
            }
        }
        (count, last)
    }

    /// `fn_8018F640`: the buttons a port pressed this frame (the merged ports for 4 up).
    #[doc(alias = "fn_8018F640")]
    pub fn tou_triggered(&self, port: i32) -> u32 {
        self.game.buttons_triggered(port.min(4) as u8) as u32
    }

    /// `fn_8018F674`: the buttons a port repeats.
    #[doc(alias = "fn_8018F674")]
    pub fn tou_repeated(&self, port: i32) -> u32 {
        self.game.buttons_repeated(port.min(4) as u8) as u32
    }

    /// `fn_8018F6A8`: the buttons a port holds.
    #[doc(alias = "fn_8018F6A8")]
    pub fn tou_held(&self, port: i32) -> u32 {
        self.game.buttons_pressed(port.min(4) as u8) as u32
    }

    /// `fn_8018F74C`: the box being played (64 for none).
    #[doc(alias = "fn_8018F74C")]
    #[doc(alias = "fn_8018F888_inline0")]
    pub fn tou_current_box(&self) -> usize {
        self.game
            .tou
            .bracket
            .iter()
            .position(|b| b.x1 != 0)
            .unwrap_or(64)
    }

    /// `fn_8018F808`: the controllers plugged in (`HSD_PadMasterStatus[i].err` clear).
    #[doc(alias = "fn_8018F808")]
    pub fn tou_pads(&self) -> i32 {
        (0..4).filter(|&i| self.tou_pad_err(i) == 0).count() as i32
    }

    /// `HSD_PadMasterStatus[port].err`.
    pub fn tou_pad_err(&self, port: usize) -> i8 {
        self.game.pads.master[port].err
    }

    /// `fn_8018F888`: the box being played (and, once the tournament's over, the next) drawn
    /// in its color.
    #[doc(alias = "fn_8018F888")]
    pub fn tou_light_box(&mut self) {
        let i = self.tou_current_box();
        self.game.tou.bracket[i].x20[1] = 0;
        if self.game.tou.tm.x33 != 5 {
            return;
        }
        let i = self.tou_current_box();
        self.game.tou.bracket[i + 1].x20[1] = 0;
    }

    /// `fn_8018FA24`: the next match's players from the box being played's seats, each
    /// one's character set for the player library, and how many there are.
    #[doc(alias = "fn_8018FA24")]
    pub fn tou_match_players(&mut self) {
        let i = self.tou_current_box();
        let mut count = 0u8;
        for p in 0..4 {
            let s = self.game.tou.bracket[i].slots[p];
            let tm = &mut self.game.tou.tm;
            if p == 0 {
                tm.x4b6 = s.x30;
                tm.x4b7 = s.x50;
            } else {
                tm.x4b8[p - 1].x8 = s.x30;
                tm.x4b8[p - 1].x9 = s.x50;
            }
            let name = tm.x37[usize::from(s.x50)].x9;
            let m = &mut tm.x4b8[p];
            m.x6 = name;
            m.x4 = s.x51;
            m.x5 = s.x52;
            m.x1 = s.x4d;
            let ckind = ckind_of(i32::from(m.x1));
            m.x0 = s.x4e;
            if m.x0 != 3 {
                count += 1;
            }
            let (slot_type, color) = (m.x0, s.x4f);
            m.x3 = color;
            m.x4 = s.x51;
            self.game.set_player_ckind(p as i32, ckind);
            self.game.set_player_pkind(p as i32, i32::from(slot_type));
            self.game.set_player_costume(p as i32, i32::from(color));
        }
        self.game.tou.tm.x30 = count;
    }

    /// `fn_8018FBD8`: a GObj's user data set to a number.
    #[doc(alias = "fn_8018FBD8")]
    #[doc(alias = "fn_8018F62C")]
    pub fn tou_set_index(&mut self, g: GObjId, i: i32) {
        self.game.world.gobj_mut(g).user_data = Some(UserData::TouIndex(i));
    }

    /// `fn_8018F62C`: the number a GObj's user data holds.
    pub fn tou_index(&self, g: GObjId) -> i32 {
        match self.game.world.gobj(g).user_data {
            Some(UserData::TouIndex(i)) => i,
            other => panic!("a tournament GObj's number: {other:?}"),
        }
    }

    /// `fn_8018FBE0`: the option, `x1C` and `x20` set, and every entrant's handicap, level,
    /// seat, name and slot type.
    #[doc(alias = "fn_8018FBE0")]
    #[allow(clippy::too_many_arguments)]
    pub fn tou_reset_entrants(
        &mut self,
        opt: i32,
        x1c: i32,
        x20: i32,
        a3: u8,
        a4: u8,
        a5: u16,
        a6: u8,
    ) {
        let tm = &mut self.game.tou.tm;
        tm.cur_option = opt;
        tm.x1c = x1c;
        tm.x20 = x20;
        for (i, e) in tm.x37.iter_mut().enumerate() {
            e.x2 = a3;
            e.x1 = a4;
            e.xd = i as u8;
            e.x9 = a5;
            e.x0 = a6;
        }
    }

    /// `fn_8018FDC4`: a joint moved, each coordinate but those at 666.
    #[doc(alias = "fn_8018FDC4")]
    pub fn tou_move(&mut self, j: JObjId, x: f64, y: f64, z: f64) {
        if fp::fctiwz(x) != 666 {
            self.tou_set_tx(Some(j), mem(x));
        }
        if fp::fctiwz(y) != 666 {
            self.tou_set_ty(Some(j), mem(y));
        }
        if fp::fctiwz(z) != 666 {
            self.tou_set_tz(Some(j), mem(z));
        }
    }

    /// `fn_8018FF9C`: a joint scaled, each axis but those at 666.
    #[doc(alias = "fn_8018FF9C")]
    pub fn tou_scale(&mut self, j: JObjId, x: f64, y: f64, z: f64) {
        let js = &mut self.game.objs.jobjs;
        if fp::fctiwz(x) != 666 {
            jobj::set_srt(js, j, |o| o.scale.x = mem(x));
        }
        if fp::fctiwz(y) != 666 {
            jobj::set_srt(js, j, |o| o.scale.y = mem(y));
        }
        if fp::fctiwz(z) != 666 {
            jobj::set_srt(js, j, |o| o.scale.z = mem(z));
        }
    }

    /// `fn_80190174`: the scene's camera (class 19, process link 18) drawing render links
    /// 0 to 2.
    #[doc(alias = "fn_80190174")]
    pub fn tou_camera(&mut self, scene: Option<DataRef>) -> GObjId {
        self.tou_scene_camera(scene, 0x12, 0, 7)
    }

    /// `fn_801901F8`: the scene's camera again (process link 21) drawing links 1 and 3.
    #[doc(alias = "fn_801901F8")]
    pub fn tou_camera2(&mut self, scene: Option<DataRef>) -> GObjId {
        self.tou_scene_camera(scene, 0x15, 2, 0xA)
    }

    fn tou_scene_camera(
        &mut self,
        scene: Option<DataRef>,
        p_link: u8,
        prio: u8,
        links: u64,
    ) -> GObjId {
        let data = self.data;
        let scene = scene.expect("a tournament scene");
        let g = &mut *self.game;
        let cam = g.world.create(0x13, p_link, prio);
        let cameras = data.ptr(scene.at(4)).expect("the scene's cameras");
        let c = cobj::load_desc(&mut g.objs, data, data.ptr(cameras)).expect("a tournament camera");
        g.world.set_obj(cam, OBJ_CAMERA, Obj::CObj(c));
        g.world
            .setup_gx_link_max(cam, CodeRef(CAMERA_CALLBACK), prio, false);
        g.world.gobj_mut(cam).gxlink_prios = links;
        cam
    }

    /// `fn_8019027C`: the scene's lights (class 11, process link 26).
    #[doc(alias = "fn_8019027C")]
    pub fn tou_lights(&mut self, scene: Option<DataRef>) {
        let data = self.data;
        let scene = scene.expect("a tournament scene");
        let g = &mut *self.game;
        let light = g.world.create(0xB, 0x1A, 0);
        let lights = data.ptr(scene.at(8)).expect("the scene's lights");
        let l = super::super::super::gr::setup::load_light_list(g, data, lights)
            .expect("a tournament light");
        g.world.set_obj(light, OBJ_LIGHT, Obj::LObj(l));
        g.world.setup_gx_link(light, CodeRef(LOBJ_CALLBACK), 1, 0);
    }

    /// `fn_801902F0`: the texts reset and their canvas made, drawn by `camera` unless
    /// `x1C` is clear.
    #[doc(alias = "fn_801902F0")]
    pub fn tou_canvas(&mut self, camera: Option<GObjId>) {
        self.sis_reset();
        let camera = if self.game.tou.tm.x1c == 0 {
            None
        } else {
            camera
        };
        self.game.tou.canvas = self.add_canvas(0, camera, 9, 0x12, 0, 3, 0, 0x13);
    }

    /// `fn_8019035C`: a model on a new GObj (class 14, process link `p_link`) drawn on
    /// render link `gx_link`, with process `proc`; with `animate`, its animation set
    /// `anim` at `frame`, and hidden if `hide`.
    #[doc(alias = "fn_8019035C")]
    #[allow(clippy::too_many_arguments)]
    pub fn tou_model(
        &mut self,
        hide: bool,
        model: DataRef,
        anim: u32,
        p_link: u8,
        gx_link: u8,
        animate: bool,
        proc: Option<u32>,
        frame: f32,
    ) -> GObjId {
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(0xE, p_link, 0);
        let j = jobj::load_joint(&mut g.objs, data, data.ptr(model)).expect("a tournament model");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world
            .setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), gx_link, 0);
        if let Some(p) = proc {
            g.world.setup_proc(gobj, CodeRef(p), 0);
        }
        if animate {
            super::super::vs::add_model_anim(&mut g.objs, data, Some(j), model, anim);
            jobj::req_anim_all(&mut g.objs, Some(j), frame);
            super::super::super::mn::common::anim_all(self, Some(j));
            if hide {
                super::super::super::mn::common::hide(self, Some(j));
            }
        }
        gobj
    }

    /// `fn_8019044C`: a joint tree at `frame`.
    #[doc(alias = "fn_8019044C")]
    pub fn tou_frame(&mut self, j: Option<JObjId>, frame: f32) {
        super::super::super::mn::common::set_frame_all(self, j, frame);
    }

    /// `fn_80190480`: the bracket camera's field of view.
    #[doc(alias = "fn_80190480")]
    pub fn tou_fov(&mut self, fov: f32) {
        let fov = if fp::fctiwz(reg(fov)) == 0 { 0.0 } else { fov };
        cobj::set_fov(&mut self.game.objs.cobjs, self.game.tou.cobj, fov);
    }

    /// `fn_801904D0`: the bracket camera at rest.
    #[doc(alias = "fn_801904D0")]
    pub fn tou_camera_rest(&mut self) {
        let c = self.game.tou.cobj.expect("the bracket's camera");
        cobj::set_interest(&mut self.game.objs, c, INTEREST);
        cobj::set_eye_position(&mut self.game.objs, c, EYE);
    }

    /// `fn_80190520`: the bracket camera looking at (`x`, `y`, `z`) from its eye distance in
    /// front (at rest for the origin).
    #[doc(alias = "fn_80190520")]
    pub fn tou_camera_to(&mut self, x: f64, y: f64, z: f64) {
        if fp::fctiwz(x) == 0 && fp::fctiwz(y) == 0 && fp::fctiwz(z) == 0 {
            self.tou_camera_rest();
            return;
        }
        let c = self.game.tou.cobj.expect("the bracket's camera");
        let mut p = Vec3::new(mem(x), mem(y), mem(z));
        cobj::set_interest(&mut self.game.objs, c, p);
        p.z = mem(fp::fadds(reg(super::EYE_Z), z));
        cobj::set_eye_position(&mut self.game.objs, c, p);
    }

    /// `gm_801905F0`, the match's start data: the announcer, the rules' defaults with the
    /// stage, the item switch, the menus' rules (no teams, the timer, stock, items, damage,
    /// the self-destruct penalty, pausing, the score), then each player of the match from
    /// `x4B8` (a random one's character drawn), and the preload cache's entries for them.
    #[doc(alias = "gm_801905F0")]
    #[doc(alias = "fn_801640B0")]
    #[doc(alias = "fn_801653E8")]
    #[doc(alias = "gmMainLib_8015ED30")]
    #[doc(alias = "fn_8016419C")]
    #[doc(alias = "fn_801641B4")]
    pub fn tou_setup_match(&mut self) {
        self.load_announcer();
        let rules = self.game.game_data.x1850.clone();
        let prefs_freq = self.game.game_data.thing.save_data.x1CB0.item_freq;
        let prefs_mask = self.game.game_data.thing.save_data.x1CB0.item_mask;
        let mut d = StartMeleeData {
            rules: super::super::vs_mode::rules_defaults(),
            ..StartMeleeData::default()
        };
        let r = &mut d.rules;
        r.is_teams = 0;
        r.stkind = self.game.tou.tm.x28 as u16;
        for (i, &kind) in super::super::vs_mode::ITEM_SWITCH_KINDS.iter().enumerate() {
            if kind != super::super::vs_mode::NO_ITEM_KIND {
                if prefs_mask & (1 << i) != 0 {
                    r.x20 |= 1 << kind;
                } else {
                    r.x20 &= !(1 << kind);
                }
            }
        }
        r.match_kind = rules.mode;
        r.timer_enabled = rules.mode != 1 || rules.stock_time_limit != 0;
        if r.timer_enabled {
            let m = if rules.mode != 1 {
                rules.time_limit
            } else {
                rules.stock_time_limit
            };
            r.time_limit = if m == 0 { 99 * 60 } else { u32::from(m) * 60 };
        }
        if r.match_kind == 1 {
            r.is_stock = true;
        }
        r.timer_counts_up = false;
        r.x4_2 = false;
        r.x4_4 = false;
        r.item_freq = prefs_freq as i8;
        r.x2_2 = false;
        r.x18 = 0;
        r.game_speed = 1.0;
        r.x30 = mem(fp::fmuls(reg(0.1), f(i32::from(rules.damage_ratio))));
        r.x3_4 = false;
        r.x3_5 = false;
        r.x3_2 = false;
        r.x3_3 = false;
        match rules.unk_xc {
            1 => r.sd_penalty = 0,
            0 => r.sd_penalty = -1,
            2 => r.sd_penalty = -2,
            _ => {}
        }
        r.disable_pausing = rules.pause == 0;
        r.x3_0 = rules.score_display != 0 && r.match_kind == 0;
        super::super::vs_mode::all_player_defaults(&mut d.players);
        for i in 0..4 {
            let tm = self.game.tou.tm.clone();
            if i < usize::from(tm.x30) {
                let m = tm.x4b8[i];
                let p = &mut d.players[i];
                p.model_scale = 1.0;
                p.nametag = m.x6.min(0x78) as u8;
                if m.x2 != 0 {
                    let icon = self.tou_random_icon();
                    let ckind = ckind_of(icon) as u8;
                    d.players[i].ckind = ckind as i8;
                    let n = super::super::super::playback::costume_count(ckind);
                    d.players[i].color = self.game.objs.rng.below(i32::from(n)) as u8;
                } else {
                    let p = &mut d.players[i];
                    p.ckind = ckind_of(i32::from(m.x1)) as i8;
                    p.color = m.x3;
                }
                let tag = d.players[i].nametag;
                let rumble = self.rumble_enabled_for(i, tag);
                let p = &mut d.players[i];
                p.slot_type = m.x0;
                p.stocks = rules.stock_count as i8;
                p.sub_color = 0;
                p.team = 0xFF;
                p.rumble_enabled = rumble && m.x0 != 1;
                p.cpu_kind = 4;
                p.cpu_level = m.x4;
                p.damage1 = 0;
                if self.game.game_data.x1850.handicap != 0 {
                    let (a, df) =
                        super::super::vs_mode::HANDICAP_RATIOS[usize::from(m.x5.wrapping_sub(1))];
                    let p = &mut d.players[i];
                    p.attack_ratio = a;
                    p.defense_ratio = df;
                } else {
                    let p = &mut d.players[i];
                    p.attack_ratio = 1.0;
                    p.defense_ratio = 1.0;
                }
            } else {
                d.players[i].slot_type = super::super::super::pl::player::PKIND_NONE as u8;
            }
        }
        let stkind = i32::from(d.rules.stkind);
        let players: Vec<(u8, i32, u8)> = (0..4)
            .map(|i| {
                let p = &d.players[i];
                (p.slot_type, i32::from(p.ckind), p.color)
            })
            .collect();
        *self.game.tou.start = d;
        self.tou_preload_match(stkind, &players, false);
    }

    /// `fn_8019EF08` and `fn_8019EE80`: the preload cache's players (each one there, or the
    /// match's first `x30`) and, unless the stage is chosen, its stage, then the cache's
    /// loads (`lbDvd_80018254`).
    #[doc(alias = "fn_8019EF08")]
    #[doc(alias = "fn_8019EE80")]
    pub fn tou_preload_match(&mut self, stkind: i32, players: &[(u8, i32, u8)], alt: bool) {
        let n = usize::from(self.game.tou.tm.x30);
        let chosen = self.tou_stage_chosen();
        for (i, &(slot_type, ckind, color)) in players.iter().enumerate().take(4) {
            let take = if alt {
                i < n
            } else {
                i32::from(slot_type) != super::super::super::pl::player::PKIND_NONE
            };
            if take {
                let gc = &mut self.dvd_mut().cache.scene.game_cache;
                gc.entries[i].char_id = ckind;
                gc.entries[i].color = color;
            }
        }
        if alt && !chosen {
            self.dvd_mut().cache.scene.game_cache.stkind = stkind;
        }
        self.preload_refresh();
    }

    /// `fn_80196594`: whether this match's stage is chosen in the stage select (chosen once,
    /// or every match).
    #[doc(alias = "fn_80196594")]
    pub fn tou_stage_chosen(&self) -> bool {
        let tm = &self.game.tou.tm;
        (tm.stage_selection_type() == 2 && tm.x32 == 0) || tm.stage_selection_type() == 3
    }

    /// `fn_80196564`: whether this match's stage is random (once, or every match).
    #[doc(alias = "fn_80196564")]
    pub fn tou_stage_random(&self) -> bool {
        let tm = &self.game.tou.tm;
        (tm.stage_selection_type() == 0 && tm.x32 == 0) || tm.stage_selection_type() == 1
    }

    /// Whether the tournament is the mode running.
    pub fn tou_running(&self) -> bool {
        self.game.machine.routing.curr_mode == GM_TOURNAMENT
    }
}

/// `DrawRectangle(x, y, w, h)`: a quad from (`x`, `y`) to (`x + w`, `y + h`).
fn rect(q: &mut Vec<Quad>, x: f64, y: f64, w: f64, h: f64, color: [u8; 4]) {
    q.push(Quad {
        x0: mem(x),
        y0: mem(y),
        x1: mem(fp::fadds(x, w)),
        y1: mem(fp::fadds(y, h)),
        color,
    });
}

/// `fn_8018C8D4`: the lines of a box of kind 0 (the first round's), by its seats' layout.
#[doc(alias = "fn_8018C8D4")]
fn lines_0(b: &super::BracketEntry, x: i32, y: i32, w: i32, h: i32, q: &mut Vec<Quad>) {
    let t = reg(b.x1c);
    let c = b.x20;
    let lit = c[1] == 0;
    match b.x4 {
        0 => {
            rect(q, f(x), f(y), t, f(h), YELLOW);
            if lit {
                rect(q, f(x), f(y), t, f(h), c);
            }
        }
        1 => {
            rect(q, f(x), f(y), t, f(h), YELLOW);
            if lit {
                if b.slots[0].x4c == 0 {
                    let half_h = h / 2;
                    rect(q, f(x), f(y.wrapping_add(half_h)), t, f(half_h), c);
                    return;
                }
                rect(q, f(x), f(y), t, f(h / 2), c);
            }
        }
        2 => {
            let half = w / 2;
            let center = x.wrapping_add(half);
            let half_h = h / 2;
            rect(q, f(center), f(y), t, f(half_h), YELLOW);
            let neg = fp::fneg(t);
            let mid_y = y.wrapping_add(half_h);
            rect(
                q,
                f(x.wrapping_add(0x2B)),
                f(mid_y),
                fp::fsubs(fp::fadds(f(w), t), 43.0),
                neg,
                YELLOW,
            );
            rect(q, f(x.wrapping_add(0x2B)), f(mid_y), t, f(half_h), YELLOW);
            let right = x.wrapping_add(w);
            rect(q, f(right), f(mid_y), t, f(half_h), YELLOW);
            if lit {
                if b.slots[0].x4c == 0 {
                    rect(q, f(x.wrapping_add(0x2B)), f(mid_y), t, f(half_h), c);
                    rect(
                        q,
                        f(x.wrapping_add(0x2B)),
                        f(mid_y),
                        f(half.wrapping_sub(0x2B)),
                        neg,
                        c,
                    );
                    return;
                }
                if b.slots[1].x4c == 0 {
                    rect(q, f(right), f(mid_y), t, f(half_h), c);
                    rect(q, f(center), f(mid_y), fp::fadds(f(half), t), neg, c);
                    return;
                }
                rect(q, f(center), f(y), t, f(half_h), c);
            }
        }
        3 => {
            let half = w / 2;
            let third_h = h / 3;
            rect(
                q,
                f(x.wrapping_add(half)),
                f(y.wrapping_add(third_h)),
                t,
                f(third_h.wrapping_sub(2)),
                YELLOW,
            );
            rect(q, f(x), f(y), t, f(third_h), YELLOW);
            let right = x.wrapping_add(w);
            rect(q, f(right), f(y), t, f(third_h), YELLOW);
            let bot_y = y.wrapping_add(h).wrapping_sub(third_h);
            rect(q, f(x), f(bot_y), t, f(third_h), YELLOW);
            rect(q, f(right), f(bot_y), t, f(third_h), YELLOW);
            let neg = fp::fneg(t);
            rect(
                q,
                f(x),
                f(y.wrapping_add(third_h)),
                fp::fadds(f(w), t),
                neg,
                YELLOW,
            );
            rect(q, f(x), f(bot_y), fp::fadds(f(w), t), neg, YELLOW);
            if lit {
                let two_third_y = y.wrapping_add(h.wrapping_mul(2) / 3);
                let sixth = (h / 6).wrapping_sub(1);
                if b.slots[0].x4c == 0 {
                    rect(q, f(x), f(two_third_y), t, f(third_h), c);
                    rect(q, f(x), f(two_third_y), f(half), neg, c);
                    rect(
                        q,
                        f(x.wrapping_add(half)),
                        f(y.wrapping_add(h / 2)),
                        t,
                        f(sixth),
                        c,
                    );
                    return;
                }
                if b.slots[1].x4c == 0 {
                    rect(q, f(right), f(two_third_y), t, f(third_h), c);
                    rect(q, f(x.wrapping_add(half)), f(two_third_y), f(half), neg, c);
                    rect(
                        q,
                        f(x.wrapping_add(half)),
                        f(y.wrapping_add(h / 2)),
                        t,
                        f(sixth),
                        c,
                    );
                    return;
                }
                if b.slots[2].x4c == 0 {
                    rect(q, f(x), f(y), t, f(third_h), c);
                    rect(q, f(x), f(y.wrapping_add(third_h)), f(half), neg, c);
                    rect(
                        q,
                        f(x.wrapping_add(half)),
                        f(y.wrapping_add(third_h)),
                        t,
                        f(sixth),
                        c,
                    );
                    return;
                }
                rect(q, f(right), f(y), t, f(third_h), c);
                rect(
                    q,
                    f(x.wrapping_add(half)),
                    f(y.wrapping_add(third_h)),
                    f(half),
                    neg,
                    c,
                );
                rect(
                    q,
                    f(x.wrapping_add(half)),
                    f(y.wrapping_add(third_h)),
                    t,
                    f(sixth),
                    c,
                );
            }
        }
        _ => {}
    }
}

/// `fn_8018D50C`: the lines of a box of kind 1 (two players), with the byes' tails of the
/// small brackets' odd choices.
#[doc(alias = "fn_8018D50C")]
#[allow(clippy::too_many_arguments)]
fn lines_1(
    b: &super::BracketEntry,
    entrants: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    y5: i32,
    q: &mut Vec<Quad>,
) {
    let t = reg(b.x1c);
    let c = b.x20;
    let lit = c[1] == 0;
    rect(q, f(x), f(y), t, f(h), YELLOW);
    let right = x.wrapping_add(w);
    rect(q, f(right), f(y), t, f(h), YELLOW);
    let neg = fp::fneg(t);
    rect(q, f(x), f(y5), fp::fadds(f(w), t), neg, YELLOW);
    if lit {
        if b.slots[0].x4c == 0 {
            rect(q, f(x), f(y), t, f(h), c);
            rect(q, f(x), f(y5), fp::fadds(f(w / 2), t), neg, c);
        } else {
            rect(q, f(right), f(y), t, f(h), c);
            let half2 = w / 2;
            rect(
                q,
                f(x.wrapping_add(half2)),
                f(y5),
                fp::fadds(f(half2), t),
                neg,
                c,
            );
        }
    }
    let bottom = y.wrapping_add(h);
    match entrants {
        1 => {
            if b.slots[1].x32 != 0 {
                rect(q, f(right), f(bottom), t, -70.0, YELLOW);
                if lit && b.slots[0].x4c != 0 {
                    rect(q, f(right), f(bottom), t, -70.0, c);
                }
            }
        }
        3 => {
            if b.slots[0].x32 != 0 {
                rect(q, f(x), f(bottom), t, -60.0, YELLOW);
                if lit && b.slots[0].x4c == 0 {
                    rect(q, f(x), f(bottom), t, -60.0, c);
                }
            } else if b.slots[1].x32 != 0 {
                rect(q, f(right), f(bottom), t, -60.0, YELLOW);
                if lit && b.slots[0].x4c != 0 {
                    rect(q, f(right), f(bottom), t, -60.0, c);
                }
            }
        }
        5 if b.slots[1].x32 != 0 => {
            rect(q, f(right), f(bottom), t, -40.0, YELLOW);
            if lit && b.slots[0].x4c != 0 {
                rect(q, f(right), f(bottom), t, -40.0, c);
            }
        }
        7 if b.slots[1].x32 != 0 => {
            let at = if b.x2 == 0 {
                bottom
            } else {
                y.wrapping_add(0x1E)
            };
            rect(q, f(right), f(at), t, -30.0, YELLOW);
            if lit && b.slots[0].x4c != 0 {
                rect(q, f(right), f(at), t, -30.0, c);
            }
        }
        _ => {}
    }
}

/// `fn_8018DC18`: the lines of a box of kind 2 (three players).
#[doc(alias = "fn_8018DC18")]
fn lines_2(b: &super::BracketEntry, x: i32, y: i32, w: i32, h: i32, y5: i32, q: &mut Vec<Quad>) {
    let t = reg(b.x1c);
    let c = b.x20;
    rect(q, f(x), f(y), t, f(h), YELLOW);
    let right = x.wrapping_add(w);
    rect(q, f(right), f(y), t, f(h), YELLOW);
    let half = w / 2;
    let center = x.wrapping_add(half);
    rect(q, f(center), f(y), t, f(h), YELLOW);
    let neg = fp::fneg(t);
    rect(q, f(x), f(y5), fp::fadds(f(w), t), neg, YELLOW);
    if c[1] == 0 {
        if b.slots[0].x4c == 0 {
            rect(q, f(x), f(y), t, f(h), c);
            rect(q, f(x), f(y5), fp::fadds(f(half), t), neg, c);
            return;
        }
        if b.slots[1].x4c == 0 {
            rect(q, f(center), f(y), t, f(h), c);
            return;
        }
        rect(q, f(right), f(y), t, f(h), c);
        rect(q, f(center), f(y5), fp::fadds(f(half), t), neg, c);
    }
}

/// `fn_8018DF68`: the lines of a box of kind 3 (four players).
#[doc(alias = "fn_8018DF68")]
fn lines_3(b: &super::BracketEntry, x: i32, y: i32, w: i32, h: i32, y5: i32, q: &mut Vec<Quad>) {
    let t = reg(b.x1c);
    let c = b.x20;
    rect(q, f(x), f(y), t, f(h), YELLOW);
    let right = x.wrapping_add(w);
    rect(q, f(right), f(y), t, f(h), YELLOW);
    let third = w / 3;
    rect(q, f(x.wrapping_add(third)), f(y), t, f(h), YELLOW);
    let right_third = right.wrapping_sub(third);
    rect(q, f(right_third), f(y), t, f(h), YELLOW);
    let neg = fp::fneg(t);
    rect(q, f(x), f(y5), fp::fadds(f(w), t), neg, YELLOW);
    if c[1] == 0 {
        if b.slots[0].x4c == 0 {
            rect(q, f(x), f(y), t, f(h), c);
            rect(q, f(x), f(y5), fp::fadds(f(w / 2), t), neg, c);
            return;
        }
        if b.slots[1].x4c == 0 {
            rect(q, f(x.wrapping_add(third)), f(y), t, f(h), c);
            rect(
                q,
                f(x.wrapping_add(third)),
                f(y5),
                fp::fsubs(fp::fadds(f(w / 2), t), f(third)),
                neg,
                c,
            );
            return;
        }
        if b.slots[2].x4c == 0 {
            rect(q, f(right_third), f(y), t, f(h), c);
            let half = w / 2;
            rect(
                q,
                f(x.wrapping_add(half)),
                f(y5),
                fp::fsubs(fp::fadds(f(half), t), f(third)),
                neg,
                c,
            );
            return;
        }
        rect(q, f(right), f(y), t, f(h), c);
        let half = w / 2;
        rect(
            q,
            f(x.wrapping_add(half)),
            f(y5),
            fp::fadds(f(half), t),
            neg,
            c,
        );
    }
}
