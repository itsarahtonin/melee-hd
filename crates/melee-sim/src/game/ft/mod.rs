//! Fighters (`melee/ft`): the per-frame processes every fighter runs (`fighter.c`), animation
//! playback (`ftanim.c`), parts and joint lookups (`ftparts.c`), the library helpers other
//! code asks of fighters (`ftlib.c`), and the action states' callbacks.
//!
//! A fighter is the generated [`Fighter`] mirror in `Game::fighters`, reached from its GObj's
//! user data. Data the original reads from its loaded files (`PlCo.dat`'s common data, a
//! character's `ftData`) is read through `DataRef`s, at the offsets the decomp's structs give.

pub mod action;
pub mod anim;
pub mod anim_buf;
pub mod attack;
pub mod bind;
pub mod camera;
pub mod captain;
pub mod clink;
pub mod co;
pub mod colanim;
pub mod coll;
pub mod commands;
pub mod common;
pub mod counted;
pub mod cpu;
pub mod cpu_ai;
pub mod cpu_attack;
pub mod cpu_decide;
pub mod cpu_move;
pub mod cpu_recover;
pub mod cpu_script;
pub mod create;
pub mod create_model;
pub mod data;
pub mod donkey;
pub mod draw;
pub mod drmario;
pub mod dynamics;
pub mod effect;
pub mod emblem;
pub mod falco;
pub mod fighter;
pub mod fox;
pub mod ftlib;
pub mod ik;
pub mod input;
mod kind_tables;
pub mod kinds;
pub mod koopa;
pub mod light;
pub mod link;
pub mod luigi;
pub mod mario;
pub mod mars;
pub mod motion;
mod motion_tables;
pub mod ness;
pub mod parts;
pub mod peach;
pub mod phys;
pub mod pichu;
pub mod pikachu;
pub mod popo;
pub mod purin;
pub mod resolve;
pub mod samus;
pub mod seak;
pub mod second;
pub mod sound;
pub mod spawn;
pub mod transform;
pub mod vars;
pub mod yoshi;
pub mod zelda;

use melee_data::archive::{ArchiveId, Archives, DataRef};

use super::handles::FighterId;
use super::state::{Game, Running, UserData};
use super::structs::Fighter;
use crate::hsd::gobj::GObjId;

/// The fighter code's globals that point into game data or hold settings.
#[derive(melee_derive::CloneFrom, Debug, Default)]
pub struct Globals {
    /// `PlCo.dat`'s `ftLoadCommonData`, which `Fighter_LoadCommonData` copies into
    /// `p_ftCommonData`, `ftPartsTable` and the other common pointers.
    pub load_common: Option<DataRef>,
    /// Each character's animation file once the game has copied it to ARAM
    /// (`ftData_Table_Unk0`), by kind.
    pub anim_files: Vec<Option<AnimFile>>,
    /// The kinds' data, costumes and loaded files (`ftdata.c`'s globals).
    pub data: data::DataGlobals,
    /// The next spawn number (`Fighter_804D64F8`).
    pub spawn_counter: u32,
    /// The first free dynamics link (`lb_00F9.c`'s `cur_data`); each link's `next` leads to
    /// the one after it.
    pub dynamics_free: Option<super::handles::DynamicsId>,
    /// The fighters' light and its first ambient light (`ftCo_09F4.c`'s `lobj0`, `lobj1`).
    pub lobj0: Option<crate::hsd::lobj::LObjId>,
    pub lobj1: Option<crate::hsd::lobj::LObjId>,
    /// The fighters' metal and clear materials (`ft_804D6580`, `ft_804D6588`).
    pub metal_mobj: Option<crate::hsd::mobj::MObjId>,
    pub clear_mobj: Option<crate::hsd::mobj::MObjId>,
    /// Kirby's copied costumes (`ft_80459B88`, `ftKb_Init_803C9FC8`).
    pub kirby: KirbyCostumes,
    /// Jigglypuff's hats (`ft_8045A1E0`).
    pub purin_hats: [Option<DataRef>; 5],
    /// Each fighter's address in the original (`Fighter*`, its user data), by slot: a few
    /// places read it as a number (Nana's computer DI takes its low byte), so it is state,
    /// recorded when `Fighter_Create` allocates the fighter (`docs/engine.md`).
    #[clone_from(if_changed)]
    pub addresses: std::collections::BTreeMap<u32, u32>,
    /// The addresses the original's allocator gives the next fighters, in creation order,
    /// as the frontend seeds them (from ssbm-rs's trace); a fighter created without one
    /// records 0. An input until an allocator model computes them.
    pub address_seed: std::collections::VecDeque<u32>,
    /// The attack collision's scratch (`ftcoll.c`'s damage logs and clank flags), which each
    /// fighter's attack collision starts over.
    pub hit_logs: attack::HitLogs,
    /// Which lines are platforms, for a filtered collision pass's floor filter: a buffer
    /// each such pass fills and empties.
    pub platforms: Vec<bool>,
}

/// The fighter kinds whose costumes Kirby can copy (`ftKb_Init_803C9FC8`'s entries).
pub const KIRBY_COSTUME_KINDS: [usize; 5] = [3, 15, 16, 22, 24];

/// Kirby's copies of other characters' costumes: a count per kind (`ft_80459B88`), and for
/// each kind Kirby copies a hat from, six models and material animations.
#[derive(melee_derive::CloneFrom, Debug, PartialEq)]
pub struct KirbyCostumes {
    pub counts: [i32; data::KIND_COUNT],
    pub models: [[(Option<DataRef>, Option<DataRef>); 6]; 5],
}

impl Default for KirbyCostumes {
    fn default() -> Self {
        KirbyCostumes {
            counts: [0; data::KIND_COUNT],
            models: [[(None, None); 6]; 5],
        }
    }
}

impl KirbyCostumes {
    /// `ftKb_Init_800EE528`.
    #[doc(alias = "ftKb_Init_800EE528")]
    pub fn reset(&mut self) {
        self.counts = [0; data::KIND_COUNT];
        self.models.fill([(None, None); 6]);
    }
}

/// A character's animation file in ARAM: one archive per animation, which a fighter copies
/// into its animation buffer when it starts that animation (`ftData_80085CD8`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq)]
pub struct AnimFile {
    /// Where in ARAM the game put it. Fighters keep which animation they last loaded as its
    /// ARAM address (`Fighter::x5A4`).
    pub aram: u32,
    /// Its animations by offset in the file, as `Fighter_WaitAnimData::x4` gives them.
    pub anims: Vec<(u32, ArchiveId)>,
}

impl Game {
    /// The fighter a GObj's user data holds.
    pub fn fighter_id(&self, gobj: GObjId) -> FighterId {
        match self.world.gobjs[gobj.0].user_data {
            Some(UserData::Fighter(f)) => f,
            other => panic!("GObj {gobj:?} holds {other:?}, not a fighter"),
        }
    }

    pub fn fighter(&self, gobj: GObjId) -> &Fighter {
        &self.fighters[self.fighter_id(gobj).0]
    }

    pub fn fighter_mut(&mut self, gobj: GObjId) -> &mut Fighter {
        let f = self.fighter_id(gobj);
        &mut self.fighters[f.0]
    }
}

impl<'a> Running<'a> {
    /// `p_ftCommonData`.
    pub fn common(&self) -> Common<'a> {
        Common {
            data: self.data,
            at: self.load_common(0),
        }
    }
}

impl Running<'_> {
    pub fn fighter(&self, gobj: GObjId) -> &Fighter {
        self.game.fighter(gobj)
    }

    pub fn fighter_mut(&mut self, gobj: GObjId) -> &mut Fighter {
        self.game.fighter_mut(gobj)
    }

    /// `ftLoadCommonData`'s pointer at `offset`.
    pub(super) fn load_common(&self, offset: u32) -> DataRef {
        let base = self
            .game
            .ft
            .load_common
            .expect("PlCo.dat's common data isn't loaded");
        self.data
            .ptr(base.at(offset))
            .expect("a null common data pointer")
    }

    /// The pointer at `offset` in `ftLoadCommonData`, which `Fighter_LoadCommonData` copies
    /// into a global (x30 is `Fighter_804D6524`, x34 `Fighter_804D6520`, x38
    /// `Fighter_804D651C`).
    pub fn common_ptr(&self, offset: u32) -> DataRef {
        self.load_common(offset)
    }

    /// `ftPartsTable[kind]`.
    pub fn parts_table(&self, kind: i32) -> PartsTable<'_> {
        let table = self.load_common(0x10);
        PartsTable {
            data: self.data,
            at: self
                .data
                .ptr(table.at(4 * kind as u32))
                .expect("a character without a parts table"),
        }
    }
}

/// Where `ftLoadCommonData` keeps the stale-move weights (`Fighter_804D6548`).
pub const STALE_WEIGHTS: u32 = 0xC;
/// Where `ftLoadCommonData` keeps the scale parameters (`Fighter_804D6524`).
pub const SCALE_PARAMS: u32 = 0x30;

/// One of the pointers `Fighter_LoadCommonData` copies out of `ftLoadCommonData`, by its
/// offset there.
pub fn common_ptr(game: &Game, data: &Archives, offset: u32) -> Option<DataRef> {
    data.ptr(game.ft.load_common?.at(offset))
}

/// `ftCommonData`, the constants all fighters share.
pub struct Common<'a> {
    data: &'a Archives,
    at: DataRef,
}

impl Common<'_> {
    pub fn f32(&self, offset: u32) -> f32 {
        self.data.f32(self.at.at(offset))
    }

    /// Where `offset` into the common data is.
    pub fn at(&self, offset: u32) -> DataRef {
        self.at.at(offset)
    }

    pub fn i32(&self, offset: u32) -> i32 {
        self.data.u32(self.at.at(offset)) as i32
    }

    pub fn u32(&self, offset: u32) -> u32 {
        self.data.u32(self.at.at(offset))
    }
}

/// `FighterPartsTable`: a character's map between its joints and the common part names.
pub struct PartsTable<'a> {
    data: &'a Archives,
    at: DataRef,
}

impl PartsTable<'_> {
    /// `part_to_joint[part]`.
    pub fn joint_of(&self, part: u32) -> u8 {
        let t = self.data.ptr(self.at.at(4)).expect("part_to_joint");
        self.data.u8(t.at(part))
    }

    /// `joint_to_part[joint]`.
    pub fn part_of(&self, joint: u32) -> u8 {
        let t = self.data.ptr(self.at).expect("joint_to_part");
        self.data.u8(t.at(joint))
    }

    /// `parts_num`.
    pub fn len(&self) -> u32 {
        self.data.u32(self.at.at(8))
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// An action state as the game's tables list it (`MotionState`): its animation, flags, move
/// id and callbacks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionState {
    pub anim_id: i32,
    pub flags: u32,
    /// The move id (top byte) and state bits (`MotionState_x8`).
    pub x8: u32,
    pub anim_cb: Option<super::handles::CodeRef>,
    pub input_cb: Option<super::handles::CodeRef>,
    pub phys_cb: Option<super::handles::CodeRef>,
    pub coll_cb: Option<super::handles::CodeRef>,
    pub cam_cb: Option<super::handles::CodeRef>,
}

/// The action-state table at `addr` in the executable (`x1C_actionStateList` and
/// `x20_actionStateList` hold such addresses).
pub fn motion_table(addr: u32) -> &'static [MotionState] {
    motion_tables::TABLES
        .iter()
        .find(|t| t.0 == addr)
        .unwrap_or_else(|| panic!("no action-state table at {addr:#x}"))
        .1
}
