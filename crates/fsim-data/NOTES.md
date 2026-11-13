# fsim-data

Loading of the `data.raw` dump (`factorio --dump-data`) and typed views: recipes, machines, modules, items, fluids, entity geometry.

## Data source

Dump: `script-output/data-raw-dump.json`, generated from vanilla Factorio (Space Age included) with `factorio --mod-directory <empty-dir> --dump-data`. Use a **vanilla** install: a dump from a modded game (Pyanodon, AAI Loaders and the like) has thousands of extra recipes and no quality data. A 2.0.76 dump has 659 recipes and qualities up to legendary. A 2.1.21 dump also loads without code changes, and the renderer was checked against it.

The full dump (~28 MB) is not kept in the repo; only the excerpt `tests/fixtures/data-excerpt.json` is.

## Structure

- `raw.rs`: `RawData` (map `type → name → Value`, a `BTreeMap`, so iteration is deterministic) and `Prototype`. Loading the full dump takes about 0.2 s in release mode.
- `recipe.rs`: `Recipe`, `Ingredient`, `Product`, `ItemKind`.
- `machine.rs`: `Machine` (every prototype with `crafting_speed`) and the power parser (`"150kW"` → watts).
- `module.rs`: `Module`. `effects.rs`: `Effects`.
- `item.rs`: `Item` (every prototype with `stack_size`) and `Fluid`.
- `geometry.rs`: `EntityGeometry`, `EntityInfo`.
- `game_dir.rs`: `GameDir`, resolver of `__base__/...` paths.
- `serde_util.rs`: `lenient_vec`. `error.rs`: `Error`.

## API

Views are built on demand from `RawData`: `recipe(name)` / `recipes()`, `machine` / `machines`, `module` / `modules`, `item` / `items`, `fluid` / `fluids`, `entity_geometry` / `entity_geometries`. Single lookups return `Result<Option<_>>`, bulk iterators return `Result<_>`.

- `Ingredient` and `Product` have `fluidbox_index` (1-based machine port number, set in refinery recipes). The renderer uses it to choose active ports.
- `Recipe`: `categories` (from `categories` in 2.1 dumps, or `category` + `additional_categories`), `energy_required` (default 0.5), ingredients and results, `allow_productivity`, `allow_quality`. `Product` has `amount_min/max`, `probability`, `ignored_by_productivity`, `extra_count_fraction`.
- `Machine`: `crafting_speed`, `crafting_categories`, `module_slots`, `energy_usage_w`, `allowed_effects`, `base_effect` (e.g. +50% productivity in the biochamber and foundry).
- `EntityGeometry`: size from `tile_width/tile_height`, or, when absent, from `collision_box` rounded up (with a 1e-6 tolerance). The `oriented(quarter_turns)` method swaps width and height.
- `GameDir`: game directory from an argument or the `FSIM_GAME_DIR` variable. `resolve("__base__/graphics/x.png")` → `<game>/data/base/graphics/x.png`. Paths containing `..` are rejected.

## How vanilla data differs from the assumptions in SPEC

- Recipes have a `category` field (plus `additional_categories`) in 2.0 dumps, and a single `categories` list in 2.1.21. The parser understands both formats.
- `energy_required` may be omitted (default 0.5).
- `energy_usage` is a string with a unit.
- `ingredients` and `results` may be `{}` or missing (`recipe-unknown`, `parameter-0`). They are treated as empty.
- `collision_box` sometimes has a third element (cliff rotation) or points written as `{x, y}`.
- Unknown fields are ignored. The list of allowed effects (`allowed_effects`) may be `{}`, which is also accepted.

## Notes and limitations

- Names are plain `String`s, because SPEC requires `fsim-data` to be independent of `fsim-blueprint`. Shared newtypes would need a separate crate.
- Labs (they have `researching_speed`, not `crafting_speed`) and layered icons (`icons`) are not covered yet. Only a single `icon` is.
- Lookup by name (`machine`, `item`, `entity_geometry`) scans all prototypes. This is enough at this scale, but not indexed.
- Entity names need not be unique across types. The first match is returned (types in alphabetical order).

## Tests

- `cargo test -p fsim-data`: 13 tests on the excerpt (`tests/data.rs`), plus tests of the power parser and the path resolver.
- The full-dump test runs only when `FSIM_DUMP` is set (otherwise it is skipped):
  ```
  FSIM_DUMP=<data-raw-dump.json> cargo test --release -p fsim-data
  ```
  It checks that all recipes, machines, modules, items, fluids and geometries load, and the sizes of known entities (rocket-silo 9×9, foundry 5×5, splitter 2×1 and others).
