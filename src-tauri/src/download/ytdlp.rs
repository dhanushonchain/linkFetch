use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use url::Url;

use crate::download::{Ctx, Report};
use crate::error::{AppError, AppResult};
use crate::models::{FileInfo, JobState};
use crate::security::filename::sanitize_stem;

/// Locate an executable in `PATH` or in the usual Homebrew / user locations.
pub fn find_executable(name: &str, env_override: &str) -> Option<PathBuf> {
    if let Ok(custom) = std::env::var(env_override) {
        let path = PathBuf::from(custom);
        if path.is_file() {
            return Some(path);
        }
    }

    if let Ok(path) = std::env::var("PATH") {
        for dir in path.split(':') {
            if dir.is_empty() {
                continue;
            }
            let candidate = Path::new(dir).join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    for dir in [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/usr/bin",
        "/opt/local/bin",
    ] {
        let candidate = Path::new(dir).join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let candidate = Path::new(&home).join(".local/bin").join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Path of the yt-dlp binary, if it is installed.
pub fn yt_dlp_path() -> Option<PathBuf> {
    find_executable("yt-dlp", "LINKFETCH_YT_DLP_PATH")
}

/// Path of the ffmpeg binary, if it is installed.
pub fn ffmpeg_path() -> Option<PathBuf> {
    find_executable("ffmpeg", "LINKFETCH_FFMPEG_PATH")
}

/// Version string of yt-dlp.
pub async fn yt_dlp_version() -> Option<String> {
    let bin = yt_dlp_path()?;
    let output = Command::new(bin).arg("--version").output().await.ok()?;
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Run yt-dlp for a video or an audio track.
pub async fn download(
    url: &Url,
    target: crate::models::Target,
    ctx: &Ctx,
    report: Report<'_>,
) -> AppResult<Vec<FileInfo>> {
    ctx.ensure_allowed(url)?;
    let bin = yt_dlp_path().ok_or(AppError::MissingYtDlp)?;
    let ffmpeg = ffmpeg_path();
    let want_audio = target == crate::models::Target::Audio;

    if want_audio && ffmpeg.is_none() {
        return Err(AppError::MissingFfmpeg);
    }

    let template = format!("{}/%(title).80B [%(id)s].%(ext)s", ctx.dest.display());
    let mut args: Vec<String> = vec![
        "--no-playlist".into(),
        "--no-warnings".into(),
        "--newline".into(),
        "--no-simulate".into(),
        "--progress".into(),
        "--progress-template".into(),
        "dl|%(progress.status)s|%(progress.downloaded_bytes)s|%(progress.total_bytes)s|%(progress.total_bytes_estimate)s".into(),
        "--print".into(),
        "after_move:filepath".into(),
        "-o".into(),
        template,
    ];
    if want_audio {
        args.extend([
            "-x".into(),
            "--audio-format".into(),
            "mp3".into(),
            "--audio-quality".into(),
            "0".into(),
        ]);
    } else if ffmpeg.is_some() {
        args.extend([
            "-f".into(),
            "bv*+ba/b".into(),
            "--merge-output-format".into(),
            "mp4".into(),
        ]);
    } else {
        // Without FFmpeg we can only take a single pre-muxed stream.
        args.extend(["-f".into(), "best".into()]);
    }
    args.push(url.to_string());

    let mut child = Command::new(&bin)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| AppError::Other(format!("could not start yt-dlp: {e}")))?;

    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");
    let stderr_buf = Arc::new(Mutex::new(String::new()));
    let stderr_task = {
        let buf = Arc::clone(&stderr_buf);
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let mut guard = buf.lock().unwrap();
                if guard.len() < 8000 {
                    guard.push_str(&line);
                    guard.push('\n');
                }
            }
        })
    };

    let mut lines = BufReader::new(stdout).lines();
    let mut output_path: Option<PathBuf> = None;

    while let Some(line) = lines.next_line().await? {
        if ctx.is_cancelled() {
            let _ = child.start_kill();
            let _ = child.wait().await;
            stderr_task.abort();
            return Err(AppError::Cancelled);
        }
        let line = line.trim_end().to_string();
        if line.starts_with("dl|") {
            handle_progress(&line, report);
        } else if line.starts_with('/') {
            output_path = Some(PathBuf::from(line));
        }
    }

    let status = child.wait().await?;
    stderr_task.abort();

    if !status.success() {
        let err = stderr_buf.lock().unwrap().clone();
        return Err(map_ytdlp_error(&err, want_audio));
    }

    let path = output_path
        .ok_or_else(|| AppError::Other("yt-dlp finished but produced no file.".into()))?;

    let path = path.canonicalize().unwrap_or(path);
    if !path.starts_with(ctx.dest.canonicalize().unwrap_or_else(|_| ctx.dest.clone())) {
        return Err(AppError::PermissionDenied(
            "yt-dlp wrote outside the destination folder".into(),
        ));
    }

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| sanitize_stem(&path.to_string_lossy()));
    let size = tokio::fs::metadata(&path).await.ok().map(|m| m.len());

    Ok(vec![FileInfo {
        path: path.to_string_lossy().to_string(),
        name,
        size,
    }])
}

/// Interpret a yt-dlp progress line (`dl|status|downloaded|total|estimate`).
fn handle_progress(line: &str, report: Report) {
    let parts: Vec<&str> = line.split('|').collect();
    if parts.len() < 5 {
        return;
    }
    let status = parts[1].trim();
    if status != "downloading" && status != "finished" {
        return;
    }
    let downloaded = parts[2].trim().parse::<u64>().ok();
    let total = parts[3]
        .trim()
        .parse::<u64>()
        .ok()
        .or_else(|| parts[4].trim().parse::<u64>().ok());

    let percent = match (downloaded, total) {
        (Some(d), Some(t)) if t > 0 => Some((d as f64 / t as f64 * 100.0).clamp(0.0, 100.0)),
        _ => None,
    };
    report(JobState::Downloading, percent, None);
}

/// Translate yt-dlp's stderr into a user facing error.
fn map_ytdlp_error(stderr: &str, audio: bool) -> AppError {
    let lower = stderr.to_ascii_lowercase();
    if lower.contains("unsupported url") {
        return AppError::UnsupportedSource("This source is not supported yet.".into());
    }
    if lower.contains("sign in to confirm")
        || lower.contains("login required")
        || lower.contains("private video")
    {
        return AppError::PermissionDenied(
            "this link requires a login, which linkFetch does not use".into(),
        );
    }
    if lower.contains("requested format is not available") && !audio {
        return AppError::UnsupportedTarget("No downloadable video format is available.".into());
    }
    if lower.contains("ffmpeg") || lower.contains("ffprobe") {
        return AppError::MissingFfmpeg;
    }
    if lower.contains("postprocessing") || lower.contains("conversion") {
        return AppError::Conversion("the file could not be converted".into());
    }
    if lower.contains("timed out") || lower.contains("timeout") {
        return AppError::Timeout;
    }
    let first = stderr
        .lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .unwrap_or("yt-dlp failed");
    AppError::Other(first.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_progress_line() {
        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = calls.clone();
        let report = move |state: JobState, percent: Option<f64>, name: Option<String>| {
            sink.lock().unwrap().push((state, percent, name));
        };
        handle_progress("dl|downloading|5242880|10485760|NA", &report);
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, Some(50.0));
    }

    #[test]
    fn maps_ytdlp_errors() {
        assert_eq!(
            map_ytdlp_error("ERROR: Unsupported URL: https://example.com", false).code(),
            "unsupported_source"
        );
        assert_eq!(
            map_ytdlp_error("ERROR: Sign in to confirm you're not a bot", false).code(),
            "permission_denied"
        );
        assert_eq!(
            map_ytdlp_error("ERROR: ffmpeg not found", true).code(),
            "missing_ffmpeg"
        );
    }
}
