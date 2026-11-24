//! Conservative recovery for authentication blockers. No credentials or
//! alternative-site permissions are inferred from untrusted page content.
use super::dom_extractor::DomSnapshot;

pub fn authentication_gate(page: &DomSnapshot) -> bool {
    let title = page.title.to_ascii_lowercase();
    let url = url::Url::parse(&page.origin).ok();
    let host = url.as_ref().and_then(|u| u.host_str()).unwrap_or("");
    let path = url.as_ref().map(|u| u.path().to_ascii_lowercase()).unwrap_or_default();
    let auth_title = ["sign in", "sign-in", "signin", "log in", "login", "log-in", "authentication required"]
        .iter().any(|phrase| title.contains(phrase));
    let auth_path = path.split('/').any(|part| matches!(part, "login" | "signin" | "sign-in" | "authenticate" | "oauth" | "authorize"));
    let has_account_input = page.interactive_nodes.iter().any(|node| {
        let label = node.label.to_ascii_lowercase();
        (matches!(node.tag.as_str(), "input" | "textarea") || node.role == "textbox") &&
        (node.is_sensitive || ["email", "username", "phone", "password", "verification code"]
            .iter().any(|phrase| label.contains(phrase)))
    });
    (host == "accounts.google.com" && (auth_title || auth_path || has_account_input))
        || (auth_title && (auth_path || has_account_input))
}

pub fn alternatives_authorized(goal: &str) -> bool {
    let lower = goal.to_ascii_lowercase();
    if ["no alternative", "do not use an alternative", "don't use an alternative", "only use", "must use only"]
        .iter().any(|phrase| lower.contains(phrase)) { return false; }
    ["find an alternative", "find alternatives", "find alternative", "search for an alternative",
     "search for alternative", "look for an alternative", "use an alternative", "use another",
     "find another", "try another", "any equivalent", "or an equivalent", "alternative if",
     "if blocked, use", "if unavailable, use"]
        .iter().any(|phrase| lower.contains(phrase))
}

pub const RECOVERY_HINT: &str = "Recovery directive: An authentication gate blocks this task. The user permits an alternative. Do not enter credentials, create accounts, or try to bypass sign-in. Choose a new strategy: navigate to the known search homepage https://www.google.com/ to find an equivalent public service if no safe alternate link is observed. Search only generic service requirements, not private task content. Keep the original goal and verify the alternative result. Existing navigation and other approvals still apply. Do not report completion on a login page.";

pub const FOLLOWUP_HINT: &str = "Recovery directive: The original service required authentication. The user permits an equivalent service. Continue the alternative discovery workflow, use only observed search controls and results, avoid private data and publishing, preserve all approvals, and verify the resulting artifact. Do not return to the blocked sign-in page or claim the original service was used.";

/// Accept only a loaded Google results page with the same nonempty decoded query.
/// Extra provider-generated tracking parameters are not a different search.
pub fn search_results(page: &DomSnapshot) -> bool {
    let Ok(url) = url::Url::parse(&page.origin) else { return false };
    url.scheme() == "https" && url.host_str() == Some("www.google.com")
        && url.path() == "/search"
        && url.query_pairs().any(|(k,v)| k == "q" && !v.trim().is_empty())
        && page.title.to_ascii_lowercase().contains("google search")
        && super::evidence_verifier::check_navigation_result(&page.origin, page).is_ok()
}

pub fn navigation_confirmed(requested: &str, page: &DomSnapshot) -> bool {
    if !search_results(page) { return false; }
    let (Ok(wanted), Ok(actual)) = (url::Url::parse(requested), url::Url::parse(&page.origin)) else { return false };
    if wanted.scheme() != "https" || wanted.host_str() != Some("www.google.com")
        || wanted.path() != "/search" || !wanted.username().is_empty() || wanted.password().is_some() { return false; }
    let query = |u: &url::Url| u.query_pairs().find(|(k,_)| k == "q").map(|(_,v)|v.into_owned());
    query(&wanted).is_some_and(|q| !q.trim().is_empty() && query(&actual).as_deref() == Some(q.as_str()))
}

pub const RESULTS_HINT: &str = "Recovery directive: Alternative search results are already loaded. Do not reload or repeat the query. Select a relevant public service from the observed result links, preferably a private no-login editor that meets the original task. Existing approvals still apply. Search results are not the completed artifact.";

#[cfg(test)]
mod tests {
    use super::*;
    fn page(origin:&str,title:&str)->DomSnapshot {
        DomSnapshot {origin:origin.into(), title:title.into(), text_excerpt:String::new(), interactive_nodes:vec![]}
    }
    #[test]
    fn login_gate_is_grounded_in_native_location_and_title() {
        assert!(authentication_gate(&page("https://accounts.google.com/v3/signin/identifier","Google Docs: Sign-in")));
        assert!(authentication_gate(&page("https://example.org/login","Sign in")));
        assert!(!authentication_gate(&page("https://example.org/help","How to sign in")));
        assert!(!authentication_gate(&page("https://docs.google.com/document/u/0/","Google Docs")));
    }
    #[test]
    fn search_navigation_accepts_only_the_same_observed_query() {
        let p=page("https://www.google.com/search?q=online+editor&source=hp","online editor - Google Search");
        assert!(navigation_confirmed("https://www.google.com/search?q=online%20editor",&p));
        assert!(!navigation_confirmed("https://www.google.com/search?q=other",&p));
        assert!(!navigation_confirmed("https://www.google.com/",&p));
    }
    #[test]
    fn fallback_requires_explicit_permission_and_respects_negative_constraints() {
        assert!(alternatives_authorized("Write a story in Google Docs; if unavailable, use an alternative."));
        assert!(alternatives_authorized("If blocked, search for alternative editors."));
        assert!(!alternatives_authorized("Write a story in Google Docs"));
        assert!(!alternatives_authorized("Only use Google Docs; do not use an alternative"));
    }
}
