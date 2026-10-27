//! The trophy lottery (`tyfigupon.c`, mode 0xC): the coins the save holds (a tenth of its
//! coin word) bet a few at a time (the stick or the D-pad up and down, X and Y, Z for all of
//! them up to twenty, B back to one), A, L, R or Start dropping them into the machine one by
//! one; the lever pulls, the capsule falls, and the trophy drawn (`Toy_80305058` with the
//! chance of a new one the panel shows) comes out on its stand and goes into the screens' own
//! table, which goes back to the save with the save marked to write. B with one coin or none
//! set ends the scene. Its mode's states are `gmtoylottery.c`'s.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::handles::CodeRef;
use super::super::render::{JOBJ_CALLBACK, LOBJ_CALLBACK};
use super::super::state::{OBJ_CAMERA, OBJ_JOBJ, OBJ_LIGHT, Obj, Running, UserData};
use super::{Loaded, TROPHY_COUNT};
use crate::fpu::{mem, reg};
use crate::hsd::cobj;
use crate::hsd::gobj::{self, GObjId, ProcId};
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::sis::TextId;
use crate::vec::Vec3;

/// The lottery's processes and render callbacks.
pub const PROC_COIN: u32 = 0x8031_4C5C;
pub const PROC_LEVER: u32 = 0x8031_52BC;
pub const PROC_TEXT: u32 = 0x8031_5574;
pub const PROC_DRAW: u32 = 0x8031_55C8;
pub const PROC_DROP: u32 = 0x8031_5C44;
pub const PROC_FALL: u32 = 0x8031_6170;
pub const PROC_TILT: u32 = 0x8031_68DC;
pub const PROC_SONG: u32 = 0x8031_6BF8;
pub const PROC_INPUT: u32 = 0x8031_6C24;
pub const RENDER_MAIN: u32 = 0x8031_4BE4;

/// `Toy_RemoveUserData`, the blocks' release (`HSD_Free`, [`super::super::state`]).
#[doc(alias = "Toy_RemoveUserData")]
pub const TOY_REMOVE_USER_DATA: u32 = 0x8030_6CF4;

/// `tyFigupon_Scene_OnEnter`, `tyFigupon_Scene_OnFrame`.
pub const ON_ENTER: CodeRef = CodeRef(0x8031_7D80);
pub const ON_FRAME: CodeRef = CodeRef(0x8031_82D4);

/// `_tyFigupon_803FEB88`: the panel's parts `lb_8001204C` takes; `_tyFigupon_804D5AA0` the
/// lever's.
const PANEL_PARTS: [i32; 13] = [4, 2, 3, 5, 6, 7, 8, 9, 13, 10, 11, 12, 14];
const LEVER_PARTS: [i32; 1] = [12];

/// The trophy's camera (`_tyFigupon_803FEEFC`), which `_tyFigupon_804D6F08` names.
pub const TROPHY_CAMERA_ADDR: u32 = 0x803F_EEFC;
const TROPHY_CAMERA: cobj::StaticDesc = cobj::StaticDesc {
    flags: 0,
    projection_type: 1,
    viewport: [0, 640, 0, 480],
    scissor: [0, 640, 0, 480],
    eye: Vec3::new(0.0, 0.0, 50.0),
    interest: Vec3::new(0.0, 0.0, 0.0),
    roll: 0.0,
    near: 0.1,
    far: 32768.0,
    projection: [40.0, 913.0 / 750.0, 0.0, 0.0],
};

/// The lottery's user data blocks (0x58 bytes of `HSD_MemAlloc`, freed with `HSD_Free`): a
/// coin's (`Toy`: its frames, its step and its fall's), the drop's and the lever's
/// (`TyFiguponUD`: their frames and the coins' and the bet's digits), the trophy's fall
/// (its speed) and the "new" sign's (`Toy_80306B18`'s frames).
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct UserBlock {
    /// Which block: 0 a coin's, 1 the lever's, 2 the drop's, 3 the trophy's fall's, 4 the
    /// "new" sign's (what its owner writes: the rest keeps what the heap held).
    pub kind: u8,
    /// The sign's `x4` and `x8` written (`Toy_80306B18`), or the drop's bet digits (`x28`).
    pub armed: bool,
    pub x4: i32,
    pub x8: i32,
    pub x10: [i32; 3],
    pub x28: [i32; 3],
    pub x44: f32,
    /// `Toy.translate` (+0x40) and `offset.y` (+0x50).
    pub step: Vec3,
    pub fall: f32,
}

/// The lottery's heap blocks and `.sbss` (`tyfigupon.c`).
#[derive(Clone, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Lottery {
    /// `_tyFigupon_804D6EF0` (`TyFiguponData`, 0x34 bytes): its two cameras, the "new" sign,
    /// the text's process, its two texts, the trophy drawn, the text's frames, the frames
    /// before input and whether a draw's trophy is still to come.
    pub data_block: u32,
    pub camera: Option<GObjId>,
    pub trophy_camera: Option<GObjId>,
    pub new_sign: Option<GObjId>,
    pub text_proc: Option<ProcId>,
    pub name: Option<TextId>,
    pub notice: Option<TextId>,
    pub drawn: i32,
    pub text_frames: i32,
    pub wait: u8,
    pub drawing: u8,
    /// `_tyFigupon_804D6EF4` (0x60 bytes): the panel, the background, the lever and the
    /// chance's GObjs, the panel's parts (and the lever's), `TyMnFigp`, the new trophies
    /// left, the coins' fall, the draw's frames and state, the coins drawn with and the bet.
    pub ef4_block: u32,
    pub panel: Option<GObjId>,
    pub bg: Option<GObjId>,
    pub lever: Option<GObjId>,
    pub chance: Option<GObjId>,
    pub jobjs: [Option<JObjId>; 16],
    pub archive: Option<Loaded>,
    pub left: i16,
    pub x56: i16,
    pub x58: i16,
    pub state: i8,
    pub drawn_bet: i8,
    pub bet: i8,
    /// `_tyFigupon_804D6EF8` (a `ToyListEntry`, 0x18 bytes): the trophy drawn's model file.
    pub entry_block: u32,
    pub entry_file: Option<DataRef>,
    pub entry_trophy: i16,
    pub entry_archive: Option<Loaded>,
    /// `_tyFigupon_804D6EFC`, `F00`: the texts' canvases; `F04` the camera's descriptor
    /// (`TyMnFigp`'s), `F08` the trophy's (the executable's).
    pub canvases: [i32; 2],
    pub camera_desc: Option<DataRef>,
    pub trophy_desc: bool,
    /// The user data blocks by address.
    pub user: std::collections::BTreeMap<u32, UserBlock>,
}

/// `HSD_PAD_*` as the copy statuses hold them.
const PAD_B: u32 = 0x200;

impl Running<'_> {
    fn lot(&mut self) -> &mut Lottery {
        &mut self.game.toys.lottery
    }

    fn lot_archive(&self) -> melee_data::archive::ArchiveId {
        self.game
            .toys
            .lottery
            .archive
            .unwrap_or_else(|| panic!("*** BG data aren't being loaded!"))
            .id
    }

    fn lot_sym(&self, name: &str) -> Option<DataRef> {
        let a = self.lot_archive();
        self.data
            .get(a)
            .public(name)
            .map(|offset| DataRef { archive: a, offset })
    }

    fn lot_j(&self, i: usize) -> Option<JObjId> {
        self.game.toys.lottery.jobjs[i]
    }

    fn lot_gobj_jobj(&self, g: GObjId) -> JObjId {
        match self.game.world.gobj(g).obj {
            Some(Obj::JObj(j)) => j,
            _ => panic!("a lottery GObj without a model"),
        }
    }

    /// A user data block (0x58 bytes of `HSD_MemAlloc`) on `gobj` with `HSD_Free` as its
    /// release (`GObj_InitUserData`): its address.
    fn lot_user_new(&mut self, gobj: GObjId, kind: u8) -> u32 {
        let a = crate::hsd::heap::mem_alloc(0x58);
        self.lot().user.insert(
            a,
            UserBlock {
                kind,
                ..UserBlock::default()
            },
        );
        self.game.world.init_user_data(gobj, 0, UserData::Toy(a));
        a
    }

    fn lot_user(&self, gobj: GObjId) -> Option<u32> {
        match self.game.world.gobj(gobj).user_data {
            Some(UserData::Toy(a)) => Some(a),
            _ => None,
        }
    }

    fn lot_ud(&mut self, a: u32) -> &mut UserBlock {
        self.game
            .toys
            .lottery
            .user
            .get_mut(&a)
            .expect("a lottery user block")
    }

    /// `GObj_RemoveUserData`: the block freed and the GObj without user data.
    fn lot_user_remove(&mut self, gobj: GObjId) {
        gobj::remove_user_data(self, gobj);
    }

    /// `_tyFigupon_80314AA8(jobj, anim, matanim, shapeanim)`: a model's animations from
    /// `TyMnFigp`, requested at 0.
    #[doc(alias = "_tyFigupon_80314AA8")]
    fn lot_anims(&mut self, j: Option<JObjId>, names: [Option<&str>; 3]) {
        let [a, m, s] = names.map(|n| n.and_then(|n| self.lot_sym(n)));
        let data = self.data;
        let o = &mut self.game.objs;
        jobj::add_anim_all(o, data, j, a, m, s);
        jobj::req_anim_all(o, j, 0.0);
    }

    fn lot_anim_all(&mut self, j: Option<JObjId>) {
        jobj::anim_all(
            &mut self.game.objs,
            self.data,
            &mut super::super::code::ParticleHooks,
            j,
        );
    }

    /// `_tyFigupon_80314B54`: the trophies held the region can win of classes 2 to 7.
    #[doc(alias = "_tyFigupon_80314B54")]
    fn lot_held(&self) -> i32 {
        (0..TROPHY_COUNT as i32)
            .filter(|&i| {
                self.toy_winnable(i) && self.toy_flags()[i as usize] & 0xFF != 0 && {
                    let c = self.toy_param_int(i, 6);
                    c != 8 && c as u32 > 1
                }
            })
            .count() as i32
    }

    /// The new trophies left in the open classes (`tyFigupon_CountRemaining`).
    fn lot_remaining(&self) -> i32 {
        let done = self.lot_held();
        let count: i32 = (0..9)
            .filter(|&i| i != 8 && i as u32 > 1 && self.toy_class_open(i))
            .map(super::super::gm::trophies::class_size)
            .sum();
        count - done
    }

    /// The coins the lottery counts: a tenth of the save's coin word (`gm_801623D8`).
    #[doc(alias = "gm_801623D8")]
    fn lot_coins(&self) -> u32 {
        self.game.game_data.thing.save_data.x1A48 / 10
    }

    /// `_tyFigupon_80314BE4`, the camera's render: the screen erased to dark blue, then its
    /// links.
    #[doc(alias = "_tyFigupon_80314BE4")]
    pub fn lot_main_render(&mut self, gobj: GObjId) {
        let c = match self.game.world.gobj(gobj).obj {
            Some(Obj::CObj(c)) => c,
            _ => panic!("the lottery's camera has no CObj"),
        };
        if crate::hsd::display::camera_begin(&mut self.game.objs, self.data, Some(c)) {
            let d = &mut self.game.objs.display.draws;
            d.push(crate::hsd::display::Draw::EraseColor([
                0x19, 0x19, 0x33, 0xFF,
            ]));
            d.push(crate::hsd::display::Draw::Erase);
            self.render_links(gobj, 7);
            crate::hsd::display::fog_set(&mut self.game.objs, None);
            crate::hsd::display::camera_end(&mut self.game.objs);
        }
    }

    /// `_tyFigupon_80314C5C`, a coin's process: its first run a block, a start from a random
    /// frame, a place near the slot and a step toward the machine (falling as it goes), and
    /// its sound; then each frame its fall, until its frames run out, its sound and the coin
    /// gone.
    #[doc(alias = "_tyFigupon_80314C5C")]
    pub fn lot_coin_proc(&mut self, gobj: GObjId) {
        let j = self.lot_gobj_jobj(gobj);
        match self.lot_user(gobj) {
            Some(a) => {
                let ud = *self.lot_ud(a);
                if ud.x8 % 30 == 0 {
                    jobj::req_anim_all(&mut self.game.objs, Some(j), 0.0);
                }
                self.lot_anim_all(Some(j));
                let left = ud.x8;
                self.lot_ud(a).x8 = left - 1;
                if left != 0 {
                    let u = self.lot_ud(a);
                    u.step.y = mem(fp::fadds(reg(u.step.y), reg(u.fall)));
                    let step = u.step;
                    let target = self.lot_j(2).expect("the machine's slot");
                    let jo = &mut self.game.objs.jobjs;
                    let t = jo[target].translate;
                    if reg(jo[j].translate.x) > reg(t.x) {
                        jobj::set_srt(jo, j, |o| {
                            o.translate.x = mem(fp::fadds(reg(o.translate.x), reg(step.x)));
                        });
                    }
                    if reg(jo[j].translate.z) > reg(t.z) {
                        jobj::set_srt(jo, j, |o| {
                            o.translate.z = mem(fp::fadds(reg(o.translate.z), reg(step.z)));
                        });
                    }
                    jobj::set_srt(jo, j, |o| {
                        o.translate.y = mem(fp::fadds(reg(o.translate.y), reg(step.y)));
                    });
                } else {
                    self.sfx_play(146, 127, 64);
                    self.lot_ud(a).x8 = 0;
                    gobj::free(self, gobj);
                }
            }
            None => {
                let a = self.lot_user_new(gobj, 0);
                let r = self.game.objs.rng.below(15);
                jobj::req_anim_all(&mut self.game.objs, Some(j), r as f32);
                let child = self.game.objs.jobjs[j].child.expect("a coin's child");
                let aobj = self.game.objs.jobjs[child].aobj;
                crate::hsd::aobj::set_rate(&mut self.game.objs.aobjs, aobj, 1.0);
                let (from, to) = (
                    self.lot_j(1).expect("the slot"),
                    self.lot_j(2).expect("the machine"),
                );
                let jo = &self.game.objs.jobjs;
                let (f, t) = (jo[from].translate, jo[to].translate);
                let r1 = self.game.objs.rng.draw_f32();
                let x = mem(fp::fmadds(reg(3.0), reg(r1), reg(f.x)));
                let r2 = self.game.objs.rng.draw_f32();
                let y = mem(fp::fmadds(reg(3.0), reg(r2), reg(f.y)));
                let z = f.z;
                let abs = |v: f64| if v < 0.0 { -v } else { v };
                let dx = abs(fp::fsubs(reg(t.x), reg(x)));
                let dz = abs(fp::fsubs(reg(t.z), reg(z)));
                let dy = abs(fp::fsubs(reg(t.y), reg(y)));
                let frames = fp::fctiwz(fp::fadds(reg(35.0), fp::frsp(f64::from(fp::fctiwz(dy)))));
                let u = self.lot_ud(a);
                u.step.x = mem(fp::fmuls(reg(-0.74), fp::fdivs(dx, reg(30.0))));
                u.step.z = mem(fp::fmuls(reg(-0.72), fp::fdivs(dz, reg(30.0))));
                u.step.y = 0.4;
                u.fall = -0.026_666_667;
                u.x8 = frames;
                self.lot().x56 = frames as i16;
                let jo = &mut self.game.objs.jobjs;
                jobj::set_srt(jo, j, |o| o.translate.x = x);
                jobj::set_srt(jo, j, |o| o.translate.y = y);
                jobj::set_srt(jo, j, |o| o.translate.z = z);
                jobj::clear_flags_all(jo, Some(j), jobj::HIDDEN);
                self.sfx_play_track(147, 127, 64, 26);
            }
        }
    }

    /// `_tyFigupon_803152BC`, the lever's process: its first run a block and its frames
    /// (thirty, and thirty more for every fifteen coins bet); then the lever's animation each
    /// frame, from its start every thirty, until they run out.
    #[doc(alias = "_tyFigupon_803152BC")]
    pub fn lot_lever_proc(&mut self, gobj: GObjId) {
        let j0 = self.lot_j(0);
        match self.lot_user(gobj) {
            Some(a) => {
                let left = self.lot_ud(a).x8;
                self.lot_ud(a).x8 = left - 1;
                if left != 0 {
                    self.lot_anim_all(j0);
                    if (left - 1) % 30 == 0 {
                        jobj::req_anim_all(&mut self.game.objs, j0, 0.0);
                    }
                } else {
                    self.lot_ud(a).x8 = 0;
                    self.lot_user_remove(gobj);
                    let p = self.game.world.current_proc().expect("the lever's process");
                    self.game.world.remove_proc(p);
                }
            }
            None => {
                let a = self.lot_user_new(gobj, 1);
                jobj::req_anim_all(&mut self.game.objs, j0, 0.0);
                let bet = i32::from(self.game.toys.lottery.bet);
                self.lot_ud(a).x8 = 0x1E + (bet * 2 / 30) * 0x1E;
            }
        }
    }

    /// `_tyFigupon_803153EC(n, first, digits, mode, _)`: `n`'s digits on the panel's parts
    /// from `first`, each requested at its digit's frame (mode 1: `50 - 5 * digit`), 0 for a
    /// zero, run once at rate 1.
    #[doc(alias = "_tyFigupon_803153EC")]
    fn lot_digits(&mut self, n: i32, first: usize, digits: usize, mode: i32) {
        let mut d = [0i32; 4];
        let mut count = 0;
        let mut num = n;
        loop {
            d[count] = num % 10;
            count += 1;
            num /= 10;
            if num <= 0 {
                break;
            }
        }
        for (i, &digit) in d.iter().enumerate().take(digits) {
            let j = self.lot_j(first + i);
            let frame = if digit != 0 {
                if mode == 1 {
                    (0x32 - digit * 5) as f32
                } else {
                    digit as f32
                }
            } else {
                0.0
            };
            jobj::req_anim_all(&mut self.game.objs, j, frame);
            let a = j.and_then(|j| self.game.objs.jobjs[j].aobj);
            crate::hsd::aobj::set_rate(&mut self.game.objs.aobjs, a, 1.0);
            self.lot_anim_all(j);
        }
    }

    /// The bet's arrow: up (1), down (2), held (0), at twenty (3) or at one or none (4).
    fn lot_arrow(&mut self, frame: i32) {
        let bet = self.game.toys.lottery.bet;
        let f = if bet <= 1 {
            4
        } else if bet == 0x14 {
            3
        } else {
            frame
        };
        let j = self.lot_j(8);
        jobj::req_anim_all(&mut self.game.objs, j, f as f32);
        let a = j.and_then(|j| self.game.objs.jobjs[j].aobj);
        crate::hsd::aobj::set_rate(&mut self.game.objs.aobjs, a, 1.0);
        self.lot_anim_all(j);
    }

    /// The bet's chance of a new trophy on the panel (`setupPercentDisplay`): the new ones
    /// left over the trophies in play, and five percent more for each coin past the first,
    /// in tenths of a percent (999 at a certainty).
    fn lot_chance(&mut self) {
        let l = &self.game.toys.lottery;
        let mut sc = i32::from(l.bet);
        let left = i32::from(l.left);
        let total = fp::frsp(f64::from(left + self.lot_held()));
        if sc != 0 {
            sc -= 1;
        }
        let pct = if left == 0 {
            0.0
        } else {
            fp::fadds(
                fp::fdivs(fp::frsp(f64::from(left)), total),
                fp::fdivs(fp::frsp(f64::from(sc * 5)), reg(100.0)),
            )
        };
        let pct = if pct >= reg(1.0) {
            reg(999.0)
        } else {
            fp::fmuls(pct, reg(1000.0))
        };
        self.lot_digits(fp::fctiwz(pct), 9, 3, 2);
    }

    /// `_tyFigupon_80315574`, the notice's process: hidden after its frames.
    #[doc(alias = "_tyFigupon_80315574")]
    pub fn lot_text_proc(&mut self, _gobj: GObjId) {
        if self.game.toys.lottery.text_frames == 0 {
            let t = self.game.toys.lottery.notice.expect("the notice");
            self.game.sis.text_mut(t).hidden = 1;
            self.lot().text_proc = None;
            let p = self
                .game
                .world
                .current_proc()
                .expect("the notice's process");
            self.game.world.remove_proc(p);
        } else {
            self.lot().text_frames -= 1;
        }
    }

    /// `_tyFigupon_803155C8`, the draw: the lever, its sound and rumble, the capsule's spin
    /// (longer with three coins or more), then the trophy drawn with the bet's chance, shown
    /// for two seconds; then the bet back to one (with coins left) and the chance shown.
    #[doc(alias = "_tyFigupon_803155C8")]
    pub fn lot_draw_proc(&mut self, _gobj: GObjId) {
        let lever = self.game.toys.lottery.lever.expect("the lever");
        let j = Some(self.lot_gobj_jobj(lever));
        let l = &self.game.toys.lottery;
        let spin = |r: &mut Self| {
            let left = r.game.toys.lottery.left;
            let jj = r.lot_j(0xE);
            jobj::req_anim_all(&mut r.game.objs, jj, if left != 0 { 0.0 } else { 1.0 });
        };
        match l.state {
            2 => {
                self.lot().x58 -= 1;
                if self.game.toys.lottery.x58 == 0 {
                    self.lot().drawing = 1;
                    self.lot().x58 = 0x14;
                    jobj::req_anim_all(&mut self.game.objs, j, 0.0);
                    self.lot().state = 3;
                    let port = self.game.menu.confirm_port as u8;
                    self.lot_rumble(port, 0x1E, 0);
                }
            }
            3 => {
                spin(self);
                self.lot_anim_all(j);
                self.lot().x58 -= 1;
                if self.game.toys.lottery.x58 == 0 {
                    self.sfx_play(0xA8, 0x7F, 0x40);
                    self.lot().x58 = 0x14;
                    self.lot().state = 4;
                }
            }
            4 => {
                spin(self);
                self.lot_anim_all(j);
                if self.game.toys.lottery.x58 < 0xE {
                    jobj::req_anim_all(&mut self.game.objs, j, 30.0);
                }
                self.lot().x58 -= 1;
                if self.game.toys.lottery.x58 == 0 {
                    self.lot().x58 = 0xE;
                    self.lot().state = 5;
                }
            }
            5 => {
                self.lot().x58 -= 1;
                if self.game.toys.lottery.x58 == 0 {
                    let sc = i32::from(self.game.toys.lottery.drawn_bet);
                    if sc >= 3 {
                        let f = fp::fmuls(fp::frsp(f64::from(sc)), reg(1.62));
                        self.lot().x58 = (fp::fctiwz(f) as i16).wrapping_add(0xE);
                        jobj::req_anim_all(&mut self.game.objs, j, 50.0);
                        let jj = self.lot_j(0xE);
                        jobj::req_anim_all(&mut self.game.objs, jj, 0.0);
                        self.sfx_play_track(0xA9, 0x7F, 0x40, 0x8A);
                    } else {
                        self.lot().x58 = 1;
                    }
                    self.lot().state = 6;
                }
            }
            6 => {
                if self.game.toys.lottery.drawn_bet >= 3 {
                    spin(self);
                    self.lot_anim_all(j);
                    if self.game.toys.lottery.x58 % 40 == 0 {
                        jobj::req_anim_all(&mut self.game.objs, j, 50.0);
                        self.sfx_play_track(0xA9, 0x7F, 0x40, 0x8A);
                    }
                    if self.game.toys.lottery.x58 < 0xE {
                        jobj::req_anim_all(&mut self.game.objs, j, 50.0);
                    }
                }
                self.lot().x58 -= 1;
                if self.game.toys.lottery.x58 == 0 {
                    if self.game.toys.lottery.drawing != 0 {
                        let left = i32::from(self.game.toys.lottery.left);
                        let total = fp::frsp(f64::from(left + self.lot_held()));
                        let mut sc = i32::from(self.game.toys.lottery.drawn_bet);
                        self.sfx_stop_all();
                        if sc != 0 {
                            sc -= 1;
                        }
                        let pct = fp::fmuls(
                            reg(100.0),
                            fp::fadds(
                                fp::fdivs(fp::frsp(f64::from(left)), total),
                                fp::fdivs(fp::frsp(f64::from(sc * 5)), reg(100.0)),
                            ),
                        );
                        let pct = if pct >= reg(100.0) { 99.9 } else { mem(pct) };
                        let t = self.toy_draw(0x63, 3, 0, pct);
                        self.lot().drawn = t;
                        self.lot_reveal(t as i16);
                        let rem = self.lot_remaining();
                        self.lot().left = rem as i16;
                        self.lot().drawing = 0;
                        self.lot().x58 = 0x78;
                        self.lot().state = 8;
                        return;
                    }
                    self.lot().state = 9;
                }
            }
            8 => {
                self.lot().x58 -= 1;
                if self.game.toys.lottery.x58 == 0 {
                    if let Some(a) = self.game.toys.lottery.entry_archive.take() {
                        self.toy_free(a);
                    }
                    if let Some(g) = self.game.toys.anim.gobj.take() {
                        gobj::free(self, g);
                        self.game.toys.anim.jobj = [None, None];
                    }
                    let t = self.game.toys.lottery.name.expect("the name");
                    self.game.sis.reset_text(self.data, t, 0);
                    self.toy_mark(self.lot_j(0xC), 0x3E7);
                    if self.game.toys.lottery.left == 0 {
                        let jj = self.lot_j(0xE);
                        jobj::req_anim_all(&mut self.game.objs, jj, 1.0);
                        self.lot_anim_all(j);
                    }
                    self.lot().state = 9;
                }
            }
            9 => {
                if self.lot_coins() != 0 {
                    self.lot().bet = 1;
                    self.lot_digits(1, 6, 2, 0);
                    self.lot_arrow(1);
                }
                self.lot_chance();
                let l = self.lot();
                l.x58 = 0;
                l.x56 = 0;
                let p = self.game.world.current_proc().expect("the draw's process");
                self.game.world.remove_proc(p);
                self.lot().state = 0;
            }
            _ => {}
        }
    }

    /// `gm_801678F8(port, kind, frames)`: a rumble on `port` if its rumble is on
    /// (`gm_80167858` without a name tag).
    #[doc(alias = "gm_801678F8")]
    #[doc(alias = "gm_80167858")]
    fn lot_rumble(&mut self, port: u8, kind: i32, frames: i32) {
        if self.port_rumble(usize::from(port)) != 0 {
            self.rumble_start(port, 3, kind, frames);
        }
    }

    /// The drop's digits, the coins and the bet (`tyFigupon_StoreDigits`).
    fn lot_store_digits(d: &mut [i32; 3], value: i32) {
        *d = [0; 3];
        let mut v = value;
        let mut i = 0;
        loop {
            d[i] = v % 10;
            i += 1;
            v /= 10;
            if v <= 0 {
                break;
            }
        }
    }

    /// `_tyFigupon_80315C44`, the coins' drop (the panel's process): its first run a block,
    /// the coins' digits and their texture animations doubled, two frames a coin; then each
    /// frame the digits rolling (the higher ones while the lower are at zero), and every
    /// other frame a coin dropped (its process, the save's coins less ten, the bet less one,
    /// the digits, the arrow, a rumble); at the end the lever's draw, fourteen frames after
    /// the last coin's fall.
    #[doc(alias = "_tyFigupon_80315C44")]
    pub fn lot_drop_proc(&mut self, gobj: GObjId) {
        let Some(a) = self.lot_user(gobj) else {
            let a = self.lot_user_new(gobj, 2);
            let total = self.lot_coins() as i32;
            Self::lot_store_digits(&mut self.lot_ud(a).x10, total);
            self.lot_digits(total, 3, 3, 1);
            for k in 3..6 {
                let child = self.lot_j(k).and_then(|j| self.game.objs.jobjs[j].child);
                let aobj = self.toy_tobj_aobj(child);
                crate::hsd::aobj::set_rate(&mut self.game.objs.aobjs, aobj, 2.0);
            }
            let bet = i32::from(self.game.toys.lottery.bet);
            self.lot_ud(a).x8 = bet * 2;
            return;
        };
        let ud = *self.lot_ud(a);
        if ud.x8 == 0 {
            self.lot_ud(a).x8 = 0;
            self.lot().bet = 0;
            self.lot_digits(self.lot_coins() as i32, 3, 3, 1);
            self.lot_digits(0, 6, 2, 0);
            self.lot_arrow(0);
            self.lot_user_remove(gobj);
            let l = self.lot();
            l.x58 = l.x56 + 0xE;
            l.state = 2;
            let p = self.game.world.current_proc().expect("the drop's process");
            self.game.world.remove_proc(p);
            return;
        }
        if ud.x10[0] == 0 && ud.x10[1] == 0 && ud.x10[2] != 0 {
            self.lot_anim_all(self.lot_j(5));
        }
        if ud.x10[0] == 0 {
            if ud.x10[1] == 0 {
                let jj = self.lot_j(4);
                jobj::req_anim_all(&mut self.game.objs, jj, 0.0);
            }
            self.lot_anim_all(self.lot_j(4));
        }
        if ud.x10[0] == 0 {
            let jj = self.lot_j(3);
            jobj::req_anim_all(&mut self.game.objs, jj, 0.0);
        }
        self.lot_anim_all(self.lot_j(3));
        self.lot_ud(a).x8 -= 1;
        if self.lot_ud(a).x8 % 2 == 0 {
            self.lot_coin();
            let coins = self.game.game_data.thing.save_data.x1A48.wrapping_sub(0xA);
            self.game.game_data.set_score(coins);
            self.lot().bet -= 1;
            let total = self.lot_coins() as i32;
            Self::lot_store_digits(&mut self.lot_ud(a).x10, total);
            let bet = i32::from(self.game.toys.lottery.bet);
            Self::lot_store_digits(&mut self.lot_ud(a).x28, bet);
            self.lot_ud(a).armed = true;
            self.lot_digits(total, 3, 3, 1);
            self.lot_digits(bet, 6, 2, 0);
            self.lot_arrow(2);
            let port = self.game.menu.confirm_port as u8;
            self.lot_rumble(port, 0x10, 0);
        }
    }

    /// A coin (`tyFigupon_CreateCoin`): `ToyFigurePonCoin_Top` on a GObj (class 9, link 9,
    /// drawn on link 0x3C), its animations at their start, hidden, with its process.
    fn lot_coin(&mut self) {
        let g = self.game.world.create(9, 9, 0);
        let joint = self.lot_sym("ToyFigurePonCoin_Top_joint");
        let data = self.data;
        let j = jobj::load_joint(&mut self.game.objs, data, joint).expect("a coin");
        let w = &mut self.game.world;
        w.set_obj(g, OBJ_JOBJ, Obj::JObj(j));
        w.setup_gx_link(g, CodeRef(JOBJ_CALLBACK), 0x3C, 0);
        self.lot_anims(
            Some(j),
            [
                Some("ToyFigurePonCoin_Top_animjoint"),
                Some("ToyFigurePonCoin_Top_matanim_joint"),
                Some("ToyFigurePonCoin_Top_shapeanim_joint"),
            ],
        );
        jobj::req_anim_all(&mut self.game.objs, Some(j), 0.0);
        self.lot_anim_all(Some(j));
        jobj::set_flags_all(&mut self.game.objs.jobjs, Some(j), jobj::HIDDEN);
        let w = &mut self.game.world;
        w.setup_proc(g, CodeRef(PROC_COIN), 0);
        w.mark_ran(g);
    }

    /// `_tyFigupon_80316170`, the trophy's fall onto its stand: its first run a block; then
    /// faster each frame until it lands (at -7.2: the screens' table's `x0` to 1, the new
    /// trophy's sound if it's the first of it, the landing's sound and a rumble).
    #[doc(alias = "_tyFigupon_80316170")]
    pub fn lot_fall_proc(&mut self, gobj: GObjId) {
        let j = self.lot_gobj_jobj(gobj);
        let y = self.game.objs.jobjs[j].translate.y;
        let Some(a) = self.lot_user(gobj) else {
            let a = self.lot_user_new(gobj, 3);
            self.lot_ud(a).x44 = 0.0;
            return;
        };
        let v = self.lot_ud(a).x44;
        if fp::fadds(reg(y), reg(v)) <= reg(-6.2) {
            let t = self.game.toys.anim.trophy;
            if self.trophy_count(t as usize) == 1 {
                self.toy_new_sound();
            }
            self.game.toys.table.x0 = 1;
            self.sfx_play(0xA7, 0x7F, 0x40);
            jobj::set_srt(&mut self.game.objs.jobjs, j, |o| o.translate.y = -7.2);
            let port = self.game.menu.confirm_port as u8;
            self.lot_rumble(port, 0xC, 0);
            self.lot_user_remove(gobj);
            let p = self.game.world.current_proc().expect("the fall's process");
            self.game.world.remove_proc(p);
            return;
        }
        let v = mem(fp::fsubs(reg(v), reg(0.6)));
        self.lot_ud(a).x44 = v;
        jobj::set_srt(&mut self.game.objs.jobjs, j, |o| {
            o.translate.y = mem(fp::fadds(reg(o.translate.y), reg(v)));
        });
    }

    /// `Toy_80306D14`: a new trophy's sound, by the screens' table's `x0` (the gallery's 0xAA,
    /// the lottery's 0xAB).
    #[doc(alias = "Toy_80306D14")]
    fn toy_new_sound(&mut self) {
        match self.game.toys.table.x0 {
            1 => {
                self.sfx_play(0xAA, 0x7F, 0x40);
            }
            2 => {
                self.sfx_play(0xAB, 0x7F, 0x40);
            }
            _ => {}
        }
    }

    /// `_tyFigupon_80316420(trophy)`: the trophy drawn: its model file loaded into the
    /// entry (`Toy_80308250`) and shown (`Toy_803087F4`), placed above the stand turned up to
    /// 30 degrees at random, falling (`_tyFigupon_80316170`); the "new" sign if it's new; one
    /// more in the table (`Toy_SetUnlockState`), the table back to the save, the save to
    /// write; its name and series mark on the panel, and the notice for two seconds when a
    /// class completed.
    #[doc(alias = "_tyFigupon_80316420")]
    #[doc(alias = "Toy_80308250")]
    #[doc(alias = "Toy_80308328")]
    fn lot_reveal(&mut self, trophy: i16) {
        let count = self.trophy_count(trophy as usize);
        let file = self.toy_model_file(i32::from(trophy));
        if let Some(a) = self.game.toys.lottery.entry_archive.take() {
            self.toy_free(a);
        }
        self.lot().entry_file = Some(file);
        self.lot().entry_trophy = trophy;
        let name = self.toy_file_name(file);
        let l = self.toy_load(&name);
        self.lot().entry_archive = Some(l);
        let (g, a) = self.toy_show(file, trophy, Some(l));
        self.lot().entry_archive = Some(a);
        let an = self.game.toys.anim;
        jobj::clear_flags_all(&mut self.game.objs.jobjs, an.jobj[0], jobj::HIDDEN);
        jobj::set_flags_all(&mut self.game.objs.jobjs, an.jobj[1], jobj::HIDDEN);
        let j = self.lot_gobj_jobj(g);
        let jo = &mut self.game.objs.jobjs;
        jobj::set_srt(jo, j, |o| o.translate.x = -1.8);
        jobj::set_srt(jo, j, |o| o.translate.y = 18.6);
        jobj::set_srt(jo, j, |o| o.translate.z = 12.0);
        jobj::set_srt(jo, j, |o| o.scale.x = 0.79);
        jobj::set_srt(jo, j, |o| o.scale.y = 0.79);
        jobj::set_srt(jo, j, |o| o.scale.z = 0.79);
        let r = self.game.objs.rng.draw_f32();
        let angle = fp::fmuls(reg(30.0), fp::fmuls(reg(2.0), fp::fsubs(reg(r), reg(0.5))));
        let rot = mem(fp::fmuls(reg(0.017_453_292), angle));
        jobj::set_rotation_y(&mut self.game.objs.jobjs, j, rot);
        let w = &mut self.game.world;
        w.setup_proc(g, CodeRef(PROC_FALL), 0);
        w.mark_ran(g);
        if count == 0 {
            let sign = self.game.toys.lottery.new_sign.expect("the new sign");
            self.toy_anim_frames(sign, 0, 0x48, 1);
            self.game.world.mark_ran(sign);
        }
        self.add_trophy(i32::from(trophy), 1);
        self.toy_table_sync(true);
        self.card_game_mark_changes();
        let t = self.game.toys.lottery.name.expect("the name");
        let s = self.toy_string(i32::from(trophy), 2, 0x128);
        self.game.sis.reset_text(self.data, t, s);
        self.toy_mark(self.lot_j(0xC), i32::from(trophy));
        if self.game.toys.table.x0 == 2 {
            if self.game.toys.lottery.text_proc.is_none() {
                let c = self.game.toys.lottery.camera.expect("the camera");
                let p = self.game.world.setup_proc(c, CodeRef(PROC_TEXT), 0);
                self.lot().text_proc = Some(p);
            }
            self.lot().text_frames = 0x12C;
            let n = self.game.toys.lottery.notice.expect("the notice");
            self.game.sis.text_mut(n).hidden = 0;
        }
    }

    /// `Toy_80306B18(gobj, frame, frames, show)`: a model shown and requested at `frame`, its
    /// frames and whether it stays shown in its user data (`Toy`), animated by
    /// `Toy_80306BB8`.
    #[doc(alias = "Toy_80306B18")]
    fn toy_anim_frames(&mut self, gobj: GObjId, frame: i32, frames: i32, show: i32) {
        let Some(a) = self.lot_user(gobj) else {
            return;
        };
        let j = self.lot_gobj_jobj(gobj);
        jobj::clear_flags_all(&mut self.game.objs.jobjs, Some(j), jobj::HIDDEN);
        jobj::req_anim_all(&mut self.game.objs, Some(j), frame as f32);
        let u = self.lot_ud(a);
        u.x8 = frames;
        u.x4 = show;
        u.armed = true;
        self.game
            .world
            .setup_proc(gobj, CodeRef(super::gallery::PROC_ANIM), 0);
        self.game.world.mark_ran(gobj);
    }

    /// `Toy_80306BB8` with user data: the model's frames run, then shown (with `x4`) and the
    /// process removed.
    pub fn toy_anim_frames_proc(&mut self, gobj: GObjId, a: u32) {
        let j = self.lot_gobj_jobj(gobj);
        let left = self.lot_ud(a).x8;
        self.lot_ud(a).x8 = left - 1;
        if left != 0 {
            self.lot_anim_all(Some(j));
        } else {
            self.lot_ud(a).x8 = 0;
            if self.lot_ud(a).x4 != 0 {
                jobj::clear_flags_all(&mut self.game.objs.jobjs, Some(j), jobj::HIDDEN);
            }
            let p = self.game.world.current_proc().expect("the model's process");
            self.game.world.remove_proc(p);
        }
    }

    /// `_tyFigupon_803168DC`, a camera's tilt by the first C-stick past 0.4 (up to 30 degrees
    /// each way), from its descriptor's place each frame; the stand's look by the tilt.
    #[doc(alias = "_tyFigupon_803168DC")]
    pub fn lot_tilt_proc(&mut self, gobj: GObjId) {
        let (mut cx, mut cy) = (0.0f32, 0.0f32);
        let dead = |v: f32| reg(v) > reg(-0.4) && reg(v) < reg(0.4);
        for i in 0..4 {
            let p = &self.game.pads.copy[i];
            cx = p.nml_sub_stick_x;
            cy = p.nml_sub_stick_y;
            if !(dead(cx) && dead(cy)) {
                break;
            }
        }
        let rot = |v: f32| {
            if dead(v) {
                0.0
            } else {
                let d = if reg(v) < 0.0 { -0.4 } else { 0.4 };
                mem(fp::fmuls(
                    reg(30.0),
                    fp::fdivs(fp::fsubs(reg(v), reg(d)), reg(0.6)),
                ))
            }
        };
        let (rot_x, rot_y) = if dead(cx) && dead(cy) {
            (0.0, 0.0)
        } else {
            (rot(cx), rot(cy))
        };
        let c = match self.game.world.gobj(gobj).obj {
            Some(Obj::CObj(c)) => c,
            _ => panic!("the lottery's camera has no CObj"),
        };
        let data = self.data;
        let desc = self
            .game
            .toys
            .lottery
            .camera_desc
            .expect("the camera's descriptor");
        let o = &mut self.game.objs;
        cobj::init(o, data, Some(c), Some(desc));
        let up = Vec3::new(0.0, 1.0, 0.0);
        let mut eye = cobj::eye_vector(o, data, Some(c)).expect("the eye vector");
        let interest = cobj::interest(o, data, c).expect("the interest");
        let mut cross = up.sdk_cross(eye);
        cross.normalize();
        let m = crate::mtx::Mtx34::rotation_about(
            cross,
            mem(fp::fmuls(reg(0.017_453_292), reg(rot_y))),
        );
        eye = m.transform(eye);
        let m =
            crate::mtx::Mtx34::rotation_about(up, mem(fp::fmuls(reg(0.017_453_292), reg(rot_x))));
        eye = m.transform(eye);
        let dist = cobj::eye_distance(o, data, Some(c));
        let eye = eye.sdk_scale(dist);
        cobj::set_eye_position(o, c, interest.sdk_sub(eye));
        let an = self.game.toys.anim;
        if an.gobj.is_some() {
            let jo = &mut self.game.objs.jobjs;
            if reg(rot_y) > reg(10.0) {
                jobj::clear_flags_all(jo, an.jobj[0], jobj::HIDDEN);
                jobj::set_flags_all(jo, an.jobj[1], jobj::HIDDEN);
            } else if reg(rot_y) < reg(-10.0) {
                jobj::clear_flags_all(jo, an.jobj[1], jobj::HIDDEN);
                jobj::set_flags_all(jo, an.jobj[0], jobj::HIDDEN);
            }
        }
    }

    /// `_tyFigupon_80316BF8`: the lottery's song (0x35), once.
    #[doc(alias = "_tyFigupon_80316BF8")]
    pub fn lot_song_proc(&mut self, _gobj: GObjId) {
        self.lb_play_music(0x35);
        let p = self.game.world.current_proc().expect("the song's process");
        self.game.world.remove_proc(p);
    }

    /// `_tyFigupon_80316C24`, the lottery's input (no draw running).
    #[doc(alias = "_tyFigupon_80316C24")]
    pub fn lot_input_proc(&mut self, gobj: GObjId) {
        if self.game.toys.lottery.state != 0 {
            return;
        }
        let stick = super::view::dead_zone(self.toy_axis(1));
        if self.game.buttons_triggered(4) & super::super::mn::main::PAD_LR_START != 0 {
            self.lb_audio_call(0);
            self.game.game_data.x1850.force_main_menu = 1;
            self.game.toys.table.exit = 1;
            return;
        }
        if self.toy_triggered() & PAD_B != 0 {
            if self.lot_coins() == 0 || self.game.toys.lottery.bet <= 1 {
                self.lb_audio_call(0);
                self.game.toys.table.exit = 1;
                return;
            }
            self.lot().bet = 1;
            self.lot_digits(1, 6, 2, 0);
            self.lot_arrow(2);
            self.lot_chance();
            self.lb_audio_call(2);
            self.lot().wait = 1;
            return;
        }
        let w = self.game.toys.lottery.wait;
        if w != 0 {
            self.lot().wait = w - 1;
            return;
        }
        if self.lot_coins() == 0 || self.game.toys.lottery.bet == 0 {
            if reg(stick) != 0.0 || self.toy_triggered() & 0x1D7C != 0 {
                self.lb_audio_call(3);
                self.lot().wait = 0x1E;
            }
            return;
        }
        if self.toy_triggered() & 0x10 != 0 {
            let coins = self.lot_coins();
            self.lot().bet = if coins > 0x14 { 0x14 } else { coins as i8 };
            let bet = i32::from(self.game.toys.lottery.bet);
            self.lot_digits(bet, 6, 2, 0);
            self.lot_arrow(1);
            self.lot_chance();
            self.lb_audio_call(2);
            self.lot().wait = 1;
            return;
        }
        if self.toy_triggered() & 0x1160 != 0 {
            let bet = self.game.toys.lottery.bet;
            if bet != 0 {
                let l = self.lot();
                l.drawn_bet = bet;
                l.x56 = 0x1E;
                let (panel, chance) = (l.panel.expect("the panel"), l.chance.expect("the chance"));
                let w = &mut self.game.world;
                w.setup_proc(panel, CodeRef(PROC_DROP), 0);
                w.mark_ran(panel);
                w.setup_proc(chance, CodeRef(PROC_LEVER), 0);
                w.mark_ran(chance);
                self.lot().state = 1;
                let w = &mut self.game.world;
                w.setup_proc(gobj, CodeRef(PROC_DRAW), 0);
                w.mark_ran(gobj);
                return;
            }
        }
        if reg(stick) <= reg(-0.1) || self.toy_held() & 0x400 != 0 || self.toy_triggered() & 4 != 0
        {
            let bet = self.game.toys.lottery.bet;
            if bet > 1 {
                self.lot().bet = bet - 1;
                self.lot_digits(i32::from(bet - 1), 6, 2, 0);
                self.lot_arrow(2);
                self.lot_chance();
                self.lb_audio_call(2);
                self.lot().wait = 3;
            }
        } else if reg(stick) >= reg(0.1)
            || self.toy_held() & 0x800 != 0
            || self.toy_triggered() & 8 != 0
        {
            let total = self.lot_coins();
            let bet = self.game.toys.lottery.bet;
            if (bet as i32 as u32) < total && bet < 0x14 {
                self.lot().bet = bet + 1;
                self.lot_digits(i32::from(bet + 1), 6, 2, 0);
                self.lot_arrow(1);
                self.lot_chance();
                self.lb_audio_call(2);
                self.lot().wait = 3;
            }
        } else {
            self.lot_arrow(0);
        }
    }

    /// `_tyFigupon_8031753C`: the lottery's machine (`TyMnFigp`): the panel (on link 0x3C,
    /// priority 1) and its parts, the series mark hidden, the background (animated by
    /// `Toy_80306BB8`), a coin on the slot, the coins' three digits, the bet's two, the arrow,
    /// the lever and its capsule (spinning while new trophies are left), and the chance's
    /// three digits on a GObj of their own.
    #[doc(alias = "_tyFigupon_8031753C")]
    fn lot_machine(&mut self) {
        if let Some(p) = self.game.toys.lottery.panel.take() {
            gobj::free(self, p);
        }
        let Some(panel) = self.lot_sym("ToyFigurePonPanel_Top_joint") else {
            panic!("*** Can not Load Panel Label(ToyFigurePonPanel_Top_joint)");
        };
        let data = self.data;
        let g = self.game.world.create(9, 9, 0);
        self.lot().panel = Some(g);
        let j = jobj::load_joint(&mut self.game.objs, data, Some(panel)).expect("the panel");
        let w = &mut self.game.world;
        w.set_obj(g, OBJ_JOBJ, Obj::JObj(j));
        w.setup_gx_link(g, CodeRef(JOBJ_CALLBACK), 0x3C, 1);
        let parts =
            super::super::gm::pause::tree_nodes(&self.game.objs.jobjs, Some(j), &PANEL_PARTS);
        for (i, p) in parts.into_iter().enumerate() {
            self.lot().jobjs[i] = p;
        }
        self.lot_anims(
            self.lot_j(0xC),
            [None, Some("ToyFigurePonPanel_zsmash_matanim_joint"), None],
        );
        self.toy_mark(self.lot_j(0xC), 0x3E7);
        let bg = self.lot_sym("ToyFigurePonBg_Top_joint");
        let g = self.game.world.create(9, 9, 0);
        self.lot().bg = Some(g);
        let j = jobj::load_joint(&mut self.game.objs, data, bg).expect("the background");
        let w = &mut self.game.world;
        w.set_obj(g, OBJ_JOBJ, Obj::JObj(j));
        w.setup_gx_link(g, CodeRef(JOBJ_CALLBACK), 0x3C, 0);
        self.lot_anims(
            Some(j),
            [
                Some("ToyFigurePonBg_Top_animjoint"),
                Some("ToyFigurePonBg_Top_matanim_joint"),
                Some("ToyFigurePonBg_Top_shapeanim_joint"),
            ],
        );
        let w = &mut self.game.world;
        w.setup_proc(g, CodeRef(super::gallery::PROC_ANIM), 0);
        w.mark_ran(g);
        let coin = self.lot_sym("ToyFigurePonCoin_Top_joint");
        let c = jobj::load_joint(&mut self.game.objs, data, coin).expect("the slot's coin");
        let jj = self.lot_j(0);
        jobj::add_child(&mut self.game.objs.jobjs, jj, Some(c));
        self.lot_anims(
            Some(c),
            [
                Some("ToyFigurePonCoin_Top_animjoint"),
                Some("ToyFigurePonCoin_Top_matanim_joint"),
                Some("ToyFigurePonCoin_Top_shapeanim_joint"),
            ],
        );
        jobj::req_anim_all(&mut self.game.objs, Some(c), 0.0);
        self.lot_anim_all(Some(c));
        let mut d = [0i32; 4];
        let mut count = 0;
        let mut total = self.lot_coins() as i32;
        let nm = self.lot_sym("ToyFigurePonNm_Top_joint");
        loop {
            d[count] = total % 10;
            count += 1;
            total /= 10;
            if total <= 0 {
                break;
            }
        }
        for (i, &digit) in d.iter().enumerate().take(3) {
            let n = jobj::load_joint(&mut self.game.objs, data, nm).expect("a digit");
            let jj = self.lot_j(3 + i);
            jobj::add_child(&mut self.game.objs.jobjs, jj, Some(n));
            self.lot_anims(
                Some(n),
                [
                    Some("ToyFigurePonNm_Top_animjoint"),
                    Some("ToyFigurePonNm_Top_matanim_joint"),
                    Some("ToyFigurePonNm_Top_shapeanim_joint"),
                ],
            );
            let f = if digit != 0 {
                (50 - digit * 5) as f32
            } else {
                0.0
            };
            jobj::req_anim_all(&mut self.game.objs, Some(n), f);
            self.lot_anim_all(Some(n));
        }
        let bet = self.lot_sym("ToyFigurePonBet_Top_joint");
        for i in 0..2 {
            let n = jobj::load_joint(&mut self.game.objs, data, bet).expect("a bet digit");
            let jj = self.lot_j(6 + i);
            jobj::add_child(&mut self.game.objs.jobjs, jj, Some(n));
            self.lot_anims(
                Some(n),
                [
                    Some("ToyFigurePonBet_Top_animjoint"),
                    Some("ToyFigurePonBet_Top_matanim_joint"),
                    Some("ToyFigurePonBet_Top_shapeanim_joint"),
                ],
            );
            jobj::req_anim_all(&mut self.game.objs, Some(n), 0.0);
            self.lot_anim_all(Some(n));
        }
        self.lot_anims(
            self.lot_j(8),
            [None, Some("ToyFigurePonPanel_zarrow_matanim_joint"), None],
        );
        let lever = self.lot_sym("ToyFigurePonLever_Top_joint");
        let g = self.game.world.create(9, 9, 0);
        self.lot().lever = Some(g);
        let j = jobj::load_joint(&mut self.game.objs, data, lever).expect("the lever");
        let w = &mut self.game.world;
        w.set_obj(g, OBJ_JOBJ, Obj::JObj(j));
        w.setup_gx_link(g, CodeRef(JOBJ_CALLBACK), 0x3C, 0);
        self.lot_anims(
            Some(j),
            [
                Some("ToyFigurePonLever_Top_animjoint"),
                Some("ToyFigurePonLever_Top_matanim_joint"),
                Some("ToyFigurePonLever_Top_shapeanim_joint"),
            ],
        );
        let parts =
            super::super::gm::pause::tree_nodes(&self.game.objs.jobjs, Some(j), &LEVER_PARTS);
        self.lot().jobjs[0xE] = parts[0];
        let remaining = self.lot_remaining();
        let jj = self.lot_j(0xE);
        jobj::req_anim_all(
            &mut self.game.objs,
            jj,
            if remaining != 0 { 0.0 } else { 1.0 },
        );
        self.lot_anim_all(self.lot_j(0xE));
        let g = self.game.world.create(9, 9, 0);
        self.lot().chance = Some(g);
        let par = self.lot_sym("ToyFigurePonPar_Top_joint");
        for i in 0..3 {
            let n = jobj::load_joint(&mut self.game.objs, data, par).expect("a chance digit");
            let jj = self.lot_j(9 + i);
            jobj::add_child(&mut self.game.objs.jobjs, jj, Some(n));
            self.lot_anims(
                Some(n),
                [
                    Some("ToyFigurePonPar_Top_animjoint"),
                    Some("ToyFigurePonPar_Top_matanim_joint"),
                    Some("ToyFigurePonPar_Top_shapeanim_joint"),
                ],
            );
            jobj::req_anim_all(&mut self.game.objs, Some(n), 0.0);
            self.lot_anim_all(Some(n));
        }
    }

    /// `_tyFigupon_80317A60`: the cameras (`TyMnFigp`'s, erasing to dark blue, and the
    /// trophy's), `SdToy`'s strings, the name's and the notice's canvases and texts, and the
    /// camera's processes (the input, both tilts, the song).
    #[doc(alias = "_tyFigupon_80317A60")]
    fn lot_cameras(&mut self) {
        let data = self.data;
        let desc = self
            .lot_sym("ScMenFigure_cam_int1_camera")
            .expect("the lottery's camera");
        let g = self.game.world.create(1, 2, 0);
        self.lot().camera = Some(g);
        let c = crate::lb::cobj::load_cobj(
            &mut self.game.objs,
            data,
            crate::lb::cobj::CObjDesc::Data(desc),
        );
        self.lot().camera_desc = Some(desc);
        let w = &mut self.game.world;
        w.set_obj(g, OBJ_CAMERA, Obj::CObj(c));
        w.setup_gx_link_max(g, CodeRef(RENDER_MAIN), 0, false);
        w.gobj_mut(g).gxlink_prios = 0x5010_0000_0000_0000;
        let t = self.game.world.create(1, 2, 0);
        self.lot().trophy_camera = Some(t);
        let c = crate::lb::cobj::load_cobj(
            &mut self.game.objs,
            data,
            crate::lb::cobj::CObjDesc::Static(&TROPHY_CAMERA),
        );
        self.lot().trophy_desc = true;
        let w = &mut self.game.world;
        w.set_obj(t, OBJ_CAMERA, Obj::CObj(c));
        w.setup_gx_link_max(t, CodeRef(super::gallery::RENDER_CAMERA), 0, false);
        w.gobj_mut(t).gxlink_prios = 0x2680_0000_0000_0000;
        if self.saved_us() {
            self.load_font_named(0, "SdToy.usd", "SIS_ToyData_E");
        } else {
            self.load_font_named(0, "SdToy.dat", "SIS_ToyData");
        }
        let n = self.add_canvas(0, Some(g), 0xB, 0xB, 0, 0x3E, 0, 0);
        self.lot().canvases[0] = n;
        let name = self.create_text(0, n, Vec3::new(5.2, 5.2, 17.2), 448.0, 64.0);
        let us = self.saved_us();
        let tx = self.game.sis.text_mut(name);
        tx.x34 = if us { [1.6, 0.7] } else { [2.0, 1.0] };
        tx.font_size = [0.0235, 0.0715];
        tx.default_alignment = 1;
        if us {
            tx.default_kerning = 1;
        }
        tx.default_fitting = 1;
        self.lot().name = Some(name);
        let n2 = self.add_canvas(0, Some(g), 0xD, 0xB, 0, 0x3E, 0, 0);
        self.lot().canvases[1] = n2;
        let notice = self.create_text(0, n2, Vec3::new(-13.2, 9.4, 17.2), 320.0, 32.0);
        let tx = self.game.sis.text_mut(notice);
        tx.font_size = [0.027, 0.028];
        tx.default_kerning = 1;
        if us {
            tx.x34 = [0.8, 0.8];
            tx.pos.x = -13.599_999;
            tx.pos.y = 9.599_999;
        }
        self.game.sis.reset_text(self.data, notice, 0x13C);
        let l = self.lot();
        l.notice = Some(notice);
        l.text_proc = None;
        l.text_frames = 0;
        l.wait = 8;
        self.game.sis.text_mut(notice).hidden = 1;
        let w = &mut self.game.world;
        w.setup_proc(g, CodeRef(PROC_INPUT), 0);
        w.setup_proc(g, CodeRef(PROC_TILT), 0);
        w.setup_proc(t, CodeRef(PROC_TILT), 0);
        w.setup_proc(g, CodeRef(PROC_SONG), 0);
        w.mark_ran(g);
    }

    /// `tyFigupon_Scene_OnEnter`: the lottery's four blocks, the table prepared (the save
    /// into it), class 3 opened in its extra categories, the gallery's table's `x0` 1,
    /// `TyMnFigp`, the cameras, the scene's lights (on link 0x34), the first light, the
    /// machine, the "new" sign (on link 0x3D, hidden, with a block), the bet (one coin if
    /// there are any), the new trophies left and the chance, and the pads renewed.
    #[doc(alias = "tyFigupon_Scene_OnEnter")]
    pub fn lot_on_enter(&mut self) {
        let alloc = |n: u32| crate::hsd::heap::mem_alloc(n);
        let data_block = alloc(0x34);
        let ef4_block = alloc(0x60);
        let entry_block = alloc(0x18);
        let light_block = alloc(0xE4);
        let user = std::mem::take(&mut self.game.toys.lottery.user);
        let canvases = self.game.toys.lottery.canvases;
        self.game.toys.lottery = Lottery {
            data_block,
            ef4_block,
            entry_block,
            canvases,
            user,
            ..Lottery::default()
        };
        self.gal_reset_lights_block(light_block);
        self.game.toys.stand = None;
        self.toy_scene_prepare();
        self.toy_mark_class(3, false, true);
        self.game.toys.table.x0 = 1;
        let name = if self.saved_us() {
            "TyMnFigp.usd"
        } else {
            "TyMnFigp.dat"
        };
        let a = self.toy_load(name);
        self.lot().archive = Some(a);
        self.lot().x58 = 0;
        self.lot_cameras();
        if let Some(list) = self.lot_sym("ScMenFigure_scene_lights") {
            let g = self.game.world.create(2, 3, 0);
            self.game.toys.gallery.scene_lights = Some(g);
            let (l, _) = self.toy_load_lights(list, false);
            let w = &mut self.game.world;
            if let Some(l) = l {
                w.set_obj(g, OBJ_LIGHT, Obj::LObj(l));
            }
            w.setup_gx_link(g, CodeRef(LOBJ_CALLBACK), 0x34, 0);
        }
        self.gal_reset_lights_block(light_block);
        self.toy_set_light(0);
        self.lot_machine();
        let nget = self.lot_sym("ToyFigurePonNget_Top_joint");
        let g = self.game.world.create(0xA, 0xA, 0);
        self.lot().new_sign = Some(g);
        let data = self.data;
        let j = jobj::load_joint(&mut self.game.objs, data, nget).expect("the new sign");
        self.game.world.set_obj(g, OBJ_JOBJ, Obj::JObj(j));
        self.lot_anims(
            Some(j),
            [
                Some("ToyFigurePonNget_Top_animjoint"),
                Some("ToyFigurePonNget_Top_matanim_joint"),
                Some("ToyFigurePonNget_Top_shapeanim_joint"),
            ],
        );
        jobj::req_anim_all(&mut self.game.objs, Some(j), 0.0);
        self.lot_anim_all(Some(j));
        jobj::set_srt(&mut self.game.objs.jobjs, j, |o| {
            o.translate.z = mem(fp::fadds(reg(o.translate.z), reg(0.5)));
        });
        self.lot_user_new(g, 4);
        self.game
            .world
            .setup_gx_link(g, CodeRef(JOBJ_CALLBACK), 0x3D, 0);
        jobj::set_flags_all(&mut self.game.objs.jobjs, Some(j), jobj::HIDDEN);
        let bet = i8::from(self.lot_coins() != 0);
        self.lot().bet = bet;
        self.lot_digits(i32::from(bet), 6, 2, 0);
        let rem = self.lot_remaining();
        self.lot().left = rem as i16;
        self.lot_chance();
        self.pads_renew_status();
    }

    /// `tyFigupon_Scene_OnFrame`: once the table says so, the lottery let go (`_tyFigupon_803181BC`:
    /// its pointers forgotten, the camera's processes removed, the new sign freed, the music
    /// stopped) and the scene ends.
    #[doc(alias = "tyFigupon_Scene_OnFrame")]
    #[doc(alias = "_tyFigupon_803181BC")]
    pub fn lot_on_frame(&mut self) {
        if self.game.toys.table.exit == 0 {
            return;
        }
        let l = self.lot();
        l.archive = None;
        self.game.toys.stand = None;
        self.game.toys.gallery.light_archive = None;
        let l = self.lot();
        l.panel = None;
        l.lever = None;
        l.chance = None;
        self.game.toys.gallery.scene_lights = None;
        self.game.toys.gallery.lights = None;
        if let Some(c) = self.game.toys.lottery.camera.take() {
            self.game.world.remove_all_procs(c);
        }
        self.lot().trophy_camera = None;
        if let Some(g) = self.game.toys.lottery.new_sign.take() {
            gobj::free(self, g);
        }
        self.lb_audio_music_stop();
        self.game.end_scene();
    }

    /// `onEnter` (`gmtoylottery.c`): the card's work areas and the card's archive (for the
    /// save the draws write).
    #[doc(alias = "onEnter@801BECA8")]
    pub fn lot_mode_enter(&mut self) {
        self.card_alloc_work_area();
        self.card_game_load_archive(0);
    }

    /// `onExit` (`gmtoylottery.c`): the special messages the lottery can unlock
    /// (`gm_80172898(4)`), then the main menu unless a challenger comes first.
    #[doc(alias = "onExit@801BECD0")]
    pub fn lot_mode_exit(&mut self) {
        self.special_unlocks(4);
        if !self.challenger_check(1, 0) {
            self.toy_mode_exit_to_menu();
        }
    }
}
