//! The metal cutscene (`vi1101.c`, `Vi1101.dat`): the character waiting on Battlefield while
//! metal Mario, and metal Luigi once he's unlocked, walk in (each a demo fighter in his
//! cutscene's slow walk, pose 10); the scene's other camera when Luigi is locked.

use melee_data::archive::DataRef;

use super::super::state::Running;
use super::Loaded;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

/// `fn_8031F548`, `fn_8031F56C`, `fn_8031F600`.
pub const JOBJ_PROC: u32 = 0x8031_F548;
pub const CAMERA_RENDER: u32 = 0x8031_F56C;
pub const CAMERA_PROC: u32 = 0x8031_F600;

/// `St_Kind_Battle`, `CKind_Mario`, `CKind_Luigi`.
const ST_KIND_BATTLE: i32 = 0x1F;
const CKIND_MARIO: i32 = 8;
const CKIND_LUIGI: i32 = 7;

/// `un_80400200.spawns`: where every player starts (the first), and the start points Mario
/// and Luigi walk from (the second and third).
const SPAWNS: [Vec3; 3] = [
    Vec3::ZERO,
    Vec3::new(15.8512, 67.0, 0.0),
    Vec3::new(-15.7657, 67.0, 0.0),
];

/// `un_804D5B08`: the screen's erase color.
const ERASE: [u8; 4] = [0, 0, 0, 0xFF];

/// `vi1101.c`'s statics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Vi1101 {
    /// `un_804D6FC0`: the scene (`visual1101Scene`).
    pub scene: Option<DataRef>,
    /// `un_804D6FC4`: the scene with the other camera (`visual1101Cam2Scene`).
    pub scene_cam2: Option<DataRef>,
    /// `un_804D6FC8`: the character's wait animations.
    pub waits: Loaded,
    /// `un_804D6FCC`: `Vi1101.dat`.
    pub archive: Loaded,
    /// `un_804D6FD0`: Mario's and Luigi's fighters.
    pub metal: [Option<GObjId>; 2],
}

impl Running<'_> {
    /// Whether Luigi is unlocked, which brings him into the scene.
    fn vi1101_luigi(&self) -> bool {
        self.game.gm.unlocks.ckind_unlocked(CKIND_LUIGI as u8)
    }

    /// The cutscene's stage and players, `un_8031F294`: Battlefield set up for a demo, the
    /// character waiting (pose 8), metal Mario and Luigi facing the camera in their slow walk
    /// (pose 10), each one's start point his root's place at his animation's frame 432; the
    /// scene's sound banks.
    #[doc(alias = "un_8031F294")]
    fn vi1101_setup(&mut self, ckind: i32, costume: i32) {
        self.vi_stage(ST_KIND_BATTLE, |_| {});
        self.fighter_demo_init();
        self.game.init_all_players();
        let v = self.game.vi.v1101;
        self.vi_player(0, ckind, costume, v.waits, 3, 1.0, Some(SPAWNS[0]));
        self.player_create_demo(0, 8);
        super::super::gr::grlib::measure_start_points(self.game, self.data);
        let mut walkers = vec![(1, CKIND_MARIO)];
        if self.vi1101_luigi() {
            walkers.push((2, CKIND_LUIGI));
        }
        for (slot, ck) in walkers {
            self.vi_player(slot, ck, 0, v.archive, 5, 0.0, Some(SPAWNS[0]));
            self.game.set_player_is_metal(slot, 1);
            self.player_create_demo(slot, 10);
            let g = self.game.player_entity(slot);
            self.game.vi.v1101.metal[slot as usize - 1] = g;
            let j = self.root_jobj(g.expect("a metal fighter"));
            jobj::req_anim_all(&mut self.game.objs, Some(j), 432.0);
            self.anim_all(j);
            self.game.gr.start_points[slot as usize] = SPAWNS[slot as usize];
            jobj::req_anim_all(&mut self.game.objs, Some(j), 0.0);
        }
        let a = &mut self.game.audio;
        a.lb.unwant_banks(0x1C);
        let banks = (0x20_u64 << 48) | crate::lb::audio::character_banks(ckind);
        a.lb.want_banks(0xC, banks);
        a.lb_start_bank_loads(self.data);
        self.game.audio.lb_wait_for_banks(self.data);
    }

    /// The camera's render callback, `fn_8031F56C`: the fighters' shadows, then the
    /// cutscene's passes erased to black, and render links 0, 7 and 9.
    #[doc(alias = "fn_8031F56C")]
    pub fn vi1101_camera_render(&mut self, gobj: GObjId) {
        self.shadow_render(false);
        self.vi_run_camera(gobj, ERASE, 0x281);
    }

    /// The camera's process, `fn_8031F600`: its animation; at frame 170 (with Luigi) and 190
    /// a rumble and a landing's sound, at frames 241 (with Luigi) and 271 another sound, at
    /// its last frame the scene's end.
    #[doc(alias = "fn_8031F600")]
    pub fn vi1101_camera_proc(&mut self, gobj: GObjId) {
        let c = self.vi_cobj(gobj);
        crate::hsd::cobj::anim(&mut self.game.objs, self.data, Some(c));
        let frame = |r: &Self| f64::from(r.vi_cobj_frame(c).0);
        if frame(self) == 170.0 && self.vi1101_luigi() {
            self.vi_rumble(0xD, 0);
            self.sfx_play(0x209, 0x7F, 0x40);
        }
        if frame(self) == 190.0 {
            self.vi_rumble(0xD, 0);
            self.sfx_play(0x209, 0x7F, 0x40);
        }
        if frame(self) == 241.0 && self.vi1101_luigi() {
            self.sfx_play(0x20A, 0x7F, 0x40);
        }
        if frame(self) == 271.0 {
            self.sfx_play(0x20A, 0x7F, 0x40);
        }
        let (cur, end) = self.vi_cobj_frame(c);
        if cur == end {
            self.vi_end();
        }
    }

    /// The scene's entry, `vi1101_Scene_OnEnter`: the music stopped, the effects, the scene's
    /// song (0x55) with the audio paused, `Vi1101.dat` and the character's wait animations,
    /// the lights, the camera (the other scene's while Luigi is locked), the models, the
    /// stage and players.
    #[doc(alias = "vi1101_Scene_OnEnter")]
    #[doc(alias = "fn_8031F548")]
    #[doc(alias = "vi1101_Scene_OnFrame")]
    pub fn vi1101_on_enter(&mut self) {
        let desc = super::enter_desc(self.game, 0x18);
        self.lb_audio_music_stop();
        super::super::ef::eflib::init(self);
        super::super::ef::load_sync(self, 0);
        self.lb_play_music(0x55);
        self.lb_audio_pause(true);
        let archive = self.vi_load("Vi1101.dat");
        let a = archive.archive.expect("Vi1101.dat");
        let scene = super::scene_symbol(self, a, "Vi1101.dat", "visual1101Scene");
        let cam2 = super::scene_symbol(self, a, "Vi1101.dat", "visual1101Cam2Scene");
        let waits = self.vi_load(super::wait_file(desc[0]));
        let v = &mut self.game.vi.v1101;
        v.archive = archive;
        v.scene = Some(scene);
        v.scene_cam2 = Some(cam2);
        v.waits = waits;
        self.vi_lights(scene);
        let camera = if self.vi1101_luigi() { scene } else { cam2 };
        self.vi_camera(camera, CAMERA_RENDER, 5, CAMERA_PROC);
        self.vi_models(scene, 9, JOBJ_PROC, |_, _, _| {});
        self.vi1101_setup(i32::from(desc[0]), i32::from(desc[1]));
        self.lb_audio_pause(false);
    }
}
