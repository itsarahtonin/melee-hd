# Port ledger

Every function in ssbm-rs's symbol table and what melee-hd did with it: ported (code and a check), replaced by a native subsystem, provably dead, or still unaccounted. `cargo run --release -p ledger` regenerates this file and [port-ledger.csv](port-ledger.csv) from the code's `#[doc(alias)]` attributes, leaf-check's tests and the lists in `ledger/`.

| Area | Ported | Ported, unchecked | Replaced | Dead | Unaccounted | Total |
| --- | --- | --- | --- | --- | --- | --- |
| **All** | 12959 | 0 | 441 | 251 | 6176 | 19827 |
| MSL | 6 | 0 | 14 | 0 | 33 | 53 |
| MetroTRK | 0 | 0 | 133 | 0 | 0 | 133 |
| Runtime | 10 | 0 | 10 | 0 | 2 | 22 |
| dolphin | 221 | 0 | 216 | 2 | 398 | 837 |
| melee/cm | 119 | 0 | 0 | 0 | 8 | 127 |
| melee/db | 3 | 0 | 0 | 0 | 77 | 80 |
| melee/ef | 64 | 0 | 1 | 0 | 0 | 65 |
| melee/ft | 5398 | 0 | 3 | 21 | 1386 | 6808 |
| melee/gm | 323 | 0 | 0 | 0 | 1322 | 1645 |
| melee/gr | 1473 | 0 | 0 | 225 | 919 | 2617 |
| melee/if | 168 | 0 | 0 | 0 | 153 | 321 |
| melee/it | 3658 | 0 | 0 | 0 | 619 | 4277 |
| melee/lb | 328 | 0 | 31 | 0 | 176 | 535 |
| melee/mn | 4 | 0 | 0 | 0 | 427 | 431 |
| melee/mp | 214 | 0 | 4 | 0 | 11 | 229 |
| melee/pl | 229 | 0 | 0 | 0 | 84 | 313 |
| melee/sfx | 21 | 0 | 0 | 0 | 0 | 21 |
| melee/ty | 17 | 0 | 0 | 0 | 134 | 151 |
| melee/vi | 0 | 0 | 0 | 0 | 84 | 84 |
| sysdolphin | 703 | 0 | 29 | 3 | 343 | 1078 |
