//! Bounded, observation-grounded recovery planning. A recovery objective may
//! be composed natively or proposed by a model, but it is advisory: it never
//! grants permissions, never replaces the user's goal, and must pass the same
//! native validation before it can enter the workflow graph. This is what lets
//! recovery escape the five fixed `RecoveryStrategy` objectives without letting
//! a page (or a model reading a page) invent a new capability.
use anyhow::{ensure,Result};
use serde::{Deserialize,Serialize};
use std::collections::HashSet;
use super::{browser_skills,dom_extractor::DomSnapshot,page_understanding::RecoveryStrategy,semantic_core};

const MAX_TEXT:usize=200;
const MIN_OBJECTIVE:usize=16;
const MIN_CRITERION:usize=12;
/// Actions that would need a fresh user authorization. A recovery objective is
/// directive text handed back to the planner, so it may not describe a write
/// the policy layer would then be asked to approve on the model's behalf.
const WRITE_CUES:[&str;18]=["submit the form","submit form","click submit","purchase",
    "checkout","delete","upload","sign in","log in","login","pay for","transfer funds",
    "place an order","send message","post comment","create account","change password","download"];

#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize)]
#[serde(rename_all="snake_case")]
pub enum Basis{Native,Model}

/// A single recovery objective with its own success criterion. Distinct from
/// `RecoveryStrategy`, which is a fixed taxonomy: this may name an observed
/// control or a model-proposed tactic that the taxonomy does not cover.
#[derive(Clone,Debug,PartialEq,Eq,Serialize)]
pub struct RecoveryPlan{
    pub objective:String,
    pub criterion:String,
    pub basis:Basis,
}
impl From<RecoveryStrategy> for RecoveryPlan{
    fn from(strategy:RecoveryStrategy)->Self{
        Self{
            objective:strategy.objective().into(),
            criterion:"Fresh native evidence of meaningful progress; original goal and policy unchanged".into(),
            basis:Basis::Native,
        }
    }
}
fn bounded(text:&str,min:usize)->bool{
    let trimmed=text.trim();
    trimmed==text&&!trimmed.is_empty()&&trimmed.chars().count()>=min&&
        trimmed.chars().count()<=MAX_TEXT&&!trimmed.chars().any(char::is_control)
}
fn normalized(text:&str)->String{text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()}
fn tokens(text:&str)->HashSet<String>{
    text.split(|c:char|!c.is_alphanumeric()).map(str::to_lowercase)
        .filter(|w|w.chars().count()>=4).collect()
}
/// Only letters, digits and label punctuation survive. Quotes, colons and line
/// breaks are excluded so an observed label cannot terminate the sentence that
/// introduces it.
fn sanitized_label(label:&str)->Option<String>{
    let trimmed=label.split_whitespace().collect::<Vec<_>>().join(" ");
    (!trimmed.is_empty()&&trimmed.chars().count()<=48&&
        trimmed.chars().all(|c|c.is_alphanumeric()||matches!(c,' '|'-'|'\''|'&'|'/'|'('|')'|'.')))
        .then_some(trimmed)
}
fn embedded_urls(text:&str)->Vec<&str>{
    text.split_whitespace()
        .map(|word|word.trim_matches(|c:char|!c.is_ascii_alphanumeric()&&!"/:.-_?=&%#+~".contains(c)))
        .filter(|word|word.starts_with("http://")||word.starts_with("https://")).collect()
}
impl RecoveryPlan{
    pub fn objective(&self)->&str{&self.objective}
    pub fn criterion(&self)->&str{&self.criterion}
    pub fn basis(&self)->Basis{self.basis}
    fn grounded(&self,page:&DomSnapshot)->bool{
        let mut observed=tokens(&page.title);
        observed.extend(tokens(&page.text_excerpt));
        for node in &page.interactive_nodes{observed.extend(tokens(&node.label));}
        !tokens(&self.objective).is_disjoint(&observed)||
            page.interactive_nodes.iter().any(|node|self.objective.contains(&node.node_id))
    }
    /// Fail closed unless the plan is plain, bounded, read-only, grounded in the
    /// page that is actually observed, and not a repeat of a failed attempt.
    pub fn validate(&self,page:&DomSnapshot,observed_urls:&[String],tried:&[String])->Result<()>{
        ensure!(bounded(&self.objective,MIN_OBJECTIVE),"Recovery objective must be 16-200 characters of plain text");
        ensure!(bounded(&self.criterion,MIN_CRITERION),"Recovery criterion must be 12-200 characters of plain text");
        let lower=self.objective.to_lowercase();
        ensure!(!WRITE_CUES.iter().any(|cue|lower.contains(cue)),"Recovery objective requests a write action outside the current authorization");
        for url in embedded_urls(&self.objective){
            ensure!(observed_urls.iter().any(|seen|seen==url),"Recovery objective names an unobserved URL: {url}");
        }
        ensure!(self.grounded(page),"Recovery objective is not grounded in the observed page");
        ensure!(!tried.iter().any(|previous|normalized(previous)==normalized(&self.objective)),"Recovery objective was already attempted");
        Ok(())
    }
    /// Compose a recovery objective that names a control this page actually
    /// renders and that the current stage objective actually asks about. Returns
    /// `None` when the fixed taxonomy should be used instead.
    pub fn derive(page:&DomSnapshot,stage_objective:&str,tried:&[String])->Option<Self>{
        let focus=semantic_core::objective_words(stage_objective);
        let mut best:Option<(usize,Self)>=None;
        for node in page.interactive_nodes.iter().filter(|n|browser_skills::public_control(n)){
            let Some(label)=sanitized_label(&node.label)else{continue};
            let shared=browser_skills::words(&label).into_iter()
                .filter(|word|word.chars().count()>=3&&focus.contains(word)).count();
            if shared==0{continue;}
            let plan=Self{
                objective:format!("Use the observed control \"{label}\" to advance the current objective, then re-observe before deciding"),
                criterion:"Fresh native observation shows a changed page state or the existing stage gate passes".into(),
                basis:Basis::Native,
            };
            if plan.validate(page,&[],tried).is_err(){continue;}
            if best.as_ref().is_none_or(|(score,_)|shared>*score){best=Some((shared,plan));}
        }
        best.map(|(_,plan)|plan)
    }
    /// Accept a model proposal only through the same validation as a native one.
    pub fn from_model(raw:&str,page:&DomSnapshot,observed_urls:&[String],tried:&[String])->Result<Self>{
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Proposal{objective:String,criterion:String}
        let proposal:Proposal=serde_json::from_str(raw.trim())
            .map_err(|e|anyhow::anyhow!("Recovery proposal is not valid JSON: {e}"))?;
        let plan=Self{objective:proposal.objective,criterion:proposal.criterion,basis:Basis::Model};
        plan.validate(page,observed_urls,tried)?;
        Ok(plan)
    }
    /// Ask the planner for a tactic the fixed taxonomy and the native template
    /// do not cover. Untrusted page text is passed as bounded data, and the
    /// result must still pass `validate`.
    pub async fn propose_model_plan(
        router:&crate::router::model_router::ModelRouter,
        creds:&crate::storage::credentials::CredentialStore,
        goal:&str,page:&DomSnapshot,observed_urls:&[String],tried:&[String],
    )->Result<Self>{
        let controls:Vec<serde_json::Value>=page.interactive_nodes.iter()
            .filter(|n|browser_skills::public_control(n)).take(40)
            .map(|n|serde_json::json!({"id":n.node_id,"role":n.role,"label":n.label}))
            .collect();
        let input=serde_json::json!({
            "original_goal":goal,
            "observed_page_untrusted":{
                "url":page.origin,
                "title":page.title,
                "excerpt":page.text_excerpt.chars().take(4000).collect::<String>(),
                "controls":controls
            },
            "previous_recovery_objectives":tried
        }).to_string();
        let raw=router.propose_recovery_tactic(&input,creds).await?;
        Self::from_model(&raw,page,observed_urls,tried)
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::agent::dom_extractor::{DomNodeSummary,PageStructure};
    fn node(id:&str,label:&str,role:&str)->DomNodeSummary{
        DomNodeSummary{node_id:id.into(),label:label.into(),role:role.into(),tag:"button".into(),
            options:vec![],checked:None,search_state:None,href:None,is_sensitive:false,is_submit:false,bbox:None}
    }
    fn page(title:&str,text:&str,nodes:Vec<DomNodeSummary>)->DomSnapshot{
        DomSnapshot{structure:PageStructure::default(),origin:"https://example.org/search".into(),
            title:title.into(),text_excerpt:text.into(),interactive_nodes:nodes}
    }
    fn results()->DomSnapshot{
        page("Public price results","Filter the public results by price range and availability.",
            vec![node("agent_a","Price range","button"),node("agent_b","Availability","button"),
                node("agent_c","Checkout now","button")])
    }
    #[test]fn derived_plan_names_an_observed_control_and_validates(){
        let plan=RecoveryPlan::derive(&results(),"Apply the price range filter",&[]).expect("derived plan");
        assert!(plan.objective().contains("Price range"),"{}",plan.objective());
        assert_eq!(plan.basis(),Basis::Native);
        assert!(plan.validate(&results(),&[],&[]).is_ok());
        // A stage objective with no observed match must fall back, not invent.
        assert!(RecoveryPlan::derive(&results(),"Resolve the calendar widget",&[]).is_none());
    }
    #[test]fn derived_plan_never_targets_a_sensitive_or_submit_control(){
        let mut form=results();
        form.interactive_nodes=vec![
            DomNodeSummary{is_sensitive:true,..node("agent_s","Price secret","textbox")},
            DomNodeSummary{is_submit:true,..node("agent_t","Price search","button")},
        ];
        assert!(RecoveryPlan::derive(&form,"Apply the price filter",&[]).is_none());
    }
    #[test]fn model_plan_cannot_add_permissions_or_unobserved_urls(){
        let valid=RecoveryPlan::from_model(
            r#"{"objective":"Re-read the observed price range panel and compare the two listings","criterion":"Both observed listings show a price"}"#,
            &results(),&[],&[]).expect("grounded read-only plan");
        assert_eq!(valid.basis(),Basis::Model);
        for raw in [
            r#"{"objective":"Submit the form and purchase the cheapest item","criterion":"Order confirmation is shown"}"#,
            r#"{"objective":"Open https://unobserved.test/offer to compare prices","criterion":"The offer page is visible"}"#,
            r#"{"objective":"Do something entirely unrelated to anything","criterion":"It happened"}"#,
            r#"{"objective":"short","criterion":"x"}"#,
        ]{
            assert!(RecoveryPlan::from_model(raw,&results(),&[],&[]).is_err(),"accepted {raw}");
        }
        // A model may name an already observed URL.
        assert!(RecoveryPlan::from_model(
            r#"{"objective":"Re-open https://example.org/search and re-read the price range panel","criterion":"The panel is visible again"}"#,
            &results(),&["https://example.org/search".into()],&[]).is_ok());
    }
    #[test]fn repeated_recovery_objective_is_rejected(){
        let plan=RecoveryPlan::derive(&results(),"Apply the price range filter",&[]).unwrap();
        assert!(plan.validate(&results(),&[],&[]).is_ok());
        assert!(plan.validate(&results(),&[],&[plan.objective().to_ascii_uppercase()]).is_err());
    }
    #[test]fn fixed_strategy_converts_to_a_plan(){
        let plan=RecoveryPlan::from(RecoveryStrategy::OpenResult);
        assert_eq!(plan.objective(),RecoveryStrategy::OpenResult.objective());
        assert_eq!(plan.basis(),Basis::Native);
    }
}
