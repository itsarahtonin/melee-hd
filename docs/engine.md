# Engine

How melee-sim models HAL's sysdolphin engine (HSD) and the game's state. [The porting guide](porting.md) has the rules every port follows; this explains the shapes they produce.

## One state, plain data

The original runs everything in one address space: joints, display objects, materials, animations, fighters, items, the scheduler. melee-sim keeps all of it in the simulation state, as the original does, including display objects and their material and texture animation. The display pass that walks them each frame is simulation too, since it spawns particles and draws from the RNG ([architecture](architecture.md)); it emits draw commands for the presentation layer, which never feeds anything back. Keeping display objects in the state costs some snapshot size but keeps one model of what the game's code touches, and rollback stays a clone.

## Objects and identity

- Every object type the original allocates from an `HSD_ObjAllocData` pool lives in a `hsd::pool::Pool` of its own (`FObjs`, the scheduler's GObjs and processes, list nodes, and so on). A handle (`FObjId`, `GObjId`, …) is a slot index.
- Freed slots are reused last in, first out, as the original reuses addresses, so code that compares identities or keys by them behaves the same. Ordering comparisons of addresses would not carry over; none has been found yet, and one that turns up must be reported.
- HAL's class system (`class.c`: `hsdNew`, class info, method tables) becomes typed pools plus a class variant on objects whose class the game overrides: fighters set custom joint, interpolated-joint, shape and material classes, stages and items custom materials, refraction effects custom display objects and shapes. A variant picks the overridden behavior where the original dispatched through its method table.
- Reference counts stay where the original keeps them: they decide when objects are freed, and so which slots are reused next.

## The object state

`hsd::Objects` holds every HSD pool (FObjs, AObjs, joints, references, display objects, materials, textures, shapes, world positions, cameras, lights, fog, particles, generators and their app-SRTs) and the HSD globals that point into them (the ID table, the current joint, the current lights, the effect banks, the random seed). The original's objects reach each other freely: an animation can bind a joint, a joint owns display objects and references, a shape references the joints it is skinned to, and an update function can do anything mid-animation (a joint's particle event spawns an effect that loads joints and animations). So functions that cross object types take `&mut Objects`, and animation passes it to update functions with no borrow held across the call.

Reference counts follow the original's: a joint stays alive while shapes, constraints or bindings refer to it, so a removal can leave joints for their last reference to free, exactly where the original does.

Recursion follows the original's too (joint trees load along sibling chains), so the simulation needs a deep stack: the tests run it on a thread with one, and the player must as well.

## Game data

- Archives load from the disc into `melee_data::archive::Archives`, numbered in load order. A pointer into loaded data is a `DataRef` (archive and offset). Archives are shared and immutable, not part of the state.
- The original occasionally writes into loaded data: `JObjLoad` sets the top bit of each particle-list entry it loads, `HSD_ArchiveLocateExtern` patches extern slots, and `psInitDataBankLocate` relocates an effect bank's offsets and sets each command list's draw order. Such writes become state or derived facts on the reader's side, decided per case: a bank is resolved and its kinds fixed as they are read.
- `HSD_IDInsertToTable`'s table, keyed by descriptor addresses, is a map keyed by `DataRef`.

## Callbacks

Function pointers in the original (process functions, render callbacks, user-data and object release functions, animation update functions) become `Copy` tokens the game dispatches on, or, where a callback is only passed down a call (an animation update function), a closure parameter that is never stored. The game's functions the scheduler and the particle system call (processes, particle user functions, the new-generator hook) are dispatched through a trait the caller passes down (`gobj::Host`, `particle::Hooks`).

## Uninitialized values

A few original functions read memory or registers they never set, and plain data can't hold what was there. Each case is probed in ssbm-rs to see what the word is in play, then modeled explicitly:

- An FObj channel whose data ends right after its first key hands its update function a stack word instead of a value (`hsd::fobj::Value::Stale`). Over every VS stage and a Final Destination replay, that word was always a RAM address, a tiny negative float; every channel was a joint's branch visibility, which an address hides. Update functions resolve what any address gives where the result doesn't depend on which address, and count the rest as stale reads, which the oracle reports, so a case that matters can't pass unnoticed.
- `resolveIKJoint1` reads two registers and two stack vectors it never set when the effector sits on the chain's root. No joint on the disc has an IK role and no game code gives one, so it can't happen in play; the port counts a stale read if it does.
- `mpLib_80056C54` adds to a register its caller left when a walk starts on a wall (a line the walker stood on turned into one). The port takes that value as an explicit parameter, which ftDonkey's and itleadead's ports must supply.
- A stage object's user data (`Ground`, from `alloc_user_data_ground`) is uncleared heap. At stage creation the heap is fresh, zero memory (the captures show it), and Slippi's playback codes zero the block anyway, so a new `Ground` starts as zero; a block reused mid-match (Final Destination's phases 14 and 17) would hold a freed object's bytes, which nothing reads before writing. A typed member of the stage variables compares only its own fields (`game::gr::vars`).
- Final Destination's fog blend (`grLast_8021C500`) hands the fog a color whose alpha it never set. Nothing reads the fog's alpha, but the comparison would; the blend panics until a probe shows what the original leaves there (it runs from phase 10 on, past the particle system the cycle needs at phase 9).
- `mpColl_8004A45C_Floor`, `mpColl_8004A678_Floor` and `mpColl_8004C328_Ceiling` take the floor (or ceiling) at the edge an object stops at, probed at that edge. A few floors in the data run right to left (on Great Bay, Pokémon Stadium and Big Blue), and such a floor can fail to reach its own end; the original then takes line -1 with the flags and normal its stack held. The port leaves those two fields as they were and counts a stale read (`mp::coll::CollStatics::stale_reads`).
- `mpColl_80043754` returns a register its caller left when a move is so long its substep count overflows (over about 10¹⁰ units, or infinite) and no substep runs. The game never moves that far; the port returns false.


## Rulesets in the state

`Game::rules` holds the gameplay toggles. `slippi` turns on Slippi's online determinism fixes that the ported code reaches: Final Destination's background cycle keeps the random seed to itself (`grLast_8021AAB0`), and stage objects' user data is zeroed when allocated. The capture importer reads it from whether Slippi's playback codes are patched into the captured code.

## Loaded data the game writes into

- The stage's parameters (`grGroundParam`): the game sets colors and the scale in them, so `game::gr::Globals::param` holds the struct and `stage_info.param` keeps pointing at the archive.
- `grDatFiles_801C6228` sets `rendermode` bit 26 in the materials a stage's `map_head` lists: `Objects::rendermode_patches` records the bits and `HSD_MObjLoadDesc` applies them.
- `Ground_801C0800` writes the stage's `ALDYakuAll` scripts into the random stage item's states in the items' data: `game::gr::Globals::yaku_scripts` records them for the items' port.


## Hangs

`mpColl_80046904` repeats its walls, ceiling and floor checks until the squeezes they apply stop changing, which on some geometry never happens: the original hangs. Its body is a function of the collision data, mpcoll's statics and a few locals, so the port watches for an iteration that starts from a state an earlier one started from and halts there instead. A loop can also drift instead of repeating (an ECB squeezed inside out widens a little each time round); the port runs it as long as the original, and checks bound it with a watchdog (`CollStatics::watchdog`, off in play) and skip the original there.

## Checking against ssbm-rs

- `leaf_check::reference()` is ssbm-rs's runtime with the disc's `main.dol` loaded and every port registered; `leaf_check::hsd_reference()` adds an OS heap and HSD's allocators, so ports that build joints, display objects and animations run there.
- Real game states come from ssbm-rs's `STATE_TRACE` (fighters, items, player blocks, stage, camera, RNG and every fighter's joint tree, each engine tick) and, for single game functions, from ssbm-rs's lockstep captures (`CAPTURE=DIR CAPTURE_FUNCS=...`), which save memory and registers at a call.
