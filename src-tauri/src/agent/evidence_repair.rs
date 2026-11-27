//! Bounded read-only evidence recovery after an independent verifier rejection.
//! Only captured public pages may satisfy this contract. A navigation, a new
//! page title, or a model's claim never counts as repaired evidence.
use super::{browser_skills,dom_extractor::DomSnapshot,orchestrator::SourceEvidence,
    policy::BrowserAction};
use std::collections::HashSet;
use url::Url;

fn host(url:&str)->Option<String>{
    let parsed=browser_skills::public_url(url)?;
    Some(parsed.host_str()?.trim_start_matches("www.").to_ascii_lowercase())
}
fn named_host(url:&str,name:&str)->bool{
    let Some(h)=host(url)else{return false};
    let name=name.to_ascii_lowercase().replace([' ','-'],"");
    let parts=h.split('.').collect::<Vec<_>>();
    parts.len()==2&&parts[0].replace('-',"")==name
}
fn upgrades(text:&str)->bool{
    let t=text.to_ascii_lowercase();
    ["everything in free, and","everything in free, plus",
     "everything from free, plus","per user per month","billed annually",
     "billed yearly","free trial"].iter().any(|marker|t.contains(marker))
}
/// Source-exact snippets tying a free offering to collaboration or team size.
/// This is a *candidate* signal, never independent truth verification.
fn candidate_quotes(source:&SourceEvidence)->HashSet<String>{
    let mut result=HashSet::new();
    let excerpt=&source.excerpt;
    for segment in excerpt.split(['\n','\r','•','.',';']){
        let t=segment.trim().to_ascii_lowercase();
        if segment.chars().count()<22||segment.chars().count()>600
            ||!super::evidence_verifier::free_team_capacity_passage(&t)
            ||upgrades(&t) {continue}
        result.insert(t.split_whitespace().collect::<Vec<_>>().join(" "));
    }
    result
}
fn best_count(sources:&[SourceEvidence],name:&str)->usize{
    sources.iter().filter(|s|named_host(&s.url,name))
        .map(|s|candidate_quotes(s).len()).max().unwrap_or(0)
}
#[derive(Debug)]
pub struct Repair {
    target:String,
    domain:String,
    original_quotes:HashSet<String>,
    seen_urls:HashSet<String>,
    search_provider:u8,
    settle_checks:u8,
    attempted:u8,
}
impl Repair {
    pub fn from_failure(goal:&str,sources:&[SourceEvidence],reason:&str)->Option<Self>{
        if !super::evidence_verifier::requires_named_free_evidence(goal)
            || !super::deliberation::read_only(goal) {return None}
        let r=reason.to_ascii_lowercase();
        if !["insufficient","missing","incomplete","need to compare",
              "no verified recommendation","unsupported","did not establish",
              "rejected objective coverage","no sufficiently grounded",
              "free-tier recommendation relies","lacks a distinguishing",
              "missing source-grounded free-tier team limit"].iter().any(|k|r.contains(k)){return None}
        let names=super::deliberation::named_research_entities(goal);
        // A native verifier may name the exact missing source. Prefer that
        // user-named target; otherwise choose the weakest recorded capacity.
        let target=names.iter().find(|name|
            r.contains(&format!("for {}",name.to_ascii_lowercase())))
            .cloned().or_else(||names.iter().min_by_key(|name|best_count(sources,name)).cloned())?;
        let source=sources.iter().find(|s|named_host(&s.url,&target))?;
        let domain=host(&source.url)?;
        let original_quotes=sources.iter().filter(|s|named_host(&s.url,&target))
            .flat_map(candidate_quotes).collect();
        Some(Self{target,domain,original_quotes,seen_urls:HashSet::new(),
            search_provider:0,settle_checks:0,attempted:0})
    }
    pub fn has_new_fact(&self,sources:&[SourceEvidence])->bool{
        sources.iter().filter(|s|named_host(&s.url,&self.target))
            .flat_map(candidate_quotes)
            .any(|q|!self.original_quotes.contains(&q))
    }
    pub fn attempted(&self)->u8{self.attempted}
    pub fn target_label(&self)->&str{&self.target}
    /// An observed public result list can hydrate after navigation. Allow
    /// only two fresh observations before trying an alternate discovery path.
    /// Waiting is not factual progress and does not reset the retry budget.
    pub fn should_settle(&mut self,page:&DomSnapshot)->bool{
        if self.search_provider==0 || self.settle_checks>=2 {return false;}
        let provider=host(&page.origin).is_some_and(|h|
            ["google.com","bing.com","duckduckgo.com"].contains(&h.as_str()));
        if !provider {return false;}
        let has_official=page.interactive_nodes.iter().any(|n|
            n.href.as_deref().is_some_and(|h|host(h).as_deref()==Some(self.domain.as_str())));
        if has_official {return false;}
        self.settle_checks+=1;
        true
    }
    /// Only user-named existing public source domains can become query filters.
    /// Search-provider pages are discovery, not collected factual evidence.
    pub fn next(&mut self,page:&DomSnapshot)->Option<BrowserAction>{
        if self.attempted>=6 || !browser_skills::public_url(&page.origin).is_some() {return None;}
        let mut matches=Vec::new();
        for node in &page.interactive_nodes{
            if node.is_sensitive||node.is_submit {continue;}
            let Some(link)=node.href.as_deref()else{continue};
            let Some(parsed)=browser_skills::public_url(link)else{continue};
            if parsed.scheme()!="https"||parsed.fragment().is_some() {continue}
            let Some(link_host)=host(link)else{continue};
            if link_host!=self.domain||self.seen_urls.contains(link) {continue}
            let material=format!("{} {}",node.label,parsed.path()).to_ascii_lowercase();
            let score=["free","pricing","plans","plan","collaborat","team",
                "limit","member","features"].iter().filter(|part|material.contains(**part)).count();
            if score>=1 {matches.push((score,link.to_owned()));}
        }
        matches.sort_by(|a,b|b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        if let Some((_,url))=matches.into_iter().find(|(_,url)|url!=&page.origin){
            self.seen_urls.insert(url.clone());
            self.attempted+=1;
            return Some(BrowserAction::Navigate{url});
        }
        if self.search_provider<2 {
            let endpoint=if self.search_provider==0{
                "https://www.google.com/search"
            }else{"https://www.bing.com/search"};
            let mut target=Url::parse(endpoint).ok()?;
            target.query_pairs_mut().append_pair("q",&format!(
                "site:{} {} free plan collaborators members limits",self.domain,self.target));
            self.search_provider+=1;self.settle_checks=0;self.attempted+=1;
            return Some(BrowserAction::Navigate{url:target.to_string()});
        }
        None
    }
}
#[cfg(test)]
mod tests{
 use super::*;
 fn s(url:&str,excerpt:&str)->SourceEvidence{SourceEvidence{url:url.into(),title:"Official pricing".into(),excerpt:excerpt.into()}}
 const GOAL:&str="Visit the official websites of Alpha, Beta and Gamma. Compare their free plans for a team of four and recommend a suitable option.";
 #[test]fn verifier_gap_chooses_weakest_named_source_not_a_search_host(){
     let inputs=vec![
        s("https://alpha.com/pricing","Alpha Free plan includes up to 10 team members. Public repositories."),
        s("https://beta.net/","Beta prices start at $5 for paid subscriptions."),
        s("https://gamma.org/plans","Gamma Free plan includes 7 collaborators per workspace."),
     ];
     let error="Independent evidence verification failed: insufficient team-limit details";
     let repair=Repair::from_failure(GOAL,&inputs,error).unwrap();
     assert!(Repair::from_failure(GOAL,&inputs,
         "Semantic evidence review rejected objective coverage").is_some());
     assert!(Repair::from_failure(GOAL,&inputs,
         "Free-tier recommendation relies on paid-plan pricing or upgrades").is_some());
     assert_eq!(Repair::from_failure(GOAL,&inputs,
         "Missing source-grounded free-tier team limit for Gamma").unwrap().target_label(),"Gamma");
     assert_eq!(repair.target_label(),"Beta");
     assert!(!repair.has_new_fact(&inputs));
     let mut changed=inputs.clone();
     changed[1]=s("https://beta.net/free","Beta Free plan supports 4 members per team workspace.");
     assert!(repair.has_new_fact(&changed));
 }
 #[test]fn no_repair_for_actionable_goals_or_unrelated_format_errors(){
    let source=vec![s("https://alpha.com","Alpha Free plan includes members.")];
    assert!(Repair::from_failure("Submit a purchase order", &source,"insufficient facts").is_none());
    assert!(Repair::from_failure(GOAL,&source,"JSON parse failure").is_none());
 }
 #[test]fn recovery_follows_only_observed_official_links_then_bounded_search(){
    use super::super::dom_extractor::DomNodeSummary;
    let sources=vec![s("https://alpha.com","Alpha pricing currently unavailable"),
        s("https://beta.net","Beta Free plan includes 5 members."),
        s("https://gamma.org","Gamma Free plan includes 5 members.")];
    let mut repair=Repair::from_failure(GOAL,&sources,"incomplete").unwrap();
    let node=|url:&str| DomNodeSummary{node_id:url.into(),label:"Free pricing".into(),
      role:"link".into(),tag:"a".into(),href:Some(url.into()),options:vec![],
      checked:None,search_state:None,is_sensitive:false,is_submit:false,bbox:None};
    let page=DomSnapshot{origin:"https://alpha.com".into(),title:"Alpha".into(),
      text_excerpt:"Official plans and pricing for teams.".repeat(4),
      interactive_nodes:vec![node("https://alpha.com/free"),node("https://alpha.evil.example/pricing")],
      structure:Default::default()};
    assert!(matches!(repair.next(&page),Some(BrowserAction::Navigate{url}) if url=="https://alpha.com/free"));
    assert!(matches!(repair.next(&page),Some(BrowserAction::Navigate{url}) if url.starts_with("https://www.google.com/search?")));
    let mut results=page.clone();
    results.origin="https://www.google.com/search?q=sample".into();
    results.interactive_nodes.clear();
    assert!(repair.should_settle(&results));
    assert!(repair.should_settle(&results));
    assert!(!repair.should_settle(&results));
    assert!(matches!(repair.next(&results),Some(BrowserAction::Navigate{url}) if url.starts_with("https://www.bing.com/search?")));
    results.origin="https://www.bing.com/search?q=sample".into();
    assert!(repair.should_settle(&results));
    assert!(repair.should_settle(&results));
    assert!(!repair.should_settle(&results));
    assert!(repair.next(&results).is_none());
    assert!(repair.attempted()<=6);
 }
}