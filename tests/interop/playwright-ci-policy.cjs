'use strict';

// Only an ephemeral Windows CI test may attach to WebView2's debug port.
// This is not a public API, not a runtime agent tool, and never enables
// remote debugging for end users.
function localCdpEndpoint(portText, githubActions) {
  if (githubActions !== 'true') {
    throw Error('Playwright CDP testing is limited to disposable GitHub Actions runners');
  }
  const port = Number(portText);
  if (!Number.isInteger(port) || port < 1024 || port > 65535) {
    throw Error('Expected a valid isolated CI CDP port');
  }
  return 'http://127.0.0.1:' + port;
}
module.exports = { localCdpEndpoint };
