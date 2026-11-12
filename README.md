# factorio-hrl

Factorio blueprint tooling in Rust: decoding, a typed model, a renderer and a random generator. The long-term goal is a fast factory simulator for RL/HRL training. See `SPEC.md` for the specification and `crates/*/NOTES.md` for per-crate notes.

## Requirements

- Stable Rust (see `rust-version` in `Cargo.toml`).
- To run the CLI: a data dump from a **vanilla Factorio install** (base game, optionally with Space Age; no mods). Sprite rendering additionally needs the game's graphics. Neither is in this repo, because they belong to Wube and cannot be redistributed. Building and testing need neither.

## Quick start

```
cargo build --release
cargo test --workspace
```

Building and testing need no game data: tests that need it skip themselves.

## Game data (needed by the CLI)

1. Generate a data dump from a vanilla game with no mods enabled (the only step required for `--schematic` renders):
   ```
   factorio --mod-directory <empty-dir> --dump-data
   ```
   The dump is written to `script-output/data-raw-dump.json` in Factorio's write-data directory.
2. Copy the dump, and for sprite renders the entity graphics, into `data/` (git-ignored):
   ```
   scripts/setup-data.sh "<factorio-install-dir>" <data-raw-dump.json>
   ```
   The script copies `graphics/entity` and `graphics/icons` of each mod, which is on the order of a couple of GB for the base game with Space Age. Without the graphics the renderer falls back to schematic rectangles with a warning.

The dump must come from the **same game version** as the graphics, otherwise machines render as rectangles. Use another data directory with `--data <dir>` or `FSIM_DATA`.

## CLI

```
cargo run --release -p fsim-cli -- gen 32x32 -e 30 --png out.png         # 30 random entities on 32x32, text on stdout
cargo run --release -p fsim-cli -- gen 40x24 -e 50 --seed 7 -o bp.txt --png out.png
cargo run --release -p fsim-cli -- gen --legacy chemical --png out.png   # old template generator
cargo run --release -p fsim-cli -- render bp.txt -o out.png --grid
cargo run --release -p fsim-cli -- info bp.txt
```

The render adds a bar under the map with a naive input/output balance (`--no-summary` turns it off). More in `crates/fsim-cli/NOTES.md`.

## Tests that use game data

Tests needing the full dump or graphics are skipped unless these variables are set:

```
FSIM_DUMP=<data-raw-dump.json> FSIM_GAME_DIR=<factorio-install-dir> cargo test --workspace
```

## Development

See `CLAUDE.md` for coding conventions. Before sending changes: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
