"""Smoke the official BrowserGym research environment, not a Lucia clone.

BrowserGym's own gymnasium environment owns browser creation, actions and
observations. A deterministic fixture is deliberately not counted as real
model success, and BrowserGym's openended reward is not a success metric.
"""
from __future__ import annotations

import json
import gymnasium as gym
import browsergym.core  # noqa: F401 - upstream registers its environments

env = gym.make(
    "browsergym/openended",
    task_kwargs={"start_url": "about:blank", "goal": "Inspect an isolated blank page"},
    headless=True,
    action_mapping=None,
    slow_mo=0,
    pre_observation_delay=0,
)
try:
    observation, info = env.reset(seed=42)
    assert isinstance(observation, dict)
    assert "url" in observation
    assert str(observation["url"]).startswith("about:blank")
    # This is the upstream BrowserGym action runner, not a custom browser.
    obs, reward, terminated, truncated, info = env.step("page.evaluate('document.title')")
    assert isinstance(obs, dict)
    assert isinstance(reward, (int, float))
    assert "last_action_error" in obs or hasattr(env.unwrapped, "last_action_error")
    assert not bool(getattr(env.unwrapped, "last_action_error", "")), "Upstream BrowserGym action failed"
    print("UPSTREAM_BROWSERGYM_OPENENDED_CAPABILITY " + json.dumps({
        "runner": "browsergym-core",
        "browser_observed": True,
        "action_executed": True,
        "independent_research_env": True,
        "lucia_task_success": False,
        "comparable_success_rate": False,
    }))
finally:
    env.close()
