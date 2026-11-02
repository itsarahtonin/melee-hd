//! The planet's explosion (`vi0402.c`, `Vi0402.dat`): models only, no stage or fighters.

use melee_data::archive::DataRef;

use super::super::render::CAMERA_CALLBACK;
use super::super::state::Running;

/// `un_8031D6E4`, `vi_8031D80C`.
pub const JOBJ_PROC: u32 = 0x8031_D6E4;
pub const CAMERA_PROC: u32 = 0x8031_D80C;

/// `vi0402.c`'s statics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Vi0402 {
    /// `un_804D6F68`: the scene (`visual0402Scene`).
    pub scene: Option<DataRef>,
}

impl Running<'_> {
    /// The scene's models and sound banks, `un_8031D708`.
    #[doc(alias = "un_8031D708")]
    #[doc(alias = "un_8031D6E4")]
    fn vi0402_models(&mut self, scene: DataRef) {
        self.vi_models(scene, 3, JOBJ_PROC, |_, _, _| {});
        let a = &mut self.game.audio;
        a.lb.unwant_banks(0x18);
        a.lb.want_banks(8, 0x100_0000_0000);
        a.lb_start_bank_loads(self.data);
        self.game.audio.lb_wait_for_banks(self.data);
    }

    /// The scene's entry, `vi0402_Scene_OnEnter`: the music stopped, the effects, the scene's
    /// song (0x58) with the audio paused, `Vi0402.dat`, its lights, camera and models, a
    /// rumble, and the players reset.
    #[doc(alias = "vi0402_Scene_OnEnter")]
    #[doc(alias = "vi_8031D80C")]
    pub fn vi0402_on_enter(&mut self) {
        self.lb_audio_music_stop();
        super::super::ef::eflib::init(self);
        super::super::ef::load_sync(self, 0);
        self.lb_play_music(0x58);
        self.lb_audio_pause(true);
        let vi = self.vi_load("Vi0402.dat");
        let scene = super::scene_symbol(
            self,
            vi.archive.expect("Vi0402.dat"),
            "Vi0402.dat",
            "visual0402Scene",
        );
        self.game.vi.v0402.scene = Some(scene);
        self.vi_lights(scene);
        let cam = self.vi_camera_unanimated(scene, CAMERA_CALLBACK, 8);
        self.game.world.gobj_mut(cam).gxlink_prios = 0x89;
        self.vi_camera_animate(cam, scene, CAMERA_PROC);
        self.vi0402_models(scene);
        self.alarm_rumble_catch_up();
        self.vi_rumble(0x22, 0);
        self.game.init_all_players();
        self.lb_audio_pause(false);
    }
}
