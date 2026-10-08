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
        // `lbAudioAx_80023F28`: the song streams from the disc (the audio port's).
        super::super::code::call(self, 0x8002_3F28, &[super::super::code::Val::Int(bgm)]);
        self.game.stage_info.x98 = bgm;
        self.game.stage_info.unk8C.b1 = alternate;
        which
    }
}

/// `Gr_Kind_Corneria`, `Gr_Kind_Venom` and `Gr_Kind_PStadium`.
const GR_KIND_CORNERIA: i32 = 0x0E;
const GR_KIND_VENOM: i32 = 0x0F;
const GR_KIND_PSTADIUM: i32 = 0x10;

impl super::super::state::Running<'_> {
    /// Pokémon Stadium's big screen shows `cmd` (`grStadium_801D2528` on its display object);
    /// any other stage has none.
    fn stadium_screen(&mut self, cmd: i32) {
        if self.game.stage_info.grkind == GR_KIND_PSTADIUM {
            super::super::code::unported_call(&format!("grStadium_801D2528({cmd})"));
        }
    }

    /// A player's entrance on Pokémon Stadium's screen, `grStadium_801D3B4C`.
    // Ports `grStadium_801D3B4C`; aliased once a check reaches it.
    pub fn stadium_entrance(&mut self, _slot: i32, _slot_type: i32) {
        self.stadium_screen(9);
    }

    /// `grStadium_801D4150`: the screen as the HUD comes on.
    // Ports `grStadium_801D4150`; aliased once a check reaches it.
    pub fn stadium_hud_on(&mut self) {
        self.stadium_screen(1);
    }

    /// `grStadium_801D4084` and `grStadium_801D410C`: the screen for Ready.
    #[doc(alias = "grStadium_801D4084")]
    // Ports `grStadium_801D410C`; aliased once a check reaches it.
    pub fn stadium_ready(&mut self, sudden_death: bool) {
        self.stadium_screen(if sudden_death { 0xA } else { 0xD });
    }

    /// `grStadium_801D4040`: the screen for GO!.
    #[doc(alias = "grStadium_801D4040")]
    pub fn stadium_go(&mut self) {
        self.stadium_screen(0xB);
    }

    /// `grStadium_801D40C8`: the screen for GAME!.
    // Ports `grStadium_801D40C8`; aliased once a check reaches it.
    pub fn stadium_game(&mut self) {
        self.stadium_screen(0xC);
    }

    /// The stage's end of the match, `Ground_801C1158`: Pokémon Stadium's screen, or
    /// Corneria's and Venom's Arwings; nothing elsewhere.
    // Ports `Ground_801C1158`; aliased once a check reaches it.
    pub fn stage_match_end(&mut self) {
        match self.game.stage_info.grkind {
            GR_KIND_PSTADIUM => super::super::code::unported_call("grStadium_801D39A0"),
            GR_KIND_CORNERIA | GR_KIND_VENOM => {
                super::super::code::unported_call("grCorneria_801E2A6C")
            }
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
