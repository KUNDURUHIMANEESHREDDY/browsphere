'use strict';
// Release configuration regression only. Real Authenticode and clean-machine
// tests are separate Windows jobs with a trusted certificate and installer.
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const root = path.resolve(__dirname, '../..');
const read = relative => fs.readFileSync(path.join(root, relative), 'utf8');

test('Windows NSIS disallows silent downgrade and has no phantom updater privileges', () => {
  const config = JSON.parse(read('src-tauri/tauri.conf.json'));
  const capabilities = JSON.parse(read('src-tauri/capabilities/default.json'));
  assert.equal(config.bundle.windows.allowDowngrades, false,
    'NSIS package must reject older versions over newer installations');
  assert.equal(config.bundle.targets.includes('nsis'), true);
  assert.equal(capabilities.permissions.includes('updater:default'), false,
    'No frontend updater permission without a verified signed-update channel');
  assert.equal(config.plugins?.updater, undefined,
    'Do not inject a placeholder, missing-pubkey updater configuration');
  const main = read('src-tauri/src/main.rs');
  assert.match(main, /Secure automatic updates are disabled until release signing is configured/);
  assert.doesNotMatch(main, /\.plugin\s*\(\s*tauri_plugin_updater::/s,
    'Enable the Tauri updater only after configuring a verified publisher key');
});

test('protected release workflow builds a natively signed embedded app, not only a wrapper', () => {
  const workflow = read('.github/workflows/release-signed-windows.yml');
  assert.match(workflow, /if: github\.ref == 'refs\/heads\/main'/);
  assert.match(workflow, /environment: windows-release/);
  assert.match(workflow, /WINDOWS_CERT_PFX_BASE64/);
  assert.match(workflow, /WINDOWS_CERT_PASSWORD/);
  assert.match(workflow, /Import-PfxCertificate/);
  assert.match(workflow, /certificateThumbprint\s*=\s*\$thumbprint/);
  assert.match(workflow, /digestAlgorithm\s*=\s*'sha256'/);
  assert.match(workflow, /--bundles nsis --config .*tauri\.signing\.release\.conf\.json/);
  assert.match(workflow, /Assert-TrustedSignature \$appExe/);
  assert.match(workflow, /Assert-TrustedSignature \$installer\.FullName/);
  assert.match(workflow, /Assert-TrustedSignature \$installed\[0\]\.FullName/);
  assert.match(workflow, /Get-AuthenticodeSignature/);
  assert.match(workflow, /Get-FileHash .*SHA256/);
  assert.match(workflow, /signed_inner_executable\s*=\s*\$true/);
  assert.match(workflow, /clean_uninstall\s*=\s*\$true/);
  assert.match(workflow, /if: \$\{\{ inputs\.publish_release \}\}/);
  assert.match(workflow, /Release tag already exists; refusing to overwrite/);
  assert.match(workflow, /Automatic updater is not yet enabled/);
});

test('unsigned development and release signing stay strictly separate', () => {
  const ci = read('.github/workflows/build-windows.yml');
  const release = read('.github/workflows/release-signed-windows.yml');
  assert.match(ci, /Browsphere-Windows-Installer[\s\S]*unsigned test artifact/);
  assert.doesNotMatch(ci, /gh release create/);
  assert.doesNotMatch(ci, /WINDOWS_CERT_PFX_BASE64/);
  assert.match(release, /workflow_dispatch:/);
  assert.doesNotMatch(release, /^\s*push:\s*$/m,
    'Release should never start from an unreviewed push');
});
