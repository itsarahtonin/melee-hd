// SPDX-License-Identifier: GPL-3.0-or-later

//! What melee-hd adds to a `.slp` file's metadata, beside Slippi's own keys: the unlocks a
//! Vanilla match read (`unlocks`, milestone 5's call of 2026-10-23), which melee-hd plays its
//! recording with and Slippi's tools leave alone, as they do any key they don't know.

use melee_sim::game::gm::Unlocks;

/// The metadata key.
pub const UNLOCKS: &str = "unlocks";

/// The `unlocks` object's fields: the save's masks, language, records and trophy, as numbers,
/// and the unlock codes' sites, as booleans (all false under Vanilla).
const CHARACTERS: &str = "characters";
const STAGES: &str = "stages";
const LANGUAGE: &str = "language";
const RECORDS: &str = "records";
const TROPHY_11A: &str = "trophy11A";
const CODES: [&str; 5] = [
    "allCharactersCode",
    "allStagesCode",
    "recordsIndexCode",
    "recordsAllCode",
    "trophiesAllCode",
];

fn code_flags(u: &mut Unlocks) -> [&mut bool; 5] {
    [
        &mut u.all_characters_code,
        &mut u.all_stages_code,
        &mut u.records_index_code,
        &mut u.records_all_code,
        &mut u.trophies_all_code,
    ]
}

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
    ];
    for (k, v) in numbers {
        key(o, k);
        o.push(b'l');
        o.extend_from_slice(&(v as u32).to_be_bytes());
    }
    let mut u = *u;
    for (k, f) in CODES.iter().zip(code_flags(&mut u)) {
        key(o, k);
        o.push(if *f { b'T' } else { b'F' });
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
    let mut out = Unlocks {
        characters: u16::try_from(int(CHARACTERS)?).ok()?,
        stages: u16::try_from(int(STAGES)?).ok()?,
        language: u8::try_from(int(LANGUAGE)?).ok()?,
        records: int(RECORDS)? as u32,
        trophy_11a: u8::try_from(int(TROPHY_11A)?).ok()?,
        ..Unlocks::default()
    };
    for (k, f) in CODES.iter().zip(code_flags(&mut out)) {
        *f = matches!(field(u, k)?, Value::Bool(true));
    }
    Some(out)
}

fn field<'a>(o: &'a [(String, Value)], k: &str) -> Option<&'a Value> {
    o.iter().find(|(n, _)| n == k).map(|(_, v)| v)
}

/// A UBJSON value, as far as the metadata needs: numbers that fit, booleans and containers;
/// strings, floats and typed arrays are read past.
enum Value {
    Int(i64),
    Bool(bool),
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
            b'S' | b'H' => {
                self.string()?;
                Value::Other
            }
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
