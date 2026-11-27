# Reuse Browser Use's actual autonomous agent (experimental)

This is a **CI-only comparative acceptance harness** using the maintained
MIT-licensed [browser-use/browser-use](https://github.com/browser-use/browser-use)
Python agent, not a new implementation of its browser/reasoning loop.
It runs real `qwen2.5:3b-instruct` via a disposable Ollama service against
the official web.dev accessibility guide and the W3C informative-images page.

**Security:** Browser Use 1.x lets the model run Python code. Domain controls
limit primary browser navigation but are **not an OS sandbox**. Do not run this
against a private browser profile, credentials, or an ordinary Windows session.
The workflow uses a disposable GitHub runner, no secrets, no inherited
Browser Use Cloud account, and no connection to the user's Tauri WebView2.

**Acceptance:** An agent-generated completion alone never passes. The model
must produce a structured exact-source quote, and a separate HTTP retrieval
of the official document must contain that precise passage. Missing,
paraphrased, invented or off-site evidence fails closed. This proves an
independent public-source match; **not** installed Lucia task completion,
security isolation, or release readiness.

Run `python -m unittest discover -s integrations/browser-use-qwen -p test_trial.py`
for the no-dependency unit tests. The GitHub Actions workflow
`lucia-upstream-browser-use.yml` runs the genuine Qwen comparison by
explicit manual dispatch, writes a result JSON for each site even on
failure, and uploads the evidence. An independent run must pass at least
3/3 on each site before proposing integration with Lucia.
