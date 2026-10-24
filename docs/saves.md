# Saves

`melee-save` (`crates/melee-save`) reads Melee's save files and writes them as a console does: raw memory card images (`.raw`, ssbm-rs's `.card`) and `.gci` files, judged as the game judges a card it loads, and written byte for byte as the console's card layers write them. It holds no simulation code: the memory card scene, `lbcardgame` and the native save that answers the game's card tasks (milestone 5's task 4, `melee-sim`'s `lb::save`) build on it, and the player's import and export (task 18). Where the player keeps the save is decided ([milestone 5](milestone-5.md#design-calls)); the player isn't wired to it yet.

## The layers

The game moves its save through four layers ([milestone 5](milestone-5.md#how-the-game-uses-the-card)). The crate has the lower three, each following the original's code:

| Layer | Original | Module |
| --- | --- | --- |
| Tasks | `lbcardnew.c`: lists of tasks (mount, check, open, read, write, create, status, delete, format, list snapshots, find a file), each run only when the one before ends with a result its mask takes | `tasks` (the lists `lbcardnew`'s builders make, in `tasks::lists`) |
| HSD card | `sysdolphin/baselib/card.c`, `crypt.c`: a card file of up to nine logical files in checksummed, enciphered blocks with sequence numbers | `state` (the format's arithmetic), `hsd` (each request's commands), `exec` (the command ring), `crypt`, `image` (a file's blocks deciphered without loss) |
| SDK | `dolphin/card`: the card's ID, two copies each of the directory and the block allocation table | `card` |

`melee` has the game's own files (the save's manifest and a snapshot's) and the load (`lb_8001BD34`), save (`lb_8001BE30`) and create (`lb_8001BC18`, `lb_8001BB48` for a snapshot) the game runs; `gci` the `.gci` format.

Every card or HSD call completes at once, as on ssbm-rs's card, whose erases and programs finish immediately; how many calls of `lbCardNew_CompleteNextTask` each task takes is the native save's to model (task 4). Nothing panics on a card's contents: each layer answers with its codes (`CardError`, the HSD layer's results, `lbCardResult` in `tasks::lb`), and `melee::Verdict` sums up a load.

## The save on the card

`SuperSmashBros0110290334`, 11 blocks: a direct-color banner and one indexed icon, so a header of 0x1E40 bytes (comment, banner, icon and palette) and its 0x30-byte digest, then block 0 in the rest of that sector, unused (the save has no file 0, and block 0 stays the zeros the header write left). Then ten blocks, each 0x20 bytes of header (checksum, logical id, sequence number, three entries of the file table) and up to 0x1FE0 of data, enciphered with the header:

| Physical | Holds |
| --- | --- |
| 1 and 10 | The save data (`GmSaveData`, 0x1790 bytes, flags 0: kept twice), both copies rewritten with the next sequence number on each save that changes it |
| 2 to 8 | The seven name tag banks (0x1F2C bytes each, flags 1: once, with a spare) |
| 9 | The spare: a bank's write goes to its stale copy or a free block, and the old copy turns stale, so the banks rotate through the blocks as they change |

A write first verifies what's there: when every block already holds the data at the newest sequence number, nothing is written. Each `CARDWrite` stamps the file's directory entry with the clock and writes the directory again (alternating its two copies, the check code one higher each time), so a save that changes the save data writes the directory twice. The header is verified the same way before a save's status update, which rewrites it (and stamps the entry) only when it differs.

Snapshots are files named by the clock's seconds, one logical file kept once (flags 3), from block 0 on: 32 blocks in the corpus.

## What a load sees

The load mounts the card (`CARDMount`'s checks: the ID's checksum, encoding and serial against the slot's flash ID, both copies of the directory and table), checks it (`CARDCheck`, which rewrites a bad copy from the good one and frees orphaned blocks), opens the file (its status, a read of the header with its digest, a scan of every block, the repair) and reads each logical file. The repair picks each file's newest whole set of blocks, marks older copies of files kept once stale, and for files kept twice clears surplus copies and copies a missing or out-of-step mirror from the other. So a load can write to the card; `melee::load` leaves the card as the game would.

| Verdict | When |
| --- | --- |
| `Loaded { open: 0 }` | The save read |
| `Loaded { open: -0x104 }` | No set of a file's blocks at one sequence number: the newest of each read |
| `NoCard` | No card in the slot |
| `Broken` | The ID, both copies of the directory or of the table, or a file's chain is bad (one bad copy is repaired) |
| `Encoding` | Formatted for the other font encoding (Shift-JIS on a US console) |
| `NoFile` | No save; another game's file of the same name doesn't count |
| `BadFile { hsd }` | The file's block count, header digest or icon address isn't the layer's (`lbCardResult` 3) |
| `Damaged { files }` | A logical file has a block missing or out of step, or one fails its checksum |

A console's slot takes the card's flash ID from the card; ssbm-rs's slot A, which made the corpus, keeps Dolphin's (`DOLPHINSLOTA`), and `Console::ssbm_rs` is that slot.

## Writes

`melee::save` and `melee::create` write as the game's tasks do, and `Card::format` formats as `CARDFormat` does. The clock is an explicit input (`card::Clock`, seconds since 2000-01-01), read wherever the library stamps the directory; `melee::comment` makes the save's comment from it (`lb_8001C658`, English or Japanese). The banner and icon come from the disc (`LbMcGame.dat`'s `MemCardIconData`, one of three banners by unlock state) and are the caller's to give.

A create writes each file's last block whole, so blocks take what follows each file in memory ([Melee's bugs](melee-bugs.md#a-new-card-files-blocks-run-past-their-files)): the next bank's first bytes, and after the seventh bank 0xB4 bytes of the game's memory, which `SaveData::after` carries.

A `.gci` is a file's 0x40-byte directory entry, as the directory holds it, then its sectors. `Gci::from_card` exports one; `Mounted::insert` brings one into a card (melee-hd's own import: the blocks allocated as `CARDCreate` allocates them, the table and the directory written once each); `Gci::folder_name` is Dolphin's GCI folder name (`01-GALE-SuperSmashBros0110290334.gci`).

## The native save on the timeline

`melee-sim`'s `lb::save` runs the game's task lists on the session's card (`CardDevice`, lent to each step like the allocator model): the list's tasks run at once through `tasks::Run`, the card data's memory as the files' buffers, and the time ssbm-rs's card takes passes on the timeline (`CardTime`). Every list starts with the mount, run at once from the list's start, then waits on `lbCardNew_CompleteNextTask` (`hsd_803AAA48`, a wait point on ssbm-rs's console when a card was in at power-on), and ends with the unmount.

A read is a chain of 512-byte segments: the code that asks for it issues the first with interrupts off, up to its DMA, which completes at once on ssbm-rs's card and raises the channel's transfer-complete interrupt (EXI0 TC, OS interrupt 10); its handler issues the next segment in 160 ticks (the deselect, the select again, three commands and the DMA: `clock::CARD_SEGMENT`), and so on, the last segment's handler finishing the transfer. Interrupts never come back on between the segments, so an event that comes due during a read runs at the next segment's boundary, and its interrupt goes ahead of the chain when the OS takes it first (the audio interrupt's, 5) and after the whole chain otherwise (the retrace's, 24; `Clock::card_chain`). ssbm-rs's probe of the boot's load and the scene's probe on five sessions (every interrupt change, EXI transfer, register read, event and interrupt delivery: `$SSBM_RS_LOCAL/research/card-chain`) gives each part's time:

| Part | Time base ticks |
| --- | --- |
| The mount's `CARDProbeEx`, interrupts off | the slot's probe and the ID's (48 with the card settled, `CardDevice::exi_probe_timed`) |
| The mount's first segment: the attach's two probes, `DoMount`'s status commands, the DMA (`MOUNT_FIRST`) | 384, interrupts off |
| The mount's other 79 segments (the ID, both directories, both tables) | 160 each |
| The mount's last handler and callbacks (`MOUNT_TAIL`) | 64, and 8 more the first time an ID passes its checksums (`VerifyID`'s `OSGetFontEncode`, which keeps the answer: `CardDevice::font_encode_known`) |
| A block's read from its step (`BLOCK_FIRST`: the select, three commands, the DMA), then 15 more segments, then its last handler (`BLOCK_TAIL`) | 96, 160 each, 64 |
| `CARDUnmount` at the list's end (`UNMOUNT`) | 24, interrupts off |

| List and outcome | Steps after the mount |
| --- | --- |
| A load that opens the file (intact or damaged) | 23: the check and the open (no transfer), then twenty block reads (steps 2 to 13 and 15 to 22), steps 14 and 23 none |
| A load that finds no file, a full card, or a broken one | 1: the check |
| A probe (`lb_8001C87C`) or a search for another game's file (`lb_8001C2D8`) | 1: the check |
| No card, or one pulled or not settled | none (the mount's failed probe takes a probe's time, below) |

The step counts are ssbm-rs's (`CARD_STEPS=1`, `CALLS=EXIDma,EXIImm,EXISync,hsd_803AAA48`). Until 2026-10-23 the transfers were fitted register reads with interrupts taken at each (`MOUNT_WORK` 1,643, `BLOCK_WORK` 323, `CHECK_WORK` 2 and `LOAD_WORK` 1): their sums came out right wherever a wait ended at an event, but each block was 24 ticks long (the unmount's 24 taken in the last block) and an event due inside a block ran at once; `clock-odd`'s audio frame and `progressive`'s last seven blocks ([the oracle](oracle.md#sessions)) showed both. `lbCardNew_CompleteAllTasks` after each scene is one more wait.

`lbCardNew_ProbeEx` (the memory card scene's probe every frame) and the pad alarm's probes (`lb_8001C600`, `lbSnap_8001D2BC`, at each alarm's time) ask the device: no card, a card not settled (the EXI library answers only three tenths of a second after it first sees one, `__EXIProbeStartTime`), or one that answers. A session's card can be pulled and put back at video fields (`Setup::card_pull`, ssbm-rs's `CARD_REMOVE=F,G`; `melee-oracle session --card-pull F,G`). Each probe takes the time of the SDK's `__EXIProbe` on ssbm-rs's console (`CardDevice::exi_probe_timed`, [the engine notes](engine.md#audio)): the channel's status register, again to clear an insertion interrupt, the time base with a card in, and the card's ID read once it answers after a pull. The scene's probe (`CARDProbeEx`) takes 48 ticks with a card that answers (the probe and the ID's own probe), 24 with one settling and 8 with a pulled card's slot empty; the pad alarm's three take 56 with the card in and settled; a task list's mount runs while the channel is attached, when a probe reads the status register alone. With no card at power-on ssbm-rs stands in for the CARD library and none takes time.

## The save written from the menus

The main menu's entry marks the save to write (`lbCardGame_SaveChanges`: the power-on time added to the save's play time, `dirty` set), and from then on each of the game's own waits runs the writer, `lb_8001CC84` ([the engine notes](engine.md#audio)): the first starts the save's list (`lb_8001BE30` through `dont_inline_helper`: the card's status brought up to date, the comment dated from the clock, `lb_8001C658`, the banner the unlocks pick, `getCurrentIcon`, and the icons), each later one a step of it (`lbCardNew_CompleteNextTask`), and the card indicator draws while it runs (`x10`). The list (mount, check, open, set status, write) runs on the lent card through `tasks::Run` at its start, as the lists the game waits on do: the HSD layer verifies each file's blocks and writes only those that differ, and stamps the directory.

Its time is the card's work, a step at each of the game's waits after the start (`lb::save::save_steps`, handed to the clock as `CardWork` through `Frames::game_wait`): at the start the comment's `OSGetTime`, the mount's probe (`CARDProbeEx`, the slot's and its ID's) and the mount's segment chain, as a load's (`MOUNT_FIRST`, `MOUNT_SEGMENTS`, `MOUNT_TAIL`, with the font encoding's read the first time an ID gets that far); then, each after its wait point (`hsd_803AAA48`), the check's and the open's step (no transfer), a step for each sector the HSD layer reads (a block's segment chain, `BLOCK_FIRST`, `BLOCK_SEGMENTS`, `BLOCK_TAIL`, as a load's blocks: the open's scan, the status set's header verify, each file's block verifies) or writes (`Layer::log`; a request starts in the call that issues its first transfer, and a later request of the same task in the call its previous one ends in), between two tasks the call that sees the first's last transfer done, runs its callbacks and starts the next (`executeNextTask`, whose requests the HSD layer takes up at its next call), and the list's end, the call that sees the last transfer done, runs the callbacks and unmounts the card (`UNMOUNT`), detaching the channel, so the pad alarm's probes after it take their full time. Each file's result (`lb_8001B6E0`) changes where the original's does: at its request's callback (`fn_8001A0B0`, in the call that sees its last transfer done) and, for the write, when the write task queues it (`taskWrite`, `Run::write_queued`). The list's end leaves the writer idle and a failure noted (`fn_8001CC30` marks the card failed), and the game's side (`Running::card_writer_take`) takes up what the waits did at the scene loop's next tick and before the display pass; a scene's end runs the rest of the list (`lb_8001CDB4`), each step's wait point there.

Writes are fitted, a stopgap until a probe of a save's writes gives their interrupts (a block write is a sector's erase and 64 page programs of 128 bytes, each page two interrupts); each fit is a named constant in `lb::save`:

| Constant | Value | What | What fixes it |
| --- | --- | --- | --- |
| `SAVE_WRITE_WAITS` | 2 waits | a sector the list writes: its transfers, then a wait the HSD layer stays busy through | with one, `2026-10-06_17-44-10`'s tick 274 ends a step early and `2026-10-06_12-42-06`'s tick 163 has the writer done (`x10`) a step before the trace's; ssbm-rs's step log (`CARD_STEPS`) shows no step for the wait between |
| `SAVE_WRITE_WORK` | 323 register reads | a sector the list writes | nothing yet: no session's tick ends on a write |
| `SAVE_OTHER_WORK` | 0 register reads | a status set or a file created (a directory's write) | nothing yet: the save's list does neither while the file is intact |

The steps' order and count otherwise follow the layers (`melee-save`'s run of the list), not a fit, and everything else is exact: what's written (the card's bytes, by `melee-save`'s layers), the tasks' order, the comment's date, the results the game reads and when the indicator draws.

## Checks

- **leaf-check `card`** runs `crypt.c`'s five functions and the HSD layer's format and request functions against ssbm-rs's on the original's `CardState` bytes: every byte pair through `encryptByte` and `decryptByte`, thousands of buffers through the checksum, encryption and decryption, and random states (the save's layout, a snapshot's, random files of every flags, blocks scattered with stale copies and sequence numbers about to wrap) through each planner and the repair, comparing the state after, every queued command's fields, the active request and the result.
- **The corpus** (`crates/melee-save/tests/corpus.rs`, reading `$SSBM_RS_LOCAL`): the 1,155 card images ssbm-rs's runs left (1,132 of the lockstep campaign, its 6 fault cases, the 13 beside the recorded sessions, 4 in `local/cards`). Every image and each of its 1,212 files reads and writes back byte for byte, and all 11,759 valid blocks are what the layer's block write makes of their contents (1,143 of them written whole by a create). The load judges 1,125 saves loaded, 14 cards with no save, 10 broken, 3 of the other encoding and 3 of ssbm-rs's hand-made layouts damaged; the fault cases as they were made (one bad directory repaired, both bad broken, Shift-JIS refused), and the card whose one good table is the format's, older than the save, broken, as `CARDCheck` finds its chain running into free blocks. The three freshly made saves in the corpus come out of a format and a create byte for byte, card and all, and each of the twelve recorded sessions' cards turns into the next one's through the saves the session made (and, once, a snapshot), all 512 KiB.

Run them with `cargo test --release -p melee-save` and `cargo test --release -p leaf-check --test card` (`SSBM_DISC` set).
