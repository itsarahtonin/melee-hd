# Porting guide

How a function gets from ssbm-rs into melee-hd. Every port follows these rules, so the simulation stays bit-exact and the ledger stays honest.

## Sources

- **What to compute:** ssbm-rs's port of the function at `melee-hd-base` (`crates/game/src/tu/<unit>.rs` in ssbm-rs). Its generated code spells out what the C leaves to the compiler: which multiply-adds MWCC fused, where values round to single precision, evaluation order, inline copies (`inl_*`), and which functions were transliterated from machine code.
- **Why:** the decomp's C (`C:\Projects\melee\src`) for names, structure and intent.
- **When those disagree or ssbm-rs transliterated the machine code:** the assembly listing (`build/GALE01/asm/...` in the decomp build) decides.
- Never port from nikhilunni/melee's source. Its documentation is fine to read.

## Floats

- A value in memory is an `f32`. A value in a register is an `f64` holding what the Gekko holds; `gekko-fp` does all arithmetic on register values (`fadds`, `fmadds(a, c, b)` = a·c + b fused, `frsqrte`, the paired-single `ps_*`).
- Loading from memory is `fp::lfs(bits)`; storing is `fp::stfs` for `stfs`, and `fp::stfs_ftz` for `psq_st` (hardware flushes a subnormal to zero there). Keep the store the original uses.
- Plain Rust float arithmetic is never used in the simulation, even where it would round the same: NaN propagation and the Slippi float mode differ.
- Single-precision scalar ops fill both paired-single lanes (`Ps::splat`); `lfs` does too; `psq_l` with one value puts 1.0 in the second lane.
- Use the game's own math (`melee_sim::math`), never `f32::sin` and friends.

## Names and the ledger

- Give ports idiomatic Rust names and types, and put `#[doc(alias = "OriginalName")]` on the item that ports each original function. One item may carry several aliases (wrappers, inline copies with a symbol).
- Where the SDK and the game compute the same thing differently, the SDK version is `sdk_*`.
- Every alias needs a check: a leaf-check test that calls `fns::OriginalName`, or a line in `ledger/checks.txt` naming the system-level check that covers it. Functions replaced by a native subsystem go in `ledger/replaced.txt`, provably dead ones in `ledger/dead.txt`, each with its reason. `cargo run --release -p ledger -- --check` must pass.

## State

- Simulation state is plain data: structs, fixed arrays, `Vec`s that only grow, and `Copy` handles. No `Box<dyn …>`, closures, `Rc` or references inside state, so a snapshot is a clone and a restore is `clone_from` without allocating.
- Objects the original allocates from an `HSD_ObjAlloc` pool live in a `hsd::pool::Pool`: freed slots are reused last in, first out, as the original reuses addresses, so identity comparisons behave the same. A handle is the slot index.
- Function pointers become `Copy` tokens (enums) that the game dispatches on (see `hsd::gobj::Kinds`).
- Game data from the disc (`melee_data::archive::Archive`) is immutable and shared, not part of the state. A pointer into it is an offset. If the original writes into loaded data during play, that data moves into the state.
- Nothing outside the state and the controllers reaches a tick: no host settings, clocks, disc timing, audio state or rendering results. Platform events the game polls are explicit inputs.

## Checks

- **Pure functions:** a leaf-check test that runs the function and ssbm-rs's port on special values, random bit patterns and values in the useful range, in both float modes (`tools/leaf-check/tests/math.rs` is the model).
- **Stateful subsystems:** a differential test that sets up the subsystem's globals in ssbm-rs's runtime (`leaf_check::reference()`, scratch memory at `SCRATCH`), drives both implementations through the same random operation sequences, and compares the full structure after every step. Register native functions at fake addresses for callbacks (`tools/leaf-check/tests/gobj.rs` is the model).
- **Systems in play:** the per-tick state comparison with ssbm-rs's `STATE_TRACE` (`docs/oracle.md`).
- After a new test passes, break the port on purpose (one constant, one fused op, one branch) and confirm the test fails. A test that can't fail checks nothing.
- When a difference turns out to come from the test (an input the game never produces), fix the test and say why in a comment. Never loosen a comparison to make a port pass.

## When to stop and ask

- The original's behavior depends on memory addresses, freed memory, or uninitialized values in a way plain data can't reproduce (example: `Archive::unaligned_relocs`).
- ssbm-rs looks wrong.
- A design question this guide and `docs/architecture.md` don't answer.
