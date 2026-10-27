//! The results screen (`gmresult.c`): its data (`lbl_8046DBE8`), the panel's model with
//! each player's KO counts, times and statistics pages, the winner's emblem and flames,
//! the panel's camera and light, the winner's fanfare, and each player's fighter posing
//! (`gm_1798.c`, [`super::results_fighters`]). The scene has no frame function: the
//! model GObj's process runs it ([`super::results_player`]).

use gekko_fp as fp;
use melee_data::archive::{ArchiveId, DataRef};

use super::super::handles::CodeRef;
use super::super::state::{OBJ_CAMERA, OBJ_JOBJ, OBJ_LIGHT, Obj, Running};
use super::super::structs::MatchEnd;
use crate::fpu::mem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::sis::TextId;
use crate::vec::Vec3;

/// `ResultsPlayerData`: a player's column of the panel.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct ResultsPlayerData {
    /// `x0_0`: the player is done (Start pressed, or a computer's or empty slot).
    pub done: bool,
    /// `x0_1` to `x0_4`: the page's arrows (more above, more below, a page before, a
    /// page after).
    pub x0_1: bool,
    pub x0_2: bool,
    pub x0_3: bool,
    pub x0_4: bool,
    pub page: u8,
    /// `x2`: a computer's frames on its page.
    pub x2: u16,
    pub scroll_offset: f32,
    pub fighter_gobj: Option<GObjId>,
    /// `camera`: what `fn_8017A318` leaves in r3 (its C returns nothing), the last
    /// camera GObj it gave a render link.
    pub camera: Option<GObjId>,
    pub stats_text: [[Option<TextId>; 10]; 3],
    pub ko_count: Option<TextId>,
    pub ko_time: Option<TextId>,
    pub jobjs: [Option<JObjId>; 15],
    pub stats_position: Vec3,
}

/// `ResultsData` (`lbl_8046DBE8`).
#[derive(Clone, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct ResultsData {
    pub x0_0: bool,
    /// `x0_1`: the panel's players' rows shown.
    pub x0_1: bool,
    /// `x0_23`: the statistics' phase (0 idle, 1 set up, 2 active).
    pub phase: u8,
    /// `x0_4`, `x0_5`: the enter data's two flags (pause on the first frame; the panel's
    /// camera drawing only link 0).
    pub x0_4: bool,
    pub x0_5: bool,
    pub x0_6: bool,
    pub x0_7: bool,
    /// `x1`: the screen's state.
    pub state: u8,
    pub num_pages: u8,
    /// `x3`: the frames left before the screen ends.
    pub x3: u8,
    /// `x4`, `x5`, `x6`: the winner, the winning team, the team's best player.
    pub x4: u8,
    pub x5: u8,
    pub x6: u8,
    /// `x8`: the screen's frames.
    pub frames: i32,
    pub xc: f32,
    /// `pnlsce`, `flmsce`: the archive's two scenes.
    pub pnlsce: Option<DataRef>,
    pub flmsce: Option<DataRef>,
    /// `x18`: the model GObj, whose process runs the screen.
    pub model_gobj: Option<GObjId>,
    pub cobj: Option<crate::hsd::cobj::CObjId>,
    /// `x20`: the winner's emblem.
    pub emblem: Option<JObjId>,
    pub x24: Option<JObjId>,
    pub x28: Option<JObjId>,
    pub x2c: Option<TextId>,
    pub x30: Option<JObjId>,
    pub x34: [Option<JObjId>; 6],
    pub x4c: [Vec3; 6],
    pub players: [ResultsPlayerData; 6],
}

/// `ResultsStatsInfo` (`lbl_8046E190`): the bonuses' page of a player (mode 2, its count).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct StatsInfo {
    pub x0: u8,
    pub x1: u8,
}

/// The results screen's statics.
#[derive(Clone, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Results {
    /// `lbl_8046DBE8`.
    pub data: ResultsData,
    /// `lbl_8046E190`.
    pub stats_info: [StatsInfo; 4],
    /// `lbl_804D65B8`: the panel's archive (`GmRst`).
    pub archive: Option<ArchiveId>,
    /// `lbl_804D3FC8`: whether only computers played (the screen ends sooner).
    pub no_human: i32,
    /// The fighters' screen (`gm_1798.c`).
    pub fighters: super::results_fighters::ResultsFighters,
}

/// `gm_WasMatchCanceled`: a match with no contest, or one restarted.
#[doc(alias = "gm_WasMatchCanceled")]
pub fn canceled(outcome: u8) -> bool {
    outcome == super::match_exit::OUTCOME_NO_CONTEST || outcome == super::match_exit::OUTCOME_RETRY
}

/// `Gm_PKind_Human`, `Gm_PKind_Cpu`, `Gm_PKind_NA`.
pub const PKIND_HUMAN: u8 = 0;
pub const PKIND_CPU: u8 = 1;
pub const PKIND_NA: u8 = 3;

/// `HSD_GOBJ_CLASS_CAMERA`, `HSD_GOBJ_CLASS_LIGHT`, the screen's models' class.
const CLASS_CAMERA: u16 = 0x13;
const CLASS_LIGHT: u16 = 0xB;
const CLASS_MODEL: u16 = 0xE;

/// The screen's processes and render callbacks.
pub const MODEL_PROC: u32 = 0x8017_9350;
pub const PANEL_PROC: u32 = 0x8017_6BCC;
pub const FLAME_PROC: u32 = 0x8017_6D18;
pub const PANEL_RENDER: u32 = 0x8017_5038;
pub const STATS_TEXT_RENDER: u32 = 0x8017_49B8;

/// `lbl_803B7B18`: each character kind's emblem among the flames' (`fn_80176BF0`).
const EMBLEMS: [(u8, u8); 26] = [
    (0x00, 0x0),
    (0x01, 0x1),
    (0x02, 0x2),
    (0x03, 0x3),
    (0x04, 0x5),
    (0x05, 0x6),
    (0x06, 0xD),
    (0x07, 0x6),
    (0x08, 0x6),
    (0x09, 0x7),
    (0x0A, 0x9),
    (0x0B, 0x8),
    (0x0C, 0x6),
    (0x0D, 0x9),
    (0x0E, 0x4),
    (0x0F, 0x9),
    (0x10, 0xA),
    (0x11, 0xC),
    (0x12, 0xD),
    (0x13, 0xD),
    (0x14, 0x2),
    (0x15, 0xD),
    (0x16, 0x6),
    (0x17, 0x7),
    (0x18, 0x9),
    (0x19, 0xD),
];
/// `lbl_803B7B18.translate`: where the title sits above its joint.
pub(super) const TITLE_OFFSET: Vec3 = Vec3::new(0.0, 4.0, 0.0);

/// The SJIS strings the panel prints: three dashes (`lbl_804D3F8C`), a minus (`lbl_804D3FA0`),
/// a plus (`lbl_804D3FA4`), and a dashed time (`lbl_804D3FA8`).
pub(super) const DASHES: &[u8] = &[0x81, 0x7C, 0x81, 0x7C, 0x81, 0x7C, 0];
pub(super) const MINUS: &[u8] = &[0x81, 0x7C, 0];
pub(super) const PLUS: &[u8] = &[0x81, 0x7B, 0];
pub(super) const NO_TIME: &[u8] = &[0x81, 0x7C, 0x81, 0x46, 0x81, 0x7C, 0];

/// The panel's column joints of each player (`fn_80175DC8`'s `lb_80011E24` indices), by
/// column joint and player.
pub(super) const COLUMNS: [[i32; 4]; 15] = [
    [0x42, 0x43, 0x44, 0x45],
    [0x1D, 0x25, 0x2D, 0x35],
    [0x1E, 0x26, 0x2E, 0x36],
    [0x3D, 0x3E, 0x3F, 0x40],
    [0x46, 0x49, 0x4C, 0x4F],
    [0x21, 0x29, 0x31, 0x39],
    [0x52, 0x53, 0x54, 0x55],
    [0x19, 0x1A, 0x1B, 0x1C],
    [0x6C, 0x6E, 0x70, 0x72],
    [0x6B, 0x6D, 0x6F, 0x71],
    [0x23, 0x2B, 0x33, 0x3B],
    [0x24, 0x2C, 0x34, 0x3C],
    [0x56, 0x57, 0x58, 0x59],
    [0x5A, 0x5B, 0x5C, 0x5D],
    [0x5E, 0x5F, 0x60, 0x61],
];

impl Running<'_> {
    /// The results the screen shows (`fn_80174274`: `lbl_8046DBE8.x94`, the enter data's).
    #[doc(alias = "fn_80174274")]
    pub fn results_end(&self) -> &MatchEnd {
        &self.game.vs_melee.results_enter.match_end
    }

    /// A player's bonuses earned, from the results' decisions (`x44C[slot].x0`, the first
    /// 0x100); each one a human earned noted in the save (`fn_8016F140`), `fn_80174284`.
    #[doc(alias = "fn_80174284")]
    #[doc(alias = "fn_8016F140")]
    fn results_bonus_count(&mut self, slot: u8) -> i32 {
        let me = self.results_end();
        let human = me.player_standings[usize::from(slot)].pkind == PKIND_HUMAN;
        let earned: Vec<usize> = (0..0x100)
            .filter(|&i| me.x44C[usize::from(slot)].x0[i] != 0)
            .collect();
        if human {
            for &i in &earned {
                self.game.note_bonus_seen(i);
            }
        }
        earned.len() as i32
    }

    /// The winner, the winning team and the team's best player, `fn_801771C0` (in a free
    /// for all, the winner).
    #[doc(alias = "fn_801771C0")]
    #[doc(alias = "fn_80165418")]
    #[doc(alias = "fn_801654A0")]
    fn results_winners(&mut self) {
        let me = self.results_end();
        let mut x4 = 0;
        for i in 0..4 {
            let p = &me.player_standings[i];
            if p.pkind != PKIND_NA && p.is_small_loser == 0 {
                x4 = i as u8;
            }
        }
        let mut x5 = 0;
        for i in 0..5 {
            let t = &me.team_standings[i];
            if t.active != 0 && t.is_small_loser == 0 {
                x5 = i as u8;
            }
        }
        let mut x6 = None;
        if me.is_teams == 1 {
            'found: for i in 0..4 {
                let pi = &me.player_standings[i];
                if pi.pkind == PKIND_NA || pi.team != x5 {
                    continue;
                }
                for j in 0..4 {
                    let pj = &me.player_standings[j];
                    if pj.pkind != PKIND_NA
                        && pj.team == x5
                        && i != j
                        && pi.is_small_loser > pj.is_small_loser
                    {
                        continue 'found;
                    }
                }
                x6 = Some(i as u8);
                break;
            }
        } else {
            x6 = Some(x4);
        }
        let d = &mut self.game.results.data;
        d.x4 = x4;
        d.x5 = x5;
        if let Some(x6) = x6 {
            d.x6 = x6;
        }
    }

    /// `gm_Scene_Results_OnEnter`: the screen's data from the enter data, the pages by
    /// whether the match was cancelled, each player's bonuses' page, the winners, the
    /// winning human's rumble, the coin counter, the panel's archive, camera and light, the
    /// model GObj and its process, the flames and emblem, the fighters' stage and their
    /// light, the winner's fanfare, then each player's images, fighter and camera.
    #[doc(alias = "gm_Scene_Results_OnEnter")]
    pub fn results_on_enter(&mut self) {
        let enter = &self.game.vs_melee.results_enter;
        let (f0, f1) = (enter.x0_0, enter.x0_1);
        let me = enter.match_end.clone();
        let r = &mut self.game.results;
        r.data = ResultsData {
            x0_4: f0,
            x0_5: f1,
            ..Default::default()
        };
        if canceled(me.outcome) {
            r.data.num_pages = 2;
        } else {
            r.data.num_pages = 3;
            if me.match_kind == 3 {
                for p in &mut r.data.players[..4] {
                    p.page = 2;
                }
            }
        }
        if !self.game.bonuses_are_off() {
            for i in 0..4u8 {
                let n = self.results_bonus_count(i);
                self.game.results.stats_info[usize::from(i)] = StatsInfo {
                    x0: 2,
                    x1: n.wrapping_mul(2).wrapping_add(2) as u8,
                };
            }
        } else {
            self.game
                .results
                .stats_info
                .fill(StatsInfo { x0: 2, x1: 0 });
        }
        self.results_winners();
        let x6 = usize::from(self.game.results.data.x6);
        let w = &me.player_standings[x6];
        if w.pkind == PKIND_HUMAN && !canceled(me.outcome) && w.x3_u[0] & 0x02 != 0 {
            self.alarm_rumble_catch_up();
            self.rumble_start(x6 as u8, 3, 0x20, 0);
        }
        let coget = self.game.ft.data.files.archive("IfCoGet.dat");
        self.init_coin_counter(coget);
        self.results_load_archive();
        self.results_panel_camera();
        let pnl = self.game.results.data.pnlsce.expect("the panel's scene");
        let lights = self.data.ptr(pnl.at(8));
        let g = &mut *self.game;
        let light = g.world.create(CLASS_LIGHT, 3, 0);
        let lobj = lights.and_then(|l| super::super::gr::setup::load_light_list(g, self.data, l));
        let lobj = lobj.unwrap_or_else(|| panic!("Error : lobj dont't get (gmResultAddLight)"));
        g.world.set_obj(light, OBJ_LIGHT, Obj::LObj(lobj));
        g.world
            .setup_gx_link(light, CodeRef(super::super::render::LOBJ_CALLBACK), 0xA, 0);
        let model = g.world.create(CLASS_MODEL, 0xF, 0);
        g.results.data.model_gobj = Some(model);
        g.world.setup_proc(model, CodeRef(MODEL_PROC), 0);
        self.results_flames();
        let costumes = [
            enter_byte(&self.game.vs_melee.results_enter, 1),
            enter_byte(&self.game.vs_melee.results_enter, 2),
            enter_byte(&self.game.vs_melee.results_enter, 3),
            enter_byte(&self.game.vs_melee.results_enter, 4),
        ];
        self.results_fighters_setup(costumes);
        self.results_fighters_light();
        if !canceled(me.outcome) {
            let theme = super::results_stats::victory_theme(me.player_standings[x6].ckind as i32);
            self.lb_play_music(theme);
        }
        for i in 0..4 {
            let p = &me.player_standings[i];
            if p.pkind == PKIND_NA {
                continue;
            }
            self.results_images(i);
            let f = self.results_fighter(i32::from(p.ckind), i32::from(p.x3_u[0] >> 2), i as i32);
            self.game.results.data.players[i].fighter_gobj = f;
            let c = self.results_fighter_camera(i as i32);
            self.game.results.data.players[i].camera = c;
        }
    }

    /// `gm_Scene_Results_OnExit`: the standings' bonuses on again (`fn_801701AC`).
    #[doc(alias = "gm_Scene_Results_OnExit")]
    pub fn results_on_exit(&mut self) {
        self.game.bonuses_on();
    }

    /// `gmResultLoadArchive`: `GmRst` (with the saved language's extension) and its two
    /// scenes, `pnlsce` and `flmsce`.
    fn results_load_archive(&mut self) {
        let name = if self.saved_us() {
            "GmRst.usd"
        } else {
            "GmRst.dat"
        };
        let a = self.load_archive(name);
        let ar = self.data.get(a);
        let sym = |s: &str| ar.public(s).map(|offset| DataRef { archive: a, offset });
        let (p, f) = (sym("pnlsce"), sym("flmsce"));
        assert!(
            p.is_some() && f.is_some(),
            "Error : Cannot read archive file (File Name : GmRst)."
        );
        let r = &mut self.game.results;
        r.archive = Some(a);
        r.data.pnlsce = p;
        r.data.flmsce = f;
    }

    /// The panel's camera, `fn_80176A6C`: the scene's camera drawing links 0 and 7, 8, 10,
    /// 11 and 14 (only 0 with the enter data's flag), and the panel's texts' canvas and
    /// strings (`SdRst`'s `SIS_ResultData`).
    #[doc(alias = "fn_80176A6C")]
    fn results_panel_camera(&mut self) {
        let data = self.data;
        let pnl = self.game.results.data.pnlsce.expect("the panel's scene");
        // `SceneDesc::cameras`, a `SceneCameraDesc` whose first word is the camera's desc.
        let cams = data.ptr(pnl.at(4)).expect("the panel's cameras");
        let desc = data.ptr(cams);
        let g = &mut *self.game;
        let gobj = g.world.create(CLASS_CAMERA, 20, 0);
        let c = crate::hsd::cobj::load_desc(&mut g.objs, data, desc)
            .unwrap_or_else(|| panic!("Error : cobj dont't get (gmResultAddPanelCamera)"));
        g.world.set_obj(gobj, OBJ_CAMERA, Obj::CObj(c));
        g.world.setup_gx_link_max(
            gobj,
            CodeRef(super::super::render::CAMERA_CALLBACK),
            8,
            false,
        );
        g.world.gobj_mut(gobj).gxlink_prios = if g.results.data.x0_5 { 1 } else { 0x4D81 };
        self.add_canvas(0, Some(gobj), 9, 0xD, 0, 0xE, 0, 0x13);
        let name = if self.saved_us() {
            "SdRst.usd"
        } else {
            "SdRst.dat"
        };
        self.load_font_named(0, name, "SIS_ResultData");
        self.game.results.data.cobj = Some(c);
    }

    /// The flames' emblem among the model's: the winning character's (`lbl_803B7B18`), or
    /// the eleventh after a cancelled match; the others hidden. `fn_80176BF0`.
    #[doc(alias = "fn_80176BF0")]
    fn results_emblem(&mut self, root: Option<JObjId>, ckind: u8, cancel: bool) -> Option<JObjId> {
        let want = if cancel {
            11
        } else {
            EMBLEMS
                .iter()
                .find(|e| e.0 == ckind)
                .map_or(-1, |e| i32::from(e.1))
        };
        let node = super::pause::tree_nodes(&self.game.objs.jobjs, root, &[5])[0];
        let mut j = node.and_then(|n| self.game.objs.jobjs[n].child);
        let (mut i, mut found) = (0, None);
        while let Some(n) = j {
            if i != want {
                jobj::set_flags_all(&mut self.game.objs.jobjs, Some(n), jobj::HIDDEN);
            } else {
                found = Some(n);
            }
            i += 1;
            j = self.game.objs.jobjs[n].next;
        }
        found
    }

    /// The flames behind the emblem and each placed player's flame, `fn_80176F60` and
    /// `fn_80176D3C`: the flames' model (`flmsce`'s first) with the winner's emblem in the
    /// winner's color, its process; then for each player that didn't win (all of them after
    /// a match with no contest) the flame of its place (`flmsce`'s 3, 2, 1 for second,
    /// third, fourth) at its spot.
    #[doc(alias = "fn_80176F60")]
    #[doc(alias = "fn_80176D3C")]
    #[doc(alias = "fn_80176BCC")]
    #[doc(alias = "fn_80176D18")]
    #[doc(alias = "fn_80179F6C")]
    fn results_flames(&mut self) {
        let data = self.data;
        let me = self.results_end().clone();
        let (x4, x6) = (
            usize::from(self.game.results.data.x4),
            self.game.results.data.x6,
        );
        let flm = self.game.results.data.flmsce.expect("the flames' scene");
        let models = data.ptr(flm).expect("the flames' models");
        let model = |i: u32| data.ptr(models.at(4 * i)).expect("a flame model");
        let m0 = model(0);
        let g = &mut *self.game;
        let gobj = g.world.create(CLASS_MODEL, 0xF, 0);
        let root = jobj::load_joint(&mut g.objs, data, data.ptr(m0));
        g.world
            .set_obj(gobj, OBJ_JOBJ, Obj::JObj(root.expect("the flames' model")));
        g.world.setup_gx_link(gobj, CodeRef(PANEL_RENDER), 0xB, 0);
        crate::lb::anim::add_model_anim(&mut g.objs, data, root, 0, m0);
        jobj::req_anim_all(&mut g.objs, root, 0.0);
        let ckind = me.player_standings[usize::from(x6)].ckind as u8;
        let emblem = self.results_emblem(root, ckind, canceled(me.outcome));
        self.game.results.data.emblem = emblem;
        let e = emblem.expect("the winner's emblem");
        let g = &mut *self.game;
        let dobj = g.objs.jobjs[e].dobj();
        let mobj = dobj.and_then(|d| g.objs.dobjs.pool[d.0].mobj);
        let aobj = mobj.and_then(|m| g.objs.mobjs.pool[m.0].aobj);
        let color = super::vs::player_color(
            x6,
            g.player_team(i32::from(x6)) as u8,
            u8::from(me.is_teams == 1),
            me.player_standings[x4].pkind,
        );
        let frame = mem(fp::fadds(1.0, fp::frsp(f64::from(color as u8))));
        crate::hsd::aobj::set_current_frame(&mut g.objs.aobjs, &mut g.objs.fobjs, aobj, frame);
        crate::hsd::aobj::set_rate(&mut g.objs.aobjs, aobj, 0.0);
        crate::hsd::mobj::anim(&mut g.objs, data, mobj);
        crate::hsd::aobj::set_current_frame(&mut g.objs.aobjs, &mut g.objs.fobjs, aobj, 0.0);
        crate::hsd::aobj::set_rate(&mut g.objs.aobjs, aobj, 1.0);
        for ty in [4, 5, 6] {
            super::super::hud::status::stop_anim_type(
                &mut g.objs,
                data,
                e,
                ty,
                super::super::mn::options::MOBJ_MASK,
            );
        }
        g.world.setup_proc(gobj, CodeRef(PANEL_PROC), 0);
        // The four spots under the root.
        let mut spots = [Vec3::default(); 4];
        let mut j = root.and_then(|r| g.objs.jobjs[r].child);
        for s in &mut spots {
            if let Some(n) = j {
                *s = g.objs.jobjs[n].translate;
                j = g.objs.jobjs[n].next;
            }
        }
        let flames = [model(3), model(2), model(1)];
        for (i, &pos) in spots.iter().enumerate() {
            let p = &me.player_standings[i];
            if p.pkind == PKIND_NA {
                continue;
            }
            let mut place = if me.is_teams == 0 {
                i32::from(p.is_big_loser)
            } else {
                i32::from(me.team_standings[usize::from(p.team)].is_big_loser)
            };
            if place > 3 {
                place = 3;
            }
            if me.outcome == super::match_exit::OUTCOME_NO_CONTEST {
                place = 1;
            } else if place == 0 {
                continue;
            }
            let m = flames[(place - 1) as usize];
            let g = &mut *self.game;
            let gobj = g.world.create(CLASS_MODEL, 0xF, 0);
            let root = jobj::load_joint(&mut g.objs, data, data.ptr(m));
            let root_j = root.expect("a flame");
            g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(root_j));
            g.world
                .setup_gx_link(gobj, CodeRef(super::super::render::JOBJ_CALLBACK), 0xB, 0);
            jobj::set_translate(&mut g.objs.jobjs, root_j, pos);
            super::vs::add_model_anim(&mut g.objs, data, root, m, 0);
            jobj::req_anim_all(&mut g.objs, root, 0.0);
            self.anim_all(root_j);
            self.game.world.setup_proc(gobj, CodeRef(FLAME_PROC), 1);
            self.game.results.fighters.gobjs[i] = Some(gobj);
        }
    }

    /// The panel's model and the flames' processes: their animations, `fn_80176BCC` and
    /// `fn_80176D18`.
    pub fn results_anim_proc(&mut self, gobj: GObjId) {
        if let Some(Obj::JObj(j)) = self.game.world.gobj(gobj).obj {
            self.anim_all(j);
        }
    }

    /// The panel's render callback, `fn_80175038`: its tree for the pass's transparency.
    #[doc(alias = "fn_80175038")]
    pub fn results_panel_render(&mut self, gobj: GObjId, pass: i32) {
        self.render_jobj(gobj, pass);
    }

    /// The process and render callbacks the screen gives its GObjs.
    pub fn results_run(&mut self, gobj: GObjId, addr: u32) -> bool {
        match addr {
            MODEL_PROC => self.results_model_proc(gobj),
            PANEL_PROC | FLAME_PROC => self.results_anim_proc(gobj),
            _ => return false,
        }
        true
    }
}

/// A byte of the results' enter data from offset 1 (`fn_8017AA78`'s `&arg0->x1`): `x1`,
/// the padding after it and `x4`'s top byte.
fn enter_byte(e: &ResultsMatchInfo, off: usize) -> u8 {
    match off {
        1 => e.x1,
        2 | 3 => 0,
        _ => (e.x4 >> 24) as u8,
    }
}

/// `ResultsMatchInfo` (`gmVsMelee_ResultsEnterData`): the results screen's enter data.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct ResultsMatchInfo {
    pub x0_0: bool,
    pub x0_1: bool,
    pub x1: u8,
    pub x4: i32,
    pub match_end: MatchEnd,
}

/// `fn_8017507C`'s colors: a team's light color, or white outside teams.
pub(super) fn column_color(me: &MatchEnd, slot: usize) -> [u8; 4] {
    if me.is_teams != 1 {
        return [0xFF; 4];
    }
    let p = &me.player_standings[slot];
    let c = if p.pkind == PKIND_NA {
        4
    } else {
        match p.team {
            0 => 0,
            1 => 1,
            2 => 3,
            3 => 2,
            _ => 4,
        }
    };
    let mut c = super::vs::hud_color(c);
    for v in &mut c[..3] {
        let x = fp::fmadds(f64::from(0.9_f32), fp::frsp(f64::from(*v)), 100.0);
        let x = if x > 255.0 { 255.0 } else { x };
        *v = crate::ppc::cvt_fp2unsigned(x) as u8;
    }
    c
}
