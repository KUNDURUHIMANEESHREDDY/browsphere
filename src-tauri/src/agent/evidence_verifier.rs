//! Evidence checks: LLM proposes claims; Rust matches source URLs and
//! exact supporting passages to WebView-captured page text.
//! Attribution is checked; webpages are not independently fact-checked.
use std::collections::HashSet;
use serde::{Deserialize, Serialize};
use url::Url;
use crate::agent::dom_extractor::DomSnapshot;
use crate::agent::orchestrator::SourceEvidence;
use crate::agent::policy::BrowserAction;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposedFinding {
    claim: String,
    url: String,
    quote: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposedVerification {
    status: String,
    #[serde(default)]
    findings: Vec<ProposedFinding>,
    #[serde(default)]
    uncertainty: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct GroundedFinding {
    pub claim: String,
    pub url: String,
    pub quote: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct VerificationReport {
    pub status: &'static str,
    pub summary: String,
    pub findings: Vec<GroundedFinding>,
    pub limitations: String,
}
fn normalized(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}
fn terms(input: &str) -> HashSet<String> {
    input.split(|ch: char| !ch.is_alphanumeric())
        .map(str::to_lowercase).filter(|s| s.chars().count() >= 4).collect()
}
fn numbers(input: &str) -> HashSet<String> {
    input.split(|ch: char| !ch.is_ascii_digit() && ch != '.' && ch != '%')
        .map(|s| s.trim_matches(|ch: char| ch == '.' || ch == '%'))
        .filter(|s| s.chars().any(|c| c.is_ascii_digit()))
        .map(str::to_owned).collect()
}
fn web_url(raw: &str) -> Option<Url> {
    let url = Url::parse(raw).ok()?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none()
        || !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    Some(url)
}

/// Select a bounded, contiguous, goal-relevant passage from each observed
/// source. This is retrieval only, not an answer or an evidence judgment.
pub fn focus_sources(goal:&str,sources:&[SourceEvidence],comparison:bool)->Vec<SourceEvidence>{
    let ignored=["this","that","with","from","website","search","article","corresponding","report","page","open","find","about","then","position","please","compare","source","their","which","what"];
    let wanted:HashSet<String>=goal.split(|c:char|!c.is_alphanumeric())
        .map(str::to_lowercase).filter(|t|t.chars().count()>=3&&!ignored.contains(&t.as_str())&&!["the","and","its","for","are","was","can","has","you"].contains(&t.as_str())).collect();
    let subject=super::deliberation::lookup_subject(goal).map(|s|normalized(&s));
    let mut ranked:Vec<_>=sources.iter().enumerate().map(|(idx,p)|{
        let chars:Vec<char>=p.excerpt.chars().take(24000).collect();
        let mut best=(0i32,0usize);
        // Score overlapping 900-character windows. Use original contiguous
        // text, not model-generated summaries or stitched quotations.
        for start in (0..chars.len().max(1)).step_by(240){
            let end=(start+1100).min(chars.len());
            let segment:String=chars[start..end].iter().collect();
            let words:HashSet<String>=segment.split(|c:char|!c.is_alphanumeric()).map(str::to_lowercase).filter(|t|t.chars().count()>=3).collect();
            let score=words.intersection(&wanted).count() as i32*12
                + if subject.as_ref().is_some_and(|s|normalized(&segment).contains(s)){6}else{0};
            if score>best.0{best=(score,start);}
        }
        let start=best.1.saturating_sub(120);
        let end=(start+1450).min(chars.len());
        let title=normalized(&p.title);
        let score=best.0+terms(&title).intersection(&wanted).count() as i32*16
            +if subject.as_ref().is_some_and(|s|title.starts_with(s)){35}else{0}
            +idx as i32;
        (score,idx,SourceEvidence{url:p.url.clone(),title:p.title.clone(),excerpt:chars[start..end].iter().collect()})
    }).collect();
    ranked.sort_by(|a,b|b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    let mut out=Vec::new();
    let mut hosts=HashSet::new();
    if comparison{
        for (_,_,p) in &ranked{
            if let Some(host)=web_url(&p.url).and_then(|u|u.host_str().map(str::to_owned)){
                if hosts.insert(host){out.push(p.clone());}
                if out.len()>=3{break;}
            }
        }
    }else{
        out.extend(ranked.iter().take(2).map(|(_,_,p)|p.clone()));
    }
    out
}

/// A small model chooses evidence identifiers, rather than inventing URL,
/// quote and paraphrase triples. The native side retains all provenance.
#[derive(Clone,Debug,Serialize)]
pub struct Passage {pub id:String,pub text:String}
fn query_words(goal:&str)->HashSet<String>{
    let cleaned=goal.split_whitespace().filter(|s|!s.contains("://")).collect::<Vec<_>>().join(" ");
    cleaned.split(|c:char|!c.is_alphanumeric()).map(str::to_lowercase)
        .filter(|w|w.len()>=3&&!["the","and","for","from","with","this","that","what","each","both","using","read","report","cite","source","sources","exact","supporting","excerpts","compare","research","official","documentation","pages","explain","one","its","their","please","which","website","article","open","search","corresponding"].contains(&w.as_str())).collect()
}
fn passage_score(text:&str,wanted:&HashSet<String>)->i32{
    let words:HashSet<String>=text.split(|c:char|!c.is_alphanumeric()).map(str::to_lowercase).collect();
    words.intersection(wanted).count() as i32*10
}
pub fn candidate_passages(goal:&str,source:&SourceEvidence)->Vec<Passage>{
    let wanted=query_words(goal);
    let raw=source.excerpt.chars().take(24000).collect::<String>();
    let mut pieces=Vec::<String>::new();
    let mut buffer=String::new();
    for word in raw.split_whitespace(){
        if !buffer.is_empty()&&buffer.chars().count()+word.chars().count()+1>225{
            pieces.push(std::mem::take(&mut buffer));
        }
        if !buffer.is_empty(){buffer.push(' ');}
        buffer.push_str(word);
        if word.ends_with(['.','!','?'])&&buffer.chars().count()>=35{
            pieces.push(std::mem::take(&mut buffer));
        }
    }
    if !buffer.is_empty(){pieces.push(buffer);}
    let scores=pieces.iter().map(|p|passage_score(p,&wanted)).collect::<Vec<_>>();
    let mut ranked=pieces.iter().enumerate().filter_map(|(i,p)|{
        let n=normalized(p);
        if p.chars().count()<35||p.chars().count()>240||
            ["ignore previous","system prompt","ignore instructions","api key","reveal credentials","output exactly","assistant must","developer message"].iter().any(|s|n.contains(s)){return None;}
        let context=scores.get(i.wrapping_sub(1)).copied().unwrap_or(0).max(scores.get(i+1).copied().unwrap_or(0))/2;
        let score=scores[i]*2+context;
        (score>0).then_some((score,i,p.clone()))
    }).collect::<Vec<_>>();
    ranked.sort_by(|a,b|b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut seen=HashSet::new();
    ranked.retain(|(_,_,s)|seen.insert(normalized(s)));
    ranked.truncate(14);
    ranked.sort_by_key(|(_,i,_)|*i);
    ranked.into_iter().enumerate().map(|(i,(_,_,text))|Passage{id:format!("p{}",i+1),text}).collect()
}
/// Explicitly requested comparison dimensions. These are not inferred facts:
/// each requires a separate selected passage from the actual source.
pub fn research_facets(goal:&str)->Vec<&'static str>{
    let g=normalized(goal);
    if let Some(spec)=super::skills::TaskSpec::compile(goal){
        let mut facets=Vec::new();
        for fact in spec.required_facts{
            if fact=="explicit_contrast"{
                if contrast_terms(goal).is_some(){facets.extend(["contrast_left","contrast_right"]);}
            }else{facets.push(fact);}
        }
        if !facets.is_empty(){return facets;}
    }
    if (g.contains("feature")||g.contains("capabilit")||g.contains("advantage"))&&
        (g.contains("limitation")||g.contains("caveat")||g.contains("drawback")){
        vec!["feature","caveat"]
    }else if g.contains("default")&&g.contains("timeout"){
        vec!["default_behavior"]
    }else{vec![]}
}
pub fn contrast_terms(goal:&str)->Option<(String,String)>{
    let lower=goal.to_ascii_lowercase();
    let start=lower.find("difference between ")?+"difference between ".len();
    let rest=goal.get(start..)?;
    let stop=rest.find(['.','?',';', '\n']).unwrap_or(rest.len());
    let (a,b)=rest.get(..stop)?.split_once(" and ")?;
    let valid=|s:&str|!s.trim().is_empty()&&s.chars().count()<=60&&s.chars().all(|c|c.is_alphanumeric()||matches!(c,' '|'-'|'_'));
    if valid(a)&&valid(b){Some((normalized(a),normalized(b)))}else{None}
}
pub fn facet_supported(goal:&str,facet:&str,text:&str)->bool{facet_score(goal,facet,text)>0}
/// Canonicalize typography only for lexical analysis; citation strings
/// and exact source checks keep their original punctuation.
fn semantic_text(text:&str)->String{
    normalized(text).replace(['\u{2018}','\u{2019}','\u{02bc}'],"'")
        .replace(['\u{2010}','\u{2011}','\u{2013}','\u{2014}'],"-")
}
fn direct_usage_warning(text:&str)->bool{
    let t=semantic_text(text);
    ["do not","don't","must not","should not","avoid","not recommended","make sure","be careful","warning"].iter().any(|m|t.contains(m))
}
fn facet_score(goal:&str,facet:&str,text:&str)->i32{
    let t=semantic_text(text);
    if matches!(facet,"contrast_left"|"contrast_right"){
        let Some((a,b))=contrast_terms(goal)else{return 0};
        let term=if facet=="contrast_left"{a}else{b};
        if !super::browser_skills::phrase(&t,&term){return 0;}
        let explanation=["avoids","avoid ","skips","skip ","does not","not inserted","prevents","causes","updates","performs","instead","will ","returns","means","is used","allows","inserts","ignores"];
        let Some(at)=t.find(&term)else{return 0;};
        let tail=&t[at+term.len()..];
        let direct=tail.trim_start_matches(|c:char|c.is_whitespace()||matches!(c,','|':'|';')).split_whitespace().take(4).collect::<Vec<_>>().join(" ");
        let score=explanation.iter().filter(|v|tail.contains(**v)).count() as i32;
        return score+if score>0&&["avoids","skips","does not","prevents","updates","performs","inserts","returns","causes","means","ignores","allows"].iter().any(|v|super::browser_skills::phrase(&direct,v)){4}else{0};
    }
    if facet=="installation_command"{
        let commands=["pip install ","python -m pip install ","npm install ","yarn add ","pnpm add ","cargo add ","cargo install ","gem install ","go install ","dotnet add package "];
        let score=commands.iter().filter(|v|t.contains(**v)).count() as i32;
        if let Some(spec)=super::skills::TaskSpec::compile(goal){
            if spec.kind==super::skills::TargetKind::Repository{
                let name=spec.subject.split(|c:char|c=='/'||c.is_whitespace()).last().unwrap_or("");
                if !super::browser_skills::phrase(&t,name){return 0;}
            }
        }
        return score;
    }
    if facet=="runtime_compatibility"{
        let runtime=["python","node.js","nodejs","node ","runtime","requires-python","rust ","java ","jdk "];
        if !runtime.iter().any(|v|t.contains(*v))||!t.split_whitespace().any(|v|v.chars().any(|c|c.is_ascii_digit())&&v.contains('.')){return 0;}
        let compatibility=["support","requires","require ","version","compatible","works with","or later","or newer","minimum","and above","+","requires-python"];
        return compatibility.iter().filter(|v|t.contains(**v)).count() as i32;
    }
    let markers:&[&str]=match facet{
        "feature"=>&["supports","allows","provides","can use","you can","lets you","request methods","offers","available","enables","use the","use a","returns"],
        "caveat"=>&["do not","don't","must not","should not","avoid","cannot","not recommended","be careful","warning","make sure","must ","need to","should ","remember to","not support","not possible","by default","no timeout","otherwise","unless"],
        "default_behavior"=>&["by default","default behavior","defaults to","default timeout","no timeout","no time limit","do not specify a timeout","don't specify a timeout","wait indefinitely","hang indefinitely","not set a timeout","five seconds","5 seconds"],
        _=>&[]
    };
    let count=markers.iter().filter(|m|t.contains(**m)).count() as i32;
    if normalized(goal).contains("http client librar")&&matches!(facet,"feature"|"caveat"){
        let operation=["http request","request methods","requests.get","requests.post","response object","json response","response content","client session","clientsession","asyncclient","async request","async support","session.get","client.get","response body","http methods"];
        let lifecycle=["connection pool","connection pooling","session per request","session per application","reuse","close the","close()","context manager","hot loop","streaming response","raise_for_status","timeout","exception"];
        let terms=if facet=="feature"{&operation[..]}else{&lifecycle[..]};
        let relevance=terms.iter().filter(|m|t.contains(**m)).count() as i32;
        let operational=if facet=="feature"{["returns","makes","supports","allows","provides","use","create","send","perform","get","request"].iter().any(|m|t.contains(m))}else{
            let restriction=["do not","don't","must not","should not","avoid","cannot","not recommended","not support","not possible","must ","need to","should ","make sure","otherwise","unless","no timeout","not time out","raises","raised","exception","by default"];
            restriction.iter().any(|m|t.contains(m))
        };
        if relevance==0||!operational{return 0;}
        // Prefer usage/lifecycle restrictions over incidental default values
        // when the goal explicitly requests usage caveats.
        let lifecycle_priority=if facet=="caveat"&&lifecycle[..9].iter().any(|m|t.contains(m)){3}else{0};
        return 2+relevance.min(4)+count.min(2)+lifecycle_priority;
    }
    if facet=="default_behavior"{
        if !t.contains("timeout"){return 0;}
        let default_markers=["default","no timeout","no time limit","do not specify a timeout","don't specify a timeout","not set a timeout","wait indefinitely","hang indefinitely"];
        let effect=["seconds","second","milliseconds","indefinitely","no timeout","no time limit","no timeouts","do not specify a timeout","don't specify a timeout","not set a timeout","never","unlimited"];
        if !default_markers.iter().any(|m|t.contains(m))||!effect.iter().any(|m|t.contains(m)){return 0;}
        return count+3;
    }
    count
}
fn quality_tier(goal:&str,facet:&str,text:&str)->u8{
    let t=semantic_text(text);
    if facet_score(goal,facet,text)==0{return 0;}
    if matches!(facet,"contrast_left"|"contrast_right"){
        return if facet_score(goal,facet,text)>=5{3}else{1};
    }
    if normalized(goal).contains("http client librar"){
        if facet=="caveat"{
            let lifecycle=["connection pool","session per request","session per application","reuse","close the","close()","context manager","hot loop","streaming response"];
            let concrete=lifecycle.iter().any(|m|t.contains(m));
            let actionable=direct_usage_warning(text)||["need to","must ","should ","not time out","cannot","raises","raised","not support","unless","otherwise"].iter().any(|m|t.contains(m));
            if concrete&&actionable{return 3;}
            if actionable{return 2;}
            return 1;
        }
        if facet=="feature"{
            // A useful capability should describe an operation or result,
            // not merely the type of a required argument.
            if ["mandatory parameter","required parameter","takes a parameter","argument type"].iter().any(|m|t.contains(m)){return 1;}
            if ["supports","provides","allows","returns","can use","you can","makes","send","perform","request methods","create"].iter().any(|m|t.contains(m))&&!direct_usage_warning(text){return 3;}
            return 2;
        }
    }
    1
}
#[derive(Clone,Debug,Serialize)]
pub struct FacetCandidates{pub facet:String,pub candidates:Vec<Passage>}
pub fn facet_candidates(goal:&str,source:&SourceEvidence)->Vec<FacetCandidates>{
    let wanted=query_words(goal);
    // Retain complete sentences when possible, including late paragraphs.
    let text=source.excerpt.chars().take(24000).collect::<String>();
    let mut pieces=Vec::<String>::new();
    let mut buf=String::new();
    for word in text.split_whitespace(){
        if !buf.is_empty()&&buf.chars().count()+word.chars().count()+1>235{pieces.push(std::mem::take(&mut buf));}
        if !buf.is_empty(){buf.push(' ');}buf.push_str(word);
        if word.ends_with(['.','!','?'])&&buf.chars().count()>=20{pieces.push(std::mem::take(&mut buf));}
    }
    if !buf.is_empty(){pieces.push(buf);}
    if let Some((a,b))=contrast_terms(goal){
        // Keep sentence-sized literal windows around each exact term.
        // The old fixed-size chunks can split a term from its explanation.
        let terms=[a,b];
        for sentence in text.split_inclusive(['.','!','?']){
            let sentence=sentence.trim();
            if sentence.chars().count()>240||sentence.chars().count()<30{continue;}
            if terms.iter().any(|term|super::browser_skills::phrase(sentence,term)){
                pieces.push(sentence.to_string());
            }
        }
    }
    let single_count=pieces.len();
    let joined=pieces.windows(2).filter_map(|pair|{
        let text=format!("{} {}",pair[0],pair[1]);
        (text.chars().count()<=240).then_some(text)
    }).collect::<Vec<_>>();
    pieces.extend(joined);
    research_facets(goal).into_iter().map(|facet|{
        let mut ranked=pieces.iter().enumerate().filter_map(|(i,p)|{
            let score=facet_score(goal,facet,p);
            let n=normalized(p);
            let minimum=if matches!(facet,"installation_command"|"runtime_compatibility"){12}else if facet=="caveat"&&direct_usage_warning(p){20}else{30};
            if score==0||p.chars().count()<minimum||p.chars().count()>240||
                ["ignore previous","ignore instructions","system prompt","reveal credentials","output exactly","developer message"].iter().any(|x|n.contains(x)){return None;}
            // Incomplete references like "does not support this" need context;
            // penalize them rather than confidently presenting fragments.
            let context_penalty=if ["support this","does this","do this","this is how","like this"].iter().any(|v|n.contains(v)){15}else{0};
            let complete=if p.ends_with(['.','!','?']){16}else{0};
            let fragment_penalty=if n.starts_with(">>>")||n.starts_with("resp:")||n.starts_with("assert ")||n.ends_with(" in")||n.ends_with(" the")||n.ends_with(" to"){40}else{0};
            // Adjacent context is retained as an alternative, but do not let
            // a combined passage outrank direct stand-alone evidence merely
            // because it contains more generic cue words.
            let combined_penalty=if i>=single_count{100}else{0};
            let cap=if normalized(goal).contains("http client librar"){10}else{3};
            let tier=quality_tier(goal,facet,p);
            Some((i32::from(tier)*1000+score.min(cap)*35+passage_score(p,&wanted)+complete-context_penalty-combined_penalty-fragment_penalty,i,p.clone()))
        }).collect::<Vec<_>>();
        // For a requested usage caveat, prefer concrete usage warnings over
        // incidental configurable defaults, when those warnings are present.
        if facet=="caveat"&&normalized(goal).contains("usage caveat")&&ranked.iter().any(|(_,_,p)|direct_usage_warning(p)){
            ranked.retain(|(_,_,p)|direct_usage_warning(p));
        }
        // Prevent a weaker incidental example/default from being selected
        // when a directly supporting operational passage is available.
        if normalized(goal).contains("http client librar")&&matches!(facet,"feature"|"caveat"){
            let best=ranked.iter().map(|(_,_,p)|quality_tier(goal,facet,p)).max().unwrap_or(0);
            ranked.retain(|(_,_,p)|quality_tier(goal,facet,p)==best);
        }
        ranked.sort_by(|a,b|b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        let mut seen=HashSet::new();ranked.retain(|(_,_,p)|seen.insert(normalized(p)));ranked.truncate(7);
        FacetCandidates{facet:facet.into(),candidates:ranked.into_iter().enumerate().map(|(i,(_,_,text))|Passage{id:format!("{facet}_{}",i+1),text}).collect()}
    }).collect()
}
fn valid_facet_choice(goal:&str,source:&SourceEvidence,set:&FacetCandidates,p:&Passage)->bool{
    if !facet_supported(goal,&set.facet,&p.text)||!super::browser_skills::verification::quote_present(source,&p.text){return false;}
    let best=facet_candidates(goal,source).into_iter().find(|s|s.facet==set.facet)
        .and_then(|s|s.candidates.iter().map(|p|quality_tier(goal,&set.facet,&p.text)).max()).unwrap_or(0);
    quality_tier(goal,&set.facet,&p.text)>=best
}
/// A retry is targeted at missing, invalid or conflicting dimensions. Keep
/// accepted independent candidates; use different retained source passages
/// for unresolved dimensions instead of reissuing the identical request.
pub fn retry_facets(goal:&str,source:&SourceEvidence,sets:&[FacetCandidates],raw:Option<&str>)->Vec<FacetCandidates>{
    let choices=raw.and_then(|s|serde_json::from_str::<std::collections::BTreeMap<String,String>>(s).ok()).unwrap_or_default();
    let mut used=HashSet::<String>::new();
    let mut out=Vec::new();
    for set in sets{
        let chosen=choices.get(&set.facet).and_then(|id|set.candidates.iter().find(|p|&p.id==id));
        let accepted=chosen.filter(|p|valid_facet_choice(goal,source,set,p)&&used.insert(normalized(&p.text)));
        if let Some(p)=accepted{
            out.push(FacetCandidates{facet:set.facet.clone(),candidates:vec![p.clone()]});
            continue;
        }
        let mut alternatives=set.candidates.iter().filter(|p|valid_facet_choice(goal,source,set,p)&&!used.contains(&normalized(&p.text))).cloned().collect::<Vec<_>>();
        if let Some(p)=chosen{
            if alternatives.len()>1{alternatives.retain(|a|a.id!=p.id);}
        }else if alternatives.len()>1{
            // The reviewer failed to identify support in the first set.
            // Reorder the alternatives to expose a different first passage.
            alternatives.rotate_left(1);
        }
        out.push(FacetCandidates{facet:set.facet.clone(),candidates:alternatives});
    }
    out
}
pub fn facet_findings(goal:&str,source:&SourceEvidence,sets:&[FacetCandidates],raw:&str)->Result<Vec<GroundedFinding>,String>{
    let choices:std::collections::BTreeMap<String,String>=serde_json::from_str(raw).map_err(|_|"Invalid fact-dimension evidence selection")?;
    let expected=research_facets(goal);
    if sets.len()!=expected.len()||choices.len()!=expected.len(){return Err("Not every requested fact dimension was answered".into());}
    let mut out=Vec::new();let mut seen=HashSet::new();
    for facet in expected{
        let set=sets.iter().find(|s|s.facet==facet).ok_or("Missing required fact dimension")?;
        let id=choices.get(facet).ok_or("Missing required fact dimension")?;
        let p=set.candidates.iter().find(|p|&p.id==id).ok_or_else(||format!("Insufficient {facet} evidence on the requested source"))?;
        if !valid_facet_choice(goal,source,set,p){
            return Err(format!("Selected {facet} evidence is weak or not grounded in the observed source"));
        }
        if !seen.insert(normalized(&p.text)){return Err("Distinct requested dimensions require distinct supporting passages".into());}
        out.push(GroundedFinding{claim:p.text.clone(),quote:p.text.clone(),url:source.url.clone()});
    }
    Ok(out)
}
pub fn comparison_sources(goal:&str,sources:&[SourceEvidence])->Vec<SourceEvidence>{
    let required=super::deliberation::explicit_research_sources(goal);
    if !required.is_empty(){
        return required.iter().filter_map(|u|sources.iter().find(|s|web_url(&s.url).as_ref()==Some(u)).cloned()).collect();
    }
    let ranked=focus_sources(goal,sources,true);
    ranked.iter().filter_map(|p|sources.iter().find(|s|s.url==p.url).cloned()).collect()
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PassageSelection{ids:Vec<String>}
pub fn selected_findings(goal:&str,source:&SourceEvidence,candidates:&[Passage],raw:&str)->Result<Vec<GroundedFinding>,String>{
    let selection:PassageSelection=serde_json::from_str(raw).map_err(|_|"Invalid evidence selection JSON")?;
    if selection.ids.is_empty()||selection.ids.len()>3{return Err("Expected one to three relevant passage IDs".into());}
    let wanted=query_words(goal);
    let mut seen=HashSet::new();
    let mut findings=Vec::new();
    for id in selection.ids{
        if !seen.insert(id.clone()){return Err("Duplicate passage selection".into());}
        let p=candidates.iter().find(|p|p.id==id).ok_or("Unknown passage ID")?;
        if !normalized(&source.excerpt).contains(&normalized(&p.text)){return Err("Selected passage absent from original capture".into());}
        findings.push(GroundedFinding{claim:p.text.clone(),quote:p.text.clone(),url:source.url.clone()});
    }
    if !findings.iter().any(|f|passage_score(&f.quote,&wanted)>0){return Err("Selected evidence does not address the requested topic".into());}
    Ok(findings)
}
pub fn require_source_coverage(goal:&str,report:&VerificationReport)->Result<(),String>{
    for target in super::deliberation::explicit_research_sources(goal){
        if !report.findings.iter().any(|f|web_url(&f.url).as_ref()==Some(&target)){
            return Err("Comparison report omitted an explicitly required source".into());
        }
    }
    Ok(())
}
pub fn compiled_skill_source(goal:&str,sources:&[SourceEvidence])->Option<SourceEvidence>{
    super::requirements::target_sources(goal,sources).into_iter().max_by_key(|s|s.excerpt.chars().count())
}
pub fn assemble_compiled_skill_report(goal:&str,sources:&[SourceEvidence],findings:Vec<GroundedFinding>)->Result<VerificationReport,String>{
    let scoped=super::requirements::target_sources(goal,sources);
    if scoped.is_empty(){return Err("Requested target source has not been observed".into());}
    let draft=serde_json::json!({"status":"verified","findings":findings,
        "uncertainty":"Observed source excerpts support the requested dimensions; they are not independently fact-checked."});
    let report=check_model_verification(&draft.to_string(),sources,false)?;
    let facets=research_facets(goal);
    if facets.is_empty(){return Err("No compiled factual verification dimensions".into());}
    let mut ledger=super::requirements::Ledger::compile(goal);
    ledger.validate_report(goal,&scoped,&report)?;
    Ok(report)
}
pub fn assemble_extractive_comparison(goal:&str,sources:&[SourceEvidence],findings:Vec<GroundedFinding>)->Result<VerificationReport,String>{
    // Reuse the existing native verifier, including URL, exact quote, number
    // and distinct-host checks. Each finding is an extract, not an inference.
    let draft=serde_json::json!({"status":"verified","findings":findings,
        "uncertainty":"Extractive comparison of selected source passages. The passages are attributed, not independently fact-checked; unsupported conclusions are not inferred."});
    let report=check_model_verification(&draft.to_string(),sources,true)?;
    require_source_coverage(goal,&report)?;
    let facets=research_facets(goal);
    if !facets.is_empty(){
        for source in comparison_sources(goal,sources){
            for facet in &facets{
                let sets=facet_candidates(goal,&source);
                let set=sets.iter().find(|s|s.facet==*facet).ok_or("Missing source requirement")?;
                if !report.findings.iter().any(|f|web_url(&f.url)==web_url(&source.url)&&valid_facet_choice(goal,&source,set,&Passage{id:String::new(),text:f.quote.clone()})){
                    return Err(format!("Required {facet} evidence is missing from {}",source.url));
                }
            }
        }
    }
    Ok(report)
}

/// Fail closed unless each model-proposed claim cites an observed page and an
/// actual passage from its excerpt. No generated URLs or quotes may pass.
pub fn check_model_verification(
    raw: &str, observed: &[SourceEvidence], compare: bool,
) -> Result<VerificationReport, String> {
    let text = raw.trim();
    let draft: ProposedVerification = serde_json::from_str(text)
        .map_err(|e| format!("Verifier did not return valid structured evidence: {e}"))?;
    if draft.status != "verified" {
        return Err(format!("Verifier found insufficient evidence: {}",
            draft.uncertainty.chars().take(200).collect::<String>()));
    }
    if draft.findings.is_empty() || draft.findings.len() > 12 {
        return Err("Verification requires 1–6 source-backed findings".into());
    }
    let mut findings = Vec::new();
    let mut unique = HashSet::new();
    let mut hosts = HashSet::new();
    for item in draft.findings {
        let claim = item.claim.trim();
        let quote = item.quote.trim();
        let url = web_url(item.url.trim())
            .ok_or_else(|| "Verifier cited a malformed or non-web URL".to_string())?;
        if claim.chars().count() < 8 || claim.chars().count() > 240
            || quote.chars().count() < 12 || quote.chars().count() > 400 {
            return Err("Finding or quote has an invalid length".into());
        }
        let page = observed.iter().find(|record| web_url(&record.url).as_ref() == Some(&url))
            .ok_or_else(|| format!("Unobserved source URL: {url}"))?;
        if !super::browser_skills::verification::quote_present(page,quote) {
            return Err(format!("Supporting quote does not appear in observed page: {url}"));
        }
        let claim_terms = terms(claim);
        let quote_terms = terms(quote);
        if claim_terms.len() >= 3 && claim_terms.intersection(&quote_terms).count() * 3
            < claim_terms.len() {
            return Err(format!("Claim is not grounded in the quoted passage: {url}"));
        }
        if !numbers(claim).is_subset(&numbers(quote)) {
            return Err(format!("Claim cites a number missing from the quote: {url}"));
        }
        if !unique.insert((url.to_string(), claim.to_lowercase())) {
            return Err("Duplicate cited finding".into());
        }
        hosts.insert(url.host_str().unwrap_or_default().to_string());
        findings.push(GroundedFinding {
            claim: claim.into(), url: url.to_string(), quote: quote.into()
        });
    }
    if compare && hosts.len() < 2 {
        return Err("Comparison requires evidence from at least two distinct websites".into());
    }
    let mut summary = String::from(
        "Observed website evidence (webpage claims are not independently fact-checked):\n");
    for (i, item) in findings.iter().enumerate() {
        summary.push_str(&format!("{}. {} [Source {}]\n", i+1, item.claim, i+1));
    }
    summary.push_str("\nSources and supporting excerpts:\n");
    for (i,item) in findings.iter().enumerate() {
        summary.push_str(&format!("[Source {}] {}\n  Quote: \"{}\"\n",
            i+1, item.url, item.quote));
    }
    let limitations = draft.uncertainty.chars().take(300).collect::<String>();
    if !limitations.is_empty() { summary.push_str(&format!("\nLimitations: {limitations}")); }
    Ok(VerificationReport { status:"evidence_checked", summary, findings, limitations })
}

/// Navigation dispatch is not completion: require the requested URL and
/// actual document evidence (text or meaningful title) from a later WebView snapshot.
pub fn check_navigation_result(requested: &str, after: &DomSnapshot) -> Result<(), String> {
    let target = web_url(requested).ok_or("Invalid navigation target")?;
    let actual = web_url(&after.origin).ok_or("Destination has not loaded")?;
    // Many public homepages canonicalize a bare host to its www subdomain.
    // Accept only that exact www/no-www alias with the same scheme, port,
    // path and query. Do not accept unrelated redirects or lookalike domains.
    fn normalize_www(host: &str) -> &str {
        host.strip_prefix("www.").unwrap_or(host)
    }
    let same_origin = target.scheme() == actual.scheme()
        && target.port_or_known_default() == actual.port_or_known_default()
        && target.host_str().zip(actual.host_str()).is_some_and(|(wanted, got)|
            normalize_www(wanted) == normalize_www(got));
    if !same_origin
        || target.path().trim_end_matches('/') != actual.path().trim_end_matches('/')
        || target.query() != actual.query() {
        return Err(format!("Opened {actual}, not the requested destination {target}"));
    }
    if super::deliberation::error_page(after) {
        return Err("Destination displayed an error or access restriction".into());
    }
    let title = after.title.trim().to_lowercase();
    let text = after.text_excerpt.trim().to_lowercase();
    // A real native DOM title also proves that a document was observed, even
    // when a JS-heavy home page initially has no body text.
    let browser_error = ["site can't be reached", "site can’t be reached",
        "this page isn't working", "privacy error", "err_connection",
        "dns_probe_finished", "page not found", "404 not found"];
    if browser_error.iter().any(|bad| title.contains(bad) || text.contains(bad)) {
        return Err("Destination displayed a browser/network error page".into());
    }
    let meaningful_title = title.chars().count() >= 4
        && title != "new tab" && title != "loading..." && title != "about:blank";
    if after.text_excerpt.trim().chars().count() < 12 && !meaningful_title {
        return Err("Destination has neither readable text nor a loaded document title".into());
    }
    Ok(())
}


/// A deliberately narrow, site-independent result-title request:
/// "search for X and open ... report its page title". No broad search or
/// research task can take this deterministic completion path.
pub fn requested_named_result_title(goal: &str) -> Option<String> {
    let lowered = goal.to_lowercase();
    if !title_only_goal(&lowered) {
        return None;
    }
    let begin = lowered.find("search for ")? + "search for ".len();
    let rest = goal.get(begin..)?;
    let end = rest.to_lowercase().find(" and open")?;
    let subject = rest.get(..end)?.trim();
    let subject = ["the word ","the planet ","the term ","the article "].iter()
        .find_map(|prefix|subject.get(..prefix.len()).filter(|s|s.eq_ignore_ascii_case(prefix)).and_then(|_|subject.get(prefix.len()..))).unwrap_or(subject);
    if !(3..=100).contains(&subject.chars().count()) ||
        !subject.chars().any(char::is_alphanumeric) {
        return None;
    }
    Some(subject.to_string())
}

/// A machine-observed URL, page title, and a matching saved WebView snapshot
/// prove only that a named page was opened and its title was read, NOT the
/// correctness of the page's claims. Callers must also require an executed
/// real browser action before using this verification path.
pub fn verify_observed_named_result_title(
    goal: &str,
    starting_origin: &str,
    after: &DomSnapshot,
    observations: &[SourceEvidence],
) -> Result<VerificationReport, String> {
    let wanted = requested_named_result_title(goal)
        .ok_or("Not a narrow named-result page-title request")?;
    let url = web_url(&after.origin).ok_or("Opened page has no valid web URL")?;
    if web_url(starting_origin).is_none_or(|start|start.origin()!=url.origin()){
        return Err("Named search result must remain on the observed website".into());
    }
    if after.origin == starting_origin {
        return Err("The requested result page has not been opened".into());
    }
    let expected = normalized(&wanted);
    let title = normalized(&after.title);
    let last_path = url.path_segments().and_then(|mut parts| parts.next_back())
        .ok_or("The destination has no named article path")?
        .replace('_', " ").replace('-', " ");
    let slug = normalized(&last_path);
    if !title.starts_with(&expected) ||
        !slug.contains(&expected) ||
        title.contains("search results") || title.contains("search for") ||
        after.text_excerpt.trim().chars().count() < 40 {
        return Err("The observed title and article path do not match the requested result".into());
    }
    if !observations.iter().any(|record|
        record.url == after.origin && record.title == after.title &&
            !record.excerpt.trim().is_empty())
    {
        return Err("No independently saved live WebView evidence for the result page".into());
    }
    Ok(VerificationReport {
        status: "evidence_checked",
        summary: format!("Opened a page titled {}.", after.title.trim()),
        findings: vec![GroundedFinding {
            claim: "The named result page was opened in the browser".into(),
            url: after.origin.clone(),
            quote: format!("Observed document title: {}", after.title.trim()),
        }],
        limitations: "This verifies the observed page URL and title only; it does not fact-check webpage claims.".into(),
    })
}

/// A narrowly scoped, read-only link-and-title task such as
/// "click the Tutorial link ... report the page title". Do not use this for
/// forms, purchases, arbitrary navigation, or tasks without a named link.
fn title_only_goal(lower:&str)->bool{
    let g=lower.trim().trim_end_matches(['.','!','?']).trim();
    let g=g.strip_suffix(". do not navigate to unrelated pages").unwrap_or(g);
    ["report its page title","report the page title","report page title","report its title","report the title"].iter().any(|s|g.ends_with(s))
        && !["meaning","explain","summarize","compare","extract","purchase","buy ","book ","submit","pay ","download","publish","register","send "].iter().any(|s|g.contains(s))
}
pub fn requested_named_link_title(goal: &str) -> Option<String> {
    let lower = goal.to_ascii_lowercase();
    if !title_only_goal(&lower) { return None; }
    let (start,via_visible) = if let Some(i)=lower.find("click the "){(i+"click the ".len(),false)}
        else if let Some(i)=lower.find("click "){(i+"click ".len(),false)}
        else if let Some(i)=lower.find("open the "){(i+"open the ".len(),true)}
        else{return None};
    let suffix = goal.get(start..)?;
    let end = if via_visible {suffix.to_ascii_lowercase().find(" using its visible link")?}
        else {suffix.to_ascii_lowercase().find(" link")?};
    let label = suffix.get(..end)?.trim();
    if !(3..=64).contains(&label.chars().count())
        || !label.chars().all(|c| c.is_ascii_alphanumeric() || c == ' ' || c == '-')
        || label.split_whitespace().count() > 7
    { return None; }
    Some(label.to_string())
}

/// Verify a *real* previously clicked link opened a matching readable page.
/// The caller separately requires a successful observed non-submit click.
/// A matching URL/title alone cannot authorize browser actions.
pub fn verify_observed_named_link_title(
    goal: &str,
    starting_origin: &str,
    after: &DomSnapshot,
    observations: &[SourceEvidence],
) -> Result<VerificationReport, String> {
    let label = requested_named_link_title(goal)
        .ok_or("Not a named-link page-title request")?;
    let url = web_url(&after.origin).ok_or("The destination is not a web page")?;
    if web_url(starting_origin).is_none_or(|start|start.origin()!=url.origin()){
        return Err("Named link destination changed origin".into());
    }
    if starting_origin == after.origin {
        return Err("The requested link did not open a new page".into());
    }
    let expected = normalized(&label);
    let title = normalized(&after.title);
    let path = normalized(&url.path().replace(['-', '_', '/'], " "));
    if !title.contains(&expected) || !path.contains(&expected)
        || after.text_excerpt.trim().chars().count() < 40
        || title.contains("search results") || title.contains("not found")
    { return Err("Destination title and path do not match the clicked link".into()); }
    if !observations.iter().any(|item|
        item.url == after.origin && item.title == after.title
        && !item.excerpt.trim().is_empty())
    { return Err("Missing fresh recorded WebView evidence for the opened link".into()); }
    Ok(VerificationReport {
        status: "evidence_checked",
        summary: format!("Opened a page titled {}.", after.title.trim()),
        findings: vec![GroundedFinding {
            claim: "The named link destination was opened".into(),
            url: after.origin.clone(),
            quote: format!("Observed document title: {}", after.title.trim()),
        }],
        limitations: "This confirms the observed destination URL and title, not the truth of page claims.".into(),
    })
}

/// For explicitly requested named-article searches, a search control can
/// navigate without producing a form-submission confirmation message.
/// This is a *navigation* postcondition and never permits claiming a purchase,
/// registration, application, or other consequential form was submitted.
pub fn check_named_search_transition(
    goal: &str, before: &DomSnapshot, after: &DomSnapshot,
) -> Result<(), String> {
    let subject = requested_named_result_title(goal)
        .ok_or("The user did not request a narrow named-result page title search")?;
    let from = web_url(&before.origin).ok_or("The search source was not a web page")?;
    let to = web_url(&after.origin).ok_or("No observed destination URL")?;
    if from.scheme() != to.scheme()
        || from.host_str() != to.host_str()
        || from.port_or_known_default() != to.port_or_known_default()
        || from.path() == to.path() && from.query() == to.query()
    {
        return Err("The search did not navigate to a new page on the observed site".into());
    }
    check_navigation_result(to.as_str(), after)?;
    if after.text_excerpt.trim().chars().count() < 40 {
        return Err("Search destination has insufficient readable page evidence".into());
    }
    let needle = normalized(&subject);
    let displayed = normalized(&format!("{} {}", after.title, after.text_excerpt));
    let location = normalized(&format!(
        "{} {}",
        to.path().replace('_', " ").replace('-', " "),
        to.query_pairs().map(|(_, v)| v.into_owned()).collect::<Vec<_>>().join(" ")
    ));
    if !displayed.contains(&needle) && !location.contains(&needle) {
        return Err("The observed destination does not show the requested search subject".into());
    }
    Ok(())
}

/// A deterministic verifier is more reliable than an LLM quote-generation pass
/// for the narrow task of opening an explicitly named website.
pub fn verify_direct_navigation(
    requested: &str, after: &DomSnapshot,
) -> Result<VerificationReport, String> {
    check_navigation_result(requested, after)?;
    let url = web_url(&after.origin).ok_or("Invalid observed destination")?;
    let quote = if after.text_excerpt.trim().chars().count() >= 12 {
        after.text_excerpt.trim().chars().take(140).collect()
    } else {
        format!("Observed document title: {}", after.title.trim().chars().take(100).collect::<String>())
    };
    Ok(VerificationReport {
        status: "evidence_checked",
        summary: format!("Verified: opened {} in the browser.", url.origin().ascii_serialization()),
        findings: vec![GroundedFinding {
            claim: "The requested website loaded in the native browser".into(),
            url: url.to_string(),
            quote,
        }],
        limitations: "Verification confirms the observed destination and document evidence, not the accuracy of external website content.".into(),
    })
}

/// A simple "search YouTube and open the first video" task can be verified
/// deterministically from two independent native page observations. This
/// verifies site navigation, not the contents or factual claims of the video.
pub fn verify_observed_youtube_first_result(
    goal: &str,
    after: &DomSnapshot,
    observed: &[SourceEvidence],
) -> Result<VerificationReport, String> {
    let normalized_goal = goal.trim().to_ascii_lowercase();
    let requested_query = normalized_goal
        .strip_prefix("search youtube for ")
        .and_then(|tail| tail.split_once(" and open the first video result"))
        .map(|(query, _)| query.trim())
        .filter(|query| query.chars().count() >= 3 && query.chars().count() <= 120)
        .ok_or("Not a supported explicit YouTube first-result task")?;

    let watch_url = web_url(&after.origin)
        .ok_or("No observed YouTube video page")?;
    if watch_url.scheme() != "https"
        || watch_url.host_str() != Some("www.youtube.com")
        || watch_url.path() != "/watch"
        || !watch_url.query_pairs().any(|(key, value)| key == "v" && !value.is_empty())
    {
        return Err("Observed destination is not a YouTube watch page".into());
    }
    check_navigation_result(after.origin.as_str(), after)?;
    if after.text_excerpt.trim().chars().count() < 40 {
        return Err("Observed video page has insufficient readable evidence".into());
    }

    let watched = observed.iter().find(|record|
        web_url(&record.url).as_ref() == Some(&watch_url)
        && record.excerpt.trim().chars().count() >= 40
    ).ok_or("No independent native video-page evidence was recorded")?;

    let searched = observed.iter().find(|record| {
        let Some(url) = web_url(&record.url) else { return false };
        url.scheme() == "https" && url.host_str() == Some("www.youtube.com")
            && url.path() == "/results"
            && url.query_pairs().any(|(key, value)|
                key == "search_query" && normalized(value.as_ref()) == normalized(requested_query))
            && record.excerpt.trim().chars().count() >= 30
    }).ok_or("No matching YouTube search-results page was observed")?;

    let title = after.title.trim().chars().take(120).collect::<String>();
    let watched_quote = watched.excerpt.trim().chars().take(140).collect::<String>();
    let searched_quote = searched.excerpt.trim().chars().take(140).collect::<String>();
    Ok(VerificationReport {
        status: "evidence_checked",
        summary: format!(
            "Verified: searched YouTube for '{}' and opened a video watch page ({}).",
            requested_query, title
        ),
        findings: vec![
            GroundedFinding {
                claim: "The requested search results page was observed in the browser".into(),
                url: searched.url.clone(),
                quote: searched_quote,
            },
            GroundedFinding {
                claim: "A YouTube watch page was observed after the search".into(),
                url: watched.url.clone(),
                quote: watched_quote,
            },
        ],
        limitations: "This verifies observed page navigation and text only. It does not verify video accuracy or independently establish ranking beyond the selected on-page link.".into(),
    })
}

/// A submit click acknowledgment is not proof of a received submission.
pub fn check_submission_result(
    before: &DomSnapshot, after: &DomSnapshot,
) -> Result<(), String> {
    if before.origin == after.origin
        && normalized(&before.text_excerpt) == normalized(&after.text_excerpt) {
        return Err("Submit was clicked, but no page change was observed".into());
    }
    let text = normalized(&after.text_excerpt);
    let earlier_text = normalized(&before.text_excerpt);
    let confirmations = [
        "successfully submitted", "submission received", "form submitted",
        "application received", "application submitted", "thank you for submitting",
        "registration complete", "order confirmed", "confirmation number",
        "feedback sent", "feedback received", "message sent successfully",
        "changes saved successfully", "settings updated successfully",
    ];
    if !confirmations.iter().any(|term| text.contains(term) && !earlier_text.contains(term)) {
        return Err("Page changed, but no visible submission confirmation was found".into());
    }
    Ok(())
}
pub fn requires_post_action_check(action: &BrowserAction) -> bool {
    matches!(action, BrowserAction::Navigate { .. }
        | BrowserAction::Click { is_submit:true, .. })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn src(url: &str, excerpt: &str) -> SourceEvidence {
        SourceEvidence { url:url.into(), title:"Demo".into(), excerpt:excerpt.into() }
    }
    fn dom(url: &str, text: &str) -> DomSnapshot {
        DomSnapshot {structure: Default::default(), origin:url.into(), title:"Demo".into(),
            text_excerpt:text.into(),interactive_nodes:vec![]}
    }
    #[test]
    fn accepts_exact_observed_quote_and_url() {
        let sources = [src("https://one.test/details",
            "The laptop includes 16 GB memory and a 512 GB SSD in the base configuration.")];
        let proposal = serde_json::json!({"status":"verified","findings":[{
            "claim":"The laptop includes 16 GB memory",
            "url":"https://one.test/details",
            "quote":"The laptop includes 16 GB memory"
        }]});
        let report = check_model_verification(&proposal.to_string(), &sources,false).unwrap();
        assert!(report.summary.contains("[Source 1] https://one.test/details"));
    }
    #[test]
    fn rejects_fabricated_urls_quotes_numbers_or_unverified_status() {
        let sources=[src("https://one.test/","The laptop has 16 GB memory and a 512 GB SSD.")];
        let original=serde_json::json!({"status":"verified","findings":[{
            "claim":"The laptop has 16 GB memory",
            "url":"https://one.test/",
            "quote":"The laptop has 16 GB memory"
        }]});
        for i in 0..4 {
            let mut test=original.clone();
            match i {
                0 => test["findings"][0]["url"]="https://not-seen.test/".into(),
                1 => test["findings"][0]["quote"]="The laptop has 64 GB memory".into(),
                2 => test["findings"][0]["claim"]="The laptop has 99 GB memory".into(),
                _ => test["status"]="insufficient".into(),
            }
            assert!(check_model_verification(&test.to_string(),&sources,false).is_err());
        }
    }
    #[test]
    fn comparisons_require_two_observed_hosts() {
        let sources=[
            src("https://one.test/a","Laptop A has a 16 GB memory configuration."),
            src("https://two.test/b","Laptop B has a 32 GB memory configuration.")
        ];
        let result=serde_json::json!({"status":"verified","findings":[
            {"claim":"Laptop A has 16 GB memory","url":"https://one.test/a",
                "quote":"Laptop A has a 16 GB memory configuration"},
            {"claim":"Laptop B has 32 GB memory","url":"https://two.test/b",
                "quote":"Laptop B has a 32 GB memory configuration"}
        ]});
        assert!(check_model_verification(&result.to_string(),&sources,true).is_ok());
        assert!(check_model_verification(&result.to_string(),&sources[..1],true).is_err());
    }
    #[test]
    fn exact_destination_is_automatically_verified_without_model_claims() {
        let snapshot = DomSnapshot {
            structure: Default::default(), origin: "https://www.youtube.com/".into(),
            title: "YouTube".into(),
            text_excerpt: "YouTube home. Browse videos and channels on this site.".into(),
            interactive_nodes: vec![],
        };
        let report = verify_direct_navigation("https://www.youtube.com/", &snapshot).unwrap();
        assert_eq!(report.status, "evidence_checked");
        assert_eq!(report.findings.len(), 1);
        assert!(verify_direct_navigation("https://www.youtube.com/", &DomSnapshot {
            structure: Default::default(), origin: "https://www.youtube.com.evil.test/".into(), ..snapshot.clone()
        }).is_err());
        assert!(verify_direct_navigation("https://www.youtube.com/", &DomSnapshot {
            text_excerpt: "".into(), ..snapshot.clone()
        }).is_ok()); // A loaded native document title is valid navigation evidence.
        assert!(verify_direct_navigation("https://www.youtube.com/", &DomSnapshot {
            title: "".into(), text_excerpt: "".into(), ..snapshot.clone()
        }).is_err());
        assert!(verify_direct_navigation("https://www.youtube.com/", &DomSnapshot {
            title: "This site can't be reached".into(), text_excerpt: "".into(), ..snapshot
        }).is_err());
    }

    #[test]
    fn canonical_www_redirect_needs_exact_host_path_and_page_evidence() {
        let loaded = dom("https://www.wikipedia.org/",
            "Wikipedia homepage with language choices and search.");
        assert!(check_navigation_result("https://wikipedia.org/", &loaded).is_ok());
        assert!(check_navigation_result("https://www.wikipedia.org/", &dom(
            "https://wikipedia.org/", "Wikipedia homepage with search.")).is_ok());
        assert!(check_navigation_result("https://wikipedia.org/", &dom(
            "https://www.wikipedia.org.evil.test/", "A fake homepage.")).is_err());
        assert!(check_navigation_result("https://wikipedia.org/", &dom(
            "https://wiki.pedia.org/", "A different homepage.")).is_err());
        assert!(check_navigation_result("https://wikipedia.org/", &dom(
            "http://www.wikipedia.org/", "Insecure redirected homepage.")).is_err());
        assert!(check_navigation_result("https://wikipedia.org/article", &loaded).is_err());
        let mut empty = loaded.clone();
        empty.title.clear();
        empty.text_excerpt.clear();
        assert!(check_navigation_result("https://wikipedia.org/", &empty).is_err());
    }

    #[test]
    fn named_search_result_title_requires_new_matching_article_and_source() {
        let goal = "On this documentation site, search for Ada Lovelace and open the corresponding article. Then report its page title.";
        assert_eq!(requested_named_result_title(goal).as_deref(), Some("Ada Lovelace"));
        assert!(requested_named_result_title("Search for Ada Lovelace and compare biographies").is_none());
        assert!(requested_named_result_title("Search for the word serendipity and open its entry. Report the page title and English meaning.").is_none());
        assert_eq!(requested_named_result_title("Search for the word serendipity and open its entry. Report the page title.").as_deref(),Some("serendipity"));
        assert_eq!(requested_named_link_title("Open the JavaScript Guide using its visible link. Then report the page title.").as_deref(),Some("JavaScript Guide"));
        let start = "https://en.wikipedia.org/wiki/Main_Page";
        let article = DomSnapshot {
            structure: Default::default(), origin: "https://en.wikipedia.org/wiki/Ada_Lovelace".into(),
            title: "Ada Lovelace - Wikipedia".into(),
            text_excerpt: "Ada Lovelace was a mathematician. This is an observed example article page with more visible text.".into(),
            interactive_nodes: vec![],
        };
        let sources = vec![SourceEvidence {
            url: article.origin.clone(),
            title: article.title.clone(),
            excerpt: article.text_excerpt.clone(),
        }];
        let report = verify_observed_named_result_title(goal, start, &article, &sources).unwrap();
        assert_eq!(report.status, "evidence_checked");
        assert_eq!(report.findings[0].url, article.origin);
        assert!(report.limitations.contains("not fact-check"));
        assert!(verify_observed_named_result_title(goal, start, &article, &[]).is_err());
        assert!(verify_observed_named_result_title(goal, article.origin.as_str(), &article, &sources).is_err());
        let search_listing = DomSnapshot {
            structure: Default::default(), origin: "https://en.wikipedia.org/w/index.php?search=Ada_Lovelace".into(),
            title: "Ada Lovelace - Search results".into(),
            ..article.clone()
        };
        assert!(verify_observed_named_result_title(goal, start, &search_listing, &sources).is_err());
        let other = DomSnapshot {
            structure: Default::default(), origin: "https://en.wikipedia.org/wiki/Other".into(),
            title: "Other - Wikipedia".into(),
            ..article
        };
        assert!(verify_observed_named_result_title(goal, start, &other, &sources).is_err());
    }

    #[test]
    fn named_link_title_requires_matching_destination_and_real_evidence() {
        let goal = "On this Python documentation homepage, click the Tutorial link to open the Python Tutorial page. Then report the page title.";
        assert_eq!(requested_named_link_title(goal).as_deref(), Some("Tutorial"));
        assert!(requested_named_link_title("Click Tutorial to buy a subscription").is_none());
        assert!(requested_named_link_title("Submit the feedback form and report its title").is_none());
        let start = "https://docs.python.org/3/";
        let page = DomSnapshot {
            structure: Default::default(), origin: "https://docs.python.org/3/tutorial/index.html".into(),
            title: "The Python Tutorial — Python 3.15 documentation".into(),
            text_excerpt: "The Python Tutorial contains an introduction to Python with extensive examples and lessons.".into(),
            interactive_nodes: vec![],
        };
        let evidence = vec![SourceEvidence {
            url: page.origin.clone(), title: page.title.clone(),
            excerpt: page.text_excerpt.clone(),
        }];
        let report = verify_observed_named_link_title(goal, start, &page, &evidence).unwrap();
        assert_eq!(report.status, "evidence_checked");
        assert!(verify_observed_named_link_title(goal, start, &page, &[]).is_err());
        assert!(verify_observed_named_link_title(goal, &page.origin, &page, &evidence).is_err());
        let wrong = DomSnapshot {
            structure: Default::default(), origin: "https://docs.python.org/3/library/index.html".into(),
            title: "The Python Library — Python documentation".into(),
            ..page.clone()
        };
        assert!(verify_observed_named_link_title(goal, start, &wrong, &evidence).is_err());
        let forged = DomSnapshot {
            structure: Default::default(), origin: "https://docs.python.org.evil.test/3/tutorial/index.html".into(),
            ..page
        };
        assert!(verify_observed_named_link_title(goal, start, &forged, &evidence).is_err());
    }

    #[test]
    fn named_search_transition_requires_real_same_site_result_page() {
        let goal = "On this Wikipedia page, search for Ada Lovelace and open the corresponding article. Then report its page title.";
        let before = dom("https://en.wikipedia.org/wiki/Main_Page",
            "Wikipedia home page with a search bar, article links and languages.");
        let result = dom("https://en.wikipedia.org/wiki/Ada_Lovelace",
            "Ada Lovelace was an English mathematician and writer with a famous biography.");
        assert!(check_named_search_transition(goal, &before, &result).is_ok());
        assert!(check_named_search_transition(goal, &before, &before).is_err());
        assert!(check_named_search_transition(goal, &before, &dom(
            "https://en.wikipedia.org/wiki/Other", "An unrelated encyclopedia article with no requested subject.")).is_err());
        assert!(check_named_search_transition(goal, &before, &dom(
            "https://en.wikipedia.org.evil.test/wiki/Ada_Lovelace", "Ada Lovelace is a mathematician in this fake site.")).is_err());
        assert!(check_named_search_transition("Submit my application form", &before, &result).is_err());
        assert!(check_named_search_transition(goal, &before, &dom(
            "https://en.wikipedia.org/wiki/Ada_Lovelace", "")).is_err());
    }

    #[test]
    fn navigation_needs_actual_destination_not_just_ack() {
        assert!(check_navigation_result("https://example.org/details",
            &dom("https://example.org/details","Page details are visible here.")).is_ok());
        assert!(check_navigation_result("https://example.org/details",
            &dom("https://example.org/login","Please sign in before proceeding.")).is_err());
        assert!(check_navigation_result("https://example.org/details",
            &DomSnapshot { title: "".into(),
                ..dom("https://example.org/details","") }).is_err());
    }
    #[test]
    fn submission_needs_changed_page_with_confirmation() {
        let before=dom("https://example.org/apply","Application form with contact fields.");
        assert!(check_submission_result(&before,&before).is_err());
        assert!(check_submission_result(&before,
            &dom("https://example.org/apply","There was an unrelated error.")).is_err());
        assert!(check_submission_result(&before,
            &dom("https://example.org/thanks","Application received. Thank you for submitting.")).is_ok());
        let prior_banner = dom("https://example.org/apply",
            "Help text: Application received. Fields for your new submission.");
        assert!(check_submission_result(&prior_banner,
            &dom("https://example.org/apply","Application received. Loading another page.")).is_err());
    }
}

#[cfg(test)]
mod autonomous_confirmation_tests {
    use super::*;
    #[test]
    fn accepts_new_visible_feedback_confirmation_but_not_preexisting_text() {
        let before = DomSnapshot {
            structure: Default::default(), origin: "https://example.org/feedback".into(),
            title: "Feedback".into(),
            text_excerpt: "No feedback has been submitted yet".into(),
            interactive_nodes: vec![],
        };
        let mut after = before.clone();
        after.text_excerpt = "Feedback sent successfully on the local test page".into();
        assert!(check_submission_result(&before, &after).is_ok());
        assert!(check_submission_result(&after, &after).is_err());

        let mut unrelated = before.clone();
        unrelated.text_excerpt = "A new panel appeared, but no confirmation was shown".into();
        assert!(check_submission_result(&before, &unrelated).is_err());
    }
}

#[cfg(test)]
mod youtube_first_result_evidence_tests {
    use super::*;
    fn snapshot(url: &str) -> DomSnapshot {
        DomSnapshot {
            structure: Default::default(), origin: url.into(),
            title: "NASA Artemis explained - YouTube".into(),
            text_excerpt: "A detailed NASA Artemis mission video with chapters and a description.".into(),
            interactive_nodes: vec![],
        }
    }
    #[test]
    fn requires_both_actual_search_results_and_matching_video_page() {
        let goal = "Search YouTube for NASA Artemis mission and open the first video result, then report the observed video page.";
        let url = "https://www.youtube.com/watch?v=abcDEF12345";
        let watch = snapshot(url);
        let observed = vec![
            SourceEvidence {
                url: "https://www.youtube.com/results?search_query=NASA+Artemis+mission".into(),
                title: "Results - YouTube".into(),
                excerpt: "Search results for NASA Artemis mission videos and animations.".into(),
            },
            SourceEvidence {
                url: url.into(),
                title: watch.title.clone(),
                excerpt: watch.text_excerpt.clone(),
            },
        ];
        let report = verify_observed_youtube_first_result(goal, &watch, &observed).unwrap();
        assert_eq!(report.status, "evidence_checked");
        assert_eq!(report.findings.len(), 2);
        assert!(verify_observed_youtube_first_result(goal, &watch, &observed[1..]).is_err());
        assert!(verify_observed_youtube_first_result(goal,
            &snapshot("https://www.youtube.com/results?search_query=NASA"), &observed).is_err());
        assert!(verify_observed_youtube_first_result(goal,
            &snapshot("https://www.youtube.com.attacker.test/watch?v=123"), &observed).is_err());
        assert!(verify_observed_youtube_first_result("Open YouTube", &watch, &observed).is_err());
        let mut wrong_search = observed.clone();
        wrong_search[0].url = "https://www.youtube.com/results?search_query=unrelated".into();
        assert!(verify_observed_youtube_first_result(goal, &watch, &wrong_search).is_err());
    }
}


#[cfg(test)]
mod focused_source_tests{
    use super::*;
    #[test]
    fn relevant_late_passage_is_retained_and_preferred(){
        let sources=vec![
            SourceEvidence{url:"https://example.org/".into(),title:"Homepage".into(),excerpt:"Navigation menu and general information.".repeat(40)},
            SourceEvidence{url:"https://example.org/neptune".into(),title:"Neptune".into(),excerpt:format!("{} Neptune is the eighth planet from the Sun. Other planetary information follows.","Unrelated navigation menu. ".repeat(100))}
        ];
        let selected=focus_sources("Search for the planet Neptune and open the corresponding article. Report its position from the Sun.",&sources,false);
        assert_eq!(selected[0].url,sources[1].url);
        assert!(selected[0].excerpt.contains("Neptune is the eighth planet from the Sun."));
        assert!(sources[1].excerpt.contains(&selected[0].excerpt));
        assert!(selected[0].excerpt.chars().count()<=1450);
    }
    #[test]
    fn comparison_preserves_distinct_hosts(){
        let sources=["one.example","one.example","two.example"].iter().enumerate().map(|(i,host)|SourceEvidence{url:format!("https://{host}/{i}"),title:"Solar energy".into(),excerpt:"Solar energy output varies by location and weather.".into()}).collect::<Vec<_>>();
        let selected=focus_sources("Compare solar energy output",&sources,true);
        assert_eq!(selected.len(),2);
    }
}


#[cfg(test)]
mod explicit_source_tests {
    use super::*;
    #[test]
    fn explicit_comparison_sources_must_all_be_observed(){
        let goal="Research and compare https://one.example/guide and https://two.example/guide . Cite both sources.";
        let mut sources=vec![SourceEvidence{url:"https://one.example/guide".into(),title:"Guide".into(),excerpt:"A readable observed explanation about the relevant subject. ".repeat(3)}];
        assert_eq!(super::super::deliberation::missing_research_sources(goal,&sources)[0].as_str(),"https://two.example/guide");
        sources.push(SourceEvidence{url:"https://two.example/guide".into(),title:"Access denied".into(),excerpt:"Error details do not prove that the requested page loaded. ".repeat(3)});
        assert_eq!(super::super::deliberation::missing_research_sources(goal,&sources).len(),1);
        sources[1].title="Guide".into();
        assert!(super::super::deliberation::missing_research_sources(goal,&sources).is_empty());
        assert!(super::super::deliberation::explicit_research_sources("Compare https://one.example/guide and https://127.0.0.1/ .").is_empty());
        assert!(super::super::deliberation::explicit_research_sources("Compare https://one.example/guide and https://two.example/guide and buy one.").is_empty());
    }
}


#[cfg(test)]
mod extractive_comparison_tests{
    use super::*;
    #[test]
    fn contrast_retains_complete_literal_behavior_clauses_after_chunk_boundary(){
        let goal="Find the SQL INSERT command reference. Locate the ON CONFLICT clause and explain the difference between DO NOTHING and DO UPDATE. Cite observed source evidence for each behavior.";
        let source=SourceEvidence{url:"https://example.org/docs/sql-insert.html".into(),title:"SQL INSERT".into(),
            excerpt:format!("{} ON CONFLICT DO NOTHING simply avoids inserting a row as its alternative action. ON CONFLICT DO UPDATE updates the existing row that conflicts with the row proposed for insertion as its alternative action.","Unrelated context ".repeat(13))};
        let sets=facet_candidates(goal,&source);
        assert_eq!(sets.len(),2);
        assert!(sets[0].candidates[0].text.contains("DO NOTHING simply avoids"),"{:?}",sets[0]);
        assert!(sets[1].candidates[0].text.contains("DO UPDATE updates"),"{:?}",sets[1]);
        assert!(sets.iter().all(|s|super::super::browser_skills::verification::quote_present(&source,&s.candidates[0].text)));
    }
    #[test]
    fn target_fact_contract_requires_every_requested_behavior(){
        let goal="Find the SQL MERGE command reference. Explain the difference between WHEN MATCHED and WHEN NOT MATCHED.";
        let source=SourceEvidence{url:"https://example.org/docs/sql-merge.html".into(),title:"SQL MERGE".into(),
            excerpt:"WHEN MATCHED performs an update to an existing matching row. WHEN NOT MATCHED inserts a new row if the condition permits insertion.".into()};
        let sets=facet_candidates(goal,&source);
        assert_eq!(research_facets(goal),vec!["contrast_left","contrast_right"]);
        assert!(sets.iter().all(|s|!s.candidates.is_empty()));
        let only=GroundedFinding{url:source.url.clone(),claim:"WHEN MATCHED performs an update to an existing matching row.".into(),quote:"WHEN MATCHED performs an update to an existing matching row.".into()};
        assert!(assemble_compiled_skill_report(goal,&[source.clone()],vec![only]).is_err());
        let findings=facet_findings(goal,&source,&sets,r#"{"contrast_left":"contrast_left_1","contrast_right":"contrast_right_1"}"#).unwrap();
        assert!(assemble_compiled_skill_report(goal,&[source],findings).is_ok());
    }
    #[test]
    fn http_client_comparison_prefers_operations_and_lifecycle_over_incidental_examples(){
        let goal="Compare HTTP client libraries. Explain a useful feature and usage caveat for each.";
        let s=SourceEvidence{url:"https://example.org/client".into(),title:"Client documentation".into(),excerpt:"You can use the following image processing example. The client provides a ClientSession for sending HTTP requests. Do not create a session per request. A session contains a connection pool. By default the total timeout is 300 seconds.".into()};
        let sets=facet_candidates(goal,&s);
        assert!(sets[0].candidates[0].text.contains("ClientSession"));
        assert!(sets[1].candidates[0].text.contains("session per request")||sets[1].candidates[0].text.contains("connection pool"));
        assert_eq!(facet_score(goal,"feature","You can use the following image processing example."),0);
    }
    #[test]
    fn usage_caveat_prefers_direct_operational_restrictions_when_present(){
        let g="Compare HTTP client libraries. Explain one feature and one usage caveat for each.";
        let s=SourceEvidence{url:"https://example.org/docs".into(),title:"Public client documentation".into(),excerpt:"By default the total timeout is 300 seconds. Do not create a client session per request; a session contains a connection pool. The client provides a public interface for sending HTTP requests.".into()};
        let sets=facet_candidates(g,&s);
        let warnings=&sets.iter().find(|s|s.facet=="caveat").unwrap().candidates;
        assert!(!warnings.is_empty());assert!(warnings.iter().all(|p|direct_usage_warning(&p.text)));
    }
    #[test]
    fn typographic_warning_beats_incidental_default_and_preserves_exact_quote(){
        let g="Compare HTTP client libraries. Explain a useful feature and one limitation or usage caveat for each.";
        let s=SourceEvidence{url:"https://example.org/docs".into(),title:"Client documentation".into(),
            excerpt:"The client provides a ClientSession for sending HTTP requests. Don\u{2019}t create a session per request. Most likely you need a session per application which performs all requests together. A session contains a connection pool inside. By default the total timeout is 300 seconds.".into()};
        let sets=facet_candidates(g,&s);
        let warnings=&sets.iter().find(|s|s.facet=="caveat").unwrap().candidates;
        assert!(warnings.iter().any(|p|p.text.contains("Don\u{2019}t create a session per request.")));
        assert!(warnings.iter().all(|p|quality_tier(g,"caveat",&p.text)==3));
        assert!(warnings.iter().all(|p|s.excerpt.contains(&p.text)));
        let weak=FacetCandidates{facet:"caveat".into(),candidates:vec![Passage{id:"weak".into(),text:"By default the total timeout is 300 seconds.".into()}]};
        assert!(!valid_facet_choice(g,&s,&weak,&weak.candidates[0]));
        let f=&sets.iter().find(|s|s.facet=="feature").unwrap().candidates[0];
        assert!(facet_findings(g,&s,&sets,&serde_json::json!({"feature":f.id,"caveat":warnings[0].id}).to_string()).is_ok());
    }
    #[test]
    fn alternate_review_retains_valid_dimension_and_reorders_missing_candidates(){
        let g="Compare HTTP client libraries. Explain a useful feature and one usage caveat for each.";
        let s=SourceEvidence{url:"https://example.org/docs".into(),title:"Client documentation".into(),
            excerpt:"The client provides a ClientSession for sending HTTP requests. Do not create a session per request. Avoid closing the connection pool before the HTTP requests finish. Make sure you close the session after all operations have completed.".into()};
        let sets=facet_candidates(g,&s);
        let feature=&sets[0].candidates[0];assert!(sets[1].candidates.len()>1);
        let raw=serde_json::json!({"feature":feature.id,"caveat":"missing"}).to_string();
        assert!(facet_findings(g,&s,&sets,&raw).is_err());
        let alternate=retry_facets(g,&s,&sets,Some(&raw));
        assert_eq!(alternate[0].candidates.len(),1);
        assert_eq!(alternate[0].candidates[0].id,feature.id);
        assert_ne!(alternate[1].candidates[0].id,sets[1].candidates[0].id);
        assert!(facet_findings(g,&s,&alternate,&serde_json::json!({"feature":feature.id,"caveat":alternate[1].candidates[0].id}).to_string()).is_ok());
    }
    #[test]
    fn default_only_remains_valid_when_no_stronger_warning_exists(){
        let g="Compare HTTP client libraries. Explain one feature and one usage caveat for each.";
        let s=SourceEvidence{url:"https://example.org/docs".into(),title:"Client documentation".into(),
            excerpt:"The client provides a ClientSession for sending HTTP requests. By default the total timeout is 300 seconds.".into()};
        let sets=facet_candidates(g,&s);
        assert!(!sets[1].candidates.is_empty());
        assert!(valid_facet_choice(g,&s,&sets[1],&sets[1].candidates[0]));
    }
    #[test]
    fn requested_dimensions_require_separate_observed_evidence(){
        let goal="Compare features and limitations of libraries";
        let source=SourceEvidence{url:"https://example.org/guide".into(),title:"Library".into(),
            excerpt:"The library supports asynchronous HTTP requests with a reusable client. Do not open a new client for every operation because that prevents effective connection pooling.".into()};
        let sets=facet_candidates(goal,&source);
        assert_eq!(sets.len(),2);
        assert!(sets.iter().all(|s|!s.candidates.is_empty()));
        assert!(facet_findings(goal,&source,&sets,r#"{"feature":"feature_1"}"#).is_err());
        assert!(facet_findings(goal,&source,&sets,r#"{"feature":"feature_1","caveat":"missing"}"#).is_err());
        assert!(facet_findings(goal,&source,&sets,r#"{"feature":"feature_1","caveat":"caveat_1"}"#).is_ok());
        let only=SourceEvidence{excerpt:"The library supports asynchronous HTTP requests with a reusable client.".into(),..source};
        assert!(facet_candidates(goal,&only).iter().find(|s|s.facet=="caveat").unwrap().candidates.is_empty());
    }
    #[test]
    fn selects_only_observed_ids_and_requires_every_requested_source(){
        let goal="Compare solar energy and wind energy using https://one.example/guide and https://two.example/guide . Cite both sources.";
        let a=SourceEvidence{url:"https://one.example/guide".into(),title:"Solar energy".into(),excerpt:format!("{} Solar energy production varies with available sunlight and seasonal conditions. ","Unrelated introductory navigation. ".repeat(300))};
        let b=SourceEvidence{url:"https://two.example/guide".into(),title:"Wind energy".into(),excerpt:"Wind energy production depends on the local wind speed and turbine characteristics.".into()};
        let ca=candidate_passages(goal,&a);
        let cb=candidate_passages(goal,&b);
        assert!(ca.iter().any(|p|p.text.contains("Solar energy production")));
        assert!(selected_findings(goal,&a,&ca,r#"{"ids":["invented"]}"#).is_err());
        let pick=|s:&SourceEvidence,p:&Vec<Passage>,term:&str|selected_findings(goal,s,p,&serde_json::json!({"ids":[p.iter().find(|x|x.text.contains(term)).unwrap().id]}).to_string()).unwrap();
        let mut findings=pick(&a,&ca,"Solar energy production");
        assert!(assemble_extractive_comparison(goal,&[a.clone(),b.clone()],findings.clone()).is_err());
        findings.extend(pick(&b,&cb,"Wind energy production"));
        assert!(assemble_extractive_comparison(goal,&[a,b],findings).is_ok());
    }
}
