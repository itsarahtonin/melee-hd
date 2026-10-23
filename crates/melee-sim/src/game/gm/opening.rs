//! The opening movie's scene (`melee/gm/gmopening.c`): the movie (`MvOpen.mth`) through the
//! movie player and its sprite, its song, the title's logo flashed early and shown over the
//! movie's end, and the mode's exits: Start or A skips to the title, Start once the logo is
//! up goes to the main menu, and the movie left to run ends in the attract loop's match.
//!
//! Also the opening mode's attract demo (`gmopeningmode.c`): which four characters, costumes
//! and stage the attract match picks ([`Running::setup_title_demo`]), and their files
//! preloaded.

use super::super::handles::CodeRef;
use super::super::state::{OBJ_CAMERA, Obj, Running};
use super::machine::{GM_MENU, GM_TITLE};
use crate::hsd::cobj;
use crate::hsd::gobj::GObjId;
use crate::hsd::sobj::{self, OBJ_SOBJ};
use crate::vec::Vec3;

/// `gm_Scene_Opening_OnEnter` and `gm_Scene_Opening_OnFrame`.
pub const ON_ENTER: CodeRef = CodeRef(0x801A_A110);
pub const ON_FRAME: CodeRef = CodeRef(0x801A_A28C);
/// `fn_801AA0E8`, the debug performance display the scene hooks (`gm_804D6724`): only a
/// debug level's dump runs it.
pub const PERF_DISPLAY: u32 = 0x801A_A0E8;
/// `gm_803DBFB4`, the opening's rate table: 1,250 frames at 30 per second, 394 at 60, then 30.
pub const RATE_TABLE: u32 = 0x803D_BFB4;
const RATE_RUNS: [(u32, u32); 3] = [(1250, 2), (394, 1), (65536, 2)];

/// The movie's file.
pub const MOVIE: &str = "MvOpen.mth";

/// `gmopening.c`'s statics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Opening {
    /// `gm_804D67D0`: the logo is up; `gm_804D67E4`: the tick the scene ends at.
    pub logo_shown: bool,
    pub end_at: u32,
    /// `gm_804D67D4`: the title mark's sprite; `gm_804D67D8`: the early logo.
    pub mark: Option<GObjId>,
    pub flash: Option<GObjId>,
    /// `gm_804D67DC`: the attract demo's files are preloading.
    pub preloaded: bool,
    /// `gm_804D67E0`, `gm_804D67E1`: the narrator's lines played; `gm_804D67E2`: the video
    /// mode set again.
    pub line0: bool,
    pub line1: bool,
    pub video_reset: bool,
    /// `gm_804D67E8`: the movie's sprite; `gm_804D67F4`: its camera.
    pub movie: Option<GObjId>,
    pub camera: Option<GObjId>,
    /// `gm_804D67EC`: the movie's clock as the scene follows it, counting on by itself past
    /// 5,400.
    pub clock: u32,
    /// `gm_804D6724`, the scene loop's debug display hook: set to [`PERF_DISPLAY`].
    pub perf_display: bool,
}

impl Running<'_> {
    /// `gm_801A9DD0`: an orthographic camera over `w` by `h` pixels (its left and right
    /// narrowed to the middle 584 for `narrow`), drawing sprites on its links
    /// (`HSD_SObjLib_803A54EC`) with render priority `prio`.
    #[doc(alias = "gm_801A9DD0")]
    pub fn sprite_camera(&mut self, gobj: GObjId, w: u16, h: u16, prio: u8, narrow: bool) {
        let data = self.data;
        let (mut left, mut right) = (0.0f32, f32::from(w));
        if narrow {
            let l = (i32::from(w) - 0x248) / 2;
            left = l as f32;
            right = (l + 0x248) as f32;
        }
        let bottom = -(i32::from(h)) as f32;
        let o = &mut self.game.objs;
        let c = cobj::alloc(o);
        cobj::set_projection_type(&mut o.cobjs, Some(c), 3);
        cobj::set_viewport(&mut o.cobjs, Some(c), [0, w as i16, 0, h as i16]);
        cobj::set_scissor(&mut o.cobjs, Some(c), [0, w, 0, h]);
        cobj::set_eye_position(o, c, Vec3::new(0.0, 0.0, 1.0));
        cobj::set_interest(o, c, Vec3::new(0.0, 0.0, 0.0));
        cobj::set_roll(o, data, Some(c), 0.0);
        cobj::set_near(&mut o.cobjs, Some(c), 0.0);
        cobj::set_far(&mut o.cobjs, Some(c), 2.0);
        cobj::set_ortho(&mut o.cobjs, Some(c), 0.0, bottom, left, right);
        let w = &mut self.game.world;
        w.set_obj(gobj, OBJ_CAMERA, Obj::CObj(c));
        w.setup_gx_link_max(gobj, CodeRef(sobj::CAMERA_RENDER), prio, false);
    }

    /// `gm_Scene_Opening_OnEnter`: the title's archive, the scene's banks, the video mode
    /// without deflicker, the title's fog, light and cameras, the sprites' cameras, the
    /// movie's sprite, its song, and the movie opened.
    #[doc(alias = "gm_Scene_Opening_OnEnter")]
    #[doc(alias = "lbMthp_8001F624")]
    pub fn opening_on_enter(&mut self) {
        self.game.opening = Opening {
            perf_display: self.game.opening.perf_display,
            movie: self.game.opening.movie,
            camera: self.game.opening.camera,
            end_at: self.game.opening.end_at,
            ..Opening::default()
        };
        self.title_load();
        let a = &mut self.game.audio;
        a.lb.unwant_banks(0x12);
        a.lb.want_banks(2, 4);
        a.lb_start_bank_loads(self.data);
        self.set_video_mode(false);
        self.title_fog();
        self.title_lights();
        self.title_erase_camera();
        let cam = self.game.world.create(0x13, 0x14, 0);
        self.game.opening.camera = Some(cam);
        self.sprite_camera(cam, 0x280, 0x1E0, 8, false);
        self.game.world.gobj_mut(cam).gxlink_prios = 0x800;
        let cam2 = self.game.world.create(0x13, 0x14, 0);
        self.sprite_camera(cam2, 0x280, 0x1E0, 0xB, true);
        self.game.world.gobj_mut(cam2).gxlink_prios = 0x2_0000;
        self.title_camera();
        let g = self.game.world.create(0xE, 0xF, 0);
        self.game.opening.movie = Some(g);
        self.game
            .world
            .setup_gx_link(g, CodeRef(crate::lb::mthp::RENDER), 0xB, 0);
        // `lbMthp_8001F624`: a 640 by 480 sprite drawing the planes.
        self.game.world.set_obj(g, OBJ_SOBJ, Obj::SObj(None));
        let (s, head) = sobj::create(
            &mut self.game.objs.sobjs,
            self.data,
            g,
            None,
            sobj::Image::Movie {
                width: 640,
                height: 480,
            },
            (0, 0),
            0x80,
            None,
        );
        self.set_sobj_head(g, head);
        self.game.objs.sobjs[s.0].flags |= 0x10;
        self.game.audio.lb_wait_for_banks(self.data);
        self.lb_play_music(0x3E);
        self.lb_audio_pause(true);
        let (entrynum, _) = self.game.ft.data.files.entry(MOVIE);
        let file = self.game.audio.devcom.file(entrynum);
        let rate = crate::lb::mthp::Rate {
            addr: RATE_TABLE,
            runs: RATE_RUNS.to_vec(),
        };
        self.game
            .audio
            .mthp_open(self.data, entrynum, file, rate, 0);
        self.lb_audio_pause(false);
        self.game.opening.perf_display = true;
    }

    /// A sprite GObj's object: its chain from `head`.
    pub fn set_sobj_head(&mut self, g: GObjId, head: sobj::SObjId) {
        let w = &mut self.game.world;
        w.take_obj(g);
        w.set_obj(g, OBJ_SOBJ, Obj::SObj(Some(head)));
    }

    /// `gm_Scene_Opening_OnFrame`: the movie's clock taken; the narrator's "Super Smash
    /// Bros. Melee" (two lines in Japanese) and the video mode reset as the logo comes; the
    /// title mark from 454 to 514, the early logo from 950 to 974 and the logo from 5,130;
    /// the attract demo's files once the movie has ended; then the exits: the scene's end
    /// 1,000 ticks after the logo, a soft reset's skip to the title, Start once the logo has
    /// settled (to the main menu), or Start or A before (to the title).
    #[doc(alias = "gm_Scene_Opening_OnFrame")]
    #[doc(alias = "lbMthp_8001F578")]
    #[doc(alias = "lbMthp_8001F5C4")]
    #[doc(alias = "lbMthp_8001F604")]
    pub fn opening_on_frame(&mut self) {
        self.game.audio.movie.take_frame();
        let t = self.game.audio.movie.game_clock();
        let o = &mut self.game.opening;
        if o.clock > 5400 {
            o.clock = o.clock.wrapping_add(1);
        } else {
            o.clock = t;
        }
        let clock = o.clock;
        if super::super::gm::language_setting_is_us() {
            if !self.game.opening.line0 && clock >= 5034 {
                self.sfx_play(0x4E21, 0x7F, 0x40);
                self.game.opening.line0 = true;
            }
        } else {
            if !self.game.opening.line0 && clock >= 4980 {
                self.sfx_play(0x4E22, 0x7F, 0x40);
                self.game.opening.line0 = true;
            }
            if !self.game.opening.line1 && clock >= 5100 {
                self.sfx_play(0x4E23, 0x7F, 0x40);
                self.game.opening.line1 = true;
            }
        }
        if !self.game.opening.video_reset && clock >= 4980 {
            self.reset_video_mode();
            self.game.opening.video_reset = true;
        }
        if self.game.opening.mark.is_none() && (0x1C6..0x202).contains(&clock) {
            let g = self.game.world.create(0xE, 0xF, 0);
            self.game.world.set_obj(g, OBJ_SOBJ, Obj::SObj(None));
            self.game
                .world
                .setup_gx_link(g, CodeRef(sobj::RENDER), 0x11, 0);
            let desc = self.game.title.mark.expect("the title mark");
            let (s, head) = sobj::create(
                &mut self.game.objs.sobjs,
                self.data,
                g,
                None,
                sobj::Image::Data(desc),
                (0, 0),
                0x80,
                None,
            );
            self.set_sobj_head(g, head);
            let so = &mut self.game.objs.sobjs[s.0];
            so.x = 82.0;
            so.y = 290.0;
            self.game.opening.mark = Some(g);
        }
        if let Some(g) = self.game.opening.mark
            && clock >= 0x202
        {
            crate::hsd::gobj::free(self, g);
            self.game.opening.mark = None;
        }
        if self.game.opening.flash.is_none() && (0x3B6..0x3CE).contains(&clock) {
            let g = self.title_flash_logo();
            self.game.opening.flash = Some(g);
        }
        if let Some(g) = self.game.opening.flash
            && clock >= 0x3CE
        {
            crate::hsd::gobj::free(self, g);
            self.game.opening.flash = None;
        }
        if !self.game.opening.logo_shown && clock >= 0x140A {
            self.title_logo();
            let o = &mut self.game.opening;
            o.logo_shown = true;
            // `600.0f + (400.0f + clock)`, back to an integer.
            let c = gekko_fp::frsp(f64::from(o.clock));
            let v = gekko_fp::fadds(600.0, gekko_fp::fadds(400.0, c));
            o.end_at = v as u32;
        }
        let ended = self.game.audio.movie.has_ended();
        if ended && !self.game.opening.preloaded && self.game.opening.logo_shown {
            self.preload_title_demo();
            self.game.opening.preloaded = true;
        }
        let o = self.game.opening;
        let triggered = self.game.buttons_triggered(4) as u32;
        let start = u32::from(crate::input::pad::BUTTON_START);
        let a = u32::from(crate::input::pad::BUTTON_A);
        if o.logo_shown && o.clock == o.end_at {
            self.lb_audio_music_stop();
            self.game.end_scene();
        } else if self.game.main_lib.xc && !ended {
            self.reset_video_mode();
            self.lb_audio_music_stop();
            self.sfx_stop_all();
            self.game.end_scene_now();
            self.set_pending_game_mode(GM_TITLE);
            self.set_new_game_mode_pending();
        } else if o.clock > 0x157C {
            if triggered & start != 0 {
                self.reset_video_mode();
                self.lb_audio_music_stop();
                self.lb_audio_call(1);
                self.game.end_scene();
                self.menu_entry_unlocks();
                self.set_new_game_mode_pending();
            }
        } else if triggered & (start | a) != 0 {
            self.reset_video_mode();
            self.lb_audio_music_stop();
            self.sfx_stop_all();
            self.lb_audio_call(1);
            self.game.end_scene();
            self.set_pending_game_mode(GM_TITLE);
            self.set_new_game_mode_pending();
        }
    }

    /// The decisions before the main menu (`gm_80173EEC`, `gm_80172898(0x100)`), then the
    /// main menu unless a challenger comes first (`gm_80173754`).
    pub fn menu_entry_unlocks(&mut self) {
        self.trophy_unlocks();
        self.special_unlocks(0x100);
        if !self.challenger_check(GM_MENU, 0) {
            self.set_pending_game_mode(GM_MENU);
        }
    }

    /// `lbMthp_8001F67C`, the movie sprite's render callback: the frame due decoded, and drawn
    /// while the player draws it.
    #[doc(alias = "lbMthp_8001F67C")]
    #[doc(alias = "HSD_SObjLib_803A49E0")]
    pub fn render_movie(&mut self, gobj: GObjId) {
        self.game.audio.mthp_decode(self.data);
        if self.game.audio.movie.draw != 0 {
            self.render_sobjs(gobj);
        }
    }

    /// `HSD_SObjLib_803A49E0`: each sprite of the GObj's chain drawn.
    pub fn render_sobjs(&mut self, gobj: GObjId) {
        let Some(Obj::SObj(mut cur)) = self.game.world.gobj(gobj).obj else {
            // "BadOBJ for SOBJ-displayfunc": nothing drawn.
            return;
        };
        while let Some(s) = cur {
            let o = &self.game.objs.sobjs[s.0];
            assert!(o.callback.is_none(), "a sprite's callback isn't ported");
            let movie = if o.flags & 0x10 != 0 {
                self.game.audio.movie.planes_draw()
            } else {
                None
            };
            self.game
                .objs
                .display
                .draws
                .push(crate::hsd::display::Draw::SObj(s, movie));
            cur = self.game.objs.sobjs[s.0].next;
        }
    }

    /// `HSD_SObjLib_803A54EC`, the sprites' camera: its links' sprites drawn through it.
    #[doc(alias = "HSD_SObjLib_803A54EC")]
    pub fn render_sprite_camera(&mut self, gobj: GObjId) {
        let Some(Obj::CObj(c)) = self.game.world.gobj(gobj).obj else {
            panic!("the sprites' camera holds no CObj");
        };
        if crate::hsd::display::camera_begin(&mut self.game.objs, self.data, Some(c)) {
            self.game
                .objs
                .display
                .draws
                .push(crate::hsd::display::Draw::SpriteSetup);
            self.render_links(gobj, 1);
            crate::hsd::display::camera_end(&mut self.game.objs);
        }
        self.game
            .objs
            .display
            .draws
            .push(crate::hsd::display::Draw::Invalidate(u32::MAX));
    }

    /// `gm_SetupTitleDemo`: the attract match's four characters, drawn from the unlocked
    /// ones the attract loop has used least (Zelda and Sheik counting as one), their colors,
    /// the match's kind, and a stage drawn likewise, other than the last one's.
    #[doc(alias = "gm_SetupTitleDemo")]
    #[doc(alias = "gm_801BF634")]
    #[doc(alias = "gm_801BF648")]
    #[doc(alias = "gm_801BF65C")]
    #[doc(alias = "gm_801BF670")]
    #[doc(alias = "gm_801BF684")]
    #[doc(alias = "gm_801BF694")]
    #[doc(alias = "gm_801BF6A8")]
    #[doc(alias = "gm_801BF6B8")]
    #[doc(alias = "gm_801BF6C8")]
    #[doc(alias = "gm_801BF6D8")]
    #[doc(alias = "gm_801BF6E8")]
    #[doc(alias = "gm_801BF6F8")]
    pub fn setup_title_demo(&mut self) {
        let mut pool = [0i32; 29];
        let mut count = 0;
        for c in 0..CKIND_PLAYABLE {
            if self.game.gm.unlocks.ckind_unlocked(c as u8) {
                pool[count] = c as i32;
                count += 1;
            }
        }
        pool[count] = CKIND_PLAYABLE as i32;
        let usage = |r: &Self, c: i32| char_usage(&r.game.game_data, c as usize);
        for i in 0..count {
            for j in i + 1..count {
                if usage(self, pool[i]) > usage(self, pool[j]) {
                    pool.swap(i, j);
                }
            }
        }
        for c in 0..4 {
            let pick = loop {
                let j = pool[self.game.objs.rng.below(8) as usize];
                let mut dup = false;
                for p in 0..c {
                    let prev = i32::from(self.game.boot_mode.demo.ckinds[p]);
                    if j == prev
                        || (j == CKIND_ZELDA && prev == CKIND_SEAK)
                        || (j == CKIND_SEAK && prev == CKIND_ZELDA)
                    {
                        dup = true;
                    }
                }
                if !dup {
                    break j;
                }
            };
            let d = &mut self.game.boot_mode.demo;
            d.ckinds[c] = pick as u8;
            d.colors[c] = 0;
            add_char_usage(&mut self.game.game_data, pick as usize);
        }
        self.game.boot_mode.demo.x8 = self.game.objs.rng.below(4) as u8;
        loop {
            self.game.boot_mode.demo.x9 = self.game.objs.rng.below(4) as u8;
            let d = self.game.boot_mode.demo;
            if d.x8 != d.x9 {
                break;
            }
        }
        let mut stages = [0i32; 30];
        let mut count = 0;
        for c in 0..0x1D {
            if self.stage_unlocked(stage_select_kind(c)) {
                stages[count] = i32::from(c);
                count += 1;
            }
        }
        stages[count] = 0x1D;
        let usage = |r: &Self, s: i32| stage_usage(&r.game.game_data, s as usize);
        for i in 0..count {
            for j in i + 1..count {
                if usage(self, stages[i]) > usage(self, stages[j]) {
                    stages.swap(i, j);
                }
            }
        }
        let pick = loop {
            let s = stages[self.game.objs.rng.below(8) as usize];
            // `gm_801BF694` gives the stage's low byte.
            let prev = self.game.boot_mode.demo.stkind as u8;
            if stage_select_kind(s as u8) != u16::from(prev) {
                break s;
            }
        };
        self.game.boot_mode.demo.stkind = stage_select_kind(pick as u8);
        add_stage_usage(&mut self.game.game_data, pick as usize);
        self.game.boot_mode.demo.match_kind = self.game.objs.rng.below(4) as u8;
    }

    /// `gm_PreloadTitleDemo`: the attract match's characters and stage into the preload
    /// cache's game cache, the cache refreshed, and their sound banks loading.
    #[doc(alias = "gm_PreloadTitleDemo")]
    pub fn preload_title_demo(&mut self) {
        self.dvd_mut().keep_class();
        let d = self.game.boot_mode.demo;
        {
            let gc = &mut self.dvd_mut().cache.scene.game_cache;
            for i in 0..4 {
                gc.entries[i].char_id = i32::from(d.ckinds[i]);
                gc.entries[i].color = d.colors[i];
            }
            gc.stkind = i32::from(d.stkind as u8);
        }
        self.game.audio.preload_refresh(self.data);
        let mut banks = 4u64;
        for &c in &d.ckinds {
            if c != 0x21 {
                banks |= crate::lb::audio::character_banks(i32::from(c));
            }
        }
        banks |= crate::lb::audio::stage_banks(i32::from(d.stkind as u8));
        let a = &mut self.game.audio;
        a.lb.unwant_banks(0x1E);
        a.lb.want_banks(0xE, banks);
        a.lb_start_bank_loads(self.data);
    }
}

/// `CKind_Playable_Count`, `CKind_Zelda`, `CKind_Seak`.
const CKIND_PLAYABLE: usize = 0x1A;
const CKIND_ZELDA: i32 = 0x12;
const CKIND_SEAK: i32 = 0x13;

/// `lbl_803B7808`: the stage kind of each stage select entry.
const STAGE_SELECT_KINDS: [u16; 30] = [
    0x04, 0x05, 0x0D, 0x06, 0x08, 0x02, 0x07, 0x03, 0x0A, 0x09, 0x19, 0x13, 0x0B, 0x0C, 0x0E, 0x0F,
    0x10, 0x11, 0x16, 0x17, 0x18, 0x12, 0x14, 0x1B, 0x1F, 0x20, 0x1C, 0x1D, 0x1E, 0x00,
];

/// `gm_801641CC`: the stage kind of stage select entry `i`.
#[doc(alias = "gm_801641CC")]
pub fn stage_select_kind(i: u8) -> u16 {
    STAGE_SELECT_KINDS[usize::from(i)]
}

/// The attract loop's history (`struct gm_random_history` over the game's data): how often
/// each character (from byte 2) and stage select entry (from byte 0x1C) has played.
fn char_usage(g: &crate::game::structs::gmm_x0, c: usize) -> u8 {
    if c == 0 {
        g.unk_2
    } else {
        g.pad_3[c - 1] as u8
    }
}

fn add_char_usage(g: &mut crate::game::structs::gmm_x0, c: usize) {
    if c == 0 {
        g.unk_2 = g.unk_2.wrapping_add(1);
    } else {
        g.pad_3[c - 1] = g.pad_3[c - 1].wrapping_add(1);
    }
}

fn stage_usage(g: &crate::game::structs::gmm_x0, s: usize) -> u8 {
    g.pad_3[0x19 + s] as u8
}

fn add_stage_usage(g: &mut crate::game::structs::gmm_x0, s: usize) {
    g.pad_3[0x19 + s] = g.pad_3[0x19 + s].wrapping_add(1);
}
