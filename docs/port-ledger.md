# Port ledger

Every function in ssbm-rs's symbol table and what melee-hd did with it: ported (code and a check), replaced by a native subsystem, provably dead, or still unaccounted. `cargo run --release -p ledger` regenerates this file and [port-ledger.csv](port-ledger.csv) from the code's `#[doc(alias)]` attributes, leaf-check's tests and the lists in `ledger/`.

| Area | Ported | Ported, unchecked | Replaced | Dead | Unaccounted | Total |
| --- | --- | --- | --- | --- | --- | --- |
| **All** | 78 | 0 | 1 | 0 | 19748 | 19827 |
| MSL | 5 | 0 | 1 | 0 | 47 | 53 |
| MetroTRK | 0 | 0 | 0 | 0 | 133 | 133 |
| Runtime | 0 | 0 | 0 | 0 | 22 | 22 |
| dolphin | 16 | 0 | 0 | 0 | 821 | 837 |
| melee/cm | 0 | 0 | 0 | 0 | 127 | 127 |
| melee/db | 0 | 0 | 0 | 0 | 80 | 80 |
| melee/ef | 0 | 0 | 0 | 0 | 65 | 65 |
| melee/ft | 0 | 0 | 0 | 0 | 6808 | 6808 |
| melee/gm | 0 | 0 | 0 | 0 | 1645 | 1645 |
| melee/gr | 0 | 0 | 0 | 0 | 2617 | 2617 |
| melee/if | 0 | 0 | 0 | 0 | 321 | 321 |
| melee/it | 0 | 0 | 0 | 0 | 4277 | 4277 |
| melee/lb | 20 | 0 | 0 | 0 | 515 | 535 |
| melee/mn | 0 | 0 | 0 | 0 | 431 | 431 |
| melee/mp | 0 | 0 | 0 | 0 | 229 | 229 |
| melee/pl | 0 | 0 | 0 | 0 | 313 | 313 |
| melee/sfx | 0 | 0 | 0 | 0 | 21 | 21 |
| melee/ty | 0 | 0 | 0 | 0 | 151 | 151 |
| melee/vi | 0 | 0 | 0 | 0 | 84 | 84 |
| sysdolphin | 37 | 0 | 0 | 0 | 1041 | 1078 |
