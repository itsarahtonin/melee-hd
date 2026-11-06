# What's left to port

Every function the [port ledger](port-ledger.md) still counts as unaccounted, grouped by the mode that reaches it and by unit, with what reaches it and its size in the executable (bytes of code, from the decomp's asm listings). Generated on 2026-11-06 from `feat/m6-ledger` at `b401d6d7` ([milestone 6](milestone-6.md), task 11): 1,078 of 19,827 functions unaccounted, against 2,105 before the task (17,220 ported with a check, 926 replaced, 603 dead).

An unaccounted row is one of these, which the columns count:

- **Ported, no check yet**: melee-hd has the code (a `// Ports X; aliased once a check reaches it` comment, a doc comment naming it, a dispatch arm on its address, or an audit's `UNCHECKED`), but no session, replay diff or leaf-check reaches it, so its alias waits.
- **Reached by a clean check, alias held**: ssbm-rs's `CALLS` shows a session or replay diff that compares clean reaching it, so melee-hd runs it, inline in another port; it has no item of its own for the alias, or its file belongs to a running task.
- **An audit says what reaches it**: the fighter, items, CPU, stage, menus or single-player audit names it (or its unit) with the path that reaches it, usually a single-player mode behind a gate, or why nothing does.
- **Named in melee-hd**: a comment in melee-hd names it, so it may be ported inline; nothing checks it.
- **Nothing known reaches it**: no port, audit or check names it. Most are per-state callbacks of states no check enters.
- **Debug only**: `ledger/deferred.txt`, code only a raised debug level reaches.

How the sweep decided (task 11): the SDK's, MSL's, the runtime's and HSD's native replacements went to `ledger/replaced.txt` (with the crash screen, the disc's file layer and the ARAM queue); `ledger/dead.txt` took the functions nothing can call (only three have no call or table entry at all: MWCC addresses a file's data from one base, so a table no symbol names can still be read, and the asm listings alone prove little else dead) and the AXFX reverb hi and chorus, which no caller selects; `ledger/deferred.txt` the USB host's libraries. The aliases came from ssbm-rs's `CALLS` over 39 checks that compare clean (19 replay diffs of verify-par.sh, 7 sessions of session-expect.txt, 13 more replay diffs of the characters), which logged every unaccounted function they call; each one reached got its alias where melee-hd ports it, with a `ledger/checks.txt` line naming those checks. The render state setters were aliased with the Fox references' frames checks, each one's copy broken on purpose to see them fail.

**Nothing here can stop a match in a mode whose gate is open** (VS, Training, Target Test, Home-Run Contest, the menus) as far as the checks show: every function those 39 checks reached is ported (the clean comparisons say so), and the open modes' units are held by the audits (`menus_audit`, `single_player_audit`, the fighter, items, CPU and stage audits), which fail on anything they reach unported. What a VS match could still reach unchecked is the first section's "ported, no check yet" and "nothing known" rows: states and item callbacks no recorded match enters.

## Summary

| Mode | Functions | Bytes | Ported, no check yet | Reached by a clean check, alias held | An audit says what reaches it | Named in melee-hd, no check yet | Nothing known reaches it | Debug only |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| [VS and the menus](#vs-and-the-menus) | 530 | 54,336 | 317 | 58 | 57 | 24 | 74 | 0 |
| [Libraries](#libraries) | 115 | 28,264 | 35 | 0 | 0 | 40 | 40 | 0 |
| [Classic](#classic) | 18 | 4,564 | 4 | 0 | 14 | 0 | 0 | 0 |
| [Adventure](#adventure) | 13 | 1,544 | 4 | 0 | 9 | 0 | 0 | 0 |
| [All-Star](#all-star) | 2 | 140 | 0 | 0 | 2 | 0 | 0 | 0 |
| [Event Match](#event-match) | 2 | 240 | 0 | 1 | 1 | 0 | 0 | 0 |
| [1P shared](#1p-shared) | 42 | 6,732 | 28 | 2 | 12 | 0 | 0 | 0 |
| [1P endings](#1p-endings) | 3 | 724 | 3 | 0 | 0 | 0 | 0 | 0 |
| [Bosses](#bosses) | 62 | 8,064 | 62 | 0 | 0 | 0 | 0 | 0 |
| [1P stages](#1p-stages) | 78 | 3,004 | 58 | 1 | 0 | 1 | 18 | 0 |
| [1P items](#1p-items) | 99 | 6,812 | 96 | 0 | 3 | 0 | 0 | 0 |
| [1P computer kinds](#1p-computer-kinds) | 6 | 3,972 | 1 | 0 | 5 | 0 | 0 | 0 |
| [Debug stages](#debug-stages) | 21 | 1,520 | 0 | 0 | 0 | 0 | 21 | 0 |
| [Debug only](#debug-only) | 87 | 21,612 | 0 | 0 | 0 | 0 | 0 | 87 |
| **All** | **1078** | **141,528** | **608** | **62** | **103** | **65** | **153** | **87** |

## VS and the menus

Reached from the open modes; nothing listed stops a match the checks run (above).

### Game libraries (`lb*`, `cm`, `mp`, `db`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/lb/lbcommand` | 5 | 312 | reached by items-pokeball1-fd: ported inline, no item of its own to carry the alias: `Command_01`, `Command_02`, `Command_03`, `Command_05`, `Command_08` |
| `melee/lb/lb_00B0` | 5 | 276 | named in `melee-render/src/hsd/material.rs` (perhaps ported inline); no check reaches it yet: `lbGetFreeAlphaRegImpl`, `lb_8000CD90`, `lb_8000CDA8`; ported in `melee-render/src/hsd/material.rs` (its doc names it); no check reaches it yet: `lbGetFreeAlphaRegister`; nothing known reaches it: `lb_8000CE40` |
| `melee/lb/lbshadow` | 1 | 376 | nothing known reaches it: `lbShadow_8000F214` |
| `melee/lb/lbspdisplay` | 5 | 4,932 | named in `melee-render/src/hsd/blur.rs` (perhaps ported inline); no check reaches it yet: `lb_800122F0`, `lb_8001271C`, `lb_8001285C`, `lb_80012994`; ported in `melee-sim/src/lb/blur.rs` (its doc names it); no check reaches it yet: `fn_800138AC` |
| `melee/lb/lb_0192` | 2 | 812 | reached by items-pokeball1-fd: ported inline, no item of its own to carry the alias: `lb_80019230`, `lb_800192A8` |
| `melee/lb/lb_0195` | 2 | 216 | nothing known reaches it: `lb_8001955C`, `lb_80019A48` |
| `melee/lb/lb_0219` | 1 | 4 | named in `melee-sim/src/lb/bgflash.rs` (perhaps ported inline); no check reaches it yet: `fn_80021C18` |
| `melee/lb/lbrefract` | 8 | 804 | nothing known reaches it: `fn_80021F70`, `fn_80021FB4`, `fn_80021FF8`, `fn_8002206C`, `lbRefract_ReadTexCoordRGBA8`, `lbRefract_DObjDispReset`, `fn_80022650`, `fn_80022940` |
| `melee/lb/lbaudio_ax` | 5 | 832 | Master Hand's and Crazy Hand's sounds: the single-player modes', which only the 1P list's gated entries lead to (`menus.txt`): `lbAudioAx_8002438C`; the console's reset button and the disc error screen, which the player's platform never raises (milestone 5's task 14) (`menus.txt`): `lbAudioAx_80024F08`, `lbAudioAx_80027DBC`; ported in `melee-sim/src/lb/sound_obj.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_800251EC`; ported (a `// Ports` comment in melee-sim), aliased once a session reaches it (`menus.txt`): `fn_800259A0` |

### The player library (`pl*`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/pl/player` | 3 | 556 | reached by items-pokeball2-bf, items-pokeball3-ps: ported inline, no item of its own to carry the alias: `Player_80036978`; nothing known reaches it: `Player_80036EA0`, `Player_80037054` |
| `melee/pl/plbonus` | 1 | 124 | nothing known reaches it: `pl_80038898` |
| `melee/pl/plbonuslib` | 3 | 116 | named in `pl/trick.rs` (perhaps ported inline); no check reaches it yet: `pl_8003FE40`; ported in `ft/co/dead.rs` (held by a `// Ports` comment); no check reaches it yet: `pl_8004065C`; nothing known reaches it: `pl_800408DC` |

### Fighters: the shared libraries (`ftaction`, `ftlib`, `ftcommon`, `ftcoll`, `ftcolanim`, `ft_*`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/ft/ftanim` | 2 | 684 | named in `ft/kirby/hat.rs` (perhaps ported inline); no check reaches it yet: `ftAnim_GetNextMatAnimJointInTree`; ported in `ft/bind.rs` (held by a `// Ports` comment); no check reaches it yet: `ftAnim_8006F628` |
| `melee/ft/ftaction` | 27 | 960 | nothing known reaches it: `ftAction_8007168C`, `ftAction_800716F8`, `ftAction_80071774`, `ftAction_800717C8`, `ftAction_8007192C`, `ftAction_80071D30`, `ftAction_80071D94`, `ftAction_80071DCC`, `ftAction_800722C8`, `ftAction_800726C0`, `ftAction_8007297C`, `ftAction_800729C4`, `ftAction_80072A4C`, `ftAction_80072ABC`, `ftAction_80072B04`, `ftAction_80072B14`, `ftAction_80072B84`, `ftAction_80072BE4`, `ftAction_80072C5C`, `ftAction_80072FE0`, `ftAction_8007309C`, `ftAction_80073108`, `ftAction_8007320C`; reached by oracle-cpu-pichu-ys: ported inline, no item of its own to carry the alias: `ftAction_80071810`; reached by items-shoot-fd, oracle-cpu-ness-fd: ported inline, no item of its own to carry the alias: `ftAction_80071F0C`; reached by items-pokeball1-fd: ported inline, no item of its own to carry the alias: `ftAction_80072E24`; named in `ft/commands.rs` (perhaps ported inline); no check reaches it yet: `ftAction_800730B8` |
| `melee/ft/ftparts` | 1 | 752 | nothing known reaches it: `ftPartsSetupSharedVtxMtx` |
| `melee/ft/ft_07C1` | 1 | 104 | named in `ft/motion.rs` (perhaps ported inline); no check reaches it yet: `ft_8007C114` |
| `melee/ft/ftcommon` | 5 | 344 | ported in `ft/phys.rs` (held by a `// Ports` comment); no check reaches it yet: `ftCommon_ClampFallSpeed`, `ftCommon_8007F76C`, `ftCommon_8007F7B4`; reached by items-pokeball1-fd: ported inline, no item of its own to carry the alias: `ftCommon_8007E2F4`; named in `ft/commands.rs` (perhaps ported inline); no check reaches it yet: `ftCommon_8007EC30` |
| `melee/ft/ftdata` | 1 | 196 | nothing known reaches it: `ftData_800858E4` |
| `melee/ft/ftlib` | 9 | 388 | nothing known reaches it: `ftLib_GetHipJObj`, `ftLib_DisableAllInput`, `ftLib_GetCollData`, `ftLib_IsShielding`; reached by rec-2026-10-06_16-58-24, tou-bracket64: ported inline, no item of its own to carry the alias: `ftLib_GetPrevPos`; reached by items-pokeball2-bf, items-pokeball3-ps: ported inline, no item of its own to carry the alias: `ftLib_GetCameraSubjectBonePos`; reached by menu-menu-tour: ported inline, no item of its own to carry the alias: `ftLib_IsCameraSubjectInBounds`; reached by items-pokeball1-fd: ported inline, no item of its own to carry the alias: `ftLib_GetPlayerIndex`; named in `it/kinds/flipper.rs` (perhaps ported inline); no check reaches it yet: `ftLib_80087284` |
| `melee/ft/ft_0877` | 4 | 68 | nothing known reaches it: `ft_80087838`, `ft_80087C58`, `ft_80087C64`; named in `ft/cpu_decide.rs` (perhaps ported inline); no check reaches it yet: `ft_80087A80` |
| `melee/ft/ftmaterial` | 3 | 1,648 | reached by items-pokeball1-fd: ported inline, no item of its own to carry the alias: `ftMaterial_800BF260`; reached by items-pokeball1-fd (`melee-render/src/hsd/material.rs`): ported inline, no item of its own to carry the alias: `ftMaterial_800BF534`, `ftMaterial_800BF6BC` |
| `melee/ft/ftcolanim` | 6 | 192 | reached by items-pokeball2-bf, items-pokeball3-ps: ported inline, no item of its own to carry the alias: `ftCo_800BFE74`, `ftCo_800BFEB4`; reached by items-pokeball5-bf, items-shoot-fd: ported inline, no item of its own to carry the alias: `ftCo_800BFE94`; nothing known reaches it: `ftCo_800BFED4`, `ftCo_800BFEF4`, `ftCo_800BFF14` |
| `melee/ft/ftdevice` | 1 | 32 | nothing known reaches it: `ftCo_800C0674` |
| `melee/ft/ft_0C31` | 2 | 72 | ported in `ft/co/entry.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_800C63BC`, `fn_800C63E0` |
| `melee/ft/ftCo_800C7590` | 1 | 32 | ported in `ft/co/capture_likelike.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_800C7890` |
| `melee/ft/ftCo_800C78B0` | 1 | 40 | ported in `ft/co/capture_likelike.rs` (held by a `// Ports` comment); no check reaches it yet: `ftCo_800C7A30` |
| `melee/ft/ftCo_800C7CA0` | 1 | 4 | reached by tou-bracket64: ported inline, no item of its own to carry the alias: `ftCo_DownReflect_IASA` |
| `melee/ft/ft_0C88` | 1 | 20 | named in `ft/commands.rs` (perhaps ported inline); no check reaches it yet: `ftCo_800C8B60` |
| `melee/ft/ft_0C8C` | 1 | 36 | nothing known reaches it: `ftCo_800C9034` |
| `melee/ft/ft_0D31` | 3 | 632 | ported in `ft/co/dead.rs` (held by a `// Ports` comment); no check reaches it yet: `ftCo_800D3E40`, `ftCo_DeadUp_Anim`, `ftCo_DeadUp_Cam` |
| `melee/ft/ft_0D4D` | 1 | 60 | ported in `ft/co/rebirth.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_800D5A30` |

### Fighters: the common states (`ftCo_*`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/ft/kinds/ftCommon/ftCo_AttackDash` | 2 | 176 | reached by items-pokeball1-fd: ported inline, no item of its own to carry the alias: `decideFighter`, `doEnter` |
| `melee/ft/kinds/ftCommon/ftCo_AttackHi3` | 1 | 96 | reached by items-pokeball2-bf, items-pokeball3-ps: ported inline, no item of its own to carry the alias: `doEnter` |
| `melee/ft/kinds/ftCommon/ftCo_AttackLw3` | 2 | 200 | reached by items-pokeball1-fd: ported inline, no item of its own to carry the alias: `decideFighter`, `doEnter` |
| `melee/ft/kinds/ftCommon/ftCo_AttackS4` | 2 | 472 | reached by items-pokeball1-fd: ported inline, no item of its own to carry the alias: `decideFighter`, `doEnter` |
| `melee/ft/kinds/ftCommon/ftCo_AttackHi4` | 1 | 96 | reached by items-pokeball2-bf, items-pokeball3-ps: ported inline, no item of its own to carry the alias: `doEnter` |
| `melee/ft/kinds/ftCommon/ftCo_AttackLw4` | 1 | 96 | reached by items-pokeball2-bf, items-pokeball3-ps: ported inline, no item of its own to carry the alias: `doEnter` |
| `melee/ft/kinds/ftCommon/ftCo_AttackAir` | 1 | 92 | reached by items-pokeball1-fd: ported inline, no item of its own to carry the alias: `decideFighter` |
| `melee/ft/kinds/ftCommon/ftCo_Damage` | 4 | 792 | named in `ft/donkey/heavy.rs` (perhaps ported inline); no check reaches it yet: `ftCo_ScaleBy154`, `ftCo_8008D8E8`; ported in `ft/co/hit.rs` (held by a `// Ports` comment); no check reaches it yet: `ftCo_8008E9D0`; named in `ft/resolve.rs` (perhaps ported inline); no check reaches it yet: `ftCo_8008EB58` |
| `melee/ft/kinds/ftCommon/ftCo_SpecialS` | 1 | 128 | reached by items-pokeball1-fd: ported inline, no item of its own to carry the alias: `doEnter` |
| `melee/ft/kinds/ftCommon/ftCo_StopWall` | 1 | 212 | ported in `ft/co/ledge.rs` (held by a `// Ports` comment); no check reaches it yet: `ftCo_8009EE30` |
| `melee/ft/kinds/ftCommon/ftCo_ThrownKirby` | 1 | 36 | nothing known reaches it: `ftCo_800BDA50` |
| `melee/ft/kinds/ftCommon/ftCo_PassiveCeil` | 1 | 4 | nothing known reaches it: `ftCo_PassiveCeil_IASA` |
| `melee/ft/kinds/ftCommon/ftCo_DamageSong` | 3 | 12 | reached by items-pokeball1-fd: ported inline, no item of its own to carry the alias: `ftCo_DamageSong_IASA`, `ftCo_DamageSongWait_IASA`, `ftCo_DamageSongRv_IASA` |
| `melee/ft/kinds/ftCommon/ftCo_AirCatch` | 4 | 248 | ported in `ft/samus/tether.rs` (held by a `// Ports` comment); no check reaches it yet: `ftCo_800C3CC0`, `ftCo_AirCatchHit_Phys`; ported in `ft/samus/tether.rs` (dispatched by address); no check reaches it yet: `ftCo_AirCatchHit_Anim`, `ftCo_AirCatchHit_IASA` |
| `melee/ft/kinds/ftCommon/ftCo_HammerWalk` | 1 | 32 | ported in `ft/co/hammer.rs` (dispatched by address); no check reaches it yet: `ftCo_HammerWalk_Phys` |
| `melee/ft/kinds/ftCommon/ftCo_HammerJump` | 2 | 36 | ported in `ft/co/hammer.rs` (dispatched by address); no check reaches it yet: `ftCo_HammerJump_IASA`, `ftCo_HammerJump_Phys` |
| `melee/ft/kinds/ftCommon/ftCo_HammerKneeBend` | 1 | 32 | ported in `ft/co/hammer.rs` (dispatched by address); no check reaches it yet: `ftCo_HammerKneeBend_IASA` |
| `melee/ft/kinds/ftCommon/ftCo_HammerFall` | 1 | 32 | ported in `ft/co/hammer.rs` (dispatched by address); no check reaches it yet: `ftCo_HammerFall_Phys` |
| `melee/ft/kinds/ftCommon/ftCo_HammerLanding` | 1 | 4 | ported in `ft/co/hammer.rs` (dispatched by address); no check reaches it yet: `ftCo_HammerLanding_IASA` |
| `melee/ft/kinds/ftCommon/ftCo_Barrel` | 9 | 752 | ported in `ft/co/barrel.rs` (held by a `// Ports` comment); no check reaches it yet: `ftCo_Barrel_Death2_Cb`, `ftCo_Barrel_Take_Dmg_Cb`, `ftCo_Barrel_Anim`, `ftCo_Barrel_IASA`, `ftCo_Barrel_Phys`, `ftCo_Barrel_Coll`, `ftCo_Barrel_Accessory1_Cb`, `fn_800C9290`, `ftCo_800C92E4` |
| `melee/ft/kinds/ftCommon/ftCo_RunDirect` | 4 | 376 | nothing known reaches it: `ftCo_RunDirect_Anim`, `ftCo_RunDirect_IASA`, `ftCo_RunDirect_Phys`, `ftCo_RunDirect_Coll` |
| `melee/ft/kinds/ftCommon/ftCo_CatchPull` | 1 | 80 | ported in `ft/co/grab.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_800DA004` |
| `melee/ft/kinds/ftCommon/ftCo_CatchWait` | 2 | 96 | ported in `ft/co/grab.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_800DA440`, `fn_800DA490` |
| `melee/ft/kinds/ftCommon/ftCo_CatchAttack` | 1 | 80 | ported in `ft/co/grab.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_800DA618` |
| `melee/ft/kinds/ftCommon/ftCo_0DC2` | 5 | 440 | reached by items-pokeball3-ps, items-held-fd (`ft/co/dispatch.rs`): ported inline, no item of its own to carry the alias: `ftCo_CaptureDamageHi_IASA`; ported in `ft/co/grab.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_800DC384`, `fn_800DC404`, `fn_800DC624`; reached by items-pokeball1-fd (`ft/co/dispatch.rs`): ported inline, no item of its own to carry the alias: `ftCo_CaptureDamageLw_IASA` |
| `melee/ft/kinds/ftCommon/ftCo_Throw` | 2 | 248 | ported in `ft/co/throw.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_800DD5EC`, `fn_800DD684` |
| `melee/ft/kinds/ftCommon/ftCo_Thrown` | 1 | 84 | ported in `ft/co/throw.rs` (held by a `// Ports` comment); no check reaches it yet: `ftCo_800DE920` |

### Fighters: the characters' own states

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/ft/kinds/ftMario/ftmario` | 2 | 144 | ported in `ft/mario/init.rs` (dispatched by address); no check reaches it yet: `ftMr_Init_OnItemInvisible`, `ftMr_Init_OnItemVisible` |
| `melee/ft/kinds/ftCaptain/ftcaptain` | 2 | 144 | ported in `ft/captain/init.rs` (dispatched by address); no check reaches it yet: `ftCa_Init_OnItemInvisible`, `ftCa_Init_OnItemVisible` |
| `melee/ft/kinds/ftLink/ftlinkspecialhi` | 1 | 4 | ported in `ft/link/special_hi.rs` (dispatched by address); no check reaches it yet: `ftLk_SpecialHi_IASA` |
| `melee/ft/kinds/ftKirby/ftkirby` | 5 | 452 | ported in `ft/kirby/init.rs` (held by a `// Ports` comment); no check reaches it yet: `ftKb_Init_OnItemPickup`, `ftKb_Init_OnItemInvisible`, `ftKb_Init_OnItemVisible`, `ftKb_Init_OnItemDrop`; nothing known reaches it: `ftKb_Init_UnkMotionStates6` |
| `melee/ft/kinds/ftKirby/ftkirbyspecialn` | 1 | 32 | ported in `ft/kirby/special_n.rs` (its doc names it); no check reaches it yet: `ftKb_EatWalk_Anim` |
| `melee/ft/kinds/ftKirby/ftkirbyspecialyoshi` | 2 | 328 | ported in `ft/kirby/special_yoshi.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_801095DC`, `fn_80109714` |
| `melee/ft/kinds/ftDonkey/ftdonkey` | 2 | 144 | ported in `ft/donkey/init.rs` (held by a `// Ports` comment); no check reaches it yet: `ftDk_Init_OnItemInvisible`, `ftDk_Init_OnItemVisible` |
| `melee/ft/kinds/ftDonkey/ftdonkeyspecials` | 1 | 136 | ported in `ft/donkey/special_s.rs` (held by a `// Ports` comment); no check reaches it yet: `doGroundTransition` |
| `melee/ft/kinds/ftNess/ftness` | 1 | 72 | ported in `ft/ness/init.rs` (held by a `// Ports` comment); no check reaches it yet: `ftNs_Init_OnItemVisible` |
| `melee/ft/kinds/ftNess/ftnessattackhi4` | 7 | 660 | named in `ft/ness/attack_hi4.rs` (perhaps ported inline); no check reaches it yet: `ftNs_AttackHi4_YoyoApplyDamage`; ported in `ft/ness/attack_hi4.rs` (dispatched by address); no check reaches it yet: `ftNs_AttackHi4Charge_IASA`, `ftNs_AttackHi4Charge_Phys`, `ftNs_AttackHi4Charge_Coll`, `ftNs_AttackHi4Release_Anim`, `ftNs_AttackHi4Release_IASA`, `ftNs_AttackHi4Release_Coll` |
| `melee/ft/kinds/ftNess/ftnessattacklw4` | 6 | 380 | ported in `ft/ness/attack_lw4.rs` (dispatched by address); no check reaches it yet: `ftNs_AttackLw4Charge_IASA`, `ftNs_AttackLw4Charge_Phys`, `ftNs_AttackLw4Charge_Coll`, `ftNs_AttackLw4Release_Anim`, `ftNs_AttackLw4Release_IASA`, `ftNs_AttackLw4Release_Coll` |
| `melee/ft/kinds/ftNess/ftnessspecials` | 1 | 32 | ported in `ft/ness/special_s.rs` (dispatched by address); no check reaches it yet: `ftNs_SpecialAirS_Phys` |
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
| `melee/ft/kinds/ftKoopa/ftkoopa` | 4 | 452 | ported in `ft/koopa/init.rs` (held by a `// Ports` comment); no check reaches it yet: `ftKp_Init_OnItemPickup`, `ftKp_Init_OnItemInvisible`, `ftKp_Init_OnItemVisible`, `ftKp_Init_OnItemDrop` |
| `melee/ft/kinds/ftZelda/ftzelda` | 3 | 368 | ported in `ft/zelda/init.rs` (held by a `// Ports` comment); no check reaches it yet: `ftZd_Init_OnItemPickup`, `ftZd_Init_OnItemInvisible`, `ftZd_Init_OnItemVisible` |
| `melee/ft/kinds/ftZelda/ftzeldaspecialhi` | 2 | 64 | ported in `ft/seak/special_n.rs` (dispatched by address); no check reaches it yet: `ftZd_SpecialHiStart_0_Phys`, `ftZd_SpecialHi_Phys` |
| `melee/ft/kinds/ftPurin/ftpurin` | 3 | 148 | nothing known reaches it: `ftPr_Init_OnUserDataRemove`, `ftPr_Init_8013C494`, `ftPr_Init_UnkMotionStates6` |
| `melee/ft/kinds/ftPurin/ftpurinspecialhi` | 4 | 72 | ported in `ft/purin/special_hi.rs` (dispatched by address); no check reaches it yet: `ftPr_SpecialHi_IASA`, `ftPr_SpecialAirHi_IASA`, `ftPr_SpecialHi_Phys`, `ftPr_SpecialAirHi_Phys` |
| `melee/ft/kinds/ftPurin/ftpurinspeciallw` | 4 | 72 | ported in `ft/purin/special_lw.rs` (dispatched by address); no check reaches it yet: `ftPr_SpecialLw_IASA`, `ftPr_SpecialAirLw_IASA`, `ftPr_SpecialLw_Phys`, `ftPr_SpecialAirLw_Phys` |
| `melee/ft/kinds/ftPurin/ftpurinspecials` | 1 | 4 | ported in `ft/purin/special_s.rs` (dispatched by address); no check reaches it yet: `ftPr_SpecialAirS_IASA` |
| `melee/ft/kinds/ftPurin/ftpurinspecialn` | 32 | 8,616 | ported in `ft/purin/special_n.rs` (dispatched by address); no check reaches it yet: `ftPr_SpecialNStart_Anim`, `ftPr_SpecialNLoop_Anim`, `ftPr_SpecialNFull_Anim`, `ftPr_SpecialNRelease_Anim`, `ftPr_SpecialNEnd_Anim`, `ftPr_SpecialAirNStart_Anim`, `ftPr_SpecialAirNChargeLoop_Anim`, `ftPr_SpecialAirNChargeFull_Anim`, `ftPr_SpecialAirNChargeRelease_Anim`, `ftPr_SpecialAirNEnd_Anim`, `ftPr_SpecialNStart_IASA`, `ftPr_SpecialNLoop_IASA`, `ftPr_SpecialNFull_IASA`, `ftPr_SpecialNTurn_IASA`, `ftPr_SpecialNEnd_IASA`, `ftPr_SpecialAirNStart_IASA`, `ftPr_SpecialAirNChargeLoop_IASA`, `ftPr_SpecialAirNChargeFull_IASA`, `ftPr_SpecialAirNChargeRelease_IASA`, `ftPr_SpecialAirNStartTurn_IASA`, `ftPr_SpecialAirNEnd_IASA`, `ftPr_SpecialNHit_IASA`, `ftPr_SpecialNLoop_Phys`, `ftPr_SpecialNFull_Phys`, `ftPr_SpecialAirNStart_Phys`, `ftPr_SpecialAirNChargeLoop_Phys`, `ftPr_SpecialAirNChargeFull_Phys`, `ftPr_SpecialAirNEnd_Phys`, `ftPr_SpecialNLoop_Coll`, `ftPr_SpecialNFull_Coll`, `ftPr_SpecialAirNChargeLoop_Coll`, `ftPr_SpecialAirNChargeFull_Coll` |
| `melee/ft/kinds/ftLuigi/ftluigi` | 2 | 144 | ported in `ft/luigi/init.rs` (dispatched by address); no check reaches it yet: `ftLg_Init_OnItemInvisible`, `ftLg_Init_OnItemVisible` |
| `melee/ft/kinds/ftMewtwo/ftmewtwo` | 4 | 16 | ported in `ft/mewtwo/init.rs` (dispatched by address); no check reaches it yet: `ftMt_Init_OnItemPickup`, `ftMt_Init_OnItemDrop`, `ftMt_Init_OnItemInvisible`, `ftMt_Init_OnItemVisible` |
| `melee/ft/kinds/ftDrMario/ftdrmario` | 4 | 452 | ported in `ft/drmario/mod.rs` (dispatched by address); no check reaches it yet: `ftDr_Init_OnItemPickup`, `ftDr_Init_OnItemInvisible`, `ftDr_Init_OnItemVisible`, `ftDr_Init_OnItemDrop` |
| `melee/ft/kinds/ftPichu/ftpichu` | 4 | 452 | ported in `ft/pichu/mod.rs` (dispatched by address); no check reaches it yet: `ftPc_Init_OnItemPickup`, `ftPc_Init_OnItemInvisible`, `ftPc_Init_OnItemVisible`, `ftPc_Init_OnItemDrop` |
| `melee/ft/kinds/ftGameWatch/ftgamewatch` | 4 | 452 | ported in `ft/gamewatch/init.rs` (held by a `// Ports` comment); no check reaches it yet: `ftGw_Init_OnItemPickup`, `ftGw_Init_OnItemInvisible`, `ftGw_Init_OnItemVisible`, `ftGw_Init_OnItemDrop` |
| `melee/ft/kinds/ftGanon/ftganon` | 4 | 452 | ported in `ft/captain/init.rs` (dispatched by address); no check reaches it yet: `ftGn_Init_OnItemPickup`, `ftGn_Init_OnItemInvisible`, `ftGn_Init_OnItemVisible`, `ftGn_Init_OnItemDrop` |

### The scene machine, VS mode and the menus (`gm`, `mn`, `ty`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/gm/gmmain_lib` | 16 | 616 | reached by tou-bracket64: ported inline, no item of its own to carry the alias: `gmMainLib_8015CD80`; reached by rec-2026-10-06_16-58-24: ported inline, no item of its own to carry the alias: `gmMainLib_8015CEB4`; a getter or setter of the game's data, which its callers' ports read or write inline (`menus.txt`): `gmMainLib_8015CF84`, `gmMainLib_8015DB00`, `gmMainLib_8015DB0C`, `gmMainLib_8015DB18`, `gmMainLib_8015ED5C`; named only by the single-player modes' code (gm_8017CBAC): the single-player modes', which only the 1P list's gated entries lead to (`menus.txt`): `gmMainLib_8015D134`, `gmMainLib_8015D1C8`, `gmMainLib_8015D25C`, `gmMainLib_8015D2F0`, `gmMainLib_8015D384`, `gmMainLib_8015D3FC`, `gmMainLib_8015D418`; named only by the single-player modes' code (gm_801B8024): the single-player modes', which only the 1P list's gated entries lead to (`menus.txt`): `gmMainLib_8015D780`; reached by items-pokeball1-fd: ported inline, no item of its own to carry the alias: `gmMainLib_GetUnlockedCharactersBitmaskPtr` |
| `melee/gm/gmvs` | 8 | 764 | reached by items-pokeball1-fd: ported inline, no item of its own to carry the alias: `gmVs_GetSceneController`, `gmVs_GetSceneState`, `gm_GetStartMeleeRules`; the single-player matches' setup (gm_16A2's lbl_8046B488: Event Match, Multi-Man Melee, Classic's and Adventure's teams and hordes), whose branches… (`menus.txt`): `fn_8016B738`; a getter or setter of the game's data, which its callers' ports read or write inline (`menus.txt`): `gm_AnyControllerPressedStart`, `gm_AnyControllerPressedZ`; ported in `gr/last.rs` (held by a `// Ports` comment); no check reaches it yet: `gm_8016ECE8`; ported in `gm/event.rs` (a running agent's file); no check reaches it yet: `gm_8016F00C` |
| `melee/gm/gmresult` | 1 | 36 | a getter or setter of the game's data, which its callers' ports read or write inline (`menus.txt`): `fn_8017435C` |
| `melee/gm/gmtou_1` | 3 | 1,828 | ported in `gm/tournament/bracket.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_801967E0`, `fn_80197FD8`; ported in `gm/tournament/setup.rs` (held by a `// Ports` comment); no check reaches it yet: `gm_8019A828` |
| `melee/gm/gmtou_2` | 1 | 328 | ported in `gm/tournament/alt.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_8019D074` |
| `melee/gm/gmscene` | 3 | 444 | the scene's debug pause handlers' tests (D-pad up with X, Z), which gm_801A4BD4 installs and only a debug level runs (`menus.txt`): `fn_801A46F4`, `fn_801A47E4`; named only by the single-player modes' code (gm_801A68D8, gm_801A6EE4, gm_801A7B00): the single-player modes', which only the 1P list's gated entries… (`menus.txt`): `gm_GetCurrentSceneEnterData` |
| `melee/gm/gmmenu` | 2 | 240 | the single-player game overs: the single-player modes', which only the 1P list's gated entries lead to (`menus.txt`): `gm_Mode_AdventureGOver_OnLoad`, `gm_Mode_AllstarGOver_OnLoad` |
| `melee/gm/gmvsmelee` | 1 | 4 | ported in the scene machine's dispatch (game/gm/machine.rs, title.rs, howto.rs, camera_mode.rs), held unaliased until a session reaches it (`menus.txt`): `gm_Mode_Vs_OnUnload` |
| `melee/gm/gmhowto` | 2 | 500 | ported in `gm/howto.rs` (its doc names it); no check reaches it yet: `gm_Scene_HowTo_OnEnter`, `gm_Scene_HowTo_OnFrame` |
| `melee/gm/gmomake15` | 2 | 500 | ported in `gm/howto.rs` (its doc names it); no check reaches it yet: `gm_Scene_Omake15_OnEnter`, `gm_Scene_Omake15_OnFrame` |
| `melee/gm/gmtitlemode` | 1 | 256 | reached by rec-2026-10-06_16-58-24, res-stock: ported inline, no item of its own to carry the alias: `onExit` |
| `melee/gm/gmdebugmode` | 1 | 36 | the debug mode's own exit from the prize screen (gmdebugmode.c, mode 6), made pending only at a debug level (task 19's); VS mode's is ported (`menus.txt`): `onExitPrize` |
| `melee/gm/gmtoumode` | 2 | 196 | ported in `gm/tournament/mod.rs` (held by a `// Ports` comment); no check reaches it yet: `gm_801B174C`, `gm_801B1788` |
| `melee/gm/gmopeningmode` | 3 | 264 | ported in `gm/title.rs` (its doc names it); no check reaches it yet: `onExitTitle`; ported in the scene machine's dispatch (game/gm/machine.rs, title.rs, howto.rs, camera_mode.rs), held unaliased until a session reaches it (`menus.txt`): `onExitHowto`, `onExitOmake15` |
| `melee/gm/gmboot` | 1 | 48 | ported in `gm/boot_mode.rs` (held by a `// Ports` comment); no check reaches it yet: `memcardOnLoad` |
| `melee/mn/mnstagesel` | 1 | 24 | a getter or setter of the game's data, which its callers' ports read or write inline (`menus.txt`): `mnStageSel_8025BC08` |
| `melee/mn/mncharsel` | 2 | 144 | ported in `mn/charsel/procs.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_8025FAC0`, `fn_8025FB2C` |
| `melee/ty/tydisplay` | 1 | 256 | ported in `ty/collection.rs` (held by a `// Ports` comment); no check reaches it yet: `tyDisplay_8031C354` |

### Stages: the shared stage code and VS stages' leftovers

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/gr/ground` | 1 | 4 | nothing known reaches it: `Ground_801BFFAC` |
| `melee/gr/granime` | 2 | 356 | nothing known reaches it: `fn_801C6EE4`; reached by items-pokeball1-fd: ported inline, no item of its own to carry the alias: `grAnime_801C6F50` |
| `melee/gr/grmaterial` | 4 | 288 | reached by items-pokeball1-fd: ported inline, no item of its own to carry the alias: `grMaterial_801C8E74`; nothing known reaches it: `grMaterial_801C9470`, `fn_801C9664`; named in `gr/material.rs` (perhaps ported inline); no check reaches it yet: `grMaterial_801C9490` |
| `melee/gr/grzakogenerator` | 1 | 180 | ported in `gr/zako.rs` (held by a `// Ports` comment); no check reaches it yet: `grZakoGenerator_801CAF08` |
| `melee/gr/grpstadium` | 1 | 340 | the screen's count, which only single-player modes show (`roster.rs`): `grStadium_801D384C` |
| `melee/gr/gricemt` | 2 | 56 | ported in `gr/icemt.rs` (held by a `// Ports` comment); no check reaches it yet: `stageGObj10_Callback3`, `fn_801FA4CC` |

### Items: the item engine (`it_26B1`, `it_2725`, `itanimlist`, `item`, `itcoll`, `itmaterial`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/it/it_26B1` | 5 | 248 | ported, held for a check (`items_audit.rs`'s UNCHECKED): `itGetTeamId`; ported in `it/lib26b1.rs` (held by a `// Ports` comment); no check reaches it yet: `it_8026C1E8`, `it_8026C334`; Master Hand's and Crazy Hand's items, which only those fighters spawn (`items_audit.rs`): `it_8026C3FC`, `it_8026C42C` |
| `melee/it/it_2725` | 1 | 76 | ported in `it/release.rs` (held by a `// Ports` comment); no check reaches it yet: `it_80274250` |
| `melee/it/itmaterial` | 3 | 1,640 | reached by items-pokeball1-fd: ported inline, no item of its own to carry the alias: `it_80277D08`; reached by items-pokeball1-fd (`melee-render/src/hsd/material.rs`): ported inline, no item of its own to carry the alias: `it_80277F90`, `it_80278108` |
| `melee/it/itanimlist` | 5 | 232 | ported, held for a check (`items_audit.rs`'s UNCHECKED): `it_802798D4`, `it_802799A8`, `fn_80279B30`; the items' own color script commands 21 and 22, which none of ItCo.usd's six item color animations uses (`items_audit.rs`): `it_80279AF0`, `it_80279B10` |

### Items: the kinds' callbacks

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/it/kinds/itcapsule` | 1 | 32 | an item's evt_unk, which only a fighter's unload runs (it_8026B7F8, game::it::evt_unk dispatches each): a player leaving a match in progress, which… (`items_audit.rs`): `itCapsule_Logic0_EvtUnk` |
| `melee/it/kinds/ittomato` | 2 | 248 | a spawn by kind of a box, Flipper or food (it_8026BE84): the All-Star rest area's (Big Blue's flying platforms make only the five kinds ported, the… (`items_audit.rs`): `it_802841B4`; an item's evt_unk, which only a fighter's unload runs (it_8026B7F8, game::it::evt_unk dispatches each): a player leaving a match in progress, which… (`items_audit.rs`): `itTomato_Logic9_EvtUnk` |
| `melee/it/kinds/itstar` | 1 | 48 | ported in `it/kinds/bat.rs` (dispatched by address); no check reaches it yet: `itStar_UnkMotion0_Phys` |
| `melee/it/kinds/itsword` | 1 | 32 | ported, held for a check (`items_audit.rs`'s UNCHECKED): `it_80284E10` |
| `melee/it/kinds/itegg` | 1 | 32 | an item's evt_unk, which only a fighter's unload runs (it_8026B7F8, game::it::evt_unk dispatches each): a player leaving a match in progress, which… (`items_audit.rs`): `itEgg_Logic3_EvtUnk` |
| `melee/it/kinds/itfreeze` | 1 | 32 | an item's evt_unk, which only a fighter's unload runs (it_8026B7F8, game::it::evt_unk dispatches each): a player leaving a match in progress, which… (`items_audit.rs`): `itFreeze_Logic17_EvtUnk` |
| `melee/it/kinds/itfoods` | 1 | 32 | an item's evt_unk, which only a fighter's unload runs (it_8026B7F8, game::it::evt_unk dispatches each): a player leaving a match in progress, which… (`items_audit.rs`): `itFoods_Logic18_EvtUnk` |
| `melee/it/kinds/ittarucann` | 3 | 504 | ported in `it/kinds/tarucann.rs` (held by a `// Ports` comment); no check reaches it yet: `it_80295F38`, `it_3F14_Logic5_Dropped`; an item's evt_unk, which only a fighter's unload runs (it_8026B7F8, game::it::evt_unk dispatches each): a player leaving a match in progress, which… (`items_audit.rs`): `itTaruCann_Logic5_EvtUnk` |
| `melee/it/kinds/itmball` | 1 | 32 | an item's evt_unk, which only a fighter's unload runs (it_8026B7F8, game::it::evt_unk dispatches each): a player leaving a match in progress, which… (`items_audit.rs`): `itMball_EvtUnk` |
| `melee/it/kinds/itlgunray` | 1 | 32 | an item's evt_unk, which only a fighter's unload runs (it_8026B7F8, game::it::evt_unk dispatches each): a player leaving a match in progress, which… (`items_audit.rs`): `itLGunRay_Logic35_EvtUnk` |
| `melee/it/kinds/itstarrodstar` | 1 | 32 | an item's evt_unk, which only a fighter's unload runs (it_8026B7F8, game::it::evt_unk dispatches each): a player leaving a match in progress, which… (`items_audit.rs`): `itStarRodStar_Logic36_EvtUnk` |
| `melee/it/kinds/itsscopebeam` | 1 | 32 | an item's evt_unk, which only a fighter's unload runs (it_8026B7F8, game::it::evt_unk dispatches each): a player leaving a match in progress, which… (`items_audit.rs`): `itSScopeBeam_Logic38_EvtUnk` |
| `melee/it/kinds/itlgunbeam` | 1 | 32 | an item's evt_unk, which only a fighter's unload runs (it_8026B7F8, game::it::evt_unk dispatches each): a player leaving a match in progress, which… (`items_audit.rs`): `itLGunBeam_Logic39_EvtUnk` |
| `melee/it/kinds/itlipstickspore` | 1 | 32 | an item's evt_unk, which only a fighter's unload runs (it_8026B7F8, game::it::evt_unk dispatches each): a player leaving a match in progress, which… (`items_audit.rs`): `itLipstickSpore_Logic37_EvtUnk` |
| `melee/it/kinds/itfflowerflame` | 1 | 32 | an item's evt_unk, which only a fighter's unload runs (it_8026B7F8, game::it::evt_unk dispatches each): a player leaving a match in progress, which… (`items_audit.rs`): `itFFlowerFlame_Logic41_EvtUnk` |
| `melee/it/kinds/itmariofireball` | 1 | 32 | nothing known reaches it: `itMarioFireball_Logic87_EvtUnk` |
| `melee/it/kinds/itkirbycutterbeam` | 1 | 32 | ported in `it/kinds/kirbycutterbeam.rs` (held by a `// Ports` comment); no check reaches it yet: `it_8029C4B4` |
| `melee/it/kinds/itfoxillusion` | 1 | 32 | ported in `it/kinds/foxillusion.rs` (held by a `// Ports` comment); no check reaches it yet: `it_8029D948` |
| `melee/it/kinds/itnesspkfire` | 1 | 32 | ported in `it/kinds/nesspkfire.rs` (held by a `// Ports` comment); no check reaches it yet: `it_802AA474` |
| `melee/it/kinds/itnesspkfirepillar` | 1 | 32 | ported in `it/kinds/nesspkfirepillar.rs` (held by a `// Ports` comment); no check reaches it yet: `itNesspkfirepillar_Logic24_EvtUnk` |
| `melee/it/kinds/itnesspkflash` | 3 | 48 | ported in `it/kinds/bat.rs` (dispatched by address); no check reaches it yet: `itNesspkflash_Logic102_Clanked`; ported in `it/kinds/arwinglaser.rs` (dispatched by address); no check reaches it yet: `itNesspkflash_Logic102_Absorbed`; ported in `it/kinds/nesspkflash.rs` (held by a `// Ports` comment); no check reaches it yet: `itNesspkflash_Logic102_EvtUnk` |
| `melee/it/kinds/itnesspkthunderball` | 2 | 68 | ported in `it/kinds/bat.rs` (dispatched by address); no check reaches it yet: `it_802AC074`; ported in `it/kinds/nesspkthunder.rs` (held by a `// Ports` comment); no check reaches it yet: `it_802AC41C` |
| `melee/it/kinds/itnessbat` | 1 | 32 | ported in `it/kinds/nessbat.rs` (held by a `// Ports` comment); no check reaches it yet: `itNessBat_Logic58_EvtUnk` |
| `melee/it/kinds/it_2ADA` | 1 | 32 | named in `it/kinds/kirbycopystar.rs` (perhaps ported inline); no check reaches it yet: `it_802ADBE4` |
| `melee/it/kinds/itkirbyhammer` | 1 | 32 | ported in `it/kinds/kirbyhammer.rs` (held by a `// Ports` comment); no check reaches it yet: `it_802ADDB0` |
| `melee/it/kinds/itfoxblaster` | 5 | 352 | ported in `it/kinds/foxblaster.rs` (dispatched by address); no check reaches it yet: `itFoxblaster_UnkMotion9_Anim`, `itFoxblaster_UnkMotion9_Phys`, `itFoxblaster_UnkMotion9_Coll`, `itFoxblaster_UnkMotion10_Phys`, `itFoxblaster_UnkMotion10_Coll` |
| `melee/it/kinds/itnesspkflashexplode` | 2 | 40 | ported in `it/kinds/bat.rs` (dispatched by address); no check reaches it yet: `itNessPKFlashExplode_Logic103_Clanked`; ported in `it/kinds/nesspkflashexplode.rs` (held by a `// Ports` comment); no check reaches it yet: `itNessPKFlashExplode_Logic103_EvtUnk` |
| `melee/it/kinds/itpikachuthunder` | 2 | 64 | ported in `it/kinds/bat.rs` (dispatched by address); no check reaches it yet: `itPikachuThunder_Logic39_Clanked`; ported in `it/evt_unk.rs` (held by a `// Ports` comment); no check reaches it yet: `itPikachuThunder_Logic39_EvtUnk` |
| `melee/it/kinds/itmariocape` | 1 | 32 | nothing known reaches it: `it_802B2870` |
| `melee/it/kinds/itpikachutjoltground` | 1 | 32 | ported in `it/kinds/pikachutjoltground.rs` (held by a `// Ports` comment); no check reaches it yet: `itPikachuTJoltGround_Logic106_EvtUnk` |
| `melee/it/kinds/itpikachutjoltair` | 1 | 32 | ported in `it/kinds/pikachutjoltair.rs` (held by a `// Ports` comment); no check reaches it yet: `itPikachuTJoltAir_Logic107_EvtUnk` |
| `melee/it/kinds/itsamusbomb` | 1 | 52 | ported in `it/evt_unk.rs` (held by a `// Ports` comment); no check reaches it yet: `itSamusBomb_Logic50_EvtUnk` |
| `melee/it/kinds/itsamuschargeshot` | 1 | 32 | ported in `it/kinds/samuschargeshot.rs` (held by a `// Ports` comment); no check reaches it yet: `itSamusChargeshot_Logic108_EvtUnk` |
| `melee/it/kinds/itsamusmissile` | 1 | 76 | ported in `it/evt_unk.rs` (held by a `// Ports` comment); no check reaches it yet: `it_2725_Logic52_EvtUnk` |
| `melee/it/kinds/itsamusgrapple` | 1 | 76 | ported in `it/evt_unk.rs` (held by a `// Ports` comment); no check reaches it yet: `itSamusGrapple_Logic53_EvtUnk` |
| `melee/it/kinds/itnessyoyo` | 1 | 64 | ported in `it/kinds/nessyoyo.rs` (held by a `// Ports` comment); no check reaches it yet: `it_2725_Logic59_EvtUnk` |
| `melee/it/kinds/itluigifireball` | 1 | 32 | nothing known reaches it: `itLuigiFireball_Logic89_EvtUnk` |
| `melee/it/kinds/itdrmariopill` | 2 | 116 | ported in `it/kinds/drmariopill.rs` (its doc names it); no check reaches it yet: `itDrMarioPill_PickedUp`; nothing known reaches it: `itDrMarioPill_EvtUnk` |
| `melee/it/kinds/itclimbersice` | 1 | 56 | ported in `it/evt_unk.rs` (held by a `// Ports` comment); no check reaches it yet: `itClimbersIce_Logic90_EvtUnk` |
| `melee/it/kinds/itclimbersblizzard` | 2 | 40 | ported in `it/kinds/bat.rs` (dispatched by address); no check reaches it yet: `itClimbersBlizzard_Clanked`; ported in `it/kinds/climbersblizzard.rs` (held by a `// Ports` comment); no check reaches it yet: `itClimbersBlizzard_EvtUnk` |
| `melee/it/kinds/itclimbersstring` | 1 | 76 | nothing known reaches it: `it_2725_Logic70_EvtUnk` |
| `melee/it/kinds/itzeldadinfire` | 2 | 16 | ported in `it/kinds/bat.rs` (dispatched by address); no check reaches it yet: `itZeldaDinFire_Logic65_Clanked`; ported in `it/kinds/arwinglaser.rs` (dispatched by address); no check reaches it yet: `itZeldaDinFire_Logic65_Absorbed` |
| `melee/it/kinds/itzeldadinfireexplode` | 1 | 8 | ported in `it/kinds/bat.rs` (dispatched by address); no check reaches it yet: `itZeldaDinFireExplode_Logic66_Clanked` |
| `melee/it/kinds/itgamewatchgreenhouse` | 1 | 32 | ported in `it/kinds/gamewatchgreenhouse.rs` (held by a `// Ports` comment); no check reaches it yet: `itGamewatchGreenhouse_EvtUnk` |
| `melee/it/kinds/itgamewatchmanhole` | 1 | 32 | ported in `it/kinds/gamewatchmanhole.rs` (held by a `// Ports` comment); no check reaches it yet: `itGameWatchManhole_Logic72_EvtUnk` |
| `melee/it/kinds/itgamewatchfire` | 1 | 32 | ported in `it/kinds/gamewatchfire.rs` (held by a `// Ports` comment); no check reaches it yet: `itGamewatchFire_EvtUnk` |
| `melee/it/kinds/itgamewatchparachute` | 1 | 32 | ported in `it/kinds/gamewatchparachute.rs` (held by a `// Ports` comment); no check reaches it yet: `itGameWatchParachute_Logic74_EvtUnk` |
| `melee/it/kinds/itgamewatchturtle` | 1 | 32 | ported in `it/kinds/gamewatchturtle.rs` (held by a `// Ports` comment); no check reaches it yet: `itGameWatchTurtle_Logic75_EvtUnk` |
| `melee/it/kinds/itgamewatchbreath` | 1 | 32 | ported in `it/kinds/gamewatchbreath.rs` (held by a `// Ports` comment); no check reaches it yet: `itGameWatchBreath_Logic76_EvtUnk` |
| `melee/it/kinds/itkirbygamewatchchefpan` | 1 | 32 | ported in `it/kinds/kirbygamewatchchefpan.rs` (held by a `// Ports` comment); no check reaches it yet: `itKirbyGameWatchChefPan_Logic113_EvtUnk` |
| `melee/it/kinds/itgamewatchjudge` | 1 | 32 | ported in `it/kinds/gamewatchjudge.rs` (held by a `// Ports` comment); no check reaches it yet: `itGameWatchJudge_Logic77_EvtUnk` |
| `melee/it/kinds/itgamewatchpanic` | 1 | 32 | ported in `it/kinds/gamewatchpanic.rs` (held by a `// Ports` comment); no check reaches it yet: `itGameWatchPanic_Logic78_EvtUnk` |
| `melee/it/kinds/itgamewatchrescue` | 1 | 32 | ported in `it/kinds/gamewatchrescue.rs` (held by a `// Ports` comment); no check reaches it yet: `itGameWatchRescue_Logic81_EvtUnk` |
| `melee/it/kinds/itgamewatchchef` | 1 | 32 | ported in `it/kinds/gamewatchchef.rs` (held by a `// Ports` comment); no check reaches it yet: `itGameWatchChef_Logic112_EvtUnk` |
| `melee/it/kinds/itcerebi` | 1 | 32 | ported in `it/kinds/cerebi.rs` (held by a `// Ports` comment); no check reaches it yet: `it_802D3F4C` |
| `melee/it/kinds/itlucky` | 19 | 940 | ported in `it/kinds/lucky.rs` (held by a `// Ports` comment); no check reaches it yet: `it_802D5710`, `itLucky_Logic44_Spawned`, `it_802D582C`, `it_802D5884`, `it_802D58BC`, `it_802D58C0`, `it_802D58EC`, `itLucky_UnkMotion3_Anim`, `itLucky_UnkMotion3_Phys`, `itLucky_UnkMotion3_Coll`, `itLucky_Logic44_PickedUp`, `itLucky_UnkMotion2_Anim`, `itLucky_UnkMotion2_Phys`, `itLucky_Logic44_Dropped`, `itLucky_Logic44_EnteredAir`, `it_802D5A2C`, `it_802D5A64`, `it_802D5A68`, `itLucky_Logic44_EvtUnk` |
| `melee/it/kinds/itheiho` | 1 | 32 | an item's evt_unk, which only a fighter's unload runs (it_8026B7F8, game::it::evt_unk dispatches each): a player leaving a match in progress, which… (`items_audit.rs`): `it_802D9A0C` |
| `melee/it/kinds/it_2E5A` | 1 | 32 | an item's evt_unk, which only a fighter's unload runs (it_8026B7F8, game::it::evt_unk dispatches each): a player leaving a match in progress, which… (`items_audit.rs`): `it_2E5A_Logic115_EvtUnk` |
| `melee/it/kinds/itgreatfoxlaser` | 1 | 32 | an item's evt_unk, which only a fighter's unload runs (it_8026B7F8, game::it::evt_unk dispatches each): a player leaving a match in progress, which… (`items_audit.rs`): `it_802EB5A8` |
| `melee/it/kinds/ittincle` | 1 | 32 | an item's evt_unk, which only a fighter's unload runs (it_8026B7F8, game::it::evt_unk dispatches each): a player leaving a match in progress, which… (`items_audit.rs`): `it_802EC850` |
| `melee/it/kinds/itkyasarin` | 1 | 32 | an item's evt_unk, which only a fighter's unload runs (it_8026B7F8, game::it::evt_unk dispatches each): a player leaving a match in progress, which… (`items_audit.rs`): `it_802EE1E0` |
| `melee/it/kinds/itkyasarinegg` | 1 | 32 | an item's evt_unk, which only a fighter's unload runs (it_8026B7F8, game::it::evt_unk dispatches each): a player leaving a match in progress, which… (`items_audit.rs`): `it_802F0320` |
| `melee/it/kinds/it_2F28` | 1 | 32 | an item's evt_unk, which only a fighter's unload runs (it_8026B7F8, game::it::evt_unk dispatches each): a player leaving a match in progress, which… (`items_audit.rs`): `it_802F2BDC` |
| `melee/it/kinds/itkirbyyoshispecialn` | 5 | 412 | ported, held for a check (`items_audit.rs`'s UNCHECKED): `it_802F2D98`; ported in `it/kinds/yoshitongue.rs` (dispatched by address); no check reaches it yet: `itKirbyyoshispecialn_UnkMotion0_Anim`, `itKirbyyoshispecialn_UnkMotion0_Phys`, `itKirbyyoshispecialn_UnkMotion0_Coll`; ported in `it/kinds/yoshitongue.rs` (held by a `// Ports` comment); no check reaches it yet: `it_802F2E7C` |
| `melee/it/kinds/ityoshiegglay` | 1 | 32 | ported in `it/kinds/yoshiegglay.rs` (held by a `// Ports` comment); no check reaches it yet: `it_27CF_Logic114_EvtUnk` |

### The HUD (`if*`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/if/ifmagnify` | 1 | 4 | ported in `hud/magnify.rs` (its doc names it); no check reaches it yet: `ifMagnify_802FC3BC` |
| `melee/if/if_2FF2` | 1 | 4 | nothing known reaches it: `fn_802FF360` |

## Libraries

**Not classified by reach yet.** The `CALLS` sweep left the SDK's and HSD's functions out: the render state functions run thousands of times a frame, which slowed ssbm-rs's runs tenfold, and the run over the rest was cut short by the wrap-up. What's below comes from the code's comments only. Most are melee-render's presentation ports (the GX getters, the matrix writers, HSD's erase, cull, line, point and light setups), which want a frames check with a mutation each, as the render state setters had; the rest are melee-sim's inline copies (the device command queue's callbacks, the allocator's `OSCreateHeap` and `OSInitAlloc`, the GObj library's defaults, `robj`'s constraints, `texp`'s frees), which want the `CALLS` sweep over a session and a replay. The console's reset (`OSResetSystem` and the reboot under it, `Reset`, `__OSDoHotReset`) is reached only by the reset button or the disc error screen, which no session input raises.

### The SDK and runtime (`dolphin`, MSL)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `MSL/printf` | 1 | 736 | named in `melee-sim/src/msl.rs` (perhaps ported inline); no check reaches it yet: `longlong2str` |
| `dolphin/mcc/mcc` | 1 | 8 | nothing known reaches it: `MCCGetLastError` |
| `dolphin/thp/THPDec` | 1 | 216 | nothing known reaches it: `THPDec_803302EC` |
| `dolphin/gx/GXInit` | 2 | 160 | ported in `melee-render/src/gx/init.rs` (its doc names it); no check reaches it yet: `__GXDefaultTexRegionCallback`, `__GXDefaultTlutRegionCallback` |
| `dolphin/gx/GXTexture` | 4 | 240 | ported in `melee-render/src/gx/texture.rs` (its doc names it); no check reaches it yet: `__GetImageTileCount`, `GXGetTexObjWidth`, `GXGetTexObjHeight`, `GXGetTexObjFmt` |
| `dolphin/gx/GXTev` | 1 | 116 | ported in `melee-render/src/gx/tev.rs` (its doc names it); no check reaches it yet: `GXSetTevColorS10` |
| `dolphin/gx/GXPixel` | 1 | 456 | ported in `melee-render/src/gx/pixel.rs` (its doc names it); no check reaches it yet: `GXInitFogAdjTable` |
| `dolphin/gx/GXStubs` | 1 | 4 | ported in `melee-render/src/gx/transform.rs` (its doc names it); no check reaches it yet: `__GXSetRange` |
| `dolphin/gx/GXTransform` | 4 | 216 | ported in `melee-render/src/gx/transform.rs` (its doc names it); no check reaches it yet: `WriteMTXPS4x3`, `WriteMTXPS4x2`, `GXGetViewportv`; named in `melee-render/src/gx/transform.rs` (perhaps ported inline); no check reaches it yet: `WriteMTXPS3x3from3x4` |
| `dolphin/os/OSAlloc` | 5 | 428 | named in `melee-sim/src/audio/heap.rs` (perhaps ported inline); no check reaches it yet: `DLInsert`; nothing known reaches it: `OSSetCurrentHeap`, `OSDestroyHeap`; ported in `melee-sim/src/hsd/heap.rs` (its doc names it); no check reaches it yet: `OSInitAlloc`; named in `melee-sim/src/hsd/heap.rs` (perhaps ported inline); no check reaches it yet: `OSCreateHeap` |
| `dolphin/os/OSReboot` | 3 | 536 | named in `session.rs` (perhaps ported inline); no check reaches it yet: `Run`; nothing known reaches it: `Callback`, `__OSReboot` |
| `dolphin/os/OSReset` | 3 | 800 | named in `melee-sim/src/lb/command.rs` (perhaps ported inline); no check reaches it yet: `Reset`; nothing known reaches it: `__OSDoHotReset`, `OSResetSystem` |
| `dolphin/ax/AXAlloc` | 3 | 68 | nothing known reaches it: `__AXGetStackHead`; ported in `melee-sim/src/audio/ax.rs` (its doc names it); no check reaches it yet: `__AXPushCallbackStack`, `__AXPopCallbackStack` |
| `dolphin/ax/AXCL` | 2 | 68 | nothing known reaches it: `__AXGetCommandListCycles`; named in `melee-sim/src/audio/mod.rs` (perhaps ported inline); no check reaches it yet: `__AXGetCommandListAddress` |
| `dolphin/ax/AXOut` | 1 | 8 | nothing known reaches it: `AXRegisterCallback` |
| `dolphin/ax/AXVPB` | 2 | 20 | nothing known reaches it: `__AXGetNumVoices`, `__AXGetPBs` |

### HSD's library (`sysdolphin`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `sysdolphin/baselib/tobj` | 3 | 308 | named in `melee-sim/src/hsd/shadow.rs` (perhaps ported inline); no check reaches it yet: `HSD_ImageDescAlloc`, `HSD_ImageDescFree`; ported in `gm/results_fighters.rs` (its doc names it); no check reaches it yet: `HSD_ImageDescCopyFromEFB` |
| `sysdolphin/baselib/state` | 7 | 512 | ported in `melee-render/src/hsd/quad.rs` (its doc names it); no check reaches it yet: `HSD_SetupRenderMode`; ported in `melee-render/src/hsd/state.rs` (its doc names it); no check reaches it yet: `HSD_SetMaterialColor`, `HSD_SetMaterialShininess`, `HSD_StateSetLineWidth`, `HSD_StateSetCullMode`, `HSD_StateSetPointSize`; named in `melee-render/src/hsd/state.rs` (perhaps ported inline); no check reaches it yet: `_HSD_StateInvalidateVtxAttr` |
| `sysdolphin/baselib/aobj` | 4 | 844 | nothing known reaches it: `callbackForeachFunc`, `TObjForeachAnim`, `RObjForeachAnim`, `DObjForeachAnim` |
| `sysdolphin/baselib/lobj` | 3 | 1,124 | ported in `melee-sim/src/hsd/display.rs` (its doc names it); no check reaches it yet: `setup_spec_lightobj`; ported in `melee-render/src/hsd/lobj.rs` (its doc names it); no check reaches it yet: `setup_point_lightobj`, `setup_spot_lightobj` |
| `sysdolphin/baselib/cobj` | 4 | 2,296 | ported in `melee-render/src/hsd/camera.rs` (its doc names it); no check reaches it yet: `HSD_CObjEraseScreen`; nothing known reaches it: `setupNormalCamera`, `setupTopHalfCamera`, `setupBottomHalfCamera` |
| `sysdolphin/baselib/pobj` | 11 | 4,928 | named in `melee-sim/src/hsd/pobj.rs` (perhaps ported inline); no check reaches it yet: `loadEnvelopeDesc`, `loadShapeSetDesc`, `resolveEnvelope`, `get_shape_normal_xyz`, `get_shape_nbt_xyz`; ported in `melee-render/src/hsd/draw.rs` (its doc names it); no check reaches it yet: `HSD_ClearVtxDesc`, `SetupSharedVtxModelMtx`; nothing known reaches it: `HSD_PObjClearMtxMark`, `HSD_PObjGetMtxMark`, `PObjSetupMtx`; named in `melee-render/src/hsd/draw.rs` (perhaps ported inline); no check reaches it yet: `HSD_PObjSetMtxMark` |
| `sysdolphin/baselib/jobj` | 1 | 68 | named in `melee-sim/src/hsd/jobj.rs` (perhaps ported inline); no check reaches it yet: `HSD_JObjAddDObj` |
| `sysdolphin/baselib/displayfunc` | 12 | 4,376 | nothing known reaches it: `mkVBillBoardMtx`, `mkHBillBoardMtx`, `mkBillBoardMtx`, `zlist_sort`, `_HSD_ZListSort`, `_HSD_ZListDisp`, `_HSD_ZListClear`, `HSD_JObjSetSPtclCallback`; ported in `melee-render/src/hsd/draw.rs` (its doc names it); no check reaches it yet: `HSD_JObjMakePositionMtx`; named in `melee-render/src/hsd/draw.rs` (perhaps ported inline); no check reaches it yet: `_HSD_mkEnvelopeModelNodeMtx`; named in `melee-render/src/hsd/camera.rs` (perhaps ported inline); no check reaches it yet: `HSD_SetEraseColor`; ported in `melee-render/src/hsd/camera.rs` (its doc names it); no check reaches it yet: `HSD_EraseRect` |
| `sysdolphin/baselib/initialize` | 2 | 20 | nothing known reaches it: `HSD_GetCurrentRenderPass`, `HSD_Init_803755A8` |
| `sysdolphin/baselib/util` | 3 | 460 | ported in `melee-render/src/hsd/state.rs` (its doc names it); no check reaches it yet: `HSD_MulColor`; nothing known reaches it: `HSD_GetNbBits`, `HSD_Index2PosNrmMtx` |
| `sysdolphin/baselib/robj` | 9 | 4,700 | named in `melee-sim/src/hsd/robj.rs` (perhaps ported inline); no check reaches it yet: `RObjUpdateFunc`, `set_dirup_matrix`, `resolveCnsDirUp`, `resolveCnsOrientation`, `resolveLimits`, `expEvaluate`, `dummy_func`, `bcexpLoadDesc`; ported in `melee-sim/src/hsd/robj.rs` (its doc names it); no check reaches it yet: `expLoadDesc` |
| `sysdolphin/baselib/shadow` | 3 | 708 | ported in `melee-render/src/hsd/camera.rs` (its doc names it); no check reaches it yet: `HSD_ShadowInit`; named in `melee-render/src/hsd/camera.rs` (perhaps ported inline); no check reaches it yet: `drawBackgroundRect`; named in `melee-sim/src/hsd/shadow.rs` (perhaps ported inline); no check reaches it yet: `makeMatrix` |
| `sysdolphin/baselib/texp` | 3 | 1,216 | named in `melee-sim/src/hsd/export.rs` (perhaps ported inline); no check reaches it yet: `HSD_TExpFreeList`; ported in `melee-sim/src/hsd/texp/mod.rs` (its doc names it); no check reaches it yet: `AssignColorReg`; named in `melee-sim/src/hsd/texp_heap.rs` (perhaps ported inline); no check reaches it yet: `HSD_TExpFreeTevDesc` |
| `sysdolphin/baselib/texpdag` | 1 | 584 | named in `melee-sim/src/hsd/texp/dag.rs` (perhaps ported inline); no check reaches it yet: `order_dag` |
| `sysdolphin/baselib/devcom` | 7 | 1,532 | nothing known reaches it: `HSD_DevComIsBusy`; ported in `melee-sim/src/audio/devcom.rs` (its doc names it); no check reaches it yet: `HSD_DevComStdCallback`; named in `melee-sim/src/audio/devcom.rs` (perhaps ported inline); no check reaches it yet: `HSD_DevComARAMCallback`, `HSD_DevComDVDStdCallback`, `HSD_DevComDVDARAMEndCallback`, `HSD_DevComDVDMemCallback`, `HSD_DevComDVDCallback` |
| `sysdolphin/baselib/hsd_3910` | 2 | 128 | nothing known reaches it: `HSD_GObj_80391260`, `HSD_GObj_803912A8` |
| `sysdolphin/baselib/gobjinit` | 1 | 36 | nothing known reaches it: `HSD_GObjSetInitDefaults` |
| `sysdolphin/baselib/psdisp` | 1 | 252 | named in `melee-render/src/hsd/psdisp.rs` (perhaps ported inline); no check reaches it yet: `getColorMatAmb` |
| `sysdolphin/baselib/psdisptev` | 1 | 12 | nothing known reaches it: `psSetupTevInvalidState` |
| `sysdolphin/baselib/sobjlib` | 1 | 48 | named in `gm/machine.rs` (perhaps ported inline); no check reaches it yet: `HSD_SObjLib_803A44A4` |
| `sysdolphin/baselib/hsd_3A76` | 1 | 32 | named in `hud/text.rs` (perhaps ported inline); no check reaches it yet: `HSD_SisLib_803A945C` |

## Classic

### Classic's mode and intro (`gmclassic`, `gm_1832`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/gm/gm_1832` | 14 | 3,332 | the single-player modes', which only the 1P list's gated entries lead to (`menus.txt`): `fn_80184A04`, `fn_80184A28`, `fn_80184A70`, `fn_80184A94`, `fn_801851C0`, `fn_801852FC`, `fn_80185408`, `fn_801855BC`, `fn_8018564C`, `fn_8018569C`, `fn_801857C4`, `fn_801859C8`, `fn_80185A0C`; ported in `gm/intro.rs` (a running agent's file); no check reaches it yet: `fn_80186080` |
| `melee/gm/gmclassic` | 4 | 1,232 | ported in `gm/one_player.rs` (a running agent's file); no check reaches it yet: `gmClassic_801B2BA4`, `gmClassic_801B2D54`, `gmClassic_801B3E44`; the single-player modes', which only the 1P list's gated entries lead to (`menus.txt`): `gmClassic_801B3F18` |

## Adventure

### Adventure's mode and intro (`gmadventure`, `gm_17E4`, `gm_1879`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/gm/gm_17E4` | 1 | 168 | the single-player modes', which only the 1P list's gated entries lead to (`menus.txt`): `gm_8017E7FC` |
| `melee/gm/gmadventure` | 12 | 1,376 | ported in `gm/one_player_match.rs` (a running agent's file); no check reaches it yet: `gm_801B4254`, `gm_801B4294`; ported in `gm/one_player.rs` (a running agent's file); no check reaches it yet: `gm_801B4350`; the single-player modes', which only the 1P list's gated entries lead to (`menus.txt`): `gm_801B4408`, `gm_801B4E58`, `gm_801B4EB8`, `gm_801B4F44`, `gm_801B4FCC`, `gm_801B5078`, `gm_801B50C4`, `gm_801B518C`; ported in `gm/adventure.rs` (a running agent's file); no check reaches it yet: `gm_801B45A4` |

## All-Star

### All-Star's mode and intro (`gmallstar`, `gm_17EB`, `gm_186E`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/gm/gm_17EB` | 1 | 100 | All-Star's data (gm_GetAllStarData and its getters), which fn_8016C7F0 reads only in the single-player branch its port refuses: the single-player… (`menus.txt`): `fn_8017EDDC` |
| `melee/gm/gmallstar` | 1 | 40 | the single-player modes', which only the 1P list's gated entries lead to (`menus.txt`): `gm_801B607C` |

## Event Match

### Event Match (`gmevent`, `mnevent`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/gm/gmevent` | 2 | 240 | reached by rec-2026-10-06_16-58-24 (`gm/event.rs`): ported inline, a running agent's file: `gm_801BC670`; the single-player modes', which only the 1P list's gated entries lead to (`menus.txt`): `gm_801BEB68` |

## 1P shared

### The single-player matches' setups, records and clear screens (`gm_16A2`, `gm_1601`, `gm_17AD`, `gm_17C0`, `gm_17BA`, `gm_18A1`, `gm_16F1`, `gm_1A3F`, `gmregclear`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/gm/gm_1601` | 4 | 408 | named only by the single-player modes' code (fn_80184AB8): the single-player modes', which only the 1P list's gated entries lead to (`menus.txt`): `fn_80160F58`; ported in `gm/one_player_match.rs` (a running agent's file); no check reaches it yet: `fn_80162DF8`; ported in `gm/tournament/bracket.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_801642A0`; a getter or setter of the game's data, which its callers' ports read or write inline (`menus.txt`): `fn_80166CBC` |
| `melee/gm/gm_16A2` | 7 | 1,368 | ported in `gm/opponents.rs` (a running agent's file); no check reaches it yet: `gm_80169384`, `fn_80169444`, `fn_8016A450`, `fn_8016A46C`, `fn_8016A488`; named only by the single-player modes' code (gm_801B4974, gm_801B4B28): the single-player modes', which only the 1P list's gated entries lead to (`menus.txt`): `gm_80169540`; reached by items-pokeball1-fd: ported inline, no item of its own to carry the alias: `fn_8016A4C8` |
| `melee/gm/gm_16F1` | 1 | 56 | ported in `gm/multi_man.rs` (its doc names it); no check reaches it yet: `gm_80173498` |
| `melee/gm/gm_17AD` | 18 | 2,124 | ported in `gm/results_stats.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_8017ADA8`, `fn_8017B2E4`, `fn_8017B348`, `fn_8017B3AC`, `fn_8017B410`, `fn_8017B4D0`, `fn_8017B534`, `fn_8017B598`, `fn_8017B5FC`, `fn_8017B660`, `fn_8017B6C4`, `fn_8017B728`, `fn_8017B78C`, `fn_8017B7F0`, `fn_8017B854`, `fn_8017B8B8`, `fn_8017B91C`, `fn_8017B9F4` |
| `melee/gm/gm_17BA` | 2 | 200 | ported in `gm/results_panel.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_8017BACC`, `fn_8017BB30` |
| `melee/gm/gm_17C0` | 6 | 2,392 | the single-player modes', which only the 1P list's gated entries lead to (`menus.txt`): `fn_8017C0C8`, `fn_8017C1A4`, `fn_8017C71C`, `fn_8017C7A0`, `fn_8017C7EC`, `gm_8017CBAC` |
| `melee/gm/gm_18A1` | 2 | 160 | the single-player modes', which only the 1P list's gated entries lead to (`menus.txt`): `gm_8018A188`, `gm_8018A1D8` |
| `melee/gm/gm_1A3F` | 2 | 24 | reached by rec-2026-10-06_16-58-24, res-stock: ported inline, no item of its own to carry the alias: `gm_GetGameModeStateExitData`; named only by the single-player modes' code (gm_801B461C): the single-player modes', which only the 1P list's gated entries lead to (`menus.txt`): `gm_GetPreviousSceneIndex` |

## 1P endings

### Game overs, endings, trophy fall, staff roll (`gm_19EF`, `gmgover`, `gmmovieend`, `gmstaffroll`, `gmregtyfall`, `gm_1A7A`, `gm_1A9B`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/gm/gm_19EF` | 3 | 724 | ported in `gm/endings/game_over.rs` (held by a `// Ports` comment); no check reaches it yet: `fn_801A0B60`, `gm_Scene_ComingSoon_OnExit`; ported in `gm/endings/game_over.rs` (its doc names it); no check reaches it yet: `gm_Scene_ComingSoon_OnEnter` |

## Bosses

### Master Hand (`ftmasterhand*`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/ft/kinds/ftMasterHand/ftmasterhandwait10` | 4 | 1,636 | ported in `ft/masterhand/wait.rs` (held by a `// Ports` comment); no check reaches it yet: `ftMh_MS_341_8014FE5C`, `ftMh_MS_341_8014FF1C`, `ifStage251`, `ftMh_MS_341_80150894` |
| `melee/ft/kinds/ftMasterHand/ftmasterhandwait12` | 1 | 156 | ported in `ft/masterhand/wait.rs` (held by a `// Ports` comment); no check reaches it yet: `ftMh_MS_389_80150D28` |
| `melee/ft/kinds/ftMasterHand/ftmasterhanddamage0` | 3 | 812 | ported in `ft/masterhand/damage.rs` (held by a `// Ports` comment); no check reaches it yet: `ftMh_MS_343_80151484`, `ftMh_Damage_Anim`, `ftMh_MS_345_Anim` |
| `melee/ft/kinds/ftMasterHand/ftmasterhandsweep` | 3 | 120 | ported in `ft/masterhand/common.rs` (held by a `// Ports` comment); no check reaches it yet: `ftMh_Damage_IASA`, `ftMh_Damage_Coll`; ported in `ft/masterhand/sweep.rs` (held by a `// Ports` comment); no check reaches it yet: `ftMh_Damage_Phys` |
| `melee/ft/kinds/ftMasterHand/ftmasterhandbackdisappear` | 5 | 496 | ported in `ft/masterhand/grab.rs` (held by a `// Ports` comment); no check reaches it yet: `ftMh_MS_381_8015483C`, `ftMh_Cancel_Anim`, `ftMh_Cancel_Phys`, `ftMh_Cancel_Coll`; ported in `ft/masterhand/common.rs` (held by a `// Ports` comment); no check reaches it yet: `ftMh_Cancel_IASA` |
| `melee/ft/kinds/ftMasterHand/ftmasterhandcapturedamagemasterhand` | 1 | 136 | ported in `ft/masterhand/grab.rs` (held by a `// Ports` comment); no check reaches it yet: `ftMh_CaptureDamageMasterHand_80155C94` |
| `melee/ft/kinds/ftMasterHand/ftmasterhandthrownmasterhand` | 9 | 96 | ported in `ft/masterhand/grab.rs` (held by a `// Ports` comment); no check reaches it yet: `ftMh_CaptureWaitMasterHand_80155D1C`; ported in `ft/masterhand/common.rs` (held by a `// Ports` comment); no check reaches it yet: `ftMh_CaptureWaitMasterHand_Anim`, `ftMh_CaptureWaitMasterHand_IASA`, `ftMh_CaptureWaitMasterHand_Phys`, `ftMh_CaptureWaitMasterHand_Coll`, `ftMh_ThrownMasterHand_Anim`, `ftMh_ThrownMasterHand_IASA`, `ftMh_ThrownMasterHand_Phys`, `ftMh_ThrownMasterHand_Coll` |

### Crazy Hand (`ftcrazyhand*`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/ft/kinds/ftCrazyHand/ftcrazyhandwait10` | 3 | 1,608 | ported in `ft/crazyhand/wait.rs` (held by a `// Ports` comment); no check reaches it yet: `ftCh_Init_801560D8`, `ftCh_Init_80156310`, `ftCh_Init_80156AD8` |
| `melee/ft/kinds/ftCrazyHand/ftcrazyhanddamage0` | 1 | 96 | ported in `ft/crazyhand/attack.rs` (dispatched by address); no check reaches it yet: `ftCh_Damage2_Phys` |
| `melee/ft/kinds/ftCrazyHand/ftcrazyhandfingerbeam` | 5 | 496 | ported in `ft/crazyhand/grab.rs` (held by a `// Ports` comment); no check reaches it yet: `ftCh_Init_80159098`, `ftCh_FingerGun1_Anim`; ported in `ft/crazyhand/common.rs` (held by a `// Ports` comment); no check reaches it yet: `ftCh_FingerGun1_IASA`, `ftCh_FingerGun1_Phys`, `ftCh_FingerGun1_Coll` |
| `melee/ft/kinds/ftCrazyHand/ftcrazyhandtaggrab` | 6 | 932 | ported in `ft/crazyhand/tag.rs` (held by a `// Ports` comment); no check reaches it yet: `ftCh_GrabUnk1_8015ADD0`, `ftCh_TagGrab_Anim`, `ftCh_TagSqueeze_Anim`, `ftCh_TagGrab_Phys`; ported in `ft/crazyhand/common.rs` (held by a `// Ports` comment); no check reaches it yet: `ftCh_TagGrab_IASA`, `ftCh_TagGrab_Coll` |
| `melee/ft/kinds/ftCrazyHand/ftcrazyhandcapturewaitcrazyhand` | 6 | 216 | ported in `ft/crazyhand/grab.rs` (held by a `// Ports` comment); no check reaches it yet: `ftCh_GrabUnk1_8015B778`, `ftCh_GrabUnk1_8015B800`; ported in `ft/crazyhand/common.rs` (held by a `// Ports` comment); no check reaches it yet: `ftCo_CaptureWaitCrazyHand_Anim`, `ftCo_CaptureWaitCrazyHand_IASA`, `ftCo_CaptureWaitCrazyHand_Phys`, `ftCo_CaptureWaitCrazyHand_Coll` |
| `melee/ft/kinds/ftCrazyHand/ftcrazyhandthrowncrazyhand` | 4 | 16 | ported in `ft/crazyhand/common.rs` (held by a `// Ports` comment); no check reaches it yet: `ftCo_ThrownCrazyHand_Anim`, `ftCo_ThrownCrazyHand_IASA`, `ftCo_ThrownCrazyHand_Phys`, `ftCo_ThrownCrazyHand_Coll` |
| `melee/ft/kinds/ftCrazyHand/ftcrazyhandtagcancel` | 1 | 156 | ported in `ft/crazyhand/wait.rs` (held by a `// Ports` comment); no check reaches it yet: `ftCh_GrabUnk1_8015B998` |

### The hands' shared code (`ftbosslib`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/ft/ftbosslib` | 10 | 1,092 | ported in `ft/bosslib.rs` (a running agent's file); no check reaches it yet: `ftBossLib_8015BD20`, `ftBossLib_IsMasterHandEntry`, `ftBossLib_8015C2A8`, `ftBossLib_8015C530`, `ftBossLib_8015C88C`, `ftBossLib_8015C9CC`, `ftBossLib_8015CA6C`, `ftBossLib_8015CB7C`, `ftBossLib_8015CB9C`, `ftBossLib_8015CC14` |

## 1P stages

### Single-player stages (adventure routes, races, trophy stages, Snag the Trophies)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/gr/grkinokoroute` | 12 | 388 | ported in `gr/kinokoroute.rs` (held by a `// Ports` comment); no check reaches it yet: `grKinokoRoute_8020741C`, `grKinokoRoute_80207544`, `grKinokoRoute_802078E8`, `grKinokoRoute_80207A94`, `grKinokoRoute_80207ACC`, `grKinokoRoute_80207AD8`, `grKinokoRoute_80207B20`, `grKinokoRoute_80207B2C`, `grKinokoRoute_80207C80`, `grKinokoRoute_80208368`, `grKinokoRoute_8020836C`, `grKinokoRoute_80208480` |
| `melee/gr/grshrineroute` | 4 | 324 | ported in `gr/shrineroute.rs` (held by a `// Ports` comment); no check reaches it yet: `grShrineRoute_8020A100`, `grShrineRoute_8020A8A0`, `grShrineRoute_8020AF38`, `grShrineRoute_OnCheckShadowRender` |
| `melee/gr/grzebesroute` | 5 | 24 | ported in `gr/zebesroute.rs` (held by a `// Ports` comment); no check reaches it yet: `grZebesRoute_8020B160`, `grZebesRoute_8020B380`, `grZebesRoute_8020B3BC`, `grZebesRoute_8020B4D4`, `grZebesRoute_8020B85C` |
| `melee/gr/grbigblueroute` | 9 | 60 | ported in `gr/bigblueroute.rs` (held by a `// Ports` comment); no check reaches it yet: `grBigBlueRoute_8020B9CC`, `grBigBlueRoute_8020BB00`, `grBigBlueRoute_8020BB0C`, `grBigBlueRoute_8020BB58`, `grBigBlueRoute_8020BB64`, `grBigBlueRoute_8020BC24`, `grBigBlueRoute_8020BC30`, `grBigBlueRoute_8020BF30`, `grBigBlueRoute_8020C1D4` |
| `melee/gr/grfigure1` | 7 | 36 | ported in `gr/figure.rs` (held by a `// Ports` comment); no check reaches it yet: `grFigure1_OnDemoInit`, `grFigure1_8020E1A0`, `grFigure1_8020E1FC`, `grFigure1_8020E25C`, `grFigure1_OnCheckShadowRender`, `grFigure1_8020E270`; reached by rec-2026-10-06_16-58-24 (`gr/figure.rs`): ported inline, a running agent's file: `grFigure1_OnTouchLine` |
| `melee/gr/grfigure2` | 5 | 28 | ported in `gr/figure.rs` (held by a `// Ports` comment); no check reaches it yet: `grFigure2_8020E434`, `grFigure2_8020E490`, `grFigure2_8020E4F0`, `grFigure2_8020E4F4`, `grFigure2_8020E4FC` |
| `melee/gr/grfigure3` | 6 | 32 | ported in `gr/figure.rs` (held by a `// Ports` comment); no check reaches it yet: `grFigure3_8020E504`, `grFigure3_8020E6C8`, `grFigure3_8020E724`, `grFigure3_8020E784`, `grFigure3_8020E788`, `grFigure3_8020E790` |
| `melee/gr/grpushon` | 11 | 216 | ported in `gr/pushon.rs` (held by a `// Ports` comment); no check reaches it yet: `grPushOn_802182C4`, `grPushOn_802183DC`, `grPushOn_80218590`, `grPushOn_8021859C`, `grPushOn_80218670`, `grPushOn_802187A4`, `grPushOn_80218880`, `grPushOn_80218ED0`, `grPushOn_80219204`, `grPushOn_80219230`, `grPushOn_80219528` |
| `melee/gr/grfigureget` | 19 | 1,896 | nothing known reaches it: `grFigureGet_OnDemoInit`, `grFigureGet_OnInit`, `grFigureGet_OnLoad`, `grFigureGet_OnStart`, `grFigureGet_802195C4`, `grFigureGet_802195CC`, `stageGObj0_OnInit`, `grFigureGet_802196E0`, `grFigureGet_802196E8`, `grFigureGet_802196EC`, `grFigureGet_802196F0`, `grFigureGet_80219890`, `grFigureGet_80219898`, `grFigureGet_80219B0C`, `grFigureGet_80219B10`, `grFigureGet_80219C34`, `grFigureGet_OnTouchLine`, `grFigureGet_OnCheckShadowRender`; named in `it/kinds/trophycoin.rs` (perhaps ported inline); no check reaches it yet: `grFigureGet_80219C50` |

## 1P items

### Single-player items (`itevyoshiegg`, `itmetamon`, `itcoin`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/it/kinds/itevyoshiegg` | 13 | 504 | ported in `it/kinds/evyoshiegg.rs` (held by a `// Ports` comment); no check reaches it yet: `itEvYoshiEgg_Logic42_Destroyed`, `itEvYoshiEgg_Logic42_Dropped`, `itEvyoshiegg_UnkMotion5_Anim`, `itEvyoshiegg_UnkMotion5_Phys`, `itEvyoshiegg_UnkMotion5_Coll`, `it_3F14_Logic42_Clanked`, `it_3F14_Logic42_HitShield`, `it_3F14_Logic42_Reflected`, `itEvYoshiEgg_Logic42_EnteredAir`, `itEvyoshiegg_UnkMotion4_Anim`, `itEvyoshiegg_UnkMotion4_Phys`, `itEvyoshiegg_UnkMotion4_Coll`; an item's evt_unk, which only a fighter's unload runs (it_8026B7F8, game::it::evt_unk dispatches each): a player leaving a match in progress, which… (`items_audit.rs`): `itEvYoshiEgg_Logic42_EvtUnk` |
| `melee/it/kinds/itmetamon` | 5 | 136 | ported in `it/kinds/metamon.rs` (its doc names it); no check reaches it yet: `it_802D306C`, `itMetamon_UnkMotion1_Anim`, `itMetamon_UnkMotion1_Phys`, `itMetamon_UnkMotion1_Coll`; an item's evt_unk, which only a fighter's unload runs (it_8026B7F8, game::it::evt_unk dispatches each): a player leaving a match in progress, which… (`items_audit.rs`): `itMetamon_Logic19_EvtUnk` |

### Adventure enemies and the hands' items (`itlikelike`, `itnokonoko`, `itpatapata`, shells, `itwhitebea`, `itmasterhand*`, `itcrazyhandbomb`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/it/kinds/itlikelike` | 6 | 784 | ported in `it/kinds/likelike.rs` (held by a `// Ports` comment); no check reaches it yet: `it_802D9B78`, `it_802DB398`, `itLikelike_UnkMotion12_Anim`, `itLikelike_UnkMotion12_Phys`, `itLikelike_UnkMotion12_Coll`, `it_802DBA68` |
| `melee/it/kinds/itnokonoko` | 14 | 952 | ported in `it/kinds/nokonoko.rs` (held by a `// Ports` comment); no check reaches it yet: `itNokonoko_UnkMotion1_Coll`, `itNokonoko_UnkMotion6_Anim`, `itNokonoko_UnkMotion6_Phys`, `it_802DD2DC`, `it_802DD4F4`, `itNokonoko_UnkMotion9_Anim`, `itNokonoko_UnkMotion9_Phys`, `it_802DD59C`, `it_802DD67C`, `itNokonoko_UnkMotion10_Anim`, `itNokonoko_UnkMotion10_Phys`, `itNokonoko_UnkMotion10_Coll`, `it_802DD78C`, `it_802DD7D0` |
| `melee/it/kinds/itzgshell` | 16 | 1,916 | ported in `it/kinds/zgshell.rs` (held by a `// Ports` comment); no check reaches it yet: `itZGShell_Logic11_Dropped`, `itZrshell_UnkMotion4_Anim`, `itZrshell_UnkMotion4_Phys`, `itZrshell_UnkMotion4_Coll`, `it_2725_Logic11_EnteredAir`, `itZrshell_UnkMotion9_Anim`, `itZrshell_UnkMotion9_Phys`, `itZrshell_UnkMotion9_Coll`, `it_802DF9F8`, `itZrshell_UnkMotion10_Anim`, `itZrshell_UnkMotion10_Phys`, `itZrshell_UnkMotion10_Coll`, `itZGShell_Logic11_Reflected`, `it_2725_Logic11_HitShield`, `itZGShell_Logic11_ShieldBounced`, `it_802DFED4` |
| `melee/it/kinds/itzrshell` | 9 | 536 | ported in `it/kinds/zgshell.rs` (held by a `// Ports` comment); no check reaches it yet: `itZRShell_Logic12_Dropped`, `itZRShell_Logic12_EnteredAir`, `itZRShell_Logic12_Destroyed`, `itZRShell_Logic12_Reflected`, `itZRShell_Logic12_Clanked`, `itZRShell_Logic12_HitShield`, `itZRShell_Logic12_ShieldBounced`, `it_802E0468`; ported in `it/kinds/zrshell.rs` (held by a `// Ports` comment); no check reaches it yet: `it_802E0488` |
| `melee/it/kinds/itpatapata` | 13 | 724 | ported in `it/kinds/patapata.rs` (held by a `// Ports` comment); no check reaches it yet: `itPatapata_Logic4_PickedUp`, `itPatapata_UnkMotion5_Anim`, `itPatapata_UnkMotion5_Phys`, `it_2725_Logic4_Dropped`, `itPatapata_UnkMotion7_Anim`, `itPatapata_UnkMotion7_Phys`, `itPatapata_UnkMotion7_Coll`, `it_2725_Logic4_Thrown`, `itPatapata_UnkMotion6_Anim`, `itPatapata_UnkMotion6_Phys`, `itPatapata_UnkMotion6_Coll`, `it_802E1694`, `it_802E16D8` |
| `melee/it/kinds/itwhitebea` | 13 | 748 | ported in `it/kinds/whitebea.rs` (held by a `// Ports` comment); no check reaches it yet: `itWhiteBea_Logic9_PickedUp`, `itWhitebea_UnkMotion8_Anim`, `itWhitebea_UnkMotion8_Phys`, `it_2725_Logic9_Dropped`, `it_2725_Logic9_Thrown`, `itWhitebea_UnkMotion9_Anim`, `itWhitebea_UnkMotion9_Phys`, `itWhitebea_UnkMotion9_Coll`, `it_802E4980`, `itWhitebea_UnkMotion11_Anim`, `itWhitebea_UnkMotion11_Phys`, `itWhitebea_UnkMotion11_Coll`, `it_802E4A24` |
| `melee/it/kinds/itmasterhandlaser` | 1 | 32 | ported in `it/kinds/masterhandlaser.rs` (held by a `// Ports` comment); no check reaches it yet: `itMasterHandLaser_Logic84_EvtUnk` |
| `melee/it/kinds/itmasterhandbullet` | 7 | 416 | ported in `it/kinds/masterhandbullet.rs` (held by a `// Ports` comment); no check reaches it yet: `itMasterHandBullet_Logic85_EvtUnk`, `it_802F0D2C`, `it_802F0F08`, `itMasterHandBullet_Logic83_Clanked`, `itMasterHandBullet_Logic83_Absorbed`, `itMasterHandBullet_Logic83_ShieldBounced`, `itMasterHandBullet_Logic83_HitShield` |
| `melee/it/kinds/itcrazyhandbomb` | 2 | 64 | an item's evt_unk, which only a fighter's unload runs (it_8026B7F8, game::it::evt_unk dispatches each): a player leaving a match in progress, which… (`items_audit.rs`): `itCrazyHandBomb_Logic86_EvtUnk`; ported in `it/kinds/crazyhandbomb.rs` (held by a `// Ports` comment); no check reaches it yet: `itCrazyHandBomb_Logic86_Reflected` |

## 1P computer kinds

### The computer kinds' other decisions (`ftCo_0A01`, `ftcpuattack`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/ft/kinds/ftCommon/ftCo_0A01` | 5 | 3,748 | ported in `ft/cpu_move.rs` (held by a `// Ports` comment); no check reaches it yet: `ftCo_800A0384`; only the decisions of computer kinds VS doesn't use call it (ftCo_800B2AFC's other cases) (`cpu_audit.rs`): `ftCo_800AEA8C`, `ftCo_800AF290`, `ftCo_800B1AB8`; Nana's decision without Popo: she sleeps whenever he does (ftCo_800BFD9C takes her out with him, ftCo_800D4F24) and wakes with him (Player_80032070),… (`cpu_audit.rs`): `ftCo_800AEFB8` |
| `melee/ft/ftcpuattack` | 1 | 224 | ftCo_800BB220 counts at most 14 frames above level 3 (5 + 10·randf at levels 3 to 5, fewer above), so Ness above level 3 never spans 21; the threat… (`cpu_audit.rs`): `ftCo_800BA080` |

## Debug stages

### The Test stage (`grtest`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/gr/grtest` | 21 | 1,520 | nothing known reaches it: `grTest_80206E2C`, `grTest_80206E30`, `grTest_UnkStage0_OnLoad`, `grTest_UnkStage0_OnStart`, `grTest_8020703C`, `grTest_80207044`, `stageGObj0_OnInit`, `grTest_8020715C`, `grTest_80207164`, `grTest_80207168`, `stageGObj2_OnInit`, `grTest_802071BC`, `grTest_802071C4`, `grTest_802073AC`, `stageGObj2_X8Callback`, `stageGObj1_OnInit`, `grTest_802073FC`, `grTest_80207404`, `grTest_80207408`, `grTest_8020740C`, `grTest_80207414` |

## Debug only

### Debug code (`ledger/deferred.txt`)

| Unit | Functions | Bytes | What reaches them |
| --- | ---: | ---: | --- |
| `melee/ft/kinds/ftZelda/ftzelda` | 1 | 92 | debug only (`ledger/deferred.txt`): `ftZd_Init_801395C8` |
| `melee/gm/gm_1601` | 1 | 176 | debug only (`ledger/deferred.txt`): `gm_80164A0C` |
| `melee/gm/gmdebugmode` | 1 | 56 | debug only (`ledger/deferred.txt`): `onEnterIntroEasy` |
| `melee/gm/gmhanyucss` | 2 | 284 | debug only (`ledger/deferred.txt`): `gm_801BED3C`, `gm_801BEDA8` |
| `melee/db/dbitem` | 2 | 200 | debug only (`ledger/deferred.txt`): `db_DisableItemSpawns`, `fn_80225E6C` |
| `melee/db/dbcamera` | 1 | 304 | debug only (`ledger/deferred.txt`): `fn_80227EB0` |
| `melee/if/if_3004` | 9 | 880 | debug only (`ledger/deferred.txt`): `un_80300D78`, `un_80300E74`, `un_80300F3C`, `un_80301028`, `un_80301074`, `un_803010C0`, `un_80301B48`, `un_80301BA8`, `un_80301D7C` |
| `dolphin/hio/hio` | 14 | 3,296 | debug only (`ledger/deferred.txt`): `ExtHandler`, `ExiHandler`, `DbgHandler`, `TxHandler`, `RxHandler`, `HIOEnumDevices`, `HIOInit`, `HIOReadMailbox`, `HIOWriteMailbox`, `HIORead`, `HIOWrite`, `HIOReadAsync`, `HIOWriteAsync`, `HIOReadStatus` |
| `dolphin/mcc/mcc` | 29 | 9,008 | debug only (`ledger/deferred.txt`): `callbackEventStream`, `MCCStreamOpen`, `MCCStreamClose`, `MCCStreamWrite`, `LoadChannelInfo`, `FlushChannelInfo`, `SetChannelInfoDirty`, `MakeMemoryMap`, `SearchFreeBlocks`, `NotifyCompulsorily`, `WaitAMinute`, `MailboxCheck`, `MCCExiCallback`, `MCCTxCallback`, `MCCRxCallback`, `mccInitializeCheck`, `MCCInit`, `MCCExit`, `MCCEnumDevices`, `MCCGetFreeBlocks`, `MCCGetChannelInfo`, `MCCGetConnectionStatus`, `MCCNotify`, `MCCSetChannelEventMask`, `MCCOpen`, `MCCClose`, `MCCRead`, `MCCWrite`, `MCCCheckAsyncDone` |
| `dolphin/mcc/fio` | 13 | 2,980 | debug only (`ledger/deferred.txt`): `fioMccChannelEvent`, `FIOInit`, `FIOExit`, `FIOQuery`, `FIOFopen`, `FIOFclose`, `FIOFwrite`, `fioPacketMakeHeader`, `fioPacketSendPacket`, `fioPacketReceiveResult`, `fioPacketReceiveDone`, `fioPacketWrite`, `fioPacketResultWrite` |
| `dolphin/os/OSMutex` | 3 | 428 | debug only (`ledger/deferred.txt`): `__OSCheckMutex`, `__OSCheckDeadLock`, `__OSCheckMutexes` |
| `dolphin/os/OSThread` | 2 | 2,028 | debug only (`ledger/deferred.txt`): `CheckThreadQueue`, `OSCheckActiveThreads` |
| `sysdolphin/baselib/hsd_392C` | 3 | 396 | debug only (`ledger/deferred.txt`): `fn_80392CCC`, `fn_80392E2C`, `hsd_803931A4` |
| `sysdolphin/baselib/hsd_3933` | 6 | 1,484 | debug only (`ledger/deferred.txt`): `fn_803932D0`, `hsd_80393328`, `hsd_80393440`, `hsd_80393840`, `hsd_80393A04`, `hsd_80393A54` |
