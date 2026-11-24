//! The fighter's structs (`ft/types.h`), hand-written ([design call 3's second
//! half](../../../../../docs/refactor.md#design-call-3s-second-half-the-owned-structs)): each
//! field's `#[at]` is its place in the original's struct, which the comparer's mirror reads
//! (`#[derive(Mirror)]`) and the layout check holds to the decomp's (`game::mirror`). The fields
//! keep the original's order, so the sync test's hash reads them as the generated structs' did.
//! The structs `Fighter` holds alone live here; the ones it shares (the hitboxes, the hurtboxes,
//! the command scripts) stay generated in `game::structs`.

mod attrs;
mod cpu;
mod damage;
mod fighter;
mod parts;

pub use attrs::*;
pub use cpu::*;
pub use damage::*;
pub use fighter::*;
pub use parts::*;
