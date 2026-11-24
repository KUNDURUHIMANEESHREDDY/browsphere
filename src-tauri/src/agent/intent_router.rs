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
            "play a song", "play songs", "play some songs",
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
            "Play music", "play some music", "Play a song!",
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
