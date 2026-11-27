# Browsphere Windows release integrity gate

This document describes the existing upstream Tauri 2 release path. It is
**not** proof that a release was signed, that the updater works, or that Lucia
passes live tasks. The public `main` branch and user installations are unchanged
until reviewed PRs are merged and a protected release is explicitly run.

## Changes in this experimental branch

- Tauri 2's built-in NSIS `allowDowngrades` is set to `false`, so a
  lower-version installer cannot silently replace a newer installation.
- Remove the unused Tauri `updater:default` capability from the main/AI
  windows. The existing `check_for_updates` command intentionally reports
  updates disabled; an updater private key, public key and validated release
  manifest are not configured. No placeholder public key or insecure fallback.
- Keep the unsigned Windows CI path separate from the manually dispatched
  release workflow. It cannot publish or access signing certificate secrets.
- The existing protected `windows-release` job imports the provided code
  signing PFX into the ephemeral Windows certificate store (not source
  control) and passes a **temporary Tauri 2 signing config override** to the
  normal upstream `tauri-apps/tauri-action` NSIS build. The built-in Tauri
  signer signs the **application executable before bundling**.
- Independently verify SHA256 Authenticode of **(1)** built app executable,
  **(2)** bundled NSIS installer, and **(3)** freshly installed app
  executable, all with the same expected signing-certificate thumbprint.
- Silently install the candidate into a unique ephemeral directory and
  confirm its NSIS uninstaller succeeds before the signed artifact can be
  published. Preserve a private-data-free integrity JSON with artifact hashes
  and clear `updater_verified=false`. Signing files and imported cert are
  removed after the job. The workflow fails closed if PFX secrets are missing
  or no valid trusted signer is available.
- CI runs no-secret release configuration regressions on Windows and Ubuntu.
  These test configuration, **not an actual signed installer**.

## Evidence required before describing a signed release as verified

1. Merge reviewed source with all stacked dependencies resolved; require
   Windows NSIS build, installed-browser UI, real model, upstream MCP/security
   and process-metrics jobs to complete rather than just compile.
2. Configure a GitHub `windows-release` environment with restricted
   approvers and an authentic **code-signing** PFX in
   `WINDOWS_CERT_PFX_BASE64` and `WINDOWS_CERT_PASSWORD`.
3. Run the **manual** `Signed Windows installer release` action on trusted
   `main` with a version-matching tag. Set `publish_release=false` for the
   first fully audited validation.
4. Require successful Authenticode checks for both the installer and app
   inside a fresh installation; review the uploaded
   `Browsphere-Signed-Release-Integrity` JSON. Re-check startup, browser
   profile isolation, uninstall and signing-certificate chain on a truly
   clean, user-like Windows machine. GitHub hosted CI is not that machine.
5. Publish only after the original artifact hashes and signing thumbprint
   have been verified against the protected branch and successful acceptance
   runs. The workflow refuses to overwrite an existing release tag.

## Updater remains disabled

Tauri v2 uses **separate** cryptographic mechanisms: Windows Authenticode
for OS-level executable publisher verification, and Tauri updater signatures
with a trusted embedded public key to authenticate downloaded update bundles.
An Authenticode-signed `setup.exe` alone does NOT configure or validate the
Tauri updater. Before enabling it, provision a protected updater signing key,
audit a fixed HTTPS update endpoint and manifest, configure a verified public
key, exercise a full signed update and downgrade/rollback scenario on an
isolated installed app, and ensure the updater privilege is confined to
trusted application UI with explicit user intent. No update endpoint or
signing key is invented in this branch.

## Known limitations

The real-Qwen six-attempt unfamiliar-site result was previously 0/6 verified
and is independent of signing. Stagehand 4.5.0 fails its native WebView2
attachment probe; the supported upstream Playwright/native readback test must
remain separately green. A passed static release guard, a created NSIS setup,
and a valid Authenticode signature are each necessary evidence, but none alone
certifies a production-ready browser.
