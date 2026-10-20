//! Backend tests that run against mocked HTTP responses.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use httpmock::prelude::*;

use crate::download::Ctx;
use crate::download::media::download;
use crate::download::text::extract;
use crate::error::AppError;
use crate::models::JobState;
use crate::security::net::build_client;
use crate::sources::MediaKind;

fn ctx(dest: std::path::PathBuf, cancel: Arc<AtomicBool>) -> Ctx {
    Ctx {
        dest,
        cancel,
        allow_private: true,
    }
}

fn client() -> reqwest::Client {
    build_client(std::time::Duration::from_secs(30)).unwrap()
}

fn url_of(server: &MockServer, path: &str) -> url::Url {
    url::Url::parse(&format!("http://{}{path}", server.address())).unwrap()
}

#[tokio::test]
async fn downloads_an_image_to_disk() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/photo.png");
        then.status(200)
            .header("content-type", "image/png")
            .body(vec![7u8; 2048]);
    });

    let dir = tempfile::tempdir().unwrap();
    let files = download(
        &client(),
        &url_of(&server, "/photo.png"),
        MediaKind::Image,
        &ctx(dir.path().to_path_buf(), Arc::new(AtomicBool::new(false))),
        &|_, _, _| {},
    )
    .await
    .unwrap();

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "photo.png");
    assert_eq!(files[0].size, Some(2048));
    assert_eq!(
        std::fs::read(dir.path().join("photo.png")).unwrap().len(),
        2048
    );
}

#[tokio::test]
async fn rejects_a_page_when_an_image_was_requested() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/page");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body("<html><body>hi</body></html>");
    });

    let dir = tempfile::tempdir().unwrap();
    let err = download(
        &client(),
        &url_of(&server, "/page"),
        MediaKind::Image,
        &ctx(dir.path().to_path_buf(), Arc::new(AtomicBool::new(false))),
        &|_, _, _| {},
    )
    .await
    .unwrap_err();

    assert_eq!(err.code(), "unsupported_source");
    assert!(dir.path().read_dir().unwrap().next().is_none());
}

#[tokio::test]
async fn maps_http_errors() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/missing");
        then.status(404);
    });

    let dir = tempfile::tempdir().unwrap();
    let err = download(
        &client(),
        &url_of(&server, "/missing"),
        MediaKind::Image,
        &ctx(dir.path().to_path_buf(), Arc::new(AtomicBool::new(false))),
        &|_, _, _| {},
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "unsupported_source");
}

#[tokio::test]
async fn does_not_follow_redirects_to_private_addresses() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/redirect");
        then.status(302)
            .header("location", "http://10.0.0.5/secret");
    });

    let dir = tempfile::tempdir().unwrap();
    let err = download(
        &client(),
        &url_of(&server, "/redirect"),
        MediaKind::Image,
        &ctx(dir.path().to_path_buf(), Arc::new(AtomicBool::new(false))),
        &|_, _, _| {},
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "http");
}

#[tokio::test]
async fn cancelled_downloads_stop_and_clean_up() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/big");
        then.status(200)
            .header("content-type", "image/png")
            .body(vec![1u8; 100_000]);
    });

    let dir = tempfile::tempdir().unwrap();
    let cancel = Arc::new(AtomicBool::new(true));
    let err = download(
        &client(),
        &url_of(&server, "/big"),
        MediaKind::Image,
        &ctx(dir.path().to_path_buf(), cancel),
        &|_, _, _| {},
    )
    .await
    .unwrap_err();

    assert_eq!(err.code(), "cancelled");
    assert!(dir.path().read_dir().unwrap().next().is_none());
}

#[tokio::test]
async fn extracts_page_text_into_a_txt_file() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/post");
        then.status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(r#"<html><head><title>Quiet mornings</title></head><body><article>
              <p>The first paragraph of the article explains how the morning light falls across the room.</p>
              <p>A second paragraph continues the story with a few more sentences about the quiet house.</p>
              <p>A third paragraph wraps the article up before the day begins in earnest.</p>
            </article></body></html>"#);
    });

    let dir = tempfile::tempdir().unwrap();
    let files = extract(
        &client(),
        &url_of(&server, "/post"),
        &ctx(dir.path().to_path_buf(), Arc::new(AtomicBool::new(false))),
    )
    .await
    .unwrap();

    assert_eq!(files[0].name, "Quiet-mornings.txt");
    let text = std::fs::read_to_string(dir.path().join("Quiet-mornings.txt")).unwrap();
    assert!(text.contains("Quiet mornings"));
    assert!(text.contains("morning light"));
    assert!(text.contains("Source:"));
}

#[tokio::test]
async fn refuses_non_pages_for_text_extraction() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/file.bin");
        then.status(200)
            .header("content-type", "application/octet-stream")
            .body("binary");
    });

    let dir = tempfile::tempdir().unwrap();
    let err = extract(
        &client(),
        &url_of(&server, "/file.bin"),
        &ctx(dir.path().to_path_buf(), Arc::new(AtomicBool::new(false))),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "unsupported_source");
}

#[tokio::test]
async fn reports_progress_when_the_size_is_known() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/pic.png");
        then.status(200)
            .header("content-type", "image/png")
            .body(vec![3u8; 1000]);
    });

    let dir = tempfile::tempdir().unwrap();
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    let last = Arc::new(std::sync::Mutex::new(0.0));
    let result = download(
        &client(),
        &url_of(&server, "/pic.png"),
        MediaKind::Image,
        &ctx(dir.path().to_path_buf(), Arc::new(AtomicBool::new(false))),
        &move |state: JobState, percent: Option<f64>, _| {
            assert_eq!(state, JobState::Downloading);
            if let Some(p) = percent {
                let mut guard = last.lock().unwrap();
                assert!(p >= *guard);
                *guard = p;
            }
            sink.lock().unwrap().push(percent);
        },
    )
    .await
    .unwrap();

    assert!(result[0].size.unwrap() > 0);
    assert!(seen.lock().unwrap().iter().any(|p| p.is_some()));
}

#[tokio::test]
async fn private_networks_are_rejected_before_any_request() {
    let dir = tempfile::tempdir().unwrap();
    let url = url::Url::parse("http://192.168.1.10/secret.png").unwrap();
    let guarded = Ctx {
        dest: dir.path().to_path_buf(),
        cancel: Arc::new(AtomicBool::new(false)),
        allow_private: false,
    };
    let err = download(&client(), &url, MediaKind::Image, &guarded, &|_, _, _| {})
        .await
        .unwrap_err();
    assert_eq!(err.code(), "invalid_url");
}

#[tokio::test]
async fn error_messages_are_human_readable() {
    let err = AppError::MissingYtDlp;
    assert!(err.to_string().contains("yt-dlp"));
    assert_eq!(err.code(), "missing_ytdlp");
}
