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
    PressKey { node_id: String, key: String },
    SelectOption { node_id: String, option_label: String },
    SetChecked { node_id: String, checked: bool, #[serde(default)] element_label: String },
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
            BrowserAction::SetChecked { node_id, checked, .. } =>
                BrowserAction::SetChecked {
                    node_id, checked, element_label: "[observed toggle]".into()
                },
            BrowserAction::SelectOption { node_id, .. } =>
                BrowserAction::SelectOption {
                    node_id, option_label: "[observed option]".into()
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


/// The user explicitly named a Wikipedia article and asked for its title
/// and source. Build one potential destination from that exact user-supplied
/// name. This is navigation authorization, never proof the article exists.
/// All page evidence and the returned title/URL must be read back natively.
pub fn requested_wikipedia_article_url(goal:&str)->Option<Url>{
    let text=goal.trim();
    let lower=text.to_ascii_lowercase();
    let prefix=["open wikipedia, find the ","open wikipedia and find the "]
        .into_iter().find(|p|lower.starts_with(p))?;
    let remainder=text.get(prefix.len()..)?;
    let index=remainder.to_ascii_lowercase().find(" article")?;
    let name=remainder.get(..index)?.trim();
    let rest=remainder.get(index+" article".len()..)?.to_ascii_lowercase();
    if !rest.contains("report")||!rest.contains("title")
        ||!rest.contains("source url")||!rest.contains("verify")
        ||name.chars().count()<3||name.chars().count()>75
        ||name.split_whitespace().count()>10
        ||!name.chars().all(|c|c.is_ascii_alphanumeric()||matches!(c,' '|'-'))
        ||!super::deliberation::read_only(goal)
    {return None;}
    let mut result=Url::parse("https://en.wikipedia.org/wiki").ok()?;
    result.path_segments_mut().ok()?.push(&name.replace(' ',"_"));
    Some(result)
}

/// Only the user's short, explicit navigation request can authorize opening a
/// new site without a second approval. Model and webpage text never qualify.
pub fn requested_navigation_url(goal: &str) -> Option<Url> {
    if let Some(article)=requested_wikipedia_article_url(goal){return Some(article);}
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

/// Unlike requested_navigation_url, this only authorizes entering a site
/// for a compound task. It must never trigger the "navigation complete" path:
/// the agent still has to play the requested music and verify the outcome.
pub fn requested_music_entry_url(goal: &str) -> Option<Url> {
    let normalized = goal.trim().trim_end_matches(['.', '!', '?'])
        .trim().to_ascii_lowercase();
    if normalized.len() > 220 || normalized.chars().any(char::is_control) {
        return None;
    }
    // For a short, explicit request to play any song, the user has
    // authorized opening a public music site, not downloading a file.
    // This is only the first navigation; actual playback still needs
    // independent native media-clock verification.
    if matches!(normalized.as_str(),
        "play any song" | "play a song" | "play music" | "play any music"
        | "play some music" | "play a music video" | "play some songs") {
        return Url::parse("https://www.youtube.com/").ok();
    }
    let request = ["open youtube and play ", "visit youtube and play ",
        "go to youtube and play ", "navigate to youtube and play "]
        .iter().find_map(|prefix| normalized.strip_prefix(prefix))?;
    if !["music", "some music", "any music", "a song", "any song",
        "some songs", "a music video"].contains(&request) {
        return None;
    }
    Url::parse("https://www.youtube.com/").ok()
}

/// A request to play *any* song delegates the choice of music. Use a
/// fixed public-domain classical composition as a bounded discovery query.
/// This never supplies a fabricated video ID or certifies playback; selecting
/// a video still requires an actual YouTube results page and observed link.
pub fn requested_unspecified_music_search_url(goal:&str)->Option<Url>{
    let g=goal.trim().trim_end_matches(['.','!','?']).trim().to_ascii_lowercase();
    if !matches!(g.as_str(),"play any song"|"play a song"|"play music"|
        "play any music"|"play some music"|"play a music video"|"play some songs"){
        return None;
    }
    let mut url=Url::parse("https://www.youtube.com/results").ok()?;
    url.query_pairs_mut().append_pair("search_query","Beethoven Fur Elise piano performance");
    Some(url)
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

/// A search waypoint for each source explicitly named by the user.
/// No inferred official URLs; results still require page evidence and policy.
pub fn named_research_waypoint(goal:&str, stage:usize)->Option<Url>{
    let names=super::deliberation::named_research_entities(goal);
    let name=names.get(stage)?;
    let mut url=Url::parse("https://www.bing.com/search").ok()?;
    url.query_pairs_mut().append_pair("q",&format!("{name} official website free plan pricing"));
    Some(url)
}

/// Enter a public search engine from the internal browser start page for
/// explicitly read-only, open-ended research. This is navigation only, not
/// evidence or permission to complete. The query is supplied by the user.
pub fn public_research_bootstrap(goal: &str, origin: &str) -> Option<Url> {
    if !matches!(origin, "about:blank" | "browsphere://start")
        || !super::deliberation::read_only(goal)
        || goal.len() > 300
        || goal.chars().any(char::is_control)
        || !["find ", "research ", "compare ", "search ", "look up ", "visit ", "plan "]
            .iter().any(|prefix| goal.trim().to_ascii_lowercase().starts_with(prefix))
        || goal.contains("http://") || goal.contains("https://")
        || requested_navigation_url(goal).is_some()
        || requested_search_entry_url(goal).is_some()
    { return None; }
    let mut url = Url::parse("https://html.duckduckgo.com/html/").ok()?;
    let query=super::course_comparison::discovery_terms(goal)
        .unwrap_or_else(||goal.trim().to_string());
    url.query_pairs_mut().append_pair("q",&query);
    Some(url)
}

/// Check only a read-only query scoped to the current observed course domain.
pub fn course_search_matches_source(goal:&str,origin:&str,target:&str)->bool{
    if super::course_comparison::count(goal).is_none(){return false;}
    let (Ok(a),Ok(b))=(Url::parse(origin),Url::parse(target))else{return false};
    if a.scheme()!="https"||b.scheme()!="https"||b.host_str()!=Some("html.duckduckgo.com")
        ||b.path()!="/html/"{return false}
    let Some(host)=super::course_comparison::site(origin)else{return false};
    b.query_pairs().any(|(k,q)|k=="q" && q.starts_with("beginner free ")
        &&q.contains("certificate duration")
        &&q.contains(&format!("-site:{host}")))
}

pub fn is_public_google_homepage(raw:&str)->bool{
    let Ok(url)=Url::parse(raw)else{return false};
    matches!(url.scheme(),"https"|"http")
        && matches!(url.host_str(),Some("google.com"|"www.google.com"))
        && url.path()=="/" && url.username().is_empty() && url.password().is_none()
        && url.query_pairs().all(|(key,value)|key=="zx" && !value.is_empty()
            && value.len()<=64 && value.bytes().all(|b|b.is_ascii_alphanumeric()))
}

/// The exact user-requested search query is distinct from transient homepage metadata.
pub fn requested_google_research_url(goal:&str)->Option<Url>{
    const PREFIX:&str="search google for ";
    if !super::deliberation::read_only(goal) ||
       !goal.to_ascii_lowercase().starts_with(PREFIX){return None;}
    let phrase=goal.get(PREFIX.len()..)?
        .split(['.','?','!','\n']).next()?.trim();
    if !(4..=150).contains(&phrase.chars().count()) ||
       phrase.split_whitespace().count()>24 ||
       phrase.chars().any(char::is_control) ||
       phrase.contains("://"){return None;}
    let mut url=Url::parse("https://www.google.com/search").ok()?;
    url.query_pairs_mut().append_pair("q",phrase);
    Some(url)
}

/// Preserve existing approval/deny policy for any target not explicitly
/// authorized by the user's own goal. Do not override run allowlists.
/// Allow an observed search-result link only when it resolves to the named
/// brand's own HTTPS registrable-style domain. Never trust inferred URLs,
/// lookalike subdomains, unrelated search ads or off-allowlist sites.
pub fn evaluate_observed_named_site_policy(
    active_origin:&str,action:&BrowserAction,allowed_domains:&[String],goal:&str,
    page:&crate::agent::dom_extractor::DomSnapshot,stage:usize
)->(RiskLevel,PolicyDecision){
    let named=super::deliberation::named_research_entities(goal);
    if let BrowserAction::Navigate{url}=action {
        // Search-result advertising and opaque redirect routes never count as
        // observed destination navigation, even if they share the origin.
        if !named.is_empty()&&Url::parse(url).ok().is_some_and(|target|
            matches!(target.host_str(),Some("www.bing.com"))
                && matches!(target.path(),"/ck/a"|"/aclk")){
            return (RiskLevel::High,PolicyDecision::Deny{
                reason:"Opaque search redirect is not an official research destination".into()
            });
        }
    }
    let base=evaluate_user_navigation_policy(active_origin,action,allowed_domains,goal);
    let (BrowserAction::Navigate{url},PolicyDecision::RequireApproval{reason})=(action,&base.1)
        else{return base};
    if !reason.starts_with("Cross-origin navigation to "){return base}
    let Ok(source)=Url::parse(&page.origin)else{return base};
    if source.host_str()!=Some("www.bing.com"){return base}
    let Ok(target)=Url::parse(url)else{return base};
    if target.scheme()!="https"||target.port().is_some()
        ||!target.username().is_empty()||target.password().is_some(){return base}
    let Some(host)=target.host_str()else{return base};
    if !host_matches_allowlist(host,allowed_domains){return base}
    let bare=host.strip_prefix("www.").unwrap_or(host);
    let labels:Vec<_>=bare.split('.').collect();
    if labels.len()!=2||!labels[1].chars().all(|c|c.is_ascii_alphabetic())||labels[1].len()<2{
        return base
    }
    // Authorize only the brand assigned to the current research stage.
    // Other named sites must be visited through their own staged objectives.
    if !named.get(stage).is_some_and(|name|
        name.to_ascii_lowercase().replace([' ','-'],"")==labels[0]){
        return base
    }
    if !page.interactive_nodes.iter().any(|node|!node.is_sensitive&&!node.is_submit
        && node.href.as_deref()==Some(url.as_str())&&(node.tag=="a"||node.role=="link")){
        return base
    }
    (RiskLevel::Low,PolicyDecision::Allow)
}

/// Opening an observed read-only course result must remain in Lucia's task
/// tab. Only the exact fresh public HTTPS link from the search results can
/// receive automatic authorization. No model-supplied site is trusted.
pub fn evaluate_observed_course_result_policy(
    active_origin:&str,action:&BrowserAction,allowed_domains:&[String],
    goal:&str,page:&crate::agent::dom_extractor::DomSnapshot,
)->(RiskLevel,PolicyDecision){
    let base=evaluate_user_navigation_policy(active_origin,action,allowed_domains,goal);
    let (BrowserAction::Navigate{url},PolicyDecision::RequireApproval{reason})=(action,&base.1)
        else{return base};
    if !reason.starts_with("Cross-origin navigation to ")
        ||super::course_comparison::count(goal).is_none()
        ||!matches!(Url::parse(active_origin).ok().and_then(|u|u.host_str().map(str::to_owned)).as_deref(),
            Some("www.bing.com"|"www.google.com"|"duckduckgo.com"|"html.duckduckgo.com"))
    {return base;}
    let Ok(u)=Url::parse(url)else{return base};
    let Some(host)=u.host_str()else{return base};
    if u.scheme()!="https"||!u.username().is_empty()||u.password().is_some()
        ||super::course_comparison::site(url).is_none()
        ||!host_matches_allowlist(host,allowed_domains){return base;}
    if page.origin!=active_origin
        ||!page.interactive_nodes.iter().any(|n|!n.is_sensitive&&!n.is_submit
            && n.href.as_deref().and_then(|href|
                super::action_intelligence::observed_search_destination(active_origin,href))
                .as_deref()==Some(url.as_str())
            && (n.role=="link"||n.tag=="a")
            && super::semantic_core::relevance(goal,&n.label)>=2)
    {return base;}
    (RiskLevel::Low,PolicyDecision::Allow)
}

pub fn evaluate_user_navigation_policy(
    active_origin: &str, action: &BrowserAction, allowed_domains: &[String], goal: &str
) -> (RiskLevel, PolicyDecision) {
    let base = evaluate_policy(active_origin, action, allowed_domains);
    let BrowserAction::Navigate { url } = action else { return base };
    let PolicyDecision::RequireApproval { ref reason } = base.1 else { return base };
    if !reason.starts_with("Cross-origin navigation to ") { return base; }
    let Ok(actual) = Url::parse(url) else { return base };
    if course_search_matches_source(goal,active_origin,url)
        && actual.host_str().is_some_and(|host|host_matches_allowlist(host,allowed_domains)){
        return (RiskLevel::Low,PolicyDecision::Allow);
    }
    let research_bootstrap = public_research_bootstrap(goal,active_origin)
        .is_some_and(|expected| actual == expected);
    if research_bootstrap && actual.host_str().is_some_and(|host|host_matches_allowlist(host,allowed_domains)) {
        return (RiskLevel::Low,PolicyDecision::Allow);
    }
    let named_search_waypoint=super::deliberation::named_research_entities(goal)
        .iter().enumerate().any(|(stage,_)|named_research_waypoint(goal,stage).as_ref()==Some(&actual));
    if named_search_waypoint && actual.host_str().is_some_and(|host|host_matches_allowlist(host,allowed_domains)){
        return (RiskLevel::Low,PolicyDecision::Allow);
    }
    let user_explicitly_named_destination = requested_navigation_url(goal)
        .is_some_and(|expected| actual == expected)
        || crate::agent::deliberation::explicit_research_sources(goal).iter().any(|expected| &actual==expected)
        || crate::agent::catalog_research::entry_allowed(goal,url)
        || crate::agent::catalog_research::discovery_navigation_allowed(goal,active_origin,url);
    let user_explicitly_named_music_entry = requested_music_entry_url(goal)
        .is_some_and(|expected| actual == expected);
    let user_explicitly_named_search_site = requested_search_host(goal)
        .is_some_and(|host| actual.scheme() == "https"
            && actual.host_str() == Some(host));
    if (user_explicitly_named_destination || user_explicitly_named_music_entry
        || user_explicitly_named_search_site)
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
        BrowserAction::PressKey { node_id, key } => match key.as_str() {
            "Enter" => (RiskLevel::Medium, PolicyDecision::Allow),
            "Escape" => (RiskLevel::Low, PolicyDecision::Allow),
            _ => (RiskLevel::High, PolicyDecision::Deny {
                reason: format!("Unsupported keyboard action on {node_id}"),
            }),
        },
        BrowserAction::SetChecked { node_id, element_label, .. } => {
            let label = element_label.to_ascii_lowercase();
            let consequential = ["consent", "terms", "privacy policy", "payment",
                "purchase", "delete", "revoke", "transfer", "subscribe", "sign up",
                "authorize", "share my data"];
            if consequential.iter().any(|word| label.contains(word)) {
                (RiskLevel::High, PolicyDecision::RequireApproval {
                    reason: format!("Consent or consequential toggle at {node_id}"),
                })
            } else {
                (RiskLevel::Medium, PolicyDecision::Allow)
            }
        }
        BrowserAction::SelectOption { node_id, option_label } => {
            let description = option_label.to_ascii_lowercase();
            let consequential = ["delete", "revoke", "purchase", "transfer", "close account",
                "publish", "consent", "cancel subscription"];
            if consequential.iter().any(|word| description.contains(word)) {
                (RiskLevel::High, PolicyDecision::RequireApproval {
                    reason: format!("Consequential option selection at {node_id}"),
                })
            } else {
                (RiskLevel::Medium, PolicyDecision::Allow)
            }
        }
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
                // Ordinary prose such as "the library's secret" is not a
                // credential. Keep explicit secret assignments and credential
                // labels gated, as well as all sensitive DOM fields.
                || ["secret:", "secret :", "secret=", "secret =", "api secret",
                    "client secret", "access token", "private key", "recovery phrase",
                    "seed phrase"].iter().any(|marker| lower.contains(marker));
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
    fn observed_control_policy_requires_approval_for_consequential_changes() {
        let simple = BrowserAction::SetChecked {
            node_id: "simple".into(), checked:true, element_label:"Include examples".into()
        };
        assert!(matches!(evaluate_policy("https://example.org", &simple, &[]).1,
            PolicyDecision::Allow));
        let consent = BrowserAction::SetChecked {
            node_id: "consent".into(), checked:true, element_label:"Agree to terms".into()
        };
        assert!(matches!(evaluate_policy("https://example.org", &consent, &[]).1,
            PolicyDecision::RequireApproval { .. }));
        let blocked = BrowserAction::SelectOption {
            node_id: "option".into(), option_label:"Delete account".into()
        };
        assert!(matches!(evaluate_policy("https://example.org", &blocked, &[]).1,
            PolicyDecision::RequireApproval { .. }));
    }

    #[test]
    fn bounded_keyboard_policy() {
        for key in ["Enter", "Escape"] {
            let action = BrowserAction::PressKey { node_id: "search".into(), key: key.into() };
            assert!(matches!(evaluate_policy("https://example.com", &action, &[]).1, PolicyDecision::Allow));
        }
        let blocked = BrowserAction::PressKey { node_id: "search".into(), key: "Control+L".into() };
        assert!(matches!(evaluate_policy("https://example.com", &blocked, &[]).1,
            PolicyDecision::Deny { .. }));
    }


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
        assert!(requested_navigation_url("Open YouTube and play any music").is_none(),
            "Compound playback task must not be reduced to navigation-only");
        for music in ["play any song", "Play any song!", "play a song",
            "play music", "play some music", "play any music"]{
            assert!(requested_navigation_url(music).is_none(),
                "Music playback cannot be marked complete by opening YouTube");
            assert_eq!(requested_music_entry_url(music).unwrap().as_str(),
                "https://www.youtube.com/");
            assert!(matches!(evaluate_user_navigation_policy(origin,
                &navigate("https://www.youtube.com/"), &[], music).1,
                PolicyDecision::Allow),"Expected trusted entry only: {music}");
        }
        let chosen=requested_unspecified_music_search_url("play any song").unwrap();
        assert_eq!(chosen.host_str(),Some("www.youtube.com"));
        assert_eq!(chosen.path(),"/results");
        assert_eq!(chosen.query_pairs().find(|(k,_)|k=="search_query").unwrap().1,
            "Beethoven Fur Elise piano performance");
        assert!(requested_unspecified_music_search_url("open youtube and play any music").is_none());
        assert!(requested_unspecified_music_search_url("how to play music").is_none());
        for unrelated in ["how to play any song", "play any song and download it",
            "play any song and buy an album", "play a random song then log in",
            "play any song https://example.org", "play any song; delete files"] {
            assert!(requested_music_entry_url(unrelated).is_none(),"{unrelated}");
        }

        assert_eq!(requested_music_entry_url("Open YouTube and play any music")
            .unwrap().as_str(), "https://www.youtube.com/");
        assert!(matches!(evaluate_user_navigation_policy(origin,
            &navigate("https://www.youtube.com/"), &[],
            "Open YouTube and play any music").1, PolicyDecision::Allow));
        for unsafe_goal in ["open youtube and play music and delete my account",
            "open youtube and play music and pay", "open youtube and play javascript:alert(1)"] {
            assert!(requested_navigation_url(unsafe_goal).is_none());
        }
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
    fn ordinary_fiction_does_not_require_credential_approval() {
        let draft = |text: &str, sensitive| BrowserAction::TypeText {
            node_id: "editor".into(), text: text.into(), is_sensitive: sensitive,
        };
        assert!(matches!(evaluate_policy("https://example.org", &draft(
            "The lantern revealed a secret passage through the library.", false), &[]).1,
            PolicyDecision::Allow));
        for text in ["client secret", "secret: example", "secret = example",
            "password", "OTP", "CVV", "private key", "recovery phrase"] {
            assert!(matches!(evaluate_policy("https://example.org",
                &draft(text, false), &[]).1, PolicyDecision::RequireApproval { .. }));
        }
        assert!(matches!(evaluate_policy("https://example.org",
            &draft("a harmless story", true), &[]).1, PolicyDecision::RequireApproval { .. }));
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

#[cfg(test)]
mod research_bootstrap_tests {
    use super::*;
    #[test] fn observed_named_sites_cannot_use_lookalike_domains() {
        use crate::agent::dom_extractor::{DomSnapshot,DomNodeSummary};
        let goal="Visit the official websites of Notion, Trello and ClickUp. Compare their free plans for a five-member team and recommend the most suitable option.";
        let origin="https://www.bing.com/search?q=Notion+official+website";
        let mut page=DomSnapshot{origin:origin.into(),title:"Search results".into(),
            text_excerpt:"Notion official website free plans".into(),
            interactive_nodes:vec![],structure:Default::default()};
        let node=|url:&str|DomNodeSummary{node_id:"n1".into(),tag:"a".into(),role:"link".into(),
            label:"Notion Official".into(),options:vec![],checked:None,search_state:None,
            href:Some(url.into()),is_submit:false,is_sensitive:false,bbox:None};
        for (url,allowed) in [("https://www.notion.so/pricing",true),
                              ("https://notion.evil.com/pricing",false),
                              ("https://evil.com/notion",false)]{
            page.interactive_nodes=vec![node(url)];
            let action=BrowserAction::Navigate{url:url.into()};
            assert_eq!(matches!(evaluate_observed_named_site_policy(origin,&action,&[],goal,&page,0).1,
                PolicyDecision::Allow),allowed,"{url}");
        }
        page.interactive_nodes=vec![node("https://www.trello.com/pricing")];
        let other_stage=BrowserAction::Navigate{url:"https://www.trello.com/pricing".into()};
        assert!(!matches!(evaluate_observed_named_site_policy(origin,&other_stage,&[],goal,&page,0).1,
            PolicyDecision::Allow),"Notion stage must not authorize Trello");
        let redirect=BrowserAction::Navigate{url:"https://www.bing.com/aclk".into()};
        assert!(matches!(evaluate_observed_named_site_policy(origin,&redirect,&[],goal,&page,1).1,
            PolicyDecision::Deny{..}),"Search ad redirects must not be navigable as official evidence");
        page.interactive_nodes.clear();
        let action=BrowserAction::Navigate{url:"https://www.notion.so/pricing".into()};
        assert!(!matches!(evaluate_observed_named_site_policy(origin,&action,&[],goal,&page,0).1,
            PolicyDecision::Allow));
    }
    #[test] fn named_source_waypoints_are_bounded_and_policy_checked() {
        let goal="Visit the official websites of Notion, Trello and ClickUp. Compare their free plans for a five-member team and recommend the most suitable option.";
        for index in 0..3 {
            let url=named_research_waypoint(goal,index).unwrap();
            assert_eq!(url.host_str(),Some("www.bing.com"));
            assert!(matches!(evaluate_user_navigation_policy(
                "https://www.notion.so/",&BrowserAction::Navigate{url:url.into()},&[],goal
            ).1,PolicyDecision::Allow));
        }
        assert!(named_research_waypoint(goal,3).is_none());
        assert!(named_research_waypoint("Buy products from Notion and Trello",0).is_none());
    }
    #[test] fn visit_multi_site_research_bootstraps_from_blank_only() {
        let goal="Visit the official websites of Notion, Trello and ClickUp. Compare their free plans for a five-member team and recommend the most suitable option.";
        let url=public_research_bootstrap(goal,"about:blank").expect("read-only visit needs a safe bootstrap");
        assert_eq!(url.host_str(),Some("html.duckduckgo.com"));
        assert!(public_research_bootstrap(goal,"https://example.org").is_none());
    }
    #[test] fn planning_with_public_research_bootstraps_without_authorizing_transactions() {
        let goal="Plan a Sunday outing in Bengaluru for a family of four under INR 2,000. Find three family-friendly attractions, check their opening hours and ticket prices, and prepare an itinerary.";
        let url=public_research_bootstrap(goal,"about:blank").expect("read-only planning needs a search entry");
        assert_eq!(url.query_pairs().find(|(k,_)|k=="q").unwrap().1,goal);
        assert!(matches!(evaluate_user_navigation_policy("about:blank",&BrowserAction::Navigate{url:url.into()},&[],goal).1,PolicyDecision::Allow));
        assert!(public_research_bootstrap(goal,"https://example.org").is_none());
        assert!(public_research_bootstrap("Plan a trip, find a hotel and book it","about:blank").is_none());
    }
    #[test] fn bounded_public_research_entry_without_fabricated_controls() {
        let goal="Find three free Python courses from different platforms. Compare free certificates and duration.";
        let url=public_research_bootstrap(goal,"about:blank").expect("expected search entry");
        assert_eq!(url.host_str(),Some("html.duckduckgo.com"));
        assert_eq!(url.query_pairs().find(|(k,_)|k=="q").unwrap().1,"free beginner Python course certificate duration");
        assert!(matches!(evaluate_user_navigation_policy("about:blank",&BrowserAction::Navigate{url:url.into()},&[],goal).1,PolicyDecision::Allow));
        assert!(public_research_bootstrap("buy a laptop","about:blank").is_none());
        assert!(public_research_bootstrap(goal,"https://example.org").is_none());
        assert!(public_research_bootstrap("Compare https://docs.python.org/3 and https://example.org","about:blank").is_none());
    }
}

#[cfg(test)]
mod course_site_navigation_tests {
    use super::*;
    use crate::agent::dom_extractor::{DomNodeSummary,DomSnapshot};
    const GOAL: &str="Find three free Python courses suitable for beginners from different learning platforms. Check whether they provide free certificates, compare their duration and recommend the best one.";
    #[test]
    fn observed_read_only_course_link_can_open_without_a_popup() {
        let source="https://www.bing.com/search?q=free+Python+courses";
        let target="https://academy.example.org/courses/python-intro";
        let mut page=DomSnapshot{origin:source.into(),title:"Search results".into(),
            text_excerpt:"Python courses and certificate listings".into(),structure:Default::default(),
            interactive_nodes:vec![DomNodeSummary{node_id:"result".into(),tag:"a".into(),role:"link".into(),
                label:"Free Python beginner course".into(),href:Some(target.into()),options:vec![],
                checked:None,search_state:None,is_sensitive:false,is_submit:false,bbox:None}]};
        let action=BrowserAction::Navigate{url:target.into()};
        assert!(matches!(evaluate_observed_course_result_policy(source,&action,&[],GOAL,&page).1,PolicyDecision::Allow));
        page.interactive_nodes.clear();
        assert!(!matches!(evaluate_observed_course_result_policy(source,&action,&[],GOAL,&page).1,PolicyDecision::Allow));
        let other=BrowserAction::Navigate{url:"https://other.example.net/courses/python".into()};
        assert!(!matches!(evaluate_observed_course_result_policy(source,&other,&[],GOAL,&page).1,PolicyDecision::Allow));
    }
}

#[cfg(test)]
mod duckduckgo_observation_tests {
    use super::*;
    use crate::agent::dom_extractor::{DomNodeSummary,DomSnapshot};
    const GOAL:&str="Find three free Python courses suitable for beginners from different learning platforms. Check whether they provide free certificates, compare their duration and recommend the best one.";
    #[test]
    fn only_observed_public_search_result_redirects_can_navigate(){
        let origin="https://html.duckduckgo.com/html/?q=free+python+course";
        let href="https://duckduckgo.com/l/?uddg=https%3A%2F%2Fwww.simplilearn.com%2Fskillup-free-online-courses%2Fpython";
        let dest=crate::agent::action_intelligence::observed_search_destination(origin,href).unwrap();
        assert_eq!(dest,"https://www.simplilearn.com/skillup-free-online-courses/python");
        let mut page=DomSnapshot{origin:origin.into(),title:"Python search results".into(),structure:Default::default(),
            text_excerpt:"Free Python courses and certificates".into(),interactive_nodes:vec![
                DomNodeSummary{node_id:"first".into(),tag:"a".into(),role:"link".into(),
                    label:"Free Python beginner course certificate".into(),href:Some(href.into()),
                    is_submit:false,is_sensitive:false,options:vec![],checked:None,search_state:None,bbox:None}]};
        let action=BrowserAction::Navigate{url:dest};
        assert!(matches!(evaluate_observed_course_result_policy(origin,&action,&[],GOAL,&page).1,PolicyDecision::Allow));
        page.interactive_nodes.clear();
        assert!(!matches!(evaluate_observed_course_result_policy(origin,&action,&[],GOAL,&page).1,PolicyDecision::Allow));
        assert!(crate::agent::action_intelligence::observed_search_destination(origin,
            "https://duckduckgo.com/l/?uddg=http%3A%2F%2F127.0.0.1%2Fprivate").is_none());
    }
}
