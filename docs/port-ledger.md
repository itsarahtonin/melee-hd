# Port ledger

Every function in ssbm-rs's symbol table and what melee-hd did with it: ported (code and a check), replaced by a native subsystem, provably dead, or still unaccounted. `cargo run --release -p ledger` regenerates this file and [port-ledger.csv](port-ledger.csv) from the code's `#[doc(alias)]` attributes, leaf-check's tests and the lists in `ledger/`.

| Area | Ported | Ported, unchecked | Replaced | Dead | Unaccounted | Total |
| --- | --- | --- | --- | --- | --- | --- |
| **All** | 16527 | 0 | 438 | 580 | 2282 | 19827 |
| MSL | 15 | 0 | 14 | 0 | 24 | 53 |
| MetroTRK | 0 | 0 | 133 | 0 | 0 | 133 |
| Runtime | 10 | 0 | 10 | 0 | 2 | 22 |
| dolphin | 223 | 0 | 216 | 2 | 396 | 837 |
| melee/cm | 125 | 0 | 0 | 0 | 2 | 127 |
| melee/db | 66 | 0 | 0 | 0 | 14 | 80 |
| melee/ef | 64 | 0 | 1 | 0 | 0 | 65 |
| melee/ft | 5956 | 0 | 3 | 55 | 794 | 6808 |
| melee/gm | 1424 | 0 | 0 | 0 | 221 | 1645 |
| melee/gr | 1957 | 0 | 0 | 510 | 150 | 2617 |
| melee/if | 305 | 0 | 0 | 0 | 16 | 321 |
| melee/it | 3998 | 0 | 0 | 8 | 271 | 4277 |
| melee/lb | 444 | 0 | 22 | 0 | 69 | 535 |
| melee/mn | 426 | 0 | 0 | 1 | 4 | 431 |
| melee/mp | 224 | 0 | 4 | 0 | 1 | 229 |
| melee/pl | 264 | 0 | 0 | 0 | 49 | 313 |
| melee/sfx | 21 | 0 | 0 | 0 | 0 | 21 |
| melee/ty | 148 | 0 | 0 | 1 | 2 | 151 |
| melee/vi | 84 | 0 | 0 | 0 | 0 | 84 |
| sysdolphin | 773 | 0 | 35 | 3 | 267 | 1078 |
