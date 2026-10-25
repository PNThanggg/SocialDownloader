use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use crate::errors::{ErrorCode, LumiError};
use crate::model::Site;

const MAX_BYTES: usize = 5 * 1024 * 1024;
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

fn referer_for(site: Site) -> Option<&'static str> {
    match site {
        Site::Tiktok => Some("https://www.tiktok.com/"),
        Site::Instagram => Some("https://www.instagram.com/"),
        Site::Facebook => Some("https://www.facebook.com/"),
        Site::Youtube | Site::Other => None,
    }
}

fn net_err(e: reqwest::Error) -> LumiError {
    LumiError::new(ErrorCode::Network, e.to_string())
}

fn too_large() -> LumiError {
    LumiError::new(ErrorCode::Unknown, "thumbnail larger than 5MB")
}

async fn fetch(url: &str, referer: Option<&str>) -> Result<String, LumiError> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(30))
        .build()
        .map_err(net_err)?;
    let mut req = client.get(url);
    if let Some(r) = referer {
        req = req.header(reqwest::header::REFERER, r);
    }
    let mut resp = req.send().await.map_err(net_err)?.error_for_status().map_err(net_err)?;
    if resp.content_length().is_some_and(|n| n > MAX_BYTES as u64) {
        return Err(too_large());
    }
    let mime = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("image/jpeg")
        .to_owned();
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(net_err)? {
        if body.len() + chunk.len() > MAX_BYTES {
            return Err(too_large());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(format!("data:{mime};base64,{}", STANDARD.encode(body)))
}

/// Returns the image as a `data:` URL so the webview needs no cross-origin access to CDNs
/// that reject hotlinking.
#[tauri::command]
pub async fn fetch_thumbnail(url: String, site: Site) -> Result<String, LumiError> {
    fetch(&url, referer_for(site)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Serves one canned response and returns the request head it received.
    async fn serve_once(response: Vec<u8>) -> (String, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/t.png", listener.local_addr().unwrap());
        let h = tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 4096];
            let n = s.read(&mut buf).await.unwrap();
            s.write_all(&response).await.unwrap();
            s.shutdown().await.ok();
            String::from_utf8_lossy(&buf[..n]).into_owned()
        });
        (url, h)
    }

    fn http(content_type: &str, body: &[u8]) -> Vec<u8> {
        let mut r = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        r.extend_from_slice(body);
        r
    }

    #[test]
    fn referer_per_site() {
        assert_eq!(referer_for(Site::Tiktok), Some("https://www.tiktok.com/"));
        assert_eq!(referer_for(Site::Instagram), Some("https://www.instagram.com/"));
        assert_eq!(referer_for(Site::Facebook), Some("https://www.facebook.com/"));
        assert_eq!(referer_for(Site::Youtube), None);
        assert_eq!(referer_for(Site::Other), None);
    }

    #[tokio::test]
    async fn returns_data_url_and_sends_headers() {
        let (url, req) = serve_once(http("image/png; charset=x", b"abc")).await;
        let out = fetch(&url, Some("https://www.tiktok.com/")).await.unwrap();
        assert_eq!(out, "data:image/png;base64,YWJj");
        let head = req.await.unwrap().to_ascii_lowercase();
        assert!(head.contains("referer: https://www.tiktok.com/"));
        assert!(head.contains("user-agent: mozilla/5.0"));
    }

    #[tokio::test]
    async fn refuses_oversized_body() {
        let (url, _req) = serve_once(http("image/jpeg", &vec![0u8; MAX_BYTES + 1])).await;
        let err = fetch(&url, None).await.unwrap_err();
        assert_eq!(err.error, ErrorCode::Unknown);
    }

    #[tokio::test]
    async fn connection_failure_is_network() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", l.local_addr().unwrap());
        drop(l);
        assert_eq!(fetch(&url, None).await.unwrap_err().error, ErrorCode::Network);
    }
}
