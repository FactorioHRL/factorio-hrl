import numpy as np
import pytest

from factorio_rl import LayoutEnv, load_training
from factorio_rl.config import ModelConfig
from factorio_rl.models import ARCHITECTURES, load_architecture, policy_for

from .specs import small_target_spec


def test_the_architectures_are_the_modules_of_the_folder():
    assert ARCHITECTURES == ("cnn", "mlp")
    with pytest.raises(ValueError, match="unknown architecture"):
        load_architecture("transformer")
    with pytest.raises(ValueError, match="architecture must be one of"):
        ModelConfig(architecture="transformer")



def test_the_deprecated_policy_key_is_ignored_with_a_warning(tmp_path):
    path = tmp_path / "old.toml"
    path.write_text('name = "x"\n[ppo]\npolicy = "MultiInputPolicy"\n')
    with pytest.warns(UserWarning, match="policy is ignored"):
        assert load_training(path).model.architecture == "mlp"


def test_options_are_checked_against_the_architecture():
    pytest.importorskip("torch")
    with pytest.raises(ValueError, match="unknown keys"):
        policy_for("cnn", {"filters": 3})
    with pytest.raises(ValueError, match="same length"):
        policy_for("cnn", {"channels": [8, 8], "dilations": [1]})
    assert policy_for("mlp", {"net_arch": [32]})[1] == {"net_arch": [32]}


@pytest.fixture(scope="module")
def env(engine):
    return LayoutEnv(engine, small_target_spec())


@pytest.mark.parametrize("architecture", ["mlp", "cnn"])
def test_each_architecture_trains_on_the_same_environment(env, architecture):
    pytest.importorskip("sb3_contrib")
    from sb3_contrib import MaskablePPO

    policy, kwargs = policy_for(architecture, {} if architecture == "mlp" else {"channels": [8, 8], "dilations": [1, 2]})
    model = MaskablePPO(policy, env, policy_kwargs=kwargs, n_steps=64, batch_size=32, n_epochs=1, device="cpu", seed=0)
    model.learn(128)
    obs, _ = env.reset(seed=0)
    action, _ = model.predict(obs, action_masks=env.action_masks(), deterministic=True)
    assert env.action_masks()[int(action)], "the chosen action is a legal one"


def test_the_conv_body_gives_one_logit_per_kind_and_tile_plus_stop(env):
    torch = pytest.importorskip("torch")
    from factorio_rl.models.cnn import ConvBody

    planes, height, width = env.observation_space["grid"].shape
    task_dim = env.observation_space["task"].shape[0]
    body = ConvBody(planes, height, width, task_dim, (4, 4), (1, 2))
    logits, state = body(torch.zeros(3, planes * height * width + task_dim))
    assert logits.shape == (3, env.action_space.n) and state.shape == (3, 8)
    # equivariance: shifting the whole picture shifts the logits (away from the border effects)
    grid = torch.zeros(1, planes, height, width)
    grid[0, 0, 4, 4] = 1
    shifted = torch.roll(grid, shifts=(2, 3), dims=(2, 3))
    flat = lambda g: torch.cat([g.flatten(1), torch.zeros(1, task_dim)], dim=1)  # noqa: E731
    a = body(flat(grid))[0][0, : height * width].reshape(height, width)
    b = body(flat(shifted))[0][0, : height * width].reshape(height, width)
    assert torch.allclose(torch.roll(a, (2, 3), (0, 1))[6:-6, 6:-6], b[6:-6, 6:-6], atol=1e-5)
