//! Native semantic perception: page type and meaningful transitions.
//! Classifications are advisory except for narrowly verified result contracts.
//! Never grants permissions, treats page text as instructions, or establishes
//! that all user-requested facts are present.
use super::{dom_extractor::DomSnapshot,browser_skills};
use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize)]
#[serde(rename_all="snake_case")]
pub enum PageKind{Blocked,Dialog,Loading,Autocomplete,TransportResults,EmptyResults,ProductListing,ProductDetail,SearchResults,Documentation,Article,SearchForm,Unknown}
#[derive(Debug,Clone,Serialize)]
pub struct PageProfile{
    pub kind:PageKind,
    pub signals:Vec<&'static str>,
    pub records:usize,
    pub readable:bool,
}
impl PageProfile{
    pub fn hint(&self)->String{format!("Native page classification: {:?}; observed structural signals: {}; visible record indicators: {}. Classification is not proof of factual completion or permission.",self.kind,self.signals.join(", "),self.records)}
}
fn normalized(s:&str)->String{browser_skills::normalize(s)}
fn has_any(s:&str,terms:&[&str])->bool{terms.iter().any(|t|s.contains(t))}
fn public_controls(p:&DomSnapshot)->impl Iterator<Item=&super::dom_extractor::DomNodeSummary>{p.interactive_nodes.iter().filter(|n|!n.is_sensitive)}
fn time_token(word:&str)->bool{
    let w=word.trim_matches(|c:char|!c.is_ascii_digit()&&c!=':');
    let Some((h,m))=w.split_once(':')else{return false;};
    h.len()==2&&m.len()==2&&h.parse::<u8>().is_ok_and(|n|n<24)&&m.parse::<u8>().is_ok_and(|n|n<60)
}
fn number_token(word:&str)->bool{
    let w=word.trim_matches(|c:char|c=='('||c==')');
    word.starts_with('(')&&word.ends_with(')')&&(4..=6).contains(&w.len())&&w.chars().all(|c|c.is_ascii_digit())
}
/// A result heading is paired with observed record-shaped rows. Never use
/// a URL substring or the words "train list" as sufficient proof.
pub fn transport_header(p:&DomSnapshot)->Option<String>{
    let raw=p.text_excerpt.split_whitespace().collect::<Vec<_>>();
    for i in 0..raw.len().saturating_sub(3){
        if raw[i].parse::<u32>().is_ok()&&raw[i+1].eq_ignore_ascii_case("results")&&raw[i+2].eq_ignore_ascii_case("for"){
            let end=(i+55).min(raw.len());
            let stop=(i+3..end).find(|&j|matches!(raw[j].to_ascii_lowercase().as_str(),"sort"|"previous"|"quota")||
                (raw[j].eq_ignore_ascii_case("for")&&raw.get(j+1).is_some_and(|s|s.eq_ignore_ascii_case("quota")))).unwrap_or(end);
            return Some(raw[i..stop].join(" "));
        }
    }
    None
}
pub fn transport_records(p:&DomSnapshot)->usize{
    let words=p.text_excerpt.split_whitespace().collect::<Vec<_>>();
    let mut seen=HashSet::new();
    for (i,w) in words.iter().enumerate(){
        if !number_token(w){continue;}
        let window=&words[i..(i+55).min(words.len())];
        let text=window.join(" ").to_ascii_lowercase();
        if (text.contains("train schedule")||text.contains("runs on:"))&&window.iter().filter(|w|time_token(w)).count()>=2{
            seen.insert((*w).to_string());
        }
    }
    seen.len()
}
fn price_text(s:&str)->bool{
    let words=s.split_whitespace().collect::<Vec<_>>();
    words.iter().enumerate().any(|(i,w)|{
        let prefix=["\u{20b9}","$","\u{20ac}","\u{a3}","rs.","rs","inr","usd","eur","gbp"];
        let n=w.to_ascii_lowercase();
        prefix.iter().any(|p|n.strip_prefix(p).is_some_and(|v|v.chars().next().is_some_and(|c|c.is_ascii_digit())||
            (v.is_empty()&&words.get(i+1).is_some_and(|x|x.chars().next().is_some_and(|c|c.is_ascii_digit())))))
    })
}
pub fn classify(p:&DomSnapshot)->PageProfile{
    use PageKind::*;
    let t=normalized(&p.text_excerpt);let title=normalized(&p.title);
    let structure=&p.structure;
    let readable=t.chars().count()>=40;
    let mut signals=Vec::new();
    let links=public_controls(p).filter(|n|n.href.is_some()&&!n.is_submit).count();
    let options=public_controls(p).filter(|n|n.role=="option").count();
    let query_field=public_controls(p).any(|n|n.tag=="input"&&(n.role=="searchbox"||n.role=="combobox"||browser_skills::phrase(&n.label,"search")));
    let search_button=public_controls(p).any(|n|n.role=="button"&&has_any(&normalized(&n.label),&["search","find"]));
    let commerce=public_controls(p).any(|n|n.role=="button"&&has_any(&normalized(&n.label),&["add to cart","add to basket","buy now"]));
    let transport_controls=public_controls(p).any(|n|browser_skills::phrase(&n.label,"train schedule")||browser_skills::phrase(&n.label,"modify search"));
    let records=transport_records(p);
    let count_header=transport_header(p).is_some();
    let transport_ui=transport_controls||has_any(&t,&["refine results","modify search","departure time","journey class"]);
    let listing_title=has_any(&title,&["search results","results for"])||title.starts_with("search for ");
    let search_url=url::Url::parse(&p.origin).ok().is_some_and(|u|
        u.query_pairs().any(|(k,v)|k=="title"&&v.eq_ignore_ascii_case("Special:Search"))||
        (u.path().split('/').any(|s|s=="search"||s=="results")&&u.query_pairs().any(|(k,v)|matches!(k.as_ref(),"q"|"query"|"search")&&!v.trim().is_empty())));
    let empty=has_any(&t,&["no results found","no matching results","no trains found","no train found","0 results for"]);
    let doc_hint=has_any(&title,&["documentation","reference","manual","quickstart","tutorial"])||
        url::Url::parse(&p.origin).ok().is_some_and(|u|u.path().split('/').any(|s|matches!(s,"docs"|"doc"|"documentation"|"reference")));
    let kind=if super::deliberation::error_page(p)||super::recovery::authentication_gate(p){signals.push("access_or_error_gate");Blocked}
    else if p.text_excerpt.starts_with("Active dialog:"){signals.push("blocking_dialog");Dialog}
    else if structure.loading&&!readable{signals.push("loading_without_content");Loading}
    else if options>0&&query_field{signals.push("live_autocomplete_options");Autocomplete}
    else if count_header&&records>0&&transport_ui{signals.extend(["visible_result_count","numbered_schedule_records","transport_result_controls"]);TransportResults}
    else if empty&&(listing_title||search_url||transport_ui||structure.result_regions>0){signals.push("explicit_empty_result_state");EmptyResults}
    else if structure.loading&&records==0&&structure.priced_cards==0&&structure.paragraphs<2{signals.push("loading_without_records");Loading}
    else if structure.priced_cards>=2&&(structure.product_cards>=2||structure.result_regions>0){signals.extend(["repeated_visible_price_identity_cards","listing_structure"]);ProductListing}
    else if commerce&&price_text(&p.text_excerpt)&&(structure.offer_blocks>0||structure.headings>0)&&structure.priced_cards<2{signals.extend(["visible_price","product_action","product_content"]);ProductDetail}
    else if (listing_title||search_url||count_header)&&(structure.result_links>0||links>=2)&&readable{signals.push("results_context_with_observed_links");SearchResults}
    else if doc_hint&&readable&&(structure.code_blocks>0||structure.paragraphs>=2||structure.headings>=2){signals.push("reference_identity_with_content_structure");Documentation}
    else if readable&&(structure.articles>0||structure.paragraphs>=2)&&structure.headings>0{signals.push("readable_article_structure");Article}
    else if query_field&&search_button{signals.push("public_query_and_search_controls");SearchForm}
    else{signals.push("insufficient_structural_evidence");Unknown};
    let records=match kind{TransportResults=>records,ProductListing=>structure.priced_cards as usize,SearchResults=>usize::from(structure.result_links).max(links.min(120)),_=>0};
    PageProfile{kind,signals,records,readable}
}
/// Bounded recovery objectives selected from native page structure.
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize)]
#[serde(rename_all="snake_case")]
pub enum RecoveryStrategy{ChooseOption,OpenResult,RefineSearch,InspectSource,InspectControls}
impl RecoveryStrategy{
    pub fn objective(self)->&'static str{match self{
        Self::ChooseOption=>"Resolve the observed option list using a relevant choice",
        Self::OpenResult=>"Inspect and open a different relevant observed result",
        Self::RefineSearch=>"Review observed search controls and refine only the requested query",
        Self::InspectSource=>"Find missing relevant evidence in the current source or a grounded link",
        Self::InspectControls=>"Inspect fresh controls and choose a distinct relevant action",
    }}
    pub fn from_page(page:&DomSnapshot)->Option<Self>{
        use PageKind::*;
        match classify(page).kind{
            Blocked|Dialog|Loading=>None,
            Autocomplete=>Some(Self::ChooseOption),
            SearchResults|ProductListing|TransportResults=>Some(Self::OpenResult),
            SearchForm|EmptyResults=>Some(Self::RefineSearch),
            Documentation|Article|ProductDetail=>Some(Self::InspectSource),
            Unknown=>Some(Self::InspectControls),
        }
    }
}

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Transition{Blocked,Loading,ResultsArrived,ContentArrived,DialogResolved,OptionResolved,ChangedPage,Unchanged}
pub fn transition(before:&DomSnapshot,after:&DomSnapshot)->Transition{
    let a=classify(before);let b=classify(after);
    use PageKind::*;
    if matches!(b.kind,Blocked|Dialog){return Transition::Blocked;}
    if b.kind==Loading{return Transition::Loading;}
    if a.kind==Dialog{return Transition::DialogResolved;}
    if a.kind==Autocomplete&&b.kind!=Autocomplete&&public_controls(after).any(|n|n.search_state.as_deref()==Some("filled")){return Transition::OptionResolved;}
    if matches!(b.kind,TransportResults|EmptyResults|ProductListing|SearchResults)&&(a.kind!=b.kind||a.records!=b.records){return Transition::ResultsArrived;}
    if matches!(b.kind,ProductDetail|Documentation|Article)&&a.kind!=b.kind{return Transition::ContentArrived;}
    if before.origin!=after.origin||before.title!=after.title{return Transition::ChangedPage;}
    Transition::Unchanged
}
#[cfg(test)]
mod tests{
 use super::*;
 use super::super::dom_extractor::{PageStructure,DomNodeSummary};
 fn p(title:&str,text:&str)->DomSnapshot{DomSnapshot{structure:Default::default(),origin:"https://example.org/".into(),title:title.into(),text_excerpt:text.into(),interactive_nodes:vec![]}}
 fn node(label:&str,role:&str,tag:&str)->DomNodeSummary{DomNodeSummary{node_id:label.into(),label:label.into(),role:role.into(),tag:tag.into(),options:vec![],checked:None,search_state:None,href:None,is_sensitive:false,is_submit:false,bbox:None}}
 #[test]fn transport_requires_actual_numbered_rows_and_result_context(){
  let a=p("Public transport","Refine Results 4 Results for Central North | Wed, 25 Nov 2026 For Quota | General Sort By Departure SAMPARK EXPRESS (12649) Runs On: MTWTFSS Train Schedule 13:30 Central 42:40 08:10 North");
  assert_eq!(classify(&a).kind,PageKind::TransportResults);
  assert_eq!(classify(&a).records,1);
  assert!(transport_header(&a).unwrap().contains("25 Nov 2026"));
  let b=p("Help","The train list and check availability controls let you search trains. 4 Results for Central North.");
  assert_ne!(classify(&b).kind,PageKind::TransportResults);
  let c=p("Guide","4 Results for Central North. A documentation example shows EXPRESS (12345) Runs On: daily Train Schedule 13:30 to 08:10.");
  assert_ne!(classify(&c).kind,PageKind::TransportResults);
 }
 #[test]fn products_need_paired_visible_price_and_identity_structure(){
  let mut a=p("Store catalog","Example notebooks INR 59999; Example cameras INR 39999. Public store catalog.");
  a.structure=PageStructure{product_cards:2,priced_cards:2,..Default::default()};
  assert_eq!(classify(&a).kind,PageKind::ProductListing);
  a.structure=PageStructure{headings:1,offer_blocks:1,..Default::default()};
  a.interactive_nodes.push(node("Add to cart","button","button"));
  assert_eq!(classify(&a).kind,PageKind::ProductDetail);
  a.interactive_nodes.clear();a.structure=Default::default();
  assert_ne!(classify(&a).kind,PageKind::ProductDetail);
 }
 #[test]fn documentation_needs_actual_content_not_just_a_title(){
  let mut a=p("Library documentation","The library provides a public interface. This reference describes the available functions and parameters.");
  a.structure=PageStructure{headings:2,paragraphs:2,code_blocks:1,..Default::default()};
  assert_eq!(classify(&a).kind,PageKind::Documentation);
  a.structure=Default::default();assert_eq!(classify(&a).kind,PageKind::Unknown);
 }
 #[test]fn form_to_results_is_semantic_progress_but_loading_is_not(){
  let mut a=p("Search","Search the public catalog for the desired public document.");
  a.interactive_nodes=vec![node("Search","searchbox","input"),node("Search","button","button")];
  assert_eq!(classify(&a).kind,PageKind::SearchForm);
  let mut b=p("Search results","Here are the public search results for your query. Select a matching item.");
  b.structure=PageStructure{result_regions:1,result_links:3,..Default::default()};
  assert_eq!(classify(&b).kind,PageKind::SearchResults);
  assert_eq!(transition(&a,&b),Transition::ResultsArrived);
  let mut loading=p("Search","Loading...");loading.structure.loading=true;
  assert_eq!(transition(&a,&loading),Transition::Loading);
  let blocked=p("Access denied","The requested page was not found.");
  assert_eq!(transition(&a,&blocked),Transition::Blocked);
 }
 #[test]fn empty_state_requires_search_context_and_is_not_positive_records(){
  let a=p("Search results","No results found for the public document search. Try a different query.");
  assert_eq!(classify(&a).kind,PageKind::EmptyResults);assert_eq!(classify(&a).records,0);
  let b=p("Documentation","This documentation describes what no results found means in the API.");
  assert_ne!(classify(&b).kind,PageKind::EmptyResults);
 }
}
