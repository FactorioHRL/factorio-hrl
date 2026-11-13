# fsim-blueprint

Decoding and encoding of Factorio (2.0) blueprints, plus a typed model of the factory layout.

## Structure

- `src/lib.rs`: `load(path)`, `decode(&str)`, `decode_json(&str)`, `encode(&Root)`, the `Error` type. Re-exports `raw::Root`.
- `src/raw.rs`: serde types faithful to the JSON (`Root`, `Blueprint`, `BlueprintBook`, `BookEntry`, `Entity`, `Tile`, `Position`). Unknown fields land in `extra`, so a round trip loses nothing. `Root::blueprints()` iterates over books recursively.
- `src/model/`: the model for RL (`Design`, `Entity`, `Direction`, `CenterPos`, names, `ItemStack`, `ModelError`), with tests in `tests.rs`.
- `tests/sample-blueprints/example_1`: sample blueprint (236 entities: chemical plants, refineries, pipes).
- `tests/sample-blueprints/beacon_assemblers`: sample blueprint (209 entities: 12 assembling-machine-3, 26 beacons, belts, pipes).
- `tests/sample-blueprints/automation_science_red`: red science blueprint (161 entities, 1.1 format: directions 0–7, undergrounds with `type`, splitters, 8 gear + red science assemblers). Comes from the [snowinmars/factorio-science-packs](https://github.com/snowinmars/factorio-science-packs) repository (file `science_red.txt`). No licence was found, so it is only a small test fixture.

## Format

`0` + base64(zlib(JSON)). Whitespace is ignored. A first character other than `0` yields `Error::UnsupportedVersion`. Errors are typed (`thiserror`) and nothing panics.

## Round trip

JSONs are compared after number normalisation: the original has `-85`, but after passing through the model it comes out as `-85.0`, because positions are `f64`. The game accepts both forms, so we treat them as semantically equal. Encoding always writes the 2.0 format.

## Model (`model`)

The model holds only what the simulation and RL need: what stands where and facing which way.

- `Design` is a flat `Vec<Entity>`. `Design::from_raw` / `from_root` convert raw → model, and `Design::to_blueprint()` exports the model to a 2.0 blueprint (entity numbers from 1, version 2.0.0.0, underground `type` in `extra`). Modules are exported to inventory 4 (crafting machines); not verified in the game.
- `Entity`: `name`, `pos`, `direction`, `mirror`, `quality`, `recipe: Option<Recipe { name, quality }>`, `modules: Vec<ItemStack>`, `io: Option<IoType>` (the blueprint's input/output `type` field: underground belt, loader).
- `CenterPos { x2, y2 }`: entity centre in doubled coordinates (integers, `Copy`, `Eq`, `Hash`). A position off the 0.5-tile grid yields `ModelError::OffGrid`.
- `Direction`: newtype over `u8`, 16-step scale (0–15). 1.x blueprints (version `major < 2`) are multiplied by 2. Constants `NORTH/EAST/SOUTH/WEST` and methods `rotate_cw`, `opposite`, `is_cardinal` exist. A missing `direction` field means north.
- `ItemStack { item, quality, count }`: modules summed and sorted by `(item, quality)`. The adapter reads the 2.0 format (`items[].items.in_inventory[]`) and 1.1 (name → count map). The slot does not affect the result, so it is dropped.
- Names are newtypes over `Arc<str>` (`EntityName`, `RecipeName`, `ItemName`); `Quality` defaults to `normal`. They are strings on purpose, because the entity list is open (mods, Space Age). A name → id dictionary for tensors will be built by the RL environment from `fsim-data`.
- Errors (`ModelError`) refer to the blueprint's `entity_number`, so the entity can be found in the game.

### What is deliberately NOT in the model

These stay in `raw` only (and in the round trip):

- `entity_number` (order in the `Vec` is enough),
- `wires` (pole reach depends on their supply area, not on wires; circuit networks are out of scope),
- `tiles` (terrain does not affect throughput),
- `label`, `description`, `version` (the version is used only inside the conversion, to normalise directions),
- `icons`: metadata shown in the game (up to 4 signals per blueprint in the library). They do not affect the simulation. This is not the recipe icon drawn on a machine, which comes from the `recipe` field.
- Book structure (`index`, `active_index`, nesting). `from_root` only flattens a book into a `Vec<Design>`.

## Notes and limitations

- Every requested item goes into `modules`, because without `fsim-data` there is no way to tell modules from e.g. fuel. Separating them can be done once a dependency on the data exists.
- The 1.1 format is read in blueprints only. A 1.1 data dump is out of scope.
- Underground belt and loader: direction is the travel direction for both entrance and exit. Behaviour in the game after import is not verified.
- There is no version packed as 4×u16 in the model yet (it exists only internally, as `major`).

## Tests

`cargo test -p fsim-blueprint`: round trip, whitespace, books, bad inputs, sample conversion (236 entities, 9 chemical plants with the plastic-bar recipe, efficiency-module-2 modules), direction scales, 1.1 format, module aggregation, rejection of bad data.
