use serde::ser::{Serialize, SerializeStruct, Serializer};

/// Every failure the frontend can see. Each variant maps to a stable `code`
/// plus a human readable `message`.
#[derive(Debug, Clone, thiserror::Error)]
pub enum AppError {
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("{0}")]
    UnsupportedSource(String),

    #[error("{0}")]
    UnsupportedTarget(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("The server returned HTTP {0}.")]
    Http(u16),

    #[error("The download timed out.")]
    Timeout,

    #[error("There is not enough disk space.")]
    NoSpace,

    #[error("yt-dlp is not installed. Install it with `brew install yt-dlp`.")]
    MissingYtDlp,

    #[error("FFmpeg is not installed. Install it with `brew install ffmpeg`.")]
    MissingFfmpeg,

    #[error("Conversion failed: {0}")]
    Conversion(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Unexpected response: {0}")]
    InvalidMedia(String),

    #[error("Cancelled.")]
    Cancelled,

    #[error("{0}")]
    Other(String),
}

impl AppError {
    /// Stable machine readable identifier used by the UI for message mapping.
    pub fn code(&self) -> &'static str {
        match self {
            AppError::InvalidUrl(_) => "invalid_url",
            AppError::UnsupportedSource(_) => "unsupported_source",
            AppError::UnsupportedTarget(_) => "unsupported_target",
            AppError::Network(_) => "network",
            AppError::Http(_) => "http",
            AppError::Timeout => "timeout",
            AppError::NoSpace => "no_space",
            AppError::MissingYtDlp => "missing_ytdlp",
            AppError::MissingFfmpeg => "missing_ffmpeg",
            AppError::Conversion(_) => "conversion",
            AppError::PermissionDenied(_) => "permission_denied",
            AppError::InvalidMedia(_) => "invalid_media",
            AppError::Cancelled => "cancelled",
            AppError::Other(_) => "other",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("code", self.code())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            AppError::Timeout
        } else if e.is_connect() {
            AppError::Network("could not connect to the server".into())
        } else {
            AppError::Network(e.to_string())
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::PermissionDenied => AppError::PermissionDenied(e.to_string()),
            std::io::ErrorKind::TimedOut => AppError::Timeout,
            std::io::ErrorKind::StorageFull => AppError::NoSpace,
            _ => AppError::Other(e.to_string()),
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;
