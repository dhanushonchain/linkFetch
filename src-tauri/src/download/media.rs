use futures_util::StreamExt;
use reqwest::Client;
use url::Url;

use crate::download::{Ctx, Report};
use crate::error::{AppError, AppResult};
use crate::models::{FileInfo, JobState};
use crate::security::filename::{sanitize_extension, sanitize_stem, unique_path};
use crate::sources::MediaKind;

/// Hard limit for a single media download.
pub const MAX_MEDIA_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Download a media file straight from the link.
pub async fn download(
    client: &Client,
    url: &Url,
    kind: MediaKind,
    ctx: &Ctx,
    report: Report<'_>,
) -> AppResult<Vec<FileInfo>> {
    ctx.ensure_allowed(url)?;
    let response = client.get(url.clone()).send().await?;
    let status = response.status();
    if !status.is_success() {
        return Err(http_error(status.as_u16()));
    }

    let mime = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    if !kind.accepts(&mime) {
        if mime.starts_with("text/html") {
            return Err(AppError::UnsupportedSource(
                "This link does not point at a media file. Try Video, Audio or Text.".into(),
            ));
        }
        return Err(AppError::InvalidMedia(format!(
            "expected {} but the server sent `{}`",
            kind.target().label(),
            if mime.is_empty() {
                "no content type"
            } else {
                &mime
            }
        )));
    }

    let total = response.content_length();
    if let Some(len) = total
        && len > MAX_MEDIA_BYTES
    {
        return Err(AppError::InvalidMedia(
            "the file is too large to download".into(),
        ));
    }

    let extension = pick_extension(&mime, url, kind);
    let stem = pick_stem(response.headers(), url);
    let path = unique_path(&ctx.dest, &stem, extension.as_deref());
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| stem.clone());

    report(JobState::Downloading, None, Some(name.clone()));

    let mut file = tokio::fs::File::create(&path).await?;
    let mut stream = response.bytes_stream();
    let mut written: u64 = 0;

    while let Some(chunk) = stream.next().await {
        if ctx.is_cancelled() {
            tokio::io::AsyncWriteExt::flush(&mut file).await.ok();
            drop(file);
            tokio::fs::remove_file(&path).await.ok();
            return Err(AppError::Cancelled);
        }
        let chunk = chunk?;
        written += chunk.len() as u64;
        if written > MAX_MEDIA_BYTES {
            drop(file);
            tokio::fs::remove_file(&path).await.ok();
            return Err(AppError::InvalidMedia(
                "the file is too large to download".into(),
            ));
        }
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
            .await
            .map_err(crate::security::net::map_io_error)?;
        if let Some(len) = total.filter(|l| *l > 0) {
            report(
                JobState::Downloading,
                Some((written as f64 / len as f64 * 100.0).clamp(0.0, 100.0)),
                Some(name.clone()),
            );
        }
    }
    tokio::io::AsyncWriteExt::flush(&mut file).await?;

    Ok(vec![FileInfo {
        size: Some(written),
        name,
        path: path.to_string_lossy().to_string(),
    }])
}

/// Map an HTTP status to a message that makes sense for the user.
pub fn http_error(code: u16) -> AppError {
    match code {
        401 | 403 => AppError::PermissionDenied(
            "the server refused access to this link (it may require a login)".into(),
        ),
        404 | 410 => AppError::UnsupportedSource("the link no longer exists".into()),
        429 => AppError::Network("the server is rate limiting requests".into()),
        other => AppError::Http(other),
    }
}

/// Extension taken from the response type, falling back to the URL.
fn pick_extension(mime: &str, url: &Url, kind: MediaKind) -> Option<String> {
    let base = mime
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let from_mime = match base.as_str() {
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/webp" => Some("webp"),
        "image/avif" => Some("avif"),
        "image/heic" => Some("heic"),
        "image/svg+xml" => Some("svg"),
        "image/gif" => Some("gif"),
        _ => None,
    };
    if let Some(ext) = from_mime {
        return sanitize_extension(ext);
    }
    let from_url = url
        .path_segments()
        .and_then(|mut s| s.next_back())
        .and_then(|s| s.rsplit_once('.').map(|(_, ext)| ext.to_string()))
        .and_then(|e| sanitize_extension(&e));
    match kind {
        MediaKind::Gif => Some("gif".into()),
        MediaKind::Image => from_url.or_else(|| Some("jpg".into())),
    }
}

/// Best effort file name for the download.
fn pick_stem(headers: &reqwest::header::HeaderMap, url: &Url) -> String {
    if let Some(name) = headers
        .get(reqwest::header::CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok())
        .and_then(parse_content_disposition)
    {
        return sanitize_stem(stem_of(&name));
    }
    url.path_segments()
        .and_then(|mut s| s.next_back())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .map(|s| sanitize_stem(stem_of(&s)))
        .unwrap_or_else(|| "download".to_string())
}

/// Strip the extension from a file name.
fn stem_of(name: &str) -> &str {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.contains('/') => stem,
        _ => name,
    }
}

/// Extract `filename` / `filename*` from a Content-Disposition header.
pub fn parse_content_disposition(value: &str) -> Option<String> {
    for part in value.split(';') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix("filename*=") {
            let rest = rest.trim_matches('"');
            // RFC 5987: charset'language'value
            let encoded = rest.rsplit_once('\'').map(|(_, v)| v).unwrap_or(rest);
            return Some(percent_decode(encoded));
        }
        if let Some(rest) = part.strip_prefix("filename=") {
            return Some(rest.trim().trim_matches('"').to_string());
        }
    }
    None
}

fn percent_decode(input: &str) -> String {
    percent_encoding::percent_decode_str(input)
        .decode_utf8_lossy()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_content_disposition() {
        assert_eq!(
            parse_content_disposition("attachment; filename=\"photo.png\""),
            Some("photo.png".to_string())
        );
        assert_eq!(
            parse_content_disposition("attachment; filename*=UTF-8''my%20photo.png"),
            Some("my photo.png".to_string())
        );
        assert_eq!(parse_content_disposition("inline"), None);
    }

    #[test]
    fn extension_prefers_mime_type() {
        let url = Url::parse("https://example.com/file.bin").unwrap();
        assert_eq!(
            pick_extension("image/png", &url, MediaKind::Image),
            Some("png".to_string())
        );
        assert_eq!(
            pick_extension("image/gif", &url, MediaKind::Gif),
            Some("gif".to_string())
        );
    }

    #[test]
    fn gif_target_always_keeps_gif_extension() {
        let url = Url::parse("https://example.com/clip").unwrap();
        assert_eq!(
            pick_extension("", &url, MediaKind::Gif),
            Some("gif".to_string())
        );
    }

    #[test]
    fn http_errors_have_meaningful_messages() {
        assert_eq!(http_error(404).code(), "unsupported_source");
        assert_eq!(http_error(403).code(), "permission_denied");
        assert_eq!(http_error(500).code(), "http");
    }
}
