use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tauri::Emitter;
use tokio::sync::{oneshot, Mutex};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::agent::action_executor::execute_browser_action;
use crate::agent::dom_extractor::{request_live_dom_snapshot, DomSnapshot};
use crate::agent::loop_runner::ApprovalRequest;
use crate::agent::policy::{evaluate_policy, BrowserAction, PolicyDecision, RiskLevel, TraceEntry};
use crate::privacy::downloads::DownloadManager;
use crate::privacy::itp::ItpEngine;
use crate::router::model_router::ModelRouter;
use crate::storage::credentials::CredentialStore;
use crate::storage::sqlite_wal::BrowserDatabase;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AutomationStep {
    /// Browser action routed through the same PolicyGate & active child WebView2
    Browser {
        action: BrowserAction,
        #[serde(default)]
        allowed_domains: Vec<String>,
    },
    /// Non-intrusive background HTTP / API request (via reqwest)
    HttpRequest {
        method: String,
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
        body: Option<String>,
    },
    /// Local filesystem / workspace file write (sandboxed to user's Browsphere Automations folder or explicit path with approval)
    LocalWriteFile {
        filename: String,
        content: String,
        #[serde(default)]
        append: bool,
    },
    /// Local command execution on Windows (cmd /C or powershell) gated by PolicyGate approval
    LocalCommand {
        command: String,
        working_dir: Option<String>,
    },
    /// LLM prompt step routed through Ollama (qwen3.5:4b) or LiteLLM
    LlmPrompt {
        prompt: String,
        origin_hint: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationJob {
    pub id: String,
    pub name: String,
    pub trigger: String, // "manual" | "interval"
    pub interval_secs: Option<u64>,
    pub enabled: bool,
    pub steps: Vec<AutomationStep>,
    pub last_status: String,
    pub last_run_at: Option<String>,
    pub run_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationStepLog {
    pub job_id: String,
    pub step_index: usize,
    pub step_kind: String,
    pub risk: RiskLevel,
    pub decision: PolicyDecision,
    pub ok: bool,
    pub detail: String,
}

pub struct AutomationEngine {
    automations_dir: PathBuf,
    active_runs: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

impl AutomationEngine {
    pub fn new(data_dir: &std::path::Path) -> Self {
        let automations_dir = data_dir.join("automations");
        let _ = std::fs::create_dir_all(&automations_dir);
        Self {
            automations_dir,
            active_runs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn automations_dir(&self) -> &std::path::Path {
        &self.automations_dir
    }

    pub fn evaluate_step_policy(step: &AutomationStep) -> (RiskLevel, PolicyDecision) {
        match step {
            AutomationStep::Browser {
                action,
                allowed_domains,
            } => evaluate_policy("https://automation.local", action, allowed_domains),
            AutomationStep::LlmPrompt { .. } => (RiskLevel::Low, PolicyDecision::Allow),
            AutomationStep::HttpRequest { method, url, .. } => {
                let m = method.trim().to_uppercase();
                if m == "GET" || m == "HEAD" {
                    (RiskLevel::Low, PolicyDecision::Allow)
                } else {
                    (
                        RiskLevel::High,
                        PolicyDecision::RequireApproval {
                            reason: format!("Outbound {m} API request to '{url}'"),
                        },
                    )
                }
            }
            AutomationStep::LocalWriteFile { filename, .. } => (
                RiskLevel::Medium,
                PolicyDecision::RequireApproval {
                    reason: format!("Write local automation artifact '{filename}'"),
                },
            ),
            AutomationStep::LocalCommand { command, .. } => (
                RiskLevel::Critical,
                PolicyDecision::RequireApproval {
                    reason: format!("Execute local command: '{command}'"),
                },
            ),
        }
    }

    fn resolve_safe_output_path(&self, filename: &str) -> PathBuf {
        let clean: String = filename
            .chars()
            .filter(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | '_'))
            .collect();
        let safe_name = if clean.is_empty() || clean == "." || clean == ".." {
            "automation-output.txt".to_string()
        } else {
            clean
        };
        self.automations_dir.join(safe_name)
    }

    #[allow(clippy::too_many_arguments)]
    async fn execute_single_step(
        &self,
        app: &tauri::AppHandle,
        run_id: Uuid,
        job: &AutomationJob,
        step_index: usize,
        step: &AutomationStep,
        router: &Arc<Mutex<ModelRouter>>,
        creds: &Arc<CredentialStore>,
        itp: &Arc<ItpEngine>,
        downloads: &Arc<DownloadManager>,
        db: &Arc<Mutex<BrowserDatabase>>,
        webview_label: &str,
        active_url: &Arc<Mutex<String>>,
        pending_snapshots: &Arc<Mutex<HashMap<Uuid, oneshot::Sender<DomSnapshot>>>>,
        pending_action_results: &Arc<Mutex<HashMap<Uuid, oneshot::Sender<crate::agent::action_executor::ActionResult>>>>,
        approval_tx: &tokio::sync::mpsc::Sender<ApprovalRequest>,
    ) -> (bool, String, String, RiskLevel, PolicyDecision) {
        let (risk, decision) = Self::evaluate_step_policy(step);
        let trace_id = Uuid::new_v4();

        let step_kind = match step {
            AutomationStep::Browser { .. } => "browser",
            AutomationStep::HttpRequest { .. } => "http",
            AutomationStep::LocalWriteFile { .. } => "local_file",
            AutomationStep::LocalCommand { .. } => "local_cmd",
            AutomationStep::LlmPrompt { .. } => "llm",
        }
        .to_string();

        let approved = match &decision {
            PolicyDecision::Deny { reason } => {
                return (
                    false,
                    step_kind,
                    format!("DENIED: {reason}"),
                    risk,
                    decision,
                );
            }
            PolicyDecision::RequireApproval { reason } => {
                let approval_id = Uuid::new_v4();
                let (res_tx, res_rx) = oneshot::channel();
                let req = ApprovalRequest {
                    approval_id,
                    run_id,
                    trace_id,
                    action: BrowserAction::Complete {
                        summary: format!("[Automation: {}] {}", job.name, reason),
                    },
                    risk: risk.clone(),
                    reason: format!("[Automation: {}] {}", job.name, reason),
                    responder: res_tx,
                };
                if approval_tx.send(req).await.is_err() {
                    return (
                        false,
                        step_kind,
                        "ABORTED: approval channel closed".to_string(),
                        risk,
                        decision,
                    );
                }
                res_rx.await.unwrap_or(false)
            }
            PolicyDecision::Allow => true,
        };

        if !approved {
            return (
                false,
                step_kind,
                "REJECTED BY USER".to_string(),
                risk,
                decision,
            );
        }

        match step {
            AutomationStep::Browser { action, .. } => {
                let res = execute_browser_action(
                    app,
                    webview_label,
                    active_url,
                    pending_action_results,
                    itp,
                    downloads,
                    db,
                    action,
                )
                .await;
                (res.success, step_kind, res.detail, risk, decision)
            }
            AutomationStep::HttpRequest {
                method,
                url,
                headers,
                body,
            } => {
                let client = reqwest::Client::new();
                let m = reqwest::Method::from_bytes(method.trim().as_bytes())
                    .unwrap_or(reqwest::Method::GET);
                let mut builder = client.request(m, url);
                for (k, v) in headers {
                    builder = builder.header(k, v);
                }
                if let Some(payload) = body {
                    builder = builder.body(payload.clone());
                }
                match builder.send().await {
                    Ok(resp) => {
                        let status = resp.status();
                        let text = resp.text().await.unwrap_or_default();
                        let snippet: String = text.chars().take(400).collect();
                        (
                            status.is_success(),
                            step_kind,
                            format!("HTTP {} -> {}", status.as_u16(), snippet.trim()),
                            risk,
                            decision,
                        )
                    }
                    Err(e) => (
                        false,
                        step_kind,
                        format!("HTTP request failed: {e}"),
                        risk,
                        decision,
                    ),
                }
            }
            AutomationStep::LocalWriteFile {
                filename,
                content,
                append,
            } => {
                let out_path = self.resolve_safe_output_path(filename);
                let write_res = if *append {
                    use std::io::Write;
                    std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&out_path)
                        .and_then(|mut f| writeln!(f, "{content}"))
                } else {
                    std::fs::write(&out_path, content.as_bytes())
                };
                match write_res {
                    Ok(()) => (
                        true,
                        step_kind,
                        format!("Wrote {}", out_path.to_string_lossy()),
                        risk,
                        decision,
                    ),
                    Err(e) => (
                        false,
                        step_kind,
                        format!("File write error: {e}"),
                        risk,
                        decision,
                    ),
                }
            }
            AutomationStep::LocalCommand {
                command,
                working_dir,
            } => {
                let cmd_owned = command.clone();
                let cwd_owned = working_dir
                    .clone()
                    .map(PathBuf::from)
                    .unwrap_or_else(|| self.automations_dir.clone());

                let spawn_res = tokio::task::spawn_blocking(move || {
                    #[cfg(target_os = "windows")]
                    let mut cmd = {
                        let mut c = std::process::Command::new("cmd");
                        c.args(["/C", &cmd_owned]);
                        c
                    };
                    #[cfg(not(target_os = "windows"))]
                    let mut cmd = {
                        let mut c = std::process::Command::new("sh");
                        c.args(["-c", &cmd_owned]);
                        c
                    };
                    cmd.current_dir(cwd_owned);
                    cmd.output()
                })
                .await;

                match spawn_res {
                    Ok(Ok(out)) => {
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        let combined = if stdout.trim().is_empty() {
                            stderr.trim().to_string()
                        } else {
                            stdout.trim().to_string()
                        };
                        let trimmed: String = combined.chars().take(400).collect();
                        (
                            out.status.success(),
                            step_kind,
                            format!(
                                "Exit {} | {}",
                                out.status.code().unwrap_or(-1),
                                trimmed
                            ),
                            risk,
                            decision,
                        )
                    }
                    Ok(Err(e)) => (
                        false,
                        step_kind,
                        format!("Command spawn failed: {e}"),
                        risk,
                        decision,
                    ),
                    Err(e) => (
                        false,
                        step_kind,
                        format!("Command join error: {e}"),
                        risk,
                        decision,
                    ),
                }
            }
            AutomationStep::LlmPrompt {
                prompt,
                origin_hint,
            } => {
                let origin = origin_hint
                    .clone()
                    .unwrap_or_else(|| "https://automation.local".to_string());
                let live_ctx = request_live_dom_snapshot(app, webview_label, pending_snapshots, 1200)
                    .await
                    .map(|s| format!("Page title: {}\nExcerpt: {}", s.title, s.text_excerpt))
                    .unwrap_or_default();
                let full_prompt = if live_ctx.is_empty() {
                    prompt.clone()
                } else {
                    format!("{prompt}\n\nActive Tab Context:\n{live_ctx}")
                };

                let proposed = {
                    let r = router.lock().await;
                    r.propose_action(&origin, &full_prompt, creds, false).await
                };
                match proposed {
                    Ok((thought, _, route)) => (
                        true,
                        step_kind,
                        format!("[{route}] Step {}: {thought}", step_index + 1),
                        risk,
                        decision,
                    ),
                    Err(e) => (
                        false,
                        step_kind,
                        format!("LLM step error: {e}"),
                        risk,
                        decision,
                    ),
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn run_automation_job(
        &self,
        app: tauri::AppHandle,
        job: AutomationJob,
        router: Arc<Mutex<ModelRouter>>,
        creds: Arc<CredentialStore>,
        itp: Arc<ItpEngine>,
        downloads: Arc<DownloadManager>,
        db: Arc<Mutex<BrowserDatabase>>,
        webview_label: String,
        active_url: Arc<Mutex<String>>,
        pending_snapshots: Arc<Mutex<HashMap<Uuid, oneshot::Sender<DomSnapshot>>>>,
        pending_action_results: Arc<Mutex<HashMap<Uuid, oneshot::Sender<crate::agent::action_executor::ActionResult>>>>,
        approval_tx: tokio::sync::mpsc::Sender<ApprovalRequest>,
        trace_tx: tokio::sync::mpsc::Sender<TraceEntry>,
    ) -> anyhow::Result<String> {
        let cancel = CancellationToken::new();
        {
            let mut runs = self.active_runs.lock().await;
            if let Some(prev) = runs.insert(job.id.clone(), cancel.clone()) {
                prev.cancel();
            }
        }

        let run_id = Uuid::new_v4();
        crate::diagnostics::record("automation", "job_started", serde_json::json!({
            "run_id": run_id.to_string(), "count": job.steps.len()
        }));
        {
            let mut guard = db.lock().await;
            let _ = guard.create_agent_run(&run_id.to_string(), &format!("[Automation] {}", job.name));
        }

        let mut final_status = "completed".to_string();

        for (idx, step) in job.steps.iter().enumerate() {
            if cancel.is_cancelled() {
                final_status = "cancelled".to_string();
                break;
            }

            let started = std::time::Instant::now();
            let (ok, step_kind, detail, risk, decision) = self
                .execute_single_step(
                    &app,
                    run_id,
                    &job,
                    idx,
                    step,
                    &router,
                    &creds,
                    &itp,
                    &downloads,
                    &db,
                    &webview_label,
                    &active_url,
                    &pending_snapshots,
                    &pending_action_results,
                    &approval_tx,
                )
                .await;

            let log_entry = AutomationStepLog {
                job_id: job.id.clone(),
                step_index: idx,
                step_kind: step_kind.clone(),
                risk: risk.clone(),
                decision: decision.clone(),
                ok,
                detail: detail.clone(),
            };
            let _ = app.emit("automation:step", &log_entry);
            crate::diagnostics::record("automation", "step_finished", serde_json::json!({
                "run_id": run_id.to_string(), "step": idx,
                "kind": step_kind, "status": if ok { "ok" } else { "failed" },
                "duration_ms": started.elapsed().as_millis() as u64
            }));

            let trace = TraceEntry {
                trace_id: Uuid::new_v4(),
                run_id,
                step_index: idx as u32,
                active_origin: format!("automation://{}", job.id),
                thought: format!("[{}] step {} ({})", job.name, idx + 1, step_kind),
                action: BrowserAction::Complete {
                    summary: detail.clone(),
                },
                risk,
                decision,
                model_route: format!("automation:{step_kind}"),
                latency_ms: started.elapsed().as_millis() as u64,
                outcome: if ok {
                    format!("OK: {detail}")
                } else {
                    format!("FAILED: {detail}")
                },
            };
            let _ = trace_tx.send(trace.sanitized()).await;

            if !ok {
                final_status = "failed".to_string();
                break;
            }
        }

        {
            let mut runs = self.active_runs.lock().await;
            runs.remove(&job.id);
        }

        {
            let mut guard = db.lock().await;
            let _ = guard.update_agent_run_status(&run_id.to_string(), &final_status);
            let _ = guard.record_automation_run_result(&job.id, &final_status);
        }

        crate::diagnostics::record("automation", "job_finished", serde_json::json!({
            "run_id": run_id.to_string(), "status": final_status
        }));
        let _ = app.emit(
            "automation:status",
            serde_json::json!({
                "job_id": job.id,
                "name": job.name,
                "status": final_status
            }),
        );

        Ok(final_status)
    }

    pub async fn stop_job(&self, job_id: &str) -> bool {
        if let Some(token) = self.active_runs.lock().await.remove(job_id) {
            token.cancel();
            return true;
        }
        false
    }

    /// Background non-intrusive scheduler loop for interval-triggered automations.
    pub fn spawn_scheduler(
        engine: Arc<AutomationEngine>,
        app: tauri::AppHandle,
        db: Arc<Mutex<BrowserDatabase>>,
        router: Arc<Mutex<ModelRouter>>,
        creds: Arc<CredentialStore>,
        itp: Arc<ItpEngine>,
        downloads: Arc<DownloadManager>,
        tabs: crate::TabManagerState,
        pending_approvals: Arc<Mutex<HashMap<Uuid, oneshot::Sender<bool>>>>,
        pending_snapshots: Arc<Mutex<HashMap<Uuid, oneshot::Sender<DomSnapshot>>>>,
        pending_action_results: Arc<Mutex<HashMap<Uuid, oneshot::Sender<crate::agent::action_executor::ActionResult>>>>,
        pause_tx: tokio::sync::watch::Sender<bool>,
    ) {
        let _ = pause_tx;
        let scheduler_fut = async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(15));
            loop {
                ticker.tick().await;
                let due_jobs: Vec<AutomationJob> = {
                    let guard = db.lock().await;
                    guard.list_due_interval_automations().unwrap_or_default()
                };
                for job in due_jobs {
                    let (approval_tx, mut approval_rx) =
                        tokio::sync::mpsc::channel::<ApprovalRequest>(8);
                    let (trace_tx, mut trace_rx) = tokio::sync::mpsc::channel::<TraceEntry>(32);

                    let pending_map = pending_approvals.clone();
                    let app_a = app.clone();
                    tauri::async_runtime::spawn(async move {
                        while let Some(req) = approval_rx.recv().await {
                            let aid = req.approval_id;
                            {
                                let mut m = pending_map.lock().await;
                                m.insert(aid, req.responder);
                            }
                            let _ = app_a.emit(
                                "agent:approval_required",
                                serde_json::json!({
                                    "approval_id": aid,
                                    "run_id": req.run_id,
                                    "trace_id": req.trace_id,
                                    "action": req.action,
                                    "risk": req.risk,
                                    "reason": req.reason
                                }),
                            );
                        }
                    });

                    let db_t = db.clone();
                    let app_t = app.clone();
                    tauri::async_runtime::spawn(async move {
                        while let Some(entry) = trace_rx.recv().await {
                            {
                                let mut g = db_t.lock().await;
                                let _ = g.insert_trace(&entry);
                            }
                            let _ = app_t.emit("agent:trace", &entry);
                        }
                    });

                    let label = tabs.active_webview_label().await;
                    let eng_c = engine.clone();
                    let app_c = app.clone();
                    let r_c = router.clone();
                    let c_c = creds.clone();
                    let i_c = itp.clone();
                    let dl_c = downloads.clone();
                    let db_c = db.clone();
                    let url_c = tabs.active_url.clone();
                    let snap_c = pending_snapshots.clone();
                    let res_c = pending_action_results.clone();

                    tauri::async_runtime::spawn(async move {
                        let _ = eng_c
                            .run_automation_job(
                                app_c,
                                job,
                                r_c,
                                c_c,
                                i_c,
                                dl_c,
                                db_c,
                                label,
                                url_c,
                                snap_c,
                                res_c,
                                approval_tx,
                                trace_tx,
                            )
                            .await;
                    });
                }
            }
        };

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let _ = handle.spawn(scheduler_fut);
        } else {
            let _ = tauri::async_runtime::spawn(scheduler_fut);
        }
    }
}
