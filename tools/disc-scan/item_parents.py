#!/usr/bin/env python3
"""Who an item's spawn parent and owner can be, the evidence for ledger/dead.txt's itGetTeamId.

Usage: item_parents.py DECOMP parents
       item_parents.py DECOMP stores OFFSET...
       item_parents.py DECOMP evt-unk
  DECOMP is the decomp's checkout (its src/melee and build/GALE01/asm are read).

parents: every spawn parent the C sets (`x0_parent_gobj = ...` and `Item_InitSpawnCommonFields`'s
second argument), each followed back through the parameters it came from to where it starts:
NULL, a Fighter's `->gobj`, an item's owner, or a function that takes it as a fighter
(`GET_FIGHTER` of it); anything else is printed for review, as is each function whose asm
references (bl, or a pointer to it) outnumber its C calls. It first lists the asm callers of
`Item_80268B18`/`B5C`/`B9C` (the only routes to `Item_80267AA8`) that have no C parent site.

stores: every asm store (`stw`) at OFFSET(rN), with the instruction before it that set the
stored register (0x518 is `Item::owner`, 0x51C `x51C`, 0xC64 `xC64_reflectGObj`, 0xCEC/0xCF0/
0xCFC the hit and touch sources); `li rN, 0x0` stores are counted, not listed.

evt-unk: each item kind's `evt_unk` (the logic tables' fifteenth entry), and whether it reaches
`it_8026B894`, which clears an owner equal to the unloading fighter.
"""
import glob
import os
import re
import sys

HEAD = re.compile(r'^[A-Za-z][^;\n]*?\b(\w+)\s*\(([^;{]*?)\)\s*\{', re.M | re.S)
SPAWNS = ('Item_80268B18', 'Item_80268B5C', 'Item_80268B9C')
TABLES = [('it/it_3F14.c', 'it_803F14C4'), ('it/it_279C.c', 'it_803F23CC'),
          ('it/it_3F2F.c', 'it_803F3100'), ('it/it_3F2F.c', 'it_803F4D20')]


def load(decomp):
    src = os.path.join(decomp, 'src/melee')
    asm = os.path.join(decomp, 'build/GALE01/asm')
    texts = {os.path.relpath(f, src).replace(os.sep, '/'): open(f, encoding='utf-8', errors='replace').read()
             for f in glob.glob(src + '/**/*.c', recursive=True)}
    asms = {os.path.relpath(f, asm).replace(os.sep, '/'): open(f, encoding='utf-8', errors='replace').read()
            for f in glob.glob(asm + '/melee/**/*.s', recursive=True)}
    defs = {}
    for rel, t in texts.items():
        heads = list(HEAD.finditer(t))
        for i, h in enumerate(heads):
            end = heads[i + 1].start() if i + 1 < len(heads) else len(t)
            params = [re.sub(r'\s+', ' ', p).strip() for p in h.group(2).split(',')]
            names = [(re.findall(r'(\w+)\s*$', p) or [p])[0] for p in params]
            defs.setdefault(h.group(1), (rel, names, t[h.end():end]))
    return texts, asms, defs


def split_args(s):
    out, depth, cur = [], 0, ''
    for ch in s:
        if ch in '([{':
            depth += 1
        elif ch in ')]}':
            if depth == 0:
                break
            depth -= 1
        if ch == ',' and depth == 0:
            out.append(cur.strip())
            cur = ''
        else:
            cur += ch
    out.append(cur.strip())
    return out


def enclosing(t, pos):
    name = None
    for h in HEAD.finditer(t[:pos]):
        name = h.group(1)
    return name


def c_calls(texts, func):
    out = []
    for rel, t in texts.items():
        for m in re.finditer(r'(?<![\w])' + re.escape(func) + r'\s*\(', t):
            line_start = t.rfind('\n', 0, m.start()) + 1
            if re.search(r'^\s*(/\*.*\*/\s*)?(static\s+)?(inline\s+)?[\w\*]+[\s\*]+$', t[line_start:m.start()]):
                continue  # a declaration or the definition
            out.append((rel, t[:m.start()].count('\n') + 1, enclosing(t, m.start()),
                        split_args(t[m.end():m.end() + 600])))
    return out


def asm_refs(asms, func):
    pat = re.compile(r'(?<![\w@$.])' + re.escape(func) + r'(?![\w$])')
    out = []
    for rel, a in asms.items():
        if func not in a:
            continue
        cur = None
        for l in a.split('\n'):
            m = re.match(r'\.(fn|obj) (\S+?),', l)
            if m:
                cur = m.group(2)
            elif pat.search(l) and not re.match(r'\s*\.(endfn|endobj|sym|global) ', l):
                out.append((rel, cur, bool(re.search(r'\tbl? ' + re.escape(func) + r'\b', l))))
    return out


def parents(decomp):
    texts, asms, defs = load(decomp)
    sites = []
    for rel, t in sorted(texts.items()):
        for m in re.finditer(r'x0_parent_gobj\s*=(?!=)\s*([^;]+);', t):
            sites.append((rel, t, m.start(), m.group(1).strip()))
        for m in re.finditer(r'Item_InitSpawnCommonFields\(\s*[^,]+,\s*([^,]+),', t):
            sites.append((rel, t, m.start(), m.group(1).strip()))
    site_fns = {enclosing(t, pos) for rel, t, pos, e in sites}
    callers = sorted({c for s in SPAWNS for _, c, _ in asm_refs(asms, s)})
    print(f'{len(callers)} asm functions call {"/".join(SPAWNS)}; {len(sites)} C parent sites')
    for c in callers:
        if c not in site_fns:
            print(f'  no parent site in {c} itself (an inline helper or a copied struct sets it)')
    seen = set()

    def is_fighter(func, e):
        body = defs[func][2]
        return re.search(r'(GET_FIGHTER|getFighter\w*|ftLib_\w+)\s*\(\s*(\(\w+\s*\*\)\s*)?' + re.escape(e) + r'\b', body)

    def trace(func, expr, depth):
        pad = '  ' * depth
        e = re.sub(r'^\(\w+\s*\*\)\s*', '', expr).strip()
        if e in ('NULL', '0', '0U'):
            print(f'{pad}{func}: {expr}: none')
        elif '->' in e and 'owner' in e:
            print(f'{pad}{func}: {expr}: an item\'s owner')
        elif re.fullmatch(r'\w+->gobj', e):
            print(f'{pad}{func}: {expr}: a Fighter\'s gobj')
        elif func in defs and e in defs[func][1]:
            rel, names, body = defs[func]
            if is_fighter(func, e):
                print(f'{pad}{func} ({rel}): {expr}: its fighter (GET_FIGHTER of it)')
                return
            idx = names.index(e)
            calls = c_calls(texts, func)
            refs = asm_refs(asms, func)
            bls = sum(1 for r in refs if r[2])
            flag = '' if bls == len(calls) and len(refs) == bls else '  <- REVIEW the asm references'
            print(f'{pad}{func} ({rel}): {expr} is parameter {idx}; {len(calls)} C calls, '
                  f'{bls} asm calls, {len(refs) - bls} other asm references{flag}')
            if len(refs) != bls:
                for r in refs:
                    if not r[2]:
                        print(f'{pad}    referenced by {r[1]} ({r[0]})')
            if (func, idx) in seen:
                return
            seen.add((func, idx))
            for crel, line, caller, args in calls:
                print(f'{pad}  {crel}:{line}')
                trace(caller, args[idx] if idx < len(args) else '?', depth + 2)
        else:
            print(f'{pad}{func}: {expr}  <- REVIEW')

    for rel, t, pos, expr in sites:
        print(f'== {rel}:{t[:pos].count(chr(10)) + 1}')
        trace(enclosing(t, pos), expr, 1)


def stores(decomp, offsets):
    _, asms, _ = load(decomp)
    for off in offsets:
        zero, listed, fns = 0, [], set()
        for rel, a in sorted(asms.items()):
            cur, setters = None, {}
            for l in a.split('\n'):
                m = re.match(r'\.fn (\S+?),', l)
                if m:
                    cur = m.group(1)
                    setters = {f'r{i}': f'r{i} as passed in' for i in range(3, 11)}
                ins = l.split('*/', 1)[-1].strip()
                s = re.match(r'stw (r\d+), ' + re.escape(off) + r'\((r\d+)\)', ins, re.I)
                if s:
                    fns.add(cur)
                    src = setters.get(s.group(1), '(set before a branch)')
                    if re.fullmatch(r'li r\d+, 0x0', src):
                        zero += 1
                    else:
                        listed.append(f'  {cur} ({rel}) {l.split("*/")[0].strip()[3:11]}: {ins}  <- {src}')
                    continue
                if ins.startswith('bl '):
                    setters['r3'] = ins
                d = re.match(r'([a-z]+\.?) (r\d+),', ins)
                if d and not d.group(1).startswith(('st', 'cmp')):
                    setters[d.group(2)] = ins
        print(f'== {off}: {len(fns)} functions store there; {zero} stores of li 0')
        print('\n'.join(listed))


def evt_unk(decomp):
    texts, _, defs = load(decomp)

    def reaches(fn, depth=0, seen=None):
        seen = seen if seen is not None else set()
        if fn == 'it_8026B894':
            return True
        if fn in seen or depth > 4 or fn not in defs:
            return False
        seen.add(fn)
        return any(reaches(c, depth + 1, seen) for c in re.findall(r'\b(\w+)\s*\(', defs[fn][2]))

    counts = {True: 0, False: 0}
    for rel, name in TABLES:
        t = texts[rel]
        body = t[t.index('{', t.index(name + '[')) + 1:]
        depth, entries, cur = 0, [], ''
        for ch in body:
            if ch == '{':
                depth += 1
                cur = ''
            elif ch == '}':
                if depth == 0:
                    break
                depth -= 1
                if depth == 0:
                    entries.append(cur)
            elif depth == 1:
                cur += ch
        for n, e in enumerate(entries):
            e = re.sub(r'/\*.*?\*/', '', e, flags=re.S)
            label = (re.findall(r'//\s*(.*)', e) or [''])[0]
            fields = [x.strip() for x in re.sub(r'//.*', '', e).split(',') if x.strip()]
            evt = fields[14] if len(fields) > 14 else '?'
            ok = reaches(evt)
            counts[ok] += 1
            if not ok:
                print(f'  {name}[{n}] {label}: evt_unk {evt} doesn\'t clear the owner')
    print(f'{counts[True]} kinds\' evt_unk reach it_8026B894; {counts[False]} don\'t (listed)')


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    decomp, cmd = sys.argv[1], sys.argv[2]
    if cmd == 'parents':
        parents(decomp)
    elif cmd == 'stores':
        stores(decomp, sys.argv[3:])
    elif cmd == 'evt-unk':
        evt_unk(decomp)
    else:
        sys.exit(__doc__)


if __name__ == '__main__':
    main()
