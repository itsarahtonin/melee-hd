# Port ledger

Every function in ssbm-rs's symbol table and what melee-hd did with it: ported (code and a check), replaced by a native subsystem, provably dead, or still unaccounted. `cargo run --release -p ledger` regenerates this file and [port-ledger.csv](port-ledger.csv) from the code's `#[doc(alias)]` attributes, leaf-check's tests and the lists in `ledger/`.

| Area | Ported | Ported, unchecked | Replaced | Dead | Unaccounted | Total |
| --- | --- | --- | --- | --- | --- | --- |
| **All** | 17934 | 0 | 941 | 790 | 162 | 19827 |
| MSL | 17 | 0 | 35 | 0 | 1 | 53 |
| MetroTRK | 0 | 0 | 133 | 0 | 0 | 133 |
| Runtime | 10 | 0 | 12 | 0 | 0 | 22 |
| dolphin | 248 | 0 | 510 | 15 | 64 | 837 |
| melee/cm | 127 | 0 | 0 | 0 | 0 | 127 |
| melee/db | 73 | 0 | 4 | 0 | 3 | 80 |
| melee/ef | 64 | 0 | 1 | 0 | 0 | 65 |
| melee/ft | 6667 | 0 | 3 | 103 | 35 | 6808 |
| melee/gm | 1631 | 0 | 0 | 6 | 8 | 1645 |
| melee/gr | 2021 | 0 | 0 | 575 | 21 | 2617 |
| melee/if | 319 | 0 | 0 | 0 | 2 | 321 |
| melee/it | 4208 | 0 | 0 | 64 | 5 | 4277 |
| melee/lb | 471 | 0 | 45 | 15 | 4 | 535 |
| melee/mn | 430 | 0 | 0 | 1 | 0 | 431 |
| melee/mp | 225 | 0 | 4 | 0 | 0 | 229 |
| melee/pl | 311 | 0 | 0 | 0 | 2 | 313 |
| melee/sfx | 21 | 0 | 0 | 0 | 0 | 21 |
| melee/ty | 150 | 0 | 0 | 1 | 0 | 151 |
| melee/vi | 84 | 0 | 0 | 0 | 0 | 84 |
| sysdolphin | 857 | 0 | 194 | 10 | 17 | 1078 |
