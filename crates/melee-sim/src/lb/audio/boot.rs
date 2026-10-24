//! The game's audio library at boot: `main`'s setup (`lbAudioAx_8002838C`) and the save
//! data's reset of the sound settings, which loads the sound bank and the banks every scene
//! keeps (`lbAudioAx_80028690`).

use melee_data::archive::{ArchiveId, Archives};

use super::{SFX_NONE, SSM_SIZE};
use crate::audio::Audio;
use crate::audio::boot::{Aram, BootTimes};

/// `s32_arr_803BB5D0`'s first column: each bank's class, which `fn_80023254` sorts by. The
/// third sound bank's room is the largest class 3 bank, the four largest class 4 ones (the
/// fighters') and the largest class 5 one (the stages').
const SSM_CLASS: [i8; 56] = [
    1, 1, 3, 3, 3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 6, 2, 6, 5, 1, 0,
];

/// The aux effects `lbAudioAx_8002838C` sets up: AXFX's standard reverb on aux A, its delay
/// on aux B (`AXDRIVER_AUX_REVERB_STD`, `AXDRIVER_AUX_DELAY`).
pub const AUX_REVERB_STD: u32 = 2;
pub const AUX_DELAY: u32 = 4;

/// The save's sound settings the audio library's reset reads: the saved language (English
/// or Japanese) and the sound balance (-100 all music to 100 all effects).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SavedSound {
    pub us: bool,
    pub balance: i8,
}

impl SavedSound {
    /// A fresh save's, in English.
    pub const DEFAULT: SavedSound = SavedSound {
        us: true,
        balance: 0,
    };
}

/// `gm_801601C4`: the announcer's volume at a sound balance, full up to the middle, less
/// toward the effects' end.
#[doc(alias = "gm_801601C4")]
pub fn balance_voice(balance: i8) -> i32 {
    use gekko_fp as fp;
    let t = fp::fmuls(
        fp::fdivs(fp::frsp(f64::from(i32::from(balance) + 100)), 200.0),
        100.0,
    );
    if t == 50.0 || t <= 50.0 {
        return 0x7F;
    }
    fp::fctiwz(fp::fmuls(127.0, fp::fdivs(fp::fsubs(100.0, t), 50.0)))
}

/// `gm_80160244`: the effects' volume at a sound balance, full from the middle up, less
/// toward the music's end.
#[doc(alias = "gm_80160244")]
pub fn balance_fx(balance: i8) -> i32 {
    use gekko_fp as fp;
    let t = fp::fmuls(
        fp::fdivs(fp::frsp(f64::from(i32::from(balance) + 100)), 200.0),
        100.0,
    );
    if t >= 50.0 {
        return 0x7F;
    }
    fp::fctiwz(fp::fmuls(127.0, fp::fdivs(t, 50.0)))
}

/// The sound bank's file (`smash2.sem`), loaded and parsed by the caller: its archive and
/// size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct SoundBank {
    pub archive: ArchiveId,
    pub len: u32,
}

/// `fn_80023254`: the banks of `class`, largest first (`lbl_80433B44`), 55 past the last.
#[doc(alias = "fn_80023254")]
fn banks_by_size(class: i8) -> [usize; 56] {
    let mut order = [55usize; 56];
    let mut used = [false; 56];
    for count in 0..=55 {
        for index in 0..=55 {
            if SSM_CLASS[index] == class && !used[index] && SSM_SIZE[order[count]] < SSM_SIZE[index]
            {
                // `fn_80023254_shift`: the list moves down a place to make room.
                order.copy_within(count..55, count + 1);
                order[count] = index;
                used[index] = true;
            }
        }
    }
    order
}

impl Audio {
    /// `lbAudioAx_8002838C`, `main`'s audio setup: the three sound banks' sizes (the common
    /// bank, the banks every scene keeps, and room for the largest of what a scene adds), the
    /// audio library on a timeline started here (`HSD_AudioInitMultiPStream`), the aux
    /// effects, the banks' room in ARAM, and no bank loaded or wanted. `ARInit`, `ARQInit` and
    /// `AIInit` set up hardware the simulation doesn't keep; the language the bank files were
    /// last named for (`lbl_804D3878`) is only ever the boot's.
    #[doc(alias = "lbAudioAx_8002838C")]
    pub fn lb_boot(&mut self, data: &Archives) -> u32 {
        let mut aram = Aram::new();
        let main = SSM_SIZE[0];
        let kept = SSM_SIZE[0x33] + SSM_SIZE[1] + SSM_SIZE[0x36];
        let mut scene = SSM_SIZE[banks_by_size(3)[0]];
        let fighters = banks_by_size(4);
        scene += fighters[..4].iter().map(|&b| SSM_SIZE[b]).sum::<u32>();
        scene += SSM_SIZE[banks_by_size(5)[0]];
        let room = main + kept + scene;
        self.init_multi_pstream(data, &mut aram, room);
        let delay = self.boot_delay;
        self.with_clock(data, |c, f| c.catch_up(BootTimes::AUX + delay, f));
        self.driver_setup_aux(0, AUX_REVERB_STD);
        self.driver_setup_aux(1, AUX_DELAY);
        for size in [main, kept, scene] {
            self.sfx_allocate_bank(size as i32);
        }
        let lb = &mut self.lb;
        lb.fgm_room = room as i32;
        lb.fgm_banks = [main as i32, kept as i32, scene as i32];
        lb.bank_want = [-1; 56];
        lb.bank_have = [-1; 56];
        lb.banks = [-1; 56];
        lb.bank_load = [-1; 56];
        lb.fgm_loaded = 0;
        lb.fgm_wanted = 0;
        lb.fgm_pending = 0;
        aram.next()
    }

    /// `lbAudioAx_80028690`, the sound settings' reset when the save data is: the settings
    /// at their defaults (`fn_800244F4`) handed to the synth (`fn_80024654(1)`), the bank
    /// files named for the US directory (the disc has `usa.ini`), and, as the language they
    /// were named for changes from none, the common bank loaded into bank 0, the sound bank
    /// (`AXDriver_8038DA70`), and the banks every scene keeps into banks 1 and 2, each waited
    /// for (`lb_800195D0` spinning); then the queued sounds and the crowd's loops cleared.
    #[doc(alias = "lbAudioAx_80028690")]
    pub fn lb_reset(&mut self, data: &Archives, bank: &SoundBank) {
        let delay = self.boot_delay;
        self.with_clock(data, |c, f| c.catch_up(BootTimes::RESET + delay, f));
        self.lb.lang = -1;
        self.lb_reset_saved(data, Some(bank), SavedSound::DEFAULT);
    }

    /// `lbAudioAx_80028690` as the save's sound settings say (`saved`): the settings at their
    /// defaults and the save's balance, the bank files named for the saved language's
    /// directory, and when that language isn't the one the files were last named for
    /// (`lbl_804D3878`, -1 at power-on), the common bank (the first time), the sound bank and
    /// the banks every scene keeps loaded for it (`bank`, the language's `smash2.sem`).
    pub fn lb_reset_saved(&mut self, data: &Archives, bank: Option<&SoundBank>, saved: SavedSound) {
        // `lbAudioAx_80024BD0`: the console's mix (`OSGetSoundMode`) noted.
        self.lb.sound_mode = u32::from(!self.mono);
        self.lb_reset_settings(saved.balance);
        self.lb_volumes(true, false);
        // `strcpy`: the directory and its terminator.
        let lang = if saved.us {
            self.lb.ssm_file[..11].copy_from_slice(b"/audio/us/\0");
            self.lb.ssm_stem_pos = 10;
            1
        } else {
            self.lb.ssm_file[..8].copy_from_slice(b"/audio/\0");
            self.lb.ssm_stem_pos = 7;
            0
        };
        let lb = &mut self.lb;
        lb.hps_file[..8].copy_from_slice(b"/audio/\0");
        lb.hps_stem_pos = 7;
        if self.lb.lang == -1 {
            self.sfx_key_off_all();
            self.sfx_unload_bank(0);
            if self.lb.banks[0] < 1 {
                self.lb_load_boot_bank(data, 0, 0);
            }
        }
        if self.lb.lang == lang {
            return;
        }
        self.lb.lang = lang;
        let bank = bank
            .copied()
            .or(self.lang_banks[lang as usize])
            .expect("the language's sound bank (smash2.sem)");
        // The language changes: the sound bank (the old one freed first, `AXDriver_8038DCFC`;
        // none at boot), then the banks every scene keeps.
        self.driver_replace_bank(data, bank);
        self.lb_reload_scene_banks(data, &[(0x33, 1), (1, 1), (0x36, 1), (2, 2)]);
        let lb = &mut self.lb;
        lb.queue = [SFX_NONE; 16];
        lb.queue_timer = [0; 16];
        lb.x6414 = 0;
        lb.duck = 1.0;
        lb.voice_duck = 1.0;
        lb.crowd_duck = 1.0;
        lb.x6418 = 0;
        lb.held = 0;
        lb.crowd = [0; 2];
        lb.crowd_volume = [0; 2];
        lb.crowd_handle = [-1; 2];
        lb.crowd_timer = [0; 2];
        lb.voice_scale = 1.0;
        lb.fx_scale = 1.0;
    }

    /// The sound bank replaced by `bank` (`AXDriver_8038DCFC`, `AXDriver_8038DA70`). A voice
    /// keeps its command stream's pointer, into the freed bank then: kept by its address, and
    /// a voice still playing reads on from the freed block (`freed_banks`).
    pub fn driver_replace_bank(&mut self, data: &Archives, bank: SoundBank) {
        let old = self.driver.bank_addr;
        if let Some(id) = self.driver.bank {
            let freed = &mut self.driver.freed_banks;
            freed.rotate_right(1);
            freed[0] = Some((id, old));
        }
        self.driver_free_bank();
        self.driver_load_bank(data, bank.archive, bank.len);
        let new = self.driver.bank_addr;
        if old != 0 {
            for v in self.driver.voices.iter_mut().filter(|v| v.stream != 0) {
                v.stream = v.stream.wrapping_add(old).wrapping_sub(new);
            }
        }
    }

    /// The banks every scene keeps, loaded again for a new language: every bank's load
    /// forgotten, sound banks 1 and 2 emptied, and `banks` loaded (slot and sound bank), each
    /// waited for: the reset's four (`lbAudioAx_80028690`), or the language options' first
    /// three (`lbAudioAx_80027AB0`), which leave sound bank 2's to the next scene.
    pub fn lb_reload_scene_banks(&mut self, data: &Archives, banks: &[(usize, i32)]) {
        let lb = &mut self.lb;
        for i in 1..55 {
            lb.bank_load[i] = -1;
            lb.bank_want[i] = -1;
            lb.bank_have[i] = -1;
            lb.banks[i] = -1;
        }
        self.sfx_unload_bank(1);
        self.sfx_unload_bank(2);
        for &(slot, b) in banks {
            if self.lb.banks[slot] < 1 {
                self.lb_load_boot_bank(data, slot, b);
            }
        }
    }

    /// Bank `slot`'s file loaded into sound bank `bank`, waited for, and marked loaded.
    fn lb_load_boot_bank(&mut self, data: &Archives, slot: usize, bank: i32) {
        self.lb.set_ssm_name(slot);
        let e = self.lb.ssm_entrynum(slot);
        self.lb.bank_load[slot] = self.sfx_load(data, e, bank, None, 0);
        self.sfx_wait_for_loads(data);
        self.lb.banks[slot] = 2;
    }

    /// `fn_800244F4`: the sound settings' defaults: every volume full, the scales 1, nothing
    /// held, paused, queued or crowding, nothing handed to the synth yet; then the save data's
    /// sound balance, centered by default, sets the announcer's and the effects' volumes
    /// (`gm_801603B0`), full both.
    #[doc(alias = "fn_800244F4")]
    #[doc(alias = "gm_801603B0")]
    fn lb_reset_settings(&mut self, balance: i8) {
        let lb = &mut self.lb;
        lb.music = 0x7F;
        lb.voice = 0x7F;
        lb.fx = 0x7F;
        lb.send = 1;
        lb.music_scale = 1.0;
        lb.scale_1 = 1.0;
        lb.scale_23_base = 1.0;
        lb.scale_23 = 1.0;
        lb.scale_47 = 1.0;
        lb.scale_8 = 1.0;
        lb.held = 0;
        lb.paused = 0;
        lb.duck = 1.0;
        lb.voice_duck = 1.0;
        lb.crowd_duck = 1.0;
        lb.x6414 = 0;
        lb.x6418 = 0;
        lb.crowd = [0; 2];
        lb.crowd_volume = [0; 2];
        lb.crowd_handle = [-1; 2];
        lb.crowd_timer = [0; 2];
        lb.flush_queue = 0;
        lb.sent_1 = 0;
        lb.sent_23 = 0;
        lb.sent_47 = 0;
        lb.sent_8 = 0;
        lb.stream_volume = 1.0;
        lb.volume_1 = 0x7F;
        lb.volume_23 = 0x7F;
        lb.volume_47 = 0x7F;
        lb.volume_8 = 0x7F;
        lb.sent_stream = 0.0;
        lb.master = 0x7F;
        // `gm_801603B0`: the save's balance (`lbAudioAx_80024614`, `lbAudioAx_800245F4`).
        lb.voice = balance_voice(balance).clamp(0, 0x7F);
        lb.fx = balance_fx(balance).clamp(0, 0x7F);
    }

    /// The announcer's and the effects' volumes at a sound balance, as `gm_801603B0` sets
    /// them outside the settings' reset (`lbAudioAx_80024614`, `lbAudioAx_800245F4` clamp).
    pub fn lb_apply_balance(&mut self, balance: i8) {
        self.lb.voice = balance_voice(balance).clamp(0, 0x7F);
        self.lb.fx = balance_fx(balance).clamp(0, 0x7F);
    }

    /// `lbAudioAx_80024F6C`: the stream's volume handed to the synth again, and the
    /// channels resumed: the announcer's and the effects', and the match's unless the pause
    /// menu holds them.
    #[doc(alias = "lbAudioAx_80024F6C")]
    pub fn lb_resume_channels(&mut self) {
        let v = self.lb.stream_volume;
        self.synth_stream_set_volume(v);
        for ch in [2, 3, 4, 9] {
            self.sfx_resume_channel(ch);
        }
        if self.lb.held == 0 {
            for ch in [5, 6, 8, 7] {
                self.sfx_resume_channel(ch);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_third_bank_holds_the_largest_scene_banks() {
        // `HSD_SynthSFXAllocateBank`'s third size in ssbm-rs's boot (0x3239C0).
        let scene = SSM_SIZE[banks_by_size(3)[0]]
            + banks_by_size(4)[..4]
                .iter()
                .map(|&b| SSM_SIZE[b])
                .sum::<u32>()
            + SSM_SIZE[banks_by_size(5)[0]];
        assert_eq!(scene, 0x32_39C0);
    }
}
