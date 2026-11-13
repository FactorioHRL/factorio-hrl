from dataclasses import replace

import numpy as np
import pytest
from gymnasium.utils.env_checker import check_env

from factorio_rl import LayoutEnv, green_circuit_task, load_task_spec
from factorio_rl.config import Supply, TaskSpec
from factorio_rl.baselines import expert_layout, genetic_search
from factorio_rl.scoring import score

from .conftest import NO_SHAPE


SMALL = TaskSpec(
    name="small",
    mode="supply_limited",
    target="electronic-circuit",
    recipes=("copper-cable", "electronic-circuit"),
    machines=("assembling-machine-1",),
    width_range=(6, 12),
    height_range=(6, 12),
    supply={"iron-plate": Supply(low=1, high=20), "copper-plate": Supply(low=1, high=30)},
)


@pytest.fixture
def env(engine):
    return LayoutEnv(engine, SMALL)


def test_gymnasium_api_check(env):
    check_env(env, skip_render_check=True)


def test_masked_random_play_never_hits_an_illegal_action(env):
    rng = np.random.default_rng(0)
    for episode in range(5):
        env.reset(seed=episode)
        done = False
        while not done:
            legal = np.flatnonzero(env.action_masks())
            _, _, terminated, truncated, info = env.step(int(rng.choice(legal)))
            assert not info["illegal"]
            done = terminated or truncated
        assert info["machines"] > 0


def test_return_equals_final_score(env):
    rng = np.random.default_rng(1)
    env.reset(seed=3)
    total, done = 0.0, False
    while not done:
        _, reward, terminated, truncated, info = env.step(int(rng.choice(np.flatnonzero(env.action_masks()))))
        total, done = total + reward, terminated or truncated
    assert total == pytest.approx(info["score"])


def test_illegal_action_is_penalised_and_changes_nothing(env):
    env.reset(options={"task": green_circuit_task(6, 6, 2, 3)})
    env.step(0)  # cable machine at (0, 0)
    _, reward, *_ = env.step(0)  # the same spot again
    assert reward < 0 and len(env.layout) == 1


def test_stop_ends_the_episode(env):
    env.reset(seed=0)
    _, reward, terminated, *_ = env.step(env.stop_action)
    assert terminated and reward == 0


def test_expert_solves_the_brief_exactly(engine):
    task = green_circuit_task(12, 12, iron=2, copper=3)  # 3 cable + 2 circuit assemblers
    layout, report = expert_layout(engine, task, NO_SHAPE)  # the brief alone, without rewards for tidy rows
    assert report["output_per_second"] == pytest.approx(2.0)
    assert score(report, NO_SHAPE) == pytest.approx(1.0)
    assert len(layout) == 5


def test_genetic_search_finds_a_good_layout(engine):
    task = green_circuit_task(9, 9, iron=1, copper=1.5)  # 2 + 1... fits in 9 cells
    layout, report = genetic_search(engine, task, population=40, generations=40, seed=0)
    assert report["valid"] and score(report) > 0.7
    assert len(layout) <= 9


def test_a_required_output_is_reached_exactly_by_the_expert(engine):
    task = replace(green_circuit_task(12, 12, None, None), required_output=4.0)  # unlimited inputs, 4 circuits/s
    layout, report = expert_layout(engine, task)
    assert report["output_per_second"] == pytest.approx(4.0)
    assert report["output_ratio"] == pytest.approx(1.0)
    assert report["waste_ratio"] == pytest.approx(0.0, abs=1e-6)
    assert len(layout) == 10  # 4 circuit + 6 cable machines


def test_the_observation_shows_how_many_machines_of_each_kind_stand_and_how_busy_they_are(env):
    obs, _ = env.reset(seed=0)
    kinds, tail = len(env.kinds), 2 * len(env.kinds)
    assert obs["task"].shape == env.observation_space["task"].shape
    assert not obs["task"][-tail:].any()  # nothing built yet
    action = int(np.flatnonzero(env.action_masks()[:-1])[0])
    obs, *_ = env.step(action)
    built, busy = obs["task"][-tail:-kinds], obs["task"][-kinds:]
    assert (built > 0).sum() == 1 and built.max() <= 1.0 and busy.max() <= 1.0
