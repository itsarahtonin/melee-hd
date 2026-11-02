//! The giant Kirby (`vi0502.c`, `Vi0502.dat`): the player's character waiting on Green
//! Greens at 0.7 its scale, a Kirby facing it whose start point the scene measures from its
//! animation's frame 120.

use melee_data::archive::DataRef;

use super::super::state::Running;
use super::Loaded;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

/// `vi0502_8031E304`, `vi0502_GObj_OnRender`, `vi0502_GObj_OnProc`.
pub const JOBJ_PROC: u32 = 0x8031_E304;
pub const CAMERA_RENDER: u32 = 0x8031_E328;
pub const CAMERA_PROC: u32 = 0x8031_E3BC;

/// `St_Kind_Greens`, `CKind_Kirby`.
const ST_KIND_GREENS: i32 = 17;
const CKIND_KIRBY: i32 = 4;

/// `un_804000D0.initial_pos`: where the character stands.
const START: Vec3 = Vec3 {
    x: 0.0,
    y: -3.0,
    z: 0.0,
};

/// `vi0502.c`'s statics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Vi0502 {
    /// `un_804D6F90`: the scene (`visual0502Scene`).
    pub scene: Option<DataRef>,
    /// `un_804D6F94`: the character's cutscene waits; `un_804D6F98`: `IrAls.dat`;
    /// `un_804D6F9C`: `Vi0502.dat`.
    pub waits: Loaded,
    pub irals: Loaded,
    pub archive: Loaded,
    /// `erase_colors_vi0502`: the fog's color.
    pub erase: [u8; 4],
    /// `kirby_gobj`: the Kirby's fighter.
    pub kirby: Option<GObjId>,
}

impl Running<'_> {
    /// The cutscene's stage and players, `vi0502_8031E124`: Green Greens at 0.7 its scale and
    /// set up for a demo, the character waiting (pose 8), and Kirby facing left in his
    /// cutscene motion (pose 14), his start point his root's place at his animation's frame
    /// 120.
    #[doc(alias = "vi0502_8031E124")]
    fn vi0502_setup(&mut self, ckind: i32, costume: i32, kirby_costume: i32) {
        self.vi_stage(ST_KIND_GREENS, |r| {
            super::super::gr::ground::set_scale(r.game, 0.7);
        });
        self.fighter_demo_init();
        self.game.init_all_players();
        let v = self.game.vi.v0502;
        self.vi_player(0, ckind, costume, v.waits, 3, 1.0, Some(START));
        self.player_create_demo(0, 8);
        self.vi_player(1, CKIND_KIRBY, kirby_costume, v.archive, 7, -1.0, None);
        self.player_create_demo(1, 14);
        let g = self.game.player_entity(1);
        self.game.vi.v0502.kirby = g;
        let j = self.root_jobj(g.expect("Kirby"));
        jobj::req_anim_all(&mut self.game.objs, Some(j), 120.0);
        self.anim_all(j);
        super::super::gr::grlib::measure_start_points(self.game, self.data);
        self.game.gr.start_points[1] = self.game.objs.jobjs[j].translate;
        jobj::req_anim_all(&mut self.game.objs, Some(j), 0.0);
        let a = &mut self.game.audio;
        a.lb.unwant_banks(0x1C);
        a.lb.want_banks(12, 0x800_0000_4000);
        a.lb_start_bank_loads(self.data);
        self.game.audio.lb_wait_for_banks(self.data);
    }

    /// The camera's process, `vi0502_GObj_OnProc`: its animation; at the eye's frame 30 a
    /// rumble and a sound, at its last frame the scene's end.
    #[doc(alias = "vi0502_GObj_OnProc")]
    pub fn vi0502_camera_proc(&mut self, gobj: GObjId) {
        let c = self.vi_cobj(gobj);
        crate::hsd::cobj::anim(&mut self.game.objs, self.data, Some(c));
        if f64::from(self.vi_eye_frame(c).0) == 30.0 {
            self.vi_rumble(14, 0);
            self.sfx_play(430_009, 0x7F, 0x40);
        }
        let (cur, end) = self.vi_eye_frame(c);
        if cur == end {
            self.vi_end();
        }
    }

    /// The scene's entry, `vi0502_Scene_OnEnter`: the music stopped, the effects, the scene's
    /// files, its fog, lights, camera and models, the stage and players.
    #[doc(alias = "vi0502_Scene_OnEnter")]
    #[doc(alias = "vi0502_GObj_OnRender")]
    #[doc(alias = "vi0502_8031E304")]
    pub fn vi0502_on_enter(&mut self) {
        let desc = self.game.vi.desc_0502;
        self.lb_audio_music_stop();
        super::super::ef::eflib::init(self);
        super::super::ef::load_sync(self, 0);
        let ckind = desc[0];
        let archive = self.vi_load("Vi0502.dat");
        let scene = super::scene_symbol(
            self,
            archive.archive.expect("Vi0502.dat"),
            "Vi0502.dat",
            "visual0502Scene",
        );
        let waits = self.vi_load(super::wait_file(ckind));
        let irals = self.vi_load("IrAls.dat");
        let v = &mut self.game.vi.v0502;
        v.scene = Some(scene);
        v.archive = archive;
        v.waits = waits;
        v.irals = irals;
        let erase = self.vi_fog_with(scene, 0xB);
        self.game.vi.v0502.erase = erase;
        self.vi_lights(scene);
        self.vi_camera(scene, CAMERA_RENDER, 5, CAMERA_PROC);
        self.vi_models(scene, 9, JOBJ_PROC, |_, _, _| {});
        self.vi0502_setup(i32::from(desc[0]), i32::from(desc[1]), i32::from(desc[3]));
    }
}
