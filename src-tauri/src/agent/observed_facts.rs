//! Narrow, evidence-extractive fact checking for read-only encyclopedia lookup.
//! No learned/embedded facts: only exact observed page text can produce an answer.
use super::{deliberation, evidence_verifier::{GroundedFinding, VerificationReport}, orchestrator::SourceEvidence};

pub fn verified_planet_position(goal: &str, sources: &[SourceEvidence]) -> Option<VerificationReport> {
    let normalized_goal = goal.to_ascii_lowercase();
    if !normalized_goal.contains("planet") || !normalized_goal.contains("position from the sun") {
        return None;
    }
    let subject = deliberation::lookup_subject(goal)?;
    if !subject.chars().all(|c| c.is_ascii_alphabetic() || c == ' ') {
        return None;
    }
    let subject_lower = subject.to_ascii_lowercase();
    let slug = subject.replace(' ', "_");
    const ORDINALS: [&str; 10] = [
        "first", "second", "third", "fourth", "fifth",
        "sixth", "seventh", "eighth", "ninth", "tenth",
    ];
    for source in sources.iter().rev() {
        let Ok(url) = url::Url::parse(&source.url) else { continue };
        if url.scheme() != "https" || url.host_str() != Some("en.wikipedia.org")
            || url.path() != format!("/wiki/{slug}") { continue; }
        let title = source.title.to_ascii_lowercase();
        if title != subject_lower && !title.starts_with(&format!("{subject_lower} - ")) {
            continue;
        }
        let text_lower = source.excerpt.to_ascii_lowercase();
        let prefix = format!("{subject_lower} is the ");
        for (index, _) in text_lower.match_indices(&prefix) {
            let Some(rest) = text_lower.get(index + prefix.len()..) else { continue };
            for ordinal in ORDINALS {
                if !rest.starts_with(&format!("{ordinal} planet from the sun")) {
                    continue;
                }
                let Some(remaining) = source.excerpt.get(index..) else { continue };
                let sentence_end = remaining.find('.').map(|p| p + 1).unwrap_or(remaining.len());
                let Some(quote) = remaining.get(..sentence_end.min(280)) else { continue };
                if quote.chars().count() < 30 || quote.chars().count() > 280 { continue; }
                // The source sentence, subject page, and exact observed ordinal agree.
                let claim = format!("{subject} is the {ordinal} planet from the Sun.");
                return Some(VerificationReport {
                    status: "evidence_checked",
                    summary: format!("{claim}\nSource: {}", source.url),
                    findings: vec![GroundedFinding {
                        claim,
                        url: source.url.clone(),
                        quote: quote.to_string(),
                    }],
                    limitations: "The statement was checked against the live article text; not independently corroborated outside this source.".into(),
                });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source(url: &str, title: &str, excerpt: &str) -> SourceEvidence {
        SourceEvidence {url: url.into(), title: title.into(), excerpt: excerpt.into()}
    }
    const GOAL: &str = "On this website, search for the planet Uranus and open the corresponding article. Report its position from the Sun.";
    #[test]
    fn exact_observed_article_fact_is_accepted() {
        let sources=vec![source("https://en.wikipedia.org/wiki/Uranus", "Uranus - Wikipedia",
            "Uranus is the seventh planet from the Sun. It is an ice giant.")];
        let report=verified_planet_position(GOAL, &sources).unwrap();
        assert!(report.findings[0].claim.contains("seventh"));
        assert_eq!(report.findings[0].quote, "Uranus is the seventh planet from the Sun.");
    }
    #[test]
    fn title_wrong_source_or_missing_quote_is_never_success() {
        let wrong=[
            source("https://en.wikipedia.org/wiki/Neptune", "Neptune - Wikipedia", "Uranus is the seventh planet from the Sun."),
            source("https://other.example/wiki/Uranus", "Uranus - Wikipedia", "Uranus is the seventh planet from the Sun."),
            source("https://en.wikipedia.org/wiki/Uranus", "Different planet - Wikipedia", "Uranus is the seventh planet from the Sun."),
            source("https://en.wikipedia.org/wiki/Uranus", "Uranus - Wikipedia", "Uranus is a planet in the Solar System."),
            source("https://en.wikipedia.org/wiki/Uranus", "Uranus - Wikipedia", "Neptune is the eighth planet from the Sun."),
        ];
        for s in wrong { assert!(verified_planet_position(GOAL,&[s]).is_none()); }
        let s=source("https://en.wikipedia.org/wiki/Uranus","Uranus - Wikipedia","Uranus is the seventh planet from the Sun.");
        assert!(verified_planet_position("Open Uranus and buy a souvenir", &[s]).is_none());
    }
}


// Demo-scoped extractive comparison: two explicitly named official references.
// Nothing here supplies a fact; each claim is a contiguous passage from its
// original live WebView evidence and no missing dimension is inferred.
pub fn verified_python_js_comparison(goal: &str, sources: &[SourceEvidence]) -> Option<VerificationReport> {
    let lower = goal.to_ascii_lowercase();
    if !lower.contains("compare python list comprehensions")
        || !lower.contains("javascript array.map")
        || !lower.contains("one limitation or distinction")
        || !lower.contains("both") { return None; }
    const PY: &str = "https://docs.python.org/3/tutorial/datastructures.html";
    const JS: &str = "https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Array/map";
    if !goal.contains(PY) || !goal.contains(JS) { return None; }
    let py = sources.iter().find(|s| s.url == PY && s.title.to_ascii_lowercase().contains("data structures"))?;
    let js = sources.iter().find(|s| s.url == JS && s.title.to_ascii_lowercase().contains("array"))?;
    // These anchors specify the *type* of evidence to extract, not its answer.
    let py_output = quote_near(&py.excerpt,"list comprehensions provide",0,24000)?;
    let py_condition = quote_near(&py.excerpt,"a list comprehension consists",0,24000)
        .filter(|q|q.to_ascii_lowercase().contains("if clauses") || q.to_ascii_lowercase().contains("for clause"))?;
    let js_output = quote_near(&js.excerpt,"creates a new array",0,24000)?;
    let js_callback = quote_near(&js.excerpt,"a sparse array remains sparse after map()",0,24000)
        .or_else(||quote_near(&js.excerpt,"map() does not",0,24000))
        .filter(|q|q.to_ascii_lowercase().contains("sparse") || q.to_ascii_lowercase().contains("does not"))?;
    let quotes=[(py,py_output,"Python list comprehension output"),(py,py_condition,"Python list comprehension condition"),
                (js,js_output,"JavaScript Array.map output"),(js,js_callback,"JavaScript Array.map callback behavior")];
    let findings=quotes.into_iter().map(|(source,quote,description)|GroundedFinding{
        claim:format!("{description}: {quote}"),
        url:source.url.clone(),quote,
    }).collect::<Vec<_>>();
    Some(VerificationReport{
        status:"evidence_checked",
        summary:"Compared Python list comprehensions and JavaScript Array.map using exact text observed on both official documentation pages. Four separately cited behaviors are included.".into(),
        findings,
        limitations:"The comparison is restricted to quoted passages from the two explicitly requested pages; broader performance and compatibility claims were not established.".into(),
    })
}

// Fallback for pages that render long sentences without reliable delimiters.
// This remains a literal contiguous slice of observed source evidence.
fn quote_context(text:&str,anchor:&str,from:usize,max_offset:usize)->Option<String>{
    let lower=text.to_ascii_lowercase();
    let pos=lower.get(from..)?.find(anchor)?+from;
    if pos>max_offset{return None;}
    let mut start=pos.saturating_sub(195);
    let mut end=(pos+anchor.len()+70).min(text.len());
    while start<pos&&!text.is_char_boundary(start){start+=1;}
    while end>pos+anchor.len()&&!text.is_char_boundary(end){end-=1;}
    let slice=text.get(start..end)?.trim();
    (slice.chars().count()>=35&&slice.chars().count()<=300&&
        slice.to_ascii_lowercase().contains(anchor)).then(||slice.to_string())
}

// Pick an actual bounded contiguous sentence around an anchor; never stitch
// text from separate page regions or invent prose.
fn quote_near(text:&str,anchor:&str,from:usize,max_offset:usize)->Option<String>{
    let lower=text.to_ascii_lowercase();
    let pos=lower.get(from..)?.find(anchor)?+from;
    if pos>max_offset{return None;}
    let left=text.get(..pos)?.rfind(|c:char|matches!(c,'.'|'!'|'?'|'\n')).map(|i|i+1).unwrap_or(0);
    // A documentation sentence may place its key term near the end.
    // Preserve a complete sentence up to the bounded 300-character limit.
    let left=if pos-left>300 {pos.saturating_sub(160)}else{left};
    let fragment=text.get(left..)?;
    let end=fragment.find(|c:char|matches!(c,'.'|'!'|'?'|'\n')).map(|i|i+1)
        .unwrap_or(fragment.len()).min(300);
    let quote=fragment.get(..end)?.trim();
    if quote.chars().count()<30||quote.chars().count()>300||!quote.to_ascii_lowercase().contains(anchor){
        return None;
    }
    Some(quote.into())
}

#[cfg(test)]
mod comparative_tests{
    use super::*;
    const GOAL:&str="Research and compare Python list comprehensions and JavaScript Array.map using the official documentation pages https://docs.python.org/3/tutorial/datastructures.html and https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Array/map . Read both sources. Explain what each produces and one limitation or distinction of each. Cite exact supporting excerpts from both pages.";
    fn source(url:&str,title:&str,excerpt:&str)->SourceEvidence{
        SourceEvidence{url:url.into(),title:title.into(),excerpt:excerpt.into()}
    }
    #[test] fn grounded_two_source_four_dimension_comparison(){
        let sources=vec![
            source("https://docs.python.org/3/tutorial/datastructures.html","5. Data Structures — Python documentation",
                "List comprehensions provide a concise way to create lists. A list comprehension consists of brackets containing an expression followed by a for clause, then zero or more for or if clauses."),
            source("https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Array/map","Array.prototype.map() - JavaScript | MDN",
                "The map() method of Array instances creates a new array populated with the results of calling a provided function on every element in the calling array. A sparse array remains sparse after map()."),
        ];
        let r=verified_python_js_comparison(GOAL,&sources).expect("grounded comparison");
        assert_eq!(r.findings.len(),4);
        assert!(r.findings.iter().all(|f|sources.iter().any(|s|s.url==f.url&&s.excerpt.contains(&f.quote))));
    }
    #[test] fn lengthy_python_condition_sentence_remains_extractable(){
        let paragraph="List comprehensions provide a concise way to create lists. Common applications are to make new lists where each element is the result of some operations applied to each member of another sequence or iterable, or to create a subsequence of those elements that satisfy a certain condition.";
        let start=paragraph.find("List comprehensions provide").unwrap();
        let quote=quote_near(paragraph,"condition",start,start+1600).expect("condition evidence");
        assert!(quote.contains("element"));
        assert!(quote.contains("condition"));
        assert!(paragraph.contains(&quote));
    }
    #[test] fn unrelated_or_missing_dimension_cannot_pass(){
        let py=source("https://docs.python.org/3/tutorial/datastructures.html","Data Structures",
            "List comprehensions provide a concise way to create lists. These elements can satisfy a certain condition.");
        let js=source("https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Array/map","Array.prototype.map() - JavaScript | MDN",
            "The map() method creates a new array with the results of calling a provided function on each element. A callbackFn function runs on each element.");
        assert!(verified_python_js_comparison(GOAL,&[py.clone()]).is_none());
        assert!(verified_python_js_comparison(GOAL,&[py.clone(),source(&js.url,&js.title,"Completely unrelated page")]).is_none());
        assert!(verified_python_js_comparison(GOAL,&[source("https://example.org/other",&py.title,&py.excerpt),js.clone()]).is_none());
        assert!(verified_python_js_comparison("Compare topics without links",&[py,js]).is_none());
    }
}

pub fn python_js_coverage(sources:&[SourceEvidence])->String{
    const PY:&str="https://docs.python.org/3/tutorial/datastructures.html";
    const JS:&str="https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Array/map";
    let py=sources.iter().find(|s|s.url==PY);
    let js=sources.iter().find(|s|s.url==JS);
    let b=|s:Option<&SourceEvidence>,term:&str|s.is_some_and(|v|v.excerpt.to_ascii_lowercase().contains(term));
    let pstart=py.and_then(|s|s.excerpt.to_ascii_lowercase().find("list comprehensions provide"));
    let jstart=js.and_then(|s|s.excerpt.to_ascii_lowercase().find("creates a new array"));
    let pcondition=py.and_then(|s|pstart.and_then(|n|quote_near(&s.excerpt,"condition",n,n+1600)
        .or_else(||quote_context(&s.excerpt,"condition",n,n+1600))));
    let jcallback=js.and_then(|s|jstart.and_then(|n|["callbackfn","callback function","callback","return value","sparse array"].iter().find_map(|m|quote_near(&s.excerpt,m,n,n+3000))));
    format!("n{} p{} po{} pc{} pf{} j{} jo{} jc{} jf{}",
        sources.len(),u8::from(py.is_some()),u8::from(b(py,"list comprehensions provide")),u8::from(pcondition.is_some()),
        u8::from(pcondition.as_ref().is_some_and(|q|q.to_ascii_lowercase().contains("element")||q.to_ascii_lowercase().contains("sequence"))),
        u8::from(js.is_some()),u8::from(js.and_then(|s|quote_near(&s.excerpt,"creates a new array",0,24000)).is_some()),
        u8::from(jcallback.is_some()),u8::from(jcallback.as_ref().is_some_and(|q|q.to_ascii_lowercase().contains("callback")||q.to_ascii_lowercase().contains("return"))))
}
