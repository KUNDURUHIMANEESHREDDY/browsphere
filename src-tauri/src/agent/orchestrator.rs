//! Lucia's specialist orchestrator: real staged execution on the existing WebView2
//! ReAct runtime. Each specialist has a bounded mission and restricted actions.
//! A stage advances only after live-page evidence passes the completion gate.
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use crate::agent::dom_extractor::DomSnapshot;
use crate::agent::policy::{requested_navigation_url, BrowserAction};
use crate::agent::evidence_verifier::check_navigation_result;

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
            Self::Research => "RESEARCH SPECIALIST: Investigate the requested subject by reading real webpages. Navigate to relevant sources, inspect content and links, and avoid assumptions. Gather evidence before completing your stage. You may navigate, click non-submitting controls, scroll, and type search queries into identified searchboxes only. Never submit forms or fill unrelated fields. Do not invent sources.",
            Self::Browser => "BROWSER SPECIALIST: Execute the user's requested navigation and website interactions one browser action at a time. Re-observe after each step. Never assume a click or navigation succeeded without acknowledgement. Request approval for actions requiring it.",
            Self::Comparison => "COMPARISON SPECIALIST: Compare only attributes grounded in observed website text and earlier collected evidence. If necessary browse to obtain missing details. Distinguish observed facts from unknowns. Use at least two distinct sources for a multi-source comparison.",
            Self::Form => "FORM SPECIALIST: Complete the user's explicitly requested workflow autonomously. Fill only fields required by the task; never invent personal information or credentials. Submit routine low-risk forms when ready, then re-observe and verify the real confirmation. The action policy automatically allows ordinary forms but still requires user approval for consequential submissions and sensitive data; never bypass it.",
            Self::Data => "DATA SPECIALIST: Extract structured information from current and relevant pages. Keep labels, units and source addresses. Navigate or scroll to collect missing information, never manufacture table values.",
            Self::Verification => "VERIFICATION SPECIALIST (READ-ONLY): Inspect the current DOM snapshot, recorded action outcomes and source evidence to determine whether the user's goal is truly supported. You may ONLY return a complete action with a cautious, evidence-backed summary. If evidence is missing, clearly say what is unverified; do not claim success.",
        }
    }

    pub fn allows(self, action: &BrowserAction) -> bool {
        match self {
            Self::Verification => matches!(action, BrowserAction::Complete { .. }),
            Self::Research | Self::Comparison | Self::Data => match action {
                BrowserAction::TypeText { is_sensitive, .. } => !is_sensitive,
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
struct GeneratedStage {
    specialist: Specialist,
    objective: String,
    success_criteria: String,
}

#[derive(Deserialize)]
struct GeneratedPlan {
    stages: Vec<GeneratedStage>,
}

/// Validate the model's proposed work before it can run. The model cannot
/// introduce new tools or bypass the existing action policy.
pub fn parse_generated_plan(goal: &str, raw: &str) -> anyhow::Result<TaskPlan> {
    let cleaned = raw.trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
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

/// A deterministic minimum plan, built from the user's intent.
/// The selected LLM decides actual actions for each specialist at runtime.
pub fn plan_task(goal: &str) -> TaskPlan {
    let lower = goal.to_lowercase();
    let mut roles: Vec<Specialist> = if contains_any(
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
        if !snapshot.origin.starts_with("https://") && !snapshot.origin.starts_with("http://") {
            return;
        }
        if snapshot.text_excerpt.trim().len() < 40 {
            return;
        }
        self.fresh_observations += 1;
        let evidence = SourceEvidence {
            url: snapshot.origin.clone(),
            title: snapshot.title.chars().take(140).collect(),
            excerpt: snapshot.text_excerpt.chars().take(1000).collect(),
        };
        if let Some(old) = self.evidence.iter_mut().find(|e| e.url == evidence.url) {
            *old = evidence;
        } else {
            self.evidence.push(evidence);
            if self.evidence.len() > 8 { self.evidence.remove(0); }
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
            excerpt: snapshot.text_excerpt.chars().take(1000).collect(),
        };
        if let Some(existing) = self.evidence.iter_mut().find(|e| e.url == observed.url) {
            *existing = observed;
        } else {
            self.evidence.push(observed);
            if self.evidence.len() > 8 { self.evidence.remove(0); }
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
        if snapshot.text_excerpt.trim().len() < 40 {
            return Err("The current page has insufficient readable evidence".into());
        }
        let is_verifier = self.specialist() == Specialist::Verification;
        if is_verifier && self.completed_summaries.len() != self.current {
            return Err("Earlier specialists have not completed".into());
        }
        if self.successful_actions == 0 {
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

    pub fn begin(&mut self) {
        self.plan.stages[self.current].status = "active";
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot(url: &str, text: &str) -> DomSnapshot {
        DomSnapshot { origin: url.to_string(), title: "Demo".into(),
            text_excerpt: text.to_string(), interactive_nodes: vec![] }
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
            node_id: "query".into(), tag: "input".into(), role: "searchbox".into(),
            label: "Search this site".into(), is_submit: false, is_sensitive: false, bbox: None
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
            origin: "https://example.org/feedback".into(),
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
