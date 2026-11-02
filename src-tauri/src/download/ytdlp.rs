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

/// Locate an executable in `PATH` or in the usual locations (cross-platform).
pub fn find_executable(name: &str, env_override: &str) -> Option<PathBuf> {
    let names: Vec<String> = if cfg!(windows) {
        vec![format!("{name}.exe"), name.to_string()]
    } else {
        vec![name.to_string()]
    };
    let check = |p: &std::path::Path| p.is_file();

    // 1 env override.
    if let Ok(custom) = std::env::var(env_override) {
        for n in &names {
            let path = PathBuf::from(&custom);
            if check(&path) {
                return Some(path);
            }
            let p = path.join(n);
            if check(&p) {
                return Some(p);
            }
        }
    }

    // 2 PATH.
    let sep = if cfg!(windows) { ';' } else { ':' };
    if let Ok(path) = std::env::var("PATH") {
        for dir in path.split(sep) {
            if dir.is_empty() {
                continue;
            }
            for n in &names {
                let candidate = Path::new(dir).join(n);
                if check(&candidate) {
                    return Some(candidate);
                }
            }
        }
    }

    // 3 Standard locations (unix).
    #[cfg(not(windows))]
    for dir in [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/usr/bin",
        "/opt/local/bin",
    ] {
        for n in &names {
            let candidate = Path::new(dir).join(n);
            if check(&candidate) {
                return Some(candidate);
            }
        }
    }

    // 4 User local bin.
    for var in ["HOME", "USERPROFILE"] {
        if let Ok(home) = std::env::var(var) {
            for n in &names {
                let candidate = Path::new(&home).join(".local/bin").join(n);
                if check(&candidate) {
                    return Some(candidate);
                }
                let candidate = Path::new(&home).join("AppData/Local/Programs").join(n);
                if check(&candidate) {
                    return Some(candidate);
                }
            }
        }
    }

    // 5 Bundled next to exe.
    if let Ok(exec_path) = std::env::current_exe() {
        if let Some(parent) = exec_path.parent() {
            for n in &names {
                let bundled = parent.join(n);
                if check(&bundled) {
                    return Some(bundled);
                }
            }
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