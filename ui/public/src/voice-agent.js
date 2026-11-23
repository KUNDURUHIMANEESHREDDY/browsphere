/* Browsphere voice agent: opt-in spoken turns layered over existing text chat.
   ElevenLabs credentials never enter WebView JS except transiently in the
   user-filled password field; all HTTP speech generation happens in Rust. */
(function () {
  'use strict';
  const invoke = window.__TAURI__?.core?.invoke?.bind(window.__TAURI__.core) ||
    window.__TAURI_INTERNALS__?.invoke?.bind(window.__TAURI_INTERNALS__);
  const state = { voiceMode: false, listening: false, recording: null, audio: null, url: null, turn: 0, config: null };
  const isPrivate = () => document.documentElement.getAttribute('data-private') === 'true';
  const byId = id => document.getElementById(id);
  const status = str => {
    const el = byId('browsphereVoiceStatus');
    if (el) {
      el.textContent = str;
      el.dataset.idle = str === 'Ready' ? 'true' : 'false';
    }
  };
  const defaults = () => ({
    provider:'elevenlabs',voice_id:'JBFqnCBsd6RMkjVDRZzb',
    model_id:'eleven_flash_v2_5',allow_cloud_tts:false,speak_replies:true
  });
  async function loadConfig() {
    if (!invoke) return defaults();
    try { state.config = await invoke('get_voice_config'); }
    catch (_) { state.config = defaults(); }
    return state.config;
  }
  async function stop() {
    ++state.turn;
    if (state.audio) { try { state.audio.pause(); state.audio.currentTime = 0; } catch (_) {} state.audio = null; }
    if (state.url) { URL.revokeObjectURL(state.url); state.url = null; }
    if (window.speechSynthesis) window.speechSynthesis.cancel();
    if (invoke) await invoke('stop_speaking').catch(() => {});
    status('Ready');
  }
  function bytesFromBase64(data) {
    const binary = atob(data); const bytes = new Uint8Array(binary.length);
    for (let i=0; i<binary.length; i++) bytes[i] = binary.charCodeAt(i);
    return bytes;
  }
  async function speak(message, force=false) {
    if ((!state.voiceMode && !force) || state.listening) return false;
    let text = String(message || '').replace(/<[^>]+>/g,' ').replace(/\s+/g,' ').trim().slice(0,850);
    if (!text) return false;
    const stopping = stop();
    const current = state.turn;
    await stopping;
    if (current !== state.turn) return false;
    const cfg=await loadConfig();
    if (current !== state.turn) return false;
    if (!cfg.speak_replies && !force) return false;
    status('Generating voice…');
    if (invoke) {
      try {
        const output = await invoke('synthesize_voice', { text, isPrivate:isPrivate() });
        if (current !== state.turn) return false;
        const audioBytes=bytesFromBase64(output.audio_base64);
        const url=URL.createObjectURL(new Blob([audioBytes],{type:output.mime_type||'audio/mpeg'}));
        state.url=url;
        const audio=new Audio(url);
        state.audio=audio;
        audio.onended=()=>{ if(current===state.turn){status('Ready');URL.revokeObjectURL(url);state.url=null;state.audio=null;} };
        audio.onerror=()=>status('Voice playback failed');
        await audio.play();
        status('Speaking · '+(output.provider==='elevenlabs'?'ElevenLabs':'Kokoro'));
        return true;
      } catch (e) {
        if (current !== state.turn) return false;
        // No cloud key or Kokoro server? Use Windows SAPI.
        try {
          await invoke('speak_text',{text,rate:0});
          status('Speaking · Windows offline voice');
          return true;
        } catch (_) {}
      }
    }
    if (window.speechSynthesis && typeof SpeechSynthesisUtterance!=='undefined') {
      let u=new SpeechSynthesisUtterance(text);
      u.onend=()=>{ if(current===state.turn)status('Ready'); };
      window.speechSynthesis.speak(u);
      status('Speaking · System voice');
      return true;
    }
    status('Speech output is unavailable');return false;
  }
  function toggleMode(on) {
    state.voiceMode = typeof on === 'boolean' ? on : !state.voiceMode;
    // Voice Agent (waveform) is independent from the microphone query button.
    for (const el of document.querySelectorAll('[data-browsphere-voice-agent]')) {
      el.setAttribute('aria-pressed', String(state.voiceMode));
      el.setAttribute('aria-label', state.voiceMode ? 'Turn off Lucia Voice Agent' : 'Start Lucia Voice Agent');
      el.title = state.voiceMode ? 'Voice Agent on · click to turn off' : 'Voice Agent · click to enable spoken replies';
    }
    if (!state.voiceMode) {
      void stop().finally(() => status('Voice Agent off'));
    } else {
      status('Voice Agent on · replies aloud');
    }
    return state.voiceMode;
  }
  function finishListening(text, target) {
    state.listening=false;
    for (const btn of document.querySelectorAll('[data-browsphere-voice-talk]')) {
      btn.disabled = !!btn.form?.querySelector('button[type="submit"]')?.disabled;
      btn.setAttribute('aria-busy', 'false');
      btn.setAttribute('aria-label', 'Dictate and send a query');
    }
    const transcript=String(text||'').trim();
    if (!transcript) {status('No speech detected. Try again.');return;}
    const form = target;
    if (!form?.isConnected) {
      status('Chat changed. Please dictate again.');
      return;
    }
    const input=form.querySelector('input:not([type=hidden]), textarea');
    if (!input || input.disabled) return;
    input.value=transcript;
    status('You said: '+transcript.slice(0,75));
    // Use the exact existing chat/agent submission path and its policy approvals.
    form.requestSubmit();
  }
  async function listenAndSend(form) {
    if (state.listening) return;
    state.listening = true;
    await stop();
    // A microphone press only transcribes and submits a query; it never enables voice mode.
    status('Listening for a query…');
    for (const b of document.querySelectorAll('[data-browsphere-voice-talk]')) {
      b.disabled = true;
      b.setAttribute('aria-busy','true');
      b.setAttribute('aria-label','Listening for a query');
    }
    const Recognition=window.SpeechRecognition||window.webkitSpeechRecognition;
    if (invoke) {
      try {
        const transcript=await invoke('listen_speech_once',{timeoutSecs:8});
        if (transcript) { finishListening(transcript,form); return; }
      } catch (e) {
        status('Windows recognizer unavailable; checking browser speech input');
      }
      // Some Windows installations have no configured SAPI recognition grammar.
      // Try the browser recognizer if available before declaring silence.
      if (!Recognition) { finishListening('',form); return; }
    }
    if (Recognition) {
      const rec=new Recognition();state.recording=rec;
      rec.lang='en-US';rec.interimResults=false;rec.maxAlternatives=1;
      let transcript='';
      rec.onresult=e=>{ transcript=e.results?.[0]?.[0]?.transcript||''; };
      rec.onend=()=>{state.recording=null;finishListening(transcript,form);};
      rec.onerror=e=>{status('Microphone: '+e.error);};
      try {rec.start();return;}catch(e){}
    }
    finishListening('',form);
    status('Speech recognition requires Windows or browser microphone support');
  }
  let dialog=null;
  function makeDialog() {
    if (dialog) return dialog;
    dialog=document.createElement('div');
    dialog.id='browsphereVoiceSettings';
    dialog.style.cssText='position:fixed;inset:0;background:rgba(0,0,0,.35);z-index:100000;display:none;align-items:center;justify-content:center;padding:20px';
    dialog.innerHTML='<section role="dialog" aria-modal="true" aria-label="Voice settings" style="background:#fff;color:#1d1d1f;width:min(400px,100%);padding:20px;border-radius:16px;font:13px system-ui;box-shadow:0 20px 65px #0004;max-height:85vh;overflow:auto">' +
      '<div style="display:flex;justify-content:space-between;align-items:center"><strong style="font-size:17px">Lucia Voice</strong><button type="button" id="bvClose" aria-label="Close">✕</button></div>'+
      '<p style="font-size:12px;color:#555;line-height:1.5">ElevenLabs is optional. Only enable cloud speech if you agree that the spoken response text is sent to ElevenLabs. API keys are stored in Windows Credential Manager.</p>'+
      '<label>Speech engine <select id="bvProvider" style="width:100%;padding:8px;margin:5px 0 10px"><option value="elevenlabs">ElevenLabs first, then Kokoro</option><option value="kokoro">Local Kokoro first</option></select></label>'+
      '<label>ElevenLabs voice ID <input id="bvVoiceId" maxlength="64" style="display:block;width:100%;padding:8px;margin:5px 0 10px"/></label>'+
      '<label>ElevenLabs API key <input id="bvKey" type="password" placeholder="Paste API key (optional)" autocomplete="off" style="display:block;width:100%;padding:8px;margin:5px 0 4px"/></label>'+
      '<span id="bvKeyStatus" style="font-size:11px;color:#666"></span>'+
      '<label style="display:block;margin:12px 0"><input type="checkbox" id="bvConsent"/> Allow sending spoken replies to ElevenLabs (cloud)</label>'+
      '<label style="display:block;margin:10px 0"><input type="checkbox" id="bvSpeak" checked/> Speak assistant replies when Voice is on</label>'+
      '<p id="bvInfo" role="status" style="min-height:18px;color:#444"></p>'+
      '<div style="display:flex;gap:8px;justify-content:space-between"><button type="button" id="bvRemove" style="color:#c33">Remove key</button><button type="button" id="bvSave" style="padding:8px 16px;background:#007AFF;color:#fff;border-radius:8px">Save voice settings</button></div>'+
      '<p style="font-size:11px;color:#666;margin-top:15px">Offline fallback: run Kokoro-FastAPI on 127.0.0.1:8880, otherwise Windows system speech is used.</p>'+
      '</section>';
    document.body.appendChild(dialog);
    dialog.querySelector('#bvClose').onclick=()=>dialog.style.display='none';
    dialog.addEventListener('click',e=>{if(e.target===dialog)dialog.style.display='none';});
    const $=id=>dialog.querySelector('#'+id);
    $('bvSave').onclick=async()=>{
      if (!invoke) { $('bvInfo').textContent='Open the Windows application to configure voice.';return; }
      try {
        let cfg=await loadConfig();
        cfg={...cfg,provider:$('bvProvider').value,voice_id:$('bvVoiceId').value.trim(),
          allow_cloud_tts:$('bvConsent').checked,speak_replies:$('bvSpeak').checked};
        await invoke('update_voice_config',{config:cfg});
        const key=$('bvKey').value.trim();
        if(key){await invoke('set_voice_api_key',{apiKey:key});$('bvKey').value='';}
        state.config=cfg;
        $('bvInfo').textContent='Voice settings saved.';
        $('bvKeyStatus').textContent=(await invoke('has_voice_api_key'))?'API key saved securely':'No API key';
      } catch(err){$('bvInfo').textContent=String(err);}
    };
    $('bvRemove').onclick=async()=>{if(invoke){try{await invoke('clear_voice_api_key');$('bvKeyStatus').textContent='No API key';$('bvKey').value='';$('bvInfo').textContent='Key removed.';}catch(err){$('bvInfo').textContent=String(err);}}};
    return dialog;
  }
  async function openSettings() {
    const d=makeDialog(),$=id=>d.querySelector('#'+id);
    let cfg=await loadConfig();
    $('bvProvider').value=cfg.provider;
    $('bvVoiceId').value=cfg.voice_id;
    $('bvConsent').checked=!!cfg.allow_cloud_tts;
    $('bvSpeak').checked=cfg.speak_replies!==false;
    $('bvKeyStatus').textContent=invoke?(await invoke('has_voice_api_key').catch(()=>false)?'API key saved securely':'No API key'):'Native app required';
    $('bvInfo').textContent='';
    d.style.display='flex';
  }
  const microphoneSvg = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" style="width:20px;height:20px"><rect x="9" y="2.5" width="6" height="12" rx="3"/><path d="M5.5 10.5v2a6.5 6.5 0 0 0 13 0v-2M12 19v3M8 22h8"/></svg>';
  const waveformSvg = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" aria-hidden="true" style="width:19px;height:19px"><path d="M2 10v4M6 7v10M10 3v18M14 6v12M18 9v6M22 11v2"/></svg>';
  const gearSvg = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" style="width:17px;height:17px"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a8 8 0 0 0 .1-6l1.1-1.1-2.5-2.5L17 6.5a8 8 0 0 0-4-1.6L12.5 3h-3L9 4.9a8 8 0 0 0-3 1.6L4.6 5.4 2.2 7.9 3.4 9a8 8 0 0 0 0 6l-1.2 1.1 2.4 2.5L6 17.5a8 8 0 0 0 3 1.6l.5 1.9h3l.5-1.9a8 8 0 0 0 4-1.6l1.1 1.1 2.5-2.5Z"/></svg>';

  function bindVoiceButtons(form, talk, settings) {
    if (talk && !talk.dataset.voiceBound) {
      talk.dataset.voiceBound = 'true';
      talk.dataset.browsphereVoiceTalk = '';
      talk.setAttribute('aria-label', 'Dictate and send a query');
      talk.title = 'Dictate and send a query';
      talk.addEventListener('click', () => listenAndSend(form));
    }
    if (settings && !settings.dataset.voiceBound) {
      settings.dataset.voiceBound = 'true';
      settings.addEventListener('click', openSettings);
    }
  }
  function bindAgentButton(el) {
    if (!el || el.dataset.voiceAgentBound) return;
    el.dataset.voiceAgentBound = 'true';
    el.setAttribute('aria-pressed', String(state.voiceMode));
    el.addEventListener('click', () => toggleMode());
  }
  function installButtons() {
    const popupForm = byId('chatComposerForm');
    if (popupForm) {
      // Distinct controls: waveform = Voice Agent mode; mic = send spoken query.
      bindVoiceButtons(popupForm, byId('btnVoice'), byId('btnVoiceSettings'));
      bindAgentButton(byId('btnVoiceAgent'));
      bindAgentButton(byId('btnVoiceOrb'));
    }
    const mainForm = byId('newTabChatForm');
    if (mainForm && !mainForm.querySelector('[data-browsphere-voice-talk]')) {
      // Match the icon-only UI when the main start-page chat is re-rendered.
      const mic = document.createElement('button');
      mic.type='button'; mic.title='Dictate and send a query';
      mic.innerHTML=microphoneSvg;
      mic.style.cssText='border-radius:999px;width:34px;height:34px;display:flex;align-items:center;justify-content:center;flex-shrink:0;background:rgba(113,133,230,.08);color:#5668a2;cursor:pointer;';
      const agent=document.createElement('button');
      agent.type='button';
      agent.dataset.browsphereVoiceAgent='';
      agent.title='Start Lucia Voice Agent';
      agent.setAttribute('aria-label','Start Lucia Voice Agent');
      agent.innerHTML=waveformSvg;
      agent.style.cssText='border-radius:11px;width:34px;height:34px;display:flex;align-items:center;justify-content:center;flex-shrink:0;background:rgba(119,112,231,.09);color:#7773cf;cursor:pointer;';
      const gear=document.createElement('button');
      gear.type='button'; gear.title='Voice settings';gear.setAttribute('aria-label','Voice settings');
      gear.innerHTML=gearSvg;
      gear.style.cssText='border-radius:9px;width:30px;height:30px;display:flex;align-items:center;justify-content:center;flex-shrink:0;color:#818ba3;cursor:pointer;';
      const submit=mainForm.querySelector('button[type="submit"]');
      if (submit) {
        submit.before(gear, agent, mic);
        mic.disabled = submit.disabled || state.listening;
        bindVoiceButtons(mainForm,mic,gear);
        bindAgentButton(agent);
      }
    }
  }
  window.addEventListener('browsphere:assistant-text',e=>{
    if(!state.voiceMode)return;
    const message=String(e.detail?.text||'');
    speak(message);
  });
  function boot(){
    installButtons();
    const parent=byId('pageViewport');
    if(parent){new MutationObserver(()=>installButtons()).observe(parent,{childList:true,subtree:true});}
    loadConfig();
  }
  if (document.readyState==='loading') document.addEventListener('DOMContentLoaded',boot,{once:true});else boot();
  window.browsphereVoice={ speak,stop,toggleMode,listenAndSend,openSettings, get enabled(){return state.voiceMode;} };
})();