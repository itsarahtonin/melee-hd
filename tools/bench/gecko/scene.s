# Injected at 0x801a6348, the scene think that Slippi's playback scene replaces
# (Playback/Core/Scene/SceneThink_Playback.asm). In DebugMelee (major scene 0xE),
# which the boot code enters at its result screen, it adds a process that loads the
# match's files, as Slippi's playback scene does once the replay is ready, and moves
# on to the match. The match's StartMeleeData follows the code (match_data).
.include "common.s"

    load r3, 0x80479D30         # scene controller
    lbz r3, 0x0(r3)             # major scene
    cmpwi r3, 0xE               # DebugMelee
    bne original

    li r3, 13
    li r4, 14
    li r5, 0
    branchl r12, GObj_Create
    bl think
    mflr r4
    li r5, 0
    branchl r12, GObj_AddProc
    branch r12, 0x801a6368

think:
    blrl
    backup
    bl match_data
    mflr r20

    # Preload the fighters and the stage.
    load r4, 0x80432078
    lbz r3, 0x60(r20)
    stw r3, 0x14(r4)
    lbz r3, 0x63(r20)
    stb r3, 0x18(r4)
    lbz r3, 0x60 + 0x24(r20)
    stw r3, 0x1C(r4)
    lbz r3, 0x63 + 0x24(r20)
    stb r3, 0x20(r4)
    lbz r3, 0x60 + 0x24 * 2(r20)
    stw r3, 0x24(r4)
    lbz r3, 0x63 + 0x24 * 2(r20)
    stb r3, 0x28(r4)
    lbz r3, 0x60 + 0x24 * 3(r20)
    stw r3, 0x2C(r4)
    lbz r3, 0x63 + 0x24 * 3(r20)
    stb r3, 0x30(r4)
    lhz r3, 0xE(r20)
    stw r3, 0xC(r4)

    # Queue the file loads.
    branchl r12, 0x80018254
    li r3, 199
    branchl r12, 0x80018c2c
    li r3, 4
    branchl r12, 0x80017700

    # Clear the sound bank queue, then queue the fighters' and the stage's banks.
    li r3, 28
    branchl r12, 0x80026F2C
    branchl r12, 0x8021b2d8
    li r21, 0
    mr r22, r20
ssm_loop:
    lbz r4, 0x60(r22)
    extsb r4, r4
    cmpwi r4, 33                # no fighter
    beq ssm_next
    load r3, 0x803bb3c0         # fighter sound banks
    mulli r4, r4, 0x10
    lbzx r3, r3, r4
    bl request_ssm
ssm_next:
    addi r21, r21, 1
    cmpwi r21, 6
    addi r22, r22, 0x24
    blt+ ssm_loop
    lhz r3, 0xE(r20)
    branchl r12, 0x8022519c     # the stage's internal ID
    mulli r4, r3, 3
    load r3, 0x803bb6b0         # stage sound banks
    lbzx r3, r3, r4
    bl request_ssm
    branchl r12, 0x80027168     # load them

    branchl r12, DiscError_ResumeGame
    branchl r12, MenuController_ChangeScreenMinor
    restore
    blr

# Common/CompatibilityHooks/RequestSSMLoad.asm: marks sound bank r3 to load.
request_ssm:
    cmpwi r3, 55
    beqlr
    load r12, 0x804337c4
    li r4, 1
    mulli r3, r3, 4
    stwx r4, r3, r12
    blr

match_data:
    blrl
    .space 0x138

original:
    lwz r3, 0x0(r31)
