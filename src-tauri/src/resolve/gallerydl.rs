use std::ffi::OsString;
use std::path::Path;

use serde_json::Value;

use crate::errors::{ErrorCode, classify};
use crate::tools::runner::gallerydl_common_args;
use crate::model::{Engine, MediaInfo, MediaItem, MediaOption, OptionId, OptionKind, OptionNote, Site};

/// The shared gallery-dl flags + `-j` + optional `--cookies` + `--` + the URL.
pub fn gallerydl_resolve_args(url: &str, cookies: Option<&Path>) -> Vec<OsString> {
    let mut args = gallerydl_common_args();
    args.push("-j".into());
    if let Some(p) = cookies {
        args.push("--cookies".into());
        args.push(p.as_os_str().to_owned());
    }
    args.extend(["--".into(), url.into()]);
    args
}

/// Instagram and TikTok photo posts go to gallery-dl first; everything else to yt-dlp.
pub fn primary_engine(site: Site, url: &str) -> Engine {
    match site {
        Site::Instagram => Engine::GalleryDl,
        Site::Tiktok if url::Url::parse(url).is_ok_and(|u| u.path().contains("/photo/")) => Engine::GalleryDl,
        _ => Engine::YtDlp,
    }
}

fn get_path<'a>(meta: &'a Value, path: &str) -> Option<&'a str> {
    let mut cur = meta;
    for key in path.split('.') {
        cur = cur.get(key)?;
    }
    cur.as_str().map(str::trim).filter(|s| !s.is_empty())
}

fn first_of(metas: &[&Value], keys: &[&str]) -> Option<String> {
    metas.iter().find_map(|m| keys.iter().find_map(|k| get_path(m, k))).map(str::to_owned)
}

fn site_name(site: Site) -> &'static str {
    match site {
        Site::Tiktok => "TikTok",
        Site::Instagram => "Instagram",
        Site::Youtube => "YouTube",
        Site::Facebook => "Facebook",
        Site::Other => "Media",
    }
}

fn dimension(meta: &Value, key: &str) -> u32 {
    meta.get(key).and_then(Value::as_u64).map_or(0, |v| v as u32)
}

fn file_ext(url: &str, meta: &Value) -> String {
    if let Some(e) = get_path(meta, "extension") {
        return e.to_ascii_lowercase();
    }
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.path().rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()))
        .unwrap_or_default()
}

fn item_from(index: u32, url: &str, meta: &Value) -> MediaItem {
    let ext = file_ext(url, meta);
    let is_video = matches!(ext.as_str(), "mp4" | "webm");
    let mk = |id: OptionId, kind, label: String, ext: &str, note| MediaOption {
        id: id.to_string(),
        kind,
        label,
        ext: ext.to_owned(),
        filesize: None,
        direct_url: Some(url.to_owned()),
        note,
    };
    if !is_video {
        return MediaItem {
            index,
            thumbnail: Some(url.to_owned()),
            is_video: false,
            options: vec![mk(OptionId::Image { item: index }, OptionKind::Image, ext.to_ascii_uppercase(), &ext, None)],
        };
    }
    let (w, h) = (dimension(meta, "width"), dimension(meta, "height"));
    let short = if w > 0 && h > 0 { w.min(h) } else { h };
    let label = if short == 0 { "MP4".to_owned() } else { format!("{short}p") };
    MediaItem {
        index,
        thumbnail: first_of(&[meta], &["display_url", "thumbnail", "cover"]),
        is_video: true,
        options: vec![
            mk(OptionId::Video { item: index, height: h }, OptionKind::Video, label, &ext, None),
            mk(OptionId::Audio { item: index }, OptionKind::Audio, "MP3".into(), "mp3", Some(OptionNote::SourceIsVideo)),
        ],
    }
}

/// `"{error}: {message}"` of the first `[-1, {error, message}]` message.
fn error_text(messages: &[Value]) -> Option<String> {
    let err = messages.iter().find(|m| m.get(0).and_then(Value::as_i64) == Some(-1))?.get(1)?;
    let text = |k| err.get(k).and_then(Value::as_str).unwrap_or("");
    Some(format!("{}: {}", text("error"), text("message")))
}

/// The error gallery-dl reported inside its `-j` output (it exits 0 and leaves stderr empty);
/// this is the raw log behind "Show details".
pub fn gallerydl_error_text(out: &str) -> Option<String> {
    error_text(&serde_json::from_str::<Vec<Value>>(out).ok()?)
}

/// Turns `gallery-dl -j` output (a JSON array of `[type, ...]` messages) into `MediaInfo`.
/// Type 3 = file, 6 = queued sub-URL (a profile/listing), 2 = directory metadata, -1 = error.
pub fn parse_gallerydl_json(site: Site, url: &str, out: &str) -> Result<MediaInfo, ErrorCode> {
    let messages: Vec<Value> = serde_json::from_str(out).map_err(|_| ErrorCode::ExtractorBroken)?;
    let kind = |m: &Value| m.get(0).and_then(Value::as_i64);
    if messages.iter().any(|m| kind(m) == Some(6)) {
        return Err(ErrorCode::PlaylistNotSupported);
    }

    let files: Vec<(&str, &Value)> = messages
        .iter()
        .filter(|m| kind(m) == Some(3))
        .filter_map(|m| Some((m.get(1)?.as_str()?, m.get(2)?)))
        .collect();
    if files.is_empty() {
        return Err(classify(site, &error_text(&messages).unwrap_or_default()));
    }

    let dir_meta = messages.iter().find(|m| kind(m) == Some(2)).and_then(|m| m.get(1));
    let mut metas = vec![files[0].1];
    metas.extend(dir_meta);

    let items: Vec<MediaItem> = files.iter().enumerate().map(|(i, (u, m))| item_from(i as u32, u, m)).collect();
    Ok(MediaInfo {
        source_url: url.to_owned(),
        site,
        engine: Engine::GalleryDl,
        title: first_of(&metas, &["description", "desc", "title", "content"])
            .unwrap_or_else(|| format!("{} post", site_name(site))),
        author: first_of(&metas, &["username", "fullname", "author.uniqueId", "author.nickname", "user.name"]),
        duration_secs: None,
        thumbnail: items.first().and_then(|i| i.thumbnail.clone()),
        items,
        limited_formats: false,
        sound: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{OptionKind, OptionNote};

    const IG: &str = include_str!("../../tests/fixtures/gallerydl/instagram_carousel_mixed.json");
    const TT: &str = include_str!("../../tests/fixtures/gallerydl/tiktok_photo.json");
    const FB: &str = include_str!("../../tests/fixtures/gallerydl/facebook_photo.json");

    #[test]
    fn ig_carousel_mixed() {
        let info = parse_gallerydl_json(Site::Instagram, "https://www.instagram.com/p/C8x/", IG).unwrap();
        assert_eq!(info.items.len(), 4);
        assert_eq!(info.engine, Engine::GalleryDl);
        assert!(!info.limited_formats);
        assert_eq!(info.title, "Weekend trip \u{1f334}");
        assert_eq!(info.author.as_deref(), Some("lumi.sample"));
        for i in [0usize, 1, 3] {
            let item = &info.items[i];
            assert!(!item.is_video);
            assert_eq!(item.options.len(), 1);
            let o = &item.options[0];
            assert_eq!(o.kind, OptionKind::Image);
            assert_eq!(o.id, format!("{i}/image"));
            assert_eq!(o.ext, "jpg");
            assert!(o.direct_url.as_deref().unwrap().ends_with(".jpg"));
            assert_eq!(item.thumbnail, o.direct_url);
        }
        let v = &info.items[2];
        assert!(v.is_video);
        assert_eq!(v.options.len(), 2);
        assert_eq!(v.options[0].kind, OptionKind::Video);
        assert_eq!(v.options[0].id, "2/video:1280");
        assert_eq!(v.options[0].label, "720p");
        assert_eq!(v.options[0].ext, "mp4");
        assert!(v.options[0].direct_url.as_deref().unwrap().ends_with(".mp4"));
        assert_eq!(v.options[1].kind, OptionKind::Audio);
        assert_eq!(v.options[1].id, "2/audio");
        assert_eq!(v.options[1].note, Some(OptionNote::SourceIsVideo));
        assert_eq!(v.thumbnail.as_deref(), Some("https://scontent.cdninstagram.com/v/t51.2885-15/100003_n.jpg"));
        assert_eq!(info.thumbnail, info.items[0].thumbnail);
    }

    #[test]
    fn tiktok_photo_fixture() {
        let info = parse_gallerydl_json(Site::Tiktok, "https://www.tiktok.com/@tiktok/photo/7666214645006421278", TT).unwrap();
        assert_eq!(info.items.len(), 3);
        assert!(info.items.iter().all(|i| !i.is_video && i.options[0].ext == "jpeg"));
        assert_eq!(info.author.as_deref(), Some("tiktok"));
        assert!(info.title.starts_with("for 39 days"));
        assert_eq!(info.source_url, "https://www.tiktok.com/@tiktok/photo/7666214645006421278");
    }

    #[test]
    fn facebook_photo_fixture() {
        let info = parse_gallerydl_json(Site::Facebook, "https://www.facebook.com/photo/?fbid=1", FB).unwrap();
        assert_eq!(info.items.len(), 1);
        assert_eq!(info.title, "A photo from the page");
        assert_eq!(info.author.as_deref(), Some("Lumi Sample Page"));
    }

    #[test]
    fn metadata_fallbacks() {
        let out = r#"[[3,"https://x/a.jpg",{"extension":"jpg","fullname":"Full Name"}]]"#;
        let info = parse_gallerydl_json(Site::Instagram, "u", out).unwrap();
        assert_eq!(info.title, "Instagram post");
        assert_eq!(info.author.as_deref(), Some("Full Name"));
        let out = r#"[[3,"https://x/a.jpg",{"extension":"jpg","description":"","desc":"d","user":{"name":"n"}}]]"#;
        let info = parse_gallerydl_json(Site::Tiktok, "u", out).unwrap();
        assert_eq!(info.title, "d");
        assert_eq!(info.author.as_deref(), Some("n"));
        let out = r#"[[3,"https://x/a.jpg",{"extension":"jpg"}]]"#;
        assert_eq!(parse_gallerydl_json(Site::Facebook, "u", out).unwrap().title, "Facebook post");
        assert_eq!(parse_gallerydl_json(Site::Facebook, "u", out).unwrap().author, None);
    }

    #[test]
    fn video_without_height_and_thumbnail_keys() {
        let out = r#"[[3,"https://x/a.webm",{"extension":"webm","thumbnail":"https://x/t.jpg"}],
                      [3,"https://x/b.mp4",{"extension":"mp4","cover":"https://x/c.jpg","height":0}]]"#;
        let info = parse_gallerydl_json(Site::Other, "u", out).unwrap();
        assert!(info.items.iter().all(|i| i.is_video));
        assert_eq!(info.items[0].options[0].label, "MP4");
        assert_eq!(info.items[0].options[0].id, "0/video:0");
        assert_eq!(info.items[0].thumbnail.as_deref(), Some("https://x/t.jpg"));
        assert_eq!(info.items[1].thumbnail.as_deref(), Some("https://x/c.jpg"));
        assert_eq!(info.thumbnail.as_deref(), Some("https://x/t.jpg"));
    }

    #[test]
    fn queue_message_means_playlist() {
        let out = r#"[[6,"https://www.tiktok.com/@tiktok/posts",{"category":"tiktok"}]]"#;
        assert_eq!(parse_gallerydl_json(Site::Tiktok, "u", out), Err(ErrorCode::PlaylistNotSupported));
    }

    #[test]
    fn no_files_is_unknown_and_errors_are_classified() {
        assert_eq!(parse_gallerydl_json(Site::Tiktok, "u", "[]"), Err(ErrorCode::Unknown));
        assert_eq!(parse_gallerydl_json(Site::Tiktok, "u", "not json"), Err(ErrorCode::ExtractorBroken));
        let out = r#"[[-1,{"error":"AbortExtraction","message":"HTTP redirect to login page (https://www.instagram.com/accounts/login/)"}]]"#;
        assert_eq!(
            parse_gallerydl_json(Site::Instagram, "u", out),
            Err(ErrorCode::LoginRequired { site: Site::Instagram })
        );
        assert_eq!(
            gallerydl_error_text(out).as_deref(),
            Some("AbortExtraction: HTTP redirect to login page (https://www.instagram.com/accounts/login/)")
        );
        assert_eq!(gallerydl_error_text("[]"), None);
        assert_eq!(gallerydl_error_text("not json"), None);
    }

    #[test]
    fn tiktok_photo_is_gallerydl() {
        assert_eq!(primary_engine(Site::Tiktok, "https://www.tiktok.com/@a/photo/1"), Engine::GalleryDl);
        assert_eq!(primary_engine(Site::Tiktok, "https://www.tiktok.com/@a/video/1"), Engine::YtDlp);
        assert_eq!(primary_engine(Site::Tiktok, "https://vt.tiktok.com/ZSabc/"), Engine::YtDlp);
        assert_eq!(primary_engine(Site::Instagram, "https://www.instagram.com/p/x/"), Engine::GalleryDl);
        assert_eq!(primary_engine(Site::Youtube, "https://youtu.be/x"), Engine::YtDlp);
        assert_eq!(primary_engine(Site::Facebook, "https://fb.watch/x"), Engine::YtDlp);
    }

    #[test]
    fn gallerydl_args_have_cookies() {
        let a = gallerydl_resolve_args("https://u", Some(Path::new("cookies.txt")));
        assert_eq!(a, ["--config-ignore", "-j", "--cookies", "cookies.txt", "--", "https://u"].map(OsString::from));
        let a = gallerydl_resolve_args("-https://u", None);
        assert_eq!(a, ["--config-ignore", "-j", "--", "-https://u"].map(OsString::from));
    }
}
