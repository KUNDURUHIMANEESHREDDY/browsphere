//! Run-scoped prerequisites for browser workflows. Native observations and
//! explicit user values only; never authorize a click or claim task completion.
use super::{dom_extractor::{DomSnapshot, DomNodeSummary}, policy::BrowserAction};
use std::{collections::HashSet, hash::{Hash, Hasher}};
use chrono::NaiveDate;

#[derive(Default)]
pub struct PrerequisitePlanner {
    attempted: HashSet<(String,String,u64)>,
    route_origin:Option<String>,
    route_confirmed:[bool;2],
    route_attempts:[u8;2],
    route_search_sent:bool,
}
fn click(n: &DomNodeSummary) -> BrowserAction {
    BrowserAction::Click {node_id:n.node_id.clone(),element_label:n.label.clone(),is_submit:n.is_submit}
}
fn unique<'a>(nodes: impl Iterator<Item=&'a DomNodeSummary>) -> Option<&'a DomNodeSummary> {
    let mut it=nodes;
    let first=it.next()?;
    if it.next().is_some() {None} else {Some(first)}
}
fn safe(n:&DomNodeSummary)->bool {
    !n.is_sensitive && !n.is_submit && n.bbox.as_ref().map_or(true,|b| b.in_viewport)
}
fn normalized(s:&str)->String {s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()}
/// Documentation sidebars frequently prepend chapter numbers to accessible
/// labels. Strip only a decimal chapter prefix, never words or arbitrary text.
fn chapter_label(s:&str)->String{
    let label=normalized(s);
    if let Some((prefix,rest))=label.split_once(' '){
        if prefix.ends_with('.') && prefix.chars().any(|c|c.is_ascii_digit()) &&
            prefix.chars().all(|c|c.is_ascii_digit()||c=='.') {
            return rest.trim().to_string();
        }
    }
    label
}
/// Explicit read-only dropdown constraints, not a generic form submission API.
/// Exact quoted field/option pairs are user-provided; ambiguous fields fail closed.
pub fn filter_constraints(goal:&str)->Option<Vec<(String,String)>>{
    let mut clauses=goal.trim().trim_end_matches('.').split(". ");
    if !clauses.next()?.eq_ignore_ascii_case("Filter results only"){return None;}
    let mut out=Vec::new();
    let mut finished=false;
    for clause in clauses {
        if clause.eq_ignore_ascii_case("Report the selected filters"){finished=true;continue;}
        if finished{return None;}
        let rest=clause.strip_prefix("Set \"")?;
        let (field,value)=rest.split_once("\" to \"")?;
        let value=value.strip_suffix('"')?;
        let valid=|s:&str| !s.trim().is_empty()&&s.chars().count()<=80&&s.chars().all(|c|c.is_alphanumeric()||matches!(c,' '|'-'|'_'|'/'));
        let unsafe_words=["password","account","payment","consent","terms","privacy","delete","purchase","subscribe","transfer","publish","credential","address","email","phone","identity","secret"];
        if !valid(field)||!valid(value)||unsafe_words.iter().any(|w|normalized(field).contains(w)||normalized(value).contains(w))
            ||out.iter().any(|(f,_):&(String,String)|normalized(f)==normalized(field)){return None;}
        out.push((field.into(),value.into()));
        if out.len()>6{return None;}
    }
    (finished&& !out.is_empty()).then_some(out)
}
fn filter_field<'a>(page:&'a DomSnapshot,field:&str)->Option<&'a DomNodeSummary>{
    super::browser_skills::form::public_select(page,field)
}
fn selected(n:&DomNodeSummary,value:&str)->bool{
    super::browser_skills::form::selected(n,value)
}
pub fn filters_verified(goal:&str,page:&DomSnapshot)->bool{
    let Some(pairs)=filter_constraints(goal) else{return false};
    !page.text_excerpt.starts_with("Active dialog:")&&!super::deliberation::error_page(page)&&
        pairs.iter().all(|(field,value)|filter_field(page,field).is_some_and(|n|selected(n,value)))
}
pub fn filter_action_allowed(goal:&str,page:&DomSnapshot,action:&BrowserAction)->bool{
    match action{
        BrowserAction::Complete{..}|BrowserAction::Scroll{..}=>true,
        BrowserAction::SelectOption{node_id,option_label}=>filter_constraints(goal).is_some_and(|pairs|pairs.iter().any(|(field,value)|
            normalized(value)==normalized(option_label)&&filter_field(page,field).is_some_and(|n|n.node_id==*node_id&&!selected(n,value)&&n.options.iter().filter(|o|normalized(o)==normalized(value)).count()==1))),
        _=>false
    }
}
fn next_filter(goal:&str,page:&DomSnapshot)->Option<BrowserAction>{
    let pairs=filter_constraints(goal)?;
    for (field,value) in pairs{
        let n=filter_field(page,&field)?;
        if selected(n,&value){continue;}
        if !safe(n){return None;}
        return super::browser_skills::form::select_action(n,&value).ok();
    }
    None
}
pub fn language(goal:&str,page:&DomSnapshot)->Option<BrowserAction> {
    let g=normalized(goal);
    let excerpt=normalized(&page.text_excerpt);
    if !excerpt.starts_with("active dialog:") ||
        !["preferred language","choose language","select language","select your language"]
          .iter().any(|s|excerpt.contains(s)) {return None;}
    let requested=["english","hindi","telugu","kannada","tamil","french","spanish","german"]
      .iter().find(|&&l| ["choose ","select ","use "].iter().any(|p|g.contains(&format!("{p}{l}"))))?;
    // This is never used to dismiss consent, authentication or legal dialogs.
    if ["consent","terms and conditions","privacy policy","captcha","sign in","payment"]
        .iter().any(|s|excerpt.contains(s)) {return None;}
    unique(page.interactive_nodes.iter().filter(|n|!n.is_sensitive
        && n.bbox.as_ref().map_or(true,|b|b.in_viewport) && n.role=="button"
        && normalized(&n.label)==*requested)).map(click)
}
pub fn route_values(goal:&str)->Option<(&str,&str)> {
    let lower=goal.to_ascii_lowercase();
    // Only opt-in, read-only route searches. No account, purchase or write workflow.
    if !lower.contains("search only") || !lower.contains("trains from ") {return None;}
    let start=lower.find("trains from ")?+"trains from ".len();
    let middle=start+lower[start..].find(" to ")?;
    let end=middle+4+lower[middle+4..].find(" for ")?;
    let (a,b)=(goal[start..middle].trim(),goal[middle+4..end].trim());
    let valid=|s:&str| !s.is_empty() && s.chars().count()<=60
        && s.chars().all(|c|c.is_alphabetic()||c==' '||c=='-'||c=='.');
    if valid(a)&&valid(b) {Some((a,b))} else {None}
}
pub(super) fn requested_date(goal:&str)->Option<NaiveDate> {
    goal.split(|c:char| !c.is_ascii_digit() && c!='-')
        .find_map(|token|NaiveDate::parse_from_str(token,"%Y-%m-%d").ok())
}
pub fn date_value_allowed(goal:&str,value:&str)->bool{
    requested_date(goal).is_some_and(|d|date_labels(d).iter().any(|s|s==&normalized(value)))
}
pub fn observed_date_choice(goal:&str,label:&str)->bool{date_value_allowed(goal,label)}
fn date_labels(date:NaiveDate)->Vec<String> {
    ["%Y-%m-%d","%d/%m/%Y","%d %B %Y","%B %d, %Y","%d %b %Y","%A, %B %d, %Y"]
        .iter().map(|format|normalized(&date.format(format).to_string())).collect()
}
/// A search-only task cannot complete on the original form or a modal.
pub fn completion_blocker(goal:&str,page:&DomSnapshot)->Option<&'static str> {
    if filter_constraints(goal).is_some() {
        return (!filters_verified(goal,page)).then_some("Requested filter selections have not all been observed");
    }
    let (from,to)=route_values(goal)?;
    let text=normalized(&page.text_excerpt);
    if text.starts_with("active dialog:") {return Some("Resolve the blocking dialog first");}
    use super::page_understanding::{classify,transport_header,PageKind};
    let profile=classify(page);
    if !matches!(profile.kind,PageKind::TransportResults|PageKind::EmptyResults){
        return Some("No observed structured train search results; the requested route and date are not verified");
    }
    let Some(header)=transport_header(page)else{return Some("Missing visible result heading for the requested journey")};
    let header=normalized(&header);
    if !super::browser_skills::phrase(&header,from)||!super::browser_skills::phrase(&header,to){
        return Some("Observed result heading does not identify both requested cities");
    }
    if let Some(date)=requested_date(goal) {
        if !date_labels(date).iter().any(|label|header.contains(label)) {
            return Some("Observed result heading does not identify the requested journey date");
        }
    }
    None
}
/// The user explicitly requested clicking a named link and reporting its title.
pub fn named_link(goal:&str,page:&DomSnapshot)->Option<(BrowserAction,String)> {
    let label=super::evidence_verifier::requested_named_link_title(goal)?;
    let g=normalized(goal);
    if !g.contains("report the page title") && !g.contains("report its page title"){return None;}
    if ["buy ","purchase","checkout","pay ","book ","submit","register","sign up","send ","delete","download","upload","compare","extract","summarize","account","consent","subscribe"," and then "]
        .iter().any(|s|g.contains(s)){return None;}
    if page.text_excerpt.starts_with("Active dialog:"){return None;}
    let source=url::Url::parse(&page.origin).ok()?;
    let mut candidates=page.interactive_nodes.iter().filter_map(|n|{
        if n.is_sensitive||n.is_submit|| !(n.tag=="a"||n.role=="link")||chapter_label(&n.label)!=normalized(&label){return None;}
        let u=url::Url::parse(n.href.as_deref()?).ok()?;
        if !matches!(u.scheme(),"http"|"https")||u.origin()!=source.origin()||u==source
            ||!u.username().is_empty()||u.password().is_some()||u.fragment().is_some(){return None;}
        Some((n,u))
    });
    let (first,target)=candidates.next()?;
    if candidates.any(|(_,u)|u!=target){return None;}
    Some((click(first),target.to_string()))
}
pub fn named_search_control(goal:&str,page:&DomSnapshot)->Option<BrowserAction>{
    if (!super::orchestrator::is_simple_named_title_task(goal)&&super::deliberation::lookup_subject(goal).is_none())||page.text_excerpt.starts_with("Active dialog:"){return None;}
    if page.interactive_nodes.iter().any(|n|!n.is_sensitive&&n.tag=="input"&&(n.role=="searchbox"||normalized(&n.label).contains("search"))){return None;}
    let origin=url::Url::parse(&page.origin).ok()?;
    let site=origin.host_str()?.split('.').filter(|s|*s!="www"&&*s!="en").rev().nth(1).unwrap_or_default().to_lowercase();
    unique(page.interactive_nodes.iter().filter(|n|{
        if !safe(n)||!matches!(n.role.as_str(),"button"|"link"|"a")&&n.tag!="a"{return false;}
        // Native accessibility names may append a keyboard shortcut, e.g. [f].
        let full=normalized(&n.label);
        let label=full.rsplit_once(" [").filter(|(_,key)|key.ends_with(']')&&key.len()<=16)
            .map(|(text,_)|text).unwrap_or(&full);
        let exact=matches!(label,"search"|"open search")||(!site.is_empty()&&label==format!("search {site}"));
        exact && n.href.as_deref().map_or(true,|href|url::Url::parse(href).ok()
            .is_some_and(|u|u.origin()==origin.origin()&&u.username().is_empty()&&u.password().is_none()))
    })).map(click)
}
pub fn named_result(goal:&str,page:&DomSnapshot)->Option<BrowserAction>{
    if page.text_excerpt.starts_with("Active dialog:"){return None;}
    let wanted=normalized(&if super::orchestrator::is_simple_named_title_task(goal){
        super::evidence_verifier::requested_named_result_title(goal)
    }else{super::deliberation::lookup_subject(goal)}?);
    let origin=url::Url::parse(&page.origin).ok()?;
    unique(page.interactive_nodes.iter().filter(|n|{
        safe(n)&&(n.tag=="a"||n.role=="link"||n.role=="option")&&normalized(&n.label)==wanted&&
        n.href.as_ref().and_then(|s|url::Url::parse(s).ok()).is_some_and(|u|u.origin()==origin.origin()&&u!=origin&&u.username().is_empty()&&u.password().is_none())
    })).map(click)
}
pub fn route_field<'a>(page:&'a DomSnapshot,index:usize)->Option<&'a DomNodeSummary>{
    let directions=if index==0{["from","origin","departure"]}else{["to","destination","arrival"]};
    unique(page.interactive_nodes.iter().filter(|n|{
        if !safe(n)||n.tag!="input"||!matches!(n.role.as_str(),"input"|"searchbox"|"combobox"|"textbox"){return false;}
        // Accessibility names often include a separate instruction sentence.
        // Interpret the role and first semantic label, not the exact whole string.
        let label=normalized(&n.label);
        let first=label.split(['.',':',';','\n']).next().unwrap_or("").trim();
        let first=first.strip_prefix("enter ").or_else(||first.strip_prefix("select ")).unwrap_or(first);
        directions.iter().any(|d|first==*d||[" station"," city"," location"].iter().any(|suffix|first==format!("{d}{suffix}")))
    }))
}
fn matching_route_options<'a>(page:&'a DomSnapshot,value:&str)->Vec<&'a DomNodeSummary>{
    super::browser_skills::autocomplete::matches(page,value)
}
/// Explicitly permitted city-or-station searches may select a station from
/// the site's observed city matches. Prefer an exact city station label,
/// otherwise the first complete station record in the site's own order.
/// Arbitrary ambiguous labels still fail closed. This is one station pair,
/// not an assertion of city-wide coverage.
fn choose_route_option<'a>(goal:&str,page:&'a DomSnapshot,value:&str)->Option<&'a DomNodeSummary>{
    if let Ok(n)=super::browser_skills::autocomplete::choose(page,value){return Some(n);}
    if !goal.to_ascii_lowercase().contains("city or station suggestions"){return None;}
    let options=matching_route_options(page,value);
    let records=options.into_iter().filter(|n|{
        let Some((name,tail))=n.label.split_once(" - ")else{return false;};
        let code=tail.split_whitespace().next().unwrap_or("");
        !name.trim().is_empty()&&(2..=6).contains(&code.len())&&code.chars().all(|c|c.is_ascii_uppercase()||c.is_ascii_digit())
    }).collect::<Vec<_>>();
    records.iter().find(|n|n.label.split_once(" - ").is_some_and(|(name,_)|normalized(name)==normalized(value))).copied().or_else(||records.first().copied())
}
fn route_date_readback(goal:&str,page:&DomSnapshot)->bool{
    let Some(date)=requested_date(goal)else{return false;};
    let expected=format!("public_date:{}",date.format("%Y-%m-%d"));
    route_field(page,0).is_some()&&route_field(page,1).is_some()&&
        page.interactive_nodes.iter().any(|n|safe(n)&&n.tag=="input"&&n.search_state.as_deref()==Some(expected.as_str()))
}
pub fn public_route_search(goal:&str,page:&DomSnapshot,action:&BrowserAction)->bool{
    if route_values(goal).is_none()||page.text_excerpt.starts_with("Active dialog:"){return false;}
    matches!(action,BrowserAction::Click{node_id,..} if page.interactive_nodes.iter().any(|n|
        n.node_id==*node_id&&!n.is_sensitive&&n.href.is_none()&&n.role=="button"&&matches!(normalized(&n.label).as_str(),"search trains"|"find trains")))
}
impl PrerequisitePlanner {
    pub fn observe_route_action(&mut self,goal:&str,before:&DomSnapshot,after:&DomSnapshot,action:&BrowserAction){
        let Some((from,to))=route_values(goal)else{return;};
        if before.origin!=after.origin||after.text_excerpt.starts_with("Active dialog:"){return;}
        let BrowserAction::Click{node_id,..}=action else{return;};
        let Some(option)=before.interactive_nodes.iter().find(|n|n.node_id==*node_id&&n.role=="option")else{return;};
        // Confirm a public matching option was actually clicked, the popup
        // resolved, and its corresponding input is still populated.
        if after.interactive_nodes.iter().any(|n|n.role=="option"){return;}
        for (i,value) in [from,to].iter().enumerate(){
            if !self.route_confirmed[i]&&super::browser_skills::autocomplete::receipt(before,after,&option.node_id,value,route_field(after,i))==super::browser_skills::Outcome::Confirmed{
                self.route_confirmed[i]=true;
                self.route_attempts[i]=0;
                break;
            }
        }
    }
    pub fn route_search_ready(&self,goal:&str,page:&DomSnapshot)->bool{
        !self.route_search_sent&&self.route_origin.as_deref()==Some(page.origin.as_str())&&self.route_confirmed==[true,true]&&
            (0..2).all(|i|route_field(page,i).is_some_and(|n|n.search_state.as_deref()==Some("filled")))&&
            route_date_readback(goal,page)&&!page.interactive_nodes.iter().any(|n|n.role=="option")
    }
    pub fn mark_route_search(&mut self){self.route_search_sent=true;}
    pub fn route_search_sent(&self)->bool{self.route_search_sent}
    pub fn next(&mut self,goal:&str,page:&DomSnapshot)->Option<(BrowserAction,&'static str)> {
        let excerpt=normalized(&page.text_excerpt);
        let proposal=if excerpt.starts_with("active dialog:") {
            language(goal,page).map(|a|(a,"explicit_language"))
        } else if filter_constraints(goal).is_some(){
            next_filter(goal,page).map(|a|(a,"explicit_filter"))
        } else if let Some((from,to))=route_values(goal) {
            if self.route_search_sent{return None;}
            if self.route_origin.as_deref()!=Some(page.origin.as_str()){
                self.route_origin=Some(page.origin.clone());self.route_confirmed=[false;2];self.route_attempts=[0;2];
            }
            let (Some(a),Some(b))=(route_field(page,0),route_field(page,1)) else {return None;};
            for (i,n) in [a,b].iter().enumerate(){
                if n.search_state.as_deref()==Some("empty"){self.route_confirmed[i]=false;}
            }
            if let Some(i)=(0..2).find(|i|!self.route_confirmed[*i]){
                let n=[a,b][i];let value=[from,to][i];
                let options=matching_route_options(page,value);
                let option=choose_route_option(goal,page,value);
                if let Some(n)=option{
                    Some((click(n),if i==0{"origin_option"}else{"destination_option"}))
                }else if !options.is_empty()||page.interactive_nodes.iter().any(|n|n.role=="option"){
                    // Multiple stations or unrelated suggestions: do not guess.
                    None
                }else if self.route_attempts[i]<3{
                    self.route_attempts[i]+=1;
                    if n.search_state.as_deref()==Some("filled")&&self.route_attempts[i]==2{
                        Some((click(n),if i==0{"reopen_origin_options"}else{"reopen_destination_options"}))
                    }else{
                        Some((BrowserAction::TypeText{node_id:n.node_id.clone(),text:value.into(),is_sensitive:false},if i==0{"origin_input"}else{"destination_input"}))
                    }
                }else{None}
            }else if self.route_search_ready(goal,page){
                unique(page.interactive_nodes.iter().filter(|n|!n.is_sensitive&&n.href.is_none()&&n.role=="button"&&matches!(normalized(&n.label).as_str(),"search trains"|"find trains")))
                    .map(|n|(click(n),"public_route_search"))
            }else if let Some(date)=requested_date(goal){
                let labels=date_labels(date);
                super::browser_skills::form::date_action(page,&labels).ok().and_then(|a|{
                    let BrowserAction::Click{node_id,..}=&a else{return None;};
                    let n=page.interactive_nodes.iter().find(|n|n.node_id==*node_id&&safe(n))?;
                    Some((a,if n.tag=="input"{"open_date_picker"}else{"exact_calendar_day"}))
                })
            }else{None}
        } else if let Some((action,_))=named_link(goal,page) {
            Some((action,"named_link"))
        } else if let Some(action)=named_search_control(goal,page) {
            Some((action,"open_named_search"))
        } else {None};
        let (action,phase)=proposal?;
        // Attempt each semantic prerequisite at most once per observed page
        // state. No persistent storage of entered text or secrets.
        let mut state=std::collections::hash_map::DefaultHasher::new();
        excerpt.hash(&mut state);
        for node in &page.interactive_nodes {
            node.label.hash(&mut state);node.search_state.hash(&mut state);
        }
        let retry_phase=if matches!(phase,"origin_input"|"reopen_origin_options"){format!("{phase}:{}",self.route_attempts[0])}
            else if matches!(phase,"destination_input"|"reopen_destination_options"){format!("{phase}:{}",self.route_attempts[1])}else{phase.to_string()};
        let key=(page.origin.clone(),retry_phase,state.finish());
        if !self.attempted.insert(key) {return None;}
        Some((action,phase))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn node(id:&str,tag:&str,role:&str,label:&str,state:Option<&str>)->DomNodeSummary {
        DomNodeSummary{node_id:id.into(),tag:tag.into(),role:role.into(),label:label.into(),
        search_state:state.map(str::to_string),options:vec![],checked:None,href:None,
        is_submit:false,is_sensitive:false,bbox:None}
    }
    fn page(text:&str,nodes:Vec<DomNodeSummary>)->DomSnapshot {
        DomSnapshot{structure: Default::default(), origin:"https://example.org".into(),title:"Test".into(),text_excerpt:text.into(),interactive_nodes:nodes}
    }
    const GOAL:&str="Test a train booking search only. Choose English if a language dialog appears. Find trains from Bengaluru to New Delhi for 2026-11-24.";
    #[test]
    fn search_opener_accepts_observed_link_and_access_key(){
        let goal="On this site, search for Ada Lovelace and open the corresponding article. Then report its page title.";
        let mut n=node("search","a","a","Search Example [f]",None);
        n.href=Some("https://example.org/search".into());
        let mut p=page("Public homepage",vec![n]);
        assert!(matches!(named_search_control(goal,&p),Some(BrowserAction::Click{node_id,..}) if node_id=="search"));
        p.interactive_nodes[0].href=Some("https://other.example/search".into());
        assert!(named_search_control(goal,&p).is_none());
    }
    #[test]
    fn numbered_documentation_link_can_be_grounded(){
        let mut n=node("chapter","a","link","1. Getting Started",None);
        n.href=Some("https://example.org/stable/book/ch01.html".into());
        let p=page("Documentation contents",vec![n]);
        assert!(named_link("Click the Getting Started link. Then report the page title.",&p).is_some());
        assert!(named_link("Click the Started link. Then report the page title.",&p).is_none());
    }
    #[test]
    fn explicit_filters_require_exact_distinct_fields_and_readback(){
        let g="Filter results only. Set \"Category\" to \"Books\". Set \"Format\" to \"Text\". Report the selected filters.";
        let mut a=node("a","select","combobox","Category",Some("selected_option:All"));
        a.options=vec!["All".into(),"Books".into()];
        let mut b=node("b","select","combobox","Format",Some("selected_option:PDF"));
        b.options=vec!["PDF".into(),"Text".into()];
        let mut p=page("Public catalog filters",vec![a,b]);
        let mut planner=PrerequisitePlanner::default();
        assert!(matches!(planner.next(g,&p),Some((BrowserAction::SelectOption{node_id,option_label},"explicit_filter")) if node_id=="a"&&option_label=="Books"));
        p.interactive_nodes[0].search_state=Some("selected_option:Books".into());
        assert!(matches!(planner.next(g,&p),Some((BrowserAction::SelectOption{node_id,option_label},"explicit_filter")) if node_id=="b"&&option_label=="Text"));
        assert!(!filters_verified(g,&p));
        p.interactive_nodes[1].search_state=Some("selected_option:Text".into());
        assert!(filters_verified(g,&p));
        p.interactive_nodes[1].is_sensitive=true;
        assert!(!filters_verified(g,&p));
        assert!(filter_constraints("Filter results only. Set \"Payment\" to \"Approve\". Report the selected filters.").is_none());
        assert!(filter_constraints("Filter results only. Set \"Category\" to \"Books\". Report the selected filters. Submit").is_none());
    }
    #[test]
    fn named_link_is_unique_same_origin_and_exact(){
        let goal="Click the Tutorial link. Then report the page title.";
        let mut n=node("tutorial","a","link","Tutorial",None);
        n.href=Some("https://example.org/tutorial/index.html".into());
        let mut p=page("Documentation homepage",vec![n.clone()]);
        assert_eq!(named_link(goal,&p).unwrap().1,"https://example.org/tutorial/index.html");
        n.node_id="wrong".into();n.href=Some("https://example.org/tutorial/other.html".into());p.interactive_nodes.push(n.clone());
        assert!(named_link(goal,&p).is_none());
        p.interactive_nodes.remove(0);p.interactive_nodes[0].href=Some("https://other.example/tutorial".into());
        assert!(named_link(goal,&p).is_none());
        assert!(named_link("Click the Tutorial link and buy a subscription. Then report the page title.",&p).is_none());
    }
    #[test]
    fn readonly_route_search_routes_to_browser_before_verifier() {
        use super::super::orchestrator::{plan_task,Specialist};
        let plan=plan_task(GOAL);
        assert_eq!(plan.stages.iter().map(|s|s.specialist).collect::<Vec<_>>(),
            vec![Specialist::Browser,Specialist::Verification]);
    }
    #[test]
    fn language_prerequisite_is_explicit_and_bounded() {
        let p=page("Active dialog: Please select your preferred language.",vec![node("h","button","button","Hindi",None),node("e","button","button","English",None)]);
        let mut planner=PrerequisitePlanner::default();
        assert!(matches!(planner.next(GOAL,&p),Some((BrowserAction::Click{node_id,..},"explicit_language")) if node_id=="e"));
        assert!(planner.next(GOAL,&p).is_none());
        assert!(PrerequisitePlanner::default().next("Find trains",&p).is_none());
        let mut blocked=p.clone();blocked.text_excerpt.push_str(" consent");
        assert!(PrerequisitePlanner::default().next(GOAL,&blocked).is_none());
    }
    #[test]
    fn explicit_language_submit_retains_submit_flag_and_rejects_sensitive_control() {
        let mut n=node("e","button","button","English",None);
        n.is_submit=true;
        let mut p=page("Active dialog: Please select your preferred language.",vec![n]);
        assert!(matches!(language(GOAL,&p),Some(BrowserAction::Click{is_submit:true,..})));
        p.interactive_nodes[0].is_sensitive=true;
        assert!(language(GOAL,&p).is_none());
    }
    #[test]
    fn exact_date_selection_requires_observed_full_date() {
        let p=page("Calendar November 2026",vec![
            node("a","input","searchbox","From station",Some("filled")),
            node("b","input","searchbox","To station",Some("filled")),
            node("date","button","button","November 24, 2026",None),
            node("other","button","button","November 25, 2026",None)]);
        let mut planner=PrerequisitePlanner::default();
        planner.route_origin=Some(p.origin.clone());planner.route_confirmed=[true,true];
        assert!(matches!(planner.next(GOAL,&p),
            Some((BrowserAction::Click{node_id,..},"exact_calendar_day")) if node_id=="date"));
        assert!(completion_blocker(GOAL,&p).is_some());
        let results=page("Refine Results 1 Results for Bengaluru New Delhi | 24/11/2026 For Quota | General Sort By Express (12345) Runs On: daily Train Schedule 18:30 Bengaluru 08:10 New Delhi",vec![]);
        assert!(completion_blocker(GOAL,&results).is_none());
    }
    #[test]
    fn verbose_accessible_route_labels_are_recognized(){
        let p=page("Public journey form",vec![node("a","input","searchbox","Enter From station. Input is Mandatory.",Some("empty")),node("b","input","searchbox","Enter To station. Input is Mandatory.",Some("empty"))]);
        assert_eq!(route_field(&p,0).unwrap().node_id,"a");assert_eq!(route_field(&p,1).unwrap().node_id,"b");
        assert!(matches!(PrerequisitePlanner::default().next(GOAL,&p),Some((BrowserAction::TypeText{node_id,..},"origin_input")) if node_id=="a"));
    }
    #[test]
    fn city_search_uses_observed_station_records_only_with_explicit_permission(){
        let mut p=page("Public journey form",vec![node("a","input","combobox","From",Some("filled")),node("b","input","searchbox","To",Some("empty"))]);
        p.interactive_nodes.push(node("c","li","option","KSR BENGALURU - SBC KARNATAKA",None));
        p.interactive_nodes.push(node("d","li","option","BENGALURU EAST - BNCE KARNATAKA",None));
        let goal=format!("{GOAL} Select the city or station suggestions.");
        assert!(matches!(PrerequisitePlanner::default().next(&goal,&p),Some((BrowserAction::Click{node_id,..},"origin_option")) if node_id=="c"));
        assert!(choose_route_option(GOAL,&p,"Bengaluru").is_none());
    }
    #[test]
    fn public_route_search_requires_two_receipts_and_exact_date(){
        let p=page("Public journey form",vec![node("a","input","searchbox","From",Some("filled")),node("b","input","searchbox","To",Some("filled")),node("d","input","input","Journey date",Some("public_date:2026-11-24")),node("s","button","button","Search Trains",None)]);
        let mut planner=PrerequisitePlanner::default();planner.route_origin=Some(p.origin.clone());
        assert!(!planner.route_search_ready(GOAL,&p));planner.route_confirmed=[true,true];
        assert!(planner.route_search_ready(GOAL,&p));
        assert!(matches!(planner.next(GOAL,&p),Some((BrowserAction::Click{node_id,..},"public_route_search")) if node_id=="s"));
        planner.mark_route_search();assert!(!planner.route_search_ready(GOAL,&p));assert!(planner.next(GOAL,&p).is_none());
    }
    #[test]
    fn ambiguous_autocomplete_never_advances_to_the_next_field(){
        let mut p=page("Public journey form",vec![node("a","input","combobox","From",Some("filled")),node("b","input","textbox","To",Some("empty"))]);
        let mut planner=PrerequisitePlanner::default();
        p.interactive_nodes.push(node("c","li","option","Bengaluru City",None));
        p.interactive_nodes.push(node("d","li","option","Bengaluru East",None));
        assert!(planner.next(GOAL,&p).is_none());
        p.interactive_nodes.truncate(2);
        assert!(matches!(planner.next(GOAL,&p),Some((BrowserAction::TypeText{node_id,..},"origin_input")) if node_id=="a"));
        assert!(!planner.route_confirmed[0]);
    }
    #[test]
    fn route_fields_use_explicit_values_and_never_submit() {
        let mut p=page("Book ticket",vec![node("a","input","searchbox","Enter From station",Some("empty")),
            node("b","input","searchbox","Enter To station",Some("empty")),node("s","button","button","Search Trains",None)]);
        let mut planner=PrerequisitePlanner::default();
        assert!(matches!(planner.next(GOAL,&p),Some((BrowserAction::TypeText{node_id,text,..},"origin_input")) if node_id=="a"&&text=="Bengaluru"));
        p.interactive_nodes[0].search_state=Some("filled".into());
        p.interactive_nodes.push(node("c","li","option","Bengaluru City",None));
        assert!(matches!(planner.next(GOAL,&p),Some((BrowserAction::Click{node_id,..},"origin_option")) if node_id=="c"));
        let before=p.clone();
        p.interactive_nodes.pop();
        planner.observe_route_action(GOAL,&before,&p,&BrowserAction::Click{node_id:"c".into(),element_label:"Bengaluru City".into(),is_submit:false});
        assert!(planner.route_confirmed[0]);
        assert!(matches!(planner.next(GOAL,&p),Some((BrowserAction::TypeText{node_id,text,..},"destination_input")) if node_id=="b"&&text=="New Delhi"));
        p.interactive_nodes[1].search_state=Some("filled".into());
        assert!(matches!(planner.next(GOAL,&p),Some((BrowserAction::Click{node_id,..},"reopen_destination_options")) if node_id=="b"));
        assert!(!planner.route_confirmed[1]);
        assert!(PrerequisitePlanner::default().next("Book trains from Bengaluru to New Delhi for tomorrow",&p).is_none());
    }
}
