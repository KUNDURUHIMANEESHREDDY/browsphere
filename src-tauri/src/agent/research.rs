//! Explicit single-document research objectives. Destination discovery and
//! factual verification are separate: a title is never a factual answer.
use super::{dom_extractor::DomSnapshot,orchestrator::SourceEvidence};
fn norm(s:&str)->String{s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()}
pub fn subject(goal:&str)->Option<String>{
 if !super::deliberation::read_only(goal)||super::evidence_verifier::requested_named_result_title(goal).is_some(){return None}
 let lower=goal.to_ascii_lowercase();
 if !["report","explain","summarize","meaning"].iter().any(|s|lower.contains(s))||
 ["compare","comparison","multiple sources","two sources","other sources","list of","fill ","form ","download","upload","account","email","message","order ","apply ","video"].iter().any(|s|lower.contains(s)){return None}
 let start=lower.find("search for ")?+"search for ".len();
 let rest=goal.get(start..)?;
 let suffix=rest.to_ascii_lowercase();
 let end=suffix.find(" and open").or_else(||suffix.find(", open"))?;
 let value=rest.get(..end)?.trim();
 let value=["the word ","the planet ","the term ","the article "].iter()
  .find_map(|p|value.get(..p.len()).filter(|s|s.eq_ignore_ascii_case(p)).and_then(|_|value.get(p.len()..))).unwrap_or(value).trim();
 if !(3..=100).contains(&value.chars().count())||value.split_whitespace().count()>12||
 !value.chars().all(|c|c.is_alphanumeric()||matches!(c,' '|'-'|'\'')){return None}
 Some(value.into())
}
pub fn query(goal:&str)->Option<String>{
 super::evidence_verifier::requested_named_result_title(goal).filter(|_|super::orchestrator::is_simple_named_title_task(goal)).or_else(||subject(goal))
}
pub fn matches_subject(goal:&str,page:&DomSnapshot)->bool{
 let Some(wanted)=subject(goal) else{return false};
 let Ok(url)=url::Url::parse(&page.origin) else{return false};
 if !matches!(url.scheme(),"http"|"https")||url.host_str().is_none()||!url.username().is_empty()||url.password().is_some()||
 super::deliberation::error_page(page)||super::deliberation::search_listing(page)||
 page.text_excerpt.starts_with("Active dialog:")||page.text_excerpt.trim().chars().count()<80{return false}
 let expected=norm(&wanted);
 let title=norm(&page.title);
 let slug=url.path_segments().and_then(|mut s|s.next_back()).unwrap_or("").replace(['_','-']," ");
 let slug=norm(&url::form_urlencoded::parse(slug.as_bytes()).next().map(|(s,_)|s.into_owned()).unwrap_or_default());
 let title_ok=title==expected||title.strip_prefix(&expected).is_some_and(|r|r.starts_with(" - ")||r.starts_with(" | ")||r.starts_with(" (")||r.starts_with(" ? "));
 title_ok&&slug==expected
}
pub fn destination(goal:&str,initial:&str,page:&DomSnapshot)->bool{
 matches_subject(goal,page)&&url::Url::parse(initial).ok().zip(url::Url::parse(&page.origin).ok()).is_some_and(|(a,b)|a.origin()==b.origin()&&a!=b)
}
pub fn source(page:&DomSnapshot)->SourceEvidence{
 SourceEvidence{url:page.origin.clone(),title:super::policy::sanitize_untrusted_page_text(&page.title,180),excerpt:super::policy::sanitize_untrusted_page_text(&page.text_excerpt,5500)}
}
#[cfg(test)]
mod tests{
 use super::*;
 fn page()->DomSnapshot{DomSnapshot{origin:"https://example.org/wiki/Neptune".into(),title:"Neptune - Encyclopedia".into(),text_excerpt:"Neptune is the eighth planet from the Sun. It is an ice giant with a thick atmosphere and many moons.".into(),interactive_nodes:vec![]}}
 const G:&str="Search for the planet Neptune, open the relevant article, and report its position and planet type.";
 #[test]fn research_scope_is_not_title_only_or_consequential(){
  assert_eq!(subject(G).as_deref(),Some("Neptune"));
  assert_eq!(subject("Search for the word serendipity and open its entry. Report the English meaning.").as_deref(),Some("serendipity"));
  assert!(subject("Search for Neptune and open its article. Report the page title.").is_none());
  assert!(subject("Search for a train and open the booking form. Report and pay for it.").is_none());
  assert!(subject("Search for Neptune and open the article and compare multiple sources.").is_none());
 }
 #[test]fn target_is_an_exact_subject_on_the_observed_site(){
  let mut p=page();assert!(destination(G,"https://example.org/",&p));
  assert!(!destination(G,"https://another.example/",&p));
  p.title="Neptune-like planets - Encyclopedia".into();assert!(!matches_subject(G,&p));
  p.title="Neptune - Encyclopedia".into();p.origin="https://example.org/help/search".into();assert!(!matches_subject(G,&p));
 }
}
