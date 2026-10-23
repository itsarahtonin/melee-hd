# Port ledger

Every function in ssbm-rs's symbol table and what melee-hd did with it: ported (code and a check), replaced by a native subsystem, provably dead, or still unaccounted. `cargo run --release -p ledger` regenerates this file and [port-ledger.csv](port-ledger.csv) from the code's `#[doc(alias)]` attributes, leaf-check's tests and the lists in `ledger/`.

| Area | Ported | Ported, unchecked | Replaced | Dead | Unaccounted | Total |
| --- | --- | --- | --- | --- | --- | --- |
| **All** | 12631 | 0 | 204 | 555 | 6437 | 19827 |
| MSL | 6 | 0 | 14 | 0 | 33 | 53 |
| MetroTRK | 0 | 0 | 133 | 0 | 0 | 133 |
| Runtime | 10 | 0 | 10 | 0 | 2 | 22 |
| dolphin | 196 | 0 | 19 | 23 | 599 | 837 |
| melee/cm | 119 | 0 | 0 | 0 | 8 | 127 |
| melee/db | 3 | 0 | 0 | 62 | 15 | 80 |
| melee/ef | 63 | 0 | 2 | 0 | 0 | 65 |
| melee/ft | 5391 | 0 | 3 | 24 | 1390 | 6808 |
| melee/gm | 147 | 0 | 0 | 41 | 1457 | 1645 |
| melee/gr | 1469 | 0 | 4 | 225 | 919 | 2617 |
| melee/if | 168 | 0 | 0 | 128 | 25 | 321 |
| melee/it | 3658 | 0 | 0 | 1 | 618 | 4277 |
| melee/lb | 236 | 0 | 0 | 7 | 292 | 535 |
| melee/mn | 2 | 0 | 0 | 0 | 429 | 431 |
| melee/mp | 214 | 0 | 4 | 0 | 11 | 229 |
| melee/pl | 229 | 0 | 0 | 0 | 84 | 313 |
| melee/sfx | 21 | 0 | 0 | 0 | 0 | 21 |
| melee/ty | 3 | 0 | 0 | 4 | 144 | 151 |
| melee/vi | 0 | 0 | 0 | 5 | 79 | 84 |
| sysdolphin | 696 | 0 | 15 | 35 | 332 | 1078 |
