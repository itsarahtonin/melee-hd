//! The game's audio library (`melee/lb/lbaudio_ax.c`) as far as game code plays and stops
//! sounds through it: volumes and pans in the game's 0-127 range doubled for the driver, the
//! fighters' and stages' tracks, Kirby's copied voices, and its tick: the crowd's two loops,
//! the sounds fighters queue, and the channels' volumes from the game's settings.

use gekko_fp as fp;

use crate::fpu::{mem, reg};
use crate::game::state::{Event, Running};

/// The id that plays nothing, and the one that keys a track off instead of playing.
pub const SFX_NONE: i32 = 0x83D60;
pub const SFX_KEY_OFF: i32 = 0x83D61;

/// The game's audio library's own state.
#[derive(Clone, Debug, PartialEq)]
pub struct LbAudio {
    /// `lbl_80433984`: each sound bank's load state (-1 not loaded, 1 loaded in the
    /// background, 2 loaded).
    pub banks: [i32; 56],
    /// `lbl_804337C4`, `lbl_804338A4`: the banks the next scene wants and the ones the loader
    /// works toward (-1 or 1); `lbl_80433A64`: each bank's load (its file's entry number) or
    /// -1.
    pub bank_want: [i32; 56],
    pub bank_have: [i32; 56],
    pub bank_load: [i32; 56],
    /// `lbl_804D6438`: the ARAM the wanted banks may take; `lbl_804D643C`, `6440`, `6444`:
    /// the three sound banks' sizes; `lbl_804D6448`: what's loaded; `lbl_804D644C`: what's
    /// wanted; `lbl_804D6450`: what's still to load.
    pub fgm_room: i32,
    pub fgm_banks: [i32; 3],
    pub fgm_loaded: i32,
    pub fgm_wanted: i32,
    pub fgm_pending: u32,
    /// `cur_ssm_file` and `ssm_stem_pos`: the bank file name being loaded, after its
    /// directory.
    pub ssm_file: [u8; 0x40],
    pub ssm_stem_pos: i32,
    /// `lbl_804D640C`: the music and the match's channels are held for the pause menu; the
    /// crowd's and the queue's timers stand still.
    pub held: i32,
    /// `paused`: the driver's sounds are paused (`lbAudioAx_80024E50`).
    pub paused: i32,
    /// `lbl_804D641C`: the queued sounds play at the next tick, all at once.
    pub flush_queue: i32,
    /// `lbl_804D6414`, `lbl_804D6418`: only ever cleared.
    pub x6414: i32,
    pub x6418: i32,
    /// `lbl_804D6420`, `lbl_804D6424`: how many want each of the crowd's loops (sounds 0x84
    /// and 0x85, on tracks 5 and 6); `lbl_804D6430`, `lbl_804D6434`: ticks until each stops
    /// unless wanted again; `lbl_804D6428`, `lbl_804D642C`: their volumes; `lbl_804D38F0`,
    /// `lbl_804D38F4`: their handles, or -1.
    pub crowd: [i32; 2],
    pub crowd_timer: [i32; 2],
    pub crowd_volume: [i32; 2],
    pub crowd_handle: [i32; 2],
    /// `lbl_8043373C`, `lbl_80433780`: the sounds fighters and items queue (or
    /// [`SFX_NONE`]), each kept ten ticks unless queued again.
    pub queue: [i32; 16],
    pub queue_timer: [i32; 16],
    /// `fn_80024654`'s settings: `lbl_804D387C`, the music's volume (0-127); `lbl_804D38C0`,
    /// its scale; `lbl_804D388C` and `lbl_804D3884`, the effects' and the announcer's volumes
    /// (0-127); `lbl_804D38CC`, the effects' master (0-127); `lbl_804D38C8`, `lbl_804D38C4`,
    /// `lbl_804D38E4`, `lbl_804D38E8`, `lbl_804D38EC`: scales (the pause menu's duck and the
    /// crowd's); `lbl_804D38D8`: the music channels' aux B send.
    pub music: i32,
    pub music_scale: f32,
    pub fx: i32,
    pub voice: i32,
    pub master: i32,
    pub fx_scale: f32,
    pub voice_scale: f32,
    pub duck: f32,
    pub voice_duck: f32,
    pub crowd_duck: f32,
    pub send: i32,
    /// What `fn_80024654` derived: `synth_volume`, the stream's volume; `lbl_804D38B0`,
    /// `lbl_804D38B4`, `lbl_804D38B8`, `lbl_804D38BC`, `lbl_804D38AC`: the scales of channels
    /// 2-3, 4-7, 8 and 1; `lbl_804D3898`, `lbl_804D38A0`, `lbl_804D38A8`, `lbl_804D3888`: their
    /// volumes; `lbl_804D38E0`: the send.
    pub stream_volume: f32,
    pub scale_23_base: f32,
    pub scale_23: f32,
    pub scale_47: f32,
    pub scale_8: f32,
    pub scale_1: f32,
    pub volume_23: i32,
    pub volume_47: i32,
    pub volume_8: i32,
    pub volume_1: i32,
    pub send_wanted: i32,
    /// What it last handed the synth, to hand only changes: `lbl_804D63F0` (the stream's
    /// volume), `lbl_804D63FC`, `lbl_804D6400`, `lbl_804D6404`, `lbl_804D63F4` (channels 2-3,
    /// 4-7, 8, 1) and `lbl_804D38DC` (the send).
    pub sent_stream: f32,
    pub sent_23: i32,
    pub sent_47: i32,
    pub sent_8: i32,
    pub sent_1: i32,
    pub sent_send: i32,
    /// `cur_hps_stem`: the playing music file's stem, empty with no music.
    pub stem: [u8; 0x40],
}

impl Default for LbAudio {
    /// As the game's data initializes it.
    fn default() -> Self {
        let mut ssm_file = [0u8; 0x40];
        ssm_file[..7].copy_from_slice(b"/audio/");
        LbAudio {
            banks: [0; 56],
            bank_want: [0; 56],
            bank_have: [0; 56],
            bank_load: [0; 56],
            fgm_room: 0,
            fgm_banks: [0; 3],
            fgm_loaded: 0,
            fgm_wanted: 0,
            fgm_pending: 0,
            ssm_file,
            ssm_stem_pos: 7,
            held: 0,
            paused: 0,
            flush_queue: 0,
            x6414: 0,
            x6418: 0,
            crowd: [0; 2],
            crowd_timer: [0; 2],
            crowd_volume: [0; 2],
            crowd_handle: [-1; 2],
            queue: [0; 16],
            queue_timer: [0; 16],
            music: 0x7F,
            music_scale: 1.0,
            fx: 0x7F,
            voice: 0x7F,
            master: 0x7F,
            fx_scale: 1.0,
            voice_scale: 1.0,
            duck: 1.0,
            voice_duck: 1.0,
            crowd_duck: 1.0,
            send: 1,
            stream_volume: 1.0,
            scale_23_base: 1.0,
            scale_23: 1.0,
            scale_47: 1.0,
            scale_8: 1.0,
            scale_1: 1.0,
            volume_23: 0x7F,
            volume_47: 0x7F,
            volume_8: 0x7F,
            volume_1: 0x7F,
            send_wanted: 1,
            sent_stream: 0.0,
            sent_23: 0,
            sent_47: 0,
            sent_8: 0,
            sent_1: 0,
            sent_send: -1,
            stem: [0; 0x40],
        }
    }
}

/// An int setting over 127 as the game computes it: the int to a float, over 127.
fn over_127(x: i32) -> f64 {
    fp::fdivs(fp::frsp(f64::from(x)), reg(127.0))
}

/// A float cut to a byte, as `(u8)` cuts one (`fctiwz`, then the low byte).
fn byte(x: f64) -> i32 {
    fp::fctiwz(x) & 0xFF
}

/// The sound bank a sound id belongs to, 55 for none, `lbAudioAx_80023130`.
#[doc(alias = "lbAudioAx_80023130")]
pub fn sfx_bank(id: i32) -> i32 {
    if (0..SFX_NONE).contains(&id) {
        for (i, &(lo, hi)) in SSM_RANGES.iter().enumerate().take(55) {
            if lo <= id && id <= hi {
                return i as i32;
            }
        }
    }
    55
}

/// A sound bank's id range, `lbAudioAx_800230C8`; none for an invalid bank.
#[doc(alias = "lbAudioAx_800230C8")]
pub fn bank_range(bank: i32) -> Option<(i32, i32)> {
    (0..55).contains(&bank).then(|| SSM_RANGES[bank as usize])
}

/// The offset of a fighter bank's sounds that come in sizes (small, normal, big),
/// `lbAudioAx_80023220`.
#[doc(alias = "lbAudioAx_80023220")]
pub fn bank_size_step(bank: i32) -> i32 {
    if (0..55).contains(&bank) {
        i32::from(SSM_SIZE_STEP[bank as usize])
    } else {
        0
    }
}

impl LbAudio {
    /// Kirby's copies of other fighters' voices, `lbAudioAx_800233EC`: with Kirby's bank of
    /// them loaded, an original voice becomes Kirby's copy; without it, a copy becomes the
    /// original.
    #[doc(alias = "lbAudioAx_800233EC")]
    pub fn kirby_voice(&self, id: i32) -> i32 {
        let bank = sfx_bank(id);
        let pairs = &KIRBY_SFX[..0x4A];
        if self.banks[0x21] == 2 {
            if matches!(bank, 6..=11 | 13 | 15..=31)
                && let Some(&(_, copy)) = pairs.iter().find(|p| p.0 == id)
            {
                return copy;
            }
        } else if bank == 0x21
            && let Some(&(orig, _)) = pairs.iter().find(|p| p.1 == id)
        {
            return orig;
        }
        id
    }
}

impl Running<'_> {
    /// `fn_80023750`: a sound at the game's volume and pan (0-127, doubled for the driver).
    fn sfx_start(&mut self, id: i32, vol: i32, pan: i32, track: i32, channel: i32) -> i32 {
        let vol = vol.wrapping_mul(2).clamp(0, 0xFF) as u8;
        let pan = pan.wrapping_mul(2).clamp(0, 0xFF) as u8;
        let handle = self
            .game
            .audio
            .sfx_start(self.data, id, vol, pan, track, channel);
        self.game.events.push(Event::SfxStart {
            handle,
            id,
            volume: vol,
            pan,
            track,
        });
        handle
    }

    /// Plays a sound on channel 5 or 6, where the crowd's chants and cheers go
    /// (`lbAudioAx_800240B4`, `lbAudioAx_8002411C`).
    #[doc(alias = "lbAudioAx_800240B4")]
    #[doc(alias = "lbAudioAx_8002411C")]
    pub fn sfx_play_channel(&mut self, id: i32, channel: i32) -> i32 {
        self.sfx_start(id, 0x7F, 0x40, 0, channel)
    }

    /// Plays a sound on no track, `lbAudioAx_800237A8`. Ids past the last play the silent
    /// one.
    #[doc(alias = "lbAudioAx_800237A8")]
    pub fn sfx_play(&mut self, id: i32, vol: i32, pan: i32) -> i32 {
        if id >= SFX_KEY_OFF {
            self.sfx_start(SFX_NONE, 0, 0x40, 0, 7)
        } else {
            self.sfx_start(id, vol, pan, 0, 7)
        }
    }

    /// Plays a sound on the track some ids keep, `lbAudioAx_80024184`: the shield, hit and
    /// crowd sounds have tracks of their own, id 525 takes `track` (none for -1), and the
    /// rest play on none.
    #[doc(alias = "lbAudioAx_80024184")]
    pub fn sfx_play_tracked(&mut self, id: i32, vol: i32, pan: i32, track: i32) -> i32 {
        let track = match id {
            86 => 9,
            87 => 8,
            88 => 7,
            89 => 12,
            90 => 11,
            91 => 10,
            111 => 13,
            112 => 14,
            113 => 15,
            101 => 16,
            102 => 17,
            103 => 18,
            84 => 23,
            0x12E => 24,
            0x20D if track != -1 => track,
            _ => 0,
        };
        self.sfx_start(id, vol, pan, track, 7)
    }

    /// Plays a sound on a track, cutting off the track's last one, or keys the track off for
    /// [`SFX_KEY_OFF`], `lbAudioAx_80023870`.
    #[doc(alias = "lbAudioAx_80023870")]
    pub fn sfx_play_track(&mut self, id: i32, vol: i32, pan: i32, track: i32) -> i32 {
        if track == 0 {
            return self.sfx_play(id, vol, pan);
        }
        if id == SFX_KEY_OFF {
            self.sfx_key_off_track(track);
            return -1;
        }
        self.sfx_start(id, vol, pan, track, 7)
    }

    /// `HSD_AudioSFXKeyOffTrack`, with its event.
    pub fn sfx_key_off_track(&mut self, track: i32) {
        self.game.audio.sfx_key_off_track(track);
        self.game.events.push(Event::SfxKeyOffTrack(track));
    }

    /// Stops a sound, `lbAudioAx_800236B8`.
    #[doc(alias = "lbAudioAx_800236B8")]
    pub fn sfx_stop(&mut self, handle: i32) -> i32 {
        self.game.audio.sfx_key_off(handle);
        self.game.events.push(Event::SfxKeyOff(handle));
        -1
    }

    /// Stops every sound, `lbAudioAx_80023694`.
    #[doc(alias = "lbAudioAx_80023694")]
    pub fn sfx_stop_all(&mut self) -> i32 {
        self.game.audio.sfx_key_off_all();
        self.game.events.push(Event::SfxKeyOffAll);
        -1
    }

    /// Whether a sound still plays, `lbAudioAx_80023710`.
    #[doc(alias = "lbAudioAx_80023710")]
    pub fn sfx_playing(&self, handle: i32) -> bool {
        self.game.audio.sfx_check(handle)
    }

    /// Pans a sound (0-127), `lbAudioAx_80024B1C`.
    #[doc(alias = "lbAudioAx_80024B1C")]
    pub fn sfx_pan(&mut self, handle: i32, pan: i32) {
        let pan = (pan.clamp(0, 0x7F) * 2) as u8;
        self.game.audio.sfx_set_pan(handle, pan);
        self.game.events.push(Event::SfxPan { handle, pan });
    }

    /// Sets a sound's volume (0-127), `lbAudioAx_80024B58`.
    #[doc(alias = "lbAudioAx_80024B58")]
    pub fn sfx_volume(&mut self, handle: i32, vol: i32) {
        let volume = (vol.clamp(0, 0x7F) * 2) as u8;
        self.game.audio.sfx_set_volume(handle, volume);
        self.game.events.push(Event::SfxVolume { handle, volume });
    }

    /// The library's tick, `lbAudioAx_80027DF8`, which the scene loop runs before the
    /// processes: the crowd's loops start, follow their volumes or stop, the channels'
    /// volumes follow the settings, and the queued sounds' timers run (or, once asked, the
    /// queued sounds all play).
    #[doc(alias = "lbAudioAx_80027DF8")]
    pub fn lb_audio_tick(&mut self) {
        let lb = &mut self.game.audio.lb;
        if lb.held == 0 {
            for i in 0..2 {
                lb.crowd_timer[i] -= 1;
                if lb.crowd_timer[i] <= 0 {
                    lb.crowd[i] = 0;
                    lb.crowd_timer[i] = 0;
                }
            }
        }
        for i in 0..2 {
            self.crowd_loop(i);
        }
        let lb = &mut self.game.audio.lb;
        lb.crowd_duck = if lb.crowd_handle == [-1, -1] {
            1.0
        } else {
            0.2
        };
        self.lb_audio_volumes();
        if self.game.audio.lb.flush_queue == 1 {
            for i in 0..16 {
                let id = self.game.audio.lb.queue[i];
                if id != SFX_NONE {
                    self.sfx_play(id, 0x7F, 0x40);
                    let lb = &mut self.game.audio.lb;
                    lb.queue[i] = SFX_NONE;
                    lb.queue_timer[i] = 0;
                }
            }
            self.game.audio.lb.flush_queue = 0;
            return;
        }
        let lb = &mut self.game.audio.lb;
        if lb.held == 0 {
            for i in 0..16 {
                if lb.queue_timer[i] > 0 {
                    lb.queue_timer[i] -= 1;
                }
                if lb.queue_timer[i] <= 0 {
                    lb.queue[i] = SFX_NONE;
                    lb.queue_timer[i] = 0;
                }
            }
        }
    }

    /// One of the crowd's loops in the tick: started at full volume if wanted and not
    /// playing, restarted if it ended, its volume followed; stopped (the silent sound on its
    /// track) once no longer wanted, the other loop then at full volume.
    fn crowd_loop(&mut self, i: usize) {
        let (id, track) = [(0x84, 5), (0x85, 6)][i];
        let lb = &self.game.audio.lb;
        let (wanted, handle, vol) = (lb.crowd[i], lb.crowd_handle[i], lb.crowd_volume[i]);
        if wanted != 0 {
            if handle != -1 {
                if !self.game.audio.sfx_check(handle) {
                    let h = self.sfx_start(id, vol, 0x40, track, 4);
                    self.game.audio.lb.crowd_handle[i] = h;
                } else {
                    self.sfx_volume(handle, vol);
                }
            } else {
                self.game.audio.lb.crowd_volume[i] = 0x7F;
                let h = self.sfx_start(id, 0x7F, 0x40, track, 4);
                let lb = &mut self.game.audio.lb;
                lb.crowd_handle[i] = h;
                lb.crowd_volume[1 - i] = 0;
            }
        } else if handle != -1 {
            self.sfx_start(SFX_NONE, vol, 0x40, track, 4);
            let lb = &mut self.game.audio.lb;
            lb.crowd_volume[i] = 0;
            lb.crowd_handle[i] = -1;
            lb.crowd_volume[1 - i] = if lb.crowd_handle[1 - i] == -1 {
                0
            } else {
                0x7F
            };
        }
    }

    /// The channels' volumes from the settings, `fn_80024654`: the stream's, then channels
    /// 2-3 (effects), 4-7, 8 and 1 (the announcer), and the music channels' aux B send, each
    /// handed to the synth when it changed (`fn_80024654(0)`; the scene setups' `(1)`, which
    /// hands them all, isn't ported).
    #[doc(alias = "fn_80024654")]
    pub fn lb_audio_volumes(&mut self) {
        let a = &mut self.game.audio;
        let lb = &mut a.lb;
        lb.stream_volume = mem(fp::fmuls(over_127(lb.music), reg(lb.music_scale)));
        if lb.sent_stream != lb.stream_volume {
            let v = lb.stream_volume;
            a.synth_stream_set_volume(v);
            a.lb.sent_stream = v;
        }
        let lb = &mut a.lb;
        let master = over_127(lb.master);
        let fx = fp::fmuls(reg(0.88), fp::frsp(f64::from(lb.fx * 2)));
        lb.scale_23_base = lb.fx_scale;
        lb.scale_23 = mem(fp::fmuls(reg(lb.fx_scale), master));
        lb.volume_23 = byte(fp::fmuls(reg(lb.scale_23), fx));
        if lb.sent_23 != lb.volume_23 {
            let v = lb.volume_23;
            a.synth_update_all_volume(v, 4, 2);
            a.synth_update_all_volume(v, 4, 3);
            a.lb.sent_23 = v;
        }
        let lb = &mut a.lb;
        lb.scale_47 = mem(fp::fmuls(master, fp::fmuls(reg(lb.fx_scale), reg(lb.duck))));
        lb.volume_47 = byte(fp::fmuls(reg(lb.scale_47), fx));
        if lb.sent_47 != lb.volume_47 {
            let v = lb.volume_47;
            for ch in 4..8 {
                a.synth_update_all_volume(v, 4, ch);
            }
            a.lb.sent_47 = v;
        }
        let lb = &mut a.lb;
        lb.scale_8 = mem(fp::fmuls(reg(lb.fx_scale), reg(lb.duck)));
        lb.volume_8 = byte(fp::fmuls(reg(lb.scale_8), fx));
        if lb.sent_8 != lb.volume_8 {
            let v = lb.volume_8;
            a.synth_update_all_volume(v, 4, 8);
            a.lb.sent_8 = v;
        }
        let lb = &mut a.lb;
        lb.scale_1 = mem(fp::fmuls(
            master,
            fp::fmuls(
                reg(lb.crowd_duck),
                fp::fmuls(reg(lb.voice_scale), reg(lb.voice_duck)),
            ),
        ));
        lb.volume_1 = byte(fp::fmuls(
            reg(lb.scale_1),
            fp::frsp(f64::from(lb.voice * 2)),
        ));
        if lb.sent_1 != lb.volume_1 {
            let v = lb.volume_1;
            a.synth_update_all_volume(v, 4, 1);
            a.lb.sent_1 = v;
        }
        let lb = &mut a.lb;
        lb.send_wanted = lb.send;
        if lb.sent_send != lb.send_wanted {
            let send = lb.send as u8 as i8;
            a.sfx_set_mix_group(5, 1, 0x20);
            a.sfx_set_mix_group(6, 1, 0x20);
            a.sfx_set_mix_group(8, 1, send);
            a.sfx_set_mix_group(7, 1, send);
            a.lb.sent_send = a.lb.send_wanted;
        }
    }

    /// Queues a sound to play when the queue next empties at once (`lbAudioAx_80024DC4`):
    /// one already queued keeps its place for ten more ticks.
    #[doc(alias = "lbAudioAx_80024DC4")]
    pub fn lb_audio_queue(&mut self, id: i32) {
        let lb = &mut self.game.audio.lb;
        if let Some(i) = lb.queue.iter().position(|&q| q == id) {
            lb.queue_timer[i] = 10;
        } else if let Some(i) = lb.queue.iter().position(|&q| q == SFX_NONE) {
            lb.queue[i] = id;
            lb.queue_timer[i] = 10;
        }
    }

    /// The crowd's first loop wanted once more, for 510 ticks (`lbAudioAx_80024FDC`).
    #[doc(alias = "lbAudioAx_80024FDC")]
    pub fn lb_audio_crowd_a(&mut self) {
        let lb = &mut self.game.audio.lb;
        lb.crowd_timer[0] = 0x1FE;
        lb.crowd[0] += 1;
    }

    /// The crowd's second loop wanted once more, for 480 ticks (`lbAudioAx_80024FF4`).
    #[doc(alias = "lbAudioAx_80024FF4")]
    pub fn lb_audio_crowd_b(&mut self) {
        let lb = &mut self.game.audio.lb;
        lb.crowd_timer[1] = 0x1E0;
        lb.crowd[1] += 1;
    }

    /// `n` fewer want the crowd's first loop (`lbAudioAx_8002500C`).
    #[doc(alias = "lbAudioAx_8002500C")]
    pub fn lb_audio_crowd_a_less(&mut self, n: i32) {
        let lb = &mut self.game.audio.lb;
        if n > 0 {
            lb.crowd[0] -= n;
            if lb.crowd[0] <= 0 {
                lb.crowd[0] = 0;
            }
        }
    }

    /// `n` fewer want the crowd's second loop (`lbAudioAx_80025038`).
    #[doc(alias = "lbAudioAx_80025038")]
    pub fn lb_audio_crowd_b_less(&mut self, n: i32) {
        let lb = &mut self.game.audio.lb;
        if n > 0 {
            lb.crowd[1] -= n;
            if lb.crowd[1] < 0 {
                lb.crowd[1] = 0;
            }
        }
    }

    /// Holds or releases the music and the match's channels for the pause menu
    /// (`lbAudioAx_80024E84`): ducked to a fifth and paused, or back.
    #[doc(alias = "lbAudioAx_80024E84")]
    pub fn lb_audio_hold(&mut self, hold: bool) {
        let a = &mut self.game.audio;
        a.lb.held = i32::from(hold);
        let (duck, set): (f32, fn(&mut crate::audio::Audio, i32) -> bool) = if hold {
            (0.2, crate::audio::Audio::sfx_pause_channel)
        } else {
            (1.0, crate::audio::Audio::sfx_resume_channel)
        };
        a.lb.duck = duck;
        a.lb.voice_duck = duck;
        for ch in [5, 6, 8, 7] {
            set(a, ch);
        }
    }

    /// Plays one of the announcer's and the crowd's set sounds (`lbAudioAx_80024030`).
    #[doc(alias = "lbAudioAx_80024030")]
    pub fn lb_audio_call(&mut self, which: i32) {
        const CALLS: [(i32, i32, i32); 11] = [
            (0xAC, 0x13, 3),
            (0xAD, 0x14, 3),
            (0xAE, 0x00, 3),
            (0xAF, 0x00, 3),
            (0xB0, 0x15, 3),
            (0xB1, 0x00, 2),
            (0xC1, 0x00, 2),
            (0xBC, 0x00, 2),
            (0xDB, 0x00, 3),
            (0xDC, 0x00, 3),
            (0xDD, 0x00, 3),
        ];
        if which < 0xB {
            let (id, track, channel) = CALLS[which as usize];
            self.sfx_start(id, 0x7F, 0x40, track, channel);
        }
    }

    /// Pauses or resumes the driver's sounds (`lbAudioAx_80024E50`); the match's channels
    /// resume unless held for the pause menu.
    #[doc(alias = "lbAudioAx_80024E50")]
    pub fn lb_audio_pause(&mut self, pause: bool) {
        let a = &mut self.game.audio;
        a.lb.paused = i32::from(pause);
        if pause {
            a.driver_pause_stream();
        } else {
            a.driver_resume_stream();
        }
        if a.lb.held == 0 {
            for ch in [5, 6, 8, 7] {
                a.sfx_resume_channel(ch);
            }
        }
    }

    /// Stops the music, `lbAudioAx_800236DC`: the driver's stream stops and no file plays.
    #[doc(alias = "lbAudioAx_800236DC")]
    pub fn lb_audio_music_stop(&mut self) -> i32 {
        self.game.audio.driver_stop();
        let lb = &mut self.game.audio.lb;
        lb.x6418 = 0;
        lb.stem[0] = 0;
        -1
    }

    /// Resets the library's mix for a new scene (`lbAudioAx_80024C84`): nothing held or
    /// paused, the crowd quiet and its tracks keyed off, the scales back to one and the
    /// match's channels resumed.
    #[doc(alias = "lbAudioAx_80024C84")]
    pub fn lb_audio_reset(&mut self) {
        let lb = &mut self.game.audio.lb;
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
        self.sfx_key_off_track(5);
        self.sfx_key_off_track(6);
        let lb = &mut self.game.audio.lb;
        lb.flush_queue = 0;
        lb.music_scale = 1.0;
        lb.scale_1 = 1.0;
        lb.scale_23_base = 1.0;
        lb.scale_23 = 1.0;
        lb.scale_47 = 1.0;
        lb.scale_8 = 1.0;
        for ch in [5, 6, 8, 7] {
            self.game.audio.sfx_resume_channel(ch);
        }
        let lb = &mut self.game.audio.lb;
        lb.send = 1;
        lb.master = 0x7F;
    }

    /// [`lb_audio_reset`](Self::lb_audio_reset), then the queued sounds play at the next
    /// tick (`lbAudioAx_80024D50`).
    #[doc(alias = "lbAudioAx_80024D50")]
    pub fn lb_audio_reset_flush(&mut self) {
        self.lb_audio_reset();
        self.game.audio.lb.flush_queue = 1;
    }

    /// Bends a sound's pitch, in cents within an octave, `lbAudioAx_80024B94`.
    #[doc(alias = "lbAudioAx_80024B94")]
    pub fn sfx_pitch(&mut self, handle: i32, cents: i32) -> bool {
        let cents = cents.clamp(-0x4B0, 0x4B0) as i16;
        self.game.events.push(Event::SfxPitch { handle, cents });
        self.game.audio.sfx_set_pitch(handle, cents)
    }
}

/// `s32_arr_803BB8D4`: each sound bank's id range.
const SSM_RANGES: [(i32, i32); 56] = [
    (0x0, 0x20f),
    (0x2710, 0x2750),
    (0x4e20, 0x4e25),
    (0x7530, 0x7546),
    (0x9c40, 0x9c4a),
    (0xc350, 0xc356),
    (0xea60, 0xeab4),
    (0x11170, 0x111df),
    (0x13880, 0x138c2),
    (0x15f90, 0x15ff5),
    (0x186a0, 0x1871c),
    (0x1adb0, 0x1ae30),
    (0x1d4c0, 0x1d504),
    (0x1fbd0, 0x1fcc8),
    (0x222e0, 0x22379),
    (0x249f0, 0x24a3f),
    (0x27100, 0x2716c),
    (0x29810, 0x29877),
    (0x2bf20, 0x2bf7d),
    (0x2e630, 0x2e6b4),
    (0x30d40, 0x30dd3),
    (0x33450, 0x334b5),
    (0x35b60, 0x35bba),
    (0x38270, 0x382d0),
    (0x3a980, 0x3a9e6),
    (0x3d090, 0x3d0de),
    (0x3f7a0, 0x3f7ea),
    (0x41eb0, 0x41f77),
    (0x445c0, 0x44623),
    (0x46cd0, 0x46d17),
    (0x493e0, 0x4943a),
    (0x4baf0, 0x4bb73),
    (0x4e200, 0x4e21d),
    (0x50910, 0x509ed),
    (0x53020, 0x53027),
    (0x55730, 0x55749),
    (0x57e40, 0x57e4a),
    (0x5a550, 0x5a551),
    (0x5cc60, 0x5cc6a),
    (0x5f370, 0x5f37d),
    (0x61a80, 0x61a8b),
    (0x64190, 0x64194),
    (0x668a0, 0x668a8),
    (0x68fb0, 0x68fb9),
    (0x6b6c0, 0x6b6d8),
    (0x6ddd0, 0x6ddd6),
    (0x704e0, 0x704e4),
    (0x72bf0, 0x72bf8),
    (0x75300, 0x75301),
    (0x77a10, 0x77a16),
    (0x7a120, 0x7a12f),
    (0x7c830, 0x7c865),
    (0x7ef40, 0x7ef41),
    (0x81650, 0x81654),
    (0x83d60, 0x83d60),
    (0x83d60, 0x83d60),
];

/// `s32_arr_803BB5D0`'s last column: where a fighter bank's sized sounds start.
const SSM_SIZE_STEP: [i8; 56] = [
    0, 0, 0, 0, 0, 0, 4, 7, 7, 6, 6, 4, 0, 9, 1, 8, 7, 2, 1, 7, 1, 6, 1, 1, 7, 1, 6, 2, 1, 6, 7, 6,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/// `offsets_arr_803BC6A4`: other fighters' voices and Kirby's copies of them.
const KIRBY_SFX: [(i32, i32); 75] = [
    (150050, 330000),
    (150053, 330003),
    (150056, 330006),
    (150059, 330009),
    (260006, 330012),
    (260009, 330015),
    (260012, 330018),
    (260015, 330021),
    (260018, 330024),
    (260021, 330027),
    (80007, 330030),
    (80010, 330033),
    (210069, 330036),
    (210072, 330039),
    (210084, 330042),
    (240076, 330045),
    (240079, 330048),
    (60079, 330051),
    (110100, 330054),
    (110103, 330057),
    (110106, 330060),
    (110109, 330063),
    (110085, 330066),
    (180025, 330069),
    (180007, 330072),
    (160082, 330075),
    (160061, 330078),
    (160064, 330081),
    (220061, 330084),
    (130021, 330087),
    (130024, 330090),
    (280073, 330093),
    (280076, 330096),
    (280088, 330099),
    (90030, 330102),
    (90012, 330105),
    (100096, 330108),
    (100099, 330111),
    (100102, 330114),
    (100105, 330117),
    (100081, 330120),
    (300085, 330123),
    (290063, 330126),
    (310102, 330129),
    (310105, 330132),
    (310108, 330135),
    (250064, 330138),
    (250067, 330141),
    (250070, 330144),
    (250073, 330147),
    (70082, 330150),
    (70061, 330153),
    (70064, 330156),
    (170026, 330159),
    (170008, 330162),
    (190103, 330165),
    (190106, 330168),
    (190109, 330171),
    (200106, 330174),
    (200109, 330177),
    (200112, 330180),
    (200115, 330183),
    (200118, 330186),
    (200121, 330189),
    (200124, 330192),
    (200127, 330195),
    (200130, 330198),
    (270134, 330201),
    (270137, 330204),
    (270140, 330207),
    (270191, 330210),
    (270197, 330213),
    (230067, 330216),
    (230070, 330219),
    (540000, 540000),
];

/// `fn_80026C04`: the library's callback when one of its background bank loads is in.
pub const BANK_LOADED: crate::game::handles::CodeRef = crate::game::handles::CodeRef(0x8002_6C04);

/// `ssm_files`' entry numbers on the disc (GALE01 revision 2's file table), with the US
/// language's directory (`/audio/us/`) and without (`/audio/`).
pub const SSM_US: [i32; 55] = [
    169, 185, 179, 178, 176, 180, 142, 144, 146, 147, 151, 153, 156, 160, 161, 165, 167, 168, 170,
    171, 172, 175, 182, 183, 184, 188, 189, 194, 192, 159, 154, 148, 173, 162, 143, 145, 157, 164,
    174, 181, 193, 155, 163, 158, 191, 141, 152, 187, 186, 139, 150, 177, 140, 166, 149,
];
const SSM_JP: [i32; 55] = [
    84, 111, 99, 98, 96, 100, 9, 12, 16, 18, 23, 48, 53, 63, 74, 79, 82, 83, 85, 86, 90, 95, 108,
    109, 110, 117, 119, 212, 208, 58, 50, 19, 91, 75, 11, 15, 55, 78, 94, 104, 211, 52, 76, 57,
    196, 8, 47, 115, 114, 2, 22, 97, 3, 81, 20,
];

/// `s32_arr_803BB5D0`'s second column: each bank's load priority (5: never in the
/// background).
const SSM_PRIORITY: [i8; 56] = [
    5, 5, 0, 0, 4, 4, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 5, 1, 2, 5, 0,
];

/// `offsets_arr_803BC4E4`'s first column: each bank's size in ARAM.
const SSM_SIZE: [u32; 56] = [
    2045824, 561760, 153024, 239264, 121888, 54176, 443328, 298400, 206368, 430560, 599712, 573216,
    487936, 477600, 586208, 526752, 328672, 372992, 372736, 513088, 562912, 509024, 416736, 580128,
    613088, 334528, 319040, 590176, 321472, 165344, 427872, 495520, 563712, 572832, 176224, 441760,
    490240, 360064, 150144, 54208, 387872, 428896, 304096, 144480, 382272, 34880, 78592, 123104,
    101504, 160576, 1168864, 348160, 363936, 663136, 1536, 0,
];

impl LbAudio {
    /// The disc file of bank `slot` in the language's directory.
    pub fn ssm_entrynum(&self, slot: usize) -> i32 {
        if self.ssm_stem_pos == 10 {
            SSM_US[slot]
        } else {
            SSM_JP[slot]
        }
    }

    /// `fn_80026650`: the next bank to load in the background: the wanted one of the highest
    /// priority not loaded or loading.
    #[doc(alias = "fn_80026650")]
    fn next_background_bank(&self) -> Option<usize> {
        (0..=4).rev().find_map(|p| {
            (0..55).find(|&i| {
                i32::from(SSM_PRIORITY[i]) == p && self.bank_have[i] == 1 && self.banks[i] == -1
            })
        })
    }
}

impl crate::audio::Audio {
    /// `fn_80026C04`: a background bank load is in (its size counted as loaded), and the
    /// next one starts.
    #[doc(alias = "fn_80026C04")]
    pub fn lb_bank_loaded(&mut self, data: &melee_data::archive::Archives, entrynum: i32, _: i32) {
        let lb = &mut self.lb;
        if entrynum != -1
            && let Some(i) = (0..55).find(|&i| lb.bank_load[i] == entrynum)
        {
            lb.banks[i] = 1;
            lb.fgm_wanted -= SSM_SIZE[i] as i32;
            lb.fgm_pending = lb.fgm_pending.wrapping_sub(SSM_SIZE[i]);
            lb.fgm_loaded += SSM_SIZE[i] as i32;
        }
        if let Some(slot) = self.lb.next_background_bank() {
            self.lb_load_bank_name(slot);
            let e = self.lb.ssm_entrynum(slot);
            self.lb.bank_load[slot] = self.sfx_load(data, e, 2, Some(BANK_LOADED), 0);
        }
    }
}

/// `ssm_files`.
const SSM_NAMES: [&str; 55] = [
    "main.ssm",
    "pokemon.ssm",
    "nr_title.ssm",
    "nr_select.ssm",
    "nr_1p.ssm",
    "nr_vs.ssm",
    "captain.ssm",
    "clink.ssm",
    "dk.ssm",
    "drmario.ssm",
    "falco.ssm",
    "fox.ssm",
    "gkoopa.ssm",
    "ice.ssm",
    "kirby.ssm",
    "koopa.ssm",
    "link.ssm",
    "luigi.ssm",
    "mario.ssm",
    "mars.ssm",
    "mewtwo.ssm",
    "ness.ssm",
    "peach.ssm",
    "pichu.ssm",
    "pikachu.ssm",
    "purin.ssm",
    "samus.ssm",
    "zs.ssm",
    "yoshi.ssm",
    "gw.ssm",
    "ganon.ssm",
    "emblem.ssm",
    "mhands.ssm",
    "kirbytm.ssm",
    "castle.ssm",
    "corneria.ssm",
    "greatbay.ssm",
    "kongo.ssm",
    "mutecity.ssm",
    "onett.ssm",
    "zebes.ssm",
    "garden.ssm",
    "klaid.ssm",
    "greens.ssm",
    "venom.ssm",
    "bigblue.ssm",
    "fourside.ssm",
    "pupupu.ssm",
    "pstadium.ssm",
    "1padv.ssm",
    "ending.ssm",
    "nr_name.ssm",
    "1pend.ssm",
    "last.ssm",
    "end.ssm",
];

impl LbAudio {
    /// `strcpy(&cur_ssm_file[ssm_stem_pos], ssm_files[slot])`: the bank's file name after the
    /// language's directory, leaving what a longer name left past its end.
    pub fn set_ssm_name(&mut self, slot: usize) {
        let at = self.ssm_stem_pos as usize;
        let name = SSM_NAMES[slot].as_bytes();
        self.ssm_file[at..at + name.len()].copy_from_slice(name);
        self.ssm_file[at + name.len()] = 0;
    }
}

impl crate::audio::Audio {
    fn lb_load_bank_name(&mut self, slot: usize) {
        self.lb.set_ssm_name(slot);
    }
}

/// `s32_arr_803BB5D0`'s third column: the order a full ARAM drops banks the next scene
/// doesn't want in.
const SSM_DROP: [i8; 55] = [
    5, 5, 0, 0, 4, 4, 3, 3, 3, 3, 3, 3, 1, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 4, 1, 5, 1, 2, 5,
];

/// `lbl_803BB3C0`'s banks: each character's (by `CharacterKind`), as a mask of bank slots.
const CHARACTER_BANKS: [u64; 33] = [
    0x40,
    0x100,
    0x800,
    0x20000000,
    0x4000,
    0x8000,
    0x10000,
    0x20000,
    0x40000,
    0x80000,
    0x100000,
    0x200000,
    0x400000,
    0x1000000,
    0x2000,
    0x2000000,
    0x4000000,
    0x10000000,
    0x8000000,
    0x8000000,
    0x400,
    0x80,
    0x200,
    0x80000000,
    0x800000,
    0x40000000,
    0x100000000,
    0,
    0,
    0x1000,
    0x100000000,
    0,
    0x2000,
];

/// `s32_arr_803BB6B0`: each ground kind's bank slot (55 for none) and its music channels'
/// aux B send.
const GROUND_BANK: [u8; 111] = [
    55, 55, 34, 55, 37, 41, 36, 55, 40, 42, 55, 55, 55, 43, 35, 44, 48, 55, 38, 45, 39, 46, 55, 55,
    55, 55, 55, 55, 47, 55, 55, 49, 49, 49, 49, 55, 55, 53, 55, 55, 49, 55, 55, 55, 55, 55, 55, 55,
    55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55,
    55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55,
    55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 0,
];
const GROUND_SEND: [u8; 111] = [
    1, 1, 1, 1, 1, 1, 1, 24, 64, 128, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 72, 136, 1, 1, 56, 56, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0,
];

/// `stage_id_map`'s ground kinds, by stage kind (`Stage_8022519C`).
const STAGE_GROUND: [u8; 286] = [
    0, 1, 12, 16, 2, 4, 8, 14, 10, 20, 18, 3, 5, 6, 7, 9, 11, 13, 21, 24, 25, 26, 15, 17, 19, 22,
    22, 27, 28, 29, 30, 36, 37, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56,
    57, 58, 59, 60, 61, 62, 63, 64, 65, 31, 2, 4, 5, 32, 7, 8, 33, 13, 13, 13, 14, 14, 16, 34, 18,
    20, 22, 22, 36, 36, 37, 37, 39, 38, 67, 66, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
    24, 25, 18, 19, 20, 21, 16, 2, 36, 2, 25, 7, 6, 37, 16, 22, 22, 24, 6, 7, 14, 15, 6, 16, 36,
    25, 4, 7, 6, 24, 6, 19, 8, 10, 11, 13, 12, 13, 22, 14, 18, 16, 16, 24, 20, 21, 19, 36, 36, 36,
    7, 6, 7, 6, 2, 4, 6, 10, 25, 18, 16, 12, 24, 7, 3, 5, 12, 25, 20, 16, 22, 16, 27, 18, 36, 37,
    3, 4, 6, 8, 10, 13, 14, 16, 24, 18, 20, 17, 22, 2, 7, 12, 36, 11, 25, 5, 15, 21, 37, 27, 9, 36,
    7, 2, 10, 20, 12, 16, 8, 6, 11, 22, 18, 3, 68, 36, 14, 5, 4, 37, 8, 2, 25, 9, 11, 21, 69, 15,
    13, 7, 12, 24, 14, 34, 6, 28, 21, 12, 25, 16, 7, 7, 17, 19, 36, 27, 7, 70, 10, 24, 37, 37, 5,
    10, 2, 3, 6, 7, 18, 14, 16, 20, 22, 25, 17, 37, 27, 15, 16, 6, 7, 37, 36, 16, 37, 25, 12, 27,
    2, 37, 6, 36, 16, 7, 36,
];

/// The scene groups' bank masks (`lbAudioAx_80026F2C`'s flags): 1 the common banks, 2 the
/// menus', 4 the fighters', 8 the stages', 16 the single-player modes'.
fn group_mask(flags: u32) -> u64 {
    let mut mask = 0u64;
    for (bit, m) in [
        (1, 0x0048_0000_0000_0003u64),
        (2, 0x3C),
        (4, 0x0080_0003_FFFF_FFC0),
        (8, 0x0023_FFFC_0000_0000),
        (16, 0x0014_0000_0000_0000),
    ] {
        if flags & bit != 0 {
            mask = mask.wrapping_add(m);
        }
    }
    mask
}

/// `lbAudioAx_80026E84`: a character's banks.
#[doc(alias = "lbAudioAx_80026E84")]
pub fn character_banks(ckind: i32) -> u64 {
    usize::try_from(ckind)
        .ok()
        .and_then(|k| CHARACTER_BANKS.get(k).copied())
        .unwrap_or(0)
}

/// `Stage_8022519C`: a stage kind's ground kind.
pub fn ground_kind(stkind: i32) -> usize {
    usize::from(STAGE_GROUND[stkind as usize])
}

/// `lbAudioAx_80026EBC`: a stage's bank.
#[doc(alias = "lbAudioAx_80026EBC")]
pub fn stage_banks(stkind: i32) -> u64 {
    match GROUND_BANK.get(ground_kind(stkind)) {
        Some(&55) | None => 0,
        Some(&shift) => 1 << shift,
    }
}

impl LbAudio {
    /// `lbAudioAx_80026F2C`: the scene groups' banks no longer wanted.
    #[doc(alias = "lbAudioAx_80026F2C")]
    pub fn unwant_banks(&mut self, flags: u32) {
        let mut mask = group_mask(flags);
        for w in self.bank_want.iter_mut().take(55) {
            if mask & 1 != 0 {
                *w = -1;
            }
            mask >>= 1;
        }
    }

    /// `lbAudioAx_8002702C`: the scene groups' banks in `mask` (and the common banks) wanted.
    #[doc(alias = "lbAudioAx_8002702C")]
    pub fn want_banks(&mut self, flags: u32, mask: u64) {
        let mut mask = mask.wrapping_add(0x0048_0000_0000_0003);
        let mut groups = group_mask(flags);
        for w in self.bank_want.iter_mut().take(55) {
            if groups & 1 != 0 && mask & 1 != 0 {
                *w = 1;
            }
            groups >>= 1;
            mask >>= 1;
        }
    }

    /// `fn_800268B4`: the wanted, loaded and still-to-load sizes, from the banks' states.
    #[doc(alias = "fn_800268B4")]
    fn count_banks(&mut self) {
        self.fgm_pending = 0;
        self.fgm_wanted = 0;
        self.fgm_loaded = 0;
        for (i, &size) in SSM_SIZE.iter().enumerate().take(55) {
            match (self.bank_have[i] != -1, self.banks[i] != -1) {
                (true, false) => {
                    self.fgm_wanted += size as i32;
                    self.fgm_pending = self.fgm_pending.wrapping_add(size);
                }
                (false, true) => self.fgm_loaded += size as i32,
                (true, true) => {
                    self.fgm_wanted += size as i32;
                    self.fgm_loaded += size as i32;
                }
                (false, false) => {}
            }
        }
    }
}

impl crate::audio::Audio {
    /// `fn_800269AC`: the background loads of banks not loaded yet are cancelled; if loads
    /// remain, waits for them and empties bank 2.
    #[doc(alias = "fn_800269AC")]
    fn lb_cancel_background_loads(&mut self, data: &melee_data::archive::Archives) {
        if self.sfx_pending_loads() == 0 {
            return;
        }
        for (i, &priority) in SSM_PRIORITY.iter().enumerate().take(55) {
            if priority != 5 && self.lb.bank_load[i] != -1 && self.lb.banks[i] == -1 {
                // The original retries up to 0x40 times with a busy delay between; the cancel
                // finds the load the first time or never.
                let e = self.lb.bank_load[i];
                self.sfx_cancel_load(data, e);
                self.lb.bank_load[i] = -1;
                self.lb.banks[i] = -1;
            }
        }
        if self.sfx_pending_loads() != 0 {
            self.sfx_wait_for_loads(data);
            self.sfx_unload_bank(2);
            for (i, &priority) in SSM_PRIORITY.iter().enumerate().take(55) {
                if priority != 5 {
                    self.lb.banks[i] = -1;
                    self.lb.bank_load[i] = -1;
                }
            }
        }
    }

    /// `fn_800267B0`: while the wanted banks don't fit, drops loaded banks the scene doesn't
    /// want, in their drop order, then packs bank 2.
    #[doc(alias = "fn_800267B0")]
    fn lb_make_room(&mut self, data: &melee_data::archive::Archives) {
        if self.lb.fgm_pending == 0 {
            return;
        }
        for order in 0..5 {
            let mut j = 0;
            while (self.lb.fgm_room as u32)
                < (self.lb.fgm_loaded as u32).wrapping_add(self.lb.fgm_pending)
                && j < 55
            {
                let lb = &self.lb;
                if lb.banks[j] != -1 && i32::from(SSM_DROP[j]) == order && lb.bank_have[j] == -1 {
                    let e = lb.bank_load[j];
                    self.sfx_group_remove(e);
                    let lb = &mut self.lb;
                    lb.bank_load[j] = -1;
                    lb.banks[j] = -1;
                    lb.fgm_loaded -= SSM_SIZE[j] as i32;
                }
                j += 1;
            }
        }
        self.sfx_bank_deflag(data, 2);
        self.sfx_bank_deflag_sync();
    }

    /// `lbAudioAx_80027168`: starts loading the wanted banks in the background, after
    /// cancelling stale loads and making room.
    #[doc(alias = "lbAudioAx_80027168")]
    pub fn lb_start_bank_loads(&mut self, data: &melee_data::archive::Archives) {
        for (i, &priority) in SSM_PRIORITY.iter().enumerate().take(55) {
            if priority != 5 && self.lb.banks[i] == 2 {
                self.lb.banks[i] = 1;
            }
        }
        let count = (0..55)
            .filter(|&i| SSM_PRIORITY[i] != 5 && self.lb.bank_want[i] != -1)
            .count();
        if count == 0 {
            return;
        }
        self.lb_cancel_background_loads(data);
        self.lb.bank_have = self.lb.bank_want;
        self.lb.count_banks();
        self.lb_make_room(data);
        assert!(
            (self.lb.fgm_room as u32)
                >= (self.lb.fgm_loaded as u32).wrapping_add(self.lb.fgm_pending),
            "FGM load size is over"
        );
        if let Some(slot) = self.lb.next_background_bank() {
            self.lb_load_bank_name(slot);
            let e = self.lb.ssm_entrynum(slot);
            self.lb.bank_load[slot] = self.sfx_load(data, e, 2, Some(BANK_LOADED), 0);
        }
    }

    /// `lbAudioAx_80027648`: waits until the wanted banks are in, then marks them loaded.
    #[doc(alias = "lbAudioAx_80027648")]
    #[doc(alias = "fn_80027488")]
    pub fn lb_wait_for_banks(&mut self, data: &melee_data::archive::Archives) {
        loop {
            let lb = &mut self.lb;
            if (0..55).any(|i| lb.bank_have[i] == 1 && lb.banks[i] == -1) {
                assert!(
                    self.synth.banks.load_count != 0,
                    "lbAudioAx_80027648 would wait forever for a bank nothing loads"
                );
                self.sfx_wait_for_loads(data);
                continue;
            }
            for i in 0..55 {
                if lb.bank_have[i] == 1 && lb.banks[i] == 1 {
                    lb.banks[i] = 2;
                }
            }
            return;
        }
    }
}

impl crate::game::state::Running<'_> {
    /// `lbAudioAx_8002785C`: the match's banks: its characters' and its stage's, loaded
    /// before it starts; the music channels' aux B send from the stage.
    #[doc(alias = "lbAudioAx_8002785C")]
    pub fn lb_load_match_banks(&mut self) {
        if self.lb_choose_match_banks() {
            let a = &mut self.game.audio;
            a.lb_start_bank_loads(self.data);
            a.lb_wait_for_banks(self.data);
        }
    }

    /// [`lb_load_match_banks`](Self::lb_load_match_banks)'s choice: the stage's send, and the
    /// banks wanted (lbAudioAx_80026F2C and lbAudioAx_8002702C). Whether any are.
    pub fn lb_choose_match_banks(&mut self) -> bool {
        assert!(
            !self.game.vs_is_singleplayer(),
            "the single-player modes' banks (gm_80169370) aren't ported"
        );
        let mut result = 0u64;
        for i in 0..4 {
            if self.game.player_pkind(i) != crate::game::pl::player::PKIND_NONE {
                result |= character_banks(self.game.player_ckind(i));
            }
        }
        let stkind = crate::game::gr::stage::stkind(self.game);
        if stkind == 0xD9 || stkind == 0xE5 {
            result |= 0x2_0000_4000;
        }
        if stkind == 0x46 || stkind == 0x47 {
            result |= 0xC00;
        }
        self.game.audio.lb.send = i32::from(GROUND_SEND[ground_kind(stkind)]);
        result |= stage_banks(stkind);
        if result != 0 {
            let lb = &mut self.game.audio.lb;
            lb.unwant_banks(0xC);
            lb.want_banks(0xC, result);
        }
        result != 0
    }
}
