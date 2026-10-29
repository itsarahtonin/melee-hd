//! The special messages' interface (`melee/if/ifprize.c`): each message's strings and kind,
//! which the Data menu's list of messages (`mninfo.c`) names its entries by.

/// `un_803F9A00`: each string's kind, by the message's string offset.
const KINDS: [i32; 76] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 3, 2, 2, 5, 5, 4, 6, 6, 8, 8,
    8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 6, 6, 5, 7,
    7, 7, 7, 7, 7, 5, 5, 5, 5, 5, 5, 5,
];

/// `un_803F9B30`: each message (`x0`) and its string offset (`x4`), to the message 66 that
/// ends the table.
const MESSAGES: [(i32, u16); 67] = {
    let mut t = [(0, 0); 67];
    let mut i = 0;
    while i < 62 {
        t[i] = (i as i32, i as u16);
        i += 1;
    }
    t[62] = (62, 63);
    t[63] = (63, 67);
    t[64] = (64, 68);
    t[65] = (65, 62);
    t[66] = (66, 0);
    t
};

/// `un_802FE3F8`: message `n`'s string, `base` plus its offset (`c`); message 0x3E, the
/// trophy's, also its second string (`d`). A message the table doesn't have leaves both as
/// they were (none here: every caller asks for one of 0 to 65).
#[doc(alias = "un_802FE3F8")]
pub fn message_strings(n: i32, base: i32) -> (Option<u16>, Option<u16>) {
    let (mut c, mut d) = (None, None);
    for &(x0, x4) in MESSAGES.iter().take_while(|e| e.0 != 66) {
        if x0 == n {
            let s = (base + i32::from(x4)) as u16;
            c = Some(s);
            if n == 0x3E {
                d = Some(s.wrapping_add(1));
            }
        }
    }
    (c, d)
}

/// The kind of message `n`'s string (`un_803F9A00` by its offset), 0 for one the table
/// doesn't have.
pub fn message_kind(n: i32) -> i32 {
    MESSAGES
        .iter()
        .take_while(|e| e.0 != 66)
        .find(|e| e.0 == n)
        .map_or(0, |e| KINDS[usize::from(e.1)])
}

use std::collections::BTreeMap;

use melee_data::archive::{ArchiveId, DataRef};

use super::super::handles::CodeRef;
use super::super::render::{CAMERA_CALLBACK, JOBJ_CALLBACK, LOBJ_CALLBACK};
use super::super::state::{OBJ_CAMERA, OBJ_JOBJ, OBJ_LIGHT, Obj, Running};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::sis::TextId;
use crate::vec::Vec3;

/// `ifPrize_Scene_OnEnter` and the interface's process.
/// The trophies' notice (`gm_ModeState_Prize_OnEnter`'s for a trophy whose notice waits).
const TROPHY_NOTICE: u16 = 0x3E;
/// The prize font's string a trophy's notice is made in.
const TROPHY_STRING: i32 = 0x4A;

pub const ON_ENTER: CodeRef = CodeRef(0x802F_EBE0);
pub const PROC: u32 = 0x802F_E470;

/// A queued notice (`struct un_802FEBE0_OnEnter_arg0`): its message, its trophy (the
/// trophies' message's), when it came, and the next notice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct PrizeEntry {
    pub kind: u16,
    pub trophy: u16,
    pub time: u32,
    pub next: Option<EntryRef>,
}

impl PrizeEntry {
    pub fn next_mut(&mut self) -> &mut Option<EntryRef> {
        &mut self.next
    }
}

/// Where a notice is: the prize's enter data, or a block of the heap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub enum EntryRef {
    Static,
    Heap(u32),
}

/// The special messages' interface's state: its enter data (`if_Scene_Prize_EnterData`)
/// and the notices queued after it in the heap; the prize state's marks of what it queued
/// (`gm_8049E558`: the messages, then each trophy's count); `un_803F9D48`: whether it shows
/// (`x0a`, `x0b`), its state (`x1`) and frame (`x2`), the song (`x3`), the notice's kind and
/// strings (`x4`, `x6`, `x8`), its time (`xC`), the panel's part (`x10`), its GObjs (`x14`
/// the panel, `x18` the camera, `x1C` the light), its texts (`x20` the message, `x24` the
/// date), the notice shown and the next (`x28`, `x2C`), the three songs (`x30`); and its
/// archive (`un_804D6D98`, with its blocks) and scene (`un_804D6D9C`).
#[derive(Clone, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct Prize {
    pub first: PrizeEntry,
    pub heap: BTreeMap<u32, PrizeEntry>,
    pub queued: [u8; 0x170],
    pub shows: u8,
    pub open: u8,
    pub state: u8,
    pub frame: i8,
    pub song: u8,
    pub kind: u16,
    pub text: u16,
    pub text2: u16,
    pub time: i32,
    pub part: Option<JObjId>,
    pub panel: Option<GObjId>,
    pub camera: Option<GObjId>,
    pub light: Option<GObjId>,
    pub message: Option<TextId>,
    pub date: Option<TextId>,
    pub current: Option<EntryRef>,
    pub next: Option<EntryRef>,
    pub songs: [i32; 3],
    pub archive: Option<ArchiveId>,
    pub blocks: (u32, u32),
    pub scene: Option<DataRef>,
}

impl Default for Prize {
    fn default() -> Self {
        Self {
            first: PrizeEntry::default(),
            heap: BTreeMap::new(),
            queued: [0; 0x170],
            shows: 0,
            open: 0,
            state: 0,
            frame: 0,
            song: 0,
            kind: 0,
            text: 0,
            text2: 0,
            time: 0,
            part: None,
            panel: None,
            camera: None,
            light: None,
            message: None,
            date: None,
            current: None,
            next: None,
            songs: [0x43, 0x44, 0x45],
            archive: None,
            blocks: (0, 0),
            scene: None,
        }
    }
}

impl Prize {
    pub fn entry(&self, r: EntryRef) -> PrizeEntry {
        match r {
            EntryRef::Static => self.first,
            EntryRef::Heap(a) => self.heap[&a],
        }
    }

    pub fn entry_mut(&mut self, r: EntryRef) -> &mut PrizeEntry {
        match r {
            EntryRef::Static => &mut self.first,
            EntryRef::Heap(a) => self.heap.get_mut(&a).expect("a queued notice"),
        }
    }
}

/// `PAD_BUTTON_A | PAD_BUTTON_START`.
const A_OR_START: u64 = 0x1100;

impl Running<'_> {
    /// `fn_802FE470`, the panel's process: five frames in, then the first notice; A or
    /// Start the next, or after the last the interface going (`un_802FE8CC`); fifteen
    /// frames out, then the interface gone and the scene ended. The panel's frame and the
    /// notice's kind drive its animations.
    #[doc(alias = "fn_802FE470")]
    pub fn prize_proc(&mut self, gobj: GObjId) {
        let p = &self.game.prize;
        if p.state == 4 {
            return;
        }
        let j = super::super::mn::common::jobj_of(self, gobj);
        match p.state {
            0 => {
                if p.frame < 5 {
                    self.game.prize.frame += 1;
                } else {
                    let e = p.entry(p.current.expect("the first notice"));
                    self.prize_show(e.kind, e.trophy, e.time);
                    self.game.prize.state = 1;
                }
            }
            1 => {
                self.game.prize.frame = 0xA;
                if self.game.buttons_triggered(4) & A_OR_START != 0 {
                    match self.game.prize.next {
                        None => self.prize_close(),
                        Some(r) => {
                            let e = self.game.prize.entry(r);
                            self.prize_show(e.kind, e.trophy, e.time);
                            self.game.prize.next = e.next;
                        }
                    }
                }
            }
            2 => {
                if p.frame < 0xF {
                    self.game.prize.frame += 1;
                } else {
                    self.game.prize.state = 3;
                }
            }
            3 => {
                let p = &self.game.prize;
                let (cam, light, panel, blocks) = (p.camera, p.light, p.panel, p.blocks);
                for g in [cam, light, panel].into_iter().flatten() {
                    gobj::free(self, g);
                }
                self.free_archive_blocks(blocks.0, blocks.1);
                let p = &mut self.game.prize;
                p.state = 4;
                p.shows = 0;
                p.open = 0;
                self.game.end_scene();
                return;
            }
            _ => {}
        }
        let p = &self.game.prize;
        let (frame, kind, part) = (f32::from(p.frame), f32::from(p.kind), p.part);
        jobj::req_anim_all(&mut self.game.objs, Some(j), frame);
        let d = part.and_then(|x| self.game.objs.jobjs[x].dobj());
        crate::hsd::dobj::req_anim_all(&mut self.game.objs, d, kind);
        let c = part.and_then(|x| self.game.objs.jobjs[x].child);
        let d = c.and_then(|x| self.game.objs.jobjs[x].dobj());
        crate::hsd::dobj::req_anim_all(&mut self.game.objs, d, kind);
        super::super::mn::common::anim_all(self, Some(j));
    }

    /// `un_802FE6A8`: the interface's camera, light and panel (its process, its animations at
    /// their start), its canvas, the message's text and the date's.
    #[doc(alias = "un_802FE6A8")]
    fn prize_make(&mut self) {
        let data = self.data;
        let scene = self.game.prize.scene.expect("the prize's scene");
        let g = &mut *self.game;
        let cam = g.world.create(19, 20, 0);
        let cameras = data.ptr(scene.at(4)).expect("the scene's cameras");
        let c =
            crate::hsd::cobj::load_desc(&mut g.objs, data, data.ptr(cameras)).expect("a camera");
        g.world.set_obj(cam, OBJ_CAMERA, Obj::CObj(c));
        g.world
            .setup_gx_link_max(cam, CodeRef(CAMERA_CALLBACK), 8, false);
        g.world.gobj_mut(cam).gxlink_prios = 0xC00;
        g.prize.camera = Some(cam);
        let light = g.world.create(11, 3, 0);
        let lights = data.ptr(scene.at(8)).expect("the scene's lights");
        let l = crate::game::gr::setup::load_light_list(g, data, lights).expect("a light");
        g.world.set_obj(light, OBJ_LIGHT, Obj::LObj(l));
        g.world.setup_gx_link(light, CodeRef(LOBJ_CALLBACK), 10, 0);
        g.prize.light = Some(light);
        let ui = g.world.create(14, 15, 0);
        let models = data.ptr(scene).expect("the scene's models");
        let m = data.ptr(models).expect("a model");
        let j = jobj::load_joint(&mut g.objs, data, data.ptr(m)).expect("the panel");
        g.world.set_obj(ui, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(ui, CodeRef(JOBJ_CALLBACK), 11, 0);
        g.world.setup_proc(ui, CodeRef(PROC), 17);
        super::super::gm::vs::add_model_anim(&mut g.objs, data, Some(j), m, 0);
        jobj::req_anim_all(&mut g.objs, Some(j), 0.0);
        super::super::mn::common::anim_all(self, Some(j));
        self.game.prize.part = super::super::mn::common::node(self, Some(j), 2);
        self.game.prize.panel = Some(ui);
        self.add_canvas(2, None, 9, 20, 0, 14, 0, 18);
        let t = self.create_text(2, 0, Vec3::new(105.0, 202.0, 0.0), 435.0, 75.0);
        let x = self.game.sis.text_mut(t);
        x.default_fitting = 1;
        x.default_alignment = 1;
        self.game.prize.message = Some(t);
        let t = self.create_text_buffer(2, 0);
        let x = self.game.sis.text_mut(t);
        x.default_alignment = 1;
        x.default_kerning = 1;
        x.text_color = [0x5A, 0x5A, 0x5A, 0xFF];
        self.game.prize.date = Some(t);
    }

    /// `un_802FE8CC`: the interface going out, its strings closed (`HSD_SisLib_803A5F50`).
    #[doc(alias = "un_802FE8CC")]
    fn prize_close(&mut self) {
        let p = &self.game.prize;
        if p.shows == 0 || p.open == 0 {
            return;
        }
        self.game.prize.state = 2;
        self.close_font(2);
    }

    /// `un_802FE918`: notice `a` shown: the song stopped and the next of the three drawn
    /// (`HSD_Randi`), the message's notice taken back, its kind, its message (a trophy's
    /// notice, for trophy `b`, its two halves around the trophy's name in string 0x4A) and the
    /// date and time it came.
    #[doc(alias = "un_802FE918")]
    fn prize_show(&mut self, a: u16, b: u16, c: u32) {
        self.lb_audio_music_stop();
        let song = self.game.prize.songs[usize::from(self.game.prize.song)];
        self.lb_play_music(song);
        let mut r = self.game.objs.rng.below(2);
        let old = i32::from(self.game.prize.song);
        let mut new = old;
        for k in 0..3 {
            if k == old {
                r += 1;
            } else if r == k {
                new = r;
                break;
            }
        }
        self.game.prize.song = new as u8;
        let s = &mut self.game.game_data.thing.save_data;
        s.x1B40[usize::from(a / 32)] &= !(1 << (a % 32));
        self.game.prize.kind = message_kind(i32::from(a)) as u16;
        let t = self.game.prize.message.expect("the prize's message");
        if a == TROPHY_NOTICE {
            // The notice's string 0x4A made of its two halves around the trophy's name
            // (`Toy_803063D4`).
            let (s, s2) = message_strings(i32::from(a), 2);
            let (s, s2) = (
                s.expect("the trophies' notice"),
                s2.expect("its second half"),
            );
            self.game.prize.text = s;
            self.game.prize.text2 = s2;
            let name = self.toy_string(i32::from(b), 0x4E, 0x174);
            let sis = &mut self.game.sis;
            sis.copy_table_string(self.data, 2, TROPHY_STRING, i32::from(s));
            sis.append_table_string(self.data, 2, TROPHY_STRING, name);
            sis.append_table_string(self.data, 2, TROPHY_STRING, i32::from(s2));
            sis.reset_text(self.data, t, TROPHY_STRING);
        } else {
            let (s, _) = message_strings(i32::from(a), 2);
            let s = s.expect("a message the table has");
            self.game.prize.text = s;
            self.game.sis.reset_text(self.data, t, i32::from(s));
        }
        let d = super::super::mn::info::datetime(c);
        let line = if self.game.game_data.saved_language() == super::super::gm::LANG_US {
            format!(
                "{:2}.{:2}.{:04}  {:02}:{:02}:{:02}",
                d.month, d.day, d.year, d.hour, d.minute, d.second
            )
        } else {
            format!(
                "{:04}.{:2}.{:2}  {:02}:{:02}:{:02}",
                d.year, d.month, d.day, d.hour, d.minute, d.second
            )
        };
        let t = self.game.prize.date.expect("the prize's date");
        self.game.sis.clear_text(t);
        self.game
            .sis
            .add_string(t, 320.0, 316.0, Some(line.as_bytes()));
    }

    /// `ifPrize_Scene_OnEnter`: the trophies' class marks (`Toy_803124BC`), the queue's
    /// first notice and the next, the song drawn (`HSD_Randi`), the first's kind and
    /// message, its time, `IfPrize`'s scene and the prize's strings (`SdPrize`), and the
    /// interface. Its exit (`ifPrize_Scene_OnExit`) does nothing.
    #[doc(alias = "ifPrize_Scene_OnEnter")]
    #[doc(alias = "ifPrize_Scene_OnExit")]
    pub fn prize_on_enter(&mut self) {
        self.mark_class_two_trophies();
        let first = self.game.prize.first;
        let song = self.game.objs.rng.below(3) as u8;
        let p = &mut self.game.prize;
        p.current = Some(EntryRef::Static);
        p.next = first.next;
        p.song = song;
        p.open = 1;
        p.kind = message_kind(i32::from(first.kind)) as u16;
        let (s, _) = message_strings(i32::from(first.kind), 2);
        if let Some(s) = s {
            p.text = s;
        }
        p.frame = -1;
        p.state = 0;
        p.time = first.time as i32;
        p.shows = 1;
        let us = self.game.game_data.saved_language() == super::super::gm::LANG_US;
        let name = if us { "IfPrize.usd" } else { "IfPrize.dat" };
        let (a, file, archive) = self.load_archive_blocks(name);
        self.game.prize.archive = Some(a);
        self.game.prize.blocks = (file, archive);
        let at = self
            .data
            .get(a)
            .public("ScInfPrize_scene_data")
            .expect("IfPrize's scene");
        self.game.prize.scene = Some(DataRef {
            archive: a,
            offset: at,
        });
        let font = if us { "SdPrize.usd" } else { "SdPrize.dat" };
        self.load_font_named(2, font, "SIS_PrizeData");
        self.prize_make();
    }
}
