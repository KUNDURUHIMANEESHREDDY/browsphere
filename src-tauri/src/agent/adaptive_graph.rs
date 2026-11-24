//! Shared bounded DAG runtime. Native callers supply receipts only after their
//! existing policy and evidence checks. The graph never dispatches actions.
use anyhow::{ensure,Context,Result};
use serde::Serialize;
use std::collections::BTreeSet;
const MAX_NODES:usize=32;
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize)]
#[serde(rename_all="snake_case")]
pub enum Kind{Browser,Verification,Tool,Comparison,Review,Observation,Handoff,Recovery}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize)]
#[serde(rename_all="snake_case")]
pub enum State{Pending,Active,Completed,Failed,Blocked,Cancelled}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize)]
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
#[derive(Clone,Debug,Serialize)]
pub struct Node{
 pub id:usize,pub kind:Kind,pub depends_on:Vec<usize>,pub state:State,
 pub receipt:Option<Receipt>,pub attempts:u8,
 // The assigned objective and criterion are local run metadata. Do not send
 // task details or possibly private user values in the public graph event.
 #[serde(skip)] pub objective:String,
 #[serde(skip)] pub criterion:String,
}
#[derive(Clone,Debug,Serialize)]
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
  self.nodes[id].depends_on=vec![next];self.nodes[id].state=State::Pending;
  self.describe(next,"Correct incomplete source research","Collect additional relevant evidence and rerun independent verification")?;
  self.revision+=1;self.start(next)?;Ok(next)
 }
 /// Suspend an active browser stage behind one bounded recovery prerequisite.
 /// Completed predecessors are immutable; recovery inherits dependencies.
 pub fn insert_recovery(&mut self,id:usize)->Result<usize>{
  ensure!(self.nodes.len()<MAX_NODES,"Recovery graph budget exhausted");
  let n=self.nodes.get(id).context("Unknown recovery parent")?;
  ensure!(n.kind==Kind::Browser&&n.state==State::Active&&n.attempts<=3,"Recovery is not eligible");
  let deps=n.depends_on.clone();
  let next=self.nodes.len();
  self.nodes.push(Node{id:next,kind:Kind::Recovery,depends_on:deps,state:State::Pending,receipt:None,attempts:0,objective:String::new(),criterion:String::new()});
  self.nodes[id].depends_on=vec![next];self.nodes[id].state=State::Pending;
  self.describe(next,"Recover the stalled objective","Observe meaningful progress or pass the existing stage gate")?;
  self.revision+=1;self.start(next)?;Ok(next)
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
 #[test]fn cancellation_never_reopens_completed_or_pending_nodes(){
  let mut g=WorkflowGraph::chain(&[Kind::Browser,Kind::Verification]).unwrap();
  g.start(0).unwrap();g.stop(true);assert!(g.ready().is_empty());assert!(!g.is_complete());
 }
}
