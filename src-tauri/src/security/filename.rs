use std::path::{Path, PathBuf};

/// Turn an arbitrary string into a safe file stem.
///
/// Keeps letters, digits, dot, dash and underscore; everything else becomes a
/// dash. Leading dots and dashes are removed so the result can never be a
/// hidden file or a relative path fragment.
pub fn sanitize_stem(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len().min(80));
    let mut last_dash = false;
    for ch in raw.chars() {
        let keep = ch.is_ascii_alphanumeric() || ch == '.' || ch == '_';
        if keep {
            out.push(ch);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out
        .trim_matches(|c| c == '-' || c == '.' || c == '_')
        .to_string();
    if trimmed.is_empty() {
        return "download".to_string();
    }
    // Keep room for the extension and the " (n)" suffix.
    let mut trimmed = trimmed;
    if trimmed.len() > 100 {
        trimmed.truncate(100);
        trimmed = trimmed.trim_end_matches(['-', '.', '_']).to_string();
    }
    if trimmed.is_empty() {
        "download".to_string()
    } else {
        trimmed
    }
}

/// Normalise an extension: lowercase, no dot, alphanumeric only.
pub fn sanitize_extension(raw: &str) -> Option<String> {
    let ext = raw.trim().trim_start_matches('.').to_ascii_lowercase();
    if ext.is_empty() || ext.len() > 8 {
        return None;
    }
    if !ext.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    Some(ext)
}

/// Build a safe file name from a stem and an extension.
pub fn safe_filename(stem: &str, extension: Option<&str>) -> String {
    let stem = sanitize_stem(stem);
    match extension.and_then(sanitize_extension) {
        Some(ext) => format!("{stem}.{ext}"),
        None => stem,
    }
}

/// Pick a non colliding path inside `dir`.
///
/// The name is sanitized first, so the result can never escape `dir`.
pub fn unique_path(dir: &Path, stem: &str, extension: Option<&str>) -> PathBuf {
    let base = safe_filename(stem, extension);
    let (stem, ext) = match base.rsplit_once('.') {
        Some((s, e)) => (s.to_string(), Some(e.to_string())),
        None => (base.clone(), None),
    };
    let mut candidate = dir.join(&base);
    let mut counter = 1;
    while candidate.exists() {
        let name = match &ext {
            Some(e) => format!("{stem} ({counter}).{e}"),
            None => format!("{stem} ({counter})"),
        };
        candidate = dir.join(name);
        counter += 1;
    }
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_traversal_and_separators() {
        assert_eq!(sanitize_stem("../../etc/passwd"), "etc-passwd");
        assert_eq!(sanitize_stem("a/b\\c"), "a-b-c");
        assert_eq!(sanitize_stem(".hidden"), "hidden");
        assert_eq!(sanitize_stem("   "), "download");
    }

    #[test]
    fn keeps_readable_characters() {
        assert_eq!(
            sanitize_stem("Cool Song (Official Video)"),
            "Cool-Song-Official-Video"
        );
    }

    #[test]
    fn builds_filename_with_extension() {
        assert_eq!(safe_filename("photo", Some("png")), "photo.png");
        assert_eq!(safe_filename("photo", Some(".PNG")), "photo.png");
        assert_eq!(safe_filename("photo", Some("../exe")), "photo");
        assert_eq!(safe_filename("photo", None), "photo");
    }

    #[test]
    fn unique_path_avoids_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let first = unique_path(dir.path(), "clip", Some("mp4"));
        std::fs::write(&first, b"x").unwrap();
        let second = unique_path(dir.path(), "clip", Some("mp4"));
        assert_ne!(first, second);
        assert_eq!(second.file_name().unwrap(), "clip (1).mp4");
        assert!(second.starts_with(dir.path()));
    }
}
