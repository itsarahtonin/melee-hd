# Port ledger

Every function in ssbm-rs's symbol table and what melee-hd did with it: ported (code and a check), replaced by a native subsystem, provably dead, or still unaccounted. `cargo run --release -p ledger` regenerates this file and [port-ledger.csv](port-ledger.csv) from the code's `#[doc(alias)]` attributes, leaf-check's tests and the lists in `ledger/`.

| Area | Ported | Ported, unchecked | Replaced | Dead | Unaccounted | Total |
| --- | --- | --- | --- | --- | --- | --- |
| **All** | 7949 | 0 | 197 | 23 | 11658 | 19827 |
| MSL | 6 | 0 | 14 | 0 | 33 | 53 |
| MetroTRK | 0 | 0 | 133 | 0 | 0 | 133 |
| Runtime | 10 | 0 | 10 | 0 | 2 | 22 |
| dolphin | 195 | 0 | 19 | 2 | 621 | 837 |
| melee/cm | 119 | 0 | 0 | 0 | 8 | 127 |
| melee/db | 2 | 0 | 0 | 0 | 78 | 80 |
| melee/ef | 63 | 0 | 2 | 0 | 0 | 65 |
| melee/ft | 3942 | 0 | 0 | 21 | 2845 | 6808 |
| melee/gm | 136 | 0 | 0 | 0 | 1509 | 1645 |
| melee/gr | 350 | 0 | 0 | 0 | 2267 | 2617 |
| melee/if | 166 | 0 | 0 | 0 | 155 | 321 |
| melee/it | 1610 | 0 | 0 | 0 | 2667 | 4277 |
| melee/lb | 228 | 0 | 0 | 0 | 307 | 535 |
| melee/mn | 2 | 0 | 0 | 0 | 429 | 431 |
| melee/mp | 214 | 0 | 4 | 0 | 11 | 229 |
| melee/pl | 220 | 0 | 0 | 0 | 93 | 313 |
| melee/sfx | 21 | 0 | 0 | 0 | 0 | 21 |
| melee/ty | 2 | 0 | 0 | 0 | 149 | 151 |
| melee/vi | 0 | 0 | 0 | 0 | 84 | 84 |
| sysdolphin | 663 | 0 | 15 | 0 | 400 | 1078 |
