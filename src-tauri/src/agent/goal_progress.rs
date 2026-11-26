//! Goal-level progress signals for the general browser orchestrator.
//! Only the native requirement ledger provides evidence coverage. This module
//! cannot authorize actions or independently declare task completion.
use super::requirements::{Ledger,State};
use std::collections::HashMap;

fn rank(state:State)->u16{
    match state {State::Missing=>0,State::Observed=>10,State::Candidate=>20,State::Verified=>30}
}

/// Track requirement *improvement*, not URL changes, model claims, or accepted
/// clicks. All values are run-local counters: no page text or personal data.
#[derive(Default,Debug)]
pub struct Tracker {
    stage:Option<usize>,
    highwater:HashMap<String,u16>,
    actions_without_gain:u8,
    recovery_plans:u8,
    supported:bool,
    missing:usize,
    covered:usize,
    gains:u32,
}
impl Tracker {
    /// Returns true only when fresh native evidence improves a compiled
    /// requirement beyond its previous best state. Initial coverage counts
    /// as baseline, not a fabricated action success.
    pub fn observe(&mut self,stage:usize,ledger:&Ledger)->bool{
        if self.stage!=Some(stage){
            self.stage=Some(stage);
            // Keep run-global evidence high-water across specialist stages.
            // Otherwise old site observations are falsely counted as new
            // evidence each time the stage index changes.
            self.actions_without_gain=0;
            self.recovery_plans=0;
        }
        self.supported=ledger.supported;
        self.missing=ledger.requirements.iter().filter(|r|r.state==State::Missing).count();
        self.covered=ledger.requirements.len().saturating_sub(self.missing);
        let mut gain=false;
        for requirement in &ledger.requirements {
            // Bound candidate count contributions; a flood of repeated page
            // text cannot indefinitely hide a plateau or certify anything.
            let value=rank(requirement.state)
                + if requirement.state==State::Candidate{
                    requirement.candidates.min(3) as u16
                }else{0};
            let previous=self.highwater.entry(requirement.id.clone()).or_insert(0);
            if value>*previous{
                if value>0 {gain=true;}
                *previous=value;
            }
        }
        if gain{
            self.actions_without_gain=0;
            self.recovery_plans=0;
            self.gains=self.gains.saturating_add(1);
        }
        gain
    }
    pub fn action_attempted(&mut self,non_terminal:bool){
        if non_terminal && self.supported && self.missing>0 {
            self.actions_without_gain=self.actions_without_gain.saturating_add(1);
        }
    }
    /// When compiled requirements are still missing after several browser
    /// attempts, one more DOM change is not sufficient evidence of progress.
    pub fn stalled(&self)->bool{
        self.supported&&self.missing>0
            &&self.actions_without_gain>=4&&self.recovery_plans<2
    }
    /// A named-source acquisition stage is complete when its OWN observed
    /// source exists. Missing later stages are tracked globally for final
    /// verification, but must not deadlock an already-evidenced current stage.
    pub fn stalled_for_stage(&self,ledger:&Ledger,stage:usize,
        named_research_stage:bool)->bool{
        if named_research_stage && ledger.requirements.get(stage).is_some_and(|r|
            r.kind=="named_source_observed"&&r.state!=State::Missing){
            return false;
        }
        self.stalled()
    }
    pub fn record_replan(&mut self){
        self.recovery_plans=self.recovery_plans.saturating_add(1);
    }
    pub fn hint(&self)->String{
        if !self.supported {
            return "No compiled requirement contract: use observed action effects and independent final verification.".into();
        }
        format!(
            "Goal evidence progress: {}/{} compiled requirements have observed candidates; {} actions without additional requirement coverage; {} evidence gains; {} bounded recovery plans. Page navigation and model agreement are NOT verified completion. If coverage is stalled, choose a DIFFERENT evidence acquisition strategy.",
            self.covered,self.covered+self.missing,self.actions_without_gain,self.gains,self.recovery_plans
        )
    }
    pub fn telemetry(&self)->serde_json::Value{
        serde_json::json!({"covered":self.covered,"missing":self.missing,
            "actions_without_gain":self.actions_without_gain,
            "evidence_gains":self.gains,"recovery_plans":self.recovery_plans,
            "plateau":self.stalled()})
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    use super::super::requirements::State as ReqState;
    fn ledger(states:&[(ReqState,usize)])->Ledger{
        let mut result=Ledger::compile("Visit the official websites of Alpha, Beta and Gamma. Compare their free plans for a team.");
        assert!(result.requirements.len()>=states.len());
        result.requirements.truncate(states.len());
        for (requirement,(state,count)) in result.requirements.iter_mut().zip(states){
            requirement.state=*state;requirement.candidates=*count;
        }
        result
    }
    #[test]
    fn a_page_change_is_not_evidence_progress(){
        let mut run=Tracker::default();let missing=ledger(&[(ReqState::Missing,0)]);
        assert!(!run.observe(0,&missing));
        for _ in 0..4 {run.action_attempted(true);assert!(!run.observe(0,&missing));}
        assert!(run.stalled());assert!(run.hint().contains("4 actions without"));
        run.record_replan();run.record_replan();assert!(!run.stalled());
    }
    #[test]
    fn fresh_candidate_resets_stall_but_duplicates_do_not(){
        let mut run=Tracker::default();
        let missing=ledger(&[(ReqState::Missing,0),(ReqState::Missing,0)]);
        run.observe(0,&missing);
        for _ in 0..4{run.action_attempted(true);}
        assert!(run.stalled());
        let candidate=ledger(&[(ReqState::Candidate,1),(ReqState::Missing,0)]);
        assert!(run.observe(0,&candidate));assert!(!run.stalled());
        for _ in 0..4 {run.action_attempted(true);assert!(!run.observe(0,&candidate));}
        assert!(run.stalled());
        assert!(run.observe(0,&ledger(&[(ReqState::Verified,1),(ReqState::Missing,0)])));
        assert!(!run.stalled());
    }
    #[test]
    fn future_named_sources_do_not_deadlock_current_completed_research_stage(){
        let mut run=Tracker::default();
        let partial=ledger(&[(ReqState::Observed,0),(ReqState::Missing,0),(ReqState::Missing,0)]);
        run.observe(0,&partial);
        for _ in 0..5{run.action_attempted(true);}
        assert!(run.stalled());
        assert!(!run.stalled_for_stage(&partial,0,true));
        assert!(run.stalled_for_stage(&partial,0,false));
        assert!(run.stalled_for_stage(&partial,1,true));
    }
    #[test]
    fn progressing_to_next_stage_does_not_recount_previous_site_evidence(){
        let mut tracker=Tracker::default();
        let alpha=ledger(&[(ReqState::Observed,0),(ReqState::Missing,0),(ReqState::Missing,0)]);
        assert!(tracker.observe(0,&alpha));
        assert!(!tracker.observe(1,&alpha));
        assert_eq!(tracker.gains,1);
        let beta=ledger(&[(ReqState::Observed,0),(ReqState::Observed,0),(ReqState::Missing,0)]);
        assert!(tracker.observe(1,&beta));
        assert_eq!(tracker.gains,2);
    }
    #[test]
    fn new_stage_resets_plateau_and_uncompiled_tasks_use_existing_action_recovery(){
        let mut run=Tracker::default();let missing=ledger(&[(ReqState::Missing,0)]);
        run.observe(0,&missing);
        for _ in 0..4{run.action_attempted(true);}
        assert!(run.stalled());
        assert!(!run.observe(1,&missing));assert!(!run.stalled());
        let unsupported=Ledger::default();
        run.observe(1,&unsupported);
        for _ in 0..6{run.action_attempted(true);}
        assert!(!run.stalled());
    }
}
