# Work log

What was built, in order, and how to check it. Newest last. Each entry names the files and the
command that shows it working, so a stage can be reviewed (or reverted) on its own.

## Plan: geometry rewards and model architectures

| stage | what | status |
|---|---|---|
| 1 | Shape weights up (learnable scale), `init_from` to continue a run, two-phase configs | done |
| 2 | `models/` folder: one interface for network architectures, `mlp` (the old one) and `cnn`, chosen in `[model]` | done |
| 3 | `cut_off` term: machines reachable only through sealed pockets of free tiles | done |
| 4 | `link` term: distance from a machine to the machines that make its ingredients | done |
| 5 | `require_access` task rule: machines nobody can reach do not produce | done |

Rules kept throughout: one concept per module, logic in Rust and weights in Python (`scoring.py`),
every term documented in `REWARDS.md`, every architecture driven by the same training TOML.

## Stage 1: learnable scale and two phases

Why: a new line of three was worth +0.0035 on a score of 0.83 (measured), too small for PPO to learn.

- `scoring.py`: default weights `adjacency` 0.05 -> 0.2, `line` 0.05 -> 0.3 (`ideal_score()` is 1.5 now), and `waste` -1 -> -2: with bigger shape rewards, idle machines added only to form a line earned about +0.007 each; at -2 they cost more than they earn.
- `config.py`, `train.py`: `[ppo] init_from = "<run>/best.zip"` continues an earlier run with this config's hyperparameters.
- `configs/training/ppo_large_phase1.toml` (shape terms off) and `ppo_large_phase2.toml` (shape terms on, `init_from`); both are `ppo_large.toml` otherwise.
- Docs: `REWARDS.md` ("A term has to be big enough to learn from", the two phases), `TUNING.md`, `README.md`.
- Check: `pytest tests/test_scoring.py`; set `init_from` in a phase 2 copy and run it for one evaluation.

## Stage 2: network architectures (`models/`)

Why: the flattened-grid MLP has no idea which tiles are neighbours, so "place it next to the one over there" must be learned separately for each of ~1000 positions.

- `src/factorio_rl/models/__init__.py`: the interface (`Architecture`: policy, options dataclass, `policy_kwargs`), discovery of the modules in the folder (`ARCHITECTURES`), `policy_for(name, options)`.
- `models/mlp.py`: the old network (`MultiInputPolicy`, option `net_arch`). `models/cnn.py`: fully convolutional `ConvPolicy` (dilated 3x3 convolutions, a logit per kind and tile, stop and value from pooled features).
- `config.py`: `[model] architecture` / `[model.options]`; the old `[ppo] policy` key is ignored with a warning. `train.py` builds the policy through `policy_for`; nothing else knows about architectures.
- Configs: `ppo_cnn.toml`, `ppo_large_cnn.toml`; every shipped training TOML has a `[model]` section.
- Tests: `tests/test_models.py` (interface, options, both architectures train, conv body shape and shift equivariance).
- First measurement (automation science 1/s, 20x20, CPU, 4 envs, one seed, 4 evaluation episodes, so only a hint): mlp 0.71 / 0.89 of the expert at 80k / 120k steps, cnn 0.92 / 1.00. The cnn is much slower per step on CPU (about 17 min for 120k steps against 1 min), so use `device = "cuda"` for it.
- Check: `pytest tests/test_models.py`; run `ppo_cnn.toml` and `ppo.toml` on the same task and compare `eval_vs_expert` in MLflow.
 reachability (`cut_off`)

Why: `enclosed` only looks at the tiles right beside a machine; a free tile inside a sealed pocket is of no use to an inserter either.

- Rust: new module `crates/fsim-env/src/access.rs` (flood fill of the free space from the grid edge; `Access::{Open, CutOff, Enclosed}`). The old `enclosed` code moved there from `shape.rs`, which is again only about same-kind machines (`adjacency`, `line`, `stray`). `Report::cut_off_ratio` is new.
- Python: reward term `cut_off` (-2.0) in `scoring.py`; `cut_off_ratio` joins the observation's task vector; `REWARDS.md` has the definition.
- Check: `cargo test -p fsim-env access`; the pocket case is `free_tiles_sealed_off_from_the_edge_do_not_count`.

## Stage 4: producer-consumer distance (`link`)

Why: the shape terms only relate machines of the *same* recipe. A blueprint works when each machine is near the machines that feed it; the distance is a proxy for belt length (wirelength in chip placement).

- Rust: new module `crates/fsim-env/src/links.rs` (`Chain`: which recipes of the task feed which, built once per task inside `Bounds`; `link_ratio`). `PlacedView::gap` and `far_tiles` moved to `report.rs` so `shape.rs` and `links.rs` share them. `Report::link_ratio` is new.
- Python: reward term `link` (-0.5); `link_ratio` joins the observation; `REWARDS.md` has the definition.
- Check: `cargo test -p fsim-env links`; `pytest tests/test_production_science.py -k link`.

## Stage 5: geometry gates the output (`require_access`)

Why: the reward terms punish unreachable machines but the output still counted them, so a blueprint that cannot work could score high.

- Rust: `Task::require_access` (default off). In `Engine::report` the steady state is solved for the reachable machines only (`access.rs`, `Access::Open`); the others count as idle.
- Python: `Task.require_access`, top-level `require_access` key in a task TOML (a commented example in `production_science_target_output.toml`); `REWARDS.md` section "Geometry that gates the output".
- Check: `cargo test -p fsim-env machines_nobody_can_reach`; `pytest tests/test_engine.py -k require_access`.

## Stage 6: ready-made CNN curricula

Why: so a run needs no editing: three phases per task, each continuing the previous one.

- `train.py`: `init_from = "latest:<training name>"` resolves to the `best.zip` of the newest finished run of that training (`resolve_init_from`); a plain path still works.
- Configs: `configs/training/cnn_{automation,production}_science_p{1,2,3}.toml` (CNN, `device = "cuda"`, phase 1 without shape and `link`, phase 2 all terms, phase 3 the same on the `..._access` task); tasks `configs/tasks/{automation,production}_science_target_output_access.toml` (`require_access = true`).
- Scripts: `scripts/train_cnn_automation_science.sh`, `scripts/train_cnn_production_science.sh`.
- Tests: `tests/test_train.py` (`latest:` picks the newest run that has a model). The configs and scripts themselves are not tested: tests cover module code only.
- Check: each phase file was run with tiny step counts on CPU in the order p1 -> p2 -> p3 (production: p1 -> p2).

## Stage 7: a line must be straight

Why: a trained agent built a "T" (a row of machines with a column through its middle) and was paid the line reward in both directions, which pushes towards mazes of machines.

- Rust (`shape.rs`): a *cluster* (twins joined by full contact) is a line only if it is one straight run of at least three; a bend, a branch, a domino or a block disqualifies the whole cluster. Lines no longer share machines, so the union-find in `units` is gone (a unit is a line or a single machine).
- Tests: `shape.rs` `a_bend_a_branch_or_a_second_row_takes_the_line_away`; `tests/test_engine.py` `test_a_line_that_grows_a_branch_stops_being_a_line`.
- `adjacency` follows the same rule (a snake of 8 machines decoded from a trained agent's blueprint still earned 0.85 of it): only straight clusters earn contacts (`machines - 1` per cluster), a bend, branch or block earns none. `straight_clusters` is the one place that finds clusters; `line` is the straight clusters of three or more.
- Check: `cargo test -p fsim-env shape`.

## Stage 8: one script, phases chosen without editing

- `scripts/train_cnn.sh <task>` is the one script; `train_cnn_<task>.sh` are two-line wrappers. `PHASES="2 3"` runs some phases; unknown phases are refused before anything trains; a failing phase stops the rest.
- `configs/training/cnn_production_science_access.toml`: phase 3 from scratch, for when phases 1 and 2 were not run.
- Tests cover module code only (`src/factorio_rl`, the Rust crates): the tests of scripts and of shipped config files were removed, and the remaining tests build their tasks inline.

## Where things stand

- Gate after stage 5: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `pytest` (58 passed) all green. Environment speed (`benchmarks/step_speed.py`): 11.7k / 12.8k / 6.4k steps/s (green circuit / LDS / production science), a little below the 14k / 15k / 7.5k of the earlier stages: more terms per step.
- All new Rust and Python code was built and tested in a copy of the repo, not in the working tree's virtualenv: run `maturin develop --uv --release` there before the next training.
- Ideas not done: an event-based bonus (a fixed reward for the machine that joins a twin, instead of a mean over all machines); curriculum over the task ranges.
