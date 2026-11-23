# Browsphere Voice Agent

Voice works alongside the existing text chat and policy-gated browsing agent.

## What works

- **Microphone icon** beside both start-page chat and floating Lucia: click to dictate and submit a query using the existing chat or agent form, with the same approvals. The microphone does not enable Voice Agent mode.
- **Voice Agent (waveform icon or central glowing orb)** independently enables spoken replies. The chat text remains visible, and Lucia can speak generated answers.
- **Simplified Lucia interface**: no separate Pause/Stop buttons, suggestion chips, letter-L logo, or visible Voice mode switch. The macOS-style popup includes the caption 'Your AI browsing partner.' and real, accessible microphone, voice settings, send, new-conversation and window controls.
- **Voice settings (⚙)**: ElevenLabs voice selection, secure API key, cloud speech consent, and Kokoro preference.
- **Fallbacks**: ElevenLabs → local Kokoro-FastAPI → Windows system voice. Choosing Kokoro goes to Kokoro before Windows.
- Voice mode is opt-in. Browser text prompts still work without speaking.
- **Private Browsing** skips ElevenLabs and does not send private speech text to the cloud; the separate Lucia popup inherits the main browser private setting.

## Set up ElevenLabs

1. Start Browsphere and open either chat.
2. Click the **gear icon** beside the microphone.
3. Choose **ElevenLabs first, then Kokoro**.
4. Enter an ElevenLabs *voice ID*, and paste your own API key into the password field.
5. Enable **Allow sending spoken replies to ElevenLabs (cloud)** if you consent to sending reply text to that service, and click **Save voice settings**. The key is stored in **Windows Credential Manager**, never in the JavaScript bundle or SQLite.
6. Click the **waveform button in Lucia's header** (or central glowing orb) to enable voice replies; click the **microphone** to record and submit a spoken query.

ElevenLabs usage is subject to the account's API allowance, subscription rules, and quota. Browsphere does not evade or reset credit limits. If an API request fails, the local fallback is attempted.

## Optional offline Kokoro

Browsphere calls the **Kokoro-FastAPI OpenAI-compatible** local endpoint:

    http://127.0.0.1:8880/v1/audio/speech

A separate Kokoro service and its model weights must be installed/running first. For example, using the project's publicly documented CPU container (requires Docker):

    docker run --rm -p 127.0.0.1:8880:8880 ghcr.io/remsky/kokoro-fastapi-cpu:latest

If no Kokoro service is running, the built-in Windows System.Speech voice is still available at no additional cost.

## Requirements and limitations

- Windows 10/11 + Tauri/WebView2 desktop build.
- Text chat needs a working configured AI model (Ollama or cloud model) to produce answers.
- Microphone recognition uses Windows speech recognition, with Web Speech recognition as a fallback when supported. Microphone access and recognition language depend on local OS/browser configuration.
- This is **push-to-talk**, not always-listening or full-duplex, wake-word conversation.
- Spoken replies are capped for latency and quota protection.
- No ElevenLabs API key was supplied for development: the authenticated cloud API and real voice synthesis have not been tested end-to-end.
- The local Kokoro service was not installed automatically. The fallback implementation is integrated but requires a running local Kokoro server.

## Validation

- Rust tests: 42 passed, 2 ignored (local-model-dependent).
- Redesigned Lucia layout: 8/8 Chromium UI checks at the actual 396px popup width, including independent waveform and microphone controls (no horizontal overflow).
- Previous native voice/Tauri and Private Browsing integration tests passed before the redesign.
- Latest JavaScript syntax checks: passed. Native Windows UI should be rechecked after restart because an extra test instance encountered an unrelated WebView2 launch-state error.

## Files

- Native voice API and secure-key commands: \`src-tauri/src/voice.rs\`
- Native command registration: \`src-tauri/src/main.rs\`
- SQLite config persistence: \`src-tauri/src/storage/sqlite_wal.rs\`
- Voice UI/controller: \`ui/public/src/voice-agent.js\`
- macOS-inspired Lucia design: \`ui/public/src/lucia-window.css\`
- Visual regression test and screenshot: \`tests/e2e/lucia-layout-smoke.mjs\`, \`tests/e2e/artifacts/lucia-final-396.png\`
- Chat integration: \`ui/public/index.html\` and \`ui/public/ai-chat.html\`
- E2E scripts: \`tests/e2e/voice-smoke.mjs\` and \`tests/e2e/voice-popup-smoke.mjs\`

For future repeatable WebView2 tests, start a *temporary development instance* with CDP debugging enabled locally. Don't leave the debugging port open after testing.


## Lucia voice-mode system prompt (added 22 November 2026)

The voice agent has a dedicated native **system prompt**, separate from TTS and speech recognition. It is defined in \`src-tauri/src/voice_system_prompt.rs\` as \`LUCIA_VOICE_CHAT_PROMPT\` and \`LUCIA_VOICE_AGENT_PROMPT\`.

- **Waveform-activated voice mode:** The dedicated waveform icon or central orb enables the voice agent system prompt and spoken replies; the microphone alone does not.
- **Text-only mode:** Until voice is activated, the frontend sends \`voiceMode: false\`; the standard chat/agent system instructions remain unchanged.
- **Provider handling:** Ollama chat and LiteLLM use a \`system\` message. Ollama agent uses the \`system\` field of \`/api/generate\`; Bedrock Converse uses the native \`--system\` parameter.
- **Content:** Natural, concise spoken replies; cautious interpretation of uncertain transcriptions; honest statement of task status; relevant context only when provided; no secrets spoken aloud; no bypass of approvals or untrusted webpage restrictions. The agent additionally retains its strict JSON action schema.
- **The prompt is not supplied from web pages or memory** and does not replace the browser's actual permission checks. Voice controls change the model's reply/planning guidance only, not the authority of a spoken command.

**Validation:** Rust tests check system-message construction and voice-only conditional injection. End-to-end responses from an actual configured model are still necessary to assess voice quality and model instruction-following.
