# Port ledger

Every function in ssbm-rs's symbol table and what melee-hd did with it: ported (code and a check), replaced by a native subsystem, provably dead, or still unaccounted. `cargo run --release -p ledger` regenerates this file and [port-ledger.csv](port-ledger.csv) from the code's `#[doc(alias)]` attributes, leaf-check's tests and the lists in `ledger/`.

| Area | Ported | Ported, unchecked | Replaced | Dead | Unaccounted | Total |
| --- | --- | --- | --- | --- | --- | --- |
| **All** | 17963 | 0 | 1007 | 815 | 42 | 19827 |
| MSL | 17 | 0 | 35 | 0 | 1 | 53 |
| MetroTRK | 0 | 0 | 133 | 0 | 0 | 133 |
| Runtime | 10 | 0 | 12 | 0 | 0 | 22 |
| dolphin | 251 | 0 | 567 | 16 | 3 | 837 |
| melee/cm | 127 | 0 | 0 | 0 | 0 | 127 |
| melee/db | 74 | 0 | 4 | 2 | 0 | 80 |
| melee/ef | 64 | 0 | 1 | 0 | 0 | 65 |
| melee/ft | 6676 | 0 | 3 | 111 | 18 | 6808 |
| melee/gm | 1632 | 0 | 0 | 8 | 5 | 1645 |
| melee/gr | 2033 | 0 | 0 | 584 | 0 | 2617 |
| melee/if | 320 | 0 | 0 | 0 | 1 | 321 |
| melee/it | 4209 | 0 | 0 | 67 | 1 | 4277 |
| melee/lb | 472 | 0 | 45 | 15 | 3 | 535 |
| melee/mn | 430 | 0 | 0 | 1 | 0 | 431 |
| melee/mp | 225 | 0 | 4 | 0 | 0 | 229 |
| melee/pl | 311 | 0 | 0 | 0 | 2 | 313 |
| melee/sfx | 21 | 0 | 0 | 0 | 0 | 21 |
| melee/ty | 150 | 0 | 0 | 1 | 0 | 151 |
| melee/vi | 84 | 0 | 0 | 0 | 0 | 84 |
| sysdolphin | 857 | 0 | 203 | 10 | 8 | 1078 |
