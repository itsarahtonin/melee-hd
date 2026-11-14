# What's left to port

Every function the [port ledger](port-ledger.md) still counts as unaccounted. Written on 2026-11-14 from main at `cc10a322` ([milestone 6](milestone-6.md), task 11, after the reach round's third part and the ledger's fifth sweep): 11 of 19,827 functions unaccounted, against 184 when the fourth sweep last generated this page and 2,105 before task 11.

## The short answer

- **11 functions are left.** melee-hd has the code for every one of them. None is unported; no recorded session or replay reaches them yet, so none has a check.
- **No mode is known to reach an unported function.** Every mode's gate is open, the audits (`menus_audit`, `single_player_audit`, the fighter, items, CPU and stage audits) fail on anything their modes reach unported, and every session and replay diff that compares clean reached nothing unported.
- What each needs is a recording that enters it: a hand recording or another bot search. The searches already made are in the [milestone's status](milestone-6.md) (task 11's rounds) and the handoffs `m6-ledger-5` and `m6-reach-2`.

## The rows

| Function | Unit | What reaches it | Tried |
| --- | --- | --- | --- |
| `pl_80038898` | `pl/plbonus` | A Classic, Adventure or All-Star stage ending with its last KO'd player's last hit a spike (a hitbox angle from 260 to 280), with the rival bonuses on: Captain Falcon's down air, say, as the last KO | A 2 h 10 min bot search with Falcon on Classic's first stage: damage built, no KO by down air |
| `ftPp_SpecialHi_80121FD8`, `ftPp_SpecialHi_801222E8` | `ftPopo/ftpopospecialhi` | Popo's Belay without Nana: Nana held or KO'd as Popo Belays where a floor gives way (Onett's drugstore roof) | Two roof falls with Mario grabbing Nana found none |
| `ftPp_SpecialHi_80122038` | `ftPopo/ftpopospecialhi` | A lone Popo's Belay air start landing | As above |
| `ftNn_Init_80123720` | `ftNana/ftnanaspecialhi` | Nana's own Belay state | As above |
| `ftAction_8007168C` | `ft/ftaction` | Link's or Young Link's down air hitting after its frame 60: a long fall onto a target (Hyrule Temple's top to its floor) | A jump's fall hits by frame 16 |
| `ftCommon_8007F7B4` | `ft/ftcommon` | Mario's Cape against Peach's Bomber | Not searched |
| `ftCo_8008EB58` | `ftCommon/ftCo_Damage` | A grabber hit lightly while its victim, in its own hitlag, takes a second, weaker hit: a third fighter's multi-hit attack on a grab pair | Not reached |
| `fn_800DC384`, `fn_800DC404` | `ftCommon/ftCo_0DC2` | A grabbed victim in the high flinch (lifted past the hold's height) touching down: a grabber on a slope, or a pummel that lifts and lowers | Not reached |
| `fn_800DD5EC` | `ftCommon/ftCo_Throw` | Kirby's aerial throws landing early on a higher floor ahead (Hyrule Temple's steps, Corneria's hull) | Not searched there |
