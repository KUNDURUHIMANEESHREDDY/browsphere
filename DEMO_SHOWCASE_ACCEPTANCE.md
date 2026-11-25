# Lucia — live demo acceptance (25 November 2026)

Purpose: rehearse a real native WebView2 demonstration, not claim production readiness.

## Medium task: multi-step research
Website: https://en.wikipedia.org/wiki/Main_Page

Paste into Lucia:
> On this website, search for the planet Uranus and open the corresponding article. Report its position from the Sun.

Pass only if the real Wikipedia search and article are observed, the final native URL is /wiki/Uranus, and a source-backed quotation actually states its position from the Sun. A failed, blocked, timed-out or unverified run is a failure.

## Complex task: two-source technical comparison
Websites:
- https://docs.python.org/3/tutorial/datastructures.html
- https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Array/map

Paste into Lucia:
> Research and compare Python list comprehensions and JavaScript Array.map using the official documentation pages https://docs.python.org/3/tutorial/datastructures.html and https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Array/map . Read both sources. Explain what each produces and one limitation or distinction of each. Cite exact supporting excerpts from both pages.

Pass only if both exact sites are observed and findings contain relevant, exact, original-source supporting passages for each language and the requested distinctions. A URL visited without the answer is NOT completion.

## One-command rehearsal
From Browsphere repository root, using the current compiled native executable:

```powershell
python tests/e2e/run-lucia-phase3.py --case compose-wiki --case compose-two-sources --repeat 2 --output tests/e2e/artifacts/demo-showcase
```

The runner isolates every WebView2 profile, saves individual logs and JSON results, checks destination and factual evidence, and fails on unverified terminal states. If websites or local Ollama models are unavailable, do not count the scenario as a pass.

## Presenter notes
- Show the browser task tab and action timeline.
- Run the same prompt actually rehearsed; do not claim general-purpose website reliability.
- Do not manually alter the result, inject evidence, or show a saved recording as a live execution.
- Keep the prior failure logs: before/after improvement is part of the technical story.
- Do not call the complex task ready until it has passed repeated real-site runs.


## Latest validated rehearsal outcome — 25 Nov 2026

**Primary live demo: Python vs JavaScript comparison — PASS 3/3 observed native runs.**
- Tested native executable: `target/debug/browsphere-demo-showcase-v3.exe`.
- The `compose-two-sources` source-backed comparison passed 1 original live run plus both repeat runs.
- Current repeated report: `tests/e2e/artifacts/demo-showcase/run-20261125T191139026359Z/results.json` (two passes for comparison).
- All checks require `status=completed`, accepted verification, source matching, and no unexpected approval. This is a **demo rehearsal result**, not general autonomy proof.

**Secondary demo: Wikipedia planet lookup — PASS 2/3 observed runs.**
- It passed the first post-fix run and the second repeated run.
- One repeated run opened `Uranus_(mythology)`, correctly failed verification, and did **not** falsely report success.
- Do not rely on this task as the only live showcase. If used, show the exact prompt and keep backup recordings plainly labeled as recordings.

### Reproduce primary demo independently

```powershell
cd C:\Users\himan\Downloads\Browsphere
python tests/e2e/run-lucia-phase3.py --exe target/debug/browsphere-demo-showcase-v3.exe --case compose-two-sources --repeat 2 --output tests/e2e/artifacts/demo-showcase
```

The standard installed executable is **not** updated by this isolated debug build; no release installer or GitHub push is implied. Keep the native task view and source evidence visible in the showcase.

## Main Lucia core validation (not merely standalone demo binary)

- Main executable: `target/debug/browsphere-core.exe` (SHA256 `5066da3277835cefd4779107439354587e98945b4e9b689a1545eb5f927b4d08`).
- `compose-two-sources` checked successfully once, then twice again using the same main binary: **3/3 passed**.
- Latest report: `tests/e2e/artifacts/core-update-validation/run-20261125T191919537180Z/results.json`.
- This validates the current development core, **not** an installed Windows release. User changes in the working tree were preserved; no GitHub push or installer replacement was performed.

## Installed Windows release — verified 25 November 2026

- NSIS release build: `cargo tauri build --bundles nsis --ci` completed with exit code 0.
- Generated installer: `target/release/bundle/nsis/Browsphere_0.1.0_x64-setup.exe`.
- Current distribution copy: `dist/Browsphere-setup.exe` (SHA-256 `D99D973C18F9FB814C8AA0C65D9DA79ADEB1F2B65941036C96E5E9658994DABF`).
- Silent installation completed with exit code 0 into `%LOCALAPPDATA%\Browsphere`.
- Installed core SHA-256: `E51D89C4B4F8C814C257E874DFB3F670DE6B761C95388F60E324AA6D3E475B3A`. The installed NSIS-patched binary differs from the original release output by three bytes of bundle metadata.
- Independent tests launched `%LOCALAPPDATA%\Browsphere\browsphere-core.exe` directly; live Python+MDN comparison verified **3/3 executions** (single run plus two repeats).
- Latest installed validation: `tests/e2e/artifacts/installed-release-validation/run-20261125T192708792632Z/results.json` (2/2 pass), and `run-20261125T192639430547Z/results.json` (1/1 pass).
- Previous installed core backup: `logs/browsphere-core-before-install-20261125.exe`.
- Previous `dist` setup backup: `dist/Browsphere-setup-before-lucia-update-20261125.exe`.
- Existing app data folder remains present. No git push was made. Installer is unsigned, so SmartScreen/publisher warnings may appear on other PCs.

These results cover the rehearsed comparison; they do not establish arbitrary cross-website task reliability.
