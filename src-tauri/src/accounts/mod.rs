mod dpapi;
mod login;
mod netscape;

use std::path::PathBuf;

use serde::Serialize;

use crate::errors::{ErrorCode, LumiError};
use crate::model::Site;

pub use dpapi::{protect, unprotect};
pub use netscape::to_netscape;

/// A site with in-app login.
pub(crate) struct Account {
    pub site: Site,
    pub slug: &'static str,
    pub title: &'static str,
    pub login_url: &'static str,
    /// `cookies_for_url` target; also the success check's cookie source.
    pub cookie_url: &'static str,
}

pub(crate) const ACCOUNTS: [Account; 2] = [
    Account {
        site: Site::Instagram,
        slug: "instagram",
        title: "Lumi — Instagram",
        login_url: "https://www.instagram.com/accounts/login/",
        cookie_url: "https://www.instagram.com/",
    },
    Account {
        site: Site::Facebook,
        slug: "facebook",
        title: "Lumi — Facebook",
        login_url: "https://www.facebook.com/login/",
        cookie_url: "https://www.facebook.com/",
    },
];

pub(crate) fn account(site: Site) -> Option<&'static Account> {
    ACCOUNTS.iter().find(|a| a.site == site)
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccountStatus {
    pub site: Site,
    pub connected: bool,
}

/// Decrypted cookie jar on disk; the file is deleted when this is dropped.
pub struct TempCookieFile(pub PathBuf);

impl Drop for TempCookieFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// `%LOCALAPPDATA%\Lumi\webdata\accounts`: the login windows' WebView2 profile (mode B reuses it).
pub fn webdata_dir() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(std::env::temp_dir).join("Lumi").join("webdata").join("accounts")
}

fn store_path(a: &Account) -> PathBuf {
    crate::tools::paths::accounts_dir().join(format!("{}.cookies.bin", a.slug))
}

fn status(a: &Account) -> AccountStatus {
    AccountStatus { site: a.site, connected: store_path(a).exists() }
}

/// Encrypts `netscape` with DPAPI into `accounts\<site>.cookies.bin` (write-then-rename).
fn save(a: &Account, netscape: &str) -> Result<(), LumiError> {
    let sealed = protect(netscape.as_bytes())?;
    let path = store_path(a);
    let part = path.with_extension("bin.part");
    let io = |what: &str, e: std::io::Error| LumiError::new(ErrorCode::Unknown, format!("{what}: {e}"));
    std::fs::create_dir_all(crate::tools::paths::accounts_dir()).map_err(|e| io("create accounts dir", e))?;
    std::fs::write(&part, &sealed).map_err(|e| io("write cookie jar", e))?;
    std::fs::rename(&part, &path).map_err(|e| {
        let _ = std::fs::remove_file(&part);
        io("store cookie jar", e)
    })
}

/// Instagram/Facebook only, and only when connected: decrypts the jar to a fresh `tmp\cookies-<uuid>.txt`
/// (unique per call, so concurrent jobs on one site never share or delete each other's file).
pub fn cookie_file_for(site: Site) -> Result<Option<TempCookieFile>, LumiError> {
    let Some(a) = account(site) else { return Ok(None) };
    let sealed = match std::fs::read(store_path(a)) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(LumiError::new(ErrorCode::Unknown, format!("read cookie jar: {e}"))),
    };
    let plain = unprotect(&sealed)?;
    let tmp = crate::tools::paths::tmp_dir();
    std::fs::create_dir_all(&tmp)
        .map_err(|e| LumiError::new(ErrorCode::Unknown, format!("create tmp dir: {e}")))?;
    // Owned before writing so a failed write still deletes whatever reached the disk.
    let file = TempCookieFile(tmp.join(format!("cookies-{}.txt", uuid::Uuid::new_v4())));
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&file.0)
        .and_then(|mut f| std::io::Write::write_all(&mut f, &plain))
        .map_err(|e| LumiError::new(ErrorCode::Unknown, format!("write temp cookie file: {e}")))?;
    Ok(Some(file))
}

#[tauri::command]
pub fn accounts_status() -> Vec<AccountStatus> {
    ACCOUNTS.iter().map(status).collect()
}

/// Opens (or focuses) the login window; resolves when login completes or the window closes.
#[tauri::command]
pub async fn connect_account(app: tauri::AppHandle, site: Site) -> Result<AccountStatus, LumiError> {
    login::connect(app, site).await
}

/// Deletes the cookie jar, then clears the login WebView profile.
#[tauri::command]
pub async fn disconnect_account(app: tauri::AppHandle, site: Site) -> Result<(), LumiError> {
    login::disconnect(app, site).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::paths::TEST_ENV_LOCK;
    use std::sync::MutexGuard;

    struct Dirs {
        _g: MutexGuard<'static, ()>,
        accounts: tempfile::TempDir,
        tmp: tempfile::TempDir,
    }

    impl Dirs {
        fn new() -> Dirs {
            let g = TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let d = Dirs { _g: g, accounts: tempfile::tempdir().unwrap(), tmp: tempfile::tempdir().unwrap() };
            unsafe {
                std::env::set_var("LUMI_ACCOUNTS_DIR", d.accounts.path());
                std::env::set_var("LUMI_TMP_DIR", d.tmp.path().join("tmp"));
            }
            d
        }
    }

    impl Drop for Dirs {
        fn drop(&mut self) {
            unsafe {
                std::env::remove_var("LUMI_ACCOUNTS_DIR");
                std::env::remove_var("LUMI_TMP_DIR");
            }
        }
    }

    #[test]
    fn temp_cookie_file_deleted_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("cookies-x.txt");
        std::fs::write(&p, "secret").unwrap();
        drop(TempCookieFile(p.clone()));
        assert!(!p.exists());
    }

    #[test]
    fn cookie_file_for_youtube_is_none() {
        let d = Dirs::new();
        // Even a stray jar named after an unsupported site is ignored.
        std::fs::write(d.accounts.path().join("youtube.cookies.bin"), protect(b"x").unwrap()).unwrap();
        assert!(cookie_file_for(Site::Youtube).unwrap().is_none());
        assert!(cookie_file_for(Site::Tiktok).unwrap().is_none());
    }

    #[test]
    fn cookie_file_for_not_connected_is_none() {
        let _d = Dirs::new();
        assert!(cookie_file_for(Site::Instagram).unwrap().is_none());
        assert_eq!(
            accounts_status(),
            [
                AccountStatus { site: Site::Instagram, connected: false },
                AccountStatus { site: Site::Facebook, connected: false },
            ]
        );
    }

    #[test]
    fn saved_jar_is_encrypted_and_decrypts_to_unique_temp_files() {
        let d = Dirs::new();
        let jar = "# Netscape HTTP Cookie File\n.instagram.com\tTRUE\t/\tTRUE\t0\tsessionid\tSECRET\n";
        save(account(Site::Instagram).unwrap(), jar).unwrap();

        let bin = d.accounts.path().join("instagram.cookies.bin");
        let sealed = std::fs::read(&bin).unwrap();
        assert!(!sealed.windows(6).any(|w| w == b"SECRET"), "jar must be encrypted at rest");
        assert_eq!(std::fs::read_dir(d.accounts.path()).unwrap().count(), 1, "no leftover temp file");
        assert!(accounts_status().contains(&AccountStatus { site: Site::Instagram, connected: true }));
        assert!(cookie_file_for(Site::Facebook).unwrap().is_none());

        let a = cookie_file_for(Site::Instagram).unwrap().unwrap();
        let b = cookie_file_for(Site::Instagram).unwrap().unwrap();
        assert_ne!(a.0, b.0, "concurrent jobs each own their file");
        for f in [&a, &b] {
            assert_eq!(f.0.parent().unwrap(), d.tmp.path().join("tmp"));
            let name = f.0.file_name().unwrap().to_str().unwrap();
            assert!(name.starts_with("cookies-") && name.ends_with(".txt"), "{name}");
            assert_eq!(std::fs::read_to_string(&f.0).unwrap(), jar);
        }
        let (pa, pb) = (a.0.clone(), b.0.clone());
        drop(a);
        assert!(!pa.exists() && pb.exists());
        drop(b);
        assert!(!pb.exists());
    }

    #[test]
    fn corrupt_jar_errors_without_leaving_a_file() {
        let d = Dirs::new();
        std::fs::write(d.accounts.path().join("facebook.cookies.bin"), b"garbage").unwrap();
        let err = cookie_file_for(Site::Facebook).err().expect("corrupt jar must fail");
        assert!(!err.detail.contains("garbage"));
        let tmp = d.tmp.path().join("tmp");
        assert!(!tmp.exists() || std::fs::read_dir(&tmp).unwrap().count() == 0);
    }
}
