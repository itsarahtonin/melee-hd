//! The stage's archives, `melee/gr/grdatfiles.c`: a table of up to four loaded archives and
//! their `map_head`s (`UnkStageDat`), which list the stage's models by map id.

use melee_data::archive::{Archives, DataRef};

use super::super::state::Game;
use super::StageArchive;

/// A `map_head` (`UnkStageDat`).
#[derive(Clone, Copy, Debug, melee_derive::StateEq)]
pub struct MapHead(pub DataRef);

/// A `map_head`'s entry for one map id (`UnkStageDat_x8_t`, 0x34 bytes).
#[derive(Clone, Copy, Debug, melee_derive::StateEq)]
pub struct MapEntry(pub DataRef);

impl MapHead {
    /// The joint-index tables (`unk0`, `unk4` entries of 0xC bytes).
    pub fn index_tables(self, data: &Archives) -> (Option<DataRef>, i32) {
        (data.ptr(self.0), data.u32(self.0.at(4)) as i32)
    }

    /// How many map ids it has entries for (`unkC`).
    pub fn count(self, data: &Archives) -> i32 {
        data.u32(self.0.at(0xC)) as i32
    }

    /// The entry for `map_id` (`unk8[map_id]`).
    pub fn entry(self, data: &Archives, map_id: i32) -> MapEntry {
        let entries = data.ptr(self.0.at(8)).expect("a map_head without entries");
        MapEntry(entries.at(0x34 * map_id as u32))
    }

    /// The light overrides (`unk18`, `unk1C`).
    pub fn light_overrides(self, data: &Archives) -> (Option<DataRef>, i32) {
        (data.ptr(self.0.at(0x18)), data.u32(self.0.at(0x1C)) as i32)
    }

    /// The shadow entries (`unk20`, `unk24`).
    pub fn shadows(self, data: &Archives) -> (Option<DataRef>, i32) {
        (data.ptr(self.0.at(0x20)), data.u32(self.0.at(0x24)) as i32)
    }
}

impl MapEntry {
    /// The model (`unk0`, an `HSD_Joint`).
    pub fn joint(self, data: &Archives) -> Option<DataRef> {
        data.ptr(self.0)
    }

    /// The animation tables (`unk4`, `unk8`, `unkC`): AnimJoint, MatAnimJoint and
    /// ShapeAnimJoint lists.
    pub fn anims(self, data: &Archives, which: u32) -> Option<DataRef> {
        data.ptr(self.0.at(4 + 4 * which))
    }

    /// Its camera (`x10`), lights (`x18`) and fog (`x1C`).
    pub fn camera(self, data: &Archives) -> Option<DataRef> {
        data.ptr(self.0.at(0x10))
    }

    pub fn lights(self, data: &Archives) -> Option<DataRef> {
        data.ptr(self.0.at(0x18))
    }

    pub fn fog(self, data: &Archives) -> Option<DataRef> {
        data.ptr(self.0.at(0x1C))
    }

    /// Its collision joints (`unk20`, `unk24`): `GrJoint` entries of three s16s.
    pub fn coll_joints(self, data: &Archives) -> (Option<DataRef>, i32) {
        (data.ptr(self.0.at(0x20)), data.u32(self.0.at(0x24)) as i32)
    }

    /// Per animation, whether it loops (`x28`, bytes), if the entry has them.
    pub fn loop_flags(self, data: &Archives) -> Option<DataRef> {
        data.ptr(self.0.at(0x28))
    }

    /// Joints whose display objects are set apart (`x2C`, s16 joint indices; `x30` of them).
    pub fn set_apart(self, data: &Archives) -> (Option<DataRef>, i32) {
        (data.ptr(self.0.at(0x2C)), data.u32(self.0.at(0x30)) as i32)
    }
}

/// A `GrJoint`: a collision joint (`x`), the map id it belongs to (`y`), and the model joint
/// it follows (`z`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct GrJoint {
    pub x: i16,
    pub y: i16,
    pub z: i16,
}

impl GrJoint {
    pub fn read(data: &Archives, at: DataRef) -> Self {
        GrJoint {
            x: data.u16(at) as i16,
            y: data.u16(at.at(2)) as i16,
            z: data.u16(at.at(4)) as i16,
        }
    }
}

/// Forgets every archive, `grDatFiles_801C6288`.
#[doc(alias = "grDatFiles_801C6288")]
pub fn clear(game: &mut Game) {
    game.gr.archives = Default::default();
}

/// The stage's own archive, the table's first entry (`grDatFiles_GetArchive`).
#[doc(alias = "grDatFiles_GetArchive")]
pub fn main_archive(game: &Game) -> &StageArchive {
    game.gr.archives[0]
        .as_ref()
        .expect("the stage's archive isn't loaded")
}

/// The main archive's `map_head`.
pub fn main_head(game: &Game) -> MapHead {
    MapHead(
        main_archive(game)
            .map_head
            .expect("a stage archive without map_head"),
    )
}

/// The archive whose `map_head` has a model for `map_id`, `grDatFiles_801C6330`.
#[doc(alias = "grDatFiles_801C6330")]
pub fn archive_for(game: &Game, data: &Archives, map_id: i32) -> Option<MapHead> {
    if map_id < 0 {
        return None;
    }
    game.gr.archives.iter().flatten().find_map(|a| {
        let head = MapHead(a.map_head?);
        (head.count(data) > map_id && head.entry(data, map_id).joint(data).is_some())
            .then_some(head)
    })
}
