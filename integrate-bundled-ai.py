from pathlib import Path
root=Path(__file__).resolve().parent
def replace_once(s,a,b):
    if s.count(a)!=1: raise RuntimeError(f'Expected unique anchor: {a[:90]} ({s.count(a)})')
    return s.replace(a,b,1)
main=root/'src-tauri/src/main.rs'
router=root/'src-tauri/src/router/model_router.rs'
m=main.read_text(encoding='utf-8')
r=router.read_text(encoding='utf-8')
m=replace_once(m,'mod router;','mod router;\nmod bundled_ai;')
m=replace_once(m,'        .setup(move |app| {','''        .setup(move |app| {
            let resource_root = app.path().resource_dir()?;
            let private_ai = tauri::async_runtime::block_on(bundled_ai::BundledAi::launch(&resource_root))?;
            if let Some(ref service) = private_ai {
                tauri::async_runtime::block_on(async {
                    state_for_sched.router.lock().await.enable_bundled(service.endpoint.clone());
                });
                tracing::info!("Bundled local AI is ready");
            }
            app.manage(std::sync::Mutex::new(private_ai));''')
m=replace_once(m,'        .run(tauri::generate_context!()) {','''        .build(tauri::generate_context!()).map(|app| {
            app.run(|handle, event| {
                if let tauri::RunEvent::Exit = event {
                    if let Some(service) = handle.try_state::<std::sync::Mutex<Option<bundled_ai::BundledAi>>>() {
                        if let Ok(mut service) = service.lock() { service.take(); }
                    }
                }
            });
        }) {''')
m=replace_once(m,'    let router = state.router.lock().await.get_config();\n    let memories','    let router = state.router.lock().await.persistable_config();\n    let memories')
m=replace_once(m,'        db.save_router_config(&config).map_err(|e| e.to_string())?;', '        let stored = state.router.lock().await.normalize_for_storage(config.clone());\n        db.save_router_config(&stored).map_err(|e| e.to_string())?;')
# Avoid holding database lock while awaiting router lock.
m=replace_once(m,'        let mut db = state.db.lock().await;\n        let stored = state.router.lock().await.normalize_for_storage(config.clone());','        let stored = state.router.lock().await.normalize_for_storage(config.clone());\n        let mut db = state.db.lock().await;')
m=replace_once(m,'    state.db.lock().await.save_router_config(&config).map_err(|e| e.to_string())?;','    let stored = state.router.lock().await.normalize_for_storage(config.clone());\n    state.db.lock().await.save_router_config(&stored).map_err(|e| e.to_string())?;')
r=replace_once(r,'    cloud_cooldowns: Arc<Mutex<HashMap<String, Instant>>>,','    cloud_cooldowns: Arc<Mutex<HashMap<String, Instant>>>,\n    bundled_endpoint: Option<String>,')
r=replace_once(r,'            cloud_cooldowns: Arc::new(Mutex::new(HashMap::new())),','            cloud_cooldowns: Arc::new(Mutex::new(HashMap::new())),\n            bundled_endpoint: None,')
r=replace_once(r,'    pub fn update_config(&mut self, config: RouterConfig) {\n        self.config = config;\n    }','''    pub fn update_config(&mut self, mut config: RouterConfig) {
        if config.ollama_url == "http://127.0.0.1:11434" {
            if let Some(endpoint) = &self.bundled_endpoint { config.ollama_url = endpoint.clone(); }
        }
        self.config = config;
    }

    pub fn enable_bundled(&mut self, endpoint: String) {
        self.bundled_endpoint = Some(endpoint.clone());
        self.config.ollama_url = endpoint;
        self.config.ollama_model = crate::bundled_ai::FAST.into();
        self.config.supervisor_model = crate::bundled_ai::PLAN.into();
        self.config.vision_model = crate::bundled_ai::VISION.into();
        self.config.prefer_local_for_simple = true;
        self.config.lucia_cloud_only = false;
        self.config.domain_overrides.clear();
    }

    pub fn normalize_for_storage(&self, mut config: RouterConfig) -> RouterConfig {
        if self.bundled_endpoint.as_deref() == Some(config.ollama_url.as_str()) {
            config.ollama_url = "http://127.0.0.1:11434".into();
        }
        config.litellm_api_key = None;
        config
    }

    pub fn persistable_config(&self) -> RouterConfig {
        self.normalize_for_storage(self.get_config())
    }

    fn structured_local_model(&self) -> &str {
        if self.bundled_endpoint.as_deref() == Some(self.config.ollama_url.as_str())
            && self.config.ollama_model == crate::bundled_ai::FAST {
            crate::bundled_ai::PLAN
        } else {
            &self.config.ollama_model
        }
    }''')
start=r.index('    async fn model_structured_response(')
end=r.index('\n    }',start)+6
section=r[start:end]
section=replace_once(section,'"model": self.config.ollama_model,','"model": self.structured_local_model(),')
r=r[:start]+section+r[end:]
main.write_text(m,encoding='utf-8')
router.write_text(r,encoding='utf-8')
print('Integrated private runtime lifecycle, local default routing, planning model and storage normalization')
