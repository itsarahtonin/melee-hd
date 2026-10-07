//! The game's state: HSD's objects and scheduler, and Melee's own objects on top.
//!
//! The original's function pointers (GObj processes, render callbacks, the callbacks fighters
//! and items keep) are [`CodeRef`]s, the original function's address. [`code`](super::code)
//! dispatches them to their ports.

use melee_data::archive::Archives;

use super::handles::{CodeRef, FighterId, ItemId};
use super::structs::Fighter;
use crate::hsd::Objects;
use crate::hsd::gobj::{GObjId, Host, Kinds, World};
use crate::hsd::jobj::JObjId;
use crate::hsd::pool::Pool;

/// What a GObj's user data is (`user_data`, by its `user_data_kind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserData {
    Fighter(FighterId),
    Item(ItemId),
}

/// What a GObj's HSD object is (`hsd_obj`, by its `obj_kind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Obj {
    JObj(JObjId),
}

/// The game's types for the scheduler.
#[derive(Clone, Debug)]
pub struct GameKinds;

impl Kinds for GameKinds {
    type Proc = CodeRef;
    type Render = CodeRef;
    type UserData = UserData;
    type Obj = Obj;
}

/// Everything a tick reads and writes.
#[derive(Clone, Debug, Default)]
pub struct Game {
    pub world: World<GameKinds>,
    pub objs: Objects,
    pub fighters: Pool<Fighter>,
}

/// The game as the scheduler's host for one call: the state and the game data it reads.
pub struct Running<'a> {
    pub game: &'a mut Game,
    pub data: &'a Archives,
}

impl Host<GameKinds> for Running<'_> {
    fn world(&mut self) -> &mut World<GameKinds> {
        &mut self.game.world
    }

    fn invoke(&mut self, gobj: GObjId, proc: CodeRef) {
        super::code::run_proc(self, gobj, proc);
    }

    fn remove_user_data(&mut self, _gobj: GObjId, _kind: u8, data: UserData) {
        panic!("removing {data:?} needs its port");
    }

    fn remove_obj(&mut self, _kind: u8, obj: Obj) {
        match obj {
            Obj::JObj(j) => crate::hsd::jobj::remove_all(&mut self.game.objs, Some(j)),
        }
    }
}
