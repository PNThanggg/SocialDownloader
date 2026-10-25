//! Download history: one entry per post link, newest first, in `%LOCALAPPDATA%\Lumi\history.json`.
//! It keeps the post's own link (never the expiring direct URL), so "Download again" reads it afresh.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::errors::{ErrorCode, LumiError};
use crate::model::{MediaInfo, Site};
use crate::resolve::ResolveCache;
use crate::site::detect_site;

/// Older entries are dropped past this.
pub const MAX_ENTRIES: usize = 300;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    /// The post's link (`MediaInfo::source_url`): what "Download again" reads.
    pub url: String,
    pub site: Site,
    pub title: String,
    pub author: Option<String>,
    /// What was saved from this link, most recent first: a file extension (`mp4`, `mp3`, `jpg`) or `all`.
    pub formats: Vec<String>,
    /// The last file (or folder, for "Download all") saved from it.
    pub path: String,
    /// Unix milliseconds of the last download.
    pub at: u64,
}

/// `all` for "Download all", else the extension of the file actually written (yt-dlp may pick its own).
fn format_of(option_id: &str, path: &str) -> String {
    if option_id == "all" {
        return "all".into();
    }
    Path::new(path).extension().map_or_else(|| "file".into(), |e| e.to_string_lossy().to_lowercase())
}

/// The entry for one finished download; `info` is what `resolve` returned for `url`, if still cached.
pub fn entry_for(info: Option<&MediaInfo>, url: &str, option_id: &str, path: &str, at: u64) -> HistoryEntry {
    let (site, title, author) = match info {
        Some(i) => (i.site, i.title.clone(), i.author.clone()),
        None => (detect_site(url).unwrap_or(Site::Other), String::new(), None),
    };
    HistoryEntry {
        url: url.to_owned(),
        site,
        title,
        author,
        formats: vec![format_of(option_id, path)],
        path: path.to_owned(),
        at,
    }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

fn unknown(detail: impl Into<String>) -> LumiError {
    LumiError::new(ErrorCode::Unknown, detail)
}

/// The history file, loaded on first use and written back after every change.
#[derive(Clone)]
pub struct History {
    file: PathBuf,
    entries: Arc<Mutex<Option<Vec<HistoryEntry>>>>,
}

impl History {
    pub fn new(file: PathBuf) -> Self {
        Self { file, entries: Arc::default() }
    }

    /// `%LOCALAPPDATA%\Lumi\history.json`.
    pub fn default_file() -> PathBuf {
        dirs::data_local_dir().unwrap_or_else(std::env::temp_dir).join("Lumi").join("history.json")
    }

    /// A missing file is an empty history. One that can't be read is set aside as `.bad`
    /// rather than overwritten, so a bug never silently wipes it.
    fn load(&self) -> Vec<HistoryEntry> {
        let Ok(text) = std::fs::read_to_string(&self.file) else { return vec![] };
        serde_json::from_str(&text).unwrap_or_else(|_| {
            let _ = std::fs::rename(&self.file, self.file.with_extension("json.bad"));
            vec![]
        })
    }

    /// Writes to a sibling temp file, then renames it over the old one.
    fn save(&self, entries: &[HistoryEntry]) -> Result<(), LumiError> {
        let io = |e: std::io::Error| unknown(format!("{}: {e}", self.file.display()));
        if let Some(dir) = self.file.parent() {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        let json = serde_json::to_string(entries).map_err(|e| unknown(e.to_string()))?;
        let tmp = self.file.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(io)?;
        std::fs::rename(&tmp, &self.file).map_err(io)
    }

    fn change(&self, f: impl FnOnce(&mut Vec<HistoryEntry>)) -> Result<(), LumiError> {
        let mut guard = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let entries = guard.get_or_insert_with(|| self.load());
        f(entries);
        self.save(entries)
    }

    pub fn list(&self) -> Vec<HistoryEntry> {
        let mut guard = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        guard.get_or_insert_with(|| self.load()).clone()
    }

    /// Puts `entry` first. An earlier entry for the same link is merged in: its formats are kept
    /// after the new one, and its title or author fills in what `entry` lacks.
    pub fn record(&self, mut entry: HistoryEntry) -> Result<(), LumiError> {
        self.change(|entries| {
            if let Some(i) = entries.iter().position(|e| e.url == entry.url) {
                let old = entries.remove(i);
                let new_formats = std::mem::take(&mut entry.formats);
                entry.formats =
                    new_formats.iter().cloned().chain(old.formats.into_iter().filter(|f| !new_formats.contains(f))).collect();
                if entry.title.is_empty() {
                    entry.title = old.title;
                }
                if entry.author.is_none() {
                    entry.author = old.author;
                }
            }
            entries.insert(0, entry);
            entries.truncate(MAX_ENTRIES);
        })
    }

    pub fn remove(&self, url: &str) -> Result<(), LumiError> {
        self.change(|entries| entries.retain(|e| e.url != url))
    }

    pub fn clear(&self) -> Result<(), LumiError> {
        self.change(Vec::clear)
    }
}

/// Called for every finished download, from any window; tells the UI with `lumi://history`.
pub fn record_done(app: &AppHandle, url: &str, option_id: &str, path: &str) {
    let info = app.state::<ResolveCache>().get(url);
    let entry = entry_for(info.as_ref(), url, option_id, path, now_ms());
    if app.state::<History>().record(entry).is_ok() {
        let _ = app.emit("lumi://history", ());
    }
}

#[tauri::command]
pub fn history_list(history: State<'_, History>) -> Vec<HistoryEntry> {
    history.list()
}

#[tauri::command]
pub fn history_remove(app: AppHandle, history: State<'_, History>, url: String) -> Result<(), LumiError> {
    history.remove(&url)?;
    let _ = app.emit("lumi://history", ());
    Ok(())
}

#[tauri::command]
pub fn history_clear(app: AppHandle, history: State<'_, History>) -> Result<(), LumiError> {
    history.clear()?;
    let _ = app.emit("lumi://history", ());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Engine;

    const URL: &str = "https://www.tiktok.com/@a/video/1";

    fn info() -> MediaInfo {
        MediaInfo {
            source_url: URL.into(), site: Site::Tiktok, engine: Engine::YtDlp, title: "Evening chill".into(),
            author: Some("@a".into()), duration_secs: None, thumbnail: None, items: vec![], limited_formats: false,
            sound: None,
        }
    }

    fn history() -> (tempfile::TempDir, History) {
        let dir = tempfile::tempdir().unwrap();
        let h = History::new(dir.path().join("Lumi").join("history.json"));
        (dir, h)
    }

    #[test]
    fn entry_takes_metadata_from_the_resolved_info() {
        let e = entry_for(Some(&info()), URL, "0/audio", r"D:\x\Evening chill.MP3", 42);
        assert_eq!(e.title, "Evening chill");
        assert_eq!(e.author.as_deref(), Some("@a"));
        assert_eq!(e.site, Site::Tiktok);
        assert_eq!(e.formats, ["mp3"]);
        assert_eq!(e.at, 42);
        // yt-dlp chose another extension: the file on disk wins.
        assert_eq!(entry_for(None, URL, "0/audio", r"D:\x\a.m4a", 0).formats, ["m4a"]);
        assert_eq!(entry_for(None, URL, "all", r"D:\x", 0).formats, ["all"]);
        let bare = entry_for(None, "https://youtu.be/x", "0/video:720", r"D:\x\a.mp4", 0);
        assert_eq!((bare.site, bare.title.as_str(), bare.author), (Site::Youtube, "", None));
    }

    #[test]
    fn record_moves_a_link_to_the_top_and_merges_formats() {
        let (_d, h) = history();
        h.record(entry_for(Some(&info()), URL, "0/video:720", r"D:\a.mp4", 1)).unwrap();
        h.record(entry_for(None, "https://youtu.be/x", "0/audio", r"D:\b.mp3", 2)).unwrap();
        h.record(entry_for(None, URL, "0/audio", r"D:\a.mp3", 3)).unwrap();
        let list = h.list();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].url, URL);
        assert_eq!(list[0].formats, ["mp3", "mp4"]);
        assert_eq!(list[0].path, r"D:\a.mp3");
        assert_eq!(list[0].at, 3);
        // The later download had no cached info: the earlier title and author stay.
        assert_eq!(list[0].title, "Evening chill");
        assert_eq!(list[0].author.as_deref(), Some("@a"));
        // The same format again is not listed twice.
        h.record(entry_for(None, URL, "0/video:1080", r"D:\c.mp4", 4)).unwrap();
        assert_eq!(h.list()[0].formats, ["mp4", "mp3"]);
    }

    #[test]
    fn survives_a_restart() {
        let (_d, h) = history();
        h.record(entry_for(Some(&info()), URL, "0/audio", r"D:\a.mp3", 1)).unwrap();
        let again = History::new(h.file.clone());
        assert_eq!(again.list(), h.list());
        assert!(!h.file.with_extension("json.tmp").exists());
    }

    #[test]
    fn keeps_only_the_newest_entries() {
        let (_d, h) = history();
        for i in 0..MAX_ENTRIES + 5 {
            h.record(entry_for(None, &format!("https://youtu.be/{i}"), "0/audio", r"D:\a.mp3", i as u64)).unwrap();
        }
        let list = h.list();
        assert_eq!(list.len(), MAX_ENTRIES);
        assert_eq!(list[0].url, format!("https://youtu.be/{}", MAX_ENTRIES + 4));
        assert_eq!(list.last().unwrap().url, "https://youtu.be/5");
    }

    #[test]
    fn remove_and_clear() {
        let (_d, h) = history();
        h.record(entry_for(None, URL, "0/audio", r"D:\a.mp3", 1)).unwrap();
        h.record(entry_for(None, "https://youtu.be/x", "0/audio", r"D:\b.mp3", 2)).unwrap();
        h.remove(URL).unwrap();
        assert_eq!(h.list().iter().map(|e| e.url.as_str()).collect::<Vec<_>>(), ["https://youtu.be/x"]);
        h.clear().unwrap();
        assert!(h.list().is_empty());
        assert!(History::new(h.file.clone()).list().is_empty());
    }

    #[test]
    fn an_unreadable_file_is_set_aside_not_overwritten() {
        let (_d, h) = history();
        std::fs::create_dir_all(h.file.parent().unwrap()).unwrap();
        std::fs::write(&h.file, "{not json").unwrap();
        assert!(h.list().is_empty());
        assert_eq!(std::fs::read_to_string(h.file.with_extension("json.bad")).unwrap(), "{not json");
        h.record(entry_for(None, URL, "0/audio", r"D:\a.mp3", 1)).unwrap();
        assert_eq!(History::new(h.file.clone()).list().len(), 1);
    }

    #[test]
    fn entry_json_shape() {
        let v = serde_json::to_value(entry_for(Some(&info()), URL, "all", r"D:\x", 7)).unwrap();
        assert_eq!(
            v,
            serde_json::json!({
                "url": URL, "site": "tiktok", "title": "Evening chill", "author": "@a",
                "formats": ["all"], "path": r"D:\x", "at": 7
            })
        );
    }
}
