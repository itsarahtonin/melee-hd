#!/usr/bin/env python3
"""Renames the generated structs' fields everywhere they're read: regenerates `game::structs`
and `game::mirror_gen` from `config.FIELD_NAMES` (`names.py`), then rewrites every use of an
old name that rustc reports, round after round of `cargo check`, until none is left.

rustc places each use by its type, so a field of another struct with the same offset name
(`Item::x18`, a local `x18`) is left alone, and the rename can be run again on any branch's
tree (one that predates the rename, or one merging into it): only uses that no longer build
are touched.

A renamed type (`config.RENAME_TYPES`, C name to Rust name) is renamed the same way: rustc
reports each use of the old name it can no longer find (a path, an import, a struct
expression or pattern, a constructor), and the old name inside that span becomes the new one.
The C names are the decomp's (`un_804A1F10_t`), so comments citing them stay as they are.

Usage: rename_readers.py [--types TYPES_JSON] [--no-regen] [-- CARGO_CHECK_ARGS...]
  TYPES_JSON defaults to $SSBM_RS_LOCAL/typegen/types.json (ssbm-rs's typegen output).
  CARGO_CHECK_ARGS default to `--workspace --all-targets -j 4`.
Run from the repository's root. Code `cargo check` doesn't build on this host (another
platform's `cfg`) isn't reached: the script ends by listing the old names it still finds
after a `.` in Rust sources, for a look by hand.
"""

import json
import os
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).parent
sys.path.insert(0, str(HERE))
import config  # noqa: E402

STRUCTS = "crates/melee-sim/src/game"
CODES = {"E0609", "E0560", "E0026"}
MESSAGES = [
    # E0609: `fp.x2114`, `offset_of!`.
    re.compile(r"no field `(\w+)` on type `(.+)`$"),
    # E0560: a struct expression's field.
    re.compile(r"struct `(.+)` has no field named `(\w+)`$"),
    # E0026: a pattern's field.
    re.compile(r"struct `(.+)` does not have a field named `(\w+)`$"),
]
# A renamed type's old name: E0412 a type, E0422 a struct expression or pattern, E0433 a path
# (`Old::default()`), E0425 a value (a unit struct), E0432 an import, E0531 a tuple pattern.
TYPE_CODES = {"E0412", "E0422", "E0433", "E0425", "E0432", "E0531"}
OLD_TYPES = {c: rust for c, rust in config.RENAME_TYPES.items() if c != rust}
OLD_TYPE_RX = re.compile(r"\b(" + "|".join(sorted(OLD_TYPES, key=len, reverse=True)) + r")\b") if OLD_TYPES else None


C_NAMES = {rust: c for c, rust in config.RENAME_TYPES.items()}


def struct_of(ty):
    """The struct a type names, by its C name (the map's key), past references, `Box` and the
    path: `&mut structs::Fighter` is `Fighter`, `ControllerMap` is `controller_map`."""
    ty = re.sub(r"^(&(mut )?|\*(mut|const) )+", "", ty.strip())
    m = re.fullmatch(r"(?:\w+::)*Box<(.+)>", ty)
    if m:
        return struct_of(m.group(1))
    name = re.sub(r"<.*$", "", ty).split("::")[-1]
    return C_NAMES.get(name, name)


def regenerate(types):
    subprocess.run([sys.executable, str(HERE / "structgen.py"), types, STRUCTS], check=True)
    subprocess.run(["rustfmt", "--edition", "2024", f"{STRUCTS}/structs.rs", f"{STRUCTS}/mirror_gen.rs"], check=True)


def type_spans(d):
    """A renamed type's old name in error `d`: (file, start, end, None, None, "type") for each
    primary span that holds it."""
    if not OLD_TYPE_RX or not OLD_TYPE_RX.search(d["message"]):
        return []
    out = []
    for s in d["spans"]:
        if s["is_primary"] and s.get("text") and any(OLD_TYPE_RX.search(t["text"]) for t in s["text"]):
            out.append((s["file_name"], s["byte_start"], s["byte_end"], None, None, "type"))
    return out


def check(args):
    """`cargo check`'s errors: (file, start, end, old, struct, code) for each misnamed field,
    and (file, start, end, None, None, "type") for each span naming a renamed type's old name."""
    p = subprocess.run(["cargo", "check", "--message-format=json", *args], capture_output=True, text=True, encoding="utf-8", errors="replace")
    found, other = [], []
    for line in p.stdout.splitlines():
        if not line.startswith("{"):
            continue
        msg = json.loads(line)
        if msg.get("reason") != "compiler-message":
            continue
        d = msg["message"]
        if d["level"] != "error":
            continue
        code = (d.get("code") or {}).get("code")
        if code in TYPE_CODES:
            spans = type_spans(d)
            if spans:
                found.extend(spans)
                continue
        hit = None
        if code in CODES:
            for i, rx in enumerate(MESSAGES):
                m = rx.search(d["message"])
                if m:
                    field, ty = (m.group(1), m.group(2)) if i == 0 else (m.group(2), m.group(1))
                    hit = (field, struct_of(ty))
        names = config.FIELD_NAMES.get(hit[1], {}) if hit else {}
        if hit and hit[0] in names:
            span = next(s for s in d["spans"] if s["is_primary"])
            found.append((span["file_name"], span["byte_start"], span["byte_end"], hit[0], hit[1], code))
        else:
            other.append(d.get("rendered") or d["message"])
    return found, other, p.returncode


def fix(found):
    by_file = {}
    for f, a, b, old, struct, code in found:
        # rustc spells one file's path two ways across targets (`tools\oracle\src\boot.rs`
        # and `tools/oracle/src/boot.rs`): one key, or the second rewrite meets stale spans.
        by_file.setdefault(os.path.normcase(os.path.normpath(f)), set()).add((a, b, old, struct, code))
    n = 0
    for f, spans in by_file.items():
        # A type's span inside another (an import list's item and the list) is renamed by the
        # outer one.
        types = [s for s in spans if s[4] == "type"]
        spans = {s for s in spans if not (s[4] == "type" and any(t != s and t[0] <= s[0] and s[1] <= t[1] for t in types))}
        data = Path(f).read_bytes()
        for a, b, old, struct, code in sorted(spans, reverse=True, key=lambda s: (s[0], s[1])):
            if code == "type":
                text = data[a:b].decode()
                new_text = OLD_TYPE_RX.sub(lambda m: OLD_TYPES[m.group(1)], text)
                data = data[:a] + new_text.encode() + data[b:]
                n += len(OLD_TYPE_RX.findall(text))
                continue
            assert data[a:b].decode() == old, f"{f}@{a}: {data[a:b]!r} isn't {old}"
            new = config.FIELD_NAMES[struct][old]
            rest = data[b:].lstrip()
            before = data[:a].rstrip()
            # A shorthand field of a struct expression or pattern keeps its binding's name.
            shorthand = code in ("E0560", "E0026") and rest[:1] in (b",", b"}") and before[-1:] in (b"{", b",")
            data = data[:a] + (f"{new}: {old}" if shorthand else new).encode() + data[b:]
            n += 1
        Path(f).write_bytes(data)
    return n


def leftovers():
    """Old names still found after a `.` in Rust sources, outside the generated files: code no
    `cargo check` here built, or another struct's field of the same name."""
    files = []
    for root in ("crates", "tools"):
        for p in Path(root).rglob("*.rs"):
            if "target" not in p.parts:
                files.append((p, p.read_text(encoding="utf-8", errors="replace").splitlines()))
    # Names something else still defines (another struct's field, a parameter) are left out:
    # rustc placed their uses.
    defined = set()
    for _, lines in files:
        for line in lines:
            defined.update(re.findall(r"(?:^\s*|[(,]\s*)(?:pub(?:\([^)]*\))?\s+)?(\w+)\s*:\s*[\w&\[(<]", line))
    olds = {o for names in config.FIELD_NAMES.values() for o in names} - defined
    rx = re.compile(r"\.(" + "|".join(sorted(olds, key=len, reverse=True)) + r")\b") if olds else None
    # Old type names that aren't also a field's or a binding's (`controller_map`).
    old_types = set(OLD_TYPES) - defined
    trx = re.compile(r"\b(" + "|".join(sorted(old_types, key=len, reverse=True)) + r")\b") if old_types else None
    out = []
    for p, lines in files:
        if p.name in ("structs.rs", "mirror_gen.rs"):
            continue
        for i, line in enumerate(lines, 1):
            s = line.strip()
            if s.startswith("//"):
                continue
            # A renamed type's old name outside comments and aliases.
            code = re.sub(r'#\[doc\(alias.*$|"[^"]*"', "", line)
            if (rx and rx.search(line)) or (trx and trx.search(code)):
                out.append(f"{p}:{i}: {s}")
    return out


def main():
    argv = sys.argv[1:]
    cargo = ["--workspace", "--all-targets", "-j", "4"]
    if "--" in argv:
        cargo = argv[argv.index("--") + 1:]
        argv = argv[: argv.index("--")]
    types = os.path.join(os.environ.get("SSBM_RS_LOCAL", "local"), "typegen", "types.json")
    if "--types" in argv:
        types = argv[argv.index("--types") + 1]
    if "--no-regen" not in argv:
        regenerate(types)
    total = 0
    for rnd in range(1, 40):
        found, other, rc = check(cargo)
        print(f"round {rnd}: {len(found)} uses to rename, {len(other)} other errors", flush=True)
        if not found:
            break
        total += fix(found)
    print(f"{total} uses renamed")
    if other:
        print("errors left:")
        for o in other[:40]:
            print(o)
    left = leftovers()
    if left:
        print(f"{len(left)} lines still name an old field after a `.` or an old type (another struct's, or code not built here):")
        for line in left[:200]:
            print("  " + line)
    sys.exit(1 if other else 0)


if __name__ == "__main__":
    main()
