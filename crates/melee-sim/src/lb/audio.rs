//! The game's audio library (`melee/lb/lbaudio_ax.c`) as far as game code plays and stops
//! sounds through it: volumes and pans in the game's 0-127 range doubled for the driver, the
//! fighters' and stages' tracks, and Kirby's copied voices.

use crate::game::state::{Event, Running};

/// The id that plays nothing, and the one that keys a track off instead of playing.
pub const SFX_NONE: i32 = 0x83D60;
pub const SFX_KEY_OFF: i32 = 0x83D61;

/// The game's audio library's own state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LbAudio {
    /// `lbl_80433984`: each sound bank's load state (2: loaded).
    pub banks: [i32; 56],
}

impl Default for LbAudio {
    fn default() -> Self {
        LbAudio { banks: [0; 56] }
    }
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
