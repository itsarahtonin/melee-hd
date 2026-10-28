//! Demo fighters (`ftdemo.c`, and the player library's setup of them in `player.c`): a
//! fighter that only poses, the results screen's, made like a match's
//! ([`Running::fighter_create`]) but from its character's demo animations (its results
//! file's), with a demo action-state table and fewer processes, and its pose's first state
//! (`ft_0BEC.c`, `ft_0BEF.c`: the winners' three poses, with Fox's blaster, Dr. Mario's pills
//! and Ness's yo-yo in hand, and the losers' clapping).

use melee_data::archive::ArchiveId;

use super::super::handles::{CodeRef, FighterId};
use super::super::pl::player::{PKIND_DEMO, mapping};
use super::super::state::{Running, UserData};
use super::super::structs::Fighter;
use super::create::AllocInfo;
use super::data::kind_tables;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;
use gekko_fp as fp;

/// `FtPart_RThumbNb`: the joint a pose's item is held at.
const PART_R_THUMB_NB: i32 = 49;

/// The demo action-state table (`ftData_803C52A0`) and its states' count (`x18`).
const DEMO_STATE_LIST: u32 = 0x803C_52A0;
const DEMO_STATES: i32 = 14;

/// Each fighter kind's demo states of its own (`ftData_UnkMotionStates0`): Mario's,
/// Kirby's, Luigi's and Giga Bowser's.
fn demo_kind_states(kind: i32) -> u32 {
    match kind {
        0 => 0x803C_7260,
        4 => 0x803C_A04C,
        0x11 => 0x803D_0868,
        0x1F => 0x803D_38C8,
        _ => 0,
    }
}

/// `ftData_UnkIntPairs[kind].count`: each kind's demo animations.
pub fn demo_anim_count(kind: i32) -> u32 {
    match kind {
        0 | 0x11 => 16,
        4 => 18,
        0x1F => 15,
        _ => 14,
    }
}

/// The demo processes and their priorities, as `ftDemo_CreateFighter` sets them up.
const DEMO_PROCS: [(u32, u8); 5] = [
    (0x8006_A360, 1),  // Fighter_procAnim
    (0x8006_B82C, 4),  // Fighter_procUpdate
    (0x8006_C27C, 5),  // Fighter_procMap
    (0x8006_C80C, 9),  // Fighter_procCollPos
    (0x8006_D9AC, 16), // Fighter_procDynamics
];

/// `ftData_803C2468[kind]->result_filename`'s character: each kind's results animations'
/// symbol in its results file (`ftDemoResultMotionFile` and the name).
const RESULT_NAMES: [&str; 27] = [
    "Mario",
    "Fox",
    "Captain",
    "Donkey",
    "Kirby",
    "Koopa",
    "Link",
    "Seak",
    "Ness",
    "Peach",
    "Popo",
    "Nana",
    "Pikachu",
    "Samus",
    "Yoshi",
    "Purin",
    "Mewtwo",
    "Luigi",
    "Mars",
    "Zelda",
    "Clink",
    "Drmario",
    "Falco",
    "Pichu",
    "Gamewatch",
    "Ganon",
    "Emblem",
];

/// A kind's results animations' symbol.
pub fn result_symbol(kind: i32) -> Option<String> {
    RESULT_NAMES
        .get(kind as usize)
        .map(|n| format!("ftDemoResultMotionFile{n}"))
}

/// A kind's demo animations, as the results file holds them: where the symbol's data is in
/// main memory (`ftData_UnkIntPairs[kind].data`) and each animation's archive, by offset.
#[derive(Clone, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct DemoFile {
    pub addr: u32,
    pub archive: Option<ArchiveId>,
}

/// What `ftDemo_CreateFighter` is asked to make (`plAllocInfo2`): the fighter kind, its slot,
/// the pose (`unk8`), and whether it's the slot's transformation.
#[derive(Clone, Copy, Debug)]
pub struct DemoAlloc {
    pub kind: i32,
    pub slot: u8,
    pub pose: i32,
    pub transformation: bool,
}

impl Running<'_> {
    /// A character's demo animations from its results file (`HSD_Archive`'s file at
    /// `file`), `Player_80036E20`: its kind's, and its second kind's where that isn't a
    /// transformation (Nana's).
    #[doc(alias = "Player_80036E20")]
    pub fn player_set_demo_archive(&mut self, ckind: i32, archive: ArchiveId, file: u32) {
        let m = mapping(ckind);
        self.demo_set_archive(i32::from(m.internal_id), archive, file);
        if m.extra_internal_id != -1 && m.has_transformation == 0 {
            self.demo_set_archive(i32::from(m.extra_internal_id), archive, file);
        }
    }

    /// A kind's demo animations' place, unless it has one, `ftDemo_SetArchiveData` (the
    /// results' table, `ftData_803C2468[kind][0]`).
    #[doc(alias = "ftDemo_SetArchiveData")]
    fn demo_set_archive(&mut self, kind: i32, archive: ArchiveId, file: u32) {
        let k = kind as usize;
        if self.game.ft.data.demo_anims[k] != 0 {
            return;
        }
        let sym = result_symbol(kind).expect("a kind without demo animations");
        let off = self
            .data
            .get(archive)
            .public(&sym)
            .unwrap_or_else(|| panic!("{sym} isn't in its results file"));
        // `HSD_ArchiveGetPublicAddress`: the data after the file's 0x20-byte header.
        self.game.ft.data.demo_anims[k] = file.wrapping_add(0x20).wrapping_add(off);
        self.game.ft.data.demo_files[k] = Some((archive, off));
    }

    /// Both of a slot's fighters to `pos` (`player_poses` too), `Player_80032A04`
    /// (`ftLib_SetPos` on each).
    #[doc(alias = "Player_80032A04")]
    #[doc(alias = "ftLib_SetPos")]
    pub fn player_set_position(&mut self, slot: i32, pos: Vec3) {
        for i in 0..2 {
            let p = self.game.player_mut(slot);
            let e = p.entity_index(i);
            p.set_pose(e, pos);
            if let Some(g) = p.player_entity[e] {
                self.game.fighter_mut(g).cur_pos = pos;
            }
        }
    }

    /// A slot's model scale, and its fighters' heights, `Player_SetScale`.
    #[doc(alias = "Player_SetScale")]
    pub fn player_set_scale(&mut self, slot: i32, scale: f32) {
        self.game.player_mut(slot).model_scale = scale;
        for i in 0..2 {
            let p = self.game.player(slot);
            if let Some(g) = p.player_entity[p.entity_index(i)] {
                self.set_scale_y(g, scale);
            }
        }
    }

    /// A slot's demo fighters in pose `pose`, `Player_80036F34`: its character's, and its
    /// second kind's where that isn't a transformation.
    #[doc(alias = "Player_80036F34")]
    pub fn player_create_demo(&mut self, slot: i32, pose: i32) {
        let ckind = self.game.player(slot).ckind;
        let m = mapping(ckind);
        let mut a = DemoAlloc {
            kind: i32::from(m.internal_id),
            slot: slot as u8,
            pose,
            transformation: false,
        };
        self.game.player_mut(slot).pkind = PKIND_DEMO;
        let g = self.demo_create_fighter(a);
        self.game.player_mut(slot).player_entity[0] = Some(g);
        if m.extra_internal_id != -1 && m.has_transformation == 0 {
            a.kind = i32::from(m.extra_internal_id);
            a.transformation = true;
            let g = self.demo_create_fighter(a);
            self.game.player_mut(slot).player_entity[1] = Some(g);
        }
    }

    /// A demo fighter, `ftDemo_CreateFighter`: made as a match's is, but with the demo
    /// action-state tables and the demo animations (the pose's range of them), five of the
    /// match's processes, and its pose's first state.
    #[doc(alias = "ftDemo_CreateFighter")]
    pub fn demo_create_fighter(&mut self, a: DemoAlloc) -> GObjId {
        let gobj = self.game.world.create(4, 8, 0);
        self.game
            .world
            .setup_gx_link(gobj, CodeRef(super::create::RENDER), 5, 0);
        let f = self.alloc_fighter_block();
        self.game
            .world
            .init_user_data(gobj, 4, UserData::Fighter(f));
        if let Some(file) = self.game.load_fighter_data(self.data, a.kind) {
            self.disc_load(file, super::data::MAIN_MEMORY);
        }
        // `initFighter`: `plAllocInfo` from the demo's, its `x6` bit the transformation.
        self.init_load(
            gobj,
            AllocInfo {
                kind: a.kind,
                slot: a.slot,
                x5: 0,
                sub: a.transformation,
                transformation: false,
            },
        );
        let fp = &mut self.game.fighters[f.0];
        let kind = fp.kind;
        let ft_data = fp.ft_data.expect("a fighter kind without data");
        fp.x18 = DEMO_STATES;
        fp.x1C_actionStateList = super::super::handles::RawPtr(DEMO_STATE_LIST);
        fp.x20_actionStateList = super::super::handles::RawPtr(demo_kind_states(kind));
        fp.x24 = self.data.ptr(ft_data.at(0x14));
        fp.x28 = self.data.ptr(ft_data.at(0x18));
        let effects = kind_tables(kind).effect_file;
        super::super::code::call(
            self,
            super::create::addr::EFASYNC_LOAD_SYNC,
            &[super::super::code::Val::Int(i32::from(effects))],
        );
        let costume = i32::from(self.game.fighters[f.0].costume_id);
        if let Some(file) = self.game.load_costume(self.data, kind, costume) {
            self.disc_load(&file, super::data::MAIN_MEMORY);
        }
        self.load_costume_joint(gobj);
        let (lo, hi) = match a.pose {
            p if p >= 9 => super::super::code::unported_call(&format!(
                "ftDemo_CreateFighter's pose {p} (ftData_UnkDemoCallbacks0)"
            )),
            8 => (13, 13),
            7 => (12, 12),
            5 | 6 => (10, 11),
            _ => (0, 9),
        };
        self.demo_anim_buffers(f, lo, hi);
        self.init_parts(f);
        self.setup_parts(gobj);
        self.costume_anims(gobj);
        self.load_second_model(gobj);
        self.set_depth_scale(f);
        self.init_parts_vis(gobj);
        self.game.reset_part_anims(f);
        self.init_bone_dynamics(f);
        self.load_blend_skeleton(gobj);
        self.measure_bones(f);
        self.create_own_light(f);
        self.on_load(gobj, super::create_model::Creator::Demo);
        self.init_hurt_capsules(f);
        let cam_box = super::super::camera::alloc_box(&mut self.game.camera.boxes, 0);
        self.game.fighters[f.0].x890_cameraBox = Some(cam_box);
        let root = self.root_jobj(gobj);
        self.create_shadow(f, root);
        for (proc, priority) in DEMO_PROCS {
            self.game.world.setup_proc(gobj, CodeRef(proc), priority);
        }
        self.fighter_spawn(gobj);
        self.demo_enter(gobj, a.pose);
        self.game.ft_disable_input(gobj);
        gobj
    }

    /// A demo fighter's animation buffers and the demo animations `lo` to `hi`, placed in
    /// its results file's (`ftData_UnkIntPairs`'s address plus each animation's offset,
    /// which the original writes into its character's data), `ftData_80085B98`. The kind's
    /// address is then forgotten, so its next fighter keeps the places.
    #[doc(alias = "ftData_80085B98")]
    fn demo_anim_buffers(&mut self, f: FighterId, lo: usize, hi: usize) {
        let kind = self.game.fighters[f.0].kind;
        let base = self.game.ft.data.demo_anims[kind as usize];
        self.alloc_anim_buffers(f);
        let count = demo_anim_count(kind);
        let fp = &mut self.game.fighters[f.0];
        fp.x5A4 = 0;
        fp.x5A8 = 0;
        fp.x58C = count;
        assert!(
            (hi as u32) < count,
            "the original's assertion failed: Demo Status error! {hi}"
        );
        if base != 0 {
            let list = fp.x24.expect("a fighter without demo animations");
            for i in lo..=hi {
                let e = list.at(0x18 * i as u32);
                let size = self.data.u32(e.at(8));
                if size != 0 {
                    assert!(
                        size <= 0xB000,
                        "the original's assertion failed: fighter figatree over! {size:#x}"
                    );
                    let at = base.wrapping_add(self.data.u32(e.at(4)));
                    self.game.ft.data.demo_x14[kind as usize][i] = at;
                }
            }
            self.game.ft.data.demo_anims[kind as usize] = 0;
        }
    }

    /// A demo fighter's pose's first state (`ftDemo_CreateFighter`'s `on_create_fighter`):
    /// the three winners' (`ftCo_800BECB0`, `ftCo_800BED88`, `ftCo_800BEF04`), the losers'
    /// (`ftCo_800BEFD4`); the other demos' aren't ported.
    fn demo_enter(&mut self, gobj: GObjId, pose: i32) {
        match pose {
            0 => self.demo_win(gobj, 0),
            1 => self.demo_win(gobj, 2),
            2 => self.demo_win(gobj, 5),
            3 => {}
            4 => {
                // `ftCo_800BEFD4`: `ftCo_MS_DeadUpFallIce`.
                self.change_motion_state(gobj, 9, 0, 0.0, 1.0, 0.0, None);
                let fp = self.game.fighter_mut(gobj);
                fp.x2219_b2 = true;
                fp.x2219_b1 = true;
            }
            p => super::super::code::unported_call(&format!(
                "ftDemo_CreateFighter's pose {p}'s first state (on_create_fighter)"
            )),
        }
    }

    /// A winner's pose: its state, the pose's flags (`setupInitialState`), and what the
    /// character holds in it (`ftCo_800BECB0`, `ftCo_800BED88`, `ftCo_800BEF04`): Fox his
    /// blaster, Dr. Mario two pills (not ported), Ness his bat.
    #[doc(alias = "ftCo_800BECB0")]
    #[doc(alias = "ftCo_800BED88")]
    #[doc(alias = "ftCo_800BEF04")]
    fn demo_win(&mut self, gobj: GObjId, msid: i32) {
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.x2219_b2 = true;
        fp.x2219_b1 = true;
        fp.x2228_b1 = true;
        fp.item_gobj = None;
        fp.x1984_heldItemSpec = None;
        let kind = fp.kind;
        match (msid, kind) {
            // Fox's blaster (the first two poses), at his right thumb, his size.
            (0 | 2, 1) => {
                let f = self.game.fighter_id(gobj);
                let fp = &self.game.fighters[f.0];
                let part = self.bone_index(fp, PART_R_THUMB_NB);
                let kind = self.fox_attr_i32(f, super::fox::attr::BLASTER_GUN_ITKIND);
                let item = super::super::it::kinds::foxblaster::spawn_demo(self, gobj, part, kind);
                self.game.fighters[f.0].item_gobj = item;
                let fp = &self.game.fighters[f.0];
                let s = mem(fp::fmuls(
                    reg(fp.x34_scale.y),
                    reg(fp.co_attrs.model_scaling),
                ));
                if let Some(i) = item {
                    super::super::it::lib26b1::set_scale(self.game, self.data, i, s);
                }
            }
            // Dr. Mario's two pills (the second pose).
            (2, 0x15) => {
                super::super::code::unported_call("ftCo_800BED88's pills (itDrMarioPill_802C09C4)")
            }
            // Ness's bat (the third pose), at his right thumb, 0.8 his size.
            (5, 8) => {
                let f = self.game.fighter_id(gobj);
                let fp = &self.game.fighters[f.0];
                let part = self.bone_index(fp, PART_R_THUMB_NB);
                let item = super::super::it::kinds::nessbat::spawn_demo(self, gobj, part);
                self.game.fighters[f.0].item_gobj = item;
                let fp = &self.game.fighters[f.0];
                let s = fp::fmuls(reg(fp.x34_scale.y), reg(fp.co_attrs.model_scaling));
                let s = mem(fp::fmuls(f64::from(0.8_f32), s));
                if let Some(i) = item {
                    super::super::it::lib26b1::set_scale(self.game, self.data, i, s);
                }
            }
            _ => {}
        }
    }
}

impl Fighter {
    /// Whether the fighter is a demo's (its action states the demo table's).
    pub fn is_demo(&self) -> bool {
        self.x1C_actionStateList.0 == DEMO_STATE_LIST
    }
}

/// The animation callbacks of the common motion states 2 and 5 (`ftCo_800BEF00`,
/// `ftCo_800BEFD0`).
pub const DEMO_ANIM_2: u32 = 0x800B_EF00;
pub const DEMO_ANIM_5: u32 = 0x800B_EFD0;
/// The common motion state 3's (`ftCo_800BED84`).
pub const DEMO_ANIM_3: u32 = 0x800B_ED84;

/// Whether a fighter is frozen in a KO toward the screen or asleep, `ftCo_800BF228`: the
/// states a results pose's held item follows its size in.
#[doc(alias = "ftCo_800BF228")]
pub fn frozen_or_asleep(game: &crate::game::state::Game, gobj: GObjId) -> bool {
    let m = game.fighter(gobj).motion_id;
    m == super::co::ids::ms::DEAD_UP_FALL_HIT_CAMERA_ICE || m == super::co::ids::ms::SLEEP
}

/// `ftCo_800BEF00`, `ftCo_800BED84` and `ftCo_800BEFD0`: empty.
#[doc(alias = "ftCo_800BEF00")]
#[doc(alias = "ftCo_800BED84")]
#[doc(alias = "ftCo_800BEFD0")]
pub fn demo_anim() {}
