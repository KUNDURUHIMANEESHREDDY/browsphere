//! Source-grounded requirements for cross-platform course comparisons.
//! No provider names, domain destinations, course answers or certificates are embedded.
use super::{orchestrator::SourceEvidence,evidence_verifier::VerificationReport,browser_skills};
use std::collections::{BTreeMap,BTreeSet};
use url::Url;

/// Build a compact topic query from the user's request without selecting any
/// course provider or manufacturing a source URL.
pub fn discovery_terms(goal:&str)->Option<String>{
    count(goal)?;
    let g=goal.to_ascii_lowercase();
    let topic=if g.contains("python") {"Python"}
        else if g.contains("javascript") {"JavaScript"}
        else if g.contains("java") {"Java"}
        else if g.contains("sql") {"SQL"}
        else {"programming"};
    Some(format!("free beginner {topic} course certificate duration"))
}
pub fn count(goal:&str)->Option<usize>{
    let g=goal.to_ascii_lowercase();
    if !super::deliberation::read_only(goal)||!g.contains("course")
        ||!g.contains("certificate")||!g.contains("duration")
        ||!(g.contains("different")||g.contains("multiple")||g.contains("platform"))
    {return None;}
    let n=if g.contains("three ")||g.contains("3 ") {3}
        else if g.contains("two ")||g.contains("2 ") {2}
        else if g.contains("four ")||g.contains("4 ") {4}
        else if g.contains("five ")||g.contains("5 ") {5}
        else {return None};
    Some(n)
}
pub(crate) fn site(url:&str)->Option<String>{
    let u=Url::parse(url).ok()?;
    if u.scheme()!="https"||u.port().is_some()||!u.username().is_empty()||u.password().is_some(){return None}
    let host=u.host_str()?.trim_start_matches("www.").to_ascii_lowercase();
    if host.parse::<std::net::IpAddr>().is_ok()
        || [".local",".localhost",".internal",".test",".invalid",".onion",".example"]
            .iter().any(|suffix|host.ends_with(suffix))
        || ["bing.com","google.com","duckduckgo.com","yahoo.com",
            "youtube.com","reddit.com","wikipedia.org"].iter().any(|s|host==*s||host.ends_with(&format!(".{s}")))
    {return None;}
    // Group subdomains of one learning platform as one source, so a course
    // page and its certificate FAQ cannot impersonate two providers.
    let labels=host.split('.').collect::<Vec<_>>();
    let n=labels.len();
    if n<2 {return None;}
    let cc_second=["co","com","org","ac","edu","gov","net"];
    let length=if n>=3 && labels[n-1].len()==2
        &&cc_second.contains(&labels[n-2]) {3}else{2};
    Some(labels[n-length..].join("."))
}
fn cert(text:&str)->bool{
    let t=text.to_ascii_lowercase();
    ["certificate","certification","certified"].iter().any(|w|t.contains(w))
        && ["free","paid","cost","fee","included","not included","purchase","no certificate"].iter().any(|w|t.contains(w))
}
/// Interpret only explicit certificate-cost wording. A "free course"
/// does not automatically imply a free certificate.
fn certificate_status(quote:&str)->&'static str{
    let t=quote.to_ascii_lowercase();
    if ["pro+ subscription","certificate with subscription","paid certificate",
        "certificate fee","pay for a certificate","purchase the certificate",
        "certificate costs","certificate is not free"].iter().any(|s|t.contains(s)){
        return "Paid certificate or subscription indicated";
    }
    if ["free certificate","free certification","certificate for free",
        "certificate is free","certificate at no cost","certificate free of charge"].iter().any(|s|t.contains(s)){
        return "Free certificate explicitly advertised";
    }
    "Free certificate not confirmed by the supporting excerpt"
}
fn duration(text:&str)->bool{
    let t=text.to_ascii_lowercase();
    let tokens=t.split(|c:char|!c.is_ascii_alphanumeric()&&c!='.')
        .filter(|s|!s.is_empty()).collect::<Vec<_>>();
    tokens.windows(2).any(|w|{
        let number=w[0].parse::<f64>().is_ok()
            ||w[0].chars().any(|c|c.is_ascii_digit());
        number&&["hour","hours","hr","hrs","minute","minutes","min","mins",
            "week","weeks","month","months","day","days"].contains(&w[1])
    })
}
fn course(text:&str,goal:&str)->bool{
    let t=text.to_ascii_lowercase();
    t.contains("course")&&t.contains("free")
        &&(!goal.to_ascii_lowercase().contains("python")||t.contains("python"))
}
fn course_page(goal:&str,source:&SourceEvidence)->bool{
    let Ok(u)=Url::parse(&source.url)else{return false};
    let path=u.path().to_ascii_lowercase();
    // Search/catalog pages can mention many unrelated durations and
    // certificate offers; they are not evidence for one actual course.
    if ["/search","/courses","/catalog","/collections","/all-courses"].contains(&path.as_str()){
        return false;
    }
    course(&format!("{} {}",source.title,source.excerpt),goal)
}
pub fn source_hosts(goal:&str,sources:&[SourceEvidence])->Vec<String>{
    let mut by_host=BTreeMap::<String,Vec<&SourceEvidence>>::new();
    for s in sources{
        if let Some(host)=site(&s.url){by_host.entry(host).or_default().push(s);}
    }
    by_host.into_iter().filter_map(|(host,rows)|{
        let contextual=rows.iter().any(|s|course_page(goal,s));
        let certified=rows.iter().any(|s|cert(&s.excerpt));
        let timed=rows.iter().any(|s|duration(&s.excerpt));
        (contextual&&certified&&timed).then_some(host)
    }).collect()
}
/// Continue discovery across distinct observed course providers. A provider
/// with missing certificate or duration detail still counts as *visited*,
/// never as verified; the final ledger preserves the stronger fact checks.
pub fn next_search(goal:&str,sources:&[SourceEvidence],current:&str)->Option<Url>{
    let required=count(goal)?;
    // A live search-result click may land on a readable but irrelevant
    // article. Record that actually visited public site for exclusion, even
    // though it contributes ZERO evidence toward the required course facts.
    // Keep the number of alternate-site searches bounded.
    let visited=sources.iter().filter_map(|s|site(&s.url))
        .collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>();
    let current_host=site(current)?;
    if !visited.contains(&current_host)||visited.len()>=required+2{return None;}
    if source_hosts(goal,sources).len()>=required{return None;}
    let hosts=visited;
    let mut query=String::from("beginner free programming course certificate duration");
    if goal.to_ascii_lowercase().contains("python"){query=String::from("beginner free Python course certificate duration");}
    for host in hosts.iter().take(5){
        query.push_str(" -site:");query.push_str(host);
    }
    let mut url=Url::parse("https://html.duckduckgo.com/html/").ok()?;
    url.query_pairs_mut().append_pair("q",&query);
    Some(url)
}

/// Assemble only source-exact course facts. Never ask a small model to choose
/// unrelated snippets from a page when the requested dimensions can be
/// identified directly in the native captured text. Missing data stays missing.
fn quote_for(text:&str, predicate:impl Fn(&str)->bool,avoid:Option<&str>)->Option<String>{
    let chars:Vec<char>=text.chars().take(24000).collect();
    for start in (0..chars.len()).step_by(20){
        let end=(start+210).min(chars.len());
        let slice=chars[start..end].iter().collect::<String>();
        if predicate(&slice) && avoid!=Some(slice.as_str()){
            return Some(slice.trim().to_string());
        }
    }
    None
}
fn duration_minutes(text:&str)->Option<u32>{
    let lower=text.to_ascii_lowercase();
    let words=lower.split(|c:char|!c.is_ascii_alphanumeric()&&c!='.')
        .filter(|s|!s.is_empty()).collect::<Vec<_>>();
    for pair in words.windows(2){
        let Ok(amount)=pair[0].parse::<f32>()else{continue;};
        if !(0.0..10000.0).contains(&amount){continue;}
        let factor=match pair[1]{
            "minute"|"minutes"|"min"|"mins"=>1.0,
            "hour"|"hours"|"hr"|"hrs"=>60.0,
            "day"|"days"=>1440.0,
            "week"|"weeks"=>10080.0,
            "month"|"months"=>43200.0,
            _=>continue,
        };
        return Some((amount*factor) as u32);
    }
    None
}
/// Course reports contain no generated URLs, fabricated snippets or speculative
/// completion. They reuse Rust's source authenticity and requirement gates.
pub fn source_report(goal:&str,sources:&[SourceEvidence])->Option<VerificationReport>{
    let needed=count(goal)?;
    let mut per_host=BTreeMap::<String,(&SourceEvidence,String,String)>::new();
    for source in sources{
        if !course_page(goal,source){continue;}
        let Some(host)=site(&source.url)else{continue;};
        if per_host.contains_key(&host){continue;}
        let Some(cert_quote)=quote_for(&source.excerpt,cert,None)else{continue;};
        let Some(time_quote)=quote_for(&source.excerpt,duration,
            if duration(&cert_quote){None}else{Some(cert_quote.as_str())})else{continue;};
        per_host.insert(host,(source,cert_quote,time_quote));
    }
    if per_host.len()<needed{return None;}
    let mut findings=Vec::new();
    let mut rows=Vec::new();
    for (host,(source,certificate,time)) in per_host.into_iter().take(needed){
        for quote in [&certificate,&time]{
            if !findings.iter().any(|f:&super::evidence_verifier::GroundedFinding|
                f.url==source.url&&f.quote==*quote){
                findings.push(super::evidence_verifier::GroundedFinding{
                    claim:quote.clone(),quote:quote.clone(),url:source.url.clone()});
            }
        }
        rows.push((host,source.title.clone(),source.url.clone(),certificate,time));
    }
    let mut report=super::evidence_verifier::assemble_extractive_comparison(
        goal,sources,findings).ok()?;
    validate(goal,sources,&report).ok()?;
    let mut summary=String::from("Three course-platform pages with source-exact certificate and duration evidence:\n");
    for (host,title,url,certificate,time) in &rows{
        let status=certificate_status(certificate);
        summary.push_str(&format!("\n{title} ({host})\n{url}\nFree certificate? {status}\nSupporting certificate excerpt: {certificate}\nDuration excerpt: {time}\n"));
    }
    // Prefer an explicitly free certificate; otherwise avoid a published
    // paid-certificate condition. Only then use duration as a tie-breaker.
    // This never equates free enrollment with free certification.
    let free_rows=rows.iter().filter(|r|
        certificate_status(&r.3)=="Free certificate explicitly advertised").collect::<Vec<_>>();
    let candidates=if free_rows.is_empty(){
        rows.iter().filter(|r|
            certificate_status(&r.3)!="Paid certificate or subscription indicated")
            .collect::<Vec<_>>()
    }else{free_rows};
    let candidates=if candidates.is_empty(){rows.iter().collect::<Vec<_>>()}else{candidates};
    if let Some((host,title,url,certificate,time))=candidates.into_iter().filter_map(|row|
        duration_minutes(&row.4).map(|minutes|(minutes,row)))
        .min_by_key(|(minutes,_)|*minutes).map(|(_,row)|row){
        summary.push_str(&format!("\nRecommendation: {title} ({host}) for a beginner choosing among the courses without an observed paid-certificate restriction, using stated duration as a tie-breaker. See {url}. Certificate status: {}. Supporting duration excerpt: {time}\n",certificate_status(certificate)));
    }else{return None;}
    summary.push_str("\nCertificate costs and durations are as stated by the observed websites. Read the provider's current enrollment terms before assuming a certificate is free; this report does not claim independent course-quality assessment.");
    report.summary=summary;
    Some(report)
}
pub fn validate(goal:&str,sources:&[SourceEvidence],report:&VerificationReport)->Result<(),String>{
    let required=count(goal).ok_or("Not a course comparison")?;
    if report.status!="evidence_checked"{return Err("Course comparison has no verified report".into());}
    let hosts=source_hosts(goal,sources);
    if hosts.len()<required{return Err(format!("Only {} of {required} course platforms have observed certificate and duration details",hosts.len()));}
    let mut passed=BTreeSet::new();
    for host in &hosts {
        let quotes=report.findings.iter().filter(|f|site(&f.url).as_deref()==Some(host.as_str()))
            .filter(|f|sources.iter().any(|s|s.url==f.url
                &&browser_skills::verification::quote_present(s,&f.quote)))
            .map(|f|f.quote.as_str()).collect::<Vec<_>>();
        if quotes.iter().any(|q|cert(q))&&quotes.iter().any(|q|duration(q)){
            passed.insert(host.clone());
        }
    }
    if passed.len()<required{return Err(format!("Only {} of {required} course platforms have source-exact certificate and duration findings",passed.len()));}
    Ok(())
}
#[cfg(test)]
mod tests{
    use super::*;
    use super::super::evidence_verifier::GroundedFinding;
    const GOAL:&str="Find three free Python courses for beginners from different learning platforms. Check whether they provide free certificates, compare their duration and recommend the best one.";
    fn source(host:&str)->SourceEvidence{
        SourceEvidence{url:format!("https://{host}/learn/python"),title:"Python beginner course".into(),
        excerpt:"Beginner Python course. Certificate: free and included. Duration: 12 hours of lessons.".into()}
    }
    fn finding(s:&SourceEvidence,quote:&str)->GroundedFinding{
        GroundedFinding{url:s.url.clone(),claim:"Observed information".into(),quote:quote.into()}
    }
    #[test]fn three_distinct_official_sources_are_required(){
        assert_eq!(count(GOAL),Some(3));
        let a=source("alpha.edu");let b=source("beta.org");let c=source("gamma.com");
        let report=VerificationReport{status:"evidence_checked".into(),summary:"Checked three platforms".into(),
            limitations:"".into(),findings:vec![
                finding(&a,"Certificate: free and included."),finding(&a,"Duration: 12 hours"),
                finding(&b,"Certificate: free and included."),finding(&b,"Duration: 12 hours"),
                finding(&c,"Certificate: free and included."),finding(&c,"Duration: 12 hours")]};
        assert!(validate(GOAL,&[a.clone(),b.clone()],&report).is_err());
        assert!(validate(GOAL,&[a.clone(),b.clone(),c.clone()],&report).is_ok());
        let mut incomplete=report.clone();incomplete.findings.pop();
        assert!(validate(GOAL,&[a,b,c],&incomplete).is_err());
    }
    #[test]fn a_free_course_does_not_automatically_mean_a_free_certificate(){
        assert_eq!(certificate_status("Free Python course with a certificate of completion"),
            "Free certificate not confirmed by the supporting excerpt");
        assert_eq!(certificate_status("Get course + certificate with Pro+ subscription"),
            "Paid certificate or subscription indicated");
        assert_eq!(certificate_status("A free certificate is included"),
            "Free certificate explicitly advertised");
    }
    #[test]fn source_exact_extractive_report_passes_only_complete_course_evidence(){
        let a=source("alpha.edu");let b=source("beta.org");let c=source("gamma.com");
        let report=source_report(GOAL,&[a.clone(),b.clone(),c.clone()]).expect("grounded comparison");
        assert!(report.summary.contains("Recommendation:"));
        assert_eq!(report.status,"evidence_checked");
        assert!(validate(GOAL,&[a,b,c],&report).is_ok());
        let missing=source_report(GOAL,&[source("alpha.edu"),source("beta.org")]);
        assert!(missing.is_none());
    }
    #[test]fn discovery_continues_from_a_real_course_site_without_claiming_facts(){
        let s=SourceEvidence{url:"https://course.school.edu/python".into(),
            title:"Free Python beginner course".into(),
            excerpt:"Free Python course for beginners; certificate policy not listed.".into()};
        assert!(source_hosts(GOAL,&[s.clone()]).is_empty());
        let next=next_search(GOAL,&[s],"https://course.school.edu/python").expect("new source search");
        assert_eq!(next.host_str(),Some("html.duckduckgo.com"));
        assert!(next.query_pairs().any(|(k,v)|k=="q"&&v.contains("-site:school.edu")));
    }
    #[test]fn irrelevant_observed_article_recovers_search_without_becoming_evidence(){
        let wrong=SourceEvidence{url:"https://example.org/articles/python-career-tips".into(),
            title:"Career advice for programmers".into(),
            excerpt:"General technology careers and unrelated news.".into()};
        assert!(source_hosts(GOAL,&[wrong.clone()]).is_empty());
        let next=next_search(GOAL,&[wrong.clone()],&wrong.url)
            .expect("actually visited irrelevant article should permit a new research search");
        assert_eq!(next.host_str(),Some("html.duckduckgo.com"));
        assert!(next.query_pairs().any(|(k,q)|k=="q"&&q.contains("-site:example.org")));
        assert!(next_search(GOAL,&[wrong.clone()],"https://unvisited.edu/course/python").is_none());
        let mut many=vec![wrong];
        for host in ["one.edu","two.org","three.net","four.com"]{
            many.push(SourceEvidence{url:format!("https://{host}/article"),
                title:"Unrelated article".into(),excerpt:"No course information".into()});
        }
        assert!(next_search(GOAL,&many,"https://four.com/article").is_none());
        assert!(source_report(GOAL,&many).is_none());
    }
    #[test]fn search_results_and_unrelated_domains_are_not_evidence(){
        let a=source("www.bing.com");let b=source("alpha.edu");
        assert_eq!(source_hosts(GOAL,&[a,b]).len(),1);
        assert_eq!(count("Buy three Python courses and certificates of duration 1 hour"),None);
    }
}
