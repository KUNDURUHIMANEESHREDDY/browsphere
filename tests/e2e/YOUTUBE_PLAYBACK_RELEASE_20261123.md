# Browsphere Windows release — YouTube playback verification (2026-11-23)

## Implemented
- For the narrowly explicit request **"Open YouTube and play any song"**, a trusted Browser + Verification plan and initial navigation to YouTube can run without an extra hosted planning call. Song selection, search, opening the result and clicking Play still require the selected action model and current live webpage evidence.
- A **native, read-only WebView2 media verifier** checks an observed YouTube watch/shorts page, an available HTML media element, unpaused state and measurable forward progress in currentTime between two fresh samples. Opening a page alone is never reported as music playing. The verifier never clicks controls, changes playback, obtains user cookies, reads media URLs, or captures raw audio/video.
- The genuine OpenRouter free-model test harness includes a YouTube music task. It requires both original real-model action/evidence gates and independent playback confirmation. Media evidence is only a boolean and a short classification; raw media page content is not written to logs.

## Verification on the Windows laptop
- `cargo test --workspace --locked --quiet`: **131 passed, 5 ignored, 0 failed** (media-clock regression included).
- JavaScript model routing + UI tests: **30 passed**; specific OpenRouter/playback harness assertions: **8 passed**.
- `cargo tauri build --bundles nsis`: **PASS**. Installer written to `dist/Browsphere-setup.exe`, standalone executable copied to `dist/Browsphere.exe`. SHA-256 installer: `42232866D62D97C2001A328F89DFFF6694D42796C66EB8AF82FEF65D5CA79C47`. **Unsigned development package**, not a signed production release.
- Native cloud-compatible **simulated-model** multi-step WebView2 integration against release executable: **PASS** (typing/search, menus, recovery, stage verification, task tab lifecycle). Not genuine hosted-model autonomy.
- Native **real public website** browser-control smoke: **5/5 passed** (Python Docs, MDN, Wiktionary, GitHub Docs, Hacker News); script drove controls without model inference.
- Native **real OpenRouter YouTube** attempt using `inclusionai/ling-3.2-flash:free`: YouTube home loaded, 12 observed controls, task run started; **OpenRouter HTTP 429** on first requested model action. Outcome: failed, **zero model-generated actions**, **no video watch page**, **no playback confirmation**. No repeated provider calls or model/account switching.

## Release gate
The unsigned installer is available for local testing. Reliable real-model multi-site autonomy and music playback remain **unverified**, blocked by upstream free-model rate limits. Do not misrepresent model 429 as proof of successful playback. The installer should be signed and tested on a clean Windows installation before a public release.
