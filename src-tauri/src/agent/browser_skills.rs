//! Reusable observation-backed browser skill primitives. These are not an
//! authorization layer: all actions must still pass native policy and binding.
use super::{dom_extractor::{DomNodeSummary,DomSnapshot},orchestrator::SourceEvidence,policy::BrowserAction};
use url::Url;
pub fn normalize(s:&str)->String{s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()}
pub fn words(s:&str)->Vec<String>{s.split(|c:char|!c.is_alphanumeric()).filter(|s|!s.is_empty()).map(str::to_lowercase).collect()}
pub fn phrase(h:&str,n:&str)->bool{let h=words(h);let n=words(n);!n.is_empty()&&h.windows(n.len()).any(|w|w==n.as_slice())}
pub fn unique<'a,T:'a>(mut candidates:impl Iterator<Item=&'a T>)->Option<&'a T>{let first=candidates.next()?;candidates.next().is_none().then_some(first)}
pub fn public_control(n:&DomNodeSummary)->bool{!n.is_sensitive&&!n.is_submit}
pub fn public_url(s:&str)->Option<Url>{
    let u=Url::parse(s).ok()?;
    (matches!(u.scheme(),"https"|"http")&&u.host_str().is_some()&&u.username().is_empty()&&u.password().is_none()).then_some(u)
}
pub fn readable(p:&DomSnapshot)->bool{
    public_url(&p.origin).is_some()&&!p.text_excerpt.starts_with("Active dialog:")&&
        !super::deliberation::error_page(p)&&!super::recovery::authentication_gate(p)&&p.text_excerpt.trim().chars().count()>=40
}
#[derive(Debug,Clone,Copy,PartialEq,Eq,serde::Serialize)]
#[serde(rename_all="snake_case")]
pub enum Outcome{Confirmed,MissingControl,AmbiguousControl,NoEffect,AwaitingObservation,Unverified,Restricted}
pub mod search{
    use super::*;
    pub fn input(n:&DomNodeSummary)->bool{public_control(n)&&n.tag=="input"&&(n.role=="searchbox"||phrase(&n.label,"search"))&&matches!(n.search_state.as_deref(),Some("empty"|"filled"))}
    pub fn opener(n:&DomNodeSummary)->bool{public_control(n)&&n.href.is_none()&&matches!(n.role.as_str(),"button"|"link")&&phrase(&n.label,"search")&&!["clear","close","cancel","advanced"].iter().any(|w|phrase(&n.label,w))}
    pub fn fields(p:&DomSnapshot)->Vec<&DomNodeSummary>{p.interactive_nodes.iter().filter(|n|input(n)).collect()}
    pub fn opened(after:&DomSnapshot)->Outcome{
        match fields(after).len(){0=>Outcome::NoEffect,1=>Outcome::Confirmed,_=>Outcome::AmbiguousControl}
    }
    /// Occupancy is a receipt for entry, not a claim about actual field text.
    /// Call only after the exact authorized query was sent to the native input.
    pub fn entered(before:&DomSnapshot,after:&DomSnapshot,id:&str)->Outcome{
        if before.origin!=after.origin{return Outcome::Unverified;}
        let Some(n)=before.interactive_nodes.iter().find(|n|n.node_id==id&&input(n))else{return Outcome::MissingControl;};
        if unique(after.interactive_nodes.iter().filter(|m|input(m)&&m.label==n.label&&m.search_state.as_deref()==Some("filled"))).is_some(){Outcome::Confirmed}else{Outcome::NoEffect}
    }
    pub fn results_observed(before:&DomSnapshot,after:&DomSnapshot,match_link:impl Fn(&DomNodeSummary)->bool)->Outcome{
        if super::super::recovery::authentication_gate(after)||super::super::deliberation::error_page(after){return Outcome::Restricted;}
        if after.interactive_nodes.iter().any(|n|public_control(n)&&match_link(n)){Outcome::Confirmed}
        else if before.origin!=after.origin{Outcome::AwaitingObservation}else{Outcome::NoEffect}
    }
}
pub mod navigation{
    use super::*;
    /// Rank distinct observed links, not arbitrary model-authored destinations.
    /// The caller owns target scoring and route budgets.
    pub fn ranked(p:&DomSnapshot,score:impl Fn(&DomNodeSummary)->u32,allowed:impl Fn(&str)->bool)->Vec<(u32,String)>{
        let mut links=std::collections::HashMap::<String,u32>::new();
        for n in &p.interactive_nodes{
            let Some(raw)=n.href.as_deref()else{continue};
            if !public_control(n)||public_url(raw).is_none()||!allowed(raw)||raw==p.origin{continue;}
            let value=score(n);if value>0{links.entry(raw.into()).and_modify(|s|*s=(*s).max(value)).or_insert(value);}
        }
        let mut out=links.into_iter().map(|(url,s)|(s,url)).collect::<Vec<_>>();
        out.sort_by(|a,b|b.0.cmp(&a.0).then(a.1.cmp(&b.1)));out
    }
    pub fn receipt(target:&str,after:&DomSnapshot)->Outcome{
        if super::super::evidence_verifier::check_navigation_result(target,after).is_ok()&&readable(after){Outcome::Confirmed}else{Outcome::Unverified}
    }
}
pub mod autocomplete{
    use super::*;
    pub fn matches<'a>(p:&'a DomSnapshot,value:&str)->Vec<&'a DomNodeSummary>{
        p.interactive_nodes.iter().filter(|n|public_control(n)&&n.role=="option"&&n.bbox.as_ref().map_or(true,|b|b.in_viewport)&&phrase(&n.label,value)).collect()
    }
    pub fn choose<'a>(p:&'a DomSnapshot,value:&str)->Result<&'a DomNodeSummary,Outcome>{
        let list=matches(p,value);let exact=list.iter().filter(|n|normalize(&n.label)==normalize(value)).copied().collect::<Vec<_>>();
        if exact.len()==1{return Ok(exact[0]);}
        match list.as_slice(){[n]=>Ok(*n),[]=>Err(Outcome::MissingControl),_=>Err(Outcome::AmbiguousControl)}
    }
    pub fn receipt(before:&DomSnapshot,after:&DomSnapshot,option_id:&str,value:&str,field:Option<&DomNodeSummary>)->Outcome{
        if before.origin!=after.origin||after.text_excerpt.starts_with("Active dialog:"){return Outcome::Unverified;}
        if !matches(before,value).iter().any(|n|n.node_id==option_id){return Outcome::MissingControl;}
        if after.interactive_nodes.iter().any(|n|n.role=="option"){return Outcome::AwaitingObservation;}
        if field.is_some_and(|n|public_control(n)&&n.search_state.as_deref()==Some("filled")){Outcome::Confirmed}else{Outcome::NoEffect}
    }
}
pub mod form{
    use super::*;
    pub fn public_select<'a>(p:&'a DomSnapshot,field:&str)->Option<&'a DomNodeSummary>{unique(p.interactive_nodes.iter().filter(|n|public_control(n)&&n.tag=="select"&&normalize(&n.label)==normalize(field)))}
    pub fn selected(n:&DomNodeSummary,value:&str)->bool{public_control(n)&&n.search_state.as_deref().and_then(|s|s.strip_prefix("selected_option:")).is_some_and(|s|normalize(s)==normalize(value))}
    pub fn select_action(n:&DomNodeSummary,value:&str)->Result<BrowserAction,Outcome>{
        if !public_control(n)||n.tag!="select"{return Err(Outcome::Restricted);}
        if selected(n,value){return Err(Outcome::Confirmed);}
        let mut options=n.options.iter().filter(|s|normalize(s)==normalize(value));
        let option=options.next().ok_or(Outcome::MissingControl)?;
        if options.next().is_some(){return Err(Outcome::AmbiguousControl);}
        Ok(BrowserAction::SelectOption{node_id:n.node_id.clone(),option_label:option.clone()})
    }
    pub fn date_action(p:&DomSnapshot,labels:&[String])->Result<BrowserAction,Outcome>{
        let dates=p.interactive_nodes.iter().filter(|n|public_control(n)&&matches!(n.role.as_str(),"button"|"gridcell"|"link")&&labels.contains(&normalize(&n.label))).collect::<Vec<_>>();
        if dates.len()>1{return Err(Outcome::AmbiguousControl);}
        if let Some(n)=dates.first(){return Ok(BrowserAction::Click{node_id:n.node_id.clone(),element_label:n.label.clone(),is_submit:false});}
        let pickers=p.interactive_nodes.iter().filter(|n|public_control(n)&&n.tag=="input"&&n.search_state.as_deref()==Some("read_only")&&(phrase(&n.label,"date")||normalize(&n.label).contains("dd/mm/yyyy"))).collect::<Vec<_>>();
        match pickers.as_slice(){[n]=>Ok(BrowserAction::Click{node_id:n.node_id.clone(),element_label:n.label.clone(),is_submit:false}),[]=>Err(Outcome::MissingControl),_=>Err(Outcome::AmbiguousControl)}
    }
}
pub mod extraction{
    use super::*;
    /// Native snapshot only, no hidden input readback or invented text.
    pub fn capture(p:&DomSnapshot)->Option<SourceEvidence>{
        readable(p).then(||SourceEvidence{url:p.origin.clone(),title:p.title.chars().take(140).collect(),excerpt:p.text_excerpt.chars().take(24000).collect()})
    }
    /// Retrieve exact contiguous passages containing every requested token.
    /// Useful for code/specification/price candidates, not a product or price
    /// verification by itself; numbers must later be tied to the right item.
    pub fn passages(source:&SourceEvidence,required:&[&str],limit:usize)->Vec<String>{
        if required.is_empty()||required.len()>12||required.iter().any(|s|s.trim().is_empty()){return vec![];}
        let tokens=source.excerpt.split_whitespace().take(6000).collect::<Vec<_>>();
        let mut results=Vec::new();
        for i in 0..tokens.len(){
            let end=(i+32).min(tokens.len());
            let text=tokens[i..end].join(" ");
            if text.chars().count()<=400&&required.iter().all(|s|normalize(&text).contains(&normalize(s)))&&!results.iter().any(|p:&String|p.contains(&text)||text.contains(p)){
                results.push(text);
                if results.len()>=limit.clamp(1,8){break;}
            }
        }
        results
    }
}
pub mod research{
    use super::*;
    pub fn observed(url:&Url,sources:&[SourceEvidence])->bool{
        sources.iter().any(|s|public_url(&s.url).as_ref()==Some(url)&&s.excerpt.trim().chars().count()>=80&&
            readable(&DomSnapshot{origin:s.url.clone(),title:s.title.clone(),text_excerpt:s.excerpt.clone(),interactive_nodes:vec![]}))
    }
    pub fn missing(required:&[Url],sources:&[SourceEvidence])->Vec<Url>{required.iter().filter(|u|!observed(u,sources)).cloned().collect()}
}
pub mod verification{
    use super::*;
    pub fn quote_present(source:&SourceEvidence,quote:&str)->bool{
        let q=normalize(quote);q.chars().count()>=12&&normalize(&source.excerpt).contains(&q)
    }
    pub fn require_dimensions<'a>(required:impl IntoIterator<Item=&'a str>,supported:impl Fn(&str)->bool)->Result<(),String>{
        for facet in required{if !supported(facet){return Err(format!("Required factual dimension missing: {facet}"));}}
        Ok(())
    }
}
pub mod recovery{
    use super::*;
    #[derive(Debug,Clone,Copy,PartialEq,Eq,serde::Serialize)]
    #[serde(rename_all="snake_case")]
    pub enum Strategy{Reobserve,AlternateObservedControl,ResolveAmbiguity,CollectEvidence,StopRestricted,StopExhausted}
    pub fn strategy(outcome:Outcome,remaining:u8)->Strategy{
        if outcome==Outcome::Restricted{return Strategy::StopRestricted;}
        if remaining==0{return Strategy::StopExhausted;}
        match outcome{
            Outcome::AwaitingObservation|Outcome::Confirmed=>Strategy::Reobserve,
            Outcome::AmbiguousControl=>Strategy::ResolveAmbiguity,
            Outcome::Unverified=>Strategy::CollectEvidence,
            Outcome::MissingControl|Outcome::NoEffect=>Strategy::AlternateObservedControl,
            Outcome::Restricted=>Strategy::StopRestricted,
        }
    }
}
#[cfg(test)]
mod tests{
 use super::*;
 fn n(id:&str,tag:&str,role:&str,label:&str,state:Option<&str>,href:Option<&str>)->DomNodeSummary{
  DomNodeSummary{node_id:id.into(),tag:tag.into(),role:role.into(),label:label.into(),search_state:state.map(str::to_owned),href:href.map(str::to_owned),options:vec![],checked:None,is_sensitive:false,is_submit:false,bbox:None}
 }
 fn page(url:&str,nodes:Vec<DomNodeSummary>)->DomSnapshot{DomSnapshot{origin:url.into(),title:"Public test document".into(),text_excerpt:"This public document contains readable information about the requested item and its supporting facts.".repeat(2),interactive_nodes:nodes}}
 #[test]fn search_requires_observed_input_and_actual_result(){
  let p=page("https://example.org/",vec![n("q","input","searchbox","Search articles",Some("empty"),None)]);
  let mut filled=p.clone();filled.interactive_nodes[0].search_state=Some("filled".into());
  assert_eq!(search::entered(&p,&filled,"q"),Outcome::Confirmed);
  assert_eq!(search::entered(&p,&p,"q"),Outcome::NoEffect);
  let result=page("https://example.org/results",vec![n("r","a","link","Target",None,Some("https://example.org/target"))]);
  assert_eq!(search::results_observed(&filled,&result,|n|n.label=="Target"),Outcome::Confirmed);
  assert_eq!(search::results_observed(&filled,&result,|n|n.label=="Other"),Outcome::AwaitingObservation);
 }
 #[test]fn navigation_deduplicates_and_checks_destination(){
  let p=page("https://example.org/",vec![n("1","a","link","Target",None,Some("https://example.org/target")),n("2","a","link","Target",None,Some("https://example.org/target")),n("3","a","link","Target",None,Some("javascript:alert(1)"))]);
  assert_eq!(navigation::ranked(&p,|_|100,|_|true),vec![(100,"https://example.org/target".into())]);
  assert_eq!(navigation::receipt("https://example.org/target",&p),Outcome::Unverified);
  assert_eq!(navigation::receipt("https://example.org/target",&page("https://example.org/target",vec![])),Outcome::Confirmed);
 }
 #[test]fn autocomplete_requires_unique_matching_suggestion_and_closed_popup(){
  let p=page("https://example.org/",vec![n("a","li","option","Central Station",None,None),n("b","li","option","Central East",None,None)]);
  assert!(matches!(autocomplete::choose(&p,"Central"),Err(Outcome::AmbiguousControl)));
  let choice=autocomplete::choose(&p,"Central Station").unwrap();
  let field=n("f","input","combobox","Station",Some("filled"),None);
  assert_eq!(autocomplete::receipt(&p,&p,&choice.node_id,"Central Station",Some(&field)),Outcome::AwaitingObservation);
  assert_eq!(autocomplete::receipt(&p,&page(&p.origin,vec![field.clone()]),&choice.node_id,"Central Station",Some(&field)),Outcome::Confirmed);
 }
 #[test]fn form_exact_select_and_date_preconditions(){
  let mut field=n("f","select","combobox","Category",Some("selected_option:All"),None);
  field.options=vec!["All".into(),"Books".into()];
  assert!(matches!(form::select_action(&field,"Books"),Ok(BrowserAction::SelectOption{..})));
  assert!(matches!(form::select_action(&field,"Music"),Err(Outcome::MissingControl)));
  field.search_state=Some("selected_option:Books".into());assert!(form::selected(&field,"Books"));
  let p=page("https://example.org/",vec![n("d","button","gridcell","24 November 2026",None,None)]);
  assert!(form::date_action(&p,&["24 november 2026".into()]).is_ok());
  assert!(form::date_action(&p,&["25 november 2026".into()]).is_err());
 }
 #[test]fn extraction_returns_only_captured_contiguous_public_text(){
  let mut p=page("https://example.org/product",vec![]);p.text_excerpt="The Example Notebook includes 16 GB memory and 256 GB storage. Listed price INR 57999. Installation example: pip install example.".into();
  let s=extraction::capture(&p).unwrap();
  assert!(!extraction::passages(&s,&["16 GB","256 GB","INR 57999"],3).is_empty());
  assert!(extraction::passages(&s,&["INR 99999"],3).is_empty());
  p.text_excerpt="Active dialog: Sign in".into();assert!(extraction::capture(&p).is_none());
 }
 #[test]fn research_checks_every_required_source(){
  let a=Url::parse("https://example.org/a").unwrap();let b=Url::parse("https://example.net/b").unwrap();
  let s=extraction::capture(&page(a.as_str(),vec![])).unwrap();
  assert_eq!(research::missing(&[a,b.clone()],&[s]),vec![b]);
 }
 #[test]fn verification_rejects_missing_facts_and_invented_quotes(){
  let s=extraction::capture(&page("https://example.org/",vec![])).unwrap();
  assert!(verification::quote_present(&s,"This public document contains readable information"));
  assert!(!verification::quote_present(&s,"This public document guarantees imaginary features"));
  assert!(verification::require_dimensions(["feature","caveat"],|s|s=="feature").is_err());
  assert!(verification::require_dimensions(["feature","caveat"],|_|true).is_ok());
 }
 #[test]fn recovery_does_not_retry_restricted_or_exhausted_work(){
  use recovery::Strategy;
  assert_eq!(recovery::strategy(Outcome::Restricted,3),Strategy::StopRestricted);
  assert_eq!(recovery::strategy(Outcome::NoEffect,0),Strategy::StopExhausted);
  assert_eq!(recovery::strategy(Outcome::AmbiguousControl,2),Strategy::ResolveAmbiguity);
  assert_eq!(recovery::strategy(Outcome::Unverified,2),Strategy::CollectEvidence);
  assert_eq!(recovery::strategy(Outcome::NoEffect,2),Strategy::AlternateObservedControl);
 }
}
