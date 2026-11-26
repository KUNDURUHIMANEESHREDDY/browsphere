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
enum Scope{Source(String),Target,Fact{source:Option<String>,facet:&'static str},CoursePlatform(usize),OpenResearch}
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
// Explicit planning deliverables are tracked even before a source is known.
// Missing facts cannot be promoted merely because a browser visited a page.
/// Reject vague "open daily" claims: at least one explicit clock reading
/// must appear alongside the source's stated hours/timings.
fn has_clock_time(text:&str)->bool{
    let mut previous="";
    for token in text.split_whitespace(){
        let value=token.trim_matches(|c:char|!(c.is_ascii_alphanumeric()||c==':'));
        let clock=value.split_once(':').and_then(|(h,m)|{
            let h=h.parse::<u8>().ok()?;
            let m=m.chars().take_while(char::is_ascii_digit).collect::<String>()
                .parse::<u8>().ok()?;
            (h<=23&&m<=59).then_some(())
        }).is_some();
        let suffix=value.strip_suffix("am").or_else(||value.strip_suffix("pm"))
            .and_then(|h|h.parse::<u8>().ok())
            .is_some_and(|h|(1..=12).contains(&h));
        let paired=matches!(value,"am"|"pm")&&previous.parse::<u8>().ok()
            .is_some_and(|h|(1..=12).contains(&h));
        if clock||suffix||paired{return true;}
        previous=value;
    }
    false
}
/// An open-ended outing requires three separately observed public attraction
/// pages with actual hours and admission-price evidence. Search result indexes,
/// shopping pages and generic directories do not count as attractions.
fn verified_planning_candidates(sources:&[SourceEvidence])->Vec<&SourceEvidence>{
    let mut seen=HashSet::<String>::new();
    sources.iter().filter(|s|{
        let Some(url)=web(&s.url)else{return false;};
        let host=url.host_str().unwrap_or("").trim_start_matches("www.");
        if ["bing.com","google.com","duckduckgo.com","yahoo.com"]
            .iter().any(|search|host==*search){return false;}
        let title=s.title.to_lowercase();
        let text=s.excerpt.to_lowercase();
        let attraction=["park","garden","museum","zoo","palace","aquarium",
            "science centre","science center","planetarium","botanical",
            "attraction","gallery","fort"].iter()
            .any(|word|title.contains(word)||text.contains(word));
        let timing=has_clock_time(&text)
            && ["open","hours","timings","time","closing"]
                .iter().any(|word|text.contains(word));
        let pricing=(text.contains('₹')||text.contains("inr ")
                ||text.contains("rs.")||text.contains("rs "))
            && ["ticket","entry","admission","fee","price"]
                .iter().any(|word|text.contains(word));
        let identity=format!("{}:{}",host,title);
        attraction&&timing&&pricing&&seen.insert(identity)
            &&browser_skills::readable(&page(s))
    }).collect()
}
fn first_currency_amount(text:&str)->Option<u32>{
    let lower=text.to_ascii_lowercase();
    let starts=lower.match_indices("inr ").map(|(i,_)|i+4)
        .chain(lower.match_indices("rs.").map(|(i,_)|i+3))
        .chain(lower.match_indices("rs ").map(|(i,_)|i+3))
        .chain(text.match_indices('₹').map(|(i,_)|i+'₹'.len_utf8()));
    for start in starts{
        let tail=text.get(start..)?.trim_start_matches(|c:char|c.is_whitespace()||c==':'||c=='=');
        let digits=tail.chars().take_while(|c|c.is_ascii_digit()||*c==',').filter(|c|*c!=',').collect::<String>();
        if let Ok(amount)=digits.parse::<u32>(){return Some(amount);}
    }
    None
}
/// Currency amount in the explicit total clause, never an arbitrary entry fee.
fn stated_outing_total(summary:&str)->Option<u32>{
    let lower=summary.to_ascii_lowercase();
    let at=lower.find("total")?;
    first_currency_amount(summary.get(at+5..)?)
}
fn planning_requirements(goal:&str)->Vec<&'static str>{
    let g=goal.to_ascii_lowercase();
    if !deliberation::read_only(goal)
        || !super::policy::public_research_objective(goal).to_ascii_lowercase().starts_with("plan "){
        return vec![];
    }
    let mut out=vec!["research_subjects"];
    for (kind,terms) in [("opening_hours", &["opening hours","opening times"][..]),("price", &["ticket price","admission price","cost","budget"][..]),("budget_constraint", &["under ","budget","within "][..]),("itinerary", &["itinerary","schedule"][..])]{
        if terms.iter().any(|t|g.contains(t)){out.push(kind);}
    }
    out
}
impl Ledger{
    pub fn compile(goal:&str)->Self{
        let mut l=Self::default();
        if !deliberation::read_only(goal){return l;}
        let sources=deliberation::explicit_research_sources(goal);
        let facets=ev::research_facets(goal);
        if let Some(n)=super::course_comparison::count(goal){
            for i in 0..n { add(&mut l.requirements,"course_platform_certificate_duration",Scope::CoursePlatform(i)); }
        }else if !sources.is_empty(){
            for u in sources{
                let url=u.to_string();
                add(&mut l.requirements,"source_observed",Scope::Source(url.clone()));
                for facet in &facets{add(&mut l.requirements,facet,Scope::Fact{source:Some(url.clone()),facet});}
            }
        }else if TaskSpec::compile(goal).is_some(){
            add(&mut l.requirements,"target_identity",Scope::Target);
            for facet in facets{add(&mut l.requirements,facet,Scope::Fact{source:None,facet});}
        }
        if l.requirements.is_empty(){for kind in planning_requirements(goal){add(&mut l.requirements,kind,Scope::OpenResearch);}}
        l.supported=!l.requirements.is_empty();l
    }
    pub fn refresh(&mut self,goal:&str,sources:&[SourceEvidence]){
        self.target_scope=target_anchor(goal,sources);
        let course_hosts=super::course_comparison::source_hosts(goal,sources);
        for r in &mut self.requirements{
            r.candidates=0;r.state=State::Missing;
            match &r.scope{
                Scope::OpenResearch=>{
                    // Candidate evidence is not final verification. Every
                    // source must independently show admission pricing and
                    // hours before synthesis can even be attempted.
                    let candidates=verified_planning_candidates(sources);
                    r.candidates=candidates.len();
                    if candidates.len()>=3{r.state=State::Candidate;}
                },
                Scope::CoursePlatform(i)=>{if course_hosts.len()>*i{r.state=State::Observed;}},
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
    /// Explicit requested URLs whose factual dimensions still need evidence.
    pub fn missing_fact_sources(&self)->Vec<String>{
        let mut out=Vec::new();
        for r in &self.requirements{
            if r.state==State::Missing{
                if let Scope::Fact{source:Some(url),..}=&r.scope{
                    if !out.contains(url){out.push(url.clone());}
                }
            }
        }
        out
    }
    pub fn current_target_scope(&self,url:&str)->bool{
        self.target_scope.as_ref().is_some_and(|scope|web(scope).is_some()&&web(scope)==web(url))
    }
    /// Explicit outstanding objective for the native action selector. A candidate
    /// remains missing until its source and claim have been verified.
    pub fn next_research_objective(&self)->Option<&'static str>{
        self.requirements.iter().find(|r|r.state==State::Missing)
            .map(|r|match r.kind {
                "research_subjects"=>"Find independent public sources for the requested subjects, not search-engine interface links",
                "opening_hours"=>"Find exact opening hours on a public attraction source",
                "price"=>"Find exact ticket or admission prices on a public attraction source",
                "budget_constraint"=>"Collect itemized costs before calculating the total budget",
                "itinerary"=>"Collect verified locations and hours before arranging an itinerary",
                "course_platform_certificate_duration"=>"Find another independent course platform with certificate and duration evidence",
                _=>"Collect primary-source evidence for the next missing requirement",
            })
    }
    pub fn ready(&self)->bool{!self.supported||self.requirements.iter().all(|r|r.state!=State::Missing)}
    /// Open-ended planning can move from browsing to independent factual review
    /// only when its specific attraction/time/price candidate contract is met.
    /// This is not approval of the itinerary or its arithmetic.
    pub fn planning_review_ready(&self)->bool{
        self.supported && self.ready() && self.requirements.iter().any(|r|
            r.kind=="itinerary" && matches!(r.state,State::Candidate|State::Verified))
    }
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
        self.refresh(goal,sources);
        if !self.supported{return Ok(());}
        if report.status!="evidence_checked"{return Err("Independent report not verified".into());}
        // A preliminary passage extractor may miss a valid, exact source quote
        // later selected by the independent verifier. Fail closed for missing
        // source/target/planning requirements, but adjudicate factual
        // requirements against the actual report using the distinct-quote
        // matching below, not against a preliminary candidate shortlist.
        if self.requirements.iter().any(|r|r.state==State::Missing
            && !matches!(&r.scope,Scope::Fact{..})){
            self.can_review()?;
        }
        if self.requirements.iter().any(|r|matches!(r.scope,Scope::OpenResearch)){
            let candidates=verified_planning_candidates(sources);
            if candidates.len()<3{
                return Err("Fewer than three independently observed attractions with opening hours and prices".into());
            }
            // An observed attraction must also have an exact quote in the
            // independently verified report, not simply appear in browser
            // history. A generic search-result excerpt is insufficient.
            let grounded=candidates.iter().filter(|source|
                report.findings.iter().any(|f|source_matches(source,&f.url)
                    &&browser_skills::verification::quote_present(source,&f.quote))
            ).count();
            if grounded<3{
                return Err("Fewer than three attractions supported by verified report findings".into());
            }
            let summary=report.summary.to_lowercase();
            if !summary.contains("itinerary") || !summary.contains("total"){
                return Err("Verified outing requires an itinerary and itemized budget total".into());
            }
            let budget=first_currency_amount(goal)
                .ok_or("Outing budget amount cannot be parsed from the original request")?;
            let total=stated_outing_total(&report.summary)
                .ok_or("Outing itinerary has no numeric rupee total")?;
            if total>=budget{
                return Err(format!("Proposed outing total INR {total} violates budget INR {budget}"));
            }
            // The only currently deterministic group-price calculation is a
            // per-person admission fee on each official attraction page.
            // Different child/adult pricing needs a richer structured quote
            // contract and must not be guessed from an unlabelled amount.
            let mut tickets=0_u32;
            for source in &candidates{
                if !source.excerpt.to_ascii_lowercase().contains("per person"){
                    return Err("Attraction ticket quote lacks per-person pricing needed for group calculation".into());
                }
                let price=first_currency_amount(&source.excerpt)
                    .ok_or("Attraction entry fee is not a numeric rupee amount")?;
                tickets=tickets.checked_add(price)
                    .ok_or("Attraction admission-price calculation overflow")?;
            }
            let group_admission=tickets.checked_mul(4)
                .ok_or("Family admission-price calculation overflow")?;
            if total<group_admission{
                return Err(format!("Outing total INR {total} understates four-person admissions INR {group_admission}"));
            }
            for r in &mut self.requirements{r.state=State::Verified;}
            return Ok(());
        }
        if super::course_comparison::count(goal).is_some(){
            super::course_comparison::validate(goal,sources,report)?;
            for r in &mut self.requirements{r.state=State::Verified;}
            return Ok(());
        }
        ev::validate_comparison_operands(goal,sources,report)?;
        let facts:Vec<_>=self.requirements.iter().enumerate().filter_map(|(i,r)|if let Scope::Fact{source,facet}=&r.scope{Some((i,source,*facet))}else{None}).collect();
        let edges:Vec<Vec<usize>>=facts.iter().map(|(_,source,facet)|report.findings.iter().enumerate().filter_map(|(i,f)|{
            let scope_ok=match source{
                Some(u)=>web(u)==web(&f.url),
                None=>self.target_scope.as_ref().is_some_and(|a|target_source(goal,a,&f.url))
            };
            (scope_ok&&sources.iter().any(|s|source_matches(s,&f.url)&&ev::source_facet_supported(goal,facet,s,&f.quote)&&browser_skills::verification::quote_present(s,&f.quote))).then_some(i)
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
 #[test]fn planning_retains_explicit_deliverables_and_blocks_unverified_completion(){
  let g="Plan a Sunday outing in Bengaluru for a family of four under INR 2000. Find three family-friendly attractions, check their opening hours and ticket prices, and prepare an itinerary.";
  let mut l=Ledger::compile(g);
  assert!(l.supported);
  assert_eq!(l.missing(),vec!["research_subjects","opening_hours","price","budget_constraint","itinerary"]);
  l.refresh(g,&[s("https://example.org/","Example","A generic page with opening hours and ticket prices.")]);
  assert!(l.can_review().is_err());
 }
 #[test]fn admission_hours_require_explicit_clock_not_vague_opening_claim(){
  assert!(has_clock_time("open 9 am to 5 pm"));
  assert!(has_clock_time("opens 09:30 and closes 17:00"));
  assert!(has_clock_time("hours are 9am - 4pm"));
  assert!(!has_clock_time("open daily with regular working hours"));
  assert!(!has_clock_time("open 24 hours without listed admission times"));
 }
 #[test]fn outing_requires_three_observed_attractions_before_review(){
  let g="Plan a Sunday outing in Bengaluru for a family of four under INR 2000. Find three family-friendly attractions, check their opening hours and ticket prices, and prepare an itinerary.";
  let places=vec![
   s("https://park.example.org/visit","Rose Garden Park Official Tickets","Rose Garden Park opening hours 9 am to 5 pm. Entry ticket price INR 100 per person. Family visitors can tour the garden."),
   s("https://museum.example.org/plan","Science Museum Tickets","Science Museum opening hours 10 am to 5 pm. Admission fee INR 150 per person, visitors can tour the exhibits."),
   s("https://zoo.example.org/hours","City Zoo Entry Tickets","City Zoo opening hours 9 am to 4 pm. Entry ticket fee INR 75 per person for visitors."),
  ];
  let mut ledger=Ledger::compile(g);
  ledger.refresh(g,&places[..2]);
  assert!(!ledger.ready());
  assert!(!ledger.planning_review_ready(),"two attractions must not trigger itinerary review");
  ledger.refresh(g,&places);
  assert!(ledger.ready(),"three independent ticket+hours pages should be evidence candidates");
  assert!(ledger.planning_review_ready(),"three genuine priced/timed attractions should trigger independent review");
  assert!(ledger.requirements.iter().all(|r|r.state==State::Candidate));
  let report=ev::VerificationReport{status:"evidence_checked".into(),summary:"Itinerary: park 9 am, museum 11 am, zoo 2 pm. Total ₹1300 including four tickets per site.".into(),limitations:String::new(),findings:places.iter().map(|p|ev::GroundedFinding{url:p.url.clone(),claim:p.excerpt.clone(),quote:p.excerpt.clone()}).collect()};
  assert!(ledger.validate_report(g,&places,&report).is_ok());
  assert!(ledger.requirements.iter().all(|r|r.state==State::Verified));
  let mut insufficient=report.clone();
  insufficient.summary="Itinerary: park, museum, zoo. Total ₹900 for four people.".into();
  assert!(ledger.validate_report(g,&places,&insufficient).is_err(),
      "the total must account for all four admission tickets at each attraction");
  insufficient.summary="Itinerary: park, museum, zoo. Total ₹2500 for four people.".into();
  assert!(ledger.validate_report(g,&places,&insufficient).is_err(),
      "an over-budget total must fail");
  let bogus=vec![places[0].clone(),places[0].clone(),places[1].clone()];
  ledger.refresh(g,&bogus);
  assert!(!ledger.ready(),"duplicate attractions may not satisfy three-source coverage");
  let search=s("https://www.bing.com/search?q=family","Science Museum Park results","Museum opening hours 10 am to 5 pm. Entry ticket price INR 150.");
  ledger.refresh(g,&[places[0].clone(),places[1].clone(),search]);
  assert!(!ledger.ready(),"search-result pages must not count as observed attractions");
 }
 #[test]fn planning_provides_missing_evidence_as_native_action_objective(){
  let g="Plan a Sunday outing. Find three family attractions, check opening hours and ticket prices, and prepare an itinerary under INR 2000.";
  let l=Ledger::compile(g);
  assert_eq!(l.next_research_objective(),Some("Find independent public sources for the requested subjects, not search-engine interface links"));
  assert!(!l.ready());
 }
 #[test]fn unsupported_goals_keep_existing_verification(){
  assert!(!Ledger::compile("Open https://example.org").supported);
  assert!(!Ledger::compile("Buy a product and report its price").supported);
 }
}
