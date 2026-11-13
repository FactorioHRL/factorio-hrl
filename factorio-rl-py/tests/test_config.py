import numpy as np
import pytest

from factorio_rl import load_task_spec, load_training
from factorio_rl.config import Supply, TaskSpec

from .conftest import FIXTURES


def test_supply_forms():
    rng = np.random.default_rng(0)
    assert Supply.parse(7, "x").sample(rng) == 7.0
    assert Supply.parse("unlimited", "x").sample(rng) is None
    assert 3 <= Supply.parse({"min": 3, "max": 5}, "x").sample(rng) <= 5
    with pytest.raises(ValueError):
        Supply.parse("lots", "x")


def test_mode_must_match_the_supplies():
    common = dict(name="t", target="x", recipes=(), machines=(), width_range=(3, 3), height_range=(3, 3))
    with pytest.raises(ValueError):
        TaskSpec(mode="grid_limited", supply={"a": Supply(fixed=1.0)}, **common)
    with pytest.raises(ValueError):
        TaskSpec(mode="supply_limited", supply={"a": Supply(unlimited=True)}, **common)


def test_unknown_training_keys_are_rejected(tmp_path):
    bad = tmp_path / "bad.toml"
    bad.write_text('name = "x"\n[ppo]\nlearning_rat = 1\n')
    with pytest.raises(ValueError, match="unknown keys"):
        load_training(bad)


def test_target_output_mode_samples_the_required_rate_and_checks_the_sections(tmp_path):
    wanted = tmp_path / "wanted.toml"
    wanted.write_text('name = "x"\nmode = "target_output"\ntarget = "x"\n[grid]\nwidth = 3\n[output]\nper_second = 4\n[supply]\na = "unlimited"\n')
    assert load_task_spec(wanted).sample(np.random.default_rng(0)).required_output == 4.0
    common = dict(name="t", target="x", recipes=(), machines=(), width_range=(3, 3), height_range=(3, 3))
    free = {"a": Supply(unlimited=True)}
    with pytest.raises(ValueError, match="output"):
        TaskSpec(mode="target_output", supply=free, **common)
    with pytest.raises(ValueError, match="output"):
        TaskSpec(mode="grid_limited", supply=free, required_output=Supply(fixed=1.0), **common)
    bad = tmp_path / "bad.toml"
    bad.write_text(
        'name = "x"\nmode = "target_output"\ntarget = "x"\n[grid]\nwidth = 3\n[output]\nper_second = 0\n[supply]\na = "unlimited"\n'
    )
    with pytest.raises(ValueError, match="positive"):
        load_task_spec(bad)


def test_require_access_is_a_task_rule_off_by_default(tmp_path):
    path = tmp_path / "t.toml"
    path.write_text('name = "x"\nmode = "grid_limited"\ntarget = "x"\n[grid]\nwidth = 3\n[supply]\na = "unlimited"\n')
    assert load_task_spec(path).sample(np.random.default_rng(0)).require_access is False
    path.write_text(
        'name = "x"\nmode = "grid_limited"\ntarget = "x"\nrequire_access = true\n[grid]\nwidth = 3\n[supply]\na = "unlimited"\n'
    )
    assert load_task_spec(path).sample(np.random.default_rng(0)).require_access is True


def test_a_complete_task_file_loads():
    spec = load_task_spec(FIXTURES / "task.toml")
    assert (spec.name, spec.mode, spec.target) == ("mock-task", "target_output", "electronic-circuit")
    assert spec.recipes == ("copper-cable", "electronic-circuit") and spec.machines == ("assembling-machine-1",)
    task = spec.sample(np.random.default_rng(0))
    assert (task.width, task.height) == (12, 8) and task.required_output == 4.0 and task.require_access
    assert task.supply == {"iron-plate": None, "copper-plate": None}


def test_a_complete_training_file_loads():
    training = load_training(FIXTURES / "training.toml")
    assert (training.name, training.algorithm, training.seed, training.device) == ("mock-training", "maskable_ppo", 3, "cpu")
    assert (training.ppo.n_envs, training.ppo.batch_size, training.ppo.init_from) == (2, 64, "latest:other")
    assert training.model.architecture == "cnn" and training.model.options == {"channels": [8, 8], "dilations": [1, 2]}
    assert (training.evaluation.every, training.evaluation.episodes) == (1000, 3)
    assert (training.stop.target_vs_expert, training.stop.patience) == (0.8, 5)
    assert training.reward == {"line": 0.5, "stray": 0}
    assert (training.live.enabled, training.live.port) == (True, 9000)
    assert (training.tracking.enabled, training.tracking.uri) == (False, "sqlite:///x.db")
