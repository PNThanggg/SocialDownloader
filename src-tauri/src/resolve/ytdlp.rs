#![allow(dead_code)]

use std::ffi::OsString;
use std::path::Path;

use serde::Deserialize;

use super::formats::{YtFormat, group_formats};
use crate::errors::ErrorCode;
use crate::model::{Engine, MediaInfo, MediaItem, Site};
use crate::tools::runner::ytdlp_common_args;

#[derive(Deserialize, Debug, Default)]
struct YtInfo {
    #[serde(rename = "_type")]
    kind: Option<String>,
    id: Option<String>,
    title: Option<String>,
    webpage_url: Option<String>,
    original_url: Option<String>,
    uploader: Option<String>,
    channel: Option<String>,
    creator: Option<String>,
    duration: Option<f64>,
    thumbnail: Option<String>,
    entries: Option<Vec<Option<YtInfo>>>,
    formats: Option<Vec<YtFormat>>,
}

/// `-J` + the shared yt-dlp flags + optional `--cookies` + the URL.
/// `--flat-playlist`: a playlist/channel link lists its entries without extracting each one, so it
/// fails fast with PlaylistNotSupported. Carousel entries (Instagram/Facebook) are full info dicts and
/// are unaffected.
pub fn ytdlp_resolve_args(url: &str, cookies: Option<&Path>) -> Vec<OsString> {
    let mut args = vec![OsString::from("-J"), OsString::from("--flat-playlist")];
    args.extend(ytdlp_common_args());
    if let Some(p) = cookies {
        args.push("--cookies".into());
        args.push(p.as_os_str().to_owned());
    }
    args.push(url.into());
    args
}

fn item_from(site: Site, index: u32, info: &YtInfo, has_deno: bool) -> MediaItem {
    // TikTok's format_id "audio" is the original-sound/music track, not the video's own audio.
    let formats: Vec<YtFormat> = info
        .formats
        .iter()
        .flatten()
        .filter(|f| !(site == Site::Tiktok && f.format_id.as_deref() == Some("audio")))
        .cloned()
        .collect();
    MediaItem {
        index,
        thumbnail: info.thumbnail.clone(),
        is_video: true,
        options: group_formats(index, &formats, has_deno),
    }
}

/// Turns `yt-dlp -J` output into `MediaInfo`. `source_url` is empty when yt-dlp reports no URL;
/// the caller (which knows the input URL) fills it in.
pub fn parse_ytdlp_json(site: Site, json: &str, has_deno: bool) -> Result<MediaInfo, ErrorCode> {
    let info: YtInfo = serde_json::from_str(json).map_err(|_| ErrorCode::ExtractorBroken)?;
    let is_playlist = matches!(info.kind.as_deref(), Some("playlist" | "multi_video"));

    let items = if is_playlist {
        if !matches!(site, Site::Instagram | Site::Facebook) {
            return Err(ErrorCode::PlaylistNotSupported);
        }
        info.entries
            .iter()
            .flatten()
            .flatten()
            .enumerate()
            .map(|(i, e)| item_from(site, i as u32, e, has_deno))
            .collect()
    } else {
        vec![item_from(site, 0, &info, has_deno)]
    };

    let thumbnail = info.thumbnail.clone().or_else(|| items.first().and_then(|i| i.thumbnail.clone()));
    Ok(MediaInfo {
        source_url: info.webpage_url.or(info.original_url).unwrap_or_default(),
        site,
        engine: Engine::YtDlp,
        title: info.title.or(info.id).unwrap_or_default(),
        author: info.uploader.or(info.channel).or(info.creator),
        duration_secs: info.duration,
        thumbnail,
        items,
        limited_formats: site == Site::Youtube && !has_deno,
        sound: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{MediaOption, OptionKind, OptionNote};

    const TIKTOK: &str = include_str!("../../tests/fixtures/ytdlp/tiktok_video.json");
    const YOUTUBE: &str = include_str!("../../tests/fixtures/ytdlp/youtube_video.json");
    const YT_PLAYLIST: &str = include_str!("../../tests/fixtures/ytdlp/youtube_playlist.json");
    const FACEBOOK: &str = include_str!("../../tests/fixtures/ytdlp/facebook_reel.json");
    const INSTAGRAM: &str = include_str!("../../tests/fixtures/ytdlp/instagram_carousel_videos.json");

    fn videos(opts: &[MediaOption]) -> Vec<&MediaOption> {
        opts.iter().filter(|o| o.kind == OptionKind::Video).collect()
    }
    fn audio(opts: &[MediaOption]) -> Vec<&MediaOption> {
        opts.iter().filter(|o| o.kind == OptionKind::Audio).collect()
    }

    #[test]
    fn tiktok_has_video_and_audio() {
        let info = parse_ytdlp_json(Site::Tiktok, TIKTOK, false).unwrap();
        assert_eq!(info.items.len(), 1);
        let opts = &info.items[0].options;
        assert!(!videos(opts).is_empty());
        assert_eq!(audio(opts).len(), 1);
        // The music-only "audio" format is excluded, so MP3 comes from the video itself.
        assert_eq!(audio(opts)[0].note, Some(OptionNote::SourceIsVideo));
        let best_combined = videos(opts).last().unwrap().direct_url.clone();
        assert!(best_combined.is_some());
        assert_eq!(audio(opts)[0].direct_url, best_combined);
        assert!(info.title.starts_with("Scramble up ur name"));
        assert_eq!(info.author.as_deref(), Some("scout2015"));
        assert_eq!(info.engine, crate::model::Engine::YtDlp);
        assert!(!info.limited_formats);
        assert_eq!(info.source_url, "https://www.tiktok.com/@scout2015/video/6718335390845095173");
        assert_eq!(info.duration_secs, Some(10.0));
    }

    #[test]
    fn tiktok_portrait_uses_short_side_label_and_real_height() {
        let info = parse_ytdlp_json(Site::Tiktok, TIKTOK, false).unwrap();
        let v = videos(&info.items[0].options);
        let labels: Vec<_> = v.iter().map(|o| o.label.as_str()).collect();
        // 720x1280 -> short side 720, 576x1024 -> short side 576 -> 480 bucket.
        assert_eq!(labels, ["720p", "480p"]);
        assert_eq!(v[0].id, "0/video:1280");
        assert_eq!(v[1].id, "0/video:1024");
        assert_eq!(v[1].note, None);
        assert_eq!(v[1].ext, "mp4");
        // Best h264 combined by tbr: the 2240 kbps one.
        assert_eq!(v[1].filesize, Some(2953029));
    }

    #[test]
    fn combined_only_audio_is_source_is_video() {
        let json = r#"{"id":"1","title":"t","formats":[
            {"format_id":"a","url":"https://x/a.mp4","ext":"mp4","width":640,"height":360,"vcodec":"avc1","acodec":"mp4a","protocol":"https","tbr":500}]}"#;
        let info = parse_ytdlp_json(Site::Other, json, false).unwrap();
        let a = audio(&info.items[0].options);
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].note, Some(OptionNote::SourceIsVideo));
        assert_eq!(a[0].id, "0/audio");
        assert_eq!(a[0].label, "MP3");
        assert_eq!(a[0].ext, "mp3");
        assert_eq!(a[0].direct_url.as_deref(), Some("https://x/a.mp4"));
    }

    #[test]
    fn audio_format_id_filter_is_tiktok_only() {
        let json = r#"{"id":"1","title":"t","formats":[
            {"format_id":"audio","url":"https://x/a.mp3","ext":"mp3","vcodec":"none","acodec":"mp3","protocol":"https"},
            {"format_id":"v","url":"https://x/v.mp4","ext":"mp4","height":720,"width":1280,"vcodec":"avc1","acodec":"none","protocol":"https"}]}"#;
        let other = parse_ytdlp_json(Site::Other, json, false).unwrap();
        assert_eq!(audio(&other.items[0].options)[0].note, Some(OptionNote::SourceAudio { ext: "mp3".into() }));
        let tt = parse_ytdlp_json(Site::Tiktok, json, false).unwrap();
        assert!(audio(&tt.items[0].options).is_empty());
    }

    #[test]
    fn youtube_groups_heights() {
        let info = parse_ytdlp_json(Site::Youtube, YOUTUBE, true).unwrap();
        let opts = &info.items[0].options;
        let v = videos(opts);
        let labels: Vec<_> = v.iter().map(|o| o.label.as_str()).collect();
        assert_eq!(labels, ["2160p", "1440p", "1080p", "720p", "480p", "360p", "240p", "144p"]);
        assert_eq!(v[2].note, Some(OptionNote::NoAudio));
        assert_eq!(v[2].id, "0/video:1080");
        let a = audio(opts);
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].note, Some(OptionNote::SourceAudio { ext: "m4a".into() }));
    }

    #[test]
    fn youtube_without_deno_is_limited() {
        assert!(parse_ytdlp_json(Site::Youtube, YOUTUBE, false).unwrap().limited_formats);
        assert!(!parse_ytdlp_json(Site::Youtube, YOUTUBE, true).unwrap().limited_formats);
    }

    #[test]
    fn playlist_rejected() {
        assert_eq!(parse_ytdlp_json(Site::Youtube, YT_PLAYLIST, true).unwrap_err(), ErrorCode::PlaylistNotSupported);
        assert_eq!(parse_ytdlp_json(Site::Tiktok, YT_PLAYLIST, true).unwrap_err(), ErrorCode::PlaylistNotSupported);
    }

    #[test]
    fn instagram_carousel_items() {
        let v: serde_json::Value = serde_json::from_str(INSTAGRAM).unwrap();
        let n = v["entries"].as_array().unwrap().len();
        let info = parse_ytdlp_json(Site::Instagram, INSTAGRAM, false).unwrap();
        assert_eq!(info.items.len(), n);
        for (i, item) in info.items.iter().enumerate() {
            assert_eq!(item.index, i as u32);
            assert!(item.is_video);
            assert!(item.thumbnail.is_some());
            assert!(item.options.iter().any(|o| o.id == format!("{i}/audio")));
        }
        assert_eq!(info.author.as_deref(), Some("example_user"));
    }

    #[test]
    fn facebook_unknown_height_is_mp4_with_height_zero() {
        let info = parse_ytdlp_json(Site::Facebook, FACEBOOK, false).unwrap();
        let opts = &info.items[0].options;
        let v = videos(opts);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].label, "MP4");
        assert_eq!(v[0].id, "0/video:0");
        assert_eq!(v[0].note, None);
        assert_eq!(audio(opts)[0].note, Some(OptionNote::SourceIsVideo));
    }

    #[test]
    fn hls_has_no_direct_url() {
        let json = r#"{"id":"1","title":"t","formats":[
            {"format_id":"h","url":"https://x/a.m3u8","ext":"mp4","width":1280,"height":720,"vcodec":"avc1","acodec":"mp4a","protocol":"m3u8_native","tbr":500},
            {"format_id":"d","url":"https://x/d.mpd","ext":"mp4","width":640,"height":360,"vcodec":"avc1","acodec":"mp4a","protocol":"http_dash_segments","tbr":300}]}"#;
        let info = parse_ytdlp_json(Site::Other, json, false).unwrap();
        let opts = &info.items[0].options;
        assert!(opts.iter().all(|o| o.direct_url.is_none()));
        assert_eq!(videos(opts).len(), 2);
    }

    #[test]
    fn filesize_adds_best_audio_for_video_only() {
        let info = parse_ytdlp_json(Site::Youtube, YOUTUBE, true).unwrap();
        let opts = &info.items[0].options;
        let a = audio(opts)[0];
        let v360 = opts.iter().find(|o| o.id == "0/video:360").unwrap();
        // 360p h264 video-only (134, https) + best audio; strictly more than the audio alone.
        assert!(v360.filesize.unwrap() > a.filesize.unwrap());
    }

    #[test]
    fn resolve_args_include_cookies_and_flags() {
        let args = ytdlp_resolve_args("https://x/y", Some(Path::new("c.txt")));
        let s: Vec<String> = args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(s.first().map(String::as_str), Some("-J"));
        for want in ["--no-playlist", "--encoding", "utf-8", "--cookies", "c.txt"] {
            assert!(s.iter().any(|a| a == want), "missing {want}");
        }
        assert_eq!(s.last().map(String::as_str), Some("https://x/y"));
        let no = ytdlp_resolve_args("u", None);
        assert!(!no.iter().any(|a| a == "--cookies"));
    }

    /// A playlist/channel link must come back fast as a flat list (then PlaylistNotSupported),
    /// not after every entry was fully extracted.
    #[test]
    fn resolve_args_are_flat() {
        let s: Vec<String> = ytdlp_resolve_args("u", None).iter().map(|a| a.to_string_lossy().into_owned()).collect();
        assert!(s.iter().any(|a| a == "--flat-playlist"));
    }

    #[test]
    fn flat_playlist_of_url_entries_is_still_rejected() {
        let json = r#"{"_type":"playlist","id":"PL","title":"p","entries":[{"_type":"url","url":"https://www.youtube.com/watch?v=a","ie_key":"Youtube"}]}"#;
        assert_eq!(parse_ytdlp_json(Site::Youtube, json, true).unwrap_err(), ErrorCode::PlaylistNotSupported);
    }
}
