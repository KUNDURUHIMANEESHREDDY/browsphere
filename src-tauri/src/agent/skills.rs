//! Run-scoped browser skills. The compiler accepts bounded public lookup
//! intent; the runtime retains phases and advances from native observations.
//! These proposals never grant navigation, input or submission permissions.
use super::{dom_extractor::{DomSnapshot,DomNodeSummary},policy::BrowserAction,semantic_core::RouteLedger};
use std::collections::HashSet;
fn words(s:&str)->Vec<String>{s.split(|c:char|!c.is_alphanumeric()).filter(|w|!w.is_empty()).map(str::to_lowercase).collect()}
fn has_phrase(h:&str,n:&str)->bool{
    let h=words(h);let n=words(n);
    !n.is_empty()&&h.windows(n.len()).any(|w|w==n.as_slice())
}
fn public_control(n:&DomNodeSummary)->bool{!n.is_sensitive&&!n.is_submit}
use super::browser_skills::{search::{input as search_input,opener as search_opener},Outcome};
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum TargetKind{Repository,Reference}
#[derive(Clone,Debug)]
pub struct TaskSpec{pub kind:TargetKind,pub subject:String,pub required_facts:Vec<&'static str>}
impl TaskSpec{
    pub fn compile(goal:&str)->Option<Self>{
        if !super::deliberation::read_only(goal){return None;}
        let lower=goal.to_ascii_lowercase();
        let (kind,start,end)=if lower.contains(" repository on "){
            (TargetKind::Repository,["find the official ","find the ","find "].as_slice()," repository on ")
        }else if lower.contains(" command reference"){
            (TargetKind::Reference,["find the ","locate the ","find "].as_slice()," command reference")
        }else{return None;};
        let a=start.iter().find_map(|p|lower.find(p).map(|i|i+p.len()))?;
        let b=a+lower[a..].find(end)?;
        let subject=goal.get(a..b)?.trim();
        if subject.chars().count()<3||subject.chars().count()>100||!subject.chars().all(|c|c.is_alphanumeric()||matches!(c,' '|'-'|'_'|'/'|'.')){return None;}
        let mut required_facts=Vec::new();
        if lower.contains("installation command"){required_facts.push("installation_command");}
        if lower.contains("supported python version")||lower.contains("supported runtime version"){required_facts.push("runtime_compatibility");}
        if lower.contains("difference between"){required_facts.push("explicit_contrast");}
        Some(Self{kind,subject:subject.into(),required_facts})
    }
    pub fn target_url(&self,url:&str)->bool{
        let Ok(u)=url::Url::parse(url)else{return false};
        let path=u.path().trim_matches('/');
        let needle=words(&self.subject);
        match self.kind{
            TargetKind::Repository=>{
                let segments=path.split('/').collect::<Vec<_>>();
                segments.len()>=2&&needle.len()>=2&&words(&segments[..2].join("/"))==needle
            }
            TargetKind::Reference=>{
                let leaf=path.rsplit('/').next().unwrap_or("").trim_end_matches(".html").trim_end_matches(".htm");
                words(leaf)==needle
            }
        }
    }
    pub fn observed_target(&self,page:&DomSnapshot)->bool{
        self.target_url(&page.origin)&&!super::deliberation::error_page(page)&&
            !super::recovery::authentication_gate(page)&&!page.text_excerpt.starts_with("Active dialog:")&&
            page.text_excerpt.chars().count()>=80
    }
    pub fn link_score(&self,n:&DomNodeSummary)->u32{
        if !public_control(n){return 0;}
        let Some(url)=n.href.as_deref()else{return 0};
        if self.target_url(url){return 120;}
        if has_phrase(&n.label,&self.subject){return 100;}
        if self.kind==TargetKind::Reference{
            let path=url::Url::parse(url).ok().map(|u|u.path().to_lowercase()).unwrap_or_default();
            if (path.contains("/doc")||path.contains("/reference"))&&
                ["current","current version","documentation","sql commands","command reference","reference","manual"].iter().any(|s|n.label.trim().eq_ignore_ascii_case(s)){return 60;}
        }
        0
    }
}
#[derive(Clone,Copy,Debug,PartialEq,Eq,serde::Serialize)]
#[serde(rename_all="snake_case")]
pub enum Phase{LocateTarget,OpenSearch,EnterQuery,SubmitQuery,InspectResults,ReadTarget,Blocked}
#[derive(Clone,Copy,Debug,PartialEq,Eq,serde::Serialize)]
#[serde(rename_all="snake_case")]
pub enum Diagnosis{None,MissingControl,AmbiguousControl,NoObservedEffect,SearchResultsMissing,TargetObserved}
pub struct SkillRun{
    pub spec:TaskSpec,pub phase:Phase,pub diagnosis:Diagnosis,
    attempted:HashSet<(String,String)>,typed:Option<(String,String)>,submitted:bool,
}
impl SkillRun{
    pub fn compile(goal:&str)->Option<Self>{Some(Self{spec:TaskSpec::compile(goal)?,phase:Phase::LocateTarget,diagnosis:Diagnosis::None,attempted:HashSet::new(),typed:None,submitted:false})}
    pub fn recovery_strategy(&self)->super::browser_skills::recovery::Strategy{
        use super::browser_skills::Outcome;
        let outcome=if self.phase==Phase::Blocked{Outcome::Restricted}else{match self.diagnosis{
            Diagnosis::AmbiguousControl=>Outcome::AmbiguousControl,
            Diagnosis::MissingControl|Diagnosis::SearchResultsMissing=>Outcome::MissingControl,
            Diagnosis::NoObservedEffect=>Outcome::NoEffect,
            Diagnosis::TargetObserved=>Outcome::Unverified,
            Diagnosis::None=>Outcome::AwaitingObservation,
        }};
        super::browser_skills::recovery::strategy(outcome,24usize.saturating_sub(self.attempted.len()) as u8)
    }
    pub fn hint(&self)->String{format!("Compiled public lookup: phase {:?}; diagnosis {:?}; recovery strategy {:?}; target must match the requested subject before completion. Required factual checks: {:?}",self.phase,self.diagnosis,self.recovery_strategy(),self.spec.required_facts)}
    pub fn observe(&mut self,page:&DomSnapshot){
        if self.spec.observed_target(page){self.phase=Phase::ReadTarget;self.diagnosis=Diagnosis::TargetObserved;}
        else if self.phase==Phase::ReadTarget{self.phase=Phase::LocateTarget;self.diagnosis=Diagnosis::MissingControl;}
    }
    fn take(&mut self,page:&DomSnapshot,key:String,phase:Phase,action:BrowserAction)->Option<BrowserAction>{
        if self.attempted.len()>=24||!self.attempted.insert((page.origin.clone(),key)){self.diagnosis=Diagnosis::NoObservedEffect;return None;}
        self.phase=phase;self.diagnosis=Diagnosis::None;Some(action)
    }
    pub fn next(&mut self,page:&DomSnapshot,routes:&RouteLedger)->Option<BrowserAction>{
        self.observe(page);
        if self.phase==Phase::ReadTarget{return None;}
        if super::recovery::authentication_gate(page)||super::deliberation::error_page(page){self.phase=Phase::Blocked;self.diagnosis=Diagnosis::MissingControl;return None;}
        // Prefer an exact, observed destination, never a synthesized URL.
        let links=super::browser_skills::navigation::ranked(page,|n|self.spec.link_score(n),|u|routes.allowed(u));
        for (score,url) in &links{
            if *score>=100{
                if let Some(a)=self.take(page,format!("target:{url}"),Phase::LocateTarget,BrowserAction::Navigate{url:url.clone()}){return Some(a);}
            }
        }
        if self.spec.kind==TargetKind::Reference{
            for (score,url) in &links{
                if *score>=60{
                    if let Some(a)=self.take(page,format!("waypoint:{url}"),Phase::LocateTarget,BrowserAction::Navigate{url:url.clone()}){return Some(a);}
                }
            }
        }
        let fields=super::browser_skills::search::fields(page);
        if fields.len()==1{
            let n=fields[0];
            if self.typed.as_ref().is_some_and(|(origin,label)|origin==&page.origin&&label==&n.label)&&
                n.search_state.as_deref()==Some("filled")&&!self.submitted{
                if let Some(a)=self.take(page,format!("submit:{}",n.label),Phase::SubmitQuery,BrowserAction::PressKey{node_id:n.node_id.clone(),key:"Enter".into()}){return Some(a);}
            }else if n.search_state.as_deref()==Some("empty")&&!self.submitted{
                if let Some(a)=self.take(page,format!("type:{}",n.label),Phase::EnterQuery,BrowserAction::TypeText{node_id:n.node_id.clone(),text:self.spec.subject.clone(),is_sensitive:false}){return Some(a);}
            }
        }else if fields.len()>1{
            self.diagnosis=Diagnosis::AmbiguousControl;return None;
        }
        if fields.is_empty()&&!self.submitted{
            let openers=page.interactive_nodes.iter().filter(|n|search_opener(n)).collect::<Vec<_>>();
            if openers.len()==1{
                let n=openers[0];
                if let Some(a)=self.take(page,format!("open:{}",n.label),Phase::OpenSearch,BrowserAction::Click{node_id:n.node_id.clone(),element_label:n.label.clone(),is_submit:false}){return Some(a);}
            }else if openers.len()>1{self.diagnosis=Diagnosis::AmbiguousControl;}
        }
        for (_,url) in links{
            if let Some(a)=self.take(page,format!("waypoint:{url}"),Phase::LocateTarget,BrowserAction::Navigate{url}){return Some(a);}
        }
        self.diagnosis=if self.submitted{Diagnosis::SearchResultsMissing}else{Diagnosis::MissingControl};
        None
    }
    pub fn receipt(&mut self,before:&DomSnapshot,after:&DomSnapshot,action:&BrowserAction){
        self.observe(after);
        if self.phase==Phase::ReadTarget{return;}
        match action{
            BrowserAction::Click{node_id,..} if before.interactive_nodes.iter().any(|n|n.node_id==*node_id&&search_opener(n))=>{
                match super::browser_skills::search::opened(after){
                    Outcome::Confirmed=>{self.phase=Phase::EnterQuery;self.diagnosis=Diagnosis::None;}
                    Outcome::AmbiguousControl=>self.diagnosis=Diagnosis::AmbiguousControl,
                    _=>self.diagnosis=Diagnosis::NoObservedEffect,
                }
            }
            BrowserAction::TypeText{node_id,text,is_sensitive:false} if text==&self.spec.subject=>{
                if let Some(n)=before.interactive_nodes.iter().find(|n|n.node_id==*node_id&&search_input(n)){
                    if super::browser_skills::search::entered(before,after,node_id)==Outcome::Confirmed{
                        self.typed=Some((after.origin.clone(),n.label.clone()));self.phase=Phase::SubmitQuery;self.diagnosis=Diagnosis::None;
                    }else{self.diagnosis=Diagnosis::NoObservedEffect;}
                }
            }
            BrowserAction::PressKey{node_id,key} if key=="Enter"&&self.typed.is_some()&&before.interactive_nodes.iter().any(|n|n.node_id==*node_id&&search_input(n))=>{
                // Dispatch is not a result receipt. Never blindly resubmit.
                self.submitted=true;self.phase=Phase::InspectResults;
                self.diagnosis=match super::browser_skills::search::results_observed(before,after,|n|self.spec.link_score(n)>=100){
                    Outcome::Confirmed|Outcome::AwaitingObservation=>Diagnosis::None,
                    _=>Diagnosis::SearchResultsMissing,
                };
            }
            _=>{}
        }
    }
    pub fn check(&self,page:&DomSnapshot,action:&BrowserAction)->Result<(),&'static str>{
        if self.spec.observed_target(page){return Ok(());}
        match action{
            BrowserAction::Complete{..}=>Err("Requested target has not been observed"),
            BrowserAction::Navigate{url}=>{
                if page.interactive_nodes.iter().any(|n|n.href.as_deref()==Some(url)&&self.spec.link_score(n)>0){Ok(())}
                else{Err("Navigation does not serve the compiled target acquisition subgoal")}
            }
            BrowserAction::Click{node_id,..}=>{
                if page.interactive_nodes.iter().any(|n|n.node_id==*node_id&&(search_opener(n)||search_input(n)||self.spec.link_score(n)>0)){Ok(())}
                else{Err("Click does not serve the current public search skill")}
            }
            BrowserAction::TypeText{node_id,text,is_sensitive:false} if text==&self.spec.subject&&page.interactive_nodes.iter().any(|n|n.node_id==*node_id&&search_input(n))=>Ok(()),
            BrowserAction::PressKey{node_id,key} if key=="Enter"&&self.typed.is_some()&&!self.submitted&&page.interactive_nodes.iter().any(|n|n.node_id==*node_id&&search_input(n)&&n.search_state.as_deref()==Some("filled"))=>Ok(()),
            BrowserAction::Scroll{..}=>Ok(()),
            _=>Err("Action is outside the compiled public search skill")
        }
    }
}
#[cfg(test)]
mod tests{
 use super::*;
 fn node(id:&str,tag:&str,role:&str,label:&str,state:Option<&str>,href:Option<&str>)->DomNodeSummary{
  DomNodeSummary{node_id:id.into(),tag:tag.into(),role:role.into(),label:label.into(),search_state:state.map(str::to_owned),href:href.map(str::to_owned),options:vec![],checked:None,is_sensitive:false,is_submit:false,bbox:None}
 }
 fn page(url:&str,nodes:Vec<DomNodeSummary>)->DomSnapshot{DomSnapshot{origin:url.into(),title:"Public lookup".into(),text_excerpt:"Public page with enough observed text to support an independent acquisition receipt.".repeat(2),interactive_nodes:nodes}}
 const GOAL:&str="Find the official acme Widget repository on GitHub. Report the installation command and supported Python version.";
 #[test]fn public_search_skill_retains_query_and_rejects_wrong_destination(){
  let mut skill=SkillRun::compile(GOAL).unwrap();let mut routes=RouteLedger::default();
  let p=page("https://example.org/",vec![node("s","button","button","Search or jump to, type / to search",None,None),node("x","a","link","Other repository",None,Some("https://example.org/features"))]);routes.observe(0,&p.origin);
  assert!(skill.check(&p,&BrowserAction::Complete{summary:"done".into()}).is_err());
  assert!(skill.check(&p,&BrowserAction::Navigate{url:"https://example.org/features".into()}).is_err());
  let open=skill.next(&p,&routes).unwrap();assert!(matches!(&open,BrowserAction::Click{node_id,..} if node_id=="s"));
  let search=page(&p.origin,vec![node("q","input","combobox","Search repositories",Some("empty"),None)]);
  skill.receipt(&p,&search,&open);assert_eq!(skill.phase,Phase::EnterQuery);
  let typed=skill.next(&search,&routes).unwrap();assert!(matches!(&typed,BrowserAction::TypeText{text,..} if text=="acme Widget"));
  let filled=page(&p.origin,vec![node("q2","input","combobox","Search repositories",Some("filled"),None)]);
  skill.receipt(&search,&filled,&typed);assert_eq!(skill.phase,Phase::SubmitQuery);
  let submit=skill.next(&filled,&routes).unwrap();assert!(matches!(&submit,BrowserAction::PressKey{key,..} if key=="Enter"));
  let results=page("https://example.org/search",vec![node("r","a","link","acme / Widget",None,Some("https://example.org/acme/widget"))]);
  skill.receipt(&filled,&results,&submit);assert_eq!(skill.phase,Phase::InspectResults);
  let result=skill.next(&results,&routes).unwrap();assert!(matches!(&result,BrowserAction::Navigate{url} if url=="https://example.org/acme/widget"));
  let target=page("https://example.org/acme/widget",vec![]);skill.observe(&target);assert_eq!(skill.phase,Phase::ReadTarget);
  assert!(skill.check(&target,&BrowserAction::Complete{summary:"request independent factual review".into()}).is_ok());
 }
 #[test]fn target_observation_rejects_search_results_and_unrelated_pages(){
  let spec=TaskSpec::compile(GOAL).unwrap();assert!(!spec.target_url("https://example.org/search?q=acme/widget"));assert!(!spec.target_url("https://example.org/other/widget"));
  assert!(spec.target_url("https://example.org/acme/widget"));assert_eq!(spec.required_facts.len(),2);
 }
}
