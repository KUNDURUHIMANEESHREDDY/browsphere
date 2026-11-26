# Browsphere / Lucia — Audience showcase acceptance (25 November 2026)

## Installed app tested
- Executable: `%LOCALAPPDATA%\Browsphere\browsphere-core.exe`
- Release SHA-256: `183B6C49A5E68BA959B404061A3D680A0C311D2C0C8827146A37C73F2913F324`
- Tested from isolated Windows WebView2 profiles; browser compatibility `17/17` for each run.
- The following are *real run results*, not plans or mock demonstrations.

| ID | Audience task | Installed-app verdict | Main failure | Raw log |
| --- | --- | --- | --- | --- |
| S1 | Three free beginner Python courses across platforms, certificate status, duration and recommendation | FAIL — `halted`, `verified=false` | Click succeeded; navigation/scroll denied, supervisor recovery repeated, loop guard safely stopped | `tests/e2e/artifacts/demo-s1-installed-20261125.log` |
| W1 | Notion vs Trello vs ClickUp free plans for five-person team | FAIL — `timeout`, `verified=false` | 240s timeout; multiple denied clicks/typing and insufficient verified source coverage | `tests/e2e/artifacts/demo-w1-installed-20261125.log` |
| P1 | Bengaluru family outing under ₹2,000 for four | FAIL — `failed`, `verified=false` | Native inference deferred when available memory fell below 2 GiB; navigation/scroll also denied | `tests/e2e/artifacts/demo-p1-installed-20261125.log` |

## Verified alternatives from installed release
- Student-oriented: Python list comprehensions versus JavaScript `Array.map` using Python docs + MDN; 3/3 previously passed.
- Professional-oriented: Compare Requests, HTTPX and aiohttp with six official-documentation evidence obligations; passed after evidence-ledger update. GitHub repository installation/version, official HTTP timeout comparison and PostgreSQL `INSERT / ON CONFLICT` also passed.
- General research: MDN `Number.isInteger` and Wikipedia Saturn fact extraction; each 2/2 previously passed.
- Missing-link failure test passed as *safe stop*, not task completion.

## Demo recommendation
**Do not promise successful live S1/W1/P1 executions.** Use the known passing tasks for an authentic live walkthrough. Show S1/W1/P1 only as scenarios under development unless independently re-tested successfully after fixes.

## Work to prioritize next
1. **S1 and P1:** identify exact rejected navigation and scroll targets in structured run traces; repair action grounding, source discovery and cross-origin navigation policy *without* broadening risky permissions or turning off the guard.
2. **W1:** focus on complete evidence acquisition and verifier ledger for the three official free-plan pages. The latest debug core has changed during other development sessions; do not equate debug results with installed release until rebuilt and retested.
3. **P1 memory:** avoid concurrent model loads, reduce runtime memory pressure and preserve the <2 GiB low-memory fail-safe. Re-test with 3+ GiB free at launch and monitor peak memory.
4. **Release gate:** rebuild/replace installed NSIS binary only after the exact S1/W1/P1 tasks pass in isolated WebView2 sessions, ideally twice each, with no false completion claims.
5. **No code-signing requirement for this one-day local demo:** the installer remains unsigned, but properly built and installed.

No source code, binaries, or release package were changed in this audience-test pass.
