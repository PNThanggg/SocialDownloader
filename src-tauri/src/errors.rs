#![allow(dead_code)]

use once_cell::sync::Lazy;
use regex::{Regex, RegexBuilder};
use serde::Serialize;

use crate::model::Site;
use crate::tools::Tool;

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum ErrorCode {
    UnsupportedUrl,
    LoginRequired { site: Site },
    PrivateOrRemoved,
    ExtractorBroken,
    Network,
    RateLimited,
    YoutubeBotCheck,
    PlaylistNotSupported,
    ToolMissing { tool: Tool },
    Unknown,
}

/// `detail` is the raw log shown only behind "Show details".
#[derive(Serialize, Debug, Clone, PartialEq, thiserror::Error)]
#[error("{error:?}: {detail}")]
pub struct LumiError {
    pub error: ErrorCode,
    pub detail: String,
}

impl LumiError {
    pub fn new(error: ErrorCode, detail: impl Into<String>) -> Self {
        Self { error, detail: detail.into() }
    }
}

// `tauri::ipc::InvokeError: From<T: Serialize>` already covers LumiError (JSON value).

fn re(pattern: &str) -> Regex {
    RegexBuilder::new(pattern).case_insensitive(true).build().expect("valid regex")
}

static BOT_CHECK: Lazy<Regex> = Lazy::new(|| re(r"sign in to confirm you(?:’|')re not a bot"));
static LOGIN: Lazy<Regex> = Lazy::new(|| {
    re(r"login required|log in|only available for registered users|AuthorizationError|cookies? .*required")
});
static PRIVATE: Lazy<Regex> = Lazy::new(|| {
    re(r"private video|video unavailable|NotFoundError|has been removed|(?:video|content) (?:is )?(?:not|no longer) available")
});
static RATE_LIMIT: Lazy<Regex> = Lazy::new(|| re(r"\b429\b|too many requests"));
static NETWORK: Lazy<Regex> = Lazy::new(|| {
    re(r"getaddrinfo failed|failed to resolve|urlopen error|timed out|connection (?:reset|refused|aborted)|network is unreachable")
});
static UNSUPPORTED: Lazy<Regex> = Lazy::new(|| re(r"unsupported url"));
static EXTRACTOR: Lazy<Regex> = Lazy::new(|| {
    re(r"please report this issue|unable to extract|unable to download webpage")
});

pub fn classify(site: Site, stderr: &str) -> ErrorCode {
    if BOT_CHECK.is_match(stderr) {
        ErrorCode::YoutubeBotCheck
    } else if LOGIN.is_match(stderr) {
        ErrorCode::LoginRequired { site }
    } else if PRIVATE.is_match(stderr) {
        ErrorCode::PrivateOrRemoved
    } else if RATE_LIMIT.is_match(stderr) {
        ErrorCode::RateLimited
    } else if NETWORK.is_match(stderr) {
        ErrorCode::Network
    } else if UNSUPPORTED.is_match(stderr) {
        ErrorCode::UnsupportedUrl
    } else if EXTRACTOR.is_match(stderr) {
        ErrorCode::ExtractorBroken
    } else {
        ErrorCode::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ErrorCode::*;

    #[test]
    fn classify_cases() {
        let cases: Vec<(Site, &str, ErrorCode)> = vec![
            (Site::Other, "ERROR: Unsupported URL: https://x", UnsupportedUrl),
            (Site::Instagram, "[Instagram] C8x: Requested content is not available, rate-limit reached or login required", LoginRequired { site: Site::Instagram }),
            (Site::Instagram, "[instagram][error] AuthorizationError: Login required", LoginRequired { site: Site::Instagram }),
            (Site::Facebook, "ERROR: [facebook] 1: This video is only available for registered users", LoginRequired { site: Site::Facebook }),
            (Site::Youtube, "ERROR: [youtube] x: Private video", PrivateOrRemoved),
            (Site::Youtube, "ERROR: [youtube] x: Video unavailable", PrivateOrRemoved),
            (Site::Instagram, "[instagram][error] NotFoundError", PrivateOrRemoved),
            (Site::Youtube, "Sign in to confirm you’re not a bot", YoutubeBotCheck),
            (Site::Youtube, "Sign in to confirm you're not a bot", YoutubeBotCheck),
            (Site::Tiktok, "ERROR: [TikTok] 1: Unable to extract universal data; please report this issue", ExtractorBroken),
            (Site::Other, "<urlopen error [Errno 11001] getaddrinfo failed>", Network),
            (Site::Other, "Failed to resolve 'example.com'", Network),
            (Site::Tiktok, "HTTP Error 429: Too Many Requests", RateLimited),
            (Site::Instagram, "[instagram][error] HttpError: '429 Too Many Requests'", RateLimited),
            (Site::Youtube, "ERROR: [youtube] x: Requested format is not available", Unknown),
            (Site::Other, "something completely different", Unknown),
            (Site::Tiktok, "please report this issue ... login required", LoginRequired { site: Site::Tiktok }),
        ];
        for (site, text, expected) in cases {
            assert_eq!(classify(site, text), expected, "input: {text}");
        }
    }

    #[test]
    fn lumi_error_converts_to_invoke_error() {
        let e = LumiError::new(Unknown, "raw log");
        let expected = serde_json::json!({"error": {"code": "unknown"}, "detail": "raw log"});
        assert_eq!(serde_json::to_value(&e).unwrap(), expected);
        let inv: tauri::ipc::InvokeError = e.into();
        assert_eq!(inv.0, expected);
    }

    #[test]
    fn error_code_json_shapes() {
        let v = serde_json::to_value(LoginRequired { site: Site::Instagram }).unwrap();
        assert_eq!(v, serde_json::json!({"code": "login_required", "site": "instagram"}));
        let v = serde_json::to_value(ToolMissing { tool: Tool::YtDlp }).unwrap();
        assert_eq!(v, serde_json::json!({"code": "tool_missing", "tool": "yt_dlp"}));
    }
}
