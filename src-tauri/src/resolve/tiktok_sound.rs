//! TikTok "original sound" row: scraped from the page's rehydration JSON, with a yt-dlp fallback.

use std::path::Path;

use once_cell::sync::Lazy;
use regex::{Regex, RegexBuilder};
use serde_json::Value;

use crate::model::SoundMeta;

static SCRIPT: Lazy<Regex> = Lazy::new(|| {
    RegexBuilder::new(r#"<script\b[^>]*\bid\s*=\s*["']__UNIVERSAL_DATA_FOR_REHYDRATION__["'][^>]*>(.*?)</script>"#)
        .dot_matches_new_line(true)
        .build()
        .expect("valid regex")
});

fn non_empty(v: Option<&Value>) -> Option<String> {
    v.and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned)
}

/// Extracts the sound from a TikTok page's `__UNIVERSAL_DATA_FOR_REHYDRATION__` script.
pub fn parse_tiktok_sound(html: &str) -> Option<SoundMeta> {
    let json = SCRIPT.captures(html)?.get(1)?.as_str();
    let root: Value = serde_json::from_str(json.trim()).ok()?;
    let music = root
        .pointer("/__DEFAULT_SCOPE__/webapp.video-detail/itemInfo/itemStruct/music")?;
    let play_url = non_empty(music.get("playUrl"))?;
    Some(SoundMeta {
        title: non_empty(music.get("title")).unwrap_or_else(|| "Original sound".into()),
        artist: non_empty(music.get("authorName")).unwrap_or_default(),
        cover: non_empty(music.get("coverLarge")).or_else(|| non_empty(music.get("coverMedium"))),
        play_url,
    })
}

/// Builds a `Cookie` header from a Netscape cookie jar, keeping only TikTok cookies.
fn cookie_header(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let pairs: Vec<String> = text
        .lines()
        .map(|l| l.strip_prefix("#HttpOnly_").unwrap_or(l))
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            (f.len() >= 7 && f[0].contains("tiktok.com")).then(|| format!("{}={}", f[5], f[6].trim()))
        })
        .collect();
    (!pairs.is_empty()).then(|| pairs.join("; "))
}

/// Fetches the page (following `vt.`/`vm.` redirects) and scrapes the sound. Any failure is `None`.
pub async fn fetch_tiktok_sound(url: &str, cookies: Option<&Path>) -> Option<SoundMeta> {
    let client = crate::thumb::browser_client().ok()?;
    let mut req = client.get(url).header(reqwest::header::REFERER, "https://www.tiktok.com/");
    if let Some(c) = cookies.and_then(cookie_header) {
        req = req.header(reqwest::header::COOKIE, c);
    }
    let html = req.send().await.ok()?.error_for_status().ok()?.text().await.ok()?;
    parse_tiktok_sound(&html)
}

/// Fallback from yt-dlp's JSON: its `format_id == "audio"` format is the music track.
pub fn ytdlp_sound_fallback(raw_json: &str) -> Option<SoundMeta> {
    let root: Value = serde_json::from_str(raw_json).ok()?;
    let play_url = root
        .get("formats")?
        .as_array()?
        .iter()
        .find(|f| f.get("format_id").and_then(Value::as_str) == Some("audio"))
        .and_then(|f| non_empty(f.get("url")))?;
    let artist = root
        .get("artists")
        .and_then(Value::as_array)
        .and_then(|a| a.iter().find_map(|v| non_empty(Some(v))))
        .or_else(|| non_empty(root.get("artist")))
        .or_else(|| non_empty(root.get("creator")))
        .or_else(|| non_empty(root.get("uploader")))
        .unwrap_or_default();
    Some(SoundMeta {
        title: non_empty(root.get("track")).unwrap_or_else(|| "Original sound".into()),
        artist,
        cover: non_empty(root.get("thumbnail")),
        play_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Hand-crafted from the real page structure (trimmed); the live test covers real pages.
    const DATA: &str = include_str!("../../tests/fixtures/tiktok/universal_data.json");
    const YT_TIKTOK: &str = include_str!("../../tests/fixtures/ytdlp/tiktok_video.json");

    fn page(json: &str) -> String {
        format!(r#"<html><head><script id="__UNIVERSAL_DATA_FOR_REHYDRATION__" type="application/json" crossorigin="anonymous">{json}</script></head></html>"#)
    }

    #[test]
    fn parse_sound_from_fixture() {
        let s = parse_tiktok_sound(&page(DATA)).unwrap();
        assert_eq!(s.title, "original sound - scout2015");
        assert_eq!(s.artist, "scout2015");
        assert_eq!(s.play_url, "https://v77.tiktokcdn.com/obj/tos-useast2a-ve-2774/abc.mp3");
        assert_eq!(s.cover.as_deref(), Some("https://p16-sign.tiktokcdn.com/large.jpeg"));
    }

    #[test]
    fn parse_sound_missing_script_is_none() {
        assert!(parse_tiktok_sound("<html><body>nope</body></html>").is_none());
        assert!(parse_tiktok_sound(&page("not json")).is_none());
    }

    #[test]
    fn parse_sound_empty_playurl_is_none() {
        let json = DATA.replace("https://v77.tiktokcdn.com/obj/tos-useast2a-ve-2774/abc.mp3", "");
        assert!(parse_tiktok_sound(&page(&json)).is_none());
    }

    #[test]
    fn parse_sound_cover_falls_back_to_medium() {
        let json = DATA.replace("coverLarge", "coverXl");
        let s = parse_tiktok_sound(&page(&json)).unwrap();
        assert_eq!(s.cover.as_deref(), Some("https://p16-sign.tiktokcdn.com/medium.jpeg"));
    }

    #[test]
    fn fallback_picks_audio_format_and_track_artist() {
        let mut v: Value = serde_json::from_str(YT_TIKTOK).unwrap();
        v["track"] = "Cool Song".into();
        v["artists"] = serde_json::json!(["Some Artist", "Other"]);
        let s = ytdlp_sound_fallback(&v.to_string()).unwrap();
        assert_eq!(s.title, "Cool Song");
        assert_eq!(s.artist, "Some Artist");
        assert!(s.play_url.starts_with("https://v58.tiktokcdn.com/"));
        assert_eq!(s.cover, non_empty(v.get("thumbnail")));
    }

    #[test]
    fn fallback_defaults_title_and_uses_uploader() {
        let s = ytdlp_sound_fallback(YT_TIKTOK).unwrap();
        assert_eq!(s.title, "Original sound");
        assert_eq!(s.artist, "scout2015");
    }

    #[test]
    fn fallback_none_without_audio_format() {
        let json = r#"{"formats":[{"format_id":"h264_540p","url":"https://x/v.mp4"}],"uploader":"u"}"#;
        assert!(ytdlp_sound_fallback(json).is_none());
        assert!(ytdlp_sound_fallback("garbage").is_none());
    }
}
