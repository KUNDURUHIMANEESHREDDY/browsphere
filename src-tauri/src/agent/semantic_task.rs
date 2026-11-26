//! Grounded semantic interpretation of the user goal, produced by the
//! existing planner model. Interpretations are advisory, never permissions
//! or evidence. Native execution and independent verification retain authority.
use serde::{Deserialize,Serialize};
use std::collections::{HashSet,BTreeSet};

#[derive(Clone,Copy,Debug,Deserialize,Serialize,PartialEq,Eq,Hash)]
#[serde(rename_all="snake_case")]
pub enum ConceptKind{Objective,Constraint,Context,Entity}
#[derive(Clone,Debug,Deserialize,Serialize)]
#[serde(deny_unknown_fields)]
pub struct Concept{
    pub kind:ConceptKind,
    pub span:String,
    // 1-based execution stage indices; empty means global context/constraint.
    pub stages:Vec<usize>,
}
#[derive(Clone,Copy,Debug,Deserialize,Serialize,PartialEq,Eq,Hash)]
#[serde(rename_all="snake_case")]
pub enum RelationKind{DependsOn,Qualifies,RefersTo}
#[derive(Clone,Debug,Deserialize,Serialize)]
#[serde(deny_unknown_fields)]
pub struct Relation{
    pub from:usize,pub to:usize,pub kind:RelationKind,
}
#[derive(Clone,Debug,Deserialize,Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticTask{
    pub concepts:Vec<Concept>,
    pub relations:Vec<Relation>,
}
/// Explicit language anchors are recovered from the user's text, independently
/// of the model's interpretation. This is a conservative coverage contract,
/// not a general natural-language parser or a new source of authority.
#[derive(Clone,Debug,Default)]
pub struct ExplicitContract{
    pub ordered:Vec<String>,
    pub constraints:Vec<String>,
}
fn sequencing(s:&str)->bool{
    let g=s.to_ascii_lowercase();
    ["then ","next ","after that ","finally ","and then ","and finally "].iter().any(|p|g.starts_with(p))
}
fn constraint(s:&str)->bool{
    let g=s.to_ascii_lowercase();
    ["do not ","don't ","never ","without ","only ","avoid ","no ","limit "].iter().any(|p|g.starts_with(p))||
        ((g.starts_with("use ")||g.starts_with("read ")||g.starts_with("search "))&&g.ends_with(" only"))
}
/// "and then"/"and finally" is one clause starter, not a clause plus a marker.
/// The whitespace split must not cut between a joining conjunction and the
/// sequencing word that completes it.
fn joining(s:&str)->bool{
    s.rsplit(|c:char|c.is_whitespace()).next()
        .is_some_and(|w|matches!(w.to_ascii_lowercase().as_str(),"and"|"but"|"or"|"so"))
}
fn clauses(goal:&str)->Vec<&str>{
    let mut out=Vec::new();
    let mut start=0usize;
    let mut quote=None;
    let mut escaped=false;
    for (i,c) in goal.char_indices(){
        if escaped{escaped=false;continue;}
        if quote.is_some()&&c=='\\'{escaped=true;continue;}
        if let Some(q)=quote{if q==c{quote=None;}continue;}
        if matches!(c,'"'|'`'){quote=Some(c);continue;}
        let tail=goal[i+c.len_utf8()..].trim_start();
        let stop=matches!(c,';'|'\n')||
            (matches!(c,'.'|'!'|'?')&&(tail.is_empty()||goal[i+c.len_utf8()..].starts_with(char::is_whitespace)))||
            (c==','&&(sequencing(tail)||constraint(tail)))||
            (c.is_whitespace()&&sequencing(tail)&&!goal[start..i].trim().is_empty()&&!joining(&goal[start..i]));
        if stop{
            let part=goal[start..i].trim().trim_end_matches(',').trim();
            if !part.is_empty(){out.push(part);}
            start=i+c.len_utf8();
        }
    }
    let last=goal[start..].trim().trim_end_matches(['.','!','?',';']).trim();
    if !last.is_empty(){out.push(last);}
    out
}
impl ExplicitContract{
    pub fn extract(goal:&str)->Self{
        let mut contract=Self::default();
        let parts=clauses(goal);
        for part in &parts{
            if constraint(part)&&part.chars().count()<=200{
                contract.constraints.push((*part).to_string());
            }
        }
        let actions=parts.iter().filter(|part|!constraint(part)).copied().collect::<Vec<_>>();
        if actions.len()>=2&&actions[0].to_ascii_lowercase().starts_with("first "){
            let chain=actions.iter().take_while(|part|**part==actions[0]||sequencing(part)).copied().collect::<Vec<_>>();
            if chain.len()>=2{
                contract.ordered=chain.into_iter().map(str::to_owned).collect();
            }
        }
        contract
    }
    pub fn required(&self)->bool{!self.ordered.is_empty()||!self.constraints.is_empty()}
}
fn objective_body(span:&str)->String{
    // Discourse markers establish order, not a new objective identity.
    // Never strip action verbs, negation, entities or qualifying content.
    let lower=span.trim().to_lowercase();
    ["first ","and then ","and finally ","after that ","then ","next ","finally "].iter()
        .find_map(|p|lower.strip_prefix(p)).unwrap_or(&lower)
        .split_whitespace().collect::<Vec<_>>().join(" ")
}
fn has_anchor(c:&Concept,anchor:&str)->bool{
    objective_body(&c.span)==objective_body(anchor)
}
/// Collapse only demonstrably equivalent objective spans, within the same
/// stage scope. Rewrite all edges through a stable old-to-new index map.
/// Do not merge objectives merely because they share words or a stage.
fn canonicalize(task:&mut SemanticTask)->anyhow::Result<()>{
    let old=std::mem::take(&mut task.concepts);
    let mut nodes:Vec<Concept>=Vec::new();
    let mut remap=Vec::with_capacity(old.len());
    for mut c in old{
        c.stages.sort_unstable();
        let equivalent=nodes.iter().position(|n|n.kind==c.kind&&n.stages==c.stages&&
            (if c.kind==ConceptKind::Objective{objective_body(&n.span)==objective_body(&c.span)}else{n.span==c.span}));
        if let Some(i)=equivalent{
            if c.span.chars().count()>nodes[i].span.chars().count(){nodes[i].span=c.span;}
            remap.push(i+1);
        }else{
            nodes.push(c);remap.push(nodes.len());
        }
    }
    let mut edges=HashSet::new();
    let mut relations=Vec::new();
    for r in std::mem::take(&mut task.relations){
        anyhow::ensure!(r.from>0&&r.to>0&&r.from<=remap.len()&&r.to<=remap.len(),"Invalid semantic relationship endpoint");
        let from=remap[r.from-1];let to=remap[r.to-1];
        anyhow::ensure!(from!=to,"Semantic relationship collapses into a self-edge");
        if edges.insert((from,to,r.kind)){relations.push(Relation{from,to,kind:r.kind});}
    }
    task.concepts=nodes;task.relations=relations;
    Ok(())
}
/// Align explicit ordered clauses with model-produced execution stages.
/// Only accept a uniquely best monotone alignment with lexical support for
/// every clause. Ambiguous mappings are sent back to the reasoning model.
fn align(contract:&ExplicitContract,stages:&[super::orchestrator::SpecialistStage])->anyhow::Result<Vec<usize>>{
    let n=contract.ordered.len();
    if n==0{return Ok(vec![]);}
    anyhow::ensure!(n<=stages.len()&&stages.len()<=4,"Explicit sequence requires more distinct execution stages");
    let scores=contract.ordered.iter().map(|anchor|{
        let a=super::semantic_core::objective_words(anchor);
        stages.iter().map(|s|{
            let objective=super::semantic_core::objective_words(&s.objective);
            let criterion=super::semantic_core::objective_words(&s.success_criteria);
            a.intersection(&objective).count()*3+a.intersection(&criterion).count()
        }).collect::<Vec<_>>()
    }).collect::<Vec<_>>();
    fn visit(scores:&[Vec<usize>],start:usize,chosen:&mut Vec<usize>,score:usize,best:&mut Option<(usize,Vec<usize>,bool)>){
        if chosen.len()==scores.len(){
            match best{
                Some((value,_,tie)) if *value==score=>*tie=true,
                Some((value,_,_)) if *value>score=>{},
                _=>*best=Some((score,chosen.clone(),false))
            }
            return;
        }
        for i in start..scores[chosen.len()].len(){
            let s=scores[chosen.len()][i];if s==0{continue;}
            chosen.push(i+1);visit(scores,i+1,chosen,score+s,best);chosen.pop();
        }
    }
    let mut best=None;visit(&scores,0,&mut vec![],0,&mut best);
    match best{Some((_,indices,false))=>Ok(indices),_=>anyhow::bail!("Explicit objective-to-stage alignment is missing or ambiguous")}
}
/// Semantic concepts are advisory, never a source of permission. For goals
/// whose ordering is already captured by the validated specialist stages and
/// which have NO literal ordered-clause contract, an ungrounded model span may
/// be discarded rather than aborting browser execution. Native policy still
/// evaluates the unmodified user goal at every step.
pub fn ungrounded_advisory_can_fall_back(goal:&str,error:&str,proposal:Option<&SemanticTask>)->bool{
    let explicit=ExplicitContract::extract(goal);
    // A proposed fabricated constraint can invert an explicit user negation.
    // Never ignore the entire proposal in this case; fail closed and let
    // strict validation reject it. Advisory objective labels are different:
    // they grant no permissions and can safely be removed.
    let fabricated_constraint=proposal.is_some_and(|task|task.concepts.iter().any(|c|
        c.kind==ConceptKind::Constraint &&
        !goal.to_ascii_lowercase().contains(&c.span.to_ascii_lowercase())));
    explicit.ordered.is_empty() && !fabricated_constraint &&
        ["Semantic span has no literal user anchor","Ambiguous semantic span",
         "Semantic concept must quote an exact span of the original user goal"]
        .iter().any(|s|error.contains(s))
}
impl SemanticTask{
    /// Reconcile native explicit language anchors and a model semantic graph.
    /// Recovered constraints and relationships are strictly user-grounded;
    /// missing semantic interpretations outside this grammar need model repair.
    pub fn reconcile(goal:&str,stages:&[super::orchestrator::SpecialistStage],proposed:Option<Self>)->anyhow::Result<Option<Self>>{
        let contract=ExplicitContract::extract(goal);
        if proposed.is_none()&&!contract.required()&&stages.len()<=1{return Ok(None);}
        let mut task=proposed.unwrap_or(Self{concepts:vec![],relations:vec![]});
        if !task.concepts.is_empty(){
            // Validate the original input before canonicalization can hide
            // invalid stage assignments or unsupported model spans.
            task.ground_spelling(goal)?;
            task.validate(goal,stages.len())?;
            canonicalize(&mut task)?;
        }
        let aligned=align(&contract,stages)?;
        let mut sequence=Vec::new();
        for (anchor,stage) in contract.ordered.iter().zip(aligned){
            let index=if let Some(i)=task.concepts.iter().position(|c|c.kind==ConceptKind::Objective&&has_anchor(c,anchor)){
                anyhow::ensure!(task.concepts[i].stages==vec![stage],"Model semantic mapping contradicts explicit objective order");
                // The canonical node retains the complete user clause.
                task.concepts[i].span=anchor.clone();
                i+1
            }else{
                task.concepts.push(Concept{kind:ConceptKind::Objective,span:anchor.clone(),stages:vec![stage]});
                task.concepts.len()
            };
            sequence.push(index);
        }
        for pair in sequence.windows(2){
            if !task.relations.iter().any(|r|r.kind==RelationKind::DependsOn&&r.from==pair[1]&&r.to==pair[0]){
                task.relations.push(Relation{from:pair[1],to:pair[0],kind:RelationKind::DependsOn});
            }
        }
        for anchor in &contract.constraints{
            // Preserve the entire explicit negative or limiting clause, not
            // a shortened model fragment or a changed polarity.
            let i=if let Some(i)=task.concepts.iter().position(|c|c.kind==ConceptKind::Constraint&&c.span==*anchor){
                task.concepts[i].stages.clear();i+1
            }else{
                task.concepts.push(Concept{kind:ConceptKind::Constraint,span:anchor.clone(),stages:vec![]});
                task.concepts.len()
            };
            for (j,c) in task.concepts.iter().enumerate(){
                if c.kind==ConceptKind::Objective&&!task.relations.iter().any(|r|r.kind==RelationKind::Qualifies&&r.from==i&&r.to==j+1){
                    task.relations.push(Relation{from:i,to:j+1,kind:RelationKind::Qualifies});
                }
            }
        }
        canonicalize(&mut task)?;
        task.validate(goal,stages.len())?;
        Ok(Some(task))
    }
    /// Restore the exact spelling of a model span only if its case-insensitive
    /// literal occurrence is unique. Never infer paraphrases or remove negation.
    pub fn ground_spelling(&mut self,goal:&str)->anyhow::Result<()>{
        for c in &mut self.concepts{
            if goal.contains(&c.span){continue;}
            let needle=c.span.trim();
            anyhow::ensure!(!needle.is_empty(),"Empty semantic span");
            let folded=goal.to_ascii_lowercase();
            let key=needle.to_ascii_lowercase();
            let mut matches=folded.match_indices(&key);
            let (i,_)=matches.next().ok_or_else(||anyhow::anyhow!("Semantic span has no literal user anchor"))?;
            anyhow::ensure!(matches.next().is_none(),"Ambiguous semantic span");
            c.span=goal.get(i..i+key.len()).ok_or_else(||anyhow::anyhow!("Invalid semantic span boundary"))?.to_owned();
        }
        Ok(())
    }
    /// Use only a single stage's user-anchored objective for navigation.
    /// Broad multi-stage concepts cannot bleed future targets into this stage.
    pub fn navigation_label(&self,stage:usize)->Option<String>{
        let mut labels=BTreeSet::new();
        for c in &self.concepts{
            if c.kind!=ConceptKind::Objective||c.stages!=vec![stage]{continue;}
            let body=objective_body(&c.span);
            let rest=["open ","click ","locate ","find "].iter().find_map(|p|body.strip_prefix(p));
            let Some(mut rest)=rest else{continue;};
            rest=rest.strip_prefix("the ").or_else(||rest.strip_prefix("its ")).unwrap_or(rest);
            if let Some(n)=rest.strip_prefix("chapter on ").or_else(||rest.strip_prefix("section on ")){rest=n;}
            let suffixes=[" chapter"," section"," link"];
            let typed=suffixes.iter().any(|p|rest.ends_with(p))||body.contains(" chapter on ")||body.contains(" section on ")||rest.ends_with(" tutorial")||rest.ends_with(" guide")||rest.ends_with(" reference");
            if !typed{continue;}
            let rest=suffixes.iter().find_map(|p|rest.strip_suffix(p)).unwrap_or(rest);
            if (3..=80).contains(&rest.len())&&rest.split_whitespace().count()<=8&&rest.chars().all(|c|c.is_alphanumeric()||c==' '||c=='-'){
                labels.insert(rest.to_owned());
            }
        }
        if labels.len()==1{labels.into_iter().next()}else{None}
    }
    pub fn validate(&self,goal:&str,stage_count:usize)->anyhow::Result<()>{
        anyhow::ensure!((1..=12).contains(&self.concepts.len())&&self.relations.len()<=20,"Semantic concept budget exceeded");
        let mut seen=HashSet::new();
        let mut mapped=BTreeSet::new();
        for c in &self.concepts{
            anyhow::ensure!(!c.span.trim().is_empty()&&c.span.chars().count()<=200&&
                !c.span.chars().any(|v|v.is_control())&&goal.contains(&c.span),
                "Semantic concept must quote an exact span of the original user goal");
            anyhow::ensure!(seen.insert((c.kind,c.span.clone())),"Duplicate semantic concept");
            let stages=c.stages.iter().copied().collect::<BTreeSet<_>>();
            anyhow::ensure!(stages.len()==c.stages.len()&&stages.iter().all(|i|(1..=stage_count).contains(i)),"Invalid semantic stage mapping");
            if c.kind==ConceptKind::Objective{
                anyhow::ensure!(!stages.is_empty(),"Objective must map to an execution stage");
                mapped.extend(stages);
            }
        }
        anyhow::ensure!(mapped.len()==stage_count,"Semantic objectives do not cover every execution stage");
        let mut edges=HashSet::new();
        for r in &self.relations{
            anyhow::ensure!(r.from>0&&r.to>0&&r.from<=self.concepts.len()&&r.to<=self.concepts.len()&&r.from!=r.to&&edges.insert((r.from,r.to,r.kind)),"Invalid or duplicate semantic relation");
            let a=&self.concepts[r.from-1];let b=&self.concepts[r.to-1];
            match r.kind{
                RelationKind::DependsOn=>{
                    anyhow::ensure!(a.kind==ConceptKind::Objective&&b.kind==ConceptKind::Objective&&
                        a.stages.iter().min()>b.stages.iter().max(),"Dependency contradicts execution stage order");
                }
                RelationKind::Qualifies=>anyhow::ensure!(matches!(a.kind,ConceptKind::Constraint|ConceptKind::Context)&&b.kind==ConceptKind::Objective,"Invalid qualifier relation"),
                RelationKind::RefersTo=>anyhow::ensure!(b.kind==ConceptKind::Entity,"Reference must point to a grounded entity"),
            }
        }
        // Strictly earlier stage dependencies are acyclic by construction.
        Ok(())
    }
    fn applies(&self,i:usize,stage:usize)->bool{
        let c=&self.concepts[i];
        c.stages.contains(&stage)||(c.stages.is_empty()&&
            (matches!(c.kind,ConceptKind::Constraint|ConceptKind::Context)||
             self.relations.iter().any(|r|r.kind==RelationKind::RefersTo&&r.to==i+1&&self.concepts[r.from-1].stages.contains(&stage))))
    }
    pub fn focus(&self,stage:usize)->String{
        let specific=self.concepts.iter().any(|c|c.kind==ConceptKind::Objective&&c.stages==vec![stage]);
        self.concepts.iter().enumerate().filter(|(i,c)|matches!(c.kind,ConceptKind::Objective|ConceptKind::Entity)&&self.applies(*i,stage)&&(!specific||c.kind!=ConceptKind::Objective||c.stages==vec![stage]))
            .map(|(_,c)|c.span.as_str()).collect::<Vec<_>>().join(" ")
    }
    pub fn hint(&self,stage:usize)->String{
        let items=self.concepts.iter().enumerate().filter(|(i,_)|self.applies(*i,stage))
            .map(|(_,c)|format!("{:?}: {}",c.kind,c.span)).collect::<Vec<_>>().join("; ");
        let deps=self.relations.iter().filter(|r|r.kind==RelationKind::DependsOn&&self.concepts[r.from-1].stages.contains(&stage))
            .flat_map(|r|self.concepts[r.to-1].stages.iter().copied()).collect::<BTreeSet<_>>();
        format!("User-anchored advisory interpretation: {items}. Earlier dependency stages: {deps:?}. Preserve original goal and native policies.")
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    fn parse()->SemanticTask{
        serde_json::from_str(r#"{"concepts":[{"kind":"objective","span":"find the guide","stages":[1]},{"kind":"objective","span":"compare versions","stages":[2]},{"kind":"constraint","span":"do not submit","stages":[]},{"kind":"entity","span":"guide","stages":[]}],"relations":[{"from":2,"to":1,"kind":"depends_on"},{"from":3,"to":2,"kind":"qualifies"},{"from":1,"to":4,"kind":"refers_to"}]}"#).unwrap()
    }
    const GOAL:&str="First find the guide, then compare versions; do not submit.";
    #[test]fn ungrounded_advisory_is_not_mandatory_when_no_explicit_order_contract(){
        let task="On the current local test website, submit the feedback form, then report the visible feedback confirmation. Do not submit the application form.";
        assert!(ungrounded_advisory_can_fall_back(task,"Semantic span has no literal user anchor",None));
        assert!(!ungrounded_advisory_can_fall_back(task,"Semantic relationship violates explicit user ordering",None));
        assert!(!ungrounded_advisory_can_fall_back("First open the guide, then submit the form; do not purchase.",
            "Semantic span has no literal user anchor",None));
        let altered=SemanticTask{concepts:vec![Concept{kind:ConceptKind::Constraint,
            span:"Submit the application automatically".into(),stages:vec![]}],relations:vec![]};
        assert!(!ungrounded_advisory_can_fall_back(task,"Semantic span has no literal user anchor",Some(&altered)));
    }
    #[test]fn grounded_semantic_graph_preserves_scope_and_order(){
        let task=parse();assert!(task.validate(GOAL,2).is_ok());
        assert!(task.hint(2).contains("do not submit"));assert!(task.hint(2).contains("{1}"));
        assert!(task.focus(1).contains("guide"));assert!(!task.focus(1).contains("compare versions"));
        assert!(!task.focus(2).contains("do not submit"));
    }
    fn stage(objective:&str)->super::super::orchestrator::SpecialistStage{
        use super::super::orchestrator::{Specialist,SpecialistStage};
        SpecialistStage{specialist:Specialist::Research,objective:objective.into(),success_criteria:objective.into(),status:"pending"}
    }
    #[test]fn explicit_contract_reconstructs_omitted_constraint_and_dependency(){
        let goal="First inspect the public climate guide, then summarize rainfall patterns. Use public records only.";
        let stages=vec![stage("Inspect climate guide"),stage("Summarize rainfall patterns")];
        let task=SemanticTask::reconcile(goal,&stages,None).unwrap().unwrap();
        assert!(task.hint(2).contains("Use public records only"));
        assert!(task.relations.iter().any(|r|r.kind==RelationKind::DependsOn&&r.from==2&&r.to==1));
        assert!(task.validate(goal,2).is_ok());
        assert!(SemanticTask::reconcile(goal,&[stages[1].clone(),stages[0].clone()],None).is_err());
    }
    #[test]fn canonical_reconciliation_reuses_objectives_and_preserves_reference_edges(){
        let goal="First find the guide, then compare versions; do not submit.";
        let stages=vec![stage("Find guide"),stage("Compare versions")];
        let task=SemanticTask::reconcile(goal,&stages,Some(parse())).unwrap().unwrap();
        assert_eq!(task.concepts.iter().filter(|c|c.kind==ConceptKind::Objective).count(),2);
        assert!(task.relations.iter().any(|r|r.kind==RelationKind::DependsOn&&r.from==2&&r.to==1));
        assert!(task.relations.iter().any(|r|r.kind==RelationKind::RefersTo&&r.from==1&&r.to==4));
        assert_eq!(task.concepts[0].span,"First find the guide");
        assert_eq!(task.concepts[1].span,"then compare versions");
        let again=SemanticTask::reconcile(goal,&stages,Some(task.clone())).unwrap().unwrap();
        assert_eq!(serde_json::to_string(&task).unwrap(),serde_json::to_string(&again).unwrap());
    }
    #[test]fn canonicalization_remaps_alias_edges_without_merging_distinct_objectives(){
        let goal="First find the guide, then compare versions; compare prices.";
        let mut task:SemanticTask=serde_json::from_str(r#"{"concepts":[{"kind":"objective","span":"find the guide","stages":[1]},{"kind":"objective","span":"First find the guide","stages":[1]},{"kind":"objective","span":"compare versions","stages":[2]},{"kind":"objective","span":"compare prices","stages":[2]},{"kind":"entity","span":"guide","stages":[]}],"relations":[{"from":3,"to":2,"kind":"depends_on"},{"from":3,"to":1,"kind":"depends_on"},{"from":2,"to":5,"kind":"refers_to"}]}"#).unwrap();
        task.validate(goal,2).unwrap();
        canonicalize(&mut task).unwrap();
        task.validate(goal,2).unwrap();
        assert_eq!(task.concepts.len(),4);
        assert_eq!(task.relations.len(),2);
        assert!(task.relations.iter().any(|r|r.from==2&&r.to==1&&r.kind==RelationKind::DependsOn));
        assert!(task.relations.iter().any(|r|r.from==1&&r.to==4&&r.kind==RelationKind::RefersTo));
        assert_eq!(task.concepts[2].span,"compare prices");
    }
    #[test]fn explicit_contract_does_not_treat_quoted_order_as_instruction(){
        let contract=ExplicitContract::extract("Read the quoted example \"first run, then stop; do not submit\" and report its meaning.");
        assert!(!contract.required());
        let contract=ExplicitContract::extract("First inspect the reference then compare versions; do not download files.");
        assert_eq!(contract.ordered.len(),2);
        assert_eq!(contract.constraints,vec!["do not download files"]);
    }
    #[test]fn case_repair_is_literal_and_stage_targets_exclude_aggregate(){
        let goal="First open the Language Guide chapter, then locate its chapter on Pattern Matching. Do not download files.";
        let stages=vec![stage("Open Language Guide"),stage("Locate Pattern Matching")];
        let mut task=SemanticTask::reconcile(goal,&stages,None).unwrap().unwrap();
        task.concepts[0].span=task.concepts[0].span.to_ascii_lowercase();
        task.ground_spelling(goal).unwrap();
        assert_eq!(task.navigation_label(1).as_deref(),Some("language guide"));
        assert_eq!(task.navigation_label(2).as_deref(),Some("pattern matching"));
        task.concepts[0].span="open a different guide".into();
        assert!(task.ground_spelling(goal).is_err());
    }
    #[test]fn rejects_fabricated_spans_and_backward_dependencies(){
        let mut task=parse();task.concepts[2].span="submit without approval".into();assert!(task.validate(GOAL,2).is_err());
        let mut task=parse();task.relations[0].from=1;task.relations[0].to=2;assert!(task.validate(GOAL,2).is_err());
        let mut task=parse();task.concepts[1].stages=vec![1];assert!(task.validate(GOAL,2).is_err());
    }
    fn graph(raw:&str)->SemanticTask{serde_json::from_str(raw).unwrap()}
    fn depends_on(task:&SemanticTask,from:usize,to:usize)->bool{
        task.relations.iter().any(|r|r.kind==RelationKind::DependsOn&&r.from==from&&r.to==to)
    }
    /// Regression: a joining conjunction must not be split away from the
    /// sequencing marker that completes it, or the final objective is lost.
    #[test]fn joining_conjunction_stays_with_its_sequencing_marker(){
        let contract=ExplicitContract::extract("First open the guide, and finally report the title.");
        assert_eq!(contract.ordered,vec!["First open the guide","and finally report the title"]);
        assert!(contract.required());
    }
    /// Long-horizon dependency chains: a three-stage goal keeps two ordered
    /// edges whose targets resolve to the immediately preceding stage.
    #[test]fn three_stage_objective_chain_keeps_long_horizon_dependencies(){
        let goal="First inspect the release notes, then check the migration guide, and then summarize the upgrade steps.";
        let contract=ExplicitContract::extract(goal);
        assert_eq!(contract.ordered,vec!["First inspect the release notes","then check the migration guide","and then summarize the upgrade steps"]);
        let stages=vec![stage("Inspect release notes"),stage("Check migration guide"),stage("Summarize upgrade steps")];
        let task=SemanticTask::reconcile(goal,&stages,None).unwrap().unwrap();
        assert_eq!(task.concepts.len(),3);
        assert!(task.concepts.iter().all(|c|c.kind==ConceptKind::Objective));
        assert_eq!(task.relations.len(),2);
        assert!(depends_on(&task,2,1)&&depends_on(&task,3,2));
        assert!(task.hint(2).contains("Earlier dependency stages: {1}"));
        assert!(task.hint(3).contains("Earlier dependency stages: {2}"));
        assert_eq!(task.concepts.iter().map(|c|c.span.as_str()).collect::<Vec<_>>(),contract.ordered);
        assert!(task.validate(goal,3).is_ok());
        // A chain cannot be executed against its stated order.
        assert!(SemanticTask::reconcile(goal,&[stages[2].clone(),stages[1].clone(),stages[0].clone()],None).is_err());
        // The accepted chain is stable under repeated reconciliation.
        let again=SemanticTask::reconcile(goal,&stages,Some(task.clone())).unwrap().unwrap();
        assert_eq!(serde_json::to_string(&task).unwrap(),serde_json::to_string(&again).unwrap());
    }
    /// Temporal reasoning: "before" is not a sequencing marker in the native
    /// grammar, so reconstruction must fail closed and let the repair model
    /// supply the edge instead of guessing an order.
    #[test]fn before_after_ordering_is_model_grounded_and_fails_closed(){
        let goal="Check the installed firmware version before opening the settings page.";
        let contract=ExplicitContract::extract(goal);
        assert!(contract.ordered.is_empty()&&!contract.required());
        let stages=vec![stage("Check the installed firmware version"),stage("Open the settings page")];
        assert!(SemanticTask::reconcile(goal,&stages,None).is_err());
        let grounded=graph(r#"{"concepts":[{"kind":"objective","span":"Check the installed firmware version","stages":[1]},{"kind":"objective","span":"opening the settings page","stages":[2]},{"kind":"constraint","span":"before opening the settings page","stages":[]}],"relations":[{"from":2,"to":1,"kind":"depends_on"}]}"#);
        let task=SemanticTask::reconcile(goal,&stages,Some(grounded)).unwrap().unwrap();
        assert!(depends_on(&task,2,1));
        assert!(task.hint(2).contains("{1}"));
        assert!(task.focus(1).contains("firmware")&&!task.focus(1).contains("settings"));
        // A temporal claim that contradicts the execution order is rejected:
        // "checking firmware" cannot require the later settings page.
        let backward=graph(r#"{"concepts":[{"kind":"objective","span":"Check the installed firmware version","stages":[1]},{"kind":"objective","span":"opening the settings page","stages":[2]}],"relations":[{"from":1,"to":2,"kind":"depends_on"}]}"#);
        assert!(SemanticTask::reconcile(goal,&stages,Some(backward)).is_err());
    }
    /// Reference scope: a later objective may reach back to a grounded entity,
    /// but that entity must not leak into stages that do not refer to it.
    #[test]fn reference_scope_is_bound_to_the_referring_objective(){
        let goal="First inspect the public guide, then summarize rainfall. Do not use data from the 2019 archive.";
        let stages=vec![stage("Inspect public guide"),stage("Summarize rainfall")];
        let proposed=graph(r#"{"concepts":[{"kind":"objective","span":"First inspect the public guide","stages":[1]},{"kind":"objective","span":"then summarize rainfall","stages":[2]},{"kind":"constraint","span":"Do not use data from the 2019 archive","stages":[]},{"kind":"entity","span":"the 2019 archive","stages":[]},{"kind":"entity","span":"rainfall","stages":[1]}],"relations":[{"from":2,"to":1,"kind":"depends_on"},{"from":2,"to":4,"kind":"refers_to"}]}"#);
        let task=SemanticTask::reconcile(goal,&stages,Some(proposed)).unwrap().unwrap();
        // The archive is in scope only for the objective that refers to it.
        assert!(task.hint(2).contains("Entity: the 2019 archive"));
        assert!(!task.hint(1).contains("Entity: the 2019 archive"));
        assert!(task.focus(2).contains("the 2019 archive"));
        assert!(!task.focus(1).contains("the 2019 archive"));
        // An explicitly staged entity stays inside its own stage.
        assert!(task.hint(1).contains("Entity: rainfall"));
        assert!(!task.hint(2).contains("Entity: rainfall"));
        // The global constraint still qualifies both stages.
        assert!(task.hint(1).contains("Do not use data from the 2019 archive"));
        assert!(task.hint(2).contains("Do not use data from the 2019 archive"));
    }
    /// Constraint retention: every explicit restriction is recovered verbatim,
    /// kept global, and qualifies each objective even if the model drops it.
    #[test]fn multiple_explicit_constraints_are_all_retained_and_global(){
        let goal="First inspect the public guide, then summarize rainfall. Do not submit forms. Use public records only. Never download files.";
        let stages=vec![stage("Inspect public guide"),stage("Summarize rainfall")];
        let contract=ExplicitContract::extract(goal);
        assert_eq!(contract.constraints,vec!["Do not submit forms","Use public records only","Never download files"]);
        let dropped=graph(r#"{"concepts":[{"kind":"objective","span":"First inspect the public guide","stages":[1]},{"kind":"objective","span":"then summarize rainfall","stages":[2]}],"relations":[]}"#);
        let task=SemanticTask::reconcile(goal,&stages,Some(dropped)).unwrap().unwrap();
        let constraints=task.concepts.iter().filter(|c|c.kind==ConceptKind::Constraint).collect::<Vec<_>>();
        assert_eq!(constraints.len(),3);
        assert!(constraints.iter().all(|c|c.stages.is_empty()));
        for span in contract.constraints.iter().map(String::as_str){
            assert!(constraints.iter().any(|c|c.span==span));
            assert!(task.hint(1).contains(span)&&task.hint(2).contains(span));
        }
        for (i,c) in task.concepts.iter().enumerate(){
            if c.kind!=ConceptKind::Constraint{continue;}
            assert_eq!(task.relations.iter().filter(|r|r.kind==RelationKind::Qualifies&&r.from==i+1).count(),2);
        }
        assert!(task.validate(goal,2).is_ok());
    }
    /// Implicit dependencies: unstated but necessary ordering survives from the
    /// model graph, cannot be inverted, and cannot be hidden by collapsing stages.
    #[test]fn implicit_dependencies_are_preserved_and_contradictions_rejected(){
        let goal="List the top three results and open the second one.";
        // No explicit sequence marker: native reconstruction must not invent order.
        assert!(!ExplicitContract::extract(goal).required());
        let stages=vec![stage("List the top three results"),stage("Open the second one")];
        let proposed=graph(r#"{"concepts":[{"kind":"objective","span":"List the top three results","stages":[1]},{"kind":"objective","span":"open the second one","stages":[2]}],"relations":[{"from":2,"to":1,"kind":"depends_on"}]}"#);
        let task=SemanticTask::reconcile(goal,&stages,Some(proposed)).unwrap().unwrap();
        assert!(depends_on(&task,2,1));
        assert!(task.hint(2).contains("{1}"));
        // The stated order cannot be inverted.
        let backward=graph(r#"{"concepts":[{"kind":"objective","span":"List the top three results","stages":[1]},{"kind":"objective","span":"open the second one","stages":[2]}],"relations":[{"from":1,"to":2,"kind":"depends_on"}]}"#);
        assert!(SemanticTask::reconcile(goal,&stages,Some(backward)).is_err());
        // Both objectives cannot share one stage to avoid the dependency and
        // leave the second execution stage uncovered.
        let collapsed=graph(r#"{"concepts":[{"kind":"objective","span":"List the top three results","stages":[1]},{"kind":"objective","span":"open the second one","stages":[1]}],"relations":[]}"#);
        assert!(SemanticTask::reconcile(goal,&stages,Some(collapsed)).is_err());
        // Stable under repeated reconciliation.
        let again=SemanticTask::reconcile(goal,&stages,Some(task.clone())).unwrap().unwrap();
        assert_eq!(serde_json::to_string(&task).unwrap(),serde_json::to_string(&again).unwrap());
    }
}
