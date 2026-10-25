#![allow(dead_code)]

pub mod install;
pub mod paths;
pub mod runner;

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

async fn self_update(t: Tool, flag: &str) -> Result<(), LumiError> {
    let out = run_tool(t, &[flag.into()]).await?;
    if out.status != 0 {
        return Err(LumiError::new(classify(Site::Other, &out.stderr), out.stderr));
    }
    Ok(())
}

#[tauri::command]
pub async fn update_tools(
    app: AppHandle,
    state: State<'_, ToolsState>,
) -> Result<Vec<ToolStatus>, LumiError> {
    for t in Tool::ALL {
        if !tool_path(t).exists() {
            continue;
        }
        let _guard = state.lock_for(t).lock().await;
        match t {
            Tool::YtDlp => self_update(t, "-U").await?,
            Tool::GalleryDl => self_update(t, "--update").await?,
            Tool::Ffmpeg | Tool::Deno => install_emitting(&app, t).await?,
        }
    }
    Ok(statuses().await)
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

    #[test]
    fn progress_serializes_camel_case() {
        let p = ToolProgress { tool: Tool::YtDlp, downloaded: 5, total: None };
        assert_eq!(
            serde_json::to_value(p).unwrap(),
            serde_json::json!({"tool": "yt_dlp", "downloaded": 5, "total": null})
        );
    }
}
