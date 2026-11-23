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
        let status = self.outcome.trim().to_ascii_uppercase();
        // Only the trusted execution STATUS prefix determines the audit result.
        // Browser content and verifier summaries are untrusted free text: a
        // successful report that mentions an earlier "failed click" must not
        // be logged as a failed verifier action.
        self.outcome = if status.starts_with("REJECTED") { "rejected" }
            else if status.starts_with("APPROVAL TIMED OUT") { "timed_out" }
            else if status.starts_with("UNVERIFIED") { "unverified" }
            else if status.starts_with("DENIED") { "denied" }
            else if status.starts_with("FAILED")
                || status.starts_with("ABORTED")
                || status.starts_with("HALTED")
                || status.starts_with("APPROVED BUT FAILED")
                || status.starts_with("REPLAY DRIFT") { "failed" }
            else if status.starts_with("POSTCONDITION CONFIRMED") { "confirmed" }
            else if status.starts_with("OK:")
                || status.starts_with("APPROVED & OK:")
                || status.starts_with("COMPLETED:") { "ok" }
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


/// Only the user's short, explicit navigation request can authorize opening a
/// new site without a second approval. Model and webpage text never qualify.
pub fn requested_navigation_url(goal: &str) -> Option<Url> {
    let g = goal.trim().trim_end_matches(['.', '!', '?']).trim().to_ascii_lowercase();
    if g.chars().any(char::is_control) || g.len() > 220 { return None; }
    let value = ["open ", "visit ", "go to ", "navigate to "].iter()
        .find_map(|prefix| g.strip_prefix(prefix))?.trim();
    let value = value.strip_prefix("the ").unwrap_or(value);
    let value = value.strip_suffix(" in the browser").or_else(|| value.strip_suffix(" in browser"))
        .or_else(|| value.strip_suffix(" in a new tab")).unwrap_or(value);
    let value = value.strip_suffix(" website").or_else(|| value.strip_suffix(" site"))
        .unwrap_or(value).trim();
    let url = match value {
        "youtube" | "youtube.com" | "www.youtube.com" => "https://www.youtube.com/".to_string(),
        "google" => "https://www.google.com/".to_string(),
        "github" => "https://github.com/".to_string(),
        "wikipedia" => "https://www.wikipedia.org/".to_string(),
        other if other.starts_with("https://") || other.starts_with("http://") => other.to_string(),
        other if other.contains('.')
            && other.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()
                || b == b'.' || b == b'-')
            && !other.starts_with('.') && !other.ends_with('.')
            && !other.contains("..") =>
                format!("https://{other}/"),
        _ => return None,
    };
    let parsed = Url::parse(&url).ok()?;
    if !matches!(parsed.scheme(), "https" | "http")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty() || parsed.password().is_some()
    { return None; }
    // Reject local network addresses and localhost: direct navigation is allowed,
    // but the agent must not silently cross into private services.
    let host = parsed.host_str()?.to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".local")
        || host.parse::<std::net::IpAddr>().is_ok()
    { return None; }
    Some(parsed)
}

/// A user who asks to search a named public website has already authorized
/// ordinary navigation to that website. Recognize only explicit request shapes,
/// never suggestions found in page content or generated model thoughts.
fn requested_search_host(goal: &str) -> Option<&'static str> {
    let normalized = goal.trim().to_ascii_lowercase();
    for (name, host) in [
        ("youtube", "www.youtube.com"),
        ("google", "www.google.com"),
        ("wikipedia", "www.wikipedia.org"),
        ("github", "github.com"),
    ] {
        if normalized.starts_with(&format!("search {name} for "))
            || normalized.starts_with(&format!("search on {name} for "))
            || (normalized.starts_with("search for ")
                && normalized.contains(&format!(" on {name}")))
        {
            return Some(host);
        }
    }
    None
}

/// The home URL is derived from the user's explicit search destination,
/// never from website instructions or the model's preferred destination.
pub fn requested_search_entry_url(goal: &str) -> Option<Url> {
    Url::parse(&format!("https://{}/", requested_search_host(goal)?)).ok()
}

/// Preserve existing approval/deny policy for any target not explicitly
/// authorized by the user's own goal. Do not override run allowlists.
pub fn evaluate_user_navigation_policy(
    active_origin: &str, action: &BrowserAction, allowed_domains: &[String], goal: &str
) -> (RiskLevel, PolicyDecision) {
    let base = evaluate_policy(active_origin, action, allowed_domains);
    let BrowserAction::Navigate { url } = action else { return base };
    let PolicyDecision::RequireApproval { ref reason } = base.1 else { return base };
    if !reason.starts_with("Cross-origin navigation to ") { return base; }
    let Ok(actual) = Url::parse(url) else { return base };
    let user_explicitly_named_destination = requested_navigation_url(goal)
        .is_some_and(|expected| actual == expected);
    let user_explicitly_named_search_site = requested_search_host(goal)
        .is_some_and(|host| actual.scheme() == "https"
            && actual.host_str() == Some(host));
    if (user_explicitly_named_destination || user_explicitly_named_search_site)
        && actual.host_str().is_some_and(|host| host_matches_allowlist(host, allowed_domains))
    {
        return (RiskLevel::Low, PolicyDecision::Allow);
    }
    base
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
                // Routine form actions are part of the user's authorized task.
                // Only consequential commitments require an explicit decision.
                // This label is rebound to the observed DOM before evaluation.
                let consequential = [
                    "application", "apply now", "register", "sign up", "create account",
                    "subscribe", "publish", "post publicly", "send email",
                    "send message", "send invitation", "sign contract", "agree to terms",
                    "accept offer", "finalize", "place order", "book now",
                    "reserve", "submit identity", "upload id", "submit tax",
                    "confirm", "consent", "authorize",
                ];
                if consequential.iter().any(|kw| label_lower.contains(kw)) {
                    return (
                        RiskLevel::High,
                        PolicyDecision::RequireApproval {
                            reason: format!(
                                "Consequential form submission via '{element_label}' ({node_id})"
                            ),
                        },
                    );
                }
                return (RiskLevel::Medium, PolicyDecision::Allow);
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
    fn trace_outcome_reads_execution_status_not_untrusted_summary_words() {
        let entry = |outcome: &str| TraceEntry {
            trace_id: Uuid::nil(),
            run_id: Uuid::nil(),
            step_index: 0,
            active_origin: "https://example.org/private?q=never-log".into(),
            thought: "Do not persist model rationale".into(),
            action: BrowserAction::Complete { summary: "Untrusted summary".into() },
            risk: RiskLevel::Low,
            decision: PolicyDecision::Allow,
            model_route: "test".into(),
            latency_ms: 0,
            outcome: outcome.into(),
        };
        assert_eq!(entry("OK: COMPLETED: Recovery succeeded after the first click failed")
            .sanitized().outcome, "ok");
        assert_eq!(entry("OK: Clicked button labelled rejected request")
            .sanitized().outcome, "ok");
        assert_eq!(entry("POSTCONDITION CONFIRMED: OK: Navigation acknowledged")
            .sanitized().outcome, "confirmed");
        assert_eq!(entry("FAILED: WebView2 action result unavailable")
            .sanitized().outcome, "failed");
        assert_eq!(entry("DENIED: Blocked by policy")
            .sanitized().outcome, "denied");
        assert_eq!(entry("APPROVAL TIMED OUT").sanitized().outcome, "timed_out");
        assert_eq!(entry("UNVERIFIED ACTION EFFECT: No snapshot")
            .sanitized().outcome, "unverified");
    }

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
    fn explicit_navigation_skips_only_matching_cross_site_approval() {
        let navigate = |url: &str| BrowserAction::Navigate { url: url.into() };
        let origin = "https://example.com/";
        assert_eq!(requested_navigation_url("open youtube").unwrap().as_str(), "https://www.youtube.com/");
        assert!(matches!(evaluate_user_navigation_policy(origin,
            &navigate("https://www.youtube.com/"), &[], "open youtube").1,
            PolicyDecision::Allow));
        for goal in ["research youtube", "open youtube and delete my account", "open youtube; buy something",
            "open youtube.example", "open localhost"] {
            assert!(!matches!(evaluate_user_navigation_policy(origin,
                &navigate("https://www.youtube.com/"), &[], goal).1, PolicyDecision::Allow), "{goal}");
        }
        assert!(matches!(evaluate_user_navigation_policy(origin,
            &navigate("https://www.youtube.com.evil.test/"), &[], "open youtube").1,
            PolicyDecision::RequireApproval { .. }));
        assert!(matches!(evaluate_user_navigation_policy(origin,
            &navigate("https://www.youtube.com/account"), &[], "open youtube").1,
            PolicyDecision::RequireApproval { .. }));
        assert!(matches!(evaluate_user_navigation_policy(origin,
            &navigate("https://www.youtube.com/"), &["example.com".into()], "open youtube").1,
            PolicyDecision::RequireApproval { .. }));
        assert!(matches!(evaluate_user_navigation_policy(origin,
            &navigate("javascript:alert(1)"), &[], "open javascript:alert(1)").1,
            PolicyDecision::Deny { .. }));
    }

    #[test]
    fn search_named_website_can_navigate_without_second_consent() {
        let goal = "Search YouTube for NASA Artemis mission and open a video result";
        let nav = |url: &str| BrowserAction::Navigate { url: url.into() };
        assert_eq!(requested_search_host(goal), Some("www.youtube.com"));
        assert_eq!(requested_search_entry_url(goal).unwrap().as_str(),
            "https://www.youtube.com/");
        assert!(requested_search_entry_url("Show me a funny video").is_none());
        assert!(matches!(evaluate_user_navigation_policy("about:blank",
            &nav("https://www.youtube.com/"), &[], goal).1,
            PolicyDecision::Allow));
        assert!(matches!(evaluate_user_navigation_policy("about:blank",
            &nav("https://www.youtube.com/results?search_query=nasa"), &[], goal).1,
            PolicyDecision::Allow));
        // No permission silently extends to a different host or explicit run block.
        for bad in ["https://www.youtube.com.attacker.example/",
            "https://accounts.google.com/", "http://www.youtube.com/"] {
            assert!(matches!(evaluate_user_navigation_policy("about:blank",
                &nav(bad), &[], goal).1, PolicyDecision::RequireApproval { .. }));
        }
        assert!(matches!(evaluate_user_navigation_policy("about:blank",
            &nav("https://www.youtube.com/"), &["example.org".into()], goal).1,
            PolicyDecision::RequireApproval { .. }));
        assert_eq!(requested_search_host("I do not want to use YouTube"), None);
        assert_eq!(requested_search_host("search another site for youtube videos"), None);
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

#[cfg(test)]
mod autonomous_form_tests {
    use super::*;

    fn submission(label: &str) -> BrowserAction {
        BrowserAction::Click {
            node_id: "submit_btn".into(), element_label: label.into(), is_submit: true,
        }
    }

    #[test]
    fn routine_form_submissions_run_without_popup() {
        for label in ["Submit feedback", "Send search query", "Submit sample form", "Apply filters"] {
            let (risk, result) = evaluate_policy("https://example.org", &submission(label), &[]);
            assert!(matches!(result, PolicyDecision::Allow), "{label}");
            assert_eq!(risk, RiskLevel::Medium);
        }
    }

    #[test]
    fn consequential_submissions_still_require_confirmation() {
        for label in ["Submit application", "Register account", "Sign up", "Publish post publicly",
                      "Pay now", "Delete account", "Send money", "Finalize contract"] {
            let (_, result) = evaluate_policy("https://example.org", &submission(label), &[]);
            assert!(matches!(result, PolicyDecision::RequireApproval { .. }), "{label}");
        }
    }
}
