//! What the audio library gives the sound test (`melee/lb/lbaudio_ax.c`): the announcer's
//! and the effects' volumes set directly, the songs the trophy gallery may take up, the
//! sounds of each of the test's categories (`LbAd.dat`'s `lbAudioLoadData`, by language), and
//! a sound played with its bank loaded first.

use super::{SFX_NONE, SSM_PRIORITY, sfx_bank};
use crate::game::state::{Event, Running};

/// `flags_arr_803BB800`: the songs the sound test notes as the trophy gallery's
/// (`gmMainLib_8015ED68`), by song.
const SONG_FLAGS: [u8; 0x62] = [
    1, 1, 1, 1, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1,
    0, 1, 1, 1, 0, 0, 1, 1, 1, 1, 1, 1, 1, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1,
    1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1,
    1, 1,
];

/// `unk_arr_803BC4A0`: each character's two songs, for a stage whose song is the first
/// player's (`Ground_801C28AC`'s -2).
const CHARACTER_SONGS: [[u8; 2]; 0x21] = [
    [0x38, 0x03],
    [0x32, 0x21],
    [0x06, 0x54],
    [0x1E, 0x1E],
    [0x23, 0x31],
    [0x04, 0x04],
    [0x22, 0x4B],
    [0x2B, 0x2B],
    [0x4D, 0x4D],
    [0x01, 0x01],
    [0x41, 0x41],
    [0x3C, 0x1F],
    [0x42, 0x42],
    [0x40, 0x41],
    [0x28, 0x02],
    [0x40, 0x40],
    [0x61, 0x33],
    [0x60, 0x5F],
    [0x22, 0x4A],
    [0x22, 0x4A],
    [0x54, 0x06],
    [0x4A, 0x4A],
    [0x07, 0x07],
    [0x01, 0x01],
    [0x3F, 0x3F],
    [0x22, 0x22],
    [0x62, 0x62],
    [0x62, 0x62],
    [0x62, 0x62],
    [0x62, 0x62],
    [0x62, 0x62],
    [0x62, 0x62],
    [0, 0],
];

/// Character `ckind`'s song `which` (0 or 1), 0x62 (none) for a kind past the table,
/// `lbAudioAx_8002305C`. The table's last row is past its initializer's, zero.
#[doc(alias = "lbAudioAx_8002305C")]
pub fn character_song(ckind: i32, which: i32) -> i32 {
    if (0..0x21).contains(&ckind) {
        return i32::from(CHARACTER_SONGS[ckind as usize][which as usize]);
    }
    0x62
}

/// Whether song `bgm` is one the sound test notes for the trophy gallery,
/// `lbAudioAx_80023090`.
#[doc(alias = "lbAudioAx_80023090")]
pub fn song_flag(bgm: i32) -> i32 {
    if !(0..0x62).contains(&bgm) {
        return 0;
    }
    i32::from(SONG_FLAGS[bgm as usize])
}

impl crate::audio::Audio {
    /// `lbAudioAx_800245F4`: the effects' volume (`lbl_804D388C`), 0 to 127.
    #[doc(alias = "lbAudioAx_800245F4")]
    pub fn lb_set_fx_volume(&mut self, v: i32) -> i32 {
        self.lb.fx = v.clamp(0, 0x7F);
        self.lb.fx
    }

    /// `lbAudioAx_80024614`: the announcer's volume (`lbl_804D3884`), 0 to 127.
    #[doc(alias = "lbAudioAx_80024614")]
    pub fn lb_set_voice_volume(&mut self, v: i32) {
        self.lb.voice = v.clamp(0, 0x7F);
    }
}

impl Running<'_> {
    /// `getAudioLoadData`: the sound test's list of category `cat`'s sounds, from the table the
    /// language picks (the setting is always English: English saves take `x4`, Japanese ones
    /// `xC`).
    fn sound_test_list(&self, cat: i32) -> melee_data::archive::DataRef {
        // The original's guard passes every category (docs/melee-bugs.md, latent defects).
        assert!(
            (0..30).contains(&cat),
            "sound test category {cat} past the table"
        );
        let base = self
            .game
            .menu
            .audio_load_data
            .expect("lbl_804D6454 (LbAd.dat's lbAudioLoadData)");
        let us = self.game.game_data.saved_language() == crate::game::gm::main_lib::LANG_US;
        let table = self
            .data
            .ptr(base.at(if us { 4 } else { 0xC }))
            .expect("lbAudioLoadData's table");
        self.data
            .ptr(table.at(4 * cat as u32))
            .expect("a category's sounds")
    }

    /// How many sounds category `cat` has, `lbAudioAx_80023968`: its list up to the
    /// [`SFX_NONE`] that ends it.
    #[doc(alias = "lbAudioAx_80023968")]
    pub fn sound_test_count(&self, cat: i32) -> i32 {
        let list = self.sound_test_list(cat);
        let mut i = 0;
        while self.data.u32(list.at(4 * i as u32)) as i32 != SFX_NONE {
            i += 1;
        }
        i
    }

    /// Category `cat`'s sound `i`, `lbAudioAx_80023A44`.
    #[doc(alias = "lbAudioAx_80023A44")]
    pub fn sound_test_id(&self, cat: i32, i: i32) -> i32 {
        let list = self.sound_test_list(cat);
        self.data.u32(list.at(4 * i as u32)) as i32
    }

    /// `lbAudioAx_80023B24`: sound `id` at full volume and in the middle, its bank first
    /// loaded into sound bank 2 unless it's one of the banks always loaded: every sound keyed
    /// off, and, when the bank isn't the one loaded, bank 2 emptied, every bank that can be
    /// loaded marked unloaded, and the bank's file loaded and waited for.
    #[doc(alias = "lbAudioAx_80023B24")]
    pub fn sound_test_play(&mut self, id: i32) -> i32 {
        let slot = sfx_bank(id) as usize;
        if SSM_PRIORITY[slot] != 5 {
            self.game.audio.sfx_key_off_all();
            self.game.events.push(Event::SfxKeyOffAll);
            if self.game.audio.lb.banks[slot] != 2 {
                assert!(slot < 55, "sound {id:#x} in no bank");
                // `fn_800269AC_delay`'s busy loop takes no time on the timeline (ssbm-rs's port
                // of it doesn't either).
                let a = &mut self.game.audio;
                a.sfx_unload_bank(2);
                for (k, &priority) in SSM_PRIORITY.iter().enumerate().take(55) {
                    if priority != 5 {
                        a.lb.bank_want[k] = -1;
                        a.lb.bank_have[k] = -1;
                        a.lb.banks[k] = -1;
                        a.lb.bank_load[k] = -1;
                    }
                }
                a.lb.set_ssm_name(slot);
                let e = a.lb.ssm_entrynum(slot);
                a.lb.bank_load[slot] = a.sfx_load(self.data, e, 2, None, 0);
                a.sfx_wait_for_loads(self.data);
                a.lb.banks[slot] = 2;
                // The load's wait ran the save's writer (`lb_800195D0`).
                self.card_writer_take();
            }
        }
        self.sfx_start(id, 0x7F, 0x40, 0, 7)
    }
}
