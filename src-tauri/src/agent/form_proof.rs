//! Independent native evidence for a user-requested form submission.
//! No model-generated "completed" text, click dispatch receipt, or unrelated
//! form's confirmation can authorize Form stage completion.
use super::{dom_extractor::DomSnapshot,policy::BrowserAction};

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct SubmissionReceipt {
    pub origin:String,
    pub confirmation:String,
    pub target:String,
}

fn words(input:&str)->Vec<String>{
    input.to_ascii_lowercase()
        .split(|c:char|!c.is_ascii_alphanumeric())
        .filter(|w|!w.is_empty()).map(str::to_string).collect()
}
fn negative_clause(goal:&str,keyword:&str)->bool{
    let lower=goal.to_ascii_lowercase();
    // Match the negation in the immediate clause, not a different sentence.
    lower.split(['.',';',',']).any(|clause|{
        (clause.contains("do not")||clause.contains("don't")||clause.contains("never"))
            && clause.contains(keyword) && clause.contains("submit")
    })
}
fn submission_requested(goal:&str)->bool{
    goal.to_ascii_lowercase().split(['.',';',',']).any(|clause|{
        (clause.contains("submit")||clause.contains("send "))&&
        !clause.contains("do not")&&!clause.contains("don't")&&
        !clause.contains("never")&&!clause.contains("without submitting")
    })
}
fn named_submit_target(goal:&str)->Option<String>{
    let lower=goal.to_ascii_lowercase();
    let clauses=lower.split(['.',';',',']).collect::<Vec<_>>();
    for clause in clauses{
        if !clause.contains("submit")||
            clause.contains("do not")||clause.contains("don't")||clause.contains("never"){
            continue;
        }
        for prefix in ["submit the ","submit my ","submit a ","submit an ","submit "]{
            let Some((_,after))=clause.split_once(prefix) else{continue};
            let Some((name,_))=after.split_once(" form")else{continue};
            let target=words(name).into_iter().filter(|w|!["this","current","requested","online","local","test"].contains(&w.as_str()))
                .collect::<Vec<_>>().join(" ");
            if !target.is_empty()&&target.len()<=65{return Some(target);}
        }
    }
    None
}
fn submit_label<'a>(page:&'a DomSnapshot,node_id:&str)->Option<&'a str>{
    page.interactive_nodes.iter().find(|n|n.node_id==node_id&&n.is_submit&&!n.is_sensitive)
        .map(|n|n.label.as_str())
}
pub fn authorize_submit(goal:&str,page:&DomSnapshot,action:&BrowserAction)->Result<String,String>{
    if !submission_requested(goal){
        return Err("The user did not authorize submitting any form".into());
    }
    let BrowserAction::Click{node_id,is_submit:true,..}=action else{
        return Err("Form proof requires an observed native submit-button click".into())
    };
    let label=submit_label(page,node_id)
        .ok_or("Submitted control was not a live, non-sensitive submit button")?.to_ascii_lowercase();
    if negative_clause(goal,&label){
        return Err("Submit control belongs to a form explicitly prohibited by the user".into())
    }
    let buttons=page.interactive_nodes.iter().filter(|n|n.is_submit&&!n.is_sensitive).collect::<Vec<_>>();
    if let Some(target)=named_submit_target(goal){
        let terms=words(&target);
        if terms.is_empty() || !terms.iter().any(|term|label.split_whitespace().any(|w|w==term)){
            // A generic Submit control may be used if it is the sole form
            // submit on the page. Never infer ownership among multiple forms.
            if buttons.len()!=1 || !["submit","send","save"].contains(&label.trim()) {
                return Err(format!("Submit control does not identify the requested {} form",target))
            }
        }
        if negative_clause(goal,&target){return Err("Requested form target is negated".into())}
        return Ok(target);
    }
    if buttons.len()!=1{
        return Err("Ambiguous submit controls; the intended form has not been identified".into())
    }
    Ok(label)
}
/// Conservative fallback when the form stage has already interacted with
/// fields and the model proposes an irrelevant action. Only an explicit
/// user-named form with exactly one matching observed submit control qualifies.
/// The ordinary action policy still authorizes/denies the actual click.
pub fn observed_intended_submit(goal:&str,page:&DomSnapshot)->Option<BrowserAction>{
    named_submit_target(goal)?;
    let options=page.interactive_nodes.iter()
        .filter(|n|n.is_submit&&!n.is_sensitive)
        .filter_map(|n|{
            let candidate=BrowserAction::Click{
                node_id:n.node_id.clone(),element_label:n.label.clone(),is_submit:true
            };
            authorize_submit(goal,page,&candidate).ok().map(|_|candidate)
        }).collect::<Vec<_>>();
    (options.len()==1).then(||options.into_iter().next()).flatten()
}
fn confirmation_terms(target:&str)->&'static [&'static str]{
    if target.contains("feedback"){&["feedback sent","feedback received","feedback submitted"]}
    else if target.contains("application"){&["application received","application submitted"]}
    else if target.contains("registration"){&["registration complete","registration successful"]}
    else if target.contains("message"){&["message sent","message received"]}
    else if target.contains("order"){&["order confirmed"]}
    else {&["successfully submitted","submission received","form submitted",
            "thank you for submitting","confirmation number"]}
}
pub fn verify_submission(goal:&str,action:&BrowserAction,before:&DomSnapshot,after:&DomSnapshot)->Result<SubmissionReceipt,String>{
    let target=authorize_submit(goal,before,action)?;
    super::evidence_verifier::check_submission_result(before,after)?;
    if !after.origin.starts_with("http") {return Err("Submission confirmation is not from a website".into())}
    let now=after.text_excerpt.to_ascii_lowercase();
    let previous=before.text_excerpt.to_ascii_lowercase();
    let confirmation=confirmation_terms(&target).iter().find(|term|
        now.contains(*term) && !previous.contains(*term)
    ).ok_or_else(||format!("No new confirmation tied to the requested {} form",target))?;
    Ok(SubmissionReceipt{origin:after.origin.clone(),confirmation:(*confirmation).into(),target})
}
pub fn receipt_visible(receipt:&SubmissionReceipt,page:&DomSnapshot)->bool{
    page.origin==receipt.origin
        &&page.text_excerpt.to_ascii_lowercase().contains(&receipt.confirmation)
}
/// A deterministic independent final check: the model supplies neither the
/// chosen form nor the claim/quote. Both come from the prior authorized
/// action and native WebView observations, with source-exact readback.
pub fn verified_report(
    receipt:&SubmissionReceipt,page:&DomSnapshot,
    sources:&[super::orchestrator::SourceEvidence]
)->Result<super::evidence_verifier::VerificationReport,String>{
    if !receipt_visible(receipt,page){
        return Err("Previously confirmed form receipt is not visible on the current page".into())
    }
    let source=sources.iter().find(|s|s.url==page.origin)
        .ok_or("No captured source exists for the form confirmation")?;
    let quote=source.excerpt.lines().map(str::trim).find(|line|{
        let len=line.chars().count();
        (25..=1600).contains(&len)&&line.to_ascii_lowercase().contains(&receipt.confirmation)
    }).ok_or("No source-exact confirmation text is present in captured page evidence")?;
    if !super::browser_skills::verification::quote_present(source,quote){
        return Err("Form confirmation quote failed source-exact readback".into())
    }
    Ok(super::evidence_verifier::VerificationReport{
        status:"evidence_checked".into(),
        summary:format!("Confirmed the requested {} form submission from new visible native page evidence.",receipt.target),
        findings:vec![super::evidence_verifier::GroundedFinding{
            claim:format!("Requested {} form submission confirmed on the resulting page",receipt.target),
            url:source.url.clone(),quote:quote.to_string()
        }],
        limitations:"Only the on-page confirmation of the authorized submission was verified; downstream processing is not independently established.".into()
    })
}
#[cfg(test)]
mod tests{
    use super::*;
    use super::super::dom_extractor::DomNodeSummary;
    fn page(text:&str)->DomSnapshot{
        DomSnapshot{
            origin:"http://127.0.0.1:18764/".into(), title:"Fixture".into(),
            text_excerpt:text.into(),structure:Default::default(),
            interactive_nodes:vec![
                DomNodeSummary{node_id:"apply".into(),label:"Submit sample application".into(),
                    role:"button".into(),tag:"button".into(),href:None,is_submit:true,is_sensitive:false,
                    options:vec![],checked:None,search_state:None,bbox:None},
                DomNodeSummary{node_id:"feedback".into(),label:"Submit feedback".into(),
                    role:"button".into(),tag:"button".into(),href:None,is_submit:true,is_sensitive:false,
                    options:vec![],checked:None,search_state:None,bbox:None}
            ]
        }
    }
    const GOAL:&str="On the current local test website, submit the feedback form, then report the visible feedback confirmation. Do not submit the application form.";
    fn click(id:&str)->BrowserAction{BrowserAction::Click{
        node_id:id.into(),element_label:"Untrusted".into(),is_submit:true
    }}
    #[test]fn negated_submit_does_not_authorize_any_form(){
        let before=page("Two forms are present on this page.");
        assert!(authorize_submit("Fill the feedback form; do not submit it",&before,&click("feedback")).is_err());
        assert!(authorize_submit("Do not submit the application form",&before,&click("apply")).is_err());
        assert!(authorize_submit("Read the page and summarize it",&before,&click("feedback")).is_err());
    }
    #[test]fn unrelated_application_cannot_be_submitted(){
        let before=page("Feedback has not been sent. No sample application submitted.");
        assert!(authorize_submit(GOAL,&before,&click("apply")).is_err());
        assert_eq!(authorize_submit(GOAL,&before,&click("feedback")).unwrap(),"feedback");
    }
    #[test]fn only_new_target_specific_confirmation_creates_receipt(){
        let before=page("Feedback has not been sent. No sample application submitted.");
        let application=page("Application received. Feedback has not been sent.");
        assert!(verify_submission(GOAL,&click("feedback"),&before,&application).is_err());
        let success=page("Feedback sent successfully on the local test page.");
        let receipt=verify_submission(GOAL,&click("feedback"),&before,&success).unwrap();
        assert!(receipt_visible(&receipt,&success));
        assert!(!receipt_visible(&receipt,&before));
        assert!(!receipt_visible(&receipt,&page("The page changed without feedback confirmation.")));
    }
    #[test]fn user_named_form_has_one_grounded_submit_candidate(){
        let before=page("Two forms are present on this page.");
        assert!(matches!(observed_intended_submit(GOAL,&before),Some(BrowserAction::Click{node_id,..}) if node_id=="feedback"));
        assert!(observed_intended_submit("Submit a form",&before).is_none());
        assert!(observed_intended_submit("Do not submit the application form",&before).is_none());
    }
    #[test]fn stale_banner_cannot_satisfy_a_new_submission(){
        let before=page("Feedback sent successfully earlier on this page.");
        let after=page("Feedback sent successfully earlier on this page. Another panel opened.");
        assert!(verify_submission(GOAL,&click("feedback"),&before,&after).is_err());
    }
    #[test]fn independent_report_requires_source_exact_visible_receipt(){
        let before=page("Feedback has not been sent.");
        let after=page("Feedback sent successfully on the local test page.");
        let receipt=verify_submission(GOAL,&click("feedback"),&before,&after).unwrap();
        let captured=super::super::orchestrator::SourceEvidence{
            url:after.origin.clone(),title:after.title.clone(),
            excerpt:after.text_excerpt.clone()
        };
        let report=verified_report(&receipt,&after,&[captured.clone()]).unwrap();
        assert_eq!(report.status,"evidence_checked");
        assert_eq!(report.findings[0].url,after.origin);
        assert_eq!(report.findings[0].quote,after.text_excerpt);
        assert!(verified_report(&receipt,&before,&[captured.clone()]).is_err());
        assert!(verified_report(&receipt,&after,&[]).is_err());
        let forged=super::super::orchestrator::SourceEvidence{
            excerpt:"A different notification appeared on the page.".into(),..captured
        };
        assert!(verified_report(&receipt,&after,&[forged]).is_err());
    }
    #[test]fn ambiguous_generic_submit_is_rejected(){
        let before=page("Forms on the same page.");
        assert!(authorize_submit("Submit a form",&before,&click("feedback")).is_err());
    }
}