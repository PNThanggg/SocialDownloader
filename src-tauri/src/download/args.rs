#![allow(dead_code)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::errors::ErrorCode;
use crate::model::{Engine, MediaInfo, MediaOption, OptionId, OptionNote};
use crate::tools::runner::ytdlp_common_args;

const TIKTOK_REFERER: &str = "https://www.tiktok.com/";
const PROGRESS_TEMPLATE: &str = "download:LUMI|%(progress.downloaded_bytes)s|%(progress.total_bytes)s|%(progress.total_bytes_estimate)s";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FfInput {
    Path(PathBuf),
    /// The only file inside this directory (gallery-dl picks the name).
    SingleFileIn(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    YtDlp(Vec<OsString>),
    GalleryDl(Vec<OsString>),
    Http { url: String, referer: String, to: PathBuf, optional: bool },
    /// `args` = everything after the inputs, including the destination.
    Ffmpeg { input: FfInput, cover: Option<PathBuf>, args: Vec<OsString> },
    MoveSingle { from_dir: PathBuf, to: PathBuf },
}

/// Full ffmpeg argument list for an `Ffmpeg` step; pass `cover` only if the file exists.
pub fn ffmpeg_command_args(input: &Path, cover: Option<&Path>, args: &[OsString]) -> Vec<OsString> {
    let mut out: Vec<OsString> = vec!["-y".into(), "-i".into(), input.into()];
    if let Some(c) = cover {
        out.extend(["-i".into(), c.into()]);
        out.extend(
            ["-map", "0:a", "-map", "1", "-c:v", "mjpeg", "-disposition:v", "attached_pic"]
                .map(OsString::from),
        );
    }
    out.extend(args.iter().cloned());
    out
}

fn os(items: &[&str]) -> Vec<OsString> {
    items.iter().map(OsString::from).collect()
}

fn item_of(opt: &OptionId) -> u32 {
    match opt {
        OptionId::Video { item, .. } | OptionId::Audio { item } | OptionId::Image { item } => *item,
        OptionId::Sound => 0,
    }
}

fn find_option<'a>(info: &'a MediaInfo, opt: &OptionId) -> Option<&'a MediaOption> {
    let id = opt.to_string();
    info.items.iter().flat_map(|i| i.options.iter()).find(|o| o.id == id)
}

pub fn needs_ffmpeg(info: &MediaInfo, opt: &OptionId) -> bool {
    match opt {
        OptionId::Audio { .. } | OptionId::Sound => true,
        OptionId::Image { .. } => false,
        OptionId::Video { .. } => {
            info.engine == Engine::YtDlp
                && matches!(find_option(info, opt).and_then(|o| o.note.as_ref()), Some(OptionNote::NoAudio))
        }
    }
}

fn tag_args(title: &str, artist: &str) -> Vec<OsString> {
    vec![
        "-metadata".into(),
        format!("title={title}").into(),
        "-metadata".into(),
        format!("artist={artist}").into(),
    ]
}

fn ytdlp_plan(
    info: &MediaInfo, opt: &OptionId, dest: &Path, tmp: &Path, cookies: Option<&Path>,
) -> Result<Vec<Plan>, ErrorCode> {
    let item = item_of(opt);
    let mut a = ytdlp_common_args();
    match opt {
        OptionId::Video { height, .. } => {
            a.extend(os(&[
                "-f",
                &format!("bv*[height<={height}]+ba[format_id!=audio]/b[height<={height}]"),
                "-S",
                "ext:mp4:m4a",
                "--merge-output-format",
                "mp4",
            ]));
        }
        OptionId::Audio { .. } => {
            a.extend(os(&["-f", "ba[format_id!=audio]/b", "-x", "--audio-format", "mp3", "--audio-quality", "0"]));
        }
        _ => return Err(ErrorCode::Unknown),
    }
    if needs_ffmpeg(info, opt) {
        a.push("--embed-metadata".into());
    }
    let stem = dest.file_stem().ok_or(ErrorCode::Unknown)?;
    let dir = dest.parent().unwrap_or_else(|| Path::new(""));
    // `-o` is an output template as a whole: escape `%` in the directory too.
    let base = dir.join(stem).to_string_lossy().replace('%', "%%");
    let template = format!("{base}.%(ext)s");
    let mut temp = OsString::from("temp:");
    temp.push(tmp);
    a.extend(os(&["--force-overwrites", "-P"]));
    a.push(temp);
    a.push("-o".into());
    a.push(template.into());
    a.extend(os(&["--newline", "--progress-template", PROGRESS_TEMPLATE]));
    if item > 0 {
        a.extend(os(&["--playlist-items", &(item + 1).to_string()]));
    }
    if let Some(c) = cookies {
        a.push("--cookies".into());
        a.push(c.into());
    }
    a.push((&info.source_url).into());
    Ok(vec![Plan::YtDlp(a)])
}

fn gallery_args(info: &MediaInfo, item: u32, out_dir: &Path, cookies: Option<&Path>) -> Vec<OsString> {
    let mut a = os(&["--range", &(item + 1).to_string(), "-D"]);
    a.push(out_dir.into());
    a.extend(os(&["-o", "filename={num}.{extension}"]));
    if let Some(c) = cookies {
        a.push("--cookies".into());
        a.push(c.into());
    }
    a.push((&info.source_url).into());
    a
}

fn gallery_plan(
    info: &MediaInfo, opt: &OptionId, dest: &Path, tmp: &Path, cookies: Option<&Path>,
) -> Result<Vec<Plan>, ErrorCode> {
    let item = item_of(opt);
    match opt {
        OptionId::Image { .. } | OptionId::Video { .. } => Ok(vec![
            Plan::GalleryDl(gallery_args(info, item, tmp, cookies)),
            Plan::MoveSingle { from_dir: tmp.to_path_buf(), to: dest.to_path_buf() },
        ]),
        OptionId::Audio { .. } => {
            let src = tmp.join("src");
            let mut args = os(&["-vn", "-c:a", "libmp3lame", "-q:a", "0"]);
            args.extend(tag_args(&info.title, info.author.as_deref().unwrap_or("")));
            args.push(dest.into());
            Ok(vec![
                Plan::GalleryDl(gallery_args(info, item, &src, cookies)),
                Plan::Ffmpeg { input: FfInput::SingleFileIn(src), cover: None, args },
            ])
        }
        OptionId::Sound => Err(ErrorCode::Unknown),
    }
}

fn sound_plan(info: &MediaInfo, dest: &Path, tmp: &Path) -> Result<Vec<Plan>, ErrorCode> {
    let s = info.sound.as_ref().ok_or(ErrorCode::Unknown)?;
    let src = tmp.join("sound.src");
    let cover_path = tmp.join("cover.jpg");
    let mut plans = vec![Plan::Http {
        url: s.play_url.clone(),
        referer: TIKTOK_REFERER.into(),
        to: src.clone(),
        optional: false,
    }];
    if let Some(c) = &s.cover {
        plans.push(Plan::Http { url: c.clone(), referer: TIKTOK_REFERER.into(), to: cover_path.clone(), optional: true });
    }
    let mut args = os(&["-c:a", "libmp3lame", "-q:a", "0", "-id3v2_version", "3"]);
    args.extend(tag_args(&s.title, &s.artist));
    args.push(dest.into());
    plans.push(Plan::Ffmpeg {
        input: FfInput::Path(src),
        cover: s.cover.as_ref().map(|_| cover_path),
        args,
    });
    Ok(plans)
}

/// `dest` is the final file path (with extension); `tmp` a job-private scratch dir.
pub fn build_plan(
    info: &MediaInfo, opt: &OptionId, dest: &Path, tmp: &Path, cookies: Option<&Path>,
) -> Result<Vec<Plan>, ErrorCode> {
    if *opt == OptionId::Sound {
        return sound_plan(info, dest, tmp);
    }
    if item_of(opt) as usize >= info.items.len() {
        return Err(ErrorCode::Unknown);
    }
    match info.engine {
        Engine::YtDlp => ytdlp_plan(info, opt, dest, tmp, cookies),
        Engine::GalleryDl => gallery_plan(info, opt, dest, tmp, cookies),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{MediaItem, OptionKind, Site, SoundMeta};

    fn opt(id: &str, kind: OptionKind, note: Option<OptionNote>) -> MediaOption {
        MediaOption { id: id.into(), kind, label: String::new(), ext: "mp4".into(), filesize: None, direct_url: None, note }
    }

    fn info(site: Site, engine: Engine, n_items: u32, options: Vec<MediaOption>) -> MediaInfo {
        MediaInfo {
            source_url: "https://x.test/v".into(), site, engine, title: "T".into(), author: Some("A".into()),
            duration_secs: None, thumbnail: None,
            items: (0..n_items)
                .map(|i| MediaItem { index: i, thumbnail: None, is_video: true, options: if i == 0 { options.clone() } else { vec![] } })
                .collect(),
            limited_formats: false,
            sound: Some(SoundMeta { title: "ST".into(), artist: "SA".into(), cover: Some("https://c/c.jpg".into()), play_url: "https://p/p.mp3".into() }),
        }
    }

    fn strs(v: &[OsString]) -> Vec<String> {
        v.iter().map(|s| s.to_string_lossy().into_owned()).collect()
    }

    fn ytdlp_args(p: &[Plan]) -> Vec<String> {
        match &p[0] {
            Plan::YtDlp(a) => strs(a),
            other => panic!("not yt-dlp: {other:?}"),
        }
    }

    fn after(a: &[String], flag: &str) -> String {
        let i = a.iter().position(|x| x == flag).unwrap_or_else(|| panic!("missing {flag}"));
        a[i + 1].clone()
    }

    fn dest() -> PathBuf { PathBuf::from(r"C:\out\My Clip.mp4") }
    fn tmp() -> PathBuf { PathBuf::from(r"C:\tmp\job") }

    #[test]
    fn plan_audio_ytdlp() {
        let i = info(Site::Tiktok, Engine::YtDlp, 1, vec![]);
        let p = build_plan(&i, &OptionId::Audio { item: 0 }, &PathBuf::from(r"C:\out\My Clip.mp3"), &tmp(), None).unwrap();
        let a = ytdlp_args(&p);
        assert!(a.starts_with(&strs(&ytdlp_common_args())));
        for f in ["-x", "mp3", "--audio-quality", "0", "--embed-metadata", "--force-overwrites"] {
            assert!(a.contains(&f.to_string()), "{f}");
        }
        assert_eq!(after(&a, "-f"), "ba[format_id!=audio]/b");
        let o = after(&a, "-o");
        assert!(o.ends_with(r"My Clip.%(ext)s"), "{o}");
        assert!(o.starts_with(r"C:\out"));
        assert_eq!(after(&a, "-P"), r"temp:C:\tmp\job");
        assert_eq!(a.last().unwrap(), "https://x.test/v");
    }

    #[test]
    fn plan_video_selectors_and_metadata() {
        let tiktok = info(Site::Tiktok, Engine::YtDlp, 1, vec![opt("0/video:1080", OptionKind::Video, None)]);
        let a = ytdlp_args(&build_plan(&tiktok, &OptionId::Video { item: 0, height: 1080 }, &dest(), &tmp(), None).unwrap());
        let f = after(&a, "-f");
        assert!(f.contains("ba[format_id!=audio]"), "{f}");
        assert_eq!(f, "bv*[height<=1080]+ba[format_id!=audio]/b[height<=1080]");
        assert!(!a.contains(&"--embed-metadata".to_string()));
        assert!(a.contains(&"--force-overwrites".to_string()));
        assert_eq!(after(&a, "--merge-output-format"), "mp4");
        assert!(after(&a, "--progress-template").starts_with("download:LUMI|"));

        let yt = info(Site::Youtube, Engine::YtDlp, 1, vec![opt("0/video:1080", OptionKind::Video, Some(OptionNote::NoAudio))]);
        let a = ytdlp_args(&build_plan(&yt, &OptionId::Video { item: 0, height: 1080 }, &dest(), &tmp(), None).unwrap());
        assert!(a.contains(&"--embed-metadata".to_string()));
    }

    #[test]
    fn plan_percent_escaped_and_cookies() {
        let i = info(Site::Tiktok, Engine::YtDlp, 1, vec![]);
        let c = PathBuf::from(r"C:\tmp\c.txt");
        let a = ytdlp_args(&build_plan(&i, &OptionId::Audio { item: 0 }, &PathBuf::from(r"C:\out\100% sure.mp3"), &tmp(), Some(&c)).unwrap());
        assert!(after(&a, "-o").ends_with("100%% sure.%(ext)s"));
        assert_eq!(after(&a, "--cookies"), r"C:\tmp\c.txt");
        assert_eq!(a.last().unwrap(), "https://x.test/v");
    }

    #[test]
    fn plan_percent_in_directory_escaped() {
        let i = info(Site::Tiktok, Engine::YtDlp, 1, vec![]);
        let a = ytdlp_args(&build_plan(&i, &OptionId::Audio { item: 0 }, &PathBuf::from(r"C:\Videos\50% off\clip.mp3"), &tmp(), None).unwrap());
        assert_eq!(after(&a, "-o"), r"C:\Videos\50%% off\clip.%(ext)s");
    }

    #[test]
    fn plan_non_ascii_dest_roundtrips() {
        let i = info(Site::Tiktok, Engine::YtDlp, 1, vec![]);
        let a = ytdlp_args(&build_plan(&i, &OptionId::Audio { item: 0 }, &PathBuf::from(r"D:\Nhạc\Bài hát 🎧.mp3"), &tmp(), None).unwrap());
        assert_eq!(after(&a, "-o"), r"D:\Nhạc\Bài hát 🎧.%(ext)s");
    }

    #[test]
    fn plan_carousel_item_uses_playlist_items() {
        let i = info(Site::Instagram, Engine::YtDlp, 3, vec![]);
        let a = ytdlp_args(&build_plan(&i, &OptionId::Audio { item: 2 }, &dest(), &tmp(), None).unwrap());
        assert_eq!(after(&a, "--playlist-items"), "3");
        let a = ytdlp_args(&build_plan(&i, &OptionId::Audio { item: 0 }, &dest(), &tmp(), None).unwrap());
        assert!(!a.contains(&"--playlist-items".to_string()));
    }

    #[test]
    fn plan_sound_with_cover_tags() {
        let i = info(Site::Tiktok, Engine::YtDlp, 1, vec![]);
        let p = build_plan(&i, &OptionId::Sound, &PathBuf::from(r"C:\out\s.mp3"), &tmp(), None).unwrap();
        assert_eq!(p.len(), 3);
        assert!(matches!(&p[0], Plan::Http { optional: false, to, referer, .. } if to.ends_with("sound.src") && referer == "https://www.tiktok.com/"));
        assert!(matches!(&p[1], Plan::Http { optional: true, to, .. } if to.ends_with("cover.jpg")));
        let Plan::Ffmpeg { input: FfInput::Path(inp), cover: Some(cv), args } = &p[2] else { panic!("{:?}", p[2]) };
        let full = strs(&ffmpeg_command_args(inp, Some(cv), args));
        assert!(full.contains(&"title=ST".to_string()) && full.contains(&"artist=SA".to_string()));
        assert!(full.contains(&"attached_pic".to_string()));
        assert_eq!(full.last().unwrap(), r"C:\out\s.mp3");
        let no_cover = strs(&ffmpeg_command_args(inp, None, args));
        assert!(!no_cover.contains(&"attached_pic".to_string()));
    }

    #[test]
    fn plan_sound_without_cover() {
        let mut i = info(Site::Tiktok, Engine::YtDlp, 1, vec![]);
        i.sound.as_mut().unwrap().cover = None;
        let p = build_plan(&i, &OptionId::Sound, &dest(), &tmp(), None).unwrap();
        assert_eq!(p.len(), 2);
        assert!(matches!(&p[1], Plan::Ffmpeg { cover: None, .. }));
    }

    #[test]
    fn plan_gallery_dl() {
        let i = info(Site::Instagram, Engine::GalleryDl, 3, vec![]);
        let p = build_plan(&i, &OptionId::Image { item: 1 }, &dest(), &tmp(), None).unwrap();
        let Plan::GalleryDl(a) = &p[0] else { panic!() };
        let a = strs(a);
        assert_eq!(after(&a, "--range"), "2");
        assert_eq!(after(&a, "-D"), r"C:\tmp\job");
        assert_eq!(after(&a, "-o"), "filename={num}.{extension}");
        assert_eq!(p[1], Plan::MoveSingle { from_dir: tmp(), to: dest() });

        let p = build_plan(&i, &OptionId::Audio { item: 0 }, &dest(), &tmp(), None).unwrap();
        let Plan::GalleryDl(a) = &p[0] else { panic!() };
        assert_eq!(after(&strs(a), "-D"), r"C:\tmp\job\src");
        let Plan::Ffmpeg { input: FfInput::SingleFileIn(d), cover: None, args } = &p[1] else { panic!() };
        assert_eq!(d, &tmp().join("src"));
        let args = strs(args);
        assert!(args.contains(&"-vn".to_string()) && args.contains(&"title=T".to_string()) && args.contains(&"artist=A".to_string()));
    }

    #[test]
    fn plan_errors() {
        let yt = info(Site::Youtube, Engine::YtDlp, 1, vec![]);
        assert_eq!(build_plan(&yt, &OptionId::Image { item: 0 }, &dest(), &tmp(), None), Err(ErrorCode::Unknown));
        assert_eq!(build_plan(&yt, &OptionId::Audio { item: 5 }, &dest(), &tmp(), None), Err(ErrorCode::Unknown));
        let mut nosound = yt.clone();
        nosound.sound = None;
        assert_eq!(build_plan(&nosound, &OptionId::Sound, &dest(), &tmp(), None), Err(ErrorCode::Unknown));
    }

    #[test]
    fn needs_ffmpeg_cases() {
        let yt = info(Site::Youtube, Engine::YtDlp, 1, vec![opt("0/video:1080", OptionKind::Video, Some(OptionNote::NoAudio))]);
        let tt = info(Site::Tiktok, Engine::YtDlp, 1, vec![opt("0/video:1080", OptionKind::Video, None)]);
        let gd = info(Site::Instagram, Engine::GalleryDl, 1, vec![opt("0/video:720", OptionKind::Video, None)]);
        assert!(needs_ffmpeg(&tt, &OptionId::Audio { item: 0 }));
        assert!(needs_ffmpeg(&tt, &OptionId::Sound));
        assert!(needs_ffmpeg(&yt, &OptionId::Video { item: 0, height: 1080 }));
        assert!(!needs_ffmpeg(&tt, &OptionId::Video { item: 0, height: 1080 }));
        assert!(!needs_ffmpeg(&gd, &OptionId::Video { item: 0, height: 720 }));
        assert!(!needs_ffmpeg(&tt, &OptionId::Image { item: 0 }));
        assert!(needs_ffmpeg(&gd, &OptionId::Audio { item: 0 }));
    }
}
