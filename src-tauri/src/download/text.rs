use reqwest::Client;
use url::Url;

use crate::download::Ctx;
use crate::error::{AppError, AppResult};
use crate::models::FileInfo;
use crate::security::filename::{sanitize_stem, unique_path};

/// Maximum size of a page we are willing to parse.
const MAX_PAGE_BYTES: usize = 8 * 1024 * 1024;

/// Minimum amount of readable text before we trust the extraction.
const MIN_TEXT_CHARS: usize = 200;

/// Fetch a page and save its readable text.
pub async fn extract(client: &Client, url: &Url, ctx: &Ctx) -> AppResult<Vec<FileInfo>> {
    ctx.ensure_allowed(url)?;
    let response = client.get(url.clone()).send().await?;
    let status = response.status();
    if !status.is_success() {
        return Err(crate::download::media::http_error(status.as_u16()));
    }

    let mime = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();

    if !mime.is_empty() && !mime.starts_with("text/html") && !mime.contains("xhtml") {
        return Err(AppError::UnsupportedSource(
            "This link does not point at a web page, so no text can be extracted.".into(),
        ));
    }

    let charset = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.split(';').skip(1).find_map(|p| {
                let (k, v) = p.split_once('=')?;
                if k.trim().eq_ignore_ascii_case("charset") {
                    Some(v.trim().trim_matches('"').to_ascii_lowercase())
                } else {
                    None
                }
            })
        })
        .unwrap_or_else(|| "utf-8".to_string());

    let body = response.bytes().await?;
    if body.len() > MAX_PAGE_BYTES {
        return Err(AppError::InvalidMedia(
            "the page is too large to read".into(),
        ));
    }
    let html = decode(&body, &charset);

    let (title, text) = extract_text(&html, url);
    let text = text.trim();
    if text.chars().count() < MIN_TEXT_CHARS {
        return Err(AppError::UnsupportedSource(
            "No readable text found. The page may need JavaScript or a login.".into(),
        ));
    }

    let heading = if title.trim().is_empty() {
        "Untitled".to_string()
    } else {
        title.trim().to_string()
    };
    let body = format!(
        "{heading}\n{}\n\nSource: {}\n\n{text}\n",
        "=".repeat(heading.chars().count().min(72)),
        url.as_str()
    );

    let path = unique_path(&ctx.dest, &sanitize_stem(&heading), Some("txt"));
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "page.txt".into());
    tokio::fs::write(&path, body).await?;

    let size = tokio::fs::metadata(&path).await.ok().map(|m| m.len());

    Ok(vec![FileInfo {
        path: path.to_string_lossy().to_string(),
        name,
        size,
    }])
}

/// Extract the article title and its readable text.
fn extract_text(html: &str, url: &Url) -> (String, String) {
    if let Ok(article) = readability::extractor::extract(&mut html.as_bytes(), url) {
        let text = article.text.trim().to_string();
        if text.chars().count() >= MIN_TEXT_CHARS {
            return (article.title, text);
        }
    }
    fallback_text(html)
}

/// Plain text extraction used when readability cannot find an article.
fn fallback_text(html: &str) -> (String, String) {
    use scraper::{Html, Selector};

    let document = Html::parse_document(html);
    let title_selector = Selector::parse("title").unwrap();
    let title = document
        .select(&title_selector)
        .next()
        .map(|t| t.text().collect::<String>().trim().to_string())
        .unwrap_or_default();

    let body_selector = Selector::parse("article, main, body").unwrap();
    let block_selector = Selector::parse("p, h1, h2, h3, li, pre, blockquote").unwrap();

    let root = document.select(&body_selector).next();
    let mut parts: Vec<String> = Vec::new();
    if let Some(root) = root {
        for node in root.select(&block_selector) {
            let text = node.text().collect::<Vec<_>>().join(" ");
            let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if !text.is_empty() {
                parts.push(text);
            }
        }
    }
    (title, parts.join("\n\n"))
}

/// Decode a page body, falling back to a lossy UTF-8 interpretation.
fn decode(bytes: &[u8], charset: &str) -> String {
    match charset {
        "utf-8" | "utf8" | "us-ascii" | "ascii" => String::from_utf8_lossy(bytes).to_string(),
        "iso-8859-1" | "latin1" | "windows-1252" | "cp1252" => {
            bytes.iter().map(|b| *b as char).collect()
        }
        _ => String::from_utf8_lossy(bytes).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r#"<html><head><title>Example article</title></head>
    <body><article><h1>Example article</h1>
    <p>This is the first paragraph of a reasonably long article body used for tests in this project.</p>
    <p>And here is a second paragraph with more words so that the extracted text passes the minimum length check.</p>
    <p>A third paragraph to be safe, because readability likes a bit of structure and enough words to work with.</p>
    </article></body></html>"#;

    #[test]
    fn readability_extracts_title_and_text() {
        let (title, text) = extract_text(PAGE, &Url::parse("https://example.com/post").unwrap());
        assert_eq!(title, "Example article");
        assert!(text.contains("first paragraph"));
    }

    #[test]
    fn fallback_extracts_paragraphs() {
        let (title, text) = fallback_text(PAGE);
        assert_eq!(title, "Example article");
        assert!(text.contains("second paragraph"));
    }

    #[test]
    fn javascript_only_page_yields_nothing() {
        let (title, text) = fallback_text("<html><body><div id=app></div></body></html>");
        assert!(text.is_empty());
        assert!(title.is_empty());
    }
}
