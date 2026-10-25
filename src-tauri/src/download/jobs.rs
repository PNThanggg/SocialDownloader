//! Download jobs: owned by Rust, progress broadcast to every window as `lumi://job`.

use std::collections::HashMap;
use std::ffi::OsString;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use futures_util::StreamExt;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use once_cell::sync::Lazy;
use regex::Regex;
use tokio::sync::Notify;

use super::args::{build_plan, ffmpeg_command_args, needs_ffmpeg, FfInput, Plan, PROGRESS_TEMPLATE};
use super::progress::parse_progress;
use crate::accounts::{cookie_file_for, TempCookieFile};
use crate::errors::{classify, ErrorCode, LumiError};
use crate::model::{Engine, MediaInfo, OptionId, Site};
use crate::resolve::{resolve_url, ResolveCache};
use crate::tools::paths::{tmp_dir, tool_path};
use crate::tools::runner::{spawn_tool, ytdlp_common_args, CREATE_NO_WINDOW};
use crate::tools::Tool;

const POSTPROCESS_TAGS: [&str; 5] = ["[ExtractAudio]", "[Merger]", "[Metadata]", "[EmbedThumbnail]", "[FixupM3u8]"];

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum JobState {
    Started,
    Progress { fraction: Option<f32> },
    Postprocessing,
    Done { path: String },
    Failed { error: LumiError },
    Cancelled,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JobEvent {
    pub job_id: String,
    pub url: String,
    pub option_id: String,
    #[serde(flatten)]
    pub state: JobState,
}

pub trait JobSink: Send + Sync + 'static {
    fn emit(&self, e: JobEvent);
}

/// Broadcasts to every window.
pub struct TauriSink(pub AppHandle);

impl JobSink for TauriSink {
    fn emit(&self, e: JobEvent) {
        let _ = self.0.emit("lumi://job", e);
    }
}

/// Cancellation request shared between `cancel_job` and the job task.
struct Cancel {
    flag: AtomicBool,
    notify: Notify,
}

impl Cancel {
    fn new() -> Self {
        Self { flag: AtomicBool::new(false), notify: Notify::new() }
    }

    fn request(&self) {
        self.flag.store(true, Ordering::SeqCst);
        self.notify.notify_one();
    }

    fn is_set(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    async fn wait(&self) {
        if !self.is_set() {
            self.notify.notified().await;
        }
    }
}

struct JobHandle {
    key: (String, String),
    cancel: Arc<Cancel>,
}

/// Running jobs; a second job for the same `(url, option_id)` is rejected.
#[derive(Clone, Default)]
pub struct JobManager {
    jobs: Arc<Mutex<HashMap<String, JobHandle>>>,
}

enum Output {
    File(PathBuf),
    Dir(PathBuf),
}

struct JobSpec {
    job_id: String,
    url: String,
    option_id: String,
    site: Site,
    output: Output,
    plan: Vec<Plan>,
    tmp: PathBuf,
    cookies: Option<TempCookieFile>,
}

enum StepErr {
    Cancelled,
    Failed(LumiError),
}

impl From<LumiError> for StepErr {
    fn from(e: LumiError) -> Self {
        StepErr::Failed(e)
    }
}

fn unknown(detail: impl Into<String>) -> LumiError {
    LumiError::new(ErrorCode::Unknown, detail)
}

async fn or_cancel<T>(cancel: &Cancel, fut: impl Future<Output = T>) -> Result<T, StepErr> {
    tokio::select! {
        v = fut => Ok(v),
        _ = cancel.wait() => Err(StepErr::Cancelled),
    }
}

/// Turns tool output into events: clamped non-decreasing fractions, one `Postprocessing`.
struct Reporter {
    sink: Arc<dyn JobSink>,
    job_id: String,
    url: String,
    option_id: String,
    last: Option<f32>,
    sent_unknown: bool,
    post_sent: bool,
}

impl Reporter {
    fn emit(&self, state: JobState) {
        self.sink.emit(JobEvent {
            job_id: self.job_id.clone(),
            url: self.url.clone(),
            option_id: self.option_id.clone(),
            state,
        });
    }

    fn progress(&mut self, downloaded: u64, total: Option<u64>) {
        match total {
            Some(t) if t > 0 => {
                let f = (downloaded as f64 / t as f64).min(1.0) as f32;
                if self.last.is_none_or(|l| f > l) {
                    self.last = Some(f);
                    self.emit(JobState::Progress { fraction: Some(f) });
                }
            }
            _ => self.indeterminate(),
        }
    }

    fn indeterminate(&mut self) {
        if self.last.is_none() && !self.sent_unknown {
            self.sent_unknown = true;
            self.emit(JobState::Progress { fraction: None });
        }
    }

    fn postprocessing(&mut self) {
        if !self.post_sent {
            self.post_sent = true;
            self.emit(JobState::Postprocessing);
        }
    }
}

/// `taskkill /T /F`: yt-dlp.exe is a PyInstaller onefile whose child survives a plain kill.
async fn kill_tree(pid: u32) {
    let mut cmd = tokio::process::Command::new("taskkill");
    cmd.args(["/T", "/F", "/PID", &pid.to_string()])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    if let Ok(mut c) = cmd.spawn() {
        let _ = c.wait().await;
    }
}

async fn run_child(
    tool: Tool, args: &[OsString], site: Site, rep: &mut Reporter, cancel: &Cancel,
) -> Result<(), StepErr> {
    let mut child = spawn_tool(tool, args)?;
    let pid = child.id().unwrap_or(0);
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let err_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        if let Some(mut e) = stderr {
            let _ = e.read_to_end(&mut buf).await;
        }
        String::from_utf8_lossy(&buf).into_owned()
    });
    let res = or_cancel(cancel, async {
        if let Some(out) = stdout {
            let mut reader = BufReader::new(out);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                match reader.read_until(b'\n', &mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                let line = String::from_utf8_lossy(&buf);
                let line = line.trim();
                if let Some(p) = parse_progress(line) {
                    rep.progress(p.downloaded, p.total);
                } else if POSTPROCESS_TAGS.iter().any(|t| line.starts_with(t)) {
                    rep.postprocessing();
                }
            }
        }
        child.wait().await
    })
    .await;
    let status = match res {
        Ok(r) => r.map_err(|e| unknown(format!("failed to wait for {tool:?}: {e}")))?,
        Err(c) => {
            kill_tree(pid).await;
            let _ = child.kill().await;
            err_task.abort();
            return Err(c);
        }
    };
    let stderr = err_task.await.unwrap_or_default();
    if status.success() {
        return Ok(());
    }
    let stderr = stderr.trim().to_owned();
    let detail = if stderr.is_empty() { format!("{tool:?} exited with {status}") } else { stderr };
    Err(StepErr::Failed(LumiError::new(classify(site, &detail), detail)))
}

async fn http_step(
    url: &str, referer: &str, to: &Path, rep: &mut Reporter, cancel: &Cancel,
) -> Result<(), StepErr> {
    let net = |e: reqwest::Error| LumiError::new(ErrorCode::Network, e.to_string());
    let io = |e: std::io::Error| unknown(format!("{}: {e}", to.display()));
    or_cancel(cancel, async {
        let client = crate::thumb::browser_client().map_err(net)?;
        let resp = client
            .get(url)
            .header(reqwest::header::REFERER, referer)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(net)?;
        let total = resp.content_length();
        if let Some(dir) = to.parent() {
            tokio::fs::create_dir_all(dir).await.map_err(io)?;
        }
        let mut file = tokio::fs::File::create(to).await.map_err(io)?;
        let mut stream = resp.bytes_stream();
        let mut done = 0u64;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(net)?;
            file.write_all(&chunk).await.map_err(io)?;
            done += chunk.len() as u64;
            rep.progress(done, total);
        }
        file.flush().await.map_err(io)?;
        Ok::<(), LumiError>(())
    })
    .await?
    .map_err(StepErr::Failed)
}

/// The single regular file inside `dir`.
fn only_file(dir: &Path) -> Result<PathBuf, LumiError> {
    let mut files = std::fs::read_dir(dir)
        .map_err(|e| unknown(format!("{}: {e}", dir.display())))?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.path());
    match (files.next(), files.next()) {
        (Some(f), None) => Ok(f),
        (None, _) => Err(unknown(format!("no file produced in {}", dir.display()))),
        _ => Err(unknown(format!("more than one file produced in {}", dir.display()))),
    }
}

/// Rename; tmp is on the system drive and `to` may be on another volume, so fall back to copy + delete.
fn move_file(from: &Path, to: &Path) -> Result<(), LumiError> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    std::fs::copy(from, to).map_err(|e| unknown(format!("copy to {}: {e}", to.display())))?;
    let _ = std::fs::remove_file(from);
    Ok(())
}

static F_FORMAT: Lazy<Regex> = Lazy::new(|| Regex::new(r"^f[\w-]+\.\w+").expect("valid regex"));

/// Whether `name` (lowercase) is an unfinished yt-dlp file for `stem`: `.part`, `.part-FragN`, `.ytdl`,
/// `.temp.<ext>` or a per-format `.f<id>.<ext>` piece. A complete media file never is.
fn is_partial(stem: &str, name: &str) -> bool {
    let Some(rest) = name.strip_prefix(stem).and_then(|r| r.strip_prefix('.')) else { return false };
    rest.ends_with(".part")
        || rest.contains(".part-frag")
        || rest.ends_with(".ytdl")
        || rest.starts_with("temp.")
        || rest.ends_with(".temp")
        || F_FORMAT.is_match(rest)
}

/// `<stem>.*` files next to `dest` with their mtime (lowercase stem match).
fn stem_files(dest: &Path) -> Vec<(String, std::fs::Metadata, PathBuf)> {
    let Some(stem) = dest.file_stem().map(|s| s.to_string_lossy().to_lowercase()) else { return vec![] };
    let dir = dest.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    rd.filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            let meta = e.metadata().ok().filter(|m| m.is_file())?;
            name.starts_with(&format!("{stem}.")).then(|| (name, meta, e.path()))
        })
        .collect()
}

fn written_since(meta: &std::fs::Metadata, started: SystemTime) -> bool {
    meta.modified().is_ok_and(|m| m >= started)
}

/// The finished `<stem>.*` file this job wrote next to `dest` (yt-dlp picks the extension itself):
/// written since `started`, never a partial, `dest`'s own extension first, else the newest.
fn newest_with_stem(dest: &Path, started: SystemTime) -> Option<PathBuf> {
    let stem = dest.file_stem()?.to_string_lossy().to_lowercase();
    let want = dest.extension().map(|e| e.to_string_lossy().to_lowercase());
    stem_files(dest)
        .into_iter()
        .filter(|(name, meta, _)| !is_partial(&stem, name) && written_since(meta, started))
        .max_by_key(|(_, meta, p)| {
            let same_ext = want.is_some() && p.extension().map(|e| e.to_string_lossy().to_lowercase()) == want;
            (same_ext, meta.modified().ok())
        })
        .map(|(_, _, p)| p)
}

async fn run_plan(
    plan: &[Plan], site: Site, output: &Output, started: SystemTime, rep: &mut Reporter, cancel: &Cancel,
) -> Result<String, StepErr> {
    for step in plan {
        if cancel.is_set() {
            return Err(StepErr::Cancelled);
        }
        match step {
            Plan::YtDlp(args) => run_child(Tool::YtDlp, args, site, rep, cancel).await?,
            Plan::GalleryDl(args) => {
                rep.indeterminate();
                run_child(Tool::GalleryDl, args, site, rep, cancel).await?
            }
            Plan::Http { url, referer, to, optional } => {
                match http_step(url, referer, to, rep, cancel).await {
                    Err(StepErr::Failed(_)) if *optional => {
                        let _ = std::fs::remove_file(to);
                    }
                    other => other?,
                }
            }
            Plan::Ffmpeg { input, cover, args } => {
                let input = match input {
                    FfInput::Path(p) => p.clone(),
                    FfInput::SingleFileIn(dir) => only_file(dir)?,
                };
                let cover = cover.as_deref().filter(|c| c.exists());
                let args = ffmpeg_command_args(&input, cover, args);
                rep.postprocessing();
                run_child(Tool::Ffmpeg, &args, site, rep, cancel).await?
            }
            Plan::MoveSingle { from_dir, to } => move_file(&only_file(from_dir)?, to)?,
        }
    }
    match output {
        Output::Dir(d) => Ok(d.to_string_lossy().into_owned()),
        Output::File(dest) => {
            // gallery-dl may stamp a file with the post's date, so an existing dest is the last resort.
            let found = newest_with_stem(dest, started).or_else(|| dest.exists().then(|| dest.clone()));
            found
                .map(|p| p.to_string_lossy().into_owned())
                .ok_or_else(|| StepErr::Failed(unknown(format!("output file missing: {}", dest.display()))))
        }
    }
}

/// Deletes the partial `<stem>.*` files (see `is_partial`) written next to `dest` since `started`.
/// Complete files are left alone: the MP4 and MP3 rows share a stem, so one may be a sibling job's
/// finished output. (With `-P temp:` yt-dlp keeps its partials in the job dir anyway.)
fn remove_stem_leftovers(dest: &Path, started: SystemTime) {
    let Some(stem) = dest.file_stem().map(|s| s.to_string_lossy().to_lowercase()) else { return };
    for (name, meta, path) in stem_files(dest) {
        if is_partial(&stem, &name) && written_since(&meta, started) {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Removes the job tmp dir; the dying child may still hold files for a moment.
async fn remove_tmp(dir: &Path) {
    for _ in 0..20 {
        if !dir.exists() || std::fs::remove_dir_all(dir).is_ok() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

impl JobManager {
    fn ensure_not_running(&self, key: &(String, String)) -> Result<(), LumiError> {
        let jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        match jobs.iter().find(|(_, h)| &h.key == key) {
            Some((existing, _)) => Err(unknown(existing.clone())),
            None => Ok(()),
        }
    }

    fn register(&self, job_id: &str, key: (String, String), cancel: Arc<Cancel>) -> Result<(), LumiError> {
        let mut jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((existing, _)) = jobs.iter().find(|(_, h)| h.key == key) {
            return Err(unknown(existing.clone()));
        }
        jobs.insert(job_id.to_owned(), JobHandle { key, cancel });
        Ok(())
    }

    fn unregister(&self, job_id: &str) {
        self.jobs.lock().unwrap_or_else(|e| e.into_inner()).remove(job_id);
    }

    #[cfg(test)]
    fn active(&self) -> usize {
        self.jobs.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    pub fn cancel(&self, job_id: &str) -> Result<(), LumiError> {
        let jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        let h = jobs.get(job_id).ok_or_else(|| unknown(format!("no running job {job_id}")))?;
        h.cancel.request();
        Ok(())
    }

    async fn media_info(url: &str, cache: &ResolveCache) -> Result<MediaInfo, LumiError> {
        match cache.get(url) {
            Some(i) => Ok(i),
            None => resolve_url(url, cache).await,
        }
    }

    /// Starts one download; returns the job id. `dest` is the final file path.
    pub async fn start(
        &self, sink: Arc<dyn JobSink>, cache: &ResolveCache, url: String, option_id: String, dest: PathBuf,
    ) -> Result<String, LumiError> {
        let info = Self::media_info(&url, cache).await?;
        let opt = OptionId::parse(&option_id).ok_or_else(|| unknown(format!("bad option id: {option_id}")))?;
        if needs_ffmpeg(&info, &opt) && !tool_path(Tool::Ffmpeg).exists() {
            return Err(LumiError::new(ErrorCode::ToolMissing { tool: Tool::Ffmpeg }, "ffmpeg is not installed"));
        }
        // Reject duplicates before any cookie file is decrypted to disk.
        self.ensure_not_running(&(url.clone(), option_id.clone()))?;
        let job_id = uuid::Uuid::new_v4().to_string();
        let tmp = tmp_dir().join(&job_id);
        let cookies = cookie_file_for(info.site)?;
        let plan = build_plan(&info, &opt, &dest, &tmp, cookies.as_ref().map(|c| c.0.as_path()))
            .map_err(|code| LumiError::new(code, "cannot build a download plan for this option"))?;
        let spec = JobSpec { job_id, url, option_id, site: info.site, output: Output::File(dest), plan, tmp, cookies };
        self.launch(sink, spec)
    }

    /// Downloads every item of `url` into `dest_dir`.
    pub async fn start_all(
        &self, sink: Arc<dyn JobSink>, cache: &ResolveCache, url: String, dest_dir: PathBuf,
    ) -> Result<String, LumiError> {
        let info = Self::media_info(&url, cache).await?;
        if info.engine == Engine::YtDlp && !tool_path(Tool::Ffmpeg).exists() {
            // `bv*+ba` merges, which needs ffmpeg.
            return Err(LumiError::new(ErrorCode::ToolMissing { tool: Tool::Ffmpeg }, "ffmpeg is not installed"));
        }
        self.ensure_not_running(&(url.clone(), "all".to_owned()))?;
        let job_id = uuid::Uuid::new_v4().to_string();
        let tmp = tmp_dir().join(&job_id);
        let cookies = cookie_file_for(info.site)?;
        let cookie_path = cookies.as_ref().map(|c| c.0.as_path());
        let plan = match info.engine {
            Engine::GalleryDl => {
                let mut a: Vec<OsString> = vec!["-D".into(), dest_dir.as_os_str().into()];
                if let Some(c) = cookie_path {
                    a.extend(["--cookies".into(), c.into()]);
                }
                a.push((&info.source_url).into());
                Plan::GalleryDl(a)
            }
            Engine::YtDlp => {
                let dir = dest_dir.to_string_lossy().replace('%', "%%");
                let dir = dir.trim_end_matches(['\\', '/']);
                let mut a = ytdlp_common_args();
                a.extend([
                    "-o".into(),
                    format!("{dir}\\%(title).80B [%(id)s].%(ext)s").into(),
                    "-f".into(),
                    "bv*+ba[format_id!=audio]/b".into(),
                    "--force-overwrites".into(),
                    "--newline".into(),
                    "--progress-template".into(),
                    PROGRESS_TEMPLATE.into(),
                ]);
                if let Some(c) = cookie_path {
                    a.extend(["--cookies".into(), c.into()]);
                }
                a.push((&info.source_url).into());
                Plan::YtDlp(a)
            }
        };
        let spec = JobSpec {
            job_id,
            url,
            option_id: "all".into(),
            site: info.site,
            output: Output::Dir(dest_dir),
            plan: vec![plan],
            tmp,
            cookies,
        };
        self.launch(sink, spec)
    }

    fn launch(&self, sink: Arc<dyn JobSink>, spec: JobSpec) -> Result<String, LumiError> {
        let cancel = Arc::new(Cancel::new());
        self.register(&spec.job_id, (spec.url.clone(), spec.option_id.clone()), cancel.clone())?;
        if let Err(e) = std::fs::create_dir_all(&spec.tmp) {
            self.unregister(&spec.job_id);
            return Err(unknown(format!("{}: {e}", spec.tmp.display())));
        }
        let id = spec.job_id.clone();
        let mgr = self.clone();
        tokio::spawn(async move { mgr.run_job(sink, spec, cancel).await });
        Ok(id)
    }

    async fn run_job(&self, sink: Arc<dyn JobSink>, spec: JobSpec, cancel: Arc<Cancel>) {
        let JobSpec { job_id, url, option_id, site, output, plan, tmp, cookies } = spec;
        let mut rep = Reporter {
            sink,
            job_id: job_id.clone(),
            url,
            option_id,
            last: None,
            sent_unknown: false,
            post_sent: false,
        };
        let started = SystemTime::now();
        let dest_existed = matches!(&output, Output::File(d) if d.exists());
        rep.emit(JobState::Started);

        let result = run_plan(&plan, site, &output, started, &mut rep, &cancel).await;
        let final_state = match result {
            Ok(path) => JobState::Done { path },
            Err(StepErr::Cancelled) => JobState::Cancelled,
            // A cancel kills the tool, which then fails: that is still a cancel.
            Err(StepErr::Failed(_)) if cancel.is_set() => JobState::Cancelled,
            Err(StepErr::Failed(error)) => JobState::Failed { error },
        };

        if !matches!(final_state, JobState::Done { .. }) {
            if let Output::File(dest) = &output {
                let modified_after_start = std::fs::metadata(dest)
                    .and_then(|m| m.modified())
                    .is_ok_and(|m| m > started);
                if !dest_existed || modified_after_start {
                    let _ = std::fs::remove_file(dest);
                }
                remove_stem_leftovers(dest, started);
            }
        }
        remove_tmp(&tmp).await;
        drop(cookies);
        self.unregister(&job_id);
        rep.emit(final_state);
    }
}

#[tauri::command]
pub async fn download(
    app: AppHandle, jobs: State<'_, JobManager>, cache: State<'_, ResolveCache>, url: String, option_id: String,
    dest: String,
) -> Result<String, LumiError> {
    jobs.start(Arc::new(TauriSink(app)), &cache, url, option_id, PathBuf::from(dest)).await
}

#[tauri::command]
pub async fn download_all(
    app: AppHandle, jobs: State<'_, JobManager>, cache: State<'_, ResolveCache>, url: String, dest_dir: String,
) -> Result<String, LumiError> {
    jobs.start_all(Arc::new(TauriSink(app)), &cache, url, PathBuf::from(dest_dir)).await
}

#[tauri::command]
pub async fn cancel_job(jobs: State<'_, JobManager>, job_id: String) -> Result<(), LumiError> {
    jobs.cancel(&job_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{MediaItem, SoundMeta};
    use crate::tools::paths::TEST_ENV_LOCK;
    use std::sync::MutexGuard;
    use std::time::{Duration, Instant};

    #[derive(Default)]
    struct VecSink(Mutex<Vec<JobEvent>>);

    impl JobSink for VecSink {
        fn emit(&self, e: JobEvent) {
            self.0.lock().unwrap().push(e);
        }
    }

    impl VecSink {
        fn events(&self) -> Vec<JobEvent> {
            self.0.lock().unwrap().clone()
        }

        fn is_final(&self) -> bool {
            self.events().last().is_some_and(|e| {
                matches!(e.state, JobState::Done { .. } | JobState::Failed { .. } | JobState::Cancelled)
            })
        }
    }

    async fn wait_until(what: &str, mut cond: impl FnMut() -> bool) {
        let t = Instant::now();
        while !cond() {
            assert!(t.elapsed() < Duration::from_secs(30), "timed out waiting for {what}");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    struct Env {
        _g: MutexGuard<'static, ()>,
        bin: tempfile::TempDir,
        tmp: tempfile::TempDir,
        out: tempfile::TempDir,
    }

    impl Env {
        fn new() -> Env {
            let g = TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let env = Env {
                _g: g,
                bin: tempfile::tempdir().unwrap(),
                tmp: tempfile::tempdir().unwrap(),
                out: tempfile::tempdir().unwrap(),
            };
            unsafe {
                std::env::set_var("LUMI_BIN_DIR", env.bin.path());
                std::env::set_var("LUMI_TMP_DIR", env.tmp.path());
            }
            env
        }

        fn tool(&self, name: &str, script: &str) {
            std::fs::write(self.bin.path().join(format!("{name}.cmd")), format!("@echo off\r\n{script}\r\n")).unwrap();
        }
    }

    impl Drop for Env {
        fn drop(&mut self) {
            unsafe {
                std::env::remove_var("LUMI_BIN_DIR");
                std::env::remove_var("LUMI_TMP_DIR");
            }
        }
    }

    const URL: &str = "https://www.tiktok.com/@a/video/1";

    fn seeded_cache() -> ResolveCache {
        let info = MediaInfo {
            source_url: URL.into(), site: Site::Tiktok, engine: Engine::YtDlp, title: "T".into(),
            author: None, duration_secs: None, thumbnail: None,
            items: vec![MediaItem { index: 0, thumbnail: None, is_video: true, options: vec![] }],
            limited_formats: false,
            sound: Some(SoundMeta { title: "S".into(), artist: "A".into(), cover: None, play_url: "http://127.0.0.1:1/x".into() }),
        };
        let c = ResolveCache::default();
        c.insert(URL, info);
        c
    }

    fn fractions(events: &[JobEvent]) -> Vec<Option<f32>> {
        events
            .iter()
            .filter_map(|e| match e.state {
                JobState::Progress { fraction } => Some(fraction),
                _ => None,
            })
            .collect()
    }

    #[tokio::test]
    async fn missing_ffmpeg_fails_fast() {
        let env = Env::new();
        let m = JobManager::default();
        let sink = Arc::new(VecSink::default());
        let err = m
            .start(sink.clone(), &seeded_cache(), URL.into(), "0/audio".into(), env.out.path().join("a.mp3"))
            .await
            .unwrap_err();
        assert_eq!(err.error, ErrorCode::ToolMissing { tool: Tool::Ffmpeg });
        assert_eq!(m.active(), 0);
        assert!(sink.events().is_empty());
        assert_eq!(std::fs::read_dir(env.tmp.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn progress_events_monotonic() {
        let env = Env::new();
        let dest = env.out.path().join("clip.mp3");
        env.tool("ffmpeg", "exit /b 0");
        // video pass then audio pass, each 0 -> 1, plus an unknown-total line and a postprocess line
        env.tool(
            "yt-dlp",
            &format!(
                "echo LUMI^|10^|NA^|NA\r\necho LUMI^|50^|100^|NA\r\necho LUMI^|100^|100^|NA\r\necho LUMI^|0^|0^|NA\r\n\
                 echo LUMI^|20^|100^|NA\r\necho [ExtractAudio] Destination\r\necho [Metadata] Adding\r\necho x> \"{}\"",
                dest.display()
            ),
        );
        let m = JobManager::default();
        let sink = Arc::new(VecSink::default());
        let id = m.start(sink.clone(), &seeded_cache(), URL.into(), "0/audio".into(), dest.clone()).await.unwrap();
        wait_until("final event", || sink.is_final()).await;
        let ev = sink.events();
        assert!(ev.iter().all(|e| e.job_id == id && e.url == URL && e.option_id == "0/audio"));
        assert_eq!(ev.first().unwrap().state, JobState::Started);
        assert_eq!(ev.last().unwrap().state, JobState::Done { path: dest.to_string_lossy().into_owned() });
        let fr = fractions(&ev);
        let known: Vec<f32> = fr.iter().flatten().copied().collect();
        assert_eq!(known, vec![0.5, 1.0]);
        assert_eq!(fr.first(), Some(&None));
        assert!(known.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(ev.iter().filter(|e| e.state == JobState::Postprocessing).count(), 1);
        assert_eq!(m.active(), 0);
        assert!(!env.tmp.path().join(&id).exists());
        assert!(dest.exists());
    }

    #[tokio::test]
    async fn failed_job_classifies_stderr_and_cleans_up() {
        let env = Env::new();
        let dest = env.out.path().join("clip.mp3");
        env.tool("ffmpeg", "exit /b 0");
        env.tool("yt-dlp", &format!("echo x> \"{}\"\r\necho ERROR: Private video 1>&2\r\nexit /b 1", dest.display()));
        let m = JobManager::default();
        let sink = Arc::new(VecSink::default());
        m.start(sink.clone(), &seeded_cache(), URL.into(), "0/audio".into(), dest.clone()).await.unwrap();
        wait_until("final event", || sink.is_final()).await;
        match sink.events().last().unwrap().state.clone() {
            JobState::Failed { error } => {
                assert_eq!(error.error, ErrorCode::PrivateOrRemoved);
                assert!(error.detail.contains("Private video"));
            }
            other => panic!("{other:?}"),
        }
        assert!(!dest.exists());
        assert_eq!(std::fs::read_dir(env.tmp.path()).unwrap().count(), 0);
    }

    /// yt-dlp chose its own extension: Done reports the file it actually wrote.
    #[tokio::test]
    async fn done_path_falls_back_to_stem_lookup() {
        let env = Env::new();
        env.tool("ffmpeg", "exit /b 0");
        let actual = env.out.path().join("clip.m4a");
        env.tool("yt-dlp", &format!("echo x> \"{}\"", actual.display()));
        let m = JobManager::default();
        let sink = Arc::new(VecSink::default());
        m.start(sink.clone(), &seeded_cache(), URL.into(), "0/audio".into(), env.out.path().join("clip.mp3"))
            .await
            .unwrap();
        wait_until("final event", || sink.is_final()).await;
        assert_eq!(sink.events().last().unwrap().state, JobState::Done { path: actual.to_string_lossy().into_owned() });
    }

    /// A file left from before this job is never reported as its output.
    #[tokio::test]
    async fn done_path_ignores_files_older_than_the_job() {
        let env = Env::new();
        env.tool("ffmpeg", "exit /b 0");
        // An old file at dest itself (e.g. from an earlier download) that this run did not rewrite.
        let stale = env.out.path().join("clip.mp3");
        std::fs::write(&stale, "old").unwrap();
        let old = SystemTime::now() - Duration::from_secs(3600);
        std::fs::File::options().write(true).open(&stale).unwrap().set_modified(old).unwrap();
        let actual = env.out.path().join("clip.opus");
        env.tool("yt-dlp", &format!("echo x> \"{}\"\r\necho x> \"{}.part\"", actual.display(), env.out.path().join("clip.zzz").display()));
        let m = JobManager::default();
        let sink = Arc::new(VecSink::default());
        m.start(sink.clone(), &seeded_cache(), URL.into(), "0/audio".into(), env.out.path().join("clip.mp3"))
            .await
            .unwrap();
        wait_until("final event", || sink.is_final()).await;
        assert_eq!(sink.events().last().unwrap().state, JobState::Done { path: actual.to_string_lossy().into_owned() });
    }

    /// The MP4 and MP3 rows share a stem: cancelling the MP3 must not delete the MP4 a sibling job just finished.
    #[tokio::test]
    async fn cancel_keeps_a_sibling_jobs_finished_file() {
        let env = Env::new();
        let sibling = env.out.path().join("clip.mp4");
        let part = env.out.path().join("clip.f140.m4a.part");
        let frag = env.out.path().join("clip.mp3.ytdl");
        env.tool("ffmpeg", "exit /b 0");
        env.tool(
            "yt-dlp",
            &format!(
                "echo LUMI^|1^|100^|NA\r\necho mp4> \"{}\"\r\necho x> \"{}\"\r\necho x> \"{}\"\r\nping -n 30 127.0.0.1 >nul",
                sibling.display(),
                part.display(),
                frag.display()
            ),
        );
        let m = JobManager::default();
        let sink = Arc::new(VecSink::default());
        let id = m.start(sink.clone(), &seeded_cache(), URL.into(), "0/audio".into(), env.out.path().join("clip.mp3")).await.unwrap();
        wait_until("files written", || frag.exists()).await;
        m.cancel(&id).unwrap();
        wait_until("final event", || sink.is_final()).await;
        assert_eq!(sink.events().last().unwrap().state, JobState::Cancelled);
        assert!(sibling.exists(), "a complete sibling file is never swept");
        assert!(!part.exists() && !frag.exists(), "partials are swept");
    }

    #[test]
    fn partial_names() {
        for p in ["clip.webm.part", "clip.mp3.ytdl", "clip.temp.mp4", "clip.f137.mp4", "clip.f251-drc.webm.part", "clip.mp4.part-Frag3"] {
            assert!(is_partial("clip", &p.to_lowercase()), "{p}");
        }
        for p in ["clip.mp4", "clip.mp3", "clip.m4a", "clip.webm", "clipper.part"] {
            assert!(!is_partial("clip", p), "{p}");
        }
    }

    async fn start_slow(env: &Env, m: &JobManager, sink: &Arc<VecSink>, dest: &Path) -> String {
        env.tool("ffmpeg", "exit /b 0");
        env.tool(
            "yt-dlp",
            &format!("echo LUMI^|1^|100^|NA\r\necho x> \"{}\"\r\nping -n 30 127.0.0.1 >nul", dest.display()),
        );
        let id = m.start(sink.clone(), &seeded_cache(), URL.into(), "0/audio".into(), dest.to_path_buf()).await.unwrap();
        wait_until("first progress", || sink.events().iter().any(|e| matches!(e.state, JobState::Progress { .. }))).await;
        wait_until("dest written", || dest.exists()).await;
        id
    }

    #[tokio::test]
    async fn duplicate_job_rejected() {
        let env = Env::new();
        let dest = env.out.path().join("clip.mp3");
        let m = JobManager::default();
        let sink = Arc::new(VecSink::default());
        let id = start_slow(&env, &m, &sink, &dest).await;
        let err = m
            .start(sink.clone(), &seeded_cache(), URL.into(), "0/audio".into(), dest.clone())
            .await
            .unwrap_err();
        assert_eq!(err.detail, id);
        assert_eq!(m.active(), 1);
        m.cancel(&id).unwrap();
        wait_until("final event", || sink.is_final()).await;
    }

    #[tokio::test]
    async fn cancel_cleans_temp_dir() {
        let env = Env::new();
        let dest = env.out.path().join("clip.mp3");
        let m = JobManager::default();
        let sink = Arc::new(VecSink::default());
        let id = start_slow(&env, &m, &sink, &dest).await;
        assert!(env.tmp.path().join(&id).exists());
        let t = Instant::now();
        m.cancel(&id).unwrap();
        wait_until("final event", || sink.is_final()).await;
        assert!(t.elapsed() < Duration::from_secs(10), "cancel must not wait for the tool");
        let ev = sink.events();
        assert_eq!(ev.last().unwrap().state, JobState::Cancelled);
        assert_eq!(ev.iter().filter(|e| e.state == JobState::Cancelled).count(), 1);
        assert!(!ev.iter().any(|e| matches!(e.state, JobState::Failed { .. })));
        assert!(!env.tmp.path().join(&id).exists());
        assert!(!dest.exists());
        assert_eq!(m.active(), 0);
        assert!(m.cancel(&id).is_err());
    }

    #[tokio::test]
    async fn cancel_keeps_preexisting_dest() {
        let env = Env::new();
        let dest = env.out.path().join("clip.mp3");
        std::fs::write(&dest, "old").unwrap();
        let m = JobManager::default();
        let sink = Arc::new(VecSink::default());
        env.tool("ffmpeg", "exit /b 0");
        env.tool("yt-dlp", "echo LUMI^|1^|100^|NA\r\nping -n 30 127.0.0.1 >nul");
        let id = m.start(sink.clone(), &seeded_cache(), URL.into(), "0/audio".into(), dest.clone()).await.unwrap();
        wait_until("first progress", || sink.events().iter().any(|e| matches!(e.state, JobState::Progress { .. }))).await;
        m.cancel(&id).unwrap();
        wait_until("final event", || sink.is_final()).await;
        assert_eq!(std::fs::read_to_string(&dest).unwrap().trim(), "old");
    }

    #[tokio::test]
    async fn cancel_removes_part_files_in_dest_dir() {
        let env = Env::new();
        let dest = env.out.path().join("clip.mp3");
        let part = env.out.path().join("clip.webm.part");
        let other = env.out.path().join("unrelated.txt");
        std::fs::write(&other, "keep").unwrap();
        env.tool("ffmpeg", "exit /b 0");
        env.tool(
            "yt-dlp",
            &format!("echo LUMI^|1^|100^|NA\r\necho x> \"{}\"\r\nping -n 30 127.0.0.1 >nul", part.display()),
        );
        let m = JobManager::default();
        let sink = Arc::new(VecSink::default());
        let id = m.start(sink.clone(), &seeded_cache(), URL.into(), "0/audio".into(), dest).await.unwrap();
        wait_until("part file", || part.exists()).await;
        m.cancel(&id).unwrap();
        wait_until("final event", || sink.is_final()).await;
        assert_eq!(sink.events().last().unwrap().state, JobState::Cancelled);
        assert!(!part.exists());
        assert!(other.exists());
        assert!(!env.tmp.path().join(&id).exists());
    }

    #[tokio::test]
    async fn download_all_ytdlp_needs_ffmpeg_and_uses_selector() {
        let env = Env::new();
        let m = JobManager::default();
        let sink = Arc::new(VecSink::default());
        let err = m.start_all(sink.clone(), &seeded_cache(), URL.into(), env.out.path().to_path_buf()).await.unwrap_err();
        assert_eq!(err.error, ErrorCode::ToolMissing { tool: Tool::Ffmpeg });
        assert_eq!(m.active(), 0);
        env.tool("ffmpeg", "exit /b 0");
        let out = env.out.path().to_path_buf();
        env.tool("yt-dlp", &format!("echo %* > \"{}\"", out.join("args.txt").display()));
        m.start_all(sink.clone(), &seeded_cache(), URL.into(), out.clone()).await.unwrap();
        wait_until("final event", || sink.is_final()).await;
        let args = std::fs::read_to_string(out.join("args.txt")).unwrap();
        assert!(args.contains("bv*+ba[format_id!=audio]/b") && args.contains("--newline") && args.contains("--progress-template"), "{args}");
    }

    #[test]
    fn job_event_json_shape() {
        let e = JobEvent { job_id: "j".into(), url: "u".into(), option_id: "0/audio".into(), state: JobState::Progress { fraction: Some(0.5) } };
        assert_eq!(
            serde_json::to_value(&e).unwrap(),
            serde_json::json!({"jobId": "j", "url": "u", "optionId": "0/audio", "state": "progress", "fraction": 0.5})
        );
        let e = JobEvent { job_id: "j".into(), url: "u".into(), option_id: "o".into(), state: JobState::Failed { error: unknown("x") } };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["state"], "failed");
        assert_eq!(v["error"]["error"]["code"], "unknown");
    }

    #[tokio::test]
    async fn download_all_gallery_uses_dest_dir() {
        let env = Env::new();
        let out = env.out.path().to_path_buf();
        // the fake echoes its args into a file so the command line can be asserted
        env.tool("gallery-dl", &format!("echo %* > \"{}\"", out.join("args.txt").display()));
        let mut c = seeded_cache().get(URL).unwrap();
        c.engine = Engine::GalleryDl;
        let cache = ResolveCache::default();
        cache.insert(URL, c);
        let m = JobManager::default();
        let sink = Arc::new(VecSink::default());
        m.start_all(sink.clone(), &cache, URL.into(), out.clone()).await.unwrap();
        wait_until("final event", || sink.is_final()).await;
        assert_eq!(sink.events().last().unwrap().state, JobState::Done { path: out.to_string_lossy().into_owned() });
        assert_eq!(sink.events().first().unwrap().option_id, "all");
        let args = std::fs::read_to_string(out.join("args.txt")).unwrap();
        assert!(args.contains("-D") && args.contains(&*out.to_string_lossy()) && args.contains(URL), "{args}");
    }

    /// Real tools, real TikTok. `cargo test live_download_tiktok_mp3 -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "hits the network and downloads yt-dlp and ffmpeg (~200MB)"]
    async fn live_download_tiktok_mp3() {
        let env = Env::new();
        for t in [Tool::YtDlp, Tool::Ffmpeg] {
            crate::tools::install::install_with(t, &mut |_, _| {}).await.unwrap();
        }
        let url = "https://www.tiktok.com/@scout2015/video/6718335390845095173";
        let cache = ResolveCache::default();
        let info = resolve_url(url, &cache).await.unwrap();
        println!("resolved: {:?} sound={}", info.title, info.sound.is_some());
        let m = JobManager::default();
        for (opt, name) in [("0/audio", "audio.mp3"), ("sound", "sound.mp3")] {
            let dest = env.out.path().join(name);
            let sink = Arc::new(VecSink::default());
            m.start(sink.clone(), &cache, url.into(), opt.into(), dest.clone()).await.unwrap();
            wait_until("final event", || sink.is_final()).await;
            let ev = sink.events();
            println!("{opt}: {} events, last = {:?}", ev.len(), ev.last().unwrap().state);
            assert!(matches!(ev.last().unwrap().state, JobState::Done { .. }), "{opt}: {:?}", ev.last());
            let size = std::fs::metadata(&dest).unwrap().len();
            println!("{opt}: {size} bytes");
            assert!(size > 0);
            let probe = std::process::Command::new(env.bin.path().join("ffprobe.exe"))
                .args(["-v", "error", "-show_format"])
                .arg(&dest)
                .output()
                .unwrap();
            let text = String::from_utf8_lossy(&probe.stdout).into_owned();
            println!("{text}");
            if opt == "0/audio" {
                let tag = |k: &str| {
                    text.lines().find_map(|l| l.strip_prefix(&format!("TAG:{k}="))).map(str::to_owned).unwrap_or_default()
                };
                let title = tag("title");
                assert!(!title.is_empty() && (info.title.starts_with(&title) || title.starts_with(&info.title)), "title tag {title:?} vs {:?}", info.title);
                assert_eq!(Some(tag("artist")), info.author, "artist tag");
            }
        }
    }
}
