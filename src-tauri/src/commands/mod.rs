use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter, Manager, State};

use crate::download::{self, Ctx};
use crate::error::{AppError, AppResult};
use crate::models::{Dependencies, ItemResult, JobResult, JobState, Progress, Target};
use crate::security::net::build_client;
use crate::security::parse_and_normalize;
use crate::sources::{self, SourceInfo};

pub const EVENT_PROGRESS: &str = "linkfetch://progress";
pub const EVENT_ITEM: &str = "linkfetch://item";
pub const EVENT_JOB: &str = "linkfetch://job";

/// Shared application state.
pub struct AppState {
    pub client: reqwest::Client,
    /// Cancellation flag per running job.
    pub jobs: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            client: build_client(std::time::Duration::from_secs(300)).expect("http client"),
            jobs: Mutex::new(HashMap::new()),
        }
    }
}

/// The user's Downloads folder, created if it does not exist.
pub fn default_destination() -> AppResult<PathBuf> {
    let home = std::env::var("HOME").map_err(|_| AppError::Other("no home directory".into()))?;
    let downloads = PathBuf::from(home).join("Downloads");
    if !downloads.exists() {
        std::fs::create_dir_all(&downloads)?;
    }
    Ok(downloads)
}

/// Start a queue of links. Returns the job id.
#[tauri::command]
pub async fn start_download(
    app: AppHandle,
    state: State<'_, AppState>,
    urls: Vec<String>,
    target: Target,
    destination: Option<String>,
) -> AppResult<String> {
    if urls.is_empty() {
        return Err(AppError::InvalidUrl("no links given".into()));
    }

    let dest = match destination {
        Some(path) if !path.trim().is_empty() => PathBuf::from(path),
        _ => default_destination()?,
    };
    std::fs::create_dir_all(&dest)?;

    let job_id = format!(
        "job-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default()
    );

    let cancel = Arc::new(AtomicBool::new(false));
    state
        .jobs
        .lock()
        .unwrap()
        .insert(job_id.clone(), Arc::clone(&cancel));

    let client = state.client.clone();
    let job_for_task = job_id.clone();
    tauri::async_runtime::spawn(async move {
        run_job(app, client, job_for_task, urls, target, dest, cancel).await;
    });

    Ok(job_id)
}

/// Cancel a running job.
#[tauri::command]
pub fn cancel_download(state: State<'_, AppState>, job_id: String) {
    if let Some(flag) = state.jobs.lock().unwrap().get(&job_id) {
        flag.store(true, Ordering::Relaxed);
    }
}

/// Which external tools are available.
#[tauri::command]
pub async fn dependency_status() -> Dependencies {
    Dependencies {
        yt_dlp: download::ytdlp::yt_dlp_path().map(|p| p.display().to_string()),
        yt_dlp_version: download::ytdlp::yt_dlp_version().await,
        ffmpeg: download::ytdlp::ffmpeg_path().map(|p| p.display().to_string()),
    }
}

/// The default destination folder.
#[tauri::command]
pub fn default_destination_command() -> AppResult<String> {
    Ok(default_destination()?.display().to_string())
}

/// Ask the user for a destination folder.
#[tauri::command]
pub async fn pick_destination(app: AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;

    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("Choose destination folder")
        .pick_folder(move |picked| {
            let _ = tx.send(
                picked
                    .and_then(|p| p.into_path().ok())
                    .map(|p| p.display().to_string()),
            );
        });
    rx.await.ok().flatten()
}

/// Reveal a file in Finder.
#[tauri::command]
pub fn reveal_in_finder(path: String) -> AppResult<()> {
    let target = PathBuf::from(&path);
    if !target.exists() {
        return Err(AppError::Other("the file no longer exists".into()));
    }
    std::process::Command::new("open")
        .arg("-R")
        .arg(&target)
        .spawn()
        .map_err(|e| AppError::Other(format!("could not reveal the file: {e}")))?;
    Ok(())
}

/// Open a file with the default application.
#[tauri::command]
pub fn open_file(app: AppHandle, path: String) -> AppResult<()> {
    use tauri_plugin_opener::OpenerExt;

    let target = PathBuf::from(&path);
    if !target.exists() {
        return Err(AppError::Other("the file no longer exists".into()));
    }
    app.opener()
        .open_path(target.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| AppError::Other(format!("could not open the file: {e}")))
}

/// Describe a link without downloading it.
#[tauri::command]
pub fn describe_url(url: String, target: Target) -> AppResult<SourceInfo> {
    let parsed = parse_and_normalize(&url)?;
    Ok(sources::describe(&parsed, target))
}

/// Run every link of a job, one after the other.
async fn run_job(
    app: AppHandle,
    client: reqwest::Client,
    job_id: String,
    urls: Vec<String>,
    target: Target,
    dest: PathBuf,
    cancel: Arc<AtomicBool>,
) {
    let total = urls.len();
    let mut succeeded = 0usize;
    let mut failed = 0usize;
    let mut cancelled = false;

    for (index, raw) in urls.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            cancelled = true;
            break;
        }

        let parsed = match parse_and_normalize(raw) {
            Ok(url) => url,
            Err(err) => {
                failed += 1;
                emit_item(
                    &app,
                    ItemResult {
                        job_id: job_id.clone(),
                        index,
                        url: raw.clone(),
                        ok: false,
                        files: Vec::new(),
                        error: Some(err),
                    },
                );
                continue;
            }
        };

        emit_progress(
            &app,
            Progress {
                job_id: job_id.clone(),
                index,
                total,
                state: JobState::Validating,
                url: parsed.to_string(),
                filename: None,
                percent: None,
                message: None,
            },
        );

        let plan = match sources::route(&parsed, target) {
            Ok(plan) => plan,
            Err(err) => {
                failed += 1;
                emit_item(
                    &app,
                    ItemResult {
                        job_id: job_id.clone(),
                        index,
                        url: parsed.to_string(),
                        ok: false,
                        files: Vec::new(),
                        error: Some(err),
                    },
                );
                continue;
            }
        };

        let ctx = Ctx {
            dest: dest.clone(),
            cancel: Arc::clone(&cancel),
            allow_private: false,
        };

        let handle = app.clone();
        let report_url = parsed.to_string();
        let report_job = job_id.clone();
        let report = move |state: JobState, percent: Option<f64>, message: Option<String>| {
            emit_progress(
                &handle,
                Progress {
                    job_id: report_job.clone(),
                    index,
                    total,
                    state,
                    url: report_url.clone(),
                    filename: message,
                    percent,
                    message: None,
                },
            );
        };

        let outcome = download::run_plan(&client, plan, &ctx, &report).await;

        match outcome {
            Ok(files) => {
                succeeded += 1;
                emit_progress(
                    &app,
                    Progress {
                        job_id: job_id.clone(),
                        index,
                        total,
                        state: JobState::Completed,
                        url: parsed.to_string(),
                        filename: files.first().map(|f| f.name.clone()),
                        percent: Some(100.0),
                        message: None,
                    },
                );
                emit_item(
                    &app,
                    ItemResult {
                        job_id: job_id.clone(),
                        index,
                        url: parsed.to_string(),
                        ok: true,
                        files,
                        error: None,
                    },
                );
            }
            Err(err) => {
                if err.code() == "cancelled" {
                    cancelled = true;
                    break;
                }
                failed += 1;
                emit_progress(
                    &app,
                    Progress {
                        job_id: job_id.clone(),
                        index,
                        total,
                        state: JobState::Failed,
                        url: parsed.to_string(),
                        filename: None,
                        percent: None,
                        message: Some(err.to_string()),
                    },
                );
                emit_item(
                    &app,
                    ItemResult {
                        job_id: job_id.clone(),
                        index,
                        url: parsed.to_string(),
                        ok: false,
                        files: Vec::new(),
                        error: Some(err),
                    },
                );
            }
        }
    }

    if cancelled {
        emit_progress(
            &app,
            Progress {
                job_id: job_id.clone(),
                index: total.saturating_sub(1),
                total,
                state: JobState::Cancelled,
                url: String::new(),
                filename: None,
                percent: None,
                message: None,
            },
        );
    }

    let _ = app.state::<AppState>().jobs.lock().unwrap().remove(&job_id);

    let _ = app.emit(
        EVENT_JOB,
        JobResult {
            job_id,
            total,
            succeeded,
            failed,
            cancelled,
        },
    );
}

fn emit_progress(app: &AppHandle, progress: Progress) {
    let _ = app.emit(EVENT_PROGRESS, progress);
}

fn emit_item(app: &AppHandle, item: ItemResult) {
    let _ = app.emit(EVENT_ITEM, item);
}
