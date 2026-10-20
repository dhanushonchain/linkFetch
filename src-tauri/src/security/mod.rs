pub mod filename;
pub mod net;

use std::net::IpAddr;

use url::Url;

use crate::error::{AppError, AppResult};

/// Parse and normalise a user supplied link.
///
/// * whitespace is trimmed
/// * a missing scheme becomes `https://`
/// * only `http`/`https` are accepted
/// * fragments are dropped (they never reach the server)
pub fn parse_and_normalize(raw: &str) -> AppResult<Url> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidUrl("empty link".into()));
    }
    if trimmed.chars().any(|c| c.is_control()) {
        return Err(AppError::InvalidUrl(
            "link contains control characters".into(),
        ));
    }

    let with_scheme = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };

    let mut parsed =
        Url::parse(&with_scheme).map_err(|_| AppError::InvalidUrl(trimmed.to_string()))?;
    match parsed.scheme() {
        "http" | "https" => {}
        other => {
            return Err(AppError::InvalidUrl(format!(
                "unsupported scheme `{other}`"
            )));
        }
    }
    match parsed.host_str() {
        None => return Err(AppError::InvalidUrl("link has no host".into())),
        Some(host) => {
            // A public host always has a dot; bare names are not fetchable.
            if !host.contains('.') {
                return Err(AppError::InvalidUrl(format!(
                    "`{host}` is not a full domain"
                )));
            }
        }
    }
    parsed.set_fragment(None);
    Ok(parsed)
}

/// Host without a leading `www.`, lowercased.
pub fn domain_of(url: &Url) -> String {
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    host.strip_prefix("www.").unwrap_or(&host).to_string()
}

/// True when the host is a loopback / link-local / private address.
pub fn is_blocked_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_unspecified()
                || v4.is_documentation()
                || v4.octets()[0] == 0
                || v4.octets()[0] == 100 && (64..128).contains(&v4.octets()[1])
                || v4.octets()[0] >= 224
        }
        IpAddr::V6(v6) => {
            let seg = v6.segments();
            v6.is_loopback()
                || v6.is_unspecified()
                || (seg[0] & 0xfe00) == 0xfc00 // unique local
                || (seg[0] & 0xffc0) == 0xfe80 // link local
                || v6.to_ipv4_mapped().is_some_and(|v4| is_blocked_ip(&IpAddr::V4(v4)))
        }
    }
}

/// Reject links that point at the local machine or a private network.
///
/// The DNS resolver guards hostnames; this covers literal IP addresses and
/// anything that reaches the network layer without a lookup.
pub fn ensure_fetchable(url: &Url) -> AppResult<()> {
    match url.host_str() {
        Some(host) if is_blocked_host(host) => Err(AppError::InvalidUrl(
            "links to local or private networks are not allowed".into(),
        )),
        _ => Ok(()),
    }
}

/// Hostnames that always point back at the local machine or a private network.
pub fn is_blocked_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    if let Ok(ip) = host.parse::<IpAddr>() {
        return is_blocked_ip(&ip);
    }
    host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".internal")
        || host.ends_with(".home.arpa")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_and_normalizes() {
        let url = parse_and_normalize("  https://Example.com/a?b=1#frag ").unwrap();
        assert_eq!(url.as_str(), "https://example.com/a?b=1");
        assert_eq!(domain_of(&url), "example.com");
    }

    #[test]
    fn adds_missing_scheme() {
        let url = parse_and_normalize("example.com/x").unwrap();
        assert_eq!(url.scheme(), "https");
    }

    #[test]
    fn rejects_empty_and_bad_urls() {
        assert!(parse_and_normalize("   ").is_err());
        assert!(parse_and_normalize("ftp://example.com").is_err());
        assert!(parse_and_normalize("https://").is_err());
        assert!(parse_and_normalize("not a url").is_err());
    }

    #[test]
    fn blocks_private_addresses() {
        assert!(is_blocked_ip(&"127.0.0.1".parse().unwrap()));
        assert!(is_blocked_ip(&"10.1.2.3".parse().unwrap()));
        assert!(is_blocked_ip(&"192.168.0.5".parse().unwrap()));
        assert!(is_blocked_ip(&"169.254.10.5".parse().unwrap()));
        assert!(is_blocked_ip(&"::1".parse().unwrap()));
        assert!(!is_blocked_ip(&"93.184.216.34".parse().unwrap()));
    }

    #[test]
    fn rejects_local_links() {
        let local = Url::parse("http://127.0.0.1/x").unwrap();
        let host = Url::parse("http://localhost/x").unwrap();
        let public = Url::parse("https://example.com/x").unwrap();
        assert!(ensure_fetchable(&local).is_err());
        assert!(ensure_fetchable(&host).is_err());
        assert!(ensure_fetchable(&public).is_ok());
    }

    #[test]
    fn blocks_local_hosts() {
        assert!(is_blocked_host("localhost"));
        assert!(is_blocked_host("printer.local"));
        assert!(is_blocked_host("192.168.1.10"));
        assert!(!is_blocked_host("example.com"));
    }
}
