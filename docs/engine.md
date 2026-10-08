# Engine

How melee-sim models HAL's sysdolphin engine (HSD) and the game's state. [The porting guide](porting.md) has the rules every port follows; this explains the shapes they produce.

## One state, plain data

The original runs everything in one address space: joints, display objects, materials, animations, fighters, items, the scheduler. melee-sim keeps all of it in the simulation state, as the original does, including display objects and their material and texture animation. The display pass that walks them each frame is simulation too, since it spawns particles and draws from the RNG ([architecture](architecture.md)); it emits draw commands for the presentation layer, which never feeds anything back. Keeping display objects in the state costs some snapshot size but keeps one model of what the game's code touches, and rollback stays a clone.

## Objects and identity

- Every object type the original allocates from an `HSD_ObjAllocData` pool lives in a `hsd::pool::Pool` of its own (`FObjs`, the scheduler's GObjs and processes, list nodes, and so on). A handle (`FObjId`, `GObjId`, …) is a slot index.
- Freed slots are reused last in, first out, as the original reuses addresses, so code that compares identities or keys by them behaves the same. Ordering comparisons of addresses would not carry over; none has been found yet, and one that turns up must be reported.
- HAL's class system (`class.c`: `hsdNew`, class info, method tables) becomes typed pools plus a class variant on objects whose class the game overrides: fighters set custom joint, interpolated-joint, shape and material classes, stages and items custom materials, refraction effects custom display objects and shapes. A variant picks the overridden behavior where the original dispatched through its method table. Default classes for loading (`HSD_JObjSetDefaultClass`, `HSD_PObjSetDefaultClass`) are state too. Fighters' materials carry their class (`MObjClass::Fighter`); the fighters' joint class (`ftJObj`), which only changes matrices on Flat Zone's flat stage, isn't modeled yet.
- Shadows (`hsd::shadow`) keep what allocating one changes: a camera and a texture from their pools, references on the joints they cast from, and the texture on the materials' shadow list. Rendering into the texture is the display pass's.
- Reference counts stay where the original keeps them: they decide when objects are freed, and so which slots are reused next.
- A hitbox's victims (`HitVictim::victim`, the user data of what it hit) are a `lb::hitcapsule::Victim`: a fighter or an item by handle, or, for something no longer live, the old address, which nothing follows.
- `ftcoll.c`'s damage logs and clank flags (`dmg_log0`, `dmg_log1`, `ftColl_804D6560`) are scratch each fighter's attack collision starts over and nothing reads afterwards; they live in `ft::attack::HitLogs`, outside the mirror, and the logs hold which fighter's hitbox and which hurtbox each hit came from rather than pointers.

## The object state

`hsd::Objects` holds every HSD pool (FObjs, AObjs, joints, references, display objects, materials, textures, shapes, world positions, cameras, lights, fog, particles, generators and their app-SRTs) and the HSD globals that point into them (the ID table, the current joint, the current lights, the effect banks, the random seed). The original's objects reach each other freely: an animation can bind a joint, a joint owns display objects and references, a shape references the joints it is skinned to, and an update function can do anything mid-animation (a joint's particle event spawns an effect that loads joints and animations). So functions that cross object types take `&mut Objects`, and animation passes it to update functions with no borrow held across the call.

Reference counts follow the original's: a joint stays alive while shapes, constraints or bindings refer to it, so a removal can leave joints for their last reference to free, exactly where the original does.

Recursion follows the original's too (joint trees load along sibling chains), so the simulation needs a deep stack: the tests run it on a thread with one, and the player must as well.

## Game data

- Archives load from the disc into `melee_data::archive::Archives`, numbered in load order. A pointer into loaded data is a `DataRef` (archive and offset). Archives are shared and immutable, not part of the state.
- The original occasionally writes into loaded data: `JObjLoad` sets the top bit of each particle-list entry it loads, `HSD_ArchiveLocateExtern` patches extern slots, and `psInitDataBankLocate` relocates an effect bank's offsets and sets each command list's draw order. Such writes become state or derived facts on the reader's side, decided per case: a bank is resolved and its kinds fixed as they are read.
- `HSD_IDInsertToTable`'s table, keyed by descriptor addresses, is a map keyed by `DataRef`.
- Fighters' files are an input like every disc load (`game::ft::data::LoadedFiles`): the frontend loads a match's fighters' data, costume and animation files before it starts, as the character select's preloading does (`vs_match::load_fighter_files`), and `Fighter_Create` finds them there by name where the debug VS mode would read the disc mid-tick. An animation file sits in ARAM, one archive per animation; its ARAM address is part of the input, since fighters keep it to know which animation they last loaded (`Fighter::x5A4`). `Match::start` lays the files out from the ARAM heap's start in slot order, as the debug VS mode's loads do; where a preloaded match puts them is up to the preload cache, which isn't simulated, so a full-state comparison with such a match would see other addresses in those fields. Where `Fighter_Create` would load each file (its data file, its costume, its animation file into ARAM), it loads it on the audio timeline (`Audio::load_file`, `lbFile_8001668C`) with the file's entry number, destination and size: its ARAM address, or for main memory 0x80000000, since the heap and so the original's address there aren't modeled yet. Without a timeline nothing happens.
- The preload cache's bookkeeping (which files are resident, their load scores) stays outside the simulation.

## Callbacks

Function pointers in the original (process functions, render callbacks, user-data and object release functions, animation update functions) become `Copy` tokens the game dispatches on, or, where a callback is only passed down a call (an animation update function), a closure parameter that is never stored. The game's functions the scheduler and the particle system call (processes, particle user functions, the new-generator hook) are dispatched through a trait the caller passes down (`gobj::Host`, `particle::Hooks`). Joints' particle callbacks, which efLib sets (an animation's particle events, and a particle list's first display), go through `particle::Hooks` too, so joint animation (`jobj::anim`, `jobj::anim_all`) takes the hooks; the game passes `code::ParticleHooks`.

The display pass isn't ported yet. Render callbacks that only decide what to draw (the HUD's, `game::hud`, and the debug text's, `game::hud::devtext`) are functions returning that decision (`hud::Draw`, `devtext::Draw`), checked against the draw calls the original makes; the name tags' camera (`fn_802FCAC4`) also moves the names as it decides, so it takes the state mutably.

## Uninitialized values

A few original functions read memory or registers they never set, and plain data can't hold what was there. Each case is probed in ssbm-rs to see what the word is in play, then modeled explicitly:

- An FObj channel whose data ends right after its first key hands its update function a stack word instead of a value (`hsd::fobj::Value::Stale`). Over every VS stage and a Final Destination replay, that word was always a RAM address, a tiny negative float; every channel was a joint's branch visibility, which an address hides. Update functions resolve what any address gives where the result doesn't depend on which address, and count the rest as stale reads, which the oracle reports, so a case that matters can't pass unnoticed.
- `resolveIKJoint1` reads two registers and two stack vectors it never set when the effector sits on the chain's root. No joint on the disc has an IK role and no game code gives one, so it can't happen in play; the port counts a stale read if it does.
- `Camera_RequestQuake` never sets a quake's length for a kind other than the four quakes and stores what its epicenter argument's register holds: 0 for a null epicenter, else the epicenter's address. The port stores 0 and counts a stale read when there is an epicenter; no caller passes such a kind.
- The cutscene camera's orbiting eye (`Camera_8002E234`) eases only the parts of the orbit its script set and reads the others (pitch, yaw or distance) from stack slots it never wrote. Master Hand's and Crazy Hand's entrances set orbits that way. The port panics there until a probe of those slots in play decides the model; the camera's other transitions never read a register they didn't set except a fourth transition kind no code sets, which panics too.
- `mpLib_80056C54` adds to a register its caller left when a walk starts on a wall (a line the walker stood on turned into one). The port takes that value as an explicit parameter, which ftDonkey's and itleadead's ports must supply.
- A stage object's user data (`Ground`, from `alloc_user_data_ground`) is uncleared heap. At stage creation the heap is fresh, zero memory (the captures show it), and Slippi's playback codes zero the block anyway, so a new `Ground` starts as zero; a block reused mid-match (Final Destination's phases 14 and 17) would hold a freed object's bytes, which nothing reads before writing. A typed member of the stage variables compares only its own fields (`game::gr::vars`).
- Final Destination's fog blend (`grLast_8021C500`) hands the fog (`Ground_801C205C`) a stack color whose alpha it never set. Probed over a replay's two background cycles, that byte is the low byte of whatever the frame's earlier calls left in the slot: the caller's stack pointer that `Camera_RequestQuake` saved there in phases 10 and 11 (0x68), a fighter pointer `ftCo_800DF0D0` saved (0x80), or an int-to-float temporary's 0 from `grLast_8021C40C`, so it depends on addresses and on which code ran before. Nothing reads a fog's alpha (`GXSetFog` takes the color's RGB, and the readers of `Ground_801C2090` look at RGB only), so the port sets the RGB, keeps the old alpha and marks it stale (`Fog::stale_alpha`), and the comparer leaves a stale alpha out; any later write of a whole color clears the mark.
- `efLib_Create` leaves an effect's `params` and `user_data` (and two bytes nothing reads) as the pool's slot had them. Every spawn sets the parts its effect's update reads before the update runs, and nothing else reads them, so a new effect starts them at zero and the comparer compares the parts the effect's update reads (`ef::Update::reads_params`); `user_data` is only ever a fighter, which `efLib_Cb_SetRotYAndTransition` reads as a joint's x scale: a fighter's facing is at that offset. Deferred spawns (`EF_QueuedEffect`) likewise compare the words their spawn kind uses.
- Some spawn cases are a character's own and read its fighter: `efSync_Spawn`'s 0x501 reads Peach's part 85, past a shorter parts table, and the action-state callbacks read the character's `mv` words. Spawned for another character, the original reads past the table or another layout; the port stops on the first and reads the same words for the second.
- `mpColl_8004A45C_Floor`, `mpColl_8004A678_Floor` and `mpColl_8004C328_Ceiling` take the floor (or ceiling) at the edge an object stops at, probed at that edge. A few floors in the data run right to left (on Great Bay, Pokémon Stadium and Big Blue), and such a floor can fail to reach its own end; the original then takes line -1 with the flags and normal its stack held. The port leaves those two fields as they were and counts a stale read (`mp::coll::CollStatics::stale_reads`).
- `lb_8001044C` (a dynamic bone chain's swing) turns a joint by writing its rotation from a stack `Quaternion` whose x, y and z come from `HSD_QuatLib_8037EB28` and whose w it never sets, so the joint's `rotate.w` becomes a stack word: probed over Final Destination captures, usually 0, sometimes an address, an int-to-float temporary's 0x43300000 or a float. The joint isn't a quaternion joint (the same code clears `JOBJ_USE_QUATERNION` on it and its descendants), so nothing reads that w. As with the fog's alpha, the port keeps the old w and marks it stale (`JObj::stale_w`), the comparer leaves a stale w out, a whole rotation written clears the mark (copies carry it), and a quaternion read of a stale w counts a stale read, as does a dynamics link taking a stale joint's rotation (`lb_8000FD48`).
- On a flat floor (`x2228_b1`), `lb_8001044C` probes with `lb_800103D8`, which reports a crossing without writing its point when the probe is level and above the floor; the chain then reads that unset point. Only a link lying exactly level above the floor's height gets there; the port stops there rather than read a stale vector.
- `lb_8000FCDC` takes the dynamics links' block from the scene's heap and `lb_8000FA94` sets only each link's joint and next; the rest of a link is written by `lb_8000FD48` and `lb_80011710` before anything reads it, except the last link's dominant axis (`unk_54`), which nothing reads. melee-sim's new block starts at zero, like a fresh heap; the importer gives each link its slot by address so the reset's order matches.
- `mpColl_80043754` returns a register its caller left when a move is so long its substep count overflows (over about 10¹⁰ units, or infinite) and no substep runs. The game never moves that far; the port returns false.
- `ftMapping_list` (a character's fighter kinds) has 33 entries, and the player blocks index it by a slot's character kind, which an empty slot can hold past the table (33 and up). The original then reads the executable's bytes after the table: a padding byte and `player.c`'s message strings, constant data the port carries along with the table (`pl::player::FT_MAPPING`). A kind past those bytes stops the simulation.
- `Player_80031AD0` hands `Fighter_Create` a `plAllocInfo` on its stack and sets two of its third byte's bits, the second fighter's and the transformation's; the other six are whatever the stack held. `Fighter_Create` reads only those two (`x6.b0` in `Fighter_UnkInitLoad_80068914`, `has_transformation` after the spawn), so `ft::create::AllocInfo` holds just them.
- A fighter's blocks (`HSD_ObjAlloc` from `fighter_alloc_data`, the attribute, parts and DObj-list pools) come back uncleared, and `Fighter_Create` leaves about 5,200 of the fighter's 0x23EC bytes, 850 to 910 of its attribute block's and 980 to 1,220 of its parts table's unwritten for Fox and Marth. Creation reads none of them: probed with the blocks filled with zeros and with ones, ssbm-rs ends with every written bit the same. In a fresh heap the rest are zero, and Slippi's playback zeroes the blocks anyway; the debug VS mode's captures show a previous scene's bytes there even in a session's first match. melee-sim starts a fighter's blocks at zero, and its check zeroes the reference's (`leaf-check` `fighter_create`). A later read of a field creation leaves unwritten would see zero where a vanilla console sees stale bytes (the decomp notes Luigi's `x222C`); such reads belong to the code that makes them, and each must be checked for it.
- `fn_8016DEEC` (facing players toward their opponents) takes a player's first opponent without measuring its distance, keeping the last distance it measured, or before any, what its caller left in a register (`f26`). The VS scene's entry leaves 0 there in every capture, debug matches' and a replay's alike, so `place_players` passes 0; with three or more players that decides who the first player faces.
- `ftColl_8007A06C` picks the logged hit with the most knockback, above -1; a log whose knockbacks were all NaN or -1 and below would leave the index it reads uninitialized (a register its caller left). Knockback can't go there with the game's data; the port stops.
- A thrown fighter's hitbox keeps its thrower in a union (`x1064_thrownHitbox.x134`, `Fighter::x1198`) that other hitboxes use for flags, so the mirror holds its bytes; the attack collision stops when it's set, until a throw's port models it.
- Like creation, the debug VS mode's later matches leave a fighter's unused entries (dynamic bone sets past `dynamics_num`, costume textures past their count, a human's computer-player pointers) and an item's unused dynamic bones holding an earlier scene's bytes, which aren't valid handles. Nothing reads them; the per-frame processes' check zeroes them on both sides before it runs (`leaf-check` `fighter_procs`).
- A fighter's address (`ft::Globals::addresses`, its `Fighter*`), the case below, is recorded when `Fighter_Create` allocates it: from `Settings::fighter_addresses` (the oracle seeds them from ssbm-rs's trace) in a match started from settings, from the captured struct's address in an imported state.
- `it_802ADF10` (the blaster's muzzle flash) keeps the pointer `efSync_Spawn` returns as an `s32` in `itFoxBlaster_ItemVars::xDE4[0]`, and code only ever moves it along the six-entry history the shots keep, never reads it otherwise. The port keeps an effect handle there (`[Option<EffectId>; 6]`), which the shift moves as it moves any address: RAM addresses are 0x80xxxxxx, negative as `s32`, and nothing tests or compares them. The importer pairs each word with the effect at its address, a live one or a freed slot (an older flash has usually ended), and the comparer pairs it as it pairs any effect pointer. The address never reaches gameplay, so the rule below for addresses that do doesn't apply; if code that reads the word turns up, the flash carries its address under that rule.
- `Stage_80224E64` and `fn_80167638` read the spawn point `Ground_801C2D24` writes, which it leaves alone when the stage lacks the point. Every VS stage has the points; the ports stop there rather than read a stale vector.
- The text system's heap (`HSD_SisLib_803A6048`) is a block of the scene's heap that nothing clears, and `HSD_SisLib_803A5ACC` leaves half of a new text's fields as the block held them (`HSD_SisLib_803A6368` writes all but `x88` and `x9F` right after, in every call the game makes). melee-sim keeps those fields as unknown (`hsd::sis::Text`'s `Option`s) until written; no ported code reads them, and the comparison takes an unknown field as the capture's. Strings' bytes past their terminator are likewise never read.
- `HSD_SisLib_803A67EC` reads a byte past a string that ends in a Shift-JIS lead byte or any character it passes as one, and goes on reading what follows. The game's strings (names, numbers) end in whole characters; the port reads zeros there.

Some of these reads hand gameplay a heap address. Nana's CPU DI (`ftCo_800AC5A0`, which the decomp marks `@bug`) passes `ftCo_800B46B8` a register it never set when her knockback is near zero, and that register holds her `Fighter*`, so the pointer's low byte becomes her stick's Y. Ice Climbers matches go different ways when the heap is laid out differently: ssbm-rs's recording setup and a Dolphin run without Slippi's codes place the fighters 0x1DC0 bytes apart. So an object whose address reaches gameplay carries, as plain data, the address the original's allocator gives it: a pool slot's address follows from the pool's base, and the heap's from a model of its allocations, laid out per ruleset (Slippi's codes take heap of their own). Until that model is checked, the oracle seeds the addresses from ssbm-rs's trace, which records them, and the comparer checks them like any other field.


## Fighters' variables

A fighter's character variables (`Fighter::u`) and action state variables (`Fighter::mv`) are unions each character's and state's code reads at its own offsets, so they stay big-endian bytes (`ft::vars::Vars`). A word that holds a GObj pointer (Fox's and Falco's blaster in `u`, their Illusion's ghost in `mv`) holds a handle instead of bytes once code stores one there, and compares as a handle; the importer types the blaster's word for Fox and Falco. The ghost is only ever written, so an imported one stays the bytes memory held. Reading a handle's word as bytes, or bytes as a handle, stops.

## Seams

Where game code calls a subsystem no port covers yet (the HUD's magnifier, most fighters' own callbacks), it goes through `game::code::call`, by the original function's address with its arguments as register values (`code::Val`), and stops there until that subsystem's port adds its arms. With the feature `stubs` (leaf-check enables it) a check can stand a function in on both sides and compare the calls each side made and their arguments, so the code around a seam is checked before what it calls is ported. The simulation itself never registers a stand-in.

## Effects

An effect (`game::ef`) is an `Effect` in its own pool, the user data of a GObj of class 8 on process link 11 or 12. Spawning goes by id through `ef::spawn` (`efSync_Spawn`), whose variadic arguments are `ef::Arg`s read in order as each case asks (`ef::Args`, which can also decode them lazily from a caller's registers). Which effect files are loaded is state (`ef::Globals::tables`); the files themselves come from the frontend, which loads every effect file it may need with the match's other archives into `ef::Globals::files`, so a sync load (`efAsync_LoadSync`) only registers the file's banks. An async load only asks the disc for a preload that the sync load later takes, so it leaves no state (`ledger/replaced.txt`). Functions other code reaches by address (`efLib_Init`, the loads, `efSync_Spawn`, destroying, pausing and flagging effects) answer `code::call`; Rust callers use `game::ef` directly, and `ef::queue` with a fighter's or item's queue head for `efAsync_Spawn`.

A captured state imports what memory holds: a freed effect, deferred spawn, generator or app-SRT that a pointer outlived (a spawn's `next`, a fighter's queue while `efAsync_QueueFlush` runs, which empties the queue head only at its end, or Final Destination's swaying piece, which keeps its generator after the generator's life ends and reads the app-SRT its removal cleared) becomes a freed slot that pairs by identity, and the display pass's cached first particle of a link, which can name a particle freed since the pass that sorted it, imports as none. A spawn captured inside a flush can hold a live deferred spawn only in the flush's registers; the pool then counts a block no memory reaches, and such a capture can't be imported (the effects' tests keep those out).

The display pass runs after each frame's engine ticks, as the scene loop runs it (`game::render`, `hsd::display`): the max render link's cameras, each camera's passes over the render links its GObj names, and the callbacks there. HSD's joint traversal (`HSD_JObjDispAll` down to `HSD_PObjDisp`) brings joint, skin and envelope matrices up to date, runs materials' texture setup and a specular material's light loads, and spawns a joint's particle list on its first display through `particle::Hooks`; cameras bring their view matrices up to date, and light GObjs take GX slots (`HSD_LObjSetupInit`). Where the original issues GX commands, the pass appends `display::Draw` commands to the frame's output (`Frame::draws`): a camera's start and end, lights, fog, each shape with its joint, display object and transparency pass, and each particle link drawn. Only the screen render pass is modeled, and the depth-sorted lists stay empty as in Melee. The HUD's render callbacks decide what to draw (`hud::Draw`: a GObj's joints in a pass, or a camera over the links its mask names) and the pass carries the decisions out; the debug text's drawing makes its camera current and hands its glyphs on as `Draw::Text`. Refraction (`lb::refract`) is set up with the VS scene: `LbRf.dat`'s table and a texture chain per entry, whose generated indirect maps and screen copy are the presentation's images (`ImageRef::Refract`, `ImageRef::RefractFrame`); the game camera's callback keeps the screen copy's projection and copies the screen while anything uses refraction. Render callbacks no port covers yet stop the simulation (`code::run_render`): the game camera's (`fn_800301D0`, with its shadows), the magnifying glasses' (`ifMagnify_802FBBDC`, `ifMagnify_802FB8C0`), the HUD's other text camera (`fn_8001FEC4`) and text (`HSD_SisLib_803A84BC`), the stage's display (`grDisplay_801C5DB0`), the fighters' (`ftDrawCommon_80080E18`), stage, item and fighter materials' own setup, and drawing a particle with an app-SRT, whose matrices the drawing brings up to date once a display frame and freezes for a one-shot app-SRT (`psDispSubAppSRT`).

## Audio

`Game::audio` holds:

- HAL's AX driver: its 96 virtual voices, the free and active lists, the handle counter and the sound bank's command stream tables.
- The synth: 64 nodes, the loaded effects, the channel volumes, the volume update list and the music stream's state.
- The AX library's 64 voices, with both copies of each parameter block (the CPU's and the DSP's) and the priority stacks.
- HAL's device command queue (`audio::devcom`), which the music stream and the game's file loads read through.
- The game's audio library's own state (`lb::audio::LbAudio`): the banks' load states, the crowd's loops, the queued sounds and the mix settings.
- The DSP model.
- The timeline (`audio::clock`).

The sound bank (`smash2.sem`) and the music files are game data: `Archive::raw`s that the driver and the stream read by offset.

**DSP models** (`audio::dsp`). In the silent model the DSP's blocks change only through the frame's sync, so a voice plays until it is keyed off and faded, stolen or dropped for the DSP's budget, as in ssbm-rs's default runs. In the sample-accurate model the DSP also runs each voice on the block list for its five milliseconds, as ssbm-rs's `AX=1` (Dolphin's AX microcode) does:

- the resampler's position, the accelerator's address through ADPCM frames or PCM, looping, and stopping at the end of the samples;
- the volume envelope and the bus ramps.

What depends on the samples' values is the mix's, and the comparison leaves those words out in that model: the decoder's predictor and history, and the resampler's and buses' last samples. `vs_match::Settings::dsp` carries the model.

**The music stream** runs as in the original. The master clock follows the playing voice through the three boxes, moves the voices' end and loop, and refills the box after the playing one through the device command queue: a disc read into a relay buffer, then on to ARAM. Its samples are the mix's, but the boxes' headers it reads into memory are state. Only in the sample-accurate model do voices advance, so only there does the stream refill.

**The timeline** (`audio::clock`) is ssbm-rs's virtual hardware:

- video fields, the pad sampling alarm, the 5 ms audio DMA and the DSP's answer;
- disc reads, each finishing 20,250 ticks after its command (ssbm-rs's `DVD_LATENCY`, Slippi's fast disc speed);
- ARAM transfers, which finish at once and raise their interrupt;
- the interrupt handlers' costs, and the scene loop's waits and register reads.

Each disc read also sets the disc driver's 10 s timeout alarm, which the read's interrupt cancels. Any event can be taken back before its time (`Clock::cancel`), as on the console: a cancelled or re-set alarm (`set_pad_alarm`, `cancel_pad_alarm`) and a stopped audio DMA (`stop_audio_dma`) leave nothing behind. ssbm-rs before its `ab3cd01` left such events queued, where a read's dead timeout cut a wait short 10 s after every disc read and moved the audio frames after it.

That bug never reached the silent Final Destination and Battlefield timelines the checks use, which start at tick 1500. In the silent model a match makes no disc reads, and the dead events from the loads before tick 0 fell near tick 600. Re-recording them with the fixed ssbm-rs changed only ticks 580 to 602, and their start captures came out identical. It did reach the `AX=1` timeline, whose music refills read the disc every few seconds: its re-recording moved ticks 2855, 5107, 5214, 6608 and 8110.

**Game file loads.** Pokémon Stadium loads each transformation's file during the match.

- *The original's own load* is asynchronous and polled: `grpstadium.c`'s `startLoad` uses `lbFile_80016580` with a callback, and the stage waits for the callback. That is the case `docs/architecture.md` treats as an input.
- *The replays ssbm-rs plays* (Slippi's codes) load it synchronously: the call returns into the codes' memory, as `lbFile_8001668C`, a request on the device command queue and then a loop that waits until the load's callback, taking interrupts as it waits.

`Audio::file_load` models the synchronous load on the timeline, inside the tick, and an audio frame that comes due during the wait runs there. A read into the game's heap moves no bytes here: the loader takes the file from its archives.

**The library's tick** (`lbAudioAx_80027DF8`) runs in the scene loop before the processes. It does three things:

- starts the crowd's two loops, follows their volumes, or stops them;
- hands the synth the channels' volumes and the music channels' aux send whenever the settings change;
- runs the queued sounds' timers, or plays the queued sounds all at once when the scene asks.

The VS scene's pause menu holds the music and the match's channels through `lbAudioAx_80024E84`, and its start and end reset the library's mix.

**The aux effects** (reverb and the like, set up by `HSD_AudioSFXSetupAux`) are presentation. Their state lives in a heap of their own, and only the aux callbacks read it. Those callbacks touch only the mix buffers, so the game reads nothing they write. The driver's aux flags are state.

**Where game logic reads audio.** Almost every audio call game code makes is fire-and-forget: handles are stored only to stop, pan or bend their sounds later. Game code calls the driver directly in one place only, the reset path's `HSD_DevComIsBusy`. Three paths make the game depend on *when a sound ends*, which is what makes voice lifetimes, the clock and the DSP model simulation:

1. *The crowd* (`melee/sfx/crowdsfx.c`, set up for every match by `fn_8016E730`). Its per-frame state machine (`un_80321A00`) moves on only when the crowd's chant voice has stopped (`lbAudioAx_80023710`, so `HSD_AudioSFXCheck`). A start that fails, for example on a channel the pause menu or camera mode muted, counts as stopped. Whether a fighter's knockback then starts a gasp and awards bonus stat 0x5B (`un_80321EBC`, then `pl_8003FDA0`) depends on that, as does the focus fighter and which crowd sound plays next. A VS match reaches it.
2. *Corneria's and Venom's Star Fox calls* (`Ground_801C54DC`, `grCorneria_801E277C`, `Ground_UpdateStarFoxSequence`). A call's window closes only after its voice line ends, so the sequence lives as long as its lines play. While it lives:
   - the HUD's percents are hidden;
   - a Fox or Falco side taunt can't start a call: starting one draws `HSD_Randi(5)` from the game's RNG and sets the taunt's damage callback;
   - a taunt during a call earns bonus stat 0x8B at the match's end.

   A VS match reaches it. In attract mode, Corneria's intro banter can block a taunt's call.
3. *The tournament bracket's music* (`gmtou_1.c`'s `fn_8019AF50`) waits for the music stream to stop (`AXDriverCheck`) before starting the winner's. This is menus only.

Other reads change nothing simulated:

- the sound options menu's stereo setting;
- the debug sound test's;
- Kirby's copied voices (`lbAudioAx_800233EC` reads whether `kirbytm.ssm` is loaded, which only picks the sample, and only single-player loads it);
- the stores and checks at fighter and item teardown, whose handles are cleared either way.

Sound objects (`lbAudioAx_800263E8`) end by frame count, not with their voice. Several sound paths draw from the game's RNG whatever audio does, so a port keeps those draws even with audio out:

- a sound object panned at 0 (`fn_80025FAC`'s `HSD_Randi(2)`; fighters' subaction sounds do this);
- `ft_PlaySFX`'s voice ids 332 to 370;
- `ft_800889F4`'s random voice;
- `ftcoll.c`'s hit sounds;
- stages' random music.

**Not ported:**

- The boot's audio setup.
- Loading sound banks: `HSD_SynthSFXLoad` and the library's bank choice `lbAudioAx_8002785C`. A match started from its settings has no banks loaded, and the driver starts nothing (the original's `HSD_AudioSFXStartParam` returns -1 the same way).
- Starting and stopping the music: `HSD_AudioPStreamStartChParam` and `lbAudioAx_800236DC`.
- The library's object pool: `lbAudioAx_8002835C`.

The VS port still hands those to the frontend as `Event::Audio`. A state imported from a single captured call has no timeline, and no audio frames run.

## Rulesets in the state

`Game::rules` holds the gameplay toggles, one per rule of the Slippi ruleset (`game::slippi`): `slippi` for the online determinism fixes, `ucf`, `neutral_spawn`, `prevent_wobbling`, `offscreen_limits` and `freeze_glitch_fix`. Among the determinism fixes the ported code reaches, Final Destination's background cycle keeps the random seed to itself (`grLast_8021AAB0`) and stage objects' user data is zeroed when allocated. UCF's pad buffer, which the code keeps in its own words, is `Game::ucf`. A replay's code list decides the rules (`Rules::from_codes`); the capture importer reads them from the codes applied in the captured game (`Rules::from_memory`).

`Game::playback` is Slippi replay playback's state in a match a replay drives (`game::playback`): the replay frame it counted, the frame it read last, and while a tick runs, the Slippi device's answers the frontend supplied for it. The pad library's `playback` flag is the playback codes' patch to it.

## Loaded data the game writes into

- The stage's parameters (`grGroundParam`): the game sets colors and the scale in them, so `game::gr::Globals::param` holds the struct and `stage_info.param` keeps pointing at the archive.
- `grDatFiles_801C6228` sets `rendermode` bit 26 in the materials a stage's `map_head` lists: `Objects::rendermode_patches` records the bits and `HSD_MObjLoadDesc` applies them. A captured state's marks are what memory holds: Pokémon Stadium's transformation file shows them gone while it's registered (its buffer is lbDvd's preload entry 0x7D5, which something writes again after the stage registers it), and its return to the default stage zeroes the first words of that file's header (it hands `grAnime_801C65B0` the file's buffer instead of its archive), so the importer finds such a file through the header copy in the stage's `HSD_Archive`.
- `Ground_801C0800` writes the stage's `ALDYakuAll` scripts into the random stage item's states in the items' data: `game::gr::Globals::yaku_scripts` records them for the items' port.
- `ftData_80085A14` writes each animation's ARAM address into the character's animation table when the animation file reaches ARAM; `Running::anim_aram` works the address out from the file's and the animation's offset instead.
- `lbRefract_PObjLoad`, the fighters' shape loader, copies each vertex's position-matrix index over its last texture-matrix index in a refracting shape's display list. Only the display pass reads display lists, so `PObjs::refract_patches` records the rewrite for it.
- `Ground_801C20E0` edits a stage's light list in place when its light overrides set flags: it drops the lights no override names and sets the others' flags. The edit leaves the list the same when it runs again, so the fighters' light (`gr::setup::load_fighter_lights`) loads the list as the edit leaves it, computed from the archive.


## Hangs

`mpColl_80046904` repeats its walls, ceiling and floor checks until the squeezes they apply stop changing, which on some geometry never happens: the original hangs. Its body is a function of the collision data, mpcoll's statics and a few locals, so the port watches for an iteration that starts from a state an earlier one started from and halts there instead. A loop can also drift instead of repeating (an ECB squeezed inside out widens a little each time round); the port runs it as long as the original, and checks bound it with a watchdog (`CollStatics::watchdog`, off in play) and skip the original there.

## Checking against ssbm-rs

- `leaf_check::reference()` is ssbm-rs's runtime with the disc's `main.dol` loaded and every port registered; `leaf_check::hsd_reference()` adds an OS heap and HSD's allocators, so ports that build joints, display objects and animations run there.
- Real game states come from ssbm-rs's `STATE_TRACE` (fighters, items, player blocks, stage, camera, RNG and every fighter's joint tree, each engine tick) and, for single game functions, from ssbm-rs's lockstep captures (`CAPTURE=DIR CAPTURE_FUNCS=...`), which save memory and registers at a call.
- A captured state holds words its owner never set: a heap block keeps an earlier object's bytes until each field is written, and some fields are written only in some states. The importer follows what the original reads and represents the rest explicitly. A pointer to a freed object or into nothing (a freed joint, GObj, fighter, item, joint table, camera box or deferred spawn, or a word that was never a pointer) dangles: it gets a slot that stands for the address and compares by address (`Maps::dangling_*`), so the original handing that block to a new object later pairs the new one afresh. A joint counts as live only in the heap and starting with its class; a freed one starts with the allocator's link. Arrays filled up to a count (a fighter's costume textures and dynamic bone sets, an item's dynamic bones: `game::ft::counted`) are mirrored up to the count; a count past the array is one never set, and nothing reads such an array. A human player's CPU block (`CpuFighter`) is never filled, so its pointers dangle; an item's ECB box leaves the joint member's last two words as they were; items' links (user data kind 6 on a GObj of class 7, `ItemLink`) aren't modeled and import as raw user data.
