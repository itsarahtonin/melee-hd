# Injected at 0x8016e748 in StartMelee, where Slippi's playback restores the replay's
# game info (Playback/Core/RestoreGameInfo.asm): sets the random seed and copies the
# match's StartMeleeData over the one StartMelee is about to read (r31), with its
# callbacks cleared. The StartMeleeData and the seed follow the code (match_data).
.include "common.s"

    branchl r12, 0x802254b8     # the instruction this replaces
    backup
    bl match_data
    mflr r30
    lwz r3, 0x138(r30)
    lis r4, 0x804D
    stw r3, 0x5F90(r4)          # random seed
    mr r3, r31
    mr r4, r30
    li r5, 0x138
    branchl r12, memcpy
    addi r3, r31, 0x40
    li r4, 0x1C
    branchl r12, Zero_AreaLength
    restore
    b done

match_data:
    blrl
    .space 0x13C

done:
