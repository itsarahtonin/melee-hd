# What's left to port

Every function the [port ledger](port-ledger.md) still counts as unaccounted. Written on 2026-11-14 ([milestone 6](milestone-6.md), task 11, after the reach round's third part, the ledger's fifth sweep and Sarah's three recordings by hand: the meteor smash bonus, Link's late down air and Kirby's early landing): 8 of 19,827 functions unaccounted, against 184 when the fourth sweep last generated this page and 2,105 before task 11.

## The short answer

- **8 functions are left.** melee-hd has the code for every one of them. None is unported; no recorded session or replay reaches them yet, so none has a check.
- **No mode is known to reach an unported function.** Every mode's gate is open, the audits (`menus_audit`, `single_player_audit`, the fighter, items, CPU and stage audits) fail on anything their modes reach unported, and every session and replay diff that compares clean reached nothing unported.
- What each needs is a recording that enters it: a hand recording or another bot search. The searches already made are in the [milestone's status](milestone-6.md) (task 11's rounds) and the handoffs `m6-ledger-5` and `m6-reach-2`.

## The rows

| Function | Unit | What reaches it | Tried |
| --- | --- | --- | --- |
| `ftPp_SpecialHi_80121FD8`, `ftPp_SpecialHi_801222E8` | `ftPopo/ftpopospecialhi` | Popo's Belay without Nana: Nana held or KO'd as Popo Belays where a floor gives way (Onett's drugstore roof) | Two roof falls with Mario grabbing Nana found none |
| `ftPp_SpecialHi_80122038` | `ftPopo/ftpopospecialhi` | A lone Popo's Belay air start landing | As above |
| `ftNn_Init_80123720` | `ftNana/ftnanaspecialhi` | Nana's own Belay state | As above |
| `ftCommon_8007F7B4` | `ft/ftcommon` | Mario's Cape against Peach's Bomber while her speed isn't zero: the Bomber ending in the air | A 2.5 h bot search; Sarah's `recordings/mario-cape` reaches it three times, each in the Bomber's grounded end at zero speed, where turning the speed changes nothing |
| `ftCo_8008EB58` | `ftCommon/ftCo_Damage` | A grabber hit lightly while its victim, in its own hitlag, takes a second, weaker hit: a third fighter's multi-hit attack on a grab pair | Not reached |
| `fn_800DC384`, `fn_800DC404` | `ftCommon/ftCo_0DC2` | A grabbed victim in the high flinch (lifted past the hold's height) touching down: a grabber on a slope, or a pummel that lifts and lowers | Not reached |
