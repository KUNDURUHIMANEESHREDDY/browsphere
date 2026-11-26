//! Objective-aware action selection and effect assessment.
//! Advisory only: the live DOM binder, policy, user authorization, and final
//! independent verifier remain the sole execution/completion authorities.
//! Avoid site-specific URLs and never invent an action target.
use super::{dom_extractor::DomSnapshot, policy::BrowserAction, semantic_core};
use std::collections::HashSet;

fn words(text:&str)->HashSet<String>{
    text.split(|c:char|!c.is_alphanumeric())
        .filter(|s|s.len()>=3)
        .map(|s|s.to_ascii_lowercase()).collect()
}

/// Reject obvious planner/DOM metadata instead of executing it as a query.
pub fn invalid_search_payload(text: &str) -> bool {
    let value = text.trim().to_lowercase();
    if value.is_empty() { return true; }
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    // Strong structural markers are sufficient; ordinary search vocabulary
    // such as "role:" and "action:" is not metadata by itself.
    let structural = ["[type]", "node_id:", "node_id=", "search_state:"]
        .iter().any(|marker| normalized.starts_with(marker)
            || normalized.contains(&format!(" {}", marker)));
    let labelled_control = normalized.contains("label:") && normalized.contains("role:");
    if structural || labelled_control || normalized.starts_with("action: search for ") {
        return true;
    }
    let imperative = ["type ", "enter ", "choose ", "write ", "generate ", "provide ",
        "insert ", "input ", "formulate ", "select ", "come up with "]
        .iter().any(|prefix| normalized.starts_with(prefix));
    let placeholder = ["search query", "search term", "search keyword", "query for a song",
        "query for a music", "relevant query", "appropriate query"]
        .iter().any(|phrase| normalized.contains(phrase));
    imperative && placeholder
}

#[test]
fn rejects_instruction_payloads_without_rejecting_real_queries() {
    for bad in ["", "Type a short search query for a song or music",
        "Generate an appropriate query", "Provide relevant search keywords",
        "[type]: input, label: Search, role: combobox, search_state: emptyAction: Type a query",
        "Action: search for music", "Enter a search term"] {
        assert!(invalid_search_payload(bad), "accepted: {}", bad);
    }
    for good in ["Beethoven piano performance", "Python free certification courses",
        "how to write a search query", "search query optimization", "type 2 diabetes",
        "ARIA role: combobox accessibility", "GitHub action: checkout", "role: software engineer"] {
        assert!(!invalid_search_payload(good), "rejected: {}", good);
    }
}

/// Informational, navigation, research and media tasks can safely avoid
/// unrelated checkout/download/account controls. Real transactions or edits
/// retain their existing dedicated native policy and approval gates.
fn non_transactional(goal:&str)->bool{
    if super::deliberation::read_only(goal){return true;}
    let terms=words(goal);
    !["buy","purchase","pay","book","checkout","order","subscribe",
        "register","submit","send","upload","download","delete","remove",
        "modify","edit","write","publish","install"].iter()
        .any(|word|terms.contains(*word))
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
        && !group.iter().any(|word|requested.contains(*word)
            || (*word=="pricing"&&["free plan","plans","pricing","cost","price","subscription"]
                .iter().any(|topic|goal.to_ascii_lowercase().contains(topic)))))
        || (label.to_ascii_lowercase().contains("sign in")
            && !goal.to_ascii_lowercase().contains("sign in"))
        || (label.to_ascii_lowercase().contains("sign up")
            && !goal.to_ascii_lowercase().contains("sign up"))
        || (label.to_ascii_lowercase().contains("contact sales")
            && !goal.to_ascii_lowercase().contains("contact sales"))
}

// Search verticals are not primary factual sources unless explicitly requested.
fn unrelated_search_vertical(goal:&str,url:&str)->bool{
    if !super::deliberation::read_only(goal){return false;}
    let Ok(u)=url::Url::parse(url)else{return false;};
    if !matches!(u.host_str(),Some("www.bing.com"|"www.google.com"|"maps.google.com"|"duckduckgo.com")){return false;}
    let terms=words(goal);
    ["maps","images","videos","shopping"].iter().any(|v|
        (u.path().split('/').any(|p|p==*v)||(*v=="maps"&&u.host_str()==Some("maps.google.com")))
        && !terms.contains(*v)&&!(*v=="maps"&&terms.contains("map")))
}
/// Deny unrelated account, purchase, download, or privacy controls for
/// read-only research. This does not deny user-authorized transactions, forms,
/// resolving a non-sensitive dialog, or pages about these very topics.
pub fn irrelevant_control(goal:&str,page:&DomSnapshot,action:&BrowserAction)->bool{
    if !non_transactional(goal){return false;}
    match action {
        BrowserAction::Scroll{delta_y:0}=>true,
        BrowserAction::Navigate{url}=>unrelated_search_vertical(goal,url),
        BrowserAction::Click{node_id,..}=>{
            page.interactive_nodes.iter().find(|n|n.node_id==*node_id)
                .is_some_and(|n|unrelated_control(goal,&n.label)
                    || n.href.as_deref().is_some_and(|url|unrelated_search_vertical(goal,url)||unrelated_control(goal,
                        &url::Url::parse(url).ok().map(|u|
                            u.path().replace(['-','_','/']," ")).unwrap_or_default())))
        }
        _=>false
    }
}

/// On a named-source research stage, follow only a direct observed HTTPS
/// link to that brand's canonical-style domain in the active task tab.
/// Search-result clicks may open an untracked popup in WebView2.
pub fn observed_named_site_result(goal:&str,page:&DomSnapshot,stage:usize)->Option<BrowserAction>{
    let names=super::deliberation::named_research_entities(goal);
    let brand=names.get(stage)?.to_ascii_lowercase().replace([' ','-'],"");
    let source=url::Url::parse(&page.origin).ok()?;
    if source.host_str()!=Some("www.bing.com")
        || !matches!(super::page_understanding::classify(page).kind,
            super::page_understanding::PageKind::SearchResults){return None}
    let mut matches=Vec::new();
    for node in &page.interactive_nodes {
        if node.is_sensitive||node.is_submit||!(node.role=="link"||node.tag=="a"){continue}
        let Some(href)=node.href.as_deref()else{continue};
        let Ok(url)=url::Url::parse(href)else{continue};
        if url.scheme()!="https"||url.port().is_some()||!url.username().is_empty()
            ||url.password().is_some(){continue}
        let Some(host)=url.host_str()else{continue};
        let bare=host.strip_prefix("www.").unwrap_or(host);
        let parts:Vec<_>=bare.split('.').collect();
        if parts.len()!=2||parts[0]!=brand||parts[1].len()<2{continue}
        let score=if url.path().contains("pricing"){2}else{1};
        matches.push((score,href.to_string()));
    }
    matches.sort_by(|a,b|b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let (_,url)=matches.into_iter().next()?;
    Some(BrowserAction::Navigate{url})
}

/// Navigate to a pricing/free-plan page only when an official named source
/// actually exposes that HTTPS link. Do not construct guessed URLs, change
/// origins, or cross an authentication/checkout boundary.
pub fn observed_official_plan_link(goal:&str,page:&DomSnapshot,stage:usize)->Option<BrowserAction>{
    let lower=goal.to_ascii_lowercase();
    if !super::deliberation::read_only(goal)
        || !["free plan","free tier","pricing","plans"].iter().any(|s|lower.contains(s))
        || page.text_excerpt.starts_with("Active dialog:"){return None;}
    let brand=super::deliberation::named_research_entities(goal)
        .get(stage)?.to_ascii_lowercase().replace([' ','-'],"");
    let current=url::Url::parse(&page.origin).ok()?;
    if current.scheme()!="https"||current.port().is_some(){return None;}
    let bare=current.host_str()?.strip_prefix("www.").unwrap_or(current.host_str()?);
    let parts=bare.split('.').collect::<Vec<_>>();
    if parts.len()!=2||parts[0]!=brand{return None;}
    if current.path().to_ascii_lowercase().contains("pricing")
        || current.path().to_ascii_lowercase().contains("/plans"){return None;}
    let mut candidates=Vec::new();
    for n in &page.interactive_nodes{
        if n.is_sensitive||n.is_submit||!(n.role=="link"||n.tag=="a"){continue;}
        let Some(target)=n.href.as_deref().and_then(|h|url::Url::parse(h).ok())else{continue;};
        if target.scheme()!="https"||target.origin()!=current.origin()
            ||target==current||target.fragment().is_some()||!target.username().is_empty()
            ||target.password().is_some(){continue;}
        let path=target.path().to_ascii_lowercase();
        let label=n.label.to_ascii_lowercase();
        if ["checkout","sign in","sign up","contact sales","billing"].iter()
            .any(|s|path.contains(&s.replace(' ',""))||label.contains(s)){continue;}
        let score=if path.contains("pricing"){3}
            else if path.contains("/plans")||path.contains("/plan/"){2}
            else if label.contains("pricing")||label=="plans"||label=="free plan"{1}
            else{0};
        if score>0{candidates.push((score,target.to_string()));}
    }
    candidates.sort_by(|a,b|b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let (score,url)=candidates.first()?.clone();
    if candidates.iter().any(|(other_score,other_url)|*other_score==score&&*other_url!=url){return None;}
    Some(BrowserAction::Navigate{url})
}

/// Decode only a documented read-only public search result redirect.
pub fn observed_search_destination(origin:&str,href:&str)->Option<String>{
    let base=url::Url::parse(origin).ok()?;
    let link=base.join(href).ok()?;
    let target=if matches!(link.host_str(),Some("duckduckgo.com"|"www.duckduckgo.com"))
        && link.path()=="/l/"{
        let decoded=link.query_pairs().find(|(k,_)|k=="uddg")?.1.to_string();
        url::Url::parse(&decoded).ok()?
    }else{link};
    if target.scheme()!="https"||target.port().is_some()
        ||!target.username().is_empty()||target.password().is_some()
        ||super::course_comparison::site(target.as_str()).is_none(){return None;}
    Some(target.to_string())
}

/// A read-only search-results page already contains navigation candidates.
/// Prefer a genuine, topical external result over retyping the search query.
/// Never infer links: the returned click must bind to an observed DOM node.
pub fn grounded_search_result(goal:&str,page:&DomSnapshot)->Option<BrowserAction>{
    if !super::deliberation::read_only(goal)
        || !matches!(super::page_understanding::classify(page).kind,
            super::page_understanding::PageKind::SearchResults)
    {return None;}
    let current=url::Url::parse(&page.origin).ok()?;
    let host=current.host_str()?;
    if !matches!(host,"www.bing.com"|"www.google.com"|"duckduckgo.com"|"html.duckduckgo.com"){return None;}
    let meaningful=words(goal);
    let mut candidates=Vec::new();
    for n in &page.interactive_nodes {
        if n.is_sensitive||n.is_submit||(n.role!="link"&&n.tag!="a")||n.label.len()<12
            || unrelated_control(goal,&n.label){continue;}
        let Some(href)=n.href.as_deref()else{continue};
        let destination=observed_search_destination(&page.origin,href);
        // Bing wraps ordinary public result links in /ck/a tracking redirects.
        // Clicking a live topical node is allowed by normal click policy;
        // never turn the redirect into a trusted factual source URL.
        let bing_result_redirect=destination.is_none()
            && host=="www.bing.com"
            && super::course_comparison::count(goal).is_none()
            && url::Url::parse(href).ok().is_some_and(|u|
                u.scheme()=="https"&&u.host_str()==Some("www.bing.com")&&u.path()=="/ck/a");
        if destination.is_none()&&!bing_result_redirect{continue;}
        if unrelated_search_vertical(goal,href){continue;}
        if super::course_comparison::count(goal).is_some(){
            let Some(provider)=destination.as_deref().and_then(super::course_comparison::site)else{continue;};
            if current.query_pairs().any(|(k,q)|k=="q"
                &&q.contains(&format!("-site:{provider}"))){continue;}
        }
        let label=n.label.to_ascii_lowercase();
        let tokens=words(&label);
        let matches=tokens.intersection(&meaningful).count();
        // Require at least two independent task-related terms. Search
        // snippets and advertising labels must not stand in for evidence.
        if matches<2{continue;}
        let path=url::Url::parse(destination.as_deref().unwrap_or(href)).ok().map(|u|u.path().to_ascii_lowercase()).unwrap_or_default();
        if super::course_comparison::count(goal).is_some()
            && (["/search","/courses","/catalog","/collections","/all-courses"].contains(&path.as_str())
               || ["/blog/","/articles/","/news/","best-free-","best-python-courses","top-python-courses"].iter().any(|p|path.contains(p))
               || ["best free", "ranked", "top 10", "best courses"].iter().any(|p|label.contains(p))) {
            continue;
        }
        let mut score=(matches as i32)*10
            +if n.bbox.as_ref().is_some_and(|r|r.in_viewport){3}else{0};
        // A course detail page is better evidence than a listicle, blog,
        // or general search/catalog page. This is URL-shape reasoning, not
        // a platform whitelist or invented course destination.
        if super::course_comparison::count(goal).is_some(){
            if ["/course/","/courses/","/academy/","/learn/","/skillup/"].iter().any(|p|path.contains(p)){
                score+=15;
            }
            if ["/blog/","/articles/","/news/"].iter().any(|p|path.contains(p)){
                score-=30;
            }
            if label.contains("top 10")||label.contains("best courses") {score-=12;}
        }
        candidates.push((score,n.node_id.clone(),n.label.clone(),destination.unwrap_or_else(||href.to_string())));
    }
    candidates.sort_by(|a,b|b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let (_,node_id,label,url)=candidates.into_iter().next()?;
    if super::course_comparison::count(goal).is_some(){
        // Search-result links commonly open popups. Use the exact observed
        // external href in the task WebView with an independent policy gate.
        Some(BrowserAction::Navigate{url})
    }else{
        Some(BrowserAction::Click{node_id,element_label:label,is_submit:false})
    }
}

/// Native requirement-driven navigations are already grounded in the
/// current task and source ledger. An opportunistic link on the current page
/// must never rewrite a deliberate next-platform research transition.
pub fn protected_native_research_transition(route:&str)->bool{
    ["course:next_platform","research:grounded_search_result",
     "research_bootstrap:","prerequisite:explicit_research_source"]
        .iter().any(|prefix|route.starts_with(prefix))
}

/// High-confidence suggestion to *replace an unrelated model proposal*, not
/// a general-purpose shortcut to choose the first result. Only a unique
/// topical HTTPS link in a fresh DOM snapshot qualifies, and the normal native
/// policy gates must still authorize the returned navigation.
pub fn better_observed_link(
    goal:&str, stage:&str, page:&DomSnapshot, proposed:&BrowserAction,
    excluded_ids:&HashSet<String>,
)->Option<BrowserAction>{
    if !non_transactional(goal)
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
        if unrelated_search_vertical(goal,url){continue;}
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
    if non_transactional(goal)
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
    #[test]
    fn named_official_search_result_navigates_current_tab_instead_of_clicking_popup() {
        use super::super::dom_extractor::DomNodeSummary;
        let goal="Visit the official websites of Notion, Trello and ClickUp. Compare their free plans for a five-member team and recommend the most suitable option.";
        let link=|id:&str,href:&str|DomNodeSummary {
            node_id:id.into(),tag:"a".into(),role:"link".into(),label:"Official website".into(),
            options:vec![],checked:None,search_state:None,href:Some(href.into()),
            is_submit:false,is_sensitive:false,bbox:None
        };
        let page=DomSnapshot{origin:"https://www.bing.com/search?q=Notion+official".into(),
            title:"Notion official website - Search".into(),
            text_excerpt:"Search results for the Notion official website, free plan pricing information and comparison.".into(),
            interactive_nodes:vec![link("bad","https://notion.evil.com/pricing"),
                link("good","https://www.notion.so/pricing")],structure:Default::default()};
        assert!(matches!(observed_named_site_result(goal,&page,0),Some(BrowserAction::Navigate{url})
            if url=="https://www.notion.so/pricing"));
        assert!(observed_named_site_result(goal,&page,1).is_none());
    }
    use super::super::dom_extractor::{BoundingBox,DomNodeSummary};
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
    #[test]
    fn research_rejects_unrequested_vertical_and_zero_scroll(){
        let goal="Plan a Sunday outing. Find attractions and ticket prices.";
        let p=page(vec![node("map","Attractions map",Some("https://www.bing.com/maps?cp=12"))]);
        let action=BrowserAction::Navigate{url:"https://www.bing.com/maps?cp=12".into()};
        assert!(irrelevant_control(goal,&p,&action));
        assert!(irrelevant_control(goal,&p,&BrowserAction::Scroll{delta_y:0}));
        assert!(irrelevant_control(goal,&p,&BrowserAction::Click{node_id:"map".into(),element_label:"Attractions map".into(),is_submit:false}));
        assert!(!irrelevant_control("Find attractions on a map",&p,&action));
    }
    #[test]
    fn search_result_redirect_requires_observed_topical_link(){
        let goal="Plan a Sunday outing in Bengaluru. Find family friendly attractions and ticket prices.";
        let p=DomSnapshot{
            origin:"https://www.bing.com/search?q=Bengaluru+family+attractions".into(),
            title:"Bengaluru family attractions - Search results".into(),
            text_excerpt:"Search results for family attractions in Bengaluru including sites and ticket information.".into(),
            structure:Default::default(),
            interactive_nodes:vec![
                node("unrelated","Microsoft account",Some("https://www.bing.com/ck/a?p=1")),
                node("attraction","Family attractions Bengaluru opening hours and ticket prices",Some("https://www.bing.com/ck/a?p=2")),
            ]
        };
        assert!(matches!(grounded_search_result(goal,&p),Some(BrowserAction::Click{node_id,..}) if node_id=="attraction"));
    }
    #[test]
    fn official_free_plan_research_uses_only_observed_safe_pricing_links(){
        let goal="Visit the official websites of Notion, Trello and ClickUp. Compare their free plans for a five-member team.";
        let mut p=page(vec![
            node("pricing","Pricing",Some("https://www.notion.so/pricing")),
            node("evil","Pricing",Some("https://notion.evil.com/pricing")),
            node("login","Sign in",Some("https://www.notion.so/login")),
        ]);
        p.origin="https://www.notion.so/".into();
        p.title="Notion".into();
        assert!(matches!(observed_official_plan_link(goal,&p,0),
            Some(BrowserAction::Navigate{url}) if url=="https://www.notion.so/pricing"));
        assert!(observed_official_plan_link(goal,&p,1).is_none());
        p.origin="https://www.notion.so/pricing".into();
        assert!(observed_official_plan_link(goal,&p,0).is_none());
        p.origin="https://notion.evil.com/".into();
        assert!(observed_official_plan_link(goal,&p,0).is_none());
        p.origin="https://www.notion.so/".into();
        p.interactive_nodes.push(node("alternate","Plans",Some("https://www.notion.so/other-pricing")));
        assert!(observed_official_plan_link(goal,&p,0).is_none(),
            "Equally scored distinct official pricing links require disambiguation");
    }
    const GOAL:&str="Find the SQL INSERT command documentation and report its syntax";
    #[test]
    fn never_replace_grounded_native_course_research_with_current_page_link(){
        for route in ["course:next_platform","research:grounded_search_result",
                      "research_bootstrap:trusted_user_query",
                      "prerequisite:explicit_research_source"]{
            assert!(protected_native_research_transition(route));
        }
        assert!(!protected_native_research_transition("ollama:qwen2.5:3b-instruct"));
    }
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
    fn protects_nontransactional_media_and_navigation_tasks_too(){
        let p=page(vec![
            node("download","Download music",None),
            node("play","Play music",None),
            node("pricing","Pricing",Some("https://unfamiliar.docs.example/pricing"))
        ]);
        let click=|id:&str|BrowserAction::Click{
            node_id:id.into(),element_label:String::new(),is_submit:false
        };
        assert!(irrelevant_control("play any song",&p,&click("download")));
        assert!(irrelevant_control("play any song",&p,&click("pricing")));
        assert!(!irrelevant_control("play any song",&p,&click("play")));
        assert!(!irrelevant_control("Purchase music and download the album",&p,&click("download")),
            "A clearly transactional request stays under the existing approval policy");
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
