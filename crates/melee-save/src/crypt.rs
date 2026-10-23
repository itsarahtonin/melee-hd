//! HSD's save encryption (`sysdolphin/baselib/crypt.c`): every block the HSD card layer writes
//! starts with a 16-byte checksum of the rest, and the rest is a running substitution keyed by
//! the byte before it.

/// The 16-byte checksum `HSD_Checksum` writes: MD5's initial state, its words' bytes reversed,
/// with each byte of `src` added to the slot its index picks, then any byte equal to the one
/// before it inverted. Not MD5.
#[doc(alias = "HSD_Checksum")]
pub fn checksum(src: &[u8]) -> [u8; 16] {
    let mut sum: [u8; 16] = [
        0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0xFE, 0xDC, 0xBA, 0x98, 0x76, 0x54, 0x32,
        0x10,
    ];
    for (i, &b) in src.iter().enumerate() {
        sum[i % 16] = sum[i % 16].wrapping_add(b);
    }
    for i in 1..16 {
        if sum[i - 1] == sum[i] {
            sum[i] ^= 0xFF;
        }
    }
    sum
}

const KEYS: [u8; 13] = [
    0x26, 0xFF, 0xE8, 0xEF, 0x42, 0xD6, 0x01, 0x54, 0x14, 0xA3, 0x80, 0xFD, 0x6E,
];

/// One byte enciphered after `prev`, the byte before it as already enciphered.
#[doc(alias = "encryptByte")]
pub fn encrypt_byte(prev: u8, cur: u8) -> u8 {
    let v = u32::from(prev ^ cur ^ KEYS[usize::from(prev % 13)]);
    (match prev % 7 {
        0 => {
            (v & 1)
                | ((v << 3) & 0x10)
                | ((v >> 1) & 2)
                | ((v << 2) & 0x20)
                | ((v >> 2) & 4)
                | ((v << 1) & 0x40)
                | ((v >> 3) & 8)
                | (v & 0x80)
        }
        1 => {
            ((v << 3) & 8)
                | ((v >> 1) & 1)
                | (v & 4)
                | ((v << 3) & 0x40)
                | ((v << 1) & 0x20)
                | ((v >> 1) & 0x10)
                | ((v << 1) & 0x80)
                | ((v >> 6) & 2)
        }
        2 => {
            ((v << 6) & 0x40)
                | ((v << 4) & 0x20)
                | ((v >> 2) & 1)
                | ((v >> 2) & 2)
                | ((v >> 1) & 8)
                | ((v << 2) & 0x80)
                | ((v >> 4) & 4)
                | ((v >> 3) & 0x10)
        }
        3 => {
            ((v << 1) & 2)
                | ((v << 2) & 8)
                | ((v << 5) & 0x80)
                | ((v << 1) & 0x10)
                | ((v >> 4) & 1)
                | ((v >> 3) & 4)
                | ((v >> 1) & 0x20)
                | ((v >> 1) & 0x40)
        }
        4 => {
            ((v << 7) & 0x80)
                | ((v << 1) & 4)
                | ((v << 3) & 0x20)
                | ((v >> 3) & 1)
                | ((v << 2) & 0x40)
                | ((v >> 4) & 2)
                | ((v >> 2) & 0x10)
                | ((v >> 4) & 8)
        }
        5 => {
            ((v & 1) << 5)
                | ((v << 5) & 0x40)
                | ((v << 2) & 0x10)
                | (v & 8)
                | ((v << 3) & 0x80)
                | ((v >> 5) & 1)
                | ((v >> 5) & 2)
                | ((v >> 5) & 4)
        }
        _ => {
            ((v << 2) & 4)
                | (v & 2)
                | ((v & 4) << 4)
                | ((v << 4) & 0x80)
                | (v & 0x10)
                | ((v >> 2) & 8)
                | ((v >> 6) & 1)
                | ((v >> 2) & 0x20)
        }
    }) as u8
}

/// Writes the checksum of `data[16..]` into `data[..16]`, then enciphers `data[16..]` in place.
/// `data` holds at least the checksum.
#[doc(alias = "HSD_Encrypt")]
pub fn encrypt(data: &mut [u8]) {
    let sum = checksum(&data[16..]);
    data[..16].copy_from_slice(&sum);
    for i in 16..data.len() {
        data[i] = encrypt_byte(data[i - 1], data[i]);
    }
}

/// One byte deciphered after `prev`, the byte before it as enciphered.
#[doc(alias = "decryptByte")]
pub fn decrypt_byte(prev: u8, cur: u8) -> u8 {
    let c = u32::from(cur);
    let out = match prev % 7 {
        0 => {
            (c & 1)
                | ((c << 1) & 4)
                | ((c << 2) & 0x10)
                | ((c << 3) & 0x40)
                | ((c >> 3) & 2)
                | ((c >> 2) & 8)
                | ((c >> 1) & 0x20)
                | (c & 0x80)
        }
        1 => {
            ((c << 1) & 2)
                | ((c << 6) & 0x80)
                | (c & 4)
                | ((c >> 3) & 1)
                | ((c << 1) & 0x20)
                | ((c >> 1) & 0x10)
                | ((c >> 3) & 8)
                | ((c >> 1) & 0x40)
        }
        2 => {
            ((c & 1) << 2)
                | ((c << 2) & 8)
                | ((c << 4) & 0x40)
                | ((c << 1) & 0x10)
                | ((c << 3) & 0x80)
                | ((c >> 4) & 2)
                | ((c >> 6) & 1)
                | ((c >> 2) & 0x20)
        }
        3 => {
            ((c << 4) & 0x10)
                | ((c >> 1) & 1)
                | ((c << 3) & 0x20)
                | ((c >> 2) & 2)
                | ((c >> 1) & 8)
                | ((c << 1) & 0x40)
                | ((c << 1) & 0x80)
                | ((c >> 5) & 4)
        }
        4 => {
            ((c << 3) & 8)
                | ((c << 4) & 0x20)
                | ((c >> 1) & 2)
                | ((c << 4) & 0x80)
                | ((c << 2) & 0x40)
                | ((c >> 3) & 4)
                | ((c >> 2) & 0x10)
                | ((c >> 7) & 1)
        }
        5 => {
            ((c & 1) << 5)
                | ((c << 5) & 0x40)
                | ((c & 4) << 5)
                | (c & 8)
                | ((c >> 2) & 4)
                | ((c >> 5) & 1)
                | ((c >> 5) & 2)
                | ((c >> 3) & 0x10)
        }
        _ => {
            ((c << 6) & 0x40)
                | (c & 2)
                | ((c >> 2) & 1)
                | ((c << 2) & 0x20)
                | (c & 0x10)
                | ((c << 2) & 0x80)
                | ((c >> 4) & 4)
                | ((c >> 4) & 8)
        }
    } as u8;
    out ^ KEYS[usize::from(prev % 13)] ^ prev
}

/// Deciphers `data[16..]` in place and checks it against the checksum in `data[..16]`: true
/// when they agree (`HSD_Decrypt`'s 0), false otherwise (its -1), the data deciphered either way.
#[doc(alias = "HSD_Decrypt")]
pub fn decrypt(data: &mut [u8]) -> bool {
    let mut prev = data[15];
    for b in &mut data[16..] {
        let cur = *b;
        *b = decrypt_byte(prev, cur);
        prev = cur;
    }
    checksum(&data[16..]) == data[..16]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decrypt_undoes_encrypt() {
        let mut s = 0x9E37_79B9_7F4A_7C15u64;
        for len in [16, 17, 0x190, 0x2000] {
            let mut data: Vec<u8> = (0..len)
                .map(|_| {
                    s ^= s << 13;
                    s ^= s >> 7;
                    s ^= s << 17;
                    s as u8
                })
                .collect();
            let plain = data.clone();
            encrypt(&mut data);
            assert!(decrypt(&mut data));
            assert_eq!(data[16..], plain[16..]);
            if len > 32 {
                encrypt(&mut data);
                data[len / 2] ^= 1;
                assert!(!decrypt(&mut data), "a flipped byte fails the checksum");
            }
        }
    }

    /// Both key on the enciphered byte before, so each byte's deciphering undoes its
    /// enciphering for every key.
    #[test]
    fn bytes_round_trip_for_every_key() {
        for prev in 0..=255u8 {
            for cur in 0..=255u8 {
                assert_eq!(decrypt_byte(prev, encrypt_byte(prev, cur)), cur);
            }
        }
    }
}
