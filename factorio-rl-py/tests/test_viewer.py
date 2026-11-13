import json

from factorio_rl import green_circuit_task
from factorio_rl.baselines import expert_layout
from factorio_rl.scoring import score
from factorio_rl.viewer import Viewer, format_stats

PNG_MAGIC = b"\x89PNG\r\n\x1a\n"


def test_render_png_returns_a_png(engine):
    task = green_circuit_task(12, 12, 2, 3)
    layout, _ = expert_layout(engine, task)
    assert engine.render_png(layout).startswith(PNG_MAGIC)


def test_publish_writes_the_page_data(engine, tmp_path):
    task = green_circuit_task(12, 12, 2, 3)
    layout, report = expert_layout(engine, task)
    viewer = Viewer(engine, out_dir=tmp_path)
    viewer.publish(task, layout, "expert", 1, score(report), report, reference=1.0)
    viewer.publish(task, layout, "expert", 2, score(report), report, reference=1.0)
    state = json.loads((tmp_path / "state.json").read_text())
    assert state["version"] == 2 and len(state["history"]) == 2
    assert state["blueprint"].startswith("0") and state["ascii"].count("M") == 9 * len(layout)
    assert (tmp_path / "latest.png").read_bytes().startswith(PNG_MAGIC)
    assert (tmp_path / "index.html").is_file()


def test_stats_are_human_readable(engine):
    task = green_circuit_task(12, 12, 2, 3)
    layout, report = expert_layout(engine, task)
    text = format_stats(task, 7, score(report), report, reference=1.1)
    assert "PRODUCES  electronic-circuit" in text and "CONSUMES" in text and "MACHINES" in text
    assert "iron-plate" in text and "copper-plate" in text and "(100%)" in text
