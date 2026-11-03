//! Giga Bowser's transformation (`vi1201v2.c`, `Vi1201v2.dat`): Final Destination set up for
//! its second Bowser demo (26), the character waiting before Bowser's trophy on its stand;
//! at the first model's frame 251 the trophy and stand go, the scene's second model comes in
//! and Giga Bowser is made in his cutscene's state (pose 15), at his own size.

use melee_data::archive::DataRef;

use super::super::handles::CodeRef;
use super::super::state::{Obj, Running};
use super::Loaded;
use super::v1201v1::{ST_KIND_LAST, TROPHY_KOOPA};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::vec::Vec3;
use gekko_fp as fp;

/// `un_803204C0`, `un_803204E4`, `un_8032074C`, `un_803208F0`, `un_80320984`, `fn_80320A1C`.
pub const STAND_PROC: u32 = 0x8032_04C0;
pub const TROPHY_PROC: u32 = 0x8032_04E4;
pub const FIRST_PROC: u32 = 0x8032_074C;
pub const CAMERA_RENDER: u32 = 0x8032_08F0;
pub const CAMERA_PROC: u32 = 0x8032_0984;
pub const FOG_PROC: u32 = 0x8032_0A1C;

/// `mn_8022EAE0`, the other models' process (the main menu's background's).
const MODEL_PROC: u32 = 0x8022_EAE0;

/// `CKind_GKoops`.
const CKIND_GKOOPS: i32 = 0x1D;

/// The first model's frame at which Giga Bowser comes in.
const TRANSFORM_FRAME: f32 = 251.0;

/// `vi1201v2.c`'s statics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Vi1201v2 {
    /// `un_804D7010`: the scene (`visual1201v2Scene`).
    pub scene: Option<DataRef>,
    /// `un_804D7014`: the stand's scene (`standScene`).
    pub stand: Option<DataRef>,
    /// `un_804D7018`: the character's wait animations.
    pub waits: Loaded,
    /// `un_804D701C`: `Vi1201v2.dat`.
    pub archive: Loaded,
    /// `un_804D7020`: the trophy's joint (`ToyKoopaModel_TopN_joint`).
    pub trophy: Option<DataRef>,
    /// `un_804D7024`: the joint the trophy and its stand follow (the first model's third).
    pub anchor: Option<JObjId>,
    /// `un_804D7028`: the fog's color at its animation's start.
    pub erase: [u8; 4],
    /// `un_804D702C`: set to 0 and never read.
    pub x2c: i32,
    /// `un_804D7030`, `un_804D7034`: the trophy's and the stand's GObjs, until they go.
    pub trophy_gobj: Option<GObjId>,
    pub stand_gobj: Option<GObjId>,
}

impl Running<'_> {
    /// The cutscene's stage and character, `un_80320508`: Final Destination set up for demo
    /// 26, the character waiting (pose 8), set to face the camera.
    #[doc(alias = "un_80320508")]
    fn vi1201v2_setup(&mut self, ckind: i32, costume: i32) {
        self.vi_stage_demo(ST_KIND_LAST, 0x1A, |_| {});
        self.fighter_demo_init();
        self.game.init_all_players();
        let waits = self.game.vi.v1201v2.waits;
        self.vi_player(0, ckind, costume, waits, 3, -1.0, Some(Vec3::ZERO));
        self.player_create_demo(0, 8);
        self.player_set_facing_all(0, 0.0);
    }

    /// Giga Bowser's entrance, `un_803205F4`: the scene's second model at its frame 251, and
    /// Giga Bowser facing left in his cutscene's state (pose 15), scaled to his size over his
    /// model's; the scene's sound banks.
    #[doc(alias = "un_803205F4")]
    fn vi1201v2_giga_bowser(&mut self) {
        let scene = self.game.vi.v1201v2.scene.expect("the scene");
        let m = super::scene_model(self, scene, 1).expect("the scene's second model");
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(0xE, 0xF, 0);
        let j = jobj::load_joint(&mut g.objs, data, data.ptr(m)).expect("a scene model");
        g.world
            .set_obj(gobj, super::super::state::OBJ_JOBJ, Obj::JObj(j));
        g.world
            .setup_gx_link(gobj, CodeRef(super::super::render::JOBJ_CALLBACK), 0xB, 0);
        super::super::gm::vs::add_model_anim(&mut g.objs, data, Some(j), m, 0);
        jobj::req_anim_all(&mut g.objs, Some(j), TRANSFORM_FRAME);
        g.world.setup_proc(gobj, CodeRef(MODEL_PROC), 0);
        let archive = self.game.vi.v1201v2.archive;
        self.vi_player(1, CKIND_GKOOPS, 0, archive, 8, -1.0, Some(Vec3::ZERO));
        self.player_create_demo(1, 0xF);
        let s = self.player_scale_ratio(1);
        let s = mem(fp::fdivs(1.0, reg(s)));
        self.player_set_scale(1, s);
        let a = &mut self.game.audio;
        a.lb.unwant_banks(0x18);
        a.lb.want_banks(8, 0x20_u64 << 48);
        a.lb_start_bank_loads(self.data);
        self.game.audio.lb_wait_for_banks(self.data);
    }

    /// The first model's process, `un_8032074C`: its animation, and at its frame 251 the
    /// trophy and the stand freed and Giga Bowser's entrance.
    #[doc(alias = "un_8032074C")]
    pub fn vi1201v2_first_proc(&mut self, gobj: GObjId) {
        let Some(Obj::JObj(j)) = self.game.world.gobj(gobj).obj else {
            panic!("a cutscene model holds no JObj");
        };
        self.anim_all(j);
        if super::super::gm::title::anim_frame(self, j) != TRANSFORM_FRAME {
            return;
        }
        if let Some(g) = self.game.vi.v1201v2.trophy_gobj.take() {
            crate::hsd::gobj::free(self, g);
        }
        if let Some(g) = self.game.vi.v1201v2.stand_gobj.take() {
            crate::hsd::gobj::free(self, g);
        }
        self.vi1201v2_giga_bowser();
    }

    /// The camera's render callback, `un_803208F0`: the fighters' shadows, then the
    /// cutscene's passes erased to the fog's first color, and render links 0, 7 and 11.
    #[doc(alias = "un_803208F0")]
    pub fn vi1201v2_camera_render(&mut self, gobj: GObjId) {
        self.shadow_render(false);
        let erase = self.game.vi.v1201v2.erase;
        self.vi_run_camera(gobj, erase, 0x881);
    }

    /// The camera's process, `un_80320984`: its animation; rumbles at frames 130 and 190 and
    /// a long one (219 ticks) at 251, at its last frame the scene's end.
    #[doc(alias = "un_80320984")]
    pub fn vi1201v2_camera_proc(&mut self, gobj: GObjId) {
        let c = self.vi_cobj(gobj);
        crate::hsd::cobj::anim(&mut self.game.objs, self.data, Some(c));
        let frame = |r: &Self| r.vi_cobj_frame(c).0;
        if frame(self) == 130.0 || frame(self) == 190.0 {
            self.vi_rumble(0xD, 0);
        }
        if frame(self) == TRANSFORM_FRAME {
            self.vi_rumble(1, 0xDB);
        }
        let (cur, end) = self.vi_cobj_frame(c);
        if cur == end {
            self.vi_end();
        }
    }

    /// The fog's process, `fn_80320A1C`: its animation.
    #[doc(alias = "fn_80320A1C")]
    pub fn vi1201v2_fog_proc(&mut self, gobj: GObjId) {
        let Some(Obj::Fog(f)) = self.game.world.gobj(gobj).obj else {
            panic!("the cutscene's fog GObj holds no fog");
        };
        crate::hsd::fog::interpret_anim(&mut self.game.objs, self.data, Some(f));
    }

    /// The scene's entry, `vi1201v2_Scene_OnEnter`: the effects, the scene's song (0x5C) with
    /// the audio paused, `Vi1201v2.dat`, Bowser's trophy, the stand and the character's wait
    /// animations (the region's class 2 trophies marked first, `Toy_803124BC`), the camera,
    /// the models but the second, the stage and character, the trophy and its stand, the fog
    /// with its animation started, and the lights.
    #[doc(alias = "vi1201v2_Scene_OnEnter")]
    #[doc(alias = "un_803207C4")]
    #[doc(alias = "un_803204C0")]
    #[doc(alias = "un_803204E4")]
    #[doc(alias = "vi1201v2_Scene_OnFrame")]
    pub fn vi1201v2_on_enter(&mut self) {
        let desc = self.game.vi.desc_1201v2;
        super::super::ef::eflib::init(self);
        super::super::ef::load_sync(self, 0);
        self.lb_play_music(0x5C);
        self.lb_audio_pause(true);
        let vi = self.vi_load("Vi1201v2.dat");
        let a = vi.archive.expect("Vi1201v2.dat");
        let scene = super::scene_symbol(self, a, "Vi1201v2.dat", "visual1201v2Scene");
        let koopa = self.vi_load("TyKoopa.dat");
        let a = koopa.archive.expect("TyKoopa.dat");
        let trophy = super::scene_symbol(self, a, "TyKoopa.dat", "ToyKoopaModel_TopN_joint");
        let stnd = self.vi_load("GmRgStnd.dat");
        let a = stnd.archive.expect("GmRgStnd.dat");
        let stand = super::scene_symbol(self, a, "GmRgStnd.dat", "standScene");
        self.mark_class_two_trophies();
        let waits = self.vi_load(super::wait_file(desc[0]));
        let v = &mut self.game.vi.v1201v2;
        v.scene = Some(scene);
        v.archive = vi;
        v.trophy = Some(trophy);
        v.stand = Some(stand);
        v.waits = waits;
        self.vi_camera(scene, CAMERA_RENDER, 8, CAMERA_PROC);
        let mut i = 0;
        while let Some(m) = super::scene_model(self, scene, i) {
            if i != 1 {
                let proc = if i == 0 { FIRST_PROC } else { MODEL_PROC };
                let j = self.vi_model_at(m, 0xB, (proc, 0));
                if i == 0 {
                    let [n] =
                        super::super::gm::pause::tree_nodes(&self.game.objs.jobjs, Some(j), &[2]);
                    self.game.vi.v1201v2.anchor = n;
                }
            }
            i += 1;
        }
        self.vi1201v2_setup(i32::from(desc[0]), i32::from(desc[1]));
        let anchor = self.game.vi.v1201v2.anchor;
        let g = self.vi_trophy(trophy, 0.55, anchor, (TROPHY_PROC, 0x17));
        self.game.vi.v1201v2.trophy_gobj = Some(g);
        let g = self.vi_stand(stand, TROPHY_KOOPA, 0.55, anchor, (STAND_PROC, 0x17));
        self.game.vi.v1201v2.stand_gobj = Some(g);
        let erase = self.vi_fog_animated(scene, FOG_PROC);
        let v = &mut self.game.vi.v1201v2;
        v.erase = erase;
        v.x2c = 0;
        self.vi_lights(scene);
        self.lb_audio_pause(false);
    }
}
