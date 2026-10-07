# Port ledger

Every function in ssbm-rs's symbol table and what melee-hd did with it: ported (code and a check), replaced by a native subsystem, provably dead, or still unaccounted. `cargo run --release -p ledger` regenerates this file and [port-ledger.csv](port-ledger.csv) from the code's `#[doc(alias)]` attributes, leaf-check's tests and the lists in `ledger/`.

| Area | Ported | Ported, unchecked | Replaced | Dead | Unaccounted | Total |
| --- | --- | --- | --- | --- | --- | --- |
| **All** | 2143 | 0 | 197 | 2 | 17485 | 19827 |
| MSL | 6 | 0 | 14 | 0 | 33 | 53 |
| MetroTRK | 0 | 0 | 133 | 0 | 0 | 133 |
| Runtime | 10 | 0 | 10 | 0 | 2 | 22 |
| dolphin | 55 | 0 | 19 | 2 | 761 | 837 |
| melee/cm | 118 | 0 | 0 | 0 | 9 | 127 |
| melee/db | 2 | 0 | 0 | 0 | 78 | 80 |
| melee/ef | 63 | 0 | 2 | 0 | 0 | 65 |
| melee/ft | 231 | 0 | 0 | 0 | 6577 | 6808 |
| melee/gm | 105 | 0 | 0 | 0 | 1540 | 1645 |
| melee/gr | 196 | 0 | 0 | 0 | 2421 | 2617 |
| melee/if | 156 | 0 | 0 | 0 | 165 | 321 |
| melee/it | 159 | 0 | 0 | 0 | 4118 | 4277 |
| melee/lb | 122 | 0 | 0 | 0 | 413 | 535 |
| melee/mn | 2 | 0 | 0 | 0 | 429 | 431 |
| melee/mp | 214 | 0 | 4 | 0 | 11 | 229 |
| melee/pl | 167 | 0 | 0 | 0 | 146 | 313 |
| melee/sfx | 1 | 0 | 0 | 0 | 20 | 21 |
| melee/ty | 0 | 0 | 0 | 0 | 151 | 151 |
| melee/vi | 0 | 0 | 0 | 0 | 84 | 84 |
| sysdolphin | 536 | 0 | 15 | 0 | 527 | 1078 |
