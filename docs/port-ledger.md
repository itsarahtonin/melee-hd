# Port ledger

Every function in ssbm-rs's symbol table and what melee-hd did with it: ported (code and a check), replaced by a native subsystem, provably dead, or still unaccounted. `cargo run --release -p ledger` regenerates this file and [port-ledger.csv](port-ledger.csv) from the code's `#[doc(alias)]` attributes, leaf-check's tests and the lists in `ledger/`.

| Area | Ported | Ported, unchecked | Replaced | Dead | Unaccounted | Total |
| --- | --- | --- | --- | --- | --- | --- |
| **All** | 990 | 0 | 195 | 2 | 18640 | 19827 |
| MSL | 6 | 0 | 14 | 0 | 33 | 53 |
| MetroTRK | 0 | 0 | 133 | 0 | 0 | 133 |
| Runtime | 10 | 0 | 10 | 0 | 2 | 22 |
| dolphin | 32 | 0 | 19 | 2 | 784 | 837 |
| melee/cm | 118 | 0 | 0 | 0 | 9 | 127 |
| melee/db | 0 | 0 | 0 | 0 | 80 | 80 |
| melee/ef | 0 | 0 | 0 | 0 | 65 | 65 |
| melee/ft | 2 | 0 | 0 | 0 | 6806 | 6808 |
| melee/gm | 2 | 0 | 0 | 0 | 1643 | 1645 |
| melee/gr | 182 | 0 | 0 | 0 | 2435 | 2617 |
| melee/if | 0 | 0 | 0 | 0 | 321 | 321 |
| melee/it | 0 | 0 | 0 | 0 | 4277 | 4277 |
| melee/lb | 62 | 0 | 0 | 0 | 473 | 535 |
| melee/mn | 0 | 0 | 0 | 0 | 431 | 431 |
| melee/mp | 117 | 0 | 4 | 0 | 108 | 229 |
| melee/pl | 0 | 0 | 0 | 0 | 313 | 313 |
| melee/sfx | 0 | 0 | 0 | 0 | 21 | 21 |
| melee/ty | 0 | 0 | 0 | 0 | 151 | 151 |
| melee/vi | 0 | 0 | 0 | 0 | 84 | 84 |
| sysdolphin | 459 | 0 | 15 | 0 | 604 | 1078 |
