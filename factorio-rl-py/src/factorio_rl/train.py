"""Run a training from two config files (see ``configs/``):

    python -m factorio_rl.train --task configs/tasks/low_density_structure.toml \\
        --training configs/training/ppo.toml --dump ../data/data-raw-dump.json [--live]

The task file says what to build (inputs, grid, recipes, allowed machines), the training file how
to learn (algorithm, timesteps, epochs, evaluation, stop conditions). Everything a run produces,
including copies of both configs, goes to ``<out_dir>/<training>__<task>__<time>/``.
"""

from __future__ import annotations

import argparse
import dataclasses
import json
import shutil
import time
from pathlib import Path

import numpy as np
from stable_baselines3.common.utils import safe_mean
from tqdm import tqdm

from .baselines import genetic_search, reference_score
from .config import TaskSpec, TrainingConfig, load_task_spec, load_training
from .engine import Engine
from .env import LayoutEnv
from .models import policy_for
from .scoring import breakdown, resolve_weights, score
from .tracking import Tracker
from .viewer import Viewer


def new_run_dir(training: TrainingConfig, spec: TaskSpec, task_path: Path, training_path: Path) -> Path:
    run = Path(training.out_dir) / f"{training.name}__{spec.name}__{time.strftime('%Y%m%d-%H%M%S')}"
    run.mkdir(parents=True)
    shutil.copy(task_path, run / "task.toml")
    shutil.copy(training_path, run / "training.toml")
    return run


def resolve_init_from(init_from: str, out_dir: str) -> str:
    """``latest:<training name>`` is the ``best.zip`` of the newest run of that training (so a phase can
    continue the previous one without anyone copying a path); anything else is taken as a path."""
    prefix = "latest:"
    if not init_from.startswith(prefix):
        return init_from
    name = init_from.removeprefix(prefix)
    # run directories are named <training>__<task>__<time>; the time sorts chronologically
    finished = sorted(Path(out_dir).glob(f"{name}__*/best.zip"), key=lambda path: path.parent.name.rsplit("__", 1)[-1])
    if not finished:
        raise FileNotFoundError(f"no finished run of training {name!r} (with a best.zip) under {out_dir!r}: run that phase first")
    print(f"continuing from {finished[-1]}")
    return str(finished[-1])


def make_viewer(training: TrainingConfig, engine: Engine) -> Viewer | None:
    if not training.live.enabled:
        return None
    viewer = Viewer(engine, port=training.live.port, game_dir=training.live.game_dir or None, weights=resolve_weights(training.reward))
    viewer.start()
    return viewer


def play(model, env: LayoutEnv, task) -> dict:
    """One greedy episode of ``model`` on ``task``; returns the final info dict."""
    obs, _ = env.reset(options={"task": task})
    done = False
    while not done:
        action, _ = model.predict(obs, action_masks=env.action_masks(), deterministic=True)
        obs, _, terminated, truncated, info = env.step(int(action))
        done = terminated or truncated
    return info


def make_eval_callback(
    engine: Engine, spec: TaskSpec, training: TrainingConfig, run: Path, viewer: Viewer | None, tracker: Tracker
):
    """Evaluates on fixed tasks against the reference score, saves the best model and applies the
    stop conditions (``target_vs_expert``, ``patience``). One evaluation = one progress-bar tick =
    one refresh of the live viewer = one point in MLflow."""
    from stable_baselines3.common.callbacks import BaseCallback

    cfg, stop = training.evaluation, training.stop
    tasks = [spec.sample(np.random.default_rng(training.seed + 1 + i)) for i in range(cfg.episodes)]
    weights = resolve_weights(training.reward)
    expert_scores = [reference_score(engine, t, weights) for t in tqdm(tasks, desc="reference scores", leave=False)]
    expert_mean = float(np.mean(expert_scores))
    env = LayoutEnv(engine, spec, reward=training.reward)
    evaluations = -(-training.ppo.total_timesteps // cfg.every)

    class EvalAndStop(BaseCallback):
        def __init__(self) -> None:
            super().__init__()
            self.next_eval = cfg.every
            self.best = -np.inf
            self.stale = 0
            self.bar: tqdm | None = None

        def _on_training_start(self) -> None:
            self.bar = tqdm(total=evaluations, desc="PPO", unit="eval")

        def _on_training_end(self) -> None:
            if self.bar:
                self.bar.close()

        def _on_step(self) -> bool:
            if self.num_timesteps < self.next_eval:
                return True
            self.next_eval += cfg.every
            infos = [play(self.model, env, t) for t in tasks]
            mean = float(np.mean([i["score"] for i in infos]))
            ratio = mean / expert_mean if expert_mean > 0 else 0.0
            metrics = {"eval_mean_score": mean, "eval_vs_expert": ratio, "eval_output_ratio": float(np.mean([i["output_ratio"] for i in infos]))}
            # what each reward term contributed on average: shows which part of the score moves
            metrics |= {f"eval_term_{name}": float(np.mean([i["reward_terms"][name] for i in infos])) for name in weights}
            if self.model.ep_info_buffer:
                metrics["train_episode_return"] = float(safe_mean([e["r"] for e in self.model.ep_info_buffer]))
            tracker.log(metrics, self.num_timesteps)
            with open(run / "eval.jsonl", "a") as f:
                f.write(json.dumps({"timesteps": self.num_timesteps, **metrics, "expert_mean": expert_mean}) + "\n")

            if viewer:
                # always the first evaluation task, so the shown scores are comparable between evaluations
                play(self.model, env, tasks[0])  # leaves the layout in `env`
                viewer.publish(tasks[0], env.layout, "PPO agent", self.num_timesteps, score(env.report, weights), env.report, expert_scores[0], mean_score=mean)

            if mean > self.best + 1e-4:
                self.best, self.stale = mean, 0
                self.model.save(run / "best")
            else:
                self.stale += 1
            self.bar.set_postfix(steps=self.num_timesteps, score=f"{mean:.3f}", vs_expert=f"{ratio:.0%}", best=f"{self.best:.3f}", stale=self.stale)
            self.bar.update(1)  # the same moment the live viewer refreshes
            if ratio >= stop.target_vs_expert:
                tqdm.write(f"stop: reached {ratio:.0%} of the reference (target {stop.target_vs_expert:.0%})")
                return False
            if self.stale >= stop.patience:
                tqdm.write(f"stop: no improvement for {self.stale} evaluations")
                return False
            return True

    return EvalAndStop()


def train_ppo(engine: Engine, spec: TaskSpec, training: TrainingConfig, run: Path, viewer: Viewer | None, tracker: Tracker) -> None:
    from sb3_contrib import MaskablePPO
    from stable_baselines3.common.env_util import make_vec_env

    p = training.ppo
    vec_env = make_vec_env(LayoutEnv, n_envs=p.n_envs, seed=training.seed, env_kwargs={"engine": engine, "spec": spec, "reward": training.reward})
    hyper = dict(
        n_steps=p.n_steps,
        batch_size=p.batch_size,
        n_epochs=p.n_epochs,
        learning_rate=p.learning_rate,
        gamma=p.gamma,
        ent_coef=p.ent_coef,
    )
    if p.init_from:  # continue an earlier run (e.g. after switching the reward weights); hyperparameters come from this config
        model = MaskablePPO.load(resolve_init_from(p.init_from, training.out_dir), env=vec_env, device=training.device, **hyper)
    else:
        policy, policy_kwargs = policy_for(training.model.architecture, training.model.options)
        model = MaskablePPO(
            policy,
            vec_env,
            policy_kwargs=policy_kwargs,
            seed=training.seed,
            device=training.device,
            verbose=0,  # progress goes to the tqdm bar and MLflow, not to stdout tables
            **hyper,
        )
    model.learn(p.total_timesteps, callback=make_eval_callback(engine, spec, training, run, viewer, tracker))
    if training.tracking.log_model:
        tracker.log_files(run / "best.zip")


def train_genetic(engine: Engine, spec: TaskSpec, training: TrainingConfig, run: Path, viewer: Viewer | None, tracker: Tracker) -> None:
    task = spec.sample(np.random.default_rng(training.seed))
    weights = resolve_weights(training.reward)
    reference = reference_score(engine, task, weights)
    g = training.genetic
    bar = tqdm(total=g.generations, desc="genetic", unit="gen")

    def show(generation: int, layout, report) -> None:
        if viewer:
            viewer.publish(task, layout, "genetic search", generation, score(report, weights), report, reference)
        tracker.log({"best_score": score(report, weights), "best_output_ratio": report["output_ratio"]} | {f"best_term_{n}": v for n, v in breakdown(report, weights).items()}, generation)
        bar.set_postfix(score=f"{score(report, weights):.3f}", reference=f"{reference:.3f}", machines=report["machines"])
        bar.update(1)  # the same moment the live viewer refreshes

    layout, report = genetic_search(engine, task, population=g.population, generations=g.generations, seed=training.seed, weights=weights, on_generation=show)
    bar.close()
    (run / "best_blueprint.txt").write_text(engine.export_blueprint(layout))
    (run / "best_report.json").write_text(json.dumps(report, indent=1))
    tracker.log_files(run / "best_blueprint.txt", run / "best_report.json")
    print(f"genetic search: score {score(report, weights):.3f} (reference {reference:.3f}), {len(layout)} machines, output {report['output_per_second']:.2f}/s")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--task", required=True, type=Path, help="task config (TOML)")
    parser.add_argument("--training", required=True, type=Path, help="training config (TOML)")
    parser.add_argument("--dump", default=None, help="data-raw-dump.json (default: $FSIM_DUMP)")
    parser.add_argument("--live", action="store_true", help="enable the live browser view whatever the config says")
    args = parser.parse_args()

    spec, training = load_task_spec(args.task), load_training(args.training)
    if args.live and not training.live.enabled:
        from dataclasses import replace

        training = replace(training, live=replace(training.live, enabled=True))
    engine = Engine(args.dump)
    run = new_run_dir(training, spec, args.task, args.training)
    print(f"run directory: {run}")
    viewer = make_viewer(training, engine)
    with Tracker(training.tracking, spec.name, run.name, {"task": spec.name, **dataclasses.asdict(training)}) as tracker:
        {"maskable_ppo": train_ppo, "genetic": train_genetic}[training.algorithm](engine, spec, training, run, viewer, tracker)
        tracker.log_files(run / "task.toml", run / "training.toml")


if __name__ == "__main__":
    main()
