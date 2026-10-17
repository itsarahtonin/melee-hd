# Port ledger

Every function in ssbm-rs's symbol table and what melee-hd did with it: ported (code and a check), replaced by a native subsystem, provably dead, or still unaccounted. `cargo run --release -p ledger` regenerates this file and [port-ledger.csv](port-ledger.csv) from the code's `#[doc(alias)]` attributes, leaf-check's tests and the lists in `ledger/`.

| Area | Ported | Ported, unchecked | Replaced | Dead | Unaccounted | Total |
| --- | --- | --- | --- | --- | --- | --- |
| **All** | 10653 | 0 | 201 | 102 | 8871 | 19827 |
| MSL | 6 | 0 | 14 | 0 | 33 | 53 |
| MetroTRK | 0 | 0 | 133 | 0 | 0 | 133 |
| Runtime | 10 | 0 | 10 | 0 | 2 | 22 |
| dolphin | 196 | 0 | 19 | 2 | 620 | 837 |
| melee/cm | 119 | 0 | 0 | 0 | 8 | 127 |
| melee/db | 2 | 0 | 0 | 0 | 78 | 80 |
| melee/ef | 63 | 0 | 2 | 0 | 0 | 65 |
| melee/ft | 5218 | 0 | 0 | 21 | 1569 | 6808 |
| melee/gm | 137 | 0 | 0 | 0 | 1508 | 1645 |
| melee/gr | 633 | 0 | 4 | 79 | 1901 | 2617 |
| melee/if | 166 | 0 | 0 | 0 | 155 | 321 |
| melee/it | 2743 | 0 | 0 | 0 | 1534 | 4277 |
| melee/lb | 231 | 0 | 0 | 0 | 304 | 535 |
| melee/mn | 2 | 0 | 0 | 0 | 429 | 431 |
| melee/mp | 214 | 0 | 4 | 0 | 11 | 229 |
| melee/pl | 227 | 0 | 0 | 0 | 86 | 313 |
| melee/sfx | 21 | 0 | 0 | 0 | 0 | 21 |
| melee/ty | 2 | 0 | 0 | 0 | 149 | 151 |
| melee/vi | 0 | 0 | 0 | 0 | 84 | 84 |
| sysdolphin | 663 | 0 | 15 | 0 | 400 | 1078 |
