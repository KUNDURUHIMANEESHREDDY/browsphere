//! Bounded native acquisition for the explicit requirement ledger. Every
//! proposal is an observed public same-repository link. Policy, binding and
//! postconditions remain outside this module.
use super::{browser_skills,dom_extractor::DomSnapshot,requirements::Ledger,policy::BrowserAction,semantic_core::RouteLedger};
use std::collections::HashSet;
#[derive(Default)]
pub struct Acquisition{attempted:HashSet<String>}
impl Acquisition{
    pub fn next(&mut self,goal:&str,page:&DomSnapshot,ledger:&Ledger,routes:&RouteLedger)->Option<BrowserAction>{
        if !ledger.supported||ledger.ready()||self.attempted.len()>=6||!browser_skills::readable(page){return None;}
        let missing=ledger.missing();
        let ranked=browser_skills::navigation::ranked(page,|n|{
            let Some(url)=n.href.as_deref()else{return 0;};
            if !super::requirements::repository_fact_link(goal,page,url){return 0;}
            let low=url.to_ascii_lowercase();
            let name=low.rsplit('/').next().unwrap_or("");
            let install=missing.contains(&"installation_command");
            let runtime=missing.contains(&"runtime_compatibility");
            if install&&(name.starts_with("readme")||name.starts_with("install")){120}
            else if runtime&&["pyproject.toml","setup.py","setup.cfg"].contains(&name){115}
            else if name.starts_with("readme")||name.starts_with("install"){90}
            else if ["docs","documentation"].contains(&name){80}else{65}
        },|u|!self.attempted.contains(u)&&routes.allowed(u));
        let (_,url)=ranked.into_iter().next()?;
        self.attempted.insert(url.clone());Some(BrowserAction::Navigate{url})
    }
    pub fn attempts(&self)->usize{self.attempted.len()}
}
#[cfg(test)]
mod tests{
 use super::*;
 use super::super::{dom_extractor::DomNodeSummary,orchestrator::SourceEvidence};
 const GOAL:&str="Find the official acme Widget repository on GitHub. Report the installation command and supported Python version.";
 fn node(id:&str,url:&str)->DomNodeSummary{DomNodeSummary{node_id:id.into(),label:id.into(),tag:"a".into(),role:"link".into(),href:Some(url.into()),options:vec![],checked:None,search_state:None,is_submit:false,is_sensitive:false,bbox:None}}
 #[test]fn acquisition_ranks_unfinished_facts_and_never_repeats_or_leaves_repo(){
  let root="https://github.com/acme/widget";let readme="https://github.com/acme/widget/blob/main/README.md";let metadata="https://github.com/acme/widget/blob/main/pyproject.toml";
  let p=DomSnapshot{structure:Default::default(),origin:root.into(),title:"GitHub - acme/widget".into(),text_excerpt:"A public widget project repository with enough introduction text to confirm the identity and no relevant commands.".into(),interactive_nodes:vec![node("metadata",metadata),node("external","https://other.example/acme/widget/blob/main/README.md"),node("readme",readme)]};
  let mut ledger=Ledger::compile(GOAL);ledger.refresh(GOAL,&[SourceEvidence{url:root.into(),title:p.title.clone(),excerpt:p.text_excerpt.clone()}]);
  let mut a=Acquisition::default();let mut routes=RouteLedger::default();routes.observe(0,root);
  assert!(matches!(a.next(GOAL,&p,&ledger,&routes),Some(BrowserAction::Navigate{url}) if url==readme));
  assert!(matches!(a.next(GOAL,&p,&ledger,&routes),Some(BrowserAction::Navigate{url}) if url==metadata));
  assert!(a.next(GOAL,&p,&ledger,&routes).is_none());assert_eq!(a.attempts(),2);
 }
}
