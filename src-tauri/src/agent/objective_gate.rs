//! Final run-local objective constraints. This can only narrow native policy,
//! never grant a permission, submit a transaction, or certify a factual answer.
use super::{dom_extractor::{DomSnapshot,DomNodeSummary},policy::BrowserAction,semantic_core::ObjectiveLedger};
pub const REVISION:&str="lucia-approved-mcp-discovery-v13.11";
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

/// A first-result button may prepend a harmless action verb to the exact
/// requested subject ("Open robotics video" versus "robotics video").
/// Accept only a single fresh, non-sensitive result in visible search results.
/// This is a label match, not an exemption from live binding or click policy.
fn unique_first_result_label(goal:&str,page:&DomSnapshot,n:&DomNodeSummary,subject:&str)->bool{
    let g=goal.to_ascii_lowercase();
    if !g.contains("search for ")||!g.contains("first")||!g.contains("result")||
        !page.text_excerpt.to_ascii_lowercase().contains("search results")||
        n.is_submit||n.is_sensitive||
        !(matches!(n.tag.as_str(),"a"|"button")||matches!(n.role.as_str(),"link"|"button")){
        return false;
    }
    let target=words(subject);
    let label=words(&n.label);
    if target.is_empty()||label.len()!=target.len()+1||
        !matches!(label[0].as_str(),"open"|"view"|"watch"|"read")||
        label[1..]!=target[..]{
        return false;
    }
    page.interactive_nodes.iter().filter(|other|
        !other.is_sensitive&&!other.is_submit&&
        (matches!(other.tag.as_str(),"a"|"button")||matches!(other.role.as_str(),"link"|"button"))&&
        words(&other.label)==label
    ).count()==1
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
            if (node.tag=="input"&&(phrase(&label,"from")||phrase(&label,"to")||phrase(&label,"station")||phrase(&label,"date")||label.contains("dd/mm/yyyy")))
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
fn lookup_link_matches(subject:&str,label:&str,url:&str)->bool{
    let Ok(u)=url::Url::parse(url)else{return false;};
    let path=u.path().replace(['-','_','/']," ");
    let decoded=url::form_urlencoded::parse(path.as_bytes()).next().map(|(s,_)|s.into_owned()).unwrap_or_default();
    let exact=words(label)==words(subject)||phrase(&decoded,subject);
    let search=u.query_pairs().any(|(k,v)|matches!(k.as_ref(),"search"|"q"|"query")&&words(&v)==words(subject));
    exact||search
}
fn relevant_navigation(goal:&str,page:&DomSnapshot,url:&str)->bool{
    if off_topic(goal,url){return false}
    if super::catalog_research::entry_allowed(goal,url){return true;}
    if goal.split_whitespace().any(|s|s.trim_matches(['(',')','[',']','"', '\'',',','.',';'])==url){return true;}
    if super::policy::requested_navigation_url(goal).is_some_and(|u|u.as_str()==url){return true;}
    if super::policy::requested_search_entry_url(goal).is_some_and(|u|u.as_str()==url){return true;}
    if let Some(subject)=super::deliberation::lookup_subject(goal){return page.interactive_nodes.iter().any(|n|!n.is_sensitive&&!n.is_submit&&n.href.as_deref()==Some(url)&&lookup_link_matches(&subject,&n.label,url));}
    if super::skills::TaskSpec::compile(goal).and_then(|s|s.explicit_repository_url(&page.origin)).as_deref()==Some(url){return true;}
    if super::requirements::repository_fact_link(goal,page,url){return true;}
    if super::skills::TaskSpec::compile(goal).is_some_and(|spec|page.interactive_nodes.iter().any(|n|n.href.as_deref()==Some(url)&&spec.link_score_on(page,n)>0)){return true;}
    if super::policy::requested_search_entry_url(goal).is_some_and(|u|u.as_str()==url){return true;}
    page.interactive_nodes.iter().any(|n|!n.is_sensitive&&!n.is_submit&&n.href.as_deref()==Some(url)&&
        (super::semantic_core::relevance(goal,&format!("{} {}",n.label,url::Url::parse(url).map(|u|u.path().replace(['-','_','/']," ")).unwrap_or_default()))>0||
            docs_waypoint(goal,&n.label,url)||phrase(&n.label,"search")))
}
#[derive(Default)]
pub struct ReplacementPlanner{
    attempted:std::collections::HashSet<(String,String)>,
    typed:Option<(String,String)>,
    stage:Option<usize>,
}
impl ReplacementPlanner{
    pub fn set_stage(&mut self,stage:usize){
        if self.stage!=Some(stage){
            self.stage=Some(stage);self.attempted.clear();self.typed=None;
        }
    }
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
        self.next_for_stage(goal,goal,page,ledger,routes)
    }
    pub fn next_for_stage(&mut self,goal:&str,objective:&str,page:&DomSnapshot,ledger:&ObjectiveLedger,routes:&super::semantic_core::RouteLedger)->Option<BrowserAction>{
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
                let stage_score=super::semantic_core::stage_relevance(goal,objective,&format!("{} {path}",n.label)) as i32;
                let score=if exact{110}else{label_score*20+path_score*10+stage_score*35+if docs_waypoint(goal,&n.label,url){35}else{0}};
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
    if super::skills::TaskSpec::compile(goal).is_some_and(|s|s.kind==super::skills::TargetKind::Reference){
        let target=match action{
            BrowserAction::Navigate{url}=>Some(url.as_str()),
            BrowserAction::Click{node_id,..}=>control(page,node_id).and_then(|n|n.href.as_deref()),
            _=>None
        };
        if target.is_some_and(|u|!super::reference_track::reference_track_allowed(page,u)){
            return Err("Reference navigation would leave the observed documentation release");
        }
    }
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
            if let Some(subject)=super::deliberation::lookup_subject(goal){
                let exact=words(&n.label)==words(&subject)||n.href.as_ref().is_some_and(|u|lookup_link_matches(&subject,&n.label,u))||unique_first_result_label(goal,page,n,&subject);
                let search_control=search_field(n)||phrase(&n.label,"search");
                if !exact&&!search_control{return Err("Named lookup must use its search control or exact requested result");}
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
    #[test]fn named_first_result_opener_passes_without_relaxing_lookup_safety(){
        let goal="On the current local test page, search for robotics video and open the first result.";
        let mut p=page();
        p.text_excerpt="Robotics video search results are now visible.".into();
        p.interactive_nodes=vec![
            node("result","Open robotics video",None),
            node("wrong","Open robotics kit video",None),
        ];
        let ledger=ObjectiveLedger::new(goal);
        let action=|id:&str|BrowserAction::Click{node_id:id.into(),element_label:"Untrusted model label".into(),is_submit:false};
        assert!(unique_first_result_label(goal,&p,&p.interactive_nodes[0],"robotics video"));
        assert!(check(goal,&p.origin,&p,&ledger,&action("result")).is_ok(),
            "An exact-subject first-result opener must survive the objective gate");
        assert!(check(goal,&p.origin,&p,&ledger,&action("wrong")).is_err());
        p.interactive_nodes[0].is_sensitive=true;
        assert!(check(goal,&p.origin,&p,&ledger,&action("result")).is_err());
        p.interactive_nodes[0].is_sensitive=false;
        p.interactive_nodes[0].is_submit=true;
        assert!(check(goal,&p.origin,&p,&ledger,&action("result")).is_err());
        p.interactive_nodes[0].is_submit=false;
        p.interactive_nodes.push(node("duplicate","Open robotics video",None));
        assert!(check(goal,&p.origin,&p,&ledger,&action("result")).is_err());
        p.interactive_nodes.pop();
        p.text_excerpt="The search form is awaiting a query.".into();
        assert!(check(goal,&p.origin,&p,&ledger,&action("result")).is_err());
    }
 fn node(id:&str,label:&str,href:Option<&str>)->DomNodeSummary{DomNodeSummary{node_id:id.into(),tag:"a".into(),role:"link".into(),label:label.into(),href:href.map(str::to_owned),options:vec![],checked:None,search_state:None,is_sensitive:false,is_submit:false,bbox:None}}
 fn page()->DomSnapshot{DomSnapshot{structure: Default::default(), origin:"https://example.org/search".into(),title:"Public search".into(),text_excerpt:"Public search form with a selection of navigation controls".into(),interactive_nodes:vec![node("wrong","Reservation Charts",Some("https://example.org/online-charts/")),node("login","Sign in",Some("https://example.org/login")),node("right","SQL INSERT",Some("https://example.org/sql-insert.html"))]}}
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
 #[test]fn recovery_prefers_current_objective_and_keeps_attempts_stage_scoped(){
  let mut p=page();
  p.interactive_nodes=vec![node("first","Installation",Some("https://example.org/installation")),node("second","Compatibility",Some("https://example.org/compatibility"))];
  let goal="Research installation and compatibility details";let l=ObjectiveLedger::new(goal);
  let mut planner=ReplacementPlanner::default();let mut routes=super::super::semantic_core::RouteLedger::default();routes.observe(0,&p.origin);
  planner.set_stage(0);
  assert!(matches!(planner.next_for_stage(goal,"Read compatibility",&p,&l,&routes),Some(BrowserAction::Navigate{url}) if url.ends_with("/compatibility")));
  assert!(matches!(planner.next_for_stage(goal,"Read compatibility",&p,&l,&routes),Some(BrowserAction::Navigate{url}) if url.ends_with("/installation")));
  assert!(planner.next_for_stage(goal,"Read compatibility",&p,&l,&routes).is_none());
  planner.set_stage(1);routes.observe(1,&p.origin);
  assert!(matches!(planner.next_for_stage(goal,"Read installation",&p,&l,&routes),Some(BrowserAction::Navigate{url}) if url.ends_with("/installation")));
 }
 #[test]fn named_lookup_rejects_irrelevant_but_observed_navigation(){
  let mut p=page();
  p.interactive_nodes.push(node("correct","Grace Hopper",Some("https://example.org/wiki/Grace_Hopper")));
  p.interactive_nodes.push(node("wrong_wiki","Article size",Some("https://example.org/wiki/Article_size")));
  let g="Search for Grace Hopper and open the corresponding article. Report its page title.";
  let l=ObjectiveLedger::new(g);
  assert!(check(g,&p.origin,&p,&l,&BrowserAction::Navigate{url:"https://example.org/wiki/Article_size".into()}).is_err());
  assert!(check(g,&p.origin,&p,&l,&BrowserAction::Click{node_id:"wrong_wiki".into(),element_label:"Article size".into(),is_submit:false}).is_err());
  assert!(check(g,&p.origin,&p,&l,&BrowserAction::Navigate{url:"https://example.org/wiki/Grace_Hopper".into()}).is_ok());
 }
 #[test]fn generic_research_rejects_login_but_preserves_observed_relevant_links(){
  let p=page();let g="Find the SQL INSERT reference and report its syntax.";let l=ObjectiveLedger::new(g);
  assert!(check(g,&p.origin,&p,&l,&BrowserAction::Click{node_id:"login".into(),element_label:"SQL INSERT".into(),is_submit:false}).is_err());
  assert!(check(g,&p.origin,&p,&l,&BrowserAction::Click{node_id:"right".into(),element_label:"SQL INSERT".into(),is_submit:false}).is_ok());
  assert!(check(g,&p.origin,&p,&l,&BrowserAction::Navigate{url:"https://example.org/login".into()}).is_err());
 }
}
