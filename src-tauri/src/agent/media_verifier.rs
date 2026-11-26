//! Independent native media-playback evidence for explicit music/video tasks.
//! The site never receives Tauri IPC. Do not log media URLs, player text,
//! typed values, or the precise playback position.
use serde::Deserialize;
use tauri::AppHandle;

#[derive(Debug, Deserialize)]
struct MediaReading {
    present: bool,
    paused: bool,
    ended: bool,
    ready: bool,
    position: f64,
    #[serde(default)]
    network_state: u16,
    #[serde(default)]
    error_code: u16,
}

fn confirmed_playback(first: &MediaReading, second: &MediaReading) -> bool {
    first.present && second.present && first.ready && second.ready
        && !first.paused && !second.paused && !second.ended
        && first.position.is_finite() && second.position.is_finite()
        && first.position >= 0.0
        && second.position > first.position + 0.20
        && second.position < first.position + 10.0
}

async fn read_media(app: &AppHandle, tab_label: &str) -> Result<MediaReading, String> {
    // Only a short, structured status is returned; never capture a media
    // source, title, cookies, audio samples, or full page content.
    let script = r#"(() => {
      const media = Array.from(document.querySelectorAll('video, audio'))
        .find(m => m instanceof HTMLMediaElement && !m.ended &&
          Number.isFinite(m.currentTime));
      if (!media) return {present:false,paused:true,ended:false,ready:false,position:0,network_state:0,error_code:0};
      return {present:true,paused:media.paused,ended:media.ended,
        ready:media.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA,
        position:media.currentTime,network_state:media.networkState,
        error_code:media.error?.code || 0};
    })()"#;
    let value = crate::agent::native_eval::eval_json(
        app, tab_label, script.into(), 1800
    ).await?;
    serde_json::from_value(value)
        .map_err(|_| "Native media probe did not return structured evidence".into())
}

/// A bounded, content-free diagnosis for recovering or stopping stalled media.
/// No player URL, title, media samples, or playback position is exposed.
fn playback_state(reading: &MediaReading) -> &'static str {
    if !reading.present { "no_media_element" }
    else if reading.error_code != 0 { "media_load_error" }
    else if reading.network_state == 3 { "media_source_unavailable" }
    else if !reading.ready { "media_buffering" }
    else if reading.paused { "media_paused" }
    else if reading.ended { "media_ended" }
    else { "media_ready_unverified" }
}

pub async fn diagnose_live_playback(app: &AppHandle, tab_label: &str) -> &'static str {
    match read_media(app,tab_label).await {
        Ok(reading) => playback_state(&reading),
        Err(_) => "media_probe_unavailable",
    }
}

/// This verifies actual advancing playback, not merely an opened YouTube URL.
/// It never starts media playback or bypasses user-gesture restrictions.
pub async fn verify_live_playback(
    app: &AppHandle, tab_label: &str, observed_url: &str,
) -> Result<(), String> {
    let url = url::Url::parse(observed_url)
        .map_err(|_| "No valid media page was observed")?;
    let host = url.host_str().unwrap_or_default();
    if url.scheme() != "https"
        || !matches!(host, "www.youtube.com" | "youtube.com" | "m.youtube.com")
        || !(url.path() == "/watch" && url.query_pairs().any(|(k,v)| k == "v" && !v.is_empty())
            || url.path().starts_with("/shorts/"))
    {
        return Err("A YouTube video page has not been observed".into());
    }
    let first = read_media(app, tab_label).await?;
    tokio::time::sleep(std::time::Duration::from_millis(900)).await;
    let second = read_media(app, tab_label).await?;
    if confirmed_playback(&first, &second) {
        Ok(())
    } else {
        Err(format!("YouTube playback is not verified ({}) - the media clock has not advanced. Opening a video URL is not enough.",playback_state(&second)))
    }
}

#[cfg(test)]
mod tests {
    use super::{confirmed_playback, playback_state, MediaReading};
    fn sample(position:f64) -> MediaReading {
        MediaReading {present:true,paused:false,ended:false,ready:true,position,network_state:1,error_code:0}
    }
    #[test]
    fn diagnosis_distinguishes_loading_from_paused_and_media_error() {
        let mut s=sample(0.0);
        s.ready=false;
        s.network_state=2;
        assert_eq!(playback_state(&s),"media_buffering");
        s.error_code=3;
        assert_eq!(playback_state(&s),"media_load_error");
        s.error_code=0;
        s.ready=true;
        s.paused=true;
        assert_eq!(playback_state(&s),"media_paused");
        s.paused=false;
        assert_eq!(playback_state(&s),"media_ready_unverified");
    }
    #[test]
    fn playback_requires_advancing_ready_media() {
        assert!(confirmed_playback(&sample(1.0), &sample(2.0)));
        assert!(!confirmed_playback(&sample(1.0), &sample(1.0)));
        assert!(!confirmed_playback(&sample(1.0), &sample(1.19)));
        assert!(!confirmed_playback(&sample(1.0), &sample(30.0)));
        let mut paused=sample(2.0);
        paused.paused=true;
        assert!(!confirmed_playback(&sample(1.0), &paused));
        let mut ended=sample(2.0);
        ended.ended=true;
        assert!(!confirmed_playback(&sample(1.0), &ended));
        let mut missing=sample(1.0);
        missing.present=false;
        assert!(!confirmed_playback(&missing, &sample(2.0)));
        let mut not_ready=sample(1.0);
        not_ready.ready=false;
        assert!(!confirmed_playback(&not_ready, &sample(2.0)));
        assert!(!confirmed_playback(&sample(f64::NAN), &sample(2.0)));
    }
}
