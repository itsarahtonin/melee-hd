//! Event Match's statics (`melee_sim::game::mn::event`'s list, `melee_sim::game::gm::event`'s
//! mode) and the Event list's user data, compared with memory.

use melee_sim::game::mirror::Mirror;

use crate::compare::Comparer;
use crate::import::sym;

impl Comparer<'_> {
    /// The Event list's `MnEventData` at `t`, its block `addr`. Bytes 1 to 3, after the
    /// cursor's row, nothing writes or reads.
    pub(crate) fn event_list_data(&mut self, path: &str, addr: u32, t: u32) {
        let Some(d) = self.game.event_list.data.get(&addr).copied() else {
            self.diff(path, "MnEventData", "none", format!("{addr:#x}"));
            return;
        };
        let Some(ud) = self.data_block(path, addr, t) else {
            return;
        };
        self.check(
            path,
            "page, first event",
            (d.page, d.first_event),
            (self.u8(ud), self.u32(ud + 4) as i32),
        );
        for i in 0..9 {
            let at = ud + 8 + 4 * i as u32;
            self.data_gobj(&format!("{path} marks[{i}]"), d.marks[i], at);
            let at = ud + 0x2C + 4 * i as u32;
            self.data_text(path, &format!("numbers[{i}]"), d.numbers[i], at);
            let at = ud + 0x50 + 4 * i as u32;
            self.data_text(path, &format!("names[{i}]"), d.names[i], at);
        }
        self.data_text(path, "score", d.score, ud + 0x74);
        self.data_text(path, "description", d.description, ud + 0x78);
    }

    /// The Event list's statics: its two models, its GObj, its delay and first event.
    pub(crate) fn event_list(&mut self) {
        let l = &self.game.event_list;
        let (model, mark, gobj, delay, start) = (l.model, l.mark, l.gobj, l.delay, l.start);
        self.data_model("MenMainConEv", model, sym("MenMainConEv"));
        self.data_model("MenMainMarkEv", mark, sym("MenMainMarkEv"));
        self.data_gobj("mnEvent_804D6C60", gobj, sym("mnEvent_804D6C60"));
        self.check(
            "mnEvent_804D6C64, 804D6C65",
            "delay, start",
            (delay, start),
            (
                self.u8(sym("mnEvent_804D6C64")),
                self.u8(sym("mnEvent_804D6C65")),
            ),
        );
    }
}

/// `gmevent.c`'s `css_data`, `vs_enter_data` and `vs_exit_data`.
const EVENT_CSS: u32 = 0x8049_7758;
const EVENT_START: u32 = 0x8049_78A0;
const EVENT_EXIT: u32 = 0x8049_79D8;

impl Comparer<'_> {
    /// Event Match's statics: the select's data, the match's enter data and its exit data.
    pub(crate) fn event_mode(&mut self) {
        let ev = &self.game.event;
        self.one_player_css("css_data (Event Match's)", &ev.css, EVENT_CSS);
        let m = self.mem_view();
        ev.start
            .compare(&m, EVENT_START, "vs_enter_data (Event Match's)", self);
        self.match_exit_info("vs_exit_data (Event Match's)", &ev.exit, EVENT_EXIT);
    }
}
