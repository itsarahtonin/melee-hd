//! The fighter's sounds (`ft_0877.c`, `ft_0881.c`): the character's sound ids adjusted for its
//! size, metal form and Kirby's copied voices, played on the fighter's own tracks (two per
//! player, the second for the second Ice Climber), whose handles the fighter keeps
//! (`x2144`..`x2160`) to stop them later.

use melee_data::archive::DataRef;

use super::super::handles::FighterId;
use super::super::state::Running;
use crate::lb::audio::{SFX_KEY_OFF, SFX_NONE, bank_range, bank_size_step, sfx_bank};

const POPO: i32 = 0x0A;
const NANA: i32 = 0x0B;
const SAMUS: i32 = 0x0D;
const GAME_WATCH: i32 = 0x18;

/// The fighter's tracks, by the handle slot that keeps their sound: the base of
/// `ft_800881D8`'s (`x2144`), `ft_80088328`'s (`x2148`), `ft_80088478`'s (`x214C`),
/// `ft_80088510`'s (`x2150`), `ft_800885A8`'s (`x2154`), `ft_80088640`'s (`x2158`) and
/// `ftCo_800886D8`'s (`x215C`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub enum Track {
    Voice,
    Voice2,
    Effect,
    Effect2,
    Effect3,
    Effect4,
    Effect5,
}

impl Track {
    fn base(self) -> i32 {
        match self {
            Track::Voice => 0x1E,
            Track::Voice2 => 0x2A,
            Track::Effect => 0x36,
            Track::Effect2 => 0x42,
            Track::Effect3 => 0x4E,
            Track::Effect4 => 0x5A,
            Track::Effect5 => 0x72,
        }
    }
}

/// The sounds that come in sizes (small, normal, big; then metal) among the shared bank's.
const SIZED: [i32; 25] = [
    407, 467, 503, 515, 290051, 509, 485, 491, 479, 473, 437, 455, 461, 449, 443, 425, 431, 419,
    413, 377, 395, 401, 389, 383, 371,
];
const SIZED_NO_METAL: [i32; 13] = [
    359, 368, 365, 362, 344, 353, 356, 350, 347, 338, 341, 335, 332,
];

impl Running<'_> {
    fn track_slot(&mut self, f: FighterId, t: Track) -> &mut i32 {
        let fp = &mut self.game.fighters[f.0];
        match t {
            Track::Voice => &mut fp.x2144,
            Track::Voice2 => &mut fp.x2148,
            Track::Effect => &mut fp.x214C,
            Track::Effect2 => &mut fp.x2150,
            Track::Effect3 => &mut fp.x2154,
            Track::Effect4 => &mut fp.x2158,
            Track::Effect5 => &mut fp.x215C,
        }
    }

    /// The handle of the sound on one of the fighter's tracks (-1 for none).
    pub(crate) fn track_handle(&self, f: FighterId, t: Track) -> i32 {
        let fp = &self.game.fighters[f.0];
        match t {
            Track::Voice => fp.x2144,
            Track::Voice2 => fp.x2148,
            Track::Effect => fp.x214C,
            Track::Effect2 => fp.x2150,
            Track::Effect3 => fp.x2154,
            Track::Effect4 => fp.x2158,
            Track::Effect5 => fp.x215C,
        }
    }

    /// Keys off one of the slot's tracks for both its fighters and forgets the handle.
    pub(crate) fn fighter_key_off_tracks(&mut self, f: FighterId, t: Track) {
        let base = t.base() + i32::from(self.game.fighters[f.0].player_idx) * 2;
        self.sfx_play_track(SFX_KEY_OFF, 0, 0x40, base);
        self.sfx_play_track(SFX_KEY_OFF, 0, 0x40, base + 1);
        *self.track_slot(f, t) = -1;
    }

    fn track(&self, f: FighterId, t: Track) -> i32 {
        let fp = &self.game.fighters[f.0];
        t.base() + i32::from(fp.player_idx) * 2 + i32::from(fp.is_sub_fighter)
    }

    /// A sized sound for the fighter's size, `ft_80087C70`: one up when small, two when big
    /// (as the size modifier and its pending change give it).
    #[doc(alias = "ft_80087C70")]
    pub fn sized_sfx(&self, f: FighterId, id: i32) -> i32 {
        let fp = &self.game.fighters[f.0];
        let (b5, b6) = (fp.x2220_b5, fp.x2220_b6);
        match fp.x2228_b3 {
            0 if b5 => id + 1,
            0 if b6 => id + 2,
            1 if !b5 => id + 2,
            2 if b5 || !b6 => id + 1,
            _ => id,
        }
    }

    /// The sound id the fighter plays for `id`, `ft_80087D0C`: Kirby's copied voices, sized
    /// and metal sounds, the Ice Climbers' alternate costumes' voices.
    #[doc(alias = "ft_80087D0C")]
    pub fn fighter_sfx(&self, f: FighterId, id: i32) -> i32 {
        let mut sfx = self.game.audio.lb.kirby_voice(id);
        let bank = sfx_bank(sfx);
        match bank {
            0 => {
                if SIZED.contains(&sfx) {
                    let mut s = self.sized_sfx(f, sfx);
                    if self.game.fighters[f.0].is_metal {
                        s += 3;
                    }
                    sfx = s;
                } else if SIZED_NO_METAL.contains(&sfx) {
                    sfx = self.sized_sfx(f, sfx);
                }
            }
            6..=31 | 33 => {
                if bank == 13 {
                    let fp = &self.game.fighters[f.0];
                    if fp.kind == POPO || fp.kind == NANA {
                        let alt = fp.costume_id >= 2;
                        if (0x1FBFD..=0x1FC62).contains(&sfx) && alt {
                            sfx += 0x66;
                        }
                        if (0x1FC63..=0x1FCC8).contains(&sfx) && !alt {
                            sfx -= 0x66;
                        }
                    }
                }
                if let Some((lo, _)) = bank_range(bank)
                    && sfx >= lo + bank_size_step(bank)
                {
                    sfx = self.sized_sfx(f, sfx);
                }
            }
            _ => {}
        }
        sfx
    }

    /// Plays one of the fighter's sounds, `ft_PlaySFX`, keeping its handle (`x2160`). The
    /// footsteps and other shared sounds 332 to 370 get a random pitch bend.
    #[doc(alias = "ft_PlaySFX")]
    pub fn fighter_play_sfx(&mut self, f: FighterId, id: i32, vol: u8, pan: u8) {
        let id = self.fighter_sfx(f, id);
        let handle = self.sfx_play(id, vol.into(), pan.into());
        self.game.fighters[f.0].x2160 = handle;
        let bend = if (332..=370).contains(&id) {
            self.game.objs.rng.below(200) - 100
        } else {
            0
        };
        self.sfx_pitch(handle, bend);
    }

    /// Stops the sound on one of the fighter's tracks and forgets its handle: `ft_80088770`,
    /// `ft_800887CC`, `ft_80088828`, `ft_80088884`, `ft_800888E0`, `ft_8008893C`,
    /// `ft_80088998`.
    #[doc(alias = "ft_80088770")]
    #[doc(alias = "ft_800887CC")]
    #[doc(alias = "ft_80088828")]
    #[doc(alias = "ft_80088884")]
    #[doc(alias = "ft_800888E0")]
    #[doc(alias = "ft_8008893C")]
    #[doc(alias = "ft_80088998")]
    pub fn fighter_stop_track(&mut self, f: FighterId, t: Track) {
        let track = self.track(f, t);
        self.sfx_play_track(SFX_KEY_OFF, 0, 0x40, track);
        *self.track_slot(f, t) = -1;
    }

    /// Plays a sound on one of the fighter's tracks: `ft_800881D8` (`Voice`), `ft_80088328`
    /// (`Voice2`), `ft_80088478`, `ft_80088510`, `ft_800885A8`, `ft_80088640`, `ftCo_800886D8`
    /// (the effect tracks). The silent id does nothing and the key-off id stops the track.
    /// A voice cuts off both voice tracks first, and a fighter whose voice is muted
    /// (`x2225_b6`) speaks only as Mr. Game & Watch or Samus.
    #[doc(alias = "ft_800881D8")]
    #[doc(alias = "ft_80088328")]
    #[doc(alias = "ft_80088478")]
    #[doc(alias = "ft_80088510")]
    #[doc(alias = "ft_800885A8")]
    #[doc(alias = "ft_80088640")]
    #[doc(alias = "ftCo_800886D8")]
    pub fn fighter_play_track(&mut self, f: FighterId, t: Track, id: i32, vol: u8, pan: u8) {
        let voice = matches!(t, Track::Voice | Track::Voice2);
        if voice {
            let fp = &self.game.fighters[f.0];
            if fp.x2225_b6 && fp.kind != GAME_WATCH && fp.kind != SAMUS {
                return;
            }
        }
        if id == SFX_NONE {
            return;
        }
        if id == SFX_KEY_OFF {
            self.fighter_stop_track(f, t);
            return;
        }
        let id = self.fighter_sfx(f, id);
        if voice {
            self.fighter_stop_track(f, Track::Voice);
            self.fighter_stop_track(f, Track::Voice2);
        }
        let track = self.track(f, t);
        let handle = self.sfx_play_track(id, vol.into(), pan.into(), track);
        *self.track_slot(f, t) = handle;
    }

    /// One of a set of sounds at random on the voice track, `ft_800889F4`.
    #[doc(alias = "ft_800889F4")]
    pub fn fighter_random_voice(&mut self, f: FighterId, ids: &[i32]) {
        let i = self.game.objs.rng.below(ids.len() as i32);
        self.fighter_play_track(f, Track::Voice, ids[i as usize], 0x7F, 0x40);
    }

    /// [`Running::fighter_random_voice`] over `n` ids (words) in the loaded data at `ids`.
    pub fn fighter_random_voice_in(&mut self, f: FighterId, ids: DataRef, n: i32) {
        let n = n.max(0);
        let i = self.game.objs.rng.below(n);
        assert!((0..n).contains(&i), "a voice set's index past its ids");
        let id = self.data.u32(ids.at(4 * i as u32)) as i32;
        self.fighter_play_track(f, Track::Voice, id, 0x7F, 0x40);
    }
}
