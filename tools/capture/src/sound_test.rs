//! The sound test's statics (`mnsoundtest.c`) and its screen's user data, compared with
//! memory.

use melee_sim::game::mirror::Cx;

use crate::compare::Comparer;

/// `HSD_Free`, the screen's user data release.
const HSD_FREE: u32 = 0x8037_F1B0;

/// The statics: the model, the screen's GObj, whether it's coming in, and the announcer's and
/// the effects' volumes it last set.
const MODEL: u32 = 0x804A_08C8;
const GOBJ: u32 = 0x804D_6C40;
const COMING_IN: u32 = 0x804D_6C44;
const VOICE: u32 = 0x804D_6C48;
const FX: u32 = 0x804D_6C4C;

impl Comparer<'_> {
    /// The screen's `soundtest_user_data` at `t`, its block `addr`. Bytes 6 and 7 are padding
    /// nothing writes or reads.
    pub(crate) fn sound_test_data(&mut self, path: &str, addr: u32, t: u32) {
        let Some(d) = self.game.sound_test.data.get(&addr).copied() else {
            self.diff(path, "soundtest_user_data", "none", format!("{addr:#x}"));
            return;
        };
        let ud = self.u32(t + 0x2C);
        let release = self.u32(t + 0x30);
        self.check(
            path,
            "user data and release",
            (addr, HSD_FREE),
            (ud, release),
        );
        if ud != addr {
            return;
        }
        self.check(
            path,
            "view, song, playing, category, sound",
            (d.view, d.song, d.playing, d.category, d.sound),
            (
                self.u8(ud),
                self.u8(ud + 1),
                self.u8(ud + 2),
                self.u8(ud + 3),
                self.u16(ud + 4),
            ),
        );
        self.check(
            path,
            "fades",
            (d.music_fade.to_bits(), d.sound_fade.to_bits()),
            (self.u32(ud + 8), self.u32(ud + 0xC)),
        );
        for (i, x) in [d.title, d.song_name, d.song_number, d.sound_number]
            .into_iter()
            .enumerate()
        {
            let theirs = self.u32(ud + 0x10 + 4 * i as u32);
            self.compare_handle(
                &format!("{path} unk{:X}", 0x10 + 4 * i),
                "TextId",
                x.map(|t| t.0),
                theirs,
            );
        }
    }

    /// The sound test's statics.
    pub(crate) fn sound_test(&mut self) {
        let s = &self.game.sound_test;
        let (m, g, coming_in, voice, fx) = (s.model, s.gobj, s.coming_in, s.voice, s.fx);
        for (k, d) in [m.joint, m.anim, m.matanim, m.shapeanim]
            .into_iter()
            .enumerate()
        {
            let theirs = self.u32(MODEL + 4 * k as u32);
            self.compare_data(&format!("mnSoundTest_804A08C8 {k}"), d, theirs);
        }
        let theirs = self.u32(GOBJ);
        self.compare_handle("mnSoundTest_804D6C40", "GObjId", g.map(|g| g.0), theirs);
        self.check(
            "mnSoundTest_804D6C44",
            "coming in",
            coming_in,
            self.u8(COMING_IN),
        );
        self.check(
            "mnSoundTest_804D6C48",
            "the announcer's volume",
            voice,
            self.u32(VOICE) as i32,
        );
        self.check(
            "mnSoundTest_804D6C4C",
            "the effects' volume",
            fx,
            self.u32(FX) as i32,
        );
    }
}
