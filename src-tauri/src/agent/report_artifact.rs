//! User-requested local research report export. No writing starts until the
//! independent factual reviewer has returned source-exact checked findings.
//! The report is generated deterministically from captured evidence: no model
//! may invent a claim, source, citation or file path at this stage.
use super::{browser_skills,evidence_verifier::VerificationReport,orchestrator::SourceEvidence};
use anyhow::{Context,ensure,Result};
use serde::Serialize;
use sha2::{Digest,Sha256};
use std::{collections::HashSet,fs,path::{Path,PathBuf}};

#[derive(Clone,Debug,Serialize)]
pub struct Receipt {
    pub path:String,
    pub sha256:String,
    pub bytes:usize,
    pub readback_verified:bool,
    pub source_count:usize,
}
pub fn requested(goal:&str)->bool{
    let lower=goal.to_ascii_lowercase();
    let report=["write a report","create a report","prepare a report",
        "generate a research report","research report"].iter().any(|s|lower.contains(s));
    report&&(lower.contains("research")||lower.contains("sources")||lower.contains("citations"))
       &&!["publish","email","send ","upload","share","delete","private","password",
            "credential","purchase","buy ","do not write","don't write",
            "in google docs","in notion","in the editor","on this page","to an editor",
            "in a shared document","in an online document"].iter().any(|s|lower.contains(s))
}
fn digest(raw:&[u8])->String{format!("{:x}",Sha256::digest(raw))}
fn safe_text(raw:&str,limit:usize)->String{
    raw.chars().filter(|c|!c.is_control()||*c=='\n')
        .take(limit).collect::<String>()
        .replace('<',"&lt;").replace('>',"&gt;")
}
fn unique_grounded_sources(report:&VerificationReport,sources:&[SourceEvidence])->Result<Vec<(String,String,String)>>{
    ensure!(report.status=="evidence_checked","Independent verification receipt required");
    let mut found=Vec::new();
    let mut urls=HashSet::new();
    let mut passages=HashSet::new();
    for finding in &report.findings{
        let u=browser_skills::public_url(&finding.url)
            .context("Nonpublic or invalid research source")?;
        ensure!(u.scheme()=="https"&&u.username().is_empty()&&u.password().is_none(),
            "Research report requires publicly observed HTTPS sources");
        ensure!((25..=1600).contains(&finding.quote.chars().count())&&
            !finding.claim.trim().is_empty()&&finding.claim.chars().count()<=700,
            "Report finding lacks a usable exact quote");
        let matching=sources.iter().any(|s|s.url==finding.url&&
            browser_skills::verification::quote_present(s,&finding.quote));
        ensure!(matching,"A report quote is absent from the native WebView source");
        urls.insert(u.to_string());
        if passages.insert((u.to_string(),finding.quote.clone())){
            found.push((u.to_string(),finding.claim.clone(),finding.quote.clone()));
        }
    }
    ensure!(urls.len()>=2,"Research report requires two distinct observed sources");
    Ok(found)
}
fn content(goal:&str,findings:&[(String,String,String)],limitations:&str)->String{
    let mut text=format!("# Verified research report\n\n## Original objective\n\n{}\n\n## Evidence-backed findings\n\n",
        safe_text(goal,1800));
    for (i,(url,claim,quote)) in findings.iter().enumerate(){
        text.push_str(&format!("### Finding {}\n\n{}\n\n> {}\n\nSource: {}\n\n",
            i+1,safe_text(claim,700),safe_text(quote,1600).replace('\n',"\n> "),
            url));
    }
    text.push_str(&format!("## Scope and limitations\n\n{}\n\n",
        safe_text(limitations,1200)));
    text
}
pub fn readback_verified(path:&Path,sha256:&str)->Result<bool>{
    let metadata=fs::symlink_metadata(path)?;
    ensure!(metadata.is_file()&&!metadata.file_type().is_symlink(),"Report must be an ordinary file");
    let bytes=fs::read(path)?;
    Ok(digest(&bytes)==sha256)
}
/// save() must run only after the parent workflow's *native* final report gate.
/// On failure it creates no success receipt; never append 'completed' early.
pub fn save(goal:&str,report:&VerificationReport,sources:&[SourceEvidence],data_dir:&Path)->Result<Receipt>{
    ensure!(requested(goal),"User did not request a local research report artifact");
    let rows=unique_grounded_sources(report,sources)?;
    let body=content(goal,&rows,&report.limitations);
    ensure!((200..=30000).contains(&body.len()),"Generated report is out of bounds");
    let root:PathBuf=data_dir.join("research-reports");
    fs::create_dir_all(&root)?;
    let id=uuid::Uuid::new_v4();
    let file=root.join(format!("lucia-report-{id}.md"));
    let tmp=root.join(format!("lucia-report-{id}.tmp"));
    let bytes=body.as_bytes();
    let sha256=digest(bytes);
    fs::write(&tmp,bytes)?;
    if let Err(e)=fs::rename(&tmp,&file){
        let _=fs::remove_file(&tmp);
        return Err(e.into());
    }
    if !readback_verified(&file,&sha256)?{
        let _=fs::remove_file(&file);
        anyhow::bail!("Report failed byte-exact disk readback");
    }
    Ok(Receipt{path:file.to_string_lossy().to_string(),sha256,
        bytes:bytes.len(),readback_verified:true,
        source_count:rows.iter().map(|(url,_,_)|url).collect::<HashSet<_>>().len()})
}
#[cfg(test)]
mod tests{
    use super::*;
    use super::super::evidence_verifier::GroundedFinding;
    fn sample()->(String,VerificationReport,Vec<SourceEvidence>){
        let goal="Research the two public docs sources and write a report with citations".to_string();
        let a="A list comprehension creates a new list from an iterable.";
        let b="Array.map creates a new array populated with callback results.";
        let sources=vec![
            SourceEvidence{url:"https://docs.python.org/example".into(),title:"Python".into(),excerpt:a.into()},
            SourceEvidence{url:"https://developer.mozilla.org/example".into(),title:"MDN".into(),excerpt:b.into()}
        ];
        let findings=sources.iter().map(|s|GroundedFinding{
            claim:s.excerpt.clone(),url:s.url.clone(),quote:s.excerpt.clone()}).collect();
        (goal,VerificationReport{status:"evidence_checked",summary:"Compared outputs".into(),
            findings,limitations:"Two documentation sites were examined.".into()},sources)
    }
    #[test]fn creates_and_reads_back_source_bound_report(){
        let (goal,report,sources)=sample();
        let dir=std::env::temp_dir().join(format!("lucia-report-unit-{}",uuid::Uuid::new_v4()));
        let receipt=save(&goal,&report,&sources,&dir).unwrap();
        assert!(receipt.readback_verified&&receipt.source_count==2);
        assert!(readback_verified(Path::new(&receipt.path),&receipt.sha256).unwrap());
        let on_disk=fs::read_to_string(&receipt.path).unwrap();
        assert!(sources.iter().all(|s|on_disk.contains(&s.url)));
        fs::write(&receipt.path,"Altered").unwrap();
        assert!(!readback_verified(Path::new(&receipt.path),&receipt.sha256).unwrap());
        fs::remove_dir_all(&dir).unwrap();
    }
    #[test]fn missing_or_unverified_evidence_cannot_create_a_report(){
        let (goal,mut report,sources)=sample();
        let dir=std::env::temp_dir().join(format!("lucia-report-denied-{}",uuid::Uuid::new_v4()));
        report.status="unverified";
        assert!(save(&goal,&report,&sources,&dir).is_err());
        assert!(!dir.exists());
        report.status="evidence_checked";
        report.findings[0].quote="An invented source quotation not on the page.".into();
        assert!(save(&goal,&report,&sources,&dir).is_err());
        assert!(!dir.exists());
        report.findings.remove(0);
        assert!(save(&goal,&report,&sources,&dir).is_err());
        assert!(!dir.exists());
        assert!(!requested("Explain the report status of my purchase"));
        assert!(requested("Research two official sources and write a report with citations"));
        assert!(!requested("Research two official sources and write a report in Google Docs"));
        assert!(!requested("Research sources and send a report by email"));
    }
}