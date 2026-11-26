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
    #[serde(skip_serializing_if="Option::is_none")]
    pub semantic: Option<super::semantic_task::SemanticTask>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub semantic_provenance: Option<&'static str>,
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
    #[serde(default)]
    semantic: Option<super::semantic_task::SemanticTask>,
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
    let semantic=generated.semantic;
    if semantic.is_some(){
        anyhow::ensure!(generated.stages.iter().all(|s|s.specialist!=Specialist::Verification),"Semantic stage mappings cannot include a model-generated verification stage");
    }
    for stage in generated.stages {
        if stage.specialist == Specialist::Verification { continue; }
        let objective = stage.objective.trim().chars()
            .filter(|c| !c.is_control() && *c != '<' && *c != '>')
            .take(200).collect::<String>();
        let success_criteria = stage.success_criteria.trim().chars()
            .filter(|c| !c.is_control() && *c != '<' && *c != '>')
            .take(200).collect::<String>();
        anyhow::ensure!(!objective.is_empty() && !success_criteria.is_empty(),
            "The model's plan is missing a stage objective or success criterion");
        let identity=(stage.specialist,objective.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase());
        anyhow::ensure!(seen.insert(identity),"Duplicate execution objective in specialist plan; provide distinct sequential objectives");
        stages.push(SpecialistStage {
            specialist: stage.specialist, objective, success_criteria, status:"pending"
        });
        anyhow::ensure!(stages.len() <= 4, "Too many execution stages requested");
    }
    anyhow::ensure!(!stages.is_empty(), "The model must assign at least one execution specialist");
    stages.push(SpecialistStage {
        specialist: Specialist::Verification,
        objective: format!("Check evidence and results for: {goal}").chars().take(200).collect(),
        success_criteria: "Confirm observed webpages and successfully executed actions; never invent success".into(),
        status: "pending",
    });
    let mut semantic=semantic;
    let mut rejected_ungrounded_advisory=false;
    if let Some(task)=semantic.as_mut(){
        if let Err(error)=task.ground_spelling(goal).and_then(|_|task.validate(goal,stages.len()-1)){
            // Advisory semantic labels cannot abort an otherwise schema-valid
            // staged plan solely because a model invented a literal span.
            // This changes no action permission: the exact original goal,
            // staged native policy and independent verification stay active.
            if super::semantic_task::ungrounded_advisory_can_fall_back(goal,&error.to_string(),Some(task)){
                semantic=None;
                rejected_ungrounded_advisory=true;
            }else{return Err(error)}
        }
    }
    let semantic_provenance=if rejected_ungrounded_advisory{
        Some("rejected_ungrounded_advisory_native_policy")
    }else{semantic.as_ref().map(|_|"model_proposed")};
    Ok(TaskPlan { goal: goal.to_string(), stages, semantic, semantic_provenance })
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
    stage_entry_evidence: Vec<SourceEvidence>,
    starting_origin: Option<String>,
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

/// Explicitly current-page forms need no destination discovery stage. The
/// ordinary native Form specialist and post-submit verifier remain mandatory.
pub fn current_page_form(goal:&str)->bool{
    let lower=goal.to_ascii_lowercase();
    ["on the current","on this page","current website","current page"].iter()
        .any(|term|lower.contains(term)) && lower.contains("form") &&
        contains_any(&lower,&["fill ","submit ","apply ","register ","complete the form","sign up"])
}
/// A deterministic minimum plan, built from the user's intent.
/// The selected LLM decides actual actions for each specialist at runtime.
pub fn plan_task(goal: &str) -> TaskPlan {
    let lower = goal.to_lowercase();
    let mut roles: Vec<Specialist> = if super::policy::requested_wikipedia_article_url(goal).is_some() || crate::agent::prerequisites::route_values(goal).is_some() || crate::agent::prerequisites::filter_constraints(goal).is_some() || is_simple_named_title_task(goal) || super::evidence_verifier::requested_named_link_title(goal).is_some() {
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
        // A user who explicitly assigned a form on the CURRENT page should
        // not be forced through a navigation specialist first. It can only
        // propose unrelated URLs and stall despite a visible, authorized
        // control. The Form stage still must satisfy native action permissions
        // and observe a post-submission confirmation before verification.
        if current_page_form(goal){
            vec![Specialist::Form]
        }else{vec![Specialist::Browser, Specialist::Form]}
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
    let named_sources=super::deliberation::named_research_entities(goal);
    if !named_sources.is_empty() {
        roles=vec![Specialist::Research;named_sources.len()];
        roles.push(Specialist::Comparison);
    }
    roles.push(Specialist::Verification);
    TaskPlan {
        goal: goal.to_string(),
        semantic: None,
        semantic_provenance: None,
        stages: roles.into_iter().enumerate().map(|(stage_index,specialist)| {
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
            let (objective, criteria)=if specialist==Specialist::Research && stage_index<named_sources.len() {
                let name=&named_sources[stage_index];
                (format!("Visit the official website of {name} and capture free-plan details for: {goal}"),
                 format!("Capture relevant plan evidence from the actual {name} website, not a search listing"))
            } else if specialist==Specialist::Comparison && !named_sources.is_empty() {
                (objective,format!("Compare evidence from all {} named websites while preserving user constraints",named_sources.len()))
            } else {(objective,criteria)};
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
            stage_entry_evidence: Vec::new(),
            starting_origin: None,
        }
    }

    pub fn specialist(&self) -> Specialist {
        self.plan.stages[self.current].specialist
    }

    pub fn stage(&self) -> &SpecialistStage {
        &self.plan.stages[self.current]
    }

    pub fn set_starting_origin(&mut self,origin:&str){
        if self.starting_origin.is_none(){self.starting_origin=Some(origin.to_owned());}
    }
    pub fn observe(&mut self, snapshot: &DomSnapshot) {
        self.set_starting_origin(&snapshot.origin);
        let Some(evidence)=super::browser_skills::extraction::capture(snapshot)else{return;};
        self.fresh_observations+=1;
        if let Some(old) = self.evidence.iter_mut().find(|e| e.url == evidence.url) {
            // Prefer the stronger actual capture on explicit factual research.
            // Never stitch different snapshots together into a purported quote.
            let goal=&self.plan.goal;
            let explicit=super::deliberation::explicit_research_sources(goal);
            let scoped=url::Url::parse(&evidence.url).ok().is_some_and(|u|explicit.contains(&u));
            if !scoped||super::evidence_verifier::fact_capture_quality(goal,&evidence)>=super::evidence_verifier::fact_capture_quality(goal,old){
                *old = evidence;
            }
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

    /// Bounded, host-generated plan state survives compact model context.
    /// Objectives are advisory and never confer permissions.
    pub fn semantic_focus(&self)->String{
        let objective=&self.stage().objective;
        let anchored=self.plan.semantic.as_ref().map(|s|s.focus(self.current+1)).unwrap_or_default();
        format!("{objective} {anchored}")
    }
    pub fn stage_navigation_label(&self)->Option<String>{
        if self.specialist()!=Specialist::Browser{return None;}
        self.plan.semantic.as_ref()?.navigation_label(self.current+1)
    }
    pub fn semantic_hint(&self)->String{
        self.plan.semantic.as_ref().map(|s|s.hint(self.current+1)).unwrap_or_else(||"No model semantic graph; use original goal and native evidence.".into())
    }
    pub fn plan_progress(&self)->String {
        let stages=self.plan.stages.iter().enumerate().map(|(i,stage)|{
            let status=if i<self.current{"completed"}else if i==self.current{"active"}else{"pending"};
            format!("{} {} {}: {}",i+1,status,stage.specialist.label(),stage.objective.chars().take(100).collect::<String>())
        }).collect::<Vec<_>>().join(" | ");
        format!("Step {}/{}; {}. Advance only the active objective; do not replay completed stages.",self.current+1,self.plan.stages.len(),stages)
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
        // A multi-source research plan acquires ONE named site per specialist
        // stage. Requiring the global source ledger to be complete in stage 1
        // deadlocks the plan before stages 2 and 3 can run. Each acquisition
        // stage still needs its own observed site (checked below); only the
        // comparison/verification stages require all source requirements.
        let staged_named_research=self.specialist()==Specialist::Research &&
            !super::deliberation::named_research_entities(&self.plan.goal).is_empty();
        // Compile factual dependencies before allowing a named source stage to
        // hand off. A visited homepage or generic 'Free' banner cannot stand in
        // for the user's requested free-tier team capacity. Later stages require
        // every named provider's source-exact candidate; independent verification
        // remains the only mechanism that can approve the final recommendation.
        if let Some(facts)=super::fact_research::Contract::compile(&self.plan.goal){
            let missing=if staged_named_research{
                facts.missing_stage(&self.plan.goal,&self.evidence,self.current)
                    .into_iter().collect::<Vec<_>>()
            }else if matches!(self.specialist(),Specialist::Comparison|Specialist::Verification){
                facts.missing(&self.plan.goal,&self.evidence)
            }else{Vec::new()};
            if let Some(next)=missing.first(){
                let facet=if next.needs_team_capacity{"free-tier collaborator/member limit"}
                    else{"free-tier plan details"};
                return Err(format!("Missing source-exact {facet} for {} before synthesis",next.source));
            }
        }
        if matches!(self.specialist(),Specialist::Comparison|Specialist::Verification){
            if let Some(mut graph)=super::fact_graph::FactGraph::compile(&self.plan.goal){
                graph.observe(&self.evidence);
                if !graph.can_synthesize(){return Err(graph.hint());}
            }
        }
        if requirements.supported&&!staged_named_research{
            requirements.refresh(&self.plan.goal,&self.evidence);
            requirements.can_review()?;
        }
        if self.last_action_failed {
            return Err("Previous browser action failed; re-observe and recover first".into());
        }
        if let Some(label)=self.stage_navigation_label(){
            if !super::prerequisites::stage_title_matches(&label,snapshot)||
                !self.evidence.iter().any(|e|e.url==snapshot.origin&&e.title==snapshot.title&&!e.excerpt.trim().is_empty())||
                self.actions_in_stage==0{
                return Err("Active named navigation objective has no independently observed destination receipt".into());
            }
        }
        if let Some(target) = requested_navigation_url(&self.plan.goal) {
            if super::policy::requested_wikipedia_article_url(&self.plan.goal).is_some(){
                super::evidence_verifier::verify_wikipedia_article(
                    &self.plan.goal,snapshot,&self.evidence)
                    .map_err(|_|"Requested Wikipedia article title and source are not independently verified")?;
            }
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
        // A search form or unrelated result cannot complete a named-title
        // acquisition stage. The final verifier still requires action receipts.
        if super::evidence_verifier::requested_named_result_title(&self.plan.goal).is_some(){
            let initial=self.starting_origin.as_deref().unwrap_or(&snapshot.origin);
            if super::evidence_verifier::verify_observed_named_result_title(&self.plan.goal,initial,snapshot,&self.evidence).is_err(){
                return Err("Requested named result title and destination are not yet observed".into());
            }
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
        // A repeated acquisition specialist represents a distinct objective.
        // Do not let the previous stage's retained sources alone satisfy it.
        if matches!(self.specialist(),Specialist::Research|Specialist::Data)
            && self.plan.stages[..self.current].iter().any(|s|s.specialist==self.specialist()){
            let mut distinct=super::semantic_core::stage_focus(&self.plan.goal,&self.stage().objective);
            for prior in &self.plan.stages[..self.current]{
                for term in super::semantic_core::stage_focus(&self.plan.goal,&prior.objective){distinct.remove(&term);}
            }
            let fresh=self.evidence.iter().filter(|now|!self.stage_entry_evidence.iter().any(|old|
                old.url==now.url&&old.title==now.title&&old.excerpt==now.excerpt));
            if !fresh.clone().any(|_|true){
                return Err("Repeated acquisition stage needs new observed evidence, not earlier stage evidence alone".into());
            }
            // Only explicit user terms unique to this stage may narrow the
            // evidence gate. Model-invented requirements do not become facts.
            if !distinct.is_empty()&&!fresh.into_iter().any(|e|{
                let found=super::semantic_core::objective_words(&format!("{} {} {}",e.title,e.url,e.excerpt));
                !distinct.is_disjoint(&found)
            }){
                return Err("Fresh evidence does not address the distinct unfinished acquisition objective".into());
            }
        }
        let named=super::deliberation::named_research_entities(&self.plan.goal);
        if !named.is_empty() {
            let observed=|name:&str|self.evidence.iter().any(|e|{
                let Ok(u)=url::Url::parse(&e.url)else{return false};
                let Some(host)=u.host_str()else{return false};
                let brand=name.to_ascii_lowercase().replace(' ',"").replace('-',"");
                let h=host.to_ascii_lowercase().replace('-',"");
                let excerpt=e.excerpt.to_ascii_lowercase();
                let needs_free=self.plan.goal.to_ascii_lowercase().contains("free plan");
                !["bing.com","google.com","duckduckgo.com","yahoo.com"].iter()
                    .any(|d|h==*d||h.ends_with(&format!(".{d}")))
                    && h.contains(&brand) && excerpt.trim().len()>=80
                    && (!needs_free || (excerpt.contains("free")
                        && (excerpt.contains("plan")||excerpt.contains("pricing"))))
            });
            if self.specialist()==Specialist::Research {
                let name=&named[self.current.min(named.len()-1)];
                if !observed(name){return Err("Named official website not observed with readable evidence".into());}
            }
            if matches!(self.specialist(),Specialist::Comparison|Specialist::Verification)
                && named.iter().any(|name|!observed(name)){
                return Err("All named official websites require observed evidence before comparison".into());
            }
        }
        if self.specialist() == Specialist::Comparison {
            let distinct_hosts: HashSet<String> = self.evidence.iter()
                .filter_map(|e| url::Url::parse(&e.url).ok()
                    .and_then(|u| u.host_str().map(str::to_owned)))
                .collect();
            if distinct_hosts.len() < named.len().max(2) {
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
        self.stage_entry_evidence=self.evidence.clone();
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
        self.stage_entry_evidence=self.evidence.clone();
        true
    }
    pub fn begin(&mut self) {
        self.plan.stages[self.current].status = "active";
    }
}

#[cfg(test)]
mod explicit_wikipedia_article_plan_tests{
    use super::*;
    #[test]
    fn wikipedia_article_lookup_uses_browser_then_verification(){
        let g="Open Wikipedia, find the Ada Lovelace article, and report its title and source URL. Verify the final webpage before finishing.";
        let p=plan_task(g);
        assert_eq!(p.stages.len(),2);
        assert_eq!(p.stages[0].specialist,Specialist::Browser);
        assert_eq!(p.stages[1].specialist,Specialist::Verification);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn each_named_site_stage_can_finish_before_global_source_coverage(){
        let goal="Visit the official websites of Alpha, Beta and Gamma. Compare their free plans for a team of four and recommend one.";
        let mut run=Orchestrator::from_plan(plan_task(goal));
        run.begin();
        let alpha=snapshot("https://alpha.com/pricing",
            &"Alpha Free plan includes up to 10 members per team workspace. ".repeat(4));
        run.observe(&alpha);
        run.action_outcome(&BrowserAction::Navigate{url:alpha.origin.clone()},true);
        assert!(run.can_complete(&alpha).is_ok(),"first source stage must not require Beta and Gamma: {:?}",run.can_complete(&alpha));
        assert!(!run.complete_stage("Observed Alpha"));
        assert!(run.can_complete(&alpha).is_err(),"second stage cannot reuse Alpha as Beta");
        let beta=snapshot("https://beta.net/pricing",
            &"Beta Free plan includes up to four members per team workspace. ".repeat(4));
        run.observe(&beta);
        run.action_outcome(&BrowserAction::Navigate{url:beta.origin.clone()},true);
        assert!(run.can_complete(&beta).is_ok(),"second source should be eligible once Beta observed: {:?}",run.can_complete(&beta));
        assert!(!run.complete_stage("Observed Beta"));
        assert!(run.can_complete(&beta).is_err(),"third stage cannot reuse Beta as Gamma");
    }
    #[test]
    fn named_research_stage_blocks_premature_completion_until_exact_team_fact(){
        let goal="Visit the official websites of Alpha, Beta and Gamma. Compare their free plans for a team of four and recommend one.";
        let mut run=Orchestrator::from_plan(plan_task(goal));
        run.begin();
        let weak=snapshot("https://alpha.com/pricing",
            &"Alpha Free plan has collaborative tools for small teams. ".repeat(4));
        run.observe(&weak);
        run.action_outcome(&BrowserAction::Navigate{url:weak.origin.clone()},true);
        assert!(run.can_complete(&weak).unwrap_err().contains("collaborator/member limit"));
        let stronger=snapshot("https://alpha.com/pricing",
            &"Alpha Free plan allows up to 10 team members in one shared workspace. ".repeat(4));
        run.observe(&stronger);
        assert!(run.can_complete(&stronger).is_ok(),"observed source-exact member limit should release only this stage: {:?}",run.can_complete(&stronger));
        run.complete_stage("Source Alpha facts captured");
        assert!(run.can_complete(&stronger).is_err(),"Alpha receipt must not satisfy Beta");
    }
    #[test]
    fn named_websites_require_separate_research_stages() {
        let goal="Visit the official websites of Notion, Trello and ClickUp. Compare their free plans for a five-member team and recommend the most suitable option.";
        let p=plan_task(goal);
        assert_eq!(p.stages.len(),5);
        assert_eq!(p.stages.iter().filter(|s|s.specialist==Specialist::Research).count(),3);
        for name in ["Notion","Trello","ClickUp"] {
            assert_eq!(p.stages.iter().filter(|s|s.objective.contains(&format!("website of {name}"))).count(),1);
        }
        assert!(p.stages[3].success_criteria.contains("all 3 named websites"));
        assert_eq!(p.stages[4].specialist,Specialist::Verification);
    }
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
    fn repeated_specialists_preserve_distinct_ordered_objectives_and_require_fresh_evidence() {
        let raw=r#"{"stages":[{"specialist":"research","objective":"Read first laptop source","success_criteria":"First source observed"},{"specialist":"research","objective":"Read second laptop source","success_criteria":"Second source observed"},{"specialist":"data","objective":"Extract laptop facts","success_criteria":"Sourced facts"}]}"#;
        let plan=parse_generated_plan("Research laptop facts",raw).unwrap();
        assert_eq!(plan.stages.len(),4);
        assert_eq!(plan.stages[1].objective,"Read second laptop source");
        let mut flow=Orchestrator::from_plan(plan);
        flow.begin();
        let a=snapshot("https://one.example/","Laptop facts and information from the first independent source.");
        flow.observe(&a);
        flow.action_outcome(&BrowserAction::Scroll{delta_y:100},true);
        assert!(flow.can_complete(&a).is_ok());
        assert!(!flow.complete_stage("First source acquired"));
        assert!(flow.plan_progress().contains("1 completed Research Agent"));
        assert!(flow.plan_progress().contains("2 active Research Agent"));
        assert!(flow.plan_progress().contains("3 pending Data Agent"));
        flow.observe(&a);
        assert!(flow.can_complete(&a).is_err());
        let b=snapshot("https://two.example/","Laptop facts and information from the second independent source.");
        flow.observe(&b);
        assert!(flow.can_complete(&b).is_ok());
        assert!(!flow.complete_stage("Second source acquired"));
        assert_eq!(flow.specialist(),Specialist::Data);
        assert!(parse_generated_plan("Research laptop facts",&raw.replace("Read second laptop source","Read first laptop source")).is_err());
    }

    #[test]
    fn repeated_acquisition_rejects_fresh_but_irrelevant_evidence(){
        let plan=parse_generated_plan("Research installation and compatibility details",
            r#"{"stages":[{"specialist":"research","objective":"Read installation details","success_criteria":"Installation observed"},{"specialist":"research","objective":"Read compatibility details","success_criteria":"Compatibility observed"}]}"#).unwrap();
        let mut flow=Orchestrator::from_plan(plan);flow.begin();
        let first=snapshot("https://example.org/install","Installation details and documented installation instructions for this public library.");
        flow.observe(&first);flow.action_outcome(&BrowserAction::Scroll{delta_y:120},true);
        assert!(flow.can_complete(&first).is_ok());flow.complete_stage("Installation observed");
        let wrong=snapshot("https://example.org/help","Installation details and a newly observed public help page with unrelated information.");
        flow.observe(&wrong);
        assert!(flow.can_complete(&wrong).unwrap_err().contains("distinct unfinished"));
        let right=snapshot("https://example.org/compat","Compatibility details and supported runtime information for the current public library.");
        flow.observe(&right);assert!(flow.can_complete(&right).is_ok());
    }
    #[test]
    fn semantic_plan_accepts_only_user_anchored_interpretation(){
        let goal="Find the guide, then compare versions; do not submit.";
        let raw=r#"{"stages":[{"specialist":"research","objective":"Locate guide","success_criteria":"Observed guide"},{"specialist":"comparison","objective":"Compare versions","success_criteria":"Sourced versions"}],"semantic":{"concepts":[{"kind":"objective","span":"Find the guide","stages":[1]},{"kind":"objective","span":"compare versions","stages":[2]},{"kind":"constraint","span":"do not submit","stages":[]}],"relations":[{"from":2,"to":1,"kind":"depends_on"}]}}"#;
        let plan=parse_generated_plan(goal,raw).unwrap();let flow=Orchestrator::from_plan(plan);
        assert!(flow.semantic_focus().contains("Find the guide"));assert!(flow.semantic_hint().contains("do not submit"));
        assert!(parse_generated_plan(goal,&raw.replace("do not submit","submit automatically")).is_err());
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
    fn current_page_form_does_not_require_unrelated_navigation(){
        let goal="On the current local test website, submit the feedback form, then report the visible feedback confirmation. Do not submit the application form.";
        let stages=plan_task(goal).stages;
        assert_eq!(stages.len(),2);
        assert_eq!(stages[0].specialist,Specialist::Form);
        assert_eq!(stages[1].specialist,Specialist::Verification);
        let remote=plan_task("Open the company website and submit its feedback form").stages;
        assert_eq!(remote[0].specialist,Specialist::Browser);
        assert_eq!(remote[1].specialist,Specialist::Form);
    }
    #[test]
    fn invented_advisory_span_does_not_abort_valid_form_stages(){
        let goal="On the current local test website, submit the feedback form, then report the visible feedback confirmation. Do not submit the application form.";
        let raw=serde_json::json!({"stages":[{"specialist":"form","objective":"Submit only the requested feedback form","success_criteria":"Visible feedback confirmation"}],
            "semantic":{"concepts":[{"kind":"objective","span":"Submit the unrelated application immediately","stages":[1]}],"relations":[]}}).to_string();
        let p=parse_generated_plan(goal,&raw).unwrap();
        assert!(p.semantic.is_none());
        assert_eq!(p.semantic_provenance,Some("rejected_ungrounded_advisory_native_policy"));
        assert_eq!(p.goal,goal);
        assert_eq!(p.stages.len(),2);
        assert_eq!(p.stages[0].specialist,Specialist::Form);
        assert_eq!(p.stages[1].specialist,Specialist::Verification);
    }
    #[test]
    fn explicit_order_cannot_be_discarded_on_an_invented_span(){
        let goal="First inspect the public guide, then submit the feedback form. Do not submit the application form.";
        let raw=serde_json::json!({"stages":[{"specialist":"research","objective":"Inspect the public guide","success_criteria":"Guide observed"},
           {"specialist":"form","objective":"Submit feedback","success_criteria":"Confirmed submitted feedback"}],
           "semantic":{"concepts":[{"kind":"objective","span":"Submit an unrelated application","stages":[1]}],"relations":[]}}).to_string();
        assert!(parse_generated_plan(goal,&raw).is_err());
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
