//! Fact-directed acquisition before synthesis for user-named FREE-tier comparisons.
//! A visit receipt is not enough: each requested named source must provide
//! exact captured free-tier passages and (when requested) a team-capacity fact.
//! Nothing here authorizes a browser action or verifies the truth of a claim.
use super::{deliberation,dom_extractor::DomSnapshot,
    evidence_verifier,orchestrator::SourceEvidence,policy::BrowserAction};
use std::collections::{HashMap,HashSet};
use url::Url;

fn primary_named_host(raw:&str,name:&str)->bool{
    let Some(parsed)=super::browser_skills::public_url(raw)else{return false};
    let Some(host)=parsed.host_str()else{return false};
    let bare=host.trim_start_matches("www.").to_ascii_lowercase();
    let labels=bare.split('.').collect::<Vec<_>>();
    labels.len()==2&&labels[0].replace('-',"")==name.to_ascii_lowercase().replace([' ','-'],"")
}
pub fn search_page(raw:&str)->bool{
    let Ok(url)=Url::parse(raw)else{return false};
    url.host_str().is_some_and(|host|matches!(host,"www.bing.com"|"bing.com"))
        &&url.path().starts_with("/search")
}
fn trusted_link(raw:&str)->Option<Url>{
    let parsed=super::browser_skills::public_url(raw)?;
    (parsed.scheme()=="https"&&parsed.port().is_none()
        &&parsed.fragment().is_none()
        &&parsed.username().is_empty()&&parsed.password().is_none())
        .then_some(parsed)
}
#[derive(Clone,Debug,serde::Serialize,PartialEq,Eq)]
pub struct MissingFact {
    pub source:String,
    pub observed:bool,
    pub needs_free_tier:bool,
    pub needs_team_capacity:bool,
    pub candidate_count:usize,
}
#[derive(Clone,Debug)]
pub struct Contract { names:Vec<String>,team:bool }
impl Contract {
    pub fn compile(goal:&str)->Option<Self>{
        if !evidence_verifier::requires_named_free_evidence(goal)
            || !deliberation::read_only(goal){return None}
        let names=deliberation::named_research_entities(goal);
        let lower=goal.to_ascii_lowercase();
        let team=["team","member","collaborat","user","seat"].iter()
            .any(|term|lower.contains(term));
        Some(Self{names,team})
    }
    pub fn missing(&self,goal:&str,sources:&[SourceEvidence])->Vec<MissingFact>{
        self.names.iter().filter_map(|name|{
            let state=evidence_verifier::named_source_fact_status(goal,sources,name);
            let needs_free_tier=!state.has_free_tier;
            let needs_team_capacity=self.team&&!state.has_team_capacity;
            (needs_free_tier||needs_team_capacity).then_some(MissingFact{
                source:name.clone(),observed:state.observed,
                needs_free_tier,needs_team_capacity,candidate_count:state.candidate_count,
            })
        }).collect()
    }
    pub fn missing_stage(&self,goal:&str,sources:&[SourceEvidence],index:usize)->Option<MissingFact>{
        let name=self.names.get(index)?;
        self.missing(goal,sources).into_iter().find(|missing|&missing.source==name)
    }
    pub fn ready(&self,goal:&str,sources:&[SourceEvidence])->bool{
        self.missing(goal,sources).is_empty()
    }
    pub fn stage_name(&self,index:usize)->Option<&str>{self.names.get(index).map(String::as_str)}
}
#[derive(Default)]
struct StageBudget { attempted:u8, links:HashSet<String>, searched:bool, settled:u8, unreadable_waits:u8 }
#[derive(Default)]
pub struct Research { by_stage:HashMap<usize,StageBudget> }
/// Call only on an active named-source research stage, using its actual observed page.
/// None means ordinary site acquisition may proceed; Stop is a bounded failed fact lookup.
pub enum Decision { Action(BrowserAction), Wait, Stop }
impl Research {
    pub fn decide(&mut self,goal:&str,contract:&Contract,index:usize,
        sources:&[SourceEvidence],page:&DomSnapshot)->Option<Decision>{
        let missing=contract.missing_stage(goal,sources,index)?;
        // The first step of source acquisition is finding the named website.
        // An official destination can be loading or unreadable; do not fall
        // through to ungrounded model actions just because capture is not ready.
        let name=contract.stage_name(index)?;
        if !missing.observed&&!search_page(&page.origin)
            &&!primary_named_host(&page.origin,name){return None;}
        let budget=self.by_stage.entry(index).or_default();
        if budget.attempted>=5{return Some(Decision::Stop)}
        let official=primary_named_host(&page.origin,name);
        let results=search_page(&page.origin);
        if !official&&!results {
            return Some(Decision::Stop); // never invent an unrelated navigation
        }
        let mut links=Vec::new();
        for node in &page.interactive_nodes {
            if node.is_sensitive||node.is_submit
                ||!(node.tag=="a"||node.role=="link"){continue}
            let Some(href)=node.href.as_deref().and_then(trusted_link)else{continue};
            let url=href.to_string();
            if !primary_named_host(&url,name)
                ||url==page.origin||budget.links.contains(&url){continue}
            if official && href.origin()!=Url::parse(&page.origin).ok()?.origin(){continue}
            let material=format!("{} {}",node.label.to_ascii_lowercase(),
                href.path().replace(['-','_','/']," ").to_ascii_lowercase());
            let risky=["login","log in","sign in","sign up","register",
                "buy","purchase","checkout","billing","contact sales","trial"];
            if risky.iter().any(|term|material.contains(term)){continue}
            let tier=["free","pricing","plans","plan","tier"];
            let seats=["member","collaborat","team","user","seat","limit","feature"];
            let score=tier.iter().filter(|s|material.contains(**s)).count()*5+
                seats.iter().filter(|s|material.contains(**s)).count()*3+
                usize::from(results&&!missing.observed);
            if score>0{links.push((score,url))}
        }
        links.sort_by(|a,b|b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        if let Some((_,url))=links.into_iter().next(){
            budget.links.insert(url.clone());budget.attempted+=1;
            return Some(Decision::Action(BrowserAction::Navigate{url}));
        }
        if official&&!missing.observed&&budget.unreadable_waits<3{
            budget.unreadable_waits+=1;
            return Some(Decision::Wait);
        }
        if official&&!budget.searched {
            // Only this user-named official site permits a known, read-only Bing
            // discovery waypoint. Never manufacture an official pricing URL.
            if let Some(url)=super::policy::named_pricing_recovery_waypoint(goal,index,&page.origin){
                budget.searched=true;budget.settled=0;budget.attempted+=1;
                return Some(Decision::Action(BrowserAction::Navigate{url:url.to_string()}));
            }
        }
        if results&&budget.settled<4{
            budget.settled+=1;return Some(Decision::Wait);
        }
        Some(Decision::Stop)
    }
}
#[cfg(test)]
mod tests{
 use super::*;
 use super::super::dom_extractor::DomNodeSummary;
 const G:&str="Visit the official websites of Alpha, Beta and Gamma. Compare their free plans for a four-member team and recommend the best.";
 fn evidence(url:&str,excerpt:&str)->SourceEvidence{
    SourceEvidence{url:url.into(),title:"Plans".into(),excerpt:excerpt.into()}
 }
 fn node(url:&str,label:&str)->DomNodeSummary{
    DomNodeSummary{node_id:url.into(),role:"link".into(),tag:"a".into(),label:label.into(),
        href:Some(url.into()),options:vec![],checked:None,search_state:None,
        is_sensitive:false,is_submit:false,bbox:None}
 }
 fn page(url:&str,links:Vec<DomNodeSummary>)->DomSnapshot{
    DomSnapshot{origin:url.into(),title:"Plans".into(),text_excerpt:
        "Plans from a public source with readable information and more than 80 characters of content.".into(),
        interactive_nodes:links,structure:Default::default()}
 }
 #[test]fn dependency_graph_identifies_unmet_facts_per_named_official_source(){
    let contract=Contract::compile(G).unwrap();
    let s=vec![evidence("https://alpha.com/","Alpha Free plan supports up to 10 members per team."),
        evidence("https://beta.org/","Beta Free plan has public project boards and tools."),
        evidence("https://gamma.net/","Gamma team pricing is available on the website.")];
    let missing=contract.missing(G,&s);
    assert_eq!(missing.len(),2);
    assert_eq!(missing[0].source,"Beta");
    assert!(!missing[0].needs_free_tier);
    assert!(missing[0].needs_team_capacity);
    assert_eq!(missing[1].source,"Gamma");
    assert!(missing[1].needs_free_tier);
    assert!(contract.missing_stage(G,&s,0).is_none());
    assert!(!contract.ready(G,&s));
 }
 #[test]fn avoids_search_result_evidence_and_impostor_domains(){
    assert!(Contract::compile("Buy the Alpha free plan").is_none());
    assert!(primary_named_host("https://alpha.com/pricing","Alpha"));
    assert!(!primary_named_host("https://alpha.com.evil.example/pricing","Alpha"));
    assert!(!primary_named_host("https://alpha.evil.example/pricing","Alpha"));
    let c=Contract::compile(G).unwrap();
    let s=vec![evidence("https://www.bing.com/search?q=beta","Beta Free plan supports 9 members."),
        evidence("https://beta.org/","Beta Free workspace tools are available.")];
    assert!(c.missing_stage(G,&s,1).unwrap().needs_team_capacity);
 }
 #[test]fn follows_observed_same_origin_plan_link_not_auth_or_cross_site(){
    let c=Contract::compile(G).unwrap();
    let sources=vec![evidence("https://beta.org/","Beta Free plan provides public project boards and tools.")];
    let current_page=page("https://beta.org/",vec![
        node("https://beta.org/pricing","Free plans"),
        node("https://beta.org/signin","Sign in"),
        node("https://beta.evil.example/pricing","Pricing"),
    ]);
    let mut research=Research::default();
    assert!(matches!(research.decide(G,&c,1,&sources,&current_page),
        Some(Decision::Action(BrowserAction::Navigate{url})) if url=="https://beta.org/pricing"));
    assert!(matches!(research.decide(G,&c,1,&sources,&current_page),
        Some(Decision::Action(BrowserAction::Navigate{url})) if url.starts_with("https://www.bing.com/search?")));
    let results=page("https://www.bing.com/search?q=beta",vec![
        node("https://beta.org/pricing/free","Beta free plan members"),
        node("https://thirdparty.org/plan","Other website"),
    ]);
    assert!(matches!(research.decide(G,&c,1,&sources,&results),
        Some(Decision::Action(BrowserAction::Navigate{url})) if url=="https://beta.org/pricing/free"));
 }
 #[test]fn unreadable_official_page_waits_before_ending_without_model_loops(){
    let c=Contract::compile(G).unwrap();
    let mut research=Research::default();
    let site=page("https://alpha.com/",vec![]);
    for _ in 0..3{
       assert!(matches!(research.decide(G,&c,0,&[],&site),Some(Decision::Wait)));
    }
    assert!(matches!(research.decide(G,&c,0,&[],&site),
       Some(Decision::Action(BrowserAction::Navigate{url})) if url.starts_with("https://www.bing.com/search?")));
    for _ in 0..4{
       assert!(matches!(research.decide(G,&c,0,&[],&site),Some(Decision::Stop)));
    }
 }
 #[test]fn first_official_source_comes_from_a_grounded_search_result(){
    let c=Contract::compile(G).unwrap();
    let mut r=Research::default();
    let results=page("https://www.bing.com/search?q=alpha",vec![
      node("https://alpha.com/free","Alpha free plan collaboration"),
      node("https://alpha.com.evil.example/pricing","Alpha pricing"),
      node("https://unrelated.net/free","Alpha alternatives"),
    ]);
    assert!(matches!(r.decide(G,&c,0,&[],&results),
       Some(Decision::Action(BrowserAction::Navigate{url})) if url=="https://alpha.com/free"));
    assert!(!c.ready(G,&[]));
    let only_home=page("https://www.bing.com/search?q=alpha",vec![
      node("https://alpha.com/","Alpha Official Website")
    ]);
    assert!(matches!(r.decide(G,&c,0,&[],&only_home),
       Some(Decision::Action(BrowserAction::Navigate{url})) if url=="https://alpha.com/"));
 }
 #[test]fn waits_bounded_times_for_missing_dynamic_search_links(){
    let c=Contract::compile(G).unwrap();
    let sources=vec![evidence("https://beta.org/","Beta Free plan provides project boards and tasks.")];
    let mut run=Research::default();
    let site=page("https://beta.org/",vec![]);
    assert!(matches!(run.decide(G,&c,1,&sources,&site),Some(Decision::Action(_))));
    let search=page("https://www.bing.com/search?q=beta",vec![]);
    for _ in 0..4{
        assert!(matches!(run.decide(G,&c,1,&sources,&search),Some(Decision::Wait)));
    }
    assert!(matches!(run.decide(G,&c,1,&sources,&search),Some(Decision::Stop)));
 }
}