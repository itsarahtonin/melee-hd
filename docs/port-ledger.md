# Port ledger

Every function in ssbm-rs's symbol table and what melee-hd did with it: ported (code and a check), replaced by a native subsystem, provably dead, or still unaccounted. `cargo run --release -p ledger` regenerates this file and [port-ledger.csv](port-ledger.csv) from the code's `#[doc(alias)]` attributes, leaf-check's tests and the lists in `ledger/`.

| Area | Ported | Ported, unchecked | Replaced | Dead | Unaccounted | Total |
| --- | --- | --- | --- | --- | --- | --- |
| **All** | 11957 | 0 | 201 | 165 | 7504 | 19827 |
| MSL | 6 | 0 | 14 | 0 | 33 | 53 |
| MetroTRK | 0 | 0 | 133 | 0 | 0 | 133 |
| Runtime | 10 | 0 | 10 | 0 | 2 | 22 |
| dolphin | 196 | 0 | 19 | 2 | 620 | 837 |
| melee/cm | 119 | 0 | 0 | 0 | 8 | 127 |
| melee/db | 2 | 0 | 0 | 0 | 78 | 80 |
| melee/ef | 63 | 0 | 2 | 0 | 0 | 65 |
| melee/ft | 5357 | 0 | 0 | 21 | 1430 | 6808 |
| melee/gm | 145 | 0 | 0 | 0 | 1500 | 1645 |
| melee/gr | 987 | 0 | 4 | 142 | 1484 | 2617 |
| melee/if | 167 | 0 | 0 | 0 | 154 | 321 |
| melee/it | 3540 | 0 | 0 | 0 | 737 | 4277 |
| melee/lb | 235 | 0 | 0 | 0 | 300 | 535 |
| melee/mn | 2 | 0 | 0 | 0 | 429 | 431 |
| melee/mp | 214 | 0 | 4 | 0 | 11 | 229 |
| melee/pl | 228 | 0 | 0 | 0 | 85 | 313 |
| melee/sfx | 21 | 0 | 0 | 0 | 0 | 21 |
| melee/ty | 2 | 0 | 0 | 0 | 149 | 151 |
| melee/vi | 0 | 0 | 0 | 0 | 84 | 84 |
| sysdolphin | 663 | 0 | 15 | 0 | 400 | 1078 |
