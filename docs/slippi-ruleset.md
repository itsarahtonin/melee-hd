# The Slippi ruleset

A survey of what the Slippi ruleset needs for live play and a plan for splitting it into small agent tasks, each checkable on its own. Surveyed on 2026-11-14 from `main` at `f7115325`, the day milestone 6's last rows landed and before its full verify; the counts are that day's. Sarah numbers the milestone.

What's settled (Sarah's words):

- 2026-10-29: "ultimately i intend to implement the slippi ruleset and default to it (may even allow for some optimizations since we don't have to model as many memory addresses and stuff due to this kind of thing)". A Slippi ruleset with Slippi's boot and menu codes ported becomes the player's default, so its replays play back exactly in Slippi Dolphin; Vanilla stays exact as an option. Until then the vanilla menus' heap leftovers stay modelled exactly ([milestone 5](milestone-5.md#design-calls)), and dropping that modelling is Sarah's call, not this plan's.
- 2026-10-23: under the Slippi ruleset the VS unlocks are always Slippi's, so `.slp` replays play exactly in Slippi Dolphin; Vanilla uses the save's and writes them into the `.slp` metadata.
- 2026-11-07, "Yes, follow Dolphin": under the Slippi ruleset and in `.slp` playback, a game read of the GPU's pixels sees the bytes from before the copy, as Slippi Dolphin with "Store EFB Copies to Texture Only" does; vanilla keeps the console's rule (`Game::copies_reach_memory`, [the engine notes](engine.md#uninitialized-values)).
- 2026-10-16: no more players' own Gecko codes ("we can let the buck stop here"). The rulesets are Vanilla, Slippi and the UCF versions.
- 2026-11-13: after milestone 6 comes the Slippi ruleset, then a code-wide refactor with its own goals discussion; the player's polish needs its own larger discussion and isn't in this plan.
- Netplay belongs to a follow-on repository on `melee-sim` (2026-10-14). Neither repository contacts Slippi's servers or online services. Everything below about Slippi's codes comes from public source read offline: Slippi Dolphin's netplay `Sys/GameSettings/GALE01r2.ini` and `Sys/Slippi/InjectionLists` as installed on this machine, the replays' own code lists, ssbm-rs's copy of the playback set, and one file of [slippi-ssbm-asm](https://github.com/project-slippi/slippi-ssbm-asm) (`Online/Slippi Online Scene/boot.asm`) read on GitHub.

The milestone is done when:

- Under the Slippi ruleset melee-hd boots as Slippi Dolphin's netplay setup boots a session nobody takes online: the code set Slippi 3.19.1 forces and recommends ([Scope](#scope)), no memory card, the memory card check, the main menu with Slippi's online entries as an offline Slippi Dolphin shows them, VS mode under Slippi's default rules, the character and stage selects with Slippi's changes, matches with Slippi's gameplay codes, the results and the way back.
- Every Slippi-ruleset session the oracle has compares clean tick by tick from power-on with ssbm-rs's trace of the same session run with the same code set, no stale read, `--heap` included.
- A Slippi-ruleset match's `.slp` holds what Slippi Dolphin 3.19.1 writes for that match, and plays back exactly in melee-hd's and ssbm-rs's playback of Slippi's codes (`melee-oracle record`).
- Each of the set's 288 codes is ported with a check, or shown not to run offline with a tripwire where it would, in a ledger of the codes held by a test as the port ledger is.
- The player offers the ruleset in its settings panel with the default Sarah picks ([design calls](#design-calls)); Vanilla stays exact and its sessions compare as before.

## Status (2026-11-14, `main` at `f7115325`): planned

Nothing started. The design calls below wait for Sarah.

## Scope

### What "the Slippi ruleset" is today

melee-hd runs Slippi's codes in one place: a replay's match. `Rules::from_codes` reads a replay's code list ([the oracle](oracle.md), "Replay playback and the Slippi ruleset"), `game::playback` ports what Slippi's playback set does to the match, and `game::slippi`, `game::ucf` and `game::player_codes` port the gameplay codes where they inject. `Rules::slippi_online()` (the 3.19 set with UCF 0.84) exists but nothing calls it. Every session from power-on, and every match the player starts from Melee's menus, runs Vanilla (`Game::rules` default), and the recorder writes an empty code list for it (`record::Setup::vanilla`).

So for live play the ruleset is everything Slippi Dolphin runs from power-on that playback doesn't: the boot and menu codes, and the system codes under them.

### The code set Slippi Dolphin runs

Slippi Dolphin's netplay build (the one Slippi's launcher installs for play; its files on this machine are dated 2026-08-07) enables, in `Sys/GameSettings/GALE01r2.ini`:

| Section | How it's enabled | Codes | Lines |
| --- | --- | --- | --- |
| Required: General Codes | always | 61 | 670 |
| Required: Slippi Recording | always | 18 | 602 |
| Required: Slippi Online | always (offline sessions too) | 196 | 5,816 |
| Recommended: Normal Lag Reduction | on unless the player turns it off | 2 | 2 |
| Recommended: Apply Delay to all In-Game Scenes | on unless turned off | 3 | 153 |
| Recommended: Lagless FoD | on unless turned off | 8 | 13 |
| **Forced and recommended** | | **288** | **7,256** |
| Optional: Freeze Final Destination Background, Widescreen 16:9, Disable Screen Shake, Center Align 2P HUD, Flash Red on Failed L-Cancel, Show Friendly Player Indicators | off unless turned on | 21 | |

Counted on 2026-11-14 by `codes.py` in the survey's scratch folder, which parses the `.ini` (each `04` code one line, each `C2` its header and body), finds each injection's function and unit in `docs/port-ledger.csv` by address, and marks whether the playback set (ssbm-rs's `crates/slippi/data/playback.ini`) has it and whether melee-sim's Slippi modules name its address. Every code is an `04` write or a `C2` injection. The user's own `User/GameSettings/GALE01.ini` defines a "Boot to CSS" code (`041BFA20 38600002`) but doesn't enable it.

**Which version.** Each of Sarah's 156 replays from Slippi 3.19.1 (May to October 2026) records exactly these 288 codes as its code list, and none of the optional ones (`slp_codes.py`: the replay's split messages of kind `0x3D`, matched by type and address). So the installed build is the one that recorded 3.19.1's replays, and the ruleset this plan ports is **Slippi 3.19.1's netplay set**. Her 47 replays from 3.19.0 record 284 codes, the same as ssbm-rs's `local/gecko/3.19.0.bin` (57,544 bytes, referenced by nothing): all in 3.19.1's, which adds the results screen's three online panel codes and moves `Online/Static/StaticData`. The existing playback rules (`game::slippi::CodeSet`) know 3.19's gameplay codes, which 3.19.0 and 3.19.1 share; the recorder writes 3.19.1's events ([the player](player.md#recording)).

**What else Slippi Dolphin's setup decides.** The `.ini`'s `[Core]` sets `FastDiscSpeed = True`, `PollingMethod = OnSIRead`, `CPUThread = True` and `GPUDeterminismMode = fake-completion`. The user's `Dolphin.ini` has `SlotA = 255` (no device: Slippi Dolphin runs with no memory card) and the SRAM's language 0 (English); `GFX.ini` has `EFBToTextureEnable = True`, the setting the 2026-11-07 decision follows. `SlippiOnlineDelay = 2` and `SlippiJukeboxEnabled = False` are the user's Slippi settings, read by the codes over EXI.

### What playback already covers

Of the 288 codes, Slippi's playback leaves 261 out of a replay's list (they're in its denylist, which marks every injection Slippi doesn't tag `[affects-gameplay]`) and applies 27: the [affects-gameplay] ones, exactly the 27 the netplay injection list tags (`list_netplay.json`). melee-hd ports all 27 (UCF 0.84's eight, neutral spawns, the freeze glitch fix, `Init Stage Data`, `Init Player Data`, off-screen damage, the three dead-up-fall fixes, Final Destination's background, Pokémon Stadium's four 3.19 hacks, Whispy's direction, Nana's DI, Stadium's monitor, wobbling's three), so a match's gameplay under the Slippi ruleset is ported already. Sarah's four offline replays from Slippi Dolphin's own menus (VS mode, scene 2.2 or Sudden Death 2.3: `Game_20260524T173350`, `T174154`, `T174859`, `T175101`) are the nearest thing to a live Slippi-ruleset match there is, and the two traced (`T174154`, `T175101`) compare clean through playback today ([the oracle](oracle.md)).

### The other 261 codes, by what they change

Grouped by `classify.py` (by the code's source path and the function it hooks):

| Group | Codes | Lines | What it changes | Reaches |
| --- | --- | --- | --- | --- |
| Gameplay (the 27 above) | 27 | 658 | the match's rules | gameplay; ported |
| Boot and main menu | 11 | 991 | `Online/Slippi Online Scene/boot.asm` (`bootOnLeave`): the boot goes from the memory card check to the main menu, skipping the opening movie and the title, and installs a one-shot over the menu's entry that opens the Online Play submenu (8) with the cursor on its first unlocked entry, after asking the app's state over EXI; it also preloads Pokémon Stadium's four transformation names for later EXI loads. `main.asm` (the scene table: Slippi's online scene), `TitleMenu`'s six (the online submenu, its locked entries, a hidden 1P entry), Skip Memcard Prompt (`gm_Scene_MemCard_OnFrame`), Debug Menu and Exiting Debug Menu Returns to CSS (`mn_8022D594`, `fn_801B09F8`) | the scene sequence, heap, presentation |
| Unlocks, default rules, messages | 21 | 21 | Unlock All Characters and Stages (14 `04` writes, the getters `Unlocks::slippi` already models), Stock Mode, 4 Stocks, 8 Minutes, No Items, Singles Stages (the VS rules' defaults in data), Disable Special Messages, Disable Trophy Messages | menus' state, the save's reads; the music choice's draw ([below](#the-save-and-unlocks)) |
| Character select, offline features | 17 | 210 | Faster CSS Load, UCF's text, D-pad down for a random stage, hands starting on HMN (2), no Fountain of Dreams in doubles, winners' names in gold (2), closing a door keeps the nametag (2), unplugging closes the door, random CSS music (3), KO stars kept from the VS mode, costume bound checks (2) | menus' state, timing, heap, presentation |
| Character select online, name entry | 39 | 1,263 | `Online/Menus/CSS` (15), its teams (7) and the 8-character name entry (21) | the online scene only |
| Stage select | 9 | 196 | L+R+A back to the character select during the stage's load; `Online/Menus/SSS` (8: Start disabled, leaving on a pick, the frozen Stadium toggle's six) | the stage select; the online scene for the toggle |
| VS splash and results (online) | 12 | 239 | the online VS screen (8), the results' panels (3), a rumble hack | the online scene, the results |
| Match HUD, pause and end | 19 | 424 | the in-game HUD's init and number parser, the LRAS message, the online pause rules, Salty Runback (`gmVsMelee_ExitVs`), the camera unrestricted and panned while paused (6), C-stick in single player, hidden nametag while invisible | the match's presentation and its end; the pause camera is simulation state |
| Rollback and netplay engine | 26 | 1,294 | `Online/Core`'s engine loop for rollback, input sending and refetching, rumble, the pad alarm, `Online/Static`'s 15 helper functions, placed at fixed low-memory addresses past `__flush_cache` for the other codes to call, the fast-forward dynamics fix | netplay; inert offline if their guards hold |
| Audio and music | 11 | 191 | `Online/Core/Music` (3, Dolphin's jukebox), music file alarms (2), the sound log (4: a live game's version of playback's), random stage music (`Ground_801C24F8`), crowd chants off | what players hear; the stage music's draw |
| Recording | 12 | 540 | the codes that send the replay over EXI | the `.slp` only; `game::record` reads what they read |
| Heap, files, text, video, polling | 75 | 1,193 | the m-ex code set (53 codes, 874 lines: 14 in `lbheap`, 10 in `gm_1A3F`'s scene loop and preload, 7 in the scene table and scene code (`gmscene`, `gmscdata`, `gm_1A36`), 7 in `tobj`, 6 in the crash screen and debug console (`dberror`, `debugconsole_main`), 5 in HSD's data, 2 in `lbdvd`, 1 in `initialize`, 1 in `lbcardgame`), `AllocSceneBuffer`, a larger text heap, the menus' text from Dolphin over EXI, files loaded over EXI (2), no disc-error or video assert stops, Polling Drift Fix + VB (6: the scene loop's polling and the XFB copy), Normal Lag Reduction (2), the in-game delay (2), compatibility hooks (3), the follower test and frame index | the heap's layout (every scene), the scene loop's timing and its controller reads, the text heap |
| Stage presentation | 9 | 36 | Lagless FoD (8: Fountain of Dreams' reflection and its costs), Pokémon Stadium's screen text | what players see, heap |
| **All** | **288** | **7,256** | | |

So what Slippi forces for every game is all 288 (the three Recommended ones are on unless a player turns them off in Slippi Dolphin's game properties), and its options are the six Optional codes plus the frozen Stadium toggle the online stage select offers. Of the 261, the online scene's codes (about 95: the online select, name entry, VS screen, results panels, the rollback engine and the static helpers) only run once a player enters Slippi Online, which needs Slippi's servers; the rest run in every offline session.

### What's out

- Netplay and everything the online scene reaches past its locked entries (the follow-on's), Slippi's servers and its online services.
- The console code set (Slippi Nintendont's, `list_console_*.json`: tournament and friendlies mods, PAL, stealth UCF): a different ruleset family that no local replay records.
- The single-player modes' own rules under the ruleset beyond what the codes do to them ([design call 7](#design-calls)).

## Inherited

- **The Vanilla unlocks in the `.slp` metadata** (milestone 5's decision of 2026-10-23): not built. Neither `game::record`, `melee-slippi`'s writer nor the player's recording writes them.
- **The "Save replays" note** ("Replays of matches started from Melee's menus may not play back exactly in Slippi Dolphin.") stays true of Vanilla; under the Slippi ruleset it no longer applies ([the player](#the-player)).
- **The EFB rule**: done (`Game::copies_reach_memory` reads `rules.slippi`).
- **Three unaccounted functions** in the ledger at `f7115325` (`ftCo_8008EB58`, `fn_800DC384`, `fn_800DC404`), which milestone 6's verify settles; none is this milestone's.
- **The refactor round's first question** (2026-11-13: whether the class pools' per-address slots are needed): the Slippi ruleset's heap layout (task 5) adds a third layout to the allocator model, so the answer changes its cost; the two are independent otherwise.

## What the oracle can use

A session is still the oracle ([the oracle](oracle.md#sessions)): every controller read from power-on, the card (none, here) and the console's setup. A Slippi-ruleset session adds one input: the code set, applied as Slippi Dolphin applies it, which ssbm-rs runs as machine code in its interpreter as it runs the playback set's. ssbm-rs needs no port of the codes, only the means to run them.

### What ssbm-rs does with Slippi's codes

- **Playback.** `ssbm-slippi` installs Slippi's bootloader and a Slippi EXI device; the bootloader asks the device for the playback set (93 codes) and the device answers playback's commands from the replay. A replay run is a debug VS match from Slippi's playback scene.
- **The benchmark's boot (`BENCH_CODES`).** `tools/run/src/bench.rs` applies an `.ini`'s enabled codes as Dolphin's code handler would, into a list at `0x80001800`, and has the bootloader load the recording codes and the `Common` codes they need from the device (the scene buffer, followers, the frame index, the compatibility hooks, and `Init Stage Data` and `Init Player Data`); the bench's codes boot straight into a match (scene 14.1). That is how its recordings "boot straight into a match with Slippi's codes loaded": the gameplay and recording codes, never the menus'.
- **Not modelled.** No run applies the netplay set. The device answers playback, recording and the bootloader's GCT (`0x75`, `0x76`, `0x88`, `0x89`, `0x8A`, `0xD0`, `0xD3`, `0xD4`), logs `0x77`, `0xD1` and `0xD2` as unsupported, and takes anything else for replay data. The session format (`SSBMINP`) carries no code set, so a session recorded with codes would play back without them.

**The probe (2026-11-14).** `BENCH_CODES` with the netplay `.ini` (its six enabled names, 7,256 lines), `MODES=1`, `--fields 900` and a copy of `2026-10-06_17-13-21`'s card, with `target/alt/release/ssbm-run-rf.exe`: the run stops before its first field (`unmapped write of 4 bytes at 0x00000004`, `pc __start+0xc`), because 7,256 lines from `0x80001800` run over the bootloader's list at `0x80002000` and the SDK's `memset` at `0x80003100`. Slippi Dolphin doesn't put this set there: its bootloader (`Sys/bootloader.gct`) asks the EXI device for the set's length and image and loads it into the arena, as ssbm-rs's playback does with the playback set. So the route is ssbm-rs's own: hand the bootloader the netplay set. The probe's files are in `$SSBM_RS_LOCAL/lockstep/slippi-survey` (the `.ini`, the card copy, `probe-boot.log`); no trace was made.

**What ssbm-rs needs** (requests for its coordinator; task 1):

1. A run option that hands the bootloader the netplay set (an `.ini` and the names to enable, Slippi Dolphin's way), with the recording codes from it rather than the bench's.
2. The Slippi EXI device's offline answers to the set's commands, as Slippi Dolphin's device gives them with nobody logged in or with no online session (the app state `FetchSlippiAppState` asks for, the online status, the delay the in-game delay reads, the premade text `LoadPremadeTextDataFromDolphin` asks for, the file length and load `EXIFileLoad` and the Stadium preload ask for, the follower test, should-record), taken from Slippi Dolphin's public `EXI_DeviceSlippi` source, and the recorded stream saved as a `.slp` as the bench does.
3. The code set in the session format (a flag naming a set, or `SSBMINP` version 3 with the set's hash), so `--inputs` plays a Slippi-ruleset session back with its codes and melee-oracle knows the session's ruleset.
4. No card in slot A, which a run without `--card` already is (the format's flag 2 clear).

### What the local data has

- **Slippi replays.** Sarah's 203 in `$SLIPPI_REPLAYS` (`slp_scenes.py`: 47 of 3.19.0 and 152 of 3.19.1 from the online scene 8.2, 4 offline from VS mode, all with the frozen Stadium flag clear), and ssbm-rs's dataset, 4,878 of 3.14 to 3.17 (10 offline). Every one plays through playback; none records a menu.
- **Sessions.** None under any Slippi code set: every session in `$SSBM_RS_LOCAL/lockstep` is vanilla from power-on.
- **Disk.** `$SSBM_RS_LOCAL/traces` holds 322 GB; the drive had 138 GB free on 2026-11-14, and the project keeps 20 GB free.

### What needs recording

At milestone 6's measured 1.8 MB a second of play for menus and 2.3 for matches, recorded and traced one at a time in ssbm-rs (about a second of one core per second of play):

| Task (below) | Sessions | Minutes of play | Traces |
| --- | --- | --- | --- |
| 2 (first stops) | 4 | 6 | 0.7 GB |
| 3 (boot, main menu, the online submenu, VS menus, rules defaults, the debug menu entry) | 8 | 15 | 1.7 GB |
| 5, 6 (heaps, polling, every scene once more under the m-ex layout) | 6 | 15 | 1.8 GB |
| 7 (the character and stage selects' features) | 8 | 20 | 2.3 GB |
| 8 (matches: each tournament stage, the Ice Climbers, CPUs, items, pause camera, Salty Runback, L+R+A, sudden death) | 12 | 45 | 6.2 GB |
| 9 (music and sounds) | 3 | 10 | 1.3 GB |
| 11, 12 (the recorder's sessions and the player's own) | 4 | 15 | 2 GB |
| **All** | about 45 | about 2 hours | about 16 GB |

That fits the disk as it stands, about 2 CPU-hours spread over the tasks. Each session's `.slp` from ssbm-rs's recording codes is also the recorder's oracle (task 11), byte for byte.

## What changes in melee-hd

### The boot and the menus

Under the ruleset the console boots with no card: the memory card scene finds none and Skip Memcard Prompt takes it past the prompt (melee-sim already runs the no-card case: `card-sessions`' "no card"); `bootOnLeave` goes to the main menu instead of the opening movie; the menu's first entry is the Online Play submenu with its cursor on the first unlocked entry. What the online entries do offline is the EXI answer melee-hd gives (design call 4). VS mode's rules start from Slippi's defaults (stocks, 4 stocks, 8 minutes, items off, the singles stage list), the character select runs Slippi's offline features, the stage select its L+R+A, the match HUD and pause its codes, the results lead back to the select (or straight to the next match with Salty Runback). The single-player modes run under the same codes (C-stick in single player, the unlocks, the special messages off).

melee-sim applies none of this by patching: as with the gameplay codes, each code is ported where it injects, behind `Game::rules` (a ruleset field for the menu codes, next to `slippi`), and the EXI device's answers become explicit inputs, as playback's are.

### Vanilla's leftovers that no longer reach a Slippi-ruleset match

[The engine notes](engine.md#uninitialized-values) list four leftovers that reach play or the picture on a console, all modelled from the heap's image in a session. Under the Slippi ruleset:

- **Brinstar's bubbles' timer** and **Fountain of Dreams' star** live in the stage objects' user data, which `Init Stage Data` (`801C154C`) zeroes: the timer starts from 0 and draws on the match's second tick, the star's bit is clear.
- **Nana's first five input ring entries** live in the `Fighter`, which `Init Player Data` (`80068EEC`) zeroes whole.
- **Nana's and every computer player's DI** reads its `Fighter*` and a caller's r5; NanaDeterminism (`800AC5B8`) clears both registers in 3.19, so no address reaches gameplay through them.
- **A collision joint's stamp** (`CollJoint::xC`) is not zeroed by any Slippi code: it still comes from the heap, under the m-ex layout's history rather than vanilla's.
- **A dropped fan's `xD4C`** and the other item, effect and link words the image supplies are not zeroed either; under the ruleset the fan reads the bytes from before a copy, as decided on 2026-11-07.

melee-sim already skips the image for a `Fighter` and a `Ground` under the Slippi ruleset (`game::leftovers`). What's left reaching a match is the collision stamp and the items' and effects' words, all rare, and all exposed in Slippi Dolphin's own play the same way (a live match's history against playback's fresh heap). How much of the image a Slippi-ruleset match still needs, and whether its matches may start from playback's fresh heap instead, is design call 3; vanilla's modelling stays as it is either way.

### The `.slp` writer

What Slippi Dolphin 3.19.1 writes for an offline match, read from Sarah's offline replays, against what `game::record` writes for Vanilla today:

| Field | Slippi Dolphin, offline | melee-hd's Vanilla today | Under the ruleset |
| --- | --- | --- | --- |
| Code list | the 288 codes, 7,256 lines, as split messages | empty | the same 288, byte for byte (Slippi's GPL-3.0 code text as data, with attribution in `THIRD_PARTY.md`) |
| UCF toggles | 1 (3.19's codes write 1) | 0 | 1 |
| Scene | 2.2 (2.3 in Sudden Death) | 2.2 | the same |
| Frozen Stadium, PAL | 0, 0 offline | 0, 0 | the same |
| Names, connect codes, UIDs, match ID | zero offline | zero | zero |
| `metadata.playedOn` | `dolphin` | `melee-hd` | design call 5 |
| The frame events | 3.19.1's | 3.19.1's, from the state | the same; the frame start's and item processes' GObjs then exist, as the codes make them |

The recorder already reads what the recording codes read; under the ruleset the recording codes run in the game, so the GObjs the recorder stands in for exist and its stand-ins go. The writer's oracle is ssbm-rs's recording of the same session (task 1's device saves it) and `melee-oracle record`.

### The save and unlocks

Slippi Dolphin's netplay setup has no card, so under it Melee never loads or writes a save: the unlocks are the codes' (`Unlocks::slippi`, which the music choice's draw on Final Destination, Battlefield, Yoshi's Story and Pokémon Stadium reads), the rules start from the codes' defaults, the special and trophy messages are off, and records, nametags and Camera Mode snapshots last only as long as the session. Under the ruleset melee-hd can follow that (no card, the native save untouched) or put the native save in slot A as Vanilla does (design call 6).

### The player

- **Settings panel.** A Ruleset choice (Slippi or Vanilla) under a new Game section, applied at Melee's next start as the debug level is ("Restart Melee…"), never to a replay or a recorded session, which carry their own. Its default is design call 2.
- **Save replays.** Under the ruleset the note doesn't apply; under Vanilla it stays.
- **The developer setting** stays a Vanilla tool: Slippi's Debug Menu code is the ruleset's own way in (task 3 reads where it leads).
- **Rollback snapshots.** A Vanilla session's snapshot holds the heap's image (1.9 to 2.5 MB at the measured ticks, against 1.23 MB for a replay's match without one, [performance](performance.md)). A Slippi-ruleset match keeps the image unless design call 3 lets it start from playback's fresh heap, in which case its snapshot is a replay's. The allocator model, which the player turns on only where an address reaches gameplay (an Ice Climber or a computer player, `Settings::vs_with`), has no such reader under the ruleset (NanaDeterminism), so the player's Slippi-ruleset matches can run without it, with a tripwire where an address would be read.

### What this leaves ready for the netplay follow-on

The online scene's codes are the follow-on's: the ledger of codes names each (the online select, name entry, VS screen, results panels, the rollback engine, the static helpers) with the guard that keeps it from running offline. The gameplay codes, the recorder writing Slippi Dolphin's `.slp`, the EXI answers as explicit inputs and the code set in the session format are the pieces a matchmaker and a wire protocol build on. Nothing here talks to a network.

## Melee's bugs and Slippi's fixes

From [Melee's bugs](melee-bugs.md), the entries the Slippi ruleset already fixes or would:

| Entry | Slippi's code | Status |
| --- | --- | --- |
| [The Ice Climbers' freeze glitch](melee-bugs.md#the-ice-climbers-freeze-glitch) | FreezeGlitchFix (`801239A8`) | in the ruleset, ported |
| [A computer player's DI with no knockback reads two registers it never set](melee-bugs.md#a-computer-players-di-with-no-knockback-reads-two-registers-it-never-set) | NanaDeterminism (`800AC5B8`; 3.19 clears both) | in the ruleset, ported |
| [Nana's input ring starts with the heap's C-sticks](melee-bugs.md#nanas-input-ring-starts-with-the-heaps-c-sticks) | Init Player Data (`80068EEC`) | in the ruleset, ported |
| [Brinstar's bubbles' timer starts from the heap](melee-bugs.md#brinstars-bubbles-timer-starts-from-the-heap) | Init Stage Data (`801C154C`) | in the ruleset, ported |
| [Fountain of Dreams' star keeps its alpha flag as the heap held it](melee-bugs.md#fountain-of-dreams-star-keeps-its-alpha-flag-as-the-heap-held-it) | Init Stage Data | in the ruleset, ported |
| [Rainbow Cruise's planks, blocks and vanishing parts keep fields as the heap held them](melee-bugs.md#rainbow-cruises-planks-blocks-and-vanishing-parts-keep-fields-as-the-heap-held-them) | Init Stage Data (the `Ground` part) | in the ruleset for the `Ground`'s bytes |
| [Final Destination's background compares green twice](melee-bugs.md#final-destinations-background-compares-green-twice) | DesyncProofBGTransformations (`8021AAE4`: the seed kept) | in the ruleset; the extra draws no longer reach the seed |
| [Off-screen damage counts render-pass marks](melee-bugs.md#off-screen-damage-counts-render-pass-marks) | BrawlOffscreenDamage (`8006A880`) | in the ruleset, ported |
| [Pokémon Stadium's monitor tests a render-pass camera](melee-bugs.md#pokémon-stadiums-monitor-tests-a-render-pass-camera) | PSCameraIndependentMonitor (`801D24FC`) | in the ruleset, ported |
| [Opponents' direction counts fighters KO'd toward the screen](melee-bugs.md#opponents-direction-counts-fighters-kod-toward-the-screen) | WhispyBlowDirFix (`8008653C`) | in the ruleset, ported |
| [Pokémon Stadium's return zeroes a file's header](melee-bugs.md#pokémon-stadiums-return-zeroes-a-files-header) | CustomZeroBuffer (`801C65C8`, in the same `grAnime_801C65B0` call) | in the ruleset; whether it changes this defect is task 5's to read |
| [A far Arwing's laser takes its spawn position from the stack the sound left](melee-bugs.md#a-far-arwings-laser-takes-its-spawn-position-from-the-stack-the-sound-left) | the live sound log's PreventDuplicateSounds (`8038D0B0`, as playback's) | the ruleset would make live play leave playback's words there (task 9) |
| [A dropped fan lands or vanishes by a word it never sets](melee-bugs.md#a-dropped-fan-lands-or-vanishes-by-a-word-it-never-sets) | none; Slippi Dolphin's EFB setting | decided 2026-11-07; the word's history stays the heap's |
| [A collision joint's stamp starts as the heap held it](melee-bugs.md#a-collision-joints-stamp-starts-as-the-heap-held-it) | none | not fixed by Slippi |
| [Homing and nearest-opponent targeting through render-placed joints](melee-bugs.md#homing-and-nearest-opponent-targeting-through-render-placed-joints) | none ("Slippi doesn't patch it") | not fixed by Slippi |

Every entry's "Fix impact" that says "breaks replay and Slippi compatibility" stays unfixed under both rulesets. Tasks that port a code fixing a defect add the code to that entry's "melee-hd" line.

## What's left to port

Codes, not functions: the port ledger's 19,827 functions don't change, since each code runs inside a function already ported. The ruleset's own ledger (task 2) lists the 288 codes. By group, from the table above: 27 ported; 21 save-facing `04` writes that melee-sim models in part already (`Unlocks::slippi`); 12 recording codes the recorder reads after; and 228 to port or show inert: about 95 online-scene codes (expected inert offline, to show), 75 heap, file, text, video and polling codes (1,193 lines; the m-ex set's 53 the biggest), and about 60 offline menu, HUD, pause, stage and audio codes. Lines are a rough size: the online scene's 3,000 or so lines need reading, not porting; the offline menus', HUD's, stage's and audio's come to about 1,800.

## Proposed tasks

Twelve tasks in three waves. Sizes: S under 20 codes or tooling, M 20 to 60 codes or a subsystem, L more. Every porting task follows [the porting guide](porting.md): each code is ported where it injects behind the ruleset's toggle, with its source path in a doc alias (`#[doc(alias = "slippi:Online/Core/...")]`) the codes' ledger reads, the sessions it names compare clean from power-on with `--all --heap`, `session-sync --rollback 7` passes on the matches it reaches, each new check is mutation-tested, a frame of each new screen compares with ssbm-rs's renderer, and a defect found goes in [Melee's bugs](melee-bugs.md). Sessions go in ssbm-rs's `local/lockstep/slippi-*` folders, traced into `local/traces/slippi-*`.

**Wave 1** (four at once)

1. **ssbm-rs runs the netplay set** (ssbm-rs's coordinator, through melee-hd's): the bootloader option, the offline device's answers and the session format's code set ([What ssbm-rs needs](#what-the-oracle-can-use)). Oracle: Slippi Dolphin's public device source for the answers; the run reaching the main menu, VS mode and a match with the set's codes; the `.slp` its recording codes write playing back through ssbm-rs's playback and comparing with the run (`ssbm-slippi`'s compare). Size: M. Depends on nothing. Touches ssbm-rs's `ssbm-slippi` and `tools/run` only.
2. **The codes' ledger and the session harness.** `ledger/slippi-codes.txt` (each of the 288: ported with its alias, recorded-only, or inert offline with the guard and a tripwire), a test that fails while one is unaccounted, as the port ledger's; melee-oracle reading the session's code set and running melee-sim under it; each group's first stops listed once task 1 lands. Oracle: its own test, then task 1's first sessions. Size: S (tooling). Depends on task 1 for the sessions, not to start. Touches `tools/ledger`, `tools/oracle`'s session, `game::session`.
3. **The recorder's Slippi `.slp`.** The 3.19.1 code list as data (with attribution), UCF toggles, the recording codes' GObjs once the ruleset makes them, `playedOn` per design call 5, and Vanilla's unlocks in the metadata (milestone 5's open decision). Oracle: Sarah's four offline replays' game starts and code lists byte for byte now, ssbm-rs's recordings once task 1 lands, `melee-oracle record` and slippi-js. Size: S. Depends on nothing. Owns `game::record`, `melee-slippi`'s writer.
4. **The online scene, shown inert offline.** For each of the about 95 online-scene codes, the guard that keeps it from running when the app state is offline (the locked entries, the scene the online submenu needs), its tripwire, and the list for the netplay follow-on. Oracle: the codes' instructions (read from the set, cross-checked with slippi-ssbm-asm's sources) and, once task 1 lands, a session pressing every online entry. Size: M (reading). Depends on nothing to start. Touches the codes' ledger.

**Wave 2** (once tasks 1 and 2 land)

5. **The heaps under the ruleset.** The m-ex set (53), `AllocSceneBuffer`, the text heap, the menus' text over EXI, files over EXI, the disc-error and video-assert codes: the allocator model's third layout (`Layout::slippi_netplay`), the heap's image under it, the preload cache's changes. Oracle: every task 3 and 7 session with `--heap`; the allocator model against memory each tick (`vs-heap`'s check). Size: L. Depends on 2. Owns `hsd::heap`'s layouts, `lb::heap`, `lb::dvd`, `game::leftovers` (coordinate with the refactor round's question).
6. **Polling, the video and the delay.** Polling Drift Fix + VB (6), Normal Lag Reduction (2), the in-game delay (2 and its EXI answer): when the scene loop takes its controller reads, and what the session records under them; the player's pacing against it. Oracle: sessions compared read by read and tick by tick; the player's `power_tests`. Size: M. Depends on 2. Touches the scene loop (`gm::scene`), `session_rec`, the player's pacing (coordinate with latency work).
7. **The boot, main menu and VS menus.** No card and Skip Memcard Prompt, `boot.asm`'s jump and one-shot, `main.asm`'s scene table, the six title-menu codes with the offline EXI answers, the debug menu codes, the unlocks and the rules' defaults, the special and trophy messages off. Oracle: task 2's sessions from power-on through the main menu, the online submenu, VS mode's rules and the debug menu entry. Size: M. Depends on 2 (and 5 for `--heap`). Owns the ruleset's field in `Game::rules`, `gm::boot_mode`, `mn::main`.

**Wave 3**

8. **The character and stage selects.** The 17 offline features, the stage select's L+R+A and Start, and the `Online/Static` helpers they call. Oracle: sessions through every feature (random stage, doubles without Fountain of Dreams, nametags kept, a door closed by unplugging, the gold names after a win). Size: M. Depends on 7. Owns `mn::charsel`'s and `mn::stagesel`'s ruleset branches.
9. **Matches, pause and results.** The HUD's init and parser, the pause camera's six, Salty Runback, L+R+A, C-stick in single player, the hidden nametag, Lagless FoD, Stadium's screen text, the in-match sound log live. Oracle: match sessions on every tournament stage, Ice Climbers and CPUs, items, sudden death, a pause with the camera moved, a runback; frames of Fountain of Dreams under Lagless FoD. Size: M. Depends on 7 (and 5, 6). Touches `gm::vs`'s exit, the camera's pause, the HUD.
10. **Music.** The jukebox hooks with the jukebox off, the music file alarms, random stage and CSS music, crowd chants off. Oracle: sessions with the mix compared (`melee-audio-check`). Size: S. Depends on 2. Touches `lb::audio`.
11. **The player.** The Ruleset setting and default, the save per design call 6, the note per ruleset, the online entries' note if any (design call 4), the allocator model off in Slippi-ruleset matches with its tripwire, the cosmetic options per design call 8. Oracle: the player's own sessions recorded under the ruleset and compared; `record_tests` under the ruleset; the cosmetic test for each option. Size: M. Depends on 3, 7 (8 and 9 to finish).
12. **The leftovers under the ruleset.** Which image reads a Slippi-ruleset session still makes (the collision stamp, items, effects, links), measured on wave 2 and 3's sessions with `--leftovers-survey`; and, if design call 3 takes it, the match's start from playback's fresh heap with the tripwire that names a read it would change. Oracle: the sessions and their `.slp` playbacks. Size: S. Depends on 5.

Tasks 1, 3 and 4 can start at once, and task 2 beside them (its ledger needs nothing; its sessions wait for task 1): four agents, with ssbm-rs's work in its own repository. Wave 2's three follow as tasks 1 and 2 land; wave 3 as task 7 does.

### Not in this sequence

- **Netplay** (the follow-on repository): the online scene's codes, matchmaking, the wire protocol.
- **The refactor round** (its goals discussion first) and **the player's polish** (milestone 7, its own discussion).
- **Slippi's optional codes beyond design call 8**, the console code set, and players' own codes (2026-10-16).
- **Relaxing Vanilla's leftover modelling**: Sarah's call once this milestone is done (design call 9).

## Design calls

| # | Call | Recommendation | Who |
| --- | --- | --- | --- |
| 1 | Which Slippi version's rules, and how to follow new ones | Slippi 3.19.1's netplay set as installed and as Sarah's 156 replays from it record (288 codes), pinned by a hash of its code list. A new Slippi version becomes a new set when Sarah updates and her replays show it: its gameplay codes join `game::slippi::CodeSet` for playback (as 3.14, 3.16 and 3.19 did), and the live ruleset moves to it only when Sarah says so, since that changes what players play and what melee-hd writes. | Sarah: compatibility |
| 2 | The default ruleset | Slippi, once this milestone's done-when holds, as Sarah said on 2026-10-29; Vanilla until then, and always offered. A player's choice is kept in `settings.txt`; switching restarts Melee from power-on. | Sarah: the player's default |
| 3 | What a Slippi-ruleset match is exact to | Slippi Dolphin's live run of the same session: the menus' heap reaches the match where no Slippi code clears it (the collision stamp, items' and effects' words), modelled from the image under the m-ex layout, as Vanilla's are. A match then plays as Slippi Dolphin's would, and its replay is as exact in Slippi Dolphin as Slippi Dolphin's own offline replays (Sarah's four are). The alternative, starting every Slippi-ruleset match from the fresh heap Slippi's playback gives it, makes every replay exact by construction and drops the image from those matches (the optimization of 2026-10-29), but differs from Slippi Dolphin's live play in those rare reads and breaks the session's comparison past each match. Task 12 measures how rare before this is final. | Sarah: exactness and the optimization |
| 4 | What the online entries do offline | Answer the codes' EXI questions as Slippi Dolphin does with nobody logged in, so the game itself shows Slippi's online entries locked; no melee-hd note, and no gate, since no game code needs one. Never contact Slippi's servers. | Sarah: what players see |
| 5 | What a live Slippi-ruleset `.slp` records | What Slippi Dolphin 3.19.1 writes offline, byte for byte: the 288 codes, UCF toggles 1, scene 2.2, online fields zero, with `playedOn: melee-hd` kept so tools can tell (Slippi's playback doesn't read it). | Sarah: `.slp` compatibility |
| 6 | The save under the ruleset | No memory card, as Slippi Dolphin's netplay setup has none: the native save stays Vanilla's, untouched by Slippi-ruleset play, and the ruleset's records, nametags and snapshots last a session. The alternative, the native save in slot A under both rulesets, keeps records and nametags but makes the ruleset's boot, heap and menus differ from Slippi Dolphin's (the card's load, Skip Memcard Prompt's other branch). The settings panel's Save section names which ruleset it serves. | Sarah: the save, and what players keep |
| 7 | The single-player modes under the ruleset | As Slippi Dolphin runs them: Slippi's codes stay on (they're the boot's, not the mode's), with the unlocks and special messages they bring; no `.slp` for them (milestone 6's call 8 stands). This revises milestone 6's call 5 ("always vanilla Melee") for the Slippi ruleset only; under Vanilla it stands. | Sarah: gameplay, and it revises an earlier call |
| 8 | Which Slippi options the player exposes | None of the six Optional codes at first: widescreen is melee-hd's own aspect setting with Slippi's fighters drawn (`Cosmetic::draw_offscreen_fighters`), and Recommended's lag reduction and delay are replaced by the player's own present mode and Added delay ("Latency: minimize first"). The three Recommended codes stay in the set, as players run them. Each Optional code that's added later is a cosmetic toggle held by `cosmetic_tests.rs` and written into the `.slp`'s list as Slippi Dolphin writes it. | Sarah: what the player offers |
| 9 | Whether Vanilla's leftover modelling may relax later | Not in this milestone: Vanilla stays exact (Sarah, 2026-10-29). Once the Slippi ruleset is the default, the refactor round can ask whether Vanilla's heap image is worth its cost, with this milestone's measurements of what a Slippi-ruleset session still reads. | Sarah: exactness |
| 10 | The codes' ledger | One line per code (its source path, injection, status: ported with an alias, recorded-only or inert with its guard), held by a test as the port ledger is. | Decided here |
| 11 | ssbm-rs's share | Task 1 is ssbm-rs's, asked through its coordinator: melee-hd doesn't change ssbm-rs, and the session format change is ssbm-rs's (`SSBMINP`). | Decided here |

## Before the tasks start

- **Sarah's design calls**: 1 to 9 above.
- **Task 1's request** to ssbm-rs's coordinator, with this page's [What ssbm-rs needs](#what-the-oracle-can-use).
- **The trace campaign**: about 45 sessions, 2 hours of play, 16 GB, in batches as each task needs them; it fits the disk as it stands (138 GB free on 2026-11-14).
- **The probe's files** in ssbm-rs's `local/lockstep/slippi-survey/` (the netplay `.ini` copied from Slippi Dolphin's install, a card copy, the probe's log); `local/traces/slippi-survey/` is empty. Task 1 can keep or replace them.
