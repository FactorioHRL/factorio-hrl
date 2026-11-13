import pytest

from factorio_rl import Placement, decode_blueprint, green_circuit_task, score
from factorio_rl.scoring import TERMS, breakdown

from .conftest import NO_SHAPE

ASM = "assembling-machine-1"


def row(cables: int, circuits: int) -> list[Placement]:
    """Cable machines, then circuit machines, in a single row of 3x3 cells."""
    kinds = ["copper-cable"] * cables + ["electronic-circuit"] * circuits
    return [Placement(ASM, recipe, i * 3, 0) for i, recipe in enumerate(kinds)]


def test_kinds_are_cable_and_circuit_assemblers(engine):
    kinds = engine.kinds(green_circuit_task(30, 30, 10, 15))
    assert [(k.recipe, k.width, k.height) for k in kinds] == [
        ("copper-cable", 3, 3),
        ("electronic-circuit", 3, 3),
    ]


def test_the_example_from_the_brief(engine):
    """10 iron/s and 15 copper/s on 30x30 need 15 cable and 10 circuit assemblers."""
    task = green_circuit_task(75, 3, iron=10, copper=15)
    report = engine.evaluate(task, row(15, 10))
    assert report["valid"]
    assert report["output_per_second"] == pytest.approx(10.0)
    assert report["output_ratio"] == pytest.approx(1.0)
    assert report["idle_machines"] == pytest.approx(0.0, abs=1e-6)
    assert score(report, NO_SHAPE) == pytest.approx(1.0)


def test_spare_machines_cost_score_but_not_output(engine):
    task = green_circuit_task(90, 3, iron=10, copper=15)
    lean, spare = engine.evaluate(task, row(15, 10)), engine.evaluate(task, row(18, 12))
    assert spare["output_per_second"] == pytest.approx(lean["output_per_second"])
    assert score(spare) < score(lean)


def test_invalid_layouts_are_reported(engine):
    task = green_circuit_task(10, 6, iron=1, copper=1)
    layout = [Placement(ASM, "copper-cable", 0, 0), Placement(ASM, "copper-cable", 2, 0), Placement(ASM, "electronic-circuit", 9, 0)]
    report = engine.evaluate(task, layout)
    assert not report["valid"] and report["output_per_second"] == 0
    assert {v["kind"] for v in report["violations"]} == {"overlap", "out_of_bounds"}


def test_legal_actions_shrink_as_machines_are_placed(engine):
    task = green_circuit_task(9, 3, iron=1, copper=1)
    assert engine.legal_actions(task, []).shape == (2, 3, 9)
    assert engine.legal_actions(task, []).sum() == 2 * 7  # x in 0..=6
    assert engine.legal_actions(task, [Placement(ASM, "copper-cable", 0, 0)]).sum() == 2 * 4  # x in 3..=6


def test_blueprint_export_import_round_trip(engine):
    task = green_circuit_task(30, 3, iron=10, copper=15)
    layout = row(4, 3)
    text = engine.export_blueprint(layout)
    assert engine.analyze_blueprint(task, text)["placed"] == engine.evaluate(task, layout)["placed"]
    assert len(decode_blueprint(text)["blueprint"]["entities"]) == 7


def test_unknown_blueprints_and_tasks_raise(engine):
    with pytest.raises(ValueError):
        engine.analyze_blueprint(green_circuit_task(3, 3, 1, 1), "1garbage")
    with pytest.raises(ValueError):
        engine.evaluate(green_circuit_task(0, 3, 1, 1), [])


def test_a_long_row_of_twins_earns_the_shape_rewards(engine):
    task = green_circuit_task(90, 3, iron=10, copper=15)
    packed = engine.evaluate(task, row(15, 10))  # 15 cable and 10 circuit machines, each kind in one row
    assert packed["adjacency_ratio"] > 0.9  # all but the two ends of a row have two full neighbours
    assert packed["line_ratio"] == pytest.approx(1.0) and packed["stray_ratio"] == 0
    shape = ("adjacency", "line", "stray", "enclosed", "cut_off", "link")
    extra = sum(TERMS[name].weight * packed[f"{name}_ratio"] for name in shape)
    assert score(packed) - score(packed, NO_SHAPE) == pytest.approx(extra)
    # a single row in a grid 3 tiles high has the grid edge above and below: its inner machines are walled in
    assert packed["enclosed_ratio"] > 0


def test_scattered_twins_are_punished_and_a_line_counts_as_one_machine(engine):
    task = green_circuit_task(90, 12, iron=10, copper=15)
    cable = lambda x, y: Placement(ASM, "copper-cable", x, y)  # noqa: E731
    line = [cable(i * 3, 0) for i in range(4)]
    assert engine.evaluate(task, line)["stray_ratio"] == 0  # one unit
    far = [*line, cable(60, 9)]
    report = engine.evaluate(task, far)
    assert report["stray_ratio"] == pytest.approx(1 / 4)  # a line (one unit) and one far machine: 1 of 4 steps apart
    assert breakdown(report)["stray"] < 0 and breakdown(report)["line"] > 0  # a penalty and a reward, on different machines
    pair = [cable(0, 0), cable(60, 9)]
    assert engine.evaluate(task, pair)["stray_ratio"] == 1  # no line needed: two machines far apart
    touching = [cable(0, 0), cable(3, 0)]
    assert engine.evaluate(task, touching)["stray_ratio"] == 0


def test_a_line_that_grows_a_branch_stops_being_a_line(engine):
    task = green_circuit_task(30, 30, iron=10, copper=15)
    cable = lambda x, y: Placement(ASM, "copper-cable", x, y)  # noqa: E731
    row = [cable(3 + i * 3, 3) for i in range(3)]
    assert engine.evaluate(task, row)["line_ratio"] == pytest.approx(0.5)
    t_shape = [*row, cable(6, 0), cable(6, 6)]  # a column through the middle of the row
    assert engine.evaluate(task, t_shape)["line_ratio"] == 0  # neither the row nor the column counts
    domino = [*row, *[cable(3 + i * 3, 6) for i in range(3)]]
    assert engine.evaluate(task, domino)["line_ratio"] == 0


def test_a_machine_walled_in_on_every_side_is_punished_hard(engine):
    task = green_circuit_task(30, 30, iron=10, copper=15)  # 100 machines fit
    block = [Placement(ASM, "copper-cable", 12 + (i % 3) * 3, 12 + (i // 3) * 3) for i in range(9)]
    report = engine.evaluate(task, block)
    assert report["enclosed_ratio"] == pytest.approx(1 / 100)  # only the middle one has no free side tile
    spread = [Placement(ASM, "copper-cable", 12 + (i % 3) * 4, 12 + (i // 3) * 4) for i in range(9)]
    assert engine.evaluate(task, spread)["enclosed_ratio"] == 0
    assert breakdown(report)["enclosed"] == pytest.approx(-0.02)
    assert breakdown(report)["enclosed"] < -0.01  # a clear cost, bigger than one idle machine (-0.01)


def test_machines_nobody_can_reach_make_nothing_when_the_task_requires_access(engine):
    from dataclasses import replace

    task = green_circuit_task(10, 10, iron=10, copper=15)
    kinds = {k.recipe: i for i, k in enumerate(engine.kinds(task))}
    layout = [Placement(ASM, "electronic-circuit" if i == 4 else "copper-cable", (i % 3) * 3, (i // 3) * 3) for i in range(9)]  # the middle one is walled in
    free = engine.evaluate(task, layout)
    gated = engine.evaluate(replace(task, require_access=True), layout)
    assert free["output_per_second"] == pytest.approx(1.0) and gated["output_per_second"] == 0
    assert free["enclosed_ratio"] > 0 and gated["idle_machines"] > free["idle_machines"]
    assert kinds  # both recipes are allowed kinds of the task


def test_an_episode_matches_the_stateless_engine(engine):
    task = green_circuit_task(12, 12, iron=2, copper=3)
    episode = engine.start(task)
    kinds = engine.kinds(task)
    layout = []
    for kind, (x, y) in [(0, (0, 0)), (1, (3, 0)), (0, (6, 3))]:
        episode.place(kind, x, y)
        layout.append(Placement(kinds[kind].machine, kinds[kind].recipe, x, y))
        assert (episode.legal() == engine.legal_actions(task, layout)).all()
        assert episode.report() == engine.evaluate(task, layout)
    assert episode.planes().sum() == 3 * 9
    with pytest.raises(ValueError):
        episode.place(0, 0, 0)  # overlaps the first machine
