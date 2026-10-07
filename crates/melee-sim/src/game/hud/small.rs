//! The HUD's smaller parts: the hazard sign Onett's, Mute City's and Big Blue's cars raise
//! (`melee/if/ifhazard.c`), the coin counter (`ifcoget.c`), the arrows pointing at players
//! off the screen's sides (`if_2FD9.c`), and the players' scores (`if_2FF2.c`).

use gekko_fp as fp;
use melee_data::archive::{ArchiveId, DataRef};

use super::super::code::{self, Val};
use super::super::gr::setup::load_light_list;
use super::super::handles::CodeRef;
use super::super::state::{Game, OBJ_CAMERA, OBJ_JOBJ, OBJ_LIGHT, Obj, Running, UserData};
use super::super::structs::{_GXColor, un_803F9E08_t, un_804A1F10_t, un_804A1F58_t};
use super::{CAMERA_RENDER, Draw, HIDDEN, JOBJ_RENDER, LIGHT_RENDER};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::sis::TextId;
use crate::hsd::{cobj, jobj};

/// The parts' processes and render callbacks.
pub const HAZARD_PROC: u32 = 0x802F_D680;
pub const HAZARD_RENDER: u32 = 0x802F_D6CC;
pub const COIN_PROC: u32 = 0x802F_ED14;
pub const ARROW_PROC: u32 = 0x802F_DA78;
pub const ARROW_RENDER: u32 = 0x802F_DA4C;
pub const SCORE_PROC: u32 = 0x802F_F218;
pub const SCORE_TEXT_RENDER: u32 = 0x802F_F360;

/// `gm_GetMatchEndPlayerScore`, the results' scoring (the match results' port's).
const GM_MATCH_END_PLAYER_SCORE: u32 = 0x8016_C658;

/// The hazard sign's statics (`un_804D6D80` to `un_804D6D8C`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Hazard {
    pub gobj: Option<GObjId>,
    /// Frames the sign has left.
    pub timer: i32,
    pub shown: i32,
    /// The HUD is hidden (a pause).
    pub hidden: i32,
}

/// The coin counter's statics.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Coins {
    pub state: un_803F9E08_t,
    /// `un_804D6DA0` (`IfCoGet.dat`) and `un_804D6DA4` (its `ScInfCgt_scene_data`).
    pub archive: Option<ArchiveId>,
    pub scene: Option<DataRef>,
}

impl Game {
    /// Shows the hazard sign for `frames` frames, from its first frame, `un_802FD604`.
    #[doc(alias = "un_802FD604")]
    pub fn show_hazard(&mut self, data: &melee_data::archive::Archives, frames: i32) {
        let j = match self.hud.hazard.gobj.map(|g| self.world.gobj(g).obj) {
            Some(Some(Obj::JObj(j))) => Some(j),
            _ => None,
        };
        jobj::req_anim_all(&mut self.objs, j, 0.0);
        jobj::anim_all(
            &mut self.objs,
            data,
            &mut crate::game::code::ParticleHooks,
            j,
        );
        self.hud.hazard.timer = frames;
        self.hud.hazard.shown = 1;
    }

    /// `un_802FD65C`.
    #[doc(alias = "un_802FD65C")]
    pub fn hide_hazard(&mut self) {
        self.hud.hazard.shown = 0;
    }

    /// `un_802FD668`: hidden while paused.
    #[doc(alias = "un_802FD668")]
    pub fn pause_hazard(&mut self) {
        self.hud.hazard.hidden = 1;
    }

    /// `un_802FD674`.
    #[doc(alias = "un_802FD674")]
    pub fn unpause_hazard(&mut self) {
        self.hud.hazard.hidden = 0;
    }

    /// The hazard sign's render callback, `fn_802FD6CC`.
    #[doc(alias = "fn_802FD6CC")]
    pub fn hazard_render(&self, gobj: GObjId, pass: i32) -> Option<Draw> {
        let h = &self.hud.hazard;
        (h.hidden == 0 && h.shown != 0).then_some(Draw::JObjs(gobj, pass))
    }

    /// The hazard sign's setup at the HUD's, which does nothing, `un_802FD704`.
    #[doc(alias = "un_802FD704")]
    pub fn init_hazard(&mut self) {}

    /// The hazard sign's freeing at the HUD's, which does nothing, `un_802FD90C`.
    #[doc(alias = "un_802FD90C")]
    pub fn free_hazard(&mut self) {}

    /// Hides the arrows while paused, `un_802FD910`.
    #[doc(alias = "un_802FD910")]
    pub fn pause_arrows(&mut self) {
        self.hud.arrows_hidden = 1;
    }

    /// `un_802FD91C`.
    #[doc(alias = "un_802FD91C")]
    pub fn unpause_arrows(&mut self) {
        self.hud.arrows_hidden = 0;
    }

    /// A human player's off-screen arrow (`kind` 1 left, 2 right) in the first free place,
    /// coloured, `un_802FD928`.
    #[doc(alias = "un_802FD928")]
    pub fn show_arrow(&mut self, slot: u8, kind: u8, color: [u8; 4]) {
        if slot >= 4 {
            return;
        }
        let human = self.player_pkind(i32::from(slot)) != super::super::pl::player::PKIND_CPU;
        let p = &mut self.hud.arrows;
        for i in 0..4 {
            if !human || p.x28[i] != 0 || p.x24[usize::from(slot)] != 0 {
                continue;
            }
            p.x24[usize::from(slot)] = 1;
            p.x2C[i] = slot;
            p.x28[i] = kind;
            p.x14[i] = _GXColor {
                r: color[0],
                g: color[1],
                b: color[2],
                a: color[3],
            };
            return;
        }
    }

    /// Takes a player's arrow away, `un_802FD9D8`.
    #[doc(alias = "un_802FD9D8")]
    pub fn hide_arrow(&mut self, slot: u8) {
        let p = &mut self.hud.arrows;
        for i in 0..4 {
            if p.x2C[i] == slot {
                p.x24[usize::from(slot)] = 0;
                p.x28[i] = 0;
            }
        }
    }

    /// The arrows' render callback, `fn_802FDA4C`.
    #[doc(alias = "fn_802FDA4C")]
    pub fn arrow_render(&self, gobj: GObjId, pass: i32) -> Option<Draw> {
        (self.hud.arrows_hidden == 0).then_some(Draw::JObjs(gobj, pass))
    }

    /// Ends the coin counter's run, `un_802FF128`'s undoing, `un_802FF190`.
    #[doc(alias = "un_802FF190")]
    pub fn stop_coin_counter(&mut self) {
        self.hud.coins.state.x0_b0 = false;
        self.hud.coins.state.x0_b1 = false;
    }

    /// Counts coins up to `target` (at most 100) at (`x`, `y`), a coin every `step`
    /// frames, `un_802FF128`.
    #[doc(alias = "un_802FF128")]
    pub fn start_coin_counter(&mut self, x: i32, y: i32, target: i32, step: i32) {
        let step = step.max(1);
        let s = &mut self.hud.coins.state;
        s.x0_b1 = true;
        s.x0_b234 = step as u8 & 7;
        s.x0_b567 = step as u8 & 7;
        s.x1 = 0;
        s.x2 = target.min(100) as u8;
        s.x4 = x;
        s.x8 = y.wrapping_neg();
    }

    /// Hides the scores, `un_802FF570`.
    #[doc(alias = "un_802FF570")]
    pub fn hide_scores(&mut self) {
        for i in 0..6 {
            let e = &mut self.hud.scores.x8[i];
            e.x10 = 1;
            if let Some(t) = e.x4 {
                self.sis.text_mut(t).hidden = 1;
            }
        }
    }
}

impl Running<'_> {
    /// The hazard sign's process, `fn_802FD680`: its time runs down.
    #[doc(alias = "fn_802FD680")]
    pub fn hazard_proc(&mut self, gobj: GObjId) {
        let h = &mut self.game.hud.hazard;
        if h.timer != 0 {
            h.timer -= 1;
            if h.timer == 0 {
                h.shown = 0;
            }
        }
        let j = match self.game.world.gobj(gobj).obj {
            Some(Obj::JObj(j)) => Some(j),
            _ => None,
        };
        jobj::anim_all(
            &mut self.game.objs,
            self.data,
            &mut crate::game::code::ParticleHooks,
            j,
        );
    }

    /// The hazard sign's model, hidden, `un_802FD708` (moved right for Onett's, `kind` 0).
    #[doc(alias = "un_802FD708")]
    pub fn create_hazard(&mut self, desc: DataRef, kind: i32) {
        let data = self.data;
        let g = &mut *self.game;
        let o = g.world.create(0xE, 15, 0);
        let j = jobj::load_joint(&mut g.objs, data, data.ptr(desc)).expect("a model");
        g.world.set_obj(o, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(o, CodeRef(HAZARD_RENDER), 11, 0);
        super::super::gm::vs::add_model_anim(&mut g.objs, data, Some(j), desc, 0);
        g.world.setup_proc(o, CodeRef(HAZARD_PROC), 17);
        g.hud.hazard.shown = 0;
        g.hud.hazard.hidden = 0;
        g.hud.hazard.gobj = Some(o);
        if kind == 0 {
            let t = g.objs.jobjs[j].translate;
            let mut v = t;
            v.x = mem(fp::fadds(reg(v.x), 18.0));
            jobj::set_translate(&mut g.objs.jobjs, j, v);
        }
    }

    /// The coin counter's process, `fn_802FED14`: hidden when not running; otherwise it
    /// counts a coin every few frames with a sound (another every tenth), placed and posed
    /// on its count.
    #[doc(alias = "fn_802FED14")]
    pub fn coin_proc(&mut self, gobj: GObjId) {
        let data = self.data;
        let Some(Obj::JObj(j)) = self.game.world.gobj(gobj).obj else {
            panic!("the coin counter without its model");
        };
        let g = &mut *self.game;
        if !g.hud.coins.state.x0_b1 {
            jobj::set_flags_all(&mut g.objs.jobjs, Some(j), HIDDEN);
            return;
        }
        let s = &mut g.hud.coins.state;
        if s.x1 > s.x2 {
            return;
        }
        jobj::clear_flags_all(&mut g.objs.jobjs, Some(j), HIDDEN);
        let s = &mut g.hud.coins.state;
        if s.x1 < s.x2 {
            if s.x0_b567 != 0 {
                s.x0_b567 -= 1;
            }
            if s.x0_b567 == 0 {
                s.x0_b567 = s.x0_b234;
                s.x1 = s.x1.wrapping_add(1);
                let id = if s.x1 != 0 && s.x1 % 10 == 0 {
                    0xB4
                } else {
                    0xB3
                };
                self.sfx_play(id, 0x7F, 0x40);
            }
        }
        let g = &mut *self.game;
        let s = &g.hud.coins.state;
        let x = mem(fp::fsubs(
            fp::fdivs(fp::frsp(f64::from(s.x4)), 10.958_900_451_660_156),
            29.200_000_762_939_453,
        ));
        let y = mem(fp::fadds(24.0, fp::fdivs(fp::frsp(f64::from(s.x8)), 10.0)));
        let n = s.x1;
        jobj::set_srt(&mut g.objs.jobjs, j, |o| o.translate.x = x);
        jobj::set_srt(&mut g.objs.jobjs, j, |o| o.translate.y = y);
        jobj::req_anim_all(&mut g.objs, Some(j), mem(fp::frsp(f64::from(n))));
        jobj::anim_all(
            &mut g.objs,
            data,
            &mut crate::game::code::ParticleHooks,
            Some(j),
        );
    }

    /// The coin counter's camera, light and hidden model from `IfCoGet.dat`, `un_802FEFAC`.
    #[doc(alias = "un_802FEFAC")]
    pub fn create_coin_counter(&mut self) {
        let data = self.data;
        let scene = self.game.hud.coins.scene.expect("ScInfCgt_scene_data");
        let g = &mut *self.game;
        let camera = g.world.create(0x13, 21, 0);
        let cameras = data.ptr(scene.at(4)).expect("the scene's cameras");
        let desc = data.ptr(cameras);
        let c = cobj::load_desc(&mut g.objs, data, desc).expect("a camera");
        g.world.set_obj(camera, OBJ_CAMERA, Obj::CObj(c));
        g.world
            .setup_gx_link_max(camera, CodeRef(CAMERA_RENDER), 9, false);
        g.world.gobj_mut(camera).gxlink_prios = 0x8400;
        let light = g.world.create(0xB, 3, 0);
        let lights = data.ptr(scene.at(8)).expect("the scene's lights");
        let l = load_light_list(g, data, lights).expect("the counter's light");
        let g = &mut *self.game;
        g.world.set_obj(light, OBJ_LIGHT, Obj::LObj(l));
        g.world.setup_gx_link(light, CodeRef(LIGHT_RENDER), 10, 0);
        let ui = g.world.create(0xE, 14, 0);
        let models = data.ptr(scene).expect("the scene's models");
        let model = data.ptr(models).expect("a model");
        let j = jobj::load_joint(&mut g.objs, data, data.ptr(model)).expect("a model");
        g.world.set_obj(ui, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(ui, CodeRef(JOBJ_RENDER), 15, 0);
        g.world.setup_proc(ui, CodeRef(COIN_PROC), 17);
        super::super::gm::vs::add_model_anim(&mut g.objs, data, Some(j), model, 0);
        jobj::set_flags_all(&mut g.objs.jobjs, Some(j), HIDDEN);
        jobj::req_anim_all(&mut g.objs, Some(j), 0.0);
        jobj::anim_all(
            &mut g.objs,
            data,
            &mut crate::game::code::ParticleHooks,
            Some(j),
        );
        g.hud.coins.state.xC = Some(ui);
    }

    /// The coin counter at the HUD's setup, `IfCoGet.dat` loaded (`lbArchive_80016DBC`,
    /// outside the simulation) with its scene, `un_802FF1B4`.
    #[doc(alias = "un_802FF1B4")]
    pub fn init_coin_counter(&mut self, if_coget: ArchiveId) {
        let s = &mut self.game.hud.coins;
        s.state.x0_b0 = true;
        s.state.x0_b1 = false;
        s.archive = Some(if_coget);
        let at = self
            .data
            .get(if_coget)
            .public("ScInfCgt_scene_data")
            .expect("IfCoGet.dat's ScInfCgt_scene_data");
        s.scene = Some(DataRef {
            archive: if_coget,
            offset: at,
        });
        self.create_coin_counter();
    }

    /// An arrow's process, `fn_802FDA78`: coloured, and placed by the player's side and
    /// place among the players there (far off the screen if none), the first arrow's turn
    /// to sound.
    #[doc(alias = "fn_802FDA78")]
    pub fn arrow_proc(&mut self, gobj: GObjId) {
        let data = self.data;
        let Some(Obj::JObj(j)) = self.game.world.gobj(gobj).obj else {
            panic!("an arrow without its model");
        };
        let Some(UserData::ArrowSlot(slot)) = self.game.world.gobj(gobj).user_data else {
            panic!("an arrow without its slot");
        };
        let s = usize::from(slot);
        let v = [0.0f32, -1.3, -2.6, -3.9].map(|x| mem(fp::frsp(f64::from(x))));
        let g = &mut *self.game;
        jobj::set_srt(&mut g.objs.jobjs, j, |o| o.translate.z = 0.0);
        let c = g.hud.arrows.x14[s].clone();
        let child = g.objs.jobjs[j].child.expect("jobj->child");
        let mut d = g.objs.jobjs[child].dobj();
        for _ in 0..3 {
            let dd = d.expect("the arrow's display objects");
            let m = g.objs.dobjs[dd].mobj.expect("dobj->mobj");
            let mat = g.objs.mobjs[m].mat.as_mut().expect("mobj->mat");
            mat.diffuse.r = c.r;
            mat.diffuse.g = c.g;
            mat.diffuse.b = c.b;
            mat.diffuse.a = c.a;
            d = g.objs.dobjs[dd].next;
        }
        let p = &mut g.hud.arrows;
        if p.x28.iter().all(|&k| k == 0) {
            p.x31 = -1;
            p.x32 = 0;
        }
        let kind = p.x28[s];
        let mut sound = false;
        match kind {
            0 => {
                jobj::set_srt(&mut g.objs.jobjs, j, |o| o.translate.z = 10000.0);
            }
            1 | 2 => {
                if p.x31 == -1 {
                    p.x31 = slot as i8;
                } else if p.x28[p.x31 as u8 as usize] == kind {
                    if p.x32 == 0 && p.x30 == 0 {
                        sound = true;
                    }
                    p.x32 = ((u32::from(p.x32) + 1) % 51) as u8;
                }
                let jn = (0..s).filter(|&i| p.x28[i] == kind).count() as u8;
                let kn = (0..4).filter(|&i| p.x28[i] == kind).count() as u8;
                let y = mem(fp::fmadds(
                    2.599_999_904_632_568_4,
                    fp::frsp(f64::from(jn)),
                    reg(v[if kn != 0 { usize::from(kn) - 1 } else { 0 }]),
                ));
                let (rot, x) = if kind == 1 {
                    (0.0, -25.0)
                } else {
                    (mem(fp::frsp(3.141_592_7)), 25.0)
                };
                jobj::set_rotation_z(&mut g.objs.jobjs, j, rot);
                jobj::set_srt(&mut g.objs.jobjs, j, |o| o.translate.y = y);
                jobj::set_srt(&mut g.objs.jobjs, j, |o| o.translate.x = x);
            }
            _ => {}
        }
        if sound {
            self.lb_audio_call(4);
        }
        jobj::anim_all(
            &mut self.game.objs,
            data,
            &mut crate::game::code::ParticleHooks,
            Some(j),
        );
    }

    /// The four arrows, from `IfAll.dat`'s `Stc_rarwmdls`, `un_802FE260`.
    #[doc(alias = "un_802FE260")]
    pub fn create_arrows(&mut self) {
        let data = self.data;
        let a = self.game.hud.archive.expect("IfAll.dat");
        self.game.hud.arrows_hidden = 0;
        let at = data
            .get(a)
            .public("Stc_rarwmdls")
            .expect("IfAll.dat's Stc_rarwmdls");
        let list = DataRef {
            archive: a,
            offset: at,
        };
        self.game.hud.arrows.x0 = Some(list);
        let model = data.ptr(list).expect("the arrows' model");
        for i in 0..4u8 {
            let g = &mut *self.game;
            let o = g.world.create(0xE, 15, 0);
            let j = jobj::load_joint(&mut g.objs, data, data.ptr(model)).expect("a model");
            g.world.gobj_mut(o).user_data = Some(UserData::ArrowSlot(i));
            g.world.set_obj(o, OBJ_JOBJ, Obj::JObj(j));
            g.world.setup_gx_link(o, CodeRef(ARROW_RENDER), 11, 0);
            super::super::gm::vs::add_model_anim(&mut g.objs, data, Some(j), model, 0);
            g.world.setup_proc(o, CodeRef(ARROW_PROC), 17);
            jobj::req_anim_all(&mut g.objs, Some(j), 0.0);
            jobj::anim_all(
                &mut g.objs,
                data,
                &mut crate::game::code::ParticleHooks,
                Some(j),
            );
            let p = &mut g.hud.arrows;
            p.x24[usize::from(i)] = 0;
            p.x28[usize::from(i)] = 0;
            p.x4[usize::from(i)] = Some(o);
        }
        let g = &mut *self.game;
        g.hud.arrows.x31 = -1;
        g.hud.arrows.x32 = 0;
        g.hud.arrows.x30 = u8::from(g.vs_x2_1());
    }

    /// Frees the arrows, `un_802FE390`.
    #[doc(alias = "un_802FE390")]
    pub fn free_arrows(&mut self) {
        for i in 0..4 {
            if let Some(o) = self.game.hud.arrows.x4[i].take() {
                gobj::free(self, o);
            }
        }
    }

    /// A player's score, capped at 9999 (`inl_getScore`).
    fn score(&mut self, slot: i32) -> i32 {
        code::call(self, GM_MATCH_END_PLAYER_SCORE, &[Val::Int(slot)])
            .int()
            .min(0x270F)
    }

    /// A score's process, `fn_802FF218`: "-" while hidden, else the score when it changes.
    #[doc(alias = "fn_802FF218")]
    pub fn score_proc(&mut self, gobj: GObjId) {
        let Some(slot) = (0..6).find(|&i| self.game.hud.scores.x8[i].x0 == Some(gobj)) else {
            return;
        };
        let e = self.game.hud.scores.x8[slot].clone();
        let t = e.x4.expect("a score's text");
        if e.x10 == 1 {
            self.game
                .sis
                .replace_string(self.data, t, e.x8 as i32, Some(b"-"));
        } else {
            let score = self.score(slot as i32);
            if e.xC != score as u32 {
                let s = crate::hsd::sis::format(b"%d", &[score]);
                self.game
                    .sis
                    .replace_string(self.data, t, e.x8 as i32, Some(&s));
                self.game.hud.scores.x8[slot].xC = score as u32;
            }
        }
    }

    /// Frees a score's GObj and text (the inline `inl_freeEntry`).
    fn free_score(&mut self, slot: usize) {
        let e = self.game.hud.scores.x8[slot].clone();
        if let Some(o) = e.x0 {
            gobj::free(self, o);
        }
        if let Some(t) = e.x4 {
            self.destroy_text(t);
        }
    }

    /// A player's score under their display, `un_802FF364`.
    #[doc(alias = "un_802FF364")]
    pub fn create_score(&mut self, slot: i32) {
        let s = slot as usize;
        let pos = self.game.player_hud_position(slot);
        self.free_score(s);
        let canvas = self.game.hud.scores.x0 as i32;
        let t = self.create_text_buffer(2, canvas);
        self.game.hud.scores.x8[s].x4 = Some(t);
        let tx = self.game.sis.text_mut(t);
        tx.default_alignment = 1;
        tx.default_kerning = 1;
        let score = self.score(slot);
        let text = crate::hsd::sis::format(b"%d", &[score]);
        let y = fp::fadds(3.200_000_047_683_716, reg(pos.y));
        let k = self.game.sis.add_string(t, reg(pos.x), y, Some(&text));
        self.game.hud.scores.x8[s].x8 = k as u32;
        let d = self.data;
        self.game
            .sis
            .set_scale(d, t, k, 0.059_999_998_658_895_49, 0.059_999_998_658_895_49);
        self.game.sis.text_mut(t).render_callback = Some(SCORE_TEXT_RENDER);
        let o = self.game.world.create(14, 15, 0);
        self.game.hud.scores.x8[s].x0 = Some(o);
        self.game.world.setup_proc(o, CodeRef(SCORE_PROC), 17);
    }

    /// The scores' canvas on the HUD's GObj, `un_802FF498`.
    #[doc(alias = "un_802FF498")]
    pub fn init_scores(&mut self) {
        self.game.hud.scores = Default::default();
        let parent = self.game.hud_gobj();
        let n = self.add_canvas(2, parent, 14, 15, 0, 11, 0, 19);
        self.game.hud.scores.x0 = n as u32;
    }

    /// Frees the scores, `un_802FF4FC`.
    #[doc(alias = "un_802FF4FC")]
    pub fn free_scores(&mut self) {
        for i in 0..6 {
            self.free_score(i);
        }
    }

    /// Shows the scores again, brought up to date, `un_802FF620`.
    #[doc(alias = "un_802FF620")]
    pub fn show_scores(&mut self) {
        for i in 0..6 {
            self.game.hud.scores.x8[i].x10 = 0;
            if let Some(t) = self.game.hud.scores.x8[i].x4 {
                self.create_score(i as i32);
                self.game
                    .sis
                    .text_mut(self.game.hud.scores.x8[i].x4.unwrap_or(t))
                    .hidden = 0;
            }
        }
    }
}

/// The scores' statics' type.
pub type Scores = un_804A1F58_t;
/// The arrows' statics' type.
pub type Arrows = un_804A1F10_t;
/// A score's text, for checks.
pub type ScoreText = Option<TextId>;
