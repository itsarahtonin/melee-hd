# What's left to port

Every function the [port ledger](port-ledger.md) still counts as unaccounted. Written on 2026-11-14 ([milestone 6](milestone-6.md), task 11, after the reach round's third part, the ledger's fifth sweep and Sarah's three recordings by hand: the meteor smash bonus, Link's late down air and Kirby's early landing, then the bot round's Cape against Peach's Bomber and the lone Belay): 3 of 19,827 functions unaccounted, against 184 when the fourth sweep last generated this page and 2,105 before task 11.

## The short answer

- **3 functions are left.** melee-hd has the code for every one of them. None is unported; no recorded session or replay reaches them yet, so none has a check.
- **No mode is known to reach an unported function.** Every mode's gate is open, the audits (`menus_audit`, `single_player_audit`, the fighter, items, CPU and stage audits) fail on anything their modes reach unported, and every session and replay diff that compares clean reached nothing unported.
- What each needs is a recording that enters it: a hand recording or another bot search. The searches already made are in the [milestone's status](milestone-6.md) (task 11's rounds) and the handoffs `m6-ledger-5` and `m6-reach-2`.

## The rows

| Function | Unit | What reaches it | Tried |
| --- | --- | --- | --- |
| `ftCo_8008EB58` | `ftCommon/ftCo_Damage` | A grabber hit lightly while its victim, in its own hitlag, takes a second, weaker hit: a third fighter's multi-hit attack on a grab pair | Not reached |
| `fn_800DC384`, `fn_800DC404` | `ftCommon/ftCo_0DC2` | A grabbed victim in the high flinch (lifted past the hold's height) touching down: a grabber on a slope, or a pummel that lifts and lowers | Not reached |
