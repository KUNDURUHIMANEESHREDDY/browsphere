//! Run-scoped semantic perception and progress tracking. No model decision
//! here authorizes an action; the fresh DOM binder and policy remain mandatory.
use super::{dom_extractor::{DomNodeSummary,DomSnapshot},policy::BrowserAction};
use std::{collections::{HashSet,VecDeque},hash::{Hash,Hasher}};
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
    if !form_intent&&!writing{scored.sort_by_key(|x|x.0);}
    scored.into_iter().map(|(_,_,n)|n.clone()).collect()
}
fn identity(n:&DomNodeSummary)->String{
    format!("{:?}|{}|{}|{}|{}",kind(n),n.tag,n.role,norm(&n.label),n.href.as_deref().unwrap_or(""))
}
fn state(page:&DomSnapshot,stage:usize)->u64{
    let mut h=std::collections::hash_map::DefaultHasher::new();
    stage.hash(&mut h);page.origin.hash(&mut h);page.title.hash(&mut h);
    page.text_excerpt.starts_with("Active dialog:").hash(&mut h);
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
            // Text retries may correct a value; existing exact-action guard
            // handles duplicate text without persisting user input.
            _=>return None
        };
        let n=page.interactive_nodes.iter().find(|n|n.node_id==*id)?;
        Some(Self{state:state(page,stage),kind,target:identity(n),key})
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
        format!("Stage {}/{}; observation revision {}; consecutive unproductive attempts {}; {} editable or date controls; {} options. Re-evaluate remaining objective from this observation. If an option list is open, select the matching option before continuing; otherwise satisfy required fields before submitting. Acknowledgement is not proof of completion.",stage+1,stage_count,self.revision,self.stalls,fields,options)
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
 #[test]fn relevant_fields_survive_noisy_toolbar_on_unfamiliar_sites(){
  for (goal,a,b) in [("Find trains from Bengaluru to Delhi","From","To"),("Search hotels from London for a date","Location","Departure date"),("Fill the event registration form","Name","Event")]{
    let mut nodes=(0..100).map(|i|n(&format!("a{i}"),"a","link","Company help information")).collect::<Vec<_>>();
    nodes.extend([n("f1","input","textbox",a),n("f2","input","combobox",b)]);
    let ranked=rank(&page(nodes),goal,24);
    assert!(ranked.iter().take(5).any(|x|x.node_id=="f1"));
    assert!(ranked.iter().take(5).any(|x|x.node_id=="f2"));
  }
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
