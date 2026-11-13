# fsim-render

Blueprint-to-PNG renderer. Depends on `fsim-blueprint` (the `Design` model), `fsim-calc` (the balance bar) and `fsim-data` (`RawData`, entity geometry, `GameDir`). Those two crates know nothing about rendering. Drawing uses `tiny-skia` (pure Rust, no system libraries).

## API

```rust
let image = fsim_render::render(&design, &raw_data, &Options::default())?;
image.save_png("out.png")?;
```

- `render(design, data, options) -> Result<Image>`. For many renders in a row, `Renderer::new(data)` is better, because it keeps a cache of loaded PNGs between calls.
- `Options`: `mode` (`Schematic` by default, or `Sprites`), `scale` (px per tile, default 32, allowed 1–512), `grid`, `indices`, `arrows`, `recipe_icons` (on by default), `game_dir`.
- `Image`: `width()`, `height()`, `pixel(x, y)`, `to_png()`, `save_png(path)`.
- Errors (`Error`): empty design, unknown entity (missing from the data), missing `game_dir` in `sprites` mode, bad `scale`, image too large (16384 px per side limit), PNG and I/O errors.

## Geometry

- Image = bounding box of entities (centre ± half the size) plus a 1-tile margin on each side. Dimensions: `ceil((bbox_width + 2) * scale)`.
- Entity size comes from `fsim-data` (`EntityGeometry`), rotated by direction. Non-cardinal entities round to the nearest cardinal direction (`quarter_turns`).
- An entity is drawn relative to its centre. Model positions are integers in doubled coordinates, so there are no rounding errors.

## Modes

- **`Schematic`:** a coloured rectangle per prototype type (assembling-machine, furnace, belts, pipes and so on; unknown types get a colour hashed from the type name). Needs no game assets.
- **`Sprites`:** real graphics from `GameDir` (`<game-dir>/data/<mod>/...`). Requires `game_dir`.
  - `sprite.rs` finds an entity's graphics in the fields `graphics_set.animation_list` (beacon), `graphics_set.animation`, then `picture`, `pictures`, `animation`, `sprite`, `idle_animation`, `off_animation`, `base_picture`, `structure`.
  - Supported shapes: `layers`, `sheets`/`sheet`, direction maps (`north`/`east`/`south`/`west`), `direction_count` and `frame_count` with `line_length`, `x`/`y`, `shift`, `scale`, `size`, `draw_as_shadow`. The first frame is drawn.
  - Sheet layout: when `frame_count > 1`, one row is one variant (columns are frames, as in a belt with 16 frames × 20 variants); with a single frame, variants lie side by side in one row (a pole with `direction_count: 4`). An explicit `line_length` takes precedence.
  - Size: a sprite pixel * `scale` corresponds to 1/32 of a tile (at an image scale of 32 px/tile it is 1:1).
  - Layers with `draw_as_glow` and `draw_as_light` are skipped (they are light overlays visible only while working).
  - An entity without graphics, with an unsupported shape or a missing file falls back to the `Schematic` rectangle, so the image is always complete.

## Draw order

1. Background, optionally the grid.
2. Shadows of all entities (`draw_as_shadow`, opacity 0.55).
3. Entities sorted by `(type rank, y, x, index)`. Ranks: belts, pipes and similar (0) < the rest (1) < inserter (2) < poles (3). Sorting by `y` covers tall sprites correctly (poles, refineries).
4. Recipe icons (when `game_dir` is set).
5. Debug overlays: direction arrows, entity numbers (a real font, see Text).

The output is deterministic (the same input gives an identical PNG).

## Recipe icon

From `recipe.icon`/`icons`, or, when absent, from `main_product` or the only result (item or fluid). The top left `icon_size`×`icon_size` square of the file is taken (icon files contain mipmap strips next to it). Position and scale come from the entity's `icon_draw_specification`, 1 tile by default. This approximates the game's alt-mode look. A translucent dark plate (95% of the icon, `ICON_PLATE*` constants) is drawn behind it, because pale or transparent icons (low density structure, plastic bar) vanish against the machine sprite. Without `game_dir` there are no icons.

## Belts (R1b, part 1)

`transport-belt` (all tiers) gets a sheet variant that depends on its neighbours. It is built from three small modules with no dependency on `tiny-skia`, so they can be moved to a shared place when layout extraction (stage 0.1) is done:

1. **`grid.rs`: `TileGrid`.** A map tile → entity index, covering the whole footprint (a splitter occupies 2 tiles).
2. **`belts.rs`: `shapes()`.** For every belt, checks 3 neighbouring tiles (back, left, right) and decides what feeds it:
   - a belt, underground or splitter facing that tile feeds it (an underground only if it is an exit; an entrance dives and does not feed forward),
   - feeding from behind → straight (side inputs are side-loading and also stay straight),
   - input from one side only, none from behind → a **curve** from that side,
   - no input at all → straight with a **start** overlay,
   - inputs from both sides and none from behind → straight.
3. **`sprite.rs`: `belt_layers()`.** Turns the shape into sheet rows from `belt_animation_set.animation_set`.

Variant numbers (1-based, row = variant, columns = 16 frames) come from comments in `base/prototypes/entity/transport-belts.lua`, because the dump does not contain them (they are engine defaults), and were checked on the sheet: 1–4 straight (E, W, N, S), 5–12 curves (`east_to_north` = input from the east, output to the north and so on), 13–20 strips on tile edges. Mods may override these indices, which we do not read.

The start is drawn (`starting_*`, an overlay on the back edge of the belt). End overlays (`ending_*`) are **not drawn**: in the sheet they lie on the same edge as the starts, despite the name, and this could not be unambiguously verified against the game.

The feeding rules are a simple version (a belt, underground or splitter facing the tile). Loaders and `linked-belt` are not handled.

## Pipes and pipe-to-ground (R1b, part 2)

A pipe picks one of 16 ready-made sprites (`pictures.straight_vertical`, `corner_up_right`, `t_up`, `cross`, `ending_*` and so on) depending on which sides it connects to. Two modules with no dependency on drawing:

1. **`fluids.rs`: openings (`Opening`).** Every entity reports where it has an open port: a tile and a side. There is a single, generic source, because ports of all entities look the same in the data (`fluid_box`, `fluid_boxes`, `input_fluid_box`, `output_fluid_box` with `pipe_connections`, where `position` is a tile inside the entity and `direction` is the side, on the 16-step scale). A pipe has 4 openings, a pipe-to-ground one regular opening (`underground` connections are skipped), and tanks and machines their own. The position is rotated by the entity direction and flipped for `mirror`.
2. **`pipes.rs`: shape.** A pipe side is connected when the neighbouring tile has an opening facing it (`Connections::faces`). Four bits (N, E, S, W) choose one of the 16 sprites. There is no separate pipe–pipe, pipe–machine or pipe–pipe-to-ground logic: it is all one rule.

A pipe-to-ground is drawn by the ordinary mechanism (`pictures` is keyed by direction). The only thing that concerns it in this step is that pipes connect to the correct side of it.

### Which machine ports are active

It depends on the recipe, and this was **verified in the game itself** (the probe in `tools/probe-fluid-boxes`; tests in `fluids.rs` have values read from Factorio 2.1.21):

- a recipe with **one** fluid on a given side (input or output) → **all** ports of that side are active (the engine joins them into one tank; hence a chemical plant with plastic has both inputs);
- with **two or more** fluids → port k for the k-th fluid;
- when a fluid has `fluidbox_index` (e.g. the `basic-oil-processing` refinery: crude is input 2, gas is output 3) → exactly that port, numbered from 1 among ports of that kind;
- no fluid on a given side → no port of that side;
- no recipe → all ports, unless the prototype has `fluid_boxes_off_when_no_fluid_recipe: true`.

`mirror` flips the ports about the machine's X axis, before rotation.

Not drawn: pipe windows (`*_window`), `pipe_covers` overlays, pipe ends at machines (`pipe_picture`, drawn dynamically by the game) and frozen/highlighted variants. `fluidbox_index` is taken from the recipe, so `fsim-data` exposes it in `Ingredient` and `Product`.

## Inserters (R1b, part 3)

An inserter consists of a platform and an arm (`inserters.rs`, drawing in `Renderer::draw_arm`).

- The **platform** (`platform_picture`) is an ordinary `Sprite4Way` sheet: four frames side by side, the frame chosen by direction (N, E, S, W). This required a fix in `sprite.rs`: `sheet` and `sheets` without `direction_count` default to 4 frames (or `frames`).
- The **arm** is two rotated sprites around the inserter's axis (the tile centre): the rod `hand_base_picture` with a joint at the axis and the hand `hand_open_picture` with jaws at the end.
- **Direction:** `pickup_position` and `insert_position` in the prototype (e.g. `[0, -1]` and `[0, 1.2]`) describe an inserter facing **south**, and the entity's `direction` is the **drop** direction. This follows from the sample blueprint: an inserter with `direction: 8` stands between a belt to the north and a machine to the south and moves items from north to south. The arm angle is the angle of the pickup vector plus `(direction - south)`.
- **Reach** is the length of the pickup vector (1 for a regular inserter, 2 for `long-handed-inserter`).

### What is reliable and what is approximate

Reliable (from the data): the platform and its frame, the arm direction and reach, the graphics of each inserter type (`inserter`, `fast`, `long-handed`, `burner`, `bulk`, `stack`).

**Approximate:** the exact position and stretch of the arm in the game live in the engine and are not in the data or the documentation. We assumed a rest pose at pickup: the rod from the axis outwards, an open hand with the jaw tips 0.15 tile beyond the pickup point (`CLAW_TIP_OVERSHOOT`). For a regular inserter the hand's shaft and the rod coincide, for a long one the rod fills the gap between the axis and the hand. It looks like the game, but is not verified pixel for pixel.

An attempt to take a screenshot from the game itself (`game.take_screenshot` via `--benchmark-graphics`) failed: the Steam version needs a restart through Steam, and the workaround (a `steam_appid.txt` file in the game directory) would require changing the install, so it was skipped. For pixel-exact agreement, a manual screenshot from the game with a few inserters in each direction is enough for comparison.

Not drawn: arm shadows (`hand_*_shadow`, they are prerendered and do not rotate with the arm), the closed hand and the item in the hand, the direction arrow, and mirroring (`mirror`/`*_flipped`).

## Balance bar (`io_summary`)

`Options::io_summary` (off by default in the library, on in `fsim`; `--no-summary` turns it off) adds below the map a bar with a naive input/output balance. The numbers come from `fsim_calc::net_flows` (see `crates/fsim-calc/NOTES.md`; it returns `Vec<Flow>`), and this crate only lays out and draws them:

- Every entity with a `recipe` and a known `crafting_speed` runs `speed / energy_required` crafts per second. Ingredients are subtracted and results added (the mean of `amount_min/max` times `probability`), and the sum over all machines gives the net balance per item or fluid.
- A negative balance is an input, a positive one an output. Intermediates produced and consumed at the same rate vanish. Numbers are amounts per second.
- Naive on purpose: no modules, beacons, quality, power, belt throughput, and no check whether the machines are connected. Furnaces and other machines without a `recipe` in the blueprint are skipped (a furnace's recipe follows from its contents).
- Look: a `Input:` header, one row per input below it (icon, name, rate `x/s`), then `Output:` and the output rows. Columns are shared by both sections; the rate is red for inputs and green for outputs. The name is the id without localisation (`iron-plate` -> `Iron plate`; the dump has no translations). The layout (`summary::layout`) is a pure function; the bar is at least as wide as the map and widens it when a row does not fit. Icons come from `product_icon` (items and fluids); without `game_dir` it is a coloured square.
- The map above the bar is unchanged, and the bar is attached at the bottom (`Image` is taller).

## Text

`text.rs` draws with a real font: DejaVu Sans (regular and bold, embedded via `include_bytes!` from `assets/fonts/`, licence in `assets/fonts/LICENSE-DejaVu.txt`) rasterised by `fontdue` (pure Rust, no system libraries). Text has a subtle shadow so it is readable on any background. Entity numbers (`indices`) are drawn with the same font. The earlier 3×5 font was removed.

## Graphics coverage

`Renderer::fallbacks()` returns the names of entities that fell back to a rectangle in the last `Sprites` render. `cargo run --release -p fsim-gen --example coverage -- data [sheet.png]` checks the whole generator pool (currently 0 of 91 fall back) and can save a sheet with all entities for visual inspection. Supported shapes were extended with: `pictures.picture` (tank), `pictures.<dir>.structure|animation` (boiler, steam engine), `pictures.single` (wall), `graphics_set.idle_animation` (centrifuge, EM plant), `robot_door.animation` (logistic chests), `animations` (pump), `chargable_graphics` (accumulator), `picture_off` (lamp), `connection_sprites` (heat pipe), roboport (`base` + `base_patch` + `base_animation`), turrets (`base_visualisation` + `folded_animation`) and loaders (`structure` sheet like an underground). A missing direction takes the opposite one, then north (e.g. a steam engine has only N and E). Layers with the `mask` flag (player colour masks) are skipped. A tank picks its frame by rotation parity.

## Splitters

A splitter consists of three parts drawn in this order (`sprite::splitter_layers`): a lane from `belt_animation_set` (straight variant by direction) on each of the two tiles, the `structure` body (one sprite per direction) and `structure_patch` (for E/W it covers the upper part of the body, for N/S it is empty). The lane under the body looks like neighbouring belts, because it uses the same frames. A splitter is still a feeder for neighbouring belts (`belts.rs`). Not drawn: lane start overlays, body animation (frame 0 only) and the frozen version.

## Underground belts

`sprite::underground_layers`: a lane (straight variant) underneath, then `back_patch`, the entrance arc (`structure.direction_in`) or exit arc (`direction_out`) depending on `Entity::io`, and `front_patch` last. The `sheet` has 4 frames, chosen by direction. Without `io` the entrance arc is drawn. An entrance no longer feeds the tile in front of it (`belts.rs`), an exit does, so the belt after an exit is straight and nothing changes before an entrance. Not drawn: connecting pipes or damage, nor a line joining the two ends.

## Scope: what exists and what does not

- **R1a (static entities):** done. Assemblers, chemical plants, refineries, furnaces, beacons, chests, poles and so on, one frame, correct direction.
- **R1b, belts, pipes, pipe-to-ground, inserters:** done (above).
- **R1b, splitters:** done. **Underground belts:** done (above).
- Also missing: `mirror` (usually changes only the pipe connection layout), tints (`tint`, `apply_runtime_tint`), `working_visualisations`, `stripes`/`filenames`, pipe overlays from `fluid_boxes`, `render_layer` of individual sprites, tiles (`tiles` are not in the model), text labels (no font).

## Tests

- Unit: sprite parser (`sprite.rs`: layers, shadows, directions, sheets, `animation_list`, belt variant rows), `TileGrid` (footprints, negative coordinates), port openings (`fluids.rs`: expected values read from the game: rotations, `mirror`, `fluidbox_index`, port activity rules, tank, pipe-to-ground without the underground side), 16 pipe shapes (`pipes.rs`), inserter arm geometry (`inserters.rs`: angle for each direction, reach, parts), `sheet`/`sheets` frames, belt shapes (`belts.rs`: start, straight, curves from each side, side-loading, inputs from both sides, splitter and underground as feeders, wide feeders, a table of all 8 curves and starts), icons, palette, digits, direction rounding.
- Integration (`tests/render.rs`): image dimensions (bbox + margin, scaling), dimension swap on rotation, entity positions from the sample blueprint (every entity centre is drawn), colours per type, overlays, determinism, errors, `Sprites` mode on synthetic PNGs created in a temp directory (including belts with different sheet variants and pipes with pipe-to-ground and machine ports: neighbours → shape → row/picture → pixel) (Wube graphics are neither needed nor kept in the repo), fallback on a missing file, recipe icon.
- Fixture: `tests/fixtures/data-excerpt.json` (2 KB, only the fields needed for geometry, from a 2.1.21 dump).
- An optional test on real graphics, enabled by environment variables (skipped otherwise):
  ```
  FSIM_DUMP=<data-raw-dump.json> FSIM_GAME_DIR=<game-dir> FSIM_RENDER_OUT=out.png \
    cargo test --release -p fsim-render --test render real -- --nocapture
  ```
  A full render of the sample blueprint (236 entities, 896×1312 px) takes about 0.17 s after the dump is loaded.
- Golden images (`tests/golden.rs`, references in `tests/golden/`) cover the `Schematic` mode with and without overlays, using the excerpt fixture; they are compared with a small per-channel tolerance. Regenerate with `UPDATE_GOLDEN=1 cargo test -p fsim-render --test golden` and review the diff. `Sprites` mode has no golden images (Wube licence, and CI has no game install); it was checked visually by hand on version 2.1.21.

## Example

`examples/render.rs` is a temporary tool for viewing results, to be replaced by `fsim render` in the CLI:

```
cargo run --release -p fsim-render --example render -- \
  <blueprint> <data-raw-dump.json> <out.png> [sprites <game-dir>] [grid] [indices] [arrows]
```
