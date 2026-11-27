use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::io::Write;
use std::sync::RwLock;
use tauri::webview::{PermissionKind, PermissionResponse};

const SUPPORTED: &[&str] = &["camera", "microphone", "geolocation", "notifications"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SitePolicy {
    pub origin: String,
    pub granted: Vec<String>,
}

// Persist in the same directory via a unique temporary file. A disk error
// must not truncate the previously accepted permission record, and an
// in-memory grant must never be acknowledged before the rename succeeds.
fn atomic_write_permissions(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().ok_or_else(||std::io::Error::new(
        std::io::ErrorKind::InvalidInput,"Missing permission-store directory"))?;
    let temp = parent.join(format!("site-permissions-{}.tmp",uuid::Uuid::new_v4()));
    let attempt = (||{
        let mut output = std::fs::OpenOptions::new().create_new(true).write(true).open(&temp)?;
        output.write_all(bytes)?;
        output.sync_all()?;
        std::fs::rename(&temp, path)
    })();
    if attempt.is_err() { let _ = std::fs::remove_file(&temp); }
    attempt
}

#[derive(Default)]
pub struct SitePermissions {
    file: PathBuf,
    grants: RwLock<BTreeMap<String, BTreeSet<String>>>,
}

impl SitePermissions {
    pub fn new(data_dir: &std::path::Path) -> Self {
        let file = data_dir.join("site-permissions.json");
        let saved = std::fs::read(&file)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<BTreeMap<String, BTreeSet<String>>>(&bytes).ok())
            .unwrap_or_default();
        let mut cleaned = BTreeMap::new();
        for (origin, permissions) in saved.into_iter().take(300) {
            if let Some(canonical) = normalize_origin(&origin) {
                if canonical == origin {
                    let accepted: BTreeSet<_> = permissions
                        .into_iter()
                        .filter(|p| SUPPORTED.contains(&p.as_str()))
                        .collect();
                    if !accepted.is_empty() { cleaned.insert(origin, accepted); }
                }
            }
        }
        Self { file, grants: RwLock::new(cleaned) }
    }

    pub fn list(&self, origin: &str) -> Result<SitePolicy, String> {
        let canonical = normalize_origin(origin).ok_or("Not a secure website origin")?;
        if canonical != origin { return Err("Origin must be canonical".into()); }
        let grants = self.grants.read().map_err(|e| e.to_string())?;
        Ok(SitePolicy {
            origin: canonical.clone(),
            granted: grants.get(&canonical).map(|p| p.iter().cloned().collect()).unwrap_or_default()
        })
    }

    pub fn update(&self, origin: &str, permission: &str, allowed: bool) -> Result<SitePolicy, String> {
        let canonical = normalize_origin(origin).ok_or("Not a secure website origin")?;
        if canonical != origin { return Err("Origin must be canonical".into()); }
        if !SUPPORTED.contains(&permission) { return Err("Unsupported site permission".into()); }
        let mut grants = self.grants.write().map_err(|e| e.to_string())?;
        if allowed && !grants.contains_key(origin) && grants.len() >= 300 {
            return Err("Maximum saved site permissions reached".into());
        }
        let before = grants.clone();
        if allowed {
            grants.entry(origin.to_owned()).or_default().insert(permission.to_owned());
        } else if let Some(permissions) = grants.get_mut(origin) {
            permissions.remove(permission);
            if permissions.is_empty() { grants.remove(origin); }
        }
        // Never acknowledge a grant unless it has actually reached disk.
        let serialized = serde_json::to_vec_pretty(&*grants).map_err(|e| e.to_string())?;
        if let Err(error) = atomic_write_permissions(&self.file, &serialized) {
            *grants = before;
            return Err(format!("Could not save site permissions: {error}"));
        }
        Ok(SitePolicy {
            origin: canonical,
            granted: grants.get(origin).map(|p| p.iter().cloned().collect()).unwrap_or_default(),
        })
    }

    pub fn response(&self, url: &url::Url, permission: PermissionKind, private: bool) -> PermissionResponse {
        if private { return PermissionResponse::Deny; }
        let permission = permission.to_string();
        if !SUPPORTED.contains(&permission.as_str()) { return PermissionResponse::Deny; }
        let Some(origin) = canonical_origin(url) else { return PermissionResponse::Deny; };
        match self.grants.read() {
            Ok(guard) if guard.get(&origin).is_some_and(|grants| grants.contains(&permission)) => PermissionResponse::Allow,
            _ => PermissionResponse::Deny
        }
    }
}

// Only HTTPS web origins receive saved permissions. Other schemes and embedded
// usernames/passwords are rejected rather than being normalized into grants.
fn canonical_origin(url: &url::Url) -> Option<String> {
    if url.scheme() != "https" || url.host_str().is_none()
        || !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    Some(url.origin().ascii_serialization())
}

fn normalize_origin(raw: &str) -> Option<String> {
    let parsed = url::Url::parse(raw).ok()?;
    let result = canonical_origin(&parsed)?;
    if raw != result { return None; }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::webview::{PermissionKind, PermissionResponse};
    #[test]
    fn only_secure_exact_origins_are_eligible() {
        assert_eq!(normalize_origin("https://example.com"), Some("https://example.com".into()));
        assert_eq!(normalize_origin("https://example.com:8443"), Some("https://example.com:8443".into()));
        for bad in ["http://example.com", "https://example.com/path", "https://other.com@foo.com",
            "https://example.com?token=x", "javascript:alert(1)", "https://example.com/"] {
            assert!(normalize_origin(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn default_denies_and_private_is_never_granted() {
        let dir = std::env::temp_dir().join(format!("browsphere-site-perm-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = SitePermissions::new(&dir);
        let url = url::Url::parse("https://example.com/path").unwrap();
        assert_eq!(store.response(&url, PermissionKind::Camera, false), PermissionResponse::Deny);
        assert!(store.update("https://example.com", "camera", true).unwrap().granted.contains(&"camera".into()));
        assert_eq!(store.response(&url, PermissionKind::Camera, false), PermissionResponse::Allow);
        assert_eq!(store.response(&url, PermissionKind::Camera, true), PermissionResponse::Deny);
        assert_eq!(store.response(&url, PermissionKind::DisplayCapture, false), PermissionResponse::Deny);
        assert_eq!(store.response(&url::Url::parse("https://other.example/path").unwrap(), PermissionKind::Camera, false), PermissionResponse::Deny);
        let restored = SitePermissions::new(&dir);
        assert_eq!(restored.response(&url, PermissionKind::Camera, false), PermissionResponse::Allow);
        restored.update("https://example.com", "camera", false).unwrap();
        assert_eq!(restored.response(&url, PermissionKind::Camera, false), PermissionResponse::Deny);
        assert!(restored.update("http://example.com", "camera", true).is_err());
        assert!(restored.update("https://example.com", "file-system-access", true).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn failed_atomic_write_does_not_acknowledge_or_leak_grants() {
        let dir = std::env::temp_dir().join(format!("browsphere-perm-write-{}",uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        // A directory where the JSON file should be forces rename failure
        // consistently on Windows and Linux without changing ACLs.
        std::fs::create_dir(dir.join("site-permissions.json")).unwrap();
        let store = SitePermissions::new(&dir);
        assert!(store.update("https://example.com","camera",true).is_err());
        assert!(store.list("https://example.com").unwrap().granted.is_empty());
        let leftover:Vec<_>=std::fs::read_dir(&dir).unwrap().filter_map(Result::ok)
            .filter(|e|e.file_name().to_string_lossy().ends_with(".tmp")).collect();
        assert!(leftover.is_empty(),"Failed permission writes must not leave temporary credential-like state");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn successful_permission_update_atomically_replaces_prior_grant() {
        let dir = std::env::temp_dir().join(format!("browsphere-perm-replace-{}",uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = SitePermissions::new(&dir);
        store.update("https://example.com","camera",true).unwrap();
        store.update("https://example.com","microphone",true).unwrap();
        let restored = SitePermissions::new(&dir);
        assert_eq!(restored.list("https://example.com").unwrap().granted,
            vec!["camera".to_string(),"microphone".to_string()]);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
