//! The item's structs (`it/types.h`), hand-written ([design call 3's second
//! half](../../../../../docs/refactor.md#design-call-3s-second-half-the-owned-structs)): each
//! field's `#[at]` is its place in the original's struct, which the comparer's mirror reads
//! (`#[derive(Mirror)]`) and the layout check holds to the decomp's (`game::mirror`). The fields
//! keep the original's order, so the sync test's hash reads them as the generated structs' did.
//! `Item` and the structs it holds alone live here; the kinds' variables (`ItemVars`' members)
//! stay generated in `game::structs`.

mod item;
mod parts;

pub use item::*;
pub use parts::*;
