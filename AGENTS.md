# Working on melee-hd

Read [README.md](README.md), [architecture](docs/architecture.md), [the oracle](docs/oracle.md), the [porting guide](docs/porting.md) and the [engine notes](docs/engine.md) before editing.

## Scope and rules

melee-hd rewrites Melee in idiomatic Rust with a simulation that stays bit-exact with ssbm-rs at its `melee-hd-base` tag. Gameplay changes exist only as explicit rulesets or toggles.

- Never commit disc images, extracted assets, captures, replays, or cards. Keep them under ignored `local/` or outside the checkout. The one exception is the benchmark matches' replays in `data/bench`, inputs the CPU AI played in ssbm-rs.
- Never commit personal paths or names (home folders, OneDrive, agents' scratch folders), build output (`dist/`, `target/`) or files over 5 MiB. The [git hooks](#git-hooks) refuse them.
- Choose a base deliberately and use a dedicated branch or worktree. Preserve work in a local commit before handing it back.
- Local implementation authorization does not authorize pushes, PRs, publication, settings, or history rewrites. Obtain explicit scoped authorization for those actions.
- Do not add assistant authorship or co-author trailers.
- Use conventional type-prefixed branches and concrete commit subjects with a capitalized first word. Include a short body.
- Match nearby conventions, keep new code comments concise, and do not hard-wrap Markdown prose.

## Reference material

Port from the decompilation and from ssbm-rs; ssbm-rs's ports at the tag decide behavior.

- [nikhilunni/melee](https://github.com/nikhilunni/melee) has no license, so all rights are reserved. Read its documentation, tracker and notes for lessons, but never port from its source files or keep them open while porting.
- [melee-pc](https://github.com/999sian/melee-pc): its `src/pc` code is GPL-3.0 and may be reused with attribution; its `docs/` has no license grant, so take notes rather than copying text.
- [Aurora](https://github.com/encounter/aurora) (MIT) is a design reference for the renderer.
- Slippi's [slippi-ssbm-asm](https://github.com/project-slippi/slippi-ssbm-asm) defines the Slippi ruleset's behavior. Do not connect to Slippi's matchmaking servers without the Slippi team's permission.

## Simulation rules

- Nothing outside the state and the controllers reaches a tick: no host settings, clocks, disc timing, audio state, or rendering results. Platform events the game polls (a disc read completing, a camera snapshot) are explicit inputs.
- Keep the original's operation order, and its single and double precision. Use `gekko-fp` where the original fused a multiply-add, used an estimate or paired singles, and the game's own math routines instead of platform math.
- RNG draws happen in the same order as the original, every tick.
- When porting reveals a bug in the original game, keep it and add an entry to [docs/melee-bugs.md](docs/melee-bugs.md).

## Verification

Run appropriate checks and state what was tested. Disc-backed checks need `SSBM_DISC`; the captured-state checks also need ssbm-rs's local data (`SSBM_RS_LOCAL`), and some replays (`SLIPPI_REPLAYS`), as [the oracle](docs/oracle.md#running-the-checks) lists. A change to the simulation is verified by [the oracle](docs/oracle.md), not by unit tests alone.

## Git hooks

Enable them once per clone (worktrees share the setting): `git config core.hooksPath .githooks`. To also refuse a name the repository can't list without giving it away, such as your username, add `git config --add path-guard.name NAME` (or set `PATH_GUARD_NAMES`).

- `pre-commit` and `pre-merge-commit` scan everything the commit records (the index: every blob, binary included, its path and its size), so a personal path never reaches a commit, including merge commits.
- `pre-push` scans every blob new to the remote in the pushed range, in every commit, so a file added and later deleted is caught too: the backstop for commits made with `--no-verify`.

The rules are `.githooks/path-guard.conf`, shared with `tools/path-guard`, which CI and `cargo test` run over the tracked files (`cargo run -p path-guard` runs it alone). A false positive is fixed in the rules, not bypassed.

## Commands

- Build: `cargo build --locked --release`.
- Check leaf functions against ssbm-rs: `SSBM_DISC=... SSBM_RS_LOCAL=... cargo test --locked --release -p leaf-check`.
- Regenerate the port ledger (`docs/port-ledger.md`): `cargo run --release -p ledger -- --check`. Mark a port with `#[doc(alias = "NAME")]` on its Rust item; list replaced and dead functions, and system-level checks, in `ledger/`.
- Compare a recording with a replay: `target/release/melee-oracle compare ORIGINAL.slp OURS.slp` (`.exe` on Windows).
- Check rollback (restore and replay every tick, bit for bit): `SSBM_DISC=... target/release/melee-oracle sync-test REPLAY.slp --rollback 7 [--fp hardware]`.
