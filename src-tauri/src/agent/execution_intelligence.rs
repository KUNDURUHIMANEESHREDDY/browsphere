//! Bounded run-local objective composition and observation receipts.
//! This module never dispatches or grants permission. Every proposed action
//! still passes native binding, specialist policy and the existing approval gate.
use super::{browser_skills,dom_extractor::DomSnapshot,evidence_verifier::{self,VerificationReport},orchestrator::SourceEvidence,policy::BrowserAction};
use std::{collections::{HashSet,VecDeque},hash::{Hash,Hasher}};
#[derive(Clone,Copy,Debug,PartialEq,Eq,serde::Serialize)]
#[serde(rename_all="snake_case")]
pub enum Phase{AcquireControl,AwaitDestination,Verify,Blocked}
#[derive(Clone,Copy,Debug,PartialEq,Eq,serde::Serialize)]
#[serde(rename_all="snake_case")]
pub enum Strategy{Observe,ClickObservedLink,NavigateObservedLink,Verify,Stop}
#[derive(Clone,Debug)]
struct LinkReceipt{source:String,target:String,action:BrowserAction}
#[derive(Clone,Debug)]
pub struct PageState{digest:u64,controls:usize,readable:bool}
#[derive(Default)]
pub struct WorldModel{
    current:Option<PageState>,recent:VecDeque<PageState>,observations:u32,
    failed:HashSet<(u64,StrategyKey)>,completed:HashSet<String>,source_count:usize,
}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
enum StrategyKey{Click,Navigate}
fn digest(p:&DomSnapshot)->u64{
    let mut h=std::collections::hash_map::DefaultHasher::new();
    p.origin.hash(&mut h);p.title.hash(&mut h);
    p.text_excerpt.chars().take(2400).collect::<String>().hash(&mut h);
    for n in p.interactive_nodes.iter().filter(|n|!n.is_sensitive){
        n.role.hash(&mut h);n.label.hash(&mut h);n.href.hash(&mut h);n.search_state.hash(&mut h);
    }
    h.finish()
}
impl WorldModel{
    pub fn observe(&mut self,p:&DomSnapshot){
        self.observations=self.observations.saturating_add(1);
        let state=PageState{digest:digest(p),controls:p.interactive_nodes.iter().filter(|n|!n.is_sensitive).count(),readable:browser_skills::readable(p)};
        if self.current.as_ref().map_or(true,|s|s.digest!=state.digest){
            if let Some(old)=self.current.take(){self.recent.push_back(old);}
            if self.recent.len()>16{self.recent.pop_front();}
        }
        self.current=Some(state);
    }
    fn failed(&mut self,key:StrategyKey){if let Some(s)=self.current.as_ref(){if self.failed.len()<48{self.failed.insert((s.digest,key));}}}
    fn excluded(&self,key:StrategyKey)->bool{self.current.as_ref().is_some_and(|s|self.failed.contains(&(s.digest,key)))}
    pub fn mark(&mut self,id:impl Into<String>){self.completed.insert(id.into());}
    pub fn sources(&mut self,evidence:&[SourceEvidence]){
        self.source_count=evidence.iter().map(|s|&s.url).collect::<HashSet<_>>().len();
    }
    pub fn status(&self)->serde_json::Value{serde_json::json!({"observations":self.observations,"distinctRecentPages":self.recent.len()+usize::from(self.current.is_some()),"controls":self.current.as_ref().map_or(0,|s|s.controls),"readable":self.current.as_ref().is_some_and(|s|s.readable),"failedStrategies":self.failed.len(),"completedObjectives":self.completed.len(),"observedSources":self.source_count})}
}
// An explicit single-link request is distinct from a named-link title task.
pub fn single_link_title(goal:&str)->bool{
    let g=goal.to_ascii_lowercase();
    super::deliberation::read_only(goal)&&g.contains("using the link on this page")&&
        g.trim().trim_end_matches(['.','!','?']).ends_with("report the title of the page you reach")&&
        !["compare","summarize","extract","download","submit","login","sign in","account","purchase","buy ","book "].iter().any(|w|g.contains(w))
}
fn single_link_candidate(p:&DomSnapshot)->Option<(BrowserAction,String)>{
    let source=browser_skills::public_url(&p.origin)?;
    let mut links=p.interactive_nodes.iter().filter(|n|(n.tag=="a"||n.role=="link")&&!n.is_sensitive&&!n.is_submit&&
        !n.label.trim().is_empty()&&n.bbox.as_ref().is_some_and(|b|b.in_viewport));
    let n=links.next()?;
    if links.next().is_some(){return None;}
    let target=browser_skills::public_url(n.href.as_deref()?)?;
    if target==source||target.fragment().is_some()||target.scheme()!="https"{return None;}
    Some((BrowserAction::Click{node_id:n.node_id.clone(),element_label:n.label.clone(),is_submit:false},target.to_string()))
}
/// A native receipt proves exact observed link target and title, not a
/// guessed relation between a human label and a URL slug (e.g. /guide).
fn verify_link(goal:&str,source:&str,target:&str,p:&DomSnapshot,evidence:&[SourceEvidence])->Result<VerificationReport,String>{
    let single=single_link_title(goal);
    let origin=browser_skills::public_url(source).ok_or("Missing starting public origin")?;
    let actual=browser_skills::public_url(&p.origin).ok_or("Missing observed destination")?;
    browser_skills::public_url(target).ok_or("Missing dispatched public link")?;
    let title_matches=if single{!p.title.trim().is_empty()}else{
        let label=evidence_verifier::requested_named_link_title(goal).ok_or("Not a named link title objective")?;
        origin.origin()==actual.origin()&&browser_skills::phrase(&p.title,&label)
    };
    if source==p.origin||
        browser_skills::navigation::receipt(target,p)!=browser_skills::Outcome::Confirmed||
        !title_matches||!browser_skills::readable(p){return Err("Observed destination or title does not match the dispatched link".into());}
    if !evidence.iter().any(|e|e.url==p.origin&&e.title==p.title&&e.excerpt.trim().chars().count()>=40){
        return Err("Missing recorded readable destination evidence".into());
    }
    Ok(VerificationReport{status:"evidence_checked",summary:format!("Opened a page titled {}.",p.title.trim()),findings:vec![evidence_verifier::GroundedFinding{claim:"The requested observed link destination was opened".into(),url:p.origin.clone(),quote:format!("Observed document title: {}",p.title.trim())}],limitations:"Verified the exact observed link destination and live document title, not the truth of document claims.".into()})
}
pub fn current_reference_subject(goal:&str)->Option<String>{
    if !super::deliberation::read_only(goal){return None;}
    let lower=goal.to_ascii_lowercase();
    let rest=lower.strip_prefix("read the current ")?;
    if !["report ","explain "].iter().any(|w|rest.contains(w))||
        !["cite ","supporting passage"].iter().any(|w|rest.contains(w))||
        [" and open "," and click "," then open "," navigate ","http://","https://","compare ","multiple ","other pages"].iter().any(|w|rest.contains(w)){return None;}
    let name=rest.strip_prefix("official ").unwrap_or(rest);
    let end=[" documentation"," reference"].iter().filter_map(|w|name.find(w)).min()?;
    let subject=name.get(..end)?.trim();
    if subject.chars().count()<3||subject.chars().count()>80||!subject.chars().all(|c|c.is_alphanumeric()||matches!(c,'.'|'_'|'-'|' ')){return None;}
    Some(subject.into())
}
pub fn current_reference_observed(goal:&str,p:&DomSnapshot,evidence:&[SourceEvidence])->bool{
    let Some(subject)=current_reference_subject(goal)else{return false;};
    browser_skills::readable(p)&&browser_skills::phrase(&p.title,&subject)&&evidence.iter().any(|e|e.url==p.origin&&e.title==p.title&&e.excerpt.trim().chars().count()>=40)
}
pub enum Next{Action(BrowserAction,&'static str),Wait,Stop(&'static str),Unmanaged}
pub struct ExecutionIntelligence{
    pub world:WorldModel,pub phase:Phase,pub strategy:Strategy,
    managed:bool,initial:String,receipt:Option<LinkReceipt>,pending:Option<LinkReceipt>,
    waiting:u8,attempts:u8,
}
impl ExecutionIntelligence{
    pub fn compile(goal:&str,initial:&str)->Self{
        let managed=(evidence_verifier::requested_named_link_title(goal).is_some()||single_link_title(goal))&&super::deliberation::read_only(goal)&&browser_skills::public_url(initial).is_some();
        Self{world:WorldModel::default(),phase:Phase::AcquireControl,strategy:Strategy::Observe,managed,initial:initial.into(),receipt:None,pending:None,waiting:0,attempts:0}
    }
    pub fn managed(&self)->bool{self.managed}
    pub fn visual_acquisition_due(&self,goal:&str,p:&DomSnapshot)->bool{
        self.managed&&self.phase==Phase::AcquireControl&&self.pending.is_none()&&self.receipt.is_none()&&self.waiting>=2&&p.origin==self.initial&&
        self.candidate(goal,p).is_none()&&super::multimodal_perception::independent_discovery_allowed(p)
    }
    pub fn restrict(&mut self){self.phase=Phase::Blocked;self.strategy=Strategy::Stop;self.pending=None;}
    /// Accept only the exact uniquely observed named public link. A model's
    /// label or URL is never accepted without this fresh DOM correspondence.
    fn candidate(&self,goal:&str,p:&DomSnapshot)->Option<(BrowserAction,String)>{
        if !self.managed||p.origin!=self.initial||!browser_skills::readable(p){return None;}
        if single_link_title(goal){single_link_candidate(p)}else{super::prerequisites::named_link(goal,p)}
    }
    pub fn dispatch_target(&self,goal:&str,p:&DomSnapshot,a:&BrowserAction)->Option<String>{
        let (expected,target)=self.candidate(goal,p)?;
        match (&expected,a){
            (BrowserAction::Click{node_id:x,..},BrowserAction::Click{node_id:y,is_submit:false,..}) if x==y=>Some(target),
            (_,BrowserAction::Navigate{url}) if url==&target=>Some(target),
            _=>None,
        }
    }
    pub fn dispatched(&mut self,source:&str,target:Option<String>,action:&BrowserAction,acknowledged:bool){
        if let Some(target)=target{
            self.attempts=self.attempts.saturating_add(1);
            if acknowledged{
                self.pending=Some(LinkReceipt{source:source.into(),target,action:action.clone()});
                self.phase=Phase::AwaitDestination;self.waiting=0;self.world.mark("control_dispatched");
            }else{
                self.world.failed(if matches!(action,BrowserAction::Navigate{..}){StrategyKey::Navigate}else{StrategyKey::Click});
            }
        }
    }
    /// Acknowledgement is not completion. A new matching readable capture is
    /// mandatory, even for an action that returned successfully.
    pub fn observe(&mut self,goal:&str,p:&DomSnapshot,evidence:&[SourceEvidence])->Option<BrowserAction>{
        self.world.observe(p);
        self.world.sources(evidence);
        if !self.managed{return None;}
        if self.receipt.is_some()&&self.report(goal,p,evidence).is_ok(){self.phase=Phase::Verify;return None;}
        if let Some(r)=self.pending.as_ref(){
            if r.source==self.initial&&browser_skills::navigation::receipt(&r.target,p)==browser_skills::Outcome::Confirmed&&
                verify_link(goal,&self.initial,&r.target,p,evidence).is_ok(){
                let r=self.pending.take().unwrap();
                let action=r.action.clone();self.receipt=Some(r);
                self.world.mark("destination_observed");self.phase=Phase::Verify;self.strategy=Strategy::Verify;
                return Some(action);
            }
        }
        None
    }
    pub fn report(&self,goal:&str,p:&DomSnapshot,evidence:&[SourceEvidence])->Result<VerificationReport,String>{
        let r=self.receipt.as_ref().ok_or("No confirmed named-link acquisition receipt")?;
        if r.source!=self.initial||browser_skills::navigation::receipt(&r.target,p)!=browser_skills::Outcome::Confirmed{return Err("Current page no longer matches the observed link".into());}
        verify_link(goal,&self.initial,&r.target,p,evidence)
    }
    pub fn next(&mut self,goal:&str,p:&DomSnapshot,evidence:&[SourceEvidence])->Next{
        if !self.managed{return Next::Unmanaged;}
        if self.phase==Phase::Blocked{return Next::Stop("The objective is blocked; do not retry restricted actions");}
        if self.report(goal,p,evidence).is_ok(){self.phase=Phase::Verify;self.strategy=Strategy::Verify;return Next::Action(BrowserAction::Complete{summary:"Observed the requested link destination and page title; verify the receipt.".into()},"objective:verified_destination");}
        if super::recovery::authentication_gate(p)||super::deliberation::error_page(p){
            self.phase=Phase::Blocked;self.strategy=Strategy::Stop;return Next::Stop("Requested public destination is unavailable or restricted");
        }
        if self.receipt.is_some(){self.phase=Phase::Blocked;self.strategy=Strategy::Stop;return Next::Stop("Previously verified destination is no longer observed");}
        if self.pending.is_some(){
            self.waiting=self.waiting.saturating_add(1);
            if self.waiting<=6{self.strategy=Strategy::Observe;return Next::Wait;}
            // A failed observation is not authorization to repeat a dispatched
            // action. Only a distinct navigation of the same freshly observed
            // public link can recover a click that did not change the page.
            let pending=self.pending.take().unwrap();
            if p.origin!=self.initial||matches!(pending.action,BrowserAction::Navigate{..}){
                self.phase=Phase::Blocked;self.strategy=Strategy::Stop;return Next::Stop("Dispatched link did not reach a verified destination");
            }
            self.world.failed(StrategyKey::Click);
            self.waiting=0;self.phase=Phase::AcquireControl;
        }
        if let Some((click,target))=self.candidate(goal,p){
            if self.attempts>=2{self.phase=Phase::Blocked;self.strategy=Strategy::Stop;return Next::Stop("Observed link recovery budget exhausted");}
            if !self.world.excluded(StrategyKey::Click){
                self.strategy=Strategy::ClickObservedLink;return Next::Action(click,"objective:observed_link");
            }
            if !self.world.excluded(StrategyKey::Navigate){
                self.strategy=Strategy::NavigateObservedLink;return Next::Action(BrowserAction::Navigate{url:target},"objective:alternate_observed_link");
            }
            self.phase=Phase::Blocked;self.strategy=Strategy::Stop;return Next::Stop("Both observed link strategies failed");
        }
        self.waiting=self.waiting.saturating_add(1);
        if self.waiting<=10{self.strategy=Strategy::Observe;Next::Wait}
        else{self.phase=Phase::Blocked;self.strategy=Strategy::Stop;Next::Stop("Requested link was not uniquely observed within the bounded observation window")}
    }
    pub fn hint(&self)->String{format!("Native objective phase {:?}; recovery strategy {:?}; completed evidence remains immutable until final review.",self.phase,self.strategy)}
}
#[cfg(test)]
mod tests{
 use super::*;
 use super::super::dom_extractor::DomNodeSummary;
 const GOAL:&str="On this document library page, click the Field Guide link. Then report its page title. Do not navigate to unrelated pages.";
 fn page(url:&str,title:&str,link:bool)->DomSnapshot{
    DomSnapshot{structure: Default::default(), origin:url.into(),title:title.into(),text_excerpt:format!("{title}. This readable public educational document describes the garden and the soil."),interactive_nodes:if link{vec![DomNodeSummary{node_id:"live".into(),tag:"a".into(),role:"link".into(),label:"Field Guide".into(),href:Some("https://example.org/field-guide".into()),options:vec![],checked:None,search_state:None,is_sensitive:false,is_submit:false,bbox:None}]}else{vec![]}}
 }
 fn evidence(p:&DomSnapshot)->Vec<SourceEvidence>{browser_skills::extraction::capture(p).into_iter().collect()}
 #[test]fn exact_observed_link_dispatch_and_fresh_receipt_advance_to_verification(){
  let start=page("https://example.org/start","Library",true);let target=page("https://example.org/field-guide","Field Guide",false);
  let mut r=ExecutionIntelligence::compile(GOAL,&start.origin);assert!(r.managed());
  r.observe(GOAL,&start,&evidence(&start));
  let Next::Action(a,_)=r.next(GOAL,&start,&[])else{panic!("expected live link")};
  let url=r.dispatch_target(GOAL,&start,&a).unwrap();
  r.dispatched(&start.origin,Some(url),&a,true);
  assert!(r.report(GOAL,&target,&evidence(&target)).is_err());
  assert!(r.observe(GOAL,&target,&[]).is_none());
  assert!(r.observe(GOAL,&target,&evidence(&target)).is_some());
  assert!(matches!(r.next(GOAL,&target,&evidence(&target)),Next::Action(BrowserAction::Complete{..},_)));
  assert!(r.report(GOAL,&target,&evidence(&target)).is_ok());
  assert!(r.report(GOAL,&start,&evidence(&start)).is_err());
 }
 #[test]fn exact_observed_url_does_not_need_to_repeat_link_label_in_slug(){
  let mut start=page("https://example.org/start","Library",true);
  start.interactive_nodes[0].href=Some("https://example.org/guide".into());
  let target=page("https://example.org/guide","Field Guide",false);
  let mut r=ExecutionIntelligence::compile(GOAL,&start.origin);
  r.observe(GOAL,&start,&[]);
  let Next::Action(a,_)=r.next(GOAL,&start,&[])else{panic!()};
  let url=r.dispatch_target(GOAL,&start,&a);
  r.dispatched(&start.origin,url,&a,true);
  assert!(r.observe(GOAL,&target,&evidence(&target)).is_some());
  assert!(r.report(GOAL,&target,&evidence(&target)).is_ok());
  let wrong=page("https://example.org/guide","Unrelated document",false);
  assert!(r.report(GOAL,&wrong,&evidence(&wrong)).is_err());
 }
 #[test]fn delayed_or_missing_control_is_observation_not_guesswork(){
  let p=page("https://example.org/start","Library",false);let mut r=ExecutionIntelligence::compile(GOAL,&p.origin);
  for _ in 0..10{r.observe(GOAL,&p,&[]);assert!(matches!(r.next(GOAL,&p,&[]),Next::Wait));}
  assert!(matches!(r.next(GOAL,&p,&[]),Next::Stop(_)));
 }
 #[test]fn failed_native_click_uses_distinct_grounded_alternative_without_replaying(){
  let p=page("https://example.org/start","Library",true);let mut r=ExecutionIntelligence::compile(GOAL,&p.origin);r.observe(GOAL,&p,&[]);
  let Next::Action(a,_)=r.next(GOAL,&p,&[])else{panic!()};
  let target=r.dispatch_target(GOAL,&p,&a);r.dispatched(&p.origin,target,&a,false);
  let Next::Action(a,_)=r.next(GOAL,&p,&[])else{panic!()};assert!(matches!(a,BrowserAction::Navigate{..}));
  let target=r.dispatch_target(GOAL,&p,&a);r.dispatched(&p.origin,target,&a,false);
  assert!(matches!(r.next(GOAL,&p,&[]),Next::Stop(_)));
 }
 #[test]fn observation_only_reference_contract_requires_exact_subject_and_source(){
  let goal="Read the current official Array.isArray documentation and report what the method determines. Cite the observed source evidence.";
  let p=page("https://example.org/array/isArray","Array.isArray() - Reference",false);
  assert_eq!(current_reference_subject(goal).as_deref(),Some("array.isarray"));
  assert!(current_reference_observed(goal,&p,&evidence(&p)));
  assert!(!current_reference_observed(goal,&p,&[]));
  let wrong=page("https://example.org/array/map","Array.map() - Reference",false);
  assert!(!current_reference_observed(goal,&wrong,&evidence(&wrong)));
  assert!(current_reference_subject("Read the current Array.isArray documentation and buy a subscription. Report results and cite evidence.").is_none());
 }
 #[test]fn explicit_single_link_requires_unique_visible_target_and_exact_receipt(){
  let goal="Find more information about public domains using the link on this page. Report the title of the page you reach.";
  let mut start=page("https://example.org/start","Library",true);
  start.interactive_nodes[0].href=Some("https://docs.example.net/reference".into());
  start.interactive_nodes[0].bbox=Some(super::super::dom_extractor::BoundingBox{x:10.,y:10.,width:100.,height:30.,in_viewport:true});
  let mut r=ExecutionIntelligence::compile(goal,&start.origin);assert!(r.managed());
  let target=page("https://www.docs.example.net/reference","Reference information",false);
  let Next::Action(a,_)=r.next(goal,&start,&[])else{panic!("expected unique visible link")};
  let url=r.dispatch_target(goal,&start,&a).unwrap();
  r.dispatched(&start.origin,Some(url),&a,true);
  let wrong=page("https://docs.example.net/other","Other information",false);
  assert!(r.observe(goal,&wrong,&evidence(&wrong)).is_none());
  assert!(r.observe(goal,&target,&evidence(&target)).is_some());
  assert!(r.report(goal,&target,&evidence(&target)).is_ok());
  let mut other=start.interactive_nodes[0].clone();other.node_id="other".into();
  start.interactive_nodes.push(other);
  assert!(single_link_candidate(&start).is_none());
  assert!(!ExecutionIntelligence::compile("Buy a subscription using the link on this page. Report the title of the page you reach.",&start.origin).managed());
 }
 #[test]fn named_link_on_multi_link_page_rejects_ambiguous_destinations(){
  let mut p=page("https://example.org/start","Library",true);
  let mut other=p.interactive_nodes[0].clone();
  other.node_id="other".into();other.label="Other reference".into();other.href=Some("https://example.org/other".into());
  p.interactive_nodes.push(other.clone());
  let mut r=ExecutionIntelligence::compile(GOAL,&p.origin);
  assert!(matches!(r.next(GOAL,&p,&[]),Next::Action(BrowserAction::Click{node_id,..},_) if node_id=="live"));
  other.label="Field Guide".into();p.interactive_nodes.push(other);
  let mut r=ExecutionIntelligence::compile(GOAL,&p.origin);
  assert!(matches!(r.next(GOAL,&p,&[]),Next::Wait));
 }
 #[test]fn policy_restriction_never_selects_an_alternate_action(){
  let p=page("https://example.org/start","Library",true);let mut r=ExecutionIntelligence::compile(GOAL,&p.origin);
  r.observe(GOAL,&p,&[]);r.restrict();
  assert!(matches!(r.next(GOAL,&p,&[]),Next::Stop(_)));
 }
 #[test]fn wrong_page_or_unobserved_url_cannot_produce_receipt(){
  let p=page("https://example.org/start","Library",true);let mut r=ExecutionIntelligence::compile(GOAL,&p.origin);
  assert!(r.dispatch_target(GOAL,&p,&BrowserAction::Navigate{url:"https://example.org/other".into()}).is_none());
  let a=BrowserAction::Navigate{url:"https://example.org/field-guide".into()};
  let target=r.dispatch_target(GOAL,&p,&a);r.dispatched(&p.origin,target,&a,true);
  let wrong=page("https://other.example/field-guide","Field Guide",false);
  assert!(r.observe(GOAL,&wrong,&evidence(&wrong)).is_none());assert!(r.report(GOAL,&wrong,&evidence(&wrong)).is_err());
  assert!(!ExecutionIntelligence::compile("Click the Field Guide link and buy a subscription. Then report the page title.",&p.origin).managed());
 }
}
