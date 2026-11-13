# fsim-cli

The `fsim` binary (clap). Data (dump and graphics) lives in `data/`, see `README.md` and `scripts/setup-data.sh`. The directory can be changed with `--data` or `FSIM_DATA`.

## Subcommands

- `fsim gen [WxH] [-e N] [--seed N] [-o file] [--png file]`: a random blueprint: `N` (default 30) random entities at random places and directions on a grid (default 32x32, a single number = a square, 1–512). `--legacy [assemblers|smelting|chemical|random]` uses the old template generator. Without `--seed` the seed is taken from the clock and printed to stderr (so the result can be reproduced). Without `-o` the blueprint text goes to stdout, while information (style, seed, entity count) goes to stderr, so `fsim gen > bp.txt` works. Too many entities for the grid is an error, and so is a grid too small in legacy mode (e.g. `chemical` needs ≥ 18 rows).
- `fsim render <bp> -o out.png`: renders an existing blueprint (the first one from the file/book).
- `fsim decode <bp|-> [--json]`: decodes a blueprint file (`-` reads the string from stdin). By default prints, per blueprint, counts of entities, recipes and modules; `--json` prints the full decoded JSON.
- `fsim calc <bp|->`: the naive input/output balance per blueprint (items per second), computed by `fsim-calc`.
- `fsim inspect {recipe|machine|module|item|entity} <name>`: prints the typed view of a prototype from the dump.

Render options (`gen --png` and `render`): the input/output balance bar under the map is on by default (`--no-summary` hides it), `--schematic` (rectangles instead of graphics), `--scale N` (px/tile), `--grid`, `--indices`, `--arrows`. When `data/game` has no graphics, the render falls back to schematic with a warning.

Tests: `tests/cli.rs` (`assert_cmd`) covers `decode` and the missing-dump error.
