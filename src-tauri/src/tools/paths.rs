use std::path::PathBuf;

use super::Tool;

#[cfg(test)]
pub(crate) static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// `LUMI_BIN_DIR` override, else `%LOCALAPPDATA%\Lumi\bin`. Not created here.
pub fn bin_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("LUMI_BIN_DIR") {
        return PathBuf::from(dir);
    }
    dirs::data_local_dir().unwrap_or_else(std::env::temp_dir).join("Lumi").join("bin")
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
}
