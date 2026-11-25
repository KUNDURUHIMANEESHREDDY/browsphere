//! Objective-aware action selection and effect assessment.
//! Advisory only: the live DOM binder, policy, user authorization, and final
//! independent verifier remain the sole execution/completion authorities.
//! Avoid site-specific URLs and never invent an action target.
use super::{dom_extractor::{DomNodeSummary,DomSnapshot}, policy::BrowserAction, semantic_core};
use std::collections::HashSet;

fn words(text:&str)->HashSet<String>{
    text.split(|c:char|!c.is_alphanumeric())
        .filter(|s|s.len()>=3)
        .map(|s|s.to_ascii_lowercase()).collect()
}

fn unrelated_control(goal:&str,label:&str)->bool{
    let requested=words(goal);
    let label_tokens=words(label);
    let groups:&[&[&str]]=&[
        &["pricing"],&["subscribe"],&["checkout"],&["payment"],&["purchase"],
        &["register"],&["login"],&["signin"],&["signup"],&["careers"],
        &["privacy"],&["account"],&["download"],&["delete"],
    ];
    groups.iter().any(|group|group.iter().any(|word|label_tokens.contains(*word))
        && !group.iter().any(|word|requested.contains(*word)))
        || (label.to_ascii_lowercase().contains("sign in")
            && !goal.to_ascii_lowercase().contains("sign in"))
        || (label.to_ascii_lowercase().contains("sign up")
            && !goal.to_ascii_lowercase().contains("sign up"))
        || (label.to_ascii_lowercase().contains("contact sales")
            && !goal.to_ascii_lowercase().contains("contact sales"))
}

/// Deny unrelated account, purchase, download, or privacy controls for
/// read-only research. This does not deny user-authorized transactions, forms,
/// resolving a non-sensitive dialog, or pages about these very topics.
pub fn irrelevant_control(goal:&str,page:&DomSnapshot,action:&BrowserAction)->bool{
    if !super::deliberation::read_only(goal){return false;}
    match action {
        BrowserAction::Click{node_id,..}=>{
            page.interactive_nodes.iter().find(|n|n.node_id==*node_id)
                .is_some_and(|n|unrelated_control(goal,&n.label)
                    || n.href.as_deref().is_some_and(|url|unrelated_control(goal,
                        &url::Url::parse(url).ok().map(|u|
                            u.path().replace(['-','_','/']," ")).unwrap_or_default())))
        }
        _=>false
    }
}

/// High-confidence suggestion to *replace an unrelated model proposal*, not
/// a general-purpose shortcut to choose the first result. Only a unique
/// topical HTTPS link in a fresh DOM snapshot qualifies, and the normal native
/// policy gates must still authorize the returned navigation.
pub fn better_observed_link(
    goal:&str, stage:&str, page:&DomSnapshot, proposed:&BrowserAction,
    excluded_ids:&HashSet<String>,
)->Option<BrowserAction>{
    if !super::deliberation::read_only(goal)
        || page.text_excerpt.starts_with("Active dialog:")
        || super::deliberation::error_page(page)
        || super::recovery::authentication_gate(page)
        || (goal.to_ascii_lowercase().contains("first")
            && goal.to_ascii_lowercase().contains("result")){
        return None;
    }
    // Never override typing, submitting, choosing an option, verification,
    // or other explicit form actions with an opportunistic link.
    let proposed_url=match proposed {
        BrowserAction::Scroll{..}=>None,
        BrowserAction::Navigate{url}=>Some(url.as_str()),
        BrowserAction::Click{node_id,is_submit:false,..}=>{
            page.interactive_nodes.iter().find(|n|n.node_id==*node_id)?
                .href.as_deref()
        }
        _=>return None,
    };
    // Explicit user destinations always win over inferred navigation.
    if proposed_url.is_some_and(|url|super::policy::requested_navigation_url(goal)
        .is_some_and(|u|u.as_str()==url)){return None;}
    let mut candidates=Vec::<(i32,String,String)>::new();
    for node in &page.interactive_nodes {
        if node.is_sensitive||node.is_submit||excluded_ids.contains(&node.node_id)
            || !(node.tag=="a"||node.role=="link")||unrelated_control(goal,&node.label){
            continue;
        }
        let Some(url)=node.href.as_deref() else {continue};
        let Ok(parsed)=url::Url::parse(url)else{continue};
        if parsed.scheme()!="https"||parsed.host_str().is_none()
            || !parsed.username().is_empty()||parsed.password().is_some()
            || parsed.fragment().is_some(){continue;}
        if super::policy::requested_navigation_url(goal).is_some_and(|u|u.as_str()==url){
            // The explicit destination should have been handled separately.
            continue;
        }
        let material=format!("{} {}",node.label,parsed.path().replace(['-','_','/']," "));
        if unrelated_control(goal,&material){continue;}
        let topic=semantic_core::relevance(goal,&material) as i32;
        let stage_match=semantic_core::stage_relevance(goal,stage,&material) as i32;
        if topic==0||stage_match==0{continue;}
        let score=topic*11+stage_match*14
            +if node.bbox.as_ref().is_some_and(|b|b.in_viewport){3}else{0};
        candidates.push((score,node.node_id.clone(),url.to_owned()));
    }
    candidates.sort_by(|a,b|b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let best=candidates.first()?;
    if best.0<28{return None;}
    // Distinct URLs with near-equal semantic relevance are ambiguous; do not
    // turn a model mistake into another arbitrary decision.
    if candidates.iter().skip(1).any(|(score,_,url)|
        url!=&best.2 && best.0-*score<10){return None;}
    let already_selected=proposed_url.is_some_and(|u|u==best.2)
        || matches!(proposed,BrowserAction::Click{node_id,..} if node_id==&best.1);
    if already_selected{return None;}
    // If the model chose another reasonably topical result, defer to it.
    if let Some(u)=proposed_url {
        if let Some((score,_,_))=candidates.iter().find(|(_,_,url)|url==u){
            if best.0-*score<12{return None;}
        }
    }
    Some(BrowserAction::Navigate{url:best.2.clone()})
}

fn control_state_changed(before:&DomSnapshot,after:&DomSnapshot)->bool{
    let summarize=|page:&DomSnapshot|{
        let mut controls:Vec<_>=page.interactive_nodes.iter()
            .filter(|n|!n.is_sensitive)
            .map(|n|format!("{}|{}|{:?}|{:?}|{:?}",
                n.role,n.label,n.search_state,n.checked,n.options)).collect();
        controls.sort();
        controls
    };
    summarize(before)!=summarize(after)
}

fn visible_topic_advanced(goal:&str,stage:&str,before:&DomSnapshot,after:&DomSnapshot)->bool{
    let score=|p:&DomSnapshot|{
        semantic_core::navigation_relevance(goal,stage,p)
            +semantic_core::relevance(goal,&p.text_excerpt)
    };
    score(after)>score(before)
}

/// A WebView action receipt proves dispatch, *not* state progress. Require a
/// native observed change for navigation, clicking, typing and scrolling.
/// Search-field/option changes count as progress even without URL changes.
/// Completion and downloads have separate existing receipts and gates.
pub fn effect_observed(
    goal:&str,stage:&str,before:&DomSnapshot,after:&DomSnapshot,
    action:&BrowserAction,
)->bool{
    if super::deliberation::error_page(after)||super::recovery::authentication_gate(after){
        return false;
    }
    // A transition from relevant research to an unrelated landing page is
    // not objective progress merely because the URL or UI changed.
    let topicality=|page:&DomSnapshot|
        semantic_core::navigation_relevance(goal,stage,page)
            +semantic_core::relevance(goal,&page.text_excerpt);
    if super::deliberation::read_only(goal)
        && topicality(before)>0 && topicality(after)==0
        && !matches!(action,BrowserAction::Complete{..}|BrowserAction::Download{..}){
        return false;
    }
    match action {
        BrowserAction::Complete{..}|BrowserAction::Download{..}=>true,
        BrowserAction::Navigate{..}=>{
            if before.origin==after.origin && before.title==after.title
                && before.text_excerpt==after.text_excerpt{return false;}
            // Changes to the URL alone are not sufficient if the resulting
            // page is an unrelated utility or account destination.
            if unrelated_control(goal,&after.title){return false;}
            after.origin!=before.origin||visible_topic_advanced(goal,stage,before,after)
                || control_state_changed(before,after)
        }
        BrowserAction::Click{..}|BrowserAction::PressKey{..}|
        BrowserAction::TypeText{..}|BrowserAction::SelectOption{..}|
        BrowserAction::SetChecked{..}|BrowserAction::Scroll{..}=>{
            if unrelated_control(goal,&after.title){return false;}
            let url_or_title=before.origin!=after.origin||before.title!=after.title;
            let controls=control_state_changed(before,after);
            let excerpt=before.text_excerpt!=after.text_excerpt;
            // Acknowledged clicks with no observable effect do not reset the
            // stall budget. A live, changing field or result list does.
            url_or_title||controls||excerpt
        }
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    use super::super::dom_extractor::BoundingBox;
    fn node(id:&str,label:&str,url:Option<&str>)->DomNodeSummary{
        DomNodeSummary{node_id:id.into(),tag:if url.is_some(){"a"}else{"button"}.into(),
            role:if url.is_some(){"link"}else{"button"}.into(),label:label.into(),
            href:url.map(str::to_owned),options:vec![],checked:None,search_state:None,
            is_submit:false,is_sensitive:false,
            bbox:Some(BoundingBox{x:0.,y:0.,width:160.,height:30.,in_viewport:true})}
    }
    fn page(nodes:Vec<DomNodeSummary>)->DomSnapshot{
        DomSnapshot{origin:"https://unfamiliar.docs.example/search".into(),
            title:"Search results".into(),
            text_excerpt:"Search results for SQL INSERT command".into(),
            structure:Default::default(),interactive_nodes:nodes}
    }
    const GOAL:&str="Find the SQL INSERT command documentation and report its syntax";
    #[test]
    fn selects_unique_objective_relevant_link_instead_of_off_topic_chrome(){
        let p=page(vec![
            node("pricing","Pricing",Some("https://unfamiliar.docs.example/pricing")),
            node("privacy","Privacy policy",Some("https://unfamiliar.docs.example/privacy")),
            node("sql","SQL INSERT syntax reference",Some("https://unfamiliar.docs.example/reference/sql/insert")),
        ]);
        let wrong=BrowserAction::Click{node_id:"pricing".into(),element_label:"Pricing".into(),is_submit:false};
        assert!(irrelevant_control(GOAL,&p,&wrong));
        assert!(matches!(better_observed_link(GOAL,"Locate SQL INSERT syntax",&p,&wrong,
            &HashSet::new()),Some(BrowserAction::Navigate{url})
                if url.ends_with("/reference/sql/insert")));
        assert!(matches!(better_observed_link(GOAL,"Locate SQL INSERT syntax",&p,
            &BrowserAction::Scroll{delta_y:450},&HashSet::new()),
            Some(BrowserAction::Navigate{..})));
        let correct=BrowserAction::Click{node_id:"sql".into(),
            element_label:"SQL INSERT syntax reference".into(),is_submit:false};
        assert!(better_observed_link(GOAL,"Locate SQL INSERT syntax",&p,&correct,&HashSet::new()).is_none());
    }
    #[test]
    fn abstains_on_ambiguous_results_or_missing_topical_evidence(){
        let p=page(vec![
            node("a","SQL INSERT docs",Some("https://unfamiliar.docs.example/guide/insert")),
            node("b","SQL INSERT docs",Some("https://unfamiliar.docs.example/reference/insert")),
        ]);
        assert!(better_observed_link(GOAL,"Locate SQL INSERT docs",&p,
            &BrowserAction::Scroll{delta_y:400},&HashSet::new()).is_none());
        let empty=page(vec![node("privacy","Privacy policy",Some("https://unfamiliar.docs.example/privacy"))]);
        assert!(better_observed_link(GOAL,"Locate SQL INSERT docs",&empty,
            &BrowserAction::Scroll{delta_y:400},&HashSet::new()).is_none());
    }
    #[test]
    fn respects_explicit_first_result_and_form_actions(){
        let p=page(vec![node("a","SQL INSERT docs",Some("https://unfamiliar.docs.example/insert"))]);
        assert!(better_observed_link("Search SQL INSERT and open the first result",
            "Open first result",&p,&BrowserAction::Scroll{delta_y:400},&HashSet::new()).is_none());
        assert!(better_observed_link(GOAL,"Locate SQL INSERT",&p,
            &BrowserAction::TypeText{node_id:"search".into(),text:"SQL INSERT".into(),
                is_sensitive:false},&HashSet::new()).is_none());
        assert!(!irrelevant_control("Read the pricing documentation",&p,
            &BrowserAction::Click{node_id:"a".into(),element_label:String::new(),is_submit:false}));
    }
    #[test]
    fn acknowledged_action_without_observed_change_is_not_progress(){
        let p=page(vec![node("sql","SQL INSERT",Some("https://unfamiliar.docs.example/insert"))]);
        let click=BrowserAction::Click{node_id:"sql".into(),element_label:"SQL INSERT".into(),is_submit:false};
        assert!(!effect_observed(GOAL,"Locate SQL INSERT",&p,&p,&click));
        assert!(!effect_observed(GOAL,"Locate SQL INSERT",&p,&p,
            &BrowserAction::Scroll{delta_y:300}));
        let changed=DomSnapshot{origin:"https://unfamiliar.docs.example/insert".into(),
            title:"SQL INSERT - Reference".into(),
            text_excerpt:"SQL INSERT syntax and examples are documented here.".into(),
            ..p.clone()};
        assert!(effect_observed(GOAL,"Locate SQL INSERT",&p,&changed,&click));
        let mut typed=p.clone();
        typed.interactive_nodes[0].search_state=Some("filled".into());
        assert!(effect_observed(GOAL,"Search SQL INSERT",&p,&typed,
            &BrowserAction::TypeText{node_id:"sql".into(),text:"SQL INSERT".into(),is_sensitive:false}));
        let login=DomSnapshot{title:"Sign in to your account".into(),..p.clone()};
        assert!(!effect_observed(GOAL,"Locate SQL INSERT",&p,&login,&click));
        let unrelated=DomSnapshot{origin:"https://unfamiliar.docs.example/home".into(),
            title:"Company home page".into(),text_excerpt:"Welcome to our company.".into(),
            interactive_nodes:vec![],..p.clone()};
        assert!(!effect_observed(GOAL,"Locate SQL INSERT",&p,&unrelated,&click),
            "Changing the URL to an unrelated page is not objective progress");
    }
}
