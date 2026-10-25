use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};
use windows::core::PCWSTR;

use crate::errors::{ErrorCode, LumiError};

fn blob(data: &[u8]) -> Result<CRYPT_INTEGER_BLOB, LumiError> {
    let len = u32::try_from(data.len()).map_err(|_| LumiError::new(ErrorCode::Unknown, "DPAPI: data too large"))?;
    Ok(CRYPT_INTEGER_BLOB { cbData: len, pbData: data.as_ptr() as *mut u8 })
}

/// Copies the system-allocated output blob into a Vec, then frees it with `LocalFree`.
unsafe fn take(out: CRYPT_INTEGER_BLOB) -> Vec<u8> {
    if out.pbData.is_null() {
        return Vec::new();
    }
    let v = unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize) }.to_vec();
    unsafe { LocalFree(Some(HLOCAL(out.pbData.cast()))) };
    v
}

/// DPAPI `CryptProtectData`, current-user scope.
pub fn protect(data: &[u8]) -> Result<Vec<u8>, LumiError> {
    let input = blob(data)?;
    let mut out = CRYPT_INTEGER_BLOB::default();
    // SAFETY: `input` borrows `data` for the call (DPAPI only reads it); `out` is freed in `take`.
    unsafe { CryptProtectData(&input, PCWSTR::null(), None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out) }
        .map_err(|e| LumiError::new(ErrorCode::Unknown, format!("DPAPI protect failed: {e}")))?;
    Ok(unsafe { take(out) })
}

/// DPAPI `CryptUnprotectData`; fails for data protected by another user or machine.
pub fn unprotect(data: &[u8]) -> Result<Vec<u8>, LumiError> {
    let input = blob(data)?;
    let mut out = CRYPT_INTEGER_BLOB::default();
    // SAFETY: as in `protect`.
    unsafe { CryptUnprotectData(&input, None, None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out) }
        .map_err(|e| LumiError::new(ErrorCode::Unknown, format!("DPAPI unprotect failed: {e}")))?;
    Ok(unsafe { take(out) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpapi_roundtrip() {
        let sealed = protect(b"x").unwrap();
        assert_ne!(sealed, b"x");
        assert_eq!(unprotect(&sealed).unwrap(), b"x");
    }

    #[test]
    fn dpapi_rejects_garbage() {
        assert!(unprotect(b"not a dpapi blob").is_err());
    }
}
