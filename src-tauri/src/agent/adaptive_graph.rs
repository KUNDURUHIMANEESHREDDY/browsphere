//! Shared bounded DAG runtime. Native callers supply receipts only after their
//! existing policy and evidence checks. The graph never dispatches actions.
use anyhow::{ensure,Context,Result};
use serde::{Serialize,Deserialize};
use std::collections::BTreeSet;
const MAX_NODES:usize=32;
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Kind{Browser,Verification,Tool,Comparison,Review,Observation,Handoff,Recovery}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum State{Pending,Active,Completed,Failed,Blocked,Cancelled}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Receipt{StageGate,FinalReport,ToolCheck,ComparisonCheck,QuoteCheck,NativeObservation,ObservedProgress}
impl Kind{
 fn accepts(self,r:Receipt)->bool{
  matches!((self,r),(Self::Browser,Receipt::StageGate)|(Self::Verification,Receipt::FinalReport)|
   (Self::Tool,Receipt::ToolCheck)|(Self::Comparison,Receipt::ComparisonCheck)|
   (Self::Review,Receipt::QuoteCheck)|(Self::Observation,Receipt::NativeObservation)|
   (Self::Recovery,Receipt::ObservedProgress))
 }
}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct Node{
 pub id:usize,pub kind:Kind,pub depends_on:Vec<usize>,pub state:State,
 pub receipt:Option<Receipt>,pub attempts:u8,
 // The assigned objective and criterion are local run metadata. Do not send
 // task details or possibly private user values in the public graph event.
 #[serde(skip)] pub objective:String,
 #[serde(skip)] pub criterion:String,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct WorkflowGraph{pub nodes:Vec<Node>,pub revision:u32}
impl WorkflowGraph{
 pub fn new(spec:Vec<(Kind,Vec<usize>)>)->Result<Self>{
  ensure!(!spec.is_empty()&&spec.len()<=MAX_NODES,"Invalid graph size");
  let count=spec.len();
  let nodes=spec.into_iter().enumerate().map(|(id,(kind,depends_on))|Node{id,kind,depends_on,state:State::Pending,receipt:None,attempts:0,objective:String::new(),criterion:String::new()}).collect::<Vec<_>>();
  for n in &nodes{
   ensure!(n.depends_on.iter().all(|d|*d<count&&*d!=n.id)&&n.depends_on.iter().copied().collect::<BTreeSet<_>>().len()==n.depends_on.len(),"Invalid graph dependencies");
  }
  let mut seen=BTreeSet::new();
  while seen.len()<count{
   let next=nodes.iter().find(|n|!seen.contains(&n.id)&&n.depends_on.iter().all(|d|seen.contains(d))).context("Workflow graph cycle")?;
   seen.insert(next.id);
  }
  Ok(Self{nodes,revision:0})
 }
 pub fn chain(kinds:&[Kind])->Result<Self>{
  Self::new(kinds.iter().enumerate().map(|(i,k)|(*k,if i==0{vec![]}else{vec![i-1]})).collect())
 }
 pub fn describe(&mut self,id:usize,objective:&str,criterion:&str)->Result<()>{
  let n=self.nodes.get_mut(id).context("Unknown objective")?;
  ensure!(matches!(n.state,State::Pending|State::Active),"Completed objective is immutable");
  n.objective=objective.chars().filter(|c|!c.is_control()).take(400).collect();
  n.criterion=criterion.chars().filter(|c|!c.is_control()).take(400).collect();
  Ok(())
 }
 /// Validate a persisted or amended graph before any scheduling.
 pub fn validate(&self)->Result<()>{
  let spec=self.nodes.iter().map(|n|(n.kind,n.depends_on.clone())).collect();
  Self::new(spec)?;
  for (i,n) in self.nodes.iter().enumerate(){
   ensure!(n.id==i,"Invalid node identity");
   ensure!(match (n.state,n.receipt){
    (State::Completed,Some(r))=>n.kind.accepts(r)&&n.attempts>0,
    (State::Completed,None)=>false,
    (_,Some(_))=>false,
    (State::Active,None)=>n.attempts>0,
    _=>true
   },"Invalid graph receipt");
   if matches!(n.state,State::Active|State::Completed){
    ensure!(n.depends_on.iter().all(|d|self.nodes[*d].state==State::Completed),"Unmet dependency");
   }
  }
  Ok(())
 }
 /// Transactional, bounded amendment of a stalled read-only browser stage.
 /// New recovery nodes depend on the original completed prerequisites, or
 /// other new nodes; old completed nodes and all original edges survive.
 /// The caller still uses the native observation and action policy.
 pub fn amend_recovery(&mut self,parent:usize,steps:&[(Kind,Vec<usize>)])->Result<Vec<usize>>{
  self.amend_recovery_inner(parent,steps,false)
 }
 fn amend_recovery_inner(&mut self,parent:usize,steps:&[(Kind,Vec<usize>)],state_scoped:bool)->Result<Vec<usize>>{
  let n=self.nodes.get(parent).context("Unknown recovery parent")?;
  ensure!(n.kind==Kind::Browser&&n.state==State::Active&&(state_scoped||n.attempts<=3),"Recovery is not eligible");
  ensure!(!steps.is_empty()&&steps.len()<=4&&self.nodes.len()+steps.len()<=MAX_NODES,"Recovery graph budget exhausted");
  ensure!(steps.iter().all(|(k,_)|matches!(k,Kind::Observation|Kind::Recovery)),"Recovery cannot add tool permissions or skip verification");
  let local=Self::new(steps.to_vec())?;
  let mut candidate=self.clone();
  let base=candidate.nodes.len();
  let inherited=n.depends_on.clone();
  let leaves:Vec<_>=(0..steps.len()).filter(|i|!local.nodes.iter().any(|n|n.depends_on.contains(i))).collect();
  ensure!(!leaves.is_empty(),"Missing recovery terminal");
  for (offset,node) in local.nodes.iter().enumerate(){
   let id=base+offset;
   let mut deps=inherited.clone();
   deps.extend(node.depends_on.iter().map(|d|base+d));
   candidate.nodes.push(Node{id,kind:node.kind,depends_on:deps,state:State::Pending,receipt:None,attempts:0,objective:String::new(),criterion:String::new()});
   let (objective,criterion)=if node.kind==Kind::Observation{("Re-observe stalled page","Fresh native page observation")}else{("Recover stalled objective","Meaningful observed progress or existing stage gate")};
   candidate.describe(id,objective,criterion)?;
  }
  candidate.nodes[parent].depends_on=inherited;
  candidate.nodes[parent].depends_on.extend(leaves.iter().map(|d|base+d));
  candidate.nodes[parent].state=State::Pending;
  candidate.revision+=1;
  candidate.validate()?;
  *self=candidate;
  Ok((base..base+steps.len()).collect())
 }
 /// Insert a bounded, observation-driven recovery objective without replacing
 /// the user's original objective or weakening its completion criteria.
 pub fn replan_observed(&mut self,parent:usize,strategy:super::page_understanding::RecoveryStrategy)->Result<(usize,usize)>{
  let mut candidate=self.clone();
  let ids=candidate.amend_recovery_inner(parent,&[(Kind::Observation,vec![]),(Kind::Recovery,vec![0])],true)?;
  candidate.describe(ids[1],strategy.objective(),"Fresh native evidence of meaningful progress; original goal and policy unchanged")?;
  candidate.start(ids[0])?;
  candidate.validate()?;
  *self=candidate;
  Ok((ids[0],ids[1]))
 }
 /// Insert a validated recovery plan. The plan may name an observed control
 /// or a model-proposed tactic outside the fixed strategy taxonomy; callers
 /// must run `RecoveryPlan::validate` first.
 pub fn replan_with(&mut self,parent:usize,plan:&super::replan::RecoveryPlan)->Result<(usize,usize)>{
  let mut candidate=self.clone();
  let ids=candidate.amend_recovery_inner(parent,&[(Kind::Observation,vec![]),(Kind::Recovery,vec![0])],true)?;
  candidate.describe(ids[1],plan.objective(),plan.criterion())?;
  candidate.start(ids[0])?;
  candidate.validate()?;
  *self=candidate;
  Ok((ids[0],ids[1]))
 }
 /// Insert an authorized tool prerequisite while a browser specialist is active.
 /// The caller is responsible for authorization and evidence verification.
 pub fn insert_tool_prerequisite(&mut self,parent:usize)->Result<usize>{
  self.validate()?;
  let n=self.nodes.get(parent).context("Unknown tool insertion parent")?;
  ensure!(matches!(n.kind,Kind::Browser|Kind::Verification)&&n.state==State::Active&&n.attempts<=4,"Tool insertion requires an active browser stage");
  ensure!(self.nodes.len()<MAX_NODES,"Dynamic graph budget exhausted");
  let mut candidate=self.clone();
  let id=candidate.nodes.len();
  let deps=n.depends_on.clone();
  candidate.nodes.push(Node{id,kind:Kind::Tool,depends_on:deps,state:State::Pending,receipt:None,attempts:0,objective:String::new(),criterion:String::new()});
  candidate.nodes[parent].depends_on.push(id);
  candidate.nodes[parent].state=State::Pending;
  candidate.describe(id,"Authorized dynamic MCP lookup","Fresh public trigger and expected tool result match")?;
  candidate.revision+=1;
  candidate.validate()?;
  candidate.start(id)?;
  *self=candidate;
  Ok(id)
 }
 pub fn renew_verification(&mut self)->Result<usize>{
  self.validate()?;
  let ids:Vec<_>=self.nodes.iter().filter(|n|n.kind==Kind::Verification).map(|n|n.id).collect();
  ensure!(ids.len()==1,"Expected one final verification node");
  let id=ids[0];
  ensure!(!self.nodes.iter().any(|n|n.depends_on.contains(&id))&&self.nodes.iter().all(|n|n.id==id||n.state==State::Completed),"Unresolved prerequisites");
  let n=&mut self.nodes[id];
  ensure!(matches!(n.state,State::Active|State::Completed),"Verification is not resumable");
  if n.state==State::Completed{n.state=State::Pending;n.receipt=None;self.revision+=1;self.start(id)?;}
  Ok(id)
 }
 /// Atomic edge rewrite of pending nodes. Preserve reachability of every
 /// original dependency, including completed prerequisites, and the final
 /// verification node. This permits refactoring a DAG without deleting work.
 pub fn revise_pending(&mut self,changes:&[(usize,Vec<usize>)])->Result<()>{
  self.validate()?;
  ensure!(!changes.is_empty()&&changes.len()<=8,"Invalid revision size");
  let mut candidate=self.clone();let mut touched=BTreeSet::new();
  for (id,deps) in changes{
   ensure!(touched.insert(*id),"Repeated revision node");
   let n=candidate.nodes.get_mut(*id).context("Unknown revision node")?;
   ensure!(n.state==State::Pending,"Only pending dependencies can change");
   n.depends_on=deps.clone();
  }
  candidate.validate()?;
  for n in &self.nodes{
   let mut seen=BTreeSet::new();let mut stack=candidate.nodes[n.id].depends_on.clone();
   while let Some(id)=stack.pop(){if seen.insert(id){stack.extend(candidate.nodes[id].depends_on.iter().copied());}}
   ensure!(n.depends_on.iter().all(|d|seen.contains(d)),"Revision removed a required prerequisite");
  }
  candidate.revision+=1;*self=candidate;Ok(())
 }
 pub fn ready(&self)->Vec<usize>{
  self.nodes.iter().filter(|n|n.state==State::Pending&&n.depends_on.iter().all(|d|self.nodes[*d].state==State::Completed)).map(|n|n.id).collect()
 }
 pub fn start(&mut self,id:usize)->Result<()>{
  ensure!(self.ready().contains(&id),"Node is not runnable");
  self.nodes[id].state=State::Active;
  self.nodes[id].attempts=self.nodes[id].attempts.saturating_add(1);
  self.revision+=1;Ok(())
 }
 pub fn complete(&mut self,id:usize,receipt:Receipt)->Result<()>{
  let n=self.nodes.get_mut(id).context("Unknown node")?;
  ensure!(n.state==State::Active&&n.kind.accepts(receipt),"Node lacks matching evidence receipt");
  n.state=State::Completed;n.receipt=Some(receipt);self.revision+=1;Ok(())
 }
 pub fn completed(&self)->usize{self.nodes.iter().filter(|n|n.state==State::Completed).count()}
 pub fn is_complete(&self)->bool{self.completed()==self.nodes.len()}
 pub fn fail(&mut self,id:usize){
  if self.nodes.get(id).is_some_and(|n|n.state==State::Active){
   self.nodes[id].state=State::Failed;
   loop{
    let blocked:Vec<_>=self.nodes.iter().filter(|n|n.state==State::Pending&&n.depends_on.iter().any(|d|matches!(self.nodes[*d].state,State::Failed|State::Blocked|State::Cancelled))).map(|n|n.id).collect();
    if blocked.is_empty(){break}
    for i in blocked{self.nodes[i].state=State::Blocked;}
   }
   self.revision+=1;
  }
 }
 pub fn stop(&mut self,cancelled:bool){
  for n in &mut self.nodes{
   if matches!(n.state,State::Active|State::Pending){n.state=if cancelled{State::Cancelled}else{State::Blocked};}
  }
  self.revision+=1;
 }
 /// Expand a terminal authorized handoff into browser stages, preserving all
 /// previously completed tool evidence and dependencies. No new tool scope.
 pub fn expand_handoff(&mut self,id:usize,kinds:&[Kind])->Result<Vec<usize>>{
  ensure!(!kinds.is_empty()&&self.nodes.len()+kinds.len()-1<=MAX_NODES,"Handoff exceeds graph budget");
  ensure!(self.nodes.get(id).is_some_and(|n|n.kind==Kind::Handoff&&n.state==State::Active)&&
   !self.nodes.iter().any(|n|n.depends_on.contains(&id)),"Handoff must be active and terminal");
  ensure!(kinds.last()==Some(&Kind::Verification)&&kinds.iter().all(|k|matches!(k,Kind::Browser|Kind::Verification)),"Handoff requires browser execution and final verification");
  self.nodes[id].kind=kinds[0];let mut ids=vec![id];
  for &kind in &kinds[1..]{
   let next=self.nodes.len();
   self.nodes.push(Node{id:next,kind,depends_on:vec![*ids.last().unwrap()],state:State::Pending,receipt:None,attempts:0,objective:String::new(),criterion:String::new()});
   ids.push(next);
  }
  self.revision+=1;Ok(ids)
 }
 /// Reopen verification behind fresh corrective research without rewriting
 /// any completed receipt.
 pub fn insert_verification_research(&mut self,id:usize)->Result<usize>{
  ensure!(self.nodes.len()<MAX_NODES,"Research recovery budget exhausted");
  let n=self.nodes.get(id).context("Unknown verifier")?;
  ensure!(n.kind==Kind::Verification&&n.state==State::Active&&n.attempts<=2,"Verifier cannot be reopened");
  let deps=n.depends_on.clone();let next=self.nodes.len();
  self.nodes.push(Node{id:next,kind:Kind::Browser,depends_on:deps,state:State::Pending,receipt:None,attempts:0,objective:String::new(),criterion:String::new()});
  self.nodes[id].depends_on.push(next);self.nodes[id].state=State::Pending;
  self.describe(next,"Correct incomplete source research","Collect additional relevant evidence and rerun independent verification")?;
  self.revision+=1;self.start(next)?;Ok(next)
 }
 /// Suspend an active browser stage behind one bounded recovery prerequisite.
 /// Completed predecessors are immutable; recovery inherits dependencies.
 pub fn insert_recovery(&mut self,id:usize)->Result<usize>{
  let next=self.amend_recovery(id,&[(Kind::Recovery,vec![])])?[0];
  self.start(next)?;Ok(next)
 }
}
#[cfg(test)]
mod tests{
 use super::*;
 #[test]fn cycles_and_unready_dispatch_are_rejected(){
  assert!(WorkflowGraph::new(vec![(Kind::Tool,vec![1]),(Kind::Tool,vec![0])]).is_err());
  let mut g=WorkflowGraph::new(vec![(Kind::Tool,vec![]),(Kind::Tool,vec![]),(Kind::Comparison,vec![0,1])]).unwrap();
  assert_eq!(g.ready(),vec![0,1]);assert!(g.start(2).is_err());
  g.describe(0,"Read first source","Exact expected value").unwrap();
  g.start(0).unwrap();g.start(1).unwrap();
  assert!(g.complete(0,Receipt::FinalReport).is_err());
  g.complete(0,Receipt::ToolCheck).unwrap();assert!(g.describe(0,"Changed","Changed").is_err());assert!(g.ready().is_empty());
  g.complete(1,Receipt::ToolCheck).unwrap();g.start(2).unwrap();
  g.complete(2,Receipt::ComparisonCheck).unwrap();assert!(g.is_complete());
 }
 #[test]fn dynamic_tool_insertion_preserves_receipts_and_blocks_parent_until_verified(){
  let mut g=WorkflowGraph::chain(&[Kind::Tool,Kind::Browser,Kind::Verification]).unwrap();
  g.start(0).unwrap();g.complete(0,Receipt::ToolCheck).unwrap();g.start(1).unwrap();
  let t=g.insert_tool_prerequisite(1).unwrap();
  assert_eq!(g.nodes[0].receipt,Some(Receipt::ToolCheck));
  assert_eq!(g.nodes[1].state,State::Pending);assert!(g.start(1).is_err());
  assert!(g.complete(t,Receipt::StageGate).is_err());
  g.complete(t,Receipt::ToolCheck).unwrap();g.start(1).unwrap();
  let t2=g.insert_tool_prerequisite(1).unwrap();g.fail(t2);
  assert_eq!(g.nodes[1].state,State::Blocked);assert_eq!(g.nodes[2].state,State::Blocked);
  assert!(g.ready().is_empty());g.validate().unwrap();
 }
 #[test]fn dynamic_insertion_rejects_wrong_parent_and_exhausted_budget(){
  let mut g=WorkflowGraph::chain(&[Kind::Tool,Kind::Browser]).unwrap();
  g.start(0).unwrap();assert!(g.insert_tool_prerequisite(0).is_err());assert!(g.insert_tool_prerequisite(1).is_err());
  let mut full=WorkflowGraph::chain(&vec![Kind::Browser;MAX_NODES]).unwrap();
  full.start(0).unwrap();assert!(full.insert_tool_prerequisite(0).is_err());
  assert_eq!(full.nodes.len(),MAX_NODES);assert_eq!(full.nodes[0].state,State::Active);
 }
 #[test]fn failed_prerequisite_blocks_transitive_dependents(){
  let mut g=WorkflowGraph::chain(&[Kind::Tool,Kind::Browser,Kind::Verification]).unwrap();
  g.start(0).unwrap();g.fail(0);
  assert_eq!(g.nodes[1].state,State::Blocked);assert_eq!(g.nodes[2].state,State::Blocked);
  assert!(g.ready().is_empty());assert!(!g.is_complete());
 }
 #[test]fn hybrid_handoff_and_dynamic_recovery_preserve_receipts(){
  let mut g=WorkflowGraph::chain(&[Kind::Tool,Kind::Handoff]).unwrap();
  g.start(0).unwrap();g.complete(0,Receipt::ToolCheck).unwrap();g.start(1).unwrap();
  let ids=g.expand_handoff(1,&[Kind::Browser,Kind::Verification]).unwrap();
  let r=g.insert_recovery(ids[0]).unwrap();assert!(g.ready().is_empty());
  assert!(g.complete(ids[0],Receipt::StageGate).is_err());
  assert!(g.complete(r,Receipt::StageGate).is_err());
  g.complete(r,Receipt::ObservedProgress).unwrap();g.start(ids[0]).unwrap();
  g.complete(ids[0],Receipt::StageGate).unwrap();g.start(ids[1]).unwrap();
  assert!(g.complete(ids[1],Receipt::StageGate).is_err());
  g.complete(ids[1],Receipt::FinalReport).unwrap();
  assert!(g.is_complete());assert_eq!(g.nodes[0].receipt,Some(Receipt::ToolCheck));
 }
 #[test]fn atomic_amendment_preserves_prior_edges_and_rejects_cycles_and_new_tools(){
  let mut g=WorkflowGraph::chain(&[Kind::Tool,Kind::Browser,Kind::Verification]).unwrap();
  g.start(0).unwrap();g.complete(0,Receipt::ToolCheck).unwrap();g.start(1).unwrap();
  let before=serde_json::to_string(&g).unwrap();
  assert!(g.amend_recovery(1,&[(Kind::Tool,vec![])]).is_err());
  assert!(g.amend_recovery(1,&[(Kind::Observation,vec![1]),(Kind::Recovery,vec![0])]).is_err());
  assert_eq!(serde_json::to_string(&g).unwrap(),before);
  let ids=g.amend_recovery(1,&[(Kind::Observation,vec![]),(Kind::Recovery,vec![0])]).unwrap();
  assert_eq!(g.ready(),vec![ids[0]]);assert!(g.start(1).is_err());
  g.start(ids[0]).unwrap();g.complete(ids[0],Receipt::NativeObservation).unwrap();
  g.start(ids[1]).unwrap();g.complete(ids[1],Receipt::ObservedProgress).unwrap();
  g.start(1).unwrap();assert_eq!(g.nodes[0].receipt,Some(Receipt::ToolCheck));g.validate().unwrap();
 }
 #[test]fn observed_replan_preserves_completed_stages_and_original_criterion(){
  use super::super::page_understanding::RecoveryStrategy;
  let mut g=WorkflowGraph::chain(&[Kind::Browser,Kind::Browser,Kind::Verification]).unwrap();
  g.describe(0,"First source","Read first source").unwrap();
  g.describe(1,"Second source","Read second source").unwrap();
  g.start(0).unwrap();g.complete(0,Receipt::StageGate).unwrap();g.start(1).unwrap();
  let (observation,recovery)=g.replan_observed(1,RecoveryStrategy::OpenResult).unwrap();
  assert_eq!(g.nodes[0].receipt,Some(Receipt::StageGate));
  assert_eq!(g.nodes[0].attempts,1);
  assert_eq!(g.nodes[1].criterion,"Read second source");
  assert_eq!(g.nodes[1].objective,"Second source");
  assert_eq!(g.nodes[recovery].objective,RecoveryStrategy::OpenResult.objective());
  assert!(g.start(1).is_err());
  assert!(g.complete(recovery,Receipt::ObservedProgress).is_err());
  g.complete(observation,Receipt::NativeObservation).unwrap();g.start(recovery).unwrap();
  assert!(g.complete(recovery,Receipt::FinalReport).is_err());
  g.complete(recovery,Receipt::ObservedProgress).unwrap();g.start(1).unwrap();
  g.complete(1,Receipt::StageGate).unwrap();g.start(2).unwrap();
  g.complete(2,Receipt::FinalReport).unwrap();assert!(g.is_complete());
 }
 #[test]fn custom_replan_uses_the_proposed_objective_and_criterion(){
  use super::super::replan::{Basis,RecoveryPlan};
  let mut g=WorkflowGraph::chain(&[Kind::Browser,Kind::Verification]).unwrap();
  g.describe(0,"Second source","Read second source").unwrap();
  g.start(0).unwrap();
  let plan=RecoveryPlan{
   objective:"Compare the two observed price panels before deciding".into(),
   criterion:"Both panels were re-observed in the latest native snapshot".into(),
   basis:Basis::Model,
  };
  let (observation,recovery)=g.replan_with(0,&plan).unwrap();
  assert_eq!(g.nodes[recovery].objective,plan.objective);
  assert_eq!(g.nodes[recovery].criterion,plan.criterion);
  assert_eq!(g.nodes[0].criterion,"Read second source");
  assert!(g.start(0).is_err());
  g.complete(observation,Receipt::NativeObservation).unwrap();
  g.start(recovery).unwrap();g.complete(recovery,Receipt::ObservedProgress).unwrap();
  g.start(0).unwrap();g.complete(0,Receipt::StageGate).unwrap();
  g.start(1).unwrap();g.complete(1,Receipt::FinalReport).unwrap();
  assert!(g.is_complete());
 }
 #[test]fn pending_graph_revision_preserves_transitive_dependencies(){
  let mut g=WorkflowGraph::new(vec![(Kind::Observation,vec![]),(Kind::Recovery,vec![0]),(Kind::Browser,vec![0,1]),(Kind::Verification,vec![2])]).unwrap();
  g.start(0).unwrap();g.complete(0,Receipt::NativeObservation).unwrap();
  g.revise_pending(&[(2,vec![1])]).unwrap();
  assert!(g.revise_pending(&[(2,vec![0])]).is_err());
  assert!(g.revise_pending(&[(0,vec![3])]).is_err());
  assert!(g.revise_pending(&[(1,vec![3])]).is_err());
  assert_eq!(g.nodes[2].depends_on,vec![1]);g.validate().unwrap();
 }
 #[test]fn renewal_does_not_replay_completed_objectives(){
  let mut g=WorkflowGraph::chain(&[Kind::Browser,Kind::Verification]).unwrap();g.start(0).unwrap();g.complete(0,Receipt::StageGate).unwrap();g.start(1).unwrap();g.complete(1,Receipt::FinalReport).unwrap();
  assert_eq!(g.renew_verification().unwrap(),1);
  assert_eq!(g.nodes[0].state,State::Completed);assert_eq!(g.nodes[0].attempts,1);
  assert_eq!(g.nodes[1].state,State::Active);assert_eq!(g.nodes[1].receipt,None);
 }
 #[test]fn cancellation_never_reopens_completed_or_pending_nodes(){
  let mut g=WorkflowGraph::chain(&[Kind::Browser,Kind::Verification]).unwrap();
  g.start(0).unwrap();g.stop(true);assert!(g.ready().is_empty());assert!(!g.is_complete());
 }
}
