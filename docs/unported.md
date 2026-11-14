# What's left to port

Nothing: every function of the [port ledger](port-ledger.md), 19,827 of 19,827, is ported with a check (17,995), replaced by a native equivalent (1,007) or proven dead (825) (2026-11-14, [milestone 6](milestone-6.md)'s task 11). There were 2,105 unaccounted rows before task 11 began.

## How the last rows were reached

The last 11 rows needed situations no earlier recording entered. Sarah recorded three by hand; a bot round took the other eight, each search first proving its setup happens (the hit lands, the grab catches) before sweeping timings. Each has a session from power-on that compares clean with `--all --heap`, a clean `session-sync --rollback 7 --shortcut`, and a mutation of its port that the session catches ([the oracle](oracle.md)):

| Rows | What reaches them | Session |
| --- | --- | --- |
| `pl_80038898` | A Classic stage won with a meteor smash (a hit angled 260 to 280, the meteor cancel's range) as the last KO: Ganondorf's down air | `recordings/meteor-ko` (Sarah) |
| `ftAction_8007168C` | Link's down air hitting late in a long fall, restarted at frame 60 by its hit | `recordings/link-dair` (Sarah) |
| `fn_800DD5EC` | Kirby's up throw on Icicle Mountain landing on a floor far above before the throw's freeze | `recordings/kirby-throw` (Sarah) |
| `ftCommon_8007F7B4` | Mario's Cape ending its turn on Peach while she bounces back from a Bomber that hit (the only Bomber end with speed) | `m6-bot-cape/cape` |
| `ftPp_SpecialHi_80121FD8`, `801222E8`, `80122038`, `ftNn_Init_80123720` | Popo's Belay without Nana (Mario holding her at the Belay's frame 5 check), on Onett's collapsing drugstore roof or landing the lone air start from a double jump; Nana's landing losing the roof | `m6-bot-belay/roof`, `blair2`, `nana` |
| `fn_800DC384`, `fn_800DC404` | A grabbed Luigi lifted into the high hold from a sinking Fountain of Dreams platform, touching down during the high flinch | `m6-bot-flinch/flinch-fod` |
| `ftCo_8008EB58` | Mario's Cape on Bowser's Klaw at its frame 16 (no reaction, hitlag, no hold to break), Fox caught by the frozen Klaw box in that hitlag, and Peach's down air hitting Fox and Bowser in one of its last three frames, Fox on the lower port | `m6-bot-grabpair/grabpair` |
