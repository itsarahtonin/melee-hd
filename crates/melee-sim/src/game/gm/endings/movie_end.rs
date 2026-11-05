//! The ending movie (`gmmovieend.c`'s scene, 0x1D): after the staff roll, the character's
//! ending movie (`MvEnd*.mth`) in a 448 by 336 sprite at (96, 72), with its song; it ends
//! after 420 of the movie's frames (counting on by itself once the movie has ended), on
//! Start from the human's port 30 ticks in, or with a soft reset while the movie plays.

use super::super::super::handles::CodeRef;
use super::super::super::state::{Obj, Running};
use crate::hsd::sobj::{self, OBJ_SOBJ};

/// `gm_Scene_MovieEnd_OnEnter`, `gm_Scene_MovieEnd_OnFrame`.
pub const ON_ENTER: CodeRef = CodeRef(0x801A_637C);
pub const ON_FRAME: CodeRef = CodeRef(0x801A_64A8);

/// `gm_803DB1F4`: each character's ending movie (Zelda's for Sheik too).
pub const MOVIES: [&str; 0x1A] = [
    "MvEndCaptain.mth",
    "MvEndDonkey.mth",
    "MvEndFox.mth",
    "MvEndGamewatch.mth",
    "MvEndKirby.mth",
    "MvEndKoopa.mth",
    "MvEndLink.mth",
    "MvEndLuigi.mth",
    "MvEndMario.mth",
    "MvEndMarth.mth",
    "MvEndMewtwo.mth",
    "MvEndNess.mth",
    "MvEndPeach.mth",
    "MvEndPikachu.mth",
    "MvEndPoponana.mth",
    "MvEndPurin.mth",
    "MvEndSamus.mth",
    "MvEndYoshi.mth",
    "MvEndZelda.mth",
    "MvEndZelda.mth",
    "MvEndFalco.mth",
    "MvEndClink.mth",
    "MvEndDrmario.mth",
    "MvEndRoy.mth",
    "MvEndPichu.mth",
    "MvEndGanon.mth",
];

/// `gm_803DB25C`: each character's song for its movie, all the same.
pub const SONG: i32 = 8;

/// The ending movie's statics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct MovieEnd {
    /// `gm_804D6738`: the movie's frames.
    pub frames: u32,
    /// `gm_804D673C`: ticks before Start counts.
    pub wait: u8,
}

impl Running<'_> {
    /// `gm_Scene_MovieEnd_OnEnter`: the sprites' camera, the movie's sprite (448 by 336 at
    /// (96, 72)), the song, and the character's movie opened with the sound paused, no rate
    /// table and no loop.
    #[doc(alias = "gm_Scene_MovieEnd_OnEnter")]
    pub fn movie_end_on_enter(&mut self) {
        self.game.endings.movie_end = MovieEnd {
            frames: 0,
            wait: 0x1E,
        };
        let cam = self.game.world.create(0x13, 0x14, 0);
        self.sprite_camera(cam, 0x280, 0x1E0, 8, false);
        self.game.world.gobj_mut(cam).gxlink_prios = 0x800;
        let g = self.game.world.create(0xE, 0xF, 0);
        self.game.world.set_obj(g, OBJ_SOBJ, Obj::SObj(None));
        self.game
            .world
            .setup_gx_link(g, CodeRef(crate::lb::mthp::RENDER), 0xB, 0);
        // `lbMthp_8001F624`: a 448 by 336 sprite drawing the planes.
        let (s, head) = sobj::create(
            &mut self.game.objs.sobjs,
            self.data,
            g,
            None,
            sobj::Image::Movie {
                width: 0x1C0,
                height: 0x150,
            },
            (0, 0),
            0x80,
            None,
        );
        self.set_sobj_head(g, head);
        let so = &mut self.game.objs.sobjs[s.0];
        so.flags |= 0x10;
        so.x = 96.0;
        so.y = 72.0;
        self.lb_play_music(SONG);
        self.lb_audio_pause(true);
        let name = MOVIES[usize::from(self.game.endings.gover.ckind())];
        let (entrynum, _) = self.game.ft.data.files.entry(name);
        let file = self.game.audio.devcom.file(entrynum);
        let rate = crate::lb::mthp::Rate::default();
        self.game
            .audio
            .mthp_open(self.data, entrynum, file, rate, 0);
        self.lb_audio_pause(false);
        self.game.opening.perf_display = true;
    }

    /// `gm_Scene_MovieEnd_OnFrame`: the movie's frame taken and counted (on by one a tick once
    /// it has ended); after the first 30 ticks, the end at 420 frames, on the human's Start or,
    /// while the movie plays, on a soft reset (`gm_801A4B74`, to the title), the music and
    /// sounds stopped.
    #[doc(alias = "gm_Scene_MovieEnd_OnFrame")]
    pub fn movie_end_on_frame(&mut self) {
        self.game.audio.movie.take_frame();
        let t = self.game.audio.movie.game_clock();
        let ended = self.game.audio.movie.has_ended();
        let m = &mut self.game.endings.movie_end;
        if ended {
            m.frames = m.frames.wrapping_add(1);
        } else {
            m.frames = t;
        }
        if m.wait != 0 {
            m.wait -= 1;
            return;
        }
        let reset = self.game.main_lib.xc && !ended;
        let port = self.game.endings.gover.port();
        let start = self.game.buttons_triggered(port) as u32
            & u32::from(crate::input::pad::BUTTON_START)
            != 0;
        if self.game.endings.movie_end.frames >= 0x1A4 || start || reset {
            self.lb_audio_music_stop();
            self.sfx_stop_all();
            if reset {
                self.game.end_scene_now();
                return;
            }
            self.game.end_scene();
        }
    }

    /// The files the ending movie reads, for the frontend: the character's movie and its song.
    pub fn movie_end_scene_files(&self) -> super::super::super::vi::SceneFiles {
        let mut f = super::super::super::vi::SceneFiles::default();
        let name = MOVIES[usize::from(self.game.endings.gover.ckind())];
        f.movies.push(name.into());
        f.songs.push(SONG as usize);
        f
    }

    /// `gm_801BEF84`, the game over modes' ending movie state's exit: the movie player let
    /// go (`lbMthp_8001F800`).
    #[doc(alias = "gm_801BEF84")]
    pub fn movie_end_exit(&mut self) {
        self.mthp_release();
    }
}
