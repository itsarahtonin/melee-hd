# The Slippi ruleset

A survey of what the Slippi ruleset needs for live play and a plan for splitting it into a handful of agent tasks, each checkable on its own. Surveyed on 2026-11-14 from `main` at `f7115325`, the day milestone 6's last rows landed and before its full verify; the counts and measurements are that day's. Sarah numbers the milestone.

What the ruleset is (Sarah, 2026-11-14, agreeing to this scope: "yes that sounds about right to me"):

1. **Slippi's gameplay rules in live play**: the 27 codes melee-hd already runs for `.slp` playback (`game::slippi`, `Game::rules`), on in a live match under a Ruleset setting. Vanilla stays the exact option.
2. **No memory card**, as in Slippi Dolphin.
3. **Slippi's unlocks** (milestone 5's call of 2026-10-23).
4. **Not** Slippi's menus, its online entries, m-ex, the netplay code set or Slippi Dolphin's other codes ("i mostly meant the no-memcard thing and the rules that decouple the bubble rendering from the sim code and fixes the CPU DI issue etc").

Also settled: `.slp` files recorded under the ruleset play back exactly in Slippi Dolphin ("to be clear yes .slp files recorded from our slippi ruleset should indeed play back exactly in slippi dolphin"); "also taking perf/code structure wins wherever we can get it; the decoupling and di fix et al should allow us to not have to do some messiness we otherwise have to do with the original ruleset"; under the ruleset and in playback a read of the GPU's pixels sees the bytes from before the copy (2026-11-07); Vanilla's heap leftovers stay modelled exactly (2026-10-29, 2026-11-12); no more players' own codes (2026-10-16); after this comes the refactor round, then the player's polish, each with its own discussion first (2026-11-13). Nothing here contacts Slippi's servers or online services.

The milestone is done when:

- The settings panel offers Vanilla and Slippi, with the default Sarah picks ([design calls](#design-calls)); Vanilla's sessions compare as before.
- Under the Slippi ruleset melee-hd boots with no memory card and Slippi's unlocks, and its menus compare clean tick by tick from power-on with ssbm-rs's trace of the same session run the same way, to each match's start.
- Every VS match under the ruleset runs Slippi 3.19's 27 gameplay codes in Slippi Dolphin's floating point, and **its `.slp` plays back exactly in Slippi Dolphin**: tick by tick the live match equals melee-hd's playback of its own `.slp`, whole state but a named list of playback's own bookkeeping; ssbm-rs's playback of the `.slp` (Slippi's real playback codes, `--fp slippi`) compares clean with melee-hd's and its replay check is clean; and Sarah's spot check in her Slippi Dolphin agrees.
- What the ruleset makes unnecessary is skipped in its matches where that wins speed or simplicity, each skip held by a tripwire or a test, and Vanilla unchanged ([What the ruleset makes moot](#what-the-ruleset-makes-moot)).

## Status (2026-11-14, `main` at `f7115325`): planned

Nothing started. Sarah answered the design calls on 2026-11-14 (see Decisions).

## What melee-hd has today

- **The gameplay codes, for playback only.** A replay's code list makes its rules (`Rules::from_codes`); `game::slippi`, `game::ucf` and `game::player_codes` port every gameplay code where it injects, in three versions of Slippi's set (3.19, 3.16, 3.14, `CodeSet`). `Rules::slippi_online()` (3.19 with UCF 0.84) exists and nothing calls it. Every session from power-on, and every match the player starts from the menus, runs Vanilla (`Game::rules` default).
- **The 27 codes are the whole of Slippi's gameplay.** Slippi Dolphin 3.19.1's netplay build runs 288 codes (7,256 lines), and each of Sarah's 156 replays from it records exactly those 288. Slippi's playback leaves 261 of them out of a replay's list (its denylist: every injection not tagged `[affects-gameplay]`) and applies 27, exactly the 27 tagged in the netplay injection list. melee-hd ports all 27: UCF 0.84's eight, neutral spawns, the freeze glitch fix, `Init Stage Data`, `Init Player Data`, off-screen damage from the stage's limits, the three dead-up-fall fixes, Final Destination's background, Pokémon Stadium's four 3.19 hacks, Whispy's direction, NanaDeterminism, Stadium's monitor and wobbling prevention's three. (Counted by the survey's `codes.py` and `slp_codes.py` over Slippi Dolphin's installed `Sys/GameSettings/GALE01r2.ini` and `Sys/Slippi/InjectionLists/list_netplay.json`, read offline, and Sarah's replays' split messages of kind `0x3D`, matched by type and address.)
- **Slippi's unlocks** are the playback set's general codes: Unlock All Characters and Stages (14 `04` writes into the unlock getters), Disable Special Messages and Disable Trophy Messages. `Unlocks::slippi` models their effect in a match (the stage music's draw on Final Destination, Battlefield, Yoshi's Story and Pokémon Stadium reads them; the Pokémon record test makes Celebi impossible); the menus' getters don't take them yet.
- **No card** already runs: the memory card scene with nothing in slot A is ported and compared (`card-sessions`' "no card"); Slippi Dolphin's setup has `SlotA = 255` (no device). On a console, and in melee-hd today, the scene then asks "No memory card in Slot A. Continue without saving?"; Slippi Dolphin skips the question with its Skip Memcard Prompt code (one `C2` at `gm_Scene_MemCard_OnFrame`), which this scope doesn't include ([design call 5](#design-calls)).
- **The recorder** writes 3.19.1's events from the state (`game::record`), with an empty code list and UCF toggles 0 for Vanilla (`record::Setup::vanilla`). Milestone 5's decision that Vanilla writes the save's unlocks into the metadata isn't built.
- **The float mode.** Slippi Dolphin's JIT computes some float operations differently from the Gekko (`gekko_fp::FpMode::Slippi`: fused ops rounded through double, `fnmadd` signs, store rounding, NaN sign, subnormals, `fctiw`), and every Slippi replay the oracle checks plays in that mode. The player never sets a mode, so it runs everything in `FpMode::Hardware`, its `--replay` of Slippi replays included.

## Exact playback in Slippi Dolphin

### What a replay's playback rebuilds, and what it doesn't

Slippi Dolphin plays a `.slp` by booting its playback code set into its own playback scene, then into the debug VS mode's match (scene 14.1) built from the replay's game start: the start data, the seed, nametags, the language, UCF's pad buffer words (carried in the code list) and the code list, whose gameplay codes it applies. Each frame it injects the recorded inputs into the fighters (`Fighter_procInput`, after the game read its own controllers), restores the recorded seed, and ends the match with the replay. It rebuilds nothing else. So a live match plays back exactly only if nothing else reached its gameplay:

| What a live match could read that playback can't rebuild | Under Vanilla from the menus | Under the Slippi ruleset |
| --- | --- | --- |
| Heap addresses (a computer player's DI: Nana's no-knockback DI reads her `Fighter*` in r30 and a caller's r5) | reached: `ics-peach-ps`, recorded in a layout other than playback's, leaves its recording at frame 10,137 when played back ([the oracle](oracle.md), "Recordings") | cleared by NanaDeterminism (3.19 clears both registers) |
| The menus' heap bytes in the match's new blocks (Brinstar's bubbles' timer, Fountain of Dreams' star, Nana's input ring, a collision joint's stamp, Rainbow Cruise's parts, the enemy spawner's words, item links, a dropped fan's word) | reached; modelled from the heap's image | `Fighter` and `Ground` zeroed by `Init Player Data` and `Init Stage Data`; **the rest still reached** (the collision stamp, items', links' and effects' words, and every in-match reuse whose placement depends on the heap's layout) |
| The display pass's phase (a doubled round every 1,001 ticks, where the pass runs once for two ticks) | off-screen damage, Stadium's monitor, Whispy's side, Final Destination's background draws, homing, Onett's cars | fixed by the codes but for **homing and nearest-opponent items through a KO'd fighter's render-placed joint** and **Onett's cars' view edges**, which Slippi doesn't patch ([Melee's bugs](melee-bugs.md#homing-and-nearest-opponent-targeting-through-render-placed-joints)) |
| Disc timing (Pokémon Stadium's transformations load asynchronously) | reached | 3.19 loads them synchronously; nothing else in a match is known to wait on the disc (task 4 confirms) |
| The float mode | the console's | must be Slippi Dolphin's JIT's |
| The unlocks (the music's draw) | the save's | Slippi's, as playback's own codes are |

So the gameplay codes alone don't make a live match's `.slp` exact: the heap's contents and layout still reach a match through the leftovers no code clears, and through which freed block a new object lands on, which depends on where the heap starts and what the match's files took from it.

### Each match from playback's heap

The way to meet the requirement by construction is to start every Slippi-ruleset VS match in the state Slippi's playback gives it, which melee-hd already builds for every replay and checks against ssbm-rs's playback traces.

**Playback's heap**, precisely (`hsd::heap::Layout::slippi_playback`, [the engine notes](engine.md#the-allocators)): Slippi's bootloader raises the console's four OS heaps to six and takes `0xC0` bytes and the playback set's code list (`0x2E00`) from the arena before the boot's heaps; the scene's main heap is the OS's third, `0x80BD8B20` to `0x817F8AC0`, the debug VS mode letting the preload cache's `AllM` heap go (`lbDvdPreload_2`); the recording codes lengthen `Fighter` to `0x2600` bytes; the match's stage and fighters' files are read into the scene's heap, not taken from the selects' preloads (`from_menus` false); the playback codes allocate the replay's code list and the backup of what its codes replace (`playback::CodeListHeap`, sized from the list) and the recording codes their buffers in the scene's heap; and the memory is fresh (`Heap::fresh`: zero as the boot left it, but for the playback scene's one tick). `Settings::from_replay` and `Match::start_from` make exactly this, with the playback and recording GObjs and the scene loop's counts that the playback scene leaves.

**Reaching it from the menus.** At the VS match scene's entry (`fn_8016E730`, where the match takes its `StartMeleeData`) the ruleset takes the game start the recorder writes from it (the start data with its callbacks cleared, the seed, nametags, the language, the code list with UCF's words as they stand) and builds the match from that as `Settings::from_replay` does, in Slippi's float mode; the match runs with the controllers' live polls where playback would inject a replay's frames, and ends by the VS mode's own exit, not the replay's. The menus' session is set aside across the match scene (its allocator model, heap image and carried pools; the match scene's own heap would have been made anew and freed whole anyway) and takes back what the match leaves for the scenes after it: the match's results in `gm`'s static data, the scene's exit data, the audio and the timeline. [Design call 3](#design-calls) weighs this against one continuous model.

**What it buys.** Every leftover a Slippi-ruleset match reads is playback's, so its `.slp` plays back as it played, with no tripwire needed for the heap; the match needs no heap image, and the vanilla layouts (`vanilla_vs`, `from_menus`) never apply to it. The check is direct: the live match and melee-hd's playback of the `.slp` it wrote run the same code from the same state, so they must agree tick by tick on the whole state, but for a named list (`Game::playback`'s replay-frame bookkeeping, the pad library's playback flag and queue, the recorder). `record_tests.rs` already plays a recording back and compares events; this extends it to the whole state every tick.

**What's left outside the construction.** Two display-phase paths Slippi doesn't patch: a homing missile or nearest-opponent item targeting through a fighter in its KO toward the screen, and Onett's cars reading the camera's view edges. Each reads state the display pass placed, and Slippi Dolphin's playback doubles its rounds at its own phase, so a match where such a read falls on a doubled round can play back differently in Slippi Dolphin, as Slippi's own replays and online peers can. melee-hd can't make that exact, since Slippi Dolphin's phase isn't known; it can see it: at each such read, compute the outcome from both placements (the pass's and the logic's) and, where they differ, mark the match ([design call 9](#design-calls)).

### Checking it against ssbm-rs

ssbm-rs can't run Slippi Dolphin's netplay set, and doesn't need to here:

- **The matches.** ssbm-rs's playback (`ssbm-run --replay FILE.slp --fp slippi`) runs Slippi's real bootloader, playback set and the replay's gameplay codes, which is everything that touches a Slippi-ruleset match. Each `.slp` the ruleset writes is traced that way, `melee-oracle diff` compares melee-hd's playback with the trace, and ssbm-rs's own replay check compares its re-recording with the `.slp`. With the live-equals-playback test above, that chains the live match to Slippi's playback codes. No ssbm-rs change.
- **The menus.** A session under the ruleset is Vanilla's menus with no card, the unlock and message writes, and Slippi's float mode. ssbm-rs runs no card already (no `--card`) and `--fp slippi`; it needs a run option to apply the playback set's general codes (16 `04` lines, small enough for the code handler's list) in a recorded and played-back session, and a flag in the session format for the ruleset (task 1). The sessions then compare from power-on to each match's start.
- **After a match** the session runs Vanilla's results and menus from the state the handoff gives back. No console or ssbm-rs run passes through that handoff, so there's no reference past it: those scenes are held by the same code's checks elsewhere, by sessions under the ruleset running with no stop, and by the results showing the match's own outcome.

**What needs recording.** About 6 menu sessions (12 minutes, about 1.3 GB at 1.8 MB a second) and about 20 matches recorded through the player under the ruleset and traced as replays (60 minutes, about 8 GB at 2.3 MB a second): every tournament stage, Ice Climbers and computer players, items with homing missiles and red shells, Onett, Sudden Death, the Special Melee modes that save a `.slp`, a pause. About 10 GB in all, against 138 GB free on 2026-11-14.

## What the ruleset makes moot

What Vanilla needs modelled in a match and a Slippi-ruleset match doesn't, with the win and how the Slippi path skips it safely. Vanilla keeps every one.

| Vanilla's modelling | Why Vanilla needs it | Under the ruleset | Win | Skipped safely by |
| --- | --- | --- | --- | --- |
| The allocator model's addresses (`Settings::heap`, on in the player whenever an Ice Climber or a computer player plays) and the pools' per-address slots | a computer player's DI reads its `Fighter*` (r30) | NanaDeterminism clears it | measured below: speed 0 to +4%, snapshots 16 to 31% smaller and no longer growing through the match | the existing stop by name with the model off (`ics-peach-ps` stops at frame 10,137: "DI with no knockback reads what ftCo_800B2790 (the fighter's address, unmodeled) left isn't ported"), and a run of the replay corpus under 3.19's rules with the model off ([design call 8](#design-calls)) |
| r5 through the computer player's frame (`cpu_ai::Leftover`, `Geometry::check_floor_r5`, `Running::excluded_floor_r5`) | Nana's DI takes her stick's X from it | 3.19's NanaDeterminism clears r5 too | the bookkeeping isn't consulted | an assertion that the Slippi path never reads it; 3.16 replays keep it (their code clears r30 only) |
| The heap's image in a match (`game::leftovers`: Brinstar's timer, Fountain of Dreams' star, Nana's ring, the collision stamp, Rainbow Cruise, the spawner, links, the fan) and the layouts `vanilla_vs` and `from_menus` | the menus' bytes reach the match | the match starts from playback's fresh heap | no image reads at creation and no image in the match's snapshot (138 to 227 KB of a session's, about 3 µs of its clone, [performance](performance.md)); no layout campaign for new match objects | `Heap::fresh`, already there: an image read in a fresh heap is an error |
| Off-screen damage from the display pass's marks (`ft::draw`'s `update_screen_visibility`, the magnifier) | `Fighter_procAnim` counts the pass's mark | BrawlOffscreenDamage tests the stage's limits in logic | damage no longer follows doubled rounds; the marks still draw the bubbles and feed the results' bonus counts | the code's branch already; a test that the Slippi path's damage is the same with the pass's marks scrambled |
| Pokémon Stadium's monitor against the pass's camera | the close-up reads the CObj the pass writes | fixed bounds (`zoom_keeps_player`) | Stadium's draws no longer follow the pass | the same kind of test |
| Whispy's and items' side from KO'd fighters' camera bones | dead fighters counted | WhispyBlowDirFix | as above | the same kind of test |
| Final Destination's background draws | they take the match's seed | 3.19 keeps them off it | as above | ported |
| The disc's pace in a match (Stadium's asynchronous loads) | reaches the seed | 3.19 loads synchronously | the player's pace reaches only the music | task 4 confirms nothing else in a match waits on the disc |
| The save in a match (unlocks, the Pokémon records) | the save's unlocks decide the music's draw | Slippi's, fixed | no save in the match's settings | `Unlocks::slippi` |

The display pass still runs as simulation under the ruleset, and a rollback still re-runs its state changes: homing, nearest-opponent items and Onett's cars read what it places, the hats' dynamics swing from it, and the magnifier's edges feed the results. Skipping it in a rollback's replayed ticks would be a later question (the netplay follow-on's), and safe only where none of those readers runs.

**Measured (2026-11-14, this machine, other work possibly running; `melee-bench` built from `f7115325`, two alternations of five runs each, `MELEE_BENCH_HEAP=off` for the model off).** These are Vanilla benchmark replays, so they show the model's cost, not a Slippi match's:

| Match | Model on, median frames/s | Model off | Change |
| --- | --- | --- | --- |
| `fox-fox-fd` | 4,252, 4,232 | 4,328, 4,356 | +2 to +3% |
| `fox-marth-ps` | 3,660, 3,633 | 3,663, 3,772 | 0 to +4% |

| `fox-fox-fd` (`snapshot-match --every 5000`) | Model on | Model off |
| --- | --- | --- |
| Snapshot at ticks 5,000, 10,000, 15,000 | 1.81, 2.00, 2.26 MB (3,001 to 3,927 allocations) | 1.52, 1.57, 1.57 MB (2,512 to 2,540) |
| Clone | 181 to 235 µs | 145 to 153 µs |
| Restore | 51 to 60 µs | 41 to 43 µs |

So the snapshot win is the per-address slots the model keeps (the exactness round's growth, [performance](performance.md)), and the speed win is small and within this machine's noise. **Code structure**: under the ruleset a match no longer touches the CPU's register bookkeeping, the heap's image or the menus' layouts, so those become Vanilla-only paths behind one ruleset test; the refactor round can isolate them, and its first question (whether the pools need per-address slots) gets a Slippi-ruleset answer for free: they don't.

## Melee's bugs and Slippi's fixes

From [Melee's bugs](melee-bugs.md), the entries the ruleset fixes:

| Entry | Slippi's code |
| --- | --- |
| [The Ice Climbers' freeze glitch](melee-bugs.md#the-ice-climbers-freeze-glitch) | FreezeGlitchFix (`801239A8`) |
| [A computer player's DI with no knockback reads two registers it never set](melee-bugs.md#a-computer-players-di-with-no-knockback-reads-two-registers-it-never-set) | NanaDeterminism (`800AC5B8`, both registers in 3.19) |
| [Nana's input ring starts with the heap's C-sticks](melee-bugs.md#nanas-input-ring-starts-with-the-heaps-c-sticks) | Init Player Data (`80068EEC`) |
| [Brinstar's bubbles' timer starts from the heap](melee-bugs.md#brinstars-bubbles-timer-starts-from-the-heap) | Init Stage Data (`801C154C`) |
| [Fountain of Dreams' star keeps its alpha flag as the heap held it](melee-bugs.md#fountain-of-dreams-star-keeps-its-alpha-flag-as-the-heap-held-it) | Init Stage Data |
| [Rainbow Cruise's planks, blocks and vanishing parts keep fields as the heap held them](melee-bugs.md#rainbow-cruises-planks-blocks-and-vanishing-parts-keep-fields-as-the-heap-held-them) | Init Stage Data, for the `Ground`'s bytes; the fresh heap for the rest |
| [A collision joint's stamp starts as the heap held it](melee-bugs.md#a-collision-joints-stamp-starts-as-the-heap-held-it) | no code; the fresh heap |
| [A dropped fan lands or vanishes by a word it never sets](melee-bugs.md#a-dropped-fan-lands-or-vanishes-by-a-word-it-never-sets) | no code; the fresh heap and the EFB rule of 2026-11-07 |
| [Final Destination's background compares green twice](melee-bugs.md#final-destinations-background-compares-green-twice) | DesyncProofBGTransformations (`8021AAE4`) |
| [Off-screen damage counts render-pass marks](melee-bugs.md#off-screen-damage-counts-render-pass-marks) | BrawlOffscreenDamage (`8006A880`) |
| [Pokémon Stadium's monitor tests a render-pass camera](melee-bugs.md#pokémon-stadiums-monitor-tests-a-render-pass-camera) | PSCameraIndependentMonitor (`801D24FC`) |
| [Opponents' direction counts fighters KO'd toward the screen](melee-bugs.md#opponents-direction-counts-fighters-kod-toward-the-screen) | WhispyBlowDirFix (`8008653C`) |

Not fixed by Slippi: [homing and nearest-opponent targeting through render-placed joints](melee-bugs.md#homing-and-nearest-opponent-targeting-through-render-placed-joints) and Onett's cars (design call 9). Each task that changes how one of these behaves under the ruleset updates the entry's "melee-hd" line.

## Proposed tasks

Six tasks. Sizes: S a few days' agent work or tooling, M a subsystem, L more. Each follows [the porting guide](porting.md): the sessions it names compare clean with `--all --heap`, `session-sync --rollback 7` passes on the matches it reaches, each new check is mutation-tested, and a defect found goes in [Melee's bugs](melee-bugs.md). Sessions go in ssbm-rs's `local/lockstep/slippi-*` folders, traced into `local/traces/slippi-*`.

1. **ssbm-rs: a session under the ruleset** (a request through ssbm-rs's coordinator). A run option applying the playback set's general codes (the 16 unlock and message writes, and Skip Memcard Prompt if design call 5 takes it) from power-on in a recorded or played-back session, with `--fp slippi` and no card, and the ruleset as a flag in the session format, so `--inputs` plays such a session back as it ran. Oracle: the codes' effect on the getters in a trace; a session recorded and played back the same. Size: S. Depends on nothing.
2. **The ruleset in the session and the menus.** `Settings`' ruleset, set from power-on: no card, Slippi's unlocks in the menus' getters with the special and trophy messages off, Slippi's float mode, `Rules::slippi_online()` for the matches; the session format's flag read and written (`session_rec`, melee-oracle). Oracle: task 1's sessions from power-on through the character and stage selects to each match's start. Size: S. Depends on task 1 for its sessions, not to start. Owns the ruleset field and the menus' unlock getters.
3. **The recorder's Slippi game start.** The code list as melee-hd runs it (design call 4) with UCF's words as they stand, UCF toggles 1, the scene, `playedOn` (design call 4), and Vanilla's unlocks in the metadata (milestone 5's open decision). Oracle: Sarah's four offline replays' game starts byte for byte where the same fields apply, `Rules::from_codes` reading the list as 3.19, `melee-oracle record`, slippi-js and ssbm-rs's parser. Size: S. Depends on nothing. Owns `game::record`, `melee-slippi`'s writer.
4. **Each match from playback's heap.** The handoff at the VS match scene (design call 3): the match built from its game start in playback's state and float mode, live polls in, the VS mode's exit and results back; which VS-scene matches playback can't rebuild from their game start (the Special Melee modes' own callbacks, Tournament's, Camera Mode's), which then save no `.slp` (design call 6); the disc's reach into a match confirmed; the display-phase tripwire (design call 9). Oracle: the live match equal to melee-hd's playback of its `.slp` tick by tick (the whole state but the named list), ssbm-rs's playback of each `.slp` with `--fp slippi` compared by `melee-oracle diff` and its replay check clean, about 20 matches recorded through the player; sessions running through several matches with no stop. Size: M to L. Depends on tasks 2 and 3. Owns `vs_match`'s start, `game::playback`'s live variant.
5. **What the ruleset makes moot.** Under the ruleset: the allocator model off in the player's Slippi matches, after a run of the replay corpus under 3.19's rules with the model off finds no stop (design call 8); the r5 bookkeeping not consulted, with its assertion; the tests that the Slippi path's off-screen damage, Stadium monitor and Whispy side don't read the pass; the measurements above repeated on the worker against a Slippi match. Oracle: the corpus run, the tests, `snapshot-match` and `native`. Size: S. Depends on task 4 for the match path; the corpus run and the tests can start at once. Touches `ft::cpu`'s bookkeeping, `Settings::vs_with`.
6. **The player.** The Ruleset setting under a Game section of the panel, its default (design call 2), applied at Melee's next start, never to a replay or a recorded session; the Save section saying the save is Vanilla's while Slippi runs with no card; Save replays' note for Vanilla only; the developer setting Vanilla-only (playback can't rebuild a raised debug level); `--replay` of a Slippi replay in Slippi's float mode; Skip Memcard Prompt if design call 5 takes it. Oracle: the player's own sessions under the ruleset compared from power-on; `record_tests` under the ruleset; Sarah's spot check of a few `.slp` files in her Slippi Dolphin. Size: S. Depends on tasks 2 and 4.

Tasks 1, 2 and 3 can start at once, and task 5's corpus run and tests beside them: four agents. Task 4 follows tasks 2 and 3; task 6 follows task 4.

## Not in this plan

- **Slippi Dolphin's menus, online entries, m-ex and the rest of its netplay set** (Sarah, 2026-11-14). For the record: the set is 288 codes, of which the 261 playback drops are the boot's jump past the title to Slippi's Online Play submenu, about 95 codes of the online scene (its character select and name entry, VS screen, results panels, rollback engine), 75 heap, file, text, video and polling codes (the m-ex set's 53 reshape every scene's heaps), and about 60 offline menu, HUD, pause, stage and audio codes. Wanting any of it later would mean ssbm-rs running the set through Slippi's bootloader with an EXI device giving Slippi Dolphin's offline answers (a probe on 2026-11-14 found its benchmark boot can't hold the set: the 7,256 lines overrun low memory before the first field), a new heap layout in melee-hd, and a ledger of the codes. Its files stay in ssbm-rs's `local/lockstep/slippi-survey/`.
- **Netplay** (the follow-on repository). This plan leaves it the gameplay codes on in live play, matches that start from a state a peer can rebuild from the game start alone, the recorder writing what Slippi's playback reads, and the display-phase reads named.
- **Relaxing Vanilla's leftover modelling**: Sarah's call, after this; the measurements above are its starting point.
- **The refactor round and the player's polish**, each after its own discussion.

## Design calls

| # | Call | Recommendation | Who |
| --- | --- | --- | --- |
| 1 | Which Slippi version's rules | 3.19's gameplay codes, written with Slippi 3.19.1's bytes (the version of Sarah's 156 replays from her current Slippi Dolphin). A newer Slippi joins playback when her replays show it; the live ruleset moves to it only when she says so. | Sarah: compatibility |
| 2 | The default ruleset | Slippi once this milestone's done-when holds, Vanilla until then, both always offered; switching restarts Melee from power-on. | Sarah: the player's default |
| 3 | How a match reaches playback's heap | Build each Slippi-ruleset match as a replay's playback is built, from the game start it records, with the menus' session set aside across the match scene and handed back the results: it reuses the path already exact against ssbm-rs's playback, and the live match can be checked against its own playback tick by tick. The alternative is one continuous model, booting the ruleset with Slippi playback's memory map (six heaps, the bootloader's room, the long `Fighter`) and making each match scene let the `AllM` heap go, load its own files and zero its heap; it keeps the session whole but changes the menus' layout, which ssbm-rs's vanilla menus then can't check. | Sarah: how the requirement is met |
| 4 | What a live `.slp` records | The code list melee-hd runs: the 27 gameplay codes and the 16 unlock and message writes, with 3.19.1's bytes, so Slippi's playback applies the same gameplay codes it would to Slippi Dolphin's own replay; UCF toggles 1; scene 2.2; online fields zero; `playedOn: melee-hd`, which playback doesn't read and which lets tools tell. The alternative, Slippi Dolphin's whole 288-code list, would claim menu and online codes melee-hd doesn't run. | Sarah: `.slp` compatibility |
| 5 | The no-card question at boot | Port Slippi's Skip Memcard Prompt (one code), so the ruleset boots past the question as Slippi Dolphin does; ssbm-rs applies it with the unlock writes. The alternative shows Melee's "Continue without saving?" at every start. | Sarah: what players see |
| 6 | Matches playback can't rebuild | A VS-scene match whose setup isn't wholly in its game start (the Special Melee modes' own callbacks, Tournament's and Camera Mode's, by task 4's audit) runs under Slippi's rules as the console runs it and saves no `.slp`, as Bonus matches and the single-player modes save none; the panel's note names them. | Sarah: what the player offers and records |
| 7 | The single-player modes under the ruleset | Milestone 6's call 5 stands: they play Vanilla's rules, with the session's no card and Slippi's unlocks, and save no `.slp`. | Sarah: gameplay |
| 8 | The allocator model in the player's Slippi matches | Off, the stop by name as its tripwire, once the replay corpus under 3.19's rules runs with it off and nothing stops: about 3% faster and snapshots 16 to 31% smaller on the benchmark measured. The oracle keeps it on and compares addresses as today. | Sarah: performance against a stop |
| 9 | Display-phase reads Slippi doesn't fix (homing and nearest-opponent items through a KO'd fighter, Onett's cars) | Keep them as the original's and Slippi's, compute each such read both ways, and when the outcomes differ note it in the player's log and the `.slp`'s metadata ("may not play back exactly"), never stopping the match. A ruleset fix would change gameplay from Slippi's and break its replays. | Sarah: what "exact" promises |
| 10 | Float mode under the ruleset | Slippi Dolphin's JIT mode for the whole session (menus included), so the matches and their handoff need no switch and ssbm-rs's sessions run `--fp slippi` throughout; Vanilla keeps the console's. | Sarah: exactness |
| 11 | The codes' ledger | Not needed in this scope: the 27 codes and 16 writes are listed in [the oracle](oracle.md)'s ruleset table and checked there; task 3's test checks that the list written is what `Rules::from_codes` reads. | Decided here |

## Decisions (Sarah, 2026-11-14)

- **Scope:** Slippi's gameplay codes in live play, no memory card, Slippi's unlocks; not Slippi Dolphin's boot, Online Play menus, m-ex or netplay code set ("i mostly meant the no-memcard thing and the rules that decouple the bubble rendering from the sim code and fixes the CPU DI issue etc"). Take performance and code-structure wins wherever the ruleset allows ("taking perf/code structure wins wherever we can get it").
- **Exact playback is required:** "to be clear yes .slp files recorded from our slippi ruleset should indeed play back exactly in slippi dolphin".
- **Call 1, version:** 3.19, as recommended. Her newest replays (October 2026) are 3.19.1, and her Slippi Dolphin install (August 2026) is newer than the last public release (3.6.4, June), so 3.19.1 is both what the oracle tested against and current: "if 3.19 is both of those things then yes it is the obvious choice".
- **Calls 2, 5, 8, 10:** as recommended. Call 5: "yes absolutely".
- **Calls 3 and 4:** as recommended, left to the coordinator ("you decide"; "whatever makes the most sense").
- **Call 6:** as recommended, tabled, not conceded: "i feel like we are in a position to at least somehow close that gap and i don't wanna concede those areas as unrecordable. maybe in a follow on repo or by making our own format or something".
- **Call 7, revised:** the single-player modes run under the Slippi ruleset too: Slippi's gameplay codes plus its Required "C-Stick in Single Player" code (`0416B480 60000000`, so the C-stick does smash attacks as in VS). They save no `.slp` while playback can't rebuild them (call 6's gap). Sarah: "if these could be improved by applying the ruleset ... they should. also ... the whole 'c-stick controls the camera in single player modes' thing ... is obnoxious".
- **Call 9:** as recommended.

## Before the tasks start

- **Sarah's design calls**: 1 to 10 above.
- **Task 1's request** to ssbm-rs's coordinator.
- **Sarah's spot check**, once task 4 records its first matches: a few of melee-hd's `.slp` files played in her Slippi Dolphin.
- **The trace campaign**: about 26 sessions and matches, about 10 GB, fits the disk as it stands.
- **The first survey's files** in ssbm-rs's `local/lockstep/slippi-survey/` (Slippi Dolphin's netplay `.ini`, a card copy, the boot probe's log) are background only; `local/traces/slippi-survey/` is empty.
