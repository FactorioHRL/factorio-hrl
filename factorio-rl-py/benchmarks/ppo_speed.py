"""Training timesteps per second of MaskablePPO on CPU and on CUDA.

    python benchmarks/ppo_speed.py --dump ../data/data-raw-dump.json [--task production_science.toml]

Same seed and settings for each device; evaluation, tracking and the viewer are off, so this is
the cost of collecting experience plus the PPO updates.
"""

from __future__ import annotations

import argparse
import time
from pathlib import Path

import torch
from sb3_contrib import MaskablePPO
from stable_baselines3.common.env_util import make_vec_env

from factorio_rl import Engine, LayoutEnv, load_task_spec

TASKS = Path(__file__).parent.parent / "configs" / "tasks"


def run(engine: Engine, task_file: str, device: str, n_envs: int, batch_size: int, timesteps: int) -> float:
    spec = load_task_spec(TASKS / task_file)
    env = make_vec_env(LayoutEnv, n_envs=n_envs, seed=0, env_kwargs={"engine": engine, "spec": spec})
    model = MaskablePPO(
        "MultiInputPolicy", env, n_steps=512, batch_size=batch_size, n_epochs=10, seed=0, device=device, verbose=0
    )
    model.learn(n_envs * 512)  # warm-up: allocations, CUDA context
    start = time.perf_counter()
    model.learn(timesteps, reset_num_timesteps=False)
    if device == "cuda":
        torch.cuda.synchronize()
    return timesteps / (time.perf_counter() - start)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--dump", default=None)
    parser.add_argument("--task", default="production_science.toml")
    parser.add_argument("--timesteps", type=int, default=16_384)
    args = parser.parse_args()
    engine = Engine(args.dump)
    print(f"{args.task}, {args.timesteps} timesteps after a warm-up")
    for n_envs, batch_size in [(8, 512), (8, 2048), (16, 4096)]:
        row = {d: run(engine, args.task, d, n_envs, batch_size, args.timesteps) for d in ("cpu", "cuda")}
        print(f"n_envs {n_envs:>2}, batch {batch_size:>4}:  cpu {row['cpu']:>6.0f} steps/s   cuda {row['cuda']:>6.0f} steps/s   (cuda/cpu {row['cuda'] / row['cpu']:.2f}x)")
