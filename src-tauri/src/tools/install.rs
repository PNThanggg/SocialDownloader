use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use super::paths::{bin_dir, exe_name};
use super::Tool;
use crate::errors::{ErrorCode, LumiError};

const YTDLP_BASE: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/";
const FFMPEG_ZIP: &str = "ffmpeg-master-latest-win64-gpl.zip";
const FFMPEG_BASE: &str = "https://github.com/yt-dlp/FFmpeg-Builds/releases/download/latest/";
const DENO_ZIP: &str = "deno-x86_64-pc-windows-msvc.zip";
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

/// What to fetch for a tool: the file URL and, if any, its checksum list URL.
struct Source {
    url: String,
    checksum_url: Option<String>,
    /// File name looked up inside the checksum list.
    checksum_name: &'static str,
}

fn source(t: Tool) -> Source {
    match t {
        Tool::YtDlp => Source {
            url: format!("{YTDLP_BASE}yt-dlp.exe"),
            checksum_url: Some(format!("{YTDLP_BASE}SHA2-256SUMS")),
            checksum_name: "yt-dlp.exe",
        },
        Tool::GalleryDl => Source {
            url: "https://github.com/mikf/gallery-dl/releases/latest/download/gallery-dl.exe".into(),
            checksum_url: None,
            checksum_name: "gallery-dl.exe",
        },
        Tool::Ffmpeg => Source {
            url: format!("{FFMPEG_BASE}{FFMPEG_ZIP}"),
            checksum_url: Some(format!("{FFMPEG_BASE}checksums.sha256")),
            checksum_name: FFMPEG_ZIP,
        },
        Tool::Deno => Source {
            url: format!("https://github.com/denoland/deno/releases/latest/download/{DENO_ZIP}"),
            checksum_url: Some(format!(
                "https://github.com/denoland/deno/releases/latest/download/{DENO_ZIP}.sha256sum"
            )),
            checksum_name: DENO_ZIP,
        },
    }
}

fn net_err(e: impl std::fmt::Display) -> LumiError {
    LumiError::new(ErrorCode::Network, e.to_string())
}

fn io_err(e: impl std::fmt::Display) -> LumiError {
    LumiError::new(ErrorCode::Unknown, e.to_string())
}

fn client() -> Result<reqwest::Client, LumiError> {
    reqwest::Client::builder().user_agent("Lumi").build().map_err(net_err)
}

/// Content-Length of the file that would be downloaded (HEAD, following redirects).
pub async fn download_size(t: Tool) -> Result<Option<u64>, LumiError> {
    let resp = client()?.head(source(t).url).send().await.map_err(net_err)?;
    let resp = resp.error_for_status().map_err(net_err)?;
    Ok(resp
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok()))
}

/// Finds the hash for `name` in a `sha256sum`-style list. A list with a single
/// entry (or a bare hash) is accepted regardless of the file name.
pub fn parse_sha256sums(text: &str, name: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    for line in &lines {
        let mut parts = line.split_whitespace();
        let (Some(hash), Some(file)) = (parts.next(), parts.next()) else { continue };
        let file = file.trim_start_matches('*');
        let base = file.rsplit(['/', '\\']).next().unwrap_or(file);
        if base == name {
            return Some(hash.to_lowercase());
        }
    }
    if let [only] = lines.as_slice() {
        return only.split_whitespace().next().map(str::to_lowercase);
    }
    None
}

pub fn verify_sha256(path: &Path, expected: &str) -> Result<(), LumiError> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).map_err(io_err)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf).map_err(io_err)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let actual = hex::encode(hasher.finalize());
    if actual.eq_ignore_ascii_case(expected.trim()) {
        Ok(())
    } else {
        Err(LumiError::new(
            ErrorCode::Unknown,
            format!("checksum mismatch: expected {expected}, got {actual}"),
        ))
    }
}

/// Moves a finished `.download` file into place; removes it if that fails.
pub fn finalize_download(tmp: &Path, dest: &Path) -> Result<(), LumiError> {
    std::fs::rename(tmp, dest).map_err(|e| {
        let _ = std::fs::remove_file(tmp);
        io_err(format!("cannot move {} to {}: {e}", tmp.display(), dest.display()))
    })
}

fn tmp_path(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(".download");
    dest.with_file_name(name)
}

type Progress<'a> = &'a mut (dyn FnMut(u64, Option<u64>) + Send);

/// Streams `url` into `tmp`. On any failure `tmp` is removed.
async fn download_to(
    client: &reqwest::Client,
    url: &str,
    tmp: &Path,
    progress: Progress<'_>,
) -> Result<(), LumiError> {
    let result = download_inner(client, url, tmp, progress).await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(tmp).await;
    }
    result
}

async fn download_inner(
    client: &reqwest::Client,
    url: &str,
    tmp: &Path,
    progress: Progress<'_>,
) -> Result<(), LumiError> {
    let resp = client.get(url).send().await.map_err(net_err)?.error_for_status().map_err(net_err)?;
    let total = resp.content_length();
    let mut file = tokio::fs::File::create(tmp).await.map_err(io_err)?;
    let mut stream = resp.bytes_stream();
    let (mut downloaded, mut last) = (0u64, Instant::now());
    progress(0, total);
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(net_err)?;
        file.write_all(&chunk).await.map_err(io_err)?;
        downloaded += chunk.len() as u64;
        if last.elapsed() >= PROGRESS_INTERVAL {
            progress(downloaded, total);
            last = Instant::now();
        }
    }
    file.flush().await.map_err(io_err)?;
    drop(file);
    if let Some(t) = total {
        if t != downloaded {
            return Err(net_err(format!("incomplete download: {downloaded} of {t} bytes")));
        }
    }
    progress(downloaded, total);
    Ok(())
}

async fn fetch_text(client: &reqwest::Client, url: &str) -> Result<String, LumiError> {
    let resp = client.get(url).send().await.map_err(net_err)?.error_for_status().map_err(net_err)?;
    resp.text().await.map_err(net_err)
}

/// Extracts the first entry matching each `wanted` suffix (e.g. `bin/ffmpeg.exe`)
/// into `dest_dir` under its base name, atomically per file.
pub fn extract_zip_entries(zip_path: &Path, dest_dir: &Path, wanted: &[&str]) -> Result<(), LumiError> {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(zip_path).map_err(io_err)?).map_err(io_err)?;
    for suffix in wanted {
        let mut found = false;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(io_err)?;
            let name = entry.name().map_err(io_err)?.replace('\\', "/");
            if entry.is_dir() || !(name == *suffix || name.ends_with(&format!("/{suffix}"))) {
                continue;
            }
            let base = suffix.rsplit('/').next().unwrap_or(suffix);
            let dest = dest_dir.join(base);
            let tmp = tmp_path(&dest);
            let written = std::fs::File::create(&tmp)
                .and_then(|mut f| std::io::copy(&mut entry, &mut f).map(|_| ()));
            if let Err(e) = written {
                let _ = std::fs::remove_file(&tmp);
                return Err(io_err(e));
            }
            finalize_download(&tmp, &dest)?;
            found = true;
            break;
        }
        if !found {
            return Err(LumiError::new(ErrorCode::Unknown, format!("{suffix} not found in archive")));
        }
    }
    Ok(())
}

/// Downloads and installs `t` into `bin_dir()`. Progress is reported through `progress`.
pub async fn install_with(t: Tool, progress: Progress<'_>) -> Result<(), LumiError> {
    let dir = bin_dir();
    tokio::fs::create_dir_all(&dir).await.map_err(io_err)?;
    let client = client()?;
    let src = source(t);
    let is_zip = matches!(t, Tool::Ffmpeg | Tool::Deno);
    let dest = if is_zip { dir.join(format!("{}.zip", exe_name(t))) } else { dir.join(exe_name(t)) };
    let tmp = tmp_path(&dest);

    download_to(&client, &src.url, &tmp, progress).await?;
    let finish = async {
        if let Some(url) = &src.checksum_url {
            let text = fetch_text(&client, url).await?;
            let expected = parse_sha256sums(&text, src.checksum_name)
                .ok_or_else(|| io_err(format!("no checksum for {} in {url}", src.checksum_name)))?;
            verify_sha256(&tmp, &expected)?;
        }
        Ok::<(), LumiError>(())
    }
    .await;
    if let Err(e) = finish {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(e);
    }

    if !is_zip {
        return finalize_download(&tmp, &dest);
    }
    let wanted: &[&str] = if t == Tool::Ffmpeg { &["bin/ffmpeg.exe", "bin/ffprobe.exe"] } else { &["deno.exe"] };
    let (tmp2, dir2) = (tmp.clone(), dir.clone());
    let extracted = tokio::task::spawn_blocking(move || extract_zip_entries(&tmp2, &dir2, wanted))
        .await
        .map_err(io_err)?;
    let _ = tokio::fs::remove_file(&tmp).await;
    extracted
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parse_sha256sums_finds_entry() {
        let text = "abc123  yt-dlp.exe\ndef  yt-dlp_x86.exe";
        assert_eq!(parse_sha256sums(text, "yt-dlp.exe"), Some("abc123".into()));
        assert_eq!(parse_sha256sums(text, "yt-dlp_x86.exe"), Some("def".into()));
        assert_eq!(parse_sha256sums(text, "nope.exe"), None);
        assert_eq!(parse_sha256sums("abc123 *deno.zip\n", "deno-x.zip"), Some("abc123".into()));
        assert_eq!(parse_sha256sums("abc123\n", "deno-x.zip"), Some("abc123".into()));
        assert_eq!(
            parse_sha256sums("AA *ffmpeg-master-latest-win64-gpl.zip\nbb  other.zip", FFMPEG_ZIP),
            Some("aa".into())
        );
    }

    #[test]
    fn verify_sha256_mismatch_errors() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a.bin");
        std::fs::write(&f, b"hello").unwrap();
        // sha256("hello")
        let good = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        assert!(verify_sha256(&f, good).is_ok());
        let err = verify_sha256(&f, "deadbeef").unwrap_err();
        assert_eq!(err.error, ErrorCode::Unknown);
        assert!(err.detail.contains("checksum mismatch"));
    }

    #[test]
    fn extract_ffmpeg_from_zip() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("f.zip");
        let mut w = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        for (name, body) in [
            ("ffmpeg-x/bin/ffmpeg.exe", "F"),
            ("ffmpeg-x/bin/ffprobe.exe", "P"),
            ("ffmpeg-x/doc/a.html", "H"),
        ] {
            w.start_file(name, opts).unwrap();
            w.write_all(body.as_bytes()).unwrap();
        }
        w.finish().unwrap();

        let out = dir.path().join("out");
        std::fs::create_dir(&out).unwrap();
        extract_zip_entries(&zip_path, &out, &["bin/ffmpeg.exe", "bin/ffprobe.exe"]).unwrap();
        let mut names: Vec<String> = std::fs::read_dir(&out)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, ["ffmpeg.exe", "ffprobe.exe"]);
        assert_eq!(std::fs::read_to_string(out.join("ffmpeg.exe")).unwrap(), "F");

        let err = extract_zip_entries(&zip_path, &out, &["deno.exe"]).unwrap_err();
        assert!(err.detail.contains("not found"));
    }

    #[test]
    fn install_writes_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("yt-dlp.exe");
        let tmp = tmp_path(&dest);
        assert_eq!(tmp.file_name().unwrap(), "yt-dlp.exe.download");
        std::fs::write(&tmp, b"data").unwrap();
        finalize_download(&tmp, &dest).unwrap();
        assert!(dest.exists() && !tmp.exists());

        // failure case: nothing to finalize, no exe appears
        let dest2 = dir.path().join("gallery-dl.exe");
        assert!(finalize_download(&tmp_path(&dest2), &dest2).is_err());
        assert!(!dest2.exists());
    }

    #[tokio::test]
    async fn failed_download_leaves_no_partial_file() {
        use tokio::io::AsyncWriteExt as _;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n0123456789").await;
            let _ = s.shutdown().await;
        });
        let dir = tempfile::tempdir().unwrap();
        let tmp = dir.path().join("x.exe.download");
        let mut noop = |_: u64, _: Option<u64>| {};
        let err = download_to(&client().unwrap(), &format!("http://127.0.0.1:{port}/x"), &tmp, &mut noop)
            .await
            .unwrap_err();
        assert_eq!(err.error, ErrorCode::Network);
        assert!(!tmp.exists());
        assert!(!dir.path().join("x.exe").exists());
    }

    #[tokio::test]
    #[ignore = "hits the network"]
    async fn live_install_ytdlp() {
        let _g = crate::tools::paths::TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("LUMI_BIN_DIR", dir.path()) };
        let mut events = 0u32;
        let mut cb = |_: u64, _: Option<u64>| events += 1;
        install_with(Tool::YtDlp, &mut cb).await.unwrap();
        let st = crate::tools::statuses().await;
        unsafe { std::env::remove_var("LUMI_BIN_DIR") };
        let y = st.iter().find(|s| s.tool == Tool::YtDlp).unwrap();
        println!("status: {y:?}, progress events: {events}");
        assert!(y.installed);
        assert!(y.version.is_some());
        assert!(!dir.path().join("yt-dlp.exe.download").exists());
    }
}
