//! The F-Zero cutscene (`vi0801.c`, `Vi0801.dat`): the scene's racers on the F-Zero race's
//! stage (`St_Kind_BigBlueRoute`), whose cars follow 23 of the models' joints
//! (`grBigBlueRoute_8020DAB4`); no fighters.

use melee_data::archive::DataRef;

use super::super::state::Running;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;

/// `vi0801_8031ED70`, `vi0801_8031EE60`, `vi0801_8031EFE4`.
pub const CAMERA_RENDER: u32 = 0x8031_ED70;
pub const JOBJ_PROC: u32 = 0x8031_EE60;
pub const CAMERA_PROC: u32 = 0x8031_EFE4;

/// Where the scene's entry makes the race's cars' object (`grBigBlueRoute_8020DAB4`): its scale
/// joint's descriptor on the stack, an ID table key that only has to be one no other build uses
/// ([`START_FRAME`](super::super::gr::bigblueroute::START_FRAME)).
const CARS_FRAME: u32 = super::super::gr::setup::SETUP_FRAME - 0x40;

/// `St_Kind_BigBlueRoute`.
const ST_KIND_BIG_BLUE_ROUTE: i32 = 73;

/// `un_80400128`: each car's model and the pre-order index of its joint there.
const CAR_JOINTS: [(u32, i32); 23] = [
    (1, 2),
    (1, 3),
    (1, 4),
    (1, 5),
    (1, 6),
    (1, 7),
    (1, 8),
    (1, 9),
    (1, 10),
    (0, 2),
    (0, 3),
    (0, 4),
    (0, 5),
    (0, 6),
    (0, 7),
    (0, 8),
    (0, 9),
    (0, 10),
    (0, 11),
    (2, 2),
    (2, 3),
    (2, 4),
    (2, 5),
];

/// `vi0801.c`'s statics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Vi0801 {
    /// `un_804D6FB8`: the scene (`visual0801Scene`).
    pub scene: Option<DataRef>,
    /// `un_804D6FBC`: the fog's color.
    pub erase: [u8; 4],
    /// `un_804A2EA8`: the joints the cars follow (23 of its 24).
    pub cars: [Option<JObjId>; 24],
}

impl Running<'_> {
    /// The camera's render callback, `vi0801_8031ED70`: as Star Fox's
    /// ([`Running::vi0601_camera_render`]) with the fog's color.
    #[doc(alias = "vi0801_8031ED70")]
    pub fn vi0801_camera_render(&mut self, gobj: GObjId) {
        let erase = self.game.vi.v0801.erase;
        self.vi_run_camera_layers(gobj, erase);
    }

    /// The scene's models and sound banks, `vi0801_8031EE84`: each model's joints the cars
    /// follow kept (`lb_80011E24`).
    #[doc(alias = "vi0801_8031EE84")]
    #[doc(alias = "vi0801_8031EE60")]
    fn vi0801_models(&mut self, scene: DataRef) {
        self.vi_models(scene, 0xB, JOBJ_PROC, |r, i, j| {
            for (k, &(model, index)) in CAR_JOINTS.iter().enumerate() {
                if i == model {
                    let [n] =
                        super::super::gm::pause::tree_nodes(&r.game.objs.jobjs, Some(j), &[index]);
                    r.game.vi.v0801.cars[k] = n;
                }
            }
        });
        let a = &mut self.game.audio;
        a.lb.unwant_banks(0x18);
        a.lb.want_banks(8, 0x2000_0000_0000);
        a.lb_start_bank_loads(self.data);
        self.game.audio.lb_wait_for_banks(self.data);
    }

    /// The camera's process, `vi0801_8031EFE4`: its animation; rumbles at frames 75, 160
    /// and 120, and at its last frame the scene's end.
    #[doc(alias = "vi0801_8031EFE4")]
    pub fn vi0801_camera_proc(&mut self, gobj: GObjId) {
        let c = self.vi_cobj(gobj);
        crate::hsd::cobj::anim(&mut self.game.objs, self.data, Some(c));
        let frame = |r: &Self| f64::from(r.vi_cobj_frame(c).0);
        if frame(self) == 75.0 || frame(self) == 160.0 {
            self.vi_rumble(0xC, 0);
        }
        if frame(self) == 120.0 {
            self.vi_rumble(0x10, 0);
        }
        let (cur, end) = self.vi_cobj_frame(c);
        if cur == end {
            self.vi_end();
        }
    }

    /// The scene's entry, `vi0801_Scene_OnEnter`: the music stopped, the effects, the scene's
    /// song (0x5B) with the audio paused, `Vi0801.dat`, the camera, the models, the F-Zero
    /// race's stage, the fog and the lights, the cars on the models' joints, the players
    /// reset.
    #[doc(alias = "vi0801_Scene_OnEnter")]
    pub fn vi0801_on_enter(&mut self) {
        self.lb_audio_music_stop();
        super::super::ef::eflib::init(self);
        super::super::ef::load_sync(self, 0);
        self.lb_play_music(0x5B);
        self.lb_audio_pause(true);
        let vi = self.vi_load("Vi0801.dat");
        let scene = super::scene_symbol(
            self,
            vi.archive.expect("Vi0801.dat"),
            "Vi0801.dat",
            "visual0801Scene",
        );
        self.game.vi.v0801.scene = Some(scene);
        self.vi_camera(scene, CAMERA_RENDER, 8, CAMERA_PROC);
        self.vi0801_models(scene);
        self.vi_stage(ST_KIND_BIG_BLUE_ROUTE, |_| {});
        let erase = self.vi_fog_with(scene, 0xB);
        self.game.vi.v0801.erase = erase;
        self.vi_lights(scene);
        let cars = self.game.vi.v0801.cars;
        super::super::gr::bigblueroute::cars_on_joints(self, &cars, 0.5, 0x17, CARS_FRAME);
        self.game.init_all_players();
        self.lb_audio_pause(false);
    }
}
