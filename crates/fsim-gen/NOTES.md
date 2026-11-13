# fsim-gen

Random blueprint generator. It is a placeholder: eventually layouts are to be produced by models, and the generator serves tests, rendering and bootstrap data. Depends on `fsim-blueprint` (the `Design` model) and `fsim-data` (footprints, recipes). It knows nothing about rendering (`fsim-render` is only a dev-dependency, for an example).

## API

```rust
let design = fsim_gen::generate(&data, Params::square(32, seed).with_entities(30))?;
let text = fsim_gen::to_blueprint_string(&design)?; // to paste into the game
```

## Random generator (`generate`)

Truly random: picks an entity, a direction (N/E/S/W) and a tile, places it if the footprint fits, and repeats until `entities` are placed (default 30, `DEFAULT_ENTITIES`). No template and no relations between entities.

- The pool has one entry per prototype (each prototype equally likely), from the types in `random::KINDS` (belts, undergrounds, splitters, loaders, inserters, machines, furnaces, labs, beacons, chests, pipes, tanks, poles, drills, boilers, generators, radar, walls, solar panels, accumulators, roboports, reactors, turrets, lamps, heat pipes). Conditions: the `player-creation` flag, not `factorio-logo*`, footprint side ≤ 5 (filters out e.g. the rocket silo).
- An `assembling-machine` gets a random non-hidden recipe matching its `crafting_categories`. Underground belts and loaders get a random input/output `io` (pairs are not linked).
- When the requested number of entities cannot be placed within `200 × entities` attempts, the result is `Error::TooCrowded` (not a hang or a silent shortfall).
- The bounding box need not fill the canvas: the image is as large as the spread of entities.
- Deterministic: `(seed, size, entities)` gives the same `Design`. Randomness comes from `rand` with `ChaCha8Rng` (a value-stable algorithm), see `rng.rs`.
- Export to text is `Design::to_blueprint()` from `fsim-blueprint` + `encode`.

## Legacy (`fsim_gen::legacy`)

The old template generator: machine bands between belt lanes, three styles (`Assemblers`, `Smelting`, `Chemical`), with splitters and undergrounds. It is not random in the sense above, but a pretty factory with random details, kept for previewing the renderer. `fsim gen --legacy <style>`.

## Footprints

`Canvas` is a tile board with an occupancy mask. An entity occupies its whole footprint from `fsim-data` after rotation: a 3×3 assembler is 9 tiles, a 2×2 furnace is 4, a 5×5 refinery is 25, a 2×1 splitter (1×2 after rotating to E/W) is 2. Placing on an occupied spot or off the board is an error; `place_if_free` returns `false`. The entity centre (`CenterPos`) comes out as `2*x + w`, so it is always on the grid.

## Legacy styles

All legacy styles fill the area to the edge (the bounding box equals the area exactly, checked by a test over 40 seeds × 3 styles):

- `Assemblers`: a lane, then repeated (inserter row, 3×3 assemblers every 4 tiles, inserter row, lane). Lanes are shared between bands, so the output of one feeds the next. Recipes come from a short pool, filtered by the machine's `crafting_categories`. Beacons and empty slots occur. Tiles left over from dividing the height become extra parallel lanes with splitters.
- `Smelting`: the same with 2×2 furnaces (stone/steel), laid out horizontally on a board with swapped dimensions and rotated by 90° (`rotate_cw`), so the lanes run vertically.
- `Chemical`: at the top 5×5 refineries, a 3×3 tank, a pipe row, 3×3 chemical plants (direction N/S, sometimes `mirror`) and an output lane, with assembler bands underneath.

The layout is only visually plausible: pipes connect to ports as the render rules dictate, with no care for whether the fluid network makes sense. Single lanes get 0–2 pairs of undergrounds (regular or fast, gap of 1–4 tiles) with input/output `io`, and chests stand above the tunnel so an inserter from the adjacent row has somewhere to drop. A test checks that each pair has the same `name` and `direction` and a distance of 2–5.

## Tests and example

- Unit: footprints, centre, splitter after rotation, bounds, board rotation, RNG.
- `tests/generate.rs` needs the full dump: `FSIM_DUMP=<data-raw-dump.json> cargo test -p fsim-gen` (without the variable the tests are skipped). It checks no tile overlap, 32×32 bbox, recipes, determinism, presence of characteristic entities and a round trip through the blueprint text.
- CLI for generating: `fsim gen` (crate `fsim-cli`). `examples/gen.rs` generates 3 blueprints of 32×32 (`assemblers-1`, `smelting-2`, `chemical-3`) as `.bp` and `.png`:
  ```
  cargo run --release -p fsim-gen --example gen -- <data-raw-dump.json> <out-dir> [game-dir]
  ```
  When rendering with sprites, the dump must come from the same game version as the game directory, otherwise graphic paths do not match and machines fall back to rectangles.
