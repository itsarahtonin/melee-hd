//! Game over and coming soon (`melee/gm/gm_19EF.c`): after a single-player run is lost,
//! the game over screen shows the character's trophy for the mode on its stand, the score,
//! and the continue: "yes" pays the coins it needs (ten a continue, the coin counter rolling
//! down), "no" ends the run, the score counting down to half before the next state. The
//! debug mode's state 9 runs it with its own data (`onEnterGameOver`). Coming soon is a
//! one-model screen the modes show before a feature a later version was to bring.

use gekko_fp as fp;
use melee_data::archive::{ArchiveId, DataRef};

use super::super::super::code::ParticleHooks;
use super::super::super::handles::CodeRef;
use super::super::super::render::{CAMERA_CALLBACK, JOBJ_CALLBACK, LOBJ_CALLBACK};
use super::super::super::state::{OBJ_CAMERA, OBJ_JOBJ, OBJ_LIGHT, Obj, Running};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, HIDDEN, JObjId};
use crate::hsd::sis::TextId;

/// `gm_Scene_GOver_OnEnter`, `gm_Scene_ComingSoon_OnEnter`: the scenes' entries (no frame
/// functions: the models' processes run them).
pub const ON_ENTER: CodeRef = CodeRef(0x801A_0A10);
pub const SOON_ON_ENTER: CodeRef = CodeRef(0x801A_0C6C);
/// `fn_8019F810` (the screen's model), `fn_8019F6EC` (the trophy's turn) and `fn_801A0B60`
/// (coming soon's model): their processes.
pub const PROC_SCREEN: u32 = 0x8019_F810;
pub const PROC_TROPHY: u32 = 0x8019_F6EC;
pub const PROC_SOON: u32 = 0x801A_0B60;

/// `HSD_PAD_A | HSD_PAD_START`, `HSD_PAD_DPADLEFT`'s and `HSD_PAD_DPADRIGHT`'s stick
/// equivalents (`0x40000`, `0x80000`: the left stick's left and right).
const PAD_A_START: u32 = 0x1100;
const PAD_LEFT: u32 = 0x4_0000;
const PAD_RIGHT: u32 = 0x8_0000;

/// The mode the screen takes the trophy from: Classic's, Adventure's, else All-Star's.
const GM_CLASSIC: u8 = 3;
const GM_ADVENTURE: u8 = 4;
const GM_ALLSTAR: u8 = 5;

/// `CKind_Mario`, the character a missing trophy falls back to.
const MARIO: u32 = 8;

/// A joint the screen keeps and the frame it plays it at (`struct unk_jobj_node`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Node {
    pub jobj: Option<JObjId>,
    pub frame: i32,
}

/// The enter and exit data (`DebugGameOverData`): the score, the mode it came from, the
/// result, the character, the human's slot and name tag, the coins a continue costs and the
/// coins held.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct GameOverData {
    pub score: u32,
    pub x4: i32,
    pub x8: u32,
    pub result: i32,
    pub ckind: u32,
    pub slot: u8,
    pub tag: u8,
    pub cost: u16,
    pub coins: u16,
}

/// `gm_19EF.c`'s statics: `lbl_804D6698` to `lbl_804D66F8` and `lbl_80479A98`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct GameOver {
    /// `lbl_804D6698`: `GmGover.dat`'s archive, and its two blocks in the scene's heap.
    pub archive: Option<ArchiveId>,
    pub blocks: (u32, u32),
    /// `lbl_804D669C` to `lbl_804D66A8`: the scenes of `GmGover`, `GmGoCoin`, `GmGoAnim` and
    /// `GmRgStnd`.
    pub scenes: [Option<DataRef>; 4],
    /// `lbl_804D66AC`: the trophy's joint.
    pub trophy: Option<DataRef>,
    /// `lbl_804D66B0` to `lbl_804D66E8`: the backdrop's loop, the trophy, "no", "yes", the
    /// end's animation, joint 1, the turning stand's model and the stand.
    pub nodes: [Node; 8],
    /// `lbl_804D66F0` to `lbl_804D66F8`: coming soon's archive (its two blocks), its joint 1
    /// and its frame.
    pub soon_archive: Option<ArchiveId>,
    pub soon_file: u32,
    pub soon_block: u32,
    pub soon_jobj: Option<JObjId>,
    pub soon_frame: u32,
    /// `lbl_80479A98`: `x0`, the screen's state (0 the entrance, 1 the question, 8 the
    /// answer's animation, 9 the continue, 10 the end).
    pub state: u8,
    /// `x4` to `x10`: the mode it came from, the score shown, half the score, and the
    /// count-down's step.
    pub mode: i32,
    pub score: i32,
    pub half: i32,
    pub step: u32,
    /// `x14`: "yes" (1) or "no" chosen; `x15`, `x16`: the human's slot and name tag.
    pub yes: u8,
    pub slot: u8,
    pub tag: u8,
    /// `x18`: the result (1 continued).
    pub result: i32,
    /// `x1C`, `x1E`: the coins a continue costs and the coins held (tens, at most 999);
    /// `x20`: too few; `x22`: the coins held.
    pub cost: u16,
    pub held: u16,
    pub short: u8,
    pub coins: u16,
    /// `x24`: the score's text.
    pub text: Option<TextId>,
    /// `x28`: the coin counter's GObj, and `x2C` to `x50` its ten coins.
    pub counter: Option<GObjId>,
    pub coin_jobjs: [Option<JObjId>; 10],
    /// `x54` to `x5C`: the counter's three digits' reels; `x60`: paying; `x64` to `x6C`:
    /// their frames; `x70`: the coins still shown.
    pub reels: [Option<JObjId>; 3],
    pub paying: u8,
    pub reel_frames: [i32; 3],
    pub shown: i32,
}

/// `lbl_804D659A` (`gm_1601.c`): the count-down's tick sound's wait.
pub type TickWait = u16;

impl Running<'_> {
    /// The game over's trophy for `ckind` in the mode running (`gm_80160474` by
    /// `gm_GetCurrentGameMode`), inline in `fn_8019F9C4`.
    fn game_over_trophy(&self, ckind: u32) -> i32 {
        let mode = match self.game.machine.routing.curr_mode {
            GM_CLASSIC => GM_CLASSIC,
            GM_ADVENTURE => GM_ADVENTURE,
            _ => GM_ALLSTAR,
        };
        super::super::unlocks::mode_trophy(ckind as u8, mode)
    }

    /// `fn_8019EFC4`: while paying, the coin counter's reels roll a coin's ten down each
    /// five ticks, the coins still shown counted down, the rest hidden.
    #[doc(alias = "fn_8019EFC4")]
    fn game_over_pay(&mut self) {
        let g = &mut self.game.endings.game_over;
        if g.paying == 0 {
            return;
        }
        let objs = &mut self.game.objs;
        let jobj = match g.counter.map(|c| self.game.world.gobj(c).obj) {
            Some(Some(Obj::JObj(j))) => Some(j),
            _ => None,
        };
        let child = jobj.and_then(|j| objs.jobjs[j].child);
        let child_next = child.and_then(|c| objs.jobjs[c].next);
        if g.shown != 0 {
            g.reel_frames[0] = g.reel_frames[0].wrapping_add(1);
            if g.reel_frames[0] % 5 == 0 && g.shown != 0 {
                g.shown -= 1;
            }
            if g.reel_frames[0] > 0 && g.reel_frames[0] <= 5 {
                g.reel_frames[1] = g.reel_frames[0] + g.reel_frames[1] / 5 * 5;
                if g.reel_frames[1] > 0 && g.reel_frames[1] <= 5 {
                    g.reel_frames[2] = g.reel_frames[1] + g.reel_frames[2] / 5 * 5;
                }
            } else if g.reel_frames[0] == 50 {
                g.reel_frames[0] = 0;
            }
        }
        for (r, f) in g.reels.iter().zip(g.reel_frames) {
            jobj::req_anim_all(objs, *r, f as f32);
        }
        game_over_coins_shown(&mut objs.jobjs, &g.coin_jobjs, g.shown);
        jobj::anim_all(objs, self.data, &mut ParticleHooks, child_next);
    }

    /// `fn_8019F1D0`: the score's text, its digits printed afresh.
    #[doc(alias = "fn_8019F1D0")]
    fn game_over_score_text(&mut self) {
        let g = &self.game.endings.game_over;
        let t = g.text.expect("the score's text");
        let s = format!("{}\0", g.score as u32);
        self.game.sis.clear_text(t);
        self.game
            .sis
            .add_string(t, 240.0, 176.0, Some(s.as_bytes()));
    }

    /// `fn_8019F2D4`: the question's models by the state: the entrance's 80 frames (with two
    /// rumbles), "yes" or "no" lit, A or Start answering (a continue's coins paid, or the
    /// game over's song for "no"), the stick choosing, and the answer's animation.
    #[doc(alias = "fn_8019F2D4")]
    fn game_over_question(&mut self, trigger: u32) {
        let (slot, tag) = {
            let g = &self.game.endings.game_over;
            (g.slot as i8 as usize, g.tag)
        };
        let yes_frame = self.game.endings.game_over.nodes[3].frame;
        if yes_frame == 10 {
            self.tou_rumble(slot, tag, 0xD, 0xA);
        }
        if yes_frame == 0x19 {
            self.tou_rumble(slot, tag, 0xD, 0x14);
        }
        let data = self.data;
        let g = &mut self.game.endings.game_over;
        let objs = &mut self.game.objs;
        let (yes, no) = (g.nodes[3].jobj, g.nodes[2].jobj);
        let mobj = |objs: &crate::hsd::Objects, j: Option<JObjId>| {
            let j = j.expect("a question's joint");
            let d = objs.jobjs[j].dobj().expect("jobj->u.dobj");
            objs.dobjs[d].mobj
        };
        let mat = |objs: &mut crate::hsd::Objects, j: Option<JObjId>, f: f32| {
            let m = mobj(objs, j);
            crate::hsd::mobj::req_anim(objs, m, f);
            crate::hsd::mobj::anim(objs, data, m);
        };
        let joint = |objs: &mut crate::hsd::Objects, j: Option<JObjId>, f: f32| {
            jobj::req_anim_all(objs, j, f);
            jobj::anim_all(objs, data, &mut ParticleHooks, j);
        };
        match g.state {
            0 => {
                if g.nodes[3].frame < 0x50 {
                    g.nodes[3].frame += 1;
                } else {
                    g.state = 1;
                }
                g.nodes[2].frame = g.nodes[3].frame;
                joint(objs, yes, g.nodes[3].frame as f32);
                joint(objs, no, g.nodes[2].frame as f32);
                mat(objs, yes, 0.0);
                mat(objs, no, 1.0);
            }
            1 => {
                if g.yes != 0 {
                    mat(objs, yes, 0.0);
                    mat(objs, no, 1.0);
                } else {
                    mat(objs, yes, 1.0);
                    mat(objs, no, 0.0);
                }
                if trigger & PAD_A_START != 0 {
                    if g.yes != 0 {
                        if g.held >= g.cost {
                            g.state = 8;
                            g.paying = 1;
                            g.coins = g.coins.wrapping_sub(g.cost.wrapping_mul(10));
                            g.held -= g.cost;
                            g.nodes[3].frame = 0x5A;
                            g.nodes[2].frame = 0x64;
                            self.lb_audio_call(1);
                            return;
                        }
                        self.lb_audio_call(3);
                        return;
                    }
                    g.state = 8;
                    g.nodes[3].frame = 0x6E;
                    g.nodes[2].frame = 0x78;
                    self.lb_audio_call(1);
                    self.lb_play_music(0x20);
                    self.sfx_play(0x9C43, 0x7F, 0x40);
                    return;
                }
                if trigger & PAD_LEFT != 0 && g.yes == 0 {
                    g.yes = 1;
                    self.lb_audio_call(2);
                }
                let g = &mut self.game.endings.game_over;
                if trigger & PAD_RIGHT != 0 && g.yes == 1 {
                    g.yes = 0;
                    self.lb_audio_call(2);
                }
            }
            8 => {
                let f = g.nodes[3].frame as f32;
                joint(objs, no, f);
                joint(objs, yes, f);
                if g.nodes[3].frame < g.nodes[2].frame {
                    g.nodes[3].frame += 1;
                } else if g.yes != 0 {
                    g.state = 9;
                    g.nodes[6].frame = 0x3C;
                } else {
                    g.state = 0xA;
                }
                if g.yes != 0 {
                    mat(objs, no, 2.0);
                } else {
                    mat(objs, yes, 2.0);
                }
            }
            _ => {}
        }
    }

    /// `fn_8019F6EC`, the turning stand's process: its entrance to frame 50 (then, with too
    /// few coins, the game over's song and the end), the continue's turn to frame 80 and,
    /// once the score is down to half and the coins paid, the scene's end with the run
    /// continued; the trophy and its stand animate.
    #[doc(alias = "fn_8019F6EC")]
    pub fn game_over_trophy_proc(&mut self, gobj: GObjId) {
        let j = match self.game.world.gobj(gobj).obj {
            Some(Obj::JObj(j)) => j,
            other => panic!("the game over's stand holds {other:?}"),
        };
        let f = self.game.endings.game_over.nodes[6].frame as f32;
        jobj::req_anim_all(&mut self.game.objs, Some(j), f);
        let g = &mut self.game.endings.game_over;
        match g.state {
            0 => {
                if g.nodes[6].frame < 0x32 {
                    g.nodes[6].frame += 1;
                } else if g.short != 0 {
                    self.sfx_play(0x9C43, 0x7F, 0x40);
                    self.lb_play_music(0x20);
                    self.game.endings.game_over.state = 0xA;
                }
            }
            9 => {
                if g.nodes[6].frame < 0x50 {
                    g.nodes[6].frame += 1;
                } else if g.score as u32 == g.half as u32 && g.shown == 0 {
                    g.result = 1;
                    self.game.end_scene();
                }
            }
            _ => {}
        }
        let g = &self.game.endings.game_over;
        let (trophy, stand) = (g.nodes[1].jobj, g.nodes[7].jobj);
        let objs = &mut self.game.objs;
        jobj::anim_all(objs, self.data, &mut ParticleHooks, Some(j));
        jobj::anim_all(objs, self.data, &mut ParticleHooks, trophy);
        jobj::anim_all(objs, self.data, &mut ParticleHooks, stand);
    }

    /// `fn_8019F810`, the screen's process: the coins paid, the backdrop's loop (hidden once
    /// answered), the score counting down to half on a continue (the mode's result set once
    /// there), the end's animation for "no" to its frame 690, then the question and the
    /// score's text.
    #[doc(alias = "fn_8019F810")]
    #[doc(alias = "fn_80168F2C")]
    pub fn game_over_screen_proc(&mut self) {
        let slot = usize::from(self.game.endings.game_over.slot);
        let trigger = self.game.pads.copy[slot].trigger;
        self.game_over_pay();
        let data = self.data;
        let g = &mut self.game.endings.game_over;
        let objs = &mut self.game.objs;
        if g.state < 8 {
            g.nodes[0].frame += 1;
            if g.nodes[0].frame > 0x31F {
                g.nodes[0].frame = 0x190;
            }
            jobj::req_anim_all(objs, g.nodes[0].jobj, g.nodes[0].frame as f32);
            jobj::anim_all(objs, data, &mut ParticleHooks, g.nodes[0].jobj);
        } else {
            jobj::set_flags_all(&mut objs.jobjs, g.nodes[0].jobj, HIDDEN);
        }
        if g.state == 9 {
            if g.score as u32 == g.half as u32 {
                if matches!(g.mode, 0..=2) {
                    g.result = 1;
                }
            } else {
                // `fn_80168F2C`: the count-down's tick, each fourth.
                let w = &mut self.game.endings.tick_wait;
                if *w == 0 {
                    *w = 3;
                    self.sfx_play_track(0x7E, 0x7F, 0x40, 1);
                } else {
                    *w -= 1;
                }
                let g = &mut self.game.endings.game_over;
                let down = (g.score as u32).wrapping_sub(g.step);
                g.score = if down < g.half as u32 {
                    g.half
                } else {
                    down as i32
                };
            }
        }
        let g = &mut self.game.endings.game_over;
        if g.state == 0xA {
            let objs = &mut self.game.objs;
            jobj::req_anim_all(objs, g.nodes[4].jobj, g.nodes[4].frame as f32);
            jobj::anim_all(objs, data, &mut ParticleHooks, g.nodes[4].jobj);
            if g.nodes[4].frame < 0x2B2 {
                g.nodes[4].frame += 1;
            } else {
                self.game.end_scene();
                self.game.endings.game_over.result = 0;
            }
        }
        self.game_over_question(trigger);
        self.game_over_score_text();
    }

    /// An archive into the scene's heap and its scene's symbol `sym`.
    fn endings_scene(&mut self, name: &str, sym: &str) -> (ArchiveId, DataRef, (u32, u32)) {
        let (a, file, block) = self.load_archive_blocks(name);
        let offset = self
            .data
            .get(a)
            .public(sym)
            .unwrap_or_else(|| panic!("{sym} isn't in {name}"));
        (a, DataRef { archive: a, offset }, (file, block))
    }

    /// `fn_8019F9C4`: the screen's files (the screen's, the coin counter's, the turning
    /// stand's, the stand's and the trophy's), its camera and light, its model with its
    /// process and joints, the coin counter set to the coins held, the turning stand, the
    /// stand on it and the trophy on the stand, the score's text and, unless the coins are
    /// too few, the screen's song.
    #[doc(alias = "fn_8019F9C4")]
    #[doc(alias = "fn_80168F7C")]
    #[doc(alias = "gm_80160564")]
    #[doc(alias = "gm_801604DC")]
    fn game_over_setup(&mut self, ckind: u32) {
        let ckind = if self.game_over_trophy(ckind) == -1 {
            MARIO
        } else {
            ckind
        };
        self.game.endings.tick_wait = 0;
        let (a, scene0, blocks) = self.endings_scene("GmGover.dat", "ScGamRegGover_scene_data");
        let (_, scene1, _) = self.endings_scene("GmGoCoin.dat", "ScGamRegGover_scene_data");
        self.mark_class_two_trophies();
        self.toy_load_dataf();
        let trophy = self.game_over_trophy(ckind);
        let file = self.toy_model_file(trophy);
        let (name, sym) = (self.toy_file_name(file), self.toy_file_symbol(file));
        let (ta, _, _) = self.load_archive_blocks(&name);
        let joint = self
            .data
            .get(ta)
            .public(&sym)
            .map(|offset| DataRef {
                archive: ta,
                offset,
            })
            .unwrap_or_else(|| panic!("{sym} isn't in {name}"));
        let (_, scene2, _) = self.endings_scene("GmGoAnim.dat", "ScGamRegGover_scene_data");
        let (_, scene3, _) = self.endings_scene("GmRgStnd.dat", "standScene");
        let g = &mut self.game.endings.game_over;
        g.archive = Some(a);
        g.blocks = blocks;
        g.scenes = [Some(scene0), Some(scene1), Some(scene2), Some(scene3)];
        g.trophy = Some(joint);
        let data = self.data;
        let gm = &mut *self.game;
        // The camera.
        let cameras = data.ptr(scene0.at(4)).expect("the scene's cameras");
        let c =
            crate::hsd::cobj::load_desc(&mut gm.objs, data, data.ptr(cameras)).expect("a camera");
        let cam = gm.world.create(0x13, 0x14, 0);
        gm.world.set_obj(cam, OBJ_CAMERA, Obj::CObj(c));
        gm.world
            .setup_gx_link_max(cam, CodeRef(CAMERA_CALLBACK), 0, false);
        gm.world.gobj_mut(cam).gxlink_prios = 0x801;
        // The light.
        let light = gm.world.create(0xB, 0xF, 0);
        let lights = data.ptr(scene0.at(8)).expect("the scene's lights");
        let l = crate::game::gr::setup::load_light_list(gm, data, lights);
        gm.world
            .set_obj(light, OBJ_LIGHT, Obj::LObj(l.expect("a light")));
        gm.world.setup_gx_link(light, CodeRef(LOBJ_CALLBACK), 0, 0);
        // The screen's model.
        let model = |scene: DataRef| {
            let models = data.ptr(scene).expect("the scene's models");
            data.ptr(models).expect("a model")
        };
        let gobj = gm.world.create(0xE, 0xF, 0);
        let m0 = model(scene0);
        let j = jobj::load_joint(&mut gm.objs, data, data.ptr(m0)).expect("the screen's model");
        gm.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        gm.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 0xB, 0);
        gm.world.setup_proc(gobj, CodeRef(PROC_SCREEN), 0);
        super::super::vs::add_model_anim(&mut gm.objs, data, Some(j), m0, 0);
        jobj::req_anim_all(&mut gm.objs, Some(j), 0.0);
        jobj::anim_all(&mut gm.objs, data, &mut ParticleHooks, Some(j));
        let g = &mut gm.endings.game_over;
        g.yes = 1;
        g.state = 0;
        for k in [0, 2, 3, 6] {
            g.nodes[k].frame = 0;
        }
        g.nodes[4].frame = 0x24E;
        for (k, index) in [(0, 0x38), (5, 1), (4, 2), (3, 0x35), (2, 0x37)] {
            g.nodes[k].jobj = super::super::pause::tree_nodes(&gm.objs.jobjs, Some(j), &[index])[0];
        }
        if g.short != 0 {
            for k in [0, 3, 2] {
                jobj::set_flags_all(&mut gm.objs.jobjs, g.nodes[k].jobj, HIDDEN);
            }
        }
        // The coin counter.
        let gobj = gm.world.create(0xE, 0xF, 0);
        let m1 = model(scene1);
        let cj = jobj::load_joint(&mut gm.objs, data, data.ptr(m1)).expect("the coin counter");
        gm.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(cj));
        gm.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 0xB, 0);
        super::super::vs::add_model_anim(&mut gm.objs, data, Some(cj), m1, 0);
        let g = &mut gm.endings.game_over;
        g.counter = Some(gobj);
        let jobjs = &gm.objs.jobjs;
        let child = jobjs[cj].child;
        let mut node = child.and_then(|c| jobjs[c].child);
        for slot in &mut g.coin_jobjs {
            *slot = node;
            let n = node.expect("a coin");
            if jobjs[n].next.is_some() {
                node = jobjs[n].next;
            }
        }
        g.shown = i32::from(g.cost);
        let next = child.and_then(|c| jobjs[c].next);
        let r0 = next.and_then(|n| jobjs[n].child);
        let r1 = r0.and_then(|n| jobjs[n].next);
        let r2 = r1.and_then(|n| jobjs[n].next);
        g.reels = [r0, r1, r2];
        let v = u32::from(g.held);
        g.reel_frames[2] = ((10 - (v / 100) % 10) * 5 % 50) as i32;
        g.reel_frames[1] = ((10 - (v / 10) % 10) * 5 % 50) as i32;
        g.reel_frames[0] = ((10 - v % 10) * 5 % 50) as i32;
        for k in [2, 1, 0] {
            jobj::req_anim_all(&mut gm.objs, g.reels[k], g.reel_frames[k] as f32);
        }
        game_over_coins_shown(&mut gm.objs.jobjs, &g.coin_jobjs, g.shown);
        jobj::anim_all(&mut gm.objs, data, &mut ParticleHooks, Some(cj));
        // The turning stand.
        let gobj = gm.world.create(0xE, 0xF, 0);
        let m2 = model(scene2);
        let aj = jobj::load_joint(&mut gm.objs, data, data.ptr(m2)).expect("the turning stand");
        gm.endings.game_over.nodes[6].jobj = Some(aj);
        gm.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(aj));
        gm.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 0xB, 0);
        gm.world.setup_proc(gobj, CodeRef(PROC_TROPHY), 0);
        super::super::vs::add_model_anim(&mut gm.objs, data, Some(aj), m2, 0);
        jobj::req_anim_all(&mut gm.objs, Some(aj), 0.0);
        jobj::anim_all(&mut gm.objs, data, &mut ParticleHooks, Some(aj));
        // The stand, on the turning stand's third joint down.
        let gobj = gm.world.create(0xE, 0xF, 0);
        let m3 = model(scene3);
        let sj = jobj::load_joint(&mut gm.objs, data, data.ptr(m3)).expect("the stand");
        gm.endings.game_over.nodes[7].jobj = Some(sj);
        gm.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(sj));
        gm.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 0xB, 0);
        set_scale_each(&mut gm.objs.jobjs, sj, 2.0);
        let jobjs = &gm.objs.jobjs;
        let c1 = jobjs[aj].child;
        let c2 = c1.and_then(|c| jobjs[c].child);
        let target = c2.and_then(|c| jobjs[c].child).expect("the stand's place");
        crate::game::ef::constrain(&mut gm.objs, sj, target, 0x9000_0001);
        crate::game::ef::constrain(&mut gm.objs, sj, target, 0x9000_0004);
        // The trophy.
        let gobj = gm.world.create(0xE, 0xF, 0);
        let tj = jobj::load_joint(&mut gm.objs, data, Some(joint)).expect("the trophy");
        gm.endings.game_over.nodes[1].jobj = Some(tj);
        gm.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(tj));
        gm.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 0xB, 0);
        let idx = self.game_over_trophy(ckind);
        let p = |r: &Running, f: u32| r.toy_param(idx, f);
        let (x, y, z) = (p(self, 0), p(self, 1), p(self, 2));
        let turn = mem(fp::fmuls(reg(0.017_453_292), reg(p(self, 5))));
        let (s3, s4) = (p(self, 3), p(self, 4));
        let scale = mem(fp::fdivs(reg(s3), reg(s4)));
        let jobjs = &mut self.game.objs.jobjs;
        let child = jobjs[tj].child.expect("the trophy's child");
        jobj::set_srt(jobjs, child, |o| o.translate.x = x);
        jobj::set_srt(jobjs, child, |o| o.translate.y = y);
        jobj::set_srt(jobjs, child, |o| o.translate.z = z);
        jobj::set_rotation_y(jobjs, child, turn);
        jobj::set_srt(jobjs, child, |o| o.scale.x = scale);
        jobj::set_srt(jobjs, child, |o| o.scale.y = scale);
        jobj::set_srt(jobjs, child, |o| o.scale.z = scale);
        set_scale_each(jobjs, tj, 2.0);
        crate::game::ef::constrain(&mut self.game.objs, tj, sj, 0x9000_0001);
        crate::game::ef::constrain(&mut self.game.objs, tj, sj, 0x9000_0004);
        // The score's text.
        let canvas = self.add_canvas(0, Some(cam), 9, 0x12, 0, 0xB, 0, 0x13);
        self.load_font_named(0, "SdIntro.dat", "SIS_IntroData");
        let t = self.create_text_buffer(0, canvas);
        let tx = self.game.sis.text_mut(t);
        tx.font_size = [0.1, 0.1];
        tx.default_alignment = 2;
        self.game.endings.game_over.text = Some(t);
        self.game.sis.add_string(t, 240.0, 176.0, Some(b" \0"));
        if self.game.endings.game_over.short == 0 {
            self.sfx_play(0x9C42, 0x7F, 0x40);
            self.lb_play_music(5);
        }
    }

    /// `gm_Scene_GOver_OnEnter`: the screen's values from the enter data (the coins held in
    /// tens up to 999, too few for a continue or not, the score's half and the count-down's
    /// step in 120 ticks, at least 1), then the screen.
    #[doc(alias = "gm_Scene_GOver_OnEnter")]
    pub fn game_over_on_enter(&mut self) {
        let d = self.game_over_data();
        let g = &mut self.game.endings.game_over;
        g.paying = 0;
        g.coins = d.coins;
        g.held = (d.coins / 10).min(999);
        g.cost = d.cost;
        g.short = u8::from(g.held < g.cost);
        g.tag = d.tag;
        g.mode = d.x8 as i32;
        g.score = d.score as i32;
        g.half = d.score as i32 / 2;
        g.step = (g.score.wrapping_sub(g.half) as u32) / 120;
        if g.step == 0 {
            g.step = 1;
        }
        g.slot = d.slot;
        self.game_over_setup(d.ckind);
    }

    /// `gm_Scene_GOver_OnExit`: the score left, the result and the coins held into the exit
    /// data, the music stopped and the screen's archive let go.
    #[doc(alias = "gm_Scene_GOver_OnExit")]
    pub fn game_over_on_exit(&mut self) {
        let g = self.game.endings.game_over;
        let d = self.game_over_data_mut();
        d.x4 = g.score;
        d.result = g.result;
        d.coins = g.coins;
        self.lb_audio_music_stop();
        let (file, block) = g.blocks;
        self.free_archive_blocks(file, block);
    }

    /// The game over scene's enter and exit data: the debug mode's (`game_over_data`), or a
    /// single-player mode's (its own task's).
    fn game_over_data(&self) -> GameOverData {
        assert!(
            self.debug_mode_scene(),
            "a single-player mode's game over data isn't ported"
        );
        self.game.endings.debug_game_over
    }

    fn game_over_data_mut(&mut self) -> &mut GameOverData {
        assert!(
            self.debug_mode_scene(),
            "a single-player mode's game over data isn't ported"
        );
        &mut self.game.endings.debug_game_over
    }

    /// `fn_801A0B60`, coming soon's process: its model's animation, held at frame 189 until
    /// A or Start on any port, and the scene's end with the animation's.
    // Ports fn_801A0B60; aliased once a check reaches it.
    pub fn coming_soon_proc(&mut self, gobj: GObjId) {
        let j = match self.game.world.gobj(gobj).obj {
            Some(Obj::JObj(j)) => j,
            other => panic!("coming soon's model holds {other:?}"),
        };
        let f = self.game.endings.game_over.soon_frame;
        let objs = &mut self.game.objs;
        jobj::req_anim_all(objs, Some(j), f as f32);
        jobj::anim_all(objs, self.data, &mut ParticleHooks, Some(j));
        let g = &mut self.game.endings.game_over;
        if g.soon_frame == 0xBD {
            let t = self.game.buttons_triggered(4);
            if t & 0x1000 != 0 || t & 0x100 != 0 {
                self.game.endings.game_over.soon_frame += 1;
            }
        } else {
            g.soon_frame += 1;
        }
        let a = self
            .game
            .endings
            .game_over
            .soon_jobj
            .and_then(|j| self.game.objs.jobjs[j].aobj);
        let aobjs = &self.game.objs.aobjs;
        let a = a.expect("coming soon's joint's animation");
        if aobjs[a].curr_frame as i32 >= aobjs[a].end_frame as i32 {
            self.game.end_scene();
        }
    }

    /// `gm_Scene_ComingSoon_OnEnter`: `IfComSn.dat`'s scene: its camera, light and model
    /// with its process, and the model's joint 1.
    // Ports gm_Scene_ComingSoon_OnEnter; aliased once a check reaches it (the single-player
    // modes' coming soon states, Classic's, Adventure's and All-Star's).
    pub fn coming_soon_on_enter(&mut self) {
        self.game.endings.game_over.soon_frame = 0;
        let (a, scene, (file, block)) = self.endings_scene("IfComSn.dat", "ScComSoon_scene_data");
        let g = &mut self.game.endings.game_over;
        g.soon_archive = Some(a);
        g.soon_file = file;
        g.soon_block = block;
        let data = self.data;
        let gm = &mut *self.game;
        let cam = gm.world.create(0x13, 0x14, 0);
        let cameras = data.ptr(scene.at(4)).expect("the scene's cameras");
        let c =
            crate::hsd::cobj::load_desc(&mut gm.objs, data, data.ptr(cameras)).expect("a camera");
        gm.world.set_obj(cam, OBJ_CAMERA, Obj::CObj(c));
        gm.world
            .setup_gx_link_max(cam, CodeRef(CAMERA_CALLBACK), 0, false);
        gm.world.gobj_mut(cam).gxlink_prios = 0x801;
        let light = gm.world.create(0xB, 0xF, 0);
        let lights = data.ptr(scene.at(8)).expect("the scene's lights");
        let l = crate::game::gr::setup::load_light_list(gm, data, lights);
        gm.world
            .set_obj(light, OBJ_LIGHT, Obj::LObj(l.expect("a light")));
        gm.world.setup_gx_link(light, CodeRef(LOBJ_CALLBACK), 0, 0);
        let gobj = gm.world.create(0xE, 0xF, 0);
        let models = data.ptr(scene).expect("the scene's models");
        let m = data.ptr(models).expect("a model");
        let j = jobj::load_joint(&mut gm.objs, data, data.ptr(m)).expect("coming soon's model");
        gm.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        gm.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 0xB, 0);
        gm.world.setup_proc(gobj, CodeRef(PROC_SOON), 0);
        super::super::vs::add_model_anim(&mut gm.objs, data, Some(j), m, 0);
        jobj::req_anim_all(&mut gm.objs, Some(j), 0.0);
        jobj::anim_all(&mut gm.objs, data, &mut ParticleHooks, Some(j));
        gm.endings.game_over.soon_jobj =
            super::super::pause::tree_nodes(&gm.objs.jobjs, Some(j), &[1])[0];
    }

    /// `gm_Scene_ComingSoon_OnExit`: the archive let go, the music stopped.
    // Ports gm_Scene_ComingSoon_OnExit; aliased once a check reaches it.
    pub fn coming_soon_on_exit(&mut self) {
        let g = &self.game.endings.game_over;
        let (file, block) = (g.soon_file, g.soon_block);
        self.free_archive_blocks(file, block);
        self.lb_audio_music_stop();
    }
}

/// The counter's coins past `shown` hidden, the rest shown (the ten in turn from the last).
fn game_over_coins_shown(
    jobjs: &mut crate::hsd::jobj::JObjs,
    coins: &[Option<JObjId>; 10],
    shown: i32,
) {
    for i in (1..=10).rev() {
        if i > shown {
            jobj::set_flags(jobjs, coins[i as usize - 1], HIDDEN);
        } else {
            jobj::clear_flags(jobjs, coins[i as usize - 1], HIDDEN);
        }
    }
}

/// `HSD_JObjSetScaleX`, `Y` and `Z` in turn (each asserting the joint, each marking it
/// dirty).
fn set_scale_each(jobjs: &mut crate::hsd::jobj::JObjs, j: JObjId, s: f32) {
    jobj::set_srt(jobjs, j, |o| o.scale.x = s);
    jobj::set_srt(jobjs, j, |o| o.scale.y = s);
    jobj::set_srt(jobjs, j, |o| o.scale.z = s);
}
