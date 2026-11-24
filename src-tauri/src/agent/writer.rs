//! Local content generation is separate from browser-action JSON. The writer
//! gets only the user's request, never existing editor content or webpage instructions.
use super::dom_extractor::{DomSnapshot,DomNodeSummary};
use crate::router::model_router::RouterConfig;

pub fn requested(goal:&str)->bool {
    let g=goal.to_ascii_lowercase();
    (g.trim_start().starts_with("write ") || g.trim_start().starts_with("draft ")) && g.contains("story")
        && g.contains("editor") && g.contains("on this page")
}
pub fn may_replace(goal:&str)->bool {
    let g=goal.to_ascii_lowercase();
    g.contains("replace the demo") && !g.contains("do not replace") && !g.contains("don't replace")
}
pub fn editor(page:&DomSnapshot)->Option<&DomNodeSummary>{
    if page.text_excerpt.starts_with("Active dialog:") {return None;}
    let mut nodes=page.interactive_nodes.iter().filter(|n|
        !n.is_sensitive && !n.is_submit && n.search_state.is_none()
        && (n.tag=="textarea" || n.role=="textbox")
        && n.bbox.as_ref().is_some_and(|b| b.in_viewport && b.height>=72.0));
    let first=nodes.next()?;
    if nodes.next().is_none(){Some(first)}else{None}
}
fn requested_title(goal:&str)->Option<&str>{
    let lower=goal.to_ascii_lowercase();
    let start=lower.find("titled ")?+7;
    let end=lower[start..].find(" in the visible ").or_else(||lower[start..].find(" in this editor")).or_else(||lower[start..].find(" on this page"))?+start;
    goal.get(start..end).map(str::trim).filter(|s|!s.is_empty()&&s.chars().count()<=120)
}
fn word_range(goal:&str)->(usize,usize){
    let words:Vec<_>=goal.split_whitespace().collect();
    for w in words.windows(4){
        if w[1]=="to" && w[3].trim_matches(|c:char|!c.is_alphabetic()).eq_ignore_ascii_case("words"){
            if let (Ok(a),Ok(b))=(w[0].parse::<usize>(),w[2].parse::<usize>()){
                if a>=10&&b>=a&&b<=300{return(a,b)}
            }
        }
    }
    (20,300)
}
pub fn validate(goal:&str,text:&str)->Result<(),String>{
    let (min,max)=word_range(goal);
    let count=text.split_whitespace().count();
    let lower=text.trim().to_ascii_lowercase();
    if count<min || count>max || text.len()>12000 {
        return Err(format!("Draft must contain {min} to {max} words; generated {count}"));
    }
    if ["type \"", "type the ", "write a ", "write an ", "here is the requested", "{", " "]
        .iter().take(6).any(|p|lower.starts_with(p)){
        return Err("Writer returned instructions or a wrapper instead of prose".into());
    }
    if let Some(title)=requested_title(goal){
        if !text.lines().next().unwrap_or("").to_ascii_lowercase().contains(&title.to_ascii_lowercase()){
            return Err("Requested title missing from first line".into());
        }
    }
    Ok(())
}
pub async fn generate(config:&RouterConfig,goal:&str)->anyhow::Result<String>{
    let endpoint=url::Url::parse(&config.ollama_url)?;
    let model=config.writer_model.trim();
    anyhow::ensure!(!config.lucia_cloud_only && !model.is_empty() && !model.to_ascii_lowercase().contains("cloud")
        && endpoint.scheme()=="http" && matches!(endpoint.host_str(),Some("localhost"|"127.0.0.1"|"[::1]"))
        && endpoint.username().is_empty()&&endpoint.password().is_none(),"Local writer unavailable");
    let client=reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(35)).build()?;
    let url=format!("{}/api/generate",config.ollama_url.trim_end_matches('/'));
    let (min,max)=word_range(goal);
    let mut correction=String::new();
    let title=requested_title(goal).unwrap_or("Choose a suitable original title");
    let theme=goal.split(". ").filter(|sentence| {
        let g=sentence.trim().to_ascii_lowercase();
        g.starts_with("include ") || g.starts_with("the story should ") || g.starts_with("theme:")
    }).take(3).collect::<Vec<_>>().join(". ");
    let theme=theme.trim_end_matches('.');
    let target=(min+max)/2;
    let sentences=((target+5)/10).max(3);
    // Keep browser instructions out of the writing prompt. Only the explicit
    // writing brief, title, length and theme belong to this model.
    let brief=if requested_title(goal).is_some() { format!("Compose a finished story titled {title}.") }
        else { goal.split(" in the visible ").next().unwrap_or(goal).split(". ").next().unwrap_or(goal).chars().take(400).collect::<String>() };
    for _ in 0..3{
        let body=serde_json::json!({"model":model,"stream":false,"keep_alive":0,
            "system":"You write original fiction. Return only the story title on the first line, then the actual story. Never output instructions or discuss browser actions.",
            "prompt":format!("{brief} {theme}. Write {sentences} complete sentences in the story, approximately 10 words each. Target {target} words total. Begin immediately with the title. {correction}"),
            "options":{"temperature":0.5,"num_ctx":2048,"num_predict":512}});
        let val:serde_json::Value=client.post(&url).json(&body).send().await?.error_for_status()?.json().await?;
        let text=val["response"].as_str().unwrap_or("").trim().to_string();
        match validate(goal,&text){Ok(())=>return Ok(text),Err(e)=>correction=e}
    }
    anyhow::bail!("Writer failed draft validation after three attempts with {model}: {correction}")
}
/// Native readback checks the same live origin and exact editor value. Never
/// return existing text to Rust, the model, or logs; only booleans.
pub async fn readback(app:&tauri::AppHandle,label:&str,origin:&str,id:&str,expected:&str)->anyhow::Result<(bool,bool)>{
    let origin=serde_json::to_string(origin)?;
    let id=serde_json::to_string(id)?;
    let expected=serde_json::to_string(expected)?;
    let script=format!(r#"(function() {{
      if(location.href!=={origin}) return {{ok:false,empty:false,match:false}};
      const el=document.querySelector('[data-aether-id="'+CSS.escape({id})+'"]');
      if(!el||!el.isConnected||el.matches(':disabled')||el.readOnly||el.closest('form')||
         (!el.isContentEditable&&el.tagName!=='TEXTAREA')||el.getBoundingClientRect().height<72)
        return {{ok:false,empty:false,match:false}};
      const value=String(el.isContentEditable?el.textContent:el.value);
      return {{ok:true,empty:value.trim()==='',match:value==={expected}}};
    }})()"#);
    let result=super::native_eval::eval_json(app,label,script,1800).await.map_err(anyhow::Error::msg)?;
    anyhow::ensure!(result["ok"]==true,"Live editor unavailable or changed");
    Ok((result["empty"]==true,result["match"]==true))
}
#[cfg(test)]
mod tests{
 use super::*;
 #[test]fn rejects_action_instead_of_story(){
  let g="Write an original short story of 50 to 90 words titled The Lantern in the Library in the visible note editor on this page.";
  assert_eq!(requested_title(g),Some("The Lantern in the Library"));
  assert!(requested(g));assert!(validate(g,"Type the title into the editor.").is_err());
  assert!(validate(g,"Wrong title\nA short story").is_err());
  assert!(!may_replace(g));
  assert!(may_replace("Replace the demo welcome text"));
  assert!(!may_replace("Do not replace the demo welcome text"));
 }
}
