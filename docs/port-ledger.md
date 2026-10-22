# Port ledger

Every function in ssbm-rs's symbol table and what melee-hd did with it: ported (code and a check), replaced by a native subsystem, provably dead, or still unaccounted. `cargo run --release -p ledger` regenerates this file and [port-ledger.csv](port-ledger.csv) from the code's `#[doc(alias)]` attributes, leaf-check's tests and the lists in `ledger/`.

| Area | Ported | Ported, unchecked | Replaced | Dead | Unaccounted | Total |
| --- | --- | --- | --- | --- | --- | --- |
| **All** | 12469 | 0 | 204 | 233 | 6921 | 19827 |
| MSL | 6 | 0 | 14 | 0 | 33 | 53 |
| MetroTRK | 0 | 0 | 133 | 0 | 0 | 133 |
| Runtime | 10 | 0 | 10 | 0 | 2 | 22 |
| dolphin | 196 | 0 | 19 | 2 | 620 | 837 |
| melee/cm | 119 | 0 | 0 | 0 | 8 | 127 |
| melee/db | 2 | 0 | 0 | 0 | 78 | 80 |
| melee/ef | 63 | 0 | 2 | 0 | 0 | 65 |
| melee/ft | 5391 | 0 | 3 | 21 | 1393 | 6808 |
| melee/gm | 146 | 0 | 0 | 0 | 1499 | 1645 |
| melee/gr | 1347 | 0 | 4 | 210 | 1056 | 2617 |
| melee/if | 167 | 0 | 0 | 0 | 154 | 321 |
| melee/it | 3653 | 0 | 0 | 0 | 624 | 4277 |
| melee/lb | 236 | 0 | 0 | 0 | 299 | 535 |
| melee/mn | 2 | 0 | 0 | 0 | 429 | 431 |
| melee/mp | 214 | 0 | 4 | 0 | 11 | 229 |
| melee/pl | 229 | 0 | 0 | 0 | 84 | 313 |
| melee/sfx | 21 | 0 | 0 | 0 | 0 | 21 |
| melee/ty | 2 | 0 | 0 | 0 | 149 | 151 |
| melee/vi | 0 | 0 | 0 | 0 | 84 | 84 |
| sysdolphin | 665 | 0 | 15 | 0 | 398 | 1078 |
