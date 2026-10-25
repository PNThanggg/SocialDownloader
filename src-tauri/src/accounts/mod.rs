use std::path::PathBuf;

use crate::errors::LumiError;
use crate::model::Site;

/// Decrypted cookie jar on disk; the file is deleted when this is dropped.
pub struct TempCookieFile(pub PathBuf);

impl Drop for TempCookieFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Stub until Task 10 wires up DPAPI-backed accounts.
pub fn cookie_file_for(_site: Site) -> Result<Option<TempCookieFile>, LumiError> {
    Ok(None)
}
