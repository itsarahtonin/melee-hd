# Benchmark-only patch, injected at 0x800ac6d0 in ftCo_800AC5A0 (the CPU's DI while being
# thrown or hit). When the fighter's knockback is nearly zero the original passes registers it
# never set as the stick: the Y stick is the low byte of r30, which the caller set to the
# fighter's address. Heap layouts differ between ssbm-rs's recording setup and Dolphin, so this
# pins that byte, on that path only, to the recording's value (patched in at `patch`), making
# the game read what it read when the match was recorded. It changes no work the game does.
#
# It replaces `bne 0x800ac744`, after `cmpwi r0, 0`: r0 is 1 on the nearly-zero path, which
# goes on to the calls that take the stick; otherwise the code falls through to computing it.

    cmpwi r0, 0
    beq done
patch:
    li r30, 0
    lis r12, 0x800A
    ori r12, r12, 0xC744        # the calls that take the stick
    mtctr r12
    bctr
done:
