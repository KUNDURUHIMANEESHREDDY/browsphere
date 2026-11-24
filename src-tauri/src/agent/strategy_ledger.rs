//! Run-local bounded strategy scheduling. A dispatch is not confirmation.
//! Callers supply grounded candidates and independent postconditions.
use serde::Serialize;
use std::collections::BTreeMap;
#[derive(Clone,Copy,Debug,Eq,PartialEq,Ord,PartialOrd,Serialize)]
#[serde(rename_all="snake_case")]
pub enum Kind{Search,Filter,Category,Product,SearchButton,Pagination}
#[derive(Clone,Copy,Debug,Eq,PartialEq,Serialize)]
#[serde(rename_all="snake_case")]
pub enum Outcome{Pending,Confirmed,NoEffect,Exhausted}
#[derive(Clone,Debug,Serialize)]
pub struct Record{pub kind:Kind,pub outcome:Outcome,pub observations:u8}
#[derive(Clone,Debug,Default)]
pub struct Ledger{
 records:BTreeMap<String,Record>,pending:Option<String>,
}
impl Ledger{
 pub fn start(&mut self,key:String,kind:Kind)->bool{
  if self.pending.is_some()||self.records.contains_key(&key)||self.records.len()>=18{return false;}
  self.records.insert(key.clone(),Record{kind,outcome:Outcome::Pending,observations:0});self.pending=Some(key);true
 }
 pub fn pending(&self)->bool{self.pending.is_some()}
 pub fn observe(&mut self,confirmed:bool,limit:u8)->Outcome{
  let Some(k)=self.pending.clone()else{return Outcome::NoEffect;};
  let r=self.records.get_mut(&k).unwrap();
  r.observations=r.observations.saturating_add(1);
  if confirmed{r.outcome=Outcome::Confirmed;}
  else if r.observations>=limit.max(1){r.outcome=Outcome::NoEffect;}
  let result=r.outcome;
  if result!=Outcome::Pending{self.pending=None;}
  result
 }
 pub fn exhaust(&mut self){
  if let Some(k)=self.pending.take(){self.records.get_mut(&k).unwrap().outcome=Outcome::Exhausted;}
 }
 pub fn score(&self,kind:Kind,base:i32)->i32{
  let failed=self.records.values().filter(|r|r.kind==kind&&matches!(r.outcome,Outcome::NoEffect|Outcome::Exhausted)).count() as i32;
  base-failed*35
 }
 pub fn used(&self,key:&str)->bool{self.records.contains_key(key)}
 pub fn outcomes(&self)->Vec<Record>{self.records.values().cloned().collect()}
}
#[cfg(test)]
mod tests{
 use super::*;
 #[test]fn pending_waits_for_observed_effect_and_rejects_duplicate(){
  let mut l=Ledger::default();assert!(l.start("first".into(),Kind::Filter));
  assert!(!l.start("other".into(),Kind::Search));
  assert_eq!(l.observe(false,4),Outcome::Pending);
  assert_eq!(l.observe(true,4),Outcome::Confirmed);
  assert!(!l.start("first".into(),Kind::Filter));
 }
 #[test]fn failed_strategy_loses_priority_and_budget_is_bounded(){
  let mut l=Ledger::default();assert!(l.start("search1".into(),Kind::Search));
  assert_eq!(l.observe(false,1),Outcome::NoEffect);
  assert!(l.score(Kind::Search,90)<l.score(Kind::Category,70));
  for i in 0..17{assert!(l.start(format!("other{i}"),Kind::Product));l.observe(true,1);}
  assert!(!l.start("overflow".into(),Kind::Search));
 }
}
