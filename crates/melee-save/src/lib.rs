//! Melee's save files outside the simulation: raw memory card images (`.raw`, ssbm-rs's
//! `.card`) and `.gci` files, read and judged as the game judges them, and written byte for
//! byte as the console's card layers write them.
//!
//! Four layers move the save between memory and a card (docs/milestone-5.md, "Saves"); this
//! crate has the lower three:
//!
//! - [`card`]: the SDK's CARD library on a 59-block card: the ID, the two directories and
//!   allocation tables, mount and check (with their repairs), open, read, write, create,
//!   status, delete and format.
//! - [`state`], [`hsd`] and [`exec`]: the HSD card layer (`sysdolphin/baselib/card.c`): a card
//!   file of logical files in checksummed, enciphered ([`crypt`]) blocks with sequence numbers,
//!   files kept twice or with spares, its scan and repair, reads, writes and creation.
//! - [`tasks`]: `lbcardnew`'s task lists and the results the game sees.
//!
//! [`melee`] has the game's own files and its load, save and create; [`gci`] the `.gci` format;
//! [`image`] a card file's blocks deciphered and enciphered again without loss.

pub mod card;
pub mod crypt;
pub mod exec;
pub mod gci;
pub mod hsd;
pub mod image;
pub mod melee;
pub mod state;
pub mod tasks;

pub use card::{Card, CardError, Clock, Console};
pub use gci::Gci;
pub use melee::{Header, Load, SaveData, Verdict};
