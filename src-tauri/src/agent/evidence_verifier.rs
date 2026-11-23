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
/// actual readable content from a later WebView snapshot.
pub fn check_navigation_result(requested: &str, after: &DomSnapshot) -> Result<(), String> {
    let target = web_url(requested).ok_or("Invalid navigation target")?;
    let actual = web_url(&after.origin).ok_or("Destination has not loaded")?;
    if target.origin() != actual.origin()
        || target.path().trim_end_matches('/') != actual.path().trim_end_matches('/')
        || target.query() != actual.query() {
        return Err(format!("Opened {actual}, not the requested destination {target}"));
    }
    if after.text_excerpt.trim().chars().count() < 12 {
        return Err("Destination has no readable page evidence yet".into());
    }
    Ok(())
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
    fn navigation_needs_actual_destination_not_just_ack() {
        assert!(check_navigation_result("https://example.org/details",
            &dom("https://example.org/details","Page details are visible here.")).is_ok());
        assert!(check_navigation_result("https://example.org/details",
            &dom("https://example.org/login","Please sign in before proceeding.")).is_err());
        assert!(check_navigation_result("https://example.org/details",
            &dom("https://example.org/details","")).is_err());
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
