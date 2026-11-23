# Browsphere — Signed Windows release guide

**Current status:** A manual, signing-gated GitHub Actions workflow exists, but no
production signing certificate or GitHub release has been created by this change.
The application **does not auto-update** until a separately verified Tauri updater
configuration and publisher public key are installed.

## One-time preparation

1. Obtain a legitimate Windows Authenticode code-signing certificate issued to the
   actual publisher. Use a provider and key-storage method appropriate to your
   organization. Never commit a certificate, password, or private key to Git.
2. In GitHub repository **Settings → Environments**, create an environment named
   `windows-release`. Protect it with required reviewers and restrict deployment
   branches to the trusted `main` branch.
3. Create two **environment secrets**, rather than repository files:
   - `WINDOWS_CERT_PFX_BASE64`: the base64 encoding of the *binary PFX certificate file*.
   - `WINDOWS_CERT_PASSWORD`: password for that certificate.
   Treat both as secrets. Do not paste either into an issue, chat, README or workflow log.
4. Confirm that `src-tauri/tauri.conf.json` has the intended product
   version (for example, `0.1.0`) and that changes are merged into `main`.
5. Trigger **Actions → Signed Windows installer release → Run workflow** from
   `main`, entering exactly `v` plus the app version. Leave
   `publish_release` unchecked for the first signing verification.

## Signing and publishing gates

The workflow checks its tag and secret configuration, runs the JavaScript and Rust
checks, builds a Windows NSIS installer, signs it using Windows SDK `signtool`,
verifies the Authenticode signature, then uploads the **verified signed**
installer. It deletes the temporary PFX file. If any signing or verification step
fails, no installer is uploaded and no GitHub release is published.

For subsequent release runs, check `publish_release` only after reviewing the
installed Windows binary on a clean, supported Windows machine. The release step
refuses to overwrite an existing release tag.

Do not treat the presence of an executable alone as code-signing evidence. On
Windows, inspect **Properties → Digital Signatures** or verify with:
`Get-AuthenticodeSignature .\Browsphere-setup.exe`.

## Not included in this milestone

- A trusted certificate, private key or certificate provisioning.
- Signed **Tauri auto-updater manifests** or seamless in-app updates. Tauri
  updater signatures use a **separate** private/public signing key system; a
  Windows Authenticode certificate alone does not configure it.
- Automatic publishing, notarization, SmartScreen reputation, or end-user
  deployment.
- Permission prompts for every WebView2 permission kind. Only Camera,
  Microphone, Location and Notifications are manageable as explicit per-origin
  persistent choices for normal HTTPS tabs. Other kinds are denied. Private
  tabs deny all site permissions regardless of stored normal-tab grants.

## Security and compatibility limitations

Site policy grants are keyed by the top-level WebView2 HTTPS origin. Tauri's
high-level permission callback does not expose a separate requesting iframe
origin. **Do not assume these controls implement full frame-aware permission
protection**; inspect WebView2's lower-level request-origin API before a public
security-sensitive release. Grants are written under the user's local
`BrowsphereAI` application-data folder and can be revoked using the lock
icon. Browser popups are denied at creation time, then presented for an
explicit **Open once** decision; this can disrupt SSO popup flows until a
reliable user-gesture-preserving popup path is implemented.

Run the desktop compatibility suite from
`tests/e2e/browser-compat.mjs` with a separately launched test instance
configured for CDP port 9240. Never enable remote debugging on your daily
browser profile.
