//! The stage's queries, `melee/gr/stage.c`: blast zones and camera limits in world space, the
//! camera's angles, the fixed camera, and the match's stage kind.

use gekko_fp as fp;

use super::super::state::Game;
use super::super::structs::StageInfo;
use crate::fpu::{mem, reg};
use crate::vec::Vec3;

/// Degrees to radians as the game rounds it (`0.0174532923847F`).
const DEG_TO_RAD: f64 = 0.017_453_292_384_743_69;

fn add(a: f32, b: f32) -> f32 {
    mem(fp::fadds(reg(a), reg(b)))
}

fn radians(deg: f32) -> f32 {
    mem(fp::fmuls(DEG_TO_RAD, reg(deg)))
}

/// The camera's deepest zoom, `Stage_GetCamMaxDepth`.
#[doc(alias = "Stage_GetCamMaxDepth")]
pub fn cam_max_depth(game: &Game) -> f32 {
    game.stage_info.cam_info.cam_max_depth
}

/// The camera's zoom rate, `Stage_GetCamZoomRate`.
#[doc(alias = "Stage_GetCamZoomRate")]
pub fn cam_zoom_rate(game: &Game) -> f32 {
    game.stage_info.cam_info.cam_zoom_rate
}

/// The camera's `x20`, `Stage_GetCamInfoX20`.
#[doc(alias = "Stage_GetCamInfoX20")]
pub fn cam_x20(game: &Game) -> f32 {
    game.stage_info.cam_info.x20
}

/// The camera's `x24`, `Stage_GetCamInfoX24`.
#[doc(alias = "Stage_GetCamInfoX24")]
pub fn cam_x24(game: &Game) -> f32 {
    game.stage_info.cam_info.x24
}

/// The camera's fixed zoom, `Stage_GetCamFixedZoom`.
#[doc(alias = "Stage_GetCamFixedZoom")]
pub fn cam_fixed_zoom(game: &Game) -> f32 {
    game.stage_info.cam_info.cam_fixed_zoom
}

/// The camera's tracking ratio, `Stage_GetCamTrackRatio`.
#[doc(alias = "Stage_GetCamTrackRatio")]
pub fn cam_track_ratio(game: &Game) -> f32 {
    game.stage_info.cam_info.cam_track_ratio
}

/// The camera's tracking smoothness, `Stage_GetCamTrackSmooth`.
#[doc(alias = "Stage_GetCamTrackSmooth")]
pub fn cam_track_smooth(game: &Game) -> f32 {
    game.stage_info.cam_info.cam_track_smooth
}

/// The pause camera's nearest, starting and farthest depths (`Stage_GetPauseCamZPosMin`,
/// `Stage_GetPauseCamZPosInit`, `Stage_GetPauseCamZPosMax`).
#[doc(alias = "Stage_GetPauseCamZPosMin")]
pub fn pause_cam_z_min(game: &Game) -> f32 {
    game.stage_info.cam_info.pausecam_zpos_min
}

#[doc(alias = "Stage_GetPauseCamZPosInit")]
pub fn pause_cam_z_init(game: &Game) -> f32 {
    game.stage_info.cam_info.pausecam_zpos_init
}

#[doc(alias = "Stage_GetPauseCamZPosMax")]
pub fn pause_cam_z_max(game: &Game) -> f32 {
    game.stage_info.cam_info.pausecam_zpos_max
}

/// Where the fixed camera's view meets the stage plane, `Stage_80224CAC`: its eye turned by
/// its angles toward z = 0.
#[doc(alias = "Stage_80224CAC")]
pub fn fixed_cam_interest(s: &StageInfo) -> Vec3 {
    let ci = &s.cam_info;
    let eye = ci.fixed_cam_pos;
    let mut dir = Vec3::new(0.0, 0.0, -100.0);
    let angles = Vec3::new(
        radians(ci.fixed_cam_vert_angle),
        radians(ci.fixed_cam_horz_angle),
        0.0,
    );
    dir.rotate_euler(angles);
    let t = fp::fdivs(reg(eye.z), fp::fneg(reg(dir.z)));
    Vec3::new(
        mem(fp::fmadds(reg(dir.x), t, reg(eye.x))),
        mem(fp::fmadds(reg(dir.y), t, reg(eye.y))),
        0.0,
    )
}

/// The fixed camera's eye, `Stage_SetVecToFixedCamPos`.
#[doc(alias = "Stage_SetVecToFixedCamPos")]
pub fn fixed_cam_pos(game: &Game) -> Vec3 {
    game.stage_info.cam_info.fixed_cam_pos
}

/// The fixed camera's field of view, `Stage_GetCamFixedFov`.
#[doc(alias = "Stage_GetCamFixedFov")]
pub fn fixed_cam_fov(game: &Game) -> f32 {
    game.stage_info.cam_info.fixed_cam_fov
}

/// Runs the match's stage's demo setup, `Stage_8022532C` (the stage kind it's handed only
/// travels with the pair).
#[doc(alias = "Stage_8022532C")]
pub fn demo_init(r: &mut super::super::state::Running, _stkind: i32, arg: i32) {
    let grkind = crate::lb::audio::ground_kind(r.game.gr.stkind) as i32;
    super::ground::demo_init(r, grkind, arg);
}

/// The camera limits' left edge in world space, `Stage_GetCamBoundsLeftOffset`.
#[doc(alias = "Stage_GetCamBoundsLeftOffset")]
pub fn cam_left(game: &Game) -> f32 {
    cam_bounds_of(&game.stage_info)[0]
}

/// `Stage_GetCamBoundsRightOffset`.
#[doc(alias = "Stage_GetCamBoundsRightOffset")]
pub fn cam_right(game: &Game) -> f32 {
    cam_bounds_of(&game.stage_info)[1]
}

/// `Stage_GetCamBoundsTopOffset`.
#[doc(alias = "Stage_GetCamBoundsTopOffset")]
pub fn cam_top(game: &Game) -> f32 {
    cam_bounds_of(&game.stage_info)[2]
}

/// `Stage_GetCamBoundsBottomOffset`.
#[doc(alias = "Stage_GetCamBoundsBottomOffset")]
pub fn cam_bottom(game: &Game) -> f32 {
    cam_bounds_of(&game.stage_info)[3]
}

/// `Stage_GetCamPanAngleRadians`.
#[doc(alias = "Stage_GetCamPanAngleRadians")]
pub fn cam_pan(game: &Game) -> f32 {
    cam_pan_of(&game.stage_info)
}

/// The blast zones in world space (`Stage_GetBlastZoneLeftOffset` and its siblings): left,
/// right, top, bottom.
#[doc(alias = "Stage_GetBlastZoneLeftOffset")]
pub fn blast_left(game: &Game) -> f32 {
    blast_zones_of(&game.stage_info)[0]
}

#[doc(alias = "Stage_GetBlastZoneRightOffset")]
pub fn blast_right(game: &Game) -> f32 {
    blast_zones_of(&game.stage_info)[1]
}

/// The top blast zone (`stGetPlyDeadUp` in the game's asserts).
#[doc(alias = "Stage_GetBlastZoneTopOffset")]
pub fn blast_top(game: &Game) -> f32 {
    blast_zones_of(&game.stage_info)[2]
}

#[doc(alias = "Stage_GetBlastZoneBottomOffset")]
pub fn blast_bottom(game: &Game) -> f32 {
    blast_zones_of(&game.stage_info)[3]
}

/// Halfway between the bottom camera limit and the bottom blast zone, `Stage_CalcUnkCamY`.
#[doc(alias = "Stage_CalcUnkCamY")]
pub fn below_camera(game: &Game) -> f32 {
    let i = &game.stage_info;
    let y = reg(i.cam_info.cam_y_offset);
    let sum = fp::fadds(
        fp::fadds(reg(i.cam_info.cam_bounds.bottom), y),
        fp::fadds(reg(i.blast_zone.bottom), y),
    );
    mem(fp::fmuls(0.5, sum))
}

/// Halfway between the bottom camera limit and [`below_camera`], `Stage_CalcUnkCamYBounds`.
#[doc(alias = "Stage_CalcUnkCamYBounds")]
pub fn below_camera_bounds(game: &Game) -> f32 {
    let i = &game.stage_info;
    let cam = fp::fadds(
        reg(i.cam_info.cam_bounds.bottom),
        reg(i.cam_info.cam_y_offset),
    );
    let half = fp::fmuls(
        0.5,
        fp::fadds(
            fp::fadds(
                reg(i.cam_info.cam_bounds.bottom),
                reg(i.cam_info.cam_y_offset),
            ),
            fp::fadds(reg(i.blast_zone.bottom), reg(i.cam_info.cam_y_offset)),
        ),
    );
    mem(fp::fmuls(0.5, fp::fadds(cam, half)))
}

/// The camera limits' center, `Stage_UnkSetVec3TCam_Offset`.
#[doc(alias = "Stage_UnkSetVec3TCam_Offset")]
pub fn cam_center(game: &Game) -> Vec3 {
    cam_center_of(&game.stage_info)
}

/// The camera's angle limits in radians (`Stage_GetCamAngleRadiansUp` and its siblings): up,
/// down, left, right.
#[doc(alias = "Stage_GetCamAngleRadiansUp")]
#[doc(alias = "Stage_GetCamAngleRadiansDown")]
#[doc(alias = "Stage_GetCamAngleRadiansLeft")]
#[doc(alias = "Stage_GetCamAngleRadiansRight")]
pub fn cam_angles(game: &Game) -> [f32; 4] {
    cam_angles_of(&game.stage_info)
}

/// Whether a stage kind is one of the five `Stage_80224DC8` singles out.
#[doc(alias = "Stage_80224DC8")]
pub fn is_special_target_stage(stkind: i32) -> bool {
    matches!(stkind, 0x3B | 0x3F | 0x42 | 0x49 | 0x4C)
}

/// A player's entry point (`Stage_80224E38`): point of interest `slot + 4`, if the stage has it
/// (the original leaves its vector alone otherwise).
#[doc(alias = "Stage_80224E38")]
pub fn entry_point(
    game: &mut Game,
    data: &melee_data::archive::Archives,
    slot: i32,
) -> Option<Vec3> {
    super::ground::point(game, data, slot + 4)
}

/// The match's stage kind, `Stage_80225194`.
#[doc(alias = "Stage_80225194")]
pub fn stkind(game: &Game) -> i32 {
    game.gr.stkind
}

// The same queries on `stage_info` itself, for code that holds other parts of the game.

/// [`cam_left`] and its siblings on `stage_info`: left, right, top, bottom.
pub fn cam_bounds_of(i: &StageInfo) -> [f32; 4] {
    let c = &i.cam_info;
    [
        add(c.cam_bounds.left, c.cam_x_offset),
        add(c.cam_bounds.right, c.cam_x_offset),
        add(c.cam_bounds.top, c.cam_y_offset),
        add(c.cam_bounds.bottom, c.cam_y_offset),
    ]
}

/// [`blast_left`] and its siblings on `stage_info`: left, right, top, bottom.
pub fn blast_zones_of(i: &StageInfo) -> [f32; 4] {
    let c = &i.cam_info;
    [
        add(i.blast_zone.left, c.cam_x_offset),
        add(i.blast_zone.right, c.cam_x_offset),
        add(i.blast_zone.top, c.cam_y_offset),
        add(i.blast_zone.bottom, c.cam_y_offset),
    ]
}

/// [`cam_pan`] on `stage_info`.
pub fn cam_pan_of(i: &StageInfo) -> f32 {
    radians(i.cam_info.cam_pan_degrees)
}

/// [`cam_center`] on `stage_info`.
pub fn cam_center_of(i: &StageInfo) -> Vec3 {
    let c = &i.cam_info;
    Vec3::new(c.cam_x_offset, c.cam_y_offset, 0.0)
}

/// [`cam_angles`] on `stage_info`: up, down, left, right.
pub fn cam_angles_of(i: &StageInfo) -> [f32; 4] {
    let c = &i.cam_info;
    [
        radians(c.cam_angle_up),
        radians(c.cam_angle_down),
        radians(c.cam_angle_left),
        radians(c.cam_angle_right),
    ]
}

/// A random spot items may appear at, `Stage_80224FDC`: up to 21 tries among the stage's item
/// points (127 on), then up to four among its first points, each drawing from fewer.
#[doc(alias = "Stage_80224FDC")]
pub fn random_item_point(game: &mut Game, data: &melee_data::archive::Archives) -> Option<Vec3> {
    let mut n = 21;
    while n != 0 {
        let id = game.objs.rng.below(n).wrapping_add(127);
        if let Some(p) = super::ground::point(game, data, id) {
            return Some(p);
        }
        n -= 1;
    }
    let mut n = 4;
    while n != 0 {
        let id = game.objs.rng.below(n);
        if let Some(p) = super::ground::point(game, data, id) {
            return Some(p);
        }
        n = id;
    }
    None
}

impl super::super::state::Running<'_> {
    /// Chooses and starts the stage's music, `Stage_80225074`: a sudden death's in a
    /// sudden death match, Super Sudden Death's in that mode, else by `which` (0 the stage's
    /// own choice, 3 its usual song, 2 its alternate, 1 its single player's); the song
    /// starts, and the stage keeps which it is and whether it's the alternate. Returns
    /// `which`.
    #[doc(alias = "Stage_80225074")]
    pub fn start_stage_music(&mut self, which: i32) -> i32 {
        let flags = if self.game.vs_x6() != 0 {
            if self.game.stage_info.unk8C.b0 || which == 2 {
                0x12
            } else {
                0x11
            }
        } else if super::super::gm::is_super_sudden_death(self.game) {
            match which {
                2 => 0x22,
                1 => 0x64,
                _ => 0x24,
            }
        } else {
            match which {
                0 => 4,
                3 => 1,
                2 => 2,
                1 => 0x44,
                _ => panic!("the original's assertion failed: stage.c:526"),
            }
        };
        let stkind = self.game.gr.stkind;
        let (bgm, alternate) = super::setup::music(self.game, self.data, stkind, flags);
        self.lb_play_music(bgm);
        self.game.stage_info.x98 = bgm;
        self.game.stage_info.unk8C.b1 = alternate;
        which
    }
}

/// `Gr_Kind_Corneria` and `Gr_Kind_Venom`.
const GR_KIND_CORNERIA: i32 = 0x0E;
const GR_KIND_VENOM: i32 = 0x0F;
use super::GR_KIND_PSTADIUM;

impl super::super::state::Running<'_> {
    /// Pokémon Stadium's big screen shows `mode` (`grStadium_801D2528` on its display object);
    /// any other stage has none.
    fn stadium_screen(&mut self, mode: i32) {
        if self.game.stage_info.grkind == GR_KIND_PSTADIUM {
            let g = super::ground::map_gobj(self.game, super::stadium::MAP_DISPLAY)
                .expect("Pokémon Stadium's display");
            super::stadium::show(self, g, mode, 0);
        }
    }

    /// A player's KO on Pokémon Stadium's screen, `grStadium_801D3B4C`: the player and its
    /// slot type, then the KO's mode.
    #[doc(alias = "grStadium_801D3B4C")]
    pub fn stadium_entrance(&mut self, slot: i32, slot_type: i32) {
        if self.game.stage_info.grkind == GR_KIND_PSTADIUM {
            let g = super::ground::map_gobj(self.game, super::stadium::MAP_DISPLAY)
                .expect("Pokémon Stadium's display");
            let gp = super::ground::ground_of(self.game, g);
            let v = self.game.grounds[gp.0].u.stadium_display();
            v.player = slot as i16;
            v.slot_type = slot_type as i16;
            super::stadium::show(self, g, 9, 0);
        }
    }

    /// `grStadium_801D4150`: the screen as the HUD comes on.
    #[doc(alias = "grStadium_801D4150")]
    pub fn stadium_hud_on(&mut self) {
        self.stadium_screen(1);
    }

    /// `grStadium_801D4084` and `grStadium_801D410C`: the screen for Ready.
    #[doc(alias = "grStadium_801D4084")]
    #[doc(alias = "grStadium_801D410C")]
    pub fn stadium_ready(&mut self, sudden_death: bool) {
        self.stadium_screen(if sudden_death { 0xA } else { 0xD });
    }

    /// `grStadium_801D4040`: the screen for GO!.
    #[doc(alias = "grStadium_801D4040")]
    pub fn stadium_go(&mut self) {
        self.stadium_screen(0xB);
    }

    /// `grStadium_801D40C8`: the screen for GAME!.
    #[doc(alias = "grStadium_801D40C8")]
    pub fn stadium_game(&mut self) {
        self.stadium_screen(0xC);
    }

    /// The stage's end of the match, `Ground_801C1158`: Pokémon Stadium's screen's names or
    /// count, or Corneria's and Venom's Arwings; nothing elsewhere.
    #[doc(alias = "Ground_801C1158")]
    pub fn stage_match_end(&mut self) {
        match self.game.stage_info.grkind {
            GR_KIND_PSTADIUM => {
                let g = super::ground::map_gobj(self.game, super::stadium::MAP_DISPLAY)
                    .expect("Pokémon Stadium's display");
                super::stadium::text_names_or_count(self, g);
            }
            GR_KIND_CORNERIA | GR_KIND_VENOM => super::corneria::match_end(self),
            _ => {}
        }
    }

    /// Starts the stage's match, `Stage_802252E4`: the stage kind given, its ground kind the
    /// selected stage's.
    #[doc(alias = "Stage_802252E4")]
    pub fn start_stage(&mut self, _stkind: i32) {
        let grkind = crate::lb::audio::ground_kind(self.game.gr.stkind) as i32;
        super::setup::start(self, grkind);
    }
}
