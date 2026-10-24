//! The attract loop's two other movies: How to Play (`melee/gm/gmhowto.c`, `MvHowto.mth`)
//! and the Special Movie (`gmomake15.c`, `MvOmake15.mth`). Each scene plays its movie through
//! the movie player's sprite with its song; Start or A ends it for the title, and its end goes
//! on with the attract loop (How to Play's back to the opening movie but every sixth time, when
//! the Special Movie comes next).

use super::super::handles::CodeRef;
use super::super::state::{Obj, Running};
use super::machine::GM_TITLE;
use crate::hsd::gobj::GObjId;
use crate::hsd::sobj::{self, OBJ_SOBJ};

/// `gm_Scene_HowTo_OnEnter`, `gm_Scene_HowTo_OnFrame`, `gm_Scene_Omake15_OnEnter` and
/// `gm_Scene_Omake15_OnFrame`.
pub const HOWTO_ON_ENTER: CodeRef = CodeRef(0x801A_CCA0);
pub const HOWTO_ON_FRAME: CodeRef = CodeRef(0x801A_CD8C);
pub const OMAKE_ON_ENTER: CodeRef = CodeRef(0x801A_CE94);
pub const OMAKE_ON_FRAME: CodeRef = CodeRef(0x801A_CF8C);

/// The movies' files.
pub const HOWTO_MOVIE: &str = "MvHowto.mth";
pub const OMAKE_MOVIE: &str = "MvOmake15.mth";

/// `gm_803DD2C0`, How to Play's rate table (`gm_801ACC94` hands it out): runs of frames, each
/// shown for so many 60ths of a second.
pub const HOWTO_RATE_TABLE: u32 = 0x803D_D2C0;
const HOWTO_RATE_RUNS: [(u32, u32); 19] = [
    (1, 19),
    (856, 1),
    (1, 85),
    (279, 1),
    (1, 59),
    (17, 1),
    (1, 59),
    (19, 1),
    (1, 59),
    (35, 1),
    (1, 27),
    (37, 1),
    (1, 59),
    (43, 1),
    (1, 59),
    (39, 1),
    (1, 59),
    (499, 1),
    (1, 67),
];
const HOWTO_RATE_TAIL: [(u32, u32); 18] = [
    (892, 1),
    (240, 2),
    (63, 1),
    (1, 77),
    (67, 1),
    (1, 67),
    (23, 1),
    (1, 71),
    (51, 1),
    (1, 59),
    (25, 1),
    (1, 87),
    (53, 1),
    (1, 95),
    (59, 1),
    (1, 101),
    (117, 1),
    (65536, 1),
];

/// How to Play's rate table (`gm_801ACC94`).
#[doc(alias = "gm_801ACC94")]
pub fn howto_rate() -> crate::lb::mthp::Rate {
    let mut runs = HOWTO_RATE_RUNS.to_vec();
    runs.extend(HOWTO_RATE_TAIL);
    crate::lb::mthp::Rate {
        addr: HOWTO_RATE_TABLE,
        runs,
    }
}

/// The movies' sprites' GObjs (`gm_804D6850`, `gm_804D6858`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Movies {
    pub howto: Option<GObjId>,
    pub omake: Option<GObjId>,
}

impl Running<'_> {
    /// A movie scene's setup: the sprites' camera (`gm_801A9DD0` over the screen, links 11),
    /// the movie's sprite GObj on render link 11 (`lbMthp_8001F67C`) with a `w` by `h`
    /// sprite (`lbMthp_8001F624`), the song, and the movie opened with the music paused
    /// around it (`lbAudioAx_80024E50`).
    #[allow(clippy::too_many_arguments)]
    fn movie_scene(
        &mut self,
        movie: &str,
        rate: crate::lb::mthp::Rate,
        song: i32,
        w: u16,
        h: u16,
        place: Option<(f32, f32)>,
    ) -> GObjId {
        let cam = self.game.world.create(0x13, 0x14, 0);
        self.sprite_camera(cam, 0x280, 0x1E0, 8, false);
        self.game.world.gobj_mut(cam).gxlink_prios = 1 << 0xB;
        let g = self.game.world.create(0xE, 0xF, 0);
        self.game.world.set_obj(g, OBJ_SOBJ, Obj::SObj(None));
        self.game
            .world
            .setup_gx_link(g, CodeRef(crate::lb::mthp::RENDER), 0xB, 0);
        let (s, head) = sobj::create(
            &mut self.game.objs.sobjs,
            self.data,
            g,
            None,
            sobj::Image::Movie {
                width: w,
                height: h,
            },
            (0, 0),
            0x80,
            None,
        );
        self.set_sobj_head(g, head);
        let so = &mut self.game.objs.sobjs[s.0];
        so.flags |= 0x10;
        if let Some((x, y)) = place {
            so.x = x;
            so.y = y;
        }
        self.lb_play_music(song);
        self.lb_audio_pause(true);
        let (entrynum, _) = self.game.ft.data.files.entry(movie);
        let file = self.game.audio.devcom.file(entrynum);
        self.game
            .audio
            .mthp_open(self.data, entrynum, file, rate, 0);
        self.lb_audio_pause(false);
        self.game.opening.perf_display = true;
        g
    }

    /// `gm_Scene_HowTo_OnEnter`: How to Play's movie, full screen, with song 0x24.
    // Ports `gm_Scene_HowTo_OnEnter`, and the scenes' other functions below theirs; aliased
    // once a session reaches them (the attract loop's first match's exit stops before).
    pub fn howto_on_enter(&mut self) {
        let rate = howto_rate();
        let g = self.movie_scene(HOWTO_MOVIE, rate, 0x24, 0x280, 0x1E0, None);
        self.game.movies.howto = Some(g);
    }

    /// `gm_Scene_Omake15_OnEnter`: the Special Movie, 448 by 336 at (96, 72), with song 0x52
    /// and no rate table (a frame a 60th).
    pub fn omake_on_enter(&mut self) {
        let rate = crate::lb::mthp::Rate::default();
        let g = self.movie_scene(OMAKE_MOVIE, rate, 0x52, 0x1C0, 0x150, Some((96.0, 72.0)));
        self.game.movies.omake = Some(g);
    }

    /// The movie's end, or Start or A, or a disc error's reset (`gmMainLib_8046B0F0.xC`)
    /// while it plays: whether the scene ends now, and whether it ends at once and by a
    /// button.
    fn movie_scene_end(&mut self) -> Option<(bool, bool)> {
        self.game.audio.movie.take_frame();
        let _ = self.game.audio.movie.game_clock();
        let ended = self.game.audio.movie.has_ended();
        let reset = self.game.main_lib.xc && !ended;
        let pressed = self.game.buttons_triggered(4)
            & u64::from(crate::input::pad::BUTTON_START | crate::input::pad::BUTTON_A)
            != 0;
        (ended || pressed || reset).then_some((reset, pressed))
    }

    /// `gm_Scene_HowTo_OnFrame`: at the movie's end, or Start or A, the music and sounds
    /// stop and the scene ends: for the title on a button, else back to the opening movie
    /// (the attract loop's count one more) but every sixth time, when the Special Movie
    /// comes next.
    pub fn howto_on_frame(&mut self) {
        let Some((reset, pressed)) = self.movie_scene_end() else {
            return;
        };
        self.lb_audio_music_stop();
        self.sfx_stop_all();
        if reset {
            self.game.end_scene_now();
        } else {
            self.game.end_scene();
        }
        if pressed {
            self.lb_audio_call(1);
            self.set_pending_game_mode(GM_TITLE);
            self.set_new_game_mode_pending();
        } else if self.game.game_data.unk_1 != 5 {
            self.game.game_data.unk_1 = self.game.game_data.unk_1.wrapping_add(1);
            self.game.machine.routing.next_state_id = 1;
        }
    }

    /// `gm_Scene_Omake15_OnFrame`: at the movie's end, or Start or A, the music stops, the
    /// attract loop's count starts over (`gmMainLib_8015DB0C`), and the scene ends for the
    /// title (with the forward sound on a button).
    pub fn omake_on_frame(&mut self) {
        let Some((reset, pressed)) = self.movie_scene_end() else {
            return;
        };
        self.lb_audio_music_stop();
        if reset {
            self.game.end_scene_now();
        } else {
            self.game.end_scene();
        }
        self.game.game_data.unk_1 = 0;
        if pressed || reset {
            if !reset {
                self.lb_audio_call(1);
            }
            self.set_pending_game_mode(GM_TITLE);
            self.set_new_game_mode_pending();
        }
    }
}
