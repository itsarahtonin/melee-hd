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

/// The sound bank's file (`smash2.sem`), loaded and parsed by the caller: its archive and
/// size.
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
    pub fn lb_boot(&mut self, data: &Archives) {
        let mut aram = Aram::new();
        let main = SSM_SIZE[0];
        let kept = SSM_SIZE[0x33] + SSM_SIZE[1] + SSM_SIZE[0x36];
        let mut scene = SSM_SIZE[banks_by_size(3)[0]];
        let fighters = banks_by_size(4);
        scene += fighters[..4].iter().map(|&b| SSM_SIZE[b]).sum::<u32>();
        scene += SSM_SIZE[banks_by_size(5)[0]];
        let room = main + kept + scene;
        self.init_multi_pstream(data, &mut aram, room);
        self.with_clock(data, |c, f| c.catch_up(BootTimes::AUX, f));
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
    }

    /// `lbAudioAx_80028690`, the sound settings' reset when the save data is: the settings
    /// at their defaults (`fn_800244F4`) handed to the synth (`fn_80024654(1)`), the bank
    /// files named for the US directory (the disc has `usa.ini`), and, as the language they
    /// were named for changes from none, the common bank loaded into bank 0, the sound bank
    /// (`AXDriver_8038DA70`), and the banks every scene keeps into banks 1 and 2, each waited
    /// for (`lb_800195D0` spinning); then the queued sounds and the crowd's loops cleared.
    #[doc(alias = "lbAudioAx_80028690")]
    pub fn lb_reset(&mut self, data: &Archives, bank: &SoundBank) {
        self.with_clock(data, |c, f| c.catch_up(BootTimes::RESET, f));
        // `lbAudioAx_80024BD0` reads the sound mode into a global the port doesn't keep.
        self.lb_reset_settings();
        self.lb_volumes(true, false);
        let lb = &mut self.lb;
        lb.ssm_file[..10].copy_from_slice(b"/audio/us/");
        lb.ssm_stem_pos = 10;
        lb.hps_file[..7].copy_from_slice(b"/audio/");
        lb.hps_stem_pos = 7;
        // The language was none: the common bank first.
        self.sfx_key_off_all();
        self.sfx_unload_bank(0);
        if self.lb.banks[0] < 1 {
            self.lb_load_boot_bank(data, 0, 0);
        }
        // The language changes: the sound bank (no old one to free, `AXDriver_8038DCFC`),
        // then the banks every scene keeps.
        self.driver_load_bank(data, bank.archive, bank.len);
        let lb = &mut self.lb;
        for i in 1..55 {
            lb.bank_load[i] = -1;
            lb.bank_want[i] = -1;
            lb.bank_have[i] = -1;
            lb.banks[i] = -1;
        }
        self.sfx_unload_bank(1);
        self.sfx_unload_bank(2);
        for (slot, b) in [(0x33, 1), (1, 1), (0x36, 1), (2, 2)] {
            if self.lb.banks[slot] < 1 {
                self.lb_load_boot_bank(data, slot, b);
            }
        }
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
    fn lb_reset_settings(&mut self) {
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
