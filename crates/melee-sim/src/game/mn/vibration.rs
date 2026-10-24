//! The rumble options (`melee/mn/mnvibration.c`): a panel for each port, which its own A
//! turns rumble on or off (a pulse when on), and with names saved, the names' list, which a
//! port reaches with right and leaves with left: up and down choose a name (the list
//! scrolling past eight) and A turns its rumble on or off. B leaves for the Options list and
//! the save is written.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::gm::title::ModelDesc;
use super::super::handles::CodeRef;
use super::super::render::JOBJ_CALLBACK;
use super::super::state::{OBJ_JOBJ, Obj, Running, UserData};
use super::options::{
    EXIT_TO, HIDDEN, MOBJ_MASK, RUMBLE, SETTINGS, SFX_BACK, SFX_FORWARD, SFX_MOVE, al,
};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::sis::TextId;
use crate::input::rumble::{Script, Table};
use crate::vec::Vec3;

/// The processes.
pub const HANDLE_INPUT: u32 = 0x8024_7510;
pub const CURSOR_THINK: u32 = 0x8024_8084;
pub const ON_ANIM_COMPLETE: u32 = 0x8024_8748;
pub const THINK: u32 = 0x8024_87A8;
pub const INTRO: u32 = 0x8024_8A78;

/// `mnVibration_PortPanelJointIds`: each port's panel among the screen's joints.
const PANEL_IDS: [usize; 4] = [0x16, 0x15, 0x14, 0x13];
/// `mnVibration_803EECE0`, the screen coming in; `mnVibration_803EECF8`, a panel's rumble
/// lights.
const INTRO_LOOP: super::super::gm::title::AnimLoop = al(0.0, 20.0, -0.1);
const LIGHTS: super::super::gm::title::AnimLoop = al(0.0, 14.0, -0.1);
/// `mnVibration_803EED04`: where a name's text goes from the names' joint.
const NAME_OFFSET: Vec3 = Vec3 {
    x: -0.4,
    y: 0.5,
    z: 0.0,
};
/// `mnVibration_804DC050` to `mnVibration_804DC060`: the intro's frames that show each
/// port's panel, then the names.
const REVEAL: [f32; 5] = [10.0, 11.0, 12.0, 13.0, 14.0];

/// `MnVibrationData`, the screen's user data (0x98 bytes of the heap).
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct VibrationData {
    /// `x0[0]`: the intro's ticks before input; `x0[1]`: the names' cursor row.
    pub intro: u8,
    pub row: u8,
    /// `x0[2..6]`: whether each port is on the names (1) or its own panel (0).
    pub on_names: [u8; 4],
    /// `x6`: whether each port's controller was in when last looked at.
    pub connected: [u8; 4],
    pub scroll: u8,
    /// The screen's first 25 joints in pre-order.
    pub jobjs: [Option<JObjId>; 25],
    /// The names' texts, by row.
    pub texts: [Option<TextId>; 8],
    pub title: Option<TextId>,
    /// The names' cursor, made once the intro shows the names; unset (the heap's bytes)
    /// before, and with no names.
    pub cursor: Option<GObjId>,
}

/// `mnvibration.c`'s statics: the models (`mnVibration_804A0868` the names' cursor, joint
/// only; `mnVibration_804A0878` a port's panel; `mnVibration_804A0888` a name's switch;
/// `mnVibration_804A0898` the screen), the screen's GObj (`mnVibration_804D6C28`) and its
/// user data.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Vibration {
    pub models: [ModelDesc; 4],
    pub gobj: Option<GObjId>,
    pub data: std::collections::BTreeMap<u32, VibrationData>,
    /// The last screen's block and the title text it left in it, which the next screen's
    /// creation reads before writing (`mnVibration_CreateScreen`'s `title_text`; the
    /// engine notes, "Uninitialized values").
    pub left: Option<(u32, Option<TextId>)>,
}

const CURSOR: usize = 0;
const PANEL: usize = 1;
const SWITCH: usize = 2;
const SCREEN: usize = 3;

impl Running<'_> {
    fn vibration_addr(&self) -> u32 {
        let g = self.game.options.vibration.gobj.expect("the rumble screen");
        match self.game.world.gobj(g).user_data {
            Some(UserData::Vibration(a)) => a,
            other => panic!("the rumble screen holds {other:?}"),
        }
    }

    fn vibration(&self) -> VibrationData {
        self.game.options.vibration.data[&self.vibration_addr()]
    }

    fn vibration_mut(&mut self) -> &mut VibrationData {
        let a = self.vibration_addr();
        self.game
            .options
            .vibration
            .data
            .get_mut(&a)
            .expect("the rumble screen's data")
    }

    /// `GetRumbleSettingOfPort`.
    #[doc(alias = "GetRumbleSettingOfPort")]
    pub fn port_rumble(&self, port: usize) -> u8 {
        self.game.game_data.thing.save_data.x1CB0.rumble_enabled[port]
    }

    /// `gmMainLib_SetRumbleEnabled`.
    #[doc(alias = "gmMainLib_SetRumbleEnabled")]
    pub fn set_port_rumble(&mut self, port: usize, on: u8) {
        self.game.game_data.thing.save_data.x1CB0.rumble_enabled[port] = on;
    }

    /// The name tags' slot the list shows on `row` (`mnVibration_GetNameSlotRaw`), 0xFF for
    /// none.
    fn name_slot_raw(&self, row: i32) -> i32 {
        let scroll = i32::from(self.vibration().scroll);
        let count = self.name_count();
        if count < 8 && row >= count {
            return 0xFF;
        }
        let idx = scroll + row;
        if count <= idx {
            return 0xFF;
        }
        idx
    }

    fn name_slot(&self, row: i32) -> u8 {
        self.name_slot_raw(row) as u8
    }

    fn name_rumble(&mut self, slot: u8) -> u8 {
        self.game
            .game_data
            .name_tag(usize::from(slot))
            .rumble_enabled
    }

    /// `mnVibration_GetNameRowJObj`: the switch of the names' row `row`.
    #[doc(alias = "mnVibration_GetNameRowJObj")]
    fn name_row_jobj(&self, row: i32) -> Option<JObjId> {
        let jobjs = &self.game.objs.jobjs;
        let mut j = self.vibration().jobjs[17].and_then(|j| jobjs[j].child);
        for _ in 0..row {
            j = j.and_then(|j| jobjs[j].next);
        }
        j
    }

    /// A port's panel: the `port`th child of the panels' joint.
    fn port_panel(&self, port: usize) -> Option<JObjId> {
        let jobjs = &self.game.objs.jobjs;
        let mut j = self.vibration().jobjs[23].and_then(|j| jobjs[j].child);
        for _ in 0..port {
            j = j.and_then(|j| jobjs[j].next);
        }
        j
    }

    fn translate_of(&self, j: Option<JObjId>) -> Vec3 {
        self.game.objs.jobjs[j.expect("jobj")].translate
    }

    /// The names' cursor on its row: at the names' joint, a row's spacing down for each.
    fn vibration_place_cursor(&mut self) {
        let d = self.vibration();
        let Some(Obj::JObj(c)) = d.cursor.and_then(|g| self.game.world.gobj(g).obj) else {
            panic!("the names' cursor without its model");
        };
        let t17 = self.translate_of(d.jobjs[17]);
        let base_y = reg(t17.y);
        let spacing = fp::fsubs(reg(self.translate_of(d.jobjs[18]).y), base_y);
        let jobjs = &mut self.game.objs.jobjs;
        jobj::set_srt(jobjs, c, |o| o.translate.x = t17.x);
        let y = fp::fmadds(spacing, fp::frsp(f64::from(d.row)), reg(t17.y));
        jobj::set_srt(jobjs, c, |o| o.translate.y = mem(y));
        jobj::set_srt(jobjs, c, |o| o.translate.z = t17.z);
    }

    /// `mnVibration_HandleInput`: B leaves (the names' texts and the title freed, the screen
    /// gone, rumble stopped, the save written); after the intro, each port's A on its own
    /// panel turns its rumble off or on (a pulse when on); with names saved, left and right
    /// move a port between its panel and the names, where any of their ports' up and down
    /// move the cursor (scrolling past eight) and A turns a name's rumble off or on.
    #[doc(alias = "mnVibration_HandleInput")]
    pub fn vibration_handle_input(&mut self, _gobj: GObjId) {
        if self.game.menu.cooldown != 0 {
            self.menu_cooldown_tick();
            return;
        }
        self.list_inputs();
        let inputs = self.game.buttons_triggered(4);
        if inputs & super::main::PAD_CANCEL != 0 {
            self.lb_audio_call(SFX_BACK);
            self.game.menu.flow.entering_menu = 0;
            self.go_to_list(SETTINGS, 0, EXIT_TO);
            let d = self.vibration();
            for t in d.texts.into_iter().flatten() {
                self.destroy_text(t);
            }
            if let Some(t) = d.title {
                self.destroy_text(t);
            }
            let g = self.game.options.vibration.gobj.expect("the rumble screen");
            gobj::free(self, g);
            self.game.pads.rumble.remove_all();
            self.card_game_mark_changes();
            return;
        }
        if reg(f32::from(self.vibration().intro)) <= reg(INTRO_LOOP.end) {
            let d = self.vibration_mut();
            d.intro = d.intro.wrapping_add(1);
            return;
        }
        for i in 0..4 {
            if self.vibration().on_names[i] != 0
                || self.game.buttons_triggered(i as u8) & super::main::PAD_CONFIRM == 0
            {
                continue;
            }
            self.lb_audio_call(SFX_FORWARD);
            let on = self.port_rumble(i) == 0;
            self.set_port_rumble(i, u8::from(on));
            let r = self.port_rumble(i);
            let panel = self.port_panel(i);
            let light = self.options_node(panel, 2);
            self.options_set_frame(light, f32::from(r));
            if on {
                self.game
                    .pads
                    .rumble
                    .add(i as u8, 0, 14, 0, Script::table(Table::Vibration));
            } else {
                self.game.pads.rumble.remove_all();
                self.game.pads.rumble.off(i as u8);
            }
            return;
        }
        if self.name_count() == 0 {
            return;
        }
        for j in 0..4 {
            let t = self.game.buttons_triggered(j as u8);
            let on = self.vibration().on_names[j];
            if t & super::super::gm::input::ANY_LEFT != 0 && on == 1 {
                self.lb_audio_call(SFX_MOVE);
                self.vibration_mut().on_names[j] = 0;
                self.vibration_port_switch(j);
            } else if t & super::super::gm::input::ANY_RIGHT != 0 && on == 0 {
                self.lb_audio_call(SFX_MOVE);
                self.vibration_mut().on_names[j] = 1;
                self.vibration_port_switch(j);
            }
        }
        let mut repeat = 0u64;
        for i in 0..4 {
            if self.vibration().on_names[i] == 1 {
                repeat |= self.game.buttons_repeated(i as u8);
            }
        }
        if repeat & super::main::PAD_CONFIRM != 0 {
            self.lb_audio_call(SFX_FORWARD);
            let row = self.vibration().row;
            let slot = self.name_slot(i32::from(row));
            let on = self.name_rumble(slot) != 1;
            self.game
                .game_data
                .name_tag(usize::from(slot))
                .rumble_enabled = u8::from(on);
            let r = self.name_rumble(slot);
            let row = self.vibration().row;
            let j = self.name_row_jobj(i32::from(row));
            self.options_set_frame(j, f32::from(r));
            return;
        }
        if repeat & super::super::gm::input::ANY_UP != 0 {
            let row = self.vibration().row;
            if row != 0 {
                if self.name_slot(i32::from(row) - 1) != 0xFF {
                    self.lb_audio_call(SFX_MOVE);
                    self.vibration_mut().row -= 1;
                    self.vibration_place_cursor();
                }
            } else if self.name_count() > 8 && self.vibration().scroll != 0 {
                self.lb_audio_call(SFX_MOVE);
                self.vibration_mut().scroll -= 1;
                self.vibration_refresh_names();
            }
        } else if repeat & super::super::gm::input::ANY_DOWN != 0 {
            let row = self.vibration().row;
            if row < 7 {
                if self.name_slot(i32::from(row) + 1) != 0xFF {
                    self.lb_audio_call(SFX_MOVE);
                    self.vibration_mut().row += 1;
                    self.vibration_place_cursor();
                }
            } else if self.name_count() > 8 && self.name_slot_raw(8) as u8 != 0xFF {
                self.lb_audio_call(SFX_MOVE);
                let d = self.vibration_mut();
                d.scroll = d.scroll.wrapping_add(1);
                if i32::from(self.vibration().scroll) >= self.name_count() {
                    self.vibration_mut().scroll = 0;
                }
                self.vibration_refresh_names();
            }
        }
    }

    /// `mnVibration_AnimatePortPanel`: a port's switch between its panel and the names.
    fn vibration_port_switch(&mut self, port: usize) {
        let d = self.vibration();
        let state = d.on_names[port];
        let j = d.jobjs[PANEL_IDS[port]];
        self.options_set_frame(j, f32::from(u16::from(state)));
    }

    /// `mnVibration_CursorThink`: the names' cursor goes with the screen.
    #[doc(alias = "mnVibration_CursorThink")]
    pub fn vibration_cursor_think(&mut self, gobj: GObjId) {
        if self.game.menu.flow.cur_menu != RUMBLE {
            gobj::free(self, gobj);
        }
    }

    /// `mnVibration_UpdatePortPanel`: a port's panel showing its number and its rumble
    /// lit or not, or with no controller in the port, nobody's.
    #[doc(alias = "mnVibration_UpdatePortPanel")]
    fn vibration_update_panel(&mut self, panel: Option<JObjId>, port: u8, connected: bool) {
        let lights = self.options_node(panel, 1);
        if connected {
            self.options_set_frame(lights, f32::from(port));
            let f = if self.port_rumble(usize::from(port)) != 0 {
                LIGHTS.end
            } else {
                LIGHTS.start
            };
            self.options_set_frame_stopped(lights, f, MOBJ_MASK);
        } else {
            self.options_set_frame(lights, 20.0);
            self.options_set_frame_stopped(lights, LIGHTS.end, MOBJ_MASK);
        }
        let number = self.options_node(panel, 3);
        let f = if connected {
            f32::from(port) + 1.0
        } else {
            20.0
        };
        self.options_set_frame(number, f);
        let switch = self.options_node(panel, 2);
        if connected {
            let r = self.port_rumble(usize::from(port));
            self.options_set_frame(switch, f32::from(r));
        } else {
            self.options_set_frame(switch, 20.0);
        }
    }

    /// `mnVibration_CreatePortPanels`: the four ports' panels, side by side under the
    /// panels' joint, hidden till the intro shows them.
    #[doc(alias = "mnVibration_CreatePortPanels")]
    fn vibration_make_panels(&mut self) {
        let d = self.vibration();
        let base_x = reg(self.translate_of(d.jobjs[23]).x);
        let spacing = fp::fsubs(reg(self.translate_of(d.jobjs[24]).x), base_x);
        let m = self.game.options.vibration.models[PANEL];
        for i in 0..4u8 {
            let data = self.data;
            let o = &mut self.game.objs;
            let j = jobj::load_joint(o, data, m.joint).expect("a port's panel");
            jobj::add_anim_all(o, data, Some(j), m.anim, m.matanim, m.shapeanim);
            self.options_set_frame(Some(j), 0.0);
            let x = mem(fp::fmuls(spacing, fp::frsp(f64::from(i))));
            jobj::set_srt(&mut self.game.objs.jobjs, j, |o| o.translate.x = x);
            let connected = self.game.pads.copy[usize::from(i)].err == 0;
            self.vibration_update_panel(Some(j), i, connected);
            let jobjs = &mut self.game.objs.jobjs;
            jobj::set_flags_all(jobjs, Some(j), HIDDEN);
            jobj::add_child(jobjs, d.jobjs[23], Some(j));
        }
    }

    /// `mnVibration_CreateNameRow`: name tag `slot` on row `row`: its name, and its switch
    /// showing its rumble.
    #[doc(alias = "mnVibration_CreateNameRow")]
    fn vibration_name_row(&mut self, slot: u8, row: u8) {
        let flag = self.name_rumble(slot);
        let d = self.vibration();
        let y17 = reg(self.translate_of(d.jobjs[17]).y);
        let spacing = fp::fsubs(reg(self.translate_of(d.jobjs[18]).y), y17);
        let at = crate::lb::joint::world_position(
            &mut self.game.objs,
            self.data,
            d.jobjs[17],
            Some(NAME_OFFSET),
        );
        let t = self.create_text_buffer(0, 1);
        self.vibration_mut().texts[usize::from(row)] = Some(t);
        let y = fp::fnmadds(spacing, fp::frsp(f64::from(row)), reg(at.y));
        let s = &mut self.game.sis;
        let text = s.text_mut(t);
        text.pos = Vec3::new(at.x, mem(y), at.z);
        text.font_size = [0.03, 0.03];
        let name = self.game.name_text(slot);
        let name = name.unwrap_or_else(|| panic!("the names' row {row} shows empty slot {slot}"));
        self.game.sis.add_string(t, 0.0, 0.0, Some(&name));
        let m = self.game.options.vibration.models[SWITCH];
        let data = self.data;
        let o = &mut self.game.objs;
        let j = jobj::load_joint(o, data, m.joint).expect("a name's switch");
        jobj::add_anim_all(o, data, Some(j), m.anim, m.matanim, m.shapeanim);
        self.options_set_frame(Some(j), f32::from(flag));
        let y = mem(fp::fmuls(spacing, fp::frsp(f64::from(row))));
        let jobjs = &mut self.game.objs.jobjs;
        jobj::set_srt(jobjs, j, |o| o.translate.y = y);
        jobj::add_child(jobjs, d.jobjs[17], Some(j));
    }

    /// `mnVibration_RefreshNameRows`: the names' rows made again from the scroll.
    #[doc(alias = "mnVibration_RefreshNameRows")]
    fn vibration_refresh_names(&mut self) {
        for i in 0..8 {
            if let Some(t) = self.vibration().texts[i] {
                self.destroy_text(t);
                self.vibration_mut().texts[i] = None;
            }
        }
        let j17 = self.vibration().jobjs[17];
        if let Some(c) = j17.and_then(|j| self.game.objs.jobjs[j].child) {
            jobj::remove_all(&mut self.game.objs, Some(c));
        }
        for row in 0..8 {
            let slot = self.name_slot(row);
            if slot != 0xFF {
                self.vibration_name_row(slot, row as u8);
            }
        }
    }

    /// `mnVibration_Think`: each port's rumble lights, and a panel shown or hidden as its
    /// port's controller comes or goes. The screen goes with B in its input process, before
    /// this runs again, and the menu's restart (L, R and Start) ends the scene: no tick finds
    /// the flow out of the rumble screen here, so `mnVibration_OnAnimComplete` is never set
    /// up (`ledger/dead.txt`).
    #[doc(alias = "mnVibration_Think")]
    pub fn vibration_think(&mut self, _gobj: GObjId) {
        assert!(
            self.game.menu.flow.cur_menu == RUMBLE,
            "mnVibration_Think out of the rumble screen sets up mnVibration_OnAnimComplete, \
             proven dead"
        );
        for port in 0..4 {
            let panel = self.port_panel(port);
            let lights = self.options_node(panel, 1);
            if self.port_rumble(port) != 0 {
                self.anim_loop_stopped(lights.expect("a panel's lights"), LIGHTS, MOBJ_MASK);
            } else {
                self.options_set_frame_stopped(lights, 0.0, MOBJ_MASK);
            }
        }
        for (port, &panel_id) in PANEL_IDS.iter().enumerate() {
            let err = self.game.pads.copy[port].err;
            let was = self.vibration().connected[port];
            if !((err != 0 && was != 0) || (err == 0 && was == 0)) {
                continue;
            }
            let j = self.vibration().jobjs[panel_id];
            if err != 0 {
                jobj::set_flags_all(&mut self.game.objs.jobjs, j, HIDDEN);
                let panel = self.port_panel(port);
                self.vibration_update_panel(panel, port as u8, false);
                self.vibration_mut().connected[port] = 0;
            } else {
                jobj::clear_flags_all(&mut self.game.objs.jobjs, j, HIDDEN);
                self.vibration_mut().on_names[port] = 0;
                self.options_set_frame(j, 0.0);
                let panel = self.port_panel(port);
                self.vibration_update_panel(panel, port as u8, true);
                self.vibration_mut().connected[port] = 1;
            }
        }
    }

    /// `mnVibration_IntroProc`: the screen coming in, each port's panel shown at its frame
    /// (its switch with a controller in), then the names and their cursor; at its end the
    /// screen's process.
    #[doc(alias = "mnVibration_IntroProc")]
    pub fn vibration_intro(&mut self, gobj: GObjId) {
        let d = self.vibration();
        let frame = self.anim_loop(d.jobjs[1].expect("the screen's frame"), INTRO_LOOP);
        if let Some(port) = REVEAL[..4].iter().position(|&f| reg(f) == frame) {
            let panel = self.port_panel(port);
            jobj::clear_flags_all(&mut self.game.objs.jobjs, panel, HIDDEN);
            if d.connected[port] != 0 {
                jobj::clear_flags_all(&mut self.game.objs.jobjs, d.jobjs[PANEL_IDS[port]], HIDDEN);
            }
        } else if reg(REVEAL[4]) == frame && self.name_count() != 0 {
            self.vibration_refresh_names();
            let m = self.game.options.vibration.models[CURSOR];
            let data = self.data;
            let g = &mut *self.game;
            let c = g.world.create(6, 7, 0x80);
            let j = jobj::load_joint(&mut g.objs, data, m.joint).expect("the names' cursor");
            g.world.set_obj(c, OBJ_JOBJ, Obj::JObj(j));
            g.world.setup_gx_link(c, CodeRef(JOBJ_CALLBACK), 4, 0x80);
            g.world.setup_proc(c, CodeRef(CURSOR_THINK), 0);
            self.vibration_mut().cursor = Some(c);
            self.vibration_place_cursor();
        }
        if frame >= reg(INTRO_LOOP.end) {
            self.options_swap_proc(gobj, THINK);
        }
    }

    /// `mnVibration_CreateScreen`: the screen on render link 4, its user data (each port's
    /// controller noted) and joints, the ports' switches hidden, the intro, the panels and
    /// the title.
    #[doc(alias = "mnVibration_CreateScreen")]
    fn vibration_make_screen(&mut self) {
        let m = self.game.options.vibration.models[SCREEN];
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(6, 7, 0x80);
        g.options.vibration.gobj = Some(gobj);
        let root = jobj::load_joint(&mut g.objs, data, m.joint).expect("the rumble screen");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(root));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 4, 0x80);
        jobj::add_anim_all(
            &mut g.objs,
            data,
            Some(root),
            m.anim,
            m.matanim,
            m.shapeanim,
        );
        jobj::req_anim_all(&mut g.objs, Some(root), 0.0);
        let addr = crate::hsd::heap::mem_alloc(0x98);
        // `title_text` isn't written before it's read: the block holds the last screen's
        // title when it's the same block, else no text's address.
        let left = match self.game.options.vibration.left {
            Some((a, t)) if a == addr => t,
            _ => None,
        };
        let connected = std::array::from_fn(|i| u8::from(self.game.pads.copy[i].err == 0));
        let mut jobjs = [None; 25];
        for (k, j) in jobjs.iter_mut().enumerate() {
            *j = self.options_node(Some(root), k as i32);
        }
        self.game.options.vibration.data.insert(
            addr,
            VibrationData {
                intro: 0,
                row: 0,
                on_names: [0; 4],
                connected,
                scroll: 0,
                jobjs,
                texts: [None; 8],
                title: left,
                cursor: None,
            },
        );
        self.game
            .world
            .init_user_data(gobj, 0, UserData::Vibration(addr));
        for k in [22, 21, 20, 19] {
            jobj::set_flags_all(&mut self.game.objs.jobjs, jobjs[k], HIDDEN);
        }
        self.game.world.setup_proc(gobj, CodeRef(INTRO), 0);
        self.vibration_make_panels();
        if let Some(t) = left {
            self.destroy_text(t);
        }
        let t = self.menu_center_text(1, 0xA7);
        self.vibration_mut().title = Some(t);
    }

    /// `mnVibration_Init`, from the Options list: the flow on the rumble screen (0x13), its
    /// models from the menus' archive, the screen and its input process.
    #[doc(alias = "mnVibration_Init")]
    pub fn vibration_open(&mut self) {
        self.game.menu.cooldown = 5;
        let f = &mut self.game.menu.flow;
        f.prev_menu = f.cur_menu;
        f.cur_menu = RUMBLE;
        f.hovered_selection = 0;
        let models = [
            "MenMainCursorVi_Top",
            "MenMainCtlVi_Top",
            "MenMainOnoffVi_Top",
            "MenMainConVi_Top",
        ];
        for (i, p) in models.into_iter().enumerate() {
            let mut m = self.menu_model(p);
            if i == CURSOR {
                m = ModelDesc {
                    joint: m.joint,
                    ..ModelDesc::default()
                };
            }
            self.game.options.vibration.models[i] = m;
        }
        self.vibration_make_screen();
        self.options_input_proc(HANDLE_INPUT);
    }

    /// A model's sections in the menus' archive (`lbArchive_LoadSections`).
    pub(crate) fn menu_model(&self, stem: &str) -> ModelDesc {
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
        ModelDesc {
            joint: sym(&format!("{stem}_joint")),
            anim: sym(&format!("{stem}_animjoint")),
            matanim: sym(&format!("{stem}_matanim_joint")),
            shapeanim: sym(&format!("{stem}_shapeanim_joint")),
        }
    }
}
