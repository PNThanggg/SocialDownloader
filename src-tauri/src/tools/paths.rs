use std::path::PathBuf;

use super::Tool;

#[cfg(test)]
pub(crate) static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// `%LOCALAPPDATA%\Lumi\bin`. Not created here. Tests redirect it with `LUMI_BIN_DIR`; a release
/// build ignores that variable, so the environment can never point Lumi at other executables.
pub fn bin_dir() -> PathBuf {
    #[cfg(test)]
    if let Some(dir) = std::env::var_os("LUMI_BIN_DIR") {
        return PathBuf::from(dir);
    }
    dirs::data_local_dir().unwrap_or_else(std::env::temp_dir).join("Lumi").join("bin")
}

/// `%LOCALAPPDATA%\Lumi\tmp` (per-job scratch dirs live below it). Tests redirect it with `LUMI_TMP_DIR`.
pub fn tmp_dir() -> PathBuf {
    #[cfg(test)]
    if let Some(dir) = std::env::var_os("LUMI_TMP_DIR") {
        return PathBuf::from(dir);
    }
    dirs::data_local_dir().unwrap_or_else(std::env::temp_dir).join("Lumi").join("tmp")
}

/// Deletes everything under `tmp_dir()`: job dirs and decrypted cookie files a crash left behind.
/// Call once at startup, before any job can run.
pub fn sweep_tmp() {
    let Ok(entries) = std::fs::read_dir(tmp_dir()) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let _ = if entry.file_type().is_ok_and(|t| t.is_dir()) {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
    }
}

/// `%LOCALAPPDATA%\Lumi\accounts` (DPAPI-encrypted cookie jars). Tests redirect it with `LUMI_ACCOUNTS_DIR`;
/// without it they get a dir that doesn't exist, so a test never decrypts the real user's cookies.
pub fn accounts_dir() -> PathBuf {
    #[cfg(test)]
    return std::env::var_os("LUMI_ACCOUNTS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("lumi-test-no-accounts"));
    #[cfg(not(test))]
    dirs::data_local_dir().unwrap_or_else(std::env::temp_dir).join("Lumi").join("accounts")
}

pub fn exe_name(t: Tool) -> &'static str {
    match t {
        Tool::YtDlp => "yt-dlp.exe",
        Tool::GalleryDl => "gallery-dl.exe",
        Tool::Ffmpeg => "ffmpeg.exe",
        Tool::Deno => "deno.exe",
    }
}

pub fn tool_path(t: Tool) -> PathBuf {
    let exe = bin_dir().join(exe_name(t));
    #[cfg(test)]
    if !exe.exists() {
        let cmd = exe.with_extension("cmd");
        if cmd.exists() {
            return cmd;
        }
    }
    exe
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_override_and_names() {
        let _g = TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("LUMI_BIN_DIR", dir.path()) };
        assert_eq!(bin_dir(), dir.path());
        assert_eq!(tool_path(Tool::Deno), dir.path().join("deno.exe"));
        std::fs::write(dir.path().join("yt-dlp.cmd"), "").unwrap();
        assert_eq!(tool_path(Tool::YtDlp), dir.path().join("yt-dlp.cmd"));
        unsafe { std::env::remove_var("LUMI_BIN_DIR") };
    }

    #[test]
    fn sweep_tmp_empties_tmp_dir() {
        let _g = TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let job = dir.path().join("job-1");
        std::fs::create_dir_all(job.join("nested")).unwrap();
        std::fs::write(job.join("nested").join("part.mp4"), "x").unwrap();
        std::fs::write(dir.path().join("cookies-abc.txt"), "secret").unwrap();
        unsafe { std::env::set_var("LUMI_TMP_DIR", dir.path()) };
        sweep_tmp();
        unsafe { std::env::remove_var("LUMI_TMP_DIR") };
        assert!(dir.path().exists(), "tmp dir itself is kept");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn sweep_tmp_without_tmp_dir_is_noop() {
        let _g = TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("LUMI_TMP_DIR", dir.path().join("missing")) };
        sweep_tmp();
        unsafe { std::env::remove_var("LUMI_TMP_DIR") };
    }

    #[test]
    fn accounts_dir_override_and_test_default() {
        let _g = TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        assert!(!accounts_dir().exists(), "tests must never see the real accounts dir");
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("LUMI_ACCOUNTS_DIR", dir.path()) };
        assert_eq!(accounts_dir(), dir.path());
        unsafe { std::env::remove_var("LUMI_ACCOUNTS_DIR") };
    }
}
