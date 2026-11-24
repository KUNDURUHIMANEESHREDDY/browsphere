# Optional three-model offline edition

The installer resource contains three pinned, unmodified Ollama model stores:
Qwen 3.5 0.8B (lightweight reasoning), LFM3 1.7B (planning), and LFM3-VL
2B (image perception). Every referenced layer is size- and SHA-256-checked;
shared blobs are deduplicated, and each model license is included.
The complete model resource is approximately 3.90 GB (decimal).

Qwen carries its supplied Apache 2.0 license. Both Liquid AI models carry
the LFM Open License v1.0, including its USD 10 million annual revenue
commercial-use threshold. Review the included terms before redistribution.

From the repository root on Windows, with the pinned models already installed:

```powershell
powershell -NoProfile -File .\prepare-bundled-model.ps1
powershell -NoProfile -File .\test-bundled-model.ps1
```

The test starts an isolated loopback Ollama service using only the staged
store, discovers all three models, runs text generation and checks the
vision model against a generated image. It limits loaded models to one and
requests unloading after each inference; it does not modify saved settings.

To build the optional installer:

```powershell
powershell -NoProfile -File .\prepare-bundled-model.ps1 -Build
```

Large binaries remain outside Git. The standard build remains unchanged.
This packages model weights, not a standalone inference runtime. Automatic
resource discovery, runtime lifecycle integration, installed-app testing
and release verification remain separate gates.
