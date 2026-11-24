//! Crash-safe local graph journal. No raw task, URLs, page text, form values,
//! actions or tool results are persisted. Only read-only destination checks
//! can be resumed, after explicit user request and fresh native observation.
use anyhow::{ensure,Context,Result};
use rusqlite::{params,Connection,OptionalExtension};
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use super::{adaptive_graph::{WorkflowGraph,Kind,State,Receipt},dom_extractor::DomSnapshot,evidence_verifier::{VerificationReport,GroundedFinding}};
use std::path::Path;
use uuid::Uuid;
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResumeProof{goal_hash:String,page_hash:String}
fn digest(parts:&[&str])->String{
 let mut h=Sha256::new();
 for part in parts{h.update((part.len() as u64).to_be_bytes());h.update(part.as_bytes());}
 format!("{:x}",h.finalize())
}
fn eligible_page(p:&DomSnapshot)->bool{
 url::Url::parse(&p.origin).ok().is_some_and(|u|
 u.scheme()=="https"&&u.host_str().is_some()&&u.username().is_empty()&&u.password().is_none()&&u.query().is_none()&&u.fragment().is_none())
 && !super::deliberation::error_page(p)&&!super::deliberation::search_listing(p)
 && !p.text_excerpt.starts_with("Active dialog:")&&!p.title.trim().is_empty()
 && !p.interactive_nodes.iter().any(|n|n.is_sensitive)
}
pub fn resumable_goal(goal:&str)->bool{
 super::policy::requested_navigation_url(goal).is_some()||
 (super::deliberation::read_only(goal)&&
 (super::evidence_verifier::requested_named_link_title(goal).is_some()||super::evidence_verifier::requested_named_result_title(goal).is_some()||super::research::subject(goal).is_some()))
}
fn graph_ready(graph:&WorkflowGraph)->bool{
 graph.validate().is_ok()&&graph.nodes.iter().filter(|n|n.kind==Kind::Verification).count()==1&&
 graph.nodes.iter().all(|n|match n.kind{
  Kind::Verification=>matches!(n.state,State::Active|State::Completed),
  Kind::Browser|Kind::Recovery|Kind::Observation=>n.state==State::Completed,
  _=>false
 })
}
impl ResumeProof{
 /// Only call after the original native destination verifier passes.
 pub fn from_checked_destination(goal:&str,page:&DomSnapshot,graph:&WorkflowGraph)->Option<Self>{
  (resumable_goal(goal)&&eligible_page(page)&&graph_ready(graph)&&
   (super::research::subject(goal).is_none()||super::research::matches_subject(goal,page))).then(||Self{
   goal_hash:digest(&[goal.trim()]),page_hash:digest(&[&page.origin,&page.title])
  })
 }
 fn matches(&self,goal:&str,page:&DomSnapshot)->bool{
  resumable_goal(goal)&&eligible_page(page)&&self.goal_hash==digest(&[goal.trim()])&&self.page_hash==digest(&[&page.origin,&page.title])
 }
}
pub fn requested(goal:&str)->bool{goal.trim().starts_with("resume checkpoint ")}
pub fn parse(goal:&str)->Result<(Uuid,&str)>{
 let tail=goal.trim().strip_prefix("resume checkpoint ").context("Expected resume checkpoint")?;
 let (id,goal)=tail.split_once(' ').context("Use resume checkpoint <run ID> <original task>")?;
 let goal=goal.trim();ensure!(!goal.is_empty()&&goal.len()<=2000&&resumable_goal(goal),"Only explicit supported read-only navigation or single-document research checkpoints can be resumed");
 Ok((Uuid::parse_str(id)?,goal))
}
pub struct Store{conn:Connection}
impl Store{
 pub fn open(path:&Path)->Result<Self>{
  let conn=Connection::open(path)?;
  conn.busy_timeout(std::time::Duration::from_secs(2))?;
  conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
   CREATE TABLE IF NOT EXISTS workflow_checkpoints(run_id TEXT PRIMARY KEY,revision INTEGER NOT NULL,graph_json TEXT NOT NULL,proof_json TEXT,updated INTEGER NOT NULL);")?;
  let store=Self{conn};store.prune()?;Ok(store)
 }
 fn prune(&self)->Result<()>{
  self.conn.execute("DELETE FROM workflow_checkpoints WHERE updated < unixepoch()-86400",[])?;
  self.conn.execute("DELETE FROM workflow_checkpoints WHERE run_id IN (SELECT run_id FROM workflow_checkpoints ORDER BY updated DESC,rowid DESC LIMIT -1 OFFSET 100)",[])?;
  Ok(())
 }
 pub fn save(&mut self,run:Uuid,graph:&WorkflowGraph,proof:Option<&ResumeProof>)->Result<()>{
  graph.validate()?;
  let encoded=serde_json::to_string(graph)?;
  let proof=proof.filter(|_|graph_ready(graph)).map(serde_json::to_string).transpose()?;
  let tx=self.conn.transaction()?;
  tx.execute("INSERT INTO workflow_checkpoints(run_id,revision,graph_json,proof_json,updated) VALUES (?1,?2,?3,?4,unixepoch()) ON CONFLICT(run_id) DO UPDATE SET revision=excluded.revision,graph_json=excluded.graph_json,proof_json=excluded.proof_json,updated=excluded.updated WHERE excluded.revision>=workflow_checkpoints.revision",params![run.to_string(),graph.revision,encoded,proof])?;
  tx.commit()?;self.prune()
 }
 fn load_checked(&mut self,id:Uuid,goal:&str,page:&DomSnapshot)->Result<(WorkflowGraph,ResumeProof)>{
  self.prune()?;
  let row:Option<(String,Option<String>)>=self.conn.query_row("SELECT graph_json,proof_json FROM workflow_checkpoints WHERE run_id=?1",[id.to_string()],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
  let (raw,proof)=row.context("Checkpoint missing or expired")?;
  ensure!(raw.len()<=32768,"Invalid checkpoint size");
  let graph:WorkflowGraph=serde_json::from_str(&raw)?;
  graph.validate()?;ensure!(graph_ready(&graph),"Checkpoint has unresolved or uncertain work; no replay");
  let proof:ResumeProof=serde_json::from_str(&proof.context("Checkpoint has no safe recovery receipt")?)?;
  ensure!(proof.matches(goal,page),"Original task or live destination differs; no actions replayed");
  super::evidence_verifier::check_navigation_result(&page.origin,page).map_err(anyhow::Error::msg)?;
  if super::policy::requested_navigation_url(goal).is_none(){
   ensure!(page.text_excerpt.trim().chars().count()>=40,"Fresh title evidence is insufficient");
  }
  Ok((graph,proof))
 }
 pub fn begin_research(&mut self,id:Uuid,goal:&str,page:&DomSnapshot)->Result<(WorkflowGraph,ResumeProof,usize)>{
  ensure!(super::research::matches_subject(goal,page),"Fresh research subject does not match");
  let (mut graph,proof)=self.load_checked(id,goal,page)?;
  let node=graph.renew_verification()?;
  Ok((graph,proof,node))
 }
 pub fn recover(&mut self,id:Uuid,goal:&str,page:&DomSnapshot)->Result<(WorkflowGraph,VerificationReport)>{
  ensure!(super::research::subject(goal).is_none(),"Factual recovery requires independent research verification");
  let (mut graph,proof)=self.load_checked(id,goal,page)?;
  let mut report=if let Some(target)=super::policy::requested_navigation_url(goal){
   super::evidence_verifier::verify_direct_navigation(target.as_str(),page).map_err(anyhow::Error::msg)?
  }else{
   // The prior native verifier established the actual click/search destination.
   // Exact fresh URL/title re-observation verifies this narrow title-only claim.
   VerificationReport{status:"evidence_checked",summary:format!("Re-observed the checked destination: {}.",page.title),findings:vec![GroundedFinding{claim:"The previously checked destination is currently visible".into(),url:page.origin.clone(),quote:format!("Observed document title: {}",page.title)}],limitations:"Revalidation of a prior checked read-only destination, not a new click or factual research.".into()}
  };
  if let Some(i)=graph.nodes.iter().position(|n|n.kind==Kind::Verification&&n.state==State::Active){
   graph.complete(i,Receipt::FinalReport)?;
  }
  ensure!(graph.is_complete(),"Checkpoint still incomplete");
  report.limitations.push_str(" Recovered without replaying browser actions or MCP calls.");
  // Persist only after fresh independent observation passes.
  self.save(id,&graph,Some(&proof))?;
  Ok((graph,report))
 }
 pub fn clear(path:&Path)->Result<()>{
  if path.exists(){let store=Self::open(path)?;store.conn.execute("DELETE FROM workflow_checkpoints",[])?;}
  Ok(())
 }
}
#[cfg(test)]
mod tests{
 use super::*;
 fn page()->DomSnapshot{DomSnapshot{structure: Default::default(), origin:"https://example.org/".into(),title:"Example Domain".into(),text_excerpt:"This domain is for use in illustrative examples in documents.".into(),interactive_nodes:vec![]}}
 #[test]fn restart_recovery_requires_original_goal_and_fresh_destination(){
  let dir=std::env::temp_dir().join(format!("lucia-checkpoint-test-{}",Uuid::new_v4()));std::fs::create_dir_all(&dir).unwrap();
  let path=dir.join("checkpoint.sqlite3");let id=Uuid::new_v4();
  let mut g=WorkflowGraph::chain(&[Kind::Browser,Kind::Verification]).unwrap();
  g.describe(0,"raw private objective must not persist","example").unwrap();
  g.start(0).unwrap();g.complete(0,Receipt::StageGate).unwrap();g.start(1).unwrap();
  let proof=ResumeProof::from_checked_destination("Open example.org",&page(),&g).unwrap();
  {let mut store=Store::open(&path).unwrap();store.save(id,&g,Some(&proof)).unwrap();}
  {let mut store=Store::open(&path).unwrap();
   let raw:String=store.conn.query_row("SELECT graph_json FROM workflow_checkpoints",[],|r|r.get(0)).unwrap();
   assert!(!raw.contains("private objective"));
   assert!(store.recover(id,"Open example.com",&page()).is_err());
   let mut wrong=page();wrong.title="Access Denied".into();
   assert!(store.recover(id,"Open example.org",&wrong).is_err());
   let (g,report)=store.recover(id,"Open example.org",&page()).unwrap();
   assert!(g.is_complete());assert_eq!(report.status,"evidence_checked");
   assert!(report.limitations.contains("without replaying"));
  }
  Store::clear(&path).unwrap();let _=std::fs::remove_dir_all(dir);
 }
 #[test]fn stale_revision_and_tampered_receipts_are_rejected(){
  let mut g=WorkflowGraph::chain(&[Kind::Browser,Kind::Verification]).unwrap();
  g.start(0).unwrap();let stale=g.clone();
  g.complete(0,Receipt::StageGate).unwrap();g.start(1).unwrap();
  let proof=ResumeProof::from_checked_destination("Open example.org",&page(),&g).unwrap();
  let mut store=Store::open(Path::new(":memory:")).unwrap();let id=Uuid::new_v4();
  store.save(id,&g,Some(&proof)).unwrap();store.save(id,&stale,None).unwrap();
  assert!(store.recover(id,"Open example.org",&page()).is_ok());
  let mut bad=g.clone();bad.nodes[0].receipt=Some(Receipt::FinalReport);
  assert!(store.save(id,&bad,Some(&proof)).is_err());
  store.conn.execute("UPDATE workflow_checkpoints SET updated=0",[]).unwrap();
  assert!(store.recover(id,"Open example.org",&page()).is_err());
 }
 #[test]fn factual_checkpoint_requires_new_verification_not_title_only(){
  let gtext="Search for Neptune and open its article. Report its planet type.";
  let mut p=page();p.origin="https://example.org/wiki/Neptune".into();p.title="Neptune - Encyclopedia".into();p.text_excerpt="Neptune is an ice giant in the outer Solar System. It is the eighth planet from the Sun.".into();
  let mut g=WorkflowGraph::chain(&[Kind::Browser,Kind::Verification]).unwrap();g.start(0).unwrap();g.complete(0,Receipt::StageGate).unwrap();g.start(1).unwrap();g.complete(1,Receipt::FinalReport).unwrap();
  let proof=ResumeProof::from_checked_destination(gtext,&p,&g).unwrap();
  let mut store=Store::open(Path::new(":memory:")).unwrap();let id=Uuid::new_v4();store.save(id,&g,Some(&proof)).unwrap();
  assert!(store.recover(id,gtext,&p).is_err());
  let (renewed,_,node)=store.begin_research(id,gtext,&p).unwrap();assert_eq!(renewed.nodes[node].state,State::Active);assert_eq!(renewed.nodes[0].state,State::Completed);
  p.title="Help page".into();assert!(store.begin_research(id,gtext,&p).is_err());
 }
 #[test]fn unfinished_or_consequential_graph_cannot_resume(){
  let mut g=WorkflowGraph::chain(&[Kind::Browser,Kind::Verification]).unwrap();g.start(0).unwrap();
  assert!(ResumeProof::from_checked_destination("Open example.org",&page(),&g).is_none());
  g.complete(0,Receipt::StageGate).unwrap();g.start(1).unwrap();
  assert!(ResumeProof::from_checked_destination("Book train tickets",&page(),&g).is_none());
  let proof=ResumeProof::from_checked_destination("Open example.org",&page(),&g).unwrap();
  let mut store=Store::open(Path::new(":memory:")).unwrap();let id=Uuid::new_v4();
  g.stop(true);store.save(id,&g,Some(&proof)).unwrap();
  assert!(store.recover(id,"Open example.org",&page()).is_err());
 }
}
