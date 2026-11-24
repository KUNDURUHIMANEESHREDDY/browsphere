//! Run-scoped prerequisites for browser workflows. Native observations and
//! explicit user values only; never authorize a click or claim task completion.
use super::{dom_extractor::{DomSnapshot, DomNodeSummary}, policy::BrowserAction};
use std::{collections::HashSet, hash::{Hash, Hasher}};
use chrono::NaiveDate;

#[derive(Default)]
pub struct PrerequisitePlanner {
    attempted: HashSet<(String,String,u64)>,
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
fn requested_date(goal:&str)->Option<NaiveDate> {
    goal.split(|c:char| !c.is_ascii_digit() && c!='-')
        .find_map(|token|NaiveDate::parse_from_str(token,"%Y-%m-%d").ok())
}
fn date_labels(date:NaiveDate)->Vec<String> {
    ["%Y-%m-%d","%d/%m/%Y","%d %B %Y","%B %d, %Y","%d %b %Y","%A, %B %d, %Y"]
        .iter().map(|format|normalized(&date.format(format).to_string())).collect()
}
/// A search-only task cannot complete on the original form or a modal.
pub fn completion_blocker(goal:&str,page:&DomSnapshot)->Option<&'static str> {
    let (from,to)=route_values(goal)?;
    let text=normalized(&page.text_excerpt);
    if text.starts_with("active dialog:") {return Some("Resolve the blocking dialog first");}
    if !["train list","trains found","train number","check availability","no trains found","no train found"]
        .iter().any(|term|text.contains(term)) {
        return Some("No observed train search results; the requested route and date are not verified");
    }
    if !text.contains(&normalized(from)) || !text.contains(&normalized(to)) {
        return Some("Observed results do not identify both requested cities");
    }
    if let Some(date)=requested_date(goal) {
        if !date_labels(date).iter().any(|label|text.contains(label)) {
            return Some("Observed results do not identify the requested journey date");
        }
    }
    None
}
impl PrerequisitePlanner {
    pub fn next(&mut self,goal:&str,page:&DomSnapshot)->Option<(BrowserAction,&'static str)> {
        let excerpt=normalized(&page.text_excerpt);
        let proposal=if excerpt.starts_with("active dialog:") {
            language(goal,page).map(|a|(a,"explicit_language"))
        } else if let Some((from,to))=route_values(goal) {
            let field=|direction:&str| unique(page.interactive_nodes.iter().filter(|n|
                safe(n) && n.tag=="input" && n.role=="searchbox" &&
                normalized(&n.label).contains(&format!("{direction} station"))));
            let (Some(a),Some(b))=(field("from"),field("to")) else {return None;};
            // Use observed options only. If the city has multiple matches,
            // leave the choice to the model rather than inventing a station.
            let choice=|value:&str| unique(page.interactive_nodes.iter().filter(|n|
                safe(n)&&n.role=="option"&&normalized(&n.label).contains(&normalized(value))));
            if a.search_state.as_deref()==Some("empty") {
                Some((BrowserAction::TypeText{node_id:a.node_id.clone(),text:from.into(),is_sensitive:false},"origin_input"))
            } else if a.search_state.as_deref()==Some("filled") && b.search_state.as_deref()==Some("empty") {
                if let Some(n)=choice(from) {Some((click(n),"origin_option"))}
                else if page.interactive_nodes.iter().any(|n|n.role=="option") {None}
                else {Some((BrowserAction::TypeText{node_id:b.node_id.clone(),text:to.into(),is_sensitive:false},"destination_input"))}
            } else if b.search_state.as_deref()==Some("filled") {
                if let Some(n)=choice(to) {Some((click(n),"destination_option"))}
                else if page.interactive_nodes.iter().any(|n|n.role=="option") {None}
                else if let Some(date)=requested_date(goal) {
                    let labels=date_labels(date);
                    if let Some(n)=unique(page.interactive_nodes.iter().filter(|n|safe(n)
                        && matches!(n.role.as_str(),"button"|"gridcell"|"link")
                        && labels.contains(&normalized(&n.label)))) {
                        Some((click(n),"exact_calendar_day"))
                    } else {
                        unique(page.interactive_nodes.iter().filter(|n|safe(n) && n.tag=="input"
                            && n.search_state.as_deref()==Some("read_only")
                            && ["date","dd/mm/yyyy"].iter().any(|term|normalized(&n.label).contains(term))))
                            .map(|n|(click(n),"open_date_picker"))
                    }
                } else {None}
            } else {None}
        } else {None};
        let (action,phase)=proposal?;
        // Attempt each semantic prerequisite at most once per observed page
        // state. No persistent storage of entered text or secrets.
        let mut state=std::collections::hash_map::DefaultHasher::new();
        excerpt.hash(&mut state);
        for node in &page.interactive_nodes {
            node.label.hash(&mut state);node.search_state.hash(&mut state);
        }
        let key=(page.origin.clone(),phase.to_string(),state.finish());
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
        DomSnapshot{origin:"https://example.org".into(),title:"Test".into(),text_excerpt:text.into(),interactive_nodes:nodes}
    }
    const GOAL:&str="Test a train booking search only. Choose English if a language dialog appears. Find trains from Bengaluru to New Delhi for 2026-11-24.";
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
        assert!(matches!(PrerequisitePlanner::default().next(GOAL,&p),
            Some((BrowserAction::Click{node_id,..},"exact_calendar_day")) if node_id=="date"));
        assert!(completion_blocker(GOAL,&p).is_some());
        let results=page("Train list Bengaluru to New Delhi 24/11/2026",vec![]);
        assert!(completion_blocker(GOAL,&results).is_none());
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
        p.interactive_nodes.pop();
        assert!(matches!(planner.next(GOAL,&p),Some((BrowserAction::TypeText{node_id,text,..},"destination_input")) if node_id=="b"&&text=="New Delhi"));
        p.interactive_nodes[1].search_state=Some("filled".into());
        assert!(planner.next(GOAL,&p).is_none());
        assert!(PrerequisitePlanner::default().next("Book trains from Bengaluru to New Delhi for tomorrow",&p).is_none());
    }
}
