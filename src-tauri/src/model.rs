#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Site { Tiktok, Instagram, Youtube, Facebook, Other }

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Engine { YtDlp, GalleryDl }

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OptionKind { Video, Audio, Image, OriginalSound }

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OptionNote { NoAudio, SourceIsVideo, SourceAudio { ext: String }, LimitedFormats }

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaOption {
    pub id: String,
    pub kind: OptionKind,
    pub label: String,
    pub ext: String,
    pub filesize: Option<u64>,
    pub direct_url: Option<String>,
    pub note: Option<OptionNote>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaItem {
    pub index: u32,
    pub thumbnail: Option<String>,
    pub is_video: bool,
    pub options: Vec<MediaOption>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SoundMeta {
    pub title: String,
    pub artist: String,
    pub cover: Option<String>,
    pub play_url: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub source_url: String,
    pub site: Site,
    pub engine: Engine,
    pub title: String,
    pub author: Option<String>,
    pub duration_secs: Option<f64>,
    pub thumbnail: Option<String>,
    pub items: Vec<MediaItem>,
    pub limited_formats: bool,
    #[serde(skip)]
    pub sound: Option<SoundMeta>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionId {
    Video { item: u32, height: u32 },
    Audio { item: u32 },
    Image { item: u32 },
    Sound,
}

impl OptionId {
    pub fn parse(s: &str) -> Option<OptionId> {
        if s == "sound" {
            return Some(OptionId::Sound);
        }
        let (item, rest) = s.split_once('/')?;
        let item: u32 = item.parse().ok()?;
        match rest {
            "audio" => Some(OptionId::Audio { item }),
            "image" => Some(OptionId::Image { item }),
            _ => {
                let height = rest.strip_prefix("video:")?.parse().ok()?;
                Some(OptionId::Video { item, height })
            }
        }
    }
}

impl fmt::Display for OptionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OptionId::Video { item, height } => write!(f, "{item}/video:{height}"),
            OptionId::Audio { item } => write!(f, "{item}/audio"),
            OptionId::Image { item } => write!(f, "{item}/image"),
            OptionId::Sound => write!(f, "sound"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn option_id_roundtrip() {
        for s in ["0/video:1080", "3/audio", "2/image", "sound"] {
            assert_eq!(OptionId::parse(s).unwrap().to_string(), s);
        }
        assert!(OptionId::parse("x/video:abc").is_none());
    }

    #[test]
    fn option_note_serializes_internally_tagged() {
        let n = OptionNote::SourceAudio { ext: "m4a".into() };
        assert_eq!(serde_json::to_string(&n).unwrap(), r#"{"kind":"source_audio","ext":"m4a"}"#);
        assert_eq!(serde_json::to_string(&OptionNote::NoAudio).unwrap(), r#"{"kind":"no_audio"}"#);
    }

    #[test]
    fn media_info_serializes_camel_case_and_skips_sound() {
        let info = MediaInfo {
            source_url: "u".into(), site: Site::Tiktok, engine: Engine::YtDlp, title: "t".into(),
            author: None, duration_secs: Some(1.5), thumbnail: None, items: vec![], limited_formats: false,
            sound: Some(SoundMeta { title: "s".into(), artist: "a".into(), cover: None, play_url: "p".into() }),
        };
        let v = serde_json::to_value(&info).unwrap();
        assert_eq!(v["sourceUrl"], "u");
        assert_eq!(v["engine"], "yt_dlp");
        assert_eq!(v["durationSecs"], 1.5);
        assert!(v.get("sound").is_none());
    }
}
