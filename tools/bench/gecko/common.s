# Macros shared by the boot codes, as slippi-ssbm-asm's Common/Common.s defines them.

.macro branchl reg, address
    lis \reg, \address @h
    ori \reg, \reg, \address @l
    mtctr \reg
    bctrl
.endm

.macro branch reg, address
    lis \reg, \address @h
    ori \reg, \reg, \address @l
    mtctr \reg
    bctr
.endm

.macro load reg, address
    lis \reg, \address @h
    ori \reg, \reg, \address @l
.endm

.macro backup
    mflr r0
    stw r0, 0x4(r1)
    stwu r1, -0xE0(r1)
    stmw r20, 0xB0(r1)
.endm

.macro restore
    lmw r20, 0xB0(r1)
    lwz r0, 0xE4(r1)
    addi r1, r1, 0xE0
    mtlr r0
.endm

.set GObj_Create, 0x803901f0
.set GObj_AddProc, 0x8038fd54
.set memcpy, 0x800031f4
.set Zero_AreaLength, 0x8000c160
.set DiscError_ResumeGame, 0x80024f6c
.set MenuController_ChangeScreenMinor, 0x801a4b60
