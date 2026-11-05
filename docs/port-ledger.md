# Port ledger

Every function in ssbm-rs's symbol table and what melee-hd did with it: ported (code and a check), replaced by a native subsystem, provably dead, or still unaccounted. `cargo run --release -p ledger` regenerates this file and [port-ledger.csv](port-ledger.csv) from the code's `#[doc(alias)]` attributes, leaf-check's tests and the lists in `ledger/`.

| Area | Ported | Ported, unchecked | Replaced | Dead | Unaccounted | Total |
| --- | --- | --- | --- | --- | --- | --- |
| **All** | 16685 | 0 | 438 | 588 | 2116 | 19827 |
| MSL | 15 | 0 | 14 | 0 | 24 | 53 |
| MetroTRK | 0 | 0 | 133 | 0 | 0 | 133 |
| Runtime | 10 | 0 | 10 | 0 | 2 | 22 |
| dolphin | 223 | 0 | 216 | 2 | 396 | 837 |
| melee/cm | 125 | 0 | 0 | 0 | 2 | 127 |
| melee/db | 66 | 0 | 0 | 0 | 14 | 80 |
| melee/ef | 64 | 0 | 1 | 0 | 0 | 65 |
| melee/ft | 6013 | 0 | 3 | 57 | 735 | 6808 |
| melee/gm | 1477 | 0 | 0 | 0 | 168 | 1645 |
| melee/gr | 1980 | 0 | 0 | 516 | 121 | 2617 |
| melee/if | 308 | 0 | 0 | 0 | 13 | 321 |
| melee/it | 4017 | 0 | 0 | 8 | 252 | 4277 |
| melee/lb | 444 | 0 | 22 | 0 | 69 | 535 |
| melee/mn | 426 | 0 | 0 | 1 | 4 | 431 |
| melee/mp | 224 | 0 | 4 | 0 | 1 | 229 |
| melee/pl | 267 | 0 | 0 | 0 | 46 | 313 |
| melee/sfx | 21 | 0 | 0 | 0 | 0 | 21 |
| melee/ty | 148 | 0 | 0 | 1 | 2 | 151 |
| melee/vi | 84 | 0 | 0 | 0 | 0 | 84 |
| sysdolphin | 773 | 0 | 35 | 3 | 267 | 1078 |
