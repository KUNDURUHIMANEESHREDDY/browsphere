//! Lucia's specialist orchestrator: real staged execution on the existing WebView2
//! ReAct runtime. Each specialist has a bounded mission and restricted actions.
//! A stage advances only after live-page evidence passes the completion gate.
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use crate::agent::dom_extractor::DomSnapshot;
use crate::agent::policy::{requested_navigation_url, BrowserAction};
use crate::agent::evidence_verifier::{check_navigation_result, requested_named_result_title};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Specialist {
    Research,
    Browser,
    Comparison,
    Form,
    Data,
    Verification,
}

impl Specialist {
    pub fn label(self) -> &'static str {
        match self {
            Self::Research => "Research Agent",
            Self::Browser => "Browser Agent",
            Self::Comparison => "Comparison Agent",
            Self::Form => "Form Agent",
            Self::Data => "Data Agent",
            Self::Verification => "Verification Agent",
        }
    }

    pub fn instructions(self) -> &'static str {
        match self {
            Self::Research => crate::lucia_prompts::RESEARCH_SPECIALIST,
            Self::Browser => crate::lucia_prompts::BROWSER_SPECIALIST,
            Self::Comparison => crate::lucia_prompts::COMPARISON_SPECIALIST,
            Self::Form => crate::lucia_prompts::FORM_SPECIALIST,
            Self::Data => crate::lucia_prompts::DATA_SPECIALIST,
            Self::Verification => crate::lucia_prompts::VERIFICATION_SPECIALIST,
        }
    }

    pub fn allows(self, action: &BrowserAction) -> bool {
        match self {
            Self::Verification => matches!(action, BrowserAction::Complete { .. }),
            Self::Research | Self::Comparison | Self::Data => match action {
                BrowserAction::TypeText { is_sensitive, .. } => !is_sensitive,
                BrowserAction::SelectOption { .. } => true,
                BrowserAction::SetChecked { .. } => false,
                BrowserAction::PressKey { key, .. } => matches!(key.as_str(), "Enter" | "Escape"),
                BrowserAction::Download { .. } => false,
                BrowserAction::Click { is_submit, .. } => !is_submit,
                _ => true,
            },
            Self::Browser => true,
            Self::Form => !matches!(action, BrowserAction::Download { .. }),
        }
    }

    pub fn allows_on_page(self, action: &BrowserAction, page: &DomSnapshot) -> bool {
        if !self.allows(action) { return false; }
        if matches!(self, Self::Research | Self::Comparison | Self::Data) {
            if let BrowserAction::PressKey { node_id, key } = action {
                if key == "Enter" {
                    return page.interactive_nodes.iter().any(|node|
                        &node.node_id == node_id && !node.is_sensitive &&
                        (node.role.eq_ignore_ascii_case("searchbox") ||
                            node.label.to_lowercase().contains("search")));
                }
            }
            if let BrowserAction::TypeText { node_id, .. } = action {
                return page.interactive_nodes.iter().any(|node|
                    &node.node_id == node_id &&
                    !node.is_sensitive &&
                    (node.role.eq_ignore_ascii_case("searchbox") ||
                        node.label.to_lowercase().contains("search")));
            }
        }
        true
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SpecialistStage {
    pub specialist: Specialist,
    pub objective: String,
    pub success_criteria: String,
    pub status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct TaskPlan {
    pub goal: String,
    pub stages: Vec<SpecialistStage>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SourceEvidence {
    pub url: String,
    pub title: String,
    pub excerpt: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneratedStage {
    specialist: Specialist,
    objective: String,
    success_criteria: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneratedPlan {
    stages: Vec<GeneratedStage>,
}

/// Validate the model's proposed work before it can run. The model cannot
/// introduce new tools or bypass the existing action policy.
pub fn parse_generated_plan(goal: &str, raw: &str) -> anyhow::Result<TaskPlan> {
    anyhow::ensure!(raw.len() <= 16_384, "Specialist plan exceeds maximum response size");
    let trimmed = raw.trim().trim_start_matches('\u{feff}').trim();
    // Models sometimes wrap otherwise valid JSON in a Markdown code fence.
    // Strip only a complete fence, not arbitrary prose or a second JSON object.
    let cleaned = if let Some(fenced) = trimmed.strip_prefix("```") {
        let (_, body) = fenced.split_once('\n')
            .ok_or_else(|| anyhow::anyhow!("Specialist plan has an incomplete JSON code fence"))?;
        body.trim().strip_suffix("```")
            .ok_or_else(|| anyhow::anyhow!("Specialist plan has an unclosed JSON code fence"))?
            .trim()
    } else {
        trimmed
    };
    anyhow::ensure!(cleaned.starts_with('{'), "Specialist plan must be one JSON object");
    let generated: GeneratedPlan = serde_json::from_str(cleaned)
        .map_err(|e| anyhow::anyhow!("The model did not return a valid specialist plan: {e}"))?;
    anyhow::ensure!(
        !generated.stages.is_empty() && generated.stages.len() <= 6,
        "The specialist plan must have 1–6 stages"
    );
    let mut seen = HashSet::new();
    let mut stages = Vec::new();
    for stage in generated.stages {
        if stage.specialist == Specialist::Verification { continue; }
        if !seen.insert(stage.specialist) { continue; }
        let objective = stage.objective.trim().chars()
            .filter(|c| !c.is_control() && *c != '<' && *c != '>')
            .take(200).collect::<String>();
        let success_criteria = stage.success_criteria.trim().chars()
            .filter(|c| !c.is_control() && *c != '<' && *c != '>')
            .take(200).collect::<String>();
        anyhow::ensure!(!objective.is_empty() && !success_criteria.is_empty(),
            "The model's plan is missing a stage objective or success criterion");
        stages.push(SpecialistStage {
            specialist: stage.specialist, objective, success_criteria, status:"pending"
        });
        anyhow::ensure!(stages.len() <= 4, "Too many execution specialists requested");
    }
    anyhow::ensure!(!stages.is_empty(), "The model must assign at least one execution specialist");
    stages.push(SpecialistStage {
        specialist: Specialist::Verification,
        objective: format!("Check evidence and results for: {goal}").chars().take(200).collect(),
        success_criteria: "Confirm observed webpages and successfully executed actions; never invent success".into(),
        status: "pending",
    });
    Ok(TaskPlan { goal: goal.to_string(), stages })
}

pub struct Orchestrator {
    pub plan: TaskPlan,
    pub current: usize,
    pub evidence: Vec<SourceEvidence>,
    completed_summaries: Vec<String>,
    successful_actions: usize,
    actions_in_stage: usize,
    fresh_observations: usize,
    last_action_failed: bool,
    form_submitted: bool,
    confirmed_navigation: Option<String>,
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|word| haystack.contains(word))
}

fn goal_terms(goal: &str) -> Vec<String> {
    const STOP: &[&str] = &[
        "about", "could", "these", "those", "there", "their", "which", "would", "please",
        "compare", "search", "browse", "research", "find", "open", "show", "tell", "with",
        "from", "into", "your", "more", "than", "then", "what", "where", "when", "them",
        "some", "that", "this", "have", "need", "want", "three", "four", "five",
    ];
    goal.split(|c: char| !c.is_ascii_alphanumeric())
        .map(str::to_lowercase)
        .filter(|s| s.len() >= 4 && !STOP.contains(&s.as_str()))
        .take(20)
        .collect()
}

/// A claimed YouTube video-result task needs an actual observed watch page.
/// This is a postcondition, not a predetermined URL or invented success.
pub(crate) fn requested_youtube_video_page(goal: &str) -> bool {
    let g = goal.to_lowercase();
    g.contains("youtube") && g.contains("video")
        && (g.contains("open ") || g.contains("click "))
}

pub(crate) fn observed_youtube_video_page(origin: &str) -> bool {
    let Ok(url) = url::Url::parse(origin) else { return false; };
    let Some(host) = url.host_str() else { return false; };
    if host != "youtube.com" && !host.ends_with(".youtube.com") {
        return false;
    }
    if url.path() == "/watch" {
        return url.query_pairs().any(|(key, value)| key == "v" && !value.is_empty());
    }
    ["/shorts/", "/live/"].iter().any(|prefix|
        url.path().strip_prefix(prefix).is_some_and(|id| !id.trim_matches('/').is_empty()))
}

/// A tightly scoped, read-only article-title lookup does not require a
/// separately billed cloud planning request. The model still decides EVERY
/// browser action using freshly observed DOM; no search, click, or result URL
/// is predetermined. Other tasks retain the model-generated specialist plan.
pub(crate) fn is_simple_named_title_task(goal: &str) -> bool {
    if requested_named_result_title(goal).is_none() { return false; }
    let lower = goal.to_lowercase();
    !contains_any(&lower, &[
        "compare", "extract", "download", "upload", "screenshot", "video",
        "purchase", "checkout", "buy ", "pay ", "order ", "book ",
        "register", "sign up", "apply ", "submit", "send ", "delete",
        "remove ", "change ", "update ", "account", "email", "message",
        "fill ", "form", "several", "multiple", "list of",
    ])
}

/// A deterministic minimum plan, built from the user's intent.
/// The selected LLM decides actual actions for each specialist at runtime.
pub fn plan_task(goal: &str) -> TaskPlan {
    let lower = goal.to_lowercase();
    let mut roles: Vec<Specialist> = if crate::agent::prerequisites::route_values(goal).is_some() || crate::agent::prerequisites::filter_constraints(goal).is_some() || is_simple_named_title_task(goal) || super::evidence_verifier::requested_named_link_title(goal).is_some() {
        vec![Specialist::Browser]
    } else if super::execution_intelligence::current_reference_subject(goal).is_some(){
        vec![Specialist::Research]
    } else if super::catalog_research::Intent::compile(goal).is_some(){
        vec![Specialist::Research]
    } else if super::skills::TaskSpec::compile(goal).is_some(){
        vec![Specialist::Research]
    } else if !super::deliberation::explicit_research_sources(goal).is_empty(){
        // Source acquisition is one stage; comparison is done by the final
        // independent source verifier, not a redundant browsing stage.
        vec![Specialist::Research]
    } else if contains_any(
        &lower, &["compare", "comparison", "versus", " vs ", "difference between", "best among"],
    ) {
        vec![Specialist::Research, Specialist::Comparison]
    } else if contains_any(
        &lower, &["fill ", "submit ", "apply ", "register ", "complete the form", "sign up"],
    ) {
        vec![Specialist::Browser, Specialist::Form]
    } else if contains_any(
        &lower, &["extract", "collect", "scrape", "table", "dataset", "spreadsheet", "list of"],
    ) {
        vec![Specialist::Browser, Specialist::Data]
    } else if contains_any(
        &lower, &["research", "find ", "search ", "look up", "sources", "investigate"],
    ) {
        vec![Specialist::Research]
    } else {
        vec![Specialist::Browser]
    };
    roles.push(Specialist::Verification);
    TaskPlan {
        goal: goal.to_string(),
        stages: roles.into_iter().map(|specialist| {
            let (objective, criteria) = match specialist {
                Specialist::Research => (
                    format!("Research authentic webpage sources relevant to: {goal}"),
                    "Collect relevant, identifiable live-page evidence and source URLs".to_string()
                ),
                Specialist::Browser => (
                    format!("Navigate and interact with the browser to advance: {goal}"),
                    "WebView actions have actual success acknowledgements".to_string()
                ),
                Specialist::Comparison => (
                    format!("Compare observed alternatives for: {goal}"),
                    "Compare claims against at least two distinct, observed sources".to_string()
                ),
                Specialist::Form => (
                    format!("Fill the necessary form fields for: {goal}"),
                    "Use only supplied information; get user approval for submissions".to_string()
                ),
                Specialist::Data => (
                    format!("Extract structured information for: {goal}"),
                    "Extract values from visible page content with source context".to_string()
                ),
                Specialist::Verification => (
                    format!("Verify the final result against the user's original goal: {goal}"),
                    "Confirm observed evidence and successful actions before reporting completion".to_string()
                ),
            };
            SpecialistStage { specialist, objective, success_criteria: criteria, status: "pending" }
        }).collect()
    }
}

impl Orchestrator {
    pub fn new(goal: &str) -> Self {
        Self::from_plan(plan_task(goal))
    }

    pub fn from_plan(plan: TaskPlan) -> Self {
        Self {
            plan, current: 0, evidence: Vec::new(),
            completed_summaries: Vec::new(), successful_actions: 0,
            actions_in_stage: 0, fresh_observations: 0,
            last_action_failed: false,
            form_submitted: false,
            confirmed_navigation: None,
        }
    }

    pub fn specialist(&self) -> Specialist {
        self.plan.stages[self.current].specialist
    }

    pub fn stage(&self) -> &SpecialistStage {
        &self.plan.stages[self.current]
    }

    pub fn observe(&mut self, snapshot: &DomSnapshot) {
        let Some(evidence)=super::browser_skills::extraction::capture(snapshot)else{return;};
        self.fresh_observations+=1;
        if let Some(old) = self.evidence.iter_mut().find(|e| e.url == evidence.url) {
            *old = evidence;
        } else {
            self.evidence.push(evidence);
            if self.evidence.len()>8{
                let required=super::deliberation::explicit_research_sources(&self.plan.goal);
                if let Some(index)=self.evidence.iter().position(|e|url::Url::parse(&e.url).ok().map_or(true,|u|!required.contains(&u))){
                    self.evidence.remove(index);
                }
            }
        }
    }

    /// Called only after a fresh, native WebView2 postcondition passed.
    pub fn confirmed_navigation(&mut self, snapshot: &DomSnapshot) {
        let Some(target) = requested_navigation_url(&self.plan.goal) else { return };
        if check_navigation_result(target.as_str(), snapshot).is_err() { return; }
        self.confirmed_navigation = Some(snapshot.origin.clone());
        self.fresh_observations += 1;
        let observed = SourceEvidence {
            url: snapshot.origin.clone(),
            title: snapshot.title.chars().take(140).collect(),
            excerpt: snapshot.text_excerpt.chars().take(24000).collect(),
        };
        if let Some(existing) = self.evidence.iter_mut().find(|e| e.url == observed.url) {
            *existing = observed;
        } else {
            self.evidence.push(observed);
            if self.evidence.len()>8{
                let required=super::deliberation::explicit_research_sources(&self.plan.goal);
                if let Some(index)=self.evidence.iter().position(|e|url::Url::parse(&e.url).ok().map_or(true,|u|!required.contains(&u))){
                    self.evidence.remove(index);
                }
            }
        }
    }

    pub fn action_outcome(&mut self, action: &BrowserAction, success: bool) {
        self.last_action_failed = !success;
        if success && !matches!(action, BrowserAction::Complete { .. }) {
            self.successful_actions += 1;
            self.actions_in_stage += 1;
            if matches!(action, BrowserAction::Click { is_submit: true, .. }) {
                self.form_submitted = true;
            }
        }
    }

    pub fn prior_evidence(&self) -> String {
        let summaries = self.completed_summaries.iter()
            .map(|s| format!("- Earlier stage summary (not independent evidence): {s}"))
            .collect::<Vec<_>>().join("\n");
        let sources = self.evidence.iter().map(|e| {
            format!("- Observed source: {} ({})\n  Visible excerpt: {}",
                e.url, e.title, e.excerpt.replace('\n', " "))
        }).collect::<Vec<_>>().join("\n");
        format!("{summaries}\n{sources}").chars().take(5000).collect()
    }

    /// Reject model-declared completion when there is no real page evidence.
    /// This is an evidence gate, not a factual truth oracle.
    pub fn can_complete(&self, snapshot: &DomSnapshot) -> Result<(), String> {
        if super::deliberation::error_page(snapshot) {return Err("The current page is an error or access restriction".into());}
        if let Some(reason) = crate::agent::prerequisites::completion_blocker(&self.plan.goal,snapshot) { return Err(reason.into()); }
        if super::skills::TaskSpec::compile(&self.plan.goal).is_some_and(|spec|!spec.observed_target(snapshot)){
            return Err("Compiled target acquisition objective has not been independently observed".into());
        }
        let mut requirements=super::requirements::Ledger::compile(&self.plan.goal);
        if requirements.supported{
            requirements.refresh(&self.plan.goal,&self.evidence);
            requirements.can_review()?;
        }
        if self.last_action_failed {
            return Err("Previous browser action failed; re-observe and recover first".into());
        }
        if let Some(target) = requested_navigation_url(&self.plan.goal) {
            if self.confirmed_navigation.as_deref() != Some(snapshot.origin.as_str())
                || self.successful_actions == 0
                || self.fresh_observations == 0
                || !self.evidence.iter().any(|e| e.url == snapshot.origin)
                || check_navigation_result(target.as_str(), snapshot).is_err()
            {
                return Err("Requested navigation has not been independently confirmed from the live browser".into());
            }
            if self.specialist() == Specialist::Verification
                && self.completed_summaries.len() != self.current
            {
                return Err("Earlier specialists have not completed".into());
            }
            if self.specialist() != Specialist::Verification && self.actions_in_stage == 0 {
                return Err("No successful navigation action in the current stage".into());
            }
            return Ok(());
        }
        if self.fresh_observations == 0 || self.evidence.is_empty() {
            return Err("No live webpage evidence was observed; do not claim completion".into());
        }
        if !crate::agent::deliberation::missing_research_sources(&self.plan.goal,&self.evidence).is_empty(){
            return Err("Not all explicitly requested research sources have been observed".into());
        }
        if requested_youtube_video_page(&self.plan.goal)
            && !observed_youtube_video_page(&snapshot.origin)
        {
            return Err("The requested video-result page has not been observed in the browser".into());
        }
        if snapshot.text_excerpt.trim().len() < 40 {
            return Err("The current page has insufficient readable evidence".into());
        }
        let is_verifier = self.specialist() == Specialist::Verification;
        if is_verifier && self.completed_summaries.len() != self.current {
            return Err("Earlier specialists have not completed".into());
        }
        // Explicit read-current-reference tasks can use actual source evidence
        // without manufacturing a click or navigation receipt.
        let observation_only=super::execution_intelligence::current_reference_observed(&self.plan.goal,snapshot,&self.evidence);
        if self.successful_actions == 0 && !observation_only {
            return Err("No browser actions were acknowledged as successful".into());
        }
        // Observation-only research, comparison, and extraction stages can
        // reuse fresh recorded source evidence. Requiring an extra click/scroll
        // after a confirmed submission can cause needless duplicate actions.
        let read_only_stage = matches!(self.specialist(),
            Specialist::Research | Specialist::Comparison | Specialist::Data);
        if !is_verifier && self.actions_in_stage == 0 && !read_only_stage {
            return Err("Specialist has not yet executed a successful browser action".into());
        }
        let terms = goal_terms(&self.plan.goal);
        if !terms.is_empty() {
            let evidence_text = self.evidence.iter().map(|e| e.excerpt.to_lowercase()).collect::<Vec<_>>().join(" ");
            if !terms.iter().any(|t| evidence_text.contains(t)) {
                return Err("Observed pages do not yet contain relevant goal evidence".into());
            }
        }
        if is_verifier || self.specialist() == Specialist::Form {
            let goal = self.plan.goal.to_lowercase();
            let needs_submit = contains_any(&goal,
                &["submit", "apply ", "register", "sign up", "confirm application"]);
            if needs_submit && !self.form_submitted {
                return Err("The requested form submission has not been acknowledged as successful".into());
            }
        }
        if self.specialist() == Specialist::Comparison {
            let distinct_hosts: HashSet<String> = self.evidence.iter()
                .filter_map(|e| url::Url::parse(&e.url).ok()
                    .and_then(|u| u.host_str().map(str::to_owned)))
                .collect();
            if distinct_hosts.len() < 2 {
                return Err("A multi-source comparison needs evidence from two distinct sites".into());
            }
        }
        Ok(())
    }

    /// Returns true only after every specialist stage, including verification.
    pub fn complete_stage(&mut self, summary: &str) -> bool {
        let current = &mut self.plan.stages[self.current];
        current.status = "completed";
        self.completed_summaries.push(summary.chars().take(320).collect());
        self.current += 1;
        self.actions_in_stage = 0;
        self.last_action_failed = false;
        if self.current < self.plan.stages.len() {
            self.plan.stages[self.current].status = "active";
            false
        } else {
            true
        }
    }

    pub fn can_reopen_research(&self)->bool{
        self.specialist()==Specialist::Verification&&self.current>0&&
            matches!(self.plan.stages[self.current-1].specialist,Specialist::Browser|Specialist::Research|Specialist::Comparison)&&
            super::deliberation::read_only(&self.plan.goal)
    }
    pub fn reopen_research(&mut self)->bool{
        if !self.can_reopen_research(){return false;}
        self.plan.stages[self.current].status="pending";
        self.current-=1;self.plan.stages[self.current].status="active";
        self.completed_summaries.pop();self.actions_in_stage=0;self.last_action_failed=false;
        true
    }
    pub fn begin(&mut self) {
        self.plan.stages[self.current].status = "active";
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot(url: &str, text: &str) -> DomSnapshot {
        DomSnapshot { structure: Default::default(), origin: url.to_string(), title: "Demo".into(),
            text_excerpt: text.to_string(), interactive_nodes: vec![] }
    }
    #[test]
    fn video_result_requires_a_real_observed_youtube_watch_url() {
        assert!(requested_youtube_video_page(
            "Search YouTube for NASA Artemis and open the first video result"));
        assert!(!requested_youtube_video_page("Open YouTube"));
        assert!(!observed_youtube_video_page("https://www.youtube.com/"));
        assert!(!observed_youtube_video_page("https://www.youtube.com/results?search_query=nasa"));
        assert!(!observed_youtube_video_page("https://www.youtube.com.evil.test/watch?v=123"));
        assert!(!observed_youtube_video_page("https://www.youtube.com/watch"));
        assert!(observed_youtube_video_page("https://www.youtube.com/watch?v=example123"));
        assert!(observed_youtube_video_page("https://www.youtube.com/shorts/example123"));
    }

    #[test]
    fn model_generated_plans_are_validated_and_ended_with_verification() {
        let raw = r#"{"stages":[
            {"specialist":"research","objective":"Visit real official sources","success_criteria":"Read two pages"},
            {"specialist":"comparison","objective":"Compare observed prices","success_criteria":"Check two sources"}
        ]}"#;
        let plan = parse_generated_plan("Compare laptops", raw).unwrap();
        assert_eq!(plan.stages[0].specialist, Specialist::Research);
        assert_eq!(plan.stages[1].specialist, Specialist::Comparison);
        assert_eq!(plan.stages[2].specialist, Specialist::Verification);
        assert!(parse_generated_plan("Demo", r#"{"stages":[{"specialist":"shell","objective":"run","success_criteria":"done"}]}"#).is_err());
        assert!(parse_generated_plan("Demo", r#"{"stages":[]}"#).is_err());
    }

    #[test]
    fn planner_accepts_complete_fences_but_rejects_extra_json_and_unknown_fields() {
        let valid = r#"{"stages":[{"specialist":"browser","objective":"Open the requested page","success_criteria":"Page observed"}]}"#;
        let fenced = format!("```json\n{valid}\n```");
        let plan = parse_generated_plan("Open page", &fenced).unwrap();
        assert_eq!(plan.stages.len(), 2);
        assert_eq!(plan.stages[1].specialist, Specialist::Verification);
        assert!(parse_generated_plan("Open page", &format!("{valid}{valid}"))
            .unwrap_err().to_string().contains("trailing characters"));
        assert!(parse_generated_plan("Open page", &format!("{valid}\nexplanation"))
            .is_err());
        assert!(parse_generated_plan("Open page", &format!("```json\n{valid}"))
            .is_err());
        assert!(parse_generated_plan("Open page", &valid.replace(
            "\"objective\":", "\"command\":\"shell\",\"objective\":"))
            .is_err());
        assert!(parse_generated_plan("Open page", &valid.replace(
            "\"stages\":", "\"tools\":[],\"stages\":"))
            .is_err());
    }

    #[test]
    fn named_article_title_lookup_skips_only_redundant_cloud_planning() {
        for goal in [
            "On this Wikipedia page, search for Ada Lovelace and open the corresponding article. Then report its page title.",
            "On this Python documentation page, search for asyncio and open the corresponding reference article. Then report its page title.",
            "On this MDN website, search for Array and open the corresponding JavaScript reference article. Then report its page title.",
            "On this Wiktionary page, search for photosynthesis and open the corresponding article. Then report its page title.",
        ] {
            assert!(is_simple_named_title_task(goal), "unexpected rejection: {goal}");
            let stages = plan_task(goal).stages.into_iter()
                .map(|stage| stage.specialist).collect::<Vec<_>>();
            assert_eq!(stages, vec![Specialist::Browser, Specialist::Verification]);
        }
        for goal in [
            "Search for Ada Lovelace and open her article, then purchase a membership and report the title",
            "Search for Ada Lovelace and open her article, then submit my form and report the title",
            "Search for Ada Lovelace and open her article, compare other sources and report the title",
            "Search for Ada Lovelace and report its title without opening the article",
            "Research Ada Lovelace and write a comparison of multiple webpages",
        ] {
            assert!(!is_simple_named_title_task(goal), "unsafe optimization: {goal}");
        }
        // Direct URL opening retains its own canonical route, not an inference shortcut.
        assert!(!is_simple_named_title_task("Open github.com"));
    }

    #[test]
    fn routes_comparison_through_real_specialists_then_verification() {
        let plan = plan_task("Compare two laptop specifications");
        assert_eq!(plan.stages.iter().map(|s| s.specialist).collect::<Vec<_>>(),
            vec![Specialist::Research, Specialist::Comparison, Specialist::Verification]);
    }
    #[test]
    fn routes_forms_and_structured_extraction_separately() {
        let form = plan_task("Fill my internship application form");
        assert_eq!(form.stages[1].specialist, Specialist::Form);
        let data = plan_task("Extract the prices into a table");
        assert_eq!(data.stages[1].specialist, Specialist::Data);
        assert_eq!(plan_task("Open example.com").stages[0].specialist, Specialist::Browser);
    }
    #[test]
    fn specialist_permissions_block_unsafe_delegation() {
        let submit = BrowserAction::Click {
            node_id: "submit".into(), element_label: "Submit".into(), is_submit: true
        };
        assert!(!Specialist::Research.allows(&submit));
        assert!(!Specialist::Comparison.allows(&submit));
        assert!(Specialist::Form.allows(&submit)); // the live policy decides if confirmation is required
        assert!(!Specialist::Verification.allows(&submit));
        assert!(Specialist::Verification.allows(&BrowserAction::Complete {summary:"Done".into()}));
    }
    #[test]
    fn research_agent_can_search_but_not_type_into_unrelated_or_sensitive_fields() {
        let mut page = snapshot("https://search.example", "An example search page with information to research.");
        page.interactive_nodes.push(crate::agent::dom_extractor::DomNodeSummary {
            href: None,
            node_id: "query".into(), tag: "input".into(), role: "searchbox".into(),
            label: "Search this site".into(), is_submit: false, is_sensitive: false, options: vec![], checked: None, search_state: None, bbox: None
        });
        let query = BrowserAction::TypeText {
            node_id: "query".into(), text: "web research".into(), is_sensitive: false
        };
        assert!(Specialist::Research.allows_on_page(&query, &page));
        let unrelated = BrowserAction::TypeText {
            node_id: "email".into(), text: "example".into(), is_sensitive: false
        };
        assert!(!Specialist::Research.allows_on_page(&unrelated, &page));
    }

    #[test]
    fn form_specialist_cannot_report_submission_without_successful_submit() {
        let mut o = Orchestrator::new("Submit my application form");
        o.begin();
        assert_eq!(o.specialist(), Specialist::Browser);
        assert!(!o.complete_stage("Navigated to the form"));
        let form = snapshot("https://apply.example", "The internship application form is ready for submission.");
        o.observe(&form);
        o.action_outcome(&BrowserAction::TypeText {
            node_id: "n_2".into(), text: "Example".into(), is_sensitive: false
        }, true);
        assert!(o.can_complete(&form).is_err());
        o.action_outcome(&BrowserAction::Click {
            node_id: "n_3".into(), element_label: "Submit".into(), is_submit: true
        }, true);
        assert!(o.can_complete(&form).is_ok());
    }

    #[test]
    fn cannot_claim_success_without_observation_and_actions() {
        let mut orchestrator = Orchestrator::new("Open example.com");
        orchestrator.begin();
        let page = snapshot("https://example.com", "Example Domain is an example website with information.");
        assert!(orchestrator.can_complete(&page).is_err());
        orchestrator.observe(&page);
        assert!(orchestrator.can_complete(&page).is_err());
        orchestrator.action_outcome(&BrowserAction::Scroll {delta_y:250}, true);
        assert!(orchestrator.can_complete(&page).is_err()); // scrolling does not open a website
        orchestrator.action_outcome(&BrowserAction::Navigate { url: "https://example.com/".into() }, true);
        orchestrator.confirmed_navigation(&page);
        assert!(orchestrator.can_complete(&page).is_ok());
    }
    #[test]
    fn short_navigation_page_still_needs_confirmation_but_can_finish_once_verified() {
        let mut o = Orchestrator::new("open youtube");
        o.begin();
        let page = snapshot("https://www.youtube.com/", "YouTube home");
        o.observe(&page);
        o.action_outcome(&BrowserAction::Navigate { url: page.origin.clone() }, true);
        assert!(o.can_complete(&page).is_err());
        o.confirmed_navigation(&page);
        assert!(o.can_complete(&page).is_ok());
        assert!(!o.complete_stage("Requested destination observed"));
        o.observe(&page);
        assert!(o.can_complete(&page).is_ok());
        let wrong = snapshot("https://youtube.com.evil.test/", "YouTube home");
        assert!(o.can_complete(&wrong).is_err());
    }

    #[test]
    fn cannot_complete_after_failed_action_or_without_two_comparison_sources() {
        let mut orchestrator = Orchestrator::new("Compare laptop specifications");
        orchestrator.begin();
        let page = snapshot("https://one.example", "Laptop specifications include memory and processor speed.");
        orchestrator.observe(&page);
        orchestrator.action_outcome(&BrowserAction::Scroll {delta_y:200}, true);
        assert!(orchestrator.can_complete(&page).is_ok());
        assert!(!orchestrator.complete_stage("Read laptop specifications"));
        assert!(orchestrator.can_complete(&page).is_err());
        orchestrator.observe(&snapshot("https://two.example", "More laptop specifications and price comparisons are available."));
        assert!(orchestrator.can_complete(&page).is_ok());
        orchestrator.action_outcome(&BrowserAction::Scroll {delta_y:200}, false);
        assert!(orchestrator.can_complete(&page).is_err());
    }
}

#[cfg(test)]
mod autonomous_read_only_transition_tests {
    use super::*;

    #[test]
    fn data_agent_uses_existing_confirmation_without_artificial_action() {
        let plan = parse_generated_plan(
            "Submit feedback on the current page",
            r#"{"stages":[
                {"specialist":"browser","objective":"Submit feedback","success_criteria":"Confirm feedback received"},
                {"specialist":"data","objective":"Read submission confirmation","success_criteria":"Extract confirmation text"}
            ]}"#
        ).unwrap();
        let mut flow = Orchestrator::from_plan(plan);
        let snapshot = DomSnapshot {
            structure: Default::default(), origin: "https://example.org/feedback".into(),
            title: "Feedback".into(),
            text_excerpt: "Feedback form. Feedback sent successfully on the local test page.".into(),
            interactive_nodes: vec![],
        };
        flow.observe(&snapshot);
        flow.action_outcome(&BrowserAction::Click {
            node_id: "send".into(), element_label: "Submit feedback".into(), is_submit: true,
        }, true);
        assert!(flow.can_complete(&snapshot).is_ok());
        assert!(!flow.complete_stage("Observed feedback confirmation"));
        assert_eq!(flow.specialist(), Specialist::Data);
        // Data is read-only; a fresh observed confirmation is sufficient.
        assert!(flow.can_complete(&snapshot).is_ok());
    }
}
