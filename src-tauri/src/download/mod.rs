use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::AppResult;
use crate::models::{FileInfo, JobState};
use crate::sources::Plan;

/// Everything a single queued item needs to run.
pub struct Ctx {
    pub dest: PathBuf,
    pub cancel: Arc<AtomicBool>,
    /// Only tests set this: it allows fetching from 127.0.0.1 mock servers.
    pub allow_private: bool,
}

impl Ctx {
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// Reject links that would reach the local network.
    pub fn ensure_allowed(&self, url: &url::Url) -> AppResult<()> {
        if self.allow_private {
            return Ok(());
        }
        crate::security::ensure_fetchable(url)
    }
}

/// Reports state changes for the item that is currently running.
pub type Report<'a> = &'a (dyn Fn(JobState, Option<f64>, Option<String>) + Send + Sync);

#[cfg(test)]
mod tests;

pub mod media;
pub mod text;
pub mod ytdlp;

/// Execute the routed plan and return the produced files.
pub async fn run_plan(
    client: &reqwest::Client,
    plan: Plan,
    ctx: &Ctx,
    report: Report<'_>,
) -> AppResult<Vec<FileInfo>> {
    match plan {
        Plan::DirectMedia { url, kind } => {
            report(JobState::Downloading, None, None);
            media::download(client, &url, kind, ctx, report).await
        }
        Plan::Text { url } => {
            report(JobState::Extracting, None, None);
            text::extract(client, &url, ctx).await
        }
        Plan::YtDlp { url, target } => {
            report(JobState::Downloading, None, None);
            ytdlp::download(&url, target, ctx, report).await
        }
    }
}
