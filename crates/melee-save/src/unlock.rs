//! Unlock-all ([milestone 5](../../../docs/milestone-5.md#unlock-all)): an edit of the save's
//! data equal to a complete save without play records, applied once. It runs no game code and
//! leaves the game no unlock event to run, so a session from the edited save is a console's
//! session from that save, its random seed untouched.
//!
//! It is HAL's own unlock-all, the debug one `gmMainLib_8015FA34` runs at a high `DbLevel`
//! (the features byte to `0xFF`, every character (`gm_80164F18`) and stage (`gm_8016468C`),
//! every special message announced and none waiting (`gm_8017297C`), every record trophy
//! given (`gm_801741FC`)), and what the design call adds: every trophy held, as
//! `Toy_SetUnlockState` gives a trophy the save hasn't held (one of it, flagged new, counted),
//! and every event cleared, which is what opens the events (`gmMainLib_8015CEFC`). Records
//! (scores, times, counts, the events' best) stay as they are.

/// `GmSaveData`'s fields the edit writes, by their offsets.
const CHARACTERS: usize = 0x0;
const STAGES: usize = 0x2;
const FEATURES: usize = 0x4;
const EVENTS: usize = 0x200;
const MESSAGES_WAITING: usize = 0x2D8;
const MESSAGES: usize = 0x2E4;
const RECORD_TROPHIES: usize = 0x2F0;
const MESSAGE_TIMES: usize = 0x318;
const TROPHY_COUNT: usize = 0x468;
const TROPHY_FLAGS: usize = 0x46C;

/// `NUM_UNLOCKABLE_CHARACTERS`, the stages' 11 bits, the special messages (`0x42`), the record
/// trophies' bits `gm_801741FC` sets (`0x12C`), `TY_TROPHY_COUNT`, and the events (`0x33`).
const UNLOCKABLE: u32 = 11;
const MESSAGE_COUNT: u32 = 0x42;
const RECORD_TROPHY_BITS: u32 = 0x12C;
pub const TROPHIES: usize = 293;
const EVENT_COUNT: u32 = 0x33;
/// A trophy's flag for one new to the save (`Toy_SetUnlockState`'s first gift).
const TROPHY_NEW: u16 = 0x8000;

fn get16(s: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([s[at], s[at + 1]])
}

fn put16(s: &mut [u8], at: usize, v: u16) {
    s[at..at + 2].copy_from_slice(&v.to_be_bytes());
}

fn get32(s: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(s[at..at + 4].try_into().expect("a word"))
}

fn put32(s: &mut [u8], at: usize, v: u32) {
    s[at..at + 4].copy_from_slice(&v.to_be_bytes());
}

/// A bit of a set of words (`bitset_test`, `bitset_set`, `bitset_clear`).
fn bit(s: &[u8], at: usize, n: u32) -> bool {
    get32(s, at + 4 * (n / 32) as usize) & 1 << (n % 32) != 0
}

fn set_bit(s: &mut [u8], at: usize, n: u32, on: bool) {
    let w = at + 4 * (n / 32) as usize;
    let v = get32(s, w);
    put32(
        s,
        w,
        if on {
            v | 1 << (n % 32)
        } else {
            v & !(1 << (n % 32))
        },
    );
}

/// Unlocks everything in `save` (`GmSaveData`'s bytes, [`crate::melee::SAVE_DATA`] of them):
/// `now` is the clock's seconds since 2000-01-01, the time each special message newly
/// announced takes (`lbTime_GetTimeInSeconds`, as `gm_8017297C` reads it).
pub fn unlock_all(save: &mut [u8], now: u32) {
    assert_eq!(save.len(), crate::melee::SAVE_DATA, "GmSaveData's bytes");
    save[FEATURES] = 0xFF;
    let all = (1u16 << UNLOCKABLE) - 1;
    put16(save, CHARACTERS, get16(save, CHARACTERS) | all);
    put16(save, STAGES, get16(save, STAGES) | all);
    // `gm_8017297C`: a message announced only now is announced as seen, not waiting.
    for i in 0..MESSAGE_COUNT {
        if !bit(save, MESSAGES, i) {
            put32(save, MESSAGE_TIMES + 4 * i as usize, now);
            set_bit(save, MESSAGES, i, true);
            set_bit(save, MESSAGES_WAITING, i, false);
        }
    }
    // `gm_801741FC`.
    for i in 0..RECORD_TROPHY_BITS {
        set_bit(save, RECORD_TROPHIES, i, true);
    }
    // `Toy_SetUnlockState(i, 1)` on each trophy the save doesn't hold, the save's table being
    // the one in play: flagged new and counted.
    for i in 0..TROPHIES {
        let at = TROPHY_FLAGS + 2 * i;
        let f = get16(save, at);
        if f & 0xFF == 0 {
            put16(save, at, ((f ^ TROPHY_NEW) & 0xFF00) | 1);
            put16(
                save,
                TROPHY_COUNT,
                get16(save, TROPHY_COUNT).wrapping_add(1),
            );
        }
    }
    let events = u64::from_be_bytes(save[EVENTS..EVENTS + 8].try_into().expect("8 bytes"));
    let events = events | ((1u64 << EVENT_COUNT) - 1);
    save[EVENTS..EVENTS + 8].copy_from_slice(&events.to_be_bytes());
}

/// Whether `save` has everything [`unlock_all`] gives.
pub fn is_unlocked(save: &[u8]) -> bool {
    let all = (1u16 << UNLOCKABLE) - 1;
    save[FEATURES] == 0xFF
        && get16(save, CHARACTERS) & all == all
        && get16(save, STAGES) & all == all
        && (0..MESSAGE_COUNT).all(|i| bit(save, MESSAGES, i))
        && (0..RECORD_TROPHY_BITS).all(|i| bit(save, RECORD_TROPHIES, i))
        && (0..TROPHIES).all(|i| get16(save, TROPHY_FLAGS + 2 * i) & 0xFF != 0)
        && u64::from_be_bytes(save[EVENTS..EVENTS + 8].try_into().expect("8 bytes"))
            & ((1u64 << EVENT_COUNT) - 1)
            == (1u64 << EVENT_COUNT) - 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlocks_a_new_save_and_leaves_records() {
        let mut s = vec![0u8; crate::melee::SAVE_DATA];
        // A play record (the VS matches played) and a held trophy.
        put32(&mut s, 0x8, 49);
        put16(&mut s, TROPHY_FLAGS + 2 * 5, 0x0003);
        put16(&mut s, TROPHY_COUNT, 1);
        // A special message waiting for its notice stays waiting.
        set_bit(&mut s, MESSAGES, 0x20, true);
        set_bit(&mut s, MESSAGES_WAITING, 0x20, true);
        put32(&mut s, MESSAGE_TIMES + 4 * 0x20, 7);
        assert!(!is_unlocked(&s));
        unlock_all(&mut s, 1234);
        assert!(is_unlocked(&s));
        assert_eq!(get32(&s, 0x8), 49);
        assert_eq!(get16(&s, TROPHY_FLAGS + 2 * 5), 0x0003);
        assert_eq!(get16(&s, TROPHY_FLAGS), TROPHY_NEW | 1);
        assert_eq!(get16(&s, TROPHY_COUNT), TROPHIES as u16);
        assert!(bit(&s, MESSAGES_WAITING, 0x20));
        assert_eq!(get32(&s, MESSAGE_TIMES + 4 * 0x20), 7);
        assert!(!bit(&s, MESSAGES_WAITING, 0x21));
        assert_eq!(get32(&s, MESSAGE_TIMES + 4 * 0x21), 1234);
        assert_eq!(
            &s[EVENTS..EVENTS + 8],
            &0x0007_FFFF_FFFF_FFFFu64.to_be_bytes()
        );
        // Applied again, nothing changes.
        let once = s.clone();
        unlock_all(&mut s, 99);
        assert_eq!(s, once);
    }
}
