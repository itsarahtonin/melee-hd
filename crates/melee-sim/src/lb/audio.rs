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
    /// `lbl_80433984`: each sound bank's load state (2: loaded).
    pub banks: [i32; 56],
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
}

impl Default for LbAudio {
    /// As the game's data initializes it.
    fn default() -> Self {
        LbAudio {
            banks: [0; 56],
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
