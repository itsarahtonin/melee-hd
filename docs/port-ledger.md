# Port ledger

Every function in ssbm-rs's symbol table and what melee-hd did with it: ported (code and a check), replaced by a native subsystem, provably dead, or still unaccounted. `cargo run --release -p ledger` regenerates this file and [port-ledger.csv](port-ledger.csv) from the code's `#[doc(alias)]` attributes, leaf-check's tests and the lists in `ledger/`.

| Area | Ported | Ported, unchecked | Replaced | Dead | Unaccounted | Total |
| --- | --- | --- | --- | --- | --- | --- |
| **All** | 14164 | 0 | 444 | 253 | 4966 | 19827 |
| MSL | 6 | 0 | 14 | 0 | 33 | 53 |
| MetroTRK | 0 | 0 | 133 | 0 | 0 | 133 |
| Runtime | 10 | 0 | 10 | 0 | 2 | 22 |
| dolphin | 221 | 0 | 216 | 2 | 398 | 837 |
| melee/cm | 124 | 0 | 0 | 0 | 3 | 127 |
| melee/db | 3 | 0 | 0 | 0 | 77 | 80 |
| melee/ef | 64 | 0 | 1 | 0 | 0 | 65 |
| melee/ft | 5412 | 0 | 3 | 21 | 1372 | 6808 |
| melee/gm | 894 | 0 | 0 | 0 | 751 | 1645 |
| melee/gr | 1473 | 0 | 0 | 225 | 919 | 2617 |
| melee/if | 175 | 0 | 0 | 0 | 146 | 321 |
| melee/it | 3665 | 0 | 0 | 0 | 612 | 4277 |
| melee/lb | 384 | 0 | 27 | 0 | 124 | 535 |
| melee/mn | 385 | 0 | 0 | 1 | 45 | 431 |
| melee/mp | 214 | 0 | 4 | 0 | 11 | 229 |
| melee/pl | 256 | 0 | 0 | 0 | 57 | 313 |
| melee/sfx | 21 | 0 | 0 | 0 | 0 | 21 |
| melee/ty | 127 | 0 | 0 | 1 | 23 | 151 |
| melee/vi | 0 | 0 | 0 | 0 | 84 | 84 |
| sysdolphin | 730 | 0 | 36 | 3 | 309 | 1078 |
