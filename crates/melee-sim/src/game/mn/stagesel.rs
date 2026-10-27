//! The stage select screen (`melee/mn/mnstagesel.c`): the stages' icons on their layout, the
//! cursor the control sticks move, the hovered stage's name and preview, and A (or Start) to
//! pick it, or the random icon's random pick (`mnStageSel_802599EC`, which keeps a short
//! memory of what it picked). The pick waits for "Now loading" before the scene ends with
//! the stage in the `SSSData`.

// The screen's loops run over its table by index, as the original's do, indexing its state's
// arrays of the same length alongside.
#![allow(clippy::needless_range_loop)]

use melee_data::archive::{ArchiveId, DataRef};

use super::super::gm::title::ModelDesc;
use super::super::handles::CodeRef;
use super::super::render::{JOBJ_CALLBACK, LOBJ_CALLBACK};
use super::super::state::{OBJ_CAMERA, OBJ_FOG, OBJ_JOBJ, OBJ_LIGHT, Obj, Running, UserData};
use crate::fpu::{mem, reg};
use crate::hsd::cobj;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId, type_mask};
use crate::vec::Vec3;
use gekko_fp as fp;

/// `mnStageSel_Scene_OnEnter` and `_OnFrame`.
pub const ON_ENTER: CodeRef = CodeRef(0x8025_A998);
pub const ON_FRAME: CodeRef = CodeRef(0x8025_B850);

/// The processes and render callback.
pub const NAME_PROC: u32 = 0x8025_9D84;
pub const PREVIEW_PROC: u32 = 0x8025_A090;
pub const CURSOR_PROC: u32 = 0x8025_A310;
pub const HOVER_PROC: u32 = 0x8025_A560;
pub const BG_PROC: u32 = 0x8025_A91C;
pub const FOG_RENDER: u32 = 0x8025_A974;

const ALL_MASK: u32 = 0x7FF;
const TOBJ_MASK: u32 = type_mask::TOBJ;
const HIDDEN: u32 = jobj::HIDDEN;

/// The stages the random pick draws from, and the entries past them (the random icon, then
/// none).
const NUM_STAGES: usize = 29;
const RANDOM: u8 = 29;
const NONE: u8 = 30;

/// A stage's entry (`StageListInfo` in `mnStageSel_803F06D0`): the fields the executable's
/// table holds and the screen only reads.
#[derive(Clone, Copy, Debug)]
pub struct StageDesc {
    /// The frame of its name and preview (`x9`), its random switch's bit (`xA`), its kind.
    pub frame: u8,
    pub switch: u8,
    pub stkind: u8,
    /// Its icon's half sizes for the cursor (`xC`, `x10`) and the hover frame's scale.
    pub half_w: f32,
    pub half_h: f32,
    pub scale_x: f32,
    pub scale_y: f32,
}

const fn s(frame: u8, switch: u8, stkind: u8, w: f32, h: f32, sx: f32, sy: f32) -> StageDesc {
    StageDesc {
        frame,
        switch,
        stkind,
        half_w: w,
        half_h: h,
        scale_x: sx,
        scale_y: sy,
    }
}

/// `mnStageSel_803F06D0`.
pub const STAGES: [StageDesc; 30] = [
    s(0x00, 0x00, 0x04, 3.1, 2.7, 1.0, 1.0),
    s(0x01, 0x0C, 0x0B, 3.1, 2.7, 1.0, 1.0),
    s(0x02, 0x01, 0x05, 3.1, 2.7, 1.0, 1.0),
    s(0x03, 0x0D, 0x0C, 3.1, 2.7, 1.0, 1.0),
    s(0x04, 0x02, 0x0D, 3.1, 2.7, 1.0, 1.0),
    s(0x05, 0x0E, 0x0E, 3.1, 2.7, 1.0, 1.0),
    s(0x08, 0x04, 0x08, 3.1, 2.7, 1.0, 1.0),
    s(0x09, 0x10, 0x10, 2.1, 2.7, 1.0, 1.0),
    s(0x0A, 0x05, 0x02, 3.1, 2.7, 1.0, 1.0),
    s(0x0B, 0x11, 0x11, 3.1, 2.7, 1.0, 1.0),
    s(0x0C, 0x06, 0x07, 3.1, 2.7, 1.0, 1.0),
    s(0x0D, 0x12, 0x16, 3.1, 2.7, 1.0, 1.0),
    s(0x06, 0x03, 0x06, 3.1, 2.7, 1.0, 1.0),
    s(0x07, 0x0F, 0x0F, 3.1, 2.7, 1.0, 1.0),
    s(0x12, 0x09, 0x09, 3.1, 2.7, 1.0, 1.0),
    s(0x13, 0x15, 0x12, 3.1, 2.7, 1.0, 1.0),
    s(0x10, 0x08, 0x0A, 3.1, 2.7, 1.0, 1.0),
    s(0x11, 0x14, 0x18, 3.1, 2.7, 1.0, 1.0),
    s(0x0E, 0x07, 0x03, 3.1, 2.7, 1.0, 1.0),
    s(0x0F, 0x13, 0x17, 3.1, 2.7, 1.0, 1.0),
    s(0x14, 0x0B, 0x13, 3.1, 2.7, 1.0, 1.0),
    s(0x15, 0x16, 0x14, 3.1, 2.7, 1.0, 1.0),
    s(0x16, 0x0A, 0x19, 3.1, 2.9, 1.0, 1.1),
    s(0x17, 0x17, 0x1B, 3.1, 2.9, 1.0, 1.1),
    s(0x18, 0x18, 0x1F, 2.9, 2.1, 0.8, 0.8),
    s(0x19, 0x19, 0x20, 2.9, 2.1, 0.8, 0.8),
    s(0x1A, 0x1A, 0x1C, 2.9, 2.1, 0.8, 0.8),
    s(0x1B, 0x1B, 0x1D, 2.9, 2.1, 0.8, 0.8),
    s(0x1C, 0x1C, 0x1E, 2.9, 2.1, 0.8, 0.8),
    s(0x1D, 0x00, 0x00, 3.6, 2.7, 1.2, 1.0),
];

/// A stage entry's fields the screen writes: its icon's joint, the random pick's memory
/// (`x4`: 0 free, above 0 frames to wait, -1 picked last), and whether it's shown
/// (`x8`: 0 hidden, 1 locked, 2 open).
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct Stage {
    pub jobj: Option<JObjId>,
    pub memory: i32,
    pub shown: u8,
}

impl Default for Stage {
    /// The executable's data: every entry shown open (2).
    fn default() -> Self {
        Stage {
            jobj: None,
            memory: 0,
            shown: 2,
        }
    }
}

/// The stage name's user data (`StageSelUserData`, 8 bytes from the heap): the stage it
/// names, its state and its frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct NameData {
    pub addr: u32,
    pub stage: u16,
    pub state: u16,
    pub frames: i32,
}

/// The screen's statics and the table it writes.
#[derive(Clone, Debug, PartialEq, melee_derive::StateEq)]
pub struct StageSel {
    /// `mnStageSel_804D6C94`: `MnSlMap`, with its blocks.
    pub archive: Option<(ArchiveId, u32, u32)>,
    pub cam: Option<DataRef>,
    pub lights: [Option<DataRef>; 2],
    pub fog: Option<DataRef>,
    /// icon large, random, special, stage name, stacked, menu border, preview, hover,
    /// cursor, layout, background, now loading.
    pub models: [ModelDesc; 12],
    /// `mnStageSel_804D6C9C`, the camera's GObj.
    pub camera: Option<GObjId>,
    /// `mnStageSel_804D6CA0`: the buttons pressed this frame.
    pub buttons: u32,
    /// `mnStageSel_804D6CA4`: frames the input waits.
    pub wait: u32,
    /// `mnStageSel_804D6CA8`: the background's frames.
    pub bg_frames: i32,
    /// `mnStageSel_804D6CAC`, `804D6CAD`: the stick past its dead zone.
    pub stick_x: i8,
    pub stick_y: i8,
    /// `mnStageSel_804D6CAE`: the hovered entry (30 for none).
    pub hovered: u8,
    /// `mnStageSel_804D6CAF`: 0 choosing, 1 loading, 2 picked.
    pub picked: u8,
    /// `mnStageSel_804D50A0`: the port that drives the cursor, -1 for any.
    pub port: i8,
    pub stages: [Stage; 30],
    /// The stage name's, the preview's (`StagePreviewUserData`: its stage and frames) and the
    /// hover frame's (its frames) user data, by their blocks.
    pub names: std::collections::BTreeMap<u32, NameData>,
    pub previews: std::collections::BTreeMap<u32, (u32, u32)>,
    pub hovers: std::collections::BTreeMap<u32, i32>,
}

impl Default for StageSel {
    fn default() -> Self {
        StageSel {
            archive: None,
            cam: None,
            lights: [None; 2],
            fog: None,
            models: [ModelDesc::default(); 12],
            camera: None,
            buttons: 0,
            wait: 0,
            bg_frames: 0,
            stick_x: 0,
            stick_y: 0,
            hovered: 0,
            picked: 0,
            port: -1,
            stages: [Stage::default(); 30],
            names: Default::default(),
            previews: Default::default(),
            hovers: Default::default(),
        }
    }
}

/// An entry's frame (`x9`): past the table, entry 30 (no stage hovered) reads the
/// string after it, `"MnSlMap.usd"` (`@629`): its 's' (docs/melee-bugs.md).
fn stage_frame(i: usize) -> u8 {
    match STAGES.get(i) {
        Some(s) => s.frame,
        None if i == usize::from(NONE) => b's',
        None => panic!("stage select entry {i} reads past the table and its string"),
    }
}

const ICON_LARGE: usize = 0;
const ICON_RANDOM: usize = 1;
const ICON_SPECIAL: usize = 2;
const STAGE_NAME: usize = 3;
const ICON_STACKED: usize = 4;
const MENU_BORDER: usize = 5;
const STAGE_PREVIEW: usize = 6;
const ICON_HOVER: usize = 7;
const CURSOR: usize = 8;
const LAYOUT: usize = 9;
const BACKGROUND: usize = 10;
const NOW_LOADING: usize = 11;

impl Running<'_> {
    /// `mnSelStageRandom`: a random stage's kind.
    #[doc(alias = "mnSelStageRandom")]
    pub fn random_stage(&mut self) -> u8 {
        let i = self.sss_random_entry();
        STAGES[i].stkind
    }

    /// Whether the random switch lets entry `i`'s stage come up (`gm_80164330` on its
    /// switch bit): unlocked, and on in the item switch's stages once the save has the
    /// switch.
    #[doc(alias = "gm_80164330")]
    fn sss_random_on(&self, switch: u8) -> bool {
        let stkind = super::super::gm::opening::stage_select_kind(switch);
        if !self.stage_unlocked(stkind) {
            return false;
        }
        if self.game.game_data.thing.save_data.x186C & 2 == 0 {
            return true;
        }
        let mask = self.game.game_data.thing.save_data.x1CB0.stage_mask;
        (1u32 << u32::from(switch)) & mask != 0
    }

    /// The random pick, `mnStageSel_802599EC`: every stage free again if none is, the
    /// waits counted down until one is free, then a free one drawn (the first if 100,000
    /// draws find none), kept from the next pick and its neighbour in its pair held for 3.
    #[doc(alias = "mnStageSel_802599EC")]
    pub fn sss_random_entry(&mut self) -> usize {
        let st = &self.game.stagesel.stages;
        let any =
            (0..NUM_STAGES).any(|i| st[i].memory >= 0 && self.sss_random_on(STAGES[i].switch));
        if !any {
            for s in &mut self.game.stagesel.stages[..NUM_STAGES] {
                s.memory = 0;
            }
        }
        loop {
            for s in &mut self.game.stagesel.stages[..NUM_STAGES] {
                if s.memory > 0 {
                    s.memory -= 1;
                }
            }
            let st = &self.game.stagesel.stages;
            if (0..NUM_STAGES).any(|i| st[i].memory == 0 && self.sss_random_on(STAGES[i].switch)) {
                break;
            }
        }
        let mut pick = None;
        for _ in 0..100_000 {
            let i = self.game.objs.rng.below(NUM_STAGES as i32) as usize;
            if self.game.stagesel.stages[i].memory == 0 && self.sss_random_on(STAGES[i].switch) {
                pick = Some(i);
                break;
            }
        }
        let i = pick.unwrap_or(0);
        self.game.stagesel.stages[i].memory = -1;
        if i < 22 {
            let other = if i & 1 != 0 { i - 1 } else { i + 1 };
            if self.game.stagesel.stages[other].memory >= 0 {
                self.game.stagesel.stages[other].memory = 3;
            }
        }
        i
    }

    /// `mnStageSel_80259C28`: A or Start on an open stage (Start alone on none, either on
    /// the random icon, which picks then) shows "Now loading" and starts the pick's wait;
    /// on a locked one the error sound.
    #[doc(alias = "mnStageSel_80259C28")]
    fn sss_confirm(&mut self) {
        let s = &self.game.stagesel;
        if s.wait != 0 {
            return;
        }
        match s.hovered {
            NONE => {
                if s.buttons & 0x1000 == 0 {
                    return;
                }
                self.game.stagesel.hovered = self.sss_random_entry() as u8;
            }
            RANDOM => {
                if s.buttons & 0x1100 == 0 {
                    return;
                }
                self.game.stagesel.hovered = self.sss_random_entry() as u8;
            }
            h => {
                if s.buttons & 0x1100 == 0 {
                    return;
                }
                if !(h < NONE && self.game.stagesel.stages[usize::from(h)].shown >= 2) {
                    self.lb_audio_call(3);
                    return;
                }
            }
        }
        let (gobj, j) = self.sss_model(NOW_LOADING, 0x87, super::main::BACK_PROC, 0);
        let _ = gobj;
        jobj::req_anim_all(&mut self.game.objs, Some(j), 0.0);
        self.sss_anim_all(Some(j));
        let s = &mut self.game.stagesel;
        s.picked = 1;
        s.wait = 0x1E;
        self.lb_audio_call(1);
    }

    fn sss_anim_all(&mut self, j: Option<JObjId>) {
        jobj::anim_all(
            &mut self.game.objs,
            self.data,
            &mut super::super::code::ParticleHooks,
            j,
        );
    }

    fn sss_stop(&mut self, j: Option<JObjId>, mask: u32) {
        let Some(j) = j else { return };
        let data = self.data;
        jobj::foreach_anim(&mut self.game.objs, j, mask, &mut |objs, a| {
            crate::hsd::aobj::stop_anim(objs, data, Some(a), None);
        });
    }

    /// A model of the screen's on a new GObj (class `class`, process link 5) on render link
    /// 4 at `prio`, the main menu's back process on it at `proc_prio`.
    fn sss_model_class(
        &mut self,
        which: usize,
        class: u16,
        prio: u8,
        proc: u32,
        proc_prio: u8,
    ) -> (GObjId, JObjId) {
        let data = self.data;
        let m = self.game.stagesel.models[which];
        let g = &mut *self.game;
        let gobj = g.world.create(class, 5, 0x80);
        let j = jobj::load_joint(&mut g.objs, data, m.joint).expect("a stage select model");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 4, prio);
        g.world.setup_proc(gobj, CodeRef(proc), proc_prio);
        jobj::add_anim_all(&mut g.objs, data, Some(j), m.anim, m.matanim, m.shapeanim);
        (gobj, j)
    }

    fn sss_model(&mut self, which: usize, prio: u8, proc: u32, proc_prio: u8) -> (GObjId, JObjId) {
        // `HSD_GOBJ_CLASS_FIGHTER` (4), the class the stage's name and "Now loading" take, as
        // the others' 4.
        self.sss_model_class(which, 4, prio, proc, proc_prio)
    }

    /// The inline `do_anim`: every animation from 0, the textures from `frame`, held.
    fn sss_do_anim(&mut self, j: Option<JObjId>, frame: f32) {
        jobj::req_anim_all(&mut self.game.objs, j, 0.0);
        jobj::req_anim_all_by_flags(&mut self.game.objs, j, 0x10, frame);
        self.sss_anim_all(j);
        self.sss_stop(j, TOBJ_MASK);
    }

    /// A stage's name (`mnStageSel_80259ED8`): its model on render link 4 (0x84) with the
    /// name process, at the stage's frame for an open stage.
    #[doc(alias = "mnStageSel_80259ED8")]
    fn sss_name(&mut self, id: u8) {
        let (gobj, j) = self.sss_model(STAGE_NAME, 0x84, super::main::BACK_PROC, 1);
        let addr = crate::hsd::heap::mem_alloc(8);
        self.game
            .world
            .init_user_data(gobj, 4, UserData::SssName(addr));
        self.game.world.setup_proc(gobj, CodeRef(NAME_PROC), 1);
        self.game.stagesel.names.insert(
            addr,
            NameData {
                addr,
                stage: u16::from(id),
                state: 0,
                frames: 0,
            },
        );
        if id < NONE && self.game.stagesel.stages[usize::from(id)].shown >= 2 {
            let f = mem(fp::fmuls(
                20.0,
                f64::from(f32::from(STAGES[usize::from(id)].frame)),
            ));
            self.sss_do_anim(Some(j), f);
        }
    }

    /// `fn_80259D84`, a stage's name: coming in (8 frames, A or Start taken meanwhile),
    /// shown until the hovered stage changes, which brings the next name in as this one goes
    /// (10 frames) and is freed.
    #[doc(alias = "fn_80259D84")]
    pub fn sss_name_proc(&mut self, gobj: GObjId) {
        let Some(UserData::SssName(addr)) = self.game.world.gobj(gobj).user_data else {
            panic!("a stage name without its user data")
        };
        let j = match self.game.world.gobj(gobj).obj {
            Some(Obj::JObj(j)) => Some(j),
            _ => None,
        };
        let d = self.game.stagesel.names[&addr];
        match d.state {
            0 => {
                if self.game.stagesel.picked == 0 {
                    self.sss_confirm();
                }
                let d = self.game.stagesel.names.get_mut(&addr).expect("a name");
                d.frames += 1;
                if d.frames as u32 >= 9 {
                    d.state = 1;
                    self.sss_stop(j, ALL_MASK);
                }
            }
            1 => {
                if self.game.stagesel.picked == 0 {
                    self.sss_confirm();
                }
                let hovered = self.game.stagesel.hovered;
                if d.stage != u16::from(hovered) {
                    if d.stage < u16::from(NONE)
                        && self.game.stagesel.stages[usize::from(d.stage)].shown >= 2
                    {
                        jobj::req_anim_all_by_flags(&mut self.game.objs, j, 1, 10.0);
                    }
                    self.lb_audio_call(2);
                    let d = self.game.stagesel.names.get_mut(&addr).expect("a name");
                    d.frames = 0;
                    d.state = 2;
                    self.sss_name(hovered);
                }
            }
            2 => {
                let d = self.game.stagesel.names.get_mut(&addr).expect("a name");
                d.frames += 1;
                if d.frames as u32 > 0xA {
                    crate::hsd::gobj::free(self, gobj);
                    if let Some(d) = self.game.stagesel.names.get_mut(&addr) {
                        d.state += 1;
                    }
                }
            }
            _ => {}
        }
    }

    /// `fn_8025A090`, the preview: the hovered open stage's picture (none for a locked one),
    /// coming in over 20 frames and held from 69; once a pick waits, it ends the wait at 90.
    #[doc(alias = "fn_8025A090")]
    pub fn sss_preview_proc(&mut self, gobj: GObjId) {
        let Some(UserData::SssPreview(addr)) = self.game.world.gobj(gobj).user_data else {
            panic!("a preview without its user data")
        };
        let j = match self.game.world.gobj(gobj).obj {
            Some(Obj::JObj(j)) => Some(j),
            _ => None,
        };
        let h = self.game.stagesel.hovered;
        let mut v = u32::from(h);
        // No stage hovered (30) reads past the table (`shown` is the string after it's 'u'),
        // which leaves 30.
        if h < NONE && self.game.stagesel.stages[usize::from(h)].shown < 2 {
            v = u32::from(NONE);
        }
        let (stage, _) = self.game.stagesel.previews[&addr];
        if stage != v {
            self.game.stagesel.previews.insert(addr, (v, 0));
            if v < 0x1D {
                let f = mem(fp::fmuls(
                    50.0,
                    f64::from(f32::from(STAGES[v as usize].frame)),
                ));
                jobj::req_anim_all(&mut self.game.objs, j, f);
                self.sss_anim_all(j);
                self.sss_stop(j, ALL_MASK);
                self.sss_translate_x(j, 0.0);
            } else {
                self.sss_translate_x(j, 100.0);
            }
        }
        let (stage, frames) = self.game.stagesel.previews[&addr];
        if frames < 0x5A {
            let frames = frames + 1;
            self.game.stagesel.previews.insert(addr, (stage, frames));
            if frames == 0x14 {
                let f = mem(fp::fmuls(
                    50.0,
                    f64::from(f32::from(stage_frame(stage as usize))),
                ));
                jobj::req_anim_all(&mut self.game.objs, j, f);
            }
            if frames == 0x45 {
                self.sss_stop(j, ALL_MASK);
            }
        } else if self.game.stagesel.picked != 0 {
            self.game.stagesel.picked = 2;
        }
    }

    fn sss_translate_x(&mut self, j: Option<JObjId>, x: f32) {
        let j = j.expect("HSD_JObjSetTranslateX: jobj");
        jobj::set_srt(&mut self.game.objs.jobjs, j, |o| o.translate.x = x);
    }

    /// `fn_8025A310`, the cursor: hidden once a pick waits, else moved by the stick within
    /// its bounds, and the stage whose icon it's over hovered.
    #[doc(alias = "fn_8025A310")]
    pub fn sss_cursor_proc(&mut self, gobj: GObjId) {
        let j = match self.game.world.gobj(gobj).obj {
            Some(Obj::JObj(j)) => j,
            _ => panic!("the cursor without its joint"),
        };
        if self.game.stagesel.picked != 0 {
            jobj::set_flags(&mut self.game.objs.jobjs, Some(j), HIDDEN);
            return;
        }
        let mut p = self.game.objs.jobjs[j].translate;
        let sx = f64::from(self.game.stagesel.stick_x as f32);
        p.x = mem(fp::fmadds(reg(0.03), sx, reg(p.x)));
        if reg(-27.0) > reg(p.x) {
            p.x = -27.0;
        }
        if reg(27.0) < reg(p.x) {
            p.x = 27.0;
        }
        let sy = f64::from(self.game.stagesel.stick_y as f32);
        p.y = mem(fp::fmadds(reg(0.03), sy, reg(p.y)));
        if reg(-19.0) > reg(p.y) {
            p.y = -19.0;
        }
        if reg(19.0) < reg(p.y) {
            p.y = 19.0;
        }
        jobj::set_translate(&mut self.game.objs.jobjs, j, p);
        let c = crate::lb::joint::world_position(&mut self.game.objs, self.data, Some(j), None);
        for i in 0..30 {
            if self.game.stagesel.stages[i].shown == 0 {
                continue;
            }
            let ij = self.game.stagesel.stages[i].jobj;
            let q = crate::lb::joint::world_position(&mut self.game.objs, self.data, ij, None);
            let (w, h) = (reg(STAGES[i].half_w), reg(STAGES[i].half_h));
            if fp::fsubs(reg(q.x), w) < reg(c.x)
                && fp::fadds(reg(q.x), w) > reg(c.x)
                && fp::fsubs(reg(q.y), h) < reg(c.y)
                && fp::fadds(reg(q.y), h) > reg(c.y)
            {
                self.game.stagesel.hovered = i as u8;
                return;
            }
        }
    }

    /// `fn_8025A560`, the hover frame: on the hovered stage's icon at its scale (away for a
    /// hidden one, a locked pair's second, or none), its animation restarted every ten
    /// frames, every frame once a pick waits.
    #[doc(alias = "fn_8025A560")]
    pub fn sss_hover_proc(&mut self, gobj: GObjId) {
        let Some(UserData::SssHover(addr)) = self.game.world.gobj(gobj).user_data else {
            panic!("the hover frame without its user data")
        };
        let j = match self.game.world.gobj(gobj).obj {
            Some(Obj::JObj(j)) => j,
            _ => panic!("the hover frame without its joint"),
        };
        let h = self.game.stagesel.hovered;
        let hu = usize::from(h);
        if h < 0x16 && h & 1 != 0 && self.game.stagesel.stages[hu].shown == 0 {
            self.sss_translate_x(Some(j), 100.0);
        } else if h < NONE {
            let ij = self.game.stagesel.stages[hu].jobj;
            let q = crate::lb::joint::world_position(&mut self.game.objs, self.data, ij, None);
            let o = &mut self.game.objs.jobjs;
            jobj::set_srt(o, j, |x| x.translate.x = q.x);
            jobj::set_srt(o, j, |x| x.translate.y = q.y);
            jobj::set_srt(o, j, |x| x.scale.x = STAGES[hu].scale_x);
            jobj::set_srt(o, j, |x| x.scale.y = STAGES[hu].scale_y);
        } else {
            self.sss_translate_x(Some(j), 100.0);
        }
        if self.game.stagesel.picked != 0 {
            jobj::req_anim_all(&mut self.game.objs, Some(j), 0.0);
            self.sss_anim_all(Some(j));
            return;
        }
        let c = self
            .game
            .stagesel
            .hovers
            .get_mut(&addr)
            .expect("the hover frame's count");
        *c += 1;
        if *c >= 10 {
            *c = 0;
            jobj::req_anim_all(&mut self.game.objs, Some(j), 0.0);
            self.sss_anim_all(Some(j));
        }
    }

    /// `fn_8025A91C`, the background: its animations restarted every 250 frames.
    #[doc(alias = "fn_8025A91C")]
    pub fn sss_bg_proc(&mut self, gobj: GObjId) {
        let j = match self.game.world.gobj(gobj).obj {
            Some(Obj::JObj(j)) => Some(j),
            _ => None,
        };
        let s = &mut self.game.stagesel;
        s.bg_frames += 1;
        if s.bg_frames >= 0xFA {
            s.bg_frames = 0;
            jobj::req_anim_all(&mut self.game.objs, j, 0.0);
            self.sss_anim_all(j);
        }
    }

    /// `fn_8025A974`, the fog's render callback.
    #[doc(alias = "fn_8025A974")]
    pub fn sss_fog_render(&mut self, gobj: GObjId) {
        self.render_fog(gobj);
    }

    /// Runs one of the screen's processes.
    pub fn stagesel_run(&mut self, gobj: GObjId, f: u32) -> bool {
        match f {
            NAME_PROC => self.sss_name_proc(gobj),
            PREVIEW_PROC => self.sss_preview_proc(gobj),
            CURSOR_PROC => self.sss_cursor_proc(gobj),
            HOVER_PROC => self.sss_hover_proc(gobj),
            BG_PROC => self.sss_bg_proc(gobj),
            _ => return false,
        }
        true
    }

    /// The stage select's enter and exit data (`gm_GetGameModeStateEnterData`): Tournament
    /// mode's own (`gm_8048BD20`), else VS mode's (the modes kept with it share it).
    fn sss_data(&self) -> &super::super::gm::vs_melee::SssData {
        if super::common::game_mode(self) == super::common::GM_TOURNAMENT {
            &self.game.tou.sss
        } else {
            &self.game.vs_melee.sss
        }
    }

    fn sss_data_mut(&mut self) -> &mut super::super::gm::vs_melee::SssData {
        if super::common::game_mode(self) == super::common::GM_TOURNAMENT {
            &mut self.game.tou.sss
        } else {
            &mut self.game.vs_melee.sss
        }
    }

    /// `mnStageSel_Scene_OnEnter`: with no stage forced, `MnSlMap`'s screen: the camera (the
    /// main menu's process turning it), the lights, the fog, the background, the border,
    /// the layout's nineteen places, the icons on them (pairs, the special and the large
    /// ones, the random one) shown by the unlocks, the cursor at its start, the stage name,
    /// the preview, the hover frame, and the screen's song.
    #[doc(alias = "mnStageSel_Scene_OnEnter")]
    pub fn stagesel_on_enter(&mut self) {
        if self.sss_data().force_stage_id >= 0 {
            return;
        }
        let us = self.game.game_data.saved_language() == super::super::gm::main_lib::LANG_US;
        let name = if us { "MnSlMap.usd" } else { "MnSlMap.dat" };
        let a = self.load_archive_blocks(name);
        let at = self
            .data
            .get(a.0)
            .public("MnSelectStageDataTable")
            .expect("the original's assertion failed: MnSelectStageDataTable isn't in MnSlMap");
        let table = DataRef {
            archive: a.0,
            offset: at,
        };
        let p = |o: u32| self.data.ptr(table.at(o));
        let s = &mut self.game.stagesel;
        s.archive = Some(a);
        s.cam = p(0);
        s.lights = [p(4), p(8)];
        s.fog = p(0xC);
        for (i, m) in s.models.iter_mut().enumerate() {
            let at = 0x10 + 0x10 * i as u32;
            *m = ModelDesc {
                joint: p(at),
                anim: p(at + 4),
                matanim: p(at + 8),
                shapeanim: p(at + 0xC),
            };
        }
        self.game.menu.cam = self.game.stagesel.cam;
        let unk = self.sss_data().unk_stage;
        let s = &mut self.game.stagesel;
        s.picked = 0;
        s.buttons = 0;
        s.stick_x = 0;
        s.stick_y = 0;
        s.hovered = NONE;
        s.port = unk.wrapping_sub(1) as i8;
        s.wait = 0x14;
        self.sss_camera();
        self.sss_lights();
        self.sss_fog();
        let (gobj, _) = self.sss_model_class(BACKGROUND, 4, 0x80, super::main::BACK_PROC, 0);
        let j = match self.game.world.gobj(gobj).obj {
            Some(Obj::JObj(j)) => Some(j),
            _ => None,
        };
        self.game.world.setup_proc(gobj, CodeRef(BG_PROC), 0);
        jobj::req_anim_all(&mut self.game.objs, j, 0.0);
        self.sss_anim_all(j);
        let (_, border) = self.sss_model_class(MENU_BORDER, 4, 0x82, super::main::BACK_PROC, 0);
        jobj::req_anim_all(&mut self.game.objs, Some(border), 0.0);
        self.sss_anim_all(Some(border));
        let (_, layout) = self.sss_model_class(LAYOUT, 4, 0x82, super::main::BACK_PROC, 4);
        let mut places = [None; 0x13];
        places[0] = self.game.objs.jobjs[layout].child;
        jobj::req_anim_all(&mut self.game.objs, places[0], 0.0);
        self.sss_anim_all(places[0]);
        for i in 0..0x12 {
            places[i + 1] = places[i].and_then(|p| self.game.objs.jobjs[p].next);
            jobj::req_anim_all(&mut self.game.objs, places[i + 1], 0.0);
            self.sss_anim_all(places[i + 1]);
        }
        for i in 0..0x1D {
            let open = self.stage_unlocked(u16::from(STAGES[i].stkind));
            self.game.stagesel.stages[i].shown = if open { 2 } else { 1 };
        }
        for i in 0..=0xA {
            let (_, icon) = self.sss_model_class(ICON_STACKED, 4, 0x83, super::main::BACK_PROC, 3);
            crate::lb::joint::constrain_to(&mut self.game.objs, Some(icon), places[i]);
            let child = self.game.objs.jobjs[icon].child;
            let second = child.and_then(|c| self.game.objs.jobjs[c].next);
            for (k, j) in [(i * 2, second), (i * 2 + 1, child)] {
                self.game.stagesel.stages[k].jobj = j;
                match self.game.stagesel.stages[k].shown {
                    0 => jobj::set_flags(&mut self.game.objs.jobjs, j, HIDDEN),
                    1 => jobj::req_anim_all_by_flags(&mut self.game.objs, j, 0x10, 1.0),
                    _ => {
                        let f = i32::from(STAGES[k].frame) / 2 + 2;
                        jobj::req_anim_all_by_flags(&mut self.game.objs, j, 0x10, f as f32);
                    }
                }
            }
            self.sss_anim_all(Some(icon));
            self.sss_stop(Some(icon), TOBJ_MASK);
        }
        for i in 0xB..=0xF {
            let (_, icon) = self.sss_model_class(ICON_SPECIAL, 4, 0x83, super::main::BACK_PROC, 3);
            self.sss_place_icon(icon, places[i], i + 13, 0x16);
        }
        let (_, icon) = self.sss_model_class(ICON_RANDOM, 4, 0x83, super::main::BACK_PROC, 3);
        crate::lb::joint::constrain_to(&mut self.game.objs, Some(icon), places[0x10]);
        self.sss_do_anim(Some(icon), 2.0);
        self.game.stagesel.stages[0x1D].jobj = Some(icon);
        for i in 0x11..=0x12 {
            let (_, icon) = self.sss_model_class(ICON_LARGE, 4, 0x83, super::main::BACK_PROC, 3);
            self.sss_place_icon(icon, places[i], i + 5, 0x14);
        }
        let (gobj, j) = self.sss_model_class(CURSOR, 4, 0x86, super::main::BACK_PROC, 2);
        self.game.world.setup_proc(gobj, CodeRef(CURSOR_PROC), 2);
        let port = self.game.stagesel.port;
        self.sss_do_anim(Some(j), f32::from(i16::from(port) + 1));
        jobj::set_translate(&mut self.game.objs.jobjs, j, Vec3::new(0.0, -13.0, 0.0));
        let (gobj, _) = self.sss_model_class(STAGE_NAME, 4, 0x84, super::main::BACK_PROC, 1);
        let addr = crate::hsd::heap::mem_alloc(8);
        self.game
            .world
            .init_user_data(gobj, 4, UserData::SssName(addr));
        self.game.world.setup_proc(gobj, CodeRef(NAME_PROC), 1);
        self.game.stagesel.names.insert(
            addr,
            NameData {
                addr,
                stage: u16::from(NONE),
                state: 0,
                frames: 0,
            },
        );
        let (gobj, j) = self.sss_model_class(STAGE_PREVIEW, 4, 0x81, super::main::BACK_PROC, 1);
        let addr = crate::hsd::heap::mem_alloc(8);
        self.game
            .world
            .init_user_data(gobj, 4, UserData::SssPreview(addr));
        self.game.world.setup_proc(gobj, CodeRef(PREVIEW_PROC), 1);
        jobj::req_anim_all(&mut self.game.objs, Some(j), 0.0);
        self.sss_anim_all(Some(j));
        self.sss_stop(Some(j), ALL_MASK);
        self.sss_translate_x(Some(j), 100.0);
        self.game
            .stagesel
            .previews
            .insert(addr, (u32::from(NONE), 0));
        let (gobj, j) = self.sss_model_class(ICON_HOVER, 4, 0x85, super::main::BACK_PROC, 1);
        let addr = crate::hsd::heap::mem_alloc(4);
        self.game
            .world
            .init_user_data(gobj, 4, UserData::SssHover(addr));
        self.game.stagesel.hovers.insert(addr, 0);
        self.game.world.setup_proc(gobj, CodeRef(HOVER_PROC), 1);
        self.sss_translate_x(Some(j), 100.0);
        let bgm = self.game.game_data.x1850.bgm;
        self.lb_play_music(i32::from(bgm));
    }

    /// A special or large icon on its place: hidden if locked (and then counted hidden),
    /// else its texture frame (its stage's frame less `base`).
    fn sss_place_icon(&mut self, icon: JObjId, place: Option<JObjId>, entry: usize, base: i32) {
        crate::lb::joint::constrain_to(&mut self.game.objs, Some(icon), place);
        self.game.stagesel.stages[entry].jobj = Some(icon);
        match self.game.stagesel.stages[entry].shown {
            0 | 1 => {
                self.game.stagesel.stages[entry].shown = 0;
                jobj::set_flags_all(&mut self.game.objs.jobjs, Some(icon), HIDDEN);
            }
            _ => {
                let f = i32::from(STAGES[entry].frame) - base;
                self.sss_do_anim(Some(icon), f as f32);
            }
        }
    }

    fn sss_camera(&mut self) {
        let data = self.data;
        let desc = self.game.stagesel.cam;
        let g = &mut *self.game;
        let gobj = g.world.create(2, 3, 0x80);
        g.stagesel.camera = Some(gobj);
        let c = cobj::load_desc(&mut g.objs, data, desc).expect("the stage select's camera");
        g.world.set_obj(gobj, OBJ_CAMERA, Obj::CObj(c));
        g.world.setup_gx_link_max(
            gobj,
            CodeRef(super::super::render::CAMERA_CALLBACK),
            0,
            false,
        );
        g.world.gobj_mut(gobj).gxlink_prios = 0x11;
        g.world
            .setup_proc(gobj, CodeRef(super::main::CAMERA_PROC), 5);
    }

    fn sss_lights(&mut self) {
        let data = self.data;
        let [d0, d1] = self.game.stagesel.lights;
        let g = &mut *self.game;
        let gobj = g.world.create(3, 4, 0x80);
        let l0 = crate::hsd::lobj::load_desc(&mut g.objs, data, d0);
        let l1 = crate::hsd::lobj::load_desc(&mut g.objs, data, d1);
        if let Some(l) = l0 {
            g.objs.lobjs[l].next = l1;
        }
        g.world
            .set_obj(gobj, OBJ_LIGHT, Obj::LObj(l0.expect("the screen's light")));
        g.world.setup_gx_link(gobj, CodeRef(LOBJ_CALLBACK), 0, 0x80);
    }

    fn sss_fog(&mut self) {
        let data = self.data;
        let desc = self.game.stagesel.fog.expect("the screen's fog");
        let g = &mut *self.game;
        let gobj = g.world.create(0xE, 0xF, 0);
        let f = crate::hsd::fog::load_desc(&mut g.objs.fogs, data, desc);
        g.world.set_obj(gobj, OBJ_FOG, Obj::Fog(f));
        g.world.setup_gx_link(gobj, CodeRef(FOG_RENDER), 0, 0x80);
    }

    /// `mnStageSel_Scene_OnFrame`: a forced stage ends the scene at once; L, R and Start back
    /// to the main menu; the buttons and stick of the cursor's port (any, the first stick
    /// out of its dead zone), the dead zone taken off; the entry's wait; B back; a pick done
    /// ends the scene with its stage.
    #[doc(alias = "mnStageSel_Scene_OnFrame")]
    pub fn stagesel_on_frame(&mut self) {
        let sss = self.sss_data();
        let (forced, no_lras) = (sss.force_stage_id, sss.no_lras);
        if forced >= 0 {
            self.game.stagesel.picked = 2;
            self.sss_data_mut().vs.start.rules.stkind = forced as u8 as u16;
            self.game.end_scene();
            return;
        }
        if no_lras == 0 && self.game.buttons_triggered(4) & super::main::PAD_LR_START != 0 {
            self.lb_audio_call(0);
            self.rumble_stop_all();
            if let Some(c) = self.game.stagesel.camera {
                crate::hsd::gobj::free(self, c);
            }
            self.game.game_data.x1850.force_main_menu = 1;
            self.change_game_mode_after_scene(super::super::gm::machine::GM_MENU);
            self.game.end_scene();
            return;
        }
        let pads = &self.game.pads.copy;
        let s = &mut self.game.stagesel;
        if s.port < 0 {
            s.buttons = pads[0].trigger | pads[1].trigger | pads[2].trigger | pads[3].trigger;
            for p in pads {
                s.stick_x = p.stick_x;
                s.stick_y = p.stick_y;
                if !(-0x1E..=0x1E).contains(&p.stick_x) || !(-0x1E..=0x1E).contains(&p.stick_y) {
                    break;
                }
            }
        } else {
            let p = &pads[s.port as usize];
            s.buttons = p.trigger;
            s.stick_x = p.stick_x;
            s.stick_y = p.stick_y;
        }
        let dz = |v: i8| -> i8 {
            if v < -0x1E {
                v + 0x1E
            } else if v > 0x1E {
                v - 0x1E
            } else {
                0
            }
        };
        s.stick_x = dz(s.stick_x);
        s.stick_y = dz(s.stick_y);
        if s.wait != 0 {
            s.wait -= 1;
            return;
        }
        if self.sss_data().x1 == 0
            && self.game.stagesel.buttons & 0x200 != 0
            && self.game.stagesel.picked == 0
        {
            self.lb_audio_call(0);
            self.game.end_scene();
        }
        if self.game.stagesel.picked == 2 {
            let h = usize::from(self.game.stagesel.hovered);
            self.sss_data_mut().vs.start.rules.stkind = u16::from(STAGES[h].stkind);
            self.game.end_scene();
        }
    }

    /// `mnStageSel_Scene_OnExit`: the archive let go; whether the match starts, and then
    /// the preload cache's game cache on its stage, refreshed.
    #[doc(alias = "mnStageSel_Scene_OnExit")]
    pub fn stagesel_on_exit(&mut self) {
        if let Some((_, file, archive)) = self.game.stagesel.archive.take() {
            self.free_archive_blocks(file, archive);
        }
        let start = self.game.stagesel.picked == 2;
        self.sss_data_mut().start_game = u8::from(start);
        if start {
            let stkind = i32::from(self.sss_data().vs.start.rules.stkind);
            self.dvd_mut().cache.scene.game_cache.stkind = stkind;
            self.preload_refresh();
        }
    }
}
