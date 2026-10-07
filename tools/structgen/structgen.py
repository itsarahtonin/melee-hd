#!/usr/bin/env python3
"""Generates melee-sim's mirrors of the game's structs from the decomp's type layouts.

Each C struct becomes a plain-data Rust struct with one field per C field: integers and
floats keep their width, arrays become arrays, nested structs nest, bitfields become their own
fields, and unions become big-endian byte arrays with accessors (state code often writes one
member and reads another, which a byte array reproduces exactly). Pointers become the handle
the config maps their pointee to (`config.py`); function pointers become `CodeRef`, the
original function's address, which the game dispatches on.

Alongside, it generates the importer and comparer the oracle uses: read a struct out of
ssbm-rs's memory (a capture, or a trace region) into the mirror, and list every field where a
mirror and memory differ.

Usage: structgen.py TYPES_JSON OUT_DIR
  TYPES_JSON is ssbm-rs's typegen output (local/typegen/types.json in an ssbm-rs checkout).
"""

import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import config  # noqa: E402

INT = {(1, True): "i8", (1, False): "u8", (2, True): "i16", (2, False): "u16", (4, True): "i32", (4, False): "u32", (8, True): "i64", (8, False): "u64"}
FLOAT = {4: "f32", 8: "f64"}
RUST_KEYWORDS = {"type", "match", "ref", "move", "fn", "mod", "use", "loop", "box", "self", "super", "crate", "where", "impl", "trait", "struct", "enum", "const", "static", "mut", "pub", "in", "as", "dyn", "let", "if", "else", "for", "while", "return", "break", "continue", "true", "false", "unsafe", "extern", "async", "await", "yield", "macro", "priv", "abstract", "final", "override", "virtual", "become", "do", "typeof", "unsized", "try"}


def rec_name(rid):
    """The C name of a record id like c:@S@Fighter, or a synthesized one for anonymous records."""
    m = re.match(r"c:@(?:S|U|SA|UA)@(.+)$", rid)
    if m:
        return m.group(1)
    return "Anon_" + re.sub(r"[^A-Za-z0-9]", "_", rid)


def rust_type_name(rid):
    return config.RENAME_TYPES.get(rec_name(rid), rec_name(rid))


def field_name(name):
    n = config.RENAME_FIELDS.get(name, name)
    return f"r#{n}" if n in RUST_KEYWORDS else n


class Gen:
    def __init__(self, types):
        self.recs = types["records"]
        self.done = set()
        self.order = []

    def pointer(self, to):
        """The handle type for a pointer to `to`."""
        if to["k"] == "fn":
            return "Option<CodeRef>"
        if to["k"] == "rec":
            name = rec_name(to["id"])
            if name in config.POINTERS:
                return config.POINTERS[name]
            if name in config.DATA_TYPES:
                return "Option<DataRef>"
        return config.DEFAULT_POINTER

    def ty(self, t):
        k = t["k"]
        if k == "int":
            return INT[(t["size"], t["signed"])]
        if k == "float":
            return FLOAT[t["size"]]
        if k == "enum":
            return INT[(t["size"], True)]
        if k == "arr":
            return f"[{self.ty(t['of'])}; {t['n']}]"
        if k == "ptr":
            return self.pointer(t["to"])
        if k == "rec":
            r = self.recs[t["id"]]
            if r["kind"] == "union":
                return f"[u8; {r['size']}]"
            self.want(t["id"])
            return rust_type_name(t["id"])
        if k == "void":
            return "()"
        raise ValueError(f"no Rust type for {t}")

    def want(self, rid):
        if rid in self.done:
            return
        self.done.add(rid)
        r = self.recs[rid]
        for f in r["fields"]:
            self.ty(f["type"])
        self.order.append(rid)

    def struct(self, rid):
        r = self.recs[rid]
        name = rust_type_name(rid)
        out = [f"/// `{rec_name(rid)}`, {r['size']:#x} bytes."]
        out.append("#[derive(Clone, Debug, PartialEq)]")
        out.append(f"pub struct {name} {{")
        for f in r["fields"]:
            off = f["offset_bits"] // 8
            bits = f" (bits {f['offset_bits'] % 8}..+{f['bits']})" if "bits" in f else ""
            t = self.ty(f["type"])
            if "bits" in f:
                t = "bool" if f["bits"] == 1 else INT[(4, False)] if f["bits"] > 16 else INT[(2, False)] if f["bits"] > 8 else "u8"
            out.append(f"    /// +{off:#x}{bits}")
            out.append(f"    pub {field_name(f['name'])}: {t},")
        out.append("}")
        out.append("")
        out.append(f"impl Default for {name} {{")
        out.append("    fn default() -> Self {")
        out.append(f"        {name} {{")
        for f in r["fields"]:
            out.append(f"            {field_name(f['name'])}: {self.default(f)},")
        out.append("        }")
        out.append("    }")
        out.append("}")
        return "\n".join(out)

    def default(self, f):
        if "bits" in f:
            return "false" if f["bits"] == 1 else "0"
        return self.default_of(f["type"])

    def default_of(self, t):
        k = t["k"]
        if k in ("int", "enum"):
            return "0"
        if k == "float":
            return "0.0"
        if k == "ptr":
            return "None" if self.pointer(t["to"]).startswith("Option<") else "Default::default()"
        if k == "arr":
            inner = self.default_of(t["of"])
            return f"[{inner}; {t['n']}]" if inner in ("0", "0.0", "None") else f"core::array::from_fn(|_| {inner})"
        if k == "rec":
            r = self.recs[t["id"]]
            if r["kind"] == "union":
                return f"[0; {r['size']}]"
            return "Default::default()"
        return "Default::default()"


def main():
    types = json.load(open(sys.argv[1], encoding="utf-8"))
    out_dir = Path(sys.argv[2])
    g = Gen(types)
    by_name = {rec_name(rid): rid for rid in types["records"]}
    for root in config.ROOTS:
        g.want(by_name[root])
    parts = [
        "// SPDX-License-Identifier: GPL-3.0-or-later",
        "// Generated by tools/structgen from the decomp's type layouts. Do not edit; change",
        "// tools/structgen/config.py and regenerate.",
        "#![allow(non_snake_case, non_camel_case_types, clippy::all)]",
        "",
        config.PRELUDE,
        "",
    ]
    for rid in g.order:
        parts.append(g.struct(rid))
        parts.append("")
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "structs.rs").write_text("\n".join(parts), encoding="utf-8", newline="\n")
    print(f"{len(g.order)} structs")


if __name__ == "__main__":
    main()
