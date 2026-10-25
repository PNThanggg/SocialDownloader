#![allow(dead_code)]

use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde::Deserialize;

use crate::model::{MediaOption, OptionId, OptionKind, OptionNote};

/// Standard quality buckets, largest first; a real quality maps to the largest bucket <= it.
const BUCKETS: [u32; 6] = [2160, 1440, 1080, 720, 480, 360];

/// The subset of a yt-dlp `formats[]` entry that grouping reads.
#[derive(Deserialize, Debug, Clone, Default)]
pub struct YtFormat {
    pub format_id: Option<String>,
    pub url: Option<String>,
    pub ext: Option<String>,
    pub audio_ext: Option<String>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub vcodec: Option<String>,
    pub acodec: Option<String>,
    pub protocol: Option<String>,
    pub tbr: Option<f64>,
    pub filesize: Option<f64>,
    pub filesize_approx: Option<f64>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Codec {
    Yes,
    No,
    Unknown,
}

fn codec(c: &Option<String>) -> Codec {
    match c.as_deref() {
        None | Some("") => Codec::Unknown,
        Some("none") => Codec::No,
        Some(_) => Codec::Yes,
    }
}

impl YtFormat {
    fn is_junk(&self) -> bool {
        // Storyboards/images: mhtml, or neither video nor audio.
        self.ext.as_deref() == Some("mhtml") || (codec(&self.vcodec) == Codec::No && codec(&self.acodec) == Codec::No)
    }

    /// Carries video and audio. Absent codec fields count as present (yt-dlp's own
    /// assumption for muxed files whose codecs it could not probe, e.g. Facebook).
    fn is_combined(&self) -> bool {
        !self.is_junk() && codec(&self.vcodec) != Codec::No && codec(&self.acodec) != Codec::No
    }

    fn is_video_only(&self) -> bool {
        codec(&self.vcodec) != Codec::No && codec(&self.acodec) == Codec::No && !self.is_junk()
    }

    fn is_audio_only(&self) -> bool {
        codec(&self.vcodec) == Codec::No && codec(&self.acodec) == Codec::Yes && !self.is_junk()
    }

    fn is_h264(&self) -> bool {
        self.vcodec.as_deref().is_some_and(|v| {
            let v = v.to_ascii_lowercase();
            v.starts_with("avc1") || v.starts_with("h264")
        })
    }

    /// The short side when both dimensions are known, else the height; 0 when unknown.
    fn quality(&self) -> u32 {
        let h = self.height.filter(|h| *h > 0.0).map(|h| h as u32);
        let w = self.width.filter(|w| *w > 0.0).map(|w| w as u32);
        match (w, h) {
            (Some(w), Some(h)) => w.min(h),
            (_, Some(h)) => h,
            _ => 0,
        }
    }

    fn size(&self) -> Option<u64> {
        self.filesize.or(self.filesize_approx).filter(|s| *s > 0.0).map(|s| s as u64)
    }

    fn bitrate(&self) -> f64 {
        self.tbr.unwrap_or(0.0)
    }

    fn direct_url(&self) -> Option<String> {
        let streaming = self.protocol.as_deref().is_some_and(|p| p.contains("m3u8") || p.contains("dash"));
        if streaming { None } else { self.url.clone() }
    }
}

fn bucket(quality: u32) -> u32 {
    if quality == 0 {
        return 0;
    }
    BUCKETS.iter().copied().find(|b| *b <= quality).unwrap_or(quality)
}

fn cmp_f64(a: f64, b: f64) -> Ordering {
    a.partial_cmp(&b).unwrap_or(Ordering::Equal)
}

/// Ordering for "better video": combined, then h264, then a directly downloadable (non-HLS/DASH) one, then larger tbr, then larger size.
fn cmp_video(a: &YtFormat, b: &YtFormat) -> Ordering {
    a.is_combined()
        .cmp(&b.is_combined())
        .then(a.is_h264().cmp(&b.is_h264()))
        .then(a.direct_url().is_some().cmp(&b.direct_url().is_some()))
        .then(cmp_f64(a.bitrate(), b.bitrate()))
        .then(a.size().unwrap_or(0).cmp(&b.size().unwrap_or(0)))
}

fn cmp_audio(a: &YtFormat, b: &YtFormat) -> Ordering {
    cmp_f64(a.bitrate(), b.bitrate()).then(a.size().unwrap_or(0).cmp(&b.size().unwrap_or(0)))
}

fn label(bucket: u32) -> String {
    if bucket == 0 { "MP4".into() } else { format!("{bucket}p") }
}

/// Collapses a raw format list into friendly rows: one per quality bucket (descending), then MP3.
/// `_has_deno` is accepted for API symmetry; `limited_formats` is decided by the caller.
pub fn group_formats(item: u32, formats: &[YtFormat], _has_deno: bool) -> Vec<MediaOption> {
    let usable: Vec<&YtFormat> = formats.iter().filter(|f| !f.is_junk()).collect();
    let best_audio_only = usable.iter().copied().filter(|f| f.is_audio_only()).max_by(|a, b| cmp_audio(a, b));
    let best_combined = usable.iter().copied().filter(|f| f.is_combined()).max_by(|a, b| cmp_audio(a, b));

    let mut buckets: BTreeMap<u32, &YtFormat> = BTreeMap::new();
    for f in usable.iter().copied().filter(|f| f.is_combined() || f.is_video_only()) {
        let key = bucket(f.quality());
        match buckets.get(&key) {
            // `>=` so a later format wins ties (yt-dlp lists worse formats first).
            Some(cur) if cmp_video(f, cur) == Ordering::Less => {}
            _ => {
                buckets.insert(key, f);
            }
        }
    }

    let mut out: Vec<MediaOption> = Vec::new();
    for (key, f) in buckets.iter().rev() {
        let combined = f.is_combined();
        let height = f.height.filter(|h| *h > 0.0).map(|h| h as u32).unwrap_or(0);
        let filesize = f.size().map(|s| {
            if combined { s } else { s + best_audio_only.and_then(|a| a.size()).unwrap_or(0) }
        });
        out.push(MediaOption {
            id: OptionId::Video { item, height }.to_string(),
            kind: OptionKind::Video,
            label: label(*key),
            ext: "mp4".into(),
            filesize,
            direct_url: f.direct_url(),
            note: if combined { None } else { Some(OptionNote::NoAudio) },
        });
    }

    if let Some(a) = best_audio_only {
        out.push(MediaOption {
            id: OptionId::Audio { item }.to_string(),
            kind: OptionKind::Audio,
            label: "MP3".into(),
            ext: "mp3".into(),
            filesize: a.size(),
            direct_url: a.direct_url(),
            note: a
                .ext
                .clone()
                .or_else(|| a.audio_ext.clone())
                .filter(|e| !e.is_empty() && e != "none")
                .map(|ext| OptionNote::SourceAudio { ext }),
        });
    } else if let Some(c) = best_combined {
        out.push(MediaOption {
            id: OptionId::Audio { item }.to_string(),
            kind: OptionKind::Audio,
            label: "MP3".into(),
            ext: "mp3".into(),
            filesize: c.size(),
            direct_url: c.direct_url(),
            note: Some(OptionNote::SourceIsVideo),
        });
    }
    out
}
