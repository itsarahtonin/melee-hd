//! Bowser's trophy (`vi1201v1.c`, `Vi1201v1.dat`): Final Destination set up for its first
//! Bowser demo (25), Bowser's trophy on its stand following one of the scene's joints, an
//! effect at the models' frame 120, and the character, made at the camera's frame 100,
//! posing as a winner (pose 1) from its results animations.

use melee_data::archive::DataRef;

use super::super::state::{Obj, Running};
use super::Loaded;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;
use crate::vec::Vec3;

/// The stack the stage's setup (`Stage_8022524C`) runs under: `vi1201v1_Scene_OnEnter`'s frames.
const FRAMES: u32 = 0x48;

/// `un_8031F990`, `un_8031F9B4`, `fn_8031FAA8`, `fn_8031FB90`, `fn_8031FC30`, `fn_8031FCBC`.
pub const STAND_PROC: u32 = 0x8031_F990;
pub const TROPHY_PROC: u32 = 0x8031_F9B4;
pub const JOBJ_PROC: u32 = 0x8031_FAA8;
pub const CAMERA_RENDER: u32 = 0x8031_FB90;
pub const CAMERA_PROC: u32 = 0x8031_FC30;
pub const FOG_PROC: u32 = 0x8031_FCBC;

/// `St_Kind_Last`.
pub(super) const ST_KIND_LAST: i32 = 0x20;

/// The trophy whose stand placement the stand takes (`Toy_803060BC(0x1E, …)`).
pub(super) const TROPHY_KOOPA: i32 = 0x1E;

/// `vi1201v1.c`'s statics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Vi1201v1 {
    /// `un_804D6FE0`: the scene (`visual1201v1Scene`).
    pub scene: Option<DataRef>,
    /// `un_804D6FE4`: the stand's scene (`standScene`).
    pub stand: Option<DataRef>,
    /// `un_804D6FE8`: `Vi1201v1.dat`, then the character's results animations.
    pub archive: Loaded,
    /// `un_804D6FEC`: the trophy's joint (`ToyKoopaModel_TopN_joint`).
    pub trophy: Option<DataRef>,
    /// `un_804D6FF0`: the joint the trophy and its stand follow (the last model's fourth).
    pub anchor: Option<JObjId>,
    /// `un_804D6FF4`: the fog's color.
    pub erase: [u8; 4],
    /// `un_804D6FF8`: the fog's ticks until its GObj goes.
    pub fog_ticks: i32,
    /// `un_804D6FFC`, `un_804D6FFD`: the character and costume.
    pub ckind: u8,
    pub costume: u8,
    /// `un_804D7000`: the character's fighter, once made.
    pub fighter: Option<GObjId>,
}

impl Running<'_> {
    /// The character, `un_8031F9D8`: the fighters' setup again, the players reset, the
    /// character facing the camera at the origin in a winner's pose (pose 1) from its results
    /// animations, and the scene's sound banks.
    #[doc(alias = "un_8031F9D8")]
    fn vi1201v1_fighter(&mut self, ckind: i32, costume: i32) {
        self.fighter_demo_init();
        self.game.init_all_players();
        let archive = self.game.vi.v1201v1.archive;
        self.vi_player(0, ckind, costume, archive, 0, 0.0, Some(Vec3::ZERO));
        self.player_create_demo(0, 1);
        self.game.vi.v1201v1.fighter = self.game.player_entity(0);
        let a = &mut self.game.audio;
        a.lb.unwant_banks(0x18);
        a.lb.want_banks(8, 0x20_u64 << 48);
        a.lb_start_bank_loads(self.data);
        self.game.audio.lb_wait_for_banks(self.data);
    }

    /// A model's process, `fn_8031FAA8`: its animation, and at its frame 120 an effect (0x42B)
    /// at its second joint's place and a sound.
    #[doc(alias = "fn_8031FAA8")]
    pub fn vi1201v1_model_proc(&mut self, gobj: GObjId) {
        let Some(Obj::JObj(j)) = self.game.world.gobj(gobj).obj else {
            panic!("a cutscene model holds no JObj");
        };
        self.anim_all(j);
        if super::super::gm::title::anim_frame(self, j) != 120.0 {
            return;
        }
        let [child] = super::super::gm::pause::tree_nodes(&self.game.objs.jobjs, Some(j), &[1]);
        let pos = self.game.objs.jobjs[child.expect("the model's second joint")].translate;
        use super::super::ef::Arg;
        super::super::ef::spawn(
            self,
            0x42B,
            Some(gobj),
            &[
                Arg::Vec(pos),
                Arg::Float(0.0),
                Arg::Float(1.0),
                Arg::Int(0x00FF_FFFF),
                Arg::Int(0x0080_8080),
            ],
        );
        self.sfx_play(0x61, 0x7F, 0x40);
    }

    /// The camera's render callback, `fn_8031FB90`: the fighters' shadows once the character
    /// is made, then the cutscene's passes erased to the fog's color, and render links 0, 7
    /// and 11.
    #[doc(alias = "fn_8031FB90")]
    pub fn vi1201v1_camera_render(&mut self, gobj: GObjId) {
        if self.game.vi.v1201v1.fighter.is_some() {
            self.shadow_render(false);
        }
        let erase = self.game.vi.v1201v1.erase;
        self.vi_run_camera(gobj, erase, 0x881);
    }

    /// The camera's process, `fn_8031FC30`: its animation; at frame 120 a rumble, at frame 100
    /// the character made, at its last frame the scene's end.
    #[doc(alias = "fn_8031FC30")]
    pub fn vi1201v1_camera_proc(&mut self, gobj: GObjId) {
        let c = self.vi_cobj(gobj);
        crate::hsd::cobj::anim(&mut self.game.objs, self.data, Some(c));
        if f64::from(self.vi_cobj_frame(c).0) == 120.0 {
            self.vi_rumble(0xD, 0);
        }
        if f64::from(self.vi_cobj_frame(c).0) == 100.0 {
            let v = self.game.vi.v1201v1;
            self.vi1201v1_fighter(i32::from(v.ckind), i32::from(v.costume));
        }
        let (cur, end) = self.vi_cobj_frame(c);
        if cur == end {
            self.vi_end();
        }
    }

    /// The fog's process, `fn_8031FCBC`: its GObj freed once it has counted 100 ticks.
    #[doc(alias = "fn_8031FCBC")]
    pub fn vi1201v1_fog_proc(&mut self, gobj: GObjId) {
        let n = self.game.vi.v1201v1.fog_ticks;
        if gekko_fp::frsp(f64::from(n)) >= 100.0 {
            crate::hsd::gobj::free(self, gobj);
        } else {
            self.game.vi.v1201v1.fog_ticks = n.wrapping_add(1);
        }
    }

    /// The scene's entry, `vi1201v1_Scene_OnEnter`: the music stopped, the effects, the
    /// scene's song (0x59) with the audio paused, `Vi1201v1.dat`, Bowser's trophy, the stand
    /// and the character's results animations (the region's class 2 trophies marked first,
    /// `Toy_803124BC`), the camera, the models, Final Destination set up for demo 25, the
    /// trophy and its stand, the fog, which goes after 100 ticks, and the lights.
    #[doc(alias = "vi1201v1_Scene_OnEnter")]
    #[doc(alias = "un_8031F990")]
    #[doc(alias = "un_8031F9B4")]
    #[doc(alias = "vi1201v1_Scene_OnFrame")]
    pub fn vi1201v1_on_enter(&mut self) {
        let desc = super::enter_desc(self.game, 0x19);
        let v = &mut self.game.vi.v1201v1;
        v.ckind = desc[0];
        v.costume = desc[1];
        v.fighter = None;
        self.lb_audio_music_stop();
        super::super::ef::eflib::init(self);
        super::super::ef::load_sync(self, 0);
        self.lb_play_music(0x59);
        self.lb_audio_pause(true);
        let vi = self.vi_load("Vi1201v1.dat");
        let a = vi.archive.expect("Vi1201v1.dat");
        let scene = super::scene_symbol(self, a, "Vi1201v1.dat", "visual1201v1Scene");
        let koopa = self.vi_load("TyKoopa.dat");
        let a = koopa.archive.expect("TyKoopa.dat");
        let trophy = super::scene_symbol(self, a, "TyKoopa.dat", "ToyKoopaModel_TopN_joint");
        let stnd = self.vi_load("GmRgStnd.dat");
        let a = stnd.archive.expect("GmRgStnd.dat");
        let stand = super::scene_symbol(self, a, "GmRgStnd.dat", "standScene");
        self.mark_class_two_trophies();
        let results = super::super::gm::results_stats::results_file(i32::from(desc[0]))
            .expect("the character's results animations");
        let archive = self.vi_load(results);
        let v = &mut self.game.vi.v1201v1;
        v.scene = Some(scene);
        v.trophy = Some(trophy);
        v.stand = Some(stand);
        v.archive = archive;
        self.vi_camera(scene, CAMERA_RENDER, 8, CAMERA_PROC);
        self.vi_models_at(scene, 0xB, (JOBJ_PROC, 0), |r, _, j| {
            let [n] = super::super::gm::pause::tree_nodes(&r.game.objs.jobjs, Some(j), &[3]);
            r.game.vi.v1201v1.anchor = n;
        });
        self.vi_stage_demo(ST_KIND_LAST, 0x19, FRAMES, |_| {});
        let anchor = self.game.vi.v1201v1.anchor;
        self.vi_trophy(trophy, 0.55, anchor, (TROPHY_PROC, 0));
        self.vi_stand(stand, TROPHY_KOOPA, 0.55, anchor, (STAND_PROC, 0));
        let (fog, erase) = self.vi_fog_gobj(scene, 0xB);
        self.game
            .world
            .setup_proc(fog, crate::game::handles::CodeRef(FOG_PROC), 0);
        let v = &mut self.game.vi.v1201v1;
        v.erase = erase;
        v.fog_ticks = 0;
        self.vi_lights(scene);
        self.lb_audio_pause(false);
    }
}
