//! Evidence checks: LLM proposes claims; Rust matches source URLs and
//! exact supporting passages to WebView-captured page text.
//! Attribution is checked; webpages are not independently fact-checked.
use std::collections::HashSet;
use serde::{Deserialize, Serialize};
use url::Url;
use crate::agent::dom_extractor::DomSnapshot;
use crate::agent::orchestrator::SourceEvidence;
use crate::agent::policy::BrowserAction;

#[derive(Clone, Debug, Deserialize)]
struct ProposedFinding {
    claim: String,
    url: String,
    quote: String,
}
#[derive(Clone, Debug, Deserialize)]
struct ProposedVerification {
    status: String,
    #[serde(default)]
    findings: Vec<ProposedFinding>,
    #[serde(default)]
    uncertainty: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct GroundedFinding {
    pub claim: String,
    pub url: String,
    pub quote: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct VerificationReport {
    pub status: &'static str,
    pub summary: String,
    pub findings: Vec<GroundedFinding>,
    pub limitations: String,
}
fn normalized(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}
fn terms(input: &str) -> HashSet<String> {
    input.split(|ch: char| !ch.is_alphanumeric())
        .map(str::to_lowercase).filter(|s| s.chars().count() >= 4).collect()
}
fn numbers(input: &str) -> HashSet<String> {
    input.split(|ch: char| !ch.is_ascii_digit() && ch != '.' && ch != '%')
        .map(|s| s.trim_matches(|ch: char| ch == '.' || ch == '%'))
        .filter(|s| s.chars().any(|c| c.is_ascii_digit()))
        .map(str::to_owned).collect()
}
fn web_url(raw: &str) -> Option<Url> {
    let url = Url::parse(raw).ok()?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none()
        || !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    Some(url)
}

/// Fail closed unless each model-proposed claim cites an observed page and an
/// actual passage from its excerpt. No generated URLs or quotes may pass.
pub fn check_model_verification(
    raw: &str, observed: &[SourceEvidence], compare: bool,
) -> Result<VerificationReport, String> {
    let text = raw.trim();
    let draft: ProposedVerification = serde_json::from_str(text)
        .map_err(|e| format!("Verifier did not return valid structured evidence: {e}"))?;
    if draft.status != "verified" {
        return Err(format!("Verifier found insufficient evidence: {}",
            draft.uncertainty.chars().take(200).collect::<String>()));
    }
    if draft.findings.is_empty() || draft.findings.len() > 6 {
        return Err("Verification requires 1–6 source-backed findings".into());
    }
    let mut findings = Vec::new();
    let mut unique = HashSet::new();
    let mut hosts = HashSet::new();
    for item in draft.findings {
        let claim = item.claim.trim();
        let quote = item.quote.trim();
        let url = web_url(item.url.trim())
            .ok_or_else(|| "Verifier cited a malformed or non-web URL".to_string())?;
        if claim.chars().count() < 8 || claim.chars().count() > 240
            || quote.chars().count() < 12 || quote.chars().count() > 400 {
            return Err("Finding or quote has an invalid length".into());
        }
        let page = observed.iter().find(|record| web_url(&record.url).as_ref() == Some(&url))
            .ok_or_else(|| format!("Unobserved source URL: {url}"))?;
        if !normalized(&page.excerpt).contains(&normalized(quote)) {
            return Err(format!("Supporting quote does not appear in observed page: {url}"));
        }
        let claim_terms = terms(claim);
        let quote_terms = terms(quote);
        if claim_terms.len() >= 3 && claim_terms.intersection(&quote_terms).count() * 3
            < claim_terms.len() {
            return Err(format!("Claim is not grounded in the quoted passage: {url}"));
        }
        if !numbers(claim).is_subset(&numbers(quote)) {
            return Err(format!("Claim cites a number missing from the quote: {url}"));
        }
        if !unique.insert((url.to_string(), claim.to_lowercase())) {
            return Err("Duplicate cited finding".into());
        }
        hosts.insert(url.host_str().unwrap_or_default().to_string());
        findings.push(GroundedFinding {
            claim: claim.into(), url: url.to_string(), quote: quote.into()
        });
    }
    if compare && hosts.len() < 2 {
        return Err("Comparison requires evidence from at least two distinct websites".into());
    }
    let mut summary = String::from(
        "Observed website evidence (webpage claims are not independently fact-checked):\n");
    for (i, item) in findings.iter().enumerate() {
        summary.push_str(&format!("{}. {} [Source {}]\n", i+1, item.claim, i+1));
    }
    summary.push_str("\nSources and supporting excerpts:\n");
    for (i,item) in findings.iter().enumerate() {
        summary.push_str(&format!("[Source {}] {}\n  Quote: \"{}\"\n",
            i+1, item.url, item.quote));
    }
    let limitations = draft.uncertainty.chars().take(300).collect::<String>();
    if !limitations.is_empty() { summary.push_str(&format!("\nLimitations: {limitations}")); }
    Ok(VerificationReport { status:"evidence_checked", summary, findings, limitations })
}

/// Navigation dispatch is not completion: require the requested URL and
/// actual document evidence (text or meaningful title) from a later WebView snapshot.
pub fn check_navigation_result(requested: &str, after: &DomSnapshot) -> Result<(), String> {
    let target = web_url(requested).ok_or("Invalid navigation target")?;
    let actual = web_url(&after.origin).ok_or("Destination has not loaded")?;
    // Many public homepages canonicalize a bare host to its www subdomain.
    // Accept only that exact www/no-www alias with the same scheme, port,
    // path and query. Do not accept unrelated redirects or lookalike domains.
    fn normalize_www(host: &str) -> &str {
        host.strip_prefix("www.").unwrap_or(host)
    }
    let same_origin = target.scheme() == actual.scheme()
        && target.port_or_known_default() == actual.port_or_known_default()
        && target.host_str().zip(actual.host_str()).is_some_and(|(wanted, got)|
            normalize_www(wanted) == normalize_www(got));
    if !same_origin
        || target.path().trim_end_matches('/') != actual.path().trim_end_matches('/')
        || target.query() != actual.query() {
        return Err(format!("Opened {actual}, not the requested destination {target}"));
    }
    let title = after.title.trim().to_lowercase();
    let text = after.text_excerpt.trim().to_lowercase();
    // A real native DOM title also proves that a document was observed, even
    // when a JS-heavy home page initially has no body text.
    let browser_error = ["site can't be reached", "site can’t be reached",
        "this page isn't working", "privacy error", "err_connection",
        "dns_probe_finished", "page not found", "404 not found"];
    if browser_error.iter().any(|bad| title.contains(bad) || text.contains(bad)) {
        return Err("Destination displayed a browser/network error page".into());
    }
    let meaningful_title = title.chars().count() >= 4
        && title != "new tab" && title != "loading..." && title != "about:blank";
    if after.text_excerpt.trim().chars().count() < 12 && !meaningful_title {
        return Err("Destination has neither readable text nor a loaded document title".into());
    }
    Ok(())
}


/// A deliberately narrow, site-independent result-title request:
/// "search for X and open ... report its page title". No broad search or
/// research task can take this deterministic completion path.
pub fn requested_named_result_title(goal: &str) -> Option<String> {
    let lowered = goal.to_lowercase();
    if !lowered.contains("report") || !lowered.contains("title") {
        return None;
    }
    let begin = lowered.find("search for ")? + "search for ".len();
    let rest = goal.get(begin..)?;
    let end = rest.to_lowercase().find(" and open")?;
    let subject = rest.get(..end)?.trim();
    if !(3..=100).contains(&subject.chars().count()) ||
        !subject.chars().any(char::is_alphanumeric) {
        return None;
    }
    Some(subject.to_string())
}

/// A machine-observed URL, page title, and a matching saved WebView snapshot
/// prove only that a named page was opened and its title was read, NOT the
/// correctness of the page's claims. Callers must also require an executed
/// real browser action before using this verification path.
pub fn verify_observed_named_result_title(
    goal: &str,
    starting_origin: &str,
    after: &DomSnapshot,
    observations: &[SourceEvidence],
) -> Result<VerificationReport, String> {
    let wanted = requested_named_result_title(goal)
        .ok_or("Not a narrow named-result page-title request")?;
    let url = web_url(&after.origin).ok_or("Opened page has no valid web URL")?;
    if web_url(starting_origin).is_none_or(|start|start.origin()!=url.origin()){
        return Err("Named search result must remain on the observed website".into());
    }
    if after.origin == starting_origin {
        return Err("The requested result page has not been opened".into());
    }
    let expected = normalized(&wanted);
    let title = normalized(&after.title);
    let last_path = url.path_segments().and_then(|mut parts| parts.next_back())
        .ok_or("The destination has no named article path")?
        .replace('_', " ").replace('-', " ");
    let slug = normalized(&last_path);
    if !title.starts_with(&expected) ||
        !slug.contains(&expected) ||
        title.contains("search results") || title.contains("search for") ||
        after.text_excerpt.trim().chars().count() < 40 {
        return Err("The observed title and article path do not match the requested result".into());
    }
    if !observations.iter().any(|record|
        record.url == after.origin && record.title == after.title &&
            !record.excerpt.trim().is_empty())
    {
        return Err("No independently saved live WebView evidence for the result page".into());
    }
    Ok(VerificationReport {
        status: "evidence_checked",
        summary: format!("Opened a page titled {}.", after.title.trim()),
        findings: vec![GroundedFinding {
            claim: "The named result page was opened in the browser".into(),
            url: after.origin.clone(),
            quote: format!("Observed document title: {}", after.title.trim()),
        }],
        limitations: "This verifies the observed page URL and title only; it does not fact-check webpage claims.".into(),
    })
}

/// A narrowly scoped, read-only link-and-title task such as
/// "click the Tutorial link ... report the page title". Do not use this for
/// forms, purchases, arbitrary navigation, or tasks without a named link.
pub fn requested_named_link_title(goal: &str) -> Option<String> {
    let lower = goal.to_ascii_lowercase();
    if !lower.contains("report") || !lower.contains("title") { return None; }
    let start = lower.find("click the ").map(|i| i + "click the ".len())
        .or_else(|| lower.find("click ").map(|i| i + "click ".len()))?;
    let suffix = goal.get(start..)?;
    let end = suffix.to_ascii_lowercase().find(" link")?;
    let label = suffix.get(..end)?.trim();
    if !(3..=64).contains(&label.chars().count())
        || !label.chars().all(|c| c.is_ascii_alphanumeric() || c == ' ' || c == '-')
        || label.split_whitespace().count() > 7
    { return None; }
    Some(label.to_string())
}

/// Verify a *real* previously clicked link opened a matching readable page.
/// The caller separately requires a successful observed non-submit click.
/// A matching URL/title alone cannot authorize browser actions.
pub fn verify_observed_named_link_title(
    goal: &str,
    starting_origin: &str,
    after: &DomSnapshot,
    observations: &[SourceEvidence],
) -> Result<VerificationReport, String> {
    let label = requested_named_link_title(goal)
        .ok_or("Not a named-link page-title request")?;
    let url = web_url(&after.origin).ok_or("The destination is not a web page")?;
    if web_url(starting_origin).is_none_or(|start|start.origin()!=url.origin()){
        return Err("Named link destination changed origin".into());
    }
    if starting_origin == after.origin {
        return Err("The requested link did not open a new page".into());
    }
    let expected = normalized(&label);
    let title = normalized(&after.title);
    let path = normalized(&url.path().replace(['-', '_'], " "));
    if !title.contains(&expected) || !path.contains(&expected)
        || after.text_excerpt.trim().chars().count() < 40
        || title.contains("search results") || title.contains("not found")
    { return Err("Destination title and path do not match the clicked link".into()); }
    if !observations.iter().any(|item|
        item.url == after.origin && item.title == after.title
        && !item.excerpt.trim().is_empty())
    { return Err("Missing fresh recorded WebView evidence for the opened link".into()); }
    Ok(VerificationReport {
        status: "evidence_checked",
        summary: format!("Opened a page titled {}.", after.title.trim()),
        findings: vec![GroundedFinding {
            claim: "The named link destination was opened".into(),
            url: after.origin.clone(),
            quote: format!("Observed document title: {}", after.title.trim()),
        }],
        limitations: "This confirms the observed destination URL and title, not the truth of page claims.".into(),
    })
}

/// For explicitly requested named-article searches, a search control can
/// navigate without producing a form-submission confirmation message.
/// This is a *navigation* postcondition and never permits claiming a purchase,
/// registration, application, or other consequential form was submitted.
pub fn check_named_search_transition(
    goal: &str, before: &DomSnapshot, after: &DomSnapshot,
) -> Result<(), String> {
    let subject = requested_named_result_title(goal)
        .ok_or("The user did not request a narrow named-result page title search")?;
    let from = web_url(&before.origin).ok_or("The search source was not a web page")?;
    let to = web_url(&after.origin).ok_or("No observed destination URL")?;
    if from.scheme() != to.scheme()
        || from.host_str() != to.host_str()
        || from.port_or_known_default() != to.port_or_known_default()
        || from.path() == to.path() && from.query() == to.query()
    {
        return Err("The search did not navigate to a new page on the observed site".into());
    }
    check_navigation_result(to.as_str(), after)?;
    if after.text_excerpt.trim().chars().count() < 40 {
        return Err("Search destination has insufficient readable page evidence".into());
    }
    let needle = normalized(&subject);
    let displayed = normalized(&format!("{} {}", after.title, after.text_excerpt));
    let location = normalized(&format!(
        "{} {}",
        to.path().replace('_', " ").replace('-', " "),
        to.query_pairs().map(|(_, v)| v.into_owned()).collect::<Vec<_>>().join(" ")
    ));
    if !displayed.contains(&needle) && !location.contains(&needle) {
        return Err("The observed destination does not show the requested search subject".into());
    }
    Ok(())
}

/// A deterministic verifier is more reliable than an LLM quote-generation pass
/// for the narrow task of opening an explicitly named website.
pub fn verify_direct_navigation(
    requested: &str, after: &DomSnapshot,
) -> Result<VerificationReport, String> {
    check_navigation_result(requested, after)?;
    let url = web_url(&after.origin).ok_or("Invalid observed destination")?;
    let quote = if after.text_excerpt.trim().chars().count() >= 12 {
        after.text_excerpt.trim().chars().take(140).collect()
    } else {
        format!("Observed document title: {}", after.title.trim().chars().take(100).collect::<String>())
    };
    Ok(VerificationReport {
        status: "evidence_checked",
        summary: format!("Verified: opened {} in the browser.", url.origin().ascii_serialization()),
        findings: vec![GroundedFinding {
            claim: "The requested website loaded in the native browser".into(),
            url: url.to_string(),
            quote,
        }],
        limitations: "Verification confirms the observed destination and document evidence, not the accuracy of external website content.".into(),
    })
}

/// A simple "search YouTube and open the first video" task can be verified
/// deterministically from two independent native page observations. This
/// verifies site navigation, not the contents or factual claims of the video.
pub fn verify_observed_youtube_first_result(
    goal: &str,
    after: &DomSnapshot,
    observed: &[SourceEvidence],
) -> Result<VerificationReport, String> {
    let normalized_goal = goal.trim().to_ascii_lowercase();
    let requested_query = normalized_goal
        .strip_prefix("search youtube for ")
        .and_then(|tail| tail.split_once(" and open the first video result"))
        .map(|(query, _)| query.trim())
        .filter(|query| query.chars().count() >= 3 && query.chars().count() <= 120)
        .ok_or("Not a supported explicit YouTube first-result task")?;

    let watch_url = web_url(&after.origin)
        .ok_or("No observed YouTube video page")?;
    if watch_url.scheme() != "https"
        || watch_url.host_str() != Some("www.youtube.com")
        || watch_url.path() != "/watch"
        || !watch_url.query_pairs().any(|(key, value)| key == "v" && !value.is_empty())
    {
        return Err("Observed destination is not a YouTube watch page".into());
    }
    check_navigation_result(after.origin.as_str(), after)?;
    if after.text_excerpt.trim().chars().count() < 40 {
        return Err("Observed video page has insufficient readable evidence".into());
    }

    let watched = observed.iter().find(|record|
        web_url(&record.url).as_ref() == Some(&watch_url)
        && record.excerpt.trim().chars().count() >= 40
    ).ok_or("No independent native video-page evidence was recorded")?;

    let searched = observed.iter().find(|record| {
        let Some(url) = web_url(&record.url) else { return false };
        url.scheme() == "https" && url.host_str() == Some("www.youtube.com")
            && url.path() == "/results"
            && url.query_pairs().any(|(key, value)|
                key == "search_query" && normalized(value.as_ref()) == normalized(requested_query))
            && record.excerpt.trim().chars().count() >= 30
    }).ok_or("No matching YouTube search-results page was observed")?;

    let title = after.title.trim().chars().take(120).collect::<String>();
    let watched_quote = watched.excerpt.trim().chars().take(140).collect::<String>();
    let searched_quote = searched.excerpt.trim().chars().take(140).collect::<String>();
    Ok(VerificationReport {
        status: "evidence_checked",
        summary: format!(
            "Verified: searched YouTube for '{}' and opened a video watch page ({}).",
            requested_query, title
        ),
        findings: vec![
            GroundedFinding {
                claim: "The requested search results page was observed in the browser".into(),
                url: searched.url.clone(),
                quote: searched_quote,
            },
            GroundedFinding {
                claim: "A YouTube watch page was observed after the search".into(),
                url: watched.url.clone(),
                quote: watched_quote,
            },
        ],
        limitations: "This verifies observed page navigation and text only. It does not verify video accuracy or independently establish ranking beyond the selected on-page link.".into(),
    })
}

/// A submit click acknowledgment is not proof of a received submission.
pub fn check_submission_result(
    before: &DomSnapshot, after: &DomSnapshot,
) -> Result<(), String> {
    if before.origin == after.origin
        && normalized(&before.text_excerpt) == normalized(&after.text_excerpt) {
        return Err("Submit was clicked, but no page change was observed".into());
    }
    let text = normalized(&after.text_excerpt);
    let earlier_text = normalized(&before.text_excerpt);
    let confirmations = [
        "successfully submitted", "submission received", "form submitted",
        "application received", "application submitted", "thank you for submitting",
        "registration complete", "order confirmed", "confirmation number",
        "feedback sent", "feedback received", "message sent successfully",
        "changes saved successfully", "settings updated successfully",
    ];
    if !confirmations.iter().any(|term| text.contains(term) && !earlier_text.contains(term)) {
        return Err("Page changed, but no visible submission confirmation was found".into());
    }
    Ok(())
}
pub fn requires_post_action_check(action: &BrowserAction) -> bool {
    matches!(action, BrowserAction::Navigate { .. }
        | BrowserAction::Click { is_submit:true, .. })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn src(url: &str, excerpt: &str) -> SourceEvidence {
        SourceEvidence { url:url.into(), title:"Demo".into(), excerpt:excerpt.into() }
    }
    fn dom(url: &str, text: &str) -> DomSnapshot {
        DomSnapshot {origin:url.into(), title:"Demo".into(),
            text_excerpt:text.into(),interactive_nodes:vec![]}
    }
    #[test]
    fn accepts_exact_observed_quote_and_url() {
        let sources = [src("https://one.test/details",
            "The laptop includes 16 GB memory and a 512 GB SSD in the base configuration.")];
        let proposal = serde_json::json!({"status":"verified","findings":[{
            "claim":"The laptop includes 16 GB memory",
            "url":"https://one.test/details",
            "quote":"The laptop includes 16 GB memory"
        }]});
        let report = check_model_verification(&proposal.to_string(), &sources,false).unwrap();
        assert!(report.summary.contains("[Source 1] https://one.test/details"));
    }
    #[test]
    fn rejects_fabricated_urls_quotes_numbers_or_unverified_status() {
        let sources=[src("https://one.test/","The laptop has 16 GB memory and a 512 GB SSD.")];
        let original=serde_json::json!({"status":"verified","findings":[{
            "claim":"The laptop has 16 GB memory",
            "url":"https://one.test/",
            "quote":"The laptop has 16 GB memory"
        }]});
        for i in 0..4 {
            let mut test=original.clone();
            match i {
                0 => test["findings"][0]["url"]="https://not-seen.test/".into(),
                1 => test["findings"][0]["quote"]="The laptop has 64 GB memory".into(),
                2 => test["findings"][0]["claim"]="The laptop has 99 GB memory".into(),
                _ => test["status"]="insufficient".into(),
            }
            assert!(check_model_verification(&test.to_string(),&sources,false).is_err());
        }
    }
    #[test]
    fn comparisons_require_two_observed_hosts() {
        let sources=[
            src("https://one.test/a","Laptop A has a 16 GB memory configuration."),
            src("https://two.test/b","Laptop B has a 32 GB memory configuration.")
        ];
        let result=serde_json::json!({"status":"verified","findings":[
            {"claim":"Laptop A has 16 GB memory","url":"https://one.test/a",
                "quote":"Laptop A has a 16 GB memory configuration"},
            {"claim":"Laptop B has 32 GB memory","url":"https://two.test/b",
                "quote":"Laptop B has a 32 GB memory configuration"}
        ]});
        assert!(check_model_verification(&result.to_string(),&sources,true).is_ok());
        assert!(check_model_verification(&result.to_string(),&sources[..1],true).is_err());
    }
    #[test]
    fn exact_destination_is_automatically_verified_without_model_claims() {
        let snapshot = DomSnapshot {
            origin: "https://www.youtube.com/".into(),
            title: "YouTube".into(),
            text_excerpt: "YouTube home. Browse videos and channels on this site.".into(),
            interactive_nodes: vec![],
        };
        let report = verify_direct_navigation("https://www.youtube.com/", &snapshot).unwrap();
        assert_eq!(report.status, "evidence_checked");
        assert_eq!(report.findings.len(), 1);
        assert!(verify_direct_navigation("https://www.youtube.com/", &DomSnapshot {
            origin: "https://www.youtube.com.evil.test/".into(), ..snapshot.clone()
        }).is_err());
        assert!(verify_direct_navigation("https://www.youtube.com/", &DomSnapshot {
            text_excerpt: "".into(), ..snapshot.clone()
        }).is_ok()); // A loaded native document title is valid navigation evidence.
        assert!(verify_direct_navigation("https://www.youtube.com/", &DomSnapshot {
            title: "".into(), text_excerpt: "".into(), ..snapshot.clone()
        }).is_err());
        assert!(verify_direct_navigation("https://www.youtube.com/", &DomSnapshot {
            title: "This site can't be reached".into(), text_excerpt: "".into(), ..snapshot
        }).is_err());
    }

    #[test]
    fn canonical_www_redirect_needs_exact_host_path_and_page_evidence() {
        let loaded = dom("https://www.wikipedia.org/",
            "Wikipedia homepage with language choices and search.");
        assert!(check_navigation_result("https://wikipedia.org/", &loaded).is_ok());
        assert!(check_navigation_result("https://www.wikipedia.org/", &dom(
            "https://wikipedia.org/", "Wikipedia homepage with search.")).is_ok());
        assert!(check_navigation_result("https://wikipedia.org/", &dom(
            "https://www.wikipedia.org.evil.test/", "A fake homepage.")).is_err());
        assert!(check_navigation_result("https://wikipedia.org/", &dom(
            "https://wiki.pedia.org/", "A different homepage.")).is_err());
        assert!(check_navigation_result("https://wikipedia.org/", &dom(
            "http://www.wikipedia.org/", "Insecure redirected homepage.")).is_err());
        assert!(check_navigation_result("https://wikipedia.org/article", &loaded).is_err());
        let mut empty = loaded.clone();
        empty.title.clear();
        empty.text_excerpt.clear();
        assert!(check_navigation_result("https://wikipedia.org/", &empty).is_err());
    }

    #[test]
    fn named_search_result_title_requires_new_matching_article_and_source() {
        let goal = "On this documentation site, search for Ada Lovelace and open the corresponding article. Then report its page title.";
        assert_eq!(requested_named_result_title(goal).as_deref(), Some("Ada Lovelace"));
        assert!(requested_named_result_title("Search for Ada Lovelace and compare biographies").is_none());
        let start = "https://en.wikipedia.org/wiki/Main_Page";
        let article = DomSnapshot {
            origin: "https://en.wikipedia.org/wiki/Ada_Lovelace".into(),
            title: "Ada Lovelace - Wikipedia".into(),
            text_excerpt: "Ada Lovelace was a mathematician. This is an observed example article page with more visible text.".into(),
            interactive_nodes: vec![],
        };
        let sources = vec![SourceEvidence {
            url: article.origin.clone(),
            title: article.title.clone(),
            excerpt: article.text_excerpt.clone(),
        }];
        let report = verify_observed_named_result_title(goal, start, &article, &sources).unwrap();
        assert_eq!(report.status, "evidence_checked");
        assert_eq!(report.findings[0].url, article.origin);
        assert!(report.limitations.contains("not fact-check"));
        assert!(verify_observed_named_result_title(goal, start, &article, &[]).is_err());
        assert!(verify_observed_named_result_title(goal, article.origin.as_str(), &article, &sources).is_err());
        let search_listing = DomSnapshot {
            origin: "https://en.wikipedia.org/w/index.php?search=Ada_Lovelace".into(),
            title: "Ada Lovelace - Search results".into(),
            ..article.clone()
        };
        assert!(verify_observed_named_result_title(goal, start, &search_listing, &sources).is_err());
        let other = DomSnapshot {
            origin: "https://en.wikipedia.org/wiki/Other".into(),
            title: "Other - Wikipedia".into(),
            ..article
        };
        assert!(verify_observed_named_result_title(goal, start, &other, &sources).is_err());
    }

    #[test]
    fn named_link_title_requires_matching_destination_and_real_evidence() {
        let goal = "On this Python documentation homepage, click the Tutorial link to open the Python Tutorial page. Then report the page title.";
        assert_eq!(requested_named_link_title(goal).as_deref(), Some("Tutorial"));
        assert!(requested_named_link_title("Click Tutorial to buy a subscription").is_none());
        assert!(requested_named_link_title("Submit the feedback form and report its title").is_none());
        let start = "https://docs.python.org/3/";
        let page = DomSnapshot {
            origin: "https://docs.python.org/3/tutorial/index.html".into(),
            title: "The Python Tutorial — Python 3.15 documentation".into(),
            text_excerpt: "The Python Tutorial contains an introduction to Python with extensive examples and lessons.".into(),
            interactive_nodes: vec![],
        };
        let evidence = vec![SourceEvidence {
            url: page.origin.clone(), title: page.title.clone(),
            excerpt: page.text_excerpt.clone(),
        }];
        let report = verify_observed_named_link_title(goal, start, &page, &evidence).unwrap();
        assert_eq!(report.status, "evidence_checked");
        assert!(verify_observed_named_link_title(goal, start, &page, &[]).is_err());
        assert!(verify_observed_named_link_title(goal, &page.origin, &page, &evidence).is_err());
        let wrong = DomSnapshot {
            origin: "https://docs.python.org/3/library/index.html".into(),
            title: "The Python Library — Python documentation".into(),
            ..page.clone()
        };
        assert!(verify_observed_named_link_title(goal, start, &wrong, &evidence).is_err());
        let forged = DomSnapshot {
            origin: "https://docs.python.org.evil.test/3/tutorial/index.html".into(),
            ..page
        };
        assert!(verify_observed_named_link_title(goal, start, &forged, &evidence).is_err());
    }

    #[test]
    fn named_search_transition_requires_real_same_site_result_page() {
        let goal = "On this Wikipedia page, search for Ada Lovelace and open the corresponding article. Then report its page title.";
        let before = dom("https://en.wikipedia.org/wiki/Main_Page",
            "Wikipedia home page with a search bar, article links and languages.");
        let result = dom("https://en.wikipedia.org/wiki/Ada_Lovelace",
            "Ada Lovelace was an English mathematician and writer with a famous biography.");
        assert!(check_named_search_transition(goal, &before, &result).is_ok());
        assert!(check_named_search_transition(goal, &before, &before).is_err());
        assert!(check_named_search_transition(goal, &before, &dom(
            "https://en.wikipedia.org/wiki/Other", "An unrelated encyclopedia article with no requested subject.")).is_err());
        assert!(check_named_search_transition(goal, &before, &dom(
            "https://en.wikipedia.org.evil.test/wiki/Ada_Lovelace", "Ada Lovelace is a mathematician in this fake site.")).is_err());
        assert!(check_named_search_transition("Submit my application form", &before, &result).is_err());
        assert!(check_named_search_transition(goal, &before, &dom(
            "https://en.wikipedia.org/wiki/Ada_Lovelace", "")).is_err());
    }

    #[test]
    fn navigation_needs_actual_destination_not_just_ack() {
        assert!(check_navigation_result("https://example.org/details",
            &dom("https://example.org/details","Page details are visible here.")).is_ok());
        assert!(check_navigation_result("https://example.org/details",
            &dom("https://example.org/login","Please sign in before proceeding.")).is_err());
        assert!(check_navigation_result("https://example.org/details",
            &DomSnapshot { title: "".into(),
                ..dom("https://example.org/details","") }).is_err());
    }
    #[test]
    fn submission_needs_changed_page_with_confirmation() {
        let before=dom("https://example.org/apply","Application form with contact fields.");
        assert!(check_submission_result(&before,&before).is_err());
        assert!(check_submission_result(&before,
            &dom("https://example.org/apply","There was an unrelated error.")).is_err());
        assert!(check_submission_result(&before,
            &dom("https://example.org/thanks","Application received. Thank you for submitting.")).is_ok());
        let prior_banner = dom("https://example.org/apply",
            "Help text: Application received. Fields for your new submission.");
        assert!(check_submission_result(&prior_banner,
            &dom("https://example.org/apply","Application received. Loading another page.")).is_err());
    }
}

#[cfg(test)]
mod autonomous_confirmation_tests {
    use super::*;
    #[test]
    fn accepts_new_visible_feedback_confirmation_but_not_preexisting_text() {
        let before = DomSnapshot {
            origin: "https://example.org/feedback".into(),
            title: "Feedback".into(),
            text_excerpt: "No feedback has been submitted yet".into(),
            interactive_nodes: vec![],
        };
        let mut after = before.clone();
        after.text_excerpt = "Feedback sent successfully on the local test page".into();
        assert!(check_submission_result(&before, &after).is_ok());
        assert!(check_submission_result(&after, &after).is_err());

        let mut unrelated = before.clone();
        unrelated.text_excerpt = "A new panel appeared, but no confirmation was shown".into();
        assert!(check_submission_result(&before, &unrelated).is_err());
    }
}

#[cfg(test)]
mod youtube_first_result_evidence_tests {
    use super::*;
    fn snapshot(url: &str) -> DomSnapshot {
        DomSnapshot {
            origin: url.into(),
            title: "NASA Artemis explained - YouTube".into(),
            text_excerpt: "A detailed NASA Artemis mission video with chapters and a description.".into(),
            interactive_nodes: vec![],
        }
    }
    #[test]
    fn requires_both_actual_search_results_and_matching_video_page() {
        let goal = "Search YouTube for NASA Artemis mission and open the first video result, then report the observed video page.";
        let url = "https://www.youtube.com/watch?v=abcDEF12345";
        let watch = snapshot(url);
        let observed = vec![
            SourceEvidence {
                url: "https://www.youtube.com/results?search_query=NASA+Artemis+mission".into(),
                title: "Results - YouTube".into(),
                excerpt: "Search results for NASA Artemis mission videos and animations.".into(),
            },
            SourceEvidence {
                url: url.into(),
                title: watch.title.clone(),
                excerpt: watch.text_excerpt.clone(),
            },
        ];
        let report = verify_observed_youtube_first_result(goal, &watch, &observed).unwrap();
        assert_eq!(report.status, "evidence_checked");
        assert_eq!(report.findings.len(), 2);
        assert!(verify_observed_youtube_first_result(goal, &watch, &observed[1..]).is_err());
        assert!(verify_observed_youtube_first_result(goal,
            &snapshot("https://www.youtube.com/results?search_query=NASA"), &observed).is_err());
        assert!(verify_observed_youtube_first_result(goal,
            &snapshot("https://www.youtube.com.attacker.test/watch?v=123"), &observed).is_err());
        assert!(verify_observed_youtube_first_result("Open YouTube", &watch, &observed).is_err());
        let mut wrong_search = observed.clone();
        wrong_search[0].url = "https://www.youtube.com/results?search_query=unrelated".into();
        assert!(verify_observed_youtube_first_result(goal, &watch, &wrong_search).is_err());
    }
}
