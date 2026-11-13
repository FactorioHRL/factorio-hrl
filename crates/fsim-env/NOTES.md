# fsim-env notes

## Speed

`factorio-rl-py/benchmarks/step_speed.py` plays random legal episodes through `LayoutEnv` (one
process, one thread) and prints steps per second. Measured on the maintainer's machine:

| stage | green circuit | low-density structure | production science |
|---|---|---|---|
| baseline | 3 750 | 4 150 | 1 170 |
| + prototype memo, ceiling memo, `lto = "fat"` | 6 000 | 6 900 | 2 750 |
| + stateful `Episode` (incremental mask, planes) | 14 200 | 15 500 | 7 500 |
| + shape, access, link terms (stages 1-5 of `factorio-rl-py/WORKLOG.md`) | 11 700 | 12 800 | 6 400 |

What was done:

- `fsim-data`: `recipe`, `machine` and `entity_geometry` are memoised per name (`Memo`), because
  each lookup scanned and parsed the dump.
- `Engine::ceiling` is solved once per task (bounded memo), it does not depend on the layout.
- `[profile.release]`: `lto = "fat"`, `codegen-units = 1`.
- `Episode`: the layout lives in Rust; a step sends one placement, the legal mask is updated
  only around the new machine, and the per-kind occupancy planes come back as bytes.

## Ideas not done yet

Profile first; the order below is a guess.

- **Genetic search decode** (`baselines.genetic_search`): `decode` recomputes the full legal mask
  for every gene (`legal_actions`). Build the layout with an `Episode` (or one occupancy map) and
  evaluate the population in parallel with `rayon`. Likely a large win for the genetic algorithm.
- **Linear programme per step**: the balance constraints have the same structure in every step,
  only the machine counts change. Build the model once and update the bounds, and memoise results
  by the sorted `(machine, recipe, count)` signature (helps the genetic search, which re-evaluates
  equal layouts). Worth it once the LP dominates a profile.
- **Several environments in one process**: release the GIL (`Python::detach`) and step many
  `Episode`s on a thread pool, instead of one process per environment in SB3. Less memory and
  less IPC, but a larger change to the training code.
