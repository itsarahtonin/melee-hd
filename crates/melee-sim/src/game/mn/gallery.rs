//! The Data menu's movies (`melee/mn/mngallery.c`): a list of two, the Special Movie and How
//! to Play, each played full in the menu through the movie player into the buffer the menu's
//! mode set aside (`rate_table`, `mnGallery_80258940`), with its song; A, B or Start, or the
//! movie's end, stops it, and B leaves for the Data list.

use melee_data::archive::DataRef;

use super::super::gm::title::ModelDesc;
use super::super::handles::CodeRef;
use super::super::state::{OBJ_JOBJ, Obj, Running, UserData};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::sis::TextId;
use crate::hsd::sobj::{self, OBJ_SOBJ};
use crate::vec::Vec3;

use super::main::{IN_A, IN_BACK, IN_DOWN, IN_START, IN_UP};

/// The processes and the camera's render callback.
pub const THINK: u32 = 0x8025_8ED0;
pub const LIST_PROC: u32 = 0x8025_90C4;
pub const CURSOR_PROC: u32 = 0x8025_91BC;
pub const CAMERA_RENDER: u32 = 0x8025_896C;

/// The movies' buffer's size (`heap_size`).
pub const HEAP_SIZE: u32 = 0x27_1000;

/// The list's user data (`mnGallery_804D6C88_userdata`, 0x24 bytes of the heap).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct GalleryData {
    /// `unk0`: a movie is open; `unk1`: ticks since it ended; `unk2`: the one open.
    pub playing: u8,
    pub ended_ticks: u8,
    pub movie: u8,
    /// `gobj4`: the movies' camera; `gobj8`: the movie's sprite.
    pub camera: Option<GObjId>,
    pub sprite: Option<GObjId>,
    /// `sobj`: the sprite, by its GObj's chain (the first).
    pub sobj: Option<sobj::SObjId>,
    pub text: Option<TextId>,
    /// 0 coming in, 1 choosing, 2 leaving, 3 playing, 4 gone.
    pub state: u8,
    /// `unk15`: the hovered movie.
    pub hovered: u8,
    pub frame: i32,
    /// The two cursors' GObjs.
    pub cursors: [Option<GObjId>; 2],
}

/// A cursor's user data (`mnGallery_child_userdata`, 8 bytes of the heap): the list's GObj
/// and its index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct CursorData {
    pub parent: GObjId,
    pub index: i32,
}

/// `mngallery.c`'s statics: the list's and the cursors' models (`mnGallery_804A0BA0`,
/// `mnGallery_804A0BB0`), the list's GObj (`mnGallery_804D6C88`), and the user data.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Gallery {
    pub list_model: ModelDesc,
    pub cursor_model: ModelDesc,
    pub gobj: Option<GObjId>,
    pub data: std::collections::BTreeMap<u32, GalleryData>,
    pub cursors: std::collections::BTreeMap<u32, CursorData>,
}

impl Running<'_> {
    fn gallery_addr(&self) -> u32 {
        let g = self.game.gallery.gobj.expect("the movies' list");
        match self.game.world.gobj(g).user_data {
            Some(UserData::GalleryList(a)) => a,
            other => panic!("the movies' list holds {other:?}"),
        }
    }

    fn gallery_mut(&mut self) -> &mut GalleryData {
        let a = self.gallery_addr();
        self.game
            .gallery
            .data
            .get_mut(&a)
            .expect("the movies' list's data")
    }

    fn gallery(&self) -> GalleryData {
        self.game.gallery.data[&self.gallery_addr()]
    }

    /// `mnGallery_80259868`, from the Data list: the flow on the movies' list (0x1A), its
    /// models from the menus' archive, its think, the list and its cursors, the hovered
    /// movie's name, and the movies' camera.
    #[doc(alias = "mnGallery_80259868")]
    #[doc(alias = "lbArchive_LoadSections")]
    pub fn gallery_open(&mut self) {
        self.game.menu.cooldown = 5;
        let f = &mut self.game.menu.flow;
        f.prev_menu = f.cur_menu;
        f.cur_menu = 0x1A;
        f.hovered_selection = 0;
        let a = self
            .game
            .menu
            .models
            .first()
            .and_then(|m| m.joint)
            .expect("the menus' archive")
            .archive;
        let ar = self.data.get(a);
        let sym = |s: &str| ar.public(s).map(|offset| DataRef { archive: a, offset });
        let model = |p: &str| ModelDesc {
            joint: sym(&format!("{p}_joint")),
            anim: sym(&format!("{p}_animjoint")),
            matanim: sym(&format!("{p}_matanim_joint")),
            shapeanim: sym(&format!("{p}_shapeanim_joint")),
        };
        self.game.gallery.list_model = model("MenMainConGa_Top");
        self.game.gallery.cursor_model = model("MenMainCursorGa_Top");
        let w = &mut self.game.world;
        let o = w.create(0, 1, 0x80);
        let p = w.setup_proc(o, CodeRef(THINK), 0);
        w.proc_mut(p).flags_3 = w.phase;
        self.gallery_list();
        let t = self.create_text(0, 1, Vec3::new(-9.5, 9.1, 17.0), 364.683_32, 38.387_72);
        let s = &mut self.game.sis;
        s.text_mut(t).font_size = [0.0521, 0.0521];
        s.reset_text(self.data, t, 0xC7);
        self.gallery_mut().text = Some(t);
        let cam = self.game.world.create(6, 3, 0x80);
        self.ortho_camera(cam, 0x280, 0x1E0, CodeRef(CAMERA_RENDER), 1);
        self.game.world.gobj_mut(cam).gxlink_prios = 0x100;
        self.gallery_mut().camera = Some(cam);
    }

    /// `mnGallery_80258A08`: an orthographic camera over `w` by `h` pixels drawing with
    /// `render` at render priority `prio`.
    #[doc(alias = "mnGallery_80258A08")]
    fn ortho_camera(&mut self, gobj: GObjId, w: u16, h: u16, render: CodeRef, prio: u8) {
        self.sprite_camera_with(gobj, w, h, prio, false, render);
    }

    /// `mnGallery_8025963C`: the list's model on render link 4 with its process (already
    /// run this tick) and its user data, and the two cursors' models, each placed by its
    /// process at once.
    #[doc(alias = "mnGallery_8025963C")]
    #[doc(alias = "mnGallery_80259604")]
    fn gallery_list(&mut self) {
        let m = self.game.gallery.list_model;
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(6, 7, 0x80);
        g.gallery.gobj = Some(gobj);
        let j = jobj::load_joint(&mut g.objs, data, m.joint).expect("the movies' list");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world
            .setup_gx_link(gobj, CodeRef(super::super::render::JOBJ_CALLBACK), 4, 0x80);
        jobj::add_anim_all(&mut g.objs, data, Some(j), m.anim, m.matanim, m.shapeanim);
        jobj::req_anim_all(&mut g.objs, Some(j), 0.0);
        self.gallery_anim_all(j);
        let addr = crate::hsd::heap::mem_alloc(0x24);
        let g = &mut *self.game;
        g.gallery.data.insert(addr, GalleryData::default());
        g.world.init_user_data(gobj, 0, UserData::GalleryList(addr));
        let p = g.world.setup_proc(gobj, CodeRef(LIST_PROC), 0);
        g.world.proc_mut(p).flags_3 = g.world.phase;
        let cm = self.game.gallery.cursor_model;
        for i in 0..2 {
            let g = &mut *self.game;
            let child = g.world.create(6, 7, 0x80);
            g.gallery.data.get_mut(&addr).expect("the list").cursors[i] = Some(child);
            let j = jobj::load_joint(&mut g.objs, data, cm.joint).expect("a movie's cursor");
            g.world.set_obj(child, OBJ_JOBJ, Obj::JObj(j));
            g.world
                .setup_gx_link(child, CodeRef(super::super::render::JOBJ_CALLBACK), 4, 0x80);
            jobj::add_anim_all(
                &mut g.objs,
                data,
                Some(j),
                cm.anim,
                cm.matanim,
                cm.shapeanim,
            );
            jobj::req_anim_all(&mut g.objs, Some(j), 0.0);
            self.gallery_anim_all(j);
            let ca = crate::hsd::heap::mem_alloc(8);
            let g = &mut *self.game;
            g.gallery.cursors.insert(
                ca,
                CursorData {
                    parent: gobj,
                    index: i as i32,
                },
            );
            g.world
                .init_user_data(child, 0, UserData::GalleryCursor(ca));
            let p = g.world.setup_proc(child, CodeRef(CURSOR_PROC), 0);
            g.world.proc_mut(p).flags_3 = g.world.phase;
            self.gallery_cursor_proc(child);
        }
    }

    fn gallery_anim_all(&mut self, j: JObjId) {
        jobj::anim_all(
            &mut self.game.objs,
            self.data,
            &mut super::super::code::ParticleHooks,
            Some(j),
        );
    }

    /// `fn_802590C4`, the list's process: 19 frames coming in, then choosing; leaving, 29
    /// frames, then the list and its cursors gone; still while a movie plays.
    #[doc(alias = "fn_802590C4")]
    pub fn gallery_list_proc(&mut self, gobj: GObjId) {
        let Some(Obj::JObj(j)) = self.game.world.gobj(gobj).obj else {
            panic!("the movies' list without its model");
        };
        let d = self.gallery();
        match d.state {
            3 => return,
            0 => {
                if d.frame < 0x13 {
                    self.gallery_mut().frame += 1;
                } else {
                    self.gallery_mut().state = 1;
                }
            }
            2 => {
                if d.frame < 0x1D {
                    self.gallery_mut().frame += 1;
                } else {
                    self.gallery_mut().state = 4;
                    // The list's own GObj goes when its process returns; its user data holds
                    // till then.
                    gobj::free(self, gobj);
                    for i in 0..2 {
                        if let Some(c) = self.gallery().cursors[i] {
                            gobj::free(self, c);
                        }
                        self.gallery_mut().cursors[i] = None;
                    }
                }
            }
            _ => {}
        }
        let frame = self.gallery().frame;
        jobj::req_anim_all(&mut self.game.objs, Some(j), frame as f32);
        self.gallery_anim_all(j);
    }

    /// `mnGallery_802591BC`, a cursor's process: placed at its entry of the list, lit when
    /// hovered, and showing its movie's name.
    #[doc(alias = "mnGallery_802591BC")]
    pub fn gallery_cursor_proc(&mut self, gobj: GObjId) {
        let Some(UserData::GalleryCursor(ca)) = self.game.world.gobj(gobj).user_data else {
            panic!("a movie's cursor without its user data");
        };
        let c = self.game.gallery.cursors[&ca];
        let Some(Obj::JObj(j)) = self.game.world.gobj(gobj).obj else {
            panic!("a movie's cursor without its model");
        };
        let Some(Obj::JObj(pj)) = self.game.world.gobj(c.parent).obj else {
            panic!("the movies' list without its model");
        };
        let hovered = self.gallery().hovered;
        let jobjs = &mut self.game.objs.jobjs;
        let mut child = jobjs[pj].child;
        for _ in 0..c.index {
            child = child.and_then(|ch| jobjs[ch].next);
        }
        if let Some(ch) = child {
            let t = jobjs[ch].translate;
            jobj::set_srt(jobjs, j, |o| o.translate.x = t.x);
            jobj::set_srt(jobjs, j, |o| o.translate.y = t.y);
            jobj::set_srt(jobjs, j, |o| o.translate.z = t.z);
        }
        let first = self.game.objs.jobjs[j].child.expect("a cursor's model");
        let d0 = self.game.objs.jobjs[first]
            .dobj()
            .expect("a cursor's shape");
        let d1 = self.game.objs.dobjs[d0]
            .next
            .expect("a cursor's second shape");
        let (m0, m1) = (self.game.objs.dobjs[d0].mobj, self.game.objs.dobjs[d1].mobj);
        let lit = if i32::from(hovered) == c.index {
            1.0
        } else {
            0.0
        };
        for m in [m0, m1] {
            crate::hsd::mobj::req_anim(&mut self.game.objs, m, lit);
            crate::hsd::mobj::anim(&mut self.game.objs, self.data, m);
        }
        for m in [m0, m1] {
            let t = m.and_then(|m| self.game.objs.mobjs[m].tobj);
            crate::hsd::tobj::req_anim(&mut self.game.objs, t, c.index as f32);
            crate::hsd::tobj::anim(&mut self.game.objs, self.data, t);
        }
    }

    /// `mnGallery_8025896C`, the movies' camera: while a movie plays, the screen erased
    /// black and the movie's sprite drawn.
    #[doc(alias = "mnGallery_8025896C")]
    pub fn gallery_camera_render(&mut self, gobj: GObjId) {
        if self.gallery().state != 3 {
            return;
        }
        let Some(Obj::CObj(c)) = self.game.world.gobj(gobj).obj else {
            panic!("the movies' camera holds no CObj");
        };
        if crate::hsd::display::camera_begin(&mut self.game.objs, self.data, Some(c)) {
            let draws = &mut self.game.objs.display.draws;
            draws.push(crate::hsd::display::Draw::EraseColor([0, 0, 0, 0xFF]));
            draws.push(crate::hsd::display::Draw::Erase);
            crate::hsd::display::camera_end(&mut self.game.objs);
        }
        self.render_sprite_camera(gobj);
    }

    /// `mnGallery_80258BC4`: the hovered movie played: its sprite on render link 8 (the
    /// Special Movie 448 by 336 at (96, 72), How to Play full screen), the menus' song
    /// stopped for the movie's, and the movie opened into the menu's buffer.
    #[doc(alias = "mnGallery_80258BC4")]
    fn gallery_play(&mut self) {
        let d = self.gallery();
        if d.playing != 0 || d.hovered >= 2 {
            return;
        }
        let mode = d.hovered;
        self.gallery_mut().movie = mode;
        let g = self.game.world.create(6, 7, 0x80);
        self.game.world.set_obj(g, OBJ_SOBJ, Obj::SObj(None));
        self.game
            .world
            .setup_gx_link(g, CodeRef(crate::lb::mthp::RENDER), 8, 0x80);
        let (w, h, place) = if mode == 0 {
            (0x1C0, 0x150, (96.0, 72.0))
        } else {
            (0x280, 0x1E0, (0.0, 0.0))
        };
        let (s, head) = sobj::create(
            &mut self.game.objs.sobjs,
            self.data,
            g,
            None,
            sobj::Image::Movie {
                width: w,
                height: h,
            },
            (0, 0),
            0x80,
            None,
        );
        self.set_sobj_head(g, head);
        let so = &mut self.game.objs.sobjs[s.0];
        so.flags |= 0x10;
        so.x = place.0;
        so.y = place.1;
        let gd = self.gallery_mut();
        gd.sprite = Some(g);
        gd.sobj = Some(s);
        self.lb_audio_music_stop();
        let (song, name, rate) = if mode == 0 {
            (
                0x52,
                super::super::gm::howto::OMAKE_MOVIE,
                crate::lb::mthp::Rate::default(),
            )
        } else {
            (
                0x24,
                super::super::gm::howto::HOWTO_MOVIE,
                super::super::gm::howto::howto_rate(),
            )
        };
        self.lb_play_music(song);
        self.lb_audio_pause(true);
        let (entrynum, _) = self.game.ft.data.files.entry(name);
        let file = self.game.audio.devcom.file(entrynum);
        let buf = (self.game.menu.gallery_rate_table, HEAP_SIZE);
        self.game
            .audio
            .mthp_open_in(self.data, entrynum, file, rate, Some(buf), 0);
        self.lb_audio_pause(false);
        self.gallery_mut().playing = 1;
    }

    /// `mnGallery_80258D50`: the movie stopped, if one plays: the player let go, the menus'
    /// song again, and its sprite gone.
    #[doc(alias = "mnGallery_80258D50")]
    fn gallery_stop(&mut self) {
        if self.gallery().playing == 0 {
            return;
        }
        self.mthp_release();
        self.lb_audio_music_stop();
        let bgm = self.game.game_data.x1850.bgm;
        self.lb_play_music(i32::from(bgm));
        let gd = self.gallery_mut();
        gd.playing = 0;
        gd.ended_ticks = 0;
        if let Some(g) = gd.sprite.take() {
            gobj::free(self, g);
        }
    }

    /// `mnGallery_80258DBC`, while a movie plays: A, B or Start on any controller (with the
    /// back sound), 50 ticks past the movie's end, or a disc error's reset stops it, the list's
    /// cameras back.
    #[doc(alias = "mnGallery_80258DBC")]
    fn gallery_playing(&mut self) {
        let buttons = self.game.pads.copy.iter().fold(0, |b, p| b | p.trigger);
        self.game.audio.movie.take_frame();
        let ended = self.game.audio.movie.has_ended();
        if ended {
            let gd = self.gallery_mut();
            gd.ended_ticks = gd.ended_ticks.wrapping_add(1);
        }
        let skip = self.game.main_lib.xc && !ended;
        let pressed = buttons & 0x1300 != 0;
        if pressed || self.gallery().ended_ticks > 0x32 || skip {
            if pressed {
                self.lb_audio_call(0);
            }
            self.gallery_mut().state = 1;
            self.menu_cameras_show();
            self.gallery_stop();
        }
    }

    /// `fn_80258ED0`, the movies' list's think: B leaves for the Data list (the list going
    /// out, the movie stopped), up and down choose a movie (its name shown), A or Start plays
    /// it (the menu's cameras hidden).
    #[doc(alias = "fn_80258ED0")]
    pub fn gallery_think(&mut self, _gobj: GObjId) {
        if self.game.menu.cooldown != 0 {
            let m = &mut self.game.menu;
            m.cooldown -= 1;
            m.input_x2 = 0;
            m.input_x4 = 0;
            return;
        }
        let state = self.gallery().state;
        if state == 3 {
            self.gallery_playing();
            return;
        }
        if state == 2 {
            return;
        }
        let b = self.list_inputs();
        if b & IN_BACK != 0 {
            self.lb_audio_call(0);
            self.game.menu.flow.entering_menu = 0;
            self.go_to_list(5, 1, 3);
            if let Some(t) = self.gallery().text {
                self.destroy_text(t);
            }
            let gd = self.gallery_mut();
            let (cam, sprite) = (gd.camera.take(), gd.sprite.take());
            for g in [cam, sprite].into_iter().flatten() {
                gobj::free(self, g);
            }
            self.gallery_stop();
            self.gallery_mut().state = 2;
            return;
        }
        if state == 1 {
            let t = self.gallery().text;
            if b & IN_UP != 0 {
                if self.gallery().hovered != 0 {
                    self.lb_audio_call(2);
                    self.gallery_mut().hovered -= 1;
                    if let Some(t) = t {
                        self.game.sis.reset_text(self.data, t, 0xC7);
                    }
                }
            } else if b & IN_DOWN != 0 {
                if self.gallery().hovered < 1 {
                    self.lb_audio_call(2);
                    self.gallery_mut().hovered += 1;
                    if let Some(t) = t {
                        self.game.sis.reset_text(self.data, t, 0xC8);
                    }
                }
            } else if b & (IN_START | IN_A) != 0 {
                self.gallery_mut().state = 3;
                self.menu_cameras_hide();
                self.gallery_play();
            }
        }
    }
}
