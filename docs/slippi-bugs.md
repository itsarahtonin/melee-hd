# Slippi's own defects

melee-hd's Slippi ruleset writes `.slp` recordings that must be byte-identical to what Slippi Dolphin would record, and melee-hd plays Slippi replays exactly as Slippi's playback codes do. So where Slippi itself gets something wrong, melee-hd does the same (Sarah, 2026-11-15: "if they have it too then so shall we, so we can reliably make identical .slp recordings"). This catalog records each such defect once, with enough detail that it could later be reported or fixed upstream ("we should also keep track of these slippi defects. could get them fixed upstream eventually"). Nothing in it has been reported: melee-hd contacts none of Slippi's servers, issue trackers or other online services, and reading Slippi's public source is the only contact.

It lists defects in Slippi's Gecko codes, in Slippi Dolphin (including Dolphin behavior Slippi relies on), in Slippi's recorder and `.slp` format, and in its playback, that make a recording not play back as it was played, make playback differ from live play or from a console, record wrong or machine-dependent values, or leave a Melee bug unpatched where Slippi patches its neighbours in a way that matters to replays. Melee's own bugs are in [Melee's bugs](melee-bugs.md), and an entry here that concerns one links to it; bugs in ssbm-rs or in melee-hd's own ports are in neither, and Slippi's intended behavior (its gameplay codes' changes, its unlocks, its menus) isn't a defect.

Slippi Dolphin's playback resyncs each fighter's position, percent and action state and the random seed from the replay every frame (its `shouldResync`), which hides many of these from someone watching a replay: a replay that leaves its recording snaps back to it. melee-hd and ssbm-rs never resync, so the oracle sees every one ([the oracle](oracle.md), line 55).

## How to read an entry

Each entry gives:

- **Where:** the Slippi component (a Gecko code's name and address, a Slippi Dolphin setting, the recorder's event and byte offset) and melee-hd's port of it.
- **Defect:** what Slippi does wrong.
- **Effect:** what players or replays see: which replays don't play back, which values differ between machines, with the numbers the sources give.
- **melee-hd:** how melee-hd matches it: modeled as Slippi does it, the tripwire that notices it, or the note the `.slp` carries.
- **Upstream fix:** what a fix in Slippi would be.
- **Fix impact:** whether a fix changes gameplay, breaks existing replays, or splits online play with older versions. Slippi's playback applies the gameplay codes a replay recorded ([the engine notes](engine.md#players-codes)), so a fix shipped as a new gameplay code leaves older replays playing as they did; peers with and without it desync where it acts.
- **Status:** "not reported" for every entry so far; "under investigation" where the finding is still open.
- **Source:** where the defect is written down, so it can be traced.

A field no source covers says "not recorded". Line numbers are those at the time of writing.

## Adding an entry

When melee-hd finds that Slippi does something wrong that a recording or a replay depends on, reproduce it as Slippi does (never fix it in passing: the `.slp` files and playback must stay identical), then add an entry here under its area, with every field above and its source. One that Slippi has since fixed, but that older replays still carry, goes under [Fixed in later Slippi versions](#fixed-in-later-slippi-versions). A Melee bug Slippi simply doesn't patch, with no Slippi code involved, gets a line in [Melee bugs Slippi doesn't patch](#melee-bugs-slippi-doesnt-patch) and its full entry in [Melee's bugs](melee-bugs.md). One that may be intended goes under [Unsure](#unsure) with the reason. Don't fill a field the sources don't support: write "not recorded".

## Playback

### Homing missiles, nearest-opponent items and Onett's cars still follow the display

- **Where:** Slippi's determinism codes, which take gameplay off what only the display pass writes (BrawlOffscreenDamage `8006A880`, PSCameraIndependentMonitor `801D24FC`, WhispyBlowDirFix `8008653C`, FreezeDeadUpFallPhysics `800D4C1C`, `800D4D68`, `80080E80`), and the reads they leave: Samus's homing missile (`it_802B64FC` through `ftLib_FindNearestOpponentInDir`) and the items that pick the nearest opponent (`ftLib_FindNearestOpponent`) through a fighter KO'd toward the screen, and Onett's cars (`grOnett_801E43E0`, the camera's view edges from `Camera_800307D0`). melee-hd: `it::owner`, `it::kinds::samusmissile`, `ft::co::dead`'s `place_by_camera`, `gr::onett::proc_cars`, and the tripwire `live::DisplayPhase` (`Game::display_phase`).
- **Defect:** Melee's logic runs at 60 Hz and its video at 59.94 Hz, so about once every 1,001 frames the game runs two logic ticks for one drawn frame, and anything only the drawing writes is a tick older on the second tick. Where those doubled ticks fall depends on the machine's timing, so Slippi keeps gameplay off what the drawing writes rather than lining machines up. It does so for off-screen damage, Pokémon Stadium's monitor, Whispy's direction and a KO'd fighter's position, but not for a fighter falling toward the screen after a KO: the drawing places that fighter's model by the camera, and a homing missile, a red shell, a Like Like, an Octorok, a ReDead, Scizor and others aim at the nearest opponent through that model. Onett's cars likewise decide when to honk, draw a random number and set the flag computer players and the camera read from the camera's view, which only the drawing updates.
- **Effect:** a homing missile in flight while an opponent falls toward the screen can turn the other way on a doubled tick and fly on differently; a nearest-opponent item can pick another target; an Onett car's honk and draw can move by a tick. Slippi Dolphin's playback doubles its ticks at its own phase, not the recording's, so such a match can play back differently from how it was played, and two online peers can desync there. It needs the KO toward the screen, a missile or such item in play and a doubled tick together, and a target near the turn's threshold; no traced match has it.
- **melee-hd:** modeled as the original does it. Under the Slippi ruleset each nearest-opponent read through a fighter the drawing placed is also computed with the other placement, and when the outcomes differ the recording's metadata carries `playbackNote` ("may not play back exactly", with the count and the first game tick) and the player's log says so; the match plays on as the original's (design call 9 of [the Slippi ruleset](slippi-ruleset.md#design-calls)). Onett's cars aren't covered: the tripwire's comment and the ruleset plan call them unported, but `gr::onett::proc_cars` has run since `b7de1bb7`, reading the camera the display pass keeps (`camera::ground_span`), so a live match on Onett notes nothing.
- **Upstream fix:** a code like WhispyBlowDirFix for the nearest-opponent searches (skip fighters in their KO, or read the logic position, as FreezeDeadUpFallPhysics keeps it), and one like PSCameraIndependentMonitor for Onett's cars (fixed view edges).
- **Fix impact:** changes homing, item targets and Onett's cars in those rare cases; as new gameplay codes they leave older replays playing as recorded.
- **Status:** not reported.
- **Source:** [engine notes, state the render pass writes](engine.md#state-the-render-pass-writes) (lines 243, 257 and 258, and line 406 for the tripwire); [the Slippi ruleset](slippi-ruleset.md) (lines 49, 66 and 145, design call 9 at line 179); [Melee's bugs](melee-bugs.md#homing-and-nearest-opponent-targeting-through-render-placed-joints), [Onett's cars](melee-bugs.md#onetts-cars-read-the-render-pass-camera); `crates/melee-sim/src/game/live.rs` (`DisplayPhase`'s comment).

### Playback's inputs never reach the controllers: Brinstar Depths' haze

- **Where:** Slippi's playback code at `0x8006B0DC` (`Fighter_procInput+0x3CC`), which writes each fighter's recorded inputs after the game has read its controller, and its pad library patches (`HSD_PadRenewRawStatus`, `HSD_PadRenewMasterStatus`), which renew the controllers' statuses from polls at rest; the reader here is Kraid's haze's process on Brinstar Depths (`grKraid_801FE2D0`, reading `HSD_PadMasterStatus[1]`). melee-hd: `game::playback` (`playback_inject`), `gr::kraid::proc_copy`.
- **Defect:** playback hands the recorded inputs to the fighters only. Game code that reads the controllers themselves sees nothing pressed in playback, though it saw the player's presses live.
- **Effect:** on Brinstar Depths, every press of A on the second controller steps the haze behind Kraid to its next animation (a debugging leftover in Melee), and the haze's animations draw random numbers. Live, port 2's presses change the haze and shift the random sequence for long stretches; a replay never presses, so its haze differs (in a render of the oracle's recording against its playback, from 02:25.39 on), and the recording's random seed parts from playback's on seven frames, where a draw fell between a frame's start and its recorded inputs. Playback puts the recorded seed back every frame, and gameplay was unchanged in that match; whether a gameplay draw can fall after the haze's in the same frame is not recorded.
- **melee-hd:** modeled both ways: a replay's playback injects into the fighters only, and a live match under the ruleset takes the console's polls, so port 2's A steps the haze there as on a console. No tripwire notes it, and the live-equals-playback check leaves the pad library's statuses out.
- **Upstream fix:** write the recorded buttons into the controllers' statuses too (the replay's raw inputs are recorded per port), or a code that drops Kraid's debugging button in all of Slippi's play.
- **Fix impact:** the first changes only playback, which would then match the recording; the second changes the haze and the random sequence on Brinstar Depths in live play, so as a new code it would leave older replays as they are.
- **Status:** not reported.
- **Source:** [the oracle](oracle.md#stages) (line 509); [Melee's bugs](melee-bugs.md#brinstar-depths-a-debugging-button-steps-kraids-hazes-animation); `crates/melee-sim/src/game/playback.rs` (the module's documentation and `playback_inject`).

### Playback's sound log cuts a slowed game's sounds

- **Where:** Slippi's playback sound codes (`Playback/Core/Sounds`): `PreventDuplicateSounds` and `AssignSoundInstanceId` in `HSD_AudioSFXStartParam` (at 0x8038D0B0, +0xBC and +0x230), `HandleFrameSoundLog` at each tick's end in the scene loop, and `NoDestroyVoice` in `ft_800881D8`. melee-hd: `game::playback::SoundLog`.
- **Defect:** the codes keep a log of the sounds each of the last seven frames played, so a rollback that plays a frame again doesn't play its sounds twice. They take the frame from the scene's count of game frames, which only advances on a tick whose game clock comes due. In a slowed game (Slo-Mo Melee) many ticks don't advance it, and the codes take each for its frame played again with no sound, so they stop every sound that frame started.
- **Effect:** a slowed game's replay in Slippi Dolphin loses nearly all its sound effects, each stopped one tick after it starts. Gameplay is unchanged.
- **melee-hd:** modeled in a replay's playback only (`Settings::from_replay`), compared on every replay's diff (`special-slomo-fd`). The player's own matches keep their sounds (`live_slow_mo_keeps_its_sounds`); a Slo-Mo Melee match isn't rebuilt as a live match under the ruleset, so it saves no `.slp` (design call 6, `live::rebuild_refusal`).
- **Upstream fix:** count frames by a counter that advances every tick, or skip the log's check on a tick whose game clock wasn't due.
- **Fix impact:** playback's sound only; no gameplay, replay or online change.
- **Status:** not reported.
- **Source:** [engine notes, rulesets in the state](engine.md#rulesets-in-the-state) (line 408); [the oracle](oracle.md#special-melee) (line 763); [the Slippi ruleset](slippi-ruleset.md) (line 27).

### Playback's sound code changes the stack a far Arwing's laser reads

- **Where:** the same sound code in `HSD_AudioSFXStartParam` (0x8038D0B0); the reader is a far Arwing's aimed laser on Corneria and Venom (`it_802E7654`). melee-hd: `it::kinds::arwinglaser` (`spawn_aimed`), `audio::driver` (`sfx_sample`), `it::Globals::stale_last_pos`.
- **Defect:** Melee's laser never sets its spawn position, so the item copies three stack words the shot's sound left into its last position (a Melee bug). Slippi's playback code in the sound's start saves three registers there (the track, the channel and the sample index), where without it the words hold what earlier calls in the tick left. So playback runs the laser with other values than the live game.
- **Effect:** none observable: only a stomp on a stompable item reads the words, and they're tiny either way. The item's state differs between playback and a console (in all 42 shots of Event 32's session). Whether Slippi Dolphin's live play runs the same sound code is not recorded.
- **melee-hd:** in a replay's playback, modeled: the spawn computes the sample from the sound played. Elsewhere the words are stale: the comparer and the heap's image leave them out.
- **Upstream fix:** a code setting the laser's spawn position (the Melee bug's fix), which takes the stack out of it.
- **Fix impact:** none in play.
- **Status:** not reported.
- **Source:** [Melee's bugs](melee-bugs.md#a-far-arwings-laser-takes-its-spawn-position-from-the-stack-the-sound-left); [the oracle](oracle.md) (line 1689).

### Playback rebuilds a match from its rules alone: Special Melee's modes don't play back

- **Where:** Slippi's recording codes, which write the match's start data with its callbacks cleared, and its playback codes, which boot every replay into the debug VS mode (scene 14.1) and rebuild the match from that start data at the VS scene's entry (`fn_8016E730+0x18`, `Playback/Core/RestoreGameInfo.asm`). melee-hd: `vs_match::Settings::from_replay`, `game::live`'s `rebuild_refusal`.
- **Defect:** a match's rules are recorded, but not the game mode or the callbacks a mode adds, so a mode that changes play through them plays back as a plain VS match.
- **Effect:** in a Stamina Mode replay a fighter out of HP lies down and is never counted out, so playback runs on to the recording's last frame (`special-stamina-ps`, 30,887 frames); Stamina Mode's knockout slowdown, Super Sudden Death's music, Fixed-Camera Mode's camera and a Sudden Death's own entry and announcement don't play back either. Whether Slippi Dolphin saves a `.slp` of each of these modes is not recorded.
- **melee-hd:** under the ruleset these matches run in the menus' session and save no `.slp` (design call 6: VS mode's, Tournament's and Invisible, Tiny, Giant, Lightning and Single-Button Melee's are rebuilt; Sudden Death, Super Sudden Death, Stamina Mode, Fixed-Camera Mode, Slo-Mo Melee and any start data holding a callback aren't). Sarah tabled the gap rather than conceding it ("i don't wanna concede those areas as unrecordable").
- **Upstream fix:** record the game mode (and what its callbacks need) in the game start, and have playback rebuild it.
- **Fix impact:** a format and playback change; no gameplay change, and older replays stay as they are.
- **Status:** not reported.
- **Source:** [the oracle](oracle.md#special-melee) (line 745); [the Slippi ruleset](slippi-ruleset.md) (line 27, and the decisions' call 6 at line 190).

### A paused match's recording

- **Where:** Slippi's recording codes: the frame start event's scene frame count (`+0x09`), which they copy from the scene's tick count, and the post-frame update's fifth state flag byte (`+0x2A`; `+0x29` of the payload without its command byte, as ssbm-rs's replay check counts), which `SendGamePostFrame.asm` copies from `Fighter`'s `0x221F` at `Fighter_procCamera`'s end: bit `0x80` is `x221F_b0`, the off-screen mark the drawing sets (`ftLib_UpdateScreenVisibility`). melee-hd: `game::record`'s `frame_start` and `flag_bytes`, `ft::draw`'s `update_screen_visibility`.
- **Defect:** the recording holds two things a pause leaves that playback can't reproduce, since a replay never pauses (playback's controllers are at rest). The scene's tick count goes on through the pause, so every frame start after it records a count the paused ticks ahead of playback's. And the drawing goes on through the pause with the pause camera, zoomed in on the player who paused, so a fighter out of that view is marked off screen; the frames after the pause record the mark until the camera's ease back from the pause view frames the fighter again, while playback, which never left the match's view, records it clear.
- **Effect:** a paused match's replay never re-records exactly, in Slippi Dolphin's own recordings too: every pause in Sarah's Slippi Dolphin 3.19.1 replays (five, in four replays of 2026-09-15 and 16) records port 2 marked off screen on the frames after it (1 to 13 frames; in three of them port 2 stands on the stage, where the match's view shows it) and a jump in the scene frame count. Under Slippi's codes neither reaches gameplay: BrawlOffscreenDamage counts off-screen time from the stage's limits, and the next drawing rewrites the mark before anything reads it, so the match plays back the same; only a tool or check comparing the recording with its playback sees it. In melee-hd's sessions, ssbm-rs's playback (Slippi's playback codes) of the live recordings differs in exactly these: `matches-a-1` (a 90-frame pause) in 311 frame starts and one mark, `pause-far` in 450 and 13, `pause-fd` in 240 and 12.
- **melee-hd:** modeled as Slippi records it: the live match runs the pause as the console does, and the recorder reads the mark and the count where the codes do. ssbm-rs's trace of the same sessions (the original code from power-on) holds the same marks as the live match's recording on every frame compared (`melee-oracle live-check`), the frames after each pause included: `pause-fd`'s whole match, 429 frames with 81 marked, and `matches-a` and `pause-far` to their first item, past which the live match and the console's run part ([the oracle](oracle.md), "The Slippi ruleset's live matches"). `melee-player`'s `a_paused_live_match_records_the_pause_as_slippi_does` holds that the live recording differs from its playback's re-recording in these two things only. The same mark the drawing sets is behind [the off-screen flag's timing](#the-off-screen-flag-follows-the-machines-timing).
- **Upstream fix:** record the logic's off-screen test (BrawlOffscreenDamage's limits) rather than the drawing's mark, as that entry suggests, and record the count of frames, not ticks (or mark the count as including paused ticks in the spec).
- **Fix impact:** no gameplay or online change (online play doesn't pause); tools reading the flag or the count would see the new values from the fix on.
- **Status:** not reported.
- **Source:** [the oracle](oracle.md#sessions), "The Slippi ruleset's live matches"; [the Slippi ruleset](slippi-ruleset.md), task 4's entry; [the player](player.md#recording).

## The recorder and the `.slp` format

### The post-frame "hitstun" word records heap addresses

- **Where:** the post-frame update's misc action state word (offset 0x2B; Slippi's spec reads it as the hitstun left), which the recording code copies from `Fighter`'s 0x2340, the current state's first variable. melee-hd: `game::record`'s `misc_as`.
- **Defect:** the recorder copies the word whatever the fighter's state keeps there, and some states keep a memory address in it: Yoshi's GObj while a fighter is in his egg, the item's address in Pikachu's and Pichu's Thunder, a Super Scope's address in its charged click ([Melee's bugs](melee-bugs.md#the-super-scopes-affordable-level-returns-its-gobj-infinite-ammunition)), one of Ness's states. An address depends on how the machine laid out its memory, so the recording holds a value no other run need reproduce.
- **Effect:** the word differs between a recording and its playback, which lays memory out its own way (Slippi Dolphin's playback codes; its Show Player Names option alone moves it): 9 of Sarah's 172 replays checked differ there, and the oracle's recordings on hundreds of frames each (526 in Green Greens' match, 476 in Kongo Jungle's). Nothing reads it as a number, and playback never reads it; a tool reading it as hitstun gets nonsense in those states.
- **melee-hd:** records the address its allocator model gives the object (playback's layout in a replay or a live match), 0 without the model; the replay checks know it as layout noise.
- **Upstream fix:** record 0, or a defined value, when the state keeps an address there, or record hitstun from the field it means.
- **Fix impact:** no gameplay or online change; tools reading the word would see different values from the fix on.
- **Status:** not reported.
- **Source:** [the oracle](oracle.md) (lines 221, 359, 530, 574 and 580); [the player](player.md#recording) (line 854).

### Items' misc bytes record what memory held

- **Where:** the item update's four misc bytes (offsets 0x26 to 0x29), copied from the item's variables at `0xDD7`, `0xDDB`, `0xDEB` and `0xDEF`. melee-hd: `game::record`'s `item_misc`.
- **Defect:** the bytes mean something for a few items (a Samus missile's kind, a turnip's face, a Charge Shot's launch and power), but the recorder copies them for every item, and most items never write them, so they hold whatever memory held before.
- **Effect:** the bytes depend on what ran before and on how memory is laid out, so a recording and its playback can disagree there; comparisons of recordings leave them out (`ssbm_slippi::compare`).
- **melee-hd:** writes the bytes of the variables it keeps as types (those four items' fields) and zero for others.
- **Upstream fix:** record the bytes only for the items that set them, 0 for others.
- **Fix impact:** none in play; tools reading them for other items would see 0.
- **Status:** not reported.
- **Source:** [the oracle](oracle.md#recordings) (line 888); `crates/melee-sim/src/game/record.rs` (`item_misc`).

### The off-screen flag follows the machine's timing

- **Where:** the post-frame update's fifth state flag byte (offset 0x2A, `Fighter`'s 0x221F: bit 0x80 is `x221F_b0`, set when the drawing finds the fighter off screen). melee-hd: `ft::draw`'s `update_screen_visibility`, `game::record`'s `flag_bytes`.
- **Defect:** the flag is set by the drawing, so it lags a tick around a doubled tick (above), and where those fall depends on the machine. Slippi's BrawlOffscreenDamage keeps it out of gameplay, but the recorder still records it.
- **Effect:** the flag differs between a recording and its playback on frames near a doubled tick: in 31 of Sarah's 172 replays checked on the entrance's frames -122 to -119, and in 10 on one frame later in the match, each at the same place in the 1,001-tick cycle; a 3.16 ranked replay differs on 12 frames from frame 928. A pause marks it too, from the pause camera's view ([a paused match's recording](#a-paused-matchs-recording)). No gameplay effect under Slippi's codes.
- **melee-hd:** the flag is the display pass's at melee-hd's own doubled ticks, as ssbm-rs's; the replay checks know it as noise.
- **Upstream fix:** record the logic's test (BrawlOffscreenDamage's limits) instead, or mark the bit display-only in the spec.
- **Fix impact:** no gameplay change; tools counting time off screen would see the logic's test.
- **Status:** not reported.
- **Source:** [the oracle](oracle.md) (lines 43, 219 and 220); [engine notes, state the render pass writes](engine.md#state-the-render-pass-writes) (line 251).

### A player who leaves an online match alive isn't recorded

- **Where:** Slippi 3.19.1's `.slp` format (no event marks a player leaving) and its playback codes (nothing removes a player). melee-hd: `game::playback`, `melee-oracle record`.
- **Defect:** when a player disconnects from an online teams match while still alive, the recording stops writing that fighter's updates from that frame, but records nothing saying the player left. Playback keeps the fighter in with blank inputs.
- **Effect:** the replay desyncs from its recording a few frames after the leave. Two of Sarah's replays, both four-player matches on Pokémon Stadium: `Game_20260531T181904` from frame 11,319 (port 4 left on the ledge with 1 stock after frame 11,306) and `Game_20260531T185444` from frame 2,192 (port 1 left standing with 4 stocks after 2,186; the frame seed from 2,281, items missing from 2,664). The game end gives the leaving port no placement. Of the 141 players in Sarah's 203 replays whose updates stop before the end, these are the only two not knocked out.
- **melee-hd:** plays them as Melee does, as ssbm-rs's original-code baselines do (they diverge in the same lines). A port whose updates stop while it still has stocks marks such a replay.
- **Upstream fix:** record an event (or a field in the game end) for a player's leave with its frame, and have playback remove the fighter there as the online game did.
- **Fix impact:** a format and playback change; no gameplay change for players; older replays still don't play back.
- **Status:** not reported; that Slippi Dolphin's own playback desyncs the same way is likely (nothing in its playback set removes a player) but not confirmed by running it.
- **Source:** [the oracle](oracle.md) (line 223); commit `0f6b2566` ("docs: Explain the two Pokémon Stadium replays that don't play back").

## Slippi Dolphin

### Slippi Dolphin's floating point isn't the GameCube's

- **Where:** Slippi's netplay Dolphin (the Ishiiruka fork), whose x86-64 JIT recorded every Slippi replay. melee-hd: `gekko_fp::FpMode::Slippi` (`crates/gekko-fp/src/lib.rs`).
- **Defect:** the JIT computes some floating-point operations differently from the GameCube's CPU: fused multiply-adds round to double and then to single, `fnmadd` and `fnmsub` give the other signed zero when the result cancels, stores round to nearest instead of truncating, invalid operations give x86's negative NaN, a subnormal multiplier keeps its bits, and `fctiw` doesn't mark a negative zero.
- **Effect:** a Slippi replay plays back exactly only with Slippi Dolphin's arithmetic: every Slippi replay the oracle checks plays in that mode, and the same inputs on a console can compute positions differently. How often that changes a match's outcome is not recorded.
- **melee-hd:** the Slippi ruleset runs the whole session in Slippi's mode (design call 10), and so does every replay's playback in the oracle; Vanilla runs the console's.
- **Upstream fix:** make the JIT's results match the GameCube's.
- **Fix impact:** changes results by the last bits, which grow over a match: every existing replay would stop playing back in a fixed Dolphin, and fixed and unfixed peers would desync; it would need the old arithmetic kept for older replays.
- **Status:** not reported.
- **Source:** [the Slippi ruleset](slippi-ruleset.md) (line 37); `crates/gekko-fp/src/lib.rs` (`FpMode`).

### EFB copies stay on the GPU

- **Where:** Slippi Dolphin's graphics setting "Store EFB Copies to Texture Only", its default and Sarah's netplay setting. melee-hd: `Game::copies_reach_memory`, `Image::pre_copy_bytes`.
- **Defect:** the game copies parts of its picture into main memory (shadows, the magnifier's bubbles, the results screen's portraits). With this setting the copies stay on the GPU and memory keeps what it held before, where a console's memory holds the pixels. Where the game later reads that memory as data, it reads other bytes than a console.
- **Effect:** where an item's block is carved from a freed copy buffer, a dropped fan lands or vanishes by a word that holds the copy's pixels on a console (a shadow's pixels always land it) and the bytes from before the copy in Slippi Dolphin; a collision joint over a shadow's buffer reads its stamp the same way. The sources show it in a Multi-Man match (`mh-15min-kongo`, where a Wire Frame had left); whether a VS match reaches it is not recorded. A player with the setting off would run a different game from one with it on; whether Slippi enforces the setting online is not recorded.
- **melee-hd:** under the Slippi ruleset and in a replay's playback the reads see the bytes from before the copy, as Slippi Dolphin's (Sarah, 2026-11-07); Vanilla keeps the console's.
- **Upstream fix:** copy to memory as a console does (Dolphin's "EFB copies to RAM"), at a cost in speed, or a code that clears the blocks the game reads so neither setting reaches play.
- **Fix impact:** changes outcomes only where the game reads copied pixels; a replay that reached such a read would play differently.
- **Status:** not reported.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values) (lines 167 and 168); [Melee's bugs](melee-bugs.md#a-dropped-fan-lands-or-vanishes-by-a-word-it-never-sets).

## Fixed in later Slippi versions

Older replays still carry these, and melee-hd plays them as Slippi's playback does.

### 3.16 replays don't record the raw C-stick

- **Where:** Slippi 3.16's pre-frame update (no raw C-stick bytes) and UCF 0.84's cardinal snap (`Fighter_procInput+0x750`), which reads the raw input bytes playback writes back from the replay. melee-hd: `game::ucf`, the recorder's pre-frame raw C-stick.
- **Defect:** UCF snaps a stick near a cardinal to exactly 1.0 from the raw input bytes, after the recorder has saved the stick's 0.9875. 3.16 replays record the main stick's raw bytes but not the C-stick's, so in playback the C-stick's snap never happens.
- **Effect:** a 3.16 replay leaves its recording where a cardinal smash DI at the end of hitlag moved the fighter 3.0 live and 2.9625 in playback: in 20 of 51 sampled 3.16 replays, none from 3.15 or 3.17. Slippi Dolphin with its resync off writes the same differences as ssbm-rs (803 on `hf-roy-fox-ys-b0c1d064`, 6,826 on `hf-roy-puff-fod-9636cc09`).
- **melee-hd:** plays them as playback does; such a replay stays a comparison with ssbm-rs, not with its recording. melee-hd's own `.slp` records the raw C-stick, as 3.17's format does.
- **Upstream fix:** done in 3.17, which records the raw C-stick.
- **Fix impact:** none left; 3.16 replays keep it.
- **Status:** not reported; fixed in 3.17.
- **Source:** [the oracle](oracle.md) (line 55, and lines 72 and 86).

### 3.16 sets each frame's seed after the stage's first draws

- **Where:** Slippi 3.16's online codes, which set each frame's seed (the frame number over the match's seed) and record it at the frame's start, and the playback codes' process that puts the recorded seed back (`playback_restore_seed`, the GObj process at `0x8016E748`'s entry +0x4B0). melee-hd: `game::playback` (`SEED_PROC`).
- **Defect:** 3.16's online game set the frame's seed after Final Destination's particle generators had drawn from it, while playback puts it back before them, so in such a frame every later draw in playback is one step on from the online game's.
- **Effect:** on the Ganondorf against Captain Falcon 3.16 replay, the pre-frame updates' seed differs from playback's at frames -123, 3877 and 7877; nothing gameplay draws then, so only the particles differ, which no replay records. A gameplay draw in such a frame would part any playback of a 3.16 replay from the online game.
- **melee-hd:** plays them as playback does, as ssbm-rs does.
- **Upstream fix:** done in 3.19, whose codes set the seed where playback puts it back.
- **Fix impact:** none left; 3.16 replays keep it.
- **Status:** not reported; fixed in 3.19.
- **Source:** [the oracle](oracle.md) (line 208).

### 3.16's NanaDeterminism leaves a register a computer player's DI reads

- **Where:** NanaDeterminism (`800AC5B8`, in `ftCo_800AC5A0`, a computer player's DI with no knockback), 3.16's variant. melee-hd: `ft::cpu_move` (`cpu_di`), `ft::cpu_ai::Leftover`, the variant from `Rules::codes`.
- **Defect:** Melee's DI with no knockback pushes the stick by two registers it never set ([Melee's bugs](melee-bugs.md#a-computer-players-di-with-no-knockback-reads-two-registers-it-never-set)). 3.16's code clears the one that holds the fighter's address (r30), but not the other (r5), which for Nana on her way back to Popo can hold her held item's address.
- **Effect:** in a 3.16 Ice Climbers match where Nana, holding an item on her way back, takes a hit with no knockback, her stick follows the item's address, which differs between the online game's memory layout and playback's. Whether any replay reaches it is not recorded.
- **melee-hd:** modeled: 3.16's code clears r30 only, and the register's value is followed through the computer player's frame; a value not followed stops by name.
- **Upstream fix:** done in 3.19, whose NanaDeterminism clears both registers.
- **Fix impact:** none left; 3.16 replays keep it.
- **Status:** not reported; fixed in 3.19.
- **Source:** [engine notes, uninitialized values](engine.md#uninitialized-values) (line 189); [the oracle](oracle.md) (lines 180 and 188).

## Melee bugs Slippi doesn't patch

Slippi's codes patch a set of Melee bugs that reach online play ([the Slippi ruleset](slippi-ruleset.md#melees-bugs-and-slippis-fixes)). These, with no Slippi code involved, can still make a Slippi recording differ from its playback, or online peers differ, because Slippi Dolphin's live game and its playback lay out and fill memory differently (the netplay code set reshapes the heaps; playback's starts each match from a fresh heap, and its Show Player Names option adds objects). Their entries are in [Melee's bugs](melee-bugs.md):

- [Homing and nearest-opponent targeting through render-placed joints](melee-bugs.md#homing-and-nearest-opponent-targeting-through-render-placed-joints) and [Onett's cars read the render-pass camera](melee-bugs.md#onetts-cars-read-the-render-pass-camera): the display's timing ([above](#homing-missiles-nearest-opponent-items-and-onetts-cars-still-follow-the-display)).
- [A collision joint's stamp starts as the heap held it](melee-bugs.md#a-collision-joints-stamp-starts-as-the-heap-held-it), [a dropped fan lands or vanishes by a word it never sets](melee-bugs.md#a-dropped-fan-lands-or-vanishes-by-a-word-it-never-sets), and the other leftovers in items', links' and effects' blocks: Slippi's Init Player Data and Init Stage Data clear only the fighters' and the stage's blocks, so these read what memory held. In Sarah's 172 replays checked, none reached gameplay.
- [The Super Scope's affordable level returns its GObj](melee-bugs.md#the-super-scopes-affordable-level-returns-its-gobj-infinite-ammunition): the ammunition becomes the scope's address; every address takes the same branches, so only the ammunition count and the fighter's state word differ between layouts (and the state word reaches the recording: [above](#the-post-frame-hitstun-word-records-heap-addresses)).

Source: [the Slippi ruleset](slippi-ruleset.md) (lines 48 and 162); [engine notes, state the render pass writes](engine.md#state-the-render-pass-writes) (line 255); [the oracle](oracle.md) (lines 217 and 221 to 225).

## Unsure

- **A fighter's height as it enters is one ulp off.** On a fighter's first frame on the stage (frame -118, -113, -108 or -103 by port, and the next frame's pre-frame update) the recorded height and playback's differ in the last bit: 370 such pairs in 102 of Sarah's 172 replays checked, equal from then on. The sources put it down to the memory layout without naming the read, so whose defect it is (Melee's, read differently under Slippi's layouts, or Slippi's) is open. Source: [the oracle](oracle.md) (lines 219 and 509).
- **The metadata's frame counts include rollback's resent frames.** On online replays with rollbacks, the metadata's per-character frame counts count every post-frame update Slippi Dolphin received, resent frames included; they may be meant as updates received rather than frames played. Source: [the oracle](oracle.md#recordings) (line 893).
- **The code list's last message carries the block before's bytes.** The recording codes send the code list in split messages through one buffer they reuse, so past a short last block the message keeps the previous block's bytes. Parsers read only the declared size, so it changes nothing; melee-hd writes the same bytes. Source: [the oracle](oracle.md#recordings) (line 897); `crates/melee-sim/src/game/record.rs` (line 471).
