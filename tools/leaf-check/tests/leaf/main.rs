//! leaf-check's checks, one test binary (the refactor's task 26): one link where there were
//! a binary per family. Each family is a module and its tests' names start with it, so a
//! family runs as `cargo test -p leaf-check --test leaf -- NAME::` where it ran as
//! `--test NAME` (docs/oracle.md, "leaf-check"; `LEAF_CHECK_SHARD` splits one as before).
//! The modules the families share are `support`'s, compiled once.
// `mpcoll` takes in `mplib`'s query, mutation and state modules and `support/mpstage.rs` as its
// own children (their `super::` is each family's own stage setup), as it did as its own binary.
#![allow(clippy::duplicate_mod)]

mod aobj;
mod archive;
mod audio;
mod bowser;
mod bytecode;
mod camera;
mod captain;
mod capture;
mod card;
mod cobj;
mod common_stops;
mod cpu_vs;
mod display;
mod donkey;
mod drmario;
mod effects;
mod falco;
mod fighter;
mod fighter_create;
mod fighter_procs;
mod fobj;
mod fog;
mod fox;
mod ftcommon;
mod ftcommon_entries;
mod game_and_watch;
mod ganon;
mod gobj;
mod hanyu;
mod heap;
mod hsd_mtx;
mod hud;
mod ice;
mod id;
mod input;
mod items;
mod items_coin;
mod items_held;
mod items_leftovers;
mod items_monsters;
mod items_on;
mod items_pokemon;
mod items_powerups;
mod items_shoot;
mod jobj;
mod jpeg;
mod jpeg_decode;
mod kirby;
mod lbcollision;
mod lbvector;
mod linear;
mod link;
mod list;
mod lobj;
mod luigi;
mod mario;
mod marth;
mod math;
mod menu;
mod mewtwo;
mod mobj;
mod mpcoll;
mod mplib;
mod nana;
mod ness;
mod particle;
mod passive_wall;
mod peach;
mod pichu;
mod pikachu;
mod player;
mod player_codes;
mod printf;
mod pulls;
mod purin;
mod quatlib;
mod replay;
mod roy;
mod rumble;
mod runtime;
mod samus;
mod samus_swing;
mod sdk_mtx;
mod sheik;
mod sis;
mod slippi;
mod sound_obj;
mod sound_test;
mod special;
mod spline;
mod stage;
mod stages;
mod thp;
mod tobj;
mod vs;
mod vs_select;
mod yoshi;
mod yoshi_shield_stun;
mod young_link;
mod zelda;

/// The modules the families share.
mod support {
    pub mod calike;
    pub mod calls;
    pub mod display;
    pub mod foxlike;
    pub mod hsd;
    pub mod lglike;
    pub mod lklike;
    pub mod materials;
    pub mod mrlike;
    pub mod mslike;
    pub mod pklike;
    pub mod rumble;
}
