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
