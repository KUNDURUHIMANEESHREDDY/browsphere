//! Run-scoped semantic perception and progress tracking. No model decision
//! here authorizes an action; the fresh DOM binder and policy remain mandatory.
use super::{dom_extractor::{DomNodeSummary,DomSnapshot},policy::BrowserAction};
use std::{collections::{HashMap,HashSet,VecDeque},hash::{Hash,Hasher}};
use serde_json::{json,Value};

fn norm(s:&str)->String{s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()}
fn tokens(s:&str)->HashSet<String>{
    s.split(|c:char|!c.is_alphanumeric()).filter(|s|!s.is_empty())
        .map(str::to_lowercase).collect()
}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
enum Kind{Input,Choice,Calendar,Submit,Editor,Link,Button,Other}
fn kind(n:&DomNodeSummary)->Kind{
    let label=norm(&n.label);let words=tokens(&label);
    if n.role=="option" || n.role=="radio" || n.role=="checkbox" {Kind::Choice}
    else if n.search_state.as_deref()==Some("read_only") || words.contains("date") || words.contains("calendar"){Kind::Calendar}
    else if n.is_submit || matches!(label.as_str(),"search"|"find"|"continue"|"next"){Kind::Submit}
    else if n.tag=="textarea" || (n.role=="textbox"&&n.bbox.as_ref().is_some_and(|b|b.height>=72.0)){Kind::Editor}
    else if matches!(n.tag.as_str(),"input"|"select") || matches!(n.role.as_str(),"textbox"|"searchbox"|"combobox"){Kind::Input}
    else if n.tag=="a"||n.role=="link"{Kind::Link}
    else if n.role=="button"||n.tag=="button"{Kind::Button}
    else{Kind::Other}
}
/// Score by affordance and task vocabulary, not DOM order or a particular site.
pub fn rank(page:&DomSnapshot,goal:&str,limit:usize)->Vec<DomNodeSummary>{
    let g=tokens(goal);
    let form_intent=["find","search","fill","select","choose","filter","book","compare"].iter().any(|w|g.contains(*w));
    let writing=g.contains("write")||g.contains("draft");
    let mut scored:Vec<_>=page.interactive_nodes.iter().enumerate().map(|(i,n)|{
        let label=tokens(&n.label);
        let mut score=label.intersection(&g).filter(|w|w.len()>2&&!matches!(w.as_str(),"the"|"and"|"for"|"this"|"with"|"page")).count().min(5) as i32*7;
        let k=kind(n);
        // Prefer semantically matching links for research, even if a
        // generic search input or navigation menu is present.
        if super::deliberation::read_only(goal) && matches!(k,Kind::Link) {
            let href=n.href.as_deref().and_then(|s|url::Url::parse(s).ok()).map(|u|u.path().replace(['-','_','/']," ")).unwrap_or_default();
            score+=relevance(goal,&format!("{} {href}",n.label)).min(6) as i32*20;
            if ["pricing","sign up","sign in","log in","careers","cookie","privacy policy","contact sales","subscribe"].iter().any(|v|norm(&n.label).contains(v))&&relevance(goal,&n.label)==0 {score-=55;}
        }
        score+=match k{
            Kind::Input if form_intent=>42,Kind::Choice=>38,Kind::Calendar if form_intent=>30,
            Kind::Submit if form_intent=>18,Kind::Editor if writing=>65,Kind::Input=>9,_=>0
        };
        if form_intent&&matches!(k,Kind::Input|Kind::Button|Kind::Other)&&
            ["from","to","origin","destination","departure","arrival","location","station","city","date","check-in","check-out","start","end"]
            .iter().any(|w|label.contains(*w)&&g.contains(*w)){score+=28;}
        if n.search_state.as_deref()==Some("empty"){score+=15;}
        if n.search_state.as_deref()==Some("blocked_submit"){score-=30;}
        if n.bbox.as_ref().is_some_and(|b|b.in_viewport){score+=8;}
        if n.is_sensitive{score-=70;}
        if label.is_empty(){score-=10;}
        (i,score,n)
    }).collect();
    scored.sort_by(|a,b|b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    scored.truncate(limit);
    // Keep the highest relevance controls first so compact model context cannot
    // lose them again. Link-only goals retain native reading order.
    if !form_intent&&!writing&&!super::deliberation::read_only(goal){scored.sort_by_key(|x|x.0);}
    scored.into_iter().map(|(_,_,n)|n.clone()).collect()
}
pub fn visual_matches(page:&DomSnapshot,hint:&str)->Vec<String>{
    let text=norm(hint);
    let mut labels=HashSet::new();
    page.interactive_nodes.iter().filter(|n|!n.is_sensitive&&n.bbox.as_ref().is_some_and(|b|b.in_viewport))
        .filter_map(|n|{
            let label=norm(&n.label);
            if label.chars().count()<4||label.len()>100||!text.contains(&label)||!labels.insert(label){return None;}
            Some(n.label.clone())
        }).take(6).collect()
}
/// Content terms from the original objective, not generic site chrome.
pub fn objective_words(goal:&str)->HashSet<String>{
    let s=goal.split_whitespace().filter(|w|!w.contains("://")).collect::<Vec<_>>().join(" ");
    tokens(&s).into_iter().filter(|w|w.len()>=3&&!["the","and","for","from","with","this","that","using","find","search","research","compare","report","open","read","explain","cite","source","sources","page","website","official","documentation","starting","locate","observed","evidence","each","both","their","correct","reference"].contains(&w.as_str())).collect()
}
pub fn relevance(goal:&str,text:&str)->usize{
    let wanted=objective_words(goal);
    let found=tokens(text);
    wanted.intersection(&found).count()
}
pub fn relevant_link(goal:&str,page:&DomSnapshot,target:&str)->bool{
    let Some(u)=url::Url::parse(target).ok()else{return false};
    page.interactive_nodes.iter().any(|n|!n.is_sensitive&&!n.is_submit&&
        n.href.as_ref().and_then(|h|url::Url::parse(h).ok()).as_ref()==Some(&u)&&
        relevance(goal,&format!("{} {}",n.label,u.path().replace(['-','_','/']," ")))>0)
}
/// Required sources constitute a run-local, observation-backed checklist.
#[derive(Default)]
pub struct ObjectiveLedger{required:Vec<url::Url>,observed:HashSet<String>,attempts:HashMap<String,u8>}
impl ObjectiveLedger{
    pub fn new(goal:&str)->Self{Self{required:super::deliberation::explicit_research_sources(goal),..Self::default()}}
    pub fn observe(&mut self,evidence:&[super::orchestrator::SourceEvidence]){
        self.observed=self.required.iter().filter(|u|super::deliberation::source_observed(u,evidence)).map(|u|u.to_string()).collect();
    }
    pub fn next(&self,origin:&str)->Option<String>{
        self.required.iter().map(|u|u.to_string()).find(|u|u!=origin&&!self.observed.contains(u)&&self.attempts.get(u).copied().unwrap_or(0)<2)
    }
    pub fn attempted(&mut self,url:&str){let count=self.attempts.entry(url.into()).or_default();*count=count.saturating_add(1);}
    pub fn permitted_destination(&self,target:&str)->bool{
        url::Url::parse(target).ok().is_some_and(|u|self.required.contains(&u))
    }
    pub fn counts(&self)->(usize,usize){(self.observed.len(),self.required.len())}
    pub fn active(&self)->bool{!self.required.is_empty()}
    pub fn complete(&self)->bool{self.active()&&self.observed.len()==self.required.len()}
    pub fn hint(&self)->String{format!("Explicit source objectives: {}/{} observed; unfinished source objectives: {}. Do not substitute unrelated pages for missing source evidence.",self.observed.len(),self.required.len(),self.required.len().saturating_sub(self.observed.len()))}
}
fn identity(n:&DomNodeSummary)->String{
    format!("{:?}|{}|{}|{}|{}",kind(n),n.tag,n.role,norm(&n.label),n.href.as_deref().unwrap_or(""))
}
fn state(page:&DomSnapshot,stage:usize)->u64{
    let mut h=std::collections::hash_map::DefaultHasher::new();
    stage.hash(&mut h);page.origin.hash(&mut h);page.title.hash(&mut h);
    page.text_excerpt.starts_with("Active dialog:").hash(&mut h);
    // Include bounded public page content in the state, so a same-URL SPA
    // result change does not look identical to its previous form. No content
    // or typed value is persisted: only a run-local digest is retained.
    norm(&page.text_excerpt.chars().take(1800).collect::<String>()).hash(&mut h);
    // Ignore DOM IDs and cosmetic ordering. Preserve meaningful field state.
    let mut controls:Vec<_>=page.interactive_nodes.iter().filter(|n|!n.is_sensitive&&
        (n.bbox.as_ref().map_or(true,|b|b.in_viewport)||matches!(kind(n),Kind::Input|Kind::Choice|Kind::Calendar)))
        .map(|n|format!("{}|{:?}|{:?}|{:?}",identity(n),n.search_state,n.checked,n.options)).collect();
    controls.sort();controls.dedup();controls.hash(&mut h);h.finish()
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Attempt { state:u64, kind:&'static str, target:String, key:Option<String> }
impl Attempt{
    fn from(action:&BrowserAction,page:&DomSnapshot,stage:usize)->Option<Self>{
        let (kind,id,key)=match action{
            BrowserAction::Click{node_id,..}=>("click",node_id,None),
            BrowserAction::PressKey{node_id,key}=>("press_key",node_id,Some(key.clone())),
            BrowserAction::SelectOption{node_id,option_label}=>("select_option",node_id,Some(norm(option_label))),
            BrowserAction::SetChecked{node_id,checked,..}=>("set_checked",node_id,Some(checked.to_string())),
            // Text retries may correct a value; existing exact-action guard
            // handles duplicate text without persisting user input.
            _=>return None
        };
        let n=page.interactive_nodes.iter().find(|n|n.node_id==*id)?;
        Some(Self{state:state(page,stage),kind,target:identity(n),key})
    }
}
/// Run-local exploration budget. Only observed document transitions are
/// recorded. Consecutive observations of the same URL are not new visits.
#[derive(Default)]
pub struct RouteLedger {
    stage:Option<usize>,
    current:Option<u64>,
    visits:HashMap<u64,u8>,
    failed:HashSet<(u64,u64)>,
}
fn route_digest(raw:&str)->Option<u64>{
    let mut u=url::Url::parse(raw).ok()?;
    if !matches!(u.scheme(),"http"|"https")||u.host_str().is_none(){return None}
    u.set_fragment(None);
    let mut h=std::collections::hash_map::DefaultHasher::new();
    u.as_str().hash(&mut h);Some(h.finish())
}
impl RouteLedger{
    pub fn observe(&mut self,stage:usize,origin:&str){
        if self.stage!=Some(stage){
            self.stage=Some(stage);self.current=None;self.visits.clear();self.failed.clear();
        }
        if let Some(id)=route_digest(origin){
            if self.current!=Some(id){
                self.current=Some(id);
                let count=self.visits.entry(id).or_default();*count=count.saturating_add(1);
            }
        }
    }
    pub fn allowed(&self,target:&str)->bool{
        let Some(id)=route_digest(target) else{return false};
        self.current!=Some(id)&&self.visits.get(&id).copied().unwrap_or(0)<2&&
            !self.current.is_some_and(|from|self.failed.contains(&(from,id)))
    }
    pub fn failed_transition(&mut self,from:&str,target:&str){
        if let (Some(a),Some(b))=(route_digest(from),route_digest(target)){
            if self.failed.len()<64{self.failed.insert((a,b));}
        }
    }
    pub fn filter(&self,candidates:Vec<String>)->Vec<String>{
        candidates.into_iter().filter(|url|self.allowed(url)).collect()
    }
}
#[derive(Default)]
pub struct RunCore{
    current:Option<u64>,revision:u32,stalls:u8,failed:VecDeque<Attempt>,
}
impl RunCore{
    pub fn observe(&mut self,page:&DomSnapshot,stage:usize)->bool{
        let now=state(page,stage);
        let changed=self.current!=Some(now);
        if changed{self.current=Some(now);self.revision+=1;}
        changed
    }
    pub fn prepare(&self,action:&BrowserAction,page:&DomSnapshot,stage:usize)->Option<Attempt>{
        Attempt::from(action,page,stage)
    }
    pub fn record(&mut self,attempt:Option<Attempt>,progress:bool){
        if progress{self.stalls=0;return}
        self.stalls=self.stalls.saturating_add(1);
        if let Some(a)=attempt{
            if !self.failed.contains(&a){self.failed.push_back(a);}
            if self.failed.len()>32{self.failed.pop_front();}
        }
    }
    pub fn stalled(&self)->bool{self.stalls>0}
    pub fn escalate(&self)->bool{self.stalls>=2}
    pub fn blocked(&self,page:&DomSnapshot,stage:usize)->Vec<Value>{
        let current=state(page,stage);
        self.failed.iter().filter(|a|a.state==current).flat_map(|a|
            page.interactive_nodes.iter().filter(move |n|identity(n)==a.target)
                .map(move |n|json!({"type":a.kind,"node_id":n.node_id,"key":a.key}))
        ).take(24).collect()
    }
    pub fn hint(&self,page:&DomSnapshot,stage:usize,stage_count:usize)->String{
        let options=page.interactive_nodes.iter().filter(|n|n.role=="option").count();
        let fields=page.interactive_nodes.iter().filter(|n|matches!(kind(n),Kind::Input|Kind::Calendar|Kind::Editor)&&!n.is_sensitive).count();
        let empty=page.interactive_nodes.iter().filter(|n|!n.is_sensitive&&n.search_state.as_deref()==Some("empty")).count();
        let filled=page.interactive_nodes.iter().filter(|n|!n.is_sensitive&&n.search_state.as_deref()==Some("filled")).count();
        let blocked=page.interactive_nodes.iter().filter(|n|n.search_state.as_deref()==Some("blocked_submit")).count();
        format!("Stage {}/{}; observation revision {}; consecutive unproductive attempts {}; {} editable/date controls ({} empty, {} filled); {} options; {} blocked submits. Filled text is not proof of an autocomplete selection. Re-evaluate remaining objective; resolve any visible option list first. Acknowledgement is not proof of completion.",stage+1,stage_count,self.revision,self.stalls,fields,empty,filled,options,blocked)
    }
}
#[cfg(test)]
mod tests{
 use super::*;
 use crate::agent::dom_extractor::BoundingBox;
 fn n(id:&str,tag:&str,role:&str,label:&str)->DomNodeSummary{
  DomNodeSummary{node_id:id.into(),tag:tag.into(),role:role.into(),label:label.into(),options:vec![],checked:None,search_state:None,href:None,is_submit:false,is_sensitive:false,bbox:Some(BoundingBox{x:1.,y:1.,width:200.,height:40.,in_viewport:true})}
 }
 fn page(nodes:Vec<DomNodeSummary>)->DomSnapshot{DomSnapshot{origin:"https://example.org/".into(),title:"Test".into(),text_excerpt:"A public search form".into(),interactive_nodes:nodes}}
 #[test]fn checklist_tracks_three_explicit_sources_and_bounded_recovery(){
  let g="Research and compare three libraries using https://one.example/docs https://two.example/docs https://three.example/docs . Cite each source.";
  let mut ledger=ObjectiveLedger::new(g);assert!(ledger.active());assert!(!ledger.complete());
  let evidence=|url:&str|super::super::orchestrator::SourceEvidence{url:url.into(),title:"Library guide".into(),excerpt:"The library documentation contains a detailed description of the feature and relevant limitations. ".repeat(2)};
  let mut rows=vec![evidence("https://one.example/docs")];ledger.observe(&rows);
  let next=ledger.next("https://one.example/docs").unwrap();assert_eq!(next,"https://two.example/docs");
  ledger.attempted(&next);ledger.attempted(&next);
  assert_eq!(ledger.next("https://one.example/docs").unwrap(),"https://three.example/docs");
  rows.push(evidence(&next));rows.push(evidence("https://three.example/docs"));ledger.observe(&rows);assert!(ledger.complete());
 }
 #[test]fn research_ranking_prefers_target_over_generic_site_chrome(){
  let mut unrelated=n("pricing","a","link","Pricing");unrelated.href=Some("https://example.org/pricing".into());
  let mut target=n("docs","a","link","SQL INSERT");target.href=Some("https://example.org/sql-insert.html".into());
  let p=page(vec![unrelated,target]);let g="Find the SQL INSERT command and report its syntax";
  assert_eq!(rank(&p,g,2)[0].node_id,"docs");
  assert!(relevant_link(g,&p,"https://example.org/sql-insert.html"));
  assert!(!relevant_link(g,&p,"https://example.org/pricing"));
 }
 #[test]fn route_ledger_bounds_revisits_and_failed_transitions_without_blocking_new_pages(){
  let mut ledger=RouteLedger::default();
  let a="https://example.org/search";let b="https://example.org/help";
  ledger.observe(0,a);ledger.observe(0,a);
  assert!(!ledger.allowed(a));assert!(ledger.allowed(b));
  ledger.observe(0,b);assert!(ledger.allowed(a));
  ledger.observe(0,a);ledger.observe(0,b);assert!(!ledger.allowed(a));
  ledger.failed_transition(b,"https://example.org/failed");
  assert!(!ledger.allowed("https://example.org/failed"));
  assert!(ledger.allowed("https://example.org/new-result"));
  ledger.observe(1,b);assert!(ledger.allowed(a));
  assert!(ledger.allowed("https://example.org/failed"));
 }
 #[test]fn relevant_fields_survive_noisy_toolbar_on_unfamiliar_sites(){
  for (goal,a,b) in [("Find trains from Bengaluru to Delhi","From","To"),("Search hotels from London for a date","Location","Departure date"),("Fill the event registration form","Name","Event")]{
    let mut nodes=(0..100).map(|i|n(&format!("a{i}"),"a","link","Company help information")).collect::<Vec<_>>();
    nodes.extend([n("f1","input","textbox",a),n("f2","input","combobox",b)]);
    let ranked=rank(&page(nodes),goal,24);
    assert!(ranked.iter().take(5).any(|x|x.node_id=="f1"));
    assert!(ranked.iter().take(5).any(|x|x.node_id=="f2"));
  }
 }
 #[test]fn same_url_content_transition_is_a_new_observation(){
  let mut p=page(vec![n("search","button","button","Search")]);
  let mut c=RunCore::default();assert!(c.observe(&p,0));
  let a=BrowserAction::Click{node_id:"search".into(),element_label:"Search".into(),is_submit:false};
  c.record(c.prepare(&a,&p,0),false);assert_eq!(c.blocked(&p,0).len(),1);
  p.text_excerpt="Results for the selected term: a public guide is now displayed.".into();
  assert!(c.observe(&p,0));assert!(c.blocked(&p,0).is_empty());
 }
 #[test]fn failed_select_value_does_not_blacklist_other_values(){
  let mut p=page(vec![n("old","select","combobox","Category")]);
  p.interactive_nodes[0].options=vec!["Books".into(),"Maps".into()];
  let mut c=RunCore::default();
  let a=BrowserAction::SelectOption{node_id:"old".into(),option_label:"Books".into()};
  c.record(c.prepare(&a,&p,0),false);
  p.interactive_nodes[0].node_id="new".into();
  assert_eq!(c.blocked(&p,0),vec![json!({"type":"select_option","node_id":"new","key":"books"})]);
 }
 #[test]fn failed_strategy_tracks_semantic_target_across_rerender_and_clears_on_progress(){
  let mut p=page(vec![n("old","button","button","Open chooser")]);
  let mut c=RunCore::default();assert!(c.observe(&p,0));
  let act=BrowserAction::PressKey{node_id:"old".into(),key:"Escape".into()};
  c.record(c.prepare(&act,&p,0),false);assert!(c.stalled());
  p.interactive_nodes[0].node_id="new".into();assert!(!c.observe(&p,0));
  assert_eq!(c.blocked(&p,0)[0]["node_id"],"new");
  c.record(None,false);assert!(c.escalate());
  p.interactive_nodes.push(n("opt","div","option","Option A"));assert!(c.observe(&p,0));
  assert!(c.blocked(&p,0).is_empty());c.record(None,true);assert!(!c.stalled());
  assert!(c.blocked(&p,1).is_empty());
 }
}
