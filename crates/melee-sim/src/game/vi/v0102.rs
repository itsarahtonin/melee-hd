//! Luigi's cutscene (`vi0102.c`, `Vi0102.dat`): Mario and Luigi at Peach's Castle, the
//! camera's animation running the scene, a rumble at its frame 190.

use melee_data::archive::DataRef;

use super::super::state::{Obj, Running};
use super::Loaded;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// The stack the stage's setup (`Stage_8022524C`) runs under: `vi0102_Scene_OnEnter`'s and `vi0102_8031CB00`'s frames.
const FRAMES: u32 = 0x28 + 0x18;

/// `vi0102_JObjCallback`, `vi0102_CameraCallback`, `vi0102_RunFrame`.
pub const JOBJ_PROC: u32 = 0x8031_CC68;
pub const CAMERA_RENDER: u32 = 0x8031_CC8C;
pub const CAMERA_PROC: u32 = 0x8031_CD20;

/// `St_Kind_Castle`, `CKind_Mario`, `CKind_Luigi`.
const ST_KIND_CASTLE: i32 = 4;
const CKIND_MARIO: i32 = 8;
const CKIND_LUIGI: i32 = 7;

/// The fog's GObj class here (`GObj_Create(0xA, 3, 0)`).
const CLASS_FOG: u16 = 0xA;

/// `vi0102.c`'s statics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Vi0102 {
    /// `un_804D6F30`: the scene (`visual0102Scene`).
    pub scene: Option<DataRef>,
    /// `un_804D6F38`: `Vi0102.dat`.
    pub archive: Loaded,
    /// `erase_colors_vi0102`: the fog's color.
    pub erase: [u8; 4],
}

impl Running<'_> {
    /// The cutscene's stage and players, `vi0102_8031CB00`: Peach's Castle set up for a demo,
    /// Mario and Luigi in their cutscene's motion (pose 9), and the scene's sound banks.
    #[doc(alias = "vi0102_8031CB00")]
    fn vi0102_setup(&mut self, mario_costume: i32, luigi_costume: i32) {
        self.vi_stage(ST_KIND_CASTLE, FRAMES, |_| {});
        self.fighter_demo_init();
        self.game.init_all_players();
        let archive = self.game.vi.v0102.archive;
        for (slot, ckind, costume) in [
            (0, CKIND_MARIO, mario_costume),
            (1, CKIND_LUIGI, luigi_costume),
        ] {
            self.vi_player(slot, ckind, costume, archive, 4, 1.0, Some(Vec3::ZERO));
            self.player_create_demo(slot, 9);
        }
        let a = &mut self.game.audio;
        a.lb.unwant_banks(0x18);
        a.lb.want_banks(8, 0x4_0000_0000);
        a.lb_start_bank_loads(self.data);
        self.game.audio.lb_wait_for_banks(self.data);
    }

    /// The camera's render callback, `vi0102_CameraCallback`: the fighters' shadows, then the
    /// cutscene's passes erased to the fog's color, and render links 0, 7 and 11.
    #[doc(alias = "vi0102_CameraCallback")]
    pub fn vi0102_camera_render(&mut self, gobj: GObjId) {
        self.shadow_render(false);
        let erase = self.game.vi.v0102.erase;
        self.vi_run_camera(gobj, erase, 0x881);
    }

    /// The camera's process, `vi0102_RunFrame`: its animation; at the eye's frame 190 a
    /// rumble, and at its last frame the scene's end.
    #[doc(alias = "vi0102_RunFrame")]
    pub fn vi0102_camera_proc(&mut self, gobj: GObjId) {
        let Some(Obj::CObj(c)) = self.game.world.gobj(gobj).obj else {
            panic!("vi0102's camera holds no CObj");
        };
        crate::hsd::cobj::anim(&mut self.game.objs, self.data, Some(c));
        if f64::from(self.vi_eye_frame(c).0) == 190.0 {
            self.vi_rumble(0x21, 0);
        }
        let (cur, end) = self.vi_eye_frame(c);
        if cur == end {
            self.vi_end();
        }
    }

    /// The scene's entry, `vi0102_Scene_OnEnter`: the music stopped, the effects, the scene's
    /// song (0x56) with the audio paused, `Vi0102.dat`, the camera, the models, the stage and
    /// players, the fog and the lights.
    #[doc(alias = "vi0102_Scene_OnEnter")]
    #[doc(alias = "vi0102_JObjCallback")]
    pub fn vi0102_on_enter(&mut self) {
        let desc = super::enter_desc(self.game, 0x11);
        self.lb_audio_music_stop();
        super::super::ef::eflib::init(self);
        super::super::ef::load_sync(self, 0);
        self.lb_play_music(0x56);
        self.lb_audio_pause(true);
        let archive = self.vi_load("Vi0102.dat");
        let scene = super::scene_symbol(
            self,
            archive.archive.expect("Vi0102.dat"),
            "Vi0102.dat",
            "visual0102Scene",
        );
        let v = &mut self.game.vi.v0102;
        v.archive = archive;
        v.scene = Some(scene);
        self.vi_camera(scene, CAMERA_RENDER, 8, CAMERA_PROC);
        self.vi_models(scene, 0xB, JOBJ_PROC, |_, _, _| {});
        self.vi0102_setup(i32::from(desc[1]), i32::from(desc[3]));
        let erase = self.vi_fog_with(scene, CLASS_FOG);
        self.game.vi.v0102.erase = erase;
        self.vi_lights(scene);
        self.lb_audio_pause(false);
    }
}
