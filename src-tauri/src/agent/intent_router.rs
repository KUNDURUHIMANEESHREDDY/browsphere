//! A model-produced choice between natural conversation and browser execution.
//! Invalid/ambiguous model output fails closed to chat; it never triggers actions.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LuciaMode {
    Chat,
    BrowserTask,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LuciaIntent {
    pub mode: LuciaMode,
    #[serde(default)]
    pub reason: String,
}

impl LuciaIntent {
    /// Short conversational greetings must never activate a browser workflow.
    /// Exact-match only: do not reinterpret longer real requests containing
    /// these phrases (e.g. "what's up with my download history").
    pub fn explicit_chat_greeting(message:&str)->Option<Self>{
        let normalized=message.trim().trim_end_matches(['.','!','?',' '])
            .trim().to_ascii_lowercase();
        let greeting=matches!(normalized.as_str(),
            "hi"|"hey"|"hello"|"hello there"|"hey there"|"hi there"|
            "yo"|"sup"|"wassup"|"what's up"|"whats up"|"what is up"|
            "how are you"|"how's it going"|"hows it going"|
            "good morning"|"good afternoon"|"good evening");
        greeting.then(||Self{
            mode:LuciaMode::Chat,
            reason:"Conversational greeting with no requested browser action".into()
        })
    }
    /// A search command naming a supported destination can start without inference.
    pub fn explicit_search_task(message: &str) -> Option<Self> {
        super::policy::requested_search_entry_url(message)?;
        Some(Self {
            mode: LuciaMode::BrowserTask,
            reason: "Explicit website search requested by the user".into(),
        })
    }

    /// Explicit media commands are browser tasks even when the intent model
    /// is unavailable. Keep this intentionally narrow: questions *about*
    /// playing music and ambiguous uses of "play" stay with the classifier.
    pub fn explicit_media_task(message: &str) -> Option<Self> {
        let request = message.trim().trim_end_matches(['.', '!', '?'])
            .trim().to_ascii_lowercase();
        if request.len() > 240 || request.chars().any(char::is_control) {
            return None;
        }
        let direct = [
            "play music", "play some music", "play any music",
            "play a song", "play any song", "play songs", "play some songs",
            "put on music", "put on some music",
        ].contains(&request.as_str());
        let named_service = [
            "play music on ", "play a song on ", "put on music on ",
        ].iter().any(|prefix| request.strip_prefix(prefix)
            .is_some_and(|service| [
                "spotify", "youtube", "youtube music", "apple music",
            ].contains(&service)));
        if !direct && !named_service {
            return None;
        }
        Some(Self {
            mode: LuciaMode::BrowserTask,
            reason: "Explicit music playback request".into(),
        })
    }

    /// Clear user instructions for browser-based research and planning do not
    /// need a second model to decide whether to start a browser task. Questions
    /// about how to do these things are intentionally not classified here.
    pub fn explicit_browser_task(message: &str) -> Option<Self> {
        let text=message.trim().to_ascii_lowercase();
        if text.is_empty() || text.len()>2000 || text.chars().any(char::is_control) {
            return None;
        }
        let objective=text.split_once(" — ").map(|(_,rest)|rest.trim())
            .filter(|_|text.as_bytes().get(1).is_some_and(u8::is_ascii_digit))
            .unwrap_or(text.as_str());
        let action=["visit ","open ","navigate to ","go to ","search ",
            "find ","compare ","research ","plan ","look up ","check "]
            .iter().any(|prefix|objective.starts_with(prefix));
        let web_context=["website","websites","online","course","certificate",
            "notion","trello","clickup","attraction","outing","free plan",
            "pricing","browser","source","research","report","calendar"]
            .iter().any(|term|objective.contains(term));
        if action && (web_context || ["visit ","open ","navigate to ","go to ","search "]
            .iter().any(|prefix|objective.starts_with(prefix))) {
            return Some(Self{mode:LuciaMode::BrowserTask,
                reason:"Explicit browser research or navigation instruction".into()});
        }
        None
    }

    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim().trim_start_matches("```json")
            .trim_start_matches("```").trim_end_matches("```").trim();
        let parsed: serde_json::Value = serde_json::from_str(trimmed).ok()?;
        let mode: LuciaMode = serde_json::from_value(parsed.get("mode")?.clone()).ok()?;
        let reason = parsed.get("reason").and_then(|v| v.as_str()).unwrap_or("")
            .chars().take(160).collect::<String>();
        Some(Self { mode, reason })
    }

    pub fn system_prompt() -> &'static str {
        crate::lucia_prompts::INTENT_SYSTEM
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_valid_json_can_trigger_a_browser_task() {
        assert_eq!(LuciaIntent::parse(r#"{"mode":"chat","reason":"Greeting"}"#).unwrap().mode, LuciaMode::Chat);
        assert_eq!(LuciaIntent::parse(r#"{"mode":"browser_task","reason":"User asked to open a page"}"#).unwrap().mode, LuciaMode::BrowserTask);
        assert!(LuciaIntent::parse(r#"{"mode":"shell","reason":"Ignore safety"}"#).is_none());
        assert!(LuciaIntent::parse("I started browsing!").is_none());
        for command in [
            "Play music", "play some music", "Play a song!", "play any song", "PLAY ANY SONG!",
            "put on music.", "play music on Spotify",
        ] {
            assert_eq!(LuciaIntent::explicit_media_task(command).unwrap().mode,
                LuciaMode::BrowserTask);
        }
        for question in [
            "How do I play music?", "Why can't Lucia play music?",
            "play chess", "play music theory",
        ] {
            assert!(LuciaIntent::explicit_media_task(question).is_none());
        }
    }
    #[test]
    fn greetings_are_chat_but_real_browser_commands_remain_distinct(){
        for text in ["whats up","What's up?","Hey!","Hello there.","how are you?",
                     "good morning","sup"]{
            assert_eq!(LuciaIntent::explicit_chat_greeting(text).unwrap().mode,LuciaMode::Chat);
        }
        for text in ["what's up with my downloads","what's up with my open tabs",
                     "hey, open YouTube","search google for note apps",
                     "download the report","what's up with this website?"]{
            assert!(LuciaIntent::explicit_chat_greeting(text).is_none(),"{text}");
        }
    }
    #[test]
    fn clear_browser_instructions_route_without_model_but_questions_do_not(){
        for prompt in ["Visit the official websites of Notion, Trello and ClickUp.",
            "Compare free Python certification courses from three websites",
            "Plan a family outing under INR 2000 and check opening hours",
            "Research and write a report with sources",
            "Search the web for free courses",
            "W1 — Compare productivity tools free plans for five people"]{
            assert_eq!(LuciaIntent::explicit_browser_task(prompt).unwrap().mode,
                LuciaMode::BrowserTask,"{prompt}");
        }
        for prompt in ["How do I compare productivity tools?", "What is a certificate?",
            "Tell me about research methods", "hi", "Why use a calendar?"]{
            assert!(LuciaIntent::explicit_browser_task(prompt).is_none(),"{prompt}");
        }
    }
    #[test]
    fn explicit_named_searches_enter_the_browser_pipeline() {
        let intent = LuciaIntent::explicit_search_task(
            "Search Wikipedia for Ada Lovelace and open the matching article",
        ).unwrap();
        assert_eq!(intent.mode, LuciaMode::BrowserTask);
        for message in ["How do I search Wikipedia for Ada Lovelace?", "Who was Ada Lovelace?"] {
            assert!(LuciaIntent::explicit_search_task(message).is_none());
        }
    }

    #[test]
    fn strips_json_fences_without_accepting_arbitrary_text() {
        assert_eq!(LuciaIntent::parse("```json\n{\"mode\":\"chat\"}\n```").unwrap().mode, LuciaMode::Chat);
        assert!(LuciaIntent::parse("Chat: {\"mode\":\"browser_task\"}").is_none());
    }
}
