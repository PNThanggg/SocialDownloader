use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use tauri::webview::Cookie;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};
use tokio::sync::oneshot;

use super::{account, save, status, store_path, to_netscape, webdata_dir, Account, AccountStatus};
use crate::errors::{ErrorCode, LumiError};
use crate::model::Site;

const POLL: Duration = Duration::from_secs(1);

fn err(detail: impl Into<String>) -> LumiError {
    LumiError::new(ErrorCode::Unknown, detail)
}

fn require(site: Site) -> Result<&'static Account, LumiError> {
    account(site).ok_or_else(|| err(format!("no account login for {site:?}")))
}

/// Instagram: `sessionid`. Facebook: both `c_user` and `xs`.
fn login_complete(site: Site, cookies: &[Cookie<'_>]) -> bool {
    let has = |name: &str| cookies.iter().any(|c| c.name() == name && !c.value().is_empty());
    match site {
        Site::Instagram => has("sessionid"),
        Site::Facebook => has("c_user") && has("xs"),
        _ => false,
    }
}

/// Fires once when `w` is destroyed (user close or ours). Dropped sender also completes the receiver.
fn on_destroyed(w: &WebviewWindow) -> oneshot::Receiver<()> {
    let (tx, rx) = oneshot::channel();
    let tx = Mutex::new(Some(tx));
    w.on_window_event(move |e| {
        if matches!(e, WindowEvent::Destroyed)
            && let Some(tx) = tx.lock().unwrap_or_else(|e| e.into_inner()).take()
        {
            let _ = tx.send(());
        }
    });
    rx
}

/// Waits until `label` is gone: the event for promptness, the label check so a missed event can't hang us.
async fn wait_closed(app: &AppHandle, label: &str, closed: &mut oneshot::Receiver<()>) {
    loop {
        tokio::select! {
            _ = &mut *closed => return,
            _ = tokio::time::sleep(POLL) => {}
        }
        if app.get_webview_window(label).is_none() {
            return;
        }
    }
}

/// `cookies*` deadlock on the main thread or inside sync handlers, so they run on a blocking thread.
async fn cookies_of(w: &WebviewWindow, url: Option<&str>) -> Option<Vec<Cookie<'static>>> {
    let w = w.clone();
    let url = match url {
        Some(u) => Some(u.parse().ok()?),
        None => None,
    };
    tauri::async_runtime::spawn_blocking(move || match url {
        Some(u) => w.cookies_for_url(u),
        None => w.cookies(),
    })
    .await
    .ok()?
    .ok()
}

fn focus(w: &WebviewWindow) {
    let _ = w.unminimize();
    let _ = w.set_focus();
}

pub(super) async fn connect(app: AppHandle, site: Site) -> Result<AccountStatus, LumiError> {
    let a = require(site)?;
    let label = format!("login-{}", a.slug);
    // One login window per site: a second Connect focuses it and waits for it to close.
    if let Some(w) = app.get_webview_window(&label) {
        focus(&w);
        wait_closed(&app, &label, &mut on_destroyed(&w)).await;
        return Ok(status(a));
    }
    let url = a.login_url.parse().map_err(|e| err(format!("login url: {e}")))?;
    // No capability lists `login-*` and none has `remote` URLs, so this external page gets no IPC.
    let built = WebviewWindowBuilder::new(&app, &label, WebviewUrl::External(url))
        .title(a.title)
        .inner_size(480.0, 720.0)
        .data_directory(webdata_dir())
        .build();
    let w = match built {
        Ok(w) => w,
        // Lost a race with a concurrent Connect for the same site.
        Err(e) => match app.get_webview_window(&label) {
            Some(w) => {
                focus(&w);
                wait_closed(&app, &label, &mut on_destroyed(&w)).await;
                return Ok(status(a));
            }
            None => return Err(err(format!("open login window: {e}"))),
        },
    };
    let mut closed = on_destroyed(&w);
    loop {
        tokio::select! {
            _ = &mut closed => return Ok(status(a)),
            _ = tokio::time::sleep(POLL) => {}
        }
        if app.get_webview_window(&label).is_none() {
            return Ok(status(a));
        }
        let Some(cookies) = cookies_of(&w, Some(a.cookie_url)).await else { continue };
        if login_complete(site, &cookies) {
            let saved = save(a, &to_netscape(&cookies));
            let _ = w.destroy();
            saved?;
            return Ok(AccountStatus { site, connected: true });
        }
    }
}

pub(super) async fn disconnect(app: AppHandle, site: Site) -> Result<(), LumiError> {
    let a = require(site)?;
    match std::fs::remove_file(store_path(a)) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(err(format!("delete cookie jar: {e}"))),
    }
    if let Some(w) = app.get_webview_window(&format!("login-{}", a.slug)) {
        let _ = w.destroy();
    }
    clear_profile(&app, &format!("logout-{}", a.slug), webdata_dir()).await
}

/// Clears every kind of browsing data in the WebView2 profile at `dir` through a hidden window.
/// The clear is fire-and-forget, so the window stays until the profile reports no cookies (max 10 s).
async fn clear_profile(app: &AppHandle, label: &str, dir: PathBuf) -> Result<(), LumiError> {
    // A sign-out for this site is already clearing: wait for it instead of racing it.
    if let Some(running) = app.get_webview_window(label) {
        wait_closed(app, label, &mut on_destroyed(&running)).await;
        return Ok(());
    }
    let url = "about:blank".parse().map_err(|e| err(format!("blank url: {e}")))?;
    let w = WebviewWindowBuilder::new(app, label, WebviewUrl::External(url))
        .data_directory(dir)
        .visible(false)
        .build()
        .map_err(|e| err(format!("open sign-out window: {e}")))?;
    let result = match w.clear_all_browsing_data() {
        Err(e) => Err(err(format!("clear browsing data: {e}"))),
        Ok(()) => {
            let mut cleared = false;
            for _ in 0..50 {
                if cookies_of(&w, None).await.is_some_and(|c| c.is_empty()) {
                    cleared = true;
                    break;
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            if cleared { Ok(()) } else { Err(err("browsing data still present after clearing")) }
        }
    };
    let _ = w.destroy();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jar(names: &[&str]) -> Vec<Cookie<'static>> {
        names.iter().map(|n| Cookie::build((n.to_string(), "v")).domain("x.com").build()).collect()
    }

    #[test]
    fn login_complete_rules() {
        assert!(login_complete(Site::Instagram, &jar(&["csrftoken", "sessionid"])));
        assert!(!login_complete(Site::Instagram, &jar(&["csrftoken", "mid"])));
        assert!(!login_complete(Site::Instagram, &[Cookie::new("sessionid", "")]));
        assert!(login_complete(Site::Facebook, &jar(&["datr", "c_user", "xs"])));
        assert!(!login_complete(Site::Facebook, &jar(&["c_user"])));
        assert!(!login_complete(Site::Facebook, &jar(&["xs"])));
        assert!(!login_complete(Site::Youtube, &jar(&["sessionid", "c_user", "xs"])));
    }
}
