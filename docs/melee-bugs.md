# Melee's own bugs

melee-hd reproduces the original game's bugs exactly: gameplay and presentation must stay identical with ssbm-rs and with Slippi replays, so a defect in Melee's code is ported as the original runs it, never fixed in passing. Where a defect's only effect is a value that provably reaches neither (an address left in a word nothing plays from), the port may use a simpler stand-in, recorded in the entry. This catalog records those defects as porting finds them, so each one is written down once with what it does and what fixing it would take.

It lists only defects in Melee's own code (NTSC 1.02): reads of registers, stack slots or memory the code never set, wrong operands or fields, copy-paste slips, loops that can't end, reads and writes out of bounds or into freed objects, values computed and dropped. Intended mechanics, however surprising, are not here, nor are bugs in ssbm-rs or in melee-hd's own ports, Dolphin's behavior, or what Slippi's and Gecko codes do (except where a Slippi code exists to patch one of these bugs, which the entry notes).

## How to read an entry

Each entry gives:

- **Where:** the original function (the decompilation's names) and the melee-hd port that reproduces it.
- **Defect:** what the code does wrong.
- **Effect:** what it changes in play, or "none observable" when nothing the game reads depends on it.
- **melee-hd:** how the port reproduces it. "Modeled" means the port computes what the original does; "stale" and "unknown until written" are the rules in [the engine notes](engine.md#uninitialized-values), under which a value the game provably never reads is left out of the comparison and a read of it is reported ([the oracle](oracle.md)); "stops" means the port panics or reports an unported case rather than guess.
- **Fix:** what a corrected version would do.
- **Fix impact:** whether a fix changes gameplay, breaks replay and Slippi compatibility (any change to a value the game reads makes old replays and Slippi peers desync), or changes something players rely on.
- **Source:** where the defect is written down, so it can be traced.

A field no source covers says "not recorded". Line numbers are those at the time of writing.

## Adding an entry

When porting shows that the original game does something wrong, keep the behavior, then add an entry here under its area, with every field above and its source (the doc section, or the port's file and line where a comment explains it). A defect no data or code can reach goes in [Latent defects](#latent-defects) instead; one that may be intended, or may be the decompilation's naming rather than the game's code, goes under [Unsure](#unsure) with the reason. Don't fill a field the sources don't support: write "not recorded".

## Fighters

### The invisible ceiling glitch

- **Where:** `Fighter_procUpdate`, `crates/melee-sim/src/game/ft/fighter.rs` (`proc_update`).
- **Defect:** when an airborne fighter's attack and shield pushback (`x98_atk_shield_kb`) decays below the decay step, the code clears the knockback's y (`x8c_kb_vel.y`) instead of the pushback's.
- **Effect:** the fighter's vertical knockback drops to zero the frame its pushback runs out; the port's comment names this the invisible ceiling glitch.
- **melee-hd:** modeled.
- **Fix:** clear `x98_atk_shield_kb.y`.
- **Fix impact:** changes gameplay and breaks replay and Slippi compatibility; players know the glitch.
- **Source:** `crates/melee-sim/src/game/ft/fighter.rs:404`.

### Knocked-down wall bounce off a right wall

- **Where:** `fn_800C7DC4` (the knocked-down fighter's bounce into `ftCo_DownReflect`), `crates/melee-sim/src/game/ft/co/down.rs` (`down_reflect`).
- **Defect:** the fighter is set against the wall (in x, by its TransN) only off a wall to its left (`RIGHT_WALL_HUG`); off a wall to its right the code instead moves it up by its TransN's y.
- **Effect:** a fighter sliding into a wall on its right is placed differently from one sliding into a wall on its left; seen in play on `Game_20260605T220809` (tick 4630).
- **melee-hd:** modeled.
- **Fix:** set the fighter against a left wall in x as for a right one.
- **Fix impact:** changes gameplay (positions after a wall bounce) and breaks replay and Slippi compatibility.
- **Source:** [the oracle](oracle.md) (line 204); `crates/melee-sim/src/game/ft/co/down.rs:272`.

### Ice block tests the right wall twice

- **Where:** `ftCo_DamageIce_Collide` (a frozen fighter's block hitting a surface), `crates/melee-sim/src/game/ft/co/ice.rs`.
- **Defect:** after a bounce the block is set against the surface by its TransN, and the code tests the right wall twice, so a left wall takes the ceiling's branch and moves the block in y.
- **Effect:** not recorded.
- **melee-hd:** modeled.
- **Fix:** test the left wall in the second test.
- **Fix impact:** changes where a frozen fighter's block ends up after hitting a left wall; breaks replay and Slippi compatibility where that happens.
- **Source:** `crates/melee-sim/src/game/ft/co/ice.rs:414`.

### Wall stop takes its side from a caller's register

- **Where:** `ftCo_8009EE30`, from `ftCo_8009EDA4` (stopping against a wall), `crates/melee-sim/src/game/ft/co/ledge.rs` (`enter_stop_wall`).
- **Defect:** it tests whether it hit a right wall on a register its caller left: the right wall hug bit the caller found facing left, or the left one it found facing right.
- **Effect:** the side follows the facing the caller took, not the collision's flags; what that changes in play is not recorded.
- **melee-hd:** modeled (the caller's value passed as `right_wall`).
- **Fix:** read the wall from the collision flags.
- **Fix impact:** not recorded.
- **Source:** `crates/melee-sim/src/game/ft/co/ledge.rs:162`.

### Knockdown impact sound's third tier is unreachable

- **Where:** `ftCo_80097630` (the knockdown's impact sound by speed), `crates/melee-sim/src/game/ft/co/down.rs` (`impact_sfx`).
- **Defect:** the code compares against the second threshold twice, so of the four sound ids it picks from, the third is never chosen.
- **Effect:** audio only: a knockdown never plays that tier's sound.
- **melee-hd:** modeled.
- **Fix:** compare against the third threshold.
- **Fix impact:** the sound plays differently; it also changes which sounds take voices, and sound handles are game state, so replays could differ in audio state.
- **Source:** `crates/melee-sim/src/game/ft/co/down.rs:93`.

### Hit color animation returns the link register

- **Where:** `ftCo_8008DA4C` (the color animation a damaging hit starts), `crates/melee-sim/src/game/ft/co/hit.rs` (`damage_color_anim`).
- **Defect:** when the hit dealt no damage the C leaves the result unset; the machine code returns the link register it saved (its caller's return address), which is never zero.
- **Effect:** the caller always takes a damageless hit's result as true; what follows from that is not recorded.
- **melee-hd:** modeled (returns true).
- **Fix:** return a defined value.
- **Fix impact:** not recorded.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values) (line 77); `crates/melee-sim/src/game/ft/co/hit.rs:245`.

### Animation frame from an unset float register

- **Where:** `ftAnim_8006E9B4` storing what `ftAnim_8006F3DC` returns, `crates/melee-sim/src/game/ft/anim.rs`.
- **Defect:** `ftAnim_8006F3DC` returns the float register `f1` unchanged when no part of the model animates, which happens when a state names an animation the character's file lacks (Yoshi's shield stun, `ftYs_MS_GuardDamage`, names animation 40, which `PlYsAJ.dat` doesn't have). `f1` then holds what the step's walk over the model's joints left there.
- **Effect:** in play the last animation the walk interprets is the egg shield's texture animation, so Yoshi's shield stun stores that animation's length (100) as its current frame; the step's frame check also compares with that register.
- **melee-hd:** modeled (`Objects::aobj_f1`; an unknown register counts a stale read).
- **Fix:** return the frame explicitly when nothing animates (and give the state an animation the file has).
- **Fix impact:** changes Yoshi's shield stun frame; breaks replay and Slippi compatibility where it matters.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values) (line 86); [the oracle](oracle.md) (line 73); `crates/melee-sim/src/game/ft/anim.rs:139`.

### Animation binds walk past a figatree's node list

- **Where:** `ftAnim_8006F7C8`, `ftAnim_8006F628` and `ftAnim_80070A10`, `crates/melee-sim/src/game/ft/bind.rs` and `crates/melee-sim/src/game/ft/second.rs`.
- **Defect:** they skip one node of the figatree per part before the part they bind from, without stopping at the node list's end (0xFF), so a part past the figatree's skeleton walks on past its data block. When Marth's throw launches Peach while she holds an item, her hand's part animation ends and the thrown animation (made for another skeleton) is put back on part 100, and the skip reads into the archive's relocation table.
- **Effect:** none observable in the replays checked: the bytes read decide only how long the walk runs, and it binds nothing.
- **melee-hd:** modeled (the skips read through the animation buffer, `ft::anim_buf`).
- **Fix:** stop at the node list's end.
- **Fix impact:** none on gameplay where the walk binds nothing.
- **Source:** [engine notes, loaded data the game writes into](engine.md#loaded-data-the-game-writes-into) (line 291).

### Nana's second animation lands in her main buffer

- **Where:** `ftData_80085E50`, `crates/melee-sim/src/game/ft/second.rs`.
- **Defect:** when Nana's second animation is in Popo's main buffer, it copies it into her main buffer, not her second, and points `x598` there; her next action's animation overwrites it, and `x598`, unchanged, then points at the same offset into that archive, or past its data.
- **Effect:** none observed: nothing has read such a figatree yet.
- **melee-hd:** modeled (`ft::second::SecondTree`, `Running::overwrite_main_buffer`).
- **Fix:** copy into her second buffer.
- **Fix impact:** not recorded.
- **Source:** [engine notes, loaded data the game writes into](engine.md#loaded-data-the-game-writes-into) (line 289).

### Cargo countdowns land on the next state's variables

- **Where:** `ftCo_CargoKneebend_Anim` and `ftCo_CargoLanding_Anim`, `crates/melee-sim/src/game/ft/co/cargo.rs`.
- **Defect:** the knee bend counts its lag down in `mv` x8 after the check that starts the cargo jump, so on the jump's first frame the step lands on the jump's own `mv` x8, the multiplier `ftCo_800CB110` just set (1.0 becomes 0.0); the cargo landing does the same to the cargo wait's `mv` x4.
- **Effect:** none observable: nothing reads either word afterwards.
- **melee-hd:** modeled (the ports keep the order).
- **Fix:** count down before the state change.
- **Fix impact:** none on gameplay.
- **Source:** [engine notes, fighters' variables](engine.md#fighters-variables) (line 113).

### Ganondorf's taunt falls through

- **Where:** `ftCo_800DEA28` (a taunt), `crates/melee-sim/src/game/ft/co/checks.rs` (`enter_appeal`).
- **Defect:** Ganondorf's case blows a wind from his top joint and then falls through to the default case, so the side taunt is entered twice.
- **Effect:** not recorded beyond the state being entered twice (seen in tick 5743 of the Ganondorf vs Captain Falcon benchmark).
- **melee-hd:** modeled.
- **Fix:** end Ganondorf's case after entering the taunt once.
- **Fix impact:** not recorded.
- **Source:** [the oracle](oracle.md) (line 46); `crates/melee-sim/src/game/ft/co/checks.rs:258`.

### A freed source's tip damage records no statistics

- **Where:** `ftColl_8007BE3C` (damage held back through hitlag landing), `crates/melee-sim/src/game/ft/attack.rs`.
- **Defect:** a tip's damage can land after its source item is freed (a pill, in tick 669 of `hf-doc-marth-bf-d1bc4efe`); the source GObj's first word is then the free list's link, so its classifier reads as neither a fighter's nor an item's.
- **Effect:** no statistics are recorded for that hit.
- **melee-hd:** modeled.
- **Fix:** keep the source's identity, or drop the hit, when the source is freed.
- **Fix impact:** changes the match's statistics (end-of-match records); not recorded whether anything in play reads them.
- **Source:** [the oracle](oracle.md) (line 208).

### A stage device's hit keeps an earlier hit's damage word

- **Where:** `ftColl_80076764` (a stage device's entry in the hit log, Brinstar's lava) with `ftColl_8007A06C` (the strongest logged hit), `crates/melee-sim/src/game/ft/attack.rs` (`HitLogs::stale_damage`).
- **Defect:** the device's entry writes no damage word (`x20`), so it keeps what the log's slot last held, and `ftColl_8007A06C` copies that word into the fighter's damage result (`dmg.x186c`) when the device's hit is the strongest.
- **Effect:** none observable: nothing reads the word there.
- **melee-hd:** modeled: each slot of the log (a static array) keeps its last damage word.
- **Fix:** write the device's damage into its entry.
- **Fix impact:** none in play.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values) (the stage device's paragraph).

## Match setup

### Facing players toward opponents uses a caller's register

- **Where:** `fn_8016DEEC`, `crates/melee-sim/src/game/gm/vs.rs` (`place_players`).
- **Defect:** it takes a player's first opponent without measuring its distance, keeping the last distance it measured, or before any, what its caller left in `f26`.
- **Effect:** with three or more players that decides which way the first player faces. The VS scene's entry leaves 0 there in every capture.
- **melee-hd:** modeled (0 passed).
- **Fix:** measure the first opponent's distance.
- **Fix impact:** can change the first player's starting facing in matches of three or more; breaks replay compatibility there.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values) (line 68); `crates/melee-sim/src/game/gm/vs.rs:1329`.

### Character kinds past the fighter-kind table

- **Where:** `ftMapping_list`, indexed by the player blocks, `crates/melee-sim/src/game/pl/player.rs` (`FT_MAPPING`).
- **Defect:** the table has 33 entries, and an empty slot's character kind (33 and up) indexes past it into the executable's following bytes: a padding byte and `player.c`'s message strings.
- **Effect:** none observable: the bytes are constant data.
- **melee-hd:** modeled (the port carries those bytes; a kind past them stops).
- **Fix:** bound the index.
- **Fix impact:** none.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values) (line 65).

## Characters

### Bowser: Whirling Fortress's landing test can't pass

- **Where:** `ftKp_SpecialAirHi_Coll`'s inline landing, `crates/melee-sim/src/game/ft/koopa/special_hi.rs`.
- **Defect:** it stores its attributes' `x70` back into itself, and starts the ground spin `x78` frames back only when the frames spun lie strictly between `x70` and `x70`, which no value does.
- **Effect:** the air landing always starts the ground spin from the same frame.
- **melee-hd:** modeled (the test kept as written; the no-op store left out).
- **Fix:** compare against the intended bounds (not recorded which).
- **Fix impact:** changes Bowser's up special landing; breaks replay and Slippi compatibility.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values) (line 89); `crates/melee-sim/src/game/ft/koopa/special_hi.rs:195`.

### Mr. Game & Watch: down tilt's interrupt doesn't crouch

- **Where:** `ftGw_AttackLw3_IASA`, `crates/melee-sim/src/game/ft/gamewatch/attack_lw3.rs`.
- **Defect:** it tests the stick held down with `ftCo_Squat_CheckInput`, which only tests and doesn't crouch.
- **Effect:** the stick held down ends the interrupt's checks with him still in the tilt.
- **melee-hd:** modeled.
- **Fix:** enter the crouch when the test passes.
- **Fix impact:** changes Mr. Game & Watch's down tilt interrupts; breaks replay compatibility.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values) (line 90).

### Mr. Game & Watch: aerial landings' items

- **Where:** `ftGw_AttackAirN_ItemTurtleOnLand` and `ItemSparkyOnLand`, `crates/melee-sim/src/game/ft/gamewatch/attack_air.rs`.
- **Defect:** the landing animations of his back and up aerials' items play only in the neutral aerial's landing, since both test `ftGw_MS_LandingAirN`.
- **Effect:** in their own landings the turtle and Sparky keep their aerial's animation.
- **melee-hd:** modeled.
- **Fix:** test each aerial's own landing state.
- **Fix impact:** visual, on items whose animation is state; not recorded whether gameplay reads it.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values) (line 90).

### Mr. Game & Watch: up aerial takes the back aerial's landing lag

- **Where:** `ftGw_LandingAirHi_Init`, `crates/melee-sim/src/game/ft/gamewatch/attack_air.rs`.
- **Defect:** the up aerial's landing takes the back aerial's landing lag.
- **Effect:** his up aerial lands with the back aerial's lag.
- **melee-hd:** modeled.
- **Fix:** use the up aerial's own landing lag.
- **Fix impact:** changes gameplay (frame data players rely on) and breaks replay and Slippi compatibility.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values) (line 90).

### Mr. Game & Watch: the trampoline is destroyed twice

- **Where:** `itGamewatchrescue_UnkMotion1_Anim` and `Item_80269528`, `crates/melee-sim/src/game/it/kinds/gamewatchrescue.rs`.
- **Defect:** the trampoline's animation callback destroys it once its fighter leaves Fire, clears its fighter and owner again in the item it just destroyed, and returns true, which has the animation process destroy it again. The GObj's free waits for its process to return, so the item is still there the second time.
- **Effect:** its destroy's hooks, effects, sounds and camera box run twice.
- **melee-hd:** modeled (both destroys run).
- **Fix:** return false after destroying it.
- **Fix impact:** changes effects, sounds and their handles; breaks replay compatibility.
- **Source:** [engine notes, objects and identity](engine.md#objects-and-identity) (line 17); [the oracle](oracle.md) (line 81).

### Kirby: Mr. Game & Watch's copy walks past its lookup

- **Where:** `ftKb_SpecialN_800F14B4` (the copy's costume put on), then `ftParts_80074D7C` and `ftParts_80074CA0` on Kirby's own parts visibility; `crates/melee-sim/src/game/ft/kirby/hat.rs` (`kb_gamewatch_hat_on`) and `crates/melee-sim/src/game/ft/parts.rs` (`vis_lookup_len`).
- **Defect:** the costume puts its variant lookup (`hat_dynamics[3]`, one entry, one per model group of the costume) in Kirby's own slot 4 (`x5AC.xC[4]`), which his model groups walk by his own count, two. Hiding or showing that slot reads a second entry past the lookup's end: the next word in the copy's file, which is a pointer.
- **Effect:** none observable: in memory the pointer is an address at or above 0x80000000, so as the entry's count (an `int`) it is negative and the walk runs no sets. Another file layout would have the game hide or show DObjs by whatever bytes follow.
- **melee-hd:** modeled: an entry's count that is a pointer in the archive reads as negative.
- **Fix:** give the lookup an entry per model group of Kirby's, or walk it by the costume's count.
- **Fix impact:** none (nothing changes in play).
- **Source:** `crates/melee-sim/src/game/ft/parts.rs:386`; the Kirby CPU match against Mr. Game & Watch (`oracle-kirby-gnw-bf`, tick 1644).

### Kirby: Bowser's copy lands from the aerial breath's start into its loop

- **Where:** `ftKb_KpSpecialAirNStart_Coll`, `crates/melee-sim/src/game/ft/kirby/special_koopa.rs` (`kb_kp_air_coll`, from `run`).
- **Defect:** landing during the aerial fire breath's start changes to the grounded breath's loop (`ftKb_MS_KpSpecialN`, Giga Bowser's copy's `GkSpecialN`) with the start's frame, where the other two aerial states land in their own counterparts and Bowser's own aerial start (`ftKp_SpecialAirNStart_Coll`) lands in his grounded start.
- **Effect:** a Kirby with Bowser's copy who lands during the aerial start goes straight into the grounded breath's loop at the start's frame, skipping the rest of the start.
- **melee-hd:** modeled. No checked match lands from the aerial start.
- **Fix:** land in the grounded start (`ftKb_MS_KpSpecialNStart`, `GkSpecialNStart`).
- **Fix impact:** changes Kirby's Bowser copy's landing; breaks replay and Slippi compatibility.
- **Source:** `crates/melee-sim/src/game/ft/kirby/special_koopa.rs:401`.

### Kirby: Giga Bowser's copy refills its breath while breathing

- **Where:** `ftKb_SpecialNKp_800FA7D4` (the breath's refill, each frame from `ftKb_Init_UnkMotionStates3` for Bowser's and Giga Bowser's copies), `crates/melee-sim/src/game/ft/kirby/special_koopa.rs` (`kb_kp_refuel`).
- **Defect:** the refill skips only Bowser's copy's breath states (443 to 448); Giga Bowser's copy breathes in states 538 to 543, which it doesn't skip.
- **Effect:** with Giga Bowser's copy (in the single-player modes, where Kirby can swallow Giga Bowser), the flame's speed and size refill every frame of the breath while the breath spends them, so it weakens more slowly than Bowser's copy's.
- **melee-hd:** modeled.
- **Fix:** skip Giga Bowser's copy's breath states too.
- **Fix impact:** changes Kirby's Giga Bowser copy in the single-player modes; VS replays never have it.
- **Source:** `crates/melee-sim/src/game/ft/kirby/special_koopa.rs:151`.

### Kirby: a victim breaking out of Yoshi's copy's egg keeps the shrink's scale

- **Where:** `ftCo_KirbyYoshiEgg_Anim`, `crates/melee-sim/src/game/ft/co/yoshi_egg.rs` (`egg_anim` with `Layer::Kirby`).
- **Defect:** when the victim breaks out, Yoshi's own egg (`ftCo_YoshiEgg_Anim`) sets the model's scale back (`Fighter_UpdateModelScale`) before the fall; Kirby's copy's egg leaves that call out. The fall's state change ends the egg's shrink callback, which would have set the scale back when the shrink was over.
- **Effect:** a victim that breaks out before the shrink's frames are over (20 with Kirby's attributes, which takes many mash inputs, since a hit's damage hook sets the scale back itself) keeps the model, and the bones its hurtboxes and hitboxes follow, at the shrink's scale until something sets it again. Not reached in any checked match.
- **melee-hd:** modeled.
- **Fix:** set the scale back on breaking out, as Yoshi's egg does.
- **Fix impact:** changes the victim's size after an early breakout; breaks replay and Slippi compatibility where it happens.
- **Source:** `crates/melee-sim/src/game/ft/co/yoshi_egg.rs:401`.

### Kirby: Giga Bowser's copy reads a copy file nothing loads

- **Where:** `ftKb_SpecialN_800F16D0` (the copy's items registered, `case Ft_Kind_GKoops`), from `ftKb_SpecialN_800F1BAC`; `crates/melee-sim/src/game/ft/kirby/copy.rs` (`kb_register_copy_items`).
- **Defect:** Giga Bowser has no copy file (`ftKb_Init_803CA9D0[31]` is empty), so the copy loading at the match's start (`ftKb_SpecialN_800EED50`) leaves his copy's data null; taking his copy then registers its flame from that data (`hats[30]->hat_dynamics[0]`), reading through the null pointer. His hat is Bowser's (`ftKb_SpecialN_800EFD64`), whose data is loaded only when Bowser is in the match.
- **Effect:** a Kirby who swallows Giga Bowser reads address 0x18, which faults on the console. VS can't put Giga Bowser in a match.
- **melee-hd:** stops (the port panics, a copy without its data), where the original faults.
- **Fix:** give Giga Bowser's copy Bowser's copy data, or load a copy file for him.
- **Fix impact:** changes only the single-player modes, where he appears.
- **Source:** `crates/melee-sim/src/game/ft/kirby/copy.rs:131`.

### Kirby: Jigglypuff's copy keeps the roll's look when hit

- **Where:** `ftKb_PrSetCallbacks` and `fn_80100E0C` (Rollout's hooks as Kirby rolls it), `crates/melee-sim/src/game/ft/kirby/special_purin.rs` (`kb_pr_roll_cbs`, `kb_pr_roll_hit`).
- **Defect:** Jigglypuff's own Rollout hooks its roll's undo to her damage and death (`ftPr_SpecialS_8013D658`: the model's scale back, `TopN` turned back to her facing, the turn the stick asked for taken). Kirby's copy hooks his generic cleanups instead (`ftKb_Init_800EE7B8`, `ftKb_Init_800EE74C`), whose copy cleanup does nothing for Jigglypuff's copy, so nothing undoes the roll's look when he's hit or KO'd mid-roll.
- **Effect:** hit in the four frames of a squash (the roll's start and each bounce), Kirby keeps the squashed model, and the bones his hurtboxes follow, until a later Rollout or something else sets his scale; `TopN` stays turned side-on and a turn the stick asked for is lost.
- **melee-hd:** modeled.
- **Fix:** hook the roll's undo, as Jigglypuff's own does.
- **Fix impact:** changes Kirby's size after such a hit; breaks replay and Slippi compatibility where it happens.
- **Source:** `crates/melee-sim/src/game/ft/kirby/special_purin.rs:250`.

### Kirby: Jigglypuff's copy's landing dust takes the roll for the slope

- **Where:** `ftKb_PrSpecialAirN_Coll` (Rollout landing from its bounces as Kirby rolls it), `crates/melee-sim/src/game/ft/kirby/special_purin.rs` (`kb_pr_special_air_n_charge_release_coll`).
- **Defect:** it works out the floor's angle and drops it, then hands the dust effect the roll's way (`mv.pr.specialn.x34`) and roll angle (`x14`) where Jigglypuff's own hands it her facing and the floor's angle.
- **Effect:** the dust as he lands from a bounce is turned by his roll angle rather than laid along the floor; an effect only.
- **melee-hd:** modeled.
- **Fix:** pass his facing and the floor's angle.
- **Fix impact:** none on gameplay.
- **Source:** `crates/melee-sim/src/game/ft/kirby/special_purin.rs:1341`.

### Mewtwo: a Shadow Ball field nothing stores

- **Where:** Shadow Ball's code reading `ftMewtwo_FighterVars::x2238_shadowBallGObj`.
- **Defect:** the field is named as a GObj but no code ever stores one: Shadow Ball only tests it before destroying his effects, and clears it.
- **Effect:** none observable: the test never passes.
- **melee-hd:** modeled (a zero word).
- **Fix:** not recorded.
- **Fix impact:** not recorded.
- **Source:** [engine notes, fighters' variables](engine.md#fighters-variables) (line 115).

### Mewtwo: the fired ball's dropped comparison

- **Where:** `fn_802C5E18`, `crates/melee-sim/src/game/it/kinds/mewtwoshadowball.rs` (`sparkle`).
- **Defect:** the full ball's test compares two variables (`x18`, `x1C`) and does nothing with the result.
- **Effect:** none observable.
- **melee-hd:** modeled (the comparison computed and dropped).
- **Fix:** not recorded.
- **Fix impact:** none.
- **Source:** `crates/melee-sim/src/game/it/kinds/mewtwoshadowball.rs:640`.

### Mewtwo: the forward throw's self-assignment

- **Where:** `it_802C519C` (the full Shadow Ball his forward throw fires), `crates/melee-sim/src/game/it/kinds/mewtwoshadowball.rs`.
- **Defect:** it writes its attributes' `x18` back over itself in his loaded data (`attr->x18 = attr->x18`).
- **Effect:** none observable.
- **melee-hd:** left out (the archive is untouched).
- **Fix:** remove the write.
- **Fix impact:** none.
- **Source:** [engine notes, loaded data the game writes into](engine.md#loaded-data-the-game-writes-into) (line 297).

### Mewtwo: a fully charged Shadow Ball's sound never plays

- **Where:** `ftMt_SpecialN_ReleaseShadowBall` and Kirby's copy's `ftKb_SpecialNMt_801071FC`, `crates/melee-sim/src/game/ft/mewtwo/special_n.rs` (`mt_n_release`) and `crates/melee-sim/src/game/ft/kirby/special_mewtwo.rs` (`kb_mt_release`).
- **Defect:** both empty the charge before testing it against a full charge to pick the shot's sound, so the test compares 0 with the full charge.
- **Effect:** a fully charged ball plays the ordinary shot's sound (`0x30DB3`), never its own (`0x30DB6`).
- **melee-hd:** modeled.
- **Fix:** test the charge before emptying it.
- **Fix impact:** changes a sound only.
- **Source:** `crates/melee-sim/src/game/ft/kirby/special_mewtwo.rs:267` and `crates/melee-sim/src/game/ft/mewtwo/special_n.rs:188`.

### Samus: the grapple swing's victim bit from the stack

- **Where:** `it_802B7160` (the grapple beam's button-sequence swing), from `fn_802B805C` through `it_802B9328`, `crates/melee-sim/src/game/it/kinds/samusgrapple.rs`.
- **Defect:** the swing copies its hitbox's grabbed-victim-only bit (`x134_4`) from byte 0x15 of `it_802B9328`'s `itSamusGrapple_HitboxData`, past the 0x14 bytes of hitbox words its caller fills. What lies there is `PSMTXConcat`'s save of `f14` from the tip's matrix setup just before, which is 0.0 in the game's code, so the bit is clear.
- **Effect:** the bit decides whom the swing can hit; in play it is clear (watched in ssbm-rs on `hf-roy-samus-fd-56052455`, frame 6217).
- **melee-hd:** modeled for that case (`samusgrapple::SwingSlot::SavedF14`); a swing with the tip's matrix not rebuilt, or from the other two callers, stops.
- **Fix:** have the caller set the bit.
- **Fix impact:** none if the bit is set clear, as play leaves it.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values) (line 75); [the oracle](oracle.md) (line 222).

### Samus: the grapple beam's ignore id from the stack

- **Where:** `it_802B7C18` (the grapple beam's spawn), `crates/melee-sim/src/game/it/kinds/samusgrapple.rs`.
- **Defect:** it fills a `SpawnItem` on its stack but never its `x40`, which becomes the item's ignore id (`xAC4_ignoreItemID`); in a CPU match it held a code address a call before had saved.
- **Effect:** none observable: items sharing an id share their hitboxes' victims, and the beam has no hitboxes.
- **melee-hd:** stale (`it::IgnoreId(None)`; a read counts a stale read).
- **Fix:** set `x40`.
- **Fix impact:** none.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values) (line 75); `crates/melee-sim/src/game/it/kinds/samusgrapple.rs:490`.

### Samus: the grapple beam reads its owner as an item

- **Where:** `it_802A2428` (the hookshot unit's scale, which the grapple beam uses), `crates/melee-sim/src/game/it/kinds/samusgrapple.rs`.
- **Defect:** it reads its owner, a fighter, as an item: the item's `scl` sits where a fighter keeps its height (`x34_scale.y`).
- **Effect:** none observable: the beam takes her height times its attributes' scale.
- **melee-hd:** modeled.
- **Fix:** read the fighter's height through the fighter.
- **Fix impact:** none.
- **Source:** [engine notes, fighters' variables](engine.md#fighters-variables) (line 117).

### Mario: the cape reads a fighter field on an item

- **Where:** `it_2725_Logic41_PickedUp` (Mario's cape, picked up), `crates/melee-sim/src/game/it/kinds/mariocape.rs`.
- **Defect:** it picks its state with `ftLib_GetGroundAir` on its own GObj; the word at the fighter's `ground_or_air` offset (`+0xE0`) is, in an item, its first dynamic bone set's count, which the cape never writes, so it holds what its block held.
- **Effect:** the state depends on the block's last item; probed zero (a fresh block) on a ranked Mario replay.
- **melee-hd:** modeled (`Item::xD4_dynamicBones`, kept from the slot's last item).
- **Fix:** test the item's own ground or air state.
- **Fix impact:** could change the picked-up cape's state where the block held a nonzero count; not recorded.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values) (line 70).

### Link: the boomerang's spin takes a stack word for w

- **Where:** `it_802A0534` (the boomerang throw), `crates/melee-sim/src/game/it/kinds/linkboomerang.rs`.
- **Defect:** it reads the hand's matrix's turn into the x, y and z of a stack `Quaternion` whose w it never sets, and writes that as the spinning part's rotation, as `lb_8001044C` does.
- **Effect:** none observable on both Link replays (nothing reads that w).
- **melee-hd:** stale (`jobj::set_rotation_stale_w`).
- **Fix:** set w, or write Euler angles.
- **Fix impact:** none.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values) (line 82).

### Sheik: the needles' bounce draws twice

- **Where:** `itSeakneedlethrown_UnkMotion0_Coll` and `it_2725_Logic109_HitShield` (and the hit hooks sharing it), `crates/melee-sim/src/game/it/kinds/seakneedlethrown.rs` (`random_abs`).
- **Defect:** `ABS(table[HSD_Randi(8)])` is a macro that evaluates its argument twice: one random draw for the sign test, another for the value.
- **Effect:** each bounce draws an extra random number, moving every later draw of the match's one random seed. The table's entries are all negative, so the value's sign comes out right.
- **melee-hd:** modeled.
- **Fix:** draw once.
- **Fix impact:** changes the random sequence after any needle bounce, so gameplay after it; breaks replay and Slippi compatibility.
- **Source:** `crates/melee-sim/src/game/it/kinds/seakneedlethrown.rs:157`.

### Sheik: the chain reads its clank switch before writing it

- **Where:** Sheik's chain swing, `itSeakChain_ItemVars::x18`, `crates/melee-sim/src/game/it/kinds/seakchain.rs` (`link_coll_clank`).
- **Defect:** the chain reads its clank switch at the start of each swing, which a new chain does before writing it, so the first swing finds the block's last item's bytes there.
- **Effect:** the switch decides only whether a link newly meeting the stage, and not already sounding, clanks: the first swing's clank depends on the previous item.
- **melee-hd:** read there and nowhere else, counted as a read.
- **Fix:** initialize the switch at the chain's spawn.
- **Fix impact:** sound only, but sound handles are state; not recorded further.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values) (line 80).

### Zelda: Din's Fire's explosion keeps its x speed

- **Where:** `itZeldadinfireexplode_UnkMotion0_Phys`, `crates/melee-sim/src/game/it/kinds/zeldadinfireexplode.rs`.
- **Defect:** it stops the vertical and depth speeds, clearing z twice and leaving x.
- **Effect:** not recorded.
- **melee-hd:** modeled.
- **Fix:** clear x.
- **Fix impact:** not recorded.
- **Source:** `crates/melee-sim/src/game/it/kinds/zeldadinfireexplode.rs:215`.

## Ice Climbers and the computer player

### A computer player's DI with no knockback reads two registers it never set

- **Where:** `ftCo_800AC5A0` (the computer player's DI in hitstun, behavior 18, marked `@bug` in the decompilation), from `ftCo_800B2790`, `crates/melee-sim/src/game/ft/cpu_move.rs` (`cpu_di`) and `crates/melee-sim/src/game/ft/cpu_ai.rs` (`Leftover`).
- **Defect:** with no knockback it pushes the stick by r5 for x and r30 for y, neither of which it or its caller sets for that path: r5 holds what the computer player's frame (`ftCo_800B3900`) left (for Nana on her way back to Popo, Popo's state or her held item's address; for a VS CPU often a stack address or a flag the behavior pick's floor check left, `mpCheckFloor`, or, on the four stages whose floors it may exclude, what that check's stage call left, `ftCo_800A1B38`: on Big Blue 1 or 0 by the floor's joint (`grBigBlue_801EF844`), on Corneria the third flag of `grCorneria_801E2D90`'s chain of joint tests, 1 when the floor's collision joint is 0, 1, 2 or 5 and else 0, and on Venom the address of the collision joints' array that `mpJointFromLine` leaves in r5), r30 its caller's `Fighter*`, so the stick's y is the low byte of the fighter's address.
- **Effect:** Ice Climbers matches go different ways when the heap is laid out differently: a match started from a console's menus and the same match under Slippi's playback give Nana different addresses, so a replay of an Ice Climbers match leaves its recording at the first such DI (Peach back-throwing Nana at frame 10,137 in the performance notes' case).
- **melee-hd:** modeled: the allocator model gives each fighter the original's address, and each function on the way returns the r5 it leaves (`cpu_ai::Leftover`), the floor search included (`Geometry::check_floor_r5`) and Mushroom Kingdom's, Corneria's and Venom's excluded floors (`Running::excluded_floor_r5`, `crates/melee-sim/src/game/ft/cpu.rs`; Mushroom Kingdom's and Venom's from the allocator model's address of the joints' array, `Geometry::joints_addr`); a leftover not followed stops, as Big Blue's excluded floors do. Slippi's NanaDeterminism code clears r30 (and r5 in 3.19's), which the Slippi ruleset follows.
- **Fix:** push the stick neutral, or along a defined direction, with no knockback.
- **Fix impact:** changes Nana's DI in those cases; breaks vanilla replay compatibility for Ice Climbers matches (Slippi's code already fixes it online, differently in 3.16 and 3.19).
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values) (lines 93 and 95); [performance](performance.md) (line 186); [the oracle](oracle.md) (line 140).

### The computer player heads for a stack vector when no island end is in reach

- **Where:** `ftCo_800A4768` (the island end beside a fighter) with its caller `ftCo_800A75DC` (heading for a fighter), from the VS decision and from `ftCo_800AE7AC` (heading back from off camera, after `ftCo_800A6FC4` finds no island toward the blast zones' middle), `crates/melee-sim/src/game/ft/cpu_vs.rs` (`cpu_island_end_beside`, `cpu_toward`, `cpu_back_on_screen`, `cpu_island_toward`) and `cpu_decide.rs` (`cpu_find_target_lr`).
- **Defect:** `ftCo_800A4768` writes its out vector only when some island end beside the fighter is in reach, and returns 0 either way; with none (an airborne target with no floor below, between every island's ends, as under Final Destination), `ftCo_800A75DC` sets the destination from its stack vector unwritten. Off camera on the ground, that vector's slot lies in the frame `ftCo_800A6FC4` has just left, where its prologue saved f20 (`stfd f20, 0xA8(r1)`), and nothing writes it between.
- **Effect:** the computer player heads for what the stack held: under the VS decision, the back chain and saved return address the target search's frame left there, two tiny negative floats, about the stage's middle at floor height. `cpu-ics-fd` reads it 25 times (first in tick 3260) and `cpu-sweep-samus-fd` from tick 5721. Off camera the destination takes f20's two words, which hold the boot's 0.0 (nothing from `main` down to the computer player's frame writes f20), so the computer player heads for (0, 0); no oracle match reaches that case (on `cpu-stage-poke-floats` every airborne target had an island end in reach), leaf-check's `cpu_vs` `OffCameraGround { target_under }` does.
- **melee-hd:** modeled (the target search returns the return address it leaves, and the VS decision hands the vector to the call; off camera the vector is f20's words, `hsd::gobj::PROC_F20`); a caller that doesn't give it stops.
- **Fix:** leave the destination as it was, or head for the target, when no end is in reach.
- **Fix impact:** changes where a computer player heads while its target is under the stage; a replay's recorded inputs keep its fighters' motion, but the CPU's state, and a free-running CPU, differ.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values) (the island end's paragraph and the off-camera one); `crates/melee-sim/src/game/ft/cpu_vs.rs` (`cpu_back_on_screen`).

### The Ice Climbers' freeze glitch

- **Where:** `ftNn_Init_80123954` (whether Nana joins Popo's Squall Hammer), `crates/melee-sim/src/game/ft/popo/nana_special_s.rs` (`nana_join_special_s`).
- **Defect:** when Popo starts his side special with Nana in hitstun, the code clears her `x1A5C`, the link to the fighter she is frozen with, so the partner it pointed to is no longer thawed through her.
- **Effect:** Slippi ships a fix for it as the freeze glitch fix (`External/FreezeGlitchFix`, a nop over that store); its effect in play isn't described in melee-hd's sources, and no replay at hand tells the fix and the original apart.
- **melee-hd:** modeled; the Slippi ruleset's fix is `Rules::freeze_glitch_fix`.
- **Fix:** keep `x1A5C`, as Slippi's code does.
- **Fix impact:** changes gameplay where the glitch would happen; the Slippi ruleset already has the fix.
- **Source:** [the oracle](oracle.md) (line 135); `crates/melee-sim/src/game/ft/popo/nana_special_s.rs:67`.

### Nana reads a held item's kind as an action state

- **Where:** `ftCo_800ADE48` (Nana's decision) calling `ftCo_800A5980`, `crates/melee-sim/src/game/ft/cpu_decide.rs` (`cpu_throws_held_item`).
- **Defect:** `ftCo_800A5980` tests a fighter's action state, and the decision hands it the `Item*`, so it reads the item's kind, the word at the same offset (`+0x10`).
- **Effect:** items of kinds 43 to 46, 211 to 213 and 217 pass as dangerous, so Nana throws them; Peach's turnips (kind 0x63) pass neither test, so she keeps one.
- **melee-hd:** modeled (the kind tested against those numbers).
- **Fix:** test the item's kind against the items meant.
- **Fix impact:** changes Nana's item throws; breaks replay compatibility for Ice Climbers matches with items about.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values) (line 76); [the oracle](oracle.md) (line 220).

### The computer player's tumble prediction takes the wrong root

- **Where:** `ftCo_800ABBA8` (tumbling, behavior 15), `crates/melee-sim/src/game/ft/cpu_move.rs` (`cpu_tumble`).
- **Defect:** predicting where its fall lands, the code takes the square root of the frames where the square belongs.
- **Effect:** the computer player's guess of whether its fall lands above its destination is wrong, which decides whether it drifts toward it; Nana's tumble runs it.
- **melee-hd:** modeled.
- **Fix:** use the square of the frames.
- **Fix impact:** changes computer players' drift out of tumble (Nana included); breaks replay compatibility for Ice Climbers matches.
- **Source:** `crates/melee-sim/src/game/ft/cpu_move.rs:984`.

### The computer player's DI switch takes a caller's register

- **Where:** `ftCo_800ADE48` (choosing a behavior), `crates/melee-sim/src/game/ft/cpu_decide.rs` (`cpu_choose_behavior`).
- **Defect:** the code never sets the DI switch's flag in the case it means to take it, using what r31 held in its caller.
- **Effect:** not recorded.
- **melee-hd:** modeled (`di_switch` is that register's truth).
- **Fix:** set the flag.
- **Fix impact:** not recorded.
- **Source:** `crates/melee-sim/src/game/ft/cpu_decide.rs:326`.

### The computer player's attack rolls on a stale flag from a Starman or hammer

- **Where:** `ftCo_800B658C` (attacking, behavior 2) reading `CpuFighter::xF8_b6`, which under the VS decision (`ftCo_800B24B8`) only `ftCo_800A20A0` writes, on frames whose target is an enemy with a Starman or a hammer (`ftCo_800A5CE0`); `crates/melee-sim/src/game/ft/cpu_attack.rs` (`cpu_attack_behavior`).
- **Defect:** the VS decision neither refreshes nor clears the flag on other frames (the other computer kinds' decisions refresh it every frame, `ftCo_CpuUpdateTargetDistance`), so it keeps the answer of the last frame an enemy had a Starman or a hammer until a roll clears it. The approach (`ftCo_800AB224`) and standing (`ftCo_800ACD5C`) read it only while that enemy is the target (`xF9_b1`), but the attack reads it without that test, stale.
- **Effect:** a computer player attacking on the ground may, at its level's chance (5% a level), roll away from an ordinary target long after the Starman or hammer has gone (`ftCo_800BA2E8`): in `cpu-approach-yoshi-bf` at tick 4,967 (no enemy with either) and `cpu-approach-captain-dl` at tick 4,803 (its target has the hammer's music, but with `x2338` set, which `ftCo_800A5CE0` doesn't take).
- **melee-hd:** modeled: the flag persists as in the original.
- **Fix:** refresh the flag on every decision, or test `xF9_b1` in the attack as the other two readers do.
- **Fix impact:** changes computer players' rolls in items-on matches; a replay's recorded inputs keep its fighters' motion, but the CPU's state, and a free-running CPU, differ.
- **Source:** `crates/melee-sim/src/game/ft/cpu_attack.rs:1169`; [the oracle](oracle.md#the-cpu-ai).

### A shrunk Yoshi's jump writes its script's end twice

- **Where:** `ftCo_800A0508` (a shrunk computer Yoshi jumping toward a destination near it), `crates/melee-sim/src/game/ft/cpu_move.rs` (`cpu_small_yoshi_jump`).
- **Defect:** `ftCo_CpuFinishWithNeutralY` already ends the script with `CpuCmd_Done`, and the function writes another `Done` after it.
- **Effect:** none observable: the script stops at the first `Done`, and the second is a byte in the script buffer past it that nothing plays.
- **melee-hd:** modeled: both are written, so the buffer compares with the original's.
- **Fix:** drop the second `Done`.
- **Fix impact:** none in play; the CPU's script buffer would differ from the original's by that byte.
- **Source:** `crates/melee-sim/src/game/ft/cpu_move.rs:349`.

### Nana's input ring starts with the heap's C-sticks

- **Where:** `ftCo_800A101C` (a computer player's setup), read by `ftCo_800B0AF4`, `crates/melee-sim/src/game/ft/cpu_ai.rs`.
- **Defect:** the setup fills the 30 entries of the ring Nana follows Popo's inputs through (`CpuFighter::xFC`) but their C-stick and two bytes after it (`cstick`, `xA`, `xB`), and points her reading end five entries behind his writing one, so her first five reads take C-sticks the block held.
- **Effect:** not recorded: her first five frames of following, at the match's start (in her entrance), take the heap's C-stick.
- **melee-hd:** modeled: in a session from power-on the ring's block holds the heap's image's bytes (`game::leftovers`, the `Fighter` read from it), and zero in a replay, as Slippi's zeroed block.
- **Fix:** clear the C-stick with the rest.
- **Fix impact:** none in replays, whose blocks are zeroed.
- **Source:** [engine notes, a session's heap leftovers](engine.md#uninitialized-values).

## Items

### A rare Pokémon is recorded before it's made

- **Where:** `it_8027A4D4` (the Poké Ball's draw) and `it_8027AB64` (the release), `crates/melee-sim/src/game/it/pokemon/mod.rs` (`rare`, `release`).
- **Defect:** the draw marks the match's rare Pokémon as out (`Item_804A0E24.rare_spawned`) and records Mew or Celebi in the save (`gm_80172BC4`, `gm_80172C04`: the clock's time, pending, noted), and the release records it again, all before `Item_80268B18` makes the item, which gives nothing back when its hold kind is at its limit (`Item_8026784C`) or no GObj or block is left. The release then returns false with nothing made.
- **Effect:** none observed: in the Celebi sessions every drawn Celebi came out. Were the spawn to fail, the match's one rare release would be spent and the save would note Mew or Celebi as seen (its special message pending; for Celebi, its trophy given at the next main menu, `gm_80173EEC`) with none on screen.
- **melee-hd:** modeled: the records are written where the original writes them (`Running::record_rare_pokemon`), before the spawn.
- **Fix:** record the rare Pokémon and mark it out only once `Item_80268B18` returns an item.
- **Fix impact:** none in play; a save could no longer note a rare Pokémon never seen.
- **Source:** `crates/melee-sim/src/game/it/pokemon/mod.rs` (`rare`, `release`); the decomp's `it_279C.c`.

### Item creation clears one coordinate three times

- **Where:** `Item_80267AA8` (a new item's variables), `crates/melee-sim/src/game/it/item.rs` (`init_vars`).
- **Defect:** it clears `xC58.x` three times and never y or z, which keep the block's last item's values.
- **Effect:** not recorded.
- **melee-hd:** modeled.
- **Fix:** clear x, y and z.
- **Fix impact:** not recorded.
- **Source:** `crates/melee-sim/src/game/it/item.rs:360`.

### A Blizzard puff's flag set in its freed block

- **Where:** `it_802750F8`, `crates/melee-sim/src/game/it/lib2725.rs`.
- **Defect:** when a Blizzard puff is freed by its first frame's collision, the code sets its flag in the freed block.
- **Effect:** a write into freed memory; seen in tick 7122 of `hf-fox-ics-ps-03b0d4e2-rf`; what it changes is not recorded.
- **melee-hd:** modeled (the freed item's slot looked up by its GObj).
- **Fix:** skip the write once the item is freed.
- **Fix impact:** not recorded.
- **Source:** [the oracle](oracle.md) (line 213).

### The weighted draw reads past its weights

- **Where:** `bisectValue`, from `it_8026C65C` (a weighted draw from a table), `crates/melee-sim/src/game/it/spawn.rs` (`bisect`).
- **Defect:** past the last weight it reads the block's uninitialized slack.
- **Effect:** none observable: there both answers are the same.
- **melee-hd:** modeled (the read left out).
- **Fix:** bound the read.
- **Fix impact:** none.
- **Source:** `crates/melee-sim/src/game/it/spawn.rs:99`.

### The Super Scope's affordable level returns its GObj: infinite ammunition

- **Where:** `it_80291DAC` (the level a charge fires with the ammunition left) and `it_80291F14` (spending a shot), `crates/melee-sim/src/game/it/kinds/sscope.rs` (`affordable`, `spend`), with the fighter's scope states in `crates/melee-sim/src/game/ft/co/item_scope.rs`.
- **Defect:** when the charge's level costs more than the ammunition left, `it_80291DAC` walks down the levels to 1 for one it can afford; when none is (or the charge's level is already 0), it ends without a return and the machine code returns its first argument, the scope's GObj address. Its callers take that as a level. A charged shot keeps it in the fighter's state (`fn_800D8140` and `fn_800D81D0`, `mv.co.common.x0`) and fires it (`fn_800D84D4`), which no level case matches, so it only clicks. A hit while charging (`fn_800D7938`) spends it: `it_80291F14`'s inline copy of `it_80291D38` has no case for it and leaves the cost in the level's register, so it subtracts the address itself, and a few shots minus an address in main memory (0x80000000 and up, a negative word) is a positive count over two billion. (`it_80291D38` called on its own returns its first argument, the GObj address, for such a level; nothing in the game calls it so.)
- **Effect:** the well-known Super Scope glitch that gives it effectively infinite ammunition: with less ammunition left than a level 1 shot costs (one shot), charge past level 0 and be hit while charging, and the scope has over two billion shots for the rest of its life. Charged without a hit, the shot only clicks, and the fighter's state word holds the scope's address, which Slippi records as the post-frame "hitstun" word.
- **melee-hd:** modeled: `affordable` returns the scope's GObj address and `spend` subtracts a level outside the cases as it is. The address is the allocator model's (`hsd::heap`), which every replay runs with and which `Settings::vs_with` turns on for a match whose item switch can spawn a Super Scope, laid out as a console's VS match from the menus; without the model the address is unknown, and reaching it panics.
- **Fix:** return no level (-1) when none is affordable.
- **Fix impact:** removes the glitch, which players know and use: a hit while charging with one shot left no longer refills the scope, and a charge then fires no click. Replays where it happens would desync.
- **Source:** `crates/melee-sim/src/game/it/kinds/sscope.rs` (`affordable`, `spend`); ssbm-rs transliterates `it_80291D38`, `it_80291DAC` and `it_80291F14` from their machine code for this.

### A Cloaking Device taken while one flashes counts refraction twice

- **Where:** `ftCo_800C88D4` (a fighter turning invisible), `crates/melee-sim/src/game/ft/powerup.rs` (`start_cloak`).
- **Defect:** it starts the cloak's look, which makes the fighter one more user of refraction (`lbRefract_80022BB8`), unless the fighter is already cloaked and not yet flashing toward the end (`x2226_b7`). A second Cloaking Device taken during the flash runs the look again, so the fighter counts as two users, but the cloak's end (`ftCo_800C8A64`, also run when the fighter is KO'd) takes only one away (`lbRefSetUnuse`).
- **Effect:** refraction's user count stays above zero for the rest of the match, so the camera keeps copying the screen and computing refraction's texture matrix every frame; nothing the game reads depends on it.
- **melee-hd:** modeled.
- **Fix:** count the fighter as a user only when it wasn't cloaked (`!x2226_b4`).
- **Fix impact:** none on gameplay; the screen copy stops after such a cloak ends.
- **Source:** `crates/melee-sim/src/game/ft/powerup.rs` (`start_cloak`); `ft_0C88.c`.

### An item's grab takes the first fighter it touches, not the nearest

- **Where:** `it_802701BC` (an item's grab boxes against fighters, the barrel cannon's catch), `crates/melee-sim/src/game/it/coll.rs` (`grab_fighters`).
- **Defect:** it keeps the nearest fighter seen so far (`xD10`, the horizontal distance, starting at the float maximum), but returns as soon as one grabbable hurtbox touches, so the comparison only ever sees the first catch.
- **Effect:** when a barrel cannon's grab box touches two fighters on the same frame, it takes the one earlier in the fighters' list rather than the nearer.
- **melee-hd:** modeled.
- **Fix:** keep looking through every fighter and take the nearest.
- **Fix impact:** changes which fighter is caught in that case; replays where it happens would desync.
- **Source:** `crates/melee-sim/src/game/it/coll.rs:29`.

### A container's record marks the stack for an item that didn't come out

- **Where:** `pl_8003E4A4` (a player's runs of containers giving each kind, from `ftLib_800874CC`), `crates/melee-sim/src/game/pl/bonus.rs` (`count_items_got`).
- **Defect:** `match_item_kind` gives -1 for a kind outside the item log, which passes the signed bound check (`< Pl_ItemLog_Terminate`), so the function marks the stack word before its array of marks. The Party Ball's rain (`it_80289BE8`) hands it -1 for a random item that didn't come out.
- **Effect:** none observable: the marked word is the function's own stack, which nothing reads.
- **melee-hd:** modeled (kinds outside the log are passed over); the Party Ball's record isn't ported and stops.
- **Fix:** test the kind for -1 as well.
- **Fix impact:** none.
- **Source:** `crates/melee-sim/src/game/pl/bonus.rs:218`.

### Unown's new model leaves its bone table naming the old one's joints

- **Where:** `it_80273318` (an item's model swapped, which Unown and its swarm use to take a letter's model), `crates/melee-sim/src/game/it/lib2725.rs` (`set_model`).
- **Defect:** it releases the item's old model (`HSD_GObjObject_80390B0C`) and makes the new one, but leaves the item's bone table (`xBBC_dynamicBoneTable`) as it was, so its entries point at the freed joints.
- **Effect:** none observable in the six Poké Ball matches: nothing reads Unown's bone table after the swap there.
- **melee-hd:** modeled: the table keeps the freed joints' handles, as the original keeps their addresses.
- **Fix:** rebuild the bone table for the new model (`Item_802682F0`).
- **Fix impact:** none unless code reads the table after a swap.
- **Source:** `crates/melee-sim/src/game/it/lib2725.rs:428`.

### Cyndaquil's flame turns an angle nothing reads

- **Where:** `it_802D61C8` and `it_802D6310` (Cyndaquil's flame touching the stage, called by the flame's collision `it_802D6798` in `itmaril.c`), `crates/melee-sim/src/game/it/kinds/hinoarashi.rs` (`touch_normal`, `turn_off_surface`).
- **Defect:** touching a floor, wall or ceiling, the flame turns its heading (`+0x78`) off the surface, but after the flame is made nothing turns that heading back into its velocity, so the turn never changes how it moves.
- **Effect:** none observable: the flames slide on as they were launched.
- **melee-hd:** modeled.
- **Fix:** recompute the velocity from the turned heading, if a bounce was intended.
- **Fix impact:** changes the flames' paths, so replays and Slippi peers would desync.
- **Source:** not recorded beyond the port (`crates/melee-sim/src/game/it/kinds/hinoarashi.rs:296`).

### A far Arwing's laser takes its spawn position from the stack the sound left

- **Where:** `it_802E7654` (a far Arwing's aimed laser on Corneria and Venom, from `grCorneria_801DED50` and `grVenom_80205F30`), `crates/melee-sim/src/game/it/kinds/arwinglaser.rs` (`spawn_aimed`) and `crates/melee-sim/src/audio/driver.rs` (`sfx_sample`).
- **Defect:** the spawn's `SpawnItem` never gets its own position (`pos`), so `Item_80268B18` copies three stack words into the item's collision as its last position and into its ECB's motion (`xB54`). Both callers play the shot's sound just before (`lbAudioAx_800237A8`), and its start (`HSD_AudioSFXStartParam`) leaves in those words the registers its prologue saved and then set: the track (0), the channel (7) and the sound's sample index (0xEDE for Corneria's 0x55734, 0xF47 for Venom's 0x6B6C9), as float bits.
- **Effect:** none observable: the laser's own collision probes from its tip, and only a stomp on a stompable item reads `xB54`; the words are tiny denormals either way.
- **melee-hd:** modeled: the caller passes the sound it played and the spawn computes its sample from the sound bank's table.
- **Fix:** set `pos` to the gun's place, as `prev_pos` is.
- **Fix impact:** none in play (the item's last position and ECB motion only, which nothing reads); the state differs from vanilla in those words.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values); [the oracle](oracle.md#stages) (Corneria and Venom); found with ssbm-rs's `WATCH_ADDR` on `taunt-fox-venom` (tick 696).

### The Octorok's stone spins by a stack word's sign

- **Where:** `it_802E8ADC` (the Octorok's stone's spawn), `crates/melee-sim/src/game/it/kinds/octarockstone.rs` (`spawn`).
- **Defect:** it never sets its `SpawnItem`'s velocity, a stack vector; the spawn copies it into the stone and reads its x's sign for the spin's way (`it_80274658`) before `it_802E89D0` sets all three.
- **Effect:** the spin's way follows what the stack held: in both Octorok oracle matches a heap address the joint lookup before it left, a negative float, for every stone (a zero there makes the spin -0.0 where the original's is 0.0).
- **melee-hd:** a stand-in negative x; all 28 stones of the two matches compare clean.
- **Fix:** set the velocity before the spawn.
- **Fix impact:** not recorded beyond the spin's sign.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values) (the Octorok's stone's paragraph).

### A stage's hurtbox item's spawn flags keep seven bits from the stack

- **Where:** `it_802E6AEC` (a stage's hurtbox item, from `grMaterial_801C8CFC` and `grMaterial_801C8D44`; Peach's Castle's among them), `crates/melee-sim/src/game/it/kinds/yaku.rs` (`spawn`).
- **Defect:** its `SpawnItem` is a stack struct (`r1 + 0x3C` of its 0xB0-byte frame), and it sets the fall flag `x44_flag.b0` by reading the flag byte back (`lbz` at `0x802E6BB8`, `it_802E6AEC + 0xCC`, `r1 + 0x80`), inserting the bit (`rlwimi`, 0x80) and storing it, so the byte's other seven bits keep what that slot held from earlier calls. ssbm-rs's lockstep reports the read once in 900 fields of `stage-peachs-castle` with only `melee/gr/grcastle` ported (`LOCKSTEP_UNINIT`); the slot's value is not recorded.
- **Effect:** none observable: the only reader of the byte is `Item_80268B18`'s placement (`itmaplib.c:233`, `it_80275E98`), which tests `b0` alone, and nothing copies the `SpawnItem` whole.
- **melee-hd:** stand-in: the spawn starts from a cleared `SpawnItem` (`SpawnItem::default()`), so those bits are 0, and the placement's port reads only 0x80 (`crates/melee-sim/src/game/it/maplib.rs`).
- **Fix:** clear the flag byte before setting `b0`.
- **Fix impact:** none.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values); the decompilation's `it_802E6AEC` (`spawn.x44_flag.x0.b0 = arg5 ? 1 : 0`) and its machine code; `itmaplib.c:233`.

### A coin scatter with no coin to make returns its caller's register

- **Where:** `it_802E609C` (making a scatter's coins), `crates/melee-sim/src/game/it/kinds/coin.rs` (`drop_all`).
- **Defect:** with no coin to make it returns a register it never set, and when a coin can't be made it tries again forever.
- **Effect:** none observable: its only caller that uses the result (`it_802E5F8C`) holds the count it passed in that register, so it gets its own count back; no match at hand fails a coin's spawn.
- **melee-hd:** modeled (it returns the count); a coin that can't be made stops instead of looping.
- **Fix:** return the count, and give up when the spawn fails.
- **Fix impact:** none in play.
- **Source:** [the oracle](oracle.md) (the coin matches); `crates/melee-sim/src/game/it/kinds/coin.rs` (`drop_all`).

## Stages and collision

### Final Destination's background compares green twice

- **Where:** `grLast_8021B2E8` (the background's cycle), `crates/melee-sim/src/game/gr/last.rs` (`advance`).
- **Defect:** picking the next background color, it redraws until the color is neither gray nor the current one, but the test against the current color compares green twice and never blue.
- **Effect:** a color that differs from the current one only in blue counts as the same and is drawn again, which takes more random numbers. Slippi 3.19's code saves and restores the seed around this call, and 3.16's never runs the timer, so only vanilla play feels it.
- **melee-hd:** modeled.
- **Fix:** compare blue.
- **Fix impact:** changes the random sequence in vanilla play on Final Destination, so gameplay after it; breaks vanilla replay compatibility (not Slippi's, whose codes keep this off the seed).
- **Source:** `crates/melee-sim/src/game/gr/last.rs:277`; [the oracle](oracle.md) (line 139).

### Final Destination's fog alpha from the stack

- **Where:** `grLast_8021C500` (the fog blend) handing `Ground_801C205C` a color, `crates/melee-sim/src/game/gr/last.rs` and `crates/melee-sim/src/game/gr/ground.rs`.
- **Defect:** the stack color it hands the fog has an alpha it never set: the low byte of whatever the frame's earlier calls left in the slot.
- **Effect:** none observable: nothing reads a fog's alpha.
- **melee-hd:** stale (`Fog::stale_alpha`).
- **Fix:** set the alpha.
- **Fix impact:** none.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values) (line 57).

### Pokémon Stadium's return zeroes a file's header

- **Where:** Pokémon Stadium's return to the default stage, handing `grAnime_801C65B0` a buffer, `crates/melee-sim/src/game/gr/stadium.rs`.
- **Defect:** it hands `grAnime_801C65B0` the transformation file's buffer instead of its archive, which zeroes the first words of that file's header.
- **Effect:** not recorded (melee-hd's importer finds such a file through the header copy in the stage's `HSD_Archive`).
- **melee-hd:** not recorded beyond the importer's handling.
- **Fix:** pass the archive.
- **Fix impact:** not recorded.
- **Source:** [engine notes, loaded data the game writes into](engine.md#loaded-data-the-game-writes-into) (line 286).

### Final Destination's swaying piece follows its generator after the generator dies

- **Where:** `grLast_8021ADD0` (the swaying piece's process), `crates/melee-sim/src/game/gr/last.rs` (`proc_sway`).
- **Defect:** the piece keeps its generator (`xE0`) after the generator's life ends, and each tick reads the generator's app-SRT through the freed block.
- **Effect:** none observable in the matches checked: the generator's removal cleared its app-SRT pointer, so the process stops there; had the block gone to a new generator first, the piece would move that generator's particles.
- **melee-hd:** modeled: the freed generator's slot is read as the original reads the block (slots are reused as the original reuses addresses), and a captured state imports the pointer as a freed slot.
- **Fix:** clear `xE0` when the generator's life ends.
- **Fix impact:** none in play.
- **Source:** [engine notes, effects](engine.md#effects); `crates/melee-sim/src/game/gr/last.rs` (`proc_sway`).

### A random pick between two entries draws below one

- **Where:** `Ground_801C5840`, `crates/melee-sim/src/game/gr/ground.rs` (`x6e4_drawn`).
- **Defect:** it picks one of `stage_info.x6E4`'s two entries with `HSD_Randi(1)`, which is always 0, so it always returns the first, and the draw still advances the random seed.
- **Effect:** the second entry is never picked, and each call takes a random number for nothing. Its callers are Icicle Mountain's and the adventure routes' trophy slots (`grZakoGenerator_801CAEB0(Ground_801C5840(), Ground_801C5940())`), so it changes which trophy can come out there and shifts the random sequence after it.
- **melee-hd:** modeled.
- **Fix:** `HSD_Randi(2)`, or drop the draw if only the first entry is meant.
- **Fix impact:** changes the random sequence (and the trophies) wherever it runs, so gameplay after it; breaks replay compatibility on those stages.
- **Source:** `crates/melee-sim/src/game/gr/ground.rs` (`x6e4_drawn`); the decompilation's `ground.c` (`Ground_801C5840`).

### Brinstar Depths: a debugging button steps Kraid's haze's animation

- **Where:** `grKraid_801FE2D0` (the process of Kraid's "copy", map 1: a translucent smoky haze drawn behind him that follows him), `crates/melee-sim/src/game/gr/kraid.rs` (`proc_copy`).
- **Defect:** a debugging leftover: each time A is pressed on the second controller (the pad library's master status, `HSD_PadMasterStatus[1]`, whoever plays that port), it steps a counter (`grKr_804D6A0C`) through the haze's seven animations, starts that one on the haze and reports it to the debugger (`*** Req Effect Anime`).
- **Effect:** in play, every new press of A on port 2 swaps the haze to the next animation, which visibly changes the haze behind Kraid, or removes it, while he's up; mashing A cycles through all seven during one of his animations. The haze is hidden while Kraid is down, and each of his state changes restarts it on the animation that state sets, though the counter keeps its place. Rendered on 2026-10-17 (ssbm-rs's GX render of the Brinstar Depths recording with the pads it was made with, against its playback, which never presses): the haze differs from 02:25.39 on. The haze's animations draw random numbers, so the presses shift the random sequence, which changes untracked cosmetic effects (hit sparks, the damage digits' shake) for long stretches; gameplay is unchanged. Under Slippi's playback the inputs reach the fighters, not the master statuses, so a replay never reproduces it: the oracle's Brinstar Depths recording parts from ssbm-rs's playback in the random seed on seven frames ([the oracle](oracle.md)).
- **melee-hd:** modeled: it reads the master status as the original does.
- **Fix:** drop the button.
- **Fix impact:** changes the copy's animation and the random sequence wherever port 2 presses A on Brinstar Depths in vanilla play; Slippi replays and online play (whose inputs don't go through the master statuses for this) would match a fixed game better, not worse.
- **Source:** `crates/melee-sim/src/game/gr/kraid.rs` (`proc_copy`); the decompilation's `grkraid.c`.

### Brinstar Depths: the turn's speed bound stored back into itself

- **Where:** `grKraid_801FE6D8` (a landing swipe turning the rock), `crates/melee-sim/src/game/gr/kraid.rs` (`swipe_hit`).
- **Defect:** it writes the parameters' lowest turning speed (`map_rot_spd_min`) back into itself, a store into the stage's loaded data that changes nothing.
- **Effect:** none observable.
- **melee-hd:** left out (the value doesn't change).
- **Fix:** drop the store.
- **Fix impact:** none.
- **Source:** the decompilation's `grkraid.c`.


### The airborne collision loop can hang

- **Where:** `mpColl_80046904` (one airborne substep), `crates/melee-sim/src/mp/coll/air.rs` and `crates/melee-sim/src/mp/coll/mod.rs`.
- **Defect:** it repeats its walls, ceiling and floor checks until the squeezes they apply stop changing, which on some geometry never happens.
- **Effect:** the console hangs.
- **melee-hd:** detects an iteration that repeats an earlier one's starting state and halts there (`mpcoll.c: mpColl_80046904 loops forever`); a loop that drifts instead runs as long as the original, with a watchdog in checks only.
- **Fix:** bound the iterations.
- **Fix impact:** where the original would hang, any outcome is new; elsewhere none.
- **Source:** [engine notes, hangs](engine.md#hangs) (line 304); `crates/melee-sim/src/mp/coll/mod.rs:296`.

### The left wall's corner case tests the ceiling again

- **Where:** the airborne wall corrections' corner case (a wall meeting a ceiling at its top with another wall beyond it), left side, `crates/melee-sim/src/mp/coll/walls.rs` (`corner`).
- **Defect:** after finding the line past the ceiling, the left side tests the ceiling line again for being a left wall, not the line past it.
- **Effect:** a plain ceiling never gets this far, so the correction runs for right walls only; what that changes in play is not recorded.
- **melee-hd:** modeled.
- **Fix:** test the line past the ceiling, as the right side does.
- **Fix impact:** changes airborne collision against such corners; breaks replay compatibility where it happens.
- **Source:** `crates/melee-sim/src/mp/coll/walls.rs:528`.

### Floors running right to left take line -1

- **Where:** `mpColl_8004A45C_Floor`, `mpColl_8004A678_Floor` and `mpColl_8004C328_Ceiling`, `crates/melee-sim/src/mp/coll/ground.rs` and `crates/melee-sim/src/mp/coll/ceiling.rs`.
- **Defect:** they take the floor (or ceiling) at the edge an object stops at; a few floors in the data run right to left (on Great Bay, Pokémon Stadium and Big Blue), and such a floor can fail to reach its own end, when the code takes line -1 with the flags and normal its stack held.
- **Effect:** not recorded.
- **melee-hd:** leaves those two fields as they were and counts a stale read (`mp::coll::CollStatics::stale_reads`).
- **Fix:** handle floors in either direction.
- **Fix impact:** not recorded.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values) (line 60).

### The floor walk's wall length starts from a caller's register

- **Where:** `mpLib_80056C54` (walking a distance along the floor), `crates/melee-sim/src/mp/probe.rs` (`walk_floor`).
- **Defect:** it keeps the running wall length in `f27`, which it never sets before the first line, so a walk whose first line is a wall adds to whatever its caller left.
- **Effect:** for the ReDead (`it_802EA804`, deciding whether the floor ahead is level enough to dodge a hit), none in play: under the item's reaction process f27 holds the boot's 0.0 (`hsd::gobj::PROC_F27`), so a walk starting on a wall (the line it last stood on tilted into one, a dynamic line on Brinstar Depths, Great Bay or Poké Floats) stops on that wall as a reset length would. Other callers not recorded.
- **melee-hd:** modeled: the value is an explicit parameter (`stale_f27`); the ReDead's damage callback passes `PROC_F27` (`it::kinds::leadead::ground_ahead`), and Donkey Kong's port must supply its own.
- **Fix:** reset the length before the first line.
- **Fix impact:** not recorded.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values) (line 55); `crates/melee-sim/src/mp/probe.rs:97`.

### Green Greens links a broken block's last corner through the wrong neighbour

- **Where:** `grGreens_802150C4` (linking the collision of the blocks around a place that broke), `crates/melee-sim/src/game/gr/greens.rs` (`link_corners`).
- **Defect:** of the four diagonal corners, the last (up and right) tests whether the block to the left of the broken place rests where the other three test the block on the corner's own side, then links the corner with the block to the right; at the grid's first place (row 0, column 0) that test reads the 32 bytes before the blocks' array, the heap cell's header.
- **Effect:** the corner's collision is linked to its right-hand neighbour when the left one rests, and left unlinked when only the right one does; what that changes in play is not recorded. The header's first word is a link (an address or null), whose top four bits are never a resting block's 3.
- **melee-hd:** modeled; the read before the array answers "not resting".
- **Fix:** test the block to the right, as the other corners do.
- **Fix impact:** changes Green Greens' block collision where it happens; breaks replay compatibility there.
- **Source:** `crates/melee-sim/src/game/gr/greens.rs` (`link_corners`, `status_at`).

### Flat Zone's oil panic frees its slick only when it has none

- **Where:** `grFlatzone_802176BC`, `crates/melee-sim/src/game/gr/flatzone.rs` (`proc_oil`).
- **Defect:** when the controller starts the oil panic again while the last slick still lasts (state 3), the man's process hides him and means to free the slick's dynamic attribute area, but tests `xCC == NULL` instead of `!= NULL`: it calls `grDynamicAttr_801CA1C0` only with a null area, which returns at once, and otherwise keeps the pointer, which the new pour overwrites.
- **Effect:** the old slick stays slippery until its own timer runs out instead of going when the man comes back. No oracle match restarts the oil panic during its slick.
- **melee-hd:** modeled.
- **Fix:** test `xCC != NULL`.
- **Fix impact:** changes gameplay (the old slick's floor terrain) and breaks replay and Slippi compatibility.
- **Source:** the decompilation's `grflatzone.c` (`grFlatzone_802176BC`); `crates/melee-sim/src/game/gr/flatzone.rs`, the comment in `proc_oil`.
### Mute City's race order reaches the word before it

- **Where:** `grMuteCity_801F1328` (the race), `crates/melee-sim/src/game/gr/mutecity.rs` (`steer_cars`).
- **Defect:** the insertion sort of the cars' order runs its inner loop down to `j == 0` and compares `arr[j]` with `arr[j - 1]`, so it reads and swaps `grMc_8049F440[-1]`, the last word of Brinstar's bubbles (`grZe_8049F170[19].x20`, a GObj pointer on Brinstar, zero from boot otherwise).
- **Effect:** once a car other than car 0 leads the order and has gone further than the car that word names (car 0 for the zero word), the two swap: the word takes the leader's index and the order holds car 0 twice, so the leader drops out of the race's closeness and pack flags until the word comes back. On a console that played Brinstar earlier in the session the word is a pointer and the sort reads a car far past the table.
- **melee-hd:** modeled: the word is `mutecity::Globals::order_before`, zero at a match's start as after boot; the comparer checks it against the trace on Mute City.
- **Fix:** stop the inner loop at `j >= 1`.
- **Fix impact:** changes the cars' flags, speeds and lanes, so their positions and their hits on fighters; breaks Mute City replays.
- **Source:** the port's `steer_cars` and `Globals::order_before`; ssbm-rs's port (`Handle::add(arr, j - 1)` at `j == 0`).

### Mute City's car generator returns a stale register

- **Where:** `grMuteCity_801F2AB0` (a car's smoke), stored by `grMuteCity_801F1A34`, `crates/melee-sim/src/game/gr/mutecity.rs` (`car_generator`, `Car::generator`).
- **Defect:** the function never returns its generator: r3 holds what its last call left, `Ground_801C0498`'s load of `stage_info.x6B0`, the stage parameters' address (0 when the generator can't be made).
- **Effect:** none observable: the car's `gen` word is only compared with null and cleared, and the parameters always exist, so it is non-null exactly when a generator was made.
- **melee-hd:** modeled: `Car::generator` says whether the word holds the parameters' address, which the comparer checks.
- **Fix:** return the generator.
- **Fix impact:** none.
- **Source:** the port's `car_generator`; the decompilation's remark on `grMuteCity_801F2AB0` ("Retail never sets the return value").

### Big Blue's Falcon Flyer looks for the road from a stack vector it never set

- **Where:** `grBigBlue_801E8D64` (the Falcon Flyer's setup), `crates/melee-sim/src/game/gr/bigblue.rs` (`init_flyer`, `flyer_stale_pos`).
- **Defect:** it searches for the road's height below `pos` (`grBigBlue_801EC58C`) before anything writes `pos`, a vector in its frame (`r1 + 0x3C`), so the search runs from the words the stage's setup left there.
- **Effect:** the Falcon Flyer's starting height, and with it the players' start points, follow those words: where they put the search within 500 above or below the road, the Flyer starts at the road's height there plus its parameter's height; elsewhere at the parameter's height alone. On the console (ssbm-rs's interpreter, probed with `WATCH_ADDR`) the setup's hiding of the flying platforms (`HSD_JObjSetFlagsAll` on map 32's tree, just before map 35 is made) leaves two of its joints' addresses in x and y (the registers its calls four levels down save) and `Ground_GetStageGObj` leaves 0 in z: `0x80EF6860`, `0x80EF6900`, 0 under Slippi's playback of `stage-big-blue`. As floats the addresses are tiny negatives, so the search runs at the origin, where the road lies 0.00005 high, and finds it. ssbm-rs's port mode leaves other words there (its ports save no registers), tiny negatives too, so the Flyer starts at the same height in every run.
- **melee-hd:** modeled: `flyer_stale_pos` gives the two joints' addresses from the allocator model, and without it the playback's, which the search can't tell from any other arena word.
- **Fix:** search from the Flyer's own place.
- **Fix impact:** none where the search lands at the origin, as it always does; a fix that searched elsewhere would change the start's heights, which breaks Big Blue replays.
- **Source:** the port's `flyer_stale_pos`; [engine notes, uninitialized values](engine.md#uninitialized-values); the decompilation's `grBigBlue_801E8D64` (`grBigBlue_801EC58C(&pos, NULL, 500.0f)` with `pos` unset) and `HSD_JObjSetFlagsAll`'s machine code.

### Big Blue's relay platform check reads the flying platforms' object

- **Where:** `grBigBlue_801EAB50` (whether the relay platform is near a point), `crates/melee-sim/src/game/gr/bigblue.rs` (`near_relay`).
- **Defect:** it takes map 32's object (the flying platforms) where the relay platform is map 36, and tests the first byte of its variables for the relay platform's flying state (2): that byte is the top byte of the flying platforms' count of placed platforms, which the Falcon Flyer resets.
- **Effect:** the check never finds the relay platform, so the flying platforms and the Falcon Flyer come in, and the Falcon Flyer keeps its speed, without regard to it (the platforms' height still avoids it through `grBigBlue_801EACE8`, which reads map 36).
- **melee-hd:** modeled: `near_relay` reads the flying platforms' count, as the original does.
- **Fix:** take map 36's object.
- **Fix impact:** changes where platforms and the Falcon Flyer come in and when they stop; breaks Big Blue replays.
- **Source:** the port's `near_relay`; the decompilation's `grBigBlue_801EAB50` (`Ground_GetMapGObj(32)`).

### Big Blue's cars start their engines in their collision slot's sound slot

- **Where:** `grBigBlue_801EE398` cases 5 and 6 (a car brought in from a side), `crates/melee-sim/src/game/gr/bigblue.rs` (`bring_car`).
- **Defect:** the engine sound starts in the stage object's sound slot numbered by the car's collision slot (0 to 29), while the volume and every later volume change go to the slot numbered by the car (0 to 3). The stage object has eight sound slots, so a car in a slot past 7 starts no sound.
- **Effect:** a car brought in may drive silently, or its engine plays in another car's slot while the volume changes go to its own; the first cars, placed by `grBigBlue_801EC6C0`, use their car's slot.
- **melee-hd:** modeled: `bring_car` starts the sound in the collision slot.
- **Fix:** start the sound in the car's slot.
- **Fix impact:** changes the cars' engine sounds only.
- **Source:** the port's `bring_car`; the decompilation's `grBigBlue_801EE398`.

### Big Blue's road marks its previous piece 0xFFFF in a seven-bit field

- **Where:** `grBigBlue_801EB004` (the road's layout), `crates/melee-sim/src/game/gr/bigblue.rs` (`lay_road`).
- **Defect:** it stores 0xFFFF, meaning no piece, into the road's previous piece, a seven-bit field, which keeps 127.
- **Effect:** none observable: nothing reads the previous piece.
- **melee-hd:** modeled: the field holds 127, which the comparer checks.
- **Fix:** a field wide enough, or a value that fits.
- **Fix impact:** none.
- **Source:** the port's `lay_road`; the decompilation's `grBigBlue_801EB004` (`prev_lane = 0xFFFF`).

### Big Blue's platforms, Falcon Flyer and relay platform turn a point behind them and drop it

- **Where:** `grBigBlue_801E6C60` (the flying platforms), `grBigBlue_801E93D8` (the Falcon Flyer), `grBigBlue_801EA05C` (the relay platform), `crates/melee-sim/src/game/gr/bigblue.rs` (`fly_platform`, `proc_flyer`, `proc_relay`).
- **Defect:** each turns a point ahead and a point behind by the road's slope (`lbVector_ApplyEulerRotation`) and adds its place to both, then uses only the point ahead.
- **Effect:** none observable: the point behind is computed and dropped.
- **melee-hd:** modeled (computed and dropped).
- **Fix:** leave it out.
- **Fix impact:** none.
- **Source:** the port's `fly_platform`, `proc_flyer` and `proc_relay`.

### Big Blue's flying platforms keep their items' GObjs, which nothing reads or clears

- **Where:** `grBigBlue_801E8A1C` (a flying platform's item), `grBigBlue_801E6C60` (its placing), `crates/melee-sim/src/game/gr/bigblue.rs` (`platform_item`, `place_platform`).
- **Defect:** the platform stores the item `it_8026BE84` made in its `x50` (an `Item_GObj*` as an integer), which no code reads; it's cleared only when the platform is placed again, so once the item is picked up, used up or gone it holds a freed GObj's address, which a later object may take.
- **Effect:** none observable: nothing reads `x50`.
- **melee-hd:** modeled: `x50` holds the item's handle, which the comparer pairs with the original's address.
- **Fix:** leave the field out, or clear it when the item goes.
- **Fix impact:** none.
- **Source:** the port's `platform_item`; the decompilation's `grBigBlue_801E8A1C` (`data[idx].x50 = (s32) it_8026BE84(&spawn)`) and `grBigBlue_801E6C60` (`data[i].x50 = 0`), the only other writes, and no read.

### A collision's setup leaves two flag bytes' bits as the heap held them

- **Where:** `mpColl_80041EE4` (a collision's setup), `crates/melee-sim/src/mp/coll/mod.rs` (`X34Flags`, `X35Flags`).
- **Defect:** it clears `x34`'s other bits but leaves `b5` (a substep met something that ends the move) and `x35`'s low seven bits as the block held them.
- **Effect:** none observable: only `mpColl_80043754`'s substep loop reads `b5`, after clearing it, and no code reads `x35`'s low bits (it uses only the top one). On Corneria's oracle match a link of Sheik's chain new from the heap held `b5` set and 0x7F in `x35` (tick 11950).
- **melee-hd:** stale: `b5` is kept but not compared, and `x35` is modeled as its top bit alone, both spliced in by `tools/structgen`.
- **Fix:** clear both bytes.
- **Fix impact:** none in play.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values).

### The Star Fox team's window writes into its freed text

- **Where:** `smashTaunt_801E24AC` (the talk window's render callback, Corneria and Venom), `crates/melee-sim/src/game/gr/corneria.rs` (`window_render`).
- **Defect:** once a line closes its font (`HSD_SisLib_803A5F50`), which frees the line's text, the window keeps its pointer and writes the text's `hidden` byte into the freed block every display pass until the window goes.
- **Effect:** none observable: the next text made in that block (the next line's) writes the byte before anything reads it.
- **melee-hd:** the write to a freed text is skipped.
- **Fix:** clear the window's text pointer when the font closes.
- **Fix impact:** none in play.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values).

### Venom's Great Fox can't hide its joints

- **Where:** `grVenom_8020454C` (the Great Fox's process), `crates/melee-sim/src/game/gr/venom.rs` (`proc_ship`).
- **Defect:** each of its seven joints is to show while its depth lies between -60,000 and 1,000, but the test hides it only when its depth is neither below 1,000 nor above -60,000, which no number is: the two bounds are joined the wrong way.
- **Effect:** the joints always show (a NaN depth alone would hide one).
- **melee-hd:** modeled, NaN included.
- **Fix:** hide a joint past either bound.
- **Fix impact:** would change what Venom draws in the distance; no gameplay.
- **Source:** `crates/melee-sim/src/game/gr/venom.rs` (`proc_ship`).

### Princess Peach's Castle: a Bullet Bill keeps pointing at its freed blaster

- **Where:** `grCastle_801CE578` (a Bullet Bill's process), `crates/melee-sim/src/game/gr/castle.rs` (`proc_bill`).
- **Defect:** when the Bullet Bill's flight ends it destroys its blaster (`Ground_801C4A08(gp->u.castle11.xCC)`) but leaves `xCC` holding the freed GObj's address, and every later tick reads it, comparing it with null to decide whether a blaster is still due.
- **Effect:** none observable: the dangling pointer is only compared with null, never followed, and being non-null it keeps the blaster from coming back, as a cleared field with a done flag would; the Bullet Bill goes once it has lingered.
- **melee-hd:** modeled: the Bullet Bill keeps the freed blaster's handle, and the comparer pairs it by the freed slot.
- **Fix:** clear `xCC` after destroying the blaster (the flight's done flag already keeps it from being made again).
- **Fix impact:** none in play.
- **Source:** `crates/melee-sim/src/game/gr/castle.rs` (`proc_bill`, "The blaster goes, but the Bullet Bill keeps pointing at it").

### Brinstar's bubbles' timer starts from the heap

- **Where:** `grZebes_801D881C` (the stage's process) and its setup (`grZebes_801D8644`, which writes `grZebes_GroundVars5`'s other fields), `crates/melee-sim/src/game/gr/zebes.rs`.
- **Defect:** the setup never writes the timer (`*(s16*) &xF8`), which the process counts down from the first tick, calling `grZebes_801DAA08` and drawing `HSD_Randf` for the next period once it passes zero; the first period is whatever the stage object's block held.
- **Effect:** on a console the first bubbles' time, and every random draw after it, depend on the heap's history: the main menu's attract match on Brinstar starts the timer from 2093 (the menus' bytes), where a fresh heap's 0 (and Slippi's zeroed block) draws on the match's second tick.
- **melee-hd:** modeled: a session from power-on starts it from the heap's image (`game::leftovers`, the `Ground` read from it: 2093 in the main menu's attract match, as the console's), a replay from 0, as Slippi's playback does.
- **Fix:** set the timer in the setup.
- **Fix impact:** changes Brinstar's random draws in every match; replays (whose blocks are zeroed) keep theirs only if the fix sets 0.
- **Source:** [engine notes, a session's heap leftovers](engine.md#uninitialized-values).

### Fountain of Dreams' star keeps its alpha flag as the heap held it

- **Where:** `Ground_801C1A20` (a stage object from a joint), called only by `grIzumi_801CCB18` (the star, `GrdIzumiStar_TopN_joint`), `crates/melee-sim/src/game/gr/setup.rs` (`stage_gobj_from_joint`).
- **Defect:** unlike `Ground_GetStageGObj`'s objects, it leaves `x10_flags`' `b6` and `b7` as the block held them (`b4` the overlay's reset writes).
- **Effect:** on a console, when the leftover `b6` is set, the star's materials blend with the alpha constant in `xC0`, also the heap's (`fn_801C8EF8`'s TEV setup); `b7` only Princess Peach's Castle reads.
- **melee-hd:** modeled: a session from power-on takes the bits from the heap's image (the `Ground` read from it), a replay has them clear, as Slippi's zeroed block.
- **Fix:** clear `b6` and `b7` as the other creation does.
- **Fix impact:** changes the star's look only where the heap left the bit set.
- **Source:** [engine notes, a session's heap leftovers](engine.md#uninitialized-values).

### A collision joint's stamp starts as the heap held it

- **Where:** `Ground_UpdateMapColl` (`Ground_804D6954`, the update counter), `crates/melee-sim/src/game/gr/ground.rs`.
- **Defect:** the stage's collision load leaves each joint's stamp (`CollJoint::xC`) unwritten, and the update skips a joint whose stamp already equals the counter it just raised.
- **Effect:** a leftover equal to the counter skips that joint's collision update once; not recorded in play.
- **melee-hd:** modeled: a session from power-on takes the stamps from the heap's image (`mp`'s load, `game::leftovers::bytes`), a replay at 0, as a fresh heap.
- **Fix:** clear the stamps at the load.
- **Fix impact:** none unless a leftover matched.
- **Source:** [engine notes, a session's heap leftovers](engine.md#uninitialized-values).

## Camera

### Brinstar's bubbles play their sound through the stage's object as if it were its `Ground`

- **Where:** `grZebes_801DB088` (a bubble's tick), which `grZebes_801DB3CC` calls with the stage's GObj where `Ground_801C5440` wants the stage object's `Ground`, `crates/melee-sim/src/game/gr/zebes.rs` (`pop_sound`).
- **Defect:** a bubble falling apart plays its sound in "slot 0" of what it takes for the stage object's sounds, which is the GObj's word at 0x20: the high half of its GX link mask (`gxlink_prios`). The old word is stopped as a sound handle unless it's -1, and the new sound's handle is written there.
- **Effect:** a non-camera object's link mask is read by nothing, so the slot works as one, with one exception: the word starts at 0, not -1, so the first bubble to fall apart stops whatever sound holds handle 0. What that cuts off in play isn't recorded.
- **melee-hd:** modeled: the handle lives in the stage GObj's mask's high half.
- **Fix:** pass the stage object's `Ground`.
- **Fix impact:** changes which sound the first falling bubble stops; nothing else.
- **Source:** `crates/melee-sim/src/game/gr/zebes.rs` (`pop_sound`, `tick_bubble`); the decompilation's `grzebes.c`.

### The cutscene camera's orbit reads unwritten stack slots

- **Where:** `Camera_8002E234` (the cutscene camera's orbiting eye), `crates/melee-sim/src/game/camera.rs`.
- **Defect:** it eases only the parts of the orbit its script set and reads the others (pitch, yaw or distance) from stack slots it never wrote. Master Hand's and Crazy Hand's entrances set orbits that way.
- **Effect:** not recorded.
- **melee-hd:** stops, until a probe of those slots in play decides the model.
- **Fix:** keep the parts the script didn't set from the camera's current orbit.
- **Fix impact:** cutscene camera only; not recorded whether gameplay reads it.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values) (line 54); `crates/melee-sim/src/game/camera.rs:2920`.

### The background color's alpha is unset

- **Where:** `Camera_GetBackgroundColor`, `crates/melee-sim/src/game/camera.rs` (`background_color`).
- **Defect:** it returns red, green and blue and leaves alpha unset.
- **Effect:** none observable in the checks (only red, green and blue are compared).
- **melee-hd:** returns the three channels.
- **Fix:** set the alpha.
- **Fix impact:** none.
- **Source:** `crates/melee-sim/src/game/camera.rs:1453`; `tools/leaf-check/tests/camera.rs:1322`.

## HUD and audio

### An announcement tests its first line twice

- **Where:** `ifStatus_802F6EA4` (showing an announcement, kind 8), `crates/melee-sim/src/game/hud/announce.rs` (`announce`).
- **Defect:** before playing the second announcer line it tests the first line's id again, not the second's.
- **Effect:** the second line plays whenever the first does, and never when the first is absent; audio only.
- **melee-hd:** modeled.
- **Fix:** test the second line's id.
- **Fix impact:** audio, and the voices sounds take, which are state; not recorded further.
- **Source:** `crates/melee-sim/src/game/hud/announce.rs:62`.

### The damage displays' reset depends on a caller's register

- **Where:** `ifStatus_802F66A4` (taking the damage models from `IfAll.dat`), `crates/melee-sim/src/game/hud/status.rs` (`load_damage_models`).
- **Defect:** it clears the displays only when its caller's `r30` isn't zero.
- **Effect:** none observable: its one caller, `ifAll_802F390C`, holds the scene's light description there.
- **melee-hd:** clears them.
- **Fix:** clear unconditionally.
- **Fix impact:** none.
- **Source:** `crates/melee-sim/src/game/hud/status.rs:620`.

### A second KO mark reads the display before the first

- **Where:** `fn_802F770C` (the KO marks' render callback), `crates/melee-sim/src/game/hud/announce.rs` (`ko_mark_render`).
- **Defect:** it finds the mark's slot by searching the slots' first marks only (the inline `GetSlot`), so for a slot's second mark the search fails and returns -1, and the callback tests `ifStatus_HudInfo.players[-1].flags.hide_all_digits`, 0x64 bytes before the displays: the padding after `ifAll_804A0FD8` in `.bss`, which nothing writes.
- **Effect:** the second mark is always drawn, even when its slot's display hides its digits; presentation only.
- **melee-hd:** modeled: a mark on no slot's first is drawn.
- **Fix:** search the second marks too (or keep the slot on the mark).
- **Fix impact:** presentation only; no gameplay, replay or Slippi compatibility change.
- **Source:** `crates/melee-sim/src/game/hud/announce.rs:443`; the Pokémon Stadium items-shoot match ([the oracle](oracle.md#shooting-and-thrown-items)).

### The music stream's clock reads past its boxes once a song has ended

- **Where:** `HSD_SynthPStreamMasterClockCallback`, `crates/melee-sim/src/audio/stream.rs` (`stream_frame`, `hako_or_past`).
- **Defect:** each audio frame the clock works out which of the stream's three boxes (`pstHakoHeader`) the playing voice is in from its current address. Once a song that doesn't loop has played its last box, its voice stops in the silence after it (`HSD_Synth_804D7784`), whose address lies before the stream's buffer, so the box comes out about 0x7FA0 and the clock reads that "box's" header 0xFF400 bytes past the three, in the heap, and sets the stopped voice's end address from it (`AXSetVoiceEndAddr`).
- **Effect:** none audible: the voice is stopped, and the next sound that takes it sets its whole address block. The special messages' songs (0x43 to 0x45) end this way; so does any song that doesn't loop.
- **melee-hd:** modeled: the header is read from the heap's image ([the engine notes](engine.md#uninitialized-values)); a session stops where the image doesn't know those bytes, and outside a session (no image) so does the run.
- **Fix:** stop the clock's work when the voice has stopped, or bound the box.
- **Fix impact:** none audible; the stopped voice's end address changes.
- **Source:** `synth.c`'s `HSD_SynthPStreamMasterClockCallback`; the Data session `prize`'s tick 560, and the results' fanfares (`rules-match`'s results screen, tick 8208).

## Engine and HSD

### The expression argument buffer outlives its heap

- **Where:** `expEvaluate` (`sysdolphin/baselib/robj.c`), `crates/melee-sim/src/hsd/robj.rs` (`evaluate`).
- **Defect:** the first expression evaluated allocates the static `arg_buf` (100 floats) from the current heap, and nothing ever clears the pointer. Each scene makes its heap anew, so after a scene change the buffer points into the new scene's heap, at whatever that has put there, and every expression's arguments are written over it.
- **Effect:** not recorded: under Slippi's playback nothing evaluates an expression before the match, so the buffer comes from the match's own heap (in the items-on replays, a Fire Flower's model runs the first expression).
- **melee-hd:** modeled within the match (`RObjs::arg_buf`, the allocation replayed); a match after scenes that evaluated an expression would find the buffer already allocated in the original, which melee-hd doesn't model (it allocates one).
- **Fix:** clear `arg_buf` when the heap is made anew, or take it from a heap that lasts.
- **Fix impact:** none where the buffer comes from the match's heap; elsewhere it changes the heap's layout, and addresses reach gameplay in a few places ([engine notes](engine.md#uninitialized-values)).
- **Source:** [engine notes, the allocators](engine.md#the-allocators).

### An FObj channel hands over a stack word

- **Where:** `FObjUpdateAnim`, `crates/melee-sim/src/hsd/fobj.rs` (`Value::Stale`).
- **Defect:** a channel whose data ends right after its first key reaches a state with no case for its interpolation, and the code hands its update function whatever its stack holds where the value would go.
- **Effect:** none observable so far: probed over every VS stage and a Final Destination replay, the word was always a RAM address (a tiny negative float), and every channel was a joint's branch visibility, which an address hides.
- **melee-hd:** update functions resolve what any address gives where that doesn't depend on which address, and count the rest as stale reads.
- **Fix:** hold the first key's value.
- **Fix impact:** none where an address hides the branch, as in every case seen.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values) (line 51); `crates/melee-sim/src/hsd/fobj.rs:28`.

### Dynamic bones write a stack word into a joint's w

- **Where:** `lb_8001044C` (a dynamic bone chain's swing), `crates/melee-sim/src/lb/dynamics.rs`.
- **Defect:** it writes a joint's rotation from a stack `Quaternion` whose x, y and z come from `HSD_QuatLib_8037EB28` and whose w it never sets, so the joint's `rotate.w` becomes a stack word (usually 0, sometimes an address, 0x43300000 or a float).
- **Effect:** none observable: the joint isn't a quaternion joint, so nothing reads that w.
- **melee-hd:** stale (`JObj::stale_w`); a quaternion read of it, or a dynamics link taking such a joint's rotation (`lb_8000FD48`), counts a stale read.
- **Fix:** set w, or write Euler angles.
- **Fix impact:** none.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values) (line 61).

### Dynamic bones read an unset crossing point

- **Where:** `lb_8001044C` probing with `lb_800103D8` on a flat floor (`x2228_b1`), `crates/melee-sim/src/lb/dynamics.rs`.
- **Defect:** `lb_800103D8` reports a crossing without writing its point when the probe is level and above the floor, and the chain then reads the unset point.
- **Effect:** not recorded; only a link lying exactly level above the floor's height gets there.
- **melee-hd:** stops.
- **Fix:** write the point, or report no crossing.
- **Fix impact:** not recorded.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values) (line 62).

### An IK turn reads a rotation as a quaternion

- **Where:** `fn_8002113C`, `crates/melee-sim/src/game/ft/ik.rs` (`turn_about`).
- **Defect:** for an Euler joint it reads the new rotation as a quaternion into a vector, so its w comes from the matrix after it on the stack.
- **Effect:** not recorded.
- **melee-hd:** modeled (w taken from that matrix element).
- **Fix:** write the Euler angles alone.
- **Fix impact:** not recorded.
- **Source:** `crates/melee-sim/src/game/ft/ik.rs:290`.

### Figatree frames end a chain through an unset pointer

- **Where:** `fn_8001E60C` (FObjs for a figatree's non-translation tracks), `crates/melee-sim/src/lb/figatree.rs`.
- **Defect:** with no FObj made, it ends the chain through a pointer it never set.
- **Effect:** not recorded.
- **melee-hd:** counts a stale read.
- **Fix:** end the chain with null.
- **Fix impact:** not recorded.
- **Source:** `crates/melee-sim/src/lb/figatree.rs:59`.

### The TEV compiler compares the wrong swap

- **Where:** `SimplifySrc` (part of `HSD_MObjCompileTev`), `crates/melee-sim/src/hsd/texp/dag.rs`.
- **Defect:** simplifying a rasterized source, it compares the stage's rasterized swap with the source's texture swap, and takes the texture swap.
- **Effect:** not recorded (a material's compiled TEV is simulation state).
- **melee-hd:** modeled.
- **Fix:** compare the rasterized swaps.
- **Fix impact:** not recorded.
- **Source:** `crates/melee-sim/src/hsd/texp/dag.rs:363`.

### TEV constants go out with components from the stack

- **Where:** `HSD_TExpSetReg` (texp.c), under every material's setup (`HSD_TExpSetupTev`) and the fighters', items' and stage objects' overlay stages (`ftMaterial_800BF534`, `ftMaterial_800BF6BC`, `it_80277F90`, `it_80278108`, `fn_801C8EF8`), so under every render callback that draws a model, Poké Floats' `fn_802130D0` and Peach's Castle's `fn_801D0924` among them; `crates/melee-render/src/hsd/texp.rs` (`set_reg`, `set_reg_values`).
- **Defect:** its eight registers are an uninitialized stack array (`reg[8]` at `r1 + 0x18`); it writes only the components its constants name, then sends each register it touched whole (`0x803853A8` to `GXSetTevKColor`, `0x803853F8` to `GXSetTevColor`), and an RGB `HSD_TE_U8` constant keeps the slot's alpha (`0x80384FE0`). The rest is what earlier calls left: on Poké Floats, C0's color is `&hsdTObj` (`0x80405570`), `GXLoadTexObj`'s save of its caller's `r30`.
- **Effect:** none observable: every stage reads only the components its constants set. The one path that would read one, a stage object's color overlay in K0 together with its alpha (`x10_flags.b6`), needs command 21 of a stage color script, which no ported stage's match nor Peach's Castle's oracle match reaches.
- **melee-hd:** stale: it sends 0 there (`texp::Regs::set`), marks those components unset in GX (`Gx::unset`), and notes a draw that reads one (`frame::UNSET_READ`), which fails the render checks; command 21 stops (`gr::material::run_overlay`).
- **Fix:** start the registers at zero, or send only the components set.
- **Fix impact:** none, as long as no stage reads an unset component; the overlay-and-alpha path would draw differently.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values) (line 105); `crates/melee-render/src/hsd/texp.rs:102`.

### A palette load's address goes out with bits from the stack

- **Where:** `GXInitTlutObj` (GXTexture.c) on a stack `GXTlutObj`: `HSD_TObjSetup`'s for a color-indexed texture's palette (tobj.c, the object at `r1 + 0x58` of its 0x90-byte frame) and `psDispParticles`'s (`gx_tlut_obj`), so under every render callback that draws a color-indexed texture, Peach's Castle's `fn_801D0924` and Poké Floats' `fn_802130D0` among them; `GXLoadTlut` sends it. `crates/melee-render/src/gx/texture.rs` (`init_tlut_obj`, `load_tlut`), with `crates/melee-render/src/hsd/draw.rs` and `hsd/psdisp.rs` as the callers.
- **Defect:** it clears the object's first word but sets only the low 21 bits (the address, `lwz` at `0x8033F044`, `clrrwi r4, r4, 21`) and the top byte (0x64, the BP register) of `loadTlut0`, so bits 21 to 23 keep what the caller's stack held there, and `GXLoadTlut` writes the word to BP register 0x64 whole. `GXLoadTlut` then copies the object, those bits and the two padding bytes after `numEntries` with it (`lwz` at `0x8033F0E8`), into the palette region (`r->tlutObj = *t`). What the slot holds is what earlier calls at that depth left: a `WATCH_ADDR` probe of ssbm-rs's interpreter on `stage-peachs-castle` finds `0x804EE614` written by `HSD_TObjSetupVolatileTev`'s `stmw r25` (a material's setup drawn just before, which leaves the heap pointer `0x80DC01A0`, so bits 21 to 23 are 6), and the store at `GXInitTlutObj + 0x3C` keeps them for the next palette set up in the same slot. Over 400 fields of that match (`GX_TRACE_BP=64`) 29,063 palette loads went out with all eight values there, 0 on 20,570 of them; over 1,500 fields of `stage-poke-floats`, 16,276 loads, 0 on 7,818.
- **Effect:** none observable: nothing reads those bits. A palette load (BP register 0x65) takes its source from the address field below them: the SDK's own field is 21 bits (`GXGetTlutObjData`), ssbm-rs's and melee-gx's loads mask to it, Dolphin's keeps only the GameCube's physical address, and on the console the game's palettes load from the right place while all eight values go out. No other GX register takes BP 0x64, and the region's copy is read again only for its `tlut` word (`GXLoadTexObj`).
- **melee-hd:** stand-in: the callers pass a cleared object (`TlutObj::default()`), so 0 goes out there. melee-gx's `palette_loads_ignore_the_bits_above_the_address` test holds the load to the address field (widening the mask to 24 bits fails it); the render checks leave BP 0x64 out of the draws they compare and compare palettes by content, and melee-gx's register hashes treat it as a trigger. With the bits forced to 6 Peach's Castle's and Poké Floats' frame checks pass unchanged, and with the address one line off all four fail.
- **Fix:** clear `loadTlut0` before setting its fields, as the first word is.
- **Fix impact:** none.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values); `crates/melee-render/src/gx/texture.rs:420`; `crates/melee-gx/src/lib.rs` (the test); the decompilation's `GXInitTlutObj` and `GXLoadTlut` and their machine code.

### A material's textures share a palette by its size alone

- **Where:** `DifferentTluts` (tobj.c), called by `HSD_TObjSetup` as it names each color-indexed texture's palette in a material's chain; `crates/melee-render/src/hsd/draw.rs` (the names as the draw sets them up) and `crates/melee-sim/src/hsd/export.rs` (`tlut_names`, the names the heap's image keeps).
- **Defect:** it should tell two palettes apart by their tables or their sizes, but compares the first palette's table with itself (`t0->lut != t0->lut`, false), so two palettes of the same entry count count as one: the second texture takes the first's name, and its own palette is never loaded.
- **Effect:** in a material with two color-indexed textures whose palettes differ but have as many entries, the second draws with the first's colors; not recorded in play. The names it writes (`HSD_Tlut::tlut_name`) stay in each palette's block, so the heap's image after a scene holds them.
- **melee-hd:** modeled: the draw shares a name where the entry counts match, and the image writes the names as each setup does.
- **Fix:** compare `t0->lut` with `t1->lut`.
- **Fix impact:** picture only, in such a material; no gameplay, replay or Slippi compatibility change (a palette's name in a freed block is a leftover no match reads).
- **Source:** the decompilation's `DifferentTluts` (`MUST_MATCH`); `crates/melee-sim/src/hsd/export.rs` (`tlut_names`).

## Saves and the memory card

### A new card file's blocks run past their files

- **Where:** `fn_803B1338` (a new card file's blocks, `sysdolphin/baselib/card.c`), `crates/melee-save/src/hsd.rs` (`write_new_blocks`).
- **Defect:** it queues every block of a file with data at a whole block's length (`sector_size - 0x20`) from the file's data, whatever the file's size, so each file's last block takes the bytes that follow the file in memory. The writes after (`fn_803AE7F8`, `fn_803AF3F0`, `fn_803B0120`) give the last block only the bytes left.
- **Effect:** none observable in play: a read copies only the file's size. The card keeps what followed each file when the save was made: after the save data the first 0x850 bytes of the first name tag bank, after each bank the next one's first 0xB4, and after the seventh bank 0xB4 bytes of whatever `gmMainLib`'s data holds after the card data; a snapshot's last block, what follows the snapshot. A later write that changes a file gives its blocks zeros there, so a save's banks at sequence 0 keep the bytes for good (1,143 such blocks in the card corpus).
- **melee-hd:** modeled: melee-save's create writes what the caller puts after each file (`SaveData::after` for the bytes after the banks), and the corpus's three freshly made saves come out of a format and a create byte for byte.
- **Fix:** give each file's last block the bytes left, as the writes do.
- **Fix impact:** changes only what a new card file holds past its files; no gameplay.
- **Source:** [saves](saves.md); `crates/melee-save/src/hsd.rs` (`write_new_blocks`).

### The spare blocks' file table names file 9

- **Where:** `fn_803B1338` writing the spares, through `fn_803AC3F8` (a block's file table), `crates/melee-save/src/state.rs` (`pack_table`).
- **Defect:** the spares are written with the file index the loop over files 1 to 8 ends on, 9, so their table covers files 7, 8 and 9, and entry 9 reads `file_sizes[9]` and `file_flags[9]`, one past the nine-entry arrays: `file_data[0]`, file 0's data address, as a size, and `file_sizes[0]` as flags.
- **Effect:** none observable for the game's files: Melee's save has no file 0, so the entry is `09 00 00 00` on every card. A layout with both file 0 data and spares would write file 0's address into the table, and a scan would take it in as file 9's size, written past the arrays again (`hsd_803AC558`).
- **melee-hd:** modeled: melee-save keeps the layer's state as the original's bytes, so the read lands where the original's does.
- **Fix:** write the spares with a file of their own or none.
- **Fix impact:** none for Melee's files.
- **Source:** [saves](saves.md); `crates/melee-save/src/state.rs` (`pack_table`).

### The directory's and table's check codes compare without wrapping

- **Where:** `VerifyDir` and `VerifyFAT` (`dolphin/card/CARDCheck.c`, the CARD library Melee links), `crates/melee-save/src/card.rs` (`verify_dir`, `verify_fat`).
- **Defect:** the newer of two copies is the one whose check code is higher, the two `s16` codes subtracted as ints rather than as `s16`, so when a code passes from 32,767 to -32,768 the newest copy reads as the older.
- **Effect:** at that write, the next mount takes the directory (or table) as it was before it: the write's change is lost (a file's time, or a file made or deleted then), and later writes go on from there. Every save that changes something writes the directory at least twice (each `CARDWrite` stamps the file's time), so a card's directory gets there after some 16,000 such saves; the table only on creates and deletes.
- **melee-hd:** modeled.
- **Fix:** compare `(s16)(a - b)`.
- **Fix impact:** none in play; a card at the wrap keeps its last write.
- **Source:** [saves](saves.md); `crates/melee-save/src/card.rs` (`verify_dir`, `verify_fat`).

## Menus and the attract loop

### The attract match's rumble is worked out from the last match's players and dropped

- **Where:** `onEnterVs` (`gmopeningmode.c`), `crates/melee-sim/src/game/gm/attract.rs` (`opening_on_enter_vs`).
- **Defect:** the entry calls `gm_LoadRumbleEnabled` on the start data before it resets the players (`gm_SetupAllPlayerDefaults`), so the rumble it works out is the last attract match's players' (or, the first time, the zeroed data's: human players with name tag 0, whose rumble it looks up in the save's name tags), and the reset overwrites it at once.
- **Effect:** none observable: the reset sets every player's rumble again, and the lookups have no side effects.
- **melee-hd:** the call is left out, with this proof in the port's comment.
- **Fix:** call `gm_LoadRumbleEnabled` after the players are set up, or drop it (every attract player is a computer, whose rumble is off).
- **Fix impact:** none.
- **Source:** `crates/melee-sim/src/game/gm/attract.rs` (`opening_on_enter_vs`).

### A cursor's unhover reads its arrow's frame and drops it

- **Where:** `mn_8022A440`, `crates/melee-sim/src/game/mn/main.rs` (`cursor_unhover`).
- **Defect:** the cursor's unhover reads its arrow's animation frame (`mn_8022F298(sp24[2])`) and throws it away, then sets the arrow's unhovered loop's start; the hover (`mn_80229F60`) uses the same read to carry the arrow's place over.
- **Effect:** none observable: the read has no side effects.
- **melee-hd:** modeled (the read is kept).
- **Fix:** drop the read.
- **Fix impact:** none.
- **Source:** `crates/melee-sim/src/game/mn/main.rs` (`cursor_unhover`).

### The rumble screen reads its title before writing it

- **Where:** `mnVibration_CreateScreen`, `crates/melee-sim/src/game/mn/vibration.rs` (`vibration_make_screen`).
- **Defect:** the screen's user data (`MnVibrationData`, 0x98 bytes from `HSD_MemAlloc`) is never cleared, and the creation tests its `title_text` (`+0x90`) and frees the text it names (`HSD_SisLib_803A5CC4`) before writing it, so it frees whatever text the heap block's last occupant left a pointer to there.
- **Effect:** none observable in the sessions: the block was fresh (zero) memory each time. Were a live text to sit where a freed title was (a text made in its slot of the text arena since), the original would free it, a text another screen still shows.
- **melee-hd:** modeled where melee-sim knows the word: the last rumble screen's block and its title (`Vibration::left`), freed again when the new block is the same; any other block's word names no text ([the engine notes](engine.md#uninitialized-values)).
- **Fix:** clear `title_text` with the other texts.
- **Fix impact:** none in play.
- **Source:** `mnvibration.c`'s `mnVibration_CreateScreen`; `crates/melee-sim/src/game/mn/vibration.rs` (`vibration_make_screen`).

### The sound screen reads its description from freed user data

- **Where:** `fn_80249A1C`, `crates/melee-sim/src/game/mn/sound.rs` (`sound_think`).
- **Defect:** when the flow leaves the screen, its process frees the screen's GObj, its user data with it (`HSD_Free`), then reads the description's text from that user data to free it.
- **Effect:** none observable: nothing allocates between the two, so the freed block still holds the pointer.
- **melee-hd:** the text is read before the GObj goes, the same text freed.
- **Fix:** read the text first, or free it before the GObj.
- **Fix impact:** none.
- **Source:** `mnsound.c`'s `fn_80249A1C`.

### Erasing all data frees the sound bank under a playing sound

- **Where:** `lbAudioAx_80028690` and `lbAudioAx_80027AB0` (`AXDriver_8038DCFC`), `crates/melee-sim/src/lb/audio/boot.rs` (`driver_replace_bank`) and `crates/melee-sim/src/audio/driver.rs` (`driver_stream_word`).
- **Defect:** a change of language frees the sound bank (`smash2.sem`) and loads the other language's, while the driver's voices keep their command streams as pointers into the old block. `lbAudioAx_80027AB0` keys every sound off first, which stops the effects but leaves a voice's stream running; `lbAudioAx_80028690` keys nothing off unless the audio was never set up. Erase All in Japanese reaches it twice in one tick: the save's reset puts the language back to English (`lbAudioAx_80028690`: the English bank), then the screen sets it back to Japanese (`lbAudioAx_80027AB0`: the Japanese bank again), with the confirming sound (0xBD) still running.
- **Effect:** the playing voice reads its next commands from the freed block: the old bank's bytes, while nothing has written over them, or the new bank's at an unrelated offset where the new block covers the old. In the `lang` session the confirming sound's next command is read from the freed Japanese bank during the English bank's load, and the voice is then taken for Erase All's sound (0xBF), so nothing is heard of it. A stream reading a new bank's bytes as commands could play or stop any sound.
- **melee-hd:** modeled: a voice's stream stays an address, and a read outside the current bank reads the freed bank that held the address (`Driver::freed_banks`).
- **Fix:** key every voice off, streams too, before freeing the bank.
- **Fix impact:** none in the sessions: the stale read's command is the confirming sound's own.
- **Source:** `lbaudio_ax.c`'s `lbAudioAx_80028690` and `lbAudioAx_80027AB0`, `axdriver.c`'s `AXDriver_8038DCFC`.

### The sound screen asks for the rules and drops them

- **Where:** `mnSound_80249C08`, `crates/melee-sim/src/game/mn/sound.rs` (`sound_make`).
- **Defect:** the screen's creation calls `gmMainLib_GetGameRules` and drops what it returns.
- **Effect:** none: the call reads nothing and has no side effects.
- **melee-hd:** the call is left out.
- **Fix:** drop the call.
- **Fix impact:** none.
- **Source:** `mnsound.c`'s `mnSound_80249C08`.
### Hovering the item switch or the extra rules copies an unwritten byte into the menu's value

- **Where:** `fn_8022F538` and `mn_80230E38` (`mnmainrule.c`), `fn_8023201C` and `mn_80233218` (`mnruleplus.c`); `crates/melee-sim/src/game/mn/rules.rs`, `rule_plus.rs`.
- **Defect:** the rules' list data's bytes `x7` and `x8` (`indexed.values[5]` and `[6]`) and the extra rules' `x7` (`rule_values.values[5]`) are never written (the lists' setups write the rules and the stock count only), but hovering the item switch, the extra rules or the random stage switch, or coming back to a list from one of them, copies the hovered option's value from them into the menu's value (`mn_804A04F0.confirmed_selection`), which then holds what the heap block held there.
- **Effect:** none observable: on those options left and right return before the value is read, the list's change test compares the byte with itself, A opens a screen that sets the value anew, and up and down set it from a written option.
- **melee-hd:** stale (`RulesMenus::confirmed_stale`): the comparer leaves the value out while it holds the byte, and a read of it stops ([the engine notes](engine.md#uninitialized-values)).
- **Fix:** write the bytes (zero) when a list is made.
- **Fix impact:** none.
- **Source:** [engine notes](engine.md#uninitialized-values).

### An item's arrow starts from a register's leftover after the frequency

- **Where:** `mnItemSw_8023453C` (`mnitemsw.c`), `crates/melee-sim/src/game/mn/item_switch.rs` (`items_update`).
- **Defect:** moving from one item to another carries the hover arrow's frame over (`anim_val`, read from the old item's arrow); moving from the frequency to an item never sets it, so the new item's arrow starts at whatever `f30` held: the boot's 0.0, since nothing from `main` down to the process writes `f30`.
- **Effect:** the arrow starts from its first frame there instead of carrying its place over; always the same, so nothing varies.
- **melee-hd:** modeled (`item_switch::PROC_F30`).
- **Fix:** start the arrow from its loop's start.
- **Fix impact:** none (the same frame).
- **Source:** [engine notes](engine.md#uninitialized-values).

### The item switch's last bit spills into the mask's upper half

- **Where:** `mn_8022E978` (`mnmain.c`), `crates/melee-sim/src/game/mn/item_switch.rs` (`set_item_enabled`).
- **Defect:** shifts an `int` 1 by the item's bit and widens it to the preferences' 64-bit item mask, so bit 31 (the item in place 22) sign-extends: turning it on sets bits 31 to 63, turning it off clears them all.
- **Effect:** none on play: the mask's readers (`gm_8016403C`, `fn_801640B0`, the VS start data's setup in `gm_1601.c`) read bits 0 to 31 only; the upper half goes into the save as it stands.
- **melee-hd:** modeled.
- **Fix:** shift a 64-bit 1.
- **Fix impact:** the save's bytes only.
- **Source:** `crates/melee-sim/src/game/mn/item_switch.rs` (`set_item_enabled`).

### The item switch reads its texts from its freed user data

- **Where:** `fn_80234C24` (`mnitemsw.c`), `crates/melee-sim/src/game/mn/item_switch.rs` (`item_switch_proc`).
- **Defect:** at the end of its way out it frees its GObj, and with it its user data (`HSD_Free`), then reads its two texts from that data to free them.
- **Effect:** none observable: nothing allocates in between, and the free leaves the block's words.
- **melee-hd:** modeled (the texts read before the free).
- **Fix:** read them before the free.
- **Fix impact:** none.
- **Source:** `crates/melee-sim/src/game/mn/item_switch.rs` (`item_switch_proc`).

### New frees the names' description and keeps it

- **Where:** `mnName_MainInput` (`mnname.c`), `crates/melee-sim/src/game/mn/names.rs` (`name_list_input`).
- **Defect:** A on New frees the list's button description (`text2`) and leaves the pointer in the list's data.
- **Effect:** none observable: the list goes as the keyboard comes in, and only a new hovered button frees the description again, which the list's process doesn't look for under the keyboard.
- **melee-hd:** modeled: the handle kept as the original keeps its pointer, compared by the freed text.
- **Fix:** clear the pointer.
- **Fix impact:** none.
- **Source:** `crates/melee-sim/src/game/mn/names.rs` (`name_list_input`).

### Deleting a name moves the Multi-Man players' names twice

- **Where:** `gmMainLib_8015DBF4` (`gmmain_lib.c`), `crates/melee-sim/src/game/mn/names.rs` (`name_deleted`).
- **Defect:** adjusts the remembered players' name tags of each VS mode for the deleted name, but the Multi-Man mode's (`table[GmVsMode_Multiman]`) twice, first and again at the end.
- **Effect:** after a name is deleted, a Multi-Man player set to the name after it ends with none, and one set to a later name moves two names down instead of one, so the Multi-Man setup offers a name other than the one chosen (milestone 6's mode). The other modes move one.
- **melee-hd:** modeled.
- **Fix:** drop the second call.
- **Fix impact:** the Multi-Man setup's names after a deletion, which reach the match's name tags (presentation and the records).
- **Source:** `crates/melee-sim/src/game/mn/names.rs` (`name_deleted`).

### A name is the text library's format

- **Where:** `mnName_80239A24`, `mnNameNew_KeySetup`, `mnNameNew_8023CE4C`, `mnNameNew_8023D130`, through `HSD_SisLib_803A6B98`; `crates/melee-sim/src/game/mn/names.rs` (`as_format`).
- **Defect:** hands each name and glyph to the text library as its `vsnprintf` format with no arguments, so a `%` in one formats from registers and the stack.
- **Effect:** none from the game's keyboard (Shift-JIS, none of whose bytes is `%`); a save edited elsewhere with a `%` in a name would print what the registers held, or fault.
- **melee-hd:** stops.
- **Fix:** format with `"%s"`.
- **Fix impact:** none.
- **Source:** `crates/melee-sim/src/game/mn/names.rs` (`as_format`).

### The character select's "ready" check reads the first port's coin for every door

- **Where:** `mnCharSel_Scene_OnFrame`'s pending start (case 1), `crates/melee-sim/src/game/mn/charsel/procs.rs` (`charsel_on_frame`).
- **Defect:** the loop over the four doors tests each door's kind but the first port's coin (`mnCharSel_804A0BD0[0]->x5`, whether a hand holds it) every time, never door `i`'s.
- **Effect:** Start goes through to the stage select unless the first port's coin is held while some door is open; another port's held coin doesn't hold it back, and the first port's does even when its own door is closed.
- **melee-hd:** modeled.
- **Fix:** test `mnCharSel_804A0BD0[i]`.
- **Fix impact:** changes when Start leaves the character select; nothing in a match.
- **Source:** `crates/melee-sim/src/game/mn/charsel/procs.rs` (`charsel_on_frame`).

### The stage select's preview reads past its table with no stage hovered

- **Where:** `mnStageSel_80259D84`'s and the preview's reads of `mnStageSel_803F06D0[hovered]`, `crates/melee-sim/src/game/mn/stagesel.rs` (`stage_frame`, the preview process).
- **Defect:** with no stage hovered the index is 30, one past the table's 30 entries, so the entry's fields come from the bytes after it, the string `"MnSlMap.usd"`: its frame byte is the `'s'` (115) and its shown byte the `'u'`.
- **Effect:** none observable: the preview's animation is requested at frame 50 × 115 on the hidden preview, and the shown byte (not below 2) leaves the preview on "none".
- **melee-hd:** modeled (the two bytes are the string's).
- **Fix:** test the index against the table's length first.
- **Fix impact:** none.
- **Source:** `crates/melee-sim/src/game/mn/stagesel.rs` (`stage_frame`).

### A name tag list's row with no character names Captain Falcon

- **Where:** the name box's list on the character select, `crates/melee-sim/src/game/mn/charsel/procs.rs` (`icon_name_or_none`).
- **Defect:** a door with no icon (25) hands `gm_80160980` `icons[25].char_kind`, one past the 25 icons, whose zero entry is Captain Falcon's kind.
- **Effect:** none observable: the text holding his name stays hidden.
- **melee-hd:** modeled.
- **Fix:** skip the name when the door has no icon.
- **Fix impact:** none.
- **Source:** `crates/melee-sim/src/game/mn/charsel/procs.rs` (`icon_name_or_none`).

### The character select's exit after L, R and Start reads its freed coins

- **Where:** `mnCharSel_Scene_OnExit`, `crates/melee-sim/src/game/mn/charsel/procs.rs` (`charsel_on_exit`, `charsel_exit_coin_up`).
- **Defect:** L, R and Start on the character select free every GObj on process links 1 to 8 (`mn_8022F138`), the coins' among them with their user data (`HSD_Free`), and end the scene with no change pending; the exit then works out the characters' sound banks from each door's coin (`mnCharSel_804A0BD0[i]->x5`, whether a hand holds it) through the table's pointers, which now name freed blocks. The hands' and name tags' tables keep dangling pointers too.
- **Effect:** the read takes the byte the freed block kept, as the coin last had it: nothing allocates between the free and the exit, in the same tick. The banks it asks for are the ones a normal exit would ask for, which the main menu's entry then lets go. Seen in `select-sessions/lras` (tick 783).
- **melee-hd:** modeled: a freed coin's block and its `x5` stay (`CharSel::left_tokens`), and the exit reads them; a block the heap has handed out again stops (`unported_call`). The tables' freed blocks stay as addresses for the comparer (`left_hands`, `left_tags`).
- **Fix:** test the scene's pending change (or clear the tables) before reading the coins.
- **Fix impact:** none in play.
- **Source:** `mncharsel.c`'s `mnCharSel_Scene_OnExit` and `mnCharSel_Scene_OnFrame`'s L, R and Start; `crates/melee-sim/src/game/mn/charsel/procs.rs`.

### The trophy collection frames every layout as the rings

- **Where:** `_tyDisplay_80319EF0` with the layouts `_tyDisplay_80318CB4`, `_tyDisplay_80319540` and `_tyDisplay_80319994` (`tydisplay.c`), `crates/melee-sim/src/game/ty/collection.rs` (`coll_frame`, `coll_clear_grid`).
- **Defect:** the scene's entry stores the layout the buttons held choose (L rows, R even rings, Y the triangle, else jittered rings) in the grid's first word (`TyDspGrid.x00`), and the framing reads it back to pick the view's sides (a margin of 14 for the rings, 7 for the rows, none for the triangle) and, for the triangle, to center the camera on x 0. But each layout starts by clearing the whole grid (`memzero(grid, 0x12E4)`), that word too, so the framing always reads 0.
- **Effect:** the rows and the triangle get the rings' sides (which bound the camera's turn), and the triangle, which grows to the left, is framed on its places' middle rather than on 0. Only the collection's view is affected.
- **melee-hd:** modeled: the layout's word clears with the grid.
- **Fix:** keep the layout outside the grid, or set it again after the layout.
- **Fix impact:** none in play; it changes the collection's framing for the L and Y layouts.
- **Source:** `tydisplay.c`'s `tyDisplay_Scene_OnEnter` and the layouts' `memzero`; `crates/melee-sim/src/game/ty/collection.rs` (`coll_frame`).

### The other records read their title before writing it

- **Where:** `mnCount_Create`, `crates/melee-sim/src/game/mn/count.rs` (`count_open`).
- **Defect:** the screen's user data (`MnCountData`, 0x5C bytes from `HSD_MemAlloc`) has its title (`+0x58`) left out of `mnCount_InitUserData`, and the creation tests it and frees the text it names (`HSD_SisLib_803A5CC4`) before writing it, so it frees whatever text the heap block's last occupant left a pointer to there, as the rumble screen does with its title.
- **Effect:** none observable in the sessions: the block held no live text's address each time. Were a live text to sit where an earlier screen's freed title was, the original would free it.
- **melee-hd:** modeled where melee-sim knows the word: the last screen's block and its title (`Count::left`), freed again when the new block is the same; any other block's word names no text, as for the rumble screen.
- **Fix:** clear the title with the rows.
- **Fix impact:** none in play.
- **Source:** `mncount.c`'s `mnCount_Create` and `mnCount_InitUserData`.

### The VS records read their cursors before writing them

- **Where:** `mnDiagram_CreateScreen`, `crates/melee-sim/src/game/mn/diagram.rs` (`diagram_screen_make`).
- **Defect:** the screen's user data (`Diagram`, 0x50 bytes from `HSD_MemAlloc`) has each cursor (`+0x3C`, `+0x3E`) set from its own low byte (`(u8) fighter_cursor_pos`), a word the block's last occupant left, then masked to its high byte, so the read is dropped and the cursors are always 0; the cursors the pages save in the rules (`xE` to `x11`) are never read back.
- **Effect:** none observable: the masked value is 0 whatever the word held. The screens always open on the first characters and names.
- **melee-hd:** the cursors start at 0.
- **Fix:** take the saved cursors from the rules, or set the cursors without reading them.
- **Fix impact:** a fix that restores the saved cursors would change where the grid opens.
- **Source:** `mndiagram.c`'s `mnDiagram_CreateScreen`.

### A VS records cell's detail aligns its name where it meant the numbers

- **Where:** `mnDiagram_PopupAnimProc`, `crates/melee-sim/src/game/mn/diagram.rs` (`diagram_popup_anim`).
- **Defect:** each frame the detail places its texts on their joints, setting the name's alignment (`text[0]`) to left first, then, after placing each of the other texts, setting the name's alignment to centered again where each placed text's was meant.
- **Effect:** the row's name is drawn centered on its joint, not left-aligned as its creation sets it, from the detail's first frame.
- **melee-hd:** modeled.
- **Fix:** set each placed text's alignment, or none.
- **Fix impact:** the name's place in the detail (presentation only).
- **Source:** `mndiagram.c`'s `mnDiagram_PopupAnimProc`.

### The VS records step through names by slot, not by the order they show

- **Where:** `mnDiagram_InputProc`'s scrolling (`mnDiagram_FindPrevName`, `mnDiagram_FindNextName`), `mnDiagram_GetPrevNameIndex` and `mnDiagram_GetNextNameIndex` (the statistics' left and right), `mnDiagram2_GetAggregatedFighterRank`; `crates/melee-sim/src/game/mn/diagram.rs`, `diagram2.rs`.
- **Defect:** the grid's and the statistics' places are indices into the names in the order shown (`mnDiagram_NameDisplayOrder`, by KOs), but the steps test the name slot of the same index for a name (`GetNameText(i)`), and the aggregated ranking takes the names' slots 0 to the count, not the names.
- **Effect:** none in the sessions: the game keeps names packed from slot 0 (`DeleteName` moves them up), so the first count of places and slots both hold names. An edited save with a gap among its names would scroll past or stop short of names, and the most and least played counts would skip some.
- **melee-hd:** modeled.
- **Fix:** test the name the place names (`GetNameText(mnDiagram_GetNameByIndex(i))`), and rank over the names.
- **Fix impact:** none for a save the game made.
- **Source:** `mndiagram.c`, `mndiagram2.c`.

### The sound test writes a sound's three-digit number past its buffer

- **Where:** `mnSoundTest_8024AA70`, `crates/melee-sim/src/game/mn/sound_test.rs` (`sound_test_sound_box`, `sound_test_number`).
- **Defect:** entering a category's sounds shows the picked sound's number, written by `mn_8022EA08` into a 3-byte stack buffer (`char string[3]`, at `sp+0x24`): a number of three digits and its terminator take four bytes. Categories hold up to 113 sounds, and the number picked stays when B leaves the category, so leaving and entering again with a sound from 100 on writes the terminator one byte past the buffer. The song's number (`mnSoundTest_8024A958`, at most 79) and the number shown as left and right move (`fn_8024AED0`, a 20-byte buffer) fit.
- **Effect:** none observable: the byte past the buffer (`sp+0x27`) is a stack slot the function never uses, and the text takes the whole number. Seen in `sound-sessions/jp` (the last category, sound 112, left and entered again).
- **melee-hd:** the number is made as a string of its own; nothing models the stack byte.
- **Fix:** a 4-byte buffer.
- **Fix impact:** none.
- **Source:** `mnsoundtest.c`'s `mnSoundTest_8024AA70`; `crates/melee-sim/src/game/mn/sound_test.rs`.

## Special Melee and Camera Mode

### Stamina Mode's stage select asks for no stage's sound bank

- **Where:** `gm_801B927C` (Stamina Mode's stage select exit), `crates/melee-sim/src/game/gm/special.rs` (`stamina_exit_sss`, `stamina_stage_banks`).
- **Defect:** where VS mode's exit (`gmVsMelee_ExitSss`) asks for the picked stage's sound bank (`lbAudioAx_80026EBC(vs->start.rules.stkind)`), Stamina Mode's passes the select's forced stage (`sss->force_stage_id`), which is -1 whenever the player picked the stage. `Stage_8022519C(-1)` reads the entry before `stage_id_map`, whose ground kind is four bytes of the assertion string before the table (`"no!=St_Player_InitPos_None"`'s `"s_No"`, 0x735F4E6F), past the banks' table, so no bank is asked for.
- **Effect:** the stage's sound bank isn't wanted from the stage select (bank 8 asked for with an empty mask); the match's own load brings the stage's sounds. Seen in `special-sessions/special-stamina`.
- **melee-hd:** modeled: a forced stage of -1 asks for no bank.
- **Fix:** pass `vs->start.rules.stkind`, as VS mode does.
- **Fix impact:** changes when the stage's bank loads, so the timeline (and what the disc and ARAM do before the match), not the match itself.
- **Source:** `gmstamina.c`'s `gm_801B927C`; `crates/melee-sim/src/game/gm/special.rs`.

### A snapshot's card file carries its header's padding as the buffer held it

- **Where:** `lbSnap_8001DC0C` and `lbSnap_8001DF6C` (`lb_8001BB48`), `crates/melee-sim/src/lb/snap_write.rs` (`snap_make`, `snap_save`).
- **Defect:** the snapshot's header (`Unk80433380_0`) is written field by field into the preload cache's buffer, which nothing clears, and the whole struct up to the compressed image (`x38`) goes to the card: the padding after `x8` (3 bytes) and after `stkind` (2 bytes) are whatever the buffer held, and the file's last block, written whole, takes the buffer's bytes past the image too.
- **Effect:** five bytes of each snapshot file (and its last block's tail) are the heap's leftovers; the album never reads them.
- **melee-hd:** modeled: the file is made from the heap's image of the buffer, the header's fields over what it held; the card `2026-10-06_17-11-18` leaves is the next recorded session's byte for byte.
- **Fix:** clear the buffer (or the header) before writing it.
- **Fix impact:** none in play; card files made after a fix differ in those bytes.
- **Source:** `lbsnap.c`'s `lbSnap_8001DC0C`; `crates/melee-sim/src/lb/snap_write.rs`.

### Camera Mode's match frees the last match's texts

- **Where:** `gmCamera_801A31FC` (the match's start) through `gmCamera_801A26C0` (the menu's first state's entry), `crates/melee-sim/src/game/gm/camera_mode.rs` (`camera_match_start`, `cam_resume`).
- **Defect:** the match's start enters the menu's first state, which frees the menu's texts (`gmCamera_VsCamUiState.x48`) when the first is set, before the start clears them: a second Camera Mode match in a session finds the last match's text pointers there, into a text arena the scene's end made anew, and `HSD_SisLib_803A5CC4` frees whatever text the new scene has at those places.
- **Effect:** none observed: no recorded session plays two Camera Mode matches. A text of the new match's at one of those places would vanish.
- **melee-hd:** modeled: the menu keeps the old texts' arena places (`CamUi::texts`) and frees what the text list holds there.
- **Fix:** clear `x48` before entering the first state.
- **Fix impact:** none in play but the vanished text.
- **Source:** `gmcamera.c`'s `gmCamera_801A31FC` and `gmCamera_801A26C0`; `crates/melee-sim/src/game/gm/camera_mode.rs`.

## Results and records

### A results pose's fighter takes an unset byte of its allocation info

- **Where:** `ftDemo_CreateFighter` (with `initFighter`'s `plAllocInfo` inlined), `crates/melee-sim/src/game/ft/demo.rs`.
- **Defect:** the stack `plAllocInfo` handed to the fighter's creation never has its byte +5 set (sp+0x11 at `0x804EEA21` in the results screen's setup), and `Fighter_Create` copies it into the fighter (`x61C`).
- **Effect:** none observable: the byte's only reader, `ftData_800859A8`, never runs for a results pose's fighter. On a console the slot holds byte 1 of the saved r28 of `HSD_ArchiveGetPublicAddress`, 0; ssbm-rs's ported callers leave a stale register's byte there (61 in its traces), from probes of its port mode (`$SSBM_RS_LOCAL/research/x61c-byte`). The costume-list corruption the byte seemed to cause isn't real.
- **melee-hd:** stand-in: the byte is 0, as a console's, and the comparer leaves `x61C` out for a demo fighter (`render_target_view`, the comment there), with the proof in the engine notes.
- **Fix:** set the byte (or clear the struct) before the creation.
- **Fix impact:** none.
- **Source:** `ftdemo.c`'s `ftDemo_CreateFighter`; the x61C probe logs.

### A winner's camera stays current when it takes no image

- **Where:** `fn_80179990` (the players' cameras' render callback, through `fn_80179D3C` to `fn_80179DA8`), `crates/melee-sim/src/game/gm/results_fighters.rs` (`results_player_render`).
- **Defect:** the callback makes its camera current (`HSD_CObjSetCurrent`) and ends it (`HSD_CObjEndCurrent`) only after it copies an image; a winner's camera copies once, when its pose has run out and the flames have gone, so on every other frame it returns with its camera still current.
- **Effect:** none observable: the next camera's callback makes its own current before drawing, and nothing between them draws.
- **melee-hd:** modeled: no `Draw::EndCamera` on those frames.
- **Fix:** end the camera on every path that set it.
- **Fix impact:** none.
- **Source:** `gm_1798.c`'s `fn_80179990` and its machine code (`.L_80179BD8` returns without `HSD_CObjEndCurrent`).

### Every column's "more below" arrow counts the first player's statistics

- **Where:** `fn_80174B4C` (a player's statistics page), `crates/melee-sim/src/game/gm/results_panel.rs` (`stats_page`).
- **Defect:** the page's count of rows to show, which decides whether the "more below" arrow (`x0_2`) shows, is `fn_80174A60(list, 0)`: the rows the first player's statistics show, whichever player's page it is. The rows a KO page shows depend on the player (a player's own row and an absent player's are left out).
- **Effect:** on the KOs page a player's arrow can show with nothing below or stay hidden with rows below, by the first player's count; the scroll itself (`fn_80177B7C`, `fn_80177DD0`) counts the player's own rows. Only what the arrow shows differs.
- **melee-hd:** modeled: the count takes slot 0.
- **Fix:** pass the page's player.
- **Fix impact:** the arrow; nothing else reads `x0_2`.
- **Source:** `gmresult.c`'s `fn_80174B4C`; ssbm-rs's port.

### The statistics' rows clip to the first player's column

- **Where:** `fn_801749B8` (the statistics' texts' render callback), `crates/melee-sim/src/game/gm/results_panel.rs` (`results_stats_text_render`).
- **Defect:** every player's statistics' texts set the scissor from the first player's statistics' spot to the first player's last row's joint (`player_data[0]`), not their own.
- **Effect:** none observable: the four columns' spots and rows' joints sit at the same heights.
- **melee-hd:** modeled.
- **Fix:** use the text's own column.
- **Fix impact:** none.
- **Source:** `gmresult.c`'s `fn_801749B8`.

### The panel's setup makes texts it never uses

- **Where:** `fn_80175C5C` and `fn_80175DC8`, `crates/melee-sim/src/game/gm/results_panel.rs` (`results_panel_texts`, `results_panel_model`).
- **Defect:** `fn_80175C5C` makes two text buffers (`HSD_SisLib_803A6754(0, 0)`) and drops them; `fn_80175DC8` frees the title text `x2C`, which nothing sets, and makes the match kind's title without keeping it there.
- **Effect:** none observable: the two empty buffers hold no string and draw nothing; the texts live in the text heap until the scene ends.
- **melee-hd:** modeled: the texts are made and kept in the text list.
- **Fix:** drop the two calls; keep the title in `x2C`.
- **Fix impact:** the text heap's layout.
- **Source:** `gmresult.c`'s `fn_80175C5C` and `fn_80175DC8`.

### Sudden Death's team count reads past the five teams

- **Where:** `gm_80166CCC` (the match's results with Sudden Death's), `crates/melee-sim/src/game/gm/records.rs` (`merge_sudden_death`).
- **Defect:** counting Sudden Death's active teams, the loop runs over six where `team_standings` has five, so its sixth `active` byte is 0xA bytes past the fifth team: the first player's self-destructs' high byte (`player_standings[0].self_destructs`).
- **Effect:** with 256 or more self-destructs by the first player in Sudden Death, the count is one more, and in a team battle the winning teams that sat Sudden Death out place one lower. No match reaches that many.
- **melee-hd:** modeled: the sixth team counts by that byte.
- **Fix:** loop over five.
- **Fix impact:** none in play.
- **Source:** `gm_1601.c`'s `gm_80166CCC`; ssbm-rs's port, which reads the byte from memory.

### A winner's portrait shows its image's uninitialized bytes until the pose ends

- **Where:** `fn_80179990` (a player's camera's render callback) and the panel's columns (`fn_80177748`), `crates/melee-sim/src/game/gm/results_fighters.rs` (`results_image_alloc`, `results_player_render`).
- **Defect:** each player's second image is a block from the scene's heap (`lb_800121FC`), which nothing clears; a loser's camera copies its fighter into it every frame, but a winner's only once its pose's frames have run out, while its column shows the image from the panel's arrival on.
- **Effect:** visible: for the seconds a winner poses, its portrait in the panel shows whatever the block held (the match's freed fighters, lights and files) as RGB5A3 texels, a field of colored noise; then the copy replaces it. It changes with the match before.
- **melee-hd:** modeled: the renderer starts each results image from the bytes the heap's image says its block held (`ResultsFighters::leftovers`); where the image doesn't know a byte it shows zero and notes it.
- **Fix:** clear the images when the scene takes them, or take the winner's copy before its column shows.
- **Fix impact:** the portraits during the pose; nothing the simulation reads.
- **Source:** `gm_1798.c`'s `fn_80179990`; ssbm-rs's renderer's frames of `results-sessions/stock` (frames 2451 to 3401).

### The results' 32-bit records wrap where the code means to hold them

- **Where:** `fn_80161C90` (a player's statistics added), `gm_8016247C` (the coins the KOs earn), `crates/melee-sim/src/game/gm/records.rs` (`add_stats`) and `results_exit.rs` (`results_add_coins`).
- **Defect:** the 32-bit sums (attacks, damage dealt, taken and recovered, play time, the distances, the coins collected, swiped and lost, the save's coin totals) are added as unsigned words and then tested against `0xFFFFFFFF` to hold them there; the test can never hold, so each sum wraps past the largest word instead. The 16-bit counts are held at 0xFFFF as meant.
- **Effect:** none in play: no save reaches four billion of any of them. A save edited to the largest value (`results-sessions/ko-max`) wraps to small numbers after a match.
- **melee-hd:** modeled: the sums wrap.
- **Fix:** add in 64 bits, or test before adding.
- **Fix impact:** the records' values past the largest word.
- **Source:** `gm_1601.c`'s `fn_80161C90` and `gm_8016247C`; `results-sessions/ko-max`, whose card after compares.

## Gameplay that reads the display pass

The game ticks on the pad alarm's 60.00 Hz while video runs at 59.94 Hz, so about once every 1,001 frames a round ticks twice and renders once ([the engine notes](engine.md#state-the-render-pass-writes)). State only the display pass writes is then a tick older on the second tick, and where gameplay reads it, the outcome depends on CPU timing rather than on the inputs. Slippi's online codes patch some of these as determinism fixes.

### Off-screen damage counts render-pass marks

- **Where:** `ftDrawCommon_80080E18` and `ftLib_UpdateScreenVisibility` (the fighter's render callback), `ifMagnify_802FBBDC`, read by `Fighter_procAnim`; `crates/melee-sim/src/game/ft/draw.rs` and `crates/melee-sim/src/game/hud/magnify.rs`.
- **Defect:** whether a fighter is off screen is decided in its draw callback against the camera, and the magnifier copies the previous pass's mark; `Fighter_procAnim` counts off-screen ticks from it and deals damage every `x7AC` of them.
- **Effect:** a doubled round while a fighter crosses the screen's edge moves the count's start or reset by a tick, so the damage lands a tick earlier or later, or once more or less; a widescreen code changes it too.
- **melee-hd:** modeled in the display pass (vanilla); the Slippi ruleset's BrawlOffscreenDamage (`Rule::OffscreenLimits`) tests the fighter's position against the stage's camera limits.
- **Fix:** test the position against fixed limits in logic, as Slippi's code does.
- **Fix impact:** changes off-screen damage timing in vanilla play; Slippi's ruleset already has it.
- **Source:** [engine notes, state the render pass writes](engine.md#state-the-render-pass-writes) (line 139).

### Pokémon Stadium's monitor tests a render-pass camera

- **Where:** `grStadium_801D32D0` (the monitor's close-up), reading the game camera's CObj that only `Camera_8002A4AC` in the display pass writes; `crates/melee-sim/src/game/gr/stadium.rs`.
- **Defect:** the close-up asks whether its player's box stays in the camera's view, which logic sees a tick late (two on a doubled round's second tick); when it leaves, the monitor draws random numbers for its next mode.
- **Effect:** a doubled round as the box crosses the view's margin moves the mode change by a tick, and the random seed with it.
- **melee-hd:** modeled against the camera the display pass brought up to date; the Slippi ruleset's PSCameraIndependentMonitor tests fixed bounds (`gr::stadium::zoom_keeps_player`).
- **Fix:** test fixed bounds in logic.
- **Fix impact:** changes the random sequence in vanilla play on Pokémon Stadium; Slippi's ruleset already has it.
- **Source:** [engine notes, state the render pass writes](engine.md#state-the-render-pass-writes) (line 140).

### Opponents' direction counts fighters KO'd toward the screen

- **Where:** `ftLib_GetOpponentsDir` (through `ftLib_GetCameraBonePos`), read by Whispy, Green Greens and items' facing (`it_8026B684`); `crates/melee-sim/src/game/it/owner.rs` (`opponents_dir`).
- **Defect:** it counts every awake fighter's camera bone toward a side, dead ones included; a fighter KO'd toward the screen is placed by the render callback through the camera's inverse view matrix, so its bone follows the display pass. A tie draws `HSD_Randi(2)`.
- **Effect:** Whispy's blow direction (and the others') can depend on where the display pass last placed a KO'd fighter, and the tie's draw moves the seed.
- **melee-hd:** modeled; the Slippi ruleset's WhispyBlowDirFix leaves out fighters in motions up to 11 (Slippi 3.14's code set lacks it, so Whispy counts dead and sleeping fighters there as the original does).
- **Fix:** leave out dead and sleeping fighters, as Slippi's code does.
- **Fix impact:** changes Whispy's direction and item facing in vanilla play; Slippi's ruleset (3.16 on) already has it.
- **Source:** [engine notes, state the render pass writes](engine.md#state-the-render-pass-writes) (line 141); [the oracle](oracle.md) (line 137).

### Homing and nearest-opponent targeting through render-placed joints

- **Where:** `it_802B64FC` (Samus's homing missile) with `ftLib_FindNearestOpponentInDir`, and items using `ftLib_FindNearestOpponent`; `crates/melee-sim/src/game/it/kinds/samusmissile.rs`, `crates/melee-sim/src/game/it/owner.rs`.
- **Defect:** the missile turns toward the nearest opponent ahead through each fighter's camera bone, skipping only sleeping fighters, not dead ones; a fighter KO'd toward the screen has its root joint placed by the render callback from the camera, and `Fighter_procMap` puts it back at the position each tick at a later priority.
- **Effect:** a homing missile in flight while an opponent falls toward the screen can turn the other way on a doubled round's tick and fly on differently; with items on, the red shell, Like Like, Octorok, ReDead, Scizor and others are exposed the same way. Slippi doesn't patch it, so Slippi's peers can desync there.
- **melee-hd:** modeled (the joint placed in the display pass and read through the joints).
- **Fix:** target from logic positions, and skip dead fighters.
- **Fix impact:** changes homing in those rare cases; breaks replay and Slippi compatibility there.
- **Source:** [engine notes, state the render pass writes](engine.md#state-the-render-pass-writes) (line 145).

### Onett's cars read the render-pass camera

- **Where:** `grOnett_801E43E0` (the car's process) reading the CObj through `Camera_800307D0`, `crates/melee-sim/src/game/gr/onett.rs` (`proc_cars`).
- **Defect:** the car takes the camera's view edges from the CObj the display pass writes; when a car passes the view's right edge with a fighter on the ground, it draws `HSD_Randi(3)`, honks and sets `stage_info.x740`, which the computer player's car avoidance and the camera read.
- **Effect:** a doubled round as the edge passes the car moves that by a tick.
- **melee-hd:** modeled: the car reads the CObj the display pass keeps (`camera::ground_span`).
- **Fix:** take the edges from logic state.
- **Fix impact:** changes Onett's random sequence and car timing.
- **Source:** [engine notes, state the render pass writes](engine.md#state-the-render-pass-writes) (line 146).

## Latent defects

Defects no game data or code reaches in play. For each, the effect is none observable, the fix is a guard or a defined value, and a fix changes nothing in play. melee-hd stops (panics, or reports an unported case) or counts a stale read where the original would go wrong, so a case that turns out reachable can't pass unnoticed.

| Defect | Where | What goes wrong | Source |
| --- | --- | --- | --- |
| IK with the effector on the chain's root | `resolveIKJoint1`, `hsd/jobj.rs` | Reads two registers and two stack vectors it never set; no joint on the disc has an IK role | [engine notes](engine.md#uninitialized-values) (line 52); `crates/melee-sim/src/hsd/jobj.rs:1573` |
| Quakes of another kind | `Camera_RequestQuake`, `game/camera.rs` | Never sets the length and stores the epicenter argument's register (its address); no caller passes such a kind | [engine notes](engine.md#uninitialized-values) (line 53); `crates/melee-sim/src/game/camera.rs:1382` |
| The cutscene camera's fourth transition kind | `script_progress`, `game/camera.rs` | Leaves the ratio unset (a register its caller left); no code sets the kind | `crates/melee-sim/src/game/camera.rs:2865` |
| The rumble screen's name rows on an empty slot | `mnVibration_CreateNameRow`, `game/mn/vibration.rs` | A row's slot is the scroll plus the row, not the row's name: with a gap among the save's names, `GetNameText` hands `HSD_SisLib_803A6B98` a null format. The game keeps names packed (`DeleteName` moves them up), so only an edited save could leave a gap | `crates/melee-sim/src/game/mn/vibration.rs` (`vibration_name_row`) |
| An overflowing substep count | `mpColl_80043754`, `mp/coll/mod.rs` | Returns a register its caller left when a move over about 10¹⁰ units runs no substep | [engine notes](engine.md#uninitialized-values) (line 64) |
| No logged hit above -1 knockback | `ftColl_8007A06C`, `game/ft/attack.rs`; `it_80270E30`, `game/it/coll.rs` | Leaves the index it reads uninitialized when every knockback is NaN or -1 and below | [engine notes](engine.md#uninitialized-values) (line 69); `crates/melee-sim/src/game/it/coll.rs:647` |
| Stages without spawn points | `Ground_801C2D24`, read by `Stage_80224E64` and `fn_80167638`; `game/gr/ground.rs`, `game/gm/vs.rs` | Leaves its vector alone (and adds into its out parameter for combined points) when the stage lacks the point; every VS stage has them | [engine notes](engine.md#uninitialized-values) (line 78); `crates/melee-sim/src/game/gr/ground.rs:207` |
| A string ending in a lead byte | `HSD_SisLib_803A67EC`, `hsd/sis.rs` | Reads past a string that ends in a Shift-JIS lead byte; the game's strings end in whole characters | [engine notes](engine.md#uninitialized-values) (line 91) |
| An output that ends exactly at its room | `hsd_803B3398` and `hsd_803B46D4` (HAL's JPEG writer), `hsd/jpeg.rs` | A block of bytes is refused unless it ends before the output's end (`<` where `<=` would do), so an image that would fill the room exactly is dropped as too big; the snapshot's room is 256,000 bytes, and the frames compressed in the sessions took up to 248,093 | `crates/melee-sim/src/hsd/jpeg.rs` (`Encoder::bytes`, `jfif`) |
| The sound test's category checks | `lbAudioAx_80023968` and `lbAudioAx_80023A44`, `lb/audio/sound_test.rs` | Guard a category and a sound with `arg0 < 0 && arg0 >= 30` and `arg1 < 0 && 0x83D60 <= arg1`, which no value passes, so an out-of-range category reads past `lbAudioLoadData`'s table; the sound test passes only categories 0 to 29 and sounds of their lists | `lbaudio_ax.c` (`MUST_MATCH`) |
| Character-specific effect spawns for another character | `efSync_Spawn`, `game/ef/sync.rs` | Case 0x501 reads Peach's part 85, past a shorter parts table, and action-state callbacks read the character's `mv` words | [engine notes](engine.md#uninitialized-values) (line 59) |
| The approach's roll with no target | `ftCo_800AB224` calling `ftCo_800BA2E8`, `game/ft/cpu_move.rs` | Passes the target on unchecked after its own null test, so with no target and `xF8_b6` set the roll reads a fighter at address 0; the VS decision sets the target on every frame it sets `xF9_b1`, which the approach needs, and nothing before the walk clears it | `crates/melee-sim/src/game/ft/cpu_move.rs:664` |
| Unown's swarm from an edge past the four | `it_802CED54`, `game/it/kinds/unknown.rs` | Spawns the swarm from an uninitialized stack vector when the edge index (`+0x84`) is outside 0 to 3; `it_802CE8D0` always sets it | `crates/melee-sim/src/game/it/kinds/unknown.rs:305` |
| Judgement's roll past its sums | `ftGw_SpecialS_GetRandomInt`, `game/ft/gamewatch/special_s.rs` | Returns a register it never set when the roll isn't below any running sum; a roll below their total always is | [engine notes](engine.md#uninitialized-values) (line 90) |
| Falcon Kick's flame in another state | `ftCa_SpecialHi_800E3EAC`, `game/ft/captain/special_lw.rs` | Reads a register and a stack word it never set outside the kick's two states; only those states call it | `crates/melee-sim/src/game/ft/captain/special_lw.rs:24` |
| The play time's cap | `gm_GetPlayTime`, `game/mn/count.rs` | Caps the VS and single-player times' sum at `u32::MAX` by testing whether the sum is over it, which a wrapped sum never is: past 2^32 seconds (136 years) it wraps | `gm_1601.c`'s `gm_GetPlayTime`; `crates/melee-sim/src/game/mn/count.rs` (`count_row_number`) |
| A statistic past the table | `mnDiagram2_GetStatValue`, `mnDiagram2_GetAggregatedFighterRank`; `game/mn/diagram2.rs` | Return or use a register they never set for a statistic outside 0 to 23 (or 0x15 to 0x17); the pages only ask for those | `crates/melee-sim/src/game/mn/diagram2.rs` (`stat_value`, `aggregated_rank`) |
| A ranking past the unlocked characters | `mnDiagram3_PopulateRankings`, `game/mn/diagram3.rs` | A rank past the unlocked characters is 25, whose records (`GetPersistentFighterData(25)`) lie past the 25 characters'; at least 14 are always unlocked and the page ranks five | `crates/melee-sim/src/game/mn/diagram3.rs` (`d3_rankings`) |
| A special message the table doesn't have | `un_802FE3F8`, `game/hud/prize.rs` | Leaves its string unset for a message outside 0 to 65; every caller asks for one of those | `crates/melee-sim/src/game/hud/prize.rs` (`message_strings`) |
| A walk's animation outside the walk | `ftWalkCommon_800DFDDC`, `game/ft/co/ground.rs` | Leaves the rate in `f31` from its caller for a state that isn't one of the walk's three | `crates/melee-sim/src/game/ft/co/ground.rs:243` |
| A Corneria talk of five lines | `grCorneria_801E2598`, `game/gr/corneria.rs` | Reads the next talk's first line past a talk's five; no talk in the table has five lines | `crates/melee-sim/src/game/gr/corneria.rs` (`talk_has_line`) |
| Venom's Arwings held still | `grVenom_80205F30`, `game/gr/venom.rs` | Tests a flag (`grVe_804D6A3C`) nothing writes, so the Arwings never hold; Corneria's counterpart holds them before the start | `crates/melee-sim/src/game/gr/venom.rs` (`FROZEN`) |
| Dancing Blade's switch from another state | Marth's side special switches, `game/ft/mars/special_s.rs` | Leaves the state it changes to unset | `crates/melee-sim/src/game/ft/mars/special_s.rs:219` |
| No surface touched | `it_80276CEC`, `game/it/maplib.rs` | Stores an unset register as the surface | `crates/melee-sim/src/game/it/maplib.rs:365` |
| Stadium's screen without its texture | `grStadium_801D1290`, `game/gr/stadium.rs` | Leaves its stack slot as it was when no texture is found; one always is | `crates/melee-sim/src/game/gr/stadium.rs:309` |
| Stadium's monitor weights leaving no mode | `grStadium_801D2A60`, `game/gr/stadium.rs` | Draws until the mode differs from the current and last ones, forever if the weights leave none that can | `crates/melee-sim/src/game/gr/stadium.rs:1004`; [engine notes](engine.md#players-codes) (line 273) |
| A damage display that isn't one | `ifStatus_802F4EDC`, `game/hud/status.rs` | Reads a null display | `crates/melee-sim/src/game/hud/status.rs:342` |
| Peach's Castle's path draw with no path left | `grCastle_801CE054`, `game/gr/castle.rs` | Takes the first entry of a stack array it never filled when all nine Bullet Bill paths are among the last three flown; only three are ever recorded | `crates/melee-sim/src/game/gr/castle.rs` (`pick_path`) |
| A sound command's pan direction past 3 | `ftAction_80072320`, `game/ft/commands.rs` | Passes what `f31` holds | `crates/melee-sim/src/game/ft/commands.rs:973` |
| PK Flash of another kind | `it_802AAA80`, `game/it/kinds/nesspkflash.rs` | Reads a register it never set | `crates/melee-sim/src/game/it/kinds/nesspkflash.rs:243` |
| The main menu's light color for a list past the table | `mn_8022C010`, `mn_8022BFBC` (inlined in `fn_8022C128`, `mn_8022C304`), `game/mn/main.rs` (`light_color`) | A list kind past 0x21 falls out of the switch and returns its own number as the color group, and a group past 4 returns its own number as the color's address (inlined, a callee-saved register its caller left); every list the menu reaches is in the table | `crates/melee-sim/src/game/mn/main.rs` (`light_color`) |
| A leaving list's process in another state | `fn_8022AF10`, `game/mn/main.rs` (`list_leave_proc`) | Leaves its animation loop pointer unset for a state other than the two it's set up for; only those two hand a list to it | `crates/melee-sim/src/game/mn/main.rs` (`list_leave_proc`) |
| An idle list without a description | `fn_8022AFEC`, `game/mn/main.rs` (`list_proc`) | Writes the description's `hidden` byte through a null pointer (address 0x4D) when an idle list has no description; every list the menu makes has its strings | `crates/melee-sim/src/game/mn/main.rs` (`list_proc`) |
| A fixed handicap of 0 | `gm_80167BC8`, `game/gm/vs_mode.rs` (`apply_menu_rules`) | Reads the handicap table one entry before its start (`handicap - 1` as an unsigned index); the menus set handicaps 1 to 9 | `crates/melee-sim/src/game/gm/vs_mode.rs` (`apply_menu_rules`) |
| Kirby's aerial Thunder Jolt for another copy | `ftKb_PkSpecialAirN_Anim`, `game/ft/kirby/special_pikachu.rs` | Takes the landing lag from `f31`, which its caller left, for a copy other than Pikachu's or Pichu's; only those two copies enter the state (ssbm-rs transliterates the function from its machine code for it) | `crates/melee-sim/src/game/ft/kirby/special_pikachu.rs:161` |
| Held needles of another kind | `it_802B18B0`, `game/it/kinds/seakneedleheld.rs` | Shows as many needles as a count it never set for a kind other than Sheik's and Kirby's copy's; only those two make them | `crates/melee-sim/src/game/it/kinds/seakneedleheld.rs:63` |
| Sheik's chain with no link out | `it_802BC080`, `game/it/kinds/seakchain.rs` | Stores a register its caller set (`env_flags`) when its loop doesn't run | `crates/melee-sim/src/game/it/kinds/seakchain.rs:796` |
| A floor probe that misses | `ftCo_800DC920`, `game/ft/co/grab.rs` | Leaves the offset a stack word it then compares | `crates/melee-sim/src/game/ft/co/grab.rs:503` |
| Rumble scripts | `HSD_PadRumbleInterpret1`, `input/rumble.rs` | Reads through a null pointer for a loop end before any loop start, and spins forever on opcodes 6 and 7 or a command that doesn't wait; no script in the game has these | `crates/melee-sim/src/input/rumble.rs:115`; `tools/leaf-check/tests/rumble.rs:64` |
| Particle commands past their tables | `hsd_8039930C` and the bank lookups, `hsd/particle.rs` | A texture group past the bank's table takes an unrelocated word for an address; a point joint past the table reads the particle links as joints; a callback index faults on the null `psCallback` | `crates/melee-sim/src/hsd/particle.rs:236`, `:850`, `:1602` |
| A tornado particle without a generator | `calcTornadoLastPos`, `melee-render/src/hsd/psdisp.rs` | Reads a null generator's position | `crates/melee-render/src/hsd/psdisp.rs:181` |
| Shape weights out of range | `PObjUpdateFunc`, `hsd/pobj.rs` | Writes past the weights for a channel out of their range | `crates/melee-sim/src/hsd/pobj.rs:582` |
| A Big Blue platform callback on another joint | `fn_801E8560`, `game/gr/bigblue.rs` | Reads the joint's index, which it never set, for a joint other than 0 to 2, and pushes the platform past the three when none has that index; only joints 0 to 2 take the callback, and the platforms' indices are 0 to 2 | `crates/melee-sim/src/game/gr/bigblue.rs:1308` |
| A Big Blue road piece without its joints | `grBigBlue_801EB004`, `game/gr/bigblue.rs` | Its asserts read the start and end joints it hasn't looked up when an earlier joint is missing; every piece's joints exist | `crates/melee-sim/src/game/gr/bigblue.rs:1827` |
| A texture's LOD bias animated | `TObjUpdateFunc`, `hsd/tobj.rs` | Writes the bias into the loaded LOD descriptor every TObj from it shares; no animation has the channel | `crates/melee-sim/src/hsd/tobj.rs:538` |
| Cameras without their parts | `makeProjectionMtx`, `HSD_CObjSetupViewingMtx`, `HSD_CObjSetRoll`, `hsd/cobj.rs` | Another projection type returns the caller's `r31`; a dirty view without an eye or interest builds from uninitialized stack and writes through a null WObj; a roll without an eye vector sets the up vector from uninitialized stack | `crates/melee-sim/src/hsd/cobj.rs:337`, `:358`, `:613` |
| A light id that isn't one | `HSD_LightID2Index`, `hsd/lobj.rs` | Asserts and returns an uninitialized index | `crates/melee-sim/src/hsd/lobj.rs:257` |
| An effect from an unloaded file | `ef::desc`, `game/ef/mod.rs` | Reads low memory for the descriptor | `crates/melee-sim/src/game/ef/mod.rs:532` |
| Waiting for banks nothing loads | `lbAudioAx_80027648`, `lb/audio.rs`; `HSD_SynthSFXBankDeflagSync`, `audio/bank.rs` | Waits, or spins with no wait point, forever | `crates/melee-sim/src/lb/audio.rs:1372`; `crates/melee-sim/src/audio/bank.rs:490` |
| The background flash's color commands | `fn_80021C80` with `fn_80021C18`, `lb/bgflash.rs` | `fn_80021C18` takes the owner's commands and does nothing, so a script with one would stay on it forever | `crates/melee-sim/src/lb/bgflash.rs:363` |
| `expf` and `powf` on large or NaN inputs | `expf`, `math.rs` | The series never settles for NaN or where the terms overflow (\|x\| about 12.7 to 14.7), and loops forever | `tools/leaf-check/tests/math.rs:154` |
| Kongo Jungle N64's barrel holding no one | `stageGObj1_GObjProc` (groldkongo.c), `game/gr/oldkongo.rs` | A hold (state 1) with no fighter clears the state but falls into the shot's start all the same (no `break`), and eleven ticks later reads the null fighter's `p_link`; nothing leaves a held barrel without its fighter (`grOldKongo_802105C8` clears both together) | `crates/melee-sim/src/game/gr/oldkongo.rs` (`proc_barrel`) |
| Kongo Jungle's barrel holding nothing | `grKongo_801D577C` (grkongo.c), `game/gr/kongo.rs` | The same fall-through as Kongo Jungle N64's: a hold (state 1) with nothing held clears the state, falls into the shot and reads the null object's `p_link` in the same tick; nothing leaves a held barrel empty (`grKongo_801D828C` clears both together, and a held Klap Trap can't be hit) | `crates/melee-sim/src/game/gr/kongo.rs` (`proc_barrel`) |
| A parasol state without an animation | `it_8028B08C`, `game/it/kinds/parasol.rs` | Indexes its lengths by the state's animation, -1 for states 0 and 3, and reads the word before the table; its callers ask only for the held states, 4 to 10 | `crates/melee-sim/src/game/it/kinds/parasol.rs` (`anim_frames`) |
| A sticking item's slope with no surface touched | `it_8027649C`, `game/it/maplib.rs` | Takes its angle from a stack vector it never set when the item touches no surface (`it_80276408` writes nothing then); the motion-sensor bomb sticks only after touching one | `crates/melee-sim/src/game/it/maplib.rs:291` |
| The Flipper's spin past π | `itFlipper_UpdateSpin`, `game/it/kinds/flipper.rs` | Brings the spin's speed, not the paddle's angle, back within ±π by a whole turn; the attributes cap the speed at 0.873 radians a frame | `crates/melee-sim/src/game/it/kinds/flipper.rs:175` |
| A particle list on a displayed joint | HSD's joint traversal (displayfunc.c, `efLib_Cb_SPtcl`) | Spawns a joint's particle list on its first display, whose generators then draw from the game's random seed, so a spawn put off by a doubled round would shift every later draw; no joint descriptor on the disc carries a particle list | [engine notes](engine.md#state-the-render-pass-writes) (line 150) |
| A grab box after a failed grapple beam spawn | `it_802B7C18` and Samus's `Catch`, `CatchDash` and `AirCatch` scripts, `game/ft/samus/tether.rs` | The parts table's last entry (part 139) keeps the freed tip joint after the beam goes, and her grab boxes on that bone would read it if the beam's spawn made no item before them; the spawn sets the entry anew every time it makes the beam | [engine notes](engine.md#uninitialized-values) (Samus's grapple beam's paragraph); `fighter_hitbox_from` |
| A Lip's Stick swing with nothing held | `ft_800CDB9C`, `game/ft/co/swing.rs` | Asserts the held item's kind before testing it for null, reading a null item; the swing always holds the stick | `crates/melee-sim/src/game/ft/co/swing.rs:257` |
| A file kept twice missing several mirrors | `fn_803AD16C` (the card layer's repair), `melee-save/src/hsd.rs` | Picks each missing mirror's block as the last free one, which it marks taken only when the copy is written, so every block of the file copies into the same block and only the last survives; Melee's save data takes one block | `crates/melee-save/src/hsd.rs` (`repair`) |
| A file table entry past nine files | `hsd_803AC558`, `melee-save/src/state.rs` | Takes a table entry's index byte unchecked and writes the size and flags past the file arrays into the rest of the card file's state (up to its comment and icon formats); a block must pass its checksum, and the game's blocks name files 0 to 9 only, 9 with size 0 | `crates/melee-save/src/state.rs` (`unpack_table`) |
| The saved language from a language setting past English | `gmMainLib_8015F600`, `game/gm/main_lib.rs` (`reset_card_file`) | Resetting the save data sets the saved language from a switch on the language setting with cases for Japanese (0) and English (1) only, so any other setting saves the uninitialized local `lang`; the boot sets the setting from the disc, always one of the two (melee-hd stops) | `crates/melee-sim/src/game/gm/main_lib.rs` (`reset_card_file`) |
| The memory card prompt sized for the message before | `gm_801AF250`, `game/gm/memcard.rs` (`memcard_probe_prompt`) | Shows the "load the save?" message (7) in a window sized for message 6's line count; the two have the same count in both languages (3 in English, 2 in Japanese) | `crates/melee-sim/src/game/gm/memcard.rs` (`memcard_probe_prompt`) |
| A trophy draw's guard one past its end | `Toy_80305058`, `game/gm/trophies.rs` (`draw_trophy_of_class`) | Resets a drawn index to 0 only when it exceeds the candidates' count, so an index equal to the count would read past them; `HSD_Randi(n)` is always below `n` | `crates/melee-sim/src/game/gm/trophies.rs` (`draw_trophy_of_class`) |
| The trophy screens' notice count reads a stale class | `_Toy_80304D30`, `game/gm/trophies.rs` (`trophy_class_notices`) | Its last loop tests the class of the last trophy the first loop met (`x`, `r29`) instead of each category's, so its count depends on that trophy, and with no trophy the region can win held it reads `r29` as the caller left it; every caller drops the count | `crates/melee-sim/src/game/gm/trophies.rs` (`trophy_class_notices`) |
| A rules list made coming in from neither side | `mn_80230E38` (`mnmainrule.c`), `mn_80233218` (`mnruleplus.c`), `game/mn/rules.rs`, `rule_plus.rs` | Takes its way in from an uninitialized pointer for state 2; every caller makes the list in state 1 or 3 (melee-hd stops) | `crates/melee-sim/src/game/mn/rules.rs` (`rules_list`) |
| A name list button past Erase | `mnName_80238964` (`mnname.c`), `game/mn/names.rs` | Returns `f1` unset for an index other than New, Sort and Erase; its only caller passes those three (melee-hd stops) | `crates/melee-sim/src/game/mn/names.rs` (`name_button_frame`) |
| The item switch left neither forward nor back | `fn_80234C24` (`mnitemsw.c`), `game/mn/item_switch.rs` | Takes its way out from an uninitialized pointer when the flow leaves it with neither the entering flag nor B; only B and the scene's end leave it (melee-hd stops) | `crates/melee-sim/src/game/mn/item_switch.rs` (`item_switch_proc`) |
| The keyboard's OK set twice | `mnNameNew_8023E0D8` (`mnnamenew.c`), `game/mn/names.rs` | Sets the OK button's joint (`jobjs[5]`) for the hovered key being OK, then again for it being back (`0x32`), whose joint is `jobjs[3]`; the keyboard opens on a key, so both are 0 | `crates/melee-sim/src/game/mn/names.rs` (`keys_first_frames`) |
| Brinstar's platform's columns no bubble reaches | `grZebes_801D881C`, `game/gr/zebes.rs` | Leaves an inner column's x from an uninitialized stack array when no bubble's edge reaches it; while the platform is whole its bubbles have reached every column on every tick checked (melee-hd counts a stale read) | `crates/melee-sim/src/game/gr/zebes.rs:607` |
| The results' places in a match neither free for all nor team | `fn_80177748`, `game/gm/results_panel.rs` (`results_rows`) | Takes a player's place picture from `r24`, which only the free for all's and the team battle's branches set, so another `is_teams` carries the caller's register over; `is_teams` is always 0 or 1 (melee-hd stops) | `crates/melee-sim/src/game/gm/results_panel.rs` (`results_rows`) |
| The automatic handicaps after a tie | `fn_80169000`, `game/gm/results_exit.rs` (`move_handicaps`) | Fills a stack array of the players by place, one entry a place; tied players share a place and leave the last places' entries unset, which the first place's and the last place's handicaps are then read through (melee-hd stops) | `crates/melee-sim/src/game/gm/results_exit.rs` (`move_handicaps`) |

## Unsure

- **Yoshi's egg and Koopa Klaw's union words.** Yoshi's egg (`ftCo_800BBED4`) sets its shrink's frames left from `mv.co.walk.fast_anim_frame`, the same word as the frames it just set, and clears the Captain's up special's `vel.y`, the same word as its mash timer; Koopa Klaw's victim tests its struggle's frames as the Captain's `vel.y` and Whirling Fortress counts frames in a word its start clears under another name. These may be the decompilation's union names for the same intended fields rather than slips in the game's code. Source: [engine notes](engine.md#uninitialized-values) (lines 87 and 88).
- **Popo's hammer holder.** Squall Hammer's hammer joint lean and visibility (`ftPp_SpecialS_8011F720`) runs only with a holder (`specials.x8`) that the entries clear and nothing sets, so it never runs. It may be code HAL left disabled on purpose. Source: `crates/melee-sim/src/game/ft/popo/special_s.rs:149`.
- **Luigi's `x222C`.** `Fighter_Create` leaves thousands of a fighter's bytes unwritten, which creation never reads; the decompilation notes a later read of Luigi's `x222C`, which on a console would see a previous object's bytes. Whether that read happens before a write isn't settled. Source: [engine notes](engine.md#uninitialized-values) (line 67).
- **Per-use lengths written into shared loaded data.** Samus's grapple beam (`it_802B75FC`) and Link's and Young Link's hookshot (`it_802A2BA4`) write their lengths for the user's height over their attributes in the loaded archive, which every user of that character shares in a match. It's a design choice that would misbehave only for users of different heights or grab kinds at once; whether it ever changes play isn't recorded. Source: [engine notes, loaded data the game writes into](engine.md#loaded-data-the-game-writes-into).
- **Relocations at unaligned offsets.** Some archives relocate words at offsets no pointer can have (not a multiple of 4), and the loader adds the load address there all the same. That is a defect in the disc's data rather than in the code. Source: `crates/melee-data/src/archive.rs:228`; [the porting guide](porting.md#when-to-stop-and-ask).
- **The mash out's buffered jump.** A thrown fighter's mash out (`ftCo_800DE5A4`) jumps free when `mv.co.thrown.xC` is set, a byte no thrown state writes: for a victim held before the throw it is the capture wait's buffered jump at the same offset (`mv.co.capturewait.xC`), so a jump pressed early in the hold carries over, but otherwise it is whatever the union held. That may be the intended hand-over rather than a slip. melee-hd reads the byte as the original does (it had read the whole word, which took the union's next bytes for a jump in a Kirby CPU match). Source: `crates/melee-sim/src/game/ft/co/throw.rs:492`.
- **Wobbling.** Slippi's ruleset carries wobbling prevention (`800DB880`, `800DBBD4`, `8008F090`), but melee-hd's sources describe it as a ruleset change, not a defect in the code, and players treat wobbling as a technique. Source: [the oracle](oracle.md) (lines 133 and 134).
- **The record trophies' time, stamped again at every menu entry.** `fn_80172C78`, which gives the trophies the save's records earn without a notice (each mode's clears, the three sets', every event's: `gm_80173EEC`, before every entry to the main menu), gives one when `gmMainLib_8015DA90` finds its notice bit (`x1B58`) clear, then sets the time it was got (`gmMainLib_8015D970`), its owned flag and that bit, and clears the bit at once (`gmMainLib_8015DA68`). The test reads the bit it then clears rather than the owned flag, so every later check gives the trophy again and stamps its time anew. It may be the intended way to give a trophy silently, with the time's restamping unnoticed or unused; whether the time shows anywhere isn't recorded. melee-hd models it. Source: `crates/melee-sim/src/game/gm/unlocks.rs:109`.
