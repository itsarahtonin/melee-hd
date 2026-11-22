//! The layout check: every hand-written game struct (`#[derive(Mirror)]`, structgen's
//! `OWNED` list) against the decomp's layout of it, as structgen reads it (ssbm-rs's
//! `typegen/types.json`, under `SSBM_RS_LOCAL`).
//!
//! For each struct: its size is the C struct's; its fields and declared padding cover every bit
//! of it once (no gap, no overlap); each field starts and ends where C fields do (it holds whole
//! C fields: one, a run of bitfields as a flag byte, a group of fields as a sub-struct), so a
//! field at the wrong offset or of the wrong size fails; a field whose type is itself a
//! hand-written struct sits where the C struct holds one of that name; and padding covers only
//! bytes no C field names. A group of fields with no C struct of its own (`c_name` none) is
//! checked field by field against the struct around it.

use std::collections::HashMap;

use super::{FieldLayout, Layout, Mirror};

/// One C field: its bits from the struct's start, and the struct it is (or is an array of).
struct CField {
    name: String,
    start: u32,
    end: u32,
    rec: Option<String>,
}

struct CRecord {
    size: u32,
    fields: Vec<CField>,
}

/// The hand-written structs, by their layouts. Each is on structgen's `OWNED` list.
fn owned() -> Vec<Layout> {
    use crate::game::camera::types::{Camera, CameraQuake, CameraTransformState, OrbitLimits};
    use crate::mp::coll::{CollData, Ecb, Surface};
    [
        CollData::layout(),
        Ecb::layout(),
        Surface::layout(),
        Camera::layout(),
        CameraTransformState::layout(),
        CameraQuake::layout(),
        OrbitLimits::layout(),
    ]
    .into_iter()
    .map(|l| l.expect("a derived layout"))
    .collect()
}

/// What's wrong with `l` against the C struct `c`, one line each; empty when it matches.
fn check(l: &Layout, c: &CRecord, name: &str) -> Vec<String> {
    let mut errors = Vec::new();
    if l.size != c.size {
        errors.push(format!(
            "{name}: size {:#x}, the decomp's {:#x}",
            l.size, c.size
        ));
    }
    // Every field and pad as a run of bits, groups flattened.
    let mut runs: Vec<(u32, u32, String, Option<&'static str>)> = Vec::new();
    let mut pads: Vec<(u32, u32)> = Vec::new();
    flatten(l, 0, "", &mut runs, &mut pads);
    let mut all: Vec<(u32, u32, String)> = runs
        .iter()
        .map(|(s, e, n, _)| (*s, *e, n.clone()))
        .collect();
    all.extend(
        pads.iter()
            .map(|&(s, e)| (s, e, format!("padding at {:#x}", s / 8))),
    );
    all.sort();
    let mut end = 0;
    for (s, e, n) in &all {
        if *s > end {
            errors.push(format!(
                "{name}: bits {}..{} (bytes {:#x}..{:#x}) held by no field: {}",
                end,
                s,
                end / 8,
                s.div_ceil(8),
                c_names(c, end, *s)
            ));
        } else if *s < end {
            errors.push(format!("{name}.{n} overlaps the field before it (starts at bit {s}, the field before ends at {end})"));
        }
        end = end.max(*e);
    }
    if end < c.size * 8 {
        errors.push(format!(
            "{name}: bytes {:#x}..{:#x} held by no field: {}",
            end / 8,
            c.size,
            c_names(c, end, c.size * 8)
        ));
    }
    let starts: Vec<u32> = c.fields.iter().map(|f| f.start).collect();
    let ends: Vec<u32> = c.fields.iter().map(|f| f.end).collect();
    for (s, e, n, nested) in &runs {
        if !starts.contains(s) {
            errors.push(format!(
                "{name}.{n}: starts at bit {s} (byte {:#x}), inside {}",
                s / 8,
                c_names(c, *s, s + 1)
            ));
        }
        if !ends.contains(e) {
            errors.push(format!(
                "{name}.{n}: ends at bit {e} (byte {:#x}), inside {}",
                e / 8,
                c_names(c, e - 1, *e)
            ));
        }
        if let Some(want) = nested {
            match c.fields.iter().find(|f| f.start == *s) {
                Some(f) if f.rec.as_deref() == Some(want) => {}
                f => errors.push(format!(
                    "{name}.{n}: a {want}, where the decomp holds {}",
                    f.map_or("nothing".to_owned(), |f| format!(
                        "{} ({})",
                        f.name,
                        f.rec.as_deref().unwrap_or("not a struct")
                    ))
                )),
            }
        }
    }
    for &(s, e) in &pads {
        let named: Vec<&str> = c
            .fields
            .iter()
            .filter(|f| f.start < e && s < f.end)
            .map(|f| f.name.as_str())
            .collect();
        if !named.is_empty() {
            errors.push(format!(
                "{name}: padding at {:#x} covers the decomp's fields {named:?}",
                s / 8
            ));
        }
    }
    errors
}

fn flatten(
    l: &Layout,
    base: u32,
    prefix: &str,
    runs: &mut Vec<(u32, u32, String, Option<&'static str>)>,
    pads: &mut Vec<(u32, u32)>,
) {
    pads.extend(
        l.pads
            .iter()
            .map(|&(a, n)| (base + a * 8, base + (a + n) * 8)),
    );
    for f in &l.fields {
        let FieldLayout {
            name,
            at,
            size,
            bits,
            nested,
        } = f;
        let start = base + at * 8;
        let path = format!("{prefix}{name}");
        if let Some((first, count)) = bits {
            runs.push((start + first, start + first + count, path, None));
            continue;
        }
        match nested() {
            Some(inner) if inner.c_name.is_none() => {
                flatten(&inner, start, &format!("{path}."), runs, pads)
            }
            inner => runs.push((start, start + size * 8, path, inner.and_then(|i| i.c_name))),
        }
    }
}

/// The C fields over bits `s..e`, for a message.
fn c_names(c: &CRecord, s: u32, e: u32) -> String {
    let names: Vec<String> = c
        .fields
        .iter()
        .filter(|f| f.start < e && s < f.end)
        .map(|f| format!("{} (bits {}..{})", f.name, f.start, f.end))
        .collect();
    if names.is_empty() {
        "no C field (the compiler's padding)".to_owned()
    } else {
        names.join(", ")
    }
}

// The decomp's layouts, from types.json.

#[derive(Debug)]
enum Json {
    Null,
    Bool,
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(HashMap<String, Json>),
}

impl Json {
    fn get(&self, k: &str) -> Option<&Json> {
        match self {
            Json::Obj(m) => m.get(k),
            _ => None,
        }
    }

    fn num(&self) -> u32 {
        match self {
            Json::Num(n) => *n as u32,
            j => panic!("not a number: {j:?}"),
        }
    }

    fn str(&self) -> &str {
        match self {
            Json::Str(s) => s,
            j => panic!("not a string: {j:?}"),
        }
    }
}

/// Just enough JSON for types.json.
struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.b[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn value(&mut self) -> Json {
        self.ws();
        match self.b[self.i] {
            b'{' => {
                self.i += 1;
                let mut m = HashMap::new();
                loop {
                    self.ws();
                    if self.b[self.i] == b'}' {
                        self.i += 1;
                        return Json::Obj(m);
                    }
                    let Json::Str(k) = self.value() else {
                        panic!("a key")
                    };
                    self.ws();
                    assert_eq!(self.b[self.i], b':');
                    self.i += 1;
                    let v = self.value();
                    m.insert(k, v);
                    self.ws();
                    if self.b[self.i] == b',' {
                        self.i += 1;
                    }
                }
            }
            b'[' => {
                self.i += 1;
                let mut v = Vec::new();
                loop {
                    self.ws();
                    if self.b[self.i] == b']' {
                        self.i += 1;
                        return Json::Arr(v);
                    }
                    v.push(self.value());
                    self.ws();
                    if self.b[self.i] == b',' {
                        self.i += 1;
                    }
                }
            }
            b'"' => {
                self.i += 1;
                let mut s = Vec::new();
                loop {
                    match self.b[self.i] {
                        b'"' => break,
                        b'\\' => {
                            self.i += 1;
                            match self.b[self.i] {
                                b'n' => s.push(b'\n'),
                                b't' => s.push(b'\t'),
                                b'u' => {
                                    let hex = std::str::from_utf8(&self.b[self.i + 1..self.i + 5])
                                        .unwrap();
                                    let c = char::from_u32(u32::from_str_radix(hex, 16).unwrap())
                                        .unwrap_or('?');
                                    s.extend(c.to_string().bytes());
                                    self.i += 4;
                                }
                                c => s.push(c),
                            }
                        }
                        c => s.push(c),
                    }
                    self.i += 1;
                }
                self.i += 1;
                Json::Str(String::from_utf8_lossy(&s).into_owned())
            }
            b't' => {
                self.i += 4;
                Json::Bool
            }
            b'f' => {
                self.i += 5;
                Json::Bool
            }
            b'n' => {
                self.i += 4;
                Json::Null
            }
            _ => {
                let s = self.i;
                while matches!(
                    self.b[self.i],
                    b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'
                ) {
                    self.i += 1;
                }
                Json::Num(
                    std::str::from_utf8(&self.b[s..self.i])
                        .unwrap()
                        .parse()
                        .unwrap(),
                )
            }
        }
    }
}

/// The C name of a record id (`c:@S@Fighter`, `c:@SA@Vec`), as structgen's `rec_name`.
fn rec_name(id: &str) -> String {
    for p in ["c:@S@", "c:@U@", "c:@SA@", "c:@UA@"] {
        if let Some(n) = id.strip_prefix(p) {
            return n.to_owned();
        }
    }
    id.to_owned()
}

/// Every record of types.json by its C name.
fn records(types: &Json) -> HashMap<String, CRecord> {
    let Some(Json::Obj(recs)) = types.get("records") else {
        panic!("no records")
    };
    let size_of = |t: &Json| -> u32 {
        fn size(t: &Json, recs: &HashMap<String, Json>) -> u32 {
            match t.get("k").unwrap().str() {
                "int" | "float" | "enum" => t.get("size").unwrap().num(),
                "ptr" => 4,
                "arr" => t.get("n").unwrap().num() * size(t.get("of").unwrap(), recs),
                "rec" => recs[t.get("id").unwrap().str()].get("size").unwrap().num(),
                k => panic!("no size for {k}"),
            }
        }
        size(t, recs)
    };
    let mut out = HashMap::new();
    for (id, r) in recs {
        let Some(Json::Arr(fields)) = r.get("fields") else {
            continue;
        };
        let fields = fields
            .iter()
            .map(|f| {
                let start = f.get("offset_bits").unwrap().num();
                let t = f.get("type").unwrap();
                let end = match f.get("bits") {
                    Some(b) => start + b.num(),
                    None => start + size_of(t) * 8,
                };
                let mut e = t;
                while e.get("k").unwrap().str() == "arr" {
                    e = e.get("of").unwrap();
                }
                let rec = (e.get("k").unwrap().str() == "rec")
                    .then(|| rec_name(e.get("id").unwrap().str()));
                CField {
                    name: f.get("name").unwrap().str().to_owned(),
                    start,
                    end,
                    rec,
                }
            })
            .collect();
        out.entry(rec_name(id)).or_insert(CRecord {
            size: r.get("size").unwrap().num(),
            fields,
        });
    }
    out
}

/// structgen's `OWNED` list: the C names config.py gives it.
fn structgen_owned() -> Vec<String> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tools/structgen/config.py"
    );
    let text = std::fs::read_to_string(path).unwrap();
    let start = text.find("\nOWNED = {").expect("config.py's OWNED");
    let body = &text[start..text[start..].find("\n}").unwrap() + start];
    body.lines()
        .filter_map(|l| {
            l.trim()
                .strip_prefix('"')
                .and_then(|l| l.split('"').next())
                .map(str::to_owned)
        })
        .collect()
}

#[test]
fn owned_structs_match_the_decomps_layout() {
    let Ok(local) = std::env::var("SSBM_RS_LOCAL") else {
        eprintln!(
            "skipped: SSBM_RS_LOCAL isn't set (the decomp's layouts are ssbm-rs's typegen/types.json)"
        );
        return;
    };
    let bytes = std::fs::read(format!("{local}/typegen/types.json")).unwrap();
    let types = Parser { b: &bytes, i: 0 }.value();
    let recs = records(&types);
    let owned = owned();
    let mut errors = Vec::new();
    for l in &owned {
        let name = l.c_name.expect("an owned struct names its C struct");
        match recs.get(name) {
            Some(c) => errors.extend(check(l, c, name)),
            None => errors.push(format!("{name}: no such C struct")),
        }
    }
    assert!(
        errors.is_empty(),
        "the hand-written structs differ from the decomp's layout:\n{}",
        errors.join("\n")
    );
    let mut ours: Vec<String> = owned.iter().map(|l| l.c_name.unwrap().to_owned()).collect();
    let mut theirs = structgen_owned();
    ours.sort();
    theirs.sort();
    assert_eq!(
        ours, theirs,
        "the layout check's list and structgen's OWNED differ"
    );
}

#[cfg(test)]
mod proof {
    use super::*;
    use crate::game::mirror::{Cx, Mem};
    use melee_data::archive::DataRef;

    /// A C struct of an int, a float, a 2-byte short, a flag byte's three bitfields and the
    /// compiler's padding: `{ int a; float b; short c; u8 d:1, e:2, f:5; /* pad */ }`.
    fn c_record() -> CRecord {
        let f = |name: &str, start: u32, end: u32| CField {
            name: name.to_owned(),
            start,
            end,
            rec: None,
        };
        CRecord {
            size: 12,
            fields: vec![
                f("a", 0, 32),
                f("b", 32, 64),
                f("c", 64, 80),
                f("d", 80, 81),
                f("e", 81, 83),
                f("f", 83, 88),
            ],
        }
    }

    #[derive(melee_derive::Mirror, Debug, Default, PartialEq)]
    #[mirror(c = "T", size = 12, pad(11, 1))]
    struct Right {
        #[at(0)]
        a: i32,
        #[at(4)]
        #[doc(alias("x4"))]
        b: f32,
        #[at(8)]
        c: i16,
        #[at(10, bits(0, 1))]
        d: bool,
        #[at(10, bits(1, 2))]
        e: u8,
        #[at(10, bits(3, 5))]
        f: u8,
    }

    /// `c` read two bytes late (from the flag byte), as a neighbour's address.
    #[derive(melee_derive::Mirror, Debug, Default)]
    #[mirror(c = "T", size = 12, pad(8, 2))]
    struct Misplaced {
        #[at(0)]
        a: i32,
        #[at(4)]
        b: f32,
        #[at(10)]
        c: i16,
    }

    /// `b` left out.
    #[derive(melee_derive::Mirror, Debug, Default)]
    #[mirror(c = "T", size = 12, pad(11, 1))]
    struct Missing {
        #[at(0)]
        a: i32,
        #[at(8)]
        c: i16,
        #[at(10)]
        flags: u8,
    }

    /// `a` read as a short: the wrong size.
    #[derive(melee_derive::Mirror, Debug, Default)]
    #[mirror(c = "T", size = 12, pad(2, 2), pad(11, 1))]
    struct Resized {
        #[at(0)]
        a: i16,
        #[at(4)]
        b: f32,
        #[at(8)]
        c: i16,
        #[at(10)]
        flags: u8,
    }

    /// The flag byte whole, and a struct size off by one.
    #[derive(melee_derive::Mirror, Debug, Default)]
    #[mirror(c = "T", size = 11)]
    struct Short {
        #[at(0)]
        a: i32,
        #[at(4)]
        b: f32,
        #[at(8)]
        c: i16,
        #[at(10)]
        flags: u8,
    }

    #[test]
    fn the_check_passes_a_right_layout_and_fails_each_mutation() {
        let c = c_record();
        assert_eq!(
            check(&Right::layout().unwrap(), &c, "T"),
            Vec::<String>::new()
        );
        let errs = |l: Option<Layout>| check(&l.unwrap(), &c, "T");
        let e = errs(Misplaced::layout());
        assert!(e.iter().any(|e| e.contains("T.c: ends at bit 96")), "{e:?}");
        let e = errs(Missing::layout());
        assert!(
            e.iter()
                .any(|e| e.contains("held by no field: b (bits 32..64)")),
            "{e:?}"
        );
        let e = errs(Resized::layout());
        assert!(e.iter().any(|e| e.contains("T.a: ends at bit 16")), "{e:?}");
        assert!(
            e.iter()
                .any(|e| e.contains("padding at 0x2 covers the decomp's fields [\"a\"]")),
            "{e:?}"
        );
        let e = errs(Short::layout());
        assert!(
            e.iter().any(|e| e.contains("size 0xb, the decomp's 0xc")),
            "{e:?}"
        );
    }

    /// Collects the comparer's labels.
    #[derive(Default)]
    struct Labels(Vec<String>);

    impl Cx for Labels {
        fn import_handle(&mut self, _: &'static str, a: u32) -> Option<u32> {
            (a != 0).then_some(a)
        }
        fn import_data(&mut self, _: u32) -> Option<DataRef> {
            None
        }
        fn compare_handle(&mut self, p: &str, _: &'static str, o: Option<u32>, t: u32) {
            if o.unwrap_or(0) != t {
                self.0.push(p.to_owned());
            }
        }
        fn compare_data(&mut self, _: &str, _: Option<DataRef>, _: u32) {}
        fn mismatch(&mut self, p: &str, o: String, t: String) {
            self.0.push(format!("{p}: {o} {t}"));
        }
    }

    /// A known image: a = 7, b = 1.5, c = -2, d = 1, e = 2, f = 5.
    const IMAGE: [u8; 12] = [0, 0, 0, 7, 0x3F, 0xC0, 0, 0, 0xFF, 0xFE, 0b1_10_00101, 0xEE];

    #[test]
    fn a_field_read_at_the_wrong_address_fails_the_compare() {
        let m = Mem {
            bytes: &IMAGE,
            base: 0x8000_0000,
        };
        let right = Right::import(&m, 0x8000_0000, &mut Labels::default());
        assert_eq!(
            right,
            Right {
                a: 7,
                b: 1.5,
                c: -2,
                d: true,
                e: 2,
                f: 5
            }
        );
        let mut l = Labels::default();
        right.compare(&m, 0x8000_0000, "t", &mut l);
        assert!(l.0.is_empty(), "{:?}", l.0);
        // The same values, with `c` read from the neighbour's address.
        let wrong = Misplaced {
            a: 7,
            b: 1.5,
            c: -2,
        };
        let mut l = Labels::default();
        wrong.compare(&m, 0x8000_0000, "t", &mut l);
        assert_eq!(l.0, ["t.c: -2 -14866"]);
        // And a renamed field's label gives the decomp's name.
        let moved = Right { b: 2.0, ..right };
        let mut l = Labels::default();
        moved.compare(&m, 0x8000_0000, "t", &mut l);
        assert_eq!(l.0, ["t.b (x4): 2.0 1.5"]);
    }
}
