# What's left to port

Every function the [port ledger](port-ledger.md) still counts as unaccounted, grouped by the mode that reaches it and by unit, with what reaches it and its size in the executable (bytes of code, from the decomp's asm listings). Generated on 2026-11-11 from `feat/m6-ledger-2` at `6acf8421` ([milestone 6](milestone-6.md), task 11's second round): 513 of 19,827 functions unaccounted, against 749 when the round began (main `bf791442`), 616 on main `218da290` (with the dead proofs' and other agents' merges) and 2,105 before the task (17,631 ported with a check, 926 replaced, 757 dead).

## The short answer

- **513 functions are left**, 83,740 bytes of code.
- **104 need only a check's bookkeeping**: a session or replay diff that compares clean already reaches them. 69 run inline in another port (or in another agent's file this round) with no item of their own to carry the alias; 35 are presentation (melee-render's GX and HSD ports, melee-audio's mixer), which a frames check with a mutation checks, as the render state setters were in the first round. Since then (`feat/m6-frames`, 2026-11-12), 33 of those 35 and the mixer's `__AXGetPBs` have their frames or mix checks with a mutation each ([run-time mutations](player.md#run-time-mutations)); `it_80277F90` and `__GXDefaultTexRegionCallback` are left, no frame failing with theirs, and the tables below are as generated.
- **258 need a recording**: melee-hd has the code (233 held by a `// Ports` comment, a doc, a dispatch arm or an audit's `UNCHECKED`; 25 named in a comment, perhaps inline), but no recorded session or replay reaches them: mostly VS states, fighter commands and item callbacks no recorded match enters.
- **61 need a port or a proof**: 16 an audit names with the path that reaches them (most say no mode's match does), 45 nothing in melee-hd names (states no code enters, the console's reset, the Test stage, HSD's depth-sorted lists, which Melee never fills).
- **90 are debug only** (`ledger/deferred.txt`): code only a raised debug level or the debug menu reaches.
- **No open-gate mode is known to reach an unported one.** Every mode's gate is open but Adventure's; the audits (`menus_audit`, `single_player_audit`, the fighter, items, CPU and stage audits) fail on anything their modes reach unported, and every session and replay diff below that compares clean reached nothing unported. The exceptions are the debug menu's: its Versus page's `CpTp_Nana` on a fighter other than Nana reaches Nana's decision without Popo (`ftCo_800AEFB8`, unported, `ledger/deferred.txt`), and the other debug-only rows. The "nothing known" rows of VS units aren't held by an audit, so no test proves a VS match can't reach them; no recorded one does.

An unaccounted row is one of these, which the columns count:

- **Ported, no check yet**: melee-hd has the code (a `// Ports X; aliased once a check reaches it` comment, a doc comment naming it, a dispatch arm on its address, or an audit's `UNCHECKED`), but no session, replay diff or leaf-check reaches it, so its alias waits.
- **Reached by a clean check, alias held**: ssbm-rs's `CALLS` shows a session or replay diff that compares clean reaching it, so melee-hd runs it, inline in another port; it has no item of its own for the alias, or its file or row belongs to another agent this round.
- **Reached, presentation**: reached the same way, but ported in melee-render or melee-audio, which a session doesn't compare: a frames check with a mutation checks it.
- **An audit says what reaches it**: the fighter, items, CPU, stage, menus or single-player audit names it (or its unit) with the path that reaches it, usually a single-player mode behind a gate, or why nothing does.
- **Named in melee-hd**: a comment in melee-hd names it, so it may be ported inline; nothing checks it.
- **Nothing known reaches it**: no port, audit or check names it. Most are per-state callbacks of states no check enters.
- **Debug only**: `ledger/deferred.txt`, code only a raised debug level reaches.

How the sweep decided (task 11, first round): the SDK's, MSL's, the runtime's and HSD's native replacements went to `ledger/replaced.txt` (with the crash screen, the disc's file layer and the ARAM queue); `ledger/dead.txt` took the functions nothing can call (only three have no call or table entry at all: MWCC addresses a file's data from one base, so a table no symbol names can still be read, and the asm listings alone prove little else dead) and the AXFX reverb hi and chorus, which no caller selects; `ledger/deferred.txt` the USB host's libraries. The aliases came from ssbm-rs's `CALLS` over 39 checks that compare clean (19 replay diffs of verify-par.sh, 7 sessions of session-expect.txt, 13 more replay diffs of the characters), which logged every unaccounted function they call; each one reached got its alias where melee-hd ports it, with a `ledger/checks.txt` line naming those checks. The render state setters were aliased with the Fox references' frames checks, each one's copy broken on purpose to see them fail.

The second round (2026-11-11) ran ssbm-rs's `CALLS` with every row then unaccounted over 107 sessions that compare clean from power-on: the 13 milestone-6 recordings (`rec-classic-clear`, `rec-allstar-clear`, Adventure's `rec-2026-10-06_12-04-48`, seven events', Home-Run's, Training's and Master Hand's) and 94 of verify-par.sh's 227 milestone-6 sessions, one of each folder first, then the rest in order as far as the budget went (`m6-stages-1p`, whose run doesn't end where the original halts, and the 132 after it are left for a later round). Each run left out what an earlier one had reached. Every reached function melee-sim ports got its alias and a `ledger/checks.txt` line naming the sessions (presentation code excepted, above); the audits' stale entries went with them (the Fox blaster's demo states and Dr. Mario's pill pickup, reached by Classic's demo poses; the stage enemy spawner's `it_8026C1E8`, reached by Adventure; the debug intro's `onEnterIntroEasy`, `un_80301BA8` and `un_80301B48`).

## Summary

| Mode | Functions | Bytes | Ported, no check yet | Reached by a clean check, alias held | Reached, presentation: wants a frames check | An audit says what reaches it | Named in melee-hd, no check yet | Nothing known reaches it | Debug only |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| [VS and the menus](#vs-and-the-menus) | 335 | 42,364 | 222 | 51 | 7 | 14 | 14 | 27 | 0 |
| [Libraries](#libraries) | 68 | 17,132 | 6 | 17 | 28 | 0 | 8 | 9 | 0 |
| [Classic](#classic) | 1 | 36 | 0 | 0 | 0 | 1 | 0 | 0 | 0 |
| [1P shared](#1p-shared) | 3 | 168 | 1 | 1 | 0 | 1 | 0 | 0 | 0 |
| [1P endings](#1p-endings) | 3 | 724 | 3 | 0 | 0 | 0 | 0 | 0 | 0 |
| [1P items](#1p-items) | 1 | 300 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| [Debug stages](#debug-stages) | 12 | 1,464 | 0 | 0 | 0 | 0 | 3 | 9 | 0 |
| [Debug only](#debug-only) | 90 | 21,552 | 0 | 0 | 0 | 0 | 0 | 0 | 90 |
| **All** | **513** | **83,740** | **233** | **69** | **35** | **16** | **25** | **45** | **90** |

## VS and the menus

Reached from the open modes; nothing listed stops a match the checks run (above).

### Game libraries (`lb*`, `cm`, `mp`, `db`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/lb/lbcommand` | 5 | 312 | reached by items-pokeball1-fd, sel1p-target: ported inline, no item of its own to carry the alias: `Command_01`, `Command_02`, `Command_05`; reached by items-pokeball1-fd, rec-mh-mash: ported inline, no item of its own to carry the alias: `Command_03`, `Command_08` |
| `melee/lb/lb_00B0` | 5 | 276 | named in `melee-render/src/hsd/material.rs` (perhaps ported inline); no check reaches it yet: `lbGetFreeAlphaRegImpl`, `lb_8000CD90`, `lb_8000CDA8`; ported in `melee-render/src/hsd/material.rs` (its doc names it); no check reaches it yet: `lbGetFreeAlphaRegister`; nothing known reaches it: `lb_8000CE40` |
| `melee/lb/lbshadow` | 1 | 376 | named in `melee-sim/src/hsd/shadow.rs` (perhaps ported inline); no check reaches it yet: `lbShadow_8000F214` |
| `melee/lb/lbspdisplay` | 5 | 4,932 | reached by rec-classic-clear; presentation (`melee-render/src/hsd/blur.rs`), which a frames check with a mutation checks, not a session: `lb_800122F0`, `lb_8001271C`, `lb_80012994`; named in `melee-render/src/hsd/blur.rs` (perhaps ported inline); no check reaches it yet: `lb_8001285C`; ported in `melee-sim/src/lb/blur.rs` (its doc names it); no check reaches it yet: `fn_800138AC` |
| `melee/lb/lb_0192` | 2 | 812 | reached by items-pokeball1-fd, sel1p-target: ported inline, no item of its own to carry the alias: `lb_80019230`, `lb_800192A8` |
| `melee/lb/lb_0195` | 2 | 216 | nothing known reaches it: `lb_8001955C`; reached by mk-run: ported inline, no item of its own to carry the alias: `lb_80019A48` |
| `melee/lb/lb_0219` | 1 | 4 | named in `melee-sim/src/lb/bgflash.rs` (perhaps ported inline); no check reaches it yet: `fn_80021C18` |
| `melee/lb/lbrefract` | 8 | 804 | nothing known reaches it: `fn_80021F70`, `fn_80021FB4`, `fn_80021FF8`, `fn_8002206C`, `lbRefract_ReadTexCoordRGBA8`, `lbRefract_DObjDispReset`, `fn_80022650`, `fn_80022940` |
| `melee/lb/lbaudio_ax` | 4 | 728 | the console's reset button and the disc error screen, which the player's platform never raises (milestone 5's task 14) (`menus.txt`): `lbAudioAx_80024F08`, `lbAudioAx_80027DBC`; ported in `melee-sim/src/lb/sound_obj.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_800251EC`; ported (a `// Ports` comment in melee-sim), aliased once a session reaches it (`menus.txt`): `fn_800259A0` |

### The player library (`pl*`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/pl/player` | 2 | 292 | reached by items-pokeball2-bf, items-pokeball3-ps: ported inline, no item of its own to carry the alias: `Player_80036978`; reached by rec-classic-clear: ported inline, no item of its own to carry the alias: `Player_80036EA0` |
| `melee/pl/plbonus` | 1 | 124 | nothing known reaches it: `pl_80038898` |
| `melee/pl/plbonuslib` | 3 | 116 | named in `pl/trick.rs` (perhaps ported inline); no check reaches it yet: `pl_8003FE40`; ported in `ft/co/dead.rs` (held by a `// Ports` comment); no check reaches it yet: `pl_8004065C`; reached by rec-classic-clear: ported inline, no item of its own to carry the alias: `pl_800408DC` |

### Fighters: the shared libraries (`ftaction`, `ftlib`, `ftcommon`, `ftcoll`, `ftcolanim`, `ft_*`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/ft/ftaction` | 5 | 128 | nothing known reaches it: `ftAction_8007168C`, `ftAction_80072C5C`; reached by oracle-cpu-pichu-ys: ported inline, no item of its own to carry the alias: `ftAction_80071810`; reached by items-shoot-fd, oracle-cpu-ness-fd: ported inline, no item of its own to carry the alias: `ftAction_80071F0C`; reached by items-pokeball1-fd, sel1p-target: ported inline, no item of its own to carry the alias: `ftAction_80072E24` |
| `melee/ft/ftparts` | 1 | 752 | nothing known reaches it: `ftPartsSetupSharedVtxMtx` |
| `melee/ft/ftcommon` | 3 | 172 | ported in `ft/phys.rs` (held by a `// Ports` comment); no check reaches it yet: `ftCommon_ClampFallSpeed`, `ftCommon_8007F76C`, `ftCommon_8007F7B4` |
| `melee/ft/ftlib` | 3 | 132 | reached by rec-classic-clear: ported inline, no item of its own to carry the alias: `ftLib_GetHipJObj`; reached by rec-2026-10-06_16-58-24, tou-bracket64: ported inline, no item of its own to carry the alias: `ftLib_GetPrevPos`; reached by items-pokeball2-bf, items-pokeball3-ps: ported inline, no item of its own to carry the alias: `ftLib_GetCameraSubjectBonePos` |
| `melee/ft/ft_0877` | 1 | 32 | nothing known reaches it: `ft_80087838` |
| `melee/ft/ftmaterial` | 3 | 1,648 | reached by items-pokeball1-fd, sel1p-target: ported inline, no item of its own to carry the alias: `ftMaterial_800BF260`; reached by items-pokeball1-fd, sel1p-target; presentation (`melee-render/src/hsd/material.rs`), which a frames check with a mutation checks, not a session: `ftMaterial_800BF534`, `ftMaterial_800BF6BC` |
| `melee/ft/ftcolanim` | 4 | 128 | reached by items-pokeball2-bf, items-pokeball3-ps: ported inline, no item of its own to carry the alias: `ftCo_800BFE74`, `ftCo_800BFEB4`; reached by items-pokeball5-bf, items-shoot-fd: ported inline, no item of its own to carry the alias: `ftCo_800BFE94`; reached by rec-2026-10-06_12-04-48: ported inline, no item of its own to carry the alias: `ftCo_800BFEF4` |
| `melee/ft/ftdevice` | 1 | 32 | reached by rec-allstar-clear: ported inline, no item of its own to carry the alias: `ftCo_800C0674` |
| `melee/ft/ftCo_800C7CA0` | 1 | 4 | reached by tou-bracket64, rec-allstar-clear: ported inline, no item of its own to carry the alias: `ftCo_DownReflect_IASA` |

### Fighters: the common states (`ftCo_*`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/ft/kinds/ftCommon/ftCo_AttackDash` | 2 | 176 | reached by items-pokeball1-fd, rec-classic-clear (`ft/co/attack.rs`): ported inline, no item of its own to carry the alias: `decideFighter`; reached by items-pokeball1-fd, rec-classic-clear (`ft/seak/special_n.rs`): ported inline, no item of its own to carry the alias: `doEnter` |
| `melee/ft/kinds/ftCommon/ftCo_AttackHi3` | 1 | 96 | reached by items-pokeball2-bf, items-pokeball3-ps (`ft/seak/special_n.rs`): ported inline, no item of its own to carry the alias: `doEnter` |
| `melee/ft/kinds/ftCommon/ftCo_AttackLw3` | 2 | 200 | reached by items-pokeball1-fd, rec-classic-clear (`ft/co/attack.rs`): ported inline, no item of its own to carry the alias: `decideFighter`; reached by items-pokeball1-fd, rec-classic-clear (`ft/seak/special_n.rs`): ported inline, no item of its own to carry the alias: `doEnter` |
| `melee/ft/kinds/ftCommon/ftCo_AttackS4` | 2 | 472 | reached by items-pokeball1-fd, rec-allstar-clear (`ft/co/attack.rs`): ported inline, no item of its own to carry the alias: `decideFighter`; reached by items-pokeball1-fd, rec-allstar-clear (`ft/seak/special_n.rs`): ported inline, no item of its own to carry the alias: `doEnter` |
| `melee/ft/kinds/ftCommon/ftCo_AttackHi4` | 1 | 96 | reached by items-pokeball2-bf, items-pokeball3-ps (`ft/seak/special_n.rs`): ported inline, no item of its own to carry the alias: `doEnter` |
| `melee/ft/kinds/ftCommon/ftCo_AttackLw4` | 1 | 96 | reached by items-pokeball2-bf, items-pokeball3-ps (`ft/seak/special_n.rs`): ported inline, no item of its own to carry the alias: `doEnter` |
| `melee/ft/kinds/ftCommon/ftCo_AttackAir` | 1 | 92 | reached by items-pokeball1-fd, rec-classic-clear (`ft/co/attack.rs`): ported inline, no item of its own to carry the alias: `decideFighter` |
| `melee/ft/kinds/ftCommon/ftCo_Damage` | 3 | 400 | named in `ft/donkey/heavy.rs` (perhaps ported inline); no check reaches it yet: `ftCo_ScaleBy154`, `ftCo_8008D8E8`; named in `ft/resolve.rs` (perhaps ported inline); no check reaches it yet: `ftCo_8008EB58` |
| `melee/ft/kinds/ftCommon/ftCo_SpecialS` | 1 | 128 | reached by items-pokeball1-fd, rec-classic-clear (`ft/seak/special_n.rs`): ported inline, no item of its own to carry the alias: `doEnter` |
| `melee/ft/kinds/ftCommon/ftCo_ThrownKirby` | 1 | 36 | nothing known reaches it: `ftCo_800BDA50` |
| `melee/ft/kinds/ftCommon/ftCo_PassiveCeil` | 1 | 4 | nothing known reaches it: `ftCo_PassiveCeil_IASA` |
| `melee/ft/kinds/ftCommon/ftCo_DamageSong` | 3 | 12 | reached by items-pokeball1-fd, rec-allstar-clear: ported inline, no item of its own to carry the alias: `ftCo_DamageSong_IASA`, `ftCo_DamageSongWait_IASA`, `ftCo_DamageSongRv_IASA` |
| `melee/ft/kinds/ftCommon/ftCo_AirCatch` | 4 | 248 | ported in `ft/samus/tether.rs` (held by a `// Ports` comment); no check reaches it yet: `ftCo_800C3CC0`, `ftCo_AirCatchHit_Phys`; ported in `ft/samus/tether.rs` (dispatched by address); no check reaches it yet: `ftCo_AirCatchHit_Anim`, `ftCo_AirCatchHit_IASA` |
| `melee/ft/kinds/ftCommon/ftCo_Barrel` | 9 | 752 | ported in `ft/co/barrel.rs` (held by a `// Ports` comment); no check reaches it yet: `ftCo_Barrel_Death2_Cb`, `ftCo_Barrel_Take_Dmg_Cb`, `ftCo_Barrel_Anim`, `ftCo_Barrel_IASA`, `ftCo_Barrel_Phys`, `ftCo_Barrel_Coll`, `ftCo_Barrel_Accessory1_Cb`, `fn_800C9290`, `ftCo_800C92E4` |
| `melee/ft/kinds/ftCommon/ftCo_RunDirect` | 1 | 280 | nothing known reaches it: `ftCo_RunDirect_IASA` |
| `melee/ft/kinds/ftCommon/ftCo_CatchPull` | 1 | 80 | ported in `ft/co/grab.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_800DA004` |
| `melee/ft/kinds/ftCommon/ftCo_CatchWait` | 1 | 80 | ported in `ft/co/grab.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_800DA440` |
| `melee/ft/kinds/ftCommon/ftCo_CatchAttack` | 1 | 80 | ported in `ft/co/grab.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_800DA618` |
| `melee/ft/kinds/ftCommon/ftCo_0DC2` | 3 | 432 | ported in `ft/co/grab.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_800DC384`, `fn_800DC404`, `fn_800DC624` |
| `melee/ft/kinds/ftCommon/ftCo_Throw` | 2 | 248 | ported in `ft/co/throw.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_800DD5EC`, `fn_800DD684` |
| `melee/ft/kinds/ftCommon/ftCo_Thrown` | 1 | 84 | ported in `ft/co/throw.rs` (held by a `// Ports` comment); no check reaches it yet: `ftCo_800DE920` |

### Fighters: the characters' own states

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/ft/kinds/ftMario/ftmario` | 1 | 72 | ported in `ft/mario/init.rs` (dispatched by address); no check reaches it yet: `ftMr_Init_OnItemVisible` |
| `melee/ft/kinds/ftCaptain/ftcaptain` | 2 | 144 | ported in `ft/captain/init.rs` (dispatched by address); no check reaches it yet: `ftCa_Init_OnItemInvisible`, `ftCa_Init_OnItemVisible` |
| `melee/ft/kinds/ftKirby/ftkirby` | 2 | 144 | ported in `ft/kirby/init.rs` (held by a `// Ports` comment); no check reaches it yet: `ftKb_Init_OnItemInvisible`, `ftKb_Init_OnItemVisible` |
| `melee/ft/kinds/ftKirby/ftkirbyspecialn` | 1 | 32 | ported in `ft/kirby/special_n.rs` (its doc names it); no check reaches it yet: `ftKb_EatWalk_Anim` |
| `melee/ft/kinds/ftKirby/ftkirbyspecialyoshi` | 2 | 328 | ported in `ft/kirby/special_yoshi.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_801095DC`, `fn_80109714` |
| `melee/ft/kinds/ftDonkey/ftdonkey` | 2 | 144 | ported in `ft/donkey/init.rs` (held by a `// Ports` comment); no check reaches it yet: `ftDk_Init_OnItemInvisible`, `ftDk_Init_OnItemVisible` |
| `melee/ft/kinds/ftDonkey/ftdonkeyspecials` | 1 | 136 | ported in `ft/donkey/special_s.rs` (held by a `// Ports` comment); no check reaches it yet: `doGroundTransition` |
| `melee/ft/kinds/ftNess/ftness` | 1 | 72 | ported in `ft/ness/init.rs` (held by a `// Ports` comment); no check reaches it yet: `ftNs_Init_OnItemVisible` |
| `melee/ft/kinds/ftNess/ftnessattackhi4` | 7 | 660 | named in `ft/ness/attack_hi4.rs` (perhaps ported inline); no check reaches it yet: `ftNs_AttackHi4_YoyoApplyDamage`; ported in `ft/ness/attack_hi4.rs` (dispatched by address); no check reaches it yet: `ftNs_AttackHi4Charge_IASA`, `ftNs_AttackHi4Charge_Phys`, `ftNs_AttackHi4Charge_Coll`, `ftNs_AttackHi4Release_Anim`, `ftNs_AttackHi4Release_IASA`, `ftNs_AttackHi4Release_Coll` |
| `melee/ft/kinds/ftNess/ftnessattacklw4` | 6 | 380 | ported in `ft/ness/attack_lw4.rs` (dispatched by address); no check reaches it yet: `ftNs_AttackLw4Charge_IASA`, `ftNs_AttackLw4Charge_Phys`, `ftNs_AttackLw4Charge_Coll`, `ftNs_AttackLw4Release_Anim`, `ftNs_AttackLw4Release_IASA`, `ftNs_AttackLw4Release_Coll` |
| `melee/ft/kinds/ftNess/ftnessspecialn` | 3 | 208 | ported in `ft/ness/special_n.rs` (dispatched by address); no check reaches it yet: `ftNs_SpecialAirNEnd_IASA`, `ftNs_SpecialAirNEnd_Phys`, `ftNs_SpecialAirNEnd_Coll` |
| `melee/ft/kinds/ftNess/ftnessspecialhi` | 8 | 32 | reached by oracle-cpu-ness-fd, oracle-cpu-yoshi-bf: ported inline, no item of its own to carry the alias: `ftNs_SpecialHiHold_IASA`, `ftNs_SpecialHiEnd_IASA`; nothing known reaches it: `ftNs_SpecialHi_IASA`; reached by items-shoot-fd, oracle-cpu-ness-fd: ported inline, no item of its own to carry the alias: `ftNs_SpecialAirHiStart_IASA`, `ftNs_SpecialAirHiHold_IASA`, `ftNs_SpecialAirHiEnd_IASA`; reached by items-shoot-fd, oracle-cpu-yoshi-bf: ported inline, no item of its own to carry the alias: `ftNs_SpecialAirHi_IASA`; reached by oracle-cpu-yoshi-bf: ported inline, no item of its own to carry the alias: `ftNs_SpecialAirHiRebound_IASA` |
| `melee/ft/kinds/ftNess/ftnessspeciallw` | 23 | 2,012 | ported in `ft/ness/special_lw.rs` (dispatched by address); no check reaches it yet: `ftNs_SpecialAirLwStart_Anim`, `ftNs_SpecialAirLwStart_IASA`, `ftNs_SpecialAirLwStart_Phys`, `ftNs_SpecialAirLwStart_Coll`, `ftNs_SpecialAirLwHold_Anim`, `ftNs_SpecialAirLwHold_IASA`, `ftNs_SpecialAirLwHold_Phys`, `ftNs_SpecialAirLwHold_Coll`, `ftNs_SpecialLwTurn_Anim`, `ftNs_SpecialAirLwTurn_Anim`, `ftNs_SpecialLwTurn_IASA`, `ftNs_SpecialAirLwTurn_IASA`, `ftNs_SpecialLwTurn_Phys`, `ftNs_SpecialAirLwTurn_Phys`, `ftNs_SpecialLwTurn_Coll`, `ftNs_SpecialAirLwTurn_Coll`, `ftNs_SpecialAirLwHit_IASA`, `ftNs_SpecialAirLwHit_Phys`, `ftNs_SpecialAirLwHit_Coll`, `ftNs_SpecialAirLwEnd_Anim`, `ftNs_SpecialAirLwEnd_IASA`, `ftNs_SpecialAirLwEnd_Phys`, `ftNs_SpecialAirLwEnd_Coll` |
| `melee/ft/kinds/ftPopo/ftpopo` | 1 | 72 | ported in `ft/popo/init.rs` (held by a `// Ports` comment); no check reaches it yet: `ftPp_Init_OnItemVisible` |
| `melee/ft/kinds/ftPopo/ftpopospecialhi` | 7 | 672 | ported in `ft/popo/special_hi.rs` (held by a `// Ports` comment); no check reaches it yet: `ftPp_SpecialHi_801217EC`, `ftPp_SpecialHi_8012184C`, `ftPp_SpecialHi_80121CE0`, `ftPp_SpecialHi_80121D40`, `ftPp_SpecialHi_80121FD8`, `ftPp_SpecialHi_80122038`, `ftPp_SpecialHi_801222E8` |
| `melee/ft/kinds/ftPopo/ftpopospeciallw` | 6 | 532 | ported in `ft/popo/special_lw.rs` (held by a `// Ports` comment); no check reaches it yet: `ftPp_SpecialAirLw_Enter`, `ftPp_SpecialAirLw_Anim`, `fn_80122B54`; ported in `ft/popo/special_lw.rs` (dispatched by address); no check reaches it yet: `ftPp_SpecialAirLw_IASA`, `ftPp_SpecialAirLw_Phys`, `ftPp_SpecialAirLw_Coll` |
| `melee/ft/kinds/ftNana/ftnanaspecialhi` | 1 | 108 | ported in `ft/popo/nana_special_hi.rs` (held by a `// Ports` comment); no check reaches it yet: `ftNn_Init_80123720` |
| `melee/ft/kinds/ftPikachu/ftpikachu` | 4 | 280 | ported in `ft/pikachu/init.rs` (held by a `// Ports` comment); no check reaches it yet: `ftPk_Init_OnItemInvisible`, `ftPk_Init_OnItemVisible`; named in `ft/powerup.rs` (perhaps ported inline); no check reaches it yet: `ftPk_Init_UnkMotionStates1`, `ftPk_Init_UnkMotionStates2` |
| `melee/ft/kinds/ftSamus/ftsamus` | 2 | 144 | ported in `ft/samus/init.rs` (held by a `// Ports` comment); no check reaches it yet: `ftSs_Init_OnItemInvisible`, `ftSs_Init_OnItemVisible` |
| `melee/ft/kinds/ftSamus/ftsamusspecialn` | 4 | 72 | ported in `ft/samus/special_n.rs` (dispatched by address); no check reaches it yet: `ftSs_SpecialAirNStart_IASA`, `ftSs_SpecialAirN_IASA`, `ftSs_SpecialAirNStart_Phys`, `ftSs_SpecialAirN_Phys` |
| `melee/ft/kinds/ftYoshi/ftyoshi` | 2 | 144 | ported in `ft/yoshi/init.rs` (held by a `// Ports` comment); no check reaches it yet: `ftYs_Init_OnItemInvisible`, `ftYs_Init_OnItemVisible` |
| `melee/ft/kinds/ftYoshi/ftyoshispecialn` | 14 | 504 | ported in `ft/yoshi/special_n.rs` (dispatched by address); no check reaches it yet: `ftYs_SpecialAirN1_0_Phys`, `ftYs_SpecialN1_1_Phys`, `ftYs_SpecialAirN1_2_Phys`, `ftYs_SpecialAirN1_1_Phys`, `ftYs_SpecialN2_1_Phys`, `ftYs_SpecialAirN2_1_Phys`, `ftYs_SpecialAirN2_0_Phys`, `ftYs_SpecialAirN1_0_Coll`, `ftYs_SpecialN1_1_Coll`, `ftYs_SpecialAirN1_2_Coll`, `ftYs_SpecialAirN1_1_Coll`, `ftYs_SpecialN2_1_Coll`, `ftYs_SpecialAirN2_1_Coll`, `ftYs_SpecialAirN2_0_Coll` |
| `melee/ft/kinds/ftYoshi/ftyoshispecialhi` | 1 | 32 | ported in `ft/yoshi/special_hi.rs` (dispatched by address); no check reaches it yet: `ftYs_SpecialAirHi_Phys` |
| `melee/ft/kinds/ftKoopa/ftkoopa` | 2 | 144 | ported in `ft/koopa/init.rs` (held by a `// Ports` comment); no check reaches it yet: `ftKp_Init_OnItemInvisible`, `ftKp_Init_OnItemVisible` |
| `melee/ft/kinds/ftZelda/ftzeldaspecialhi` | 2 | 64 | ported in `ft/seak/special_n.rs` (dispatched by address); no check reaches it yet: `ftZd_SpecialHiStart_0_Phys`, `ftZd_SpecialHi_Phys` |
| `melee/ft/kinds/ftPurin/ftpurin` | 3 | 148 | nothing known reaches it: `ftPr_Init_OnUserDataRemove`, `ftPr_Init_8013C494`, `ftPr_Init_UnkMotionStates6` |
| `melee/ft/kinds/ftPurin/ftpurinspecialhi` | 4 | 72 | ported in `ft/purin/special_hi.rs` (dispatched by address); no check reaches it yet: `ftPr_SpecialHi_IASA`, `ftPr_SpecialAirHi_IASA`, `ftPr_SpecialHi_Phys`, `ftPr_SpecialAirHi_Phys` |
| `melee/ft/kinds/ftPurin/ftpurinspeciallw` | 4 | 72 | ported in `ft/purin/special_lw.rs` (dispatched by address); no check reaches it yet: `ftPr_SpecialLw_IASA`, `ftPr_SpecialAirLw_IASA`, `ftPr_SpecialLw_Phys`, `ftPr_SpecialAirLw_Phys` |
| `melee/ft/kinds/ftPurin/ftpurinspecials` | 1 | 4 | ported in `ft/purin/special_s.rs` (dispatched by address); no check reaches it yet: `ftPr_SpecialAirS_IASA` |
| `melee/ft/kinds/ftPurin/ftpurinspecialn` | 30 | 8,520 | ported in `ft/purin/special_n.rs` (dispatched by address); no check reaches it yet: `ftPr_SpecialNStart_Anim`, `ftPr_SpecialNLoop_Anim`, `ftPr_SpecialNFull_Anim`, `ftPr_SpecialNRelease_Anim`, `ftPr_SpecialNEnd_Anim`, `ftPr_SpecialAirNStart_Anim`, `ftPr_SpecialAirNChargeLoop_Anim`, `ftPr_SpecialAirNChargeFull_Anim`, `ftPr_SpecialAirNChargeRelease_Anim`, `ftPr_SpecialAirNEnd_Anim`, `ftPr_SpecialNStart_IASA`, `ftPr_SpecialNLoop_IASA`, `ftPr_SpecialNFull_IASA`, `ftPr_SpecialNTurn_IASA`, `ftPr_SpecialNEnd_IASA`, `ftPr_SpecialAirNStart_IASA`, `ftPr_SpecialAirNChargeLoop_IASA`, `ftPr_SpecialAirNChargeFull_IASA`, `ftPr_SpecialAirNChargeRelease_IASA`, `ftPr_SpecialAirNStartTurn_IASA`, `ftPr_SpecialAirNEnd_IASA`, `ftPr_SpecialNHit_IASA`, `ftPr_SpecialNLoop_Phys`, `ftPr_SpecialAirNStart_Phys`, `ftPr_SpecialAirNChargeLoop_Phys`, `ftPr_SpecialAirNEnd_Phys`, `ftPr_SpecialNLoop_Coll`, `ftPr_SpecialNFull_Coll`, `ftPr_SpecialAirNChargeLoop_Coll`, `ftPr_SpecialAirNChargeFull_Coll` |
| `melee/ft/kinds/ftLuigi/ftluigi` | 1 | 72 | ported in `ft/luigi/init.rs` (dispatched by address); no check reaches it yet: `ftLg_Init_OnItemInvisible` |
| `melee/ft/kinds/ftMewtwo/ftmewtwo` | 2 | 8 | ported in `ft/mewtwo/init.rs` (dispatched by address); no check reaches it yet: `ftMt_Init_OnItemInvisible`, `ftMt_Init_OnItemVisible` |
| `melee/ft/kinds/ftDrMario/ftdrmario` | 1 | 72 | ported in `ft/drmario/mod.rs` (dispatched by address); no check reaches it yet: `ftDr_Init_OnItemInvisible` |
| `melee/ft/kinds/ftPichu/ftpichu` | 4 | 452 | ported in `ft/pichu/mod.rs` (dispatched by address); no check reaches it yet: `ftPc_Init_OnItemPickup`, `ftPc_Init_OnItemInvisible`, `ftPc_Init_OnItemVisible`, `ftPc_Init_OnItemDrop` |
| `melee/ft/kinds/ftGanon/ftganon` | 1 | 72 | ported in `ft/captain/init.rs` (dispatched by address); no check reaches it yet: `ftGn_Init_OnItemInvisible` |

### The scene machine, VS mode and the menus (`gm`, `mn`, `ty`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/gm/gmmain_lib` | 6 | 144 | reached by rec-2026-10-06_16-58-24: ported inline, no item of its own to carry the alias: `gmMainLib_8015CEB4`; a getter or setter of the game's data, which its callers' ports read or write inline (`menus.txt`): `gmMainLib_8015CF84`, `gmMainLib_8015DB00`, `gmMainLib_8015DB0C`, `gmMainLib_8015DB18`; reached by items-pokeball1-fd, sel1p-target: ported inline, no item of its own to carry the alias: `gmMainLib_GetUnlockedCharactersBitmaskPtr` |
| `melee/gm/gmvs` | 1 | 192 | a getter or setter of the game's data, which its callers' ports read or write inline (`menus.txt`): `gm_AnyControllerPressedZ` |
| `melee/gm/gmresult` | 1 | 36 | a getter or setter of the game's data, which its callers' ports read or write inline (`menus.txt`): `fn_8017435C` |
| `melee/gm/gmtou_1` | 3 | 1,828 | ported in `gm/tournament/bracket.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_801967E0`, `fn_80197FD8`; ported in `gm/tournament/setup.rs` (held by a `// Ports` comment); no check reaches it yet: `gm_8019A828` |
| `melee/gm/gmtou_2` | 1 | 328 | ported in `gm/tournament/alt.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_8019D074` |
| `melee/gm/gmvsmelee` | 1 | 4 | ported in the scene machine's dispatch (game/gm/machine.rs, title.rs, howto.rs, camera_mode.rs), held unaliased until a session reaches it (`menus.txt`): `gm_Mode_Vs_OnUnload` |
| `melee/gm/gmhowto` | 2 | 500 | ported in `gm/howto.rs` (its doc names it); no check reaches it yet: `gm_Scene_HowTo_OnEnter`, `gm_Scene_HowTo_OnFrame` |
| `melee/gm/gmomake15` | 2 | 500 | ported in `gm/howto.rs` (its doc names it); no check reaches it yet: `gm_Scene_Omake15_OnEnter`, `gm_Scene_Omake15_OnFrame` |
| `melee/gm/gmtitlemode` | 1 | 256 | reached by rec-2026-10-06_16-58-24, res-stock (`gm/menu.rs`): ported inline, no item of its own to carry the alias: `onExit` |
| `melee/gm/gmdebugmode` | 1 | 36 | ported in `gm/challenger.rs` (another agent's file); no check reaches it yet: `onExitPrize` |
| `melee/gm/gmtoumode` | 2 | 196 | ported in `gm/tournament/mod.rs` (held by a `// Ports` comment); no check reaches it yet: `gm_801B174C`, `gm_801B1788` |
| `melee/gm/gmopeningmode` | 3 | 264 | ported in `gm/title.rs` (its doc names it); no check reaches it yet: `onExitTitle`; ported in the scene machine's dispatch (game/gm/machine.rs, title.rs, howto.rs, camera_mode.rs), held unaliased until a session reaches it (`menus.txt`): `onExitHowto`, `onExitOmake15` |
| `melee/gm/gmboot` | 1 | 48 | ported in `gm/boot_mode.rs` (held by a `// Ports` comment); no check reaches it yet: `memcardOnLoad` |
| `melee/mn/mnstagesel` | 1 | 24 | a getter or setter of the game's data, which its callers' ports read or write inline (`menus.txt`): `mnStageSel_8025BC08` |
| `melee/mn/mncharsel` | 1 | 108 | ported in `mn/charsel/procs.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_8025FAC0` |

### Stages: the shared stage code and VS stages' leftovers

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/gr/ground` | 1 | 4 | nothing known reaches it: `Ground_801BFFAC` |
| `melee/gr/granime` | 1 | 284 | reached by items-pokeball1-fd, sel1p-target: ported inline, no item of its own to carry the alias: `grAnime_801C6F50` |
| `melee/gr/grmaterial` | 4 | 288 | reached by items-pokeball1-fd, rec-mh-mash: ported inline, no item of its own to carry the alias: `grMaterial_801C8E74`; nothing known reaches it: `grMaterial_801C9470`, `fn_801C9664`; named in `gr/material.rs` (perhaps ported inline); no check reaches it yet: `grMaterial_801C9490` |

### Items: the item engine (`it_26B1`, `it_2725`, `itanimlist`, `item`, `itcoll`, `itmaterial`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/it/it_26B1` | 2 | 64 | ported, held for a check (`items_audit.rs`'s UNCHECKED): `itGetTeamId`; ported in `it/lib26b1.rs` (held by a `// Ports` comment); no check reaches it yet: `it_8026C334` |
| `melee/it/it_2725` | 1 | 76 | ported in `it/release.rs` (held by a `// Ports` comment); no check reaches it yet: `it_80274250` |
| `melee/it/itmaterial` | 3 | 1,640 | reached by items-pokeball1-fd, sel1p-target: ported inline, no item of its own to carry the alias: `it_80277D08`; reached by items-pokeball1-fd, sel1p-target; presentation (`melee-render/src/hsd/material.rs`), which a frames check with a mutation checks, not a session: `it_80277F90`, `it_80278108` |

### Items: the kinds' callbacks

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/it/kinds/ittomato` | 1 | 216 | a spawn by kind of a box, Flipper or food (it_8026BE84): the All-Star rest area's (Big Blue's flying platforms make only the five kinds ported, the… (`items_audit.rs`): `it_802841B4` |
| `melee/it/kinds/ittarucann` | 2 | 472 | ported in `it/kinds/tarucann.rs` (held by a `// Ports` comment); no check reaches it yet: `it_80295F38`, `it_3F14_Logic5_Dropped` |
| `melee/it/kinds/itfoxblaster` | 3 | 340 | ported in `it/kinds/foxblaster.rs` (dispatched by address); no check reaches it yet: `itFoxblaster_UnkMotion9_Anim`, `itFoxblaster_UnkMotion9_Phys`, `itFoxblaster_UnkMotion9_Coll` |
| `melee/it/kinds/itpikachuthunder` | 1 | 56 | ported in `it/evt_unk.rs` (held by a `// Ports` comment); no check reaches it yet: `itPikachuThunder_Logic39_EvtUnk` |
| `melee/it/kinds/itsamusbomb` | 1 | 52 | ported in `it/evt_unk.rs` (held by a `// Ports` comment); no check reaches it yet: `itSamusBomb_Logic50_EvtUnk` |
| `melee/it/kinds/itsamusmissile` | 1 | 76 | ported in `it/evt_unk.rs` (held by a `// Ports` comment); no check reaches it yet: `it_2725_Logic52_EvtUnk` |
| `melee/it/kinds/itsamusgrapple` | 1 | 76 | ported in `it/evt_unk.rs` (held by a `// Ports` comment); no check reaches it yet: `itSamusGrapple_Logic53_EvtUnk` |
| `melee/it/kinds/itnessyoyo` | 1 | 64 | ported in `it/kinds/nessyoyo.rs` (held by a `// Ports` comment); no check reaches it yet: `it_2725_Logic59_EvtUnk` |
| `melee/it/kinds/itclimbersice` | 1 | 56 | ported in `it/evt_unk.rs` (held by a `// Ports` comment); no check reaches it yet: `itClimbersIce_Logic90_EvtUnk` |
| `melee/it/kinds/itclimbersstring` | 1 | 76 | nothing known reaches it: `it_2725_Logic70_EvtUnk` |
| `melee/it/kinds/itlucky` | 18 | 908 | ported in `it/kinds/lucky.rs` (held by a `// Ports` comment); no check reaches it yet: `it_802D5710`, `itLucky_Logic44_Spawned`, `it_802D582C`, `it_802D5884`, `it_802D58BC`, `it_802D58C0`, `it_802D58EC`, `itLucky_UnkMotion3_Anim`, `itLucky_UnkMotion3_Phys`, `itLucky_UnkMotion3_Coll`, `itLucky_Logic44_PickedUp`, `itLucky_UnkMotion2_Anim`, `itLucky_UnkMotion2_Phys`, `itLucky_Logic44_Dropped`, `itLucky_Logic44_EnteredAir`, `it_802D5A2C`, `it_802D5A64`, `it_802D5A68` |
| `melee/it/kinds/itkirbyyoshispecialn` | 5 | 412 | ported, held for a check (`items_audit.rs`'s UNCHECKED): `it_802F2D98`; ported in `it/kinds/yoshitongue.rs` (dispatched by address); no check reaches it yet: `itKirbyyoshispecialn_UnkMotion0_Anim`, `itKirbyyoshispecialn_UnkMotion0_Phys`, `itKirbyyoshispecialn_UnkMotion0_Coll`; ported in `it/kinds/yoshitongue.rs` (held by a `// Ports` comment); no check reaches it yet: `it_802F2E7C` |

### The HUD (`if*`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/if/if_2FF2` | 1 | 4 | nothing known reaches it: `fn_802FF360` |

## Libraries

Classified by reach in the second round: the `CALLS` sweep took in the SDK's and HSD's rows once the first run had found the ones called millions of times a frame (`get_shape_normal_xyz`, `WriteMTXPS4x3`, `HSD_Index2PosNrmMtx`) and dropped them. What melee-sim ports went to aliases; what's left reached is melee-render's (frames checks with a mutation each) or inline. The console's reset (`OSResetSystem` and the reboot under it, `Reset`, `__OSDoHotReset`) is reached only by the reset button or the disc error screen, which no session input raises.

### The SDK and runtime (`dolphin`, MSL)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `MSL/printf` | 1 | 736 | named in `melee-sim/src/msl.rs` (perhaps ported inline); no check reaches it yet: `longlong2str` |
| `dolphin/thp/THPDec` | 1 | 216 | nothing known reaches it: `THPDec_803302EC` |
| `dolphin/gx/GXInit` | 2 | 160 | reached by sel1p-target; presentation (`melee-render/src/gx/init.rs`), which a frames check with a mutation checks, not a session: `__GXDefaultTexRegionCallback`, `__GXDefaultTlutRegionCallback` |
| `dolphin/gx/GXTexture` | 4 | 240 | reached by sel1p-target; presentation (`melee-render/src/gx/texture.rs`), which a frames check with a mutation checks, not a session: `__GetImageTileCount`, `GXGetTexObjWidth`, `GXGetTexObjHeight`, `GXGetTexObjFmt` |
| `dolphin/gx/GXTev` | 1 | 116 | reached by sel1p-target; presentation (`melee-render/src/gx/tev.rs`), which a frames check with a mutation checks, not a session: `GXSetTevColorS10` |
| `dolphin/gx/GXPixel` | 1 | 456 | ported in `melee-render/src/gx/pixel.rs` (its doc names it); no check reaches it yet: `GXInitFogAdjTable` |
| `dolphin/gx/GXStubs` | 1 | 4 | ported in `melee-render/src/gx/transform.rs` (its doc names it); no check reaches it yet: `__GXSetRange` |
| `dolphin/gx/GXTransform` | 4 | 216 | reached by sel1p-target; presentation (`melee-render/src/gx/transform.rs`), which a frames check with a mutation checks, not a session: `WriteMTXPS4x3`, `WriteMTXPS3x3from3x4`, `WriteMTXPS4x2`; ported in `melee-render/src/gx/transform.rs` (its doc names it); no check reaches it yet: `GXGetViewportv` |
| `dolphin/os/OSAlloc` | 1 | 16 | reached by sel1p-target: ported inline, no item of its own to carry the alias: `OSSetCurrentHeap` |
| `dolphin/os/OSReboot` | 3 | 536 | named in `session.rs` (perhaps ported inline); no check reaches it yet: `Run`; nothing known reaches it: `Callback`, `__OSReboot` |
| `dolphin/os/OSReset` | 3 | 800 | named in `melee-sim/src/lb/command.rs` (perhaps ported inline); no check reaches it yet: `Reset`; nothing known reaches it: `__OSDoHotReset`, `OSResetSystem` |
| `dolphin/ax/AXVPB` | 1 | 12 | reached by sel1p-target: ported inline, no item of its own to carry the alias: `__AXGetPBs` |

### HSD's library (`sysdolphin`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `sysdolphin/baselib/state` | 7 | 512 | reached by rec-classic-clear; presentation (`melee-render/src/hsd/quad.rs`), which a frames check with a mutation checks, not a session: `HSD_SetupRenderMode`; reached by sel1p-target; presentation (`melee-render/src/hsd/state.rs`), which a frames check with a mutation checks, not a session: `HSD_SetMaterialColor`, `HSD_SetMaterialShininess`, `HSD_StateSetCullMode`, `_HSD_StateInvalidateVtxAttr`; reached by rec-classic-clear; presentation (`melee-render/src/hsd/state.rs`), which a frames check with a mutation checks, not a session: `HSD_StateSetLineWidth`, `HSD_StateSetPointSize` |
| `sysdolphin/baselib/aobj` | 2 | 612 | reached by sel1p-target: ported inline, no item of its own to carry the alias: `callbackForeachFunc`, `DObjForeachAnim` |
| `sysdolphin/baselib/lobj` | 2 | 728 | reached by sel1p-target; presentation (`melee-render/src/hsd/lobj.rs`), which a frames check with a mutation checks, not a session: `setup_point_lightobj`; reached by rec-classic-clear; presentation (`melee-render/src/hsd/lobj.rs`), which a frames check with a mutation checks, not a session: `setup_spot_lightobj` |
| `sysdolphin/baselib/cobj` | 4 | 2,296 | reached by sel1p-target; presentation (`melee-render/src/hsd/camera.rs`), which a frames check with a mutation checks, not a session: `HSD_CObjEraseScreen`; reached by sel1p-target: ported inline, no item of its own to carry the alias: `setupNormalCamera`; nothing known reaches it: `setupTopHalfCamera`, `setupBottomHalfCamera` |
| `sysdolphin/baselib/pobj` | 7 | 3,284 | reached by sel1p-target; presentation (`melee-render/src/hsd/draw.rs`), which a frames check with a mutation checks, not a session: `HSD_ClearVtxDesc`; named in `melee-sim/src/hsd/pobj.rs` (perhaps ported inline); no check reaches it yet: `get_shape_nbt_xyz`; reached by sel1p-target: ported inline, no item of its own to carry the alias: `HSD_PObjClearMtxMark`, `PObjSetupMtx`; reached by rec-allstar-clear; presentation (`melee-render/src/hsd/draw.rs`), which a frames check with a mutation checks, not a session: `HSD_PObjSetMtxMark`; reached by rec-allstar-clear: ported inline, no item of its own to carry the alias: `HSD_PObjGetMtxMark`; ported in `melee-render/src/hsd/draw.rs` (its doc names it); no check reaches it yet: `SetupSharedVtxModelMtx` |
| `sysdolphin/baselib/jobj` | 1 | 68 | named in `melee-sim/src/hsd/jobj.rs` (perhaps ported inline); no check reaches it yet: `HSD_JObjAddDObj` |
| `sysdolphin/baselib/displayfunc` | 9 | 4,264 | reached by rec-classic-clear: ported inline, no item of its own to carry the alias: `mkVBillBoardMtx`, `mkHBillBoardMtx`; reached by sel1p-target: ported inline, no item of its own to carry the alias: `mkBillBoardMtx`, `_HSD_ZListDisp`, `_HSD_ZListClear`; reached by sel1p-target; presentation (`melee-render/src/hsd/draw.rs`), which a frames check with a mutation checks, not a session: `HSD_JObjMakePositionMtx`, `_HSD_mkEnvelopeModelNodeMtx`; nothing known reaches it: `zlist_sort`; reached by sel1p-target; presentation (`melee-render/src/hsd/camera.rs`), which a frames check with a mutation checks, not a session: `HSD_EraseRect` |
| `sysdolphin/baselib/initialize` | 1 | 12 | reached by sel1p-target: ported inline, no item of its own to carry the alias: `HSD_Init_803755A8` |
| `sysdolphin/baselib/util` | 3 | 460 | reached by sel1p-target; presentation (`melee-render/src/hsd/state.rs`), which a frames check with a mutation checks, not a session: `HSD_MulColor`; reached by rec-allstar-clear: ported inline, no item of its own to carry the alias: `HSD_GetNbBits`; reached by sel1p-target: ported inline, no item of its own to carry the alias: `HSD_Index2PosNrmMtx` |
| `sysdolphin/baselib/robj` | 3 | 312 | named in `melee-sim/src/hsd/robj.rs` (perhaps ported inline); no check reaches it yet: `RObjUpdateFunc`, `dummy_func`; ported in `melee-sim/src/hsd/robj.rs` (its doc names it); no check reaches it yet: `expLoadDesc` |
| `sysdolphin/baselib/shadow` | 2 | 436 | reached by sel1p-target; presentation (`melee-render/src/hsd/camera.rs`), which a frames check with a mutation checks, not a session: `HSD_ShadowInit`, `drawBackgroundRect` |
| `sysdolphin/baselib/texp` | 1 | 352 | ported in `melee-sim/src/hsd/texp/mod.rs` (its doc names it); no check reaches it yet: `AssignColorReg` |
| `sysdolphin/baselib/devcom` | 1 | 24 | nothing known reaches it: `HSD_DevComIsBusy` |
| `sysdolphin/baselib/psdisp` | 1 | 252 | named in `melee-render/src/hsd/psdisp.rs` (perhaps ported inline); no check reaches it yet: `getColorMatAmb` |
| `sysdolphin/baselib/psdisptev` | 1 | 12 | reached by sel1p-target: ported inline, no item of its own to carry the alias: `psSetupTevInvalidState` |

## Classic

### Classic's mode and intro (`gmclassic`, `gm_1832`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/gm/gm_1832` | 1 | 36 | the single-player modes', which only the 1P list's gated entries lead to (`menus.txt`): `fn_80184A94` |

## 1P shared

### The single-player matches' setups, records and clear screens (`gm_16A2`, `gm_1601`, `gm_17AD`, `gm_17C0`, `gm_17BA`, `gm_18A1`, `gm_16F1`, `gm_1A3F`, `gmregclear`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/gm/gm_1601` | 2 | 160 | ported in `gm/tournament/bracket.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_801642A0`; a getter or setter of the game's data, which its callers' ports read or write inline (`menus.txt`): `fn_80166CBC` |
| `melee/gm/gm_1A3F` | 1 | 8 | reached by rec-2026-10-06_16-58-24, res-stock: ported inline, no item of its own to carry the alias: `gm_GetGameModeStateExitData` |

## 1P endings

### Game overs, endings, trophy fall, staff roll (`gm_19EF`, `gmgover`, `gmmovieend`, `gmstaffroll`, `gmregtyfall`, `gm_1A7A`, `gm_1A9B`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/gm/gm_19EF` | 3 | 724 | ported in `gm/endings/game_over.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_801A0B60`, `gm_Scene_ComingSoon_OnExit`; ported in `gm/endings/game_over.rs` (its doc names it); no check reaches it yet: `gm_Scene_ComingSoon_OnEnter` |

## 1P items

### Adventure enemies and the hands' items (`itlikelike`, `itnokonoko`, `itpatapata`, shells, `itwhitebea`, `itmasterhand*`, `itcrazyhandbomb`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/it/kinds/itzgshell` | 1 | 300 | another agent's row this round (Adventure's enemies, the capture's let-go, the hands' bullet, the event egg): `itZrshell_UnkMotion9_Coll` |

## Debug stages

### The Test stage (`grtest`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/gr/grtest` | 12 | 1,464 | nothing known reaches it: `grTest_80206E30`, `grTest_UnkStage0_OnLoad`, `grTest_UnkStage0_OnStart`, `grTest_80207044`, `grTest_80207164`, `grTest_802071C4`, `stageGObj2_X8Callback`, `grTest_80207404`, `grTest_8020740C`; named in `gr/figure.rs` (perhaps ported inline); no check reaches it yet: `stageGObj0_OnInit`, `stageGObj2_OnInit`; named in `gr/greens.rs` (perhaps ported inline); no check reaches it yet: `stageGObj1_OnInit` |

## Debug only

### Debug code (`ledger/deferred.txt`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/ft/kinds/ftCommon/ftCo_0A01` | 1 | 728 | debug only (`ledger/deferred.txt`): `ftCo_800AEFB8` |
| `melee/ft/kinds/ftZelda/ftzelda` | 1 | 92 | debug only (`ledger/deferred.txt`): `ftZd_Init_801395C8` |
| `melee/gm/gm_1601` | 1 | 176 | debug only (`ledger/deferred.txt`): `gm_80164A0C` |
| `melee/gm/gmhanyucss` | 2 | 284 | debug only (`ledger/deferred.txt`): `gm_801BED3C`, `gm_801BEDA8` |
| `melee/gr/grtest` | 9 | 56 | debug only (`ledger/deferred.txt`): `grTest_80206E2C`, `grTest_8020703C`, `grTest_8020715C`, `grTest_80207168`, `grTest_802071BC`, `grTest_802073AC`, `grTest_802073FC`, `grTest_80207408`, `grTest_80207414` |
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
