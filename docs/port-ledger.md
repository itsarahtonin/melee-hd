# Port ledger

Every function in ssbm-rs's symbol table and what melee-hd did with it: ported (code and a check), replaced by a native subsystem, provably dead, or still unaccounted. `cargo run --release -p ledger` regenerates this file and [port-ledger.csv](port-ledger.csv) from the code's `#[doc(alias)]` attributes, leaf-check's tests and the lists in `ledger/`.

| Area | Ported | Ported, unchecked | Replaced | Dead | Unaccounted | Total |
| --- | --- | --- | --- | --- | --- | --- |
| **All** | 17299 | 0 | 926 | 618 | 984 | 19827 |
| MSL | 17 | 0 | 35 | 0 | 1 | 53 |
| MetroTRK | 0 | 0 | 133 | 0 | 0 | 133 |
| Runtime | 10 | 0 | 12 | 0 | 0 | 22 |
| dolphin | 226 | 0 | 502 | 14 | 95 | 837 |
| melee/cm | 127 | 0 | 0 | 0 | 0 | 127 |
| melee/db | 73 | 0 | 4 | 0 | 3 | 80 |
| melee/ef | 64 | 0 | 1 | 0 | 0 | 65 |
| melee/ft | 6415 | 0 | 3 | 57 | 333 | 6808 |
| melee/gm | 1533 | 0 | 0 | 0 | 112 | 1645 |
| melee/gr | 1996 | 0 | 0 | 531 | 90 | 2617 |
| melee/if | 310 | 0 | 0 | 0 | 11 | 321 |
| melee/it | 4063 | 0 | 0 | 8 | 206 | 4277 |
| melee/lb | 463 | 0 | 39 | 0 | 33 | 535 |
| melee/mn | 427 | 0 | 0 | 1 | 3 | 431 |
| melee/mp | 225 | 0 | 4 | 0 | 0 | 229 |
| melee/pl | 306 | 0 | 0 | 0 | 7 | 313 |
| melee/sfx | 21 | 0 | 0 | 0 | 0 | 21 |
| melee/ty | 149 | 0 | 0 | 1 | 1 | 151 |
| melee/vi | 84 | 0 | 0 | 0 | 0 | 84 |
| sysdolphin | 790 | 0 | 193 | 6 | 89 | 1078 |
