use std::ffi::OsString;
use std::process::Stdio;

use tokio::process::{Child, Command};

use super::paths::{bin_dir, tool_path};
use super::Tool;
use crate::errors::{ErrorCode, LumiError};

pub(crate) const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone)]
pub struct RunOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Flags every yt-dlp invocation must carry; arg builders append them. `--ignore-config`: a user's
/// yt-dlp config (e.g. a different `-o` or `--mtime`) must never change what Lumi gets.
pub fn ytdlp_common_args() -> Vec<OsString> {
    ["--ignore-config", "--encoding", "utf-8", "--no-playlist", "--no-warnings"].map(OsString::from).to_vec()
}

/// Flags every gallery-dl invocation must carry: never read a user's gallery-dl config.
pub fn gallerydl_common_args() -> Vec<OsString> {
    vec![OsString::from("--config-ignore")]
}

/// Flags spawn_tool prepends for yt-dlp, depending on which helpers are installed.
fn injected_args(tool: Tool, has_deno: bool, has_ffmpeg: bool) -> Vec<OsString> {
    let mut out = Vec::new();
    if tool != Tool::YtDlp {
        return out;
    }
    if has_deno {
        let mut v = OsString::from("deno:");
        v.push(tool_path(Tool::Deno));
        out.push("--js-runtimes".into());
        out.push(v);
    }
    if has_ffmpeg {
        out.push("--ffmpeg-location".into());
        out.push(bin_dir().into_os_string());
    }
    out
}

fn build(t: Tool, args: &[OsString]) -> Result<Command, LumiError> {
    let path = tool_path(t);
    if !path.exists() {
        return Err(LumiError::new(ErrorCode::ToolMissing { tool: t }, format!("{t:?} is not installed")));
    }
    let mut cmd = Command::new(path);
    cmd.args(injected_args(t, tool_path(Tool::Deno).exists(), tool_path(Tool::Ffmpeg).exists()))
        .args(args)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    Ok(cmd)
}

pub fn spawn_tool(t: Tool, args: &[OsString]) -> Result<Child, LumiError> {
    build(t, args)?
        .spawn()
        .map_err(|e| LumiError::new(ErrorCode::Unknown, format!("failed to start {t:?}: {e}")))
}

pub async fn run_tool(t: Tool, args: &[OsString]) -> Result<RunOutput, LumiError> {
    let child = spawn_tool(t, args)?;
    let out = child
        .wait_with_output()
        .await
        .map_err(|e| LumiError::new(ErrorCode::Unknown, format!("failed to run {t:?}: {e}")))?;
    Ok(RunOutput {
        status: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::paths::TEST_ENV_LOCK;

    #[tokio::test]
    async fn run_tool_missing_binary() {
        let _g = TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("LUMI_BIN_DIR", dir.path()) };
        let err = run_tool(Tool::YtDlp, &[]).await.unwrap_err();
        unsafe { std::env::remove_var("LUMI_BIN_DIR") };
        assert_eq!(err.error, ErrorCode::ToolMissing { tool: Tool::YtDlp });
    }

    #[test]
    fn common_args_exact() {
        let a: Vec<String> =
            ytdlp_common_args().into_iter().map(|s| s.to_string_lossy().into_owned()).collect();
        assert_eq!(a, ["--ignore-config", "--encoding", "utf-8", "--no-playlist", "--no-warnings"]);
        assert_eq!(gallerydl_common_args(), [OsString::from("--config-ignore")]);
    }

    #[test]
    fn injection_only_for_ytdlp() {
        let _g = TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("LUMI_BIN_DIR", dir.path()) };
        assert!(injected_args(Tool::YtDlp, false, false).is_empty());
        assert!(injected_args(Tool::GalleryDl, true, true).is_empty());
        let both = injected_args(Tool::YtDlp, true, true);
        let deno = format!("deno:{}", dir.path().join("deno.exe").display());
        assert_eq!(
            both,
            vec![
                OsString::from("--js-runtimes"),
                OsString::from(deno),
                OsString::from("--ffmpeg-location"),
                dir.path().as_os_str().to_os_string(),
            ]
        );
        assert_eq!(injected_args(Tool::YtDlp, false, true).len(), 2);
        unsafe { std::env::remove_var("LUMI_BIN_DIR") };
    }
}
