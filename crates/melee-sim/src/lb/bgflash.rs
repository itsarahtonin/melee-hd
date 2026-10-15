//! The background flash (`melee/lb/lbbgflash.c`, `lb_0219.c`): a screen-sized color drawn
//! over the stage by a camera of its own. Fades step its color toward a target each frame;
//! `LbBf.dat`'s color animations (a heavy hit's flash, say) set it through a color overlay
//! of the flash's GObj, scaled by the flash scale.
//!
//! The simulation keeps the flash's state (`lbl_80433658`), the overlay's GObj and user data,
//! the animation table and the scale. Drawing it is [`Draw::Quad`]s of the color.

use gekko_fp as fp;
use melee_data::archive::{ArchiveId, Archives, DataRef};

use crate::fpu::{mem, reg};
use crate::game::handles::CodeRef;
use crate::game::state::{Game, OBJ_CAMERA, Obj, Running, UserData};
use crate::game::structs::{_GXColor, BgFlashData, BgFlashUserData};
use crate::hsd::cobj::{self, StaticDesc};
use crate::hsd::display::{Draw, Quad};
use crate::hsd::gobj::GObjId;
use crate::lb::colanim;
use crate::vec::Vec3;

/// `fn_800204C8`, the flash GObj's process.
pub const PROC: u32 = 0x8002_04C8;
/// `fn_8001FEC4`, the flash GObj's render callback.
pub const RENDER: u32 = 0x8001_FEC4;
/// `lbBgFlash_Proc`, the overlay GObj's process.
pub const OVERLAY_PROC: u32 = 0x8002_1B04;
/// `lbBgFlash_Free`, the overlay GObj's user data's release.
pub const FREE: u32 = 0x8002_19E4;
/// The overlay GObj's user data kind.
pub const USER_DATA_KIND: u8 = 0xE;
/// `HSD_GObj_803910D8`, the flash camera's render callback.
const CAMERA_RENDER: u32 = 0x8039_10D8;

/// The fades' colors (`lbl_804D3840` to `lbl_804D384C`).
const BLACK: _GXColor = color(0, 0, 0, 255);
const CLEAR: _GXColor = color(0, 0, 0, 0);
const WHITE: _GXColor = color(255, 255, 255, 255);

/// The screen's width and height in the flash camera's units.
const WIDTH: i32 = 640;
const HEIGHT: i32 = 480;

const fn color(r: u8, g: u8, b: u8, a: u8) -> _GXColor {
    _GXColor { r, g, b, a }
}

/// `lbl_803BB028`: the flash's camera, looking at the screen's 640 by 480 units.
const CAMERA_DESC: StaticDesc = StaticDesc {
    flags: 0,
    projection_type: 1,
    viewport: [0, 640, 0, 480],
    scissor: [0, 640, 0, 480],
    eye: Vec3::new(320.0, -240.0, 415.692_2),
    interest: Vec3::new(320.0, -240.0, 0.0),
    roll: 0.0,
    near: 0.1,
    far: 30000.0,
    projection: [60.0, 1.333_333_3, 0.0, 0.0],
};

/// The flash's state.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct BgFlash {
    /// `lbl_80433658`: the mode, the colors and the fade's steps, the wipe's, and the GObjs.
    pub data: BgFlashData,
    /// `flash_gobj`: the GObj whose color overlay plays `LbBf.dat`'s animations.
    pub gobj: Option<GObjId>,
    /// Its user data (`BgFlashUserData`): the alpha the animations scale by and the overlay.
    pub user: Option<BgFlashUserData>,
    /// `lbl_804D63DC`: `LbBf.dat`'s `lbBgFlashColAnimData`, each animation's script and
    /// priority.
    pub table: Option<DataRef>,
    /// `flash_scale`: what the animations' colors are scaled by.
    pub scale: f32,
    /// Where the original's pool put the user data (`docs/engine.md`).
    pub user_addr: u32,
}

/// A color channel's byte as the fade's float holds it (`stb` of `fctiwz`).
fn to_u8(v: f32) -> u8 {
    fp::fctiwz(reg(v)) as u8
}

fn channels(c: &_GXColor) -> [u8; 4] {
    [c.r, c.g, c.b, c.a]
}

/// One step of the fade, `fn_8001FC08`: each channel moves by its step, stopping at the
/// target's.
#[doc(alias = "fn_8001FC08")]
pub fn fade_step(d: &mut BgFlashData) {
    let target = channels(&d.x8);
    for ((cur, step), target) in d.x10.iter_mut().zip(d.x20).zip(target) {
        let val = fp::fadds(reg(*cur), reg(step));
        let t = fp::frsp(f64::from(target));
        let past = if step > 0.0 { val >= t } else { val <= t };
        *cur = mem(if past { t } else { val });
    }
}

impl BgFlash {
    /// The flash GObj's process, `fn_800204C8`: modes 0 to 2 step the fade and take its
    /// color; modes 3 and 4 advance the wipe.
    #[doc(alias = "fn_800204C8")]
    pub fn proc(&mut self) {
        let d = &mut self.data;
        match d.state.mode {
            0..=2 => {
                fade_step(d);
                d.xC = color(
                    to_u8(d.x10[0]),
                    to_u8(d.x10[1]),
                    to_u8(d.x10[2]),
                    to_u8(d.x10[3]),
                );
            }
            3 | 4 if d.x30 == 0 => {
                for _ in 0..d.x3C {
                    if d.x34 < WIDTH {
                        d.x34 = d.x34.wrapping_add(i32::from(d.x31));
                    } else if d.x38 < HEIGHT {
                        d.x38 = d.x38.wrapping_add(i32::from(d.x32));
                        d.x34 = 0;
                    } else {
                        d.x33 = 1;
                        return;
                    }
                }
            }
            _ => {}
        }
    }

    /// Fades from `from` to `to` over `frames` frames (at least 1), `lbBgFlash_800206D4`.
    #[doc(alias = "lbBgFlash_800206D4")]
    pub fn fade(&mut self, from: _GXColor, to: _GXColor, frames: i32) {
        let count = fp::frsp(f64::from(frames.max(1)));
        let d = &mut self.data;
        d.state.active = false;
        d.state.mode = 2;
        d.x4 = from.clone();
        d.xC = d.x4.clone();
        d.x8 = to.clone();
        let (a, b) = (channels(&from), channels(&to));
        for i in 0..4 {
            d.x10[i] = f32::from(a[i]);
            let diff = i32::from(b[i]) - i32::from(a[i]);
            d.x20[i] = mem(fp::fdivs(fp::frsp(f64::from(diff)), count));
        }
    }

    /// Fades from white to black, `lbBgFlash_800205F0`.
    #[doc(alias = "lbBgFlash_800205F0")]
    pub fn fade_white_to_black(&mut self, frames: i32) {
        self.fade(WHITE, BLACK, frames.max(1));
        self.data.state.mode = 0;
    }

    /// Fades to black, `lbBgFlash_8002063C`.
    #[doc(alias = "lbBgFlash_8002063C")]
    pub fn fade_to_black(&mut self, frames: i32) {
        self.fade(CLEAR, BLACK, frames.max(1));
        self.data.state.mode = 0;
    }

    /// Fades to white, `lbBgFlash_80020688`.
    #[doc(alias = "lbBgFlash_80020688")]
    pub fn fade_to_white(&mut self, frames: i32) {
        self.fade(CLEAR, WHITE, frames.max(1));
        self.data.state.mode = 0;
    }

    /// Holds a color, `lbBgFlash_InitState`.
    #[doc(alias = "lbBgFlash_InitState")]
    pub fn set_color(&mut self, c: _GXColor) {
        self.data.state.active = false;
        self.data.state.mode = 5;
        self.data.xC = c;
    }

    /// Holds black at `alpha`, `fn_800208B0`.
    #[doc(alias = "fn_800208B0")]
    pub fn set_black(&mut self, alpha: u8) {
        self.set_color(color(0, 0, 0, alpha));
    }

    /// Sets the scale the animations' colors take, `lbBgFlash_SetFlashScale`.
    #[doc(alias = "lbBgFlash_SetFlashScale")]
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale;
    }

    /// The quads the flash draws, to `out`, `fn_8001FEC4`'s: none while off; its color over the
    /// screen in modes 0 to 2 and 5; a finished wipe black over it in mode 3; else the
    /// wipe's strips, downward from the top (mode 3) or from where it stands (mode 4).
    pub fn quads(&self, out: &mut dyn FnMut(Quad)) {
        let d = &self.data;
        if d.state.active {
            return;
        }
        let mode = d.state.mode;
        let quad = |x0: i32, y0: i32, x1: i32, y1: i32, c: [u8; 4]| Quad {
            x0: x0 as f32,
            y0: -y0 as f32,
            x1: x1 as f32,
            y1: -y1 as f32,
            color: c,
        };
        let black = channels(&BLACK);
        if !(3..5).contains(&mode) {
            out(quad(0, 0, WIDTH, HEIGHT, channels(&d.xC)));
            return;
        }
        if d.x33 != 0 {
            if mode == 3 {
                out(quad(0, 0, WIDTH, HEIGHT, black));
            }
            return;
        }
        if d.x30 != 0 {
            return;
        }
        let h = i32::from(d.x32);
        assert!(h != 0, "fn_8001FEC4: a wipe of strips 0 high never ends");
        if mode == 3 {
            let mut y = 0;
            while y <= d.x38 {
                let right = if y == d.x38 { d.x34 } else { WIDTH };
                out(quad(0, y, right, y + h, black));
                y += h;
            }
        } else {
            let mut y = d.x38;
            while y <= HEIGHT {
                let left = if y == d.x38 { d.x34 } else { 0 };
                out(quad(left, y, WIDTH, y + h, black));
                y += h;
            }
        }
    }
}

impl Game {
    /// Starts `LbBf.dat`'s color animation `id` on the flash's overlay unless one of higher
    /// priority plays, `lbBgFlash_80021C48` ([`colanim::start`]); `frames` limits it (0 for
    /// its script's length).
    #[doc(alias = "lbBgFlash_80021C48")]
    pub fn bg_flash_anim(&mut self, data: &Archives, id: u32, frames: u32) -> bool {
        let table = self.bgflash.table.expect("LbBf.dat's lbBgFlashColAnimData");
        let co = &mut self
            .bgflash
            .user
            .as_mut()
            .expect("the flash's user data")
            .x4;
        colanim::start(co, data, table, id as i32, frames as i32)
    }

    /// Stops the flash's overlay, `fn_80021C1C`.
    #[doc(alias = "fn_80021C1C")]
    pub fn bg_flash_reset(&mut self) {
        let u = self.bgflash.user.as_mut().expect("the flash's user data");
        colanim::clear(&mut u.x4);
    }

    /// The flash's user data released, `lbBgFlash_Free`.
    #[doc(alias = "lbBgFlash_Free")]
    pub fn bg_flash_free(&mut self) {
        self.bgflash.user = None;
        crate::hsd::heap::obj_free(crate::hsd::heap::Kind::BgFlash, self.bgflash.user_addr);
    }
}

impl Running<'_> {
    /// Makes the flash's camera GObj, drawing its render link 16 at `priority`, and the flash
    /// GObj, `lbBgFlash_800208EC` (`lbBgFlash_800209F4` with 10). The flash starts off.
    #[doc(alias = "lbBgFlash_800208EC")]
    #[doc(alias = "lbBgFlash_800209F4")]
    pub fn bg_flash_create(&mut self, priority: u8) {
        let g = &mut *self.game;
        let camera = g.world.create(0x14, 0x16, 0);
        g.bgflash.data.x44 = Some(camera);
        let c = cobj::load_static_desc(&mut g.objs, self.data, &CAMERA_DESC);
        g.world.set_obj(camera, OBJ_CAMERA, Obj::CObj(c));
        g.world
            .setup_gx_link_max(camera, CodeRef(CAMERA_RENDER), priority, false);
        g.world.gobj_mut(camera).gxlink_prios = 0x10000;
        let flash = g.world.create(0x15, 0x16, 2);
        g.bgflash.data.x40 = Some(flash);
        g.world
            .setup_gx_link(flash, CodeRef(RENDER), 0x10, priority);
        g.world.setup_proc(flash, CodeRef(PROC), 0);
        g.bgflash.data.state.active = true;
        g.bgflash.data.state.mode = 0;
    }

    /// Sets up the flash at a match's start, `lbBgFlash_Init`: the overlay's GObj, whose
    /// animations' alpha scales by `alpha`, `LbBf.dat`'s table, and the flash's GObjs.
    #[doc(alias = "lbBgFlash_Init")]
    pub fn bg_flash_init(&mut self, alpha: u8, lb_bf: ArchiveId) {
        let g = &mut *self.game;
        let gobj = g.world.create(0xE, 0xE, 0);
        g.world
            .init_user_data(gobj, USER_DATA_KIND, UserData::BgFlash);
        g.bgflash.gobj = Some(gobj);
        g.bgflash.scale = 1.0;
        g.bgflash.user = Some(BgFlashUserData {
            x0: alpha,
            ..BgFlashUserData::default()
        });
        g.bgflash.user_addr = crate::hsd::heap::obj_alloc(crate::hsd::heap::Kind::BgFlash);
        // `lbArchive_LoadSymbols("LbBf.dat", ...)`.
        self.disc_load("LbBf.dat", crate::game::ft::data::MAIN_MEMORY);
        let g = &mut *self.game;
        let offset = self
            .data
            .get(lb_bf)
            .public("lbBgFlashColAnimData")
            .expect("LbBf.dat's lbBgFlashColAnimData");
        g.bgflash.table = Some(DataRef {
            archive: lb_bf,
            offset,
        });
        self.bg_flash_create(6);
        self.game.bg_flash_reset();
        self.game.world.setup_proc(gobj, CodeRef(OVERLAY_PROC), 1);
    }

    /// The overlay GObj's process, `lbBgFlash_Proc`: the overlay runs, and while it colors,
    /// the flash holds its color scaled by the flash scale, its alpha by the user data's;
    /// once it stops, the flash clears.
    #[doc(alias = "lbBgFlash_Proc")]
    pub fn bg_flash_proc(&mut self) {
        let was = self.bg_flash_user().x4.x7C_color_enable;
        self.bg_flash_run();
        let u = self.bg_flash_user();
        if u.x4.x7C_color_enable {
            let scale = reg(self.game.bgflash.scale);
            let c = &u.x4.x2C_hex;
            let s = |v: u8| fp::fctiwz(fp::fmuls(fp::frsp(f64::from(v)), scale)) as u8;
            let a = (i32::from(c.a) * i32::from(u.x0) / 255) as u8;
            let c = color(s(c.r), s(c.g), s(c.b), a);
            self.game.bgflash.set_color(c);
        } else if was {
            self.game.bgflash.set_black(0);
        }
    }

    fn bg_flash_user(&self) -> &BgFlashUserData {
        self.game
            .bgflash
            .user
            .as_ref()
            .expect("the flash's user data")
    }

    /// Runs the overlay for the frame, `fn_80021C80`, stopping it each time it ends.
    #[doc(alias = "fn_80021C80")]
    fn bg_flash_run(&mut self) {
        let data = self.data;
        let co = &mut self.game.bgflash.user.as_mut().expect("user data").x4;
        // `fn_80021C18` takes the owner's commands and does nothing, so the script would
        // stay on one.
        while colanim::step(data, co, &mut |_, op| {
            panic!("LbBf.dat's color command {op:#x} never ends in the original")
        }) {
            colanim::clear(co);
        }
    }

    /// The flash GObj's render callback, `fn_8001FEC4`: the current camera's viewing matrix
    /// for the debug drawing (`hsd_80391A04`), then the flash's quads.
    #[doc(alias = "fn_8001FEC4")]
    pub fn render_bg_flash(&mut self) {
        if self.game.bgflash.data.state.active {
            return;
        }
        let c = self
            .game
            .objs
            .cobjs
            .current
            .expect("hsd_80391A04: no camera");
        cobj::viewing_mtx_copy(&mut self.game.objs, self.data, c);
        let g = &mut *self.game;
        let draws = &mut g.objs.display.draws;
        g.bgflash.quads(&mut |q| draws.push(Draw::Quad(q)));
    }
}
