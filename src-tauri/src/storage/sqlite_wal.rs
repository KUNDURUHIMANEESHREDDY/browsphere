use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use url::Url;
use crate::automation::engine::{AutomationJob, AutomationStep};
use chrono::DateTime;
use uuid::Uuid;

use crate::agent::policy::{BrowserAction, PolicyDecision, RiskLevel, TraceEntry};
use crate::privacy::itp::DownloadRecord;
use crate::router::model_router::RouterConfig;
use crate::aws::AwsConfig;
use crate::agent::memory::{relevant_overlap, MemoryCandidate};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub blocked_trackers: u32,
    pub visited_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRunSummary {
    pub run_id: String,
    pub goal: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub step_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedCredentialMeta {
    pub site: String,
    pub username: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItpHostCount {
    pub blocked_host: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowMacro {
    pub macro_id: String,
    pub origin_host: String,
    pub goal_display: String,
    pub steps: Vec<BrowserAction>,
    pub hit_count: u32,
    pub last_used_at: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub scope: String,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
    pub kind: String,
    pub key: String,
    pub evidence_count: i64,
    pub expires_at: String,
}

pub struct BrowserDatabase {
    conn: Connection,
}

impl BrowserDatabase {
    pub fn open(db_path: &Path) -> anyhow::Result<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS agent_memories (
                id TEXT PRIMARY KEY,
                scope TEXT NOT NULL DEFAULT 'global',
                content TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_agent_memories_scope ON agent_memories(scope);

            CREATE TABLE IF NOT EXISTS history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                url TEXT NOT NULL,
                title TEXT NOT NULL,
                visit_count INTEGER NOT NULL DEFAULT 1,
                blocked_trackers INTEGER NOT NULL DEFAULT 0,
                visited_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS agent_runs (
                run_id TEXT PRIMARY KEY,
                goal TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT ''
            );

            CREATE TABLE IF NOT EXISTS agent_traces (
                trace_id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL,
                step_index INTEGER NOT NULL,
                active_origin TEXT NOT NULL,
                thought TEXT NOT NULL,
                action_json TEXT NOT NULL,
                risk TEXT NOT NULL,
                decision_json TEXT NOT NULL,
                model_route TEXT NOT NULL,
                latency_ms INTEGER NOT NULL,
                outcome TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS downloads (
                id TEXT PRIMARY KEY,
                url TEXT NOT NULL,
                filename TEXT NOT NULL,
                saved_path TEXT NOT NULL DEFAULT '',
                bytes_downloaded INTEGER NOT NULL DEFAULT 0,
                total_bytes INTEGER,
                status TEXT NOT NULL,
                error TEXT,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS itp_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                origin TEXT NOT NULL,
                blocked_host TEXT NOT NULL,
                resource_type TEXT NOT NULL,
                blocked_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY,
                value_json TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS saved_credentials (
                site TEXT NOT NULL,
                username TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY (site, username)
            );

            CREATE TABLE IF NOT EXISTS workflow_macros (
                macro_id TEXT PRIMARY KEY,
                origin_host TEXT NOT NULL,
                goal_norm TEXT NOT NULL,
                goal_display TEXT NOT NULL,
                steps_json TEXT NOT NULL,
                hit_count INTEGER NOT NULL DEFAULT 0,
                last_used_at TEXT NOT NULL,
                created_at TEXT NOT NULL,
                UNIQUE(origin_host, goal_norm)
            );

            CREATE TABLE IF NOT EXISTS automation_jobs (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                trigger TEXT NOT NULL DEFAULT 'manual',
                interval_secs INTEGER,
                enabled INTEGER NOT NULL DEFAULT 1,
                steps_json TEXT NOT NULL,
                last_status TEXT NOT NULL DEFAULT 'idle',
                last_run_at TEXT,
                run_count INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );
            ",
        )?;

        // Safe additive migrations for existing SQLite databases
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN saved_path TEXT NOT NULL DEFAULT ''", []);
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN bytes_downloaded INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN total_bytes INTEGER", []);
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN error TEXT", []);
        let _ = conn.execute("ALTER TABLE agent_runs ADD COLUMN updated_at TEXT NOT NULL DEFAULT ''", []);

        // Backward-compatible migrations: existing user memories remain accessible.
        let _ = conn.execute("ALTER TABLE agent_memories ADD COLUMN memory_kind TEXT NOT NULL DEFAULT 'legacy'", []);
        let _ = conn.execute("ALTER TABLE agent_memories ADD COLUMN memory_key TEXT NOT NULL DEFAULT ''", []);
        let _ = conn.execute("ALTER TABLE agent_memories ADD COLUMN evidence_count INTEGER NOT NULL DEFAULT 1", []);
        let _ = conn.execute("ALTER TABLE agent_memories ADD COLUMN expires_at TEXT NOT NULL DEFAULT ''", []);
        conn.execute_batch("CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_memories_key ON agent_memories(scope, memory_key) WHERE memory_key <> '';")?;
        // Convert old automatically-written task descriptions to expiring memories.
        let legacy_expiry = (Utc::now() + chrono::Duration::days(90)).to_rfc3339();
        let _ = conn.execute(
            "UPDATE agent_memories SET memory_kind='workflow', expires_at=?1 WHERE memory_kind='legacy' AND content LIKE 'Successful browsing workflow:%'",
            [legacy_expiry]
        );
        conn.execute("DELETE FROM agent_memories WHERE expires_at <> '' AND expires_at <= ?1", [Utc::now().to_rfc3339()])?;
        Ok(Self { conn })
    }

    // Local memory records. Autonomous writes use save_learned_memory filters.
    pub fn save_memory(&mut self, scope: &str, content: &str) -> anyhow::Result<MemoryEntry> {
        let scope = scope.trim().to_lowercase();
        let content = content.trim();
        anyhow::ensure!(!content.is_empty() && content.len() <= 2000, "Memory must contain 1–2000 bytes");
        anyhow::ensure!(scope == "global" || (scope.len() <= 253 && scope.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')), "Invalid memory scope");
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO agent_memories (id, scope, content, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
            params![id, scope, content, now],
        )?;
        Ok(MemoryEntry { id, scope, content: content.to_string(), created_at: now.clone(), updated_at: now, kind: "legacy".into(), key: String::new(), evidence_count: 1, expires_at: String::new() })
    }

    // Called only by the agent after observed completion, never by page content.
    // Deduplicate and bound retained automatic memories per site.
    pub fn save_learned_memory(&mut self, scope: &str, content: &str) -> anyhow::Result<Option<MemoryEntry>> {
        let scope = scope.trim().to_lowercase();
        let content = content.trim();
        if scope == "global" || scope.is_empty() || content.is_empty() {
            return Ok(None);
        }
        let already_saved: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM agent_memories WHERE scope = ?1 AND content = ?2",
            params![scope, content],
            |r| r.get(0),
        )?;
        if already_saved > 0 { return Ok(None); }
        let total_for_site: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM agent_memories WHERE scope = ?1",
            params![scope],
            |r| r.get(0),
        )?;
        if total_for_site >= 30 { return Ok(None); }
        self.save_memory(&scope, content).map(Some)
    }

    /// Add or consolidate a structured observation. The same key accumulates
    /// evidence, and explicit new preferences replace contradictory old values.
    /// Old workflow and lesson memories expire instead of living forever.
    pub fn upsert_inferred_memory(&mut self, candidate: &MemoryCandidate) -> anyhow::Result<Option<MemoryEntry>> {
        let scope = candidate.scope.trim().to_lowercase();
        let kind = candidate.kind;
        let key = candidate.key.trim().to_lowercase();
        let content = candidate.content.trim();
        anyhow::ensure!(matches!(kind, "preference" | "workflow" | "lesson"), "Unknown memory type");
        anyhow::ensure!(scope == "global" && kind == "preference" ||
            scope != "global" && scope.len() <= 253 &&
            scope.contains('.') && scope.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-'),
            "Invalid memory scope");
        anyhow::ensure!(key.len() >= 4 && key.len() <= 250 &&
            key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':'),
            "Invalid memory key");
        anyhow::ensure!(!content.is_empty() && content.len() <= 500, "Invalid memory content");
        let now = Utc::now().to_rfc3339();
        let expires_at = match candidate.ttl_days {
            Some(days) => {
                anyhow::ensure!((1..=365).contains(&days), "Invalid retention");
                (Utc::now() + chrono::Duration::days(days)).to_rfc3339()
            }
            None => String::new(),
        };
        self.conn.execute(
            "DELETE FROM agent_memories WHERE expires_at <> '' AND expires_at <= ?1",
            [now.as_str()],
        )?;
        let mut existing = self.conn.prepare(
            "SELECT id, content, evidence_count, created_at FROM agent_memories WHERE scope=?1 AND memory_key=?2 LIMIT 1"
        )?;
        let found = existing.query_row(params![scope, key], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, String>(3)?))
        }).optional()?;
        drop(existing);
        if let Some((id, old_content, evidence, created_at)) = found {
            // A consolidated workflow/lesson is another verified observation even if its
            // wording differs. Changed preferences, however, replace old contradictory evidence.
            let new_evidence = if old_content == content || kind != "preference" {
                (evidence + 1).min(100000)
            } else {
                1
            };
            // A newer confirmed preference overrides an obsolete opposite preference.
            self.conn.execute(
                "UPDATE agent_memories SET content=?1, updated_at=?2, evidence_count=?3, expires_at=?4 WHERE id=?5",
                params![content, now, new_evidence, expires_at, id],
            )?;
            return Ok(Some(MemoryEntry {
                id, scope, content: content.to_string(), created_at,
                updated_at: now, kind: kind.into(), key, evidence_count: new_evidence, expires_at,
            }));
        }
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM agent_memories WHERE scope=?1", [&scope], |r| r.get(0)
        )?;
        if count >= 40 {
            // Keep bounded per-site context. Never prune existing manual/legacy notes.
            let deleted = self.conn.execute(
                "DELETE FROM agent_memories WHERE id IN (
                   SELECT id FROM agent_memories WHERE scope=?1 AND memory_kind IN ('workflow','lesson')
                   ORDER BY updated_at ASC LIMIT 1
                 )", [&scope],
            )?;
            if deleted == 0 { return Ok(None); }
        }
        let id = Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO agent_memories (id,scope,content,created_at,updated_at,memory_kind,memory_key,evidence_count,expires_at)
             VALUES (?1,?2,?3,?4,?4,?5,?6,1,?7)",
            params![id, scope, content, now, kind, key, expires_at],
        )?;
        Ok(Some(MemoryEntry {
            id, scope, content: content.to_string(), created_at: now.clone(),
            updated_at: now, kind: kind.into(), key, evidence_count: 1, expires_at,
        }))
    }

    pub fn list_memories(&self, scope: Option<&str>, query: Option<&str>, limit: usize) -> anyhow::Result<Vec<MemoryEntry>> {
        let scope = scope.unwrap_or("").trim().to_lowercase();
        let query = query.unwrap_or("").trim().to_lowercase();
        let mut stmt = self.conn.prepare(
            "SELECT id, scope, content, created_at, updated_at, memory_kind, memory_key, evidence_count, expires_at FROM agent_memories
             WHERE (?1 = '' OR scope = ?1) AND (?2 = '' OR instr(lower(content), ?2) > 0)
               AND (expires_at = '' OR expires_at > ?4)
             ORDER BY updated_at DESC LIMIT ?3"
        )?;
        let rows = stmt.query_map(params![scope, query, limit.min(200) as i64, Utc::now().to_rfc3339()], |r| Ok(MemoryEntry {
            id: r.get(0)?, scope: r.get(1)?, content: r.get(2)?, created_at: r.get(3)?, updated_at: r.get(4)?,
            kind: r.get(5)?, key: r.get(6)?, evidence_count: r.get(7)?, expires_at: r.get(8)?
        }))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn delete_memory(&mut self, id: &str) -> anyhow::Result<bool> {
        Ok(self.conn.execute("DELETE FROM agent_memories WHERE id = ?1", [id])? > 0)
    }

    pub fn clear_memories(&mut self) -> anyhow::Result<usize> {
        Ok(self.conn.execute("DELETE FROM agent_memories", [])?)
    }

    /// Recall only site-scoped context plus explicit global preferences; never
    /// allow a memory saved on one domain to leak into another domain.
    pub fn recall_memories(&self, origin: &str, goal: &str, limit: usize) -> anyhow::Result<Vec<MemoryEntry>> {
        let host = Url::parse(origin).ok().filter(|u| matches!(u.scheme(), "https" | "http"))
            .and_then(|u| u.host_str().map(str::to_owned)).unwrap_or_default();
        let items = self.list_memories(None, None, 200)?;
        let mut scored: Vec<(i64, MemoryEntry)> = items.into_iter().filter_map(|m| {
            if m.scope != "global" && m.scope != host { return None; }
            let overlap = relevant_overlap(goal, &m.content) as i64;
            let score = if m.scope == "global" && (m.kind == "preference" || m.kind == "legacy") {
                100
            } else if m.kind == "lesson" && m.scope == host {
                10 + overlap * 4
            } else if overlap > 0 {
                overlap * 8 + m.evidence_count.min(4) + if m.scope == host { 3 } else { 0 }
            } else { return None; };
            Some((score, m))
        }).collect();
        scored.sort_by(|a,b| b.0.cmp(&a.0).then_with(|| b.1.updated_at.cmp(&a.1.updated_at)));
        Ok(scored.into_iter().take(limit.min(8)).map(|(_,m)| m).collect())
    }

    pub fn save_voice_config(&mut self, config: &crate::voice::VoiceConfig) -> anyhow::Result<()> {
        config.validate()?;
        self.conn.execute(
            "INSERT OR REPLACE INTO app_settings (key,value_json,updated_at) VALUES ('voice_config',?1,?2)",
            params![serde_json::to_string(config)?, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn load_voice_config(&self) -> anyhow::Result<Option<crate::voice::VoiceConfig>> {
        let raw: Option<String> = self.conn.query_row(
            "SELECT value_json FROM app_settings WHERE key='voice_config'", [],
            |row| row.get(0),
        ).optional()?;
        match raw {
            Some(value) => {
                let cfg: crate::voice::VoiceConfig = serde_json::from_str(&value)?;
                cfg.validate()?;
                Ok(Some(cfg))
            },
            None => Ok(None)
        }
    }

    pub fn save_aws_config(&mut self, config: &AwsConfig) -> anyhow::Result<()> {
        config.validate()?;
        self.conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES ('aws_config', ?1, ?2)",
            params![serde_json::to_string(config)?, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn load_aws_config(&self) -> anyhow::Result<Option<AwsConfig>> {
        let raw: Option<String> = self.conn.query_row(
            "SELECT value_json FROM app_settings WHERE key='aws_config'",
            [], |row| row.get(0)
        ).optional()?;
        match raw {
            Some(json) => {
                let cfg: AwsConfig = serde_json::from_str(&json)?;
                cfg.validate()?;
                Ok(Some(cfg))
            }
            None => Ok(None)
        }
    }

    pub fn save_router_config(&mut self, config: &RouterConfig) -> anyhow::Result<()> {
        let mut sanitized = config.clone();
        sanitized.litellm_api_key = None; // Secrets stay strictly in Windows Credential Manager
        let json_str = serde_json::to_string(&sanitized)?;
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES ('router_config', ?1, ?2)",
            params![json_str, now],
        )?;
        Ok(())
    }

    pub fn load_router_config(&self) -> anyhow::Result<Option<RouterConfig>> {
        let mut stmt = self.conn.prepare(
            "SELECT value_json FROM app_settings WHERE key = 'router_config' LIMIT 1",
        )?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let raw: String = row.get(0)?;
            if let Ok(cfg) = serde_json::from_str::<RouterConfig>(&raw) {
                return Ok(Some(cfg));
            }
        }
        Ok(None)
    }

    pub fn record_visit(
        &mut self,
        url: &str,
        title: Option<&str>,
        blocked_trackers: u32,
    ) -> anyhow::Result<()> {
        let now = Utc::now().to_rfc3339();
        let display_title = title.unwrap_or(url);
        self.conn.execute(
            "INSERT INTO history (url, title, visit_count, blocked_trackers, visited_at)
             VALUES (?1, ?2, 1, ?3, ?4)",
            params![url, display_title, blocked_trackers, now],
        )?;
        Ok(())
    }

    pub fn get_history(&self, limit: usize) -> anyhow::Result<Vec<HistoryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, url, title, blocked_trackers, visited_at
             FROM history ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], |row| {
            Ok(HistoryEntry {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                blocked_trackers: row.get(3)?,
                visited_at: row.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn clear_history(&mut self) -> anyhow::Result<()> {
        self.conn.execute("DELETE FROM history", [])?;
        Ok(())
    }

    pub fn record_itp_block(
        &mut self,
        origin: &str,
        blocked_host: &str,
        resource_type: &str,
    ) -> anyhow::Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO itp_events (origin, blocked_host, resource_type, blocked_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![origin, blocked_host, resource_type, now],
        )?;
        Ok(())
    }

    pub fn create_agent_run(&mut self, run_id: &str, goal: &str) -> anyhow::Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT OR REPLACE INTO agent_runs (run_id, goal, status, created_at, updated_at)
             VALUES (?1, ?2, 'running', ?3, ?3)",
            params![run_id, goal, now],
        )?;
        Ok(())
    }

    pub fn update_agent_run_status(&mut self, run_id: &str, status: &str) -> anyhow::Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE agent_runs SET status = ?1, updated_at = ?2 WHERE run_id = ?3",
            params![status, now, run_id],
        )?;
        Ok(())
    }

    pub fn get_recent_runs(&self, limit: usize) -> anyhow::Result<Vec<AgentRunSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT r.run_id, r.goal, r.status, r.created_at,
                    CASE WHEN r.updated_at = '' THEN r.created_at ELSE r.updated_at END,
                    (SELECT COUNT(*) FROM agent_traces t WHERE t.run_id = r.run_id) AS step_count
             FROM agent_runs r
             ORDER BY r.rowid DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], |row| {
            let steps: i64 = row.get(5)?;
            Ok(AgentRunSummary {
                run_id: row.get(0)?,
                goal: row.get(1)?,
                status: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                step_count: steps.max(0) as u32,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn insert_trace(&mut self, entry: &TraceEntry) -> anyhow::Result<()> {
        let now = Utc::now().to_rfc3339();
        let action_json = serde_json::to_string(&entry.action)?;
        let risk_str = serde_json::to_string(&entry.risk)?;
        let decision_json = serde_json::to_string(&entry.decision)?;

        self.conn.execute(
            "INSERT INTO agent_traces (
                trace_id, run_id, step_index, active_origin, thought,
                action_json, risk, decision_json, model_route, latency_ms, outcome, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                entry.trace_id.to_string(),
                entry.run_id.to_string(),
                entry.step_index,
                entry.active_origin,
                entry.thought,
                action_json,
                risk_str,
                decision_json,
                entry.model_route,
                entry.latency_ms as i64,
                entry.outcome,
                now
            ],
        )?;
        Ok(())
    }

    pub fn get_traces_filtered(
        &self,
        run_id: Option<&str>,
        risk_filter: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<Vec<TraceEntry>> {
        let risk_json_opt = risk_filter
            .map(|r| r.trim().to_lowercase())
            .filter(|r| !r.is_empty() && r != "all")
            .map(|r| format!("\"{r}\""));
        let run_id_opt = run_id
            .map(|r| r.trim().to_string())
            .filter(|r| !r.is_empty() && r != "all");

        let mut stmt = self.conn.prepare(
            "SELECT trace_id, run_id, step_index, active_origin, thought,
                    action_json, risk, decision_json, model_route, latency_ms, outcome
             FROM agent_traces
             WHERE (?1 IS NULL OR run_id = ?1)
               AND (?2 IS NULL OR risk = ?2)
             ORDER BY rowid DESC LIMIT ?3",
        )?;

        let rows = stmt.query_map(params![run_id_opt, risk_json_opt, limit as i64], |row| {
            let trace_id_str: String = row.get(0)?;
            let run_id_str: String = row.get(1)?;
            let step_index: u32 = row.get(2)?;
            let active_origin: String = row.get(3)?;
            let thought: String = row.get(4)?;
            let action_json: String = row.get(5)?;
            let risk_raw: String = row.get(6)?;
            let decision_json: String = row.get(7)?;
            let model_route: String = row.get(8)?;
            let latency_ms: i64 = row.get(9)?;
            let outcome: String = row.get(10)?;

            let action: BrowserAction =
                serde_json::from_str(&action_json).unwrap_or(BrowserAction::Scroll { delta_y: 0 });
            let risk: RiskLevel =
                serde_json::from_str(&risk_raw).unwrap_or(RiskLevel::Low);
            let decision: PolicyDecision =
                serde_json::from_str(&decision_json).unwrap_or(PolicyDecision::Allow);

            Ok(TraceEntry {
                trace_id: Uuid::parse_str(&trace_id_str).unwrap_or_else(|_| Uuid::new_v4()),
                run_id: Uuid::parse_str(&run_id_str).unwrap_or_else(|_| Uuid::new_v4()),
                step_index,
                active_origin,
                thought,
                action,
                risk,
                decision,
                model_route,
                latency_ms: latency_ms.max(0) as u64,
                outcome,
            })
        })?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn get_recent_traces(&self, limit: usize) -> anyhow::Result<Vec<TraceEntry>> {
        self.get_traces_filtered(None, None, limit)
    }

    pub fn upsert_download(&mut self, rec: &DownloadRecord) -> anyhow::Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT OR REPLACE INTO downloads (
                id, url, filename, saved_path, bytes_downloaded, total_bytes, status, error, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                rec.id.to_string(),
                rec.url,
                rec.filename,
                rec.saved_path,
                rec.bytes_downloaded as i64,
                rec.total_bytes.map(|v| v as i64),
                rec.status,
                rec.error,
                now
            ],
        )?;
        Ok(())
    }

    pub fn get_recent_downloads(&self, limit: usize) -> anyhow::Result<Vec<DownloadRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, url, filename, saved_path, bytes_downloaded, total_bytes, status, error
             FROM downloads ORDER BY rowid DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], |row| {
            let id_str: String = row.get(0)?;
            let bytes_dl: i64 = row.get(4)?;
            let total_b: Option<i64> = row.get(5)?;
            Ok(DownloadRecord {
                id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
                url: row.get(1)?,
                filename: row.get(2)?,
                saved_path: row.get(3)?,
                bytes_downloaded: bytes_dl.max(0) as u64,
                total_bytes: total_b.map(|v| v.max(0) as u64),
                status: row.get(6)?,
                error: row.get(7)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn clear_downloads(&mut self) -> anyhow::Result<()> {
        self.conn.execute("DELETE FROM downloads", [])?;
        Ok(())
    }

    pub fn get_itp_breakdown(&self, limit: usize) -> anyhow::Result<Vec<ItpHostCount>> {
        let mut stmt = self.conn.prepare(
            "SELECT blocked_host, COUNT(*) AS cnt
             FROM itp_events
             GROUP BY blocked_host
             ORDER BY cnt DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], |row| {
            let cnt: i64 = row.get(1)?;
            Ok(ItpHostCount {
                blocked_host: row.get(0)?,
                count: cnt.max(0) as u64,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn get_total_itp_blocked(&self) -> anyhow::Result<u64> {
        let cnt: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM itp_events", [], |row| row.get(0))
            .unwrap_or(0);
        Ok(cnt.max(0) as u64)
    }

    pub fn upsert_credential_meta(&mut self, site: &str, username: &str) -> anyhow::Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT OR REPLACE INTO saved_credentials (site, username, updated_at)
             VALUES (?1, ?2, ?3)",
            params![site.trim(), username.trim(), now],
        )?;
        Ok(())
    }

    pub fn list_credentials_meta(&self) -> anyhow::Result<Vec<SavedCredentialMeta>> {
        let mut stmt = self.conn.prepare(
            "SELECT site, username, updated_at
             FROM saved_credentials
             ORDER BY site ASC, username ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(SavedCredentialMeta {
                site: row.get(0)?,
                username: row.get(1)?,
                updated_at: row.get(2)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn delete_credential_meta(&mut self, site: &str, username: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "DELETE FROM saved_credentials WHERE site = ?1 AND username = ?2",
            params![site.trim(), username.trim()],
        )?;
        Ok(())
    }

    pub fn normalize_origin_host(origin: &str) -> String {
        if let Ok(parsed) = Url::parse(origin) {
            if let Some(host) = parsed.host_str() {
                return host.trim().to_lowercase();
            }
        }
        origin
            .trim()
            .to_lowercase()
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .split('/')
            .next()
            .unwrap_or("unknown")
            .to_string()
    }

    pub fn normalize_goal(goal: &str) -> String {
        goal.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    }

    pub fn find_workflow_macro(
        &self,
        origin: &str,
        goal: &str,
    ) -> anyhow::Result<Option<WorkflowMacro>> {
        let host = Self::normalize_origin_host(origin);
        let goal_norm = Self::normalize_goal(goal);
        if goal_norm.is_empty() {
            return Ok(None);
        }
        let mut stmt = self.conn.prepare(
            "SELECT macro_id, origin_host, goal_display, steps_json, hit_count, last_used_at, created_at
             FROM workflow_macros
             WHERE origin_host = ?1 AND goal_norm = ?2
             LIMIT 1",
        )?;
        let mut rows = stmt.query(params![host, goal_norm])?;
        if let Some(row) = rows.next()? {
            let steps_raw: String = row.get(3)?;
            let steps: Vec<BrowserAction> = serde_json::from_str(&steps_raw).unwrap_or_default();
            let hits: i64 = row.get(4)?;
            return Ok(Some(WorkflowMacro {
                macro_id: row.get(0)?,
                origin_host: row.get(1)?,
                goal_display: row.get(2)?,
                steps,
                hit_count: hits.max(0) as u32,
                last_used_at: row.get(5)?,
                created_at: row.get(6)?,
            }));
        }
        Ok(None)
    }

    pub fn get_workflow_macro_by_id(&self, macro_id: &str) -> anyhow::Result<Option<WorkflowMacro>> {
        let mut stmt = self.conn.prepare(
            "SELECT macro_id, origin_host, goal_display, steps_json, hit_count, last_used_at, created_at
             FROM workflow_macros
             WHERE macro_id = ?1
             LIMIT 1",
        )?;
        let mut rows = stmt.query(params![macro_id])?;
        if let Some(row) = rows.next()? {
            let steps_raw: String = row.get(3)?;
            let steps: Vec<BrowserAction> = serde_json::from_str(&steps_raw).unwrap_or_default();
            let hits: i64 = row.get(4)?;
            return Ok(Some(WorkflowMacro {
                macro_id: row.get(0)?,
                origin_host: row.get(1)?,
                goal_display: row.get(2)?,
                steps,
                hit_count: hits.max(0) as u32,
                last_used_at: row.get(5)?,
                created_at: row.get(6)?,
            }));
        }
        Ok(None)
    }

    pub fn upsert_workflow_macro(
        &mut self,
        origin: &str,
        goal: &str,
        steps: &[BrowserAction],
    ) -> anyhow::Result<WorkflowMacro> {
        let host = Self::normalize_origin_host(origin);
        let goal_norm = Self::normalize_goal(goal);
        let steps_json = serde_json::to_string(steps)?;
        let now = Utc::now().to_rfc3339();

        let existing = self.find_workflow_macro(&host, goal)?;
        let (macro_id, hit_count, created_at) = match existing {
            Some(m) => (m.macro_id, m.hit_count, m.created_at),
            None => (Uuid::new_v4().to_string(), 0, now.clone()),
        };

        self.conn.execute(
            "INSERT INTO workflow_macros (
                macro_id, origin_host, goal_norm, goal_display, steps_json, hit_count, last_used_at, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(origin_host, goal_norm) DO UPDATE SET
                goal_display = excluded.goal_display,
                steps_json = excluded.steps_json,
                last_used_at = excluded.last_used_at",
            params![
                macro_id,
                host,
                goal_norm,
                goal.trim(),
                steps_json,
                hit_count as i64,
                now,
                created_at
            ],
        )?;

        Ok(WorkflowMacro {
            macro_id,
            origin_host: host,
            goal_display: goal.trim().to_string(),
            steps: steps.to_vec(),
            hit_count,
            last_used_at: now,
            created_at,
        })
    }

    pub fn increment_macro_hit(&mut self, macro_id: &str) -> anyhow::Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE workflow_macros SET hit_count = hit_count + 1, last_used_at = ?1 WHERE macro_id = ?2",
            params![now, macro_id],
        )?;
        Ok(())
    }

    pub fn list_workflow_macros(&self, limit: usize) -> anyhow::Result<Vec<WorkflowMacro>> {
        let mut stmt = self.conn.prepare(
            "SELECT macro_id, origin_host, goal_display, steps_json, hit_count, last_used_at, created_at
             FROM workflow_macros
             ORDER BY last_used_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], |row| {
            let steps_raw: String = row.get(3)?;
            let steps: Vec<BrowserAction> = serde_json::from_str(&steps_raw).unwrap_or_default();
            let hits: i64 = row.get(4)?;
            Ok(WorkflowMacro {
                macro_id: row.get(0)?,
                origin_host: row.get(1)?,
                goal_display: row.get(2)?,
                steps,
                hit_count: hits.max(0) as u32,
                last_used_at: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn delete_workflow_macro(&mut self, macro_id: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "DELETE FROM workflow_macros WHERE macro_id = ?1",
            params![macro_id],
        )?;
        Ok(())
    }

    pub fn upsert_automation_job(
        &mut self,
        id_opt: Option<String>,
        name: &str,
        trigger: &str,
        interval_secs: Option<u64>,
        enabled: bool,
        steps: &[AutomationStep],
    ) -> anyhow::Result<AutomationJob> {
        let now = Utc::now().to_rfc3339();
        let job_id = id_opt
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let steps_json = serde_json::to_string(steps)?;
        let trig = if trigger == "interval" { "interval" } else { "manual" };
        let iv_opt: Option<i64> = if trig == "interval" {
            Some(interval_secs.unwrap_or(300).max(30) as i64)
        } else {
            interval_secs.map(|v| v as i64)
        };

        self.conn.execute(
            "INSERT INTO automation_jobs (
                id, name, trigger, interval_secs, enabled, steps_json, last_status, last_run_at, run_count, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'idle', NULL, 0, ?7)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                trigger = excluded.trigger,
                interval_secs = excluded.interval_secs,
                enabled = excluded.enabled,
                steps_json = excluded.steps_json",
            params![
                job_id,
                name.trim(),
                trig,
                iv_opt,
                if enabled { 1_i64 } else { 0_i64 },
                steps_json,
                now
            ],
        )?;

        self.get_automation_job(&job_id)?
            .ok_or_else(|| anyhow::anyhow!("Failed to read saved automation job"))
    }

    pub fn get_automation_job(&self, id: &str) -> anyhow::Result<Option<AutomationJob>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, trigger, interval_secs, enabled, steps_json, last_status, last_run_at, run_count
             FROM automation_jobs WHERE id = ?1 LIMIT 1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            let steps_raw: String = row.get(5)?;
            let steps: Vec<AutomationStep> = serde_json::from_str(&steps_raw).unwrap_or_default();
            let iv_i64: Option<i64> = row.get(3)?;
            let en_i64: i64 = row.get(4)?;
            let rc_i64: i64 = row.get(8)?;
            return Ok(Some(AutomationJob {
                id: row.get(0)?,
                name: row.get(1)?,
                trigger: row.get(2)?,
                interval_secs: iv_i64.map(|v| v.max(0) as u64),
                enabled: en_i64 != 0,
                steps,
                last_status: row.get(6)?,
                last_run_at: row.get(7)?,
                run_count: rc_i64.max(0) as u32,
            }));
        }
        Ok(None)
    }

    pub fn list_automation_jobs(&self, limit: usize) -> anyhow::Result<Vec<AutomationJob>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, trigger, interval_secs, enabled, steps_json, last_status, last_run_at, run_count
             FROM automation_jobs ORDER BY rowid DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], |row| {
            let steps_raw: String = row.get(5)?;
            let steps: Vec<AutomationStep> = serde_json::from_str(&steps_raw).unwrap_or_default();
            let iv_i64: Option<i64> = row.get(3)?;
            let en_i64: i64 = row.get(4)?;
            let rc_i64: i64 = row.get(8)?;
            Ok(AutomationJob {
                id: row.get(0)?,
                name: row.get(1)?,
                trigger: row.get(2)?,
                interval_secs: iv_i64.map(|v| v.max(0) as u64),
                enabled: en_i64 != 0,
                steps,
                last_status: row.get(6)?,
                last_run_at: row.get(7)?,
                run_count: rc_i64.max(0) as u32,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn set_automation_enabled(&mut self, id: &str, enabled: bool) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE automation_jobs SET enabled = ?1 WHERE id = ?2",
            params![if enabled { 1_i64 } else { 0_i64 }, id],
        )?;
        Ok(())
    }

    pub fn delete_automation_job(&mut self, id: &str) -> anyhow::Result<()> {
        self.conn.execute("DELETE FROM automation_jobs WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn record_automation_run_result(&mut self, id: &str, status: &str) -> anyhow::Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE automation_jobs
             SET last_status = ?1, last_run_at = ?2, run_count = run_count + 1
             WHERE id = ?3",
            params![status, now, id],
        )?;
        Ok(())
    }

    pub fn list_due_interval_automations(&self) -> anyhow::Result<Vec<AutomationJob>> {
        let all = self.list_automation_jobs(100)?;
        let now_dt = Utc::now();
        let mut due = Vec::new();
        for job in all {
            if !job.enabled || job.trigger != "interval" {
                continue;
            }
            let interval = job.interval_secs.unwrap_or(300).max(30) as i64;
            let is_due = match &job.last_run_at {
                None => true,
                Some(ts) => {
                    if let Ok(parsed) = DateTime::parse_from_rfc3339(ts) {
                        (now_dt - parsed.with_timezone(&Utc)).num_seconds() >= interval
                    } else {
                        true
                    }
                }
            };
            if is_due {
                due.push(job);
            }
        }
        Ok(due)
    }
}

#[cfg(test)]
mod memory_tests {
    use super::*;

    #[test]
    fn memories_are_persistent_scoped_and_deletable() {
        let path = std::env::temp_dir().join(format!("browsphere-memory-test-{}.db", Uuid::new_v4()));
        {
            let mut db = BrowserDatabase::open(&path).unwrap();
            let global = db.save_memory("global", "Prefer concise technical explanations").unwrap();
            let site = db.save_memory("example.com", "Project roadmap is in the docs").unwrap();
            assert!(db.save_memory("a/b", "invalid").is_err());
            assert!(db.save_memory("global", "").is_err());
            assert_eq!(db.list_memories(None, None, 10).unwrap().len(), 2);
            assert_eq!(db.list_memories(None, Some("roadmap"), 10).unwrap().len(), 1);
            assert!(db.recall_memories("https://example.com", "Find the project roadmap", 5).unwrap().iter().any(|m| m.id == site.id));
            assert!(!db.recall_memories("https://another.com", "Find the project roadmap", 5).unwrap().iter().any(|m| m.id == site.id));
            assert!(db.recall_memories("https://another.com", "Summarize this", 5).unwrap().iter().any(|m| m.id == global.id));
            let learned = db.save_learned_memory("example.com", "Successful browsing workflow: Find docs").unwrap();
            assert!(learned.is_some());
            assert!(db.save_learned_memory("example.com", "Successful browsing workflow: Find docs").unwrap().is_none());
            assert!(db.save_learned_memory("global", "Should not learn automatically").unwrap().is_none());
            assert!(db.delete_memory(&learned.unwrap().id).unwrap());
            assert!(db.delete_memory(&site.id).unwrap());
        }
        {
            let mut db = BrowserDatabase::open(&path).unwrap();
            assert_eq!(db.list_memories(None, None, 10).unwrap().len(), 1);
            assert_eq!(db.clear_memories().unwrap(), 1);
            assert!(db.list_memories(None, None, 10).unwrap().is_empty());
        }
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(format!("{}-wal", path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", path.display()));
    }
}

#[cfg(test)]
mod smart_memory_tests {
    use super::*;
    use crate::agent::memory::{infer_memories, RecoveredFailure};

    #[test]
    fn merges_workflows_replaces_preferences_and_expires_lessons() {
        let path = std::env::temp_dir().join(format!("browsphere-smart-memory-{}.db", Uuid::new_v4()));
        let mut db = BrowserDatabase::open(&path).unwrap();

        let first = infer_memories("https://docs.rust-lang.org", "Find Rust ownership documentation", true, &[]);
        let second = infer_memories("https://docs.rust-lang.org", "Search Rust ownership docs", true, &[]);
        let a = db.upsert_inferred_memory(&first[0]).unwrap().unwrap();
        let b = db.upsert_inferred_memory(&second[0]).unwrap().unwrap();
        assert_eq!(a.id, b.id, "similar successful workflows should consolidate");
        assert_eq!(b.evidence_count, 2);
        assert_eq!(db.list_memories(Some("docs.rust-lang.org"), None, 20).unwrap().len(), 1);
        assert!(db.recall_memories("https://docs.rust-lang.org", "Find Rust ownership docs", 5).unwrap()
            .iter().any(|m| m.id == a.id));
        assert!(!db.recall_memories("https://other.example", "Find Rust ownership docs", 5).unwrap()
            .iter().any(|m| m.id == a.id));

        let p1 = infer_memories("https://docs.rust-lang.org", "I prefer concise responses", true, &[]);
        let p2 = infer_memories("https://docs.rust-lang.org", "I prefer detailed explanations", true, &[]);
        let old = db.upsert_inferred_memory(&p1[0]).unwrap().unwrap();
        let new = db.upsert_inferred_memory(&p2[0]).unwrap().unwrap();
        assert_eq!(old.id, new.id);
        assert_ne!(old.content, new.content);
        assert_eq!(new.evidence_count, 1);
        assert!(db.recall_memories("https://other.example", "Search Rust docs", 5).unwrap()
            .iter().any(|m| m.id == new.id));

        let with_lesson = infer_memories("https://docs.rust-lang.org", "Find Rust ownership documentation", true,
            &[RecoveredFailure::StaleControl]);
        let lesson = db.upsert_inferred_memory(&with_lesson[1]).unwrap().unwrap();
        assert_eq!(lesson.kind, "lesson");
        assert!(!lesson.expires_at.is_empty());
        db.conn.execute("UPDATE agent_memories SET expires_at='2000-01-01T00:00:00+00:00' WHERE id=?1",
            [&lesson.id]).unwrap();
        assert!(!db.list_memories(None, None, 20).unwrap().iter().any(|m| m.id == lesson.id));
        drop(db);

        let db = BrowserDatabase::open(&path).unwrap();
        assert_eq!(db.conn.query_row("SELECT COUNT(*) FROM agent_memories WHERE id=?1", [&lesson.id],
            |r| r.get::<_, i64>(0)).unwrap(), 0, "expired memory should be purged on startup");
        drop(db);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(format!("{}-wal", path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", path.display()));
    }

    #[test]
    fn rejected_sensitive_or_invalid_inferences_cannot_be_written() {
        let path = std::env::temp_dir().join(format!("browsphere-smart-memory-{}.db", Uuid::new_v4()));
        let mut db = BrowserDatabase::open(&path).unwrap();
        assert!(db.upsert_inferred_memory(&MemoryCandidate {
            scope: "global".into(), kind: "workflow", key: "workflow:docs".into(),
            content: "Should not be global".into(), ttl_days: Some(90),
        }).is_err());
        assert!(db.upsert_inferred_memory(&MemoryCandidate {
            scope: "evil/example".into(), kind: "lesson", key: "lesson:bad".into(),
            content: "Should not save".into(), ttl_days: Some(60),
        }).is_err());
        drop(db);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(format!("{}-wal", path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", path.display()));
    }
}

#[cfg(test)]
mod provider_model_catalog_tests {
    use super::*;

    #[test]
    fn fetched_model_ids_survive_database_restart_without_storing_the_api_key() {
        let path = std::env::temp_dir()
            .join(format!("browsphere-cloud-catalog-{}.db", Uuid::new_v4()));
        let endpoint = "https://api.groq.com/openai/v1/chat/completions";
        {
            let mut db = BrowserDatabase::open(&path).unwrap();
            let mut cfg = RouterConfig::default();
            cfg.litellm_url = endpoint.into();
            cfg.litellm_model = "second-model".into();
            cfg.litellm_api_key = Some("test-only-secret-never-persist".into());
            cfg.cloud_model_catalogs.insert(
                endpoint.into(), vec!["first-model".into(), "second-model".into()],
            );
            db.save_router_config(&cfg).unwrap();
        }
        {
            let db = BrowserDatabase::open(&path).unwrap();
            let reloaded = db.load_router_config().unwrap().unwrap();
            assert_eq!(reloaded.litellm_model, "second-model");
            assert_eq!(
                reloaded.cloud_model_catalogs.get(endpoint).unwrap(),
                &vec!["first-model".to_string(), "second-model".to_string()]
            );
            assert!(reloaded.litellm_api_key.is_none());
        }
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
    }

    #[test]
    fn older_router_config_without_catalogs_still_deserializes() {
        let mut old = serde_json::to_value(RouterConfig::default()).unwrap();
        old.as_object_mut().unwrap().remove("cloud_model_catalogs");
        let config: RouterConfig = serde_json::from_value(old).unwrap();
        assert!(config.cloud_model_catalogs.is_empty());
    }
}
