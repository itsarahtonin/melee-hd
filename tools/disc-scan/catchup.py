#!/usr/bin/env python3
"""Which fighter commands a state started partway can run (`ftAction_80073354`'s table,
`ftAction_803C07AC`), the evidence for ledger/dead.txt's proofs of its handlers.

Usage: catchup.py ASM_DIR SCAN_OUTPUT OPCODE...
  ASM_DIR is the decomp's build/GALE01/asm; SCAN_OUTPUT is what
  `melee-disc-scan fighter-commands OPCODE...` printed for the same opcodes.

`ftAction_80073354` runs only from `Fighter_ChangeMotionState`, and only when the start frame
it's given isn't 0 and its flags lack `Ft_MF_UpdateCmd` (1 << 14). This reads every call of
`Fighter_ChangeMotionState` in the asm listings and, walking back from the call through the
straight-line code before it (no label, no other call), the state (`li r4`), flags (`r5`) and
start frame (`f1`, a constant from the file's data or unknown) it passes. For each call that may
start partway (a start that isn't the constant 0, and flags without the bit or unknown) whose
state is a constant, it lists the commands of the given opcodes in that state's script for each
character the code can run for (shared code, `ftCommon` and `ft/*.s`, counts for every one);
the calls whose state isn't a constant are listed last, to trace by hand to their callers.
"""
import collections
import glob
import os
import re
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
TABLES = os.path.join(HERE, '../../crates/melee-sim/src/game/ft/motion_tables.rs')
UPDATE_CMD = 1 << 14
CH = {'Mr': 'MR', 'Fx': 'FX', 'Ca': 'CA', 'Dk': 'DK', 'Kb': 'KB', 'Kp': 'KP', 'Lk': 'LK', 'Sk': 'SK',
      'Ns': 'NS', 'Pe': 'PE', 'Pp': 'PP', 'Nn': 'NN', 'Pk': 'PK', 'Ss': 'SS', 'Ys': 'YS', 'Pr': 'PR',
      'Mt': 'MT', 'Lg': 'LG', 'Ms': 'MS', 'Zd': 'ZD', 'Cl': 'CL', 'Dr': 'DR', 'Fc': 'FC', 'Pc': 'PC',
      'Gw': 'GW', 'Gn': 'GN', 'Fe': 'FE', 'Gk': 'GK', 'Mh': 'MH', 'Ch': 'CH', 'Bo': None, 'Gl': None,
      'Sb': 'SB'}
# The characters each kinds directory's code runs for (clones share their original's code).
DIRS = {'ftMario': ['Mr', 'Dr'], 'ftFox': ['Fx', 'Fc'], 'ftCaptain': ['Ca', 'Gn'],
        'ftDonkey': ['Dk'], 'ftKirby': ['Kb'], 'ftKoopa': ['Kp', 'Gk'], 'ftLink': ['Lk', 'Cl'],
        'ftSeak': ['Sk'], 'ftNess': ['Ns'], 'ftPeach': ['Pe'], 'ftPopo': ['Pp', 'Nn'],
        'ftNana': ['Nn', 'Pp'], 'ftPikachu': ['Pk', 'Pc'], 'ftSamus': ['Ss'], 'ftYoshi': ['Ys'],
        'ftPurin': ['Pr'], 'ftMewtwo': ['Mt'], 'ftLuigi': ['Lg'], 'ftMars': ['Ms', 'Fe'],
        'ftZelda': ['Zd'], 'ftCLink': ['Cl'], 'ftDrMario': ['Dr'], 'ftFalco': ['Fc'],
        'ftPichu': ['Pc'], 'ftGameWatch': ['Gw'], 'ftGanon': ['Gn'], 'ftEmblem': ['Fe'],
        'ftGigaKoopa': ['Gk'], 'ftMasterHand': ['Mh'], 'ftCrazyHand': ['Ch'],
        'ftZakoBoy': ['Bo'], 'ftZakoGirl': ['Gl'], 'ftSandbag': ['Sb']}


def motion_tables():
    tables, cur = {}, None
    for line in open(TABLES, encoding='utf-8'):
        m = re.match(r'pub static (\w+):', line)
        if m:
            cur = m.group(1)
            tables[cur] = []
            continue
        m = re.match(r'\s+anim_id: (-?\d+),', line)
        if m and cur:
            tables[cur].append(int(m.group(1)))
    return tables


def anim_of(tables, ch, msid):
    if msid < 341:
        t = tables['FT_DATA_MOTION_STATE_LIST']
        return t[msid] if msid < len(t) else None
    t = tables.get(f'FT_{CH[ch]}_INIT_MOTION_STATE_TABLE') if CH.get(ch) else None
    return t[msid - 341] if t and msid - 341 < len(t) else None


def float_consts(lines):
    c, cur = {}, None
    for l in lines:
        m = re.match(r'\.obj ("?[@\w$.]+"?), ', l)
        if m:
            cur = m.group(1).strip('"')
            continue
        m = re.match(r'\s+\.float (\S+)', l)
        if m and cur:
            c[cur], cur = float(m.group(1)), None
            continue
        m = re.match(r'\s+\.4byte 0x([0-9A-Fa-f]{8})', l)
        if m and cur:
            c[cur], cur = struct.unpack('>f', bytes.fromhex(m.group(1)))[0], None
    return c


def ins(l):
    m = re.match(r'/\* ([0-9A-F]{8}) [0-9A-F]{8}  (?:[0-9A-F]{2} ){4}\*/\t(\S+)\s*(.*)', l)
    return (m.group(1), m.group(2), m.group(3)) if m else None


def calls(asm):
    """Each Fighter_ChangeMotionState call: (file, function, address, state, flags, start)."""
    out = []
    for f in sorted(glob.glob(asm + '/melee/**/*.s', recursive=True)):
        lines = open(f, encoding='utf-8', errors='replace').read().split('\n')
        if 'Fighter_ChangeMotionState' not in '\n'.join(lines):
            continue
        c = float_consts(lines)
        fn = None
        for i, l in enumerate(lines):
            m = re.match(r'\.fn (\S+?),', l)
            if m:
                fn = m.group(1)
            x = ins(l)
            if not x or x[1] != 'bl' or x[2] != 'Fighter_ChangeMotionState':
                continue
            r4 = r5 = f1 = None
            r5done, r5reg, r5lo = False, 'r5', None
            for j in range(i - 1, max(i - 40, 0), -1):
                if lines[j].startswith(('.L_', '.fn')):
                    break
                y = ins(lines[j])
                if not y:
                    continue
                op, a = y[1], y[2]
                if op.startswith(('st', 'cmp', 'tw', 'fcmp', 'psq_st')):
                    continue
                if op in ('bl', 'b', 'blr', 'bctrl', 'blrl'):
                    break
                if r4 is None and re.match(r'r4,', a):
                    mm = re.match(r'r4, (-?0x[0-9a-fA-F]+|-?\d+)$', a)
                    r4 = int(mm.group(1), 0) if op == 'li' and mm else f'?{op} {a}'
                if not r5done and re.match(rf'{r5reg},', a):
                    if op in ('li', 'lis'):
                        try:
                            v = int(a.split(', ')[1], 0) << (16 if op == 'lis' else 0)
                            r5 = (v + (r5lo or 0)) & 0xFFFFFFFF
                        except ValueError:
                            r5 = f'?{op} {a}'
                        r5done = True
                    elif op in ('addi', 'ori') and r5lo is None:
                        mm = re.match(r'r\d+, (r\d+), (-?0x[0-9a-fA-F]+|-?\d+)$', a)
                        if mm and mm.group(1) != 'r0':
                            r5lo, r5reg = int(mm.group(2), 0), mm.group(1)
                        else:
                            r5, r5done = f'?{op} {a}', True
                    elif op == 'mr':
                        r5reg = a.split(', ')[1]
                    else:
                        r5, r5done = f'?{op} {a}', True
                if f1 is None and re.match(r'f1,', a):
                    mm = re.match(r'f1, "?([@\w$.]+)"?@sda21\(r0\)', a)
                    f1 = c[mm.group(1)] if op == 'lfs' and mm and mm.group(1) in c else f'?{op} {a}'
            out.append((os.path.relpath(f, asm).replace('\\', '/'), fn, x[0], r4, r5, f1))
    return out


def main():
    asm, scan, want = sys.argv[1], sys.argv[2], {int(o) for o in sys.argv[3:]}
    tables = motion_tables()
    occ = collections.defaultdict(list)
    op = None
    for l in open(scan, encoding='utf-8'):
        m = re.match(r'opcode (\d+):', l)
        if m:
            op = int(m.group(1))
            continue
        m = re.match(r'\s+Pl(\w\w)\.dat \S+ anims\[(\d+)\] (\S+) at \S+ \(\S+\), frame (\d+)', l)
        if m and op in want:
            occ[(m.group(1), int(m.group(2)))].append((op, int(m.group(4)), m.group(3)))
    every = calls(asm)
    partway = [c for c in every
               if c[5] != 0.0 and not (isinstance(c[4], int) and c[4] & UPDATE_CMD)]
    print(f'{len(every)} calls of Fighter_ChangeMotionState, {len(partway)} may start partway')
    hits, unknown = collections.defaultdict(list), []
    for f, fn, addr, r4, r5, f1 in partway:
        d = f.split('/')
        kd = d[3] if len(d) > 4 and d[2] == 'kinds' else None
        chars = DIRS[kd] if kd and kd != 'ftCommon' else list(CH)
        if not isinstance(r4, int):
            unknown.append((f, fn, addr, r4, r5, f1))
            continue
        for ch in chars:
            a = anim_of(tables, ch, r4)
            for op_, fr, name in occ.get((ch, a), []) if a is not None and a >= 0 else []:
                hits[op_].append((ch, r4, a, name, fr, fn, addr, r5, f1))
    for op_ in sorted(want):
        print(f'opcode {op_}: {len(hits[op_])} commands in states a call may start partway')
        for ch, msid, a, name, fr, fn, addr, r5, f1 in hits[op_]:
            r5s = f'{r5:#x}' if isinstance(r5, int) else r5
            print(f'  {ch} state {msid} anims[{a}] {name} frame {fr}: {fn} {addr} flags {r5s} start {f1}')
    print('calls that may start partway whose state is not a constant:')
    for f, fn, addr, r4, r5, f1 in unknown:
        r5s = f'{r5:#x}' if isinstance(r5, int) else r5
        print(f'  {f} {fn} {addr} state {r4} flags {r5s} start {f1}')


main()
