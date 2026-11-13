# fsim-py

PyO3 bindings, built into the Python package `factorio-rl-py` as `factorio_rl._native`. The layer is deliberately thin: every structured value crosses the boundary as a **JSON string** (the schema is the serde types of `fsim-env`), so Python needs no mirror of the Rust types.

## API

- `Engine(dump_path)`: loads the dump once.
  - `kinds(task_json) -> json`
  - `evaluate(task_json, layout_json) -> json` (a `Report`)
  - `legal_actions(task_json, layout_json) -> bytes` (one byte per `[kind, y, x]`)
  - `analyze_blueprint(task_json, blueprint) -> json`
  - `render_png(layout_json, game_dir=None, scale=32) -> bytes`: the layout drawn by `fsim-render` (real sprites if `game_dir` holds the game graphics, schematic otherwise)
  - `export_blueprint(layout_json) -> str`
- `decode_blueprint(blueprint) -> json`: the full JSON the game stores in the blueprint string.

Errors become `ValueError`.

## Build

The crate is a `cdylib` with `pyo3/extension-module` and `abi3-py310`, so it never links libpython and a plain `cargo build` / `cargo test --workspace` works without Python dev headers. It has no Rust tests; the Python suite (`factorio-rl-py/tests`) covers it. Build and install with `maturin develop` from `factorio-rl-py` (see its README).
