//! Run-scoped fact dependency graph shared by research, synthesis, and review.
 //! Extracts candidate *passages* only from native browser-captured source text.
 //! No claim is verified until an independent report supplies source-exact quotes.
use super::{browser_skills,deliberation,evidence_verifier as ev,
            orchestrator::SourceEvidence};
use serde::Serialize;
use std::collections::{BTreeSet,HashSet};

#[derive(Clone,Debug,Serialize,PartialEq,Eq)]
#[serde(rename_all="snake_case")]
pub enum EvidenceState { Missing, Candidate, Conflicted, Verified }

#[derive(Clone,Debug,Serialize)]
pub struct FactNode {
    pub source:String,
    pub dimension:String,
    pub state:EvidenceState,
    pub candidates:usize,
    pub quote_urls:usize,
    #[serde(skip)]
    observations:Vec<(String,String)>,
}
#[derive(Clone,Debug)]
enum SourceScope { Url(String), Named(String) }
#[derive(Clone,Debug)]
enum Dimension { Facet(&'static str), FreeTier, FreeTeamCapacity }

#[derive(Clone,Debug)]
struct Obligation { source:SourceScope, dimension:Dimension }
#[derive(Clone,Debug)]
pub struct FactGraph { goal:String, obligations:Vec<Obligation>, nodes:Vec<FactNode> }
fn clean(s:&str)->String{s.split_whitespace().collect::<Vec<_>>().join(" ").to_ascii_lowercase()}
fn website(url:&str)->Option<url::Url>{browser_skills::public_url(url)}
fn is_primary_named(url:&str,name:&str)->bool{
    let Some(url)=website(url)else{return false};
    let Some(host)=url.host_str()else{return false};
    let labels=host.strip_prefix("www.").unwrap_or(host).split('.').collect::<Vec<_>>();
    labels.len()==2&&labels[0].replace('-',"")==name.to_ascii_lowercase().replace([' ','-'],"")
}
fn matches_scope(source:&SourceScope,url:&str)->bool{
    match source{
        SourceScope::Url(target)=>website(url).is_some()&&website(url)==website(target),
        SourceScope::Named(name)=>is_primary_named(url,name),
    }
}
fn source_label(source:&SourceScope)->String{
    match source{SourceScope::Url(url)=>url.clone(),SourceScope::Named(name)=>name.clone()}
}
fn dim_label(dimension:&Dimension)->String{
    match dimension{
        Dimension::Facet(f)=>f.to_string(),
        Dimension::FreeTier=>"free_tier".into(),
        Dimension::FreeTeamCapacity=>"free_team_capacity".into()
    }
}
fn free_tier_text(text:&str)->bool{
    let lower=clean(text);
    (lower.contains("free plan")||lower.contains("free tier")
        ||lower.contains("free forever"))
        &&!["free trial","everything from free, plus","everything in free, plus",
             "everything in free, and","per user/month","per user per month",
             "billed annually"].iter().any(|x|lower.contains(x))
}
fn chunks(text:&str)->Vec<String>{
    // Bound text scanned per source. Repeated page boilerplate cannot produce
    // infinite evidence or unbounded memory usage.
    text.chars().take(24000).collect::<String>()
        .split_inclusive(['.','!','?','\n'])
        .filter_map(|part|{
            let s=part.trim();
            ((25..=450).contains(&s.chars().count())).then(||s.to_string())
        }).take(350).collect()
}
fn capacity_value(raw:&str)->Option<String>{
    if !ev::free_team_capacity_passage(raw){return None}
    let tokens=raw.to_ascii_lowercase().split(|c:char|!c.is_ascii_alphanumeric())
        .filter(|v|!v.is_empty()).map(str::to_string).collect::<Vec<_>>();
    // Only a member/collaborator/seat limit is an accepted capacity value.
    // 'Unlimited repositories' is never interpreted as unlimited members.
    for (i,token) in tokens.iter().enumerate(){
        if !(token.starts_with("member")||token.starts_with("collaborator")
            ||token.starts_with("user")||token.starts_with("seat")
            ||token.starts_with("guest")){continue}
        let from=i.saturating_sub(4);
        for word in tokens[from..i].iter().rev(){
            if word=="unlimited"{return Some("unlimited".into())}
            if let Ok(number)=word.parse::<u16>(){
                if number>0&&number<=10000{return Some(number.to_string())}
            }
        }
    }
    None
}
fn observations(goal:&str,source:&SourceEvidence,dimension:&Dimension)->Vec<String>{
    match dimension{
        Dimension::Facet(facet)=>ev::facet_candidates(goal,source).into_iter()
            .filter(|row|row.facet==*facet)
            .flat_map(|row|row.candidates.into_iter().map(|p|p.text))
            .filter(|passage|source.excerpt.contains(passage)).take(12).collect(),
        Dimension::FreeTier=>chunks(&source.excerpt).into_iter()
            .filter(|text|free_tier_text(text)).take(12).collect(),
        Dimension::FreeTeamCapacity=>chunks(&source.excerpt).into_iter()
            .filter(|text|capacity_value(text).is_some()).take(12).collect(),
    }
}
fn conflict(dimension:&Dimension, passages:&[(String,String)])->bool{
    if !matches!(dimension,Dimension::FreeTeamCapacity){return false}
    let values=passages.iter().filter_map(|(_,quote)|capacity_value(quote))
        .collect::<BTreeSet<_>>();
    values.len()>1
}
impl FactGraph{
    /// Only explicit, inspectable sources are compiled. Arbitrary LLM
    /// conclusions and unnamed inferred domains never become requirements.
    pub fn compile(goal:&str)->Option<Self>{
        if !deliberation::read_only(goal){return None}
        let urls=deliberation::explicit_research_sources(goal);
        let dims=ev::research_facets(goal);
        let mut obligations=Vec::new();
        if !urls.is_empty() && !dims.is_empty(){
            for url in urls{
                for facet in &dims{
                    obligations.push(Obligation{source:SourceScope::Url(url.to_string()),
                        dimension:Dimension::Facet(*facet)});
                }
            }
        }else if ev::requires_named_free_evidence(goal){
            let team=goal.split(|c:char|!c.is_ascii_alphanumeric())
                .any(|w|["team","member","members","collaborators","users","seats"].contains(&w.to_ascii_lowercase().as_str()));
            for name in deliberation::named_research_entities(goal){
                obligations.push(Obligation{source:SourceScope::Named(name.clone()),
                    dimension:Dimension::FreeTier});
                if team{obligations.push(Obligation{source:SourceScope::Named(name),
                    dimension:Dimension::FreeTeamCapacity});}
            }
        }
        if obligations.is_empty()||obligations.len()>36{return None}
        let nodes=obligations.iter().map(|o|FactNode{
            source:source_label(&o.source),dimension:dim_label(&o.dimension),
            state:EvidenceState::Missing,candidates:0,quote_urls:0,observations:vec![]
        }).collect();
        Some(Self{goal:goal.to_string(),obligations,nodes})
    }
    pub fn observe(&mut self,sources:&[SourceEvidence]){
        for (o,node) in self.obligations.iter().zip(&mut self.nodes){
            node.observations.clear();
            let mut unique=HashSet::new();
            for s in sources.iter().filter(|s|matches_scope(&o.source,&s.url)){
                for quote in observations(&self.goal,s,&o.dimension){
                    if unique.insert((s.url.clone(),clean(&quote))){
                        node.observations.push((s.url.clone(),quote));
                    }
                }
            }
            node.candidates=node.observations.len();
            node.quote_urls=node.observations.iter().map(|(url,_)|url)
                .collect::<HashSet<_>>().len();
            node.state=if conflict(&o.dimension,&node.observations){
                EvidenceState::Conflicted
            }else if node.candidates>0{EvidenceState::Candidate}
            else{EvidenceState::Missing};
        }
    }
    pub fn next_gap(&self)->Option<&FactNode>{
        // Conflicts block synthesis, but research missing entirely is first.
        self.nodes.iter().find(|n|n.state==EvidenceState::Missing)
            .or_else(||self.nodes.iter().find(|n|n.state==EvidenceState::Conflicted))
    }
    pub fn next_gap_for_source(&self,source:&str)->Option<&FactNode>{
        self.nodes.iter().find(|n|n.source.eq_ignore_ascii_case(source)
            &&n.state==EvidenceState::Missing)
            .or_else(||self.nodes.iter().find(|n|n.source.eq_ignore_ascii_case(source)
                &&n.state==EvidenceState::Conflicted))
    }
    pub fn can_synthesize(&self)->bool{
        self.nodes.iter().all(|n|matches!(n.state,EvidenceState::Candidate|EvidenceState::Verified))
    }
    pub fn nodes(&self)->&[FactNode]{&self.nodes}
    pub fn hint(&self)->String{
        match self.next_gap(){
            Some(f)=>format!("Fact dependency gap: {} on {} is {:?}. Research the exact requested source and obtain a quoted passage. Do not synthesize or declare success.",f.dimension,f.source,f.state),
            None=>"Required fact candidates captured; these are NOT verified. Independent source-exact validation is still mandatory.".into()
        }
    }
    /// Verifying the graph never replaces the existing independent review.
    /// Exact quotations must appear in BOTH native captures and the review.
    pub fn validate_report(&mut self,sources:&[SourceEvidence],report:&ev::VerificationReport)->Result<(),String>{
        if report.status!="evidence_checked"{
            return Err("Independent verification receipt is missing".into())
        }
        if !self.can_synthesize(){return Err(self.hint())}
        // Validate atomically. Selected excerpts may use a larger contiguous
        // native source quote than the candidate extraction did. Revalidate
        // the exact fact predicate against the independent source-bound quote;
        // neither model claims nor unobserved search snippets may certify it.
        for (obligation,node) in self.obligations.iter().zip(&self.nodes){
            let matched=report.findings.iter().any(|f|{
                sources.iter().filter(|s|s.url==f.url&&
                    matches_scope(&obligation.source,&s.url)).any(|source|{
                        if !browser_skills::verification::quote_present(source,&f.quote){return false;}
                        match &obligation.dimension{
                            Dimension::Facet(facet)=>ev::source_facet_supported(&self.goal,facet,source,&f.quote),
                            Dimension::FreeTier=>free_tier_text(&f.quote),
                            Dimension::FreeTeamCapacity=>capacity_value(&f.quote).is_some()
                        }
                    })
            });
            if !matched{
                return Err(format!("No exact reviewed source fact for {} on {}",node.dimension,node.source))
            }
        }
        for node in &mut self.nodes{node.state=EvidenceState::Verified;}
        Ok(())
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    fn source(url:&str,text:&str)->SourceEvidence{
        SourceEvidence{url:url.into(),title:"Official free plans".into(),excerpt:text.into()}
    }
    const GOAL:&str="Visit the official websites of Alpha, Beta and Gamma. Compare their free plans for a four-member team.";
    #[test]fn requirements_are_source_and_dimension_scoped(){
        let mut g=FactGraph::compile(GOAL).unwrap();
        assert_eq!(g.nodes().len(),6);
        assert!(!g.can_synthesize());
        g.observe(&[source("https://alpha.com/pricing","Alpha Free plan supports up to 10 members per team. Free tier includes collaboration features."),
            source("https://beta.net/plans","Beta Free plan supports up to 4 members per workspace. Free tier includes project sharing."),
            source("https://gamma.org/pricing","Gamma Free plan includes up to 6 collaborators per team. Free tier supports shared tasks.")]);
        assert!(g.can_synthesize(),"{:?}",g.nodes());
        assert!(g.nodes().iter().all(|n|n.state==EvidenceState::Candidate));
    }
    #[test]fn url_domain_lookalikes_and_search_results_are_not_source_evidence(){
        let mut g=FactGraph::compile(GOAL).unwrap();
        g.observe(&[source("https://alpha.evil.com/pricing","Alpha Free plan supports 8 members per team."),
            source("https://www.bing.com/search?q=alpha","Alpha Free plan supports 8 members per team.")]);
        assert_eq!(g.nodes()[0].state,EvidenceState::Missing);
        assert_eq!(g.nodes()[1].state,EvidenceState::Missing);
    }
    #[test]fn contradictory_capacity_on_one_site_blocks_synthesis(){
        let mut g=FactGraph::compile(GOAL).unwrap();
        g.observe(&[
            source("https://alpha.com/pricing","Alpha Free plan supports up to 5 members per team."),
            source("https://alpha.com/support","Alpha Free plan supports up to 10 members per team."),
        ]);
        assert_eq!(g.nodes()[1].state,EvidenceState::Conflicted);
        assert!(!g.can_synthesize());
    }
    #[test]fn stage_research_focus_prefers_current_source_without_waiving_global_gaps(){
        let mut g=FactGraph::compile(GOAL).unwrap();
        g.observe(&[source("https://alpha.com/pricing","Alpha Free plan supports up to 10 members per team.")]);
        assert_eq!(g.next_gap_for_source("Beta").unwrap().source,"Beta");
        assert_eq!(g.next_gap().unwrap().source,"Beta");
        assert!(g.next_gap_for_source("Alpha").is_none());
        assert!(!g.can_synthesize());
    }
    #[test]fn exact_independent_quotes_promote_all_requirements_atomically(){
        let a="Alpha Free plan supports up to 10 members per team.";
        let b="Beta Free plan includes up to 5 collaborators per workspace.";
        let c="Gamma Free plan supports up to 6 users per team.";
        let mut g=FactGraph::compile(GOAL).unwrap();
        let rows=[source("https://alpha.com/pricing",a),source("https://beta.net/plans",b),source("https://gamma.org/plans",c)];
        g.observe(&rows);
        assert!(g.can_synthesize());
        let finding=|url:&str,quote:&str|ev::GroundedFinding{
            claim:quote.to_string(),url:url.to_string(),quote:quote.to_string()};
        let mut report=ev::VerificationReport{
            status:"evidence_checked",summary:"Confirmed free-tier capacities".into(),
            findings:vec![finding(&rows[0].url,a),finding(&rows[1].url,b)],
            limitations:"Captured website claims only".into()
        };
        assert!(g.validate_report(&rows,&report).is_err());
        assert!(g.nodes().iter().all(|n|n.state==EvidenceState::Candidate));
        report.findings.push(finding(&rows[2].url,c));
        g.validate_report(&rows,&report).unwrap();
        assert!(g.nodes().iter().all(|n|n.state==EvidenceState::Verified));
    }
    #[test]fn observed_candidates_cannot_be_claimed_verified_without_exact_report(){
        let mut g=FactGraph::compile(GOAL).unwrap();
        g.observe(&[source("https://alpha.com/","Alpha Free plan supports 6 members per team.")]);
        let empty=ev::VerificationReport{
            status:"evidence_checked",summary:"Unverified".into(),findings:vec![],limitations:String::new()
        };
        assert!(g.validate_report(&[source("https://alpha.com/","Alpha Free plan supports 6 members per team.")],&empty).is_err());
        assert_eq!(g.nodes()[1].state,EvidenceState::Candidate);
    }
}