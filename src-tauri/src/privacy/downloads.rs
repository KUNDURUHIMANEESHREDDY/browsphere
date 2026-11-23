use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::Emitter;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;

use crate::privacy::itp::{DownloadRecord, ItpEngine};
use crate::storage::sqlite_wal::BrowserDatabase;

pub struct DownloadManager {
    client: reqwest::Client,
    downloads_dir: PathBuf,
    active_tokens: Arc<Mutex<HashMap<Uuid, CancellationToken>>>,
}

impl DownloadManager {
    pub fn new(fallback_data_dir: &Path) -> Self {
        let downloads_dir = Self::resolve_downloads_dir(fallback_data_dir);
        let _ = std::fs::create_dir_all(&downloads_dir);
        let client = reqwest::Client::builder()
            .user_agent("BrowsphereAI/0.1 (Windows NT 10.0; Win64; x64)")
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            client,
            downloads_dir,
            active_tokens: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn resolve_downloads_dir(fallback_data_dir: &Path) -> PathBuf {
        if let Ok(profile) = std::env::var("USERPROFILE") {
            let mut p = PathBuf::from(profile);
            p.push("Downloads");
            if p.exists() || std::fs::create_dir_all(&p).is_ok() {
                return p;
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            let mut p = PathBuf::from(home);
            p.push("Downloads");
            if p.exists() || std::fs::create_dir_all(&p).is_ok() {
                return p;
            }
        }
        let mut p = fallback_data_dir.to_path_buf();
        p.push("downloads");
        p
    }

    pub fn downloads_dir(&self) -> &Path {
        &self.downloads_dir
    }

    /// Generates a unique destination path inside the Downloads folder without overwriting existing files.
    pub fn resolve_unique_path(&self, sanitized_filename: &str) -> PathBuf {
        let base_path = self.downloads_dir.join(sanitized_filename);
        if !base_path.exists() {
            return base_path;
        }
        let stem = Path::new(sanitized_filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("download");
        let ext = Path::new(sanitized_filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        for idx in 1..=999 {
            let candidate_name = if ext.is_empty() {
                format!("{stem} ({idx})")
            } else {
                format!("{stem} ({idx}).{ext}")
            };
            let candidate = self.downloads_dir.join(candidate_name);
            if !candidate.exists() {
                return candidate;
            }
        }
        self.downloads_dir
            .join(format!("{stem}-{}", Uuid::new_v4()))
    }

    pub async fn cancel_download(&self, id: Uuid) -> bool {
        if let Some(token) = self.active_tokens.lock().await.remove(&id) {
            token.cancel();
            return true;
        }
        false
    }

    /// Starts a real background streaming HTTP(S) download directly to disk,
    /// persisting byte progress in SQLite WAL and emitting live byte-level events.
    pub async fn start_streaming_download(
        &self,
        app: tauri::AppHandle,
        db: Arc<Mutex<BrowserDatabase>>,
        itp: Arc<ItpEngine>,
        raw_url: String,
        suggested_filename: Option<String>,
    ) -> anyhow::Result<DownloadRecord> {
        let parsed = Url::parse(&raw_url)?;
        match parsed.scheme() {
            "http" | "https" => {}
            other => anyhow::bail!("Download blocked: unsupported scheme '{other}:'"),
        }

        let inferred_name = suggested_filename
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                parsed
                    .path_segments()
                    .and_then(|segs| segs.last())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "download.bin".to_string());

        let clean_name = itp.sanitize_filename(&inferred_name);
        let dest_path = self.resolve_unique_path(&clean_name);
        let saved_path_str = dest_path.to_string_lossy().to_string();

        let id = Uuid::new_v4();
        let initial_record = DownloadRecord {
            id,
            url: parsed.to_string(),
            filename: dest_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&clean_name)
                .to_string(),
            saved_path: saved_path_str.clone(),
            bytes_downloaded: 0,
            total_bytes: None,
            status: "downloading".to_string(),
            error: None,
        };

        {
            let mut guard = db.lock().await;
            guard.upsert_download(&initial_record)?;
        }

        let cancel_token = CancellationToken::new();
        {
            let mut tokens = self.active_tokens.lock().await;
            tokens.insert(id, cancel_token.clone());
        }

        let _ = app.emit("download:progress", &initial_record);

        let client = self.client.clone();
        let tokens_map = self.active_tokens.clone();
        let mut record = initial_record.clone();

        tokio::spawn(async move {
            let outcome: anyhow::Result<()> = async {
                let mut resp = client.get(&record.url).send().await?.error_for_status()?;
                record.total_bytes = resp.content_length();
                let _ = app.emit("download:progress", &record);

                let mut file = tokio::fs::File::create(&dest_path).await?;
                let mut last_emit = Instant::now();
                let mut bytes_since_emit: u64 = 0;
                let mut downloaded: u64 = 0;

                loop {
                    tokio::select! {
                        _ = cancel_token.cancelled() => {
                            drop(file);
                            let _ = tokio::fs::remove_file(&dest_path).await;
                            record.status = "cancelled".to_string();
                            record.bytes_downloaded = downloaded;
                            return Ok(());
                        }
                        chunk_res = resp.chunk() => {
                            match chunk_res? {
                                Some(bytes) => {
                                    file.write_all(&bytes).await?;
                                    let chunk_len = bytes.len() as u64;
                                    downloaded += chunk_len;
                                    bytes_since_emit += chunk_len;
                                    record.bytes_downloaded = downloaded;

                                    if last_emit.elapsed() >= Duration::from_millis(80)
                                        || bytes_since_emit >= 65_536
                                    {
                                        last_emit = Instant::now();
                                        bytes_since_emit = 0;
                                        {
                                            let mut guard = db.lock().await;
                                            let _ = guard.upsert_download(&record);
                                        }
                                        let _ = app.emit("download:progress", &record);
                                    }
                                }
                                None => break,
                            }
                        }
                    }
                }

                file.flush().await?;
                record.bytes_downloaded = downloaded;
                record.status = "completed".to_string();
                Ok(())
            }
            .await;

            {
                let mut tokens = tokens_map.lock().await;
                tokens.remove(&id);
            }

            match outcome {
                Ok(()) => {
                    {
                        let mut guard = db.lock().await;
                        let _ = guard.upsert_download(&record);
                    }
                    if record.status == "cancelled" {
                        let _ = app.emit("download:progress", &record);
                    } else {
                        let _ = app.emit("download:completed", &record);
                    }
                }
                Err(err) => {
                    let _ = tokio::fs::remove_file(&dest_path).await;
                    record.status = "failed".to_string();
                    record.error = Some(err.to_string());
                    {
                        let mut guard = db.lock().await;
                        let _ = guard.upsert_download(&record);
                    }
                    let _ = app.emit("download:failed", &record);
                }
            }
        });

        Ok(initial_record)
    }
}
