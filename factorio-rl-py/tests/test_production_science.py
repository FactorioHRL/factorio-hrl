import json

import pytest

from factorio_rl import Placement
from factorio_rl.baselines import reference_score
from factorio_rl.scoring import ideal_score

from .specs import PACK, PRODUCTION_RECIPES, production_task, unlimited_production_task


def test_the_agent_can_build_every_stage_of_the_chain(engine):
    kinds = engine.kinds(production_task(32, 32, {"iron-plate": 10.0}))
    assert len(kinds) == 11
    assert {(k.width, k.height) for k in kinds} == {(3, 3)}
    assert {k.recipe for k in kinds} == set(PRODUCTION_RECIPES)


def test_raw_costs_per_pack_match_the_wiki(engine):
    """https://wiki.factorio.com/Production_science_pack, "Raw Costs" (without the oil cracking and water)."""
    task = unlimited_production_task(90, 45)
    kinds = engine.kinds(task)
    # plenty of every machine: the program uses only what the chain needs
    layout = [Placement(k.machine, k.recipe, (n % 30) * 3, (n // 30) * 3) for n, k in enumerate(k for _ in range(40) for k in kinds)]
    report = engine.evaluate(task, layout)
    assert report["valid"], report["violations"]
    packs = report["output_per_second"]
    assert packs > 0
    draw = {f["item"]: -f["per_second"] / packs for f in report["flows"] if f["per_second"] < 0}
    assert draw["iron-plate"] == pytest.approx(52.5, rel=0.01)
    assert draw["copper-plate"] == pytest.approx(19.2, rel=0.01)
    assert draw["coal"] == pytest.approx(3.3, rel=0.02)
    assert draw["stone"] == pytest.approx(11.7, rel=0.01)


def test_expert_does_not_apply_so_the_reference_is_the_ideal(engine):
    assert reference_score(engine, production_task(32, 32, {"iron-plate": 10.0})) == ideal_score()


def test_the_viewer_draws_more_than_eight_kinds(engine, tmp_path):
    """Regression: the text grid once had a symbol per kind and ran out with 11 kinds."""
    from factorio_rl.viewer import Viewer

    task = production_task(32, 32, {"iron-plate": 10.0})
    kinds = engine.kinds(task)
    layout = [Placement(k.machine, k.recipe, (n % 10) * 3, (n // 10) * 3) for n, k in enumerate(kinds)]
    report = engine.evaluate(task, layout)
    Viewer(engine, out_dir=tmp_path).publish(task, layout, "test", 1, 0.0, report, mean_score=0.0)
    ascii_grid = json.loads((tmp_path / "state.json").read_text())["ascii"]
    assert set(ascii_grid) <= {"M", ".", "\n"} and ascii_grid.count("M") == 9 * len(kinds)


def chain_with(engine, **counts: int) -> dict:
    """Report for a chain with 40 machines of every kind except those named (recipe with `_` for `-`)."""
    task = unlimited_production_task(90, 45)
    wanted = {k.recipe: counts.get(k.recipe.replace("-", "_"), 40) for k in engine.kinds(task)}
    layout, n = [], 0
    for kind in engine.kinds(task):
        for _ in range(wanted[kind.recipe]):
            layout.append(Placement(kind.machine, kind.recipe, (n % 30) * 3, (n // 30) * 3))
            n += 1
    return engine.evaluate(task, layout)


def test_output_is_the_flow_of_the_whole_chain_not_the_count_of_final_machines(engine):
    """One productivity-module machine makes 0.05 modules/s and a module is a third of a craft of
    packs: the packs per second follow that scarce stage, however many pack machines stand."""
    one, two, three = (chain_with(engine, productivity_module=n)["output_per_second"] for n in (1, 2, 3))
    assert one == pytest.approx(0.15, rel=0.01)
    assert two == pytest.approx(2 * one, rel=0.01) and three == pytest.approx(3 * one, rel=0.01)
    starved = chain_with(engine, rail=0)["output_per_second"]
    assert starved == 0  # no rails at all: no packs, whatever else stands


def test_a_machine_far_from_the_makers_of_its_ingredients_costs_a_link(engine):
    task = unlimited_production_task(60, 60)
    kinds = {k.recipe: i for i, k in enumerate(engine.kinds(task))}

    def chain_report(pack_at: tuple[int, int]) -> dict:
        episode = engine.start(task)
        for recipe, (x, y) in [("rail", (0, 0)), ("electric-furnace", (6, 0)), ("productivity-module", (12, 0)), ("production-science-pack", pack_at)]:
            episode.place(kinds[recipe], x, y)
        return episode.report()

    near, far = chain_report((3, 3)), chain_report((45, 45))
    assert near["link_ratio"] < far["link_ratio"]
    # the pack machine has three ingredients made in the layout; far away, each link is capped at one,
    # and the sum is divided by the 400 machines that fit on 60x60
    assert far["link_ratio"] == pytest.approx(3 / 400) and near["link_ratio"] < 0.001
