import pytest

from factorio_rl import load_training
from factorio_rl.scoring import TERMS, breakdown, ideal_score, reward_term, resolve_weights, score

REPORT = {"output_ratio": 0.8, "waste_ratio": 0.2, "adjacency_ratio": 0.5, "line_ratio": 0.5, "stray_ratio": 0.4, "enclosed_ratio": 0.1, "cut_off_ratio": 0.05, "link_ratio": 0.2}


def test_the_score_is_the_sum_of_the_weighted_terms():
    parts = breakdown(REPORT)
    assert parts["output"] == pytest.approx(0.8) and parts["waste"] == pytest.approx(-0.4)
    assert parts["stray"] < 0 < parts["line"] and parts["enclosed"] == pytest.approx(-0.2) and parts["cut_off"] == pytest.approx(-0.1) and parts["link"] == pytest.approx(-0.1)
    assert score(REPORT) == pytest.approx(sum(parts.values()))
    assert score(REPORT, {"output": 2.0}) == pytest.approx(score(REPORT) + 0.8)


def test_the_ideal_score_counts_rewards_only():
    assert ideal_score() == pytest.approx(1.5)
    assert ideal_score({"line": 0.25, "waste": -5.0}) == pytest.approx(1.0 + 0.2 + 0.25)


def test_unknown_terms_are_rejected_with_the_known_ones():
    with pytest.raises(ValueError, match="unknown reward terms.*known"):
        resolve_weights({"lines": 0.1})


def test_a_custom_term_joins_the_score_and_the_breakdown():
    base = score(REPORT)

    @reward_term("tiles_used", weight=-0.5)
    def tiles_used(report: dict) -> float:
        """Share of the grid that is built over."""
        return report["tiles_used"] / report["tiles_total"]

    try:
        report = {**REPORT, "tiles_used": 30, "tiles_total": 60}
        assert breakdown(report)["tiles_used"] == pytest.approx(-0.25)
        assert score(report) == pytest.approx(base - 0.25)
        assert TERMS["tiles_used"].doc == "Share of the grid that is built over."
        with pytest.raises(ValueError, match="twice"):
            reward_term("tiles_used", 1.0)(tiles_used)
    finally:
        del TERMS["tiles_used"]


def test_weights_come_from_the_training_config(tmp_path):
    path = tmp_path / "t.toml"
    path.write_text('name = "x"\n[reward]\nline = 0.2\nstray = 0\n')
    assert resolve_weights(load_training(path).reward)["line"] == 0.2
    path.write_text('name = "x"\n[reward]\nnope = 1\n')
    with pytest.raises(ValueError, match="unknown reward terms"):
        load_training(path)
