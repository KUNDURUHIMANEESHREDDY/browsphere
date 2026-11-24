//! Read-only public train schedule verification. Strict observed result grammar;
//! never infer seat inventory from a timetable, refresh or booking control.
use super::{browser_skills as bs,dom_extractor::DomSnapshot,evidence_verifier::{GroundedFinding,VerificationReport},orchestrator::SourceEvidence};
use chrono::{NaiveDate,NaiveTime,Duration};
use std::collections::HashSet;

#[derive(Debug,Clone)]
struct Schedule{
    name:String,number:String,departure:NaiveTime,arrival:NaiveTime,
    from:String,to:String,departure_date:NaiveDate,arrival_date:NaiveDate,quote:String,
}
fn clock(s:&str)->Option<NaiveTime>{
    if s.len()!=5{return None;}
    NaiveTime::parse_from_str(s,"%H:%M").ok()
}
fn duration(s:&str)->Option<Duration>{
    let (h,m)=s.split_once(':')?;
    let h=h.parse::<i64>().ok()?;let m=m.parse::<i64>().ok()?;
    (h>=0&&h<=240&&(0..60).contains(&m)).then(||Duration::minutes(h*60+m))
}
fn train_number(s:&str)->Option<&str>{
    let n=s.strip_prefix('(')?.strip_suffix(')')?;
    (n.len()==5&&n.chars().all(|c|c.is_ascii_digit())).then_some(n)
}
fn same_date(tokens:&[&str],d:NaiveDate)->bool{
    tokens.join(" ").eq_ignore_ascii_case(&d.format("%a, %d %b").to_string())||
    tokens.join(" ").eq_ignore_ascii_case(&d.format("%a, %-d %b").to_string())
}
fn within_station_list(list:&str,station:&str)->bool{
    !station.is_empty()&&bs::words(station).len()<=8&&bs::phrase(list,station)
}
fn verified_origin(initial:&str,actual:&str)->bool{
    let (Some(a),Some(b))=(bs::public_url(initial),bs::public_url(actual))else{return false;};
    a.scheme()=="https"&&a.origin()==b.origin()&&b.query().is_none()&&b.fragment().is_none()
}
/// Only a simple explicitly requested local departure lower bound or no
/// departure constraint is supported. Unknown/extra time constraints fail closed.
fn departure_floor(goal:&str)->Result<Option<NaiveTime>,&'static str>{
    let g=goal.to_ascii_lowercase();
    let Some((_,tail))=g.split_once(" departing at or after ")else{
        return if ["departing","departure","evening","morning","after ","before ","between ","arriving"].iter().any(|s|g.contains(s)){
            Err("Unsupported journey time constraint")
        }else{Ok(None)};
    };
    let value=tail.split_once(" india time").ok_or("Missing explicit India time constraint")?.0.trim();
    if value.split_whitespace().count()!=2{return Err("Unsupported departure time");}
    let (clock,meridiem)=value.split_once(' ').ok_or("Invalid departure time")?;
    let expanded=if clock.contains(':'){format!("{clock} {}",meridiem.to_uppercase())}else{format!("{clock}:00 {}",meridiem.to_uppercase())};
    let t=NaiveTime::parse_from_str(&expanded,"%I:%M %p").map_err(|_|"Invalid departure time")?;
    if !tail[value.len()..].starts_with(" india time."){return Err("Unsupported extra journey time constraints");}
    Ok(Some(t))
}
fn read_schedules(goal:&str,page:&DomSnapshot)->Result<(String,NaiveDate,Vec<Schedule>),&'static str>{
    let (origin,destination)=super::prerequisites::route_values(goal).ok_or("Unsupported public journey goal")?;
    let day=super::prerequisites::requested_date(goal).ok_or("Journey date must be explicit")?;
    if !bs::readable(page)||page.structure.loading||super::page_understanding::classify(page).kind!=super::page_understanding::PageKind::TransportResults{
        return Err("A fresh readable train results page is required");
    }
    let tokens=page.text_excerpt.split_whitespace().collect::<Vec<_>>();
    if tokens.len()>6000{return Err("Schedule evidence exceeds bounded parser");}
    let heads=tokens.windows(3).enumerate().filter(|(_,w)|w[0].parse::<usize>().is_ok()&&w[1].eq_ignore_ascii_case("results")&&w[2].eq_ignore_ascii_case("for")).map(|(i,_)|i).collect::<Vec<_>>();
    if heads.len()!=1{return Err("No unique visible journey result heading");}
    let head=heads[0];let count=tokens[head].parse::<usize>().map_err(|_|"Invalid result count")?;
    if count==0||count>80{return Err("No bounded positive schedule result set");}
    let header_end=(head+3..tokens.len().min(head+55)).find(|&i|tokens[i..].starts_with(&["For","Quota"])).ok_or("Incomplete result heading")?;
    let pipe=(head+3..header_end).find(|&i|tokens[i]=="|").ok_or("Missing result date separator")?;
    let route=tokens[head+3..pipe].join(" ");
    let route_words=bs::words(&route);let from_words=bs::words(origin);let to_words=bs::words(destination);
    let a=route_words.windows(from_words.len()).position(|w|w==from_words.as_slice()).ok_or("Wrong departure city")?;
    let b=route_words.windows(to_words.len()).position(|w|w==to_words.as_slice()).ok_or("Wrong destination city")?;
    if a+from_words.len()>b{return Err("Journey cities are reversed or ambiguous");}
    let header_date=tokens[pipe+1..header_end].join(" ");
    if !header_date.eq_ignore_ascii_case(&day.format("%a, %d %b %Y").to_string())&&
       !header_date.eq_ignore_ascii_case(&day.format("%a, %-d %b %Y").to_string()){
        return Err("Exact journey date is missing or does not match");
    }
    let header=tokens[head..header_end].join(" ");
    // Station alternatives must come from the visible route-specific filters.
    // Do not guess city-to-station aliases from a model or a lookup table.
    let from_filter=tokens[..head].windows(4).position(|w|w==["FROM","STATIONS","Select","All"]).ok_or("Departure station filter not observed")?+4;
    let to_filter=tokens[..head].windows(4).position(|w|w==["TO","STATIONS","Select","All"]).ok_or("Destination station filter not observed")?;
    if from_filter>=to_filter{return Err("Invalid station filter order");}
    let from_list=tokens[from_filter..to_filter].join(" ");let to_list=tokens[to_filter+4..head].join(" ");
    let start=(header_end..tokens.len()).find(|&i|tokens[i]=="DayNext"&&tokens.get(i+1)==Some(&"Day")).map(|i|i+2).ok_or("Missing result list boundary")?;
    let mut cursor=start;let mut rows=Vec::new();let mut seen=HashSet::new();
    while cursor<tokens.len(){
        let Some(relative)=tokens[cursor..].iter().take(18).position(|s|train_number(s).is_some())else{break;};
        let i=cursor+relative;
        let name=tokens[cursor..i].join(" ");
        let number=train_number(tokens[i]).unwrap();
        if name.is_empty()||name.len()>100||!name.chars().all(|c|c.is_alphanumeric()||matches!(c,' '|'.'|'-'|'/'|'&'))||!seen.insert(number.to_string()){return Err("Malformed or duplicate train identity");}
        if tokens.get(i+1..i+3)!=Some(&["Runs","On:"][..])||tokens.get(i+4..i+6)!=Some(&["Train","Schedule"][..]){return Err("Missing train-specific schedule boundary");}
        let dep=clock(tokens.get(i+6).ok_or("Missing departure")?).ok_or("Invalid departure clock")?;
        if tokens.get(i+7)!=Some(&"|"){return Err("Missing departure station separator");}
        let from_end=(i+8..tokens.len().min(i+19)).find(|&j|tokens[j]=="|").ok_or("Missing departure station")?;
        let from=tokens[i+8..from_end].join(" ");
        let date_start=from_end+1;
        if !same_date(tokens.get(date_start..date_start+3).ok_or("Missing train departure date")?,day){return Err("Train departure date differs from requested date");}
        let elapsed=duration(tokens.get(date_start+3).ok_or("Missing duration")?).ok_or("Invalid journey duration")?;
        if elapsed<=Duration::zero(){return Err("Invalid zero duration");}
        let arr=clock(tokens.get(date_start+4).ok_or("Missing arrival")?).ok_or("Invalid arrival clock")?;
        if tokens.get(date_start+5)!=Some(&"|"){return Err("Missing arrival station separator");}
        let to_start=date_start+6;
        let to_end=(to_start..tokens.len().min(to_start+12)).find(|&j|tokens[j]=="|").ok_or("Missing destination station")?;
        let to=tokens[to_start..to_end].join(" ");
        if !within_station_list(&from_list,&from)||!within_station_list(&to_list,&to){return Err("A train endpoint is outside the observed journey station filters");}
        let expected=day.and_time(dep).checked_add_signed(elapsed).ok_or("Journey duration overflow")?;
        if expected.time()!=arr||!same_date(tokens.get(to_end+1..to_end+4).ok_or("Missing arrival date")?,expected.date()){
            return Err("Arrival time or date conflicts with observed journey duration");
        }
        let quote=tokens[cursor..to_end+4].join(" ");
        rows.push(Schedule{name,number:number.into(),departure:dep,arrival:arr,from,to,departure_date:day,arrival_date:expected.date(),quote});
        // Only advance across an explicit card boundary. Never borrow dates
        // or clocks from the next train, a filter, or the page footer.
        let next=(to_end+4..tokens.len().min(to_end+150)).find(|&j|tokens[j].ends_with("OTHER")&&tokens.get(j+1)==Some(&"DATES"));
        let Some(next)=next else{break;};cursor=next+2;
        if rows.len()>count{return Err("Unexpected extra train records");}
    }
    if rows.len()!=count{return Err("Visible result count does not match fully parsed train records");}
    Ok((header,day,rows))
}
pub fn verify(goal:&str,initial:&str,page:&DomSnapshot,sources:&[SourceEvidence],search_sent:bool)->Result<VerificationReport,String>{
    if !search_sent||!verified_origin(initial,&page.origin){return Err("No acknowledged search and matching public results origin".into());}
    let floor=departure_floor(goal)?;
    let (header,day,rows)=read_schedules(goal,page)?;
    let source=sources.iter().find(|s|s.url==page.origin&&s.title==page.title.chars().take(140).collect::<String>()).ok_or("No matching retained result evidence")?;
    if !bs::verification::quote_present(source,&header)||rows.iter().any(|r|!bs::verification::quote_present(source,&r.quote)){return Err("Schedule evidence is not contiguous in the retained live source".into());}
    let matches=rows.iter().filter(|r|floor.map_or(true,|f|r.departure>=f)).collect::<Vec<_>>();
    let mut findings=vec![GroundedFinding{claim:format!("Observed {} scheduled trains for {day}; {} match the requested departure window.",rows.len(),matches.len()),url:source.url.clone(),quote:header}];
    for r in &matches{
        findings.push(GroundedFinding{
            claim:format!("{} ({}) departs {} at {} on {} and arrives {} at {} on {} (scheduled local times).",r.name,r.number,r.from,r.departure.format("%H:%M"),r.departure_date,r.to,r.arrival.format("%H:%M"),r.arrival_date),
            url:source.url.clone(),quote:r.quote.clone()
        });
    }
    let limitations="Scheduled services only. Seat availability was not independently verified; refresh and booking controls are not seat inventory. No booking, reservation or payment was performed.".to_string();
    let summary=format!("{} observed scheduled train(s) match the requested departure window on {day}.\n{}\n{limitations}",matches.len(),findings.iter().skip(1).map(|f|f.claim.as_str()).collect::<Vec<_>>().join("\n"));
    Ok(VerificationReport{status:"evidence_checked",summary,findings,limitations})
}

#[cfg(test)]
mod tests{
 use super::*;
 const G:&str="Test a train booking search only. Find trains from Bengaluru to New Delhi for 2026-11-25 departing at or after 6 PM India time. Search public trains and report observed train names, times and any displayed availability.";
 const TEXT:&str=include_str!("../../../tests/e2e/fixtures/public-train-results-v13.txt");
 fn p(text:&str)->DomSnapshot{DomSnapshot{structure:Default::default(),origin:"https://www.irctc.co.in/nget/booking/train-list".into(),title:"Public train results".into(),text_excerpt:text.into(),interactive_nodes:vec![]}}
 fn check(g:&str,text:&str,search_sent:bool)->Result<VerificationReport,String>{
    let page=p(text);let source=SourceEvidence{url:page.origin.clone(),title:page.title.clone(),excerpt:text.into()};
    verify(g,"https://www.irctc.co.in/nget/train-search",&page,&[source],search_sent)
 }
 #[test]fn actual_capture_has_four_schedules_and_two_evening_departures(){
    let report=check(G,TEXT,true).unwrap();
    assert_eq!(report.findings.len(),3);assert!(report.summary.contains("12627")&&report.summary.contains("22691"));
    assert!(!report.summary.contains("12649")&&!report.summary.contains("22685"));
    assert!(report.limitations.contains("Seat availability was not independently verified"));
    assert!(report.findings.iter().all(|f|bs::normalize(TEXT).contains(&bs::normalize(&f.quote))));
 }
 #[test]fn incorrect_route_date_or_time_cannot_pass(){
    assert!(check(G,&TEXT.replace("4 Results for KSR BENGALURU NEW DELHI","4 Results for NEW DELHI KSR BENGALURU"),true).is_err());
    assert!(check(G,&TEXT.replace("25 Nov 2026","26 Nov 2026"),true).is_err());
    assert!(check(G,&TEXT.replace("19:20 | KSR","17:20 | KSR"),true).is_err());
    assert!(check(G,&TEXT.replace("05:30 | HAZRAT","05:31 | HAZRAT"),true).is_err());
    assert!(check(G,&TEXT.replace("19:20 | KSR BENGALURU | Wed, 25 Nov","19:20 | KSR BENGALURU | Thu, 26 Nov"),true).is_err());
 }
 #[test]fn count_alias_and_card_boundaries_are_required(){
    assert!(check(G,&TEXT.replace("4 Results for","5 Results for"),true).is_err());
    assert!(check(G,&TEXT.replace("19:20 | KSR BENGALURU","19:20 | OTHER STATION"),true).is_err());
    assert!(check(G,&TEXT.replace("(12627) Runs On:","(12627) Unknown:"),true).is_err());
    assert!(check(G,&TEXT.replace("(22691)","(12627)"),true).is_err());
 }
 #[test]fn no_search_receipt_no_fresh_source_no_verification(){
    assert!(check(G,TEXT,false).is_err());
    let page=p(TEXT);assert!(verify(G,"https://other.example/",&page,&[],true).is_err());
    assert!(verify(G,"https://www.irctc.co.in/nget/train-search",&page,&[],true).is_err());
    assert!(check(&G.replace("6 PM","6 PM and before 8 PM"),TEXT,true).is_err());
 }
}
