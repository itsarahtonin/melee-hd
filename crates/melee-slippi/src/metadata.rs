// SPDX-License-Identifier: GPL-3.0-or-later

//! What melee-hd adds to a `.slp` file's metadata, beside Slippi's own keys: the unlocks a
//! Vanilla match read (`unlocks`, milestone 5's call of 2026-10-23), which melee-hd plays its
//! recording with and Slippi's tools leave alone, as they do any key they don't know.

use melee_sim::game::gm::{Unlocks, code};

/// The metadata key.
pub const UNLOCKS: &str = "unlocks";

/// The `unlocks` object's fields: the save's masks, language and records, the trophy and the
/// unlock codes' sites (`Unlocks::codes`, 0 under Vanilla), as numbers.
const CHARACTERS: &str = "characters";
const STAGES: &str = "stages";
const LANGUAGE: &str = "language";
const RECORDS: &str = "records";
const TROPHY_11A: &str = "trophy11A";
const CODES: &str = "codes";

/// The five booleans recordings carried before `codes` (2026-11-14, from `f8492689` to the
/// ruleset's sessions), read into the bits they stood for.
const OLD_CODES: [(&str, u16); 5] = [
    ("allCharactersCode", code::ALL_CHARACTERS),
    ("allStagesCode", code::ALL_STAGES),
    ("recordsIndexCode", code::SPECIAL_MESSAGES_OFF),
    ("recordsAllCode", code::RECORDS_ALL),
    ("trophiesAllCode", code::TROPHIES_ALL),
];

/// Writes `"unlocks": {...}`, a key and its object, into a UBJSON object.
pub fn write_unlocks(o: &mut Vec<u8>, u: &Unlocks) {
    key(o, UNLOCKS);
    o.push(b'{');
    let numbers = [
        (CHARACTERS, i64::from(u.characters)),
        (STAGES, i64::from(u.stages)),
        (LANGUAGE, i64::from(u.language)),
        (RECORDS, i64::from(u.records)),
        (TROPHY_11A, i64::from(u.trophy_11a)),
        (CODES, i64::from(u.codes)),
    ];
    for (k, v) in numbers {
        key(o, k);
        o.push(b'l');
        o.extend_from_slice(&(v as u32).to_be_bytes());
    }
    o.push(b'}');
}

/// An object key: its length as a `uint8` and its bytes.
pub(crate) fn key(o: &mut Vec<u8>, k: &str) {
    o.push(b'U');
    o.push(k.len() as u8);
    o.extend_from_slice(k.as_bytes());
}

/// The unlocks a `.slp` file's metadata carries, if it has them, whole.
pub fn unlocks(file: &[u8]) -> Option<Unlocks> {
    let mut r = Ubjson { data: file, at: 0 };
    let Value::Object(top) = r.value()? else {
        return None;
    };
    let Value::Object(meta) = field(&top, "metadata")? else {
        return None;
    };
    let Value::Object(u) = field(meta, UNLOCKS)? else {
        return None;
    };
    let int = |k: &str| match field(u, k)? {
        Value::Int(v) => Some(*v),
        _ => None,
    };
    let codes = match int(CODES) {
        Some(c) => u16::try_from(c).ok()?,
        None => OLD_CODES
            .iter()
            .filter(|(k, _)| matches!(field(u, k), Some(Value::Bool(true))))
            .fold(0, |c, (_, bit)| c | bit),
    };
    Some(Unlocks {
        characters: u16::try_from(int(CHARACTERS)?).ok()?,
        stages: u16::try_from(int(STAGES)?).ok()?,
        language: u8::try_from(int(LANGUAGE)?).ok()?,
        records: int(RECORDS)? as u32,
        trophy_11a: u8::try_from(int(TROPHY_11A)?).ok()?,
        codes,
    })
}

/// What played the match, as the metadata's `playedOn` says (`dolphin` for Slippi Dolphin,
/// `nintendont` for a console's Slippi, `ssbm-rs`, `melee-hd`), if it says.
pub fn played_on(file: &[u8]) -> Option<String> {
    let mut r = Ubjson { data: file, at: 0 };
    let Value::Object(top) = r.value()? else {
        return None;
    };
    let Value::Object(meta) = field(&top, "metadata")? else {
        return None;
    };
    match field(meta, "playedOn")? {
        Value::Str(s) => Some(s.clone()),
        _ => None,
    }
}

fn field<'a>(o: &'a [(String, Value)], k: &str) -> Option<&'a Value> {
    o.iter().find(|(n, _)| n == k).map(|(_, v)| v)
}

/// A UBJSON value, as far as the metadata needs: numbers that fit, booleans, strings and
/// containers; floats and typed arrays are read past.
enum Value {
    Int(i64),
    Bool(bool),
    Str(String),
    Object(Vec<(String, Value)>),
    Other,
}

struct Ubjson<'a> {
    data: &'a [u8],
    at: usize,
}

impl Ubjson<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let b = self.data.get(self.at..self.at + n)?;
        self.at += n;
        Some(b)
    }

    fn byte(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }

    /// A number of the type `marker` names.
    fn int(&mut self, marker: u8) -> Option<i64> {
        let n = match marker {
            b'i' => i64::from(self.byte()? as i8),
            b'U' => i64::from(self.byte()?),
            b'I' => i64::from(i16::from_be_bytes(self.take(2)?.try_into().ok()?)),
            b'l' => i64::from(i32::from_be_bytes(self.take(4)?.try_into().ok()?)),
            b'L' => i64::from_be_bytes(self.take(8)?.try_into().ok()?),
            _ => return None,
        };
        Some(n)
    }

    fn len(&mut self) -> Option<usize> {
        let m = self.byte()?;
        usize::try_from(self.int(m)?).ok()
    }

    fn string(&mut self) -> Option<String> {
        let n = self.len()?;
        Some(String::from_utf8_lossy(self.take(n)?).into_owned())
    }

    fn value(&mut self) -> Option<Value> {
        let m = self.byte()?;
        self.value_of(m)
    }

    fn value_of(&mut self, m: u8) -> Option<Value> {
        Some(match m {
            b'i' | b'U' | b'I' | b'l' | b'L' => Value::Int(self.int(m)?),
            b'T' => Value::Bool(true),
            b'F' => Value::Bool(false),
            b'Z' | b'N' => Value::Other,
            b'd' => {
                self.take(4)?;
                Value::Other
            }
            b'D' => {
                self.take(8)?;
                Value::Other
            }
            b'C' => {
                self.take(1)?;
                Value::Other
            }
            b'S' | b'H' => Value::Str(self.string()?),
            b'{' => {
                let mut o = Vec::new();
                loop {
                    if self.data.get(self.at) == Some(&b'}') {
                        self.at += 1;
                        break;
                    }
                    let k = self.string()?;
                    let v = self.value()?;
                    o.push((k, v));
                }
                Value::Object(o)
            }
            b'[' => {
                // A typed array (`$` type `#` count), or values up to `]`.
                if self.data.get(self.at) == Some(&b'$') {
                    self.at += 1;
                    let t = self.byte()?;
                    if self.byte()? != b'#' {
                        return None;
                    }
                    let n = self.len()?;
                    let size = match t {
                        b'U' | b'i' | b'C' => 1,
                        b'I' => 2,
                        b'l' | b'd' => 4,
                        b'L' | b'D' => 8,
                        _ => 0,
                    };
                    if size > 0 {
                        self.take(n.checked_mul(size)?)?;
                    } else {
                        for _ in 0..n {
                            self.value_of(t)?;
                        }
                    }
                } else {
                    while self.data.get(self.at) != Some(&b']') {
                        self.value()?;
                    }
                    self.at += 1;
                }
                Value::Other
            }
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A recording made before `codes` (its five booleans) reads into the same bits.
    #[test]
    fn old_code_flags_read_as_bits() {
        let mut o = vec![b'{'];
        key(&mut o, "metadata");
        o.push(b'{');
        key(&mut o, UNLOCKS);
        o.push(b'{');
        for k in [CHARACTERS, STAGES, LANGUAGE, RECORDS, TROPHY_11A] {
            key(&mut o, k);
            o.push(b'l');
            o.extend_from_slice(&1u32.to_be_bytes());
        }
        for (k, _) in OLD_CODES {
            key(&mut o, k);
            o.push(if k.starts_with("all") { b'T' } else { b'F' });
        }
        o.extend_from_slice(b"}}}");
        let u = unlocks(&o).expect("the old unlocks");
        assert_eq!(u.codes, code::ALL_CHARACTERS | code::ALL_STAGES);
        assert_eq!((u.characters, u.trophy_11a), (1, 1));
    }
}
