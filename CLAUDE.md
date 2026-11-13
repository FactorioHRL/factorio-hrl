# factorio-hrl

Rust workspace (`crates/fsim-*`): Factorio blueprint decoding, data dump, renderer, generator, CLI. Long-term goal: a fast steady-state factory simulator as an RL/HRL environment (see `SPEC.md`). Run `scripts/setup-data.sh` for game data (`README.md`). The maintainer extends this code by hand, so optimise for the next reader.

## Language

Write everything in English: code, identifiers, comments, doc comments, error and CLI messages, commit messages. Docs (`README.md`, `SPEC.md`, `NOTES.md`) are English too.

## Reuse before writing

Search crates.io for a maintained, widely used crate (`cargo search`, lib.rs, downloads, last release) before writing a helper. Adopt it when it covers the need, and add it to `[workspace.dependencies]`. Hand-roll only when no such crate fits, and say why in a one-line comment. Check the workspace for an existing function first (DRY).

## Rust standard

- Gate: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` all pass before a task is done.
- Model the domain in types: newtypes (`Direction`, `CenterPos`, `ItemName`), enums for variants, `Option` for absence. Invalid states fail at construction (`TryFrom`/`FromStr`), so inner code trusts its inputs.
- Errors: one `thiserror` enum per crate in `error.rs`, propagated with `?`. Library code returns `Result`; `unwrap`/`expect`/indexing that can panic appear only in tests, or with an `expect` message stating the invariant.
- API: take `&str`/`&[T]`/`impl AsRef<Path>`, return owned or iterator types. Prefer iterator chains and `impl Iterator` to intermediate `Vec`s. Derive `Debug, Clone, PartialEq, Eq, Hash` where meaningful; implement std traits (`From`, `Display`, `FromStr`, `Default`) instead of ad-hoc methods.
- Visibility: `pub(crate)` by default, `pub` only for the intended API. Re-export the public surface from `lib.rs`.
- Docs: `///` on every public item states what and why (invariants, units, coordinate conventions such as doubled coordinates), not how. Comments explain non-obvious decisions.
- `unsafe` is forbidden workspace-wide. Keep MSRV (`rust-version` in `Cargo.toml`) and `serde` for all data formats.

## DRY and KISS

- One source of truth per fact: share logic across crates through the lower crate (`fsim-blueprint`, `fsim-data`) instead of copying it.
- Choose the simplest design that passes the tests: plain functions and structs first; traits, generics and macros only once there are two real users.
- Add a dependency or abstraction for a present need, not a speculative one.
- Keep functions short and single-purpose; split a module when it holds two concepts.

## Tests

- Unit tests sit next to the code; integration tests live in `crates/*/tests/`. Tests needing the full dump or game graphics skip themselves when `FSIM_DUMP` / `FSIM_GAME_DIR` are unset.
- A bug fix starts with a failing test.
- Tests cover module code only: `src/factorio_rl` and the Rust crates. Scripts (`scripts/`), shipped configs (`configs/`) and docs are not tested, and tests build their inputs inline instead of reading shipped files.
- Wube graphics and full dumps stay out of the repo; fixtures are small excerpts (`data-excerpt.json`).
- Determinism is a feature: the same seed and input give identical output.

## Python (`factorio-rl-py`)

- Python is a thin client: rules, validation and scoring live in Rust (`fsim-env`, `fsim-calc`) and cross `fsim-py` as JSON. Add logic to the Rust side first, so every client shares it.
- Reuse libraries (`gymnasium`, `stable-baselines3`, `numpy`) before writing helpers. Type-annotate public functions and keep modules small.
- `pytest` runs on `tests/fixtures/data-excerpt.json` and needs no game data.
