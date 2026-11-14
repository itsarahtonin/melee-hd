# Port ledger

Every function in ssbm-rs's symbol table and what melee-hd did with it: ported (code and a check), replaced by a native subsystem, provably dead, or still unaccounted. `cargo run --release -p ledger` regenerates this file and [port-ledger.csv](port-ledger.csv) from the code's `#[doc(alias)]` attributes, leaf-check's tests and the lists in `ledger/`.

| Area | Ported | Ported, unchecked | Replaced | Dead | Unaccounted | Total |
| --- | --- | --- | --- | --- | --- | --- |
| **All** | 17969 | 0 | 1007 | 814 | 37 | 19827 |
| MSL | 18 | 0 | 35 | 0 | 0 | 53 |
| MetroTRK | 0 | 0 | 133 | 0 | 0 | 133 |
| Runtime | 10 | 0 | 12 | 0 | 0 | 22 |
| dolphin | 252 | 0 | 567 | 18 | 0 | 837 |
| melee/cm | 127 | 0 | 0 | 0 | 0 | 127 |
| melee/db | 74 | 0 | 4 | 2 | 0 | 80 |
| melee/ef | 64 | 0 | 1 | 0 | 0 | 65 |
| melee/ft | 6669 | 0 | 3 | 104 | 32 | 6808 |
| melee/gm | 1636 | 0 | 0 | 9 | 0 | 1645 |
| melee/gr | 2033 | 0 | 0 | 584 | 0 | 2617 |
| melee/if | 321 | 0 | 0 | 0 | 0 | 321 |
| melee/it | 4209 | 0 | 0 | 64 | 4 | 4277 |
| melee/lb | 472 | 0 | 45 | 18 | 0 | 535 |
| melee/mn | 430 | 0 | 0 | 1 | 0 | 431 |
| melee/mp | 225 | 0 | 4 | 0 | 0 | 229 |
| melee/pl | 312 | 0 | 0 | 0 | 1 | 313 |
| melee/sfx | 21 | 0 | 0 | 0 | 0 | 21 |
| melee/ty | 150 | 0 | 0 | 1 | 0 | 151 |
| melee/vi | 84 | 0 | 0 | 0 | 0 | 84 |
| sysdolphin | 862 | 0 | 203 | 13 | 0 | 1078 |
