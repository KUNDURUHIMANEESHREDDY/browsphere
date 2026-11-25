//! Opt-in structured public-page extraction. No model-generated cell or chart values.
use anyhow::{ensure,Context,Result};
use serde::{Deserialize,Serialize};
use super::evidence_verifier::{GroundedFinding,VerificationReport};
use super::dom_extractor::DomSnapshot;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Mode{Tables,Charts,Both}
pub fn requested(goal:&str)->Option<Mode>{
    let g=goal.trim().trim_end_matches('.').to_ascii_lowercase();
    match g.as_str(){
        "extract table from this page"|"extract tables from this page"|"extract the table from this page"|"extract the tables from this page"=>Some(Mode::Tables),
        "extract chart from this page"|"extract charts from this page"|"extract the chart from this page"|"extract the charts from this page"=>Some(Mode::Charts),
        "extract tables and charts from this page"|"extract charts and tables from this page"|"extract the tables and charts from this page"=>Some(Mode::Both),
        _=>None
    }
}
#[derive(Debug,Clone,Deserialize,Serialize,PartialEq,Eq)]
#[serde(deny_unknown_fields)]
struct Table{caption:String,headers:Vec<String>,rows:Vec<Vec<String>>}
#[derive(Debug,Clone,Deserialize,Serialize,PartialEq,Eq)]
#[serde(deny_unknown_fields)]
struct Point{label:String,value:String}
#[derive(Debug,Clone,Deserialize,Serialize,PartialEq,Eq)]
#[serde(deny_unknown_fields)]
struct Chart{caption:String,points:Vec<Point>}
#[derive(Debug,Clone,Deserialize,Serialize,PartialEq,Eq)]
#[serde(deny_unknown_fields)]
pub struct Extraction{url:String,title:String,tables:Vec<Table>,charts:Vec<Chart>,omitted_tables:usize,omitted_charts:usize}
fn field(s:&str,allow_empty:bool)->bool{
    (allow_empty||!s.is_empty())&&s.trim()==s&&s.chars().count()<=160&&!s.chars().any(char::is_control)
}
fn numeric(s:&str)->bool{
    let t=s.trim_end_matches(|c:char|c.is_ascii_alphabetic()||c=='%').trim_end();
    !t.is_empty()&&t.parse::<f64>().is_ok_and(|v|v.is_finite())&&s.len()<=32
}
/// Collapse whitespace exactly as the DOM snapshot does, without changing case,
/// so any slice of the result is a literal passage of the observed page.
fn collapsed(s:&str)->String{s.split_whitespace().collect::<Vec<_>>().join(" ")}
fn find_ignoring_case(hay:&[char],needle:&[char])->Option<usize>{
    if needle.is_empty()||needle.len()>hay.len(){return None;}
    (0..=hay.len()-needle.len()).find(|&i|(0..needle.len())
        .all(|j|hay[i+j].to_lowercase().to_string()==needle[j].to_lowercase().to_string()))
}
/// Return the exact contiguous page passage that contains every part in order,
/// or `None` when the rendered text was not observed. A quote is never
/// synthesized from structured fields: separators, headers and labels are
/// presentation, not page text, and must not appear inside a citation.
fn observed_quote(excerpt:&str,parts:&[&str])->Option<String>{
    let hay=collapsed(excerpt);
    let needle=parts.iter().map(|p|collapsed(p)).filter(|p|!p.is_empty())
        .collect::<Vec<_>>().join(" ");
    let hay=hay.chars().collect::<Vec<_>>();
    let needle=needle.chars().collect::<Vec<_>>();
    let at=find_ignoring_case(&hay,&needle)?;
    Some(hay[at..at+needle.len()].iter().collect())
}
impl Extraction{
    fn validate(&self)->Result<()>{
        super::workflow::workflow_public_url(&self.url)?;
        ensure!(!self.title.is_empty()&&self.title.chars().count()<=512&&self.tables.len()<=4&&self.charts.len()<=4,"Invalid extraction bounds");
        for t in &self.tables{
            let n=t.headers.len();
            ensure!(field(&t.caption,true)&&(2..=10).contains(&n)&&(1..=20).contains(&t.rows.len())&&t.headers.iter().all(|h|field(h,false))&&t.headers.iter().collect::<std::collections::BTreeSet<_>>().len()==n,"Invalid table headings or size");
            ensure!(t.rows.iter().all(|r|r.len()==n&&r.iter().all(|v|field(v,true))),"Nonrectangular or invalid table cells");
        }
        for c in &self.charts{
            ensure!(field(&c.caption,false)&&(2..=30).contains(&c.points.len())&&c.points.iter().all(|p|field(&p.label,false)&&field(&p.value,false)&&numeric(&p.value))&&c.points.iter().map(|p|&p.label).collect::<std::collections::BTreeSet<_>>().len()==c.points.len(),"Invalid chart labels or explicit numeric values");
        }
        Ok(())
    }
    fn report(&self,mode:Mode,excerpt:&str)->Result<VerificationReport>{
        self.validate()?;
        let want_tables=mode!=Mode::Charts;
        let want_charts=mode!=Mode::Tables;
        ensure!((!want_tables||!self.tables.is_empty())&&(!want_charts||!self.charts.is_empty()),"No complete supported table or explicitly labelled SVG chart for the requested extraction; no values inferred");
        let mut findings=vec![];
        let mut lines=vec![];
        let mut ungrounded=0usize;
        if want_tables{
            for (i,t) in self.tables.iter().enumerate(){
                let title=if t.caption.is_empty(){format!("Table {}",i+1)}else{t.caption.clone()};
                lines.push(title.clone());
                lines.push(t.headers.join(" | "));
                for row in &t.rows{
                    lines.push(row.join(" | "));
                    let parts=row.iter().map(String::as_str).collect::<Vec<_>>();
                    match observed_quote(excerpt,&parts){
                        Some(quote)=>findings.push(GroundedFinding{
                            claim:format!("{title}: {}",t.headers.iter().zip(row).map(|(h,v)|format!("{h} = {v}")).collect::<Vec<_>>().join("; ")),
                            url:self.url.clone(),quote
                        }),
                        None=>ungrounded+=1,
                    }
                }
            }
        }
        if want_charts{
            for c in &self.charts{
                lines.push(c.caption.clone());
                for p in &c.points{
                    lines.push(format!("{}: {}",p.label,p.value));
                    match observed_quote(excerpt,&[&p.label,&p.value]){
                        Some(quote)=>findings.push(GroundedFinding{claim:format!("{} ? {}: {}",c.caption,p.label,p.value),url:self.url.clone(),quote}),
                        None=>ungrounded+=1,
                    }
                }
            }
        }
        let limitations=format!("Native rendered cells and explicit SVG text pairs only; values are transcribed, not independently fact-checked. Unsupported/oversized table regions omitted: {}; unsupported graphic regions omitted: {}. Merged cells, canvas/raster charts, unlabeled marks, axes and series inference are not supported. Findings quote only contiguous rendered page text; a value whose rendered text was not observed contiguously is reported but not cited as a finding ({} omitted).",self.omitted_tables,self.omitted_charts,ungrounded);
        Ok(VerificationReport{status:if ungrounded==0{"evidence_checked"}else{"partial_evidence"},summary:format!("{}\nSource: {}\n{}",lines.join("\n"),self.url,limitations),findings,limitations})
    }
}
async fn capture(app:&tauri::AppHandle,label:&str,page:&DomSnapshot)->Result<Extraction>{
    let raw=super::native_eval::eval_json(app,label,include_str!("structured_data.js").to_owned(),3000).await.map_err(anyhow::Error::msg)?;
    let data:Extraction=serde_json::from_value(raw).context("No native structured extraction")?;
    data.validate()?;
    ensure!(data.url==page.origin&&data.title==page.title,"Structured extraction source changed");
    Ok(data)
}
pub async fn run(app:&tauri::AppHandle,label:&str,page:&DomSnapshot,mode:Mode,config:&crate::router::model_router::RouterConfig,cancel:tokio_util::sync::CancellationToken)->Result<VerificationReport>{
    super::workflow::workflow_public_url(&page.origin)?;
    ensure!(super::multimodal_perception::independent_discovery_allowed(page),"Structured extraction requires an observed public page");
    let result=tokio::select!{biased;_=cancel.cancelled()=>anyhow::bail!("Extraction cancelled"),r=async{
        let first=capture(app,label,page).await?;
        let second=capture(app,label,page).await?;
        ensure!(first==second,"Structured data changed between native observations");
        let native=first.report(mode,&page.text_excerpt);
        if mode==Mode::Tables||first.omitted_charts==0{return native;}
        let pixels=super::pixel_chart::run(app,label,page,config).await?;
        if pixels.charts.is_empty(){return native;}
        let mut report=native.or_else(|_|{
            if mode==Mode::Both&&!first.tables.is_empty(){first.report(Mode::Tables,&page.text_excerpt)}else{
                Ok(VerificationReport{status:"partial_evidence",summary:String::new(),findings:vec![],limitations:String::new()})
            }
        })?;
        // Model transcriptions are kept out of grounded findings. Two reads
        // by the same model are not independent proof, even if identical.
        report.status="partial_evidence";
        report.summary.push_str("\n\nProvisional visual chart readings (not independently verified):");
        for chart in pixels.charts{
            report.summary.push_str(&format!("\n{} ({}):",chart.caption,chart.kind));
            for p in chart.points{report.summary.push_str(&format!("\n{}: {}",p.label,p.value));}
        }
        let native_limitations=report.limitations.clone();
        report.limitations=format!("{native_limitations}\nCanvas/raster readings are provisional agreement between two local model passes, not independent verification. No values estimated from geometry. Unreadable or unsupported visual regions: {}.",pixels.omitted);
        report.summary.push_str(&format!("\nSource: {}\n{}",first.url,report.limitations));
        Ok(report)
    }=>r?};
    Ok(result)
}
#[cfg(test)]
mod tests{
    use super::*;
    use serde_json::json;
    fn sample()->Extraction{serde_json::from_value(json!({"url":"https://example.org/data","title":"Data","tables":[{"caption":"Rainfall","headers":["Month","mm"],"rows":[["Jan","12"],["Feb","9"]]}],"charts":[{"caption":"Rainfall chart","points":[{"label":"Jan","value":"12 mm"},{"label":"Feb","value":"9 mm"}]}],"omitted_tables":0,"omitted_charts":0})).unwrap()}
    /// Exactly what `document.body.innerText` renders for this fixture after the
    /// snapshot collapses whitespace.
    const EXCERPT:&str="Rainfall Month mm Jan 12 Feb 9 Rainfall chart Jan 12 mm Feb 9 mm";
    #[test]fn exact_structured_intents_only(){
        assert_eq!(requested("Extract tables and charts from this page."),Some(Mode::Both));
        assert_eq!(requested("Extract tables from this page and send them"),None);
        assert_eq!(requested("Extract charts from my account"),None);
    }
    #[test]fn extraction_transcribes_exact_cells_and_pairs(){
        let report=sample().report(Mode::Both,EXCERPT).unwrap();
        assert_eq!(report.status,"evidence_checked");
        assert_eq!(report.findings.len(),4);
        assert_eq!(report.findings[0].claim,"Rainfall: Month = Jan; mm = 12");
        assert_eq!(report.findings[0].quote,"Jan 12");
        assert_eq!(report.findings[1].quote,"Feb 9");
        assert_eq!(report.findings[2].quote,"Jan 12 mm");
        assert_eq!(report.findings[3].quote,"Feb 9 mm");
        assert!(report.limitations.contains("canvas/raster"));
    }
    /// The citation contract: every quote is a contiguous passage of the
    /// observed page, never a transcription joined with separators the page
    /// does not render.
    #[test]fn findings_quote_only_contiguous_rendered_text(){
        let report=sample().report(Mode::Both,EXCERPT).unwrap();
        let page=collapsed(EXCERPT).to_lowercase();
        assert!(!report.findings.is_empty());
        for f in &report.findings{
            let quote=collapsed(&f.quote).to_lowercase();
            assert!(page.contains(&quote),"{:?} is not contiguous page text",f.quote);
            assert!(!f.quote.contains('|')&&!f.quote.contains('=')&&!f.quote.contains(": "),"quote uses a synthesized separator: {:?}",f.quote);
        }
        // Case and internal whitespace in the observation do not invalidate a
        // real passage.
        let noisy="RAINFALL\n Month\t\tmm   jan\n12";
        assert_eq!(sample().report(Mode::Tables,noisy).unwrap().findings[0].quote,"jan 12");
    }
    /// A value the page did not render contiguously is reported in the summary
    /// but is never cited as grounded evidence.
    #[test]fn ungrounded_values_are_not_cited(){
        let report=sample().report(Mode::Tables,"Rainfall Month mm Jan 12").unwrap();
        assert_eq!(report.status,"partial_evidence");
        assert_eq!(report.findings.len(),1);
        assert_eq!(report.findings[0].quote,"Jan 12");
        assert!(report.summary.contains("Feb | 9"));
        assert!(report.limitations.contains("1 omitted"));
    }
    #[test]fn chart_pairs_require_rendered_svg_text(){
        let tables_only="Rainfall Month mm Jan 12 Feb 9";
        let report=sample().report(Mode::Charts,tables_only).unwrap();
        assert_eq!(report.status,"partial_evidence");
        assert!(report.findings.is_empty());
        assert!(report.summary.contains("Jan: 12 mm"));
        assert!(report.limitations.contains("2 omitted"));
    }
    #[test]fn refuses_ambiguous_missing_oversized_or_private_data(){
        let mut data=sample();data.tables[0].rows[0].pop();assert!(data.report(Mode::Tables,EXCERPT).is_err());
        let mut data=sample();data.charts[0].points[0].value="approximately high".into();assert!(data.report(Mode::Charts,EXCERPT).is_err());
        let mut data=sample();data.charts.clear();assert!(data.report(Mode::Both,EXCERPT).is_err());
        let mut data=sample();data.url="https://example.org/account".into();assert!(data.report(Mode::Tables,EXCERPT).is_err());
        let mut data=sample();data.tables[0].rows=vec![vec!["Jan".into(),"12".into()];21];assert!(data.report(Mode::Tables,EXCERPT).is_err());
    }
}
