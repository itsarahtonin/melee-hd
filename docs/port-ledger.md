# Port ledger

Every function in ssbm-rs's symbol table and what melee-hd did with it: ported (code and a check), replaced by a native subsystem, provably dead, or still unaccounted. `cargo run --release -p ledger` regenerates this file and [port-ledger.csv](port-ledger.csv) from the code's `#[doc(alias)]` attributes, leaf-check's tests and the lists in `ledger/`.

| Area | Ported | Ported, unchecked | Replaced | Dead | Unaccounted | Total |
| --- | --- | --- | --- | --- | --- | --- |
| **All** | 3638 | 0 | 197 | 2 | 15990 | 19827 |
| MSL | 6 | 0 | 14 | 0 | 33 | 53 |
| MetroTRK | 0 | 0 | 133 | 0 | 0 | 133 |
| Runtime | 10 | 0 | 10 | 0 | 2 | 22 |
| dolphin | 55 | 0 | 19 | 2 | 761 | 837 |
| melee/cm | 118 | 0 | 0 | 0 | 9 | 127 |
| melee/db | 2 | 0 | 0 | 0 | 78 | 80 |
| melee/ef | 63 | 0 | 2 | 0 | 0 | 65 |
| melee/ft | 1073 | 0 | 0 | 0 | 5735 | 6808 |
| melee/gm | 106 | 0 | 0 | 0 | 1539 | 1645 |
| melee/gr | 208 | 0 | 0 | 0 | 2409 | 2617 |
| melee/if | 156 | 0 | 0 | 0 | 165 | 321 |
| melee/it | 743 | 0 | 0 | 0 | 3534 | 4277 |
| melee/lb | 148 | 0 | 0 | 0 | 387 | 535 |
| melee/mn | 2 | 0 | 0 | 0 | 429 | 431 |
| melee/mp | 214 | 0 | 4 | 0 | 11 | 229 |
| melee/pl | 171 | 0 | 0 | 0 | 142 | 313 |
| melee/sfx | 1 | 0 | 0 | 0 | 20 | 21 |
| melee/ty | 0 | 0 | 0 | 0 | 151 | 151 |
| melee/vi | 0 | 0 | 0 | 0 | 84 | 84 |
| sysdolphin | 562 | 0 | 15 | 0 | 501 | 1078 |
