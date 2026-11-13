# Context: where the project stands

A handoff file: read it to rebuild the picture of the project, what was done and why, what is left,
and the rules the maintainer wants followed. Details live in the files it points to; this one is the
map. (Last updated at the end of the reward-geometry work, see `factorio-rl-py/WORKLOG.md` stages 1-8.)

## What the project is

`factorio-hrl`: Rust tooling for Factorio blueprints (decode, model, render, generate) that is turning
into a fast factory simulator and an RL / genetic-search framework. Today it solves a restricted
task: place crafting machines (with recipes) on a grid so that a target item is produced at the best
or at a required rate. No belts or inserters yet; `SPEC.md` has the roadmap.

## Standing rules (from the maintainer)

- All code, comments, docs, CLI and error messages are in **English** (the maintainer talks Polish in chat).
- Prefer proven open-source crates / libraries over hand-rolled code; high Rust standard, DRY, KISS,
  one concept per module. The maintainer extends the code by hand, so write for the next reader.
- **Never commit.** The maintainer commits. Say when a step is finished.
- After each step give copy-paste commands to inspect the new functionality.
- Docs and README stay generic (vanilla Factorio); the game files in `data/` are the maintainer's setup.
- **Tests cover module code only** (`src/factorio_rl`, the Rust crates). No tests for `scripts/`,
  `configs/` or docs, and tests do not read shipped files: they run from `src/` and `tests/` alone (mock configs and the
  data excerpt live in `tests/fixtures/`, shared tasks in `tests/specs.py`). Also in `CLAUDE.md`.
- Do not rebuild the extension (`maturin develop`) while a training runs: the compiled
  `src/factorio_rl/_native.abi3.so` lives in the source tree the running process uses. Develop and test in a
  copy of the repo (rsync without `target`, `.venv`, `runs`, `live`, `*.so`) with its own virtualenv.
- Stage 0.0.2 (modules, beacons, quality) is deferred and written into `SPEC.md`.

## Layout of the repository

| path | what |
|---|---|
| `crates/fsim-blueprint` | blueprint decode / encode and the typed model (`Design`, entities) |
| `crates/fsim-data` | typed views of the `data-raw-dump.json` (recipes, machines, geometry); `Memo` caches parsed prototypes |
| `crates/fsim-render` | PNG renderer (sprites from the game directory or schematic boxes), golden tests |
| `crates/fsim-gen` | random blueprint generator |
| `crates/fsim-calc` | naive balance and the LP steady state (`good_lp` + `microlp`): crafts per group, item balances, supply limits, optional cap on the target |
| `crates/fsim-env` | the machine-layout task: `Task`, `Engine` (validate, evaluate, legal actions), `Episode` (incremental, used by the RL environment), and the layout measures below |
| `crates/fsim-py` | PyO3 bindings (`factorio_rl._native`), JSON strings and bytes across the boundary |
| `crates/fsim-cli` | `fsim` command: gen, render, decode, calc, inspect |
| `factorio-rl-py/` | the Python side: Gymnasium environment, training, viewer, configs, docs |

`fsim-env` modules: `engine.rs` (report), `episode.rs`, `shape.rs` (same-recipe machines: adjacency, line,
stray), `access.rs` (enclosed, cut off), `links.rs` (producer-consumer distance), `report.rs`, `task.rs`.
Each crate may have a `NOTES.md`; `crates/fsim-env/NOTES.md` holds the speed table and the performance
ideas not done yet.

`factorio-rl-py/src/factorio_rl`: `engine.py` (Engine, Episode wrappers), `env.py` (`LayoutEnv`), `scoring.py`
(reward terms), `config.py` (TOML task and training configs), `models/` (architectures), `train.py`,
`baselines.py` (brute-force expert, genetic search), `viewer.py` (live page), `tracking.py` (MLflow).

Docs in `factorio-rl-py/`: `README.md` (how to run), `REWARDS.md` (every reward term), `TUNING.md` (what to
set in a training TOML), `WORKLOG.md` (the stages, with files and checks).

## How it works

- **Task** (`Task`): grid size, supply per item (a rate, or unlimited), allowed recipes and machines, the
  target item, optionally `required_output` and `require_access`. Modes in a task TOML: `supply_limited`,
  `grid_limited` (inputs unlimited, maximise), `target_output` (inputs unlimited, make exactly this rate).
- **Output** is the LP steady state of the whole chain (the scarcest stage caps everything downstream);
  with `required_output` the target is capped there, so extra machines idle.
- **Score** = sum of weighted reward terms (`scoring.py`): `output +1`, `waste -2`, `adjacency +0.2`, `line +0.3`,
  `stray -0.1`, `enclosed -2`, `cut_off -2`, `link -0.5`. The reward of a step is the change of the score, so the
  episode return equals the final score. Weights are overridden per run in `[reward]` of the training TOML.
  New terms: one registered function (see `REWARDS.md`).
- **Environment**: action `Discrete(K * H * W + 1)` (kind, y, x, last = stop), `action_masks()` for
  `MaskablePPO`; observation Dict `grid` (kinds+1 planes) and `task` vector (grid size, supplies, ceiling, the ratios,
  per kind count and utilisation). Any change of the observation or the reward makes old models useless.
- **Training**: `python -m factorio_rl.train --task <task.toml> --training <training.toml> --dump <dump> [--live]`.
  Runs go to `runs/<training>__<task>__<time>/`; tqdm bar (one tick per evaluation, same as a live refresh);
  MLflow in `runs/mlflow.db` (`mlflow ui --backend-store-uri sqlite:///runs/mlflow.db`).
  Stop on `target_vs_expert` or `patience`. `[ppo] init_from = "latest:<training name>"` continues a run.
- **Architectures** (`[model] architecture`): `mlp` (flattened grid) or `cnn` (fully convolutional, one logit per
  kind and tile). New architecture: drop `models/<name>.py` with an `ARCHITECTURE`.
- **Algorithms**: `maskable_ppo` and `genetic`.

## Design decisions worth remembering

- A term must be big enough to learn from: a new line of three was +0.0035 on a score of 0.83 and was never
  learned, hence the larger shape weights. A machine added only for its shape reward must cost more than it earns
  (`waste` -2 for that reason).
- `waste` and `enclosed`/`cut_off`/`link` are measured against the machines that fit on the grid, not the
  machines built, so the first machine of a half-built chain is not "100% waste" and extra machines do not dilute them.
- `adjacency` and `line` count only *straight clusters* of twins (full-side contact, one row or column): a bend,
  branch, block or snake earns nothing, so the agent cannot grow a maze and collect the reward.
- `stray` joins the units of a recipe (a line counts as one) with a minimum spanning tree, so starting a line
  away from the rest costs one link, never one per machine.
- `enclosed`: no free tile beside a machine (grid edge is a wall). `cut_off`: free tiles only in pockets sealed
  from the grid edge. `require_access` (task rule): such machines produce nothing and count as idle.
- Two-phase training: first the goal without shape terms, then all terms, then `require_access`.
- GPU: on production science the CNN/MLP training is 3-6x faster on GPU; the environment itself does
  about 6-13k steps/s per thread (`factorio-rl-py/benchmarks/step_speed.py`).
- Measured once (one seed, small task, CPU): the CNN reached the expert's score in about 120k steps, the MLP
  in 150k+. Only a hint.

## Configs and scripts

- Tasks (`configs/tasks`): green circuit, low density structure, production science, each with `_grid_limited`
  or `_target_output` variants; `automation_science_target_output` (1 pack/s, 20x20, assembler-1) and the
  `*_target_output_access` variants (`require_access = true`).
- Training (`configs/training`): `ppo`, `ppo_large`, `genetic`, `ppo_cnn`, `ppo_large_cnn`, the phased
  `ppo_large_phase1/2`, and the CNN curricula `cnn_{automation,production}_science_p1/p2/p3` plus the standalone
  `cnn_production_science_access` (phase 3 from scratch).
- Scripts (`scripts/`, helpers, untested on purpose): `train_cnn.sh <task>` with `PHASES="2 3"`, and wrappers
  `train_cnn_automation_science.sh`, `train_cnn_production_science.sh`.

```bash
cd factorio-rl-py && source .venv/bin/activate
maturin develop --uv --release && pytest -q                  # build the extension, run the Python tests
cargo test --workspace                                        # from the repo root
scripts/train_cnn_automation_science.sh --live                # three phases
python -m factorio_rl.train --task configs/tasks/production_science_target_output_access.toml \
  --training configs/training/cnn_production_science_access.toml --dump ../data/data-raw-dump.json --live
```

`gio: ... Operation not supported` when the viewer starts is WSL failing to open a browser; open
http://localhost:8000 by hand.

## State at the time of writing

- Gate: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`
  and `pytest` (58 tests) were green. Nothing is committed.
- The maintainer was running production science trainings with the CNN configs; their results are not known
  here. They tuned `cnn_production_science_p3.toml` (`n_epochs = 20`, `line = 0.1`).
- A trained agent built a snake / T shape; the straight-cluster rule above was the answer. Not yet checked on a
  new run whether `eval_term_line` now rises and the snake is gone.

## What is left

- **Check the training results**: after phase 2 look at `eval_term_*` in MLflow (is `line` rising, is
  `enclosed` shrinking, is `output` at 1); compare `mlp` and `cnn` on the same task and seed; tune weights
  (`waste`, `link`, `adjacency`, `line`) from those curves. Weights and step counts in the shipped configs are
  estimates, not tuned values.
- **Ideas not done**: an event-based bonus (a fixed reward for the machine that joins a twin, instead of a
  mean over all machines); `MIN_FREE_TILES = 2` (separate access for input and output); a curriculum over the
  task ranges; capping `adjacency` further if mazes persist.
- **Performance** (`crates/fsim-env/NOTES.md`): genetic search decode with an `Episode` and `rayon`; build the
  LP once and memoise results by machine counts; several environments in one process with the GIL released.
- **Roadmap** (`SPEC.md`): 0.0.2 modules, beacons, quality in the calculator; 0.1.0 connection extraction
  (belts, inserters) from the grid, the step that makes the blueprints actually buildable; then the later
  stages listed there. The reward terms above (`enclosed`, `cut_off`, `link`, `require_access`) are
  stand-ins for logistics until belts and inserters exist.
