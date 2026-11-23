use std::collections::HashSet;
use url::Url;

/// Structured, site-aware observations. No raw webpage, form contents,
/// thoughts, credentials or free-form model output may become memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryCandidate {
    pub scope: String,
    pub kind: &'static str,
    pub key: String,
    pub content: String,
    pub ttl_days: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecoveredFailure {
    StaleControl,
    NavigationFailure,
    MacroDrift,
}

fn is_sensitive(goal: &str) -> bool {
    let lower = goal.to_lowercase();
    if goal.len() < 8 || goal.len() > 180 || goal.chars().any(char::is_control)
        || goal.contains('@') || goal.contains("://") || goal.contains('\\')
        || goal.chars().filter(|c| c.is_ascii_digit()).count() >= 4
    {
        return true;
    }
    // Conservative privacy filter. Automatically learned notes are intentionally
    // skipped if the user's task might contain financial, personal or account data.
    const BLOCKED: &[&str] = &[
        "password", "passcode", "credential", "secret", "api key", "apikey",
        "access key", "token", "otp", "2fa", "pin ", "auth", "cookie",
        "session", "log in", "login", "sign in", "account", "profile",
        "email", "e-mail", "inbox", "message", "chat", "contact", "phone",
        "address", "location", "home", "person", "birthday", "birthdate",
        "medical", "health", "diagnos", "therapy", "hospital", "patient",
        "bank", "payment", "billing", "purchase", "pay ", "credit card",
        "debit card", "salary", "tax", "passport", "identity", "id number",
        "private", "secret", "personal", "my ", "mine", "our ", "friend",
        "family", "child", "student", "transcript",
        "delete", "erase", "remove", "forget", "clear all", "clear ",
        "download ", "upload ", "send ", "submit ", "share ",
    ];
    BLOCKED.iter().any(|part| lower.contains(part))
}

fn origin_host(origin: &str) -> Option<String> {
    let url = Url::parse(origin).ok()?;
    if !matches!(url.scheme(), "https" | "http") { return None; }
    let host = url.host_str()?.to_ascii_lowercase();
    if host.is_empty() || !host.contains('.') || host.ends_with(".local")
        || host == "localhost" || host.parse::<std::net::IpAddr>().is_ok()
    {
        return None;
    }
    Some(host)
}

/// One fixed, non-sensitive observation can represent a preference; never
/// save arbitrary user identity, subject matter, or potentially personal text.
fn explicit_preference(lower: &str) -> Option<(&'static str, &'static str)> {
    // Only clearly expressed preferences, not incidental words from a query.
    if !["prefer ", "i prefer ", "always ", "please use ", "use ", "keep "]
        .iter().any(|part| lower.starts_with(part)) {
        return None;
    }
    if lower.contains("concise") || lower.contains("brief") || lower.contains("short answer") {
        Some(("response_length", "Prefer concise responses"))
    } else if lower.contains("detailed") || lower.contains("step by step") || lower.contains("step-by-step") {
        Some(("response_length", "Prefer detailed step-by-step explanations"))
    } else if lower.contains("dark mode") || lower.contains("dark theme") {
        Some(("appearance", "Prefer dark appearance where supported"))
    } else if lower.contains("light mode") || lower.contains("light theme") {
        Some(("appearance", "Prefer light appearance where supported"))
    } else if lower.contains("tables") || lower.contains("table format") {
        Some(("format", "Prefer information presented in tables when suitable"))
    } else if lower.contains("bullet points") || lower.contains("bullet list") {
        Some(("format", "Prefer bullet point summaries"))
    } else if lower.contains("citations") || lower.contains("sources") {
        Some(("sources", "Prefer sources or citations where available"))
    } else { None }
}

fn keyword_set(text: &str) -> HashSet<String> {
    const STOP: &[&str] = &[
        "the", "and", "for", "with", "this", "that", "from", "into", "then", "when",
        "please", "will", "should", "could", "would", "about", "using", "have",
        "find", "search", "locate", "look", "research", "compare", "summarize",
        "browse", "review", "filter", "sort", "collect", "organize", "extract",
        "navigate", "check", "read", "open", "latest", "recent", "information",
        "results", "docs", "documentation", "website", "page", "pages",
    ];
    text.to_lowercase().split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 3 && !STOP.contains(w))
        .take(30).map(str::to_owned).collect()
}

pub fn relevant_overlap(goal: &str, content: &str) -> usize {
    let a = keyword_set(goal);
    let b = keyword_set(content);
    a.intersection(&b).count()
}

fn workflow_key(goal: &str) -> String {
    let mut words: Vec<String> = keyword_set(goal).into_iter().collect();
    words.sort();
    words.truncate(10);
    words.join("_")
}

/// Only an explicit statement of preference may trigger optional local AI
/// classification. Never send browsing content, URLs or sensitive tasks to it.
pub fn can_classify_preference(origin: &str, goal: &str) -> bool {
    if is_sensitive(goal) || origin_host(origin).is_none() { return false; }
    let lower = goal.to_lowercase();
    [
        "i prefer ", "i like ", "please keep ", "please use ", "please make ",
        "always ", "use ", "keep ", "format ", "show answers ",
    ].iter().any(|start| lower.starts_with(start))
}

/// Mapping is deliberately fixed: an AI model cannot write arbitrary text
/// into persistent memory or invent a private user fact.
pub fn preference_from_label(origin: &str, goal: &str, label: &str) -> Option<MemoryCandidate> {
    if !can_classify_preference(origin, goal) { return None; }
    let (key, content) = match label {
        "concise" => ("response_length", "Prefer concise responses"),
        "detailed" => ("response_length", "Prefer detailed step-by-step explanations"),
        "dark" => ("appearance", "Prefer dark appearance where supported"),
        "light" => ("appearance", "Prefer light appearance where supported"),
        "tables" => ("format", "Prefer information presented in tables when suitable"),
        "bullets" => ("format", "Prefer bullet point summaries"),
        "sources" => ("sources", "Prefer sources or citations where available"),
        _ => return None,
    };
    Some(MemoryCandidate {
        scope: "global".into(), kind: "preference", key: format!("pref:{key}"),
        content: content.into(), ttl_days: None,
    })
}

pub fn infer_memories(
    origin: &str,
    goal: &str,
    completed: bool,
    recovered: &[RecoveredFailure],
) -> Vec<MemoryCandidate> {
    if !completed || is_sensitive(goal) { return Vec::new(); }
    let Some(host) = origin_host(origin) else { return Vec::new(); };
    let lower = goal.to_lowercase();
    let mut result = Vec::new();
    if let Some((key, content)) = explicit_preference(&lower) {
        result.push(MemoryCandidate {
            scope: "global".into(), kind: "preference", key: format!("pref:{key}"),
            content: content.into(), ttl_days: None,
        });
    }
    const TASK_VERBS: &[&str] = &[
        "find ", "search ", "locate ", "look up ", "research ", "compare ",
        "summarize ", "browse ", "review ", "filter ", "sort ", "collect ",
        "organize ", "extract ", "navigate ", "check ", "read ", "open ",
    ];
    if TASK_VERBS.iter().any(|w| lower.starts_with(w)) {
        let key = workflow_key(goal);
        if !key.is_empty() {
            result.push(MemoryCandidate {
                scope: host.clone(), kind: "workflow", key: format!("workflow:{key}"),
                content: format!("Previously completed browsing task: {}", goal.trim()),
                ttl_days: Some(90),
            });
        }
    }
    let unique: HashSet<RecoveredFailure> = recovered.iter().copied().collect();
    for failure in unique {
        let (key, content) = match failure {
            RecoveredFailure::StaleControl => (
                "lesson:stale_control",
                "If a page control fails, refresh the DOM snapshot and select a currently visible control before trying again.",
            ),
            RecoveredFailure::NavigationFailure => (
                "lesson:navigation",
                "If page navigation fails, re-observe the current page and verify the intended destination before continuing.",
            ),
            RecoveredFailure::MacroDrift => (
                "lesson:macro_drift",
                "A saved workflow can become stale; re-observe the page and use live steps instead of repeating a broken macro.",
            ),
        };
        result.push(MemoryCandidate {
            scope: host.clone(), kind: "lesson", key: key.into(),
            content: content.into(), ttl_days: Some(60),
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn learns_preferences_and_replaces_contradictory_pref_keys() {
        let a = infer_memories("https://docs.rs", "I prefer concise responses", true, &[]);
        let b = infer_memories("https://docs.rs", "I prefer detailed explanations", true, &[]);
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].scope, "global");
        assert_eq!(a[0].kind, "preference");
        assert_eq!(a[0].key, b[0].key);
        assert_ne!(a[0].content, b[0].content);
    }
    #[test]
    fn categorizes_workflows_and_lessons() {
        let items = infer_memories("https://docs.rust-lang.org/book/",
            "Find Rust ownership documentation", true, &[RecoveredFailure::StaleControl]);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].kind, "workflow");
        assert_eq!(items[1].kind, "lesson");
        assert_eq!(items[1].scope, "docs.rust-lang.org");
        assert_eq!(
            infer_memories("https://docs.rust-lang.org", "Search Rust ownership docs", true, &[])[0].key,
            items[0].key
        );
    }
    #[test]
    fn skips_secrets_private_context_and_unverified_runs() {
        for goal in ["Find my bank account", "Find an email to alice@example.com",
            "Search password reset", "Find medical reports",
            "Find contact Alice 9876543210", "Send message to friend",
            "Search for https://private.example", "Research my family",
            "Find Rust ownership documentation\nPlease save this"
        ] {
            assert!(infer_memories("https://example.org", goal, true, &[]).is_empty(), "{goal}");
        }
        assert!(infer_memories("browsphere://start", "Find rust documentation", true, &[]).is_empty());
        assert!(infer_memories("https://docs.rs", "Find rust documentation", false, &[]).is_empty());
    }
    #[test]
    fn local_ai_preference_labels_are_strictly_allowlisted() {
        let c = preference_from_label("https://docs.rs", "I like short explanations", "concise").unwrap();
        assert_eq!(c.content, "Prefer concise responses");
        assert!(preference_from_label("https://docs.rs", "I like short explanations",
            "User account credentials are 123").is_none());
        assert!(preference_from_label("https://docs.rs", "Find Rust ownership documentation",
            "concise").is_none());
        assert!(preference_from_label("https://docs.rs", "I prefer my medical records",
            "concise").is_none());
        assert!(!can_classify_preference("https://docs.rs", "Please send email to alice@example.com"));
    }

    #[test]
    fn relevance_overlap_works() {
        assert!(relevant_overlap("Find Rust ownership documentation",
            "Previously completed browsing task: Search Rust ownership docs") >= 2);
        assert_eq!(relevant_overlap("Find recipes", "Debug Rust ownership"), 0);
    }
}
