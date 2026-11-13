# What's left to port

Every function the [port ledger](port-ledger.md) still counts as unaccounted, grouped by the mode that reaches it and by unit, with what reaches it and its size in the executable (bytes of code, from the decomp's asm listings). Generated on 2026-11-13 from `feat/m6-ledger-4` at `643a03c5` ([milestone 6](milestone-6.md), task 11's fourth round): 184 of 19,827 functions unaccounted, against 260 on `100381ae` where the round began, 297 after the third round, 513 after the second and 2,105 before the task (17,914 ported with a check, 941 replaced, 788 dead).

## The short answer

- **184 functions are left**, 37,264 bytes of code.
- **8 need only a check's bookkeeping**: a session or replay diff that compares clean already reaches them. 3 are held for a session where they change something (Kirby's item visibility, Jigglypuff's hat freed as she leaves); 5 are presentation (melee-render's), whose run-time mutations fail no frame tried: an item's light overlay and its owner's color overlay, a horizontal billboard, the texture cache's region, and the matrix marks' clear, which clears marks nothing stores.
- **71 need a recording**: melee-hd has the code (61 held by a `// Ports` comment, a doc, a dispatch arm or an audit's `UNCHECKED`; 10 named in a comment, perhaps inline), but no recorded session or replay reaches them: mostly VS states, fighter commands and item callbacks no recorded match enters.
- **2 need a port or a proof**: 0 an audit names with the path that reaches them, 2 nothing in melee-hd names: two draw-only functions the routes round left for a frames check (`lb_8000CE40`, Kirby losing a hat whose parts went on a bone with no model of its own; `fn_802FF360`, the score popup's empty text callback).
- **103 are debug only** (`ledger/deferred.txt`): code only a raised debug level or the debug menu reaches.
- **No mode is known to reach an unported function outside the debug menu.** Every mode's gate is open; the audits (`menus_audit`, `single_player_audit`, the fighter, items, CPU and stage audits) fail on anything their modes reach unported, and every session and replay diff below that compares clean reached nothing unported (the third round's 359 more sessions, each clean to its trace's end, reached none outside the debug menu's rows). The exceptions are the debug menu's: its Versus page's `CpTp_Nana` on a fighter other than Nana reaches Nana's decision without Popo (`ftCo_800AEFB8`, unported, `ledger/deferred.txt`), and the other debug-only rows. The "nothing known" rows of VS units aren't held by an audit, so no test proves a VS match can't reach them; no recorded one does.

An unaccounted row is one of these, which the columns count:

- **Ported, no check yet**: melee-hd has the code (a `// Ports X; aliased once a check reaches it` comment, a doc comment naming it, a dispatch arm on its address, or an audit's `UNCHECKED`), but no session, replay diff or leaf-check reaches it, so its alias waits.
- **Reached by a clean check, alias held**: ssbm-rs's `CALLS` shows a session or replay diff that compares clean reaching it, so melee-hd runs it, inline in another port; it has no item of its own for the alias, its file or row belongs to another agent this round, or it changes nothing the session compares, so no mutation of it could fail there.
- **Reached, presentation**: reached the same way, but ported in melee-render or melee-audio, which a session doesn't compare: a frames check with a mutation checks it.
- **An audit says what reaches it**: the fighter, items, CPU, stage, menus or single-player audit names it (or its unit) with the path that reaches it, usually a single-player mode behind a gate, or why nothing does.
- **Named in melee-hd**: a comment in melee-hd names it, so it may be ported inline; nothing checks it.
- **Nothing known reaches it**: no port, audit or check names it. Most are per-state callbacks of states no check enters.
- **Debug only**: `ledger/deferred.txt`, code only a raised debug level reaches.

How the sweep decided (task 11, first round): the SDK's, MSL's, the runtime's and HSD's native replacements went to `ledger/replaced.txt` (with the crash screen, the disc's file layer and the ARAM queue); `ledger/dead.txt` took the functions nothing can call (only three have no call or table entry at all: MWCC addresses a file's data from one base, so a table no symbol names can still be read, and the asm listings alone prove little else dead) and the AXFX reverb hi and chorus, which no caller selects; `ledger/deferred.txt` the USB host's libraries. The aliases came from ssbm-rs's `CALLS` over 39 checks that compare clean (19 replay diffs of verify-par.sh, 7 sessions of session-expect.txt, 13 more replay diffs of the characters), which logged every unaccounted function they call; each one reached got its alias where melee-hd ports it, with a `ledger/checks.txt` line naming those checks. The render state setters were aliased with the Fox references' frames checks, each one's copy broken on purpose to see them fail.

The second round (2026-11-11) ran ssbm-rs's `CALLS` with every row then unaccounted over 107 sessions that compare clean from power-on: the 13 milestone-6 recordings (`rec-classic-clear`, `rec-allstar-clear`, Adventure's `rec-2026-10-06_12-04-48`, seven events', Home-Run's, Training's and Master Hand's) and 94 of verify-par.sh's 227 milestone-6 sessions, one of each folder first, then the rest in order as far as the budget went (`m6-stages-1p`, whose run doesn't end where the original halts, and the 132 after it are left for a later round). Each run left out what an earlier one had reached. Every reached function melee-sim ports got its alias and a `ledger/checks.txt` line naming the sessions (presentation code excepted, above); the audits' stale entries went with them (the Fox blaster's demo states and Dr. Mario's pill pickup, reached by Classic's demo poses; the stage enemy spawner's `it_8026C1E8`, reached by Adventure; the debug intro's `onEnterIntroEasy`, `un_80301BA8` and `un_80301B48`).

The third round (2026-11-12, `feat/m6-ledger-3`) ran ssbm-rs's `CALLS` with every row still unaccounted (but the second round's reached ones) over the 359 sessions no round had swept, each bounded by its trace's end: the rest of verify-par.sh's milestone-6 list (`m6-stages-1p` cut at field 36,100, just before the field where the original halts; the replay round's `rr-*` sessions with their runner hooks; the guard, unlock, event, Adventure and routes sessions added since) and milestone 5's menu, card, data, trophy, special, results, tie, exit, unlock, Tournament, select, vanilla and debug sessions (the card pulls, slot B, stamp, boot-B, clock and no-card sessions left out: ssbm-rs's runner would need their options). No run hung. They reached 51 functions unaccounted on main `5e8ed352`; 28 melee-sim ports got their aliases with `ledger/checks.txt` lines naming the session and the field of its first call (the barrel cannon's rider states and launch from Event 26, Jigglypuff's full-charge Rollout states from `dp-rollout`, Ness's yo-yo damage and item visibility, Great Bay's sea test, the debug pause's Z, the results' back sound, the grab's high flinch switch, How to Play's frame, the title's exit from the attract loop, Tournament's handicap, Pikachu's ear models, Pichu's item pickup, Kirby's Yoshi-tongue release), the barrel's countdown and Ness's item visibility each with a mutation the session caught. The rest are inline, other agents' rows, held for a session where they change something, presentation, or debug only (`ledger/deferred.txt`'s Hanyu select, debug menu, item and shadow rows, which the debug sessions reach and which stay deferred).

The fourth round (2026-11-13, `feat/m6-ledger-4`) settled the rows the three sweeps had found reached but left, and the rows that needed a port or a proof. Aliased with `ledger/checks.txt` lines: 56 melee-sim ports inline in another item (the command interpreter's own commands, the catch-up's skips, a color overlay's owner commands, the common attacks' kind switches and entries, the common and Ness's up special's empty IASAs, getters, the material classes' registrations, HSD's animation walks, the depth-sorted lists' display and clear, the scene machine's hooks, Peach's Castle's moat reading the previous position), each carried by the item that runs it, three of them broken on purpose to see their checks fail; 8 melee-render ports with a frames check and a run-time mutation each (the screen camera, the envelope matrices' slots, the particles' TEV cache, two kinds of billboard, the shared-vertex and flat fighters' rigid matrices, the magnifier bubble's background); and Event 51's three-stock note in the save, checked by review (Sarah, 2026-11-10). Dead: refraction's three texel readers (no code calls a read callback), the depth-sorted lists' sort (`zsort_sorting` is never set), the top- and bottom-half cameras (no render pass is ever 1 or 2) and the THP decoder's APP1 to APP15 skip (`tools/disc-scan/thp_markers.py`: no frame of the disc's 103 movies and pictures holds one). Replaced, with Sarah's reset and disc error decision: the disc error screen's check and frame callback and the sound stop of the reset and disc error paths. Debug only: the Test stage's other twelve callbacks. Seven rows the generator had as nothing known turned out ported and held (a skip of the catch-up, the ceiling tech's empty IASA, a 1P spike bonus, Kirby swallowing a Kirby, Classic's third opponent in the intro, a flat fighter's shared-vertex matrices) and wait for a recording.

## Summary

| Mode | Functions | Bytes | Ported, no check yet | Reached by a clean check, alias held | Reached, presentation: wants a frames check | An audit says what reaches it | Named in melee-hd, no check yet | Nothing known reaches it | Debug only |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| [VS and the menus](#vs-and-the-menus) | 67 | 8,140 | 55 | 3 | 2 | 0 | 5 | 2 | 0 |
| [Libraries](#libraries) | 11 | 5,284 | 3 | 0 | 3 | 0 | 5 | 0 | 0 |
| [Classic](#classic) | 1 | 36 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| [1P shared](#1p-shared) | 1 | 144 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| [1P endings](#1p-endings) | 1 | 268 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| [Debug only](#debug-only) | 103 | 23,392 | 0 | 0 | 0 | 0 | 0 | 0 | 103 |
| **All** | **184** | **37,264** | **61** | **3** | **5** | **0** | **10** | **2** | **103** |

## VS and the menus

Reached from the open modes; nothing listed stops a match the checks run (above).

### Game libraries (`lb*`, `cm`, `mp`, `db`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/lb/lb_00B0` | 2 | 180 | named in `melee-render/src/hsd/material.rs` (perhaps ported inline); no check reaches it yet: `lbGetFreeAlphaRegImpl`; nothing known reaches it: `lb_8000CE40` |
| `melee/lb/lbaudio_ax` | 1 | 492 | ported in `melee-sim/src/lb/sound_obj.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_800251EC` |

### The player library (`pl*`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/pl/plbonus` | 1 | 124 | ported in `pl/death.rs` (held by a `// Ports` comment: the spike's one less in a 1P stage's bonuses); no check reaches it yet: `pl_80038898` |
| `melee/pl/plbonuslib` | 1 | 36 | named in `pl/trick.rs` (perhaps ported inline); no check reaches it yet: `pl_8003FE40` |

### Fighters: the shared libraries (`ftaction`, `ftlib`, `ftcommon`, `ftcoll`, `ftcolanim`, `ft_*`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/ft/ftaction` | 2 | 32 | ported in `ft/commands.rs` (the catch-up's `skip_8007168c`, dispatched by index); no check reaches it yet: `ftAction_8007168C`; ported in `ft/commands.rs` (the catch-up's `skip_80072c5c`, dispatched by index); no check reaches it yet: `ftAction_80072C5C` |
| `melee/ft/ftparts` | 1 | 752 | ported in `melee-render/src/hsd/draw.rs` (held by a `// Ports` comment: no frame checked draws a flat fighter's shared-vertex shape); no check reaches it yet: `ftPartsSetupSharedVtxMtx` |
| `melee/ft/ftcommon` | 2 | 100 | ported in `ft/phys.rs` (held by a `// Ports` comment); no check reaches it yet: `ftCommon_ClampFallSpeed`, `ftCommon_8007F7B4` |
| `melee/ft/ftdevice` | 1 | 32 | reached by rec-allstar-clear; presentation (`melee-render/src/hsd/material.rs`, an item in its owner's color overlay), held by a `// Ports` comment: its mutation fails none of the 16 frames tried: `ftCo_800C0674` |

### Fighters: the common states (`ftCo_*`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/ft/kinds/ftCommon/ftCo_Damage` | 3 | 400 | named in `ft/donkey/heavy.rs` (perhaps ported inline); no check reaches it yet: `ftCo_ScaleBy154`, `ftCo_8008D8E8`; named in `ft/resolve.rs` (perhaps ported inline); no check reaches it yet: `ftCo_8008EB58` |
| `melee/ft/kinds/ftCommon/ftCo_ThrownKirby` | 1 | 36 | ported in `ft/co/capture_kirby.rs` (held by a `// Ports` comment: Kirby swallowing a Kirby with a copy); no check reaches it yet: `ftCo_800BDA50` |
| `melee/ft/kinds/ftCommon/ftCo_PassiveCeil` | 1 | 4 | ported in `ft/co/dispatch.rs` (an empty arm, dispatched by address); no check reaches it yet: `ftCo_PassiveCeil_IASA` |
| `melee/ft/kinds/ftCommon/ftCo_CatchPull` | 1 | 80 | ported in `ft/co/grab.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_800DA004` |
| `melee/ft/kinds/ftCommon/ftCo_CatchWait` | 1 | 80 | ported in `ft/co/grab.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_800DA440` |
| `melee/ft/kinds/ftCommon/ftCo_CatchAttack` | 1 | 80 | ported in `ft/co/grab.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_800DA618` |
| `melee/ft/kinds/ftCommon/ftCo_0DC2` | 2 | 132 | ported in `ft/co/grab.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_800DC384`, `fn_800DC404` |
| `melee/ft/kinds/ftCommon/ftCo_Throw` | 2 | 248 | ported in `ft/co/throw.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_800DD5EC`, `fn_800DD684` |

### Fighters: the characters' own states

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/ft/kinds/ftMario/ftmario` | 1 | 72 | ported in `ft/mario/init.rs` (dispatched by address); no check reaches it yet: `ftMr_Init_OnItemVisible` |
| `melee/ft/kinds/ftCaptain/ftcaptain` | 2 | 144 | ported in `ft/captain/init.rs` (held by a `// Ports` comment); no check reaches it yet: `ftCa_Init_OnItemInvisible`; ported in `ft/captain/init.rs` (dispatched by address); no check reaches it yet: `ftCa_Init_OnItemVisible` |
| `melee/ft/kinds/ftKirby/ftkirby` | 2 | 144 | reached by rr-mode07: ported (`ft/kirby/init.rs`), held: they change nothing there (no part animation on Kirby's item hand), so no session can fail on them yet: `ftKb_Init_OnItemInvisible`, `ftKb_Init_OnItemVisible` |
| `melee/ft/kinds/ftDonkey/ftdonkey` | 2 | 144 | ported in `ft/donkey/init.rs` (held by a `// Ports` comment); no check reaches it yet: `ftDk_Init_OnItemInvisible`, `ftDk_Init_OnItemVisible` |
| `melee/ft/kinds/ftNess/ftnessspeciallw` | 11 | 1,020 | ported in `ft/ness/special_lw.rs` (dispatched by address); no check reaches it yet: `ftNs_SpecialLwTurn_Anim`, `ftNs_SpecialAirLwTurn_Anim`, `ftNs_SpecialLwTurn_IASA`, `ftNs_SpecialAirLwTurn_IASA`, `ftNs_SpecialLwTurn_Phys`, `ftNs_SpecialAirLwTurn_Phys`, `ftNs_SpecialLwTurn_Coll`, `ftNs_SpecialAirLwTurn_Coll`, `ftNs_SpecialAirLwHit_IASA`, `ftNs_SpecialAirLwHit_Phys`, `ftNs_SpecialAirLwHit_Coll` |
| `melee/ft/kinds/ftPopo/ftpopo` | 1 | 72 | ported in `ft/popo/init.rs` (held by a `// Ports` comment); no check reaches it yet: `ftPp_Init_OnItemVisible` |
| `melee/ft/kinds/ftPopo/ftpopospecialhi` | 5 | 480 | ported in `ft/popo/special_hi.rs` (held by a `// Ports` comment); no check reaches it yet: `ftPp_SpecialHi_801217EC`, `ftPp_SpecialHi_80121CE0`, `ftPp_SpecialHi_80121FD8`, `ftPp_SpecialHi_80122038`, `ftPp_SpecialHi_801222E8` |
| `melee/ft/kinds/ftNana/ftnanaspecialhi` | 1 | 108 | ported in `ft/popo/nana_special_hi.rs` (held by a `// Ports` comment); no check reaches it yet: `ftNn_Init_80123720` |
| `melee/ft/kinds/ftSamus/ftsamus` | 2 | 144 | ported in `ft/samus/init.rs` (held by a `// Ports` comment); no check reaches it yet: `ftSs_Init_OnItemInvisible`, `ftSs_Init_OnItemVisible` |
| `melee/ft/kinds/ftZelda/ftzeldaspecialhi` | 1 | 32 | ported in `ft/seak/special_n.rs` (dispatched by address); no check reaches it yet: `ftZd_SpecialHi_Phys` |
| `melee/ft/kinds/ftPurin/ftpurin` | 1 | 92 | reached by rt-train: ported (`ft/unload.rs`), held for a Jigglypuff in a hat leaving, which its hold names: `ftPr_Init_8013C494` |
| `melee/ft/kinds/ftPurin/ftpurinspecialn` | 1 | 4 | ported in `ft/purin/special_n.rs` (dispatched by address); no check reaches it yet: `ftPr_SpecialAirNStartTurn_IASA` |
| `melee/ft/kinds/ftDrMario/ftdrmario` | 1 | 72 | ported in `ft/drmario/mod.rs` (dispatched by address); no check reaches it yet: `ftDr_Init_OnItemInvisible` |
| `melee/ft/kinds/ftPichu/ftpichu` | 1 | 84 | ported in `ft/pichu/mod.rs` (dispatched by address); no check reaches it yet: `ftPc_Init_OnItemDrop` |

### The scene machine, VS mode and the menus (`gm`, `mn`, `ty`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/gm/gmtou_1` | 2 | 1,760 | ported in `gm/tournament/bracket.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_801967E0`, `fn_80197FD8` |

### Items: the item engine (`it_26B1`, `it_2725`, `itanimlist`, `item`, `itcoll`, `itmaterial`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/it/itmaterial` | 1 | 376 | reached by items-pokeball1-fd, sel1p-target; presentation (`melee-render/src/hsd/material.rs`, an item's light overlay): its mutation fails no frame of the standard set (`feat/m6-frames`): `it_80277F90` |

### Items: the kinds' callbacks

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/it/kinds/ittarucann` | 1 | 88 | ported in `it/kinds/tarucann.rs` (held by a `// Ports` comment); no check reaches it yet: `it_3F14_Logic5_Dropped` |
| `melee/it/kinds/itfoxblaster` | 3 | 340 | ported in `it/kinds/foxblaster.rs` (dispatched by address); no check reaches it yet: `itFoxblaster_UnkMotion9_Anim`, `itFoxblaster_UnkMotion9_Phys`, `itFoxblaster_UnkMotion9_Coll` |
| `melee/it/kinds/itlucky` | 4 | 152 | ported in `it/kinds/lucky.rs` (held by a `// Ports` comment); no check reaches it yet: `itLucky_Logic44_EnteredAir`, `it_802D5A2C`, `it_802D5A64`, `it_802D5A68` |

### The HUD (`if*`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/if/if_2FF2` | 1 | 4 | nothing known reaches it: `fn_802FF360` |

## Libraries

Classified by reach in the second round: the `CALLS` sweep took in the SDK's and HSD's rows once the first run had found the ones called millions of times a frame (`get_shape_normal_xyz`, `WriteMTXPS4x3`, `HSD_Index2PosNrmMtx`) and dropped them. What melee-sim ports went to aliases; what's left reached is melee-render's (frames checks with a mutation each) or inline. The console's reset and reboot are in `ledger/replaced.txt` (melee-hd restarts from power-on).

### The SDK and runtime (`dolphin`, MSL)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `MSL/printf` | 1 | 736 | named in `melee-sim/src/msl.rs` (perhaps ported inline); no check reaches it yet: `longlong2str` |
| `dolphin/gx/GXInit` | 1 | 124 | reached by sel1p-target; presentation (`melee-render/src/gx/init.rs`): its mutation fails no frame, since the checks don't compare where a texture sits in the cache (`feat/m6-frames`): `__GXDefaultTexRegionCallback` |
| `dolphin/gx/GXPixel` | 1 | 456 | ported in `melee-render/src/gx/pixel.rs` (its doc names it); no check reaches it yet: `GXInitFogAdjTable` |

### HSD's library (`sysdolphin`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `sysdolphin/baselib/pobj` | 2 | 2,300 | named in `melee-sim/src/hsd/pobj.rs` (perhaps ported inline); no check reaches it yet: `get_shape_nbt_xyz`; reached by sel1p-target; presentation (`melee-render/src/hsd/draw.rs`): it clears matrix marks `HSD_PObjSetMtxMark` never stores, so no mutation of it can change a frame: `HSD_PObjClearMtxMark` |
| `sysdolphin/baselib/jobj` | 1 | 68 | named in `melee-sim/src/hsd/jobj.rs` (perhaps ported inline); no check reaches it yet: `HSD_JObjAddDObj` |
| `sysdolphin/baselib/displayfunc` | 1 | 692 | reached by rec-classic-clear; presentation (`melee-render/src/hsd/draw.rs`, a horizontal billboard), held by a `// Ports` comment: its mutation fails none of the 27 frames tried: `mkHBillBoardMtx` |
| `sysdolphin/baselib/robj` | 2 | 304 | named in `melee-sim/src/hsd/robj.rs` (perhaps ported inline); no check reaches it yet: `RObjUpdateFunc`; ported in `melee-sim/src/hsd/robj.rs` (its doc names it); no check reaches it yet: `expLoadDesc` |
| `sysdolphin/baselib/texp` | 1 | 352 | ported in `melee-sim/src/hsd/texp/mod.rs` (its doc names it); no check reaches it yet: `AssignColorReg` |
| `sysdolphin/baselib/psdisp` | 1 | 252 | named in `melee-render/src/hsd/psdisp.rs` (perhaps ported inline); no check reaches it yet: `getColorMatAmb` |

## Classic

### Classic's mode and intro (`gmclassic`, `gm_1832`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/gm/gm_1832` | 1 | 36 | ported in `gm/intro.rs` (held by a `// Ports` comment: Classic's intro with a third opponent); no check reaches it yet: `fn_80184A94` |

## 1P shared

### The single-player matches' setups, records and clear screens (`gm_16A2`, `gm_1601`, `gm_17AD`, `gm_17C0`, `gm_17BA`, `gm_18A1`, `gm_16F1`, `gm_1A3F`, `gmregclear`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/gm/gm_1601` | 1 | 144 | ported in `gm/tournament/bracket.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_801642A0` |

## 1P endings

### Game overs, endings, trophy fall, staff roll (`gm_19EF`, `gmgover`, `gmmovieend`, `gmstaffroll`, `gmregtyfall`, `gm_1A7A`, `gm_1A9B`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/gm/gm_19EF` | 1 | 268 | ported in `gm/endings/game_over.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_801A0B60` |

## Debug only

### Debug code (`ledger/deferred.txt`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/lb/lbshadow` | 1 | 376 | debug only (`ledger/deferred.txt`): `lbShadow_8000F214` |
| `melee/ft/kinds/ftCommon/ftCo_0A01` | 1 | 728 | debug only (`ledger/deferred.txt`): `ftCo_800AEFB8` |
| `melee/ft/kinds/ftZelda/ftzelda` | 1 | 92 | debug only (`ledger/deferred.txt`): `ftZd_Init_801395C8` |
| `melee/gm/gm_1601` | 1 | 176 | debug only (`ledger/deferred.txt`): `gm_80164A0C` |
| `melee/gm/gmhanyucss` | 2 | 284 | debug only (`ledger/deferred.txt`): `gm_801BED3C`, `gm_801BEDA8` |
| `melee/gr/grtest` | 21 | 1,520 | debug only (`ledger/deferred.txt`): `grTest_80206E2C`, `grTest_80206E30`, `grTest_UnkStage0_OnLoad`, `grTest_UnkStage0_OnStart`, `grTest_8020703C`, `grTest_80207044`, `stageGObj0_OnInit`, `grTest_8020715C`, `grTest_80207164`, `grTest_80207168`, `stageGObj2_OnInit`, `grTest_802071BC`, `grTest_802071C4`, `grTest_802073AC`, `stageGObj2_X8Callback`, `stageGObj1_OnInit`, `grTest_802073FC`, `grTest_80207404`, `grTest_80207408`, `grTest_8020740C`, `grTest_80207414` |
| `melee/db/dbitem` | 2 | 200 | debug only (`ledger/deferred.txt`): `db_DisableItemSpawns`, `fn_80225E6C` |
| `melee/db/dbcamera` | 1 | 304 | debug only (`ledger/deferred.txt`): `fn_80227EB0` |
| `melee/it/kinds/itmetamon` | 1 | 4 | debug only (`ledger/deferred.txt`): `it_802D306C` |
| `melee/if/if_3004` | 1 | 80 | debug only (`ledger/deferred.txt`): `un_80301D7C` |
| `dolphin/hio/hio` | 14 | 3,296 | debug only (`ledger/deferred.txt`): `ExtHandler`, `ExiHandler`, `DbgHandler`, `TxHandler`, `RxHandler`, `HIOEnumDevices`, `HIOInit`, `HIOReadMailbox`, `HIOWriteMailbox`, `HIORead`, `HIOWrite`, `HIOReadAsync`, `HIOWriteAsync`, `HIOReadStatus` |
| `dolphin/mcc/mcc` | 30 | 9,016 | debug only (`ledger/deferred.txt`): `callbackEventStream`, `MCCStreamOpen`, `MCCStreamClose`, `MCCStreamWrite`, `LoadChannelInfo`, `FlushChannelInfo`, `SetChannelInfoDirty`, `MakeMemoryMap`, `SearchFreeBlocks`, `NotifyCompulsorily`, `WaitAMinute`, `MailboxCheck`, `MCCExiCallback`, `MCCTxCallback`, `MCCRxCallback`, `mccInitializeCheck`, `MCCInit`, `MCCExit`, `MCCEnumDevices`, `MCCGetFreeBlocks`, `MCCGetLastError`, `MCCGetChannelInfo`, `MCCGetConnectionStatus`, `MCCNotify`, `MCCSetChannelEventMask`, `MCCOpen`, `MCCClose`, `MCCRead`, `MCCWrite`, `MCCCheckAsyncDone` |
| `dolphin/mcc/fio` | 13 | 2,980 | debug only (`ledger/deferred.txt`): `fioMccChannelEvent`, `FIOInit`, `FIOExit`, `FIOQuery`, `FIOFopen`, `FIOFclose`, `FIOFwrite`, `fioPacketMakeHeader`, `fioPacketSendPacket`, `fioPacketReceiveResult`, `fioPacketReceiveDone`, `fioPacketWrite`, `fioPacketResultWrite` |
| `dolphin/os/OSMutex` | 3 | 428 | debug only (`ledger/deferred.txt`): `__OSCheckMutex`, `__OSCheckDeadLock`, `__OSCheckMutexes` |
| `dolphin/os/OSThread` | 2 | 2,028 | debug only (`ledger/deferred.txt`): `CheckThreadQueue`, `OSCheckActiveThreads` |
| `sysdolphin/baselib/hsd_392C` | 3 | 396 | debug only (`ledger/deferred.txt`): `fn_80392CCC`, `fn_80392E2C`, `hsd_803931A4` |
| `sysdolphin/baselib/hsd_3933` | 6 | 1,484 | debug only (`ledger/deferred.txt`): `fn_803932D0`, `hsd_80393328`, `hsd_80393440`, `hsd_80393840`, `hsd_80393A04`, `hsd_80393A54` |
