# Engine

How melee-sim models HAL's sysdolphin engine (HSD) and the game's state. [The porting guide](porting.md) has the rules every port follows; this explains the shapes they produce.

## One state, plain data

The original runs everything in one address space: joints, display objects, materials, animations, fighters, items, the scheduler. melee-sim keeps all of it in the simulation state, as the original does, including display objects and their material and texture animation. The renderer reads that state and draws; it never feeds anything back. Keeping display objects in the state costs some snapshot size but keeps one model of what the game's code touches, and rollback stays a clone.

## Objects and identity

- Every object type the original allocates from an `HSD_ObjAllocData` pool lives in a `hsd::pool::Pool` of its own (`FObjs`, the scheduler's GObjs and processes, list nodes, and so on). A handle (`FObjId`, `GObjId`, …) is a slot index.
- Freed slots are reused last in, first out, as the original reuses addresses, so code that compares identities or keys by them behaves the same. Ordering comparisons of addresses would not carry over; none has been found yet, and one that turns up must be reported.
- HAL's class system (`class.c`: `hsdNew`, class info, method tables) becomes typed pools plus a class variant on objects whose class the game overrides: fighters set custom joint, interpolated-joint, shape and material classes, stages and items custom materials, refraction effects custom display objects and shapes. A variant picks the overridden behavior where the original dispatched through its method table.
- Reference counts stay where the original keeps them: they decide when objects are freed, and so which slots are reused next.

## Game data

- Archives load from the disc into `melee_data::archive::Archives`, numbered in load order. A pointer into loaded data is a `DataRef` (archive and offset). Archives are shared and immutable, not part of the state.
- The original occasionally writes into loaded data: `JObjLoad` sets the top bit of each particle-list entry it loads, and `HSD_ArchiveLocateExtern` patches extern slots. Such writes become state or derived facts on the reader's side, decided per case.
- `HSD_IDInsertToTable`'s table, keyed by descriptor addresses, is a map keyed by `DataRef`.

## Callbacks

Function pointers in the original (process functions, render callbacks, user-data and object release functions, animation update functions) become `Copy` tokens the game dispatches on, or, where a callback is only passed down a call (an animation update function), a closure parameter that is never stored.

## Checking against ssbm-rs

- `leaf_check::reference()` is ssbm-rs's runtime with the disc's `main.dol` loaded and every port registered; `leaf_check::hsd_reference()` adds an OS heap and HSD's allocators, so ports that build joints, display objects and animations run there.
- Real game states come from ssbm-rs's `STATE_TRACE` (fighters, items, player blocks, stage, camera, RNG and every fighter's joint tree, each engine tick) and, for single game functions, from ssbm-rs's lockstep captures (`CAPTURE=DIR CAPTURE_FUNCS=...`), which save memory and registers at a call.
