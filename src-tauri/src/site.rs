#![allow(dead_code)]

use crate::model::Site;
use url::Url;

const HOSTS: [(&str, Site); 6] = [
    ("tiktok.com", Site::Tiktok),
    ("instagram.com", Site::Instagram),
    ("youtube.com", Site::Youtube),
    ("youtu.be", Site::Youtube),
    ("facebook.com", Site::Facebook),
    ("fb.watch", Site::Facebook),
];

/// None = not an http(s) URL.
pub fn detect_site(url: &str) -> Option<Site> {
    let u = Url::parse(url.trim()).ok()?;
    if !matches!(u.scheme(), "http" | "https") {
        return None;
    }
    let host = u.host_str()?.to_ascii_lowercase();
    let site = HOSTS
        .iter()
        .find(|(h, _)| host == *h || host.ends_with(&format!(".{h}")))
        .map_or(Site::Other, |(_, s)| *s);
    Some(site)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_site_variants() {
        for (u, s) in [
            ("https://vt.tiktok.com/ZSabc/", Site::Tiktok),
            ("https://www.tiktok.com/@a/video/1?is_from_webapp=1", Site::Tiktok),
            ("https://www.tiktok.com/@a/photo/7", Site::Tiktok),
            ("https://www.instagram.com/reel/C8x/?igsh=abc", Site::Instagram),
            ("https://instagram.com/p/C8x/", Site::Instagram),
            ("https://youtu.be/dQw4w9WgXcQ?si=x", Site::Youtube),
            ("https://m.youtube.com/shorts/abc", Site::Youtube),
            ("https://fb.watch/abc/", Site::Facebook),
            ("https://m.facebook.com/watch/?v=1", Site::Facebook),
            ("https://web.facebook.com/share/r/abc/", Site::Facebook),
            ("https://vimeo.com/1", Site::Other),
        ] {
            assert_eq!(detect_site(u), Some(s), "{u}");
        }
        assert_eq!(detect_site("not a url"), None);
        assert_eq!(detect_site("  https://youtu.be/x  "), Some(Site::Youtube));
    }

    #[test]
    fn detect_site_rejects_lookalikes_and_non_http() {
        assert_eq!(detect_site("https://nottiktok.com/x"), Some(Site::Other));
        assert_eq!(detect_site("https://tiktok.com.evil.io/x"), Some(Site::Other));
        assert_eq!(detect_site("ftp://www.tiktok.com/x"), None);
        assert_eq!(detect_site("file:///C:/x.mp4"), None);
    }
}
