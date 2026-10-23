# Saves

`melee-save` (`crates/melee-save`) reads Melee's save files and writes them as a console does: raw memory card images (`.raw`, ssbm-rs's `.card`) and `.gci` files, judged as the game judges a card it loads, and written byte for byte as the console's card layers write them. It holds no simulation code: the memory card scene, `lbcardgame` and the native save that answers the game's card tasks (milestone 5's task 4) build on it, and the player's import and export (task 18). Where the player keeps the save is one of Sarah's open design calls ([milestone 5](milestone-5.md#design-calls)); nothing is wired into the player or the simulation yet.

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

## Checks

- **leaf-check `card`** runs `crypt.c`'s five functions and the HSD layer's format and request functions against ssbm-rs's on the original's `CardState` bytes: every byte pair through `encryptByte` and `decryptByte`, thousands of buffers through the checksum, encryption and decryption, and random states (the save's layout, a snapshot's, random files of every flags, blocks scattered with stale copies and sequence numbers about to wrap) through each planner and the repair, comparing the state after, every queued command's fields, the active request and the result.
- **The corpus** (`crates/melee-save/tests/corpus.rs`, reading `$SSBM_RS_LOCAL`): the 1,155 card images ssbm-rs's runs left (1,132 of the lockstep campaign, its 6 fault cases, the 13 beside the recorded sessions, 4 in `local/cards`). Every image and each of its 1,212 files reads and writes back byte for byte, and all 11,759 valid blocks are what the layer's block write makes of their contents (1,143 of them written whole by a create). The load judges 1,125 saves loaded, 14 cards with no save, 10 broken, 3 of the other encoding and 3 of ssbm-rs's hand-made layouts damaged; the fault cases as they were made (one bad directory repaired, both bad broken, Shift-JIS refused), and the card whose one good table is the format's, older than the save, broken, as `CARDCheck` finds its chain running into free blocks. The three freshly made saves in the corpus come out of a format and a create byte for byte, card and all, and each of the twelve recorded sessions' cards turns into the next one's through the saves the session made (and, once, a snapshot), all 512 KiB.

Run them with `cargo test --release -p melee-save` and `cargo test --release -p leaf-check --test card` (`SSBM_DISC` set).
