//! Brinstar's cutscene (`vi0401.c`, `Vi0401.dat`): the player's character waiting on
//! Brinstar, placed at the scene's first model's joint, with the scene's information overlay
//! (`visual0401InfoScene`) through the HUD's damage camera (`IfAll.dat`'s
//! `ScInfDmg_scene_data`).

use melee_data::archive::DataRef;

use super::super::render::CAMERA_CALLBACK;
use super::super::state::Running;
use super::Loaded;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;

/// `fn_8031D168`, `vi0401_8031D18C`, `vi0401_8031D1B0`, `vi0401_8031D23C`, the scene's
/// frame function.
pub const INFO_PROC: u32 = 0x8031_D168;
pub const JOBJ_PROC: u32 = 0x8031_D18C;
pub const CAMERA_RENDER: u32 = 0x8031_D1B0;
pub const CAMERA_PROC: u32 = 0x8031_D23C;
pub const ON_FRAME: u32 = 0x8031_D698;

/// `St_Kind_Zebes`.
const ST_KIND_ZEBES: i32 = 6;

/// `vi0401.c`'s statics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Vi0401 {
    /// `un_804D6F48`, `un_804D6F4C`, `un_804D6F50`: the scene, its overlay, the damage
    /// camera's scene.
    pub scene: Option<DataRef>,
    pub info: Option<DataRef>,
    pub damage: Option<DataRef>,
    /// `un_804D6F54`: the character's cutscene waits.
    pub waits: Loaded,
    /// `un_804D6F58`: the first model's joint 1, where the character stands.
    pub spot: Option<JObjId>,
    /// `erase_colors_vi0401`: the fog's color.
    pub erase: [u8; 4],
}

impl Running<'_> {
    /// The cutscene's stage and player, `un_8031D030`: Brinstar set up for a demo, the
    /// character waiting (pose 8) at the scene's spot, the scene's sound banks and the
    /// reverb's send.
    #[doc(alias = "un_8031D030")]
    fn vi0401_setup(&mut self, ckind: i32, costume: i32) {
        self.vi_stage(ST_KIND_ZEBES, |_| {});
        self.fighter_demo_init();
        self.game.init_all_players();
        let waits = self.game.vi.v0401.waits;
        let spot = self.game.vi.v0401.spot.expect("the cutscene's spot");
        let pos = self.game.objs.jobjs[spot].translate;
        self.vi_player(0, ckind, costume, waits, 3, 1.0, Some(pos));
        self.player_create_demo(0, 8);
        let a = &mut self.game.audio;
        a.lb.unwant_banks(0x18);
        a.lb.want_banks(8, 0x100 << 32);
        a.lb_start_bank_loads(self.data);
        self.game.audio.lb_wait_for_banks(self.data);
        self.game.audio.lb_set_send(0x88);
    }

    /// The camera's render callback, `vi0401_8031D1B0`.
    #[doc(alias = "vi0401_8031D1B0")]
    pub fn vi0401_camera_render(&mut self, gobj: GObjId) {
        let erase = self.game.vi.v0401.erase;
        self.vi_run_camera(gobj, erase, 0x281);
    }

    /// The camera's process, `vi0401_8031D23C`: its animation, and at its last frame the
    /// scene's end.
    #[doc(alias = "vi0401_8031D23C")]
    pub fn vi0401_camera_proc(&mut self, gobj: GObjId) {
        self.vi_camera_anim_ends(gobj);
    }

    /// The scene's frame function, `vi0401_Scene_OnFrame`: Start on Adventure's player's
    /// port ends it, the rumble stopped (the music plays on).
    #[doc(alias = "vi0401_Scene_OnFrame")]
    pub fn vi0401_on_frame(&mut self) {
        let slot = self.adventure_slot();
        if self.game.buttons_triggered(slot) & u64::from(crate::input::pad::BUTTON_START) != 0 {
            self.vi_end();
        }
    }

    /// The scene's entry, `vi0401_Scene_OnEnter`: the music stopped, the effects, the scene's
    /// files, its fog, lights, camera and models, the overlay's camera and models, the stage
    /// and player, a rumble and three sounds.
    #[doc(alias = "vi0401_Scene_OnEnter")]
    #[doc(alias = "fn_8031D168")]
    #[doc(alias = "vi0401_8031D18C")]
    pub fn vi0401_on_enter(&mut self) {
        let desc = super::enter_desc(self.game, 0x12);
        self.lb_audio_music_stop();
        super::super::ef::eflib::init(self);
        super::super::ef::load_sync(self, 0);
        let ckind = desc[0];
        let vi = self.vi_load("Vi0401.dat");
        let a = vi.archive.expect("Vi0401.dat");
        let scene = super::scene_symbol(self, a, "Vi0401.dat", "visual0401Scene");
        let info = super::scene_symbol(self, a, "Vi0401.dat", "visual0401InfoScene");
        let hud = self.vi_load("IfAll.dat");
        let damage = super::scene_symbol(
            self,
            hud.archive.expect("IfAll.dat"),
            "IfAll.dat",
            "ScInfDmg_scene_data",
        );
        let waits = self.vi_load(super::wait_file(ckind));
        let v = &mut self.game.vi.v0401;
        v.scene = Some(scene);
        v.info = Some(info);
        v.damage = Some(damage);
        v.waits = waits;
        let erase = self.vi_fog_with(scene, 0xB);
        self.game.vi.v0401.erase = erase;
        self.vi_lights(scene);
        let cam = self.vi_camera_unanimated(scene, CAMERA_RENDER, 5);
        self.game.world.gobj_mut(cam).gxlink_prios = 0x2A9;
        self.vi_camera_animate(cam, scene, CAMERA_PROC);
        self.vi_models(scene, 9, JOBJ_PROC, |r, i, j| {
            if i == 0 {
                let [n] = super::super::gm::pause::tree_nodes(&r.game.objs.jobjs, Some(j), &[1]);
                r.game.vi.v0401.spot = n;
            }
        });
        let cam2 = self.vi_camera_unanimated(damage, CAMERA_CALLBACK, 8);
        self.game.world.gobj_mut(cam2).gxlink_prios = 0x801;
        self.vi_models(info, 0xB, INFO_PROC, |_, _, _| {});
        self.vi0401_setup(i32::from(desc[0]), i32::from(desc[1]));
        self.alarm_rumble_catch_up();
        self.vi_rumble(1, 0);
        for id in [0x61A88, 0x61A89, 0x61A8A] {
            self.sfx_play(id, 0x7F, 0x40);
        }
    }
}
