#![allow(dead_code)]

pub mod install;
pub mod paths;
pub mod runner;

use std::ffi::OsString;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::errors::{classify, LumiError};
use crate::model::Site;

use paths::tool_path;
use runner::run_tool;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Tool { YtDlp, GalleryDl, Ffmpeg, Deno }

impl Tool {
    pub const ALL: [Tool; 4] = [Tool::YtDlp, Tool::GalleryDl, Tool::Ffmpeg, Tool::Deno];
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    pub tool: Tool,
    pub installed: bool,
    pub version: Option<String>,
}

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolProgress {
    pub tool: Tool,
    pub downloaded: u64,
    pub total: Option<u64>,
}

/// One lock per tool so the same tool is never installed twice concurrently.
#[derive(Default)]
pub struct ToolsState {
    yt_dlp: tokio::sync::Mutex<()>,
    gallery_dl: tokio::sync::Mutex<()>,
    ffmpeg: tokio::sync::Mutex<()>,
    deno: tokio::sync::Mutex<()>,
}

impl ToolsState {
    fn lock_for(&self, t: Tool) -> &tokio::sync::Mutex<()> {
        match t {
            Tool::YtDlp => &self.yt_dlp,
            Tool::GalleryDl => &self.gallery_dl,
            Tool::Ffmpeg => &self.ffmpeg,
            Tool::Deno => &self.deno,
        }
    }
}

fn parse_version(t: Tool, stdout: &str) -> Option<String> {
    let first = stdout.lines().next()?.trim();
    let v = match t {
        Tool::Ffmpeg => first.split_whitespace().nth(2)?,
        _ => first,
    };
    (!v.is_empty()).then(|| v.to_string())
}

async fn tool_version(t: Tool) -> Option<String> {
    let arg = if t == Tool::Ffmpeg { "-version" } else { "--version" };
    let out = run_tool(t, &[arg.into()]).await.ok()?;
    if out.status != 0 {
        return None;
    }
    parse_version(t, &out.stdout)
}

async fn status_of(t: Tool) -> ToolStatus {
    let installed = tool_path(t).exists();
    let version = if installed { tool_version(t).await } else { None };
    ToolStatus { tool: t, installed, version }
}

pub async fn statuses() -> Vec<ToolStatus> {
    futures_util::future::join_all(Tool::ALL.map(status_of)).await
}

#[tauri::command]
pub async fn tools_status() -> Vec<ToolStatus> {
    statuses().await
}

#[tauri::command]
pub async fn tool_download_size(tool: Tool) -> Result<Option<u64>, LumiError> {
    install::download_size(tool).await
}

#[tauri::command]
pub async fn install_tool(
    app: AppHandle,
    state: State<'_, ToolsState>,
    tool: Tool,
) -> Result<ToolStatus, LumiError> {
    let _guard = state.lock_for(tool).lock().await;
    install_emitting(&app, tool).await?;
    Ok(status_of(tool).await)
}

async fn install_emitting(app: &AppHandle, tool: Tool) -> Result<(), LumiError> {
    let mut emit = |downloaded: u64, total: Option<u64>| {
        let _ = app.emit("lumi://tool-progress", ToolProgress { tool, downloaded, total });
    };
    install::install_with(tool, &mut emit).await
}

/// One tool whose update failed.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolUpdateError {
    pub tool: Tool,
    pub error: LumiError,
}

/// What "Update" did: every tool's status afterwards, plus the tools that could not be updated.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReport {
    pub statuses: Vec<ToolStatus>,
    pub errors: Vec<ToolUpdateError>,
}

/// The self-update flags; ffmpeg and deno have none (they are fetched on demand, with a size prompt).
fn self_update_args(t: Tool) -> Option<[&'static str; 2]> {
    match t {
        Tool::YtDlp => Some(["--ignore-config", "-U"]),
        Tool::GalleryDl => Some(["--config-ignore", "--update"]),
        Tool::Ffmpeg | Tool::Deno => None,
    }
}

async fn self_update(t: Tool, args: [&str; 2]) -> Result<(), LumiError> {
    let out = run_tool(t, &args.map(OsString::from)).await?;
    if out.status != 0 {
        let detail = if out.stderr.trim().is_empty() { out.stdout } else { out.stderr };
        return Err(LumiError::new(classify(Site::Other, &detail), detail));
    }
    Ok(())
}

/// Self-updates the installed yt-dlp and gallery-dl, carrying on past a failure.
async fn update_all(state: &ToolsState) -> UpdateReport {
    let mut errors = Vec::new();
    for t in Tool::ALL {
        let Some(args) = self_update_args(t) else { continue };
        if !tool_path(t).exists() {
            continue;
        }
        let _guard = state.lock_for(t).lock().await;
        if let Err(error) = self_update(t, args).await {
            errors.push(ToolUpdateError { tool: t, error });
        }
    }
    UpdateReport { statuses: statuses().await, errors }
}

#[tauri::command]
pub async fn update_tools(state: State<'_, ToolsState>) -> Result<UpdateReport, LumiError> {
    Ok(update_all(&state).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_parsing() {
        assert_eq!(parse_version(Tool::YtDlp, "2025.01.01\n"), Some("2025.01.01".into()));
        assert_eq!(
            parse_version(Tool::Ffmpeg, "ffmpeg version N-1234-gabc Copyright (c) 2000\nbuilt"),
            Some("N-1234-gabc".into())
        );
        assert_eq!(
            parse_version(Tool::Deno, "deno 2.1.4 (stable)\nv8 13"),
            Some("deno 2.1.4 (stable)".into())
        );
        assert_eq!(parse_version(Tool::YtDlp, ""), None);
    }

    /// Update touches only yt-dlp and gallery-dl, keeps going after one fails, and reports it.
    #[tokio::test]
    async fn update_self_updates_and_reports_per_tool() {
        let _g = paths::TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("LUMI_BIN_DIR", dir.path()) };
        let log = |n: &str| dir.path().join(format!("{n}.log"));
        let fake = |n: &str, body: &str| {
            let script = format!("@echo off\r\necho %*>> \"{}\"\r\n{body}\r\n", log(n).display());
            std::fs::write(dir.path().join(format!("{n}.cmd")), script).unwrap();
        };
        // spawn_tool may prepend --ffmpeg-location, so look for -U anywhere in the arguments.
        fake("yt-dlp", "echo %* | findstr /C:\" -U\" >nul && (echo ERROR: Unable to update 1>&2 & exit /b 1)\r\necho 2026.10.21");
        fake("gallery-dl", "echo 1.33.0");
        fake("ffmpeg", "echo ffmpeg version 7.1 x");
        let report = update_all(&ToolsState::default()).await;
        let read = |n: &str| std::fs::read_to_string(log(n)).unwrap_or_default();
        let (yt, gdl, ff) = (read("yt-dlp"), read("gallery-dl"), read("ffmpeg"));
        unsafe { std::env::remove_var("LUMI_BIN_DIR") };
        assert!(yt.contains("--ignore-config -U"), "{yt}");
        assert!(gdl.contains("--config-ignore --update"), "gallery-dl still updated after yt-dlp failed: {gdl}");
        assert!(!ff.contains("-U") && !ff.contains("update"), "ffmpeg is never re-downloaded: {ff}");
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0].tool, Tool::YtDlp);
        assert!(report.errors[0].error.detail.contains("Unable to update"));
        assert_eq!(report.statuses.len(), 4);
        assert!(!report.statuses.iter().find(|s| s.tool == Tool::Deno).unwrap().installed);
        let v = serde_json::to_value(&report).unwrap();
        assert_eq!(v["errors"][0]["tool"], "yt_dlp");
        assert_eq!(v["errors"][0]["error"]["error"]["code"], "unknown");
    }

    #[test]
    fn progress_serializes_camel_case() {
        let p = ToolProgress { tool: Tool::YtDlp, downloaded: 5, total: None };
        assert_eq!(
            serde_json::to_value(p).unwrap(),
            serde_json::json!({"tool": "yt_dlp", "downloaded": 5, "total": null})
        );
    }
}
