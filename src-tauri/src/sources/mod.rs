use serde::Serialize;
use url::Url;

use crate::error::{AppError, AppResult};
use crate::models::Target;

/// A site we know about, so the UI can explain limitations before downloading.
pub struct SiteDef {
    name: &'static str,
    hosts: &'static [&'static str],
    /// Sites that do not expose their media as a plain file URL.
    no_direct_media: bool,
}

const KNOWN_SITES: &[SiteDef] = &[
    SiteDef {
        name: "YouTube",
        hosts: &["youtube.com", "youtu.be", "youtube-nocookie.com"],
        no_direct_media: true,
    },
    SiteDef {
        name: "Twitter / X",
        hosts: &["twitter.com", "x.com"],
        no_direct_media: true,
    },
    SiteDef {
        name: "Instagram",
        hosts: &["instagram.com"],
        no_direct_media: true,
    },
    SiteDef {
        name: "Facebook",
        hosts: &["facebook.com", "fb.watch", "fb.com"],
        no_direct_media: true,
    },
    SiteDef {
        name: "TikTok",
        hosts: &["tiktok.com"],
        no_direct_media: true,
    },
    SiteDef {
        name: "Reddit",
        hosts: &["reddit.com", "redd.it"],
        no_direct_media: true,
    },
    SiteDef {
        name: "LinkedIn",
        hosts: &["linkedin.com"],
        no_direct_media: true,
    },
    SiteDef {
        name: "Vimeo",
        hosts: &["vimeo.com"],
        no_direct_media: true,
    },
    SiteDef {
        name: "SoundCloud",
        hosts: &["soundcloud.com"],
        no_direct_media: true,
    },
    SiteDef {
        name: "Bandcamp",
        hosts: &["bandcamp.com"],
        no_direct_media: true,
    },
    SiteDef {
        name: "Twitch",
        hosts: &["twitch.tv"],
        no_direct_media: true,
    },
];

/// Media kind a direct download has to produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Image,
    Gif,
}

impl MediaKind {
    pub fn target(self) -> Target {
        match self {
            MediaKind::Image => Target::Image,
            MediaKind::Gif => Target::Gif,
        }
    }

    /// MIME types that are acceptable for this kind.
    pub fn accepts(self, mime: &str) -> bool {
        let mime = mime
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        match self {
            MediaKind::Image => mime.starts_with("image/") && mime != "image/gif",
            MediaKind::Gif => mime == "image/gif",
        }
    }
}

/// What the downloader will do with a link.
#[derive(Debug, Clone)]
pub enum Plan {
    /// Fetch the file itself.
    DirectMedia { url: Url, kind: MediaKind },
    /// Fetch the page and extract readable text.
    Text { url: Url },
    /// Hand the link to yt-dlp.
    YtDlp { url: Url, target: Target },
}

/// Known site for a host, if any.
pub fn known_site(host: &str) -> Option<&'static SiteDef> {
    let host = host.to_ascii_lowercase();
    KNOWN_SITES.iter().find(|site| {
        site.hosts
            .iter()
            .any(|h| host == *h || host.ends_with(&format!(".{h}")))
    })
}

/// Route a link to the strategy that can handle it.
pub fn route(url: &Url, target: Target) -> AppResult<Plan> {
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let site = known_site(&host);

    match target {
        Target::Video | Target::Audio => Ok(Plan::YtDlp {
            url: url.clone(),
            target,
        }),
        Target::Image | Target::Gif => {
            if let Some(site) = site
                && site.no_direct_media
            {
                return Err(AppError::UnsupportedTarget(format!(
                    "{} does not expose direct media files. Try Video, Audio or Text.",
                    site.name
                )));
            }
            let kind = if target == Target::Gif {
                MediaKind::Gif
            } else {
                MediaKind::Image
            };
            Ok(Plan::DirectMedia {
                url: url.clone(),
                kind,
            })
        }
        Target::Text => Ok(Plan::Text { url: url.clone() }),
    }
}

/// What the UI needs to know about a link before the user downloads.
#[derive(Debug, Clone, Serialize)]
pub struct SourceInfo {
    pub host: String,
    /// Friendly site name when we recognise it.
    pub site: Option<String>,
    pub supported_targets: Vec<Target>,
    /// Human readable reason when `target` is not in `supported_targets`.
    pub unsupported: Option<String>,
}

/// Describe a link: which site it belongs to and what can be done with it.
pub fn describe(url: &Url, target: Target) -> SourceInfo {
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let site = known_site(&host);

    let mut supported = vec![Target::Video, Target::Audio, Target::Text];
    let direct_media_ok = site.is_none_or(|s| !s.no_direct_media);
    if direct_media_ok {
        supported.push(Target::Image);
        supported.push(Target::Gif);
    }

    let unsupported = if supported.contains(&target) {
        None
    } else {
        Some(match site {
            Some(site) => format!(
                "{} does not expose direct media files. Try Video, Audio or Text.",
                site.name
            ),
            None => "This target is not supported for this link.".to_string(),
        })
    };

    SourceInfo {
        host: crate::security::domain_of(url),
        site: site.map(|s| s.name.to_string()),
        supported_targets: supported,
        unsupported,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::parse_and_normalize;

    fn url(raw: &str) -> Url {
        parse_and_normalize(raw).unwrap()
    }

    #[test]
    fn routes_video_to_ytdlp() {
        let plan = route(&url("https://youtu.be/abc"), Target::Video).unwrap();
        assert!(matches!(plan, Plan::YtDlp { .. }));
    }

    #[test]
    fn routes_image_on_generic_host_to_direct_media() {
        let plan = route(&url("https://example.com/a.png"), Target::Image).unwrap();
        assert!(matches!(
            plan,
            Plan::DirectMedia {
                kind: MediaKind::Image,
                ..
            }
        ));
    }

    #[test]
    fn rejects_image_for_social_sites() {
        let err = route(&url("https://instagram.com/p/x"), Target::Image).unwrap_err();
        assert_eq!(err.code(), "unsupported_target");
    }

    #[test]
    fn routes_text_everywhere() {
        let plan = route(&url("https://example.com/post"), Target::Text).unwrap();
        assert!(matches!(plan, Plan::Text { .. }));
    }

    #[test]
    fn describes_supported_targets() {
        let info = describe(&url("https://instagram.com/p/x"), Target::Image);
        assert!(!info.supported_targets.contains(&Target::Image));
        assert!(info.unsupported.unwrap().contains("Instagram"));
        assert!(info.supported_targets.contains(&Target::Video));
    }

    #[test]
    fn gif_kind_only_accepts_gif() {
        assert!(MediaKind::Gif.accepts("image/gif"));
        assert!(!MediaKind::Gif.accepts("image/png"));
        assert!(MediaKind::Image.accepts("image/png"));
        assert!(!MediaKind::Image.accepts("image/gif"));
    }
}
