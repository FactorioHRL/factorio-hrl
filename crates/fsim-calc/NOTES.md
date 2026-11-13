# fsim-calc

The factory calculator. Stage 0.0.1: a naive input/output balance, like the in-game rate calculator. Depends on `fsim-blueprint` (the `Design` model) and `fsim-data` (recipes, machines). It knows nothing about rendering; `fsim-render` and `fsim-cli` consume it.

## API

```rust
let flows = fsim_calc::net_flows(&design, &raw_data)?; // Vec<Flow>
```

- `Flow { name, per_second }`: net balance of one item or fluid. Negative is an input (consumed more than produced), positive is an output; `is_input()` tells them apart.
- `net_flows` runs every entity with a `recipe` and a known `crafting_speed` at `speed / energy_required` crafts per second. Ingredients are subtracted and results added (the mean of `amount_min/max` times `probability`), then everything is summed per name. Intermediates produced and consumed at the same rate vanish. The result lists inputs first (largest demand first), then outputs (largest production first); zero balances are skipped.
- `Error` wraps `fsim_data::Error`.

## Steady state under a supply (`steady.rs`)

`net_flows` assumes unlimited inputs. When the raw inputs arrive at fixed rates the machines share them, and the output is the solution of a small linear program (`good_lp` with the pure-Rust `microlp` solver):

- `steady_state(design, data, supply, target) -> SteadyState`: machines are grouped by `(machine, recipe)`; the variable per group is its crafts per second, bounded by `count * speed / energy_required`. Every item must balance (made ≥ used) and supplied items may be used up to their supply (`Supply` maps an item to `Some(rate)`, or `None` for unlimited). The objective is the net production of `target`, minus a tiny penalty per craft so machines that add nothing stay idle. `Group::utilisation()` is the used share of a group's capacity, and `SteadyState::flows` the net balance per item.
- `ceiling(data, kinds, max_machines, supply, target) -> f64`: the same program with the machine counts as variables (relaxed to reals) under a total budget: the most any layout of at most `max_machines` machines of those kinds could make.

Logistics is ignored: every machine reaches every item.

## Deliberately naive

No modules, beacons, quality, power, belt throughput, and no check that machines are connected. Machines without a `recipe` in the blueprint (e.g. furnaces) are skipped. Modules, beacons and quality are stage 0.0.2 (see SPEC).

## Tests

Unit tests on a tiny inline dump (plus the LP: 15 cable + 10 circuit assemblers use a 10 iron + 15 copper supply fully, missing intermediates, idle surplus, the machine-budget ceiling): rates scale with crafting speed, intermediates cancel, balanced items disappear, probability and ranges use the expectation, ordering, machines without recipe or data are ignored.

Try it: `fsim calc <blueprint>` (see `fsim-cli`).
