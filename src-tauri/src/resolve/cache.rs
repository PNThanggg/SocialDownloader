#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::model::MediaInfo;

/// Resolved media keyed by URL, so a later download can look up what `resolve` returned.
/// Cheap to clone (shared), so a running job can refresh an entry.
#[derive(Default, Clone)]
pub struct ResolveCache(Arc<Mutex<HashMap<String, (Instant, MediaInfo)>>>);

impl ResolveCache {
    pub fn get(&self, url: &str) -> Option<MediaInfo> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).get(url).map(|(_, i)| i.clone())
    }

    /// How long ago `url` was resolved.
    pub fn age(&self, url: &str) -> Option<Duration> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).get(url).map(|(t, _)| t.elapsed())
    }

    pub fn insert(&self, url: impl Into<String>, info: MediaInfo) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).insert(url.into(), (Instant::now(), info));
    }

    /// Test helper: an entry resolved `ago`.
    #[cfg(test)]
    pub fn insert_aged(&self, url: impl Into<String>, info: MediaInfo, ago: Duration) {
        let at = Instant::now().checked_sub(ago).expect("uptime longer than `ago`");
        self.0.lock().unwrap_or_else(|e| e.into_inner()).insert(url.into(), (at, info));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Engine, Site};

    #[test]
    fn insert_and_get() {
        let info = MediaInfo {
            source_url: "u".into(), site: Site::Other, engine: Engine::YtDlp, title: "t".into(),
            author: None, duration_secs: None, thumbnail: None, items: vec![], limited_formats: false, sound: None,
        };
        let c = ResolveCache::default();
        assert!(c.get("u").is_none());
        assert!(c.age("u").is_none());
        c.insert("u", info.clone());
        assert_eq!(c.get("u"), Some(info.clone()));
        assert!(c.age("u").unwrap() < Duration::from_secs(5));
        // Clones share the entries.
        let other = c.clone();
        other.insert_aged("v", info, Duration::from_secs(60));
        assert!(c.age("v").unwrap() >= Duration::from_secs(60));
    }
}
