//! Explicit read-only retailer research. Navigation remains subject to native
//! permission, allowlist and live action binding. No purchases or sign-ins.
use super::{browser_skills as bs,dom_extractor::{DomSnapshot,DomNodeSummary},policy::BrowserAction,evidence_verifier::{GroundedFinding,VerificationReport},orchestrator::SourceEvidence,execution_intelligence::Next};
use serde::Serialize;
use super::strategy_ledger::{Ledger as StrategyLedger,Kind as StrategyKind,Outcome as StrategyOutcome,Record as StrategyRecord};
use std::collections::{HashSet,BTreeMap};
const RETAILERS:[(&str,&str);2]=[("croma","https://www.croma.com/"),("reliance digital","https://www.reliancedigital.in/")];
#[derive(Clone,Debug)]
pub struct Intent{query:String,identity:Vec<String>,chip:String,ram:String,storage:String,size:String,sites:Vec<&'static str>}
fn word(s:&str,w:&str)->bool{bs::phrase(s,w)}
fn capacity(text:&str,unit:&str)->Option<String>{
 let w=bs::words(text);
 let mut result=Vec::new();
 for (i,x) in w.iter().enumerate(){
  if x.chars().all(|c|c.is_ascii_digit())&&w.get(i+1).map(String::as_str)==Some(unit){result.push(x.clone());}
  else if let Some(n)=x.strip_suffix(unit){if !n.is_empty()&&n.chars().all(|c|c.is_ascii_digit()){result.push(n.into());}}
 }
 result.sort();result.dedup();(result.len()==1).then(||result.remove(0))
}
fn has_cap(text:&str,n:&str,unit:&str)->bool{let w=bs::words(text);w.iter().any(|x|x==&format!("{n}{unit}"))||w.windows(2).any(|x|x[0]==n&&x[1]==unit)}
fn host(s:&str)->Option<String>{let u=bs::public_url(s)?;if u.scheme()!="https"||u.port().is_some()||u.fragment().is_some(){return None;}Some(u.host_str()?.trim_start_matches("www.").to_string())}
fn root(s:&str)->bool{bs::public_url(s).is_some_and(|u|u.path()=="/"&&u.query().is_none())}
impl Intent{
 pub fn compile(goal:&str)->Option<Self>{
  if !super::deliberation::read_only(goal){return None;}
  let g=goal.to_ascii_lowercase();
  if !g.contains("do not perform any transaction.")||!g.contains("price")||!g.contains("product page")||!g.starts_with("research the "){return None;}
  let (subject,tail)=goal.get("research the ".len()..)?.split_once(" on ")?;
  let (names,_)=tail.split_once('.')?;
  let names=bs::normalize(names);
  let names=names.split(" and ").collect::<Vec<_>>();
  if names.len()!=2||names[0]==names[1]{return None;}
  let sites=names.iter().map(|n|RETAILERS.iter().find(|(name,_)|name==n).map(|(_,u)|*u)).collect::<Option<Vec<_>>>()?;
  let (name,specs)=subject.split_once(" with ")?;
  let identity=bs::words(name).into_iter().filter(|w|w.chars().any(char::is_alphabetic)&&!["inch","inches"].contains(&w.as_str())).collect::<Vec<_>>();
  if identity.len()<2||identity.len()>8||!subject.chars().all(|c|c.is_alphanumeric()||matches!(c,' '|'-'|','|'.')){return None;}
  let size=capacity(name,"inch")?;
  let (chip_part,rest)=specs.split_once(',')?;
  let (memory_part,storage_part)=rest.split_once(" and ")?;
  let parts=[chip_part.trim(),memory_part.trim(),storage_part.trim()];
  let chip=bs::words(parts[0]).first()?.clone();
  if !word(parts[0],"chip")||!chip.chars().any(|c|c.is_ascii_digit())||!chip.chars().all(|c|c.is_ascii_alphanumeric()){return None;}
  let (ram,storage)=(capacity(parts[1],"gb")?,capacity(parts[2],"gb")?);
  if !word(parts[1],"memory")||!word(parts[2],"ssd")||ram==storage{return None;}
  let query=format!("{} {} {}GB {}GB",identity.join(" "),chip,ram,storage);
  Some(Self{query,identity,chip,ram,storage,size,sites})
 }
 pub fn entry(&self,url:&str)->bool{self.sites.iter().any(|x|*x==url||self.discovery_url(x)==url)}
 fn site(&self,url:&str)->Option<&'static str>{let h=host(url)?;self.sites.iter().copied().find(|s|host(s).as_deref()==Some(&h))}
 fn discovery_url(&self,site:&str)->String{
  let mut u=url::Url::parse("https://www.google.com/search").unwrap();
  u.query_pairs_mut().append_pair("q",&format!("site:{} {}",host(site).unwrap_or_default(),self.query));u.to_string()
 }
 fn discovery_site(&self,raw:&str)->Option<&'static str>{
  let u=bs::public_url(raw)?;
  if u.scheme()!="https"||u.host_str()!=Some("www.google.com")||u.port().is_some()||u.path()!="/search"||u.fragment().is_some(){return None;}
  let q=u.query_pairs().filter(|(k,_)|k=="q").map(|(_,v)|v.into_owned()).collect::<Vec<_>>();
  if q.len()!=1{return None;}
  self.sites.iter().copied().find(|site|q[0]==format!("site:{} {}",host(site).unwrap_or_default(),self.query))
 }
 fn discovery_target(&self,from:&str,to:&str)->bool{
  let Some(site)=self.discovery_site(from)else{return false;};
  let Some(u)=bs::public_url(to)else{return false;};
  self.site(to)==Some(site)&&u.query().is_none()&&u.fragment().is_none()&&
    (u.path().starts_with("/product/")||u.path().contains("/p/"))&&self.spec_score(&u.path().replace(['-','_','/']," "))>=130
 }
 fn identity_ok(&self,s:&str)->bool{self.identity.iter().all(|w|word(s,w))}
 fn model_conflict(&self,s:&str)->bool{
  let words=bs::words(s);let i=self.chip.find(|c:char|c.is_ascii_digit()).unwrap_or(self.chip.len());let prefix=&self.chip[..i];
  !prefix.is_empty()&&words.iter().any(|w|w!=&self.chip&&w.strip_prefix(prefix).is_some_and(|v|!v.is_empty()&&v.chars().all(|c|c.is_ascii_digit())))
 }
 fn spec_score(&self,s:&str)->u32{
  if !self.identity_ok(s)||self.model_conflict(s){return 0;}
  let mut score=100;
  if word(s,&self.chip){score+=30;}
  if has_cap(s,&self.ram,"gb"){score+=20;}
  if has_cap(s,&self.storage,"gb"){score+=20;}
  score
 }
 fn complete_specs(&self,s:&str)->bool{
  if self.spec_score(s)!=170{return false;}
  // Every GB capacity in the primary product heading must match one of
  // the requested capacities. Never combine headings from different cards.
  let w=bs::words(s);
  for (i,x) in w.iter().enumerate(){
   let n=if w.get(i+1).map(String::as_str)==Some("gb"){Some(x.as_str())}else{x.strip_suffix("gb")};
   if let Some(n)=n{if n.chars().all(|c|c.is_ascii_digit())&&!n.is_empty()&&n!=self.ram&&n!=self.storage{return false;}}
  }
  // A marketed 13-inch class may have a fractional display measurement;
  // accept only an exact 13-inch token or 13.x-inch in the primary heading.
  let low=bs::normalize(s);
  if has_cap(s,&self.size,"inch")||has_cap(s,&self.size,"inches"){return true;}
  let needle=format!("{}.",self.size);
  low.match_indices(&needle).any(|(i,_)|{
    let tail=&low[i+needle.len()..];
    let digits=tail.bytes().take_while(u8::is_ascii_digit).count();
    digits>0&&(i==0||!low.as_bytes()[i-1].is_ascii_digit())&&tail[digits..].trim_start_matches(|c:char|c.is_whitespace()||c=='-').starts_with("inch")
  })
 }
 fn link_score(&self,p:&DomSnapshot,n:&DomNodeSummary)->u32{
  let Some(url)=n.href.as_deref()else{return 0;};
  if !(self.site(&p.origin).is_some()&&self.site(url)==self.site(&p.origin)||self.discovery_target(&p.origin,url))||root(url)||url==p.origin||!bs::public_control(n){return 0;}
  let Some(u)=bs::public_url(url)else{return 0;};
  if u.query().is_some()||forbidden(&format!("{} {}",n.label,u.path())){return 0;}
  let path=u.path().to_ascii_lowercase();
  if !(path.starts_with("/product/")||path.contains("/p/")){return 0;}
  let text=format!("{} {}",n.label,u.path().replace(['-','_','/']," "));
  let score=self.spec_score(&text);
  if score<130{return 0;}
  // A visible title with two capacities must match both requested values.
  // Generic model links with absent capacities may be inspected, but an
  // explicit contradictory variant must never be selected.
  let label=bs::words(&n.label);
  let caps=label.iter().enumerate().filter_map(|(i,w)|{
   let v=if label.get(i+1).map(String::as_str)==Some("gb"){Some(w.as_str())}else{w.strip_suffix("gb")};
   v.filter(|v|!v.is_empty()&&v.chars().all(|c|c.is_ascii_digit()))
  }).collect::<Vec<_>>();
  if caps.iter().any(|v|*v!=self.ram&&*v!=self.storage){return 0;}
  if word(&n.label,"tb")||label.iter().any(|w|w.strip_suffix("tb").is_some_and(|v|!v.is_empty()&&v.chars().all(|c|c.is_ascii_digit()))){return 0;} // at least matching model family, not a merchant/category name
  score
 }
 fn broad_query(&self)->String{self.identity.join(" ")}
 fn query_allowed(&self,s:&str)->bool{s==self.query||s==format!("{} {}",self.identity.join(" "),self.chip)||s==self.broad_query()}
 // Alternative discovery is restricted to observed same-retailer category
 // destinations with an explicit product-family or requested-chip label.
 fn category_score(&self,p:&DomSnapshot,n:&DomNodeSummary)->u32{
  let Some(url)=n.href.as_deref()else{return 0;};
  if self.site(&p.origin).is_none()||self.site(url)!=self.site(&p.origin)||root(url)||url==p.origin||!bs::public_control(n){return 0;}
  let Some(u)=bs::public_url(url)else{return 0;};
  if u.query().is_some()||u.fragment().is_some()||forbidden(&format!("{} {}",n.label,u.path())){return 0;}
  let path=u.path().to_ascii_lowercase();
  if path.starts_with("/product/")||path.contains("/p/"){return 0;}
  // An observed exact-brand storefront is a bounded discovery waypoint,
  // not product evidence. Some retailers expose only this link on the home page.
  let brand=self.identity.first().map(String::as_str).unwrap_or("");
  let first=path.trim_start_matches('/').split('/').next().unwrap_or("");
  let second=path.trim_start_matches('/').split('/').nth(1).unwrap_or("");
  let storefront=(!brand.is_empty())&&(
    (first==format!("{brand}-store")&&second=="b")||
    (first=="collection"&&[format!("{brand}-store"),format!("{brand}-homeview"),brand.to_string()].iter().any(|x|x==second)));
  if storefront{return 30;}
  if !(path.starts_with("/collection/")||path.starts_with("/sections/")||path.contains("/c/")||path.ends_with("/c")||path.starts_with("/category/")){return 0;}
  let text=format!("{} {}",n.label,path.replace(['-','_','/']," "));
  if self.model_conflict(&text){return 0;}
  let family=self.identity.iter().skip(1).all(|w|word(&text,w));
  let chip=word(&text,&self.chip)&&word(&text,&self.identity[0]);
  if family&&chip{return 80;}if family{return 65;}if chip{return 55;}
  // A family index can be a useful intermediate waypoint from an exact
  // brand storefront. Match a complete path segment (singular/plural),
  // not accessory text or a different named model. Never certify the page
  // as a product; the full product identity still applies at link/offer stage.
  let current_path=bs::public_url(&p.origin).map(|u|u.path().to_ascii_lowercase()).unwrap_or_default();
  let on_brand=current_path.starts_with(&format!("/{brand}-store/"))||
    current_path.starts_with(&format!("/collection/{brand}-"));
  let family_head=self.identity.get(1).map(String::as_str).unwrap_or("");
  let family_plural=format!("{family_head}s");
  let has_family_segment=!family_head.is_empty()&&path.split('/').any(|seg|seg==family_head||seg==family_plural);
  if on_brand&&has_family_segment&&!["accessories","cases","covers","chargers","adapters","sleeves"].iter().any(|x|word(&text,x)){45}else{0}

 }
 fn chip_filter(&self,p:&DomSnapshot,n:&DomNodeSummary)->bool{
  if !result_page(p)||self.site(&p.origin).is_none()||!bs::public_control(n)||n.role!="checkbox"||n.checked!=Some(false)||n.href.is_some(){return false;}
  let label=bs::words(&n.label);
  let brand=self.identity.first().map(String::as_str).unwrap_or("");
  let chip=self.chip.as_str();
  matches!(label.iter().map(String::as_str).collect::<Vec<_>>().as_slice(),
    [c] if *c==chip)||
  matches!(label.iter().map(String::as_str).collect::<Vec<_>>().as_slice(),
    [c,"chip"] if *c==chip)||
  matches!(label.iter().map(String::as_str).collect::<Vec<_>>().as_slice(),
    [b,c] | [b,c,"chip"] if *b==brand&&*c==chip)
 }
}
fn forbidden(s:&str)->bool{["buy now","add to cart","add to basket","checkout","payment","place order","sign in","log in","login","register","subscribe","compare products","contact sales","privacy policy"].iter().any(|w|word(s,w))}
fn input(n:&DomNodeSummary)->bool{
 bs::search::input(n)&&!["category","search brand","brand filter","filter","within results","pincode","location"].iter().any(|w|word(&n.label,w))
}
fn opener(n:&DomNodeSummary)->bool{
 bs::search::opener(n)||(bs::public_control(n)&&n.href.is_none()&&n.role=="combobox"&&n.tag!="input"&&n.label.trim().eq_ignore_ascii_case("search"))
}
fn search_button(n:&DomNodeSummary)->bool{
 bs::public_control(n)&&n.href.is_none()&&matches!(n.role.as_str(),"button")&&
 ["search","search products","submit search"].contains(&bs::normalize(&n.label).as_str())
}
fn result_page(p:&DomSnapshot)->bool{
 bs::public_url(&p.origin).is_some_and(|u|u.query_pairs().any(|(k,v)|matches!(k.as_ref(),"q"|"query"|"search"|"text")&&!v.is_empty())||
   u.path().split('/').any(|x|matches!(x,"search"|"search-results")))
}
fn visible(n:&DomNodeSummary)->bool{n.bbox.as_ref().map_or(true,|b|b.in_viewport)}
fn price_value(s:&str)->Option<u64>{
 let t=s.trim();
 let v=t.strip_prefix('\u{20b9}').or_else(||t.strip_prefix("Rs.")).or_else(||t.strip_prefix("INR"))?.trim();
 let (whole,frac)=v.split_once('.').unwrap_or((v,"00"));
 if frac.len()!=2||!frac.chars().all(|c|c.is_ascii_digit())||!whole.chars().all(|c|c.is_ascii_digit()||c==','){return None;}
 let amount=whole.replace(',',"").parse::<u64>().ok()?;
 (amount>=1000&&amount<100_000_000).then_some(amount*100+frac.parse::<u64>().ok()?)
}
#[derive(Clone,Debug)]
struct Product{url:String,heading:String,price:String,amount:u64,quote:String}
fn product(i:&Intent,p:&DomSnapshot,sources:&[SourceEvidence])->Option<Product>{
 if i.site(&p.origin).is_none()||root(&p.origin)||p.structure.loading||!bs::readable(p)||!bs::public_url(&p.origin).is_some_and(|u|u.query().is_none()&&(u.path().starts_with("/product/")||u.path().contains("/p/"))){return None;}
 let record=p.structure.product.as_ref()?;
 if !i.complete_specs(&record.heading)||!i.identity_ok(&p.title)||i.model_conflict(&p.title){return None;}
 let amount=price_value(&record.price)?;
 if !bs::normalize(&record.context).contains(&bs::normalize(&record.heading))||
    !bs::normalize(&record.context).contains(&bs::normalize(&record.price)){return None;}
 let source=sources.iter().find(|s|s.url==p.origin&&bs::verification::quote_present(s,&record.context))?;
 Some(Product{url:source.url.clone(),heading:record.heading.clone(),price:record.price.clone(),amount,quote:record.context.clone()})
}
#[derive(Clone,Debug)]
enum Expected{Search{origin:String,query:String},Filter{label:String},Destination{url:String},Pagination{scope:String,links:Vec<String>}}
fn pagination_scope(p:&DomSnapshot)->String{
 let Some(u)=bs::public_url(&p.origin)else{return String::new();};
 let mut fields=u.query_pairs().filter(|(k,_)|!matches!(k.as_ref(),"page"|"page_no"|"pageNumber")).map(|(k,v)|(k.into_owned(),v.into_owned())).collect::<Vec<_>>();fields.sort();
 format!("{}{}{:?}",u.origin().ascii_serialization(),u.path(),fields)
}
fn product_links(p:&DomSnapshot)->Vec<String>{
 let mut v=p.interactive_nodes.iter().filter_map(|n|n.href.as_ref().filter(|u|u.contains("/product/")||u.contains("/p/"))).cloned().collect::<Vec<_>>();
 v.sort();v.dedup();v
}
fn next_page(n:&DomNodeSummary)->bool{
 bs::public_control(n)&&n.href.is_none()&&n.role=="button"&&["next page","go to next page","next results page"].contains(&bs::normalize(&n.label).as_str())
}
fn query_matches(p:&DomSnapshot,q:&str)->bool{
 bs::public_url(&p.origin).is_some_and(|u|u.query_pairs().any(|(k,v)|
   matches!(k.as_ref(),"q"|"query"|"search"|"text")&&bs::normalize(&v)==bs::normalize(q)))
}
fn filter_observed(p:&DomSnapshot,label:&str)->bool{
 let checked=p.interactive_nodes.iter().any(|n|n.role=="checkbox"&&bs::normalize(&n.label)==bs::normalize(label)&&n.checked==Some(true));
 let selected=bs::public_url(&p.origin).is_some_and(|u|u.query_pairs().any(|(k,v)|k.eq_ignore_ascii_case("processor")&&bs::normalize(&v)==bs::normalize(label)));
 let remove=p.interactive_nodes.iter().any(|n|!n.is_sensitive&&bs::normalize(&n.label)==format!("remove {} filter",bs::normalize(label)));
 checked||(selected&&remove)
}
impl Expected{
 fn confirmed(&self,p:&DomSnapshot)->bool{
  if !bs::readable(p)||p.structure.loading{return false;}
  match self{
   Self::Search{origin,query}=>p.origin!=*origin&&result_page(p)&&query_matches(p,query),
   Self::Filter{label}=>filter_observed(p,label),
   Self::Destination{url}=>bs::navigation::receipt(url,p)==bs::Outcome::Confirmed,
   Self::Pagination{scope,links}=>pagination_scope(p)==*scope&&!product_links(p).is_empty()&&product_links(p)!=*links,
  }
 }
 fn limit(&self)->u8{match self{Self::Search{..}|Self::Filter{..}|Self::Pagination{..}=>6,Self::Destination{..}=>4}}
}
#[derive(Clone,Debug,Default)]
struct SiteState{result_signature:Vec<String>,settle_extensions:u8,actions:u8,pages:u8,discovery_started:bool,discovery_wait:u8,entry_attempted:bool,attempts:HashSet<String>,typed:Option<(String,String,String)>,waiting:u8,short_query:bool,restart:bool,blocked:bool,submitted:bool,filtered:bool,broad_query:bool,category_attempts:u8,submitted_query:Option<(String,String,String)>,strategy:StrategyLedger,expected:Option<Expected>,observations:u8}
#[derive(Serialize)]
pub struct SiteStatus{pub retailer:String,pub model:bool,pub chip:bool,pub memory:bool,pub storage:bool,pub price:bool,pub status:&'static str,pub attempts:usize,pub submitted:bool,pub filtered:bool,pub observations:u8,pub actions:u8,pub pages:u8,pub discovery_started:bool,pub strategies:Vec<StrategyRecord>}
pub struct Run{pub intent:Intent,states:BTreeMap<&'static str,SiteState>,products:BTreeMap<&'static str,Product>}
impl Run{
 pub fn new(goal:&str)->Option<Self>{let intent=Intent::compile(goal)?;Some(Self{states:intent.sites.iter().map(|s|(*s,SiteState::default())).collect(),products:BTreeMap::new(),intent})}
 pub fn ready(&self)->bool{self.products.len()==self.intent.sites.len()}
 pub fn statuses(&self)->Vec<SiteStatus>{self.intent.sites.iter().map(|s|{let verified=self.products.contains_key(s);SiteStatus{retailer:host(s).unwrap_or_default(),model:verified,chip:verified,memory:verified,storage:verified,price:verified,status:if verified{"verified"}else if self.states[s].blocked{"unverified"}else{"missing"},attempts:self.states[s].attempts.len(),submitted:self.states[s].submitted,filtered:self.states[s].filtered,observations:self.states[s].observations,actions:self.states[s].actions,pages:self.states[s].pages,discovery_started:self.states[s].discovery_started,strategies:self.states[s].strategy.outcomes()}}).collect()}
 pub fn next(&mut self,p:&DomSnapshot,sources:&[SourceEvidence])->Next{
  let out=self.next_inner(p,sources);
  if let Some(site)=self.intent.sites.iter().copied().find(|s|!self.products.contains_key(s)&&!self.states[s].blocked){
   let state=self.states.get_mut(site).unwrap();
   if matches!(&out,Next::Action(a,_) if !matches!(a,BrowserAction::Complete{..})){state.actions=state.actions.saturating_add(1);}
  }
  out
 }
 fn next_inner(&mut self,p:&DomSnapshot,sources:&[SourceEvidence])->Next{
  self.products.retain(|site,item|self.intent.site(&item.url)==Some(*site)&&self.intent.complete_specs(&item.heading)&&price_value(&item.price)==Some(item.amount)&&sources.iter().any(|s|s.url==item.url&&bs::verification::quote_present(s,&item.quote)));
  let current=self.intent.site(&p.origin);
  if let Some(site)=current{
   if let Some(item)=product(&self.intent,p,sources){self.products.insert(site,item);}
  }
  if self.ready(){return Next::Action(BrowserAction::Complete{summary:"Matching public products observed at both requested retailers; verify evidence.".into()},"catalog:handoff");}
  let site=self.intent.sites.iter().copied().find(|s|!self.products.contains_key(s)&&!self.states[s].blocked);
  let Some(site)=site else{return Next::Stop("Matching product and price evidence unavailable from one or more named retailers; no comparison claimed");};
  let state=self.states.get_mut(site).unwrap();
  if state.observations>=60||state.actions>=22{state.strategy.exhaust();state.expected=None;state.blocked=true;return Next::Wait;}
  state.observations=state.observations.saturating_add(1);
  if current==Some(site){state.entry_attempted=true;}
  if self.intent.discovery_site(&p.origin)==Some(site)&&state.discovery_started{
   if super::recovery::authentication_gate(p)||super::deliberation::error_page(p){state.blocked=true;return Next::Wait;}
   let ranked=bs::navigation::ranked(p,|n|self.intent.link_score(p,n),|u|!state.attempts.contains(&format!("link:{u}")));
   if let Some((_,url))=ranked.into_iter().next(){
    state.attempts.insert(format!("link:{url}"));state.waiting=0;
    return Next::Action(BrowserAction::Navigate{url},"catalog:discovered_product");
   }
   state.discovery_wait+=1;
   if state.discovery_wait>=5{state.blocked=true;}
   return Next::Wait;
  }
  if current!=Some(site){
   if state.entry_attempted{if !state.discovery_started{state.discovery_started=true;return Next::Action(BrowserAction::Navigate{url:self.intent.discovery_url(site)},"catalog:public_search_discovery");}state.blocked=true;return Next::Wait;}
   state.entry_attempted=true;return Next::Action(BrowserAction::Navigate{url:site.into()},"catalog:retailer_entry");
  }
  if super::recovery::authentication_gate(p)||super::deliberation::error_page(p)||state.attempts.len()>=18{
    state.strategy.exhaust();state.expected=None;if !state.discovery_started{state.discovery_started=true;return Next::Action(BrowserAction::Navigate{url:self.intent.discovery_url(site)},"catalog:public_search_discovery");}state.blocked=true;return Next::Wait;
  }
  // Reconcile delayed effects even when the executor's immediate receipt
  // was a failure. Only independent fresh observations close a strategy.
  if let Some(expected)=state.expected.clone(){
   let confirmed=expected.confirmed(p);
   let outcome=state.strategy.observe(confirmed,expected.limit());
   if outcome==StrategyOutcome::Pending{return Next::Wait;}
   state.expected=None;
   if let Expected::Pagination{scope,..}=&expected{if pagination_scope(p)!=*scope{state.blocked=true;return Next::Wait;}}
   if matches!(expected,Expected::Filter{..}){
    state.filtered=confirmed;
    if confirmed{state.waiting=0;state.result_signature.clear();state.settle_extensions=0;return Next::Wait;}
   }
   if matches!(expected,Expected::Pagination{..})&&confirmed{state.waiting=0;return Next::Wait;}
   // A late confirmed action opens a fresh result-observation window.
   // Do not reset its state or launch another query in the same turn.
   if confirmed&&matches!(expected,Expected::Search{..}|Expected::Filter{..}){
    state.waiting=0;return Next::Wait;
   }
   // A confirmed query/filter means a rendered result can be inspected.
   // A failed query makes a different discovery strategy preferable.
   state.waiting=5;
  }
  if p.structure.loading||!bs::readable(p){
   state.waiting+=1;
   if state.waiting>6{if !state.discovery_started{state.discovery_started=true;return Next::Action(BrowserAction::Navigate{url:self.intent.discovery_url(site)},"catalog:public_search_discovery");}state.blocked=true;}
   return Next::Wait;
  }
  let mut candidates=Vec::<(u32,String,BrowserAction)>::new();
  for (score,url) in bs::navigation::ranked(p,|n|self.intent.link_score(p,n),|_|true){
   candidates.push((score,format!("link:{url}"),BrowserAction::Navigate{url}));
  }
  // Hydration may replace the list after its URL or filter is already
  // confirmed. Observe changes in product links rather than volatile IDs;
  // extend the settling window at most three times per result interaction.
  if state.submitted&&result_page(p){
   let links=product_links(p);
   if links!=state.result_signature{
    state.result_signature=links;
    if state.settle_extensions<3{state.settle_extensions+=1;state.waiting=0;return Next::Wait;}
   }
  }
  // Only legacy/initial states without a recorded strategy need a short
  // settling window. Pending dispatches above own their observation budget.
  if candidates.is_empty()&&state.submitted&&state.waiting<2{
    state.waiting+=1;return Next::Wait;
  }
  if candidates.is_empty()&&state.submitted&&state.category_attempts<3&&
    state.strategy.score(StrategyKind::Search,90)<90{
   let ranked=bs::navigation::ranked(p,|n|self.intent.category_score(p,n),|u|!state.attempts.contains(&format!("category:{u}")));
   if let Some((score,url))=ranked.into_iter().next(){
    if state.strategy.score(StrategyKind::Category,70+(score/4) as i32)>=state.strategy.score(StrategyKind::Search,90){
     let key=format!("category:{url}");
     if state.strategy.start(key.clone(),StrategyKind::Category){
      state.attempts.insert(key);state.category_attempts+=1;state.waiting=0;state.typed=None;
      state.expected=Some(Expected::Destination{url:url.clone()});
      return Next::Action(BrowserAction::Navigate{url},"catalog:adaptive_category_recovery");
     }
    }
   }
  }
  // Use an exact observed processor checkbox if the search engine returned
  // only other models. Do not change location, consent or availability.
  if state.submitted&&!state.filtered&&candidates.is_empty(){
   let matching=p.interactive_nodes.iter().filter(|n|self.intent.chip_filter(p,n)).collect::<Vec<_>>();
   // Prefer a uniquely brand-qualified chip; ambiguous duplicates are not
   // actionable. The selected ID is still bound against the fresh DOM.
   let qualified=matching.iter().copied().filter(|n|word(&n.label,&self.intent.identity[0])).collect::<Vec<_>>();
   let selected=if qualified.len()==1{Some(qualified[0])}else if qualified.is_empty()&&matching.len()==1{Some(matching[0])}else{None};
   if let Some(n)=selected{
    let query=bs::public_url(&p.origin).and_then(|u|u.query_pairs().find(|(k,_)|matches!(k.as_ref(),"q"|"query"|"search"|"text")).map(|(_,v)|bs::normalize(&v))).unwrap_or_else(||"observed_results".into());
    let key=format!("chip-filter:{query}:{}",self.intent.chip);
    if !state.attempts.contains(&key)&&state.strategy.start(key.clone(),StrategyKind::Filter){
     state.attempts.insert(key);state.waiting=0;
     state.expected=Some(Expected::Filter{label:n.label.clone()});
     return Next::Action(BrowserAction::SetChecked{node_id:n.node_id.clone(),checked:true,element_label:n.label.clone()},"catalog:exact_processor_filter");
    }
   }
  }
  if candidates.is_empty()&&state.submitted&&state.pages<2&&result_page(p)&&!product_links(p).is_empty(){
   if let Some(n)=bs::unique(p.interactive_nodes.iter().filter(|n|next_page(n)&&visible(n))){
    let key=format!("pagination:{}:{:?}",p.origin,product_links(p));
    if state.strategy.start(key.clone(),StrategyKind::Pagination){
     state.attempts.insert(key);state.pages+=1;state.waiting=0;
     state.expected=Some(Expected::Pagination{scope:pagination_scope(p),links:product_links(p)});
     return Next::Action(BrowserAction::Click{node_id:n.node_id.clone(),element_label:n.label.clone(),is_submit:false},"catalog:next_page");
    }
   }
  }
  let fields=p.interactive_nodes.iter().filter(|n|input(n)&&visible(n)).collect::<Vec<_>>();
  if let Some(n)=bs::unique(fields.iter().copied()).filter(|_|!state.submitted){
   let query=if state.broad_query{self.intent.broad_query()}else if state.short_query{format!("{} {}",self.intent.identity.join(" "),self.intent.chip)}else{self.intent.query.clone()};
   let key=format!("type:{}:{}",p.origin,query);
   if state.typed.as_ref().is_some_and(|(o,l,q)|o==&p.origin&&l==&n.label&&q==&query)&&n.search_state.as_deref()==Some("filled"){
    candidates.push((95,format!("enter:{}:{}",p.origin,query),BrowserAction::PressKey{node_id:n.node_id.clone(),key:"Enter".into()}));
   }else{
    candidates.push((90,key,BrowserAction::TypeText{node_id:n.node_id.clone(),text:query,is_sensitive:false}));
   }
  }else if fields.is_empty(){
   if let Some(n)=bs::unique(p.interactive_nodes.iter().filter(|n|opener(n)&&visible(n))).filter(|_|!state.submitted){
    candidates.push((80,format!("open:{}:{}",p.origin,n.label),BrowserAction::Click{node_id:n.node_id.clone(),element_label:n.label.clone(),is_submit:false}));
   }
  }
  candidates.retain(|(_,k,_)|!state.attempts.contains(k));candidates.sort_by(|a,b|b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
  if let Some((_,k,a))=candidates.into_iter().next(){
   match &a{
    BrowserAction::Navigate{url}=>{
     if !state.strategy.start(k.clone(),StrategyKind::Product){state.blocked=true;return Next::Wait;}
     state.expected=Some(Expected::Destination{url:url.clone()});
    }
    BrowserAction::PressKey{key,..} if key=="Enter"=>{
     let Some((origin,_,query))=state.typed.clone()else{return Next::Wait;};
     if !state.strategy.start(k.clone(),StrategyKind::Search){state.blocked=true;return Next::Wait;}
     state.expected=Some(Expected::Search{origin,query});
     // Store the exact acknowledged entry before dispatch. The next live
     // snapshot, not the executor return code, determines result progress.
     state.submitted_query=state.typed.clone();state.submitted=true;state.result_signature.clear();state.settle_extensions=0;
    }
    _=>{}
   }
   state.attempts.insert(k);state.waiting=0;
   return Next::Action(a,"catalog:ranked_public_action");
  }
  if state.waiting<2{state.waiting+=1;return Next::Wait;}
  if state.submitted&&!result_page(p)&&state.typed.is_none()&&state.submitted_query.as_ref().is_some_and(|(o,_,q)|o==&p.origin&&self.intent.query_allowed(q)){
   if let Some(n)=bs::unique(p.interactive_nodes.iter().filter(|n|search_button(n)&&visible(n))){
    if bs::unique(p.interactive_nodes.iter().filter(|n|input(n)&&n.search_state.as_deref()==Some("filled")&&state.submitted_query.as_ref().is_some_and(|(_,l,_)|l==&n.label))).is_some()&&state.attempts.insert(format!("button:{}",p.origin)){
     let Some((origin,_,query))=state.submitted_query.clone()else{return Next::Wait;};
     if state.strategy.start(format!("button:{}",p.origin),StrategyKind::SearchButton){
      state.expected=Some(Expected::Search{origin,query});state.waiting=0;
      return Next::Action(BrowserAction::Click{node_id:n.node_id.clone(),element_label:n.label.clone(),is_submit:false},"catalog:search_button_fallback");
     }
    }
   }
  }
  if !state.short_query{state.short_query=true;state.typed=None;state.submitted=false;state.filtered=false;state.waiting=0;return Next::Wait;}
  if !state.broad_query{state.broad_query=true;state.typed=None;state.submitted=false;state.filtered=false;state.waiting=0;return Next::Wait;}
  if state.category_attempts<3{
   let ranked=bs::navigation::ranked(p,|n|self.intent.category_score(p,n),|u|!state.attempts.contains(&format!("category:{u}")));
   if let Some((_,url))=ranked.into_iter().next(){
    let key=format!("category:{url}");
    if state.strategy.start(key.clone(),StrategyKind::Category){
     state.attempts.insert(key);state.category_attempts+=1;state.waiting=0;state.typed=None;
     state.expected=Some(Expected::Destination{url:url.clone()});
     return Next::Action(BrowserAction::Navigate{url},"catalog:observed_category_fallback");
    }
   }
  }
  if !state.restart&&!root(&p.origin)&&!result_page(p){state.restart=true;state.typed=None;state.submitted=false;state.waiting=0;return Next::Action(BrowserAction::Navigate{url:site.into()},"catalog:restart_public_search");}
  if !state.discovery_started{state.discovery_started=true;state.waiting=0;return Next::Action(BrowserAction::Navigate{url:self.intent.discovery_url(site)},"catalog:public_search_discovery");}
  state.blocked=true;Next::Wait
 }
 pub fn receipt(&mut self,before:&DomSnapshot,after:&DomSnapshot,a:&BrowserAction){
  let Some(site)=self.intent.site(&before.origin)else{return;};
  let state=self.states.get_mut(site).unwrap();
  match a{
   BrowserAction::TypeText{node_id,text,is_sensitive:false} if self.intent.query_allowed(text)=>{
    if bs::search::entered(before,after,node_id)==bs::Outcome::Confirmed{
     if let Some(n)=before.interactive_nodes.iter().find(|n|n.node_id==*node_id){state.typed=Some((after.origin.clone(),n.label.clone(),text.clone()));}
    }
   }
   BrowserAction::PressKey{key,..} if key=="Enter"=>{if let Some(typed)=state.typed.take(){state.submitted_query=Some(typed);}state.submitted=true;state.waiting=0;state.filtered=false;}
   BrowserAction::SetChecked{node_id,checked:true,..}=>{
    if let Some(n)=before.interactive_nodes.iter().find(|n|n.node_id==*node_id&&self.intent.chip_filter(before,n)){
     state.filtered=filter_observed(after,&n.label);
    }
   }
   _=>{}
  }
 }
 pub fn authorized_filter(&self,p:&DomSnapshot,a:&BrowserAction)->bool{
  matches!(a,BrowserAction::SetChecked{checked:true,..})&&self.check(p,a).is_ok()
 }
 fn search_button_allowed(&self,p:&DomSnapshot,n:&DomNodeSummary)->bool{
  search_button(n)&&self.intent.site(&p.origin).is_some_and(|site|{
   let state=&self.states[site];
   state.submitted&&!result_page(p)&&state.submitted_query.as_ref().is_some_and(|(o,l,q)|
     o==&p.origin&&self.intent.query_allowed(q)&&bs::unique(p.interactive_nodes.iter().filter(|m|input(m)&&m.label==*l&&m.search_state.as_deref()==Some("filled"))).is_some())
  })
 }
 pub fn check(&self,p:&DomSnapshot,a:&BrowserAction)->Result<(),&'static str>{
  let n=|id:&str|p.interactive_nodes.iter().find(|n|n.node_id==id&&!n.is_sensitive);
  match a{
   BrowserAction::Navigate{url} if self.intent.entry(url)||p.interactive_nodes.iter().any(|n|n.href.as_deref()==Some(url)&&(self.intent.link_score(p,n)>0||self.intent.category_score(p,n)>0))=>Ok(()),
   BrowserAction::TypeText{node_id,text,is_sensitive:false} if self.intent.site(&p.origin).is_some()&&self.intent.query_allowed(text)&&n(node_id).is_some_and(input)=>Ok(()),
   BrowserAction::Click{node_id,..} if self.intent.site(&p.origin).is_some()&&n(node_id).is_some_and(|n|!forbidden(&n.label)&&bs::public_control(n)&&(opener(n)||self.intent.link_score(p,n)>0||self.search_button_allowed(p,n)||(next_page(n)&&result_page(p)&&self.intent.site(&p.origin).is_some_and(|s|matches!(self.states[s].expected,Some(Expected::Pagination{..})))&&!product_links(p).is_empty())))=>Ok(()),
   BrowserAction::PressKey{node_id,key} if key=="Enter"&&n(node_id).is_some_and(|n|input(n)&&n.search_state.as_deref()==Some("filled")&&self.intent.site(&p.origin).is_some_and(|s|self.states[s].typed.as_ref().is_some_and(|(o,l,q)|o==&p.origin&&l==&n.label&&self.intent.query_allowed(q))))=>Ok(()),
   BrowserAction::SetChecked{node_id,checked:true,..} if self.intent.site(&p.origin).is_some_and(|s|self.states[s].submitted)&&n(node_id).is_some_and(|n|self.intent.chip_filter(p,n))=>Ok(()),
   BrowserAction::Complete{..} if self.ready()=>Ok(()),
   _=>Err("Action does not serve the bounded read-only catalog objective")
  }
 }
 pub fn partial_report(&self,sources:&[SourceEvidence])->VerificationReport{
  let mut findings=Vec::new();let mut summary=String::new();
  for site in &self.intent.sites{
   let name=host(site).unwrap_or_default();
   if let Some(p)=self.products.get(site).filter(|p|self.intent.site(&p.url)==Some(*site)&&self.intent.complete_specs(&p.heading)&&price_value(&p.price)==Some(p.amount)&&sources.iter().any(|s|s.url==p.url&&bs::verification::quote_present(s,&p.quote))){
    let claim=format!("{name}: {} ? displayed price {}",p.heading,p.price);
    summary.push_str(&format!("{claim}\nSource: {}\n",p.url));findings.push(GroundedFinding{claim,url:p.url.clone(),quote:p.quote.clone()});
   }else{summary.push_str(&format!("{name}: matching product and price not verified.\n"));}
  }
  summary.push_str("No complete price comparison was verified.");
  VerificationReport{status:"partial_evidence",summary,findings,limitations:"Missing evidence is not proof of absence or stock status. Search results are discovery hints only. No transaction performed.".into()}
 }
 pub fn report(&self,sources:&[SourceEvidence])->Result<VerificationReport,String>{
  if !self.ready(){return Err("Both retailers need independently observed matching products".into());}
  let mut findings=Vec::new();let mut summary=String::new();
  for site in &self.intent.sites{
   let p=self.products.get(site).ok_or("Missing product")?;
   if !self.intent.complete_specs(&p.heading)||price_value(&p.price)!=Some(p.amount)||!sources.iter().any(|s|s.url==p.url&&bs::verification::quote_present(s,&p.quote)){return Err("Retained product evidence is incomplete".into());}
   let claim=format!("{}: {} ? displayed price {}",host(site).unwrap_or_default(),p.heading,p.price);
   summary.push_str(&format!("{claim}\nSource: {}\n",p.url));
   findings.push(GroundedFinding{claim,url:p.url.clone(),quote:p.quote.clone()});
  }
  let a=&self.products[self.intent.sites[0]];let b=&self.products[self.intent.sites[1]];
  summary.push_str(&format!("Displayed price difference: INR {:.2}.",a.amount.abs_diff(b.amount) as f64/100.0));
  Ok(VerificationReport{status:"evidence_checked",summary,findings,limitations:"Observed public product heading and scoped displayed selling price only. Stock, delivery, discounts, final checkout price and seller claims are not independently verified. No transaction performed.".into()})
 }
}
pub fn entry_allowed(goal:&str,url:&str)->bool{Intent::compile(goal).is_some_and(|i|i.entry(url))}
pub fn discovery_navigation_allowed(goal:&str,from:&str,to:&str)->bool{Intent::compile(goal).is_some_and(|i|i.discovery_target(from,to))}
#[cfg(test)]
mod tests{
 use super::*;
 use super::super::dom_extractor::{PageStructure,ProductObservation};
 const G:&str="Research the Apple MacBook Air 13-inch with M4 chip, 16 GB memory and 256 GB SSD on Croma and Reliance Digital. Find matching public product listings, compare their listed Indian rupee prices and confirm that the chip, memory and storage match. Cite both product pages and their observed price evidence. If unavailable, report the limitation. Do not perform any transaction.";
 fn page(url:&str,heading:&str,price:&str)->DomSnapshot{
  let ctx=format!("{heading} {price}");
  DomSnapshot{origin:url.into(),title:heading.into(),text_excerpt:format!("{ctx} Public product page with technical specifications."),interactive_nodes:vec![],structure:PageStructure{product:Some(ProductObservation{heading:heading.into(),price:price.into(),context:ctx}),..Default::default()}}
 }
 fn source(p:&DomSnapshot)->SourceEvidence{SourceEvidence{url:p.origin.clone(),title:p.title.clone(),excerpt:p.text_excerpt.clone()}}
 #[test]fn macbook_is_not_booking_and_exact_retailer_scope(){
  let i=Intent::compile(G).unwrap();assert_eq!(i.sites.len(),2);assert!(i.entry(RETAILERS[1].1));
  assert!(!i.entry("https://www.reliancedigital.in.evil.example/"));
  assert!(Intent::compile(&G.replace("Do not perform any transaction.","Buy the cheapest one.")).is_none());
  assert!(!super::super::deliberation::read_only("Find hotels and book one."));
 }
 #[test]fn reject_wrong_product_and_contradictory_specs(){
  let i=Intent::compile(G).unwrap();let title="Apple MacBook Air 13.6 inch M4 16GB RAM 256GB SSD";
  assert!(i.complete_specs(title));assert!(!i.complete_specs(&title.replace("M4","M2")));assert!(!i.complete_specs(&title.replace("16GB","116GB")));
  assert!(!i.complete_specs(&title.replace("13.6","15.3")));assert!(!i.complete_specs("Croma television 16GB 256GB M4"));
  assert!(!i.complete_specs(&format!("{title} 512GB")));
 }
 #[test]fn two_separate_retailers_and_exact_scoped_price_are_required(){
  let mut r=Run::new(G).unwrap();
  let a=page("https://www.croma.com/macbook/p/1","Apple MacBook Air 13.6 inch M4 16GB RAM 256GB SSD","\u{20b9}89,990.00");
  let b=page("https://www.reliancedigital.in/macbook/p/2","Apple MacBook Air 13-inch M4 16 GB RAM 256 GB SSD","\u{20b9}85,000.00");
  let sources=vec![source(&a),source(&b)];
  assert!(matches!(r.next(&a,&sources),Next::Action(BrowserAction::Navigate{url},_) if url==RETAILERS[1].1));
  assert!(r.report(&sources).is_err());
  assert!(matches!(r.next(&b,&sources),Next::Action(BrowserAction::Complete{..},_)));
  assert_eq!(r.report(&sources).unwrap().findings.len(),2);
  assert!(r.report(&sources[..1]).is_err());
 }
 #[test]fn custom_search_combobox_and_filter_input_are_distinct(){
  use super::super::dom_extractor::DomNodeSummary;
  let mut r=Run::new(G).unwrap();
  let mut p=page(RETAILERS[0].1,"Public retailer home page with no product","INR 10000");
  let node=|id:&str,tag:&str,role:&str,label:&str,search:Option<&str>|DomNodeSummary{node_id:id.into(),tag:tag.into(),role:role.into(),label:label.into(),options:vec![],checked:None,search_state:search.map(str::to_owned),href:None,is_submit:false,is_sensitive:false,bbox:None};
  p.interactive_nodes=vec![node("open","div","combobox","search",None),node("facet","input","input","Search Category",Some("empty"))];
  assert!(matches!(r.next(&p,&[]),Next::Action(BrowserAction::Click{node_id,..},_) if node_id=="open"));
  assert!(r.check(&p,&BrowserAction::Click{node_id:"open".into(),element_label:"search".into(),is_submit:false}).is_ok());
  assert!(r.check(&p,&BrowserAction::TypeText{node_id:"facet".into(),text:r.intent.query.clone(),is_sensitive:false}).is_err());
 }
 #[test]fn exact_processor_filter_is_bounded_and_rejects_other_models(){
  let mut r=Run::new(G).unwrap();
  let mut p=page("https://www.croma.com/search?q=macbook","Public results","INR 10000");
  p.structure.product=None;
  let node=|id:&str,label:&str|DomNodeSummary{node_id:id.into(),tag:"div".into(),role:"checkbox".into(),label:label.into(),options:vec![],checked:Some(false),search_state:None,href:None,is_submit:false,is_sensitive:false,bbox:None};
  p.interactive_nodes=vec![node("wrong","Apple M5 chip"),node("right","Apple M4 chip"),node("consent","Accept cookies")];
  let state=r.states.get_mut(RETAILERS[0].1).unwrap();state.submitted=true;state.waiting=5;
  let a=match r.next(&p,&[]){Next::Action(a,_)=>a,_=>panic!("expected exact chip filter")};
  assert!(matches!(&a,BrowserAction::SetChecked{node_id,checked:true,..} if node_id=="right"));
  assert!(r.check(&p,&a).is_ok());
  assert!(r.check(&p,&BrowserAction::SetChecked{node_id:"wrong".into(),checked:true,element_label:"Apple M5 chip".into()}).is_err());
  assert!(r.check(&p,&BrowserAction::SetChecked{node_id:"consent".into(),checked:true,element_label:"Accept cookies".into()}).is_err());
  assert!(!matches!(r.next(&p,&[]),Next::Action(BrowserAction::SetChecked{..},_)));
 }
 #[test]fn alternate_category_links_are_grounded_and_do_not_license_other_products(){
  let mut r=Run::new(G).unwrap();
  let mut p=page(RETAILERS[0].1,"Public retailer category","INR 10000");
  p.structure.product=None;
  let node=|id:&str,label:&str,url:&str|DomNodeSummary{node_id:id.into(),tag:"a".into(),role:"link".into(),label:label.into(),options:vec![],checked:None,search_state:None,href:Some(url.into()),is_submit:false,is_sensitive:false,bbox:None};
  p.interactive_nodes=vec![node("good","MacBook Air","https://www.croma.com/category/macbook-air"),node("wrong","Apple M5 MacBook Air","https://www.croma.com/category/apple-m5"),node("external","MacBook Air","https://other.example/category/macbook-air")];
  let state=r.states.get_mut(RETAILERS[0].1).unwrap();state.submitted=true;state.short_query=true;state.broad_query=true;state.waiting=5;
  let action=match r.next(&p,&[]){Next::Action(a,_)=>a,_=>panic!("expected observed category")};
  assert!(matches!(&action,BrowserAction::Navigate{url} if url.ends_with("/category/macbook-air")));
  assert!(r.check(&p,&action).is_ok());
  assert!(r.check(&p,&BrowserAction::Navigate{url:"https://www.croma.com/category/apple-m5".into()}).is_err());
  assert!(r.check(&p,&BrowserAction::Navigate{url:"https://other.example/category/macbook-air".into()}).is_err());
  assert!(!matches!(r.next(&p,&[]),Next::Action(BrowserAction::Navigate{..},_)));
 }
 #[test]fn observed_exact_brand_store_is_a_waypoint_not_product_evidence(){
  let r=Run::new(G).unwrap();
  let mut p=page(RETAILERS[0].1,"Public retailer home","INR 10000");
  p.structure.product=None;
  let node=|id:&str,url:&str|DomNodeSummary{node_id:id.into(),tag:"a".into(),role:"a".into(),label:"undefined".into(),options:vec![],checked:None,search_state:None,href:Some(url.into()),is_sensitive:false,is_submit:false,bbox:None};
  p.interactive_nodes=vec![node("brand","https://www.croma.com/apple-store/b/b-0025"),node("wrong","https://www.croma.com/samsung-store/b/b-0025"),node("external","https://other.example/apple-store/b/b-0025")];
  assert_eq!(r.intent.category_score(&p,&p.interactive_nodes[0]),30);
  assert_eq!(r.intent.link_score(&p,&p.interactive_nodes[0]),0);
  assert_eq!(r.intent.category_score(&p,&p.interactive_nodes[1]),0);
  assert_eq!(r.intent.category_score(&p,&p.interactive_nodes[2]),0);
  assert!(r.check(&p,&BrowserAction::Navigate{url:p.interactive_nodes[0].href.clone().unwrap()}).is_ok());
  assert!(r.report(&[]).is_err());
  let category=node("macbooks","https://www.croma.com/computers-tablets/laptops/macbooks/c/906");
  assert_eq!(r.intent.category_score(&p,&category),0);
  p.origin=p.interactive_nodes[0].href.clone().unwrap();
  assert_eq!(r.intent.category_score(&p,&category),45);
  assert_eq!(r.intent.link_score(&p,&category),0);
  assert_eq!(r.intent.category_score(&p,&node("accessory","https://www.croma.com/macbooks/accessories/c/12")),0);
 }
 #[test]fn asynchronous_filter_receipt_is_reconciled_without_duplicate_alias(){
  let mut r=Run::new(G).unwrap();let mut p=page("https://www.croma.com/search?q=macbook","Public results","INR 10000");p.structure.product=None;
  let node=|id:&str,label:&str,checked:Option<bool>|DomNodeSummary{node_id:id.into(),tag:"div".into(),role:"checkbox".into(),label:label.into(),options:vec![],checked,search_state:None,href:None,is_submit:false,is_sensitive:false,bbox:None};
  p.interactive_nodes=vec![node("first","Apple M4 chip",Some(false)),node("alias","M4",Some(false))];
  let state=r.states.get_mut(RETAILERS[0].1).unwrap();state.submitted=true;state.waiting=5;
  assert!(matches!(r.next(&p,&[]),Next::Action(BrowserAction::SetChecked{node_id,..},_) if node_id=="first"));
  assert!(matches!(r.next(&p,&[]),Next::Wait));
  p.origin.push_str("&processor=Apple%20M4%20chip");
  assert!(matches!(r.next(&p,&[]),Next::Wait));
  p.interactive_nodes.push(node("remove","remove Apple M4 chip filter",None));
  let next=r.next(&p,&[]);
  assert!(r.states[RETAILERS[0].1].filtered);
  assert!(!matches!(next,Next::Action(BrowserAction::SetChecked{..},_)));
  assert_eq!(r.states[RETAILERS[0].1].strategy.outcomes()[0].outcome,StrategyOutcome::Confirmed);
 }
 #[test]fn failed_search_promotes_observed_alternative_without_restart(){
  let mut r=Run::new(G).unwrap();let mut p=page(RETAILERS[0].1,"Public retailer category","INR 10000");p.structure.product=None;
  p.interactive_nodes=vec![DomNodeSummary{node_id:"family".into(),tag:"a".into(),role:"link".into(),label:"MacBook Air".into(),options:vec![],checked:None,search_state:None,href:Some("https://www.croma.com/category/macbook-air".into()),is_submit:false,is_sensitive:false,bbox:None}];
  let state=r.states.get_mut(RETAILERS[0].1).unwrap();
  state.submitted=true;state.waiting=5;
  assert!(state.strategy.start("search".into(),StrategyKind::Search));state.strategy.observe(false,1);
  assert!(matches!(r.next(&p,&[]),Next::Action(BrowserAction::Navigate{url},"catalog:adaptive_category_recovery") if url.ends_with("macbook-air")));
  assert!(matches!(r.next(&p,&[]),Next::Wait));
 }
 #[test]fn per_source_observation_budget_protects_other_retailer(){
  let mut r=Run::new(G).unwrap();let mut p=page(RETAILERS[0].1,"Public retailer home","INR 10000");p.structure.product=None;
  r.states.get_mut(RETAILERS[0].1).unwrap().observations=60;
  assert!(matches!(r.next(&p,&[]),Next::Wait));
  assert!(matches!(r.next(&p,&[]),Next::Action(BrowserAction::Navigate{url},_) if url==RETAILERS[1].1));
 }
 #[test]fn search_discovery_is_scoped_and_partial_is_not_success(){
  let mut r=Run::new(G).unwrap();let site=RETAILERS[0].1;
  let url=r.intent.discovery_url(site);
  assert!(r.intent.entry(&url));assert_eq!(r.intent.discovery_site(&url),Some(site));
  let mut search=page(&url,"Public search results","INR 10000");search.structure.product=None;
  let node=|id:&str,url:&str,label:&str|DomNodeSummary{node_id:id.into(),tag:"a".into(),role:"link".into(),label:label.into(),options:vec![],checked:None,search_state:None,href:Some(url.into()),is_sensitive:false,is_submit:false,bbox:None};
  let candidate="https://www.croma.com/apple-macbook-air-m4-16gb-256gb/p/123";
  search.interactive_nodes=vec![node("wrong","https://www.croma.com/apple-macbook-air-m5/p/1","Apple MacBook Air M5"),node("right",candidate,"Apple MacBook Air M4 16GB 256GB")];
  r.states.get_mut(site).unwrap().discovery_started=true;
  let next=r.next(&search,&[]);
  assert!(matches!(&next,Next::Action(BrowserAction::Navigate{url}, "catalog:discovered_product") if url==candidate));
  assert!(r.check(&search,&BrowserAction::Navigate{url:candidate.into()}).is_ok());
  assert!(r.check(&search,&BrowserAction::Navigate{url:"https://www.croma.com/apple-macbook-air-m4/p/999".into()}).is_err());
  assert_eq!(r.states[site].actions,1);assert_eq!(r.states[site].observations,1);
  assert!(matches!(r.next(&search,&[]),Next::Wait));assert_eq!(r.states[site].actions,1);assert_eq!(r.states[site].observations,2);
  assert!(!r.intent.entry("https://www.google.com/search?q=private"));
  let target="https://www.croma.com/apple-macbook-air-m4-16gb-256gb/p/123";
  assert!(discovery_navigation_allowed(G,&url,target));
  assert!(!discovery_navigation_allowed(G,&url,"https://www.reliancedigital.in/product/apple-macbook-air-m4"));
  assert!(!discovery_navigation_allowed(G,&url,"https://www.croma.com/apple-macbook-air-m5/p/123"));
  let a=page(target,"Apple MacBook Air 13.6 inch M4 16GB RAM 256GB SSD","INR 89990");
  let sources=vec![source(&a)];let _=r.next(&a,&sources);
  let report=r.partial_report(&sources);
  assert_eq!(report.status,"partial_evidence");assert_eq!(report.findings.len(),1);
  assert!(report.summary.contains("reliancedigital.in: matching product and price not verified"));
  assert!(r.report(&sources).is_err());assert!(r.partial_report(&[]).findings.is_empty());
 }
 #[test]fn pagination_requires_changed_results_and_preserved_query(){
  let mut a=page("https://www.croma.com/search?q=macbook&page=1&processor=M4","Public catalog","INR 10000");
  let mut b=a.clone();b.origin="https://www.croma.com/search?q=macbook&page=2&processor=M4".into();
  let node=|id:&str,url:&str|DomNodeSummary{node_id:id.into(),tag:"a".into(),role:"link".into(),label:"product".into(),options:vec![],checked:None,search_state:None,href:Some(url.into()),is_sensitive:false,is_submit:false,bbox:None};
  a.interactive_nodes.push(node("a","https://www.croma.com/a/p/1"));
  b.interactive_nodes.push(node("b","https://www.croma.com/b/p/2"));
  let e=Expected::Pagination{scope:pagination_scope(&a),links:product_links(&a)};
  assert!(e.confirmed(&b));assert!(!e.confirmed(&a));
  b.origin="https://www.croma.com/search?q=macbook&page=2&processor=M5".into();assert!(!e.confirmed(&b));
 }
 #[test]fn no_unsupported_price_or_unrelated_action(){
  let mut r=Run::new(G).unwrap();
  let p=page("https://www.croma.com/macbook/p/1","Apple MacBook Air 13.6 inch M4 16GB RAM 256GB SSD","\u{20b9}89,990.00");
  assert!(product(&r.intent,&p,&[]).is_none());
  assert!(r.check(&p,&BrowserAction::Navigate{url:"https://www.croma.com/tv/p/1".into()}).is_err());
  assert!(r.check(&p,&BrowserAction::Complete{summary:"done".into()}).is_err());
  let bad=page(&p.origin,&p.title,"MRP \u{20b9}99,999");
  assert!(product(&r.intent,&bad,&[source(&bad)]).is_none());
  let _=r.next(&bad,&[source(&bad)]);
 }
}
