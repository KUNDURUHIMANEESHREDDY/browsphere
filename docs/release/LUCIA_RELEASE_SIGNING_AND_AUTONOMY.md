# Lucia release trust and expanded autonomy acceptance — 2026-11-25

## Windows signing status
The NSIS release installer at `dist/Browsphere-setup.exe` was rebuilt and successfully installed. The installer is **unsigned**. Checks for a private-key-backed Code Signing certificate in the CurrentUser and LocalMachine Windows Personal stores found none. An installed trusted certificate, hosted signing service, or authorized organizational signing setup is required before Windows Authenticode signing can be completed.

Do **not** self-sign and present it as a trusted publisher signature or bypass Windows warnings to claim publisher verification. The current installer can still be used for the controlled local showcase; disclose that it is unsigned.

The build script `build-windows-installer.bat` already has a conditional `PFX_PATH` / `PFX_PASSWORD` signing step when `signtool` is installed; avoid placing certificate passwords in source control. After signing, check the signature with `Get-AuthenticodeSignature dist\Browsphere-setup.exe` and require status `Valid` before labeling the release signed.

## Installed release research benchmarks
Installer installation was validated with a native WebView2, isolated browser profiles, and independent factual predicates.

- Primary Python vs JavaScript official documentation comparison: **3/3** pass on installed release (see `tests/e2e/artifacts/installed-release-validation`)
- Held-out MDN Number.isInteger reference: **2/2** pass
- Held-out Wikipedia Saturn search/position: **2/2** pass
- Three-source Requests/HTTPX/aiohttp feature/caveat comparison: **FAIL**; all six factual checks were missing, final status `unverified`, zero false completion claims. See `tests/e2e/artifacts/expanded-installed-validation/run-20261125T193252856052Z/results.json`

A successful short benchmark is **not** evidence of general autonomy. The three-source failure is a genuine gap in evidence synthesis or coverage. No release source code was changed or rebuilt as part of the expanded tests. Do not advertise that complex heterogeneous workflows are generally working.

## Safest demo option
Use the previously validated two-source Python vs JavaScript task. The MDN and Wikipedia Saturn tasks are rehearsed additional workflows. Do not showcase the three-source comparison as a reliably completed autonomous task without fixing and rerunning it.


## Expanded verification fix and installed release (25 Nov 2026, final)
- Added a strict Rust requirement-ledger acceptance path to `src-tauri/src/router/model_router.rs`. The model-based semantic coverage reviewer remains in place when the ledger cannot prove full objective coverage; no evidence is invented or inferred.
- Updated the independent Phase 3 evaluator to treat a verified missing-link safe stop with terminal state `unverified` as a **safe stop**, *not* successful task completion. Requires no decoy, no invented URL, and no false verification.
- On the experimental binary, previously failed three-source HTTP client comparison **passed twice**; Requests GitHub repository installation/version research **passed twice**. A missing-link negative test passed as a safe stop.
- Rebuilt the Windows NSIS installer, installed it (exit code 0) and verified the **installed release** on the following live cases:
  - `level-2` — 3 HTTP libraries / 3 official sources / 6 distinct evidence obligations: PASS.
  - `level-3` — official GitHub repo + install command + supported Python version: PASS.
  - `level-7` — reconcile apparent default timeout conflicts across three official docs: PASS.
  - `level-8` — PostgreSQL INSERT / ON CONFLICT DO NOTHING versus DO UPDATE: PASS.
  - `missing-link` — expected unverified safe stop without false completion: PASS (safety, **not** completion).
- Installed-release reports:
  - `tests/e2e/artifacts/installed-ledger-validation/run-20261125T194454480012Z/results.json`
  - `tests/e2e/artifacts/installed-ledger-validation/run-20261125T194655231714Z/results.json`
- Installed core: `%LOCALAPPDATA%\Browsphere\browsphere-core.exe`, SHA-256 `183B6C49A5E68BA959B404061A3D680A0C311D2C0C8827146A37C73F2913F324`.
- Latest installer: `dist/Browsphere-setup.exe`, SHA-256 `9995D0820918B456EBF1D5F2628DB04AC639A52A32D120FF6A90319A45A0B5AB`.
- Previous installed executable backed up at `logs/browsphere-core-before-ledger-fix-20261125.exe`. User data left in place.
- `Get-AuthenticodeSignature` still returns `NotSigned`; no trusted code-signing certificate with private key was available. Signing **cannot** be completed without acquiring/connecting a valid certificate or signing service.
- These are successful **bounded real-world benchmarks**, not a guarantee for all websites or production readiness. No GitHub push has been made.
