# Choosing training settings

A practical guide to the `[ppo]`, `[evaluation]`, `[stop]` and `device` keys of
`configs/training/*.toml`. The values are starting points based on common PPO practice and on the
measurements in this repo; the only way to know what suits a task is to compare runs in MLflow
(`mlflow ui --backend-store-uri sqlite:///runs/mlflow.db`).

## How an episode looks here

One episode places machines one at a time, at most `tiles / smallest_footprint + 5` of them
(about 70 steps on 24x24, about 120 on 32x32 with 3x3 machines). The reward of a step is the
change of `scoring.score`, so the episode return equals the final score (at most `scoring.ideal_score()`, 1.5 with the default weights). Rewards
are small and dense, which suits PPO without reward scaling.

## Settings, in the order to think about them

| key | what it does | sensible range | notes |
|---|---|---|---|
| `n_envs` | episodes played in parallel | 8-16 (up to your CPU cores) | More means a bigger, less noisy rollout and more varied tasks per update. |
| `n_steps` | steps per environment per rollout | 256-1024 | Keep it a few times the episode length (120) so every rollout holds whole episodes. |
| rollout | `n_envs * n_steps` | 4 096-16 384 | The experience gathered before each learning phase. |
| `batch_size` | minibatch for one gradient step | rollout / 4 to rollout / 16 | Gradient steps per rollout = `rollout / batch_size * n_epochs`. A batch equal to the rollout leaves only `n_epochs` steps, which learns slowly. |
| `n_epochs` | passes over each rollout | 4-10 | More passes squeeze more from the data but push the policy away from the one that collected it (PPO clips this). Lower it if scores jump around. |
| `learning_rate` | step size | 1e-4 to 1e-3 | 3e-4 is the usual default. Larger batches tolerate (and often need) a larger rate. Too high: scores collapse after a good phase. |
| `gamma` | discount | 0.99-1.0 | The return is the final score, so long horizons matter: use 0.995-1.0. Do not go below 0.99. |
| `ent_coef` | bonus for a random policy | 0.003-0.03 | Keeps exploration alive. The action space is large (kinds x tiles), so too little entropy makes the policy stick to a bad layout early. Too much and it never commits. |
| `total_timesteps` | upper bound | see below | The stop conditions normally end the run earlier. |
| `device` | `cpu` or `cuda` | | Measured on production science: GPU is 3-6x faster than 16 CPU cores, because the network is large (12k inputs, 11k outputs). For small tasks measure with `benchmarks/ppo_speed.py --task <file>`. |

Not exposed yet (SB3 defaults apply): GAE lambda (0.95), clip range (0.2), network size
(`[64, 64]`), learning-rate schedule. Add them to `PPOConfig` when a run needs them.

## Network architecture

`[model] architecture` picks the network (`mlp` or `cnn`, see `src/factorio_rl/models/`); the rest
of the file is the same for both. The `cnn` sees which tiles are neighbours, which matters for
geometry (lines, access for inserters); in a first measurement on a small task it reached the
expert's score in about 120k steps against 150k+ for the `mlp`. It is much slower per step on a CPU,
so give it `device = "cuda"`. Compare two architectures on the same task and seed in MLflow.

## Evaluation and stopping

- `evaluation.every`: 5-10 rollouts' worth of steps (e.g. 50 000 with a 8 192 rollout). Each
  evaluation costs `episodes` greedy episodes and is one tqdm tick and one live-viewer refresh.
- `evaluation.episodes`: 10-20. The tasks are a fixed sample, so the curve is comparable between
  evaluations, but few episodes mean a noisy mean. Do not judge a change by one evaluation.
- `stop.patience`: 10-20 evaluations, because the evaluation mean is noisy and learning has
  plateaus before it improves again. Too small a value stops runs that were about to improve.
- `stop.target_vs_expert`: only meaningful when the expert applies (two kinds of equal
  footprint). Otherwise the reference is the ideal score and integer machine counts keep the
  agent below it, so aim for 0.8-0.9 and let `patience` decide.

## How much to train

There is no formula; use these as orders of magnitude and watch the curve.

- Green circuit, 2 kinds: 100k-500k steps.
- Low-density structure, 5 kinds: 1M-3M.
- Production science, 11 kinds, 32x32: several million. A 30k-step smoke test only reached 30%
  of the ideal.

Decide by `eval_vs_expert` in MLflow: still rising means keep going, flat for `patience`
evaluations means the settings, not the time, are the limit.

## A tuning routine

1. Start from `ppo.toml` (small task) or `ppo_large.toml` (big task); set `device`.
2. Run 3 seeds of the same config (`seed` in the file) before drawing conclusions: RL runs
   differ a lot between seeds.
3. Change one thing at a time. Usual order: `ent_coef`, then `learning_rate`, then
   `n_envs` / `batch_size`, then `gamma`.
4. Read the curves in MLflow:
   - `train_episode_return` rises but `eval_mean_score` does not: the agent overfits the sampled
     tasks, or the greedy policy differs from the sampling one. Raise `ent_coef` or the task
     variety.
   - Both rise, then collapse: lower `learning_rate` or `n_epochs`.
   - Flat from the start: raise `ent_coef` and `n_envs`; check the shape weights and the
     waste penalty are not drowning the output term (`[reward]`, see `REWARDS.md`; the `eval_term_*`
     metrics show each term's contribution).
5. Too hard from scratch? Narrow the task ranges (a fixed grid and a small supply range), learn
   there, then widen the ranges in the task file.

## Genetic search

`population` and `generations` trade run time for quality: `population * generations` layouts
are evaluated. 60 x 200 is a quick baseline; raise `population` first when the best score
plateaus early. It uses no GPU and does not need `[ppo]`.
