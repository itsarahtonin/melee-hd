# Merges the pipeline lists of the cache folders given, in turn, into OUT: each entry once,
# where first listed (tools/bench/collect-pipelines.sh).
#
#   python3 -I tools/bench/merge-pipelines.py OUT CACHE...
#
# pipelines.bin holds 8-word keys; specialized-pipelines.bin a mark, then entries of 8 + 84
# words (melee_gpu::Renderer::specialized_key_format).
import os, sys
out, folders = sys.argv[1], sys.argv[2:]
def merge(name, head, size):
    seen, entries, mark = set(), [], None
    for f in folders:
        b = open(os.path.join(f, name), 'rb').read()
        if head:
            if mark is None:
                mark = b[:head]
            assert b[:head] == mark, f
        b = b[head:]
        assert len(b) % size == 0, f
        for i in range(0, len(b), size):
            e = b[i:i + size]
            if e not in seen:
                seen.add(e)
                entries.append(e)
    open(os.path.join(out, name), 'wb').write((mark or b'') + b''.join(entries))
    print(f'{name}: {len(entries)} entries')
merge('pipelines.bin', 0, 32)
merge('specialized-pipelines.bin', 4, 4 * (8 + 84))
