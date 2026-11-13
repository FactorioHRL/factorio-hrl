# factorio-rl

Learn machine layouts for Factorio with RL or genetic search. The Rust engine (crates `fsim-calc`, `fsim-env`, `fsim-py`) validates a layout and computes its steady-state output; this package wraps it in a Gymnasium environment.

## The task

Given a grid `W×H`, the inputs arriving per second, the allowed recipes and machines (nothing else: no belts, no inserters), place machines so that the output of the target item is high **and no machine is wasted**. Example: 10 iron/s and 15 copper/s on 30×30 → 15 cable and 10 circuit assemblers (assembling-machine-1).

Two modes (`mode` in the task config):

- **`supply_limited`**: the inputs are limited; use them fully, no more machines than needed.
- **`grid_limited`**: every input is `"unlimited"`; make as much as the grid allows. The report's `flows` says what the layout draws per second (the input it would need).

Logistics is ignored (every machine reaches every item), see `SPEC.md`, section 8.

## Setup

Needs a vanilla `data-raw-dump.json` (see the root `README.md`). Tests use a small excerpt in `tests/fixtures`, so they need no game data.

```
cd factorio-rl-py
uv venv --python 3.12 && source .venv/bin/activate
uv pip install maturin numpy gymnasium pytest   # Python >= 3.11
maturin develop --uv --release      # builds the Rust extension into the venv
pytest
```

Training needs more: `uv pip install stable-baselines3 sb3-contrib` (pulls PyTorch).

## Use

```python
from factorio_rl import Engine, green_circuit_task, Placement

engine = Engine("../data/data-raw-dump.json")          # or $FSIM_DUMP
task = green_circuit_task(30, 30, iron=10, copper=15)

recipes = ["copper-cable"] * 15 + ["electronic-circuit"] * 10
layout = [Placement("assembling-machine-1", r, (i % 10) * 3, (i // 10) * 3) for i, r in enumerate(recipes)]
report = engine.evaluate(task, layout)                  # dict: valid, violations, output_per_second, ...
print(report["output_per_second"], report["output_ratio"], report["waste_ratio"])
print(engine.export_blueprint(layout))                  # paste into the game
print(engine.analyze_blueprint(task, "0eNq..."))        # validate any blueprint string against the task
print(report["flows"])                                  # net items/s: negative = drawn from the supply
```

- `Engine`: `kinds`, `evaluate`, `legal_actions`, `analyze_blueprint`, `export_blueprint`; `decode_blueprint(str)` gives the full blueprint JSON.
- `LayoutEnv(engine, task_spec)`: Gymnasium environment (see its docstring for the action, observation and reward). `action_masks()` marks legal actions.
- `scoring.score`: a weighted sum of reward terms (output, idle machines, and how machines doing the same thing sit: touching, in lines, strays); the environment's episode return equals the final score. `REWARDS.md` lists the terms and how to add your own; weights go in the `[reward]` section of the training config.
- `baselines.expert_layout`: brute-force best layout (the score to beat). `baselines.genetic_search`: a small GA template.

## Configure and train

A run is two TOML files: a **task** (what to build) and a **training** (how to learn).

```toml
# configs/tasks/low_density_structure.toml
name = "low-density-structure"
mode = "supply_limited"                 # or "grid_limited"
target = "low-density-structure"
recipes = ["low-density-structure", "plastic-bar"]
machines = ["assembling-machine-2", "chemical-plant"]   # what the agent may build
[grid]                                  # sampled per episode (or: width = 20, height = 20)
min = 8
max = 20
[supply]                                # items/s: a number, { min, max } or "unlimited"
copper-plate = { min = 1, max = 15 }
steel-plate = { min = 1, max = 15 }
coal = { min = 1, max = 15 }
petroleum-gas = "unlimited"
```

```toml
# configs/training/ppo.toml (excerpt)
algorithm = "maskable_ppo"              # or "genetic"
device = "cpu"
[ppo]
total_timesteps = 500_000               # upper bound
n_envs = 4
n_epochs = 10                           # optimisation epochs per batch
learning_rate = 3e-4
[evaluation]
every = 20_000                          # timesteps between evaluations on fixed tasks
episodes = 20
[stop]
target_vs_expert = 0.98                 # stop at this share of the brute-force expert's score ...
patience = 8                            # ... or after this many evaluations without improvement
[live]
enabled = false                         # browser view, see below
```

Shipped tasks:

- `green_circuit`, `green_circuit_grid_limited`
- `low_density_structure`, `low_density_structure_grid_limited`: plastic is made on the grid in chemical plants.
- `production_science`, `production_science_grid_limited`: purple science from the raw inputs of the [wiki](https://wiki.factorio.com/Production_science_pack) (iron and copper plates, coal, stone, petroleum gas). The agent builds the whole chain: cable, circuits, advanced circuits, plastic, steel and stone brick in electric furnaces, iron sticks, rails, electric furnaces, productivity modules (11 kinds of machine, grids 24–32). A test checks that the raw cost per pack equals the wiki's (52.5 iron, 19.2 copper, 3.3 coal, 11.7 stone).

Shipped trainings: `ppo`, `ppo_large` (for big tasks like production science), `genetic`.

The brute-force expert covers tasks with two kinds of machine of equal footprint. For bigger tasks the reference score is the ideal `scoring.ideal_score()` (1.5 with the default weights), so `target_vs_expert` is then a share of that ideal.

```
python -m factorio_rl.train --task configs/tasks/low_density_structure.toml \
    --training configs/training/ppo.toml --dump ../data/data-raw-dump.json
```

Every evaluation compares the agent with the expert on fixed tasks, saves `best.zip` when the mean score improves, and stops training when a stop condition holds. Everything lands in `runs/<training>__<task>__<time>/`: copies of both configs, `best.zip`, `eval.jsonl`, and for the genetic algorithm `best_blueprint.txt` and `best_report.json`. A 60k-step PPO run on 4 CPU environments takes about half a minute.

## Network architectures

The network is chosen in the training config, and every architecture runs on the same TOMLs:

```toml
[model]
architecture = "cnn"            # "mlp" (default) or "cnn"
[model.options]                 # only what the architecture takes; its defaults when absent
channels = [32, 64, 64, 64]
```

`configs/training/ppo_cnn.toml` and `ppo_large_cnn.toml` are `ppo.toml` and `ppo_large.toml` with the
convolutional network. The architectures live in `src/factorio_rl/models/`, one module each; the
module's name is the name in the TOML. To add one, drop `models/<name>.py` with a module-level
`ARCHITECTURE = Architecture(policy=..., options=<dataclass with defaults>, policy_kwargs=<function>)`
(`mlp.py` is the smallest example, `cnn.py` a custom policy): the training loop, evaluation, MLflow and
the live viewer need no change. See `models/__init__.py` for the interface and `WORKLOG.md` for what
was built and when.

## Ready-made CNN curricula

For the two required-output tasks there is a full set of CNN configs, three phases each, and a script
that runs them one after the other (each phase continues the best model of the previous one by itself,
`init_from = "latest:<name of the previous training>"`):

| task | script | phase 1 | phase 2 | phase 3 |
|---|---|---|---|---|
| automation science, 20x20, 1 pack/s | `scripts/train_cnn_automation_science.sh` | `cnn_automation_science_p1` | `_p2` | `_p3` |
| production science, 32x32, 0.3 packs/s | `scripts/train_cnn_production_science.sh` | `cnn_production_science_p1` | `_p2` | `_p3` |

1. **p1**: only the goal (output, waste, walled-in and cut-off machines); shape and `link` terms off.
2. **p2**: every reward term on (see `REWARDS.md`); lower learning rate and entropy.
3. **p3**: p2's rewards on the `..._access` task, where machines nothing can reach make nothing.

```bash
source .venv/bin/activate
scripts/train_cnn_automation_science.sh --live       # --live and other arguments go to every phase
PHASES="2 3" scripts/train_cnn_production_science.sh # only some phases, without editing the script
```

The scripts are thin wrappers of `scripts/train_cnn.sh <task>`. To train phase 3 without phases 1 and 2, use
the standalone `cnn_production_science_access.toml` (starts from scratch).

All phases use `device = "cuda"`. A phase ends when its stop conditions hold, so the next one continues
from the best model so far. Phases can also be run one by one with `python -m factorio_rl.train`
(the script shows the exact commands); a later phase fails with a clear message if the earlier one
has not produced a `best.zip` yet.

## A required output instead of a supply limit

`mode = "target_output"` (see `configs/tasks/green_circuit_target_output.toml`, `automation_science_target_output.toml` and `production_science_target_output.toml`) asks for one concrete output: inputs are unlimited (the report still says what the layout draws) and `[output] per_second = 4` is the rate to produce, a number or `{ min, max }` sampled per episode. The layout is rated against exactly that rate: more output does not count, and machines beyond what it needs idle and are penalised as waste.

## Choosing settings

`TUNING.md` explains the `[ppo]`, `[evaluation]` and `[stop]` keys, sensible ranges and a tuning routine.

## Speed

`python benchmarks/step_speed.py --dump <data-raw-dump.json>` prints environment steps per second under random legal play. The environment builds layouts through `Engine.start(task)` (an `Episode`): the layout stays in Rust, a step sends one machine, and the legal-action mask and occupancy planes are updated incrementally. See `crates/fsim-env/NOTES.md` for the numbers and the ideas not done yet.

## Progress and tracking

Stdout is one tqdm bar. One tick is one evaluation (PPO) or one generation (genetic), the same moment the live page refreshes, and the postfix shows the mean score, the share of the reference, the best score so far and the evaluations without improvement.

MLflow records the runs locally (`[tracking]` in the training config; `enabled = false` turns it off). It stores little on purpose: all parameters once, a few metrics per evaluation (`eval_mean_score`, `eval_vs_expert`, `eval_output_ratio`, `eval_term_<reward term>`, `train_episode_return`), and the config copies and the best blueprint at the end. Nothing is logged per training step. The best model (10-30 MB) stays in the run directory; set `log_model = true` to copy it into MLflow as well. Browse with:

```bash
mlflow ui --backend-store-uri sqlite:///runs/mlflow.db
```

## Watch it live

A local web page shows a layout drawn by the Rust renderer (real sprites when `game_dir` points at the game graphics, e.g. `../data/game`; schematic boxes otherwise), the grid as text, the status with the inputs the layout draws, the evaluation history against the expert's score, and the blueprint string to paste into the game. Set `[live] enabled = true` in the training config, or pass `--live`; it opens at http://localhost:8000 and shows the first evaluation task at every evaluation (PPO) or the best layout per generation (genetic).

```
python -m factorio_rl.train --task configs/tasks/green_circuit.toml --training configs/training/ppo.toml --live
```

From your own code: `Viewer(engine, game_dir=...).start()` and `viewer.publish(task, layout, label, step, score, report)`. `engine.render_png(layout, game_dir)` returns the PNG alone. Only the machines' bounding box is drawn, so the picture size changes as machines are added.

## Tests

`pytest` covers the engine (the 15/10 example, violations, action mask, blueprint round trip), the environment (`check_env`, masked random play, return = final score), the configs (every shipped file loads, mode checks), the low-density-structure task (plastic chain, grid-limited input report) and the viewer.
