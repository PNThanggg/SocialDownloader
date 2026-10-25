#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Mutex;

use crate::model::MediaInfo;

/// Resolved media keyed by URL, so a later download can look up what `resolve` returned.
#[derive(Default)]
pub struct ResolveCache(Mutex<HashMap<String, MediaInfo>>);

impl ResolveCache {
    pub fn get(&self, url: &str) -> Option<MediaInfo> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).get(url).cloned()
    }

    pub fn insert(&self, url: impl Into<String>, info: MediaInfo) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).insert(url.into(), info);
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
        c.insert("u", info.clone());
        assert_eq!(c.get("u"), Some(info));
    }
}
