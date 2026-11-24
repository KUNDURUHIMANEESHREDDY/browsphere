//! Keep observed documentation navigation within its selected release.
use super::{browser_skills as bs,dom_extractor::DomSnapshot};
use std::collections::HashSet;
use url::Url;
fn version_segment(u:&Url)->Option<String>{
    let parts=u.path_segments()?.collect::<Vec<_>>();
    let i=parts.iter().position(|s|matches!(*s,"docs"|"doc"|"documentation"|"reference"))?;
    let v=*parts.get(i+1)?;
    let named=matches!(v,"current"|"stable"|"latest"|"dev"|"devel"|"master"|"main");
    let numeric=!v.is_empty()&&v.chars().next().is_some_and(|c|c.is_ascii_digit())&&
        v.split('.').all(|s|!s.is_empty()&&s.chars().all(|c|c.is_ascii_digit()));
    (named||numeric).then(||v.to_string())
}
fn current_label(s:&str)->bool{
    let words=bs::words(s);
    matches!(words.first().map(String::as_str),Some("current"|"stable"|"latest"))&&
        words.iter().skip(1).all(|w|matches!(w.as_str(),"version"|"release")||w.chars().all(|c|c.is_ascii_digit()))
}
/// Stay in the documentation release selected by the observed current link
/// or explicitly opened starting branch. No version number is guessed.
pub fn reference_track_allowed(page:&DomSnapshot,target:&str)->bool{
    let (Some(from),Some(to))=(bs::public_url(&page.origin),bs::public_url(target))else{return false;};
    if from.origin()!=to.origin(){return false;}
    let Some(dest)=version_segment(&to)else{return true;};
    if let Some(current)=version_segment(&from){return current==dest;}
    let tracks=page.interactive_nodes.iter().filter(|n|bs::public_control(n)&&current_label(&n.label))
        .filter_map(|n|n.href.as_deref().and_then(bs::public_url))
        .filter(|u|u.origin()==from.origin()).filter_map(|u|version_segment(&u)).collect::<HashSet<_>>();
    // Ambiguous version labels must not license arbitrary historical branches.
    tracks.is_empty()||tracks.len()==1&&tracks.contains(&dest)
}

#[cfg(test)]
mod tests{
 use super::*;
 use super::super::dom_extractor::DomNodeSummary;
 fn page(origin:&str,nodes:Vec<DomNodeSummary>)->DomSnapshot{DomSnapshot{structure:Default::default(),origin:origin.into(),title:"Public documentation".into(),text_excerpt:"Public documentation with release links.".into(),interactive_nodes:nodes}}
 fn node(id:&str,label:&str,href:Option<&str>)->DomNodeSummary{DomNodeSummary{node_id:id.into(),label:label.into(),href:href.map(str::to_owned),tag:"a".into(),role:"link".into(),options:vec![],checked:None,search_state:None,is_sensitive:false,is_submit:false,bbox:None}}
 #[test]fn documentation_version_affinity_blocks_historical_branches(){
  let p=page("https://example.org/docs/",vec![node("c","Current (18)",Some("https://example.org/docs/current/index.html"))]);
  assert!(reference_track_allowed(&p,"https://example.org/docs/current/sql-commands.html"));
  assert!(!reference_track_allowed(&p,"https://example.org/docs/7.1/sql-insert.html"));
  let current=page("https://example.org/docs/current/sql-commands.html",vec![]);
  assert!(!reference_track_allowed(&current,"https://example.org/docs/7.1/sql-insert.html"));
  assert!(reference_track_allowed(&current,"https://example.org/docs/current/sql-insert.html"));
  let selected=page("https://example.org/docs/17/index.html",vec![]);
  assert!(reference_track_allowed(&selected,"https://example.org/docs/17/sql-insert.html"));
  assert!(!reference_track_allowed(&selected,"https://example.org/docs/current/sql-insert.html"));
 }
}
