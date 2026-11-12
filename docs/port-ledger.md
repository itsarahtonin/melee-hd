# Port ledger

Every function in ssbm-rs's symbol table and what melee-hd did with it: ported (code and a check), replaced by a native subsystem, provably dead, or still unaccounted. `cargo run --release -p ledger` regenerates this file and [port-ledger.csv](port-ledger.csv) from the code's `#[doc(alias)]` attributes, leaf-check's tests and the lists in `ledger/`.

| Area | Ported | Ported, unchecked | Replaced | Dead | Unaccounted | Total |
| --- | --- | --- | --- | --- | --- | --- |
| **All** | 17696 | 0 | 926 | 777 | 428 | 19827 |
| MSL | 17 | 0 | 35 | 0 | 1 | 53 |
| MetroTRK | 0 | 0 | 133 | 0 | 0 | 133 |
| Runtime | 10 | 0 | 12 | 0 | 0 | 22 |
| dolphin | 237 | 0 | 502 | 14 | 84 | 837 |
| melee/cm | 127 | 0 | 0 | 0 | 0 | 127 |
| melee/db | 73 | 0 | 4 | 0 | 3 | 80 |
| melee/ef | 64 | 0 | 1 | 0 | 0 | 65 |
| melee/ft | 6543 | 0 | 3 | 101 | 161 | 6808 |
| melee/gm | 1610 | 0 | 0 | 4 | 31 | 1645 |
| melee/gr | 2019 | 0 | 0 | 575 | 23 | 2617 |
| melee/if | 319 | 0 | 0 | 0 | 2 | 321 |
| melee/it | 4173 | 0 | 0 | 63 | 41 | 4277 |
| melee/lb | 463 | 0 | 39 | 11 | 22 | 535 |
| melee/mn | 429 | 0 | 0 | 1 | 1 | 431 |
| melee/mp | 225 | 0 | 4 | 0 | 0 | 229 |
| melee/pl | 307 | 0 | 0 | 0 | 6 | 313 |
| melee/sfx | 21 | 0 | 0 | 0 | 0 | 21 |
| melee/ty | 150 | 0 | 0 | 1 | 0 | 151 |
| melee/vi | 84 | 0 | 0 | 0 | 0 | 84 |
| sysdolphin | 825 | 0 | 193 | 7 | 53 | 1078 |
