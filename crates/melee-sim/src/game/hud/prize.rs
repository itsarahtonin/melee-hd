//! The special messages' interface (`melee/if/ifprize.c`): each message's strings and kind,
//! which the Data menu's list of messages (`mninfo.c`) names its entries by.

/// `un_803F9A00`: each string's kind, by the message's string offset.
const KINDS: [i32; 76] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 3, 2, 2, 5, 5, 4, 6, 6, 8, 8,
    8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 6, 6, 5, 7,
    7, 7, 7, 7, 7, 5, 5, 5, 5, 5, 5, 5,
];

/// `un_803F9B30`: each message (`x0`) and its string offset (`x4`), to the message 66 that
/// ends the table.
const MESSAGES: [(i32, u16); 67] = {
    let mut t = [(0, 0); 67];
    let mut i = 0;
    while i < 62 {
        t[i] = (i as i32, i as u16);
        i += 1;
    }
    t[62] = (62, 63);
    t[63] = (63, 67);
    t[64] = (64, 68);
    t[65] = (65, 62);
    t[66] = (66, 0);
    t
};

/// `un_802FE3F8`: message `n`'s string, `base` plus its offset (`c`); message 0x3E, the
/// trophy's, also its second string (`d`). A message the table doesn't have leaves both as
/// they were (none here: every caller asks for one of 0 to 65).
#[doc(alias = "un_802FE3F8")]
pub fn message_strings(n: i32, base: i32) -> (Option<u16>, Option<u16>) {
    let (mut c, mut d) = (None, None);
    for &(x0, x4) in MESSAGES.iter().take_while(|e| e.0 != 66) {
        if x0 == n {
            let s = (base + i32::from(x4)) as u16;
            c = Some(s);
            if n == 0x3E {
                d = Some(s.wrapping_add(1));
            }
        }
    }
    (c, d)
}

/// The kind of message `n`'s string (`un_803F9A00` by its offset), 0 for one the table
/// doesn't have.
pub fn message_kind(n: i32) -> i32 {
    MESSAGES
        .iter()
        .take_while(|e| e.0 != 66)
        .find(|e| e.0 == n)
        .map_or(0, |e| KINDS[usize::from(e.1)])
}
