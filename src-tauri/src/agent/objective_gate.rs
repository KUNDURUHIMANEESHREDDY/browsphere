//! Final run-local objective constraints. This can only narrow native policy,
//! never grant a permission, submit a transaction, or certify a factual answer.
use super::{dom_extractor::{DomSnapshot,DomNodeSummary},policy::BrowserAction,semantic_core::ObjectiveLedger};
pub const REVISION:&str="lucia-role-orchestration-v7";
fn words(s:&str)->Vec<String>{s.split(|c:char|!c.is_alphanumeric()).filter(|w|!w.is_empty()).map(str::to_lowercase).collect()}
fn phrase(text:&str,part:&str)->bool{
    let t=words(text);let p=words(part);
    !p.is_empty()&&t.windows(p.len()).any(|w|w==p.as_slice())
}
fn off_topic(goal:&str,target:&str)->bool{
    // Reject unrelated utility links, not documentation about these topics.
    let n=target.replace(['-','_','/']," ");
    ["pricing","contact sales","sign in","sign up","login","register","checkout","careers","privacy policy","reservation charts","online charts"].iter()
        .any(|w|phrase(&n,w)&&!phrase(goal,w))
}
fn control<'a>(page:&'a DomSnapshot,id:&str)->Option<&'a DomNodeSummary>{
    page.interactive_nodes.iter().find(|n|n.node_id==id&&!n.is_sensitive)
}
fn search_field(n:&DomNodeSummary)->bool{
    n.tag=="input"&&(n.role=="searchbox"||n.role=="combobox"||phrase(&n.label,"search"))
}
fn route_guard(goal:&str,initial:&str,page:&DomSnapshot,action:&BrowserAction)->Result<(),&'static str>{
    let Some((from,to))=super::prerequisites::route_values(goal)else{return Ok(())};
    if matches!(action,BrowserAction::Complete{..}){
        return super::prerequisites::completion_blocker(goal,page).map_or(Ok(()),Err);
    }
    if matches!(action,BrowserAction::Scroll{..}){return Ok(())}
    if let BrowserAction::Navigate{url}=action{
        return if url==initial&&page.origin!=initial{Ok(())}else{Err("Stay with the original search form and observed result; no unrelated navigation")};
    }
    let node=match action{
        BrowserAction::Click{node_id,..}|BrowserAction::TypeText{node_id,..}|BrowserAction::PressKey{node_id,..}|BrowserAction::SelectOption{node_id,..}=>control(page,node_id),
        _=>None
    }.ok_or("Search-only route action is not an observed public control")?;
    if off_topic(goal,&node.label)||node.href.as_ref().is_some_and(|h|off_topic(goal,h)){
        return Err("Unrelated navigation or account control does not advance the route search");
    }
    if page.text_excerpt.starts_with("Active dialog:"){
        if let (Some(BrowserAction::Click{node_id,..}),BrowserAction::Click{node_id:chosen,..})=(super::prerequisites::language(goal,page),action){
            if node_id==*chosen{return Ok(())}
        }
        return Err("Only the explicitly requested language choice can resolve this modal");
    }
    match action{
        BrowserAction::TypeText{text,is_sensitive:false,..} if node.tag=="input"=>{
            let label=node.label.to_lowercase();
            let exact=|value:&str|text.trim().eq_ignore_ascii_case(value);
            if ((phrase(&label,"from")||phrase(&label,"origin"))&&exact(from))
                ||((phrase(&label,"to")||phrase(&label,"destination"))&&exact(to)){return Ok(())}
            if phrase(&label,"date")&&super::prerequisites::date_value_allowed(goal,text){return Ok(())}
            Err("Use only the exact requested city or journey date in its matching field")
        }
        BrowserAction::Click{..}=>{
            let label=node.label.to_lowercase();
            if node.role=="option"{
                if phrase(&label,from)||phrase(&label,to){return Ok(())}
                return Err("Station suggestion is not grounded in either requested city");
            }
            if (node.tag=="input"&&(phrase(&label,"from")||phrase(&label,"to")||phrase(&label,"station")||phrase(&label,"date")))
                || (node.href.is_none()&&["search","search trains","find trains","calendar","journey date"].iter().any(|s|label.trim()==*s))
                || super::prerequisites::observed_date_choice(goal,&node.label){return Ok(())}
            Err("Control is not a route field, matching station option, exact date or public search")
        }
        BrowserAction::PressKey{key,..} if key=="Escape"&&search_field(node)=>Ok(()),
        BrowserAction::PressKey{key,..} if key=="Enter"&&node.role=="searchbox"&&node.search_state.as_deref()==Some("filled")&&page.interactive_nodes.iter().any(|n|n.role=="option"&&(phrase(&n.label,from)||phrase(&n.label,to)))=>Ok(()),
        _=>Err("Action is not part of the bounded read-only journey search")
    }
}
fn public_query(goal:&str)->Option<String>{
    let lower=goal.to_ascii_lowercase();
    let patterns=[("find the official "," repository on "),("find the "," repository on "),("research the "," on "),("search for "," and open"),("search for "," then "),("find the "," command reference")];
    for (prefix,suffix) in patterns{
        let Some(start)=lower.find(prefix).map(|i|i+prefix.len())else{continue};
        let Some(end)=lower[start..].find(suffix).map(|i|i+start)else{continue};
        let value=goal[start..end].trim();
        if value.chars().count()>=3&&value.chars().count()<=100&&
            value.chars().all(|c|c.is_alphanumeric()||matches!(c,' '|'-'|'_'|'/'|'.')){return Some(value.into())}
    }
    None
}
fn docs_waypoint(goal:&str,label:&str,url:&str)->bool{
    if !phrase(goal,"documentation")&&!phrase(goal,"reference"){return false;}
    let path=url::Url::parse(url).ok().map(|u|u.path().to_lowercase()).unwrap_or_default();
    (path.contains("/doc")||path.contains("/reference"))&&
        ["current","current version","documentation","reference","sql commands","command reference","manual","user guide"].iter().any(|p|label.trim().eq_ignore_ascii_case(p))
}
fn relevant_navigation(goal:&str,page:&DomSnapshot,url:&str)->bool{
    if off_topic(goal,url){return false}
    if goal.split_whitespace().any(|s|s.trim_matches(['(',')','[',']','"', '\'',',','.',';'])==url){return true;}
    if super::policy::requested_navigation_url(goal).is_some_and(|u|u.as_str()==url){return true;}
    if super::policy::requested_search_entry_url(goal).is_some_and(|u|u.as_str()==url){return true;}
    if super::deliberation::lookup_subject(goal).is_some()&&super::deliberation::navigation_grounded(goal,page,url){return true;}
    page.interactive_nodes.iter().any(|n|!n.is_sensitive&&!n.is_submit&&n.href.as_deref()==Some(url)&&
        (super::semantic_core::relevance(goal,&format!("{} {}",n.label,url::Url::parse(url).map(|u|u.path().replace(['-','_','/']," ")).unwrap_or_default()))>0||
            docs_waypoint(goal,&n.label,url)||phrase(&n.label,"search")))
}
#[derive(Default)]
pub struct ReplacementPlanner{
    attempted:std::collections::HashSet<(String,String)>,
    typed:Option<(String,String)>,
}
impl ReplacementPlanner{
    pub fn note(&mut self,goal:&str,before:&DomSnapshot,action:&BrowserAction,ok:bool){
        if !ok{return;}
        if let BrowserAction::TypeText{node_id,text,is_sensitive:false}=action{
            if public_query(goal).as_ref()==Some(text){
                if let Some(n)=control(before,node_id){self.typed=Some((before.origin.clone(),n.label.clone()));}
            }
        }
        if matches!(action,BrowserAction::PressKey{key,..} if key=="Enter"){self.typed=None;}
    }
    /// Choose a distinct, directly grounded public action. Normal policy and
    /// live binding must still run after this decision.
    pub fn next(&mut self,goal:&str,page:&DomSnapshot,ledger:&ObjectiveLedger,routes:&super::semantic_core::RouteLedger)->Option<BrowserAction>{
        if !super::deliberation::read_only(goal)||ledger.active()||
            super::prerequisites::route_values(goal).is_some()||super::prerequisites::filter_constraints(goal).is_some()||
            super::deliberation::error_page(page)||super::recovery::authentication_gate(page)||page.text_excerpt.starts_with("Active dialog:"){return None;}
        let mut proposals=Vec::<(i32,String,BrowserAction)>::new();
        let query=public_query(goal);
        for n in &page.interactive_nodes{
            if n.is_sensitive||n.is_submit{continue;}
            if off_topic(goal,&n.label){continue;}
            if let Some(q)=query.as_ref(){
                if search_field(n){
                    if self.typed.as_ref().is_some_and(|(origin,label)|origin==&page.origin&&label==&n.label)&&n.search_state.as_deref()==Some("filled"){
                        proposals.push((95,format!("enter:{}",n.label),BrowserAction::PressKey{node_id:n.node_id.clone(),key:"Enter".into()}));
                    }else if n.search_state.as_deref()==Some("empty"){
                        proposals.push((90,format!("type:{}",n.label),BrowserAction::TypeText{node_id:n.node_id.clone(),text:q.clone(),is_sensitive:false}));
                    }
                }else if n.href.is_none()&&matches!(n.role.as_str(),"button"|"link")&&phrase(&n.label,"search"){
                    proposals.push((65,format!("search:{}",n.label),BrowserAction::Click{node_id:n.node_id.clone(),element_label:n.label.clone(),is_submit:false}));
                }
            }
            if let Some(url)=n.href.as_ref(){
                if !routes.allowed(url)||!relevant_navigation(goal,page,url){continue;}
                let label_score=super::semantic_core::relevance(goal,&n.label) as i32;
                let path=url::Url::parse(url).ok().map(|u|u.path().replace(['-','_','/']," ")).unwrap_or_default();
                let path_score=super::semantic_core::relevance(goal,&path) as i32;
                let exact=query.as_ref().is_some_and(|q|phrase(&format!("{} {path}",n.label),q));
                let score=if exact{110}else{label_score*20+path_score*10+if docs_waypoint(goal,&n.label,url){35}else{0}};
                if score>=20{proposals.push((score,format!("link:{url}"),BrowserAction::Navigate{url:url.clone()}));}
            }
        }
        proposals.sort_by(|a,b|b.0.cmp(&a.0));
        for (_,key,action) in proposals{
            if self.attempted.len()>=40{break;}
            if self.attempted.insert((page.origin.clone(),key)){return Some(action);}
        }
        None
    }
}
pub fn check(goal:&str,initial:&str,page:&DomSnapshot,ledger:&ObjectiveLedger,action:&BrowserAction)->Result<(),&'static str>{
    route_guard(goal,initial,page,action)?;
    if ledger.active(){
        match action{
            BrowserAction::Navigate{url} if ledger.permitted_destination(url)=>{},
            BrowserAction::Complete{..} if ledger.complete()=>{},
            BrowserAction::Scroll{..}=>{},
            _=>return Err("Finish the exact requested source objectives; unrelated actions and premature completion are blocked")
        }
    }
    if !super::deliberation::read_only(goal)||ledger.active(){return Ok(())}
    match action{
        BrowserAction::Navigate{url}=>{
            if !relevant_navigation(goal,page,url){return Err("Navigation has no grounded connection to the original research objective");}
        }
        BrowserAction::Click{node_id,..}=>{
            let n=control(page,node_id).ok_or("Research control is not a live public control")?;
            if off_topic(goal,&n.label)||n.href.as_ref().is_some_and(|h|off_topic(goal,h)){
                return Err("Research cannot navigate to unrelated account, pricing or transaction controls");
            }
        }
        BrowserAction::TypeText{node_id,text,is_sensitive:false}=>{
            let n=control(page,node_id).ok_or("Research input is not observed")?;
            if !search_field(n)||super::semantic_core::relevance(goal,text)==0{
                return Err("Research typing must target a public search input and a task-related query");
            }
        }
        BrowserAction::PressKey{node_id,key} if key=="Enter"=>{
            let n=control(page,node_id).ok_or("Research search input is not observed")?;
            if !search_field(n)||n.search_state.as_deref()!=Some("filled"){
                return Err("Enter requires an observed filled public search field");
            }
        }
        BrowserAction::Download{..}|BrowserAction::SetChecked{..}=>return Err("Read-only factual research does not authorize this action"),
        _=>{}
    }
    Ok(())
}
#[cfg(test)]
mod tests{
 use super::*;
 use super::super::dom_extractor::DomNodeSummary;
 fn node(id:&str,label:&str,href:Option<&str>)->DomNodeSummary{DomNodeSummary{node_id:id.into(),tag:"a".into(),role:"link".into(),label:label.into(),href:href.map(str::to_owned),options:vec![],checked:None,search_state:None,is_sensitive:false,is_submit:false,bbox:None}}
 fn page()->DomSnapshot{DomSnapshot{origin:"https://example.org/search".into(),title:"Public search".into(),text_excerpt:"Public search form with a selection of navigation controls".into(),interactive_nodes:vec![node("wrong","Reservation Charts",Some("https://example.org/online-charts/")),node("login","Sign in",Some("https://example.org/login")),node("right","SQL INSERT",Some("https://example.org/sql-insert.html"))]}}
 #[test]fn journey_cannot_leave_form_or_click_unrelated_chart(){
  let g="Find trains from Bengaluru to New Delhi for 2026-11-24. Search only.";let p=page();let l=ObjectiveLedger::new(g);
  assert!(check(g,&p.origin,&p,&l,&BrowserAction::Navigate{url:"https://example.org/online-charts/".into()}).is_err());
  assert!(check(g,&p.origin,&p,&l,&BrowserAction::Click{node_id:"wrong".into(),element_label:"Search".into(),is_submit:false}).is_err());
 }
 #[test]fn explicit_sources_constrain_every_specialist_and_completion(){
  let g="Compare the official docs https://one.example/guide https://two.example/guide . Cite each source.";
  let l=ObjectiveLedger::new(g);let p=page();assert!(l.active());
  assert!(check(g,&p.origin,&p,&l,&BrowserAction::Navigate{url:"https://two.example/guide".into()}).is_ok());
  assert!(check(g,&p.origin,&p,&l,&BrowserAction::Navigate{url:"https://one.example/pricing".into()}).is_err());
  assert!(check(g,&p.origin,&p,&l,&BrowserAction::Complete{summary:"Done".into()}).is_err());
 }
 #[test]fn replacement_uses_ranked_observed_links_and_never_repeats(){
  let mut p=page();p.interactive_nodes.push(node("unrelated","Televisions",Some("https://example.org/televisions")));
  let g="Find the SQL INSERT reference and report its syntax.";let l=ObjectiveLedger::new(g);
  let mut planner=ReplacementPlanner::default();let mut routes=super::super::semantic_core::RouteLedger::default();routes.observe(0,&p.origin);
  assert!(matches!(planner.next(g,&p,&l,&routes),Some(BrowserAction::Navigate{url}) if url=="https://example.org/sql-insert.html"));
  assert!(planner.next(g,&p,&l,&routes).is_none());
  assert!(check(g,&p.origin,&p,&l,&BrowserAction::Navigate{url:"https://example.org/televisions".into()}).is_err());
 }
 #[test]fn generic_research_rejects_login_but_preserves_observed_relevant_links(){
  let p=page();let g="Find the SQL INSERT reference and report its syntax.";let l=ObjectiveLedger::new(g);
  assert!(check(g,&p.origin,&p,&l,&BrowserAction::Click{node_id:"login".into(),element_label:"SQL INSERT".into(),is_submit:false}).is_err());
  assert!(check(g,&p.origin,&p,&l,&BrowserAction::Click{node_id:"right".into(),element_label:"SQL INSERT".into(),is_submit:false}).is_ok());
  assert!(check(g,&p.origin,&p,&l,&BrowserAction::Navigate{url:"https://example.org/login".into()}).is_err());
 }
}
