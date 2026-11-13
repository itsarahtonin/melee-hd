#!/usr/bin/env python3
"""The JPEG markers of every movie frame and THP image on the disc, the evidence for
ledger/dead.txt's THPDec_803302EC.

Usage: thp_markers.py DISC
  DISC is the game's disc image.

THPVideoDecode is called only by the movie player (lbMthp's fn_8001EF5C, on an .mth file's frames)
and the 1P endings' picture (lbMthp8001FAA0, on a .thp file). Each walks a frame's markers from
its start of image to its start of scan; THPDec_803302EC skips an APP1 to APP15 segment (0xE1 to
0xEF). This walks every frame of every .mth file (from the header's first frame and size, each
frame starting with the next one's size, the JPEG at its offset 4, for the header's frame count)
and every .thp file (the JPEG at offset 0) the same way and counts the markers it meets. A frame
whose walk doesn't reach a start of scan is counted apart.
"""
import collections
import struct
import sys


def files(iso):
    """Every file of the disc's file system table: (path, offset, size)."""
    def rd(off, n):
        iso.seek(off)
        return iso.read(n)

    fst_off, fst_size = struct.unpack('>II', rd(0x424, 8))
    fst = rd(fst_off, fst_size)
    n = struct.unpack('>I', fst[8:12])[0]
    strs = fst[n * 12:]
    out = []
    parents = [(n, '')]
    for i in range(1, n):
        w0, off, size = struct.unpack('>III', fst[i * 12:i * 12 + 12])
        name = strs[w0 & 0xFFFFFF:strs.index(b'\0', w0 & 0xFFFFFF)].decode('ascii', 'replace')
        while parents and i >= parents[-1][0]:
            parents.pop()
        prefix = parents[-1][1] if parents else ''
        if w0 >> 24:
            parents.append((size, prefix + name + '/'))
        else:
            out.append((prefix + name, off, size))
    return out


def walk(rec, p):
    """THPVideoDecode's walk from p to the start of scan: the markers met, or None."""
    seen = []
    while p + 2 <= len(rec):
        if rec[p] != 0xFF:
            return None
        p += 1
        while rec[p] == 0xFF:
            p += 1
        m = rec[p]
        p += 1
        seen.append(m)
        if m == 0xDA:
            return seen
        if m != 0xD8:
            p += struct.unpack('>H', rec[p:p + 2])[0]
    return None


def main():
    iso = open(sys.argv[1], 'rb')
    markers = collections.Counter()
    totals = collections.Counter()
    for path, off, size in files(iso):
        low = path.lower()
        if not (low.endswith('.mth') or low.endswith('.thp')):
            continue
        iso.seek(off)
        data = iso.read(size)
        if low.endswith('.thp'):
            recs = [(data, 0)]
        else:
            w = struct.unpack('>16I', data[:64])
            at, count, frame_size = w[8], w[7], w[10]
            recs = []
            for _ in range(count):
                recs.append((data[at:at + frame_size], 4))
                next_size = struct.unpack('>I', data[at:at + 4])[0]
                at += frame_size
                frame_size = next_size
        bad = 0
        for rec, p in recs:
            seen = walk(rec, p)
            if seen is None:
                bad += 1
            else:
                markers.update(seen)
        totals['files'] += 1
        totals['frames'] += len(recs)
        totals['not walked'] += bad
        print(f'{path}: {len(recs)} frames, {bad} not walked to a scan')
    print(f"{totals['files']} files, {totals['frames']} frames, {totals['not walked']} not walked to a scan")
    print('markers met:', ', '.join(f'0x{k:02X} {v}' for k, v in sorted(markers.items())))
    app = sum(v for k, v in markers.items() if 0xE1 <= k <= 0xEF)
    print(f'APP1 to APP15 (THPDec_803302EC): {app}')


if __name__ == '__main__':
    main()
