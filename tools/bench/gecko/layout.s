# Injected at 0x80068ee4 in Fighter_Create, just before it allocates the fighter's block
# (HSD_ObjAlloc on fighter_alloc_data), for a match where a fighter runs CPU code (Nana): lays
# the heap out from there as Slippi's playback does. The CPU's DI with no knockback reads a byte
# of the fighter's address (docs/melee-bugs.md), so the match goes the same way only where the
# fighters have the addresses playback gives them. The recording run (Slippi's bootloader and
# recording codes) and Dolphin (the boot codes alone) both hold less of the heap below this
# point than playback does, so this allocates the difference once: the heap's first free cell
# then starts at the cell playback's first fighter takes (patched in at `patch`). Each later
# fighter finds the cell at or past it and allocates nothing. It changes nothing else the game
# does.
#
# It replaces `addi r3, r3, -0x7030` (r3 = &fighter_alloc_data), which it ends with.

.include "common.s"

.set HSD_GetHeap, 0x80375404
.set HSD_MemAlloc, 0x8037f1e4
.set HeapArray, 0x804d7360

    backup
    branchl r12, HSD_GetHeap
    load r4, HeapArray
    lwz r4, 0(r4)
    mulli r3, r3, 12
    add r4, r4, r3
    lwz r4, 4(r4)               # the heap's first free cell
patch:
    lis r3, 0
    ori r3, r3, 0               # the cell playback's first fighter takes
    subf r3, r4, r3             # what lies between
    cmpwi r3, 0x40
    blt done                    # nothing to fill (or less than a cell)
    lwz r5, 8(r4)
    cmplw r5, r3
    ble done                    # the free cell doesn't reach it: leave the heap be
    subi r3, r3, 0x20           # a cell's header
    branchl r12, HSD_MemAlloc
done:
    restore
    lis r3, 0x8046
    addi r3, r3, -0x7030
