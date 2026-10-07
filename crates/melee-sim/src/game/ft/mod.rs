//! Fighters (`melee/ft`): the per-frame processes every fighter runs (`fighter.c`), animation
//! playback (`ftanim.c`), parts and joint lookups (`ftparts.c`), the library helpers other
//! code asks of fighters (`ftlib.c`), and the action states' callbacks.
//!
//! A fighter is the generated [`Fighter`] mirror in `Game::fighters`, reached from its GObj's
//! user data. Data the original reads from its loaded files (`PlCo.dat`'s common data, a
//! character's `ftData`) is read through `DataRef`s, at the offsets the decomp's structs give.

pub mod action;
pub mod anim;
pub mod bind;
pub mod co;
pub mod colanim;
pub mod coll;
pub mod commands;
pub mod common;
pub mod dynamics;
pub mod fighter;
pub mod ftlib;
pub mod input;
pub mod motion;
mod motion_tables;
pub mod parts;
pub mod phys;

use melee_data::archive::{ArchiveId, Archives, DataRef};

use super::handles::FighterId;
use super::state::{Game, Running, UserData};
use super::structs::Fighter;
use crate::hsd::gobj::GObjId;

/// The fighter code's globals that point into game data or hold settings.
#[derive(Clone, Debug, Default)]
pub struct Globals {
    /// `PlCo.dat`'s `ftLoadCommonData`, which `Fighter_LoadCommonData` copies into
    /// `p_ftCommonData`, `ftPartsTable` and the other common pointers.
    pub load_common: Option<DataRef>,
    /// How many stage hazards blow fighters around (`ft_804D6578`'s count); none is ported.
    pub wind_sources: u32,
    /// How many stage devices can bury fighters (`ftDevice_BuryThingCount`); none is ported.
    pub bury_things: u32,
    /// Each character's animation file once the game has copied it to ARAM
    /// (`ftData_Table_Unk0`), by kind.
    pub anim_files: Vec<Option<AnimFile>>,
}

/// A character's animation file in ARAM: one archive per animation, which a fighter copies
/// into its animation buffer when it starts that animation (`ftData_80085CD8`).
#[derive(Clone, Debug, Default, PartialEq)]
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

impl Running<'_> {
    pub fn fighter(&self, gobj: GObjId) -> &Fighter {
        self.game.fighter(gobj)
    }

    pub fn fighter_mut(&mut self, gobj: GObjId) -> &mut Fighter {
        self.game.fighter_mut(gobj)
    }

    /// `ftLoadCommonData`'s pointer at `offset`.
    fn load_common(&self, offset: u32) -> DataRef {
        let base = self
            .game
            .ft
            .load_common
            .expect("PlCo.dat's common data isn't loaded");
        self.data
            .ptr(base.at(offset))
            .expect("a null common data pointer")
    }

    /// `p_ftCommonData`.
    pub fn common(&self) -> Common<'_> {
        Common {
            data: self.data,
            at: self.load_common(0),
        }
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

/// `ftCommonData`, the constants all fighters share.
pub struct Common<'a> {
    data: &'a Archives,
    at: DataRef,
}

impl Common<'_> {
    pub fn f32(&self, offset: u32) -> f32 {
        self.data.f32(self.at.at(offset))
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
