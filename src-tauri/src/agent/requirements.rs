//! Run-local requirement coverage. Candidate evidence is NOT a verified fact.
//! Compiles only supported explicit read-only contracts; does not invent
//! implicit requirements or grant additional browser permissions.
use super::{browser_skills,deliberation,dom_extractor::DomSnapshot,evidence_verifier as ev,orchestrator::SourceEvidence,skills::TaskSpec};
use serde::Serialize;
use std::collections::HashSet;
use url::Url;

#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize)]
#[serde(rename_all="snake_case")]
pub enum State{Missing,Observed,Candidate,Verified}
#[derive(Clone,Debug,Serialize)]
pub struct Requirement{
    pub id:String,pub kind:&'static str,pub state:State,pub candidates:usize,
    #[serde(skip)] scope:Scope,
}
#[derive(Clone,Debug)]
enum Scope{Source(String),Target,Fact{source:Option<String>,facet:&'static str}}
#[derive(Clone,Debug,Default,Serialize)]
pub struct Ledger{
    pub requirements:Vec<Requirement>,
    pub supported:bool,
    #[serde(skip)] target_scope:Option<String>,
}
fn add(rows:&mut Vec<Requirement>,kind:&'static str,scope:Scope){
    rows.push(Requirement{id:format!("r{}",rows.len()+1),kind,state:State::Missing,candidates:0,scope});
}
fn web(s:&str)->Option<Url>{browser_skills::public_url(s)}
fn source_matches(s:&SourceEvidence,wanted:&str)->bool{web(&s.url).is_some()&&web(&s.url)==web(wanted)}
fn page(s:&SourceEvidence)->DomSnapshot{DomSnapshot{structure:Default::default(),origin:s.url.clone(),title:s.title.clone(),text_excerpt:s.excerpt.clone(),interactive_nodes:vec![]}}
/// The exact root of a compiled target is anchored in native recorded evidence.
/// GitHub target identity must be visible, and the user explicitly named GitHub.
fn target_anchor(goal:&str,sources:&[SourceEvidence])->Option<String>{
    let spec=TaskSpec::compile(goal)?;
    sources.iter().find(|s|spec.observed_target(&page(s))&&
        spec.explicit_repository_url(&s.url).map_or(true,|root|web(&root)==web(&s.url)))
        .map(|s|s.url.clone())
}
/// Only same-origin exact repository descendants or the exact reference
/// may contribute facts. Cross-origin documentation needs a future explicit
/// provenance contract; do not silently accept arbitrary third-party sources.
pub fn target_source(goal:&str,anchor:&str,source:&str)->bool{
    let Some(spec)=TaskSpec::compile(goal)else{return false;};
    let (Some(a),Some(b))=(web(anchor),web(source))else{return false;};
    if a.origin()!=b.origin(){return false;}
    if a==b{return true;}
    spec.explicit_repository_url(anchor).is_some_and(|root|
        web(&root).is_some_and(|r|a==r&&b.path().starts_with(&format!("{}/",r.path().trim_end_matches('/')))&&
            b.query().is_none()&&b.fragment().is_none()&&spec.target_url(source)))
}
pub fn target_sources(goal:&str,sources:&[SourceEvidence])->Vec<SourceEvidence>{
    let Some(anchor)=target_anchor(goal,sources)else{return vec![];};
    sources.iter().filter(|s|target_source(goal,&anchor,&s.url)&&browser_skills::readable(&page(s))).cloned().collect()
}
impl Ledger{
    pub fn compile(goal:&str)->Self{
        let mut l=Self::default();
        if !deliberation::read_only(goal){return l;}
        let sources=deliberation::explicit_research_sources(goal);
        let facets=ev::research_facets(goal);
        if !sources.is_empty(){
            for u in sources{
                let url=u.to_string();
                add(&mut l.requirements,"source_observed",Scope::Source(url.clone()));
                for facet in &facets{add(&mut l.requirements,facet,Scope::Fact{source:Some(url.clone()),facet});}
            }
        }else if TaskSpec::compile(goal).is_some(){
            add(&mut l.requirements,"target_identity",Scope::Target);
            for facet in facets{add(&mut l.requirements,facet,Scope::Fact{source:None,facet});}
        }
        l.supported=!l.requirements.is_empty();l
    }
    pub fn refresh(&mut self,goal:&str,sources:&[SourceEvidence]){
        self.target_scope=target_anchor(goal,sources);
        for r in &mut self.requirements{
            r.candidates=0;r.state=State::Missing;
            match &r.scope{
                Scope::Target=>{if self.target_scope.is_some(){r.state=State::Observed;}},
                Scope::Source(url)=>{if sources.iter().any(|s|source_matches(s,url)&&browser_skills::readable(&page(s))){r.state=State::Observed;}},
                Scope::Fact{source,facet}=>{
                    let mut seen=HashSet::new();
                    for s in sources.iter().filter(|s|match source{
                        Some(u)=>source_matches(s,u),
                        None=>self.target_scope.as_ref().is_some_and(|a|target_source(goal,a,&s.url))
                    }){
                        if !browser_skills::readable(&page(s)){continue;}
                        for c in ev::facet_candidates(goal,s).into_iter().filter(|x|x.facet==*facet).flat_map(|x|x.candidates){
                            seen.insert((s.url.clone(),browser_skills::normalize(&c.text)));
                        }
                    }
                    r.candidates=seen.len();if r.candidates>0{r.state=State::Candidate;}
                }
            }
        }
    }
    pub fn ready(&self)->bool{!self.supported||self.requirements.iter().all(|r|r.state!=State::Missing)}
    pub fn missing(&self)->Vec<&'static str>{self.requirements.iter().filter(|r|r.state==State::Missing).map(|r|r.kind).collect()}
    pub fn hint(&self)->String{
        if !self.supported{return "No compiled fact contract; existing independent verification still applies.".into();}
        format!("Requirements: {} observed/candidate of {}; missing: {:?}. Candidate passages are not verified facts. Gather missing evidence before completing.",self.requirements.iter().filter(|r|r.state!=State::Missing).count(),self.requirements.len(),self.missing())
    }
    pub fn can_review(&self)->Result<(),String>{
        if self.ready(){Ok(())}else{Err(format!("Missing requirement evidence: {}",self.missing().join(", ")))}
    }
    /// Mark verified only after the independent report returns actual supported
    /// findings. Each factual requirement needs its own distinct quote.
    pub fn validate_report(&mut self,goal:&str,sources:&[SourceEvidence],report:&ev::VerificationReport)->Result<(),String>{
        self.refresh(goal,sources);self.can_review()?;
        if !self.supported{return Ok(());}
        if report.status!="evidence_checked"{return Err("Independent report not verified".into());}
        let facts:Vec<_>=self.requirements.iter().enumerate().filter_map(|(i,r)|if let Scope::Fact{source,facet}=&r.scope{Some((i,source,*facet))}else{None}).collect();
        let edges:Vec<Vec<usize>>=facts.iter().map(|(_,source,facet)|report.findings.iter().enumerate().filter_map(|(i,f)|{
            let scope_ok=match source{
                Some(u)=>web(u)==web(&f.url),
                None=>self.target_scope.as_ref().is_some_and(|a|target_source(goal,a,&f.url))
            };
            (scope_ok&&ev::facet_supported(goal,facet,&f.quote)&&sources.iter().any(|s|source_matches(s,&f.url)&&browser_skills::verification::quote_present(s,&f.quote))).then_some(i)
        }).collect()).collect();
        // Maximum bipartite matching, keyed by canonical source and quote.
        // This prevents reuse of one finding (including duplicate claims) for
        // multiple requirements, even when a passage contains multiple cues.
        let mut keys=Vec::<(String,String)>::new();
        let indices=report.findings.iter().map(|f|{
            let key=(web(&f.url).map(|u|u.to_string()).unwrap_or_default(),browser_skills::normalize(&f.quote));
            if let Some(i)=keys.iter().position(|k|k==&key){i}else{keys.push(key);keys.len()-1}
        }).collect::<Vec<_>>();
        let edges=edges.into_iter().map(|v|v.into_iter().map(|i|indices[i]).collect::<Vec<_>>()).collect::<Vec<_>>();
        fn assign(i:usize,edges:&[Vec<usize>],seen:&mut HashSet<usize>,owners:&mut[Option<usize>])->bool{
            for &j in &edges[i]{
                if !seen.insert(j){continue;}
                if owners[j].map_or(true,|old|assign(old,edges,seen,owners)){owners[j]=Some(i);return true;}
            }false
        }
        let mut owners=vec![None;keys.len()];
        for i in 0..facts.len(){
            if !assign(i,&edges,&mut HashSet::new(),&mut owners){
                return Err(format!("No distinct verified finding for requirement {}",self.requirements[facts[i].0].kind));
            }
        }
        for r in &mut self.requirements{r.state=State::Verified;}
        Ok(())
    }
}
/// Minimal bounded collection for a missing repository fact. Only a live link
/// to a documentation/metadata file inside the already observed repository.
pub fn repository_fact_link(goal:&str,page:&DomSnapshot,url:&str)->bool{
    let Some(spec)=TaskSpec::compile(goal)else{return false;};
    let Some(root)=spec.explicit_repository_url(&page.origin)else{return false;};
    if !spec.observed_target(page)||!target_source(goal,&root,url)||url==page.origin||url==root{return false;}
    let Some(u)=web(url)else{return false;};
    let path=u.path().to_ascii_lowercase();
    let name=path.rsplit('/').next().unwrap_or("");
    (path.contains("/blob/")||path.contains("/tree/"))&&
        (["readme","readme.md","readme.rst","pyproject.toml","setup.py","setup.cfg","installation.md","install.md","installation.rst","install.rst","docs","documentation"].contains(&name))&&
        page.interactive_nodes.iter().any(|n|browser_skills::public_control(n)&&n.href.as_deref()==Some(url))
}
#[cfg(test)]
mod tests{
 use super::*;
 fn s(url:&str,title:&str,excerpt:&str)->SourceEvidence{SourceEvidence{url:url.into(),title:title.into(),excerpt:excerpt.into()}}
 const REPO:&str="Find the official acme Widget repository on GitHub. Open its README or installation documentation. Report the installation command and the supported Python version.";
 #[test]fn repository_arrival_is_not_fact_completion(){
  let root=s("https://github.com/acme/widget","GitHub - acme/widget","The public widget repository contains an introduction to this library and its examples. Browse the repository.");
  let mut l=Ledger::compile(REPO);l.refresh(REPO,&[root.clone()]);
  assert_eq!(l.requirements[0].state,State::Observed);assert_eq!(l.missing(),vec!["installation_command","runtime_compatibility"]);assert!(!l.ready());
  let doc=s("https://github.com/acme/widget/blob/main/README.md","acme/widget README","Installation instructions: pip install widget. The library supports Python 3.10 or later.");
  l.refresh(REPO,&[root,doc]);assert!(l.ready());assert!(l.requirements.iter().all(|r|r.state!=State::Verified));
 }
 #[test]fn wrong_host_or_repository_cannot_supply_missing_facts(){
  let root=s("https://github.com/acme/widget","GitHub - acme/widget","The public widget repository contains an introduction to this library and its examples. Browse the repository.");
  let wrong=s("https://github.com/other/widget/blob/main/README.md","Other widget","Installation instructions: pip install widget. The library supports Python 3.10 or later.");
  let spoof=s("https://example.org/acme/widget","acme/widget","Installation instructions: pip install widget. The library supports Python 3.10 or later.");
  let mut l=Ledger::compile(REPO);l.refresh(REPO,&[root,wrong,spoof]);assert!(!l.ready());assert_eq!(l.missing().len(),2);
 }
 #[test]fn explicit_sources_each_have_independent_fact_requirements(){
  let g="Compare the official documentation https://one.example/guide https://two.example/guide . Explain one feature and one usage caveat for each.";
  let a=s("https://one.example/guide","One","The library supports a useful public interface for fetching records. Do not use this interface with private account information.");
  let b=s("https://two.example/guide","Two","The library provides an interface for displaying public records and returning structured information.");
  let mut l=Ledger::compile(g);l.refresh(g,&[a,b]);assert_eq!(l.requirements.len(),6);assert_eq!(l.missing(),vec!["caveat"]);
 }
 #[test]fn verified_report_requires_distinct_exact_scoped_quotes(){
  let g="Find the SQL MERGE command reference. Explain the difference between WHEN MATCHED and WHEN NOT MATCHED.";
  let a=s("https://example.org/docs/sql-merge.html","SQL MERGE","WHEN MATCHED performs an update to an existing matching row. WHEN NOT MATCHED inserts a new row if the condition permits insertion.");
  let mut l=Ledger::compile(g);l.refresh(g,&[a.clone()]);assert!(l.ready());
  let f=|q:&str|ev::GroundedFinding{url:a.url.clone(),claim:q.into(),quote:q.into()};
  let mut report=ev::VerificationReport{status:"evidence_checked",summary:String::new(),limitations:String::new(),findings:vec![f("WHEN MATCHED performs an update to an existing matching row.")]};
  assert!(l.validate_report(g,&[a.clone()],&report).is_err());
  report.findings.push(f("WHEN NOT MATCHED inserts a new row if the condition permits insertion."));
  assert!(l.validate_report(g,&[a],&report).is_ok());assert!(l.requirements.iter().all(|r|r.state==State::Verified));
 }
 #[test]fn unsupported_goals_keep_existing_verification(){
  assert!(!Ledger::compile("Open https://example.org").supported);
  assert!(!Ledger::compile("Buy a product and report its price").supported);
 }
}
