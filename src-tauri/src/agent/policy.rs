use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BrowserAction {
    Navigate { url: String },
    Click { node_id: String, element_label: String, is_submit: bool },
    TypeText { node_id: String, text: String, is_sensitive: bool },
    Scroll { delta_y: i32 },
    Download { url: String, suggested_filename: Option<String> },
    Complete { summary: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyDecision {
    Allow,
    RequireApproval { reason: String },
    Deny { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEntry {
    pub trace_id: Uuid,
    pub run_id: Uuid,
    pub step_index: u32,
    pub active_origin: String,
    pub thought: String,
    pub action: BrowserAction,
    pub risk: RiskLevel,
    pub decision: PolicyDecision,
    pub model_route: String,
    pub latency_ms: u64,
    pub outcome: String,
}

impl TraceEntry {
    /// Protect user text, URL parameters, and page/model content in persisted
    /// traces and UI events. This is a diagnostic record, not executable replay.
    pub fn sanitized(mut self) -> Self {
        fn site_only(raw: &str) -> String {
            Url::parse(raw)
                .ok()
                .filter(|u| matches!(u.scheme(), "http" | "https"))
                .map(|u| u.origin().ascii_serialization())
                .unwrap_or_else(|| "[unavailable site]".into())
        }
        self.active_origin = site_only(&self.active_origin);
        self.action = match self.action {
            BrowserAction::TypeText { node_id, is_sensitive, .. } =>
                BrowserAction::TypeText {
                    node_id, text: "[redacted]".into(), is_sensitive
                },
            BrowserAction::Navigate { url } =>
                BrowserAction::Navigate { url: site_only(&url) },
            BrowserAction::Download { url, .. } =>
                BrowserAction::Download {
                    url: site_only(&url), suggested_filename: None
                },
            BrowserAction::Click { node_id, is_submit, .. } =>
                BrowserAction::Click {
                    node_id, element_label: "[observed control]".into(), is_submit
                },
            BrowserAction::Complete { .. } =>
                BrowserAction::Complete { summary: "[stage completion]".into() },
            other => other,
        };
        self.thought = "[model rationale omitted from audit]".into();
        self.decision = match self.decision {
            PolicyDecision::RequireApproval { .. } =>
                PolicyDecision::RequireApproval { reason: "Approval required".into() },
            PolicyDecision::Deny { .. } =>
                PolicyDecision::Deny { reason: "Blocked by policy".into() },
            other => other,
        };
        let status = self.outcome.to_ascii_uppercase();
        self.outcome = if status.contains("REJECTED") { "rejected" }
            else if status.contains("TIMED OUT") { "timed_out" }
            else if status.contains("UNVERIFIED") { "unverified" }
            else if status.contains("DENIED") { "denied" }
            else if status.contains("FAILED") || status.contains("ABORTED") { "failed" }
            else if status.contains("CONFIRMED") { "confirmed" }
            else if status.contains("OK") || status.contains("COMPLETED") { "ok" }
            else { "stopped_or_unknown" }.into();
        self
    }
}

pub fn sanitize_untrusted_page_text(raw: &str, max_chars: usize) -> String {
    let trimmed: String = raw.chars().take(max_chars).collect();
    trimmed.replace("</untrusted_page_data>", "&lt;/untrusted_page_data&gt;")
}

fn host_matches_allowlist(host: &str, allowed_domains: &[String]) -> bool {
    if allowed_domains.is_empty() {
        return true;
    }
    let h = host.to_lowercase();
    allowed_domains.iter().any(|allowed| {
        let a = allowed.trim().to_lowercase();
        h == a || h.ends_with(&format!(".{a}"))
    })
}

pub fn evaluate_policy(
    active_origin: &str,
    action: &BrowserAction,
    allowed_domains: &[String],
) -> (RiskLevel, PolicyDecision) {
    match action {
        BrowserAction::Scroll { .. } | BrowserAction::Complete { .. } => {
            (RiskLevel::Low, PolicyDecision::Allow)
        }
        BrowserAction::Navigate { url } => {
            let parsed = match Url::parse(url) {
                Ok(u) => u,
                Err(_) => {
                    return (
                        RiskLevel::Critical,
                        PolicyDecision::Deny {
                            reason: "Malformed navigation URL".to_string(),
                        },
                    );
                }
            };

            match parsed.scheme() {
                "http" | "https" => {}
                scheme => {
                    return (
                        RiskLevel::Critical,
                        PolicyDecision::Deny {
                            reason: format!("Scheme '{scheme}:' is blocked by policy"),
                        },
                    );
                }
            }

            if !parsed.username().is_empty() || parsed.password().is_some() {
                return (RiskLevel::Critical, PolicyDecision::Deny {
                    reason: "Credentials embedded in navigation URLs are forbidden".to_string(),
                });
            }
            if let Some(host) = parsed.host_str() {
                if !host_matches_allowlist(host, allowed_domains) {
                    return (
                        RiskLevel::High,
                        PolicyDecision::RequireApproval {
                            reason: format!("Target host '{host}' is outside run allowlist"),
                        },
                    );
                }
                let current_host = Url::parse(active_origin)
                    .ok()
                    .and_then(|u| u.host_str().map(|s| s.to_lowercase()));
                if current_host.as_deref() != Some(&host.to_lowercase()) {
                    return (
                        RiskLevel::Medium,
                        PolicyDecision::RequireApproval {
                            reason: format!("Cross-origin navigation to '{host}'"),
                        },
                    );
                }
            }

            (RiskLevel::Low, PolicyDecision::Allow)
        }
        BrowserAction::Click {
            node_id,
            element_label,
            is_submit,
        } => {
            let label_lower = element_label.to_lowercase();
            let high_risk_keywords = [
                "pay", "checkout", "purchase", "buy now", "delete", "remove", "revoke",
                "transfer", "send money", "confirm order", "authorize",
            ];
            if high_risk_keywords.iter().any(|kw| label_lower.contains(kw)) {
                return (
                    RiskLevel::Critical,
                    PolicyDecision::RequireApproval {
                        reason: format!("High-consequence UI control '{element_label}' ({node_id})"),
                    },
                );
            }
            if *is_submit {
                return (
                    RiskLevel::High,
                    PolicyDecision::RequireApproval {
                        reason: format!("Form submission via '{element_label}' ({node_id})"),
                    },
                );
            }
            (RiskLevel::Medium, PolicyDecision::Allow)
        }
        BrowserAction::TypeText {
            node_id,
            text,
            is_sensitive,
        } => {
            let lower = text.to_lowercase();
            let secret_like = *is_sensitive
                || lower.contains("password")
                || lower.contains("otp")
                || lower.contains("cvv")
                || lower.contains("secret");
            if secret_like {
                return (
                    RiskLevel::High,
                    PolicyDecision::RequireApproval {
                        reason: format!("Sensitive input into node '{node_id}'"),
                    },
                );
            }
            (RiskLevel::Medium, PolicyDecision::Allow)
        }
        BrowserAction::Download { url, suggested_filename } => {
            let valid_target = Url::parse(url).ok().filter(|parsed| {
                matches!(parsed.scheme(), "http" | "https")
                    && parsed.host_str().is_some()
                    && parsed.username().is_empty()
                    && parsed.password().is_none()
            });
            if valid_target.is_none() {
                return (RiskLevel::Critical, PolicyDecision::Deny {
                    reason: "Download requires a web URL without embedded credentials".into(),
                });
            }
            let name = suggested_filename.as_deref().unwrap_or("file");
            (
                RiskLevel::High,
                PolicyDecision::RequireApproval {
                    reason: format!("Download '{name}' from '{url}'"),
                },
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_non_web_navigation() {
        let (_, decision) = evaluate_policy("https://example.com", &BrowserAction::Navigate { url: "javascript:alert(1)".into() }, &[]);
        assert!(matches!(decision, PolicyDecision::Deny { .. }));
    }

    #[test]
    fn cross_origin_requires_approval() {
        let (_, decision) = evaluate_policy("https://example.com", &BrowserAction::Navigate { url: "https://another.example/path".into() }, &[]);
        assert!(matches!(decision, PolicyDecision::RequireApproval { .. }));
    }

    #[test]
    fn sensitive_input_requires_approval() {
        let (_, decision) = evaluate_policy("https://example.com", &BrowserAction::TypeText { node_id: "login".into(), text: "example".into(), is_sensitive: true }, &[]);
        assert!(matches!(decision, PolicyDecision::RequireApproval { .. }));
    }
}

#[cfg(test)]
mod production_url_policy_tests {
    use super::*;

    #[test]
    fn rejects_credentialed_navigation_and_non_web_downloads() {
        for target in ["javascript:alert(1)", "file:///C:/secret.txt",
                       "https://user:password@example.org/private"] {
            let (_, decision) = evaluate_policy("https://example.org",
                &BrowserAction::Download { url: target.into(), suggested_filename: None }, &[]);
            assert!(matches!(decision, PolicyDecision::Deny { .. }), "{target}");
        }
        let (_, decision) = evaluate_policy("https://example.org",
            &BrowserAction::Navigate { url: "https://user:secret@example.org/".into() }, &[]);
        assert!(matches!(decision, PolicyDecision::Deny { .. }));
        let (_, decision) = evaluate_policy("https://example.org",
            &BrowserAction::Download {
                url: "https://example.org/report.pdf".into(),
                suggested_filename: Some("report.pdf".into()),
            }, &[]);
        assert!(matches!(decision, PolicyDecision::RequireApproval { .. }));
    }
}

#[cfg(test)]
mod production_trace_privacy_tests {
    use super::*;

    #[test]
    fn audit_record_cannot_expose_typed_values_or_secret_urls() {
        let trace = TraceEntry {
            trace_id: Uuid::new_v4(),
            run_id: Uuid::new_v4(),
            step_index: 1,
            active_origin: "https://example.org/profile?token=hidden-token#private".into(),
            thought: "User entered confidential phrase my-private-value".into(),
            action: BrowserAction::TypeText {
                node_id: "n_1".into(), text: "my-private-value".into(), is_sensitive: false
            },
            risk: RiskLevel::Medium,
            decision: PolicyDecision::RequireApproval {
                reason: "A private reason my-private-value".into()
            },
            model_route: "ollama:test".into(),
            latency_ms: 9,
            outcome: "OK: typed my-private-value".into(),
        };
        let audit = trace.sanitized();
        let json = serde_json::to_string(&audit).unwrap();
        assert!(!json.contains("my-private-value"));
        assert!(!json.contains("hidden-token"));
        assert!(!json.contains("/profile"));
        assert!(json.contains("https://example.org"));
        assert!(json.contains("[redacted]"));
        assert!(json.contains("Approval required"));
    }

    #[test]
    fn audit_drops_download_paths_and_form_completion_summaries() {
        let make = |action| TraceEntry {
            trace_id: Uuid::new_v4(),
            run_id: Uuid::new_v4(),
            step_index: 0,
            active_origin: "https://example.org/".into(),
            thought: "No details".into(),
            action,
            risk: RiskLevel::Low,
            decision: PolicyDecision::Allow,
            model_route: "test".into(),
            latency_ms: 0,
            outcome: "OK".into(),
        };
        let download = make(BrowserAction::Download {
            url: "https://example.org/private.pdf?session=hidden-token".into(),
            suggested_filename: Some("private-person.pdf".into()),
        }).sanitized();
        let serialized = serde_json::to_string(&download).unwrap();
        assert!(!serialized.contains("private.pdf"));
        assert!(!serialized.contains("private-person"));
        assert!(!serialized.contains("hidden-token"));

        let completion = make(BrowserAction::Complete {
            summary: "Personal account details".into(),
        }).sanitized();
        assert!(!serde_json::to_string(&completion).unwrap().contains("Personal account"));
    }
}
