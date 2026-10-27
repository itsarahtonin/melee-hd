//! The results screen's fighters (`gm_1798.c`): the stage they stand on (the dummy stage,
//! the effects, the items, the players reset), each player's fighter in its pose (a
//! winner's by the buttons held or a draw, a loser's clapping) and camera, which renders
//! the fighter into an image the panel shows (`lbl_8046E1B0`, `lbl_8046E3AC`), and the
//! winners' second camera that draws them big.

use gekko_fp as fp;

use super::super::handles::CodeRef;
use super::super::state::{OBJ_CAMERA, OBJ_LIGHT, Obj, Running};
use super::super::structs::{HSD_ImageDesc, MatchEnd};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;
use crate::vec::Vec3;

/// `lbl_8046E3AC_t`: the fighters' screen's state.
#[derive(Clone, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct DisplayState {
    /// `x0_0`, `x0_4`, `x0_6`.
    pub x0_0: u8,
    pub x0_4: u8,
    pub x0_6: u8,
    /// `player_flags`: a loser's image taken once its pose's frames ran out.
    pub player_flags: [u8; 4],
    pub x6: [u16; 4],
    /// The pose each player took (0 to 2 a winner's, 4 a loser's).
    pub variant: [u8; 4],
    pub char_kind: [i32; 4],
    /// The poses the enter data asks for, one more than the pose (0 none).
    pub costume_override: [u8; 4],
    /// The results with the places of a match with no contest made all second.
    pub match_end: MatchEnd,
    pub scissor_x: [u16; 4],
    pub scissor_y: [u16; 4],
    pub dim_w1: [u16; 4],
    pub dim_w2: [u16; 4],
    pub dim_h1: [u16; 4],
    pub dim_h2: [u16; 4],
    pub score_tbl: [[i16; 4]; 4],
    pub x22f4: [[i16; 4]; 4],
}

/// The fighters' screen's statics (`lbl_8046E1B0`, `lbl_8046E38C`, `lbl_8046E39C`,
/// `lbl_8046E3AC`).
#[derive(Clone, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct ResultsFighters {
    /// `player_img1`, `player_img2`, `shared_img`: the images the cameras copy into.
    pub player_img1: [HSD_ImageDesc; 4],
    pub player_img2: [HSD_ImageDesc; 4],
    pub shared_img: HSD_ImageDesc,
    /// `lbl_8046E38C`: each player's flame GObj.
    pub gobjs: [Option<GObjId>; 4],
    /// `lbl_8046E39C`: the panel's four columns' joints.
    pub jobjs: [Option<JObjId>; 4],
    pub state: DisplayState,
    /// Each image's pixels' block in the scene's heap and its size, by [`image_addr`]'s
    /// index: the GPU's copies write them ([`crate::hsd::image::Src::RenderTarget`]).
    pub buffers: [(u32, u32); 9],
}

/// The images the cameras copy into, by index: a player's first (0 to 3), its second (4 to
/// 7) and the shared one (8). An image descriptor's address in `lbl_8046E1B0`
/// (`player_img1` at 0x104, `player_img2` at 0x164, `shared_img` at 0x1C4), which the
/// panel's textures point to (`ImageRef::Static`).
pub const SHARED_IMAGE: u8 = 8;

/// `lbl_8046E1B0`'s address.
const DISPLAY_DATA: u32 = 0x8046_E1B0;

/// The address of image `n`'s descriptor ([`SHARED_IMAGE`]).
pub fn image_addr(n: u8) -> u32 {
    match n {
        0..=3 => DISPLAY_DATA + 0x104 + 0x18 * u32::from(n),
        4..=7 => DISPLAY_DATA + 0x164 + 0x18 * u32::from(n - 4),
        _ => DISPLAY_DATA + 0x1C4,
    }
}

impl ResultsFighters {
    /// Image `n`'s descriptor ([`SHARED_IMAGE`]).
    pub fn image(&self, n: u8) -> &HSD_ImageDesc {
        match n {
            0..=3 => &self.player_img1[usize::from(n)],
            4..=7 => &self.player_img2[usize::from(n - 4)],
            _ => &self.shared_img,
        }
    }

    /// The image whose descriptor is at `addr`, if one of the screen's.
    pub fn image_at(addr: u32) -> Option<u8> {
        (0..=SHARED_IMAGE).find(|&n| image_addr(n) == addr)
    }
}

/// `lbl_804D3FD0` to `lbl_804D3FF8`: the images' sizes and the copies' offsets, by place.
const DIM_W1: [u16; 4] = [0x50, 0x50, 0x46, 0x34];
const DIM_H1: [u16; 4] = [0x6E, 0x72, 0x64, 0x4A];
const DIM_W2: [u16; 4] = [0x34, 0x34, 0x34, 0x34];
const DIM_H2: [u16; 4] = [0x4A, 0x4A, 0x4A, 0x4A];
const SCISSOR_Y: [u16; 4] = [0x0C, 0x08, 0x06, 0x00];
const SCISSOR_X: [u16; 4] = [0x0E, 0x0E, 0x06, 0x00];
/// `gmResultScoreTableInit`, `gmResultX22F4Init`: the winners' cameras' sideways steps, by
/// how many won.
const SCORE_TBL: [[i16; 4]; 4] = [
    [0, 0, 0, 0],
    [-14, 14, 0, 0],
    [-18, 0, 18, 0],
    [-22, -7, 7, 22],
];
const X22F4: [[i16; 4]; 4] = [
    [0x18, 0, 0, 0],
    [0x15, 0x15, 0, 0],
    [0x12, 0x12, 0x12, 0],
    [0xE, 0xE, 0xE, 0xE],
];

/// `GX_TF_RGB5A3`, the images' format.
const GX_TF_RGB5A3: i32 = 5;

/// The cameras' render callbacks: a player's (`fn_80179D3C` to `fn_80179DA8`) and a
/// winner's second (`fn_80179DCC` to `fn_80179F04`).
pub const PLAYER_RENDER: [u32; 4] = [0x8017_9D3C, 0x8017_9D60, 0x8017_9D84, 0x8017_9DA8];
pub const WINNER_RENDER: [u32; 4] = [0x8017_9DCC, 0x8017_9E34, 0x8017_9E9C, 0x8017_9F04];

/// `lbl_803B7B68`'s eyes and interests: the players' cameras' (`x4C`, `x58`) and the
/// winners' second (`x24`, `x30`), the fighters' spots (`x74`, `x80`), and the losers'
/// scales by place (`x8C`).
const PLAYER_EYE: Vec3 = Vec3::new(0.0, 100.0, 62.0);
const PLAYER_INTEREST: Vec3 = Vec3::new(0.0, 100.0, 0.0);
const WINNER_EYE: Vec3 = Vec3::new(0.0, 100.0, 62.0);
const WINNER_INTEREST: Vec3 = Vec3::new(0.0, 100.0, 0.0);
const LOSER_SPOT: Vec3 = Vec3::new(0.0, 0.0, -100.0);
const WINNER_SPOT: Vec3 = Vec3::new(0.0, 0.0, -100.0);
const LOSER_SCALES: [f32; 4] = [0.75, 0.48, 0.4, 0.307];

/// `scissor_init$352`: the players' cameras' scissor.
const PLAYER_SCISSOR: [u16; 4] = [270, 370, 124, 276];

/// `gmResultCameraDesc` (with `gmResultCameraEyeDesc` and `gmResultCameraInterestDesc`):
/// the winners' second camera.
const WINNER_CAMERA: crate::hsd::cobj::StaticDesc = crate::hsd::cobj::StaticDesc {
    flags: 0,
    projection_type: 1,
    viewport: [0, 640, 0, 480],
    scissor: [0, 640, 0, 480],
    eye: Vec3::new(0.0, 0.0, 62.0),
    interest: Vec3::new(0.0, 0.0, 0.0),
    roll: 0.0,
    near: 1.0,
    far: 5000.0,
    projection: [19.999998, 1.216667, 0.0, 0.0],
};

include!("results_tables.rs");

/// A player's place (0 the winner): its own, or its team's in teams.
pub(super) fn place(me: &MatchEnd, slot: usize) -> i32 {
    let p = &me.player_standings[slot];
    if me.is_teams == 0 {
        i32::from(p.is_big_loser)
    } else {
        i32::from(me.team_standings[usize::from(p.team)].is_big_loser)
    }
}

/// A pose's index into the per-pose tables, the losers' (and past) the fourth.
fn pose_index(variant: u8) -> usize {
    if variant <= 2 {
        usize::from(variant)
    } else {
        3
    }
}

impl Running<'_> {
    /// How many won, less one, `fn_801795D4` (on the screen's copy of the results).
    #[doc(alias = "fn_801795D4")]
    pub fn results_winner_count(&self) -> usize {
        let me = &self.game.results.fighters.state.match_end;
        let n = (0..4)
            .filter(|&i| me.player_standings[i].pkind != 3 && place(me, i) == 0)
            .count();
        n.saturating_sub(1)
    }

    /// Which winner a player is (counting the winners before it), `fn_801796F0`.
    #[doc(alias = "fn_801796F0")]
    pub fn results_winner_index(&self, slot: usize) -> usize {
        let me = &self.game.results.fighters.state.match_end;
        let mut n = 0;
        for i in 0..4 {
            if me.player_standings[i].pkind != 3 && place(me, i) == 0 {
                if i == slot {
                    return n;
                }
                n += 1;
            }
        }
        n
    }

    /// The fighters' stage, `fn_8017AA78`: the background flash, the camera, the dynamics'
    /// links, the collisions, the dummy stage, the items, the effects, the fighters' pools,
    /// the players reset, the shared image, the screen's copy of the results (every player
    /// second after a match with no contest), and the images' sizes.
    #[doc(alias = "fn_8017AA78")]
    #[doc(alias = "ftDemo_ObjAllocInit")]
    #[doc(alias = "Item_80266FA8")]
    #[doc(alias = "it_8027870C")]
    pub fn results_fighters_setup(&mut self, costumes: [u8; 4]) {
        self.bg_flash_create(6);
        super::super::camera::init(&mut self.game.camera, 8);
        self.game.alloc_dynamics(self.data);
        self.game.coll.reset();
        super::super::gr::setup::prepare(self.game);
        super::super::gr::setup::select(self, 0, None);
        let grkind = self.game.stage_info.grkind;
        super::super::gr::setup::setup(self, grkind);
        let kept = self.take_cached_named("ItCo.usd");
        let it_co = self.game.ft.data.files.archive("ItCo.usd");
        super::super::it::item::load_common_data(self.game, self.data, it_co, kept);
        super::super::it::item::init_globals(self.game, self.data);
        super::super::ef::eflib::init(self);
        super::super::ef::load_sync(self, 0);
        self.fighter_demo_init();
        self.game.init_all_players();
        let f = &mut self.game.results.fighters;
        f.shared_img = HSD_ImageDesc::default();
        self.results_image_alloc(Image::Shared, 0x64, 0x98);
        let me = self.results_end().clone();
        let f = &mut self.game.results.fighters;
        f.state.match_end = me;
        f.state.x0_0 = 1;
        f.state.x0_4 = 0;
        f.state.x0_6 = 0;
        f.state.dim_w1 = DIM_W1;
        f.state.dim_h1 = DIM_H1;
        f.state.dim_w2 = DIM_W2;
        f.state.dim_h2 = DIM_H2;
        f.state.scissor_y = SCISSOR_Y;
        f.state.scissor_x = SCISSOR_X;
        for i in 0..4 {
            let s = &mut f.state;
            s.player_flags[i] = 0;
            s.costume_override[i] = costumes[i];
            if s.match_end.outcome == super::match_exit::OUTCOME_NO_CONTEST {
                s.match_end.player_standings[i].is_big_loser = 1;
                let team = usize::from(s.match_end.player_standings[i].team);
                s.match_end.team_standings[team].is_big_loser = 1;
            }
            s.x6[i] = 0;
            s.score_tbl[i] = SCORE_TBL[i];
            s.x22f4[i] = X22F4[i];
        }
    }

    /// The fighters' light (the panel scene's lights again), drawn on link 0, `fn_8017A004`.
    #[doc(alias = "fn_8017A004")]
    pub fn results_fighters_light(&mut self) {
        let pnl = self.game.results.data.pnlsce.expect("the panel's scene");
        let lights = self.data.ptr(pnl.at(8)).expect("the panel's lights");
        let g = &mut *self.game;
        let gobj = g.world.create(0xB, 3, 0);
        let l = super::super::gr::setup::load_light_list(g, self.data, lights);
        g.world
            .set_obj(gobj, OBJ_LIGHT, Obj::LObj(l.expect("a light")));
        g.world
            .setup_gx_link(gobj, CodeRef(super::super::render::LOBJ_CALLBACK), 0, 0);
    }

    /// An image's pixels from the scene's heap (`lb_800121FC` with no preload entry), which
    /// only the GPU's copies write: the block keeps what it held until the first.
    fn results_image_alloc(&mut self, which: Image, w: u16, h: u16) {
        let size = crate::hsd::heap::tex_buffer_size(u32::from(w), u32::from(h), 5);
        let size = size.next_multiple_of(32);
        let at = crate::hsd::heap::mem_alloc_unclaimed(size);
        let d = super::super::gr::stadium::screen_image(w, h, GX_TF_RGB5A3);
        let f = &mut self.game.results.fighters;
        let n = match which {
            Image::Shared => {
                f.shared_img = d;
                SHARED_IMAGE
            }
            Image::One(i) => {
                f.player_img1[i] = d;
                i as u8
            }
            Image::Two(i) => {
                f.player_img2[i] = d;
                4 + i as u8
            }
        };
        f.buffers[usize::from(n)] = (at, size);
    }

    /// `HSD_ImageDescCopyFromEFB` into image `n` from (`x`, `y`), the frame cleared after it
    /// when `clear`: a draw, and the GPU's pixels in the image's block, which the simulation
    /// never reads.
    fn results_copy(&mut self, n: u8, x: u16, y: u16, clear: bool) {
        self.game
            .objs
            .display
            .draws
            .push(crate::hsd::display::Draw::ResultsCopy {
                image: n,
                x,
                y,
                clear,
            });
        let (at, size) = self.game.results.fighters.buffers[usize::from(n)];
        if at != 0 {
            crate::hsd::heap::with(|h| {
                h.image
                    .hold(at, at + size, crate::hsd::image::Src::RenderTarget)
            });
        }
    }

    /// The erase color a player's image takes, its HUD color (`gm_80160968` of
    /// `gm_80160854`), `HSD_SetEraseColor`.
    fn results_erase_color(&mut self, slot: usize) {
        let me = &self.game.results.fighters.state.match_end;
        let p = &me.player_standings[slot];
        let c = super::vs::hud_color(super::vs::player_color(
            slot as u8,
            p.team,
            u8::from(me.is_teams == 1),
            p.pkind,
        ));
        self.game
            .objs
            .display
            .draws
            .push(crate::hsd::display::Draw::EraseColor(c));
    }

    /// A player's camera's render callback, `fn_80179990` (through `fn_80179D3C` to
    /// `fn_80179DA8`, one a player): a loser's fighter rendered every frame onto its HUD
    /// color and copied into its second image (and its first, before the panel's columns
    /// come in), the frame erased and the shared image copied with a clear; the flame's
    /// texture shows the first image (before the columns), the column's the second. A
    /// winner's is taken once, when its pose's frames have run out after the flames went
    /// away, into its second image, which its column shows. A winner's camera stays current
    /// when it takes nothing (its C ends the camera only after a copy).
    #[doc(alias = "fn_80179990")]
    #[doc(alias = "fn_80179D3C")]
    #[doc(alias = "fn_80179D60")]
    #[doc(alias = "fn_80179D84")]
    #[doc(alias = "fn_80179DA8")]
    pub fn results_player_render(&mut self, gobj: GObjId, slot: usize) {
        use crate::hsd::display::{self, Draw};
        let _ = self.results_winner_count();
        let _ = self.results_winner_index(slot);
        let c = match self.game.world.gobj(gobj).obj {
            Some(Obj::CObj(c)) => Some(c),
            _ => None,
        };
        let f = &self.game.results.fighters;
        let k = place(&f.state.match_end, slot) as usize;
        // The flame's model's first child, a loser's.
        let child = if k != 0 {
            match f.gobjs[slot].and_then(|g| self.game.world.gobj(g).obj) {
                Some(Obj::JObj(j)) => self.game.objs.jobjs[j].child,
                _ => None,
            }
        } else {
            None
        };
        if !display::camera_begin(&mut self.game.objs, self.data, c) {
            return;
        }
        let s = &self.game.results.fighters.state;
        let (w, h, sx, sy) = (s.dim_w1[k], s.dim_h1[k], s.scissor_x[k], s.scissor_y[k]);
        let x = 0x140 - (w / 4) * 2;
        let y = 0xF4 - (h / 2) * 2;
        let second = 4 + slot as u8;
        if k != 0 {
            let columns = s.x0_4 != 0;
            self.results_erase_color(slot);
            self.game.objs.display.draws.push(Draw::Erase);
            self.camera_fighters(gobj, 0);
            self.results_copy(second, sx + x, sy + y, false);
            if !columns {
                self.results_copy(slot as u8, x, y, false);
            }
            self.game.objs.display.draws.push(Draw::EraseFull);
            self.results_copy(SHARED_IMAGE, 0x10E, 0x7C, true);
            display::camera_end(&mut self.game.objs);
            if !columns {
                let child = child.expect("the original's flame has a child");
                self.results_set_texture(child, false, slot as u8);
            } else {
                let col = self.game.results.fighters.jobjs[slot].expect("a column");
                self.results_set_texture(col, true, second);
            }
        } else {
            let fighter = self.game.player_entity(slot as i32);
            let done = fighter.is_some_and(|g| !self.anim_frames_remaining(g));
            let s = &self.game.results.fighters.state;
            if done && s.player_flags[slot] == 0 && s.x0_6 != 0 {
                self.results_erase_color(slot);
                self.game.objs.display.draws.push(Draw::Erase);
                self.camera_fighters(gobj, 0);
                self.results_copy(second, sx + x, sy + y, false);
                self.game.objs.display.draws.push(Draw::EraseFull);
                self.results_copy(SHARED_IMAGE, 0x10E, 0x7C, true);
                display::camera_end(&mut self.game.objs);
                self.game.results.fighters.state.player_flags[slot] = 1;
                let col = self.game.results.fighters.jobjs[slot].expect("a column");
                self.results_set_texture(col, true, second);
            }
        }
    }

    /// The texture of a joint's display object (`u.dobj`, or its next when `next`) shows
    /// image `n` (its material's first texture's `imagedesc`).
    fn results_set_texture(&mut self, j: JObjId, next: bool, n: u8) {
        let objs = &mut self.game.objs;
        let d = objs.jobjs[j].dobj().expect("a joint with a display object");
        let d = if next {
            objs.dobjs[d].next.expect("a second display object")
        } else {
            d
        };
        let m = objs.dobjs[d].mobj.expect("a material");
        let t = objs.mobjs[m].tobj.expect("a texture");
        objs.tobjs[t].imagedesc = Some(crate::hsd::tobj::ImageRef::Static(image_addr(n)));
    }

    /// A winner's second camera's render callback, `fn_80179DCC` (to `fn_80179F04`, one a
    /// player): the fighters' passes, then render link 7's three passes (the panel).
    #[doc(alias = "fn_80179DCC")]
    #[doc(alias = "fn_80179E34")]
    #[doc(alias = "fn_80179E9C")]
    #[doc(alias = "fn_80179F04")]
    pub fn results_winner_render(&mut self, gobj: GObjId) {
        use crate::hsd::display;
        let c = match self.game.world.gobj(gobj).obj {
            Some(Obj::CObj(c)) => Some(c),
            _ => None,
        };
        if display::camera_begin(&mut self.game.objs, self.data, c) {
            self.camera_fighters(gobj, 0);
            self.game.world.gobj_mut(gobj).gxlink_prios = 0x80;
            self.render_links(gobj, 7);
            display::camera_end(&mut self.game.objs);
        }
    }

    /// A player's two images, of its place's sizes, `fn_8017A9B4`.
    #[doc(alias = "fn_8017A9B4")]
    pub fn results_images(&mut self, slot: usize) {
        let s = &self.game.results.fighters.state;
        let k = place(&s.match_end, slot) as usize;
        let (w1, h1, w2, h2) = (s.dim_w1[k], s.dim_h1[k], s.dim_w2[k], s.dim_h2[k]);
        self.results_image_alloc(Image::One(slot), w1, h1);
        self.results_image_alloc(Image::Two(slot), w2, h2);
    }

    /// A player's fighter on the screen, `fn_8017A67C`: its character's results file
    /// loaded (Zelda or Sheik as it ended the match), the slot set up as a demo player
    /// facing the camera (Mr. Game & Watch to the side), its pose (a winner's by B, Y or X
    /// held, else drawn; a loser's), then its fighter made at its spot and scaled: big for a
    /// winner, by place for a loser. None for a character without a results file.
    #[doc(alias = "fn_8017A67C")]
    #[doc(alias = "gm_80160438")]
    pub fn results_fighter(&mut self, kind: i32, costume: i32, slot: i32) -> Option<GObjId> {
        let s = &self.game.results.fighters.state;
        let k = place(&s.match_end, slot as usize);
        let file = super::results_stats::results_file(kind)?;
        let mut kind = kind;
        if (kind - 18) as u32 <= 1 {
            kind = if s.match_end.player_standings[slot as usize].ftkind == 7 {
                0x13
            } else {
                0x12
            };
        }
        let file = super::results_stats::results_file(kind).unwrap_or(file);
        let (a, block, _) = self.load_archive_blocks(file);
        self.player_set_demo_archive(kind, a, block);
        let g = &mut *self.game;
        g.set_player_ckind(slot, kind);
        g.set_player_costume(slot, costume);
        g.set_player_port(slot, slot);
        g.set_player_pkind(slot, super::super::pl::player::PKIND_DEMO);
        g.set_player_facing(slot, if kind == 3 { -1.0 } else { 0.0 });
        let mut variant: i32 = if k == 0 {
            let b = self.game.pads.copy[slot as usize].button;
            if b & 0x200 != 0 {
                0
            } else if b & 0x800 != 0 {
                1
            } else if b & 0x400 != 0 {
                2
            } else {
                self.game.objs.rng.below(3)
            }
        } else {
            4
        };
        if variant != 4 {
            let o = self.game.results.fighters.state.costume_override[slot as usize];
            if o != 0 {
                variant = i32::from(o) - 1;
            }
        }
        let y = mem(fp::fmuls(100.0, fp::frsp(f64::from(slot + 1))));
        if k == 0 {
            let mut pos = WINNER_SPOT;
            pos.y = y;
            self.player_set_position(slot, pos);
            let scale = mem(fp::fmuls(f64::from(1.8_f32), reg(WIN_SCALE[kind as usize])));
            self.player_set_scale(slot, scale);
            self.player_create_demo(slot, variant);
        } else {
            let z = CAMERA_PARAMS[kind as usize].1[pose_index(variant as u8)];
            self.player_create_demo(slot, variant);
            let h = self.player_nametag_height(slot);
            let per = fp::fmuls(reg(LOSER_SCALES[k as usize]), fp::fdivs(20.0, h));
            self.player_set_scale(slot, mem(fp::fmuls(reg(z), per)));
            let mut pos = LOSER_SPOT;
            pos.y = y;
            self.player_set_position(slot, pos);
        }
        let s = &mut self.game.results.fighters.state;
        s.variant[slot as usize] = variant as u8;
        s.char_kind[slot as usize] = kind;
        self.game.player_entity(slot)
    }

    /// A player's camera onto its fighter, rendering into its images, `fn_8017A318`: the
    /// camera's eye and interest offset by the fighter's place and pose, nearer for a
    /// winner, the scissor; a winner's second camera after (`fn_8017A078`). Returns what the
    /// original leaves in r3, its C returning nothing: the last camera GObj it gave a
    /// render link.
    #[doc(alias = "fn_8017A318")]
    pub fn results_fighter_camera(&mut self, slot: i32) -> Option<GObjId> {
        let _ = self.results_winner_count();
        let _ = self.results_winner_index(slot as usize);
        let s = &self.game.results.fighters.state;
        let k = place(&s.match_end, slot as usize);
        let kind = s.char_kind[slot as usize] as usize;
        let vi = pose_index(s.variant[slot as usize]);
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(0x13, 0x14, 0);
        let c = crate::hsd::cobj::load_static_desc(&mut g.objs, data, &PLAYER_CAMERA);
        g.world.set_obj(gobj, OBJ_CAMERA, Obj::CObj(c));
        let n = fp::frsp(f64::from(slot + 1));
        let mut eye = PLAYER_EYE;
        let mut interest = PLAYER_INTEREST;
        eye.y = mem(fp::fmuls(reg(eye.y), n));
        interest.y = mem(fp::fmuls(reg(interest.y), n));
        let (offs, scales) = CAMERA_PARAMS[kind];
        eye.y = mem(fp::fadds(reg(eye.y), reg(offs[4 + vi])));
        interest.y = mem(fp::fadds(reg(interest.y), reg(offs[4 + vi])));
        eye.x = mem(fp::fadds(reg(eye.x), reg(offs[vi])));
        interest.x = mem(fp::fadds(reg(interest.x), reg(offs[vi])));
        let po = &PLACE_OFFSETS[kind];
        let (dx, dy) = (reg(po[0][k as usize]), reg(po[1][k as usize]));
        eye.x = mem(fp::fadds(reg(eye.x), dx));
        interest.x = mem(fp::fadds(reg(interest.x), dx));
        eye.y = mem(fp::fadds(reg(eye.y), dy));
        interest.y = mem(fp::fadds(reg(interest.y), dy));
        if k == 0 {
            eye.z = mem(fp::fadds(reg(eye.z), 320.0));
        }
        let near = fp::fsubs(1.0, reg(scales[vi]));
        let step = if near < 0.0 { 100.0 } else { 300.0 };
        eye.z = mem(fp::fmadds(step, near, reg(eye.z)));
        let objs = &mut self.game.objs;
        crate::hsd::cobj::set_eye_position(objs, c, eye);
        crate::hsd::cobj::set_interest(objs, c, interest);
        crate::hsd::cobj::set_scissor(&mut objs.cobjs, Some(c), PLAYER_SCISSOR);
        let g = &mut *self.game;
        g.world
            .setup_gx_link_max(gobj, CodeRef(PLAYER_RENDER[slot as usize]), 0, false);
        if k == 0 {
            return Some(self.results_winner_camera(slot));
        }
        Some(gobj)
    }

    /// A winner's second camera, drawing it big beside the others, `fn_8017A078`: the eye
    /// and interest raised by the slot and the fighter's height, stepped sideways by which
    /// winner it is, nearer with fewer winners (and a little for three poses that reach
    /// out).
    #[doc(alias = "fn_8017A078")]
    fn results_winner_camera(&mut self, slot: i32) -> GObjId {
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(0x13, 0x14, 0);
        let c = crate::hsd::cobj::load_static_desc(&mut g.objs, data, &WINNER_CAMERA);
        g.world.set_obj(gobj, OBJ_CAMERA, Obj::CObj(c));
        let n = fp::frsp(f64::from(slot + 1));
        let mut eye = WINNER_EYE;
        let mut interest = WINNER_INTEREST;
        let h = self.player_nametag_height(slot);
        eye.y = mem(fp::fmadds(reg(eye.y), n, fp::fmuls(f64::from(0.7_f32), h)));
        let h = self.player_nametag_height(slot);
        interest.y = mem(fp::fmadds(
            reg(interest.y),
            n,
            fp::fmuls(f64::from(0.7_f32), h),
        ));
        let mode = self.results_winner_count();
        let idx = self.results_winner_index(slot as usize);
        let s = &self.game.results.fighters.state;
        let val = fp::frsp(f64::from(s.score_tbl[mode][idx]));
        interest.x = mem(fp::fsubs(reg(interest.x), val));
        eye.x = mem(fp::fsubs(reg(eye.x), val));
        interest.y = mem(fp::fsubs(reg(interest.y), 10.0));
        eye.y = mem(fp::fsubs(reg(eye.y), 10.0));
        let m = fp::frsp(f64::from(mode as i32 - 2));
        eye.z = mem(fp::fadds(reg(eye.z), fp::fmadds(20.0, m, 110.0)));
        if mode == 0 {
            let kind = s.char_kind[slot as usize];
            let v = s.variant[slot as usize];
            let add = match (kind, v) {
                (5, 1) => Some(6.0),
                (9, 1) => Some(7.5),
                (0, 2) => Some(6.0),
                _ => None,
            };
            if let Some(a) = add {
                eye.z = mem(fp::fadds(reg(eye.z), a));
            }
        }
        let objs = &mut self.game.objs;
        crate::hsd::cobj::set_eye_position(objs, c, eye);
        crate::hsd::cobj::set_interest(objs, c, interest);
        self.game
            .world
            .setup_gx_link_max(gobj, CodeRef(WINNER_RENDER[slot as usize]), 5, false);
        gobj
    }

    /// The players' columns' four joints, under the panel's first (`fn_80179F84`).
    #[doc(alias = "fn_80179F84")]
    pub fn results_columns(&mut self, j: Option<JObjId>) {
        let mut child = j.and_then(|j| self.game.objs.jobjs[j].child);
        for i in 0..4 {
            self.game.results.fighters.jobjs[i] = child;
            child = child.and_then(|c| self.game.objs.jobjs[c].next);
        }
        self.game.results.fighters.state.x0_4 = 1;
    }

    /// The losers' flames hidden off to the left, and the background fading to dark, as
    /// the panel comes in, `fn_80179854`.
    #[doc(alias = "fn_80179854")]
    pub fn results_flames_away(&mut self) {
        self.game.bgflash.fade(
            super::super::structs::_GXColor {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            },
            super::super::structs::_GXColor {
                r: 0,
                g: 0,
                b: 0,
                a: 0x3C,
            },
            0x1E,
        );
        for i in 0..4 {
            let s = &self.game.results.fighters.state;
            let me = &s.match_end;
            if me.player_standings[i].pkind != 3 && place(me, i) != 0 {
                let gobj = self.game.results.fighters.gobjs[i];
                if let Some(Obj::JObj(j)) = gobj.and_then(|g| self.game.world.gobj(g).obj) {
                    let mut t = self.game.objs.jobjs[j].translate;
                    t.x = -300.0;
                    crate::hsd::jobj::set_translate(&mut self.game.objs.jobjs, j, t);
                }
                self.game.results.fighters.state.x0_6 = 1;
            }
        }
    }
}

/// Which of the screen's images.
#[derive(Clone, Copy)]
enum Image {
    Shared,
    One(usize),
    Two(usize),
}

/// The players' cameras' descriptor (`CameraKindData::cobj_desc`, `gmResultCameraDesc`'s
/// place in the tables the code reads it through): the same as the winners'.
const PLAYER_CAMERA: crate::hsd::cobj::StaticDesc = WINNER_CAMERA;
