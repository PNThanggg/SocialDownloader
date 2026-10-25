pub mod cache;
pub mod formats;
pub mod gallerydl;
pub mod tiktok_sound;
pub mod ytdlp;

use once_cell::sync::Lazy;
use regex::{Regex, RegexBuilder};
use tauri::State;

use crate::accounts::cookie_file_for;
use crate::errors::{ErrorCode, LumiError, classify};
use crate::model::{Engine, MediaInfo, MediaOption, OptionKind, Site, SoundMeta};
use crate::site::detect_site;
use crate::tools::Tool;
use crate::tools::paths::tool_path;
use crate::tools::runner::run_tool;

pub use cache::ResolveCache;
use gallerydl::{gallerydl_error_text, gallerydl_resolve_args, parse_gallerydl_json, primary_engine};
use tiktok_sound::{fetch_tiktok_sound, ytdlp_sound_fallback};
use ytdlp::{needs_full_extraction, parse_ytdlp_json, ytdlp_resolve_args, ytdlp_resolve_args_full};

static NO_VIDEO: Lazy<Regex> = Lazy::new(|| {
    RegexBuilder::new(r"There is no video|No video formats found").case_insensitive(true).build().expect("valid regex")
});

/// Whether the other engine is worth a try after the primary failed.
fn should_fallback(code: &ErrorCode, stderr: &str) -> bool {
    matches!(code, ErrorCode::UnsupportedUrl | ErrorCode::Unknown) || NO_VIDEO.is_match(stderr)
}

fn other(engine: Engine) -> Engine {
    match engine {
        Engine::YtDlp => Engine::GalleryDl,
        Engine::GalleryDl => Engine::YtDlp,
    }
}

/// The parsed info plus, for yt-dlp, its raw JSON (the TikTok sound fallback reads it).
async fn run_engine(engine: Engine, site: Site, url: &str) -> Result<(MediaInfo, Option<String>), LumiError> {
    // Held until this function returns so the decrypted cookie file outlives the child process.
    let cookies = cookie_file_for(site)?;
    let cookie_path = cookies.as_ref().map(|c| c.0.as_path());
    let (tool, args) = match engine {
        Engine::YtDlp => (Tool::YtDlp, ytdlp_resolve_args(url, cookie_path)),
        Engine::GalleryDl => (Tool::GalleryDl, gallerydl_resolve_args(url, cookie_path)),
    };
    let mut out = run_tool(tool, &args).await?;
    if out.status == 0 && engine == Engine::YtDlp && needs_full_extraction(site, &out.stdout) {
        out = run_tool(tool, &ytdlp_resolve_args_full(url, cookie_path)).await?;
    }
    if out.status != 0 {
        return Err(LumiError::new(classify(site, &out.stderr), out.stderr));
    }
    let parsed = match engine {
        Engine::YtDlp => parse_ytdlp_json(site, &out.stdout, tool_path(Tool::Deno).exists()),
        Engine::GalleryDl => parse_gallerydl_json(site, url, &out.stdout),
    };
    let raw = (engine == Engine::YtDlp).then(|| out.stdout.clone());
    match parsed {
        Ok(info) => Ok((info, raw)),
        Err(code) => {
            // A clean exit with nothing usable: gallery-dl reports why in its `-j` output
            // (`[-1, {error, message}]`) and sometimes on stderr; both are the raw log.
            let reported = (engine == Engine::GalleryDl).then(|| gallerydl_error_text(&out.stdout)).flatten();
            let detail = [reported.as_deref(), Some(out.stderr.trim())]
                .into_iter()
                .flatten()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            let code = if code == ErrorCode::Unknown { classify(site, &detail) } else { code };
            Err(LumiError::new(code, detail))
        }
    }
}

/// Adds the "Original sound" row to item 0 and records the sound. No-op without an item 0.
fn attach_sound(info: &mut MediaInfo, sound: SoundMeta) {
    let Some(item) = info.items.first_mut() else { return };
    item.options.push(MediaOption {
        id: "sound".into(),
        kind: OptionKind::OriginalSound,
        label: if sound.artist.is_empty() { sound.title.clone() } else { format!("{} — {}", sound.title, sound.artist) },
        ext: "mp3".into(),
        filesize: None,
        direct_url: Some(sound.play_url.clone()),
        note: None,
    });
    info.sound = Some(sound);
}

/// Resolves `url` with the site's primary engine, trying the other engine once on a soft failure.
/// The result is cached under both the trimmed input URL and `info.source_url`.
pub async fn resolve_url(url: &str, cache: &ResolveCache) -> Result<MediaInfo, LumiError> {
    resolve_inner(url, cache, |u: String| async move {
        // Decrypted only on this rare path; the file is deleted when `cookies` drops.
        let cookies = cookie_file_for(Site::Tiktok).ok().flatten();
        fetch_tiktok_sound(&u, cookies.as_ref().map(|c| c.0.as_path())).await
    })
    .await
}

/// TikTok sound: the yt-dlp `audio` format first (free); `scrape` (the page fetch) only when that yields nothing.
/// It is injected so offline tests stay deterministic.
async fn resolve_inner<F, Fut>(url: &str, cache: &ResolveCache, scrape: F) -> Result<MediaInfo, LumiError>
where
    F: FnOnce(String) -> Fut,
    Fut: std::future::Future<Output = Option<SoundMeta>>,
{
    let url = url.trim();
    let site = detect_site(url)
        .ok_or_else(|| LumiError::new(ErrorCode::UnsupportedUrl, format!("not an http(s) URL: {url}")))?;
    let primary = primary_engine(site, url);
    let (mut info, raw) = match run_engine(primary, site, url).await {
        Ok(info) => info,
        Err(first) if should_fallback(&first.error, &first.detail) => {
            // Both failed: the primary engine's error is the more meaningful one.
            run_engine(other(primary), site, url).await.map_err(|_| first)?
        }
        Err(e) => return Err(e),
    };
    if info.source_url.is_empty() {
        info.source_url = url.to_owned();
    }
    if site == Site::Tiktok {
        let sound = match raw.as_deref().and_then(ytdlp_sound_fallback) {
            Some(s) => Some(s),
            None => scrape(url.to_owned()).await,
        };
        if let Some(sound) = sound {
            attach_sound(&mut info, sound);
        }
    }
    cache.insert(url, info.clone());
    cache.insert(info.source_url.clone(), info.clone());
    Ok(info)
}

#[tauri::command]
pub async fn resolve(cache: State<'_, ResolveCache>, url: String) -> Result<MediaInfo, LumiError> {
    resolve_url(&url, &cache).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::paths::TEST_ENV_LOCK;
    use std::path::Path;

    const YT_TIKTOK: &str = include_str!("../../tests/fixtures/ytdlp/tiktok_video.json");
    const GDL_IG: &str = include_str!("../../tests/fixtures/gallerydl/instagram_carousel_mixed.json");

    /// A fake `<name>.cmd` that prints `stdout` (from a sidecar file) and `stderr`, then exits with `code`.
    fn fake_tool(dir: &Path, name: &str, stdout: &str, stderr: &str, code: i32) {
        std::fs::write(dir.join(format!("{name}.out")), stdout).unwrap();
        std::fs::write(dir.join(format!("{name}.err")), stderr).unwrap();
        let script = format!("@echo off\r\ntype \"%~dp0{name}.out\"\r\ntype \"%~dp0{name}.err\" 1>&2\r\nexit /b {code}\r\n");
        std::fs::write(dir.join(format!("{name}.cmd")), script).unwrap();
    }

    async fn no_scrape(_: String) -> Option<SoundMeta> {
        None
    }

    fn with_bin_dir() -> (std::sync::MutexGuard<'static, ()>, tempfile::TempDir) {
        let g = TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("LUMI_BIN_DIR", dir.path()) };
        (g, dir)
    }

    #[test]
    fn fallback_on_no_video() {
        assert!(should_fallback(&ErrorCode::Unknown, "There is no video in this post"));
        assert!(should_fallback(&ErrorCode::ExtractorBroken, "ERROR: [x] No video formats found!"));
        assert!(should_fallback(&ErrorCode::UnsupportedUrl, ""));
        assert!(should_fallback(&ErrorCode::Unknown, ""));
        assert!(!should_fallback(&ErrorCode::LoginRequired { site: Site::Instagram }, "login required"));
        assert!(!should_fallback(&ErrorCode::PlaylistNotSupported, ""));
        assert!(!should_fallback(&ErrorCode::ToolMissing { tool: Tool::YtDlp }, ""));
    }

    #[tokio::test]
    async fn rejects_non_urls() {
        let err = resolve_url("  not a url ", &ResolveCache::default()).await.unwrap_err();
        assert_eq!(err.error, ErrorCode::UnsupportedUrl);
    }

    #[tokio::test]
    async fn primary_success_fills_source_url_and_caches_both_keys() {
        let (_g, dir) = with_bin_dir();
        let mut json: serde_json::Value = serde_json::from_str(YT_TIKTOK).unwrap();
        json.as_object_mut().unwrap().remove("webpage_url");
        json.as_object_mut().unwrap().remove("original_url");
        fake_tool(dir.path(), "yt-dlp", &json.to_string(), "", 0);
        let cache = ResolveCache::default();
        let input = "https://vt.tiktok.com/ZSabc/";
        let info = resolve_url(&format!("  {input}\n"), &cache).await.unwrap();
        unsafe { std::env::remove_var("LUMI_BIN_DIR") };
        assert_eq!(info.engine, Engine::YtDlp);
        assert_eq!(info.source_url, input);
        assert_eq!(cache.get(input), Some(info));
    }

    #[tokio::test]
    async fn cache_also_keyed_by_reported_source_url() {
        let (_g, dir) = with_bin_dir();
        fake_tool(dir.path(), "yt-dlp", YT_TIKTOK, "", 0);
        let cache = ResolveCache::default();
        let info = resolve_inner("https://vt.tiktok.com/ZSabc/", &cache, no_scrape).await.unwrap();
        unsafe { std::env::remove_var("LUMI_BIN_DIR") };
        assert_eq!(info.source_url, "https://www.tiktok.com/@scout2015/video/6718335390845095173");
        assert_eq!(cache.get(&info.source_url), Some(info.clone()));
        assert!(cache.get("https://vt.tiktok.com/ZSabc/").is_some());
    }

    #[tokio::test]
    async fn tiktok_appends_sound_option_to_item_zero_from_ytdlp_fallback() {
        let (_g, dir) = with_bin_dir();
        let mut json: serde_json::Value = serde_json::from_str(YT_TIKTOK).unwrap();
        json["track"] = "Cool Song".into();
        json["artists"] = serde_json::json!(["Some Artist"]);
        fake_tool(dir.path(), "yt-dlp", &json.to_string(), "", 0);
        let info = resolve_inner("https://www.tiktok.com/@a/video/1", &ResolveCache::default(), no_scrape).await.unwrap();
        unsafe { std::env::remove_var("LUMI_BIN_DIR") };
        let opt = info.items[0].options.last().unwrap();
        assert_eq!(opt.id, "sound");
        assert_eq!(opt.kind, OptionKind::OriginalSound);
        assert_eq!(opt.label, "Cool Song — Some Artist");
        assert_eq!(opt.ext, "mp3");
        assert_eq!(opt.filesize, None);
        assert!(opt.direct_url.as_deref().unwrap().starts_with("https://v58.tiktokcdn.com/"));
        assert_eq!(info.sound.unwrap().play_url, *opt.direct_url.as_ref().unwrap());
    }

    #[tokio::test]
    async fn ytdlp_sound_wins_and_scrape_is_only_a_fallback() {
        let other = || SoundMeta { title: "Scraped".into(), artist: "".into(), cover: None, play_url: "https://s/p.mp3".into() };
        // yt-dlp has an audio format: the injected scraper must not be used.
        let (_g, dir) = with_bin_dir();
        fake_tool(dir.path(), "yt-dlp", YT_TIKTOK, "", 0);
        let info = resolve_inner("https://www.tiktok.com/@a/video/1", &ResolveCache::default(), |_| async move {
            panic!("scrape must not run when yt-dlp has the audio format")
        })
        .await
        .unwrap();
        assert!(info.sound.unwrap().play_url.starts_with("https://v58.tiktokcdn.com/"));
        // No audio format: the scraper's result is used (empty artist: no separator in the label).
        let mut json: serde_json::Value = serde_json::from_str(YT_TIKTOK).unwrap();
        json["formats"].as_array_mut().unwrap().retain(|f| f["format_id"] != "audio");
        fake_tool(dir.path(), "yt-dlp", &json.to_string(), "", 0);
        let s = other();
        let info = resolve_inner("https://www.tiktok.com/@a/video/1", &ResolveCache::default(), |_| async move { Some(s) })
            .await
            .unwrap();
        unsafe { std::env::remove_var("LUMI_BIN_DIR") };
        assert_eq!(info.items[0].options.last().unwrap().label, "Scraped");
        assert_eq!(info.sound.unwrap().play_url, "https://s/p.mp3");
    }

    #[tokio::test]
    async fn non_tiktok_gets_no_sound_option() {
        let (_g, dir) = with_bin_dir();
        // Same payload (has an "audio" format) but reached via a YouTube URL.
        fake_tool(dir.path(), "yt-dlp", YT_TIKTOK, "", 0);
        let info = resolve_inner("https://www.youtube.com/watch?v=x", &ResolveCache::default(), no_scrape).await.unwrap();
        unsafe { std::env::remove_var("LUMI_BIN_DIR") };
        assert!(info.sound.is_none());
        assert!(info.items[0].options.iter().all(|o| o.kind != OptionKind::OriginalSound));
    }

    #[tokio::test]
    async fn falls_back_once_to_the_other_engine() {
        let (_g, dir) = with_bin_dir();
        fake_tool(dir.path(), "gallery-dl", "[]", "", 0);
        fake_tool(dir.path(), "yt-dlp", YT_TIKTOK, "", 0);
        let info = resolve_url("https://www.instagram.com/reel/C8x/", &ResolveCache::default()).await.unwrap();
        unsafe { std::env::remove_var("LUMI_BIN_DIR") };
        assert_eq!(info.engine, Engine::YtDlp);
        assert_eq!(info.site, Site::Instagram);
    }

    /// A Facebook post whose flat result has only link entries is read again, fully.
    #[tokio::test]
    async fn facebook_link_entries_are_extracted_fully() {
        let (_g, dir) = with_bin_dir();
        let flat = r#"{"_type":"playlist","id":"p","title":"t","entries":[{"_type":"url","url":"https://www.facebook.com/x/videos/1/"}]}"#;
        let full = include_str!("../../tests/fixtures/ytdlp/facebook_reel.json");
        std::fs::write(dir.path().join("flat.out"), flat).unwrap();
        std::fs::write(dir.path().join("full.out"), full).unwrap();
        let script = "@echo off\r\necho %* | findstr /C:\"--flat-playlist\" >nul && (type \"%~dp0flat.out\" & exit /b 0)\r\ntype \"%~dp0full.out\"\r\n";
        std::fs::write(dir.path().join("yt-dlp.cmd"), script).unwrap();
        let info = resolve_url("https://www.facebook.com/x/videos/1/", &ResolveCache::default()).await.unwrap();
        unsafe { std::env::remove_var("LUMI_BIN_DIR") };
        assert_eq!(info.items[0].options[0].id, "0/video:0");
    }

    #[tokio::test]
    async fn photo_post_uses_gallerydl_without_fallback() {
        let (_g, dir) = with_bin_dir();
        fake_tool(dir.path(), "gallery-dl", GDL_IG, "", 0);
        let info = resolve_url("https://www.instagram.com/p/C8x/", &ResolveCache::default()).await.unwrap();
        unsafe { std::env::remove_var("LUMI_BIN_DIR") };
        assert_eq!(info.engine, Engine::GalleryDl);
        assert_eq!(info.items.len(), 4);
    }

    #[tokio::test]
    async fn both_fail_returns_primary_error() {
        let (_g, dir) = with_bin_dir();
        fake_tool(dir.path(), "gallery-dl", "", "boom from gallery-dl", 1);
        fake_tool(dir.path(), "yt-dlp", "", "ERROR: Unsupported URL: x", 1);
        let err = resolve_url("https://www.instagram.com/p/C8x/", &ResolveCache::default()).await.unwrap_err();
        unsafe { std::env::remove_var("LUMI_BIN_DIR") };
        assert_eq!(err.error, ErrorCode::Unknown);
        assert!(err.detail.contains("boom from gallery-dl"));
    }

    #[tokio::test]
    async fn login_required_does_not_fall_back() {
        let (_g, dir) = with_bin_dir();
        let login = r#"[[-1,{"error":"AbortExtraction","message":"HTTP redirect to login page"}]]"#;
        fake_tool(dir.path(), "gallery-dl", login, "", 0);
        fake_tool(dir.path(), "yt-dlp", YT_TIKTOK, "", 0);
        let err = resolve_url("https://www.instagram.com/p/C8x/", &ResolveCache::default()).await.unwrap_err();
        unsafe { std::env::remove_var("LUMI_BIN_DIR") };
        assert_eq!(err.error, ErrorCode::LoginRequired { site: Site::Instagram });
        assert_eq!(err.detail, "AbortExtraction: HTTP redirect to login page");
    }

    /// gallery-dl exits 0 with its error inside the JSON: that text is what "Show details" shows.
    #[tokio::test]
    async fn gallerydl_error_message_is_the_detail() {
        let (_g, dir) = with_bin_dir();
        let boom = r#"[[-1,{"error":"HttpError","message":"'500 Internal Server Error' for 'https://www.instagram.com/p/C8x/'"}]]"#;
        fake_tool(dir.path(), "gallery-dl", boom, "", 0);
        fake_tool(dir.path(), "yt-dlp", "", "ERROR: Unsupported URL: x", 1);
        let err = resolve_url("https://www.instagram.com/p/C8x/", &ResolveCache::default()).await.unwrap_err();
        unsafe { std::env::remove_var("LUMI_BIN_DIR") };
        assert_eq!(err.error, ErrorCode::Unknown);
        assert!(err.detail.starts_with("HttpError: '500 Internal Server Error'"), "{}", err.detail);
    }

    #[tokio::test]
    async fn missing_primary_tool_is_reported() {
        let (_g, _dir) = with_bin_dir();
        let err = resolve_url("https://www.youtube.com/watch?v=x", &ResolveCache::default()).await.unwrap_err();
        unsafe { std::env::remove_var("LUMI_BIN_DIR") };
        assert_eq!(err.error, ErrorCode::ToolMissing { tool: Tool::YtDlp });
    }

    /// Runs the real tools against real URLs. `cargo test live_resolve_samples -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "hits the network and downloads yt-dlp and gallery-dl"]
    async fn live_resolve_samples() {
        let _g = TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("LUMI_BIN_DIR", dir.path()) };
        for t in [Tool::YtDlp, Tool::GalleryDl] {
            crate::tools::install::install_with(t, &mut |_, _| {}).await.unwrap();
        }
        let cache = ResolveCache::default();
        let tiktok = resolve_url("https://www.tiktok.com/@scout2015/video/6718335390845095173", &cache).await;
        let youtube = resolve_url("https://www.youtube.com/watch?v=jNQXAC9IVRw", &cache).await;
        let insta = resolve_url("https://www.instagram.com/p/CuE2WNQs6vH/", &cache).await;
        unsafe { std::env::remove_var("LUMI_BIN_DIR") };

        let summarize = |r: &Result<MediaInfo, LumiError>| match r {
            Ok(i) => format!(
                "OK engine={:?} title={:?} author={:?} items={} options={:?}",
                i.engine,
                i.title,
                i.author,
                i.items.len(),
                i.items[0].options.iter().map(|o| format!("{}:{}", o.id, o.label)).collect::<Vec<_>>()
            ),
            Err(e) => format!("ERR {:?}", e.error),
        };
        println!("tiktok   : {}", summarize(&tiktok));
        println!("youtube  : {}", summarize(&youtube));
        println!("instagram: {}", summarize(&insta));
        assert!(!tiktok.unwrap().items[0].options.is_empty());
        assert!(!youtube.unwrap().items[0].options.is_empty());
        match insta {
            Ok(i) => assert!(!i.items.is_empty()),
            Err(e) => assert_eq!(e.error, ErrorCode::LoginRequired { site: Site::Instagram }),
        }
    }

    /// Real TikTok links: licensed track, original sound, vt. short link.
    /// `cargo test live_tiktok_sound -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "hits the network and downloads yt-dlp and gallery-dl"]
    async fn live_tiktok_sound() {
        let _g = TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("LUMI_BIN_DIR", dir.path()) };
        for t in [Tool::YtDlp, Tool::GalleryDl] {
            crate::tools::install::install_with(t, &mut |_, _| {}).await.unwrap();
        }
        let links = [
            ("licensed", "https://www.tiktok.com/@charlidamelio/video/7699134829455052062"),
            ("original", "https://www.tiktok.com/@charlidamelio/video/7700403350977662221"),
            ("vt-short", "https://vt.tiktok.com/ZSffquwF2/"),
        ];
        let mut found = 0;
        for (name, url) in links {
            let r = resolve_url(url, &ResolveCache::default()).await;
            let scraped = fetch_tiktok_sound(url, None).await;
            match r {
                Ok(info) => match info.sound {
                    Some(s) => {
                        found += 1;
                        let src = if scraped.as_ref() == Some(&s) { "page scrape" } else { "yt-dlp fallback" };
                        let row = info.items[0].options.iter().find(|o| o.id == "sound").is_some();
                        println!("{name}: source={src} row={row} title={:?} artist={:?} play_url={}...", s.title, s.artist, &s.play_url[..s.play_url.len().min(60)]);
                    }
                    None => println!("{name}: no sound (scrape={})", scraped.is_some()),
                },
                Err(e) => println!("{name}: ERR {:?} (scrape={})", e.error, scraped.is_some()),
            }
        }
        unsafe { std::env::remove_var("LUMI_BIN_DIR") };
        assert!(found > 0, "no link produced a sound row");
    }
}
