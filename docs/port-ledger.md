# Port ledger

Every function in ssbm-rs's symbol table and what melee-hd did with it: ported (code and a check), replaced by a native subsystem, provably dead, or still unaccounted. `cargo run --release -p ledger` regenerates this file and [port-ledger.csv](port-ledger.csv) from the code's `#[doc(alias)]` attributes, leaf-check's tests and the lists in `ledger/`.

| Area | Ported | Ported, unchecked | Replaced | Dead | Unaccounted | Total |
| --- | --- | --- | --- | --- | --- | --- |
| **All** | 14583 | 0 | 438 | 253 | 4553 | 19827 |
| MSL | 15 | 0 | 14 | 0 | 24 | 53 |
| MetroTRK | 0 | 0 | 133 | 0 | 0 | 133 |
| Runtime | 10 | 0 | 10 | 0 | 2 | 22 |
| dolphin | 222 | 0 | 216 | 2 | 397 | 837 |
| melee/cm | 124 | 0 | 0 | 0 | 3 | 127 |
| melee/db | 65 | 0 | 0 | 0 | 15 | 80 |
| melee/ef | 64 | 0 | 1 | 0 | 0 | 65 |
| melee/ft | 5415 | 0 | 3 | 21 | 1369 | 6808 |
| melee/gm | 972 | 0 | 0 | 0 | 673 | 1645 |
| melee/gr | 1473 | 0 | 0 | 225 | 919 | 2617 |
| melee/if | 290 | 0 | 0 | 0 | 31 | 321 |
| melee/it | 3686 | 0 | 0 | 0 | 591 | 4277 |
| melee/lb | 428 | 0 | 22 | 0 | 85 | 535 |
| melee/mn | 405 | 0 | 0 | 1 | 25 | 431 |
| melee/mp | 224 | 0 | 4 | 0 | 1 | 229 |
| melee/pl | 256 | 0 | 0 | 0 | 57 | 313 |
| melee/sfx | 21 | 0 | 0 | 0 | 0 | 21 |
| melee/ty | 141 | 0 | 0 | 1 | 9 | 151 |
| melee/vi | 0 | 0 | 0 | 0 | 84 | 84 |
| sysdolphin | 772 | 0 | 35 | 3 | 268 | 1078 |
