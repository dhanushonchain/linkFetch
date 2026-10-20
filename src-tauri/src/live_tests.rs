//! Live network checks. They are ignored by default and only run on purpose:
//! `cargo test -- --ignored --nocapture`

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::download::Ctx;
use crate::download::media::download;
use crate::download::text::extract;
use crate::download::ytdlp;
use crate::models::Target;
use crate::security::net::build_client;
use crate::sources::MediaKind;

fn ctx(dest: std::path::PathBuf) -> Ctx {
    Ctx {
        dest,
        cancel: Arc::new(AtomicBool::new(false)),
        allow_private: false,
    }
}

#[tokio::test]
#[ignore = "needs network"]
async fn live_image_download() {
    let dir = tempfile::tempdir().unwrap();
    let url = url::Url::parse("https://www.python.org/static/img/python-logo.png").unwrap();
    let files = download(
        &build_client(std::time::Duration::from_secs(60)).unwrap(),
        &url,
        MediaKind::Image,
        &ctx(dir.path().to_path_buf()),
        &|_, _, _| {},
    )
    .await
    .unwrap();
    println!("image -> {:?}", files);
    assert!(files[0].size.unwrap() > 1000);
}

#[tokio::test]
#[ignore = "needs network"]
async fn live_text_extraction() {
    let dir = tempfile::tempdir().unwrap();
    let url = url::Url::parse("https://www.rfc-editor.org/rfc/rfc9110.html").unwrap();
    let files = extract(
        &build_client(std::time::Duration::from_secs(60)).unwrap(),
        &url,
        &ctx(dir.path().to_path_buf()),
    )
    .await
    .unwrap();
    println!("text -> {:?}", files);
    assert!(files[0].name.ends_with(".txt"));
}

#[tokio::test]
#[ignore = "needs network"]
async fn live_ytdlp_video() {
    let dir = tempfile::tempdir().unwrap();
    let url = url::Url::parse("https://download.samplelib.com/mp4/sample-5s.mp4").unwrap();
    let files = ytdlp::download(
        &url,
        Target::Video,
        &ctx(dir.path().to_path_buf()),
        &|_, _, _| {},
    )
    .await
    .unwrap();
    println!("video -> {:?}", files);
    assert!(files[0].size.unwrap() > 1000);
}

#[tokio::test]
#[ignore = "needs network"]
async fn live_ytdlp_audio() {
    let dir = tempfile::tempdir().unwrap();
    let url = url::Url::parse("https://download.samplelib.com/mp4/sample-5s.mp4").unwrap();
    let files = ytdlp::download(
        &url,
        Target::Audio,
        &ctx(dir.path().to_path_buf()),
        &|_, _, _| {},
    )
    .await
    .unwrap();
    println!("audio -> {:?}", files);
    assert!(files[0].name.ends_with(".mp3"));
}
