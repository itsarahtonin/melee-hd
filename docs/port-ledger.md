# Port ledger

Every function in ssbm-rs's symbol table and what melee-hd did with it: ported (code and a check), replaced by a native subsystem, provably dead, or still unaccounted. `cargo run --release -p ledger` regenerates this file and [port-ledger.csv](port-ledger.csv) from the code's `#[doc(alias)]` attributes, leaf-check's tests and the lists in `ledger/`.

| Area | Ported | Ported, unchecked | Replaced | Dead | Unaccounted | Total |
| --- | --- | --- | --- | --- | --- | --- |
| **All** | 3735 | 0 | 197 | 2 | 15893 | 19827 |
| MSL | 6 | 0 | 14 | 0 | 33 | 53 |
| MetroTRK | 0 | 0 | 133 | 0 | 0 | 133 |
| Runtime | 10 | 0 | 10 | 0 | 2 | 22 |
| dolphin | 57 | 0 | 19 | 2 | 759 | 837 |
| melee/cm | 119 | 0 | 0 | 0 | 8 | 127 |
| melee/db | 2 | 0 | 0 | 0 | 78 | 80 |
| melee/ef | 63 | 0 | 2 | 0 | 0 | 65 |
| melee/ft | 1078 | 0 | 0 | 0 | 5730 | 6808 |
| melee/gm | 106 | 0 | 0 | 0 | 1539 | 1645 |
| melee/gr | 211 | 0 | 0 | 0 | 2406 | 2617 |
| melee/if | 159 | 0 | 0 | 0 | 162 | 321 |
| melee/it | 745 | 0 | 0 | 0 | 3532 | 4277 |
| melee/lb | 184 | 0 | 0 | 0 | 351 | 535 |
| melee/mn | 2 | 0 | 0 | 0 | 429 | 431 |
| melee/mp | 214 | 0 | 4 | 0 | 11 | 229 |
| melee/pl | 170 | 0 | 0 | 0 | 143 | 313 |
| melee/sfx | 21 | 0 | 0 | 0 | 0 | 21 |
| melee/ty | 0 | 0 | 0 | 0 | 151 | 151 |
| melee/vi | 0 | 0 | 0 | 0 | 84 | 84 |
| sysdolphin | 588 | 0 | 15 | 0 | 475 | 1078 |
