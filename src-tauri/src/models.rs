use serde::{Deserialize, Serialize};

/// The kind of content the user wants out of a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    Video,
    Audio,
    /// Video track and extracted audio track at the same time.
    #[serde(rename = "video_audio")]
    VideoAudio,
    Image,
    Gif,
    Text,
}

impl Target {
    pub fn label(self) -> &'static str {
        match self {
            Target::Video => "Video",
            Target::Audio => "Audio",
            Target::VideoAudio => "Video + Audio",
            Target::Image => "Image",
            Target::Gif => "GIF",
            Target::Text => "Text",
        }
    }
}

/// Lifecycle of a single queued item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobState {
    Validating,
    Downloading,
    Extracting,
    Converting,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
pub struct Progress {
    pub job_id: String,
    /// Zero based index of the current item in the queue.
    pub index: usize,
    pub total: usize,
    pub state: JobState,
    pub url: String,
    pub filename: Option<String>,
    /// 0.0 - 100.0 when the underlying tool reports reliable progress.
    pub percent: Option<f64>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileInfo {
    pub path: String,
    pub name: String,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ItemResult {
    pub job_id: String,
    pub index: usize,
    pub url: String,
    pub ok: bool,
    pub files: Vec<FileInfo>,
    pub error: Option<crate::error::AppError>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobResult {
    pub job_id: String,
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub cancelled: bool,
}

/// Where the frontend can find the optional external tools.
#[derive(Debug, Clone, Serialize)]
pub struct Dependencies {
    pub yt_dlp: Option<String>,
    pub yt_dlp_version: Option<String>,
    pub ffmpeg: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::Target;

    /// The frontend sends these exact strings.
    #[test]
    fn deserializes_frontend_values() {
        let cases = [
            ("video", Target::Video),
            ("audio", Target::Audio),
            ("video_audio", Target::VideoAudio),
            ("image", Target::Image),
            ("gif", Target::Gif),
            ("text", Target::Text),
        ];
        for (value, expected) in cases {
            let parsed: Target = serde_json::from_str(&format!("\"{value}\"")).unwrap();
            assert_eq!(parsed, expected);
        }
    }
}
