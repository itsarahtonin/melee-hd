//! The three Kirbys (`vi0501.c`, `Vi0501.dat`): the player's character waiting on Green
//! Greens, scaled to 0.7, with three Kirbys walking in, each a demo fighter whose start point
//! the scene measures from its animation's frame 140.

use melee_data::archive::DataRef;

use super::super::state::Running;
use super::Loaded;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;
use gekko_fp as fp;

/// `vi_8031DC80`, `fn_8031DD14`.
pub const CAMERA_RENDER: u32 = 0x8031_DC80;
pub const CAMERA_PROC: u32 = 0x8031_DD14;

/// `St_Kind_Greens`, `CKind_Kirby`.
const ST_KIND_GREENS: i32 = 17;
const CKIND_KIRBY: i32 = 4;

/// `vi0501.c`'s statics.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Vi0501 {
    /// `un_804D6F70`: the scene (`visual0501Scene`).
    pub scene: Option<DataRef>,
    /// `un_804D6F74`: `Vi0501.dat`; `un_804D6F78`: the character's cutscene waits.
    pub archive: Loaded,
    pub waits: Loaded,
    /// `erase_colors_vi0501`: the fog's color.
    pub erase: [u8; 4],
    /// `un_804D6F80`: the stage's scale before the scene's (`Ground_801C0498`).
    pub scale: f32,
    /// `un_804A2E98`: the Kirbys' fighters.
    pub kirbys: [Option<GObjId>; 3],
}

impl Running<'_> {
    /// The cutscene's stage and players, `un_8031D9F8`: Green Greens at 0.7 its scale and set
    /// up for a demo, the character waiting (pose 8), and the three Kirbys in costume
    /// `kirby_costume` with their counts, walking (poses 11 to 13), each one's start point
    /// its root's place at its animation's frame 140 times 1/0.7 the stage's old scale.
    #[doc(alias = "un_8031D9F8")]
    fn vi0501_setup(&mut self, ckind: i32, costume: i32, kirby_costume: i32, counts: [u8; 3]) {
        self.vi_stage(ST_KIND_GREENS, |r| {
            r.game.vi.v0501.scale = super::super::gr::ground::scale(r.game);
            super::super::gr::ground::set_scale(r.game, 0.7);
        });
        self.fighter_demo_init();
        self.game.init_all_players();
        let v = self.game.vi.v0501;
        self.vi_player(0, ckind, costume, v.waits, 3, 1.0, Some(Vec3::ZERO));
        self.player_create_demo(0, 8);
        super::super::gr::grlib::measure_start_points(self.game, self.data);
        for i in 1..4 {
            let count = counts[i - 1];
            let slot = i as i32;
            self.player_set_demo_archive_idx(
                CKIND_KIRBY,
                v.archive.archive.expect("Vi0501.dat"),
                v.archive.file,
                6,
            );
            self.player_80031da8(i32::from(count), kirby_costume);
            self.game.set_player_flags_bit1(slot);
            self.game.set_player_ckind(slot, CKIND_KIRBY);
            self.game.set_player_costume(slot, kirby_costume);
            self.game.set_player_port(slot, 0);
            self.game
                .set_player_pkind(slot, super::super::pl::player::PKIND_DEMO);
            self.game.set_player_facing(slot, 1.0);
            self.game.set_player_pos(slot, Vec3::ZERO);
            self.game.set_player_unk4d(slot, count as i8);
            self.player_create_demo(slot, slot + 0xA);
            let g = self.game.player_entity(slot);
            self.game.vi.v0501.kirbys[i - 1] = g;
            let j = self.root_jobj(g.expect("a Kirby"));
            jobj::req_anim_all(&mut self.game.objs, Some(j), 140.0);
            self.anim_all(j);
            let t = self.game.objs.jobjs[j].translate;
            // `getScale`.
            let s = fp::fmuls(f64::from(1.428_571_5_f32), reg(self.game.vi.v0501.scale));
            let p = Vec3::new(
                mem(fp::fmuls(reg(t.x), s)),
                mem(fp::fmuls(reg(t.y), s)),
                mem(fp::fmuls(reg(t.z), s)),
            );
            self.game.gr.start_points[i] = p;
            jobj::req_anim_all(&mut self.game.objs, Some(j), 0.0);
        }
        let a = &mut self.game.audio;
        a.lb.unwant_banks(0x1C);
        a.lb.want_banks(0xC, 0x800_0000_4000);
        a.lb_start_bank_loads(self.data);
        self.game.audio.lb_wait_for_banks(self.data);
    }

    /// The camera's process, `fn_8031DD14`: its animation; at frames 60, 70 and 85 a rumble
    /// and the Kirbys' landing sounds, at 98, 108 and 123 another sound, and at its last
    /// frame the scene's end.
    #[doc(alias = "fn_8031DD14")]
    pub fn vi0501_camera_proc(&mut self, gobj: GObjId) {
        let c = self.vi_cobj(gobj);
        crate::hsd::cobj::anim(&mut self.game.objs, self.data, Some(c));
        let frame = |r: &Self| f64::from(r.vi_cobj_frame(c).0);
        let f = frame(self);
        if f == 60.0 || f == 70.0 || f == 85.0 {
            self.vi_rumble(0xC, 0);
            self.sfx_play(0x222F9, 0x7F, 0x40);
            if frame(self) == 60.0 {
                self.sfx_play(0x73, 0x7F, 0x40);
            }
            if frame(self) == 70.0 {
                self.sfx_play(0x74, 0x7F, 0x40);
            }
            if frame(self) == 85.0 {
                self.sfx_play(0x73, 0x7F, 0x40);
            }
        }
        let f = frame(self);
        if f == 98.0 || f == 108.0 || f == 123.0 {
            self.sfx_play(0x22308, 0x7F, 0x40);
        }
        let (cur, end) = self.vi_cobj_frame(c);
        if cur == end {
            self.vi_end();
        }
    }

    /// The scene's entry, `vi0501_Scene_OnEnter`: the music stopped, the effects, the scene's
    /// files, its fog, lights, camera and models (on the main menu's model process), the
    /// stage and players, and two sounds.
    #[doc(alias = "vi0501_Scene_OnEnter")]
    #[doc(alias = "vi_8031DC80")]
    pub fn vi0501_on_enter(&mut self) {
        let desc = super::enter_desc(self.game, 0x14);
        self.lb_audio_music_stop();
        super::super::ef::eflib::init(self);
        super::super::ef::load_sync(self, 0);
        let ckind = desc[0];
        let archive = self.vi_load("Vi0501.dat");
        let scene = super::scene_symbol(
            self,
            archive.archive.expect("Vi0501.dat"),
            "Vi0501.dat",
            "visual0501Scene",
        );
        let waits = self.vi_load(super::wait_file(ckind));
        let v = &mut self.game.vi.v0501;
        v.scene = Some(scene);
        v.archive = archive;
        v.waits = waits;
        let erase = self.vi_fog_with(scene, 0xB);
        self.game.vi.v0501.erase = erase;
        self.vi_lights(scene);
        self.vi_camera(scene, CAMERA_RENDER, 5, CAMERA_PROC);
        self.vi_models(scene, 9, super::super::mn::main::BACK_PROC, |_, _, _| {});
        self.vi0501_setup(
            i32::from(desc[0]),
            i32::from(desc[1]),
            i32::from(desc[3]),
            [desc[4], desc[5], desc[6]],
        );
        self.sfx_play(0x20B, 0x7F, 0x40);
        self.sfx_play(0x20C, 0x7F, 0x40);
    }
}
