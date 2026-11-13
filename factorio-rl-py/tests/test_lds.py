import numpy as np
import pytest

from factorio_rl import LayoutEnv, Placement
from factorio_rl.baselines import expert_layout
from factorio_rl.scoring import score

from .specs import LDS, lds_spec, lds_task

ASM2, CHEM = "assembling-machine-2", "chemical-plant"


def test_the_agent_may_build_structure_assemblers_and_chemical_plants(engine):
    kinds = engine.kinds(lds_task())
    assert [(k.machine, k.recipe) for k in kinds] == [(ASM2, LDS), (CHEM, "plastic-bar")]


def test_plastic_is_needed_before_anything_is_made(engine):
    task = lds_task()
    only_assemblers = engine.evaluate(task, [Placement(ASM2, LDS, 0, 0)])
    assert only_assemblers["output_per_second"] == 0


def test_expert_balances_assemblers_and_plants(engine):
    layout, report = expert_layout(engine, lds_task(copper=3, steel=3, coal=3))
    recipes = [p.recipe for p in layout]
    assert recipes.count(LDS) > recipes.count("plastic-bar") >= 1
    assert report["output_per_second"] > 0 and score(report) > 0.5


def test_grid_limited_task_reports_the_inputs_it_needs(engine):
    task = lds_spec(grid_limited=True).sample(np.random.default_rng(0))
    layout, report = expert_layout(engine, task)
    draw = {f["item"]: -f["per_second"] for f in report["flows"] if f["per_second"] < 0}
    # per structure: 20 copper, 2 steel, 5 plastic (= 2.5 coal + 50 gas)
    made = report["output_per_second"]
    assert made > 0
    assert draw["copper-plate"] == pytest.approx(20 * made)
    assert draw["steel-plate"] == pytest.approx(2 * made)
    assert draw["coal"] == pytest.approx(2.5 * made)
    assert draw["petroleum-gas"] == pytest.approx(50 * made)


def test_environment_runs_on_the_lds_tasks(engine):
    env = LayoutEnv(engine, lds_spec())
    env.reset(seed=0)
    rng = np.random.default_rng(0)
    for _ in range(20):
        legal = np.flatnonzero(env.action_masks())
        _, _, terminated, truncated, info = env.step(int(rng.choice(legal)))
        assert not info["illegal"]
        if terminated or truncated:
            break
