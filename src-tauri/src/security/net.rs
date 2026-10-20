use std::io;
use std::sync::Arc;
use std::time::Duration;

use reqwest::redirect::{Attempt, Policy};
use reqwest::{Client, Url};

use crate::security::is_blocked_host;

/// Resolver that refuses to hand out addresses inside the local network.
///
/// This is what protects against SSRF and DNS rebinding: the check happens on
/// the address that is actually dialled, not on the hostname that was typed.
#[derive(Debug, Clone, Default)]
pub struct GuardedResolver;

impl reqwest::dns::Resolve for GuardedResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        Box::pin(async move {
            let host = name.as_str().to_string();
            if is_blocked_host(&host) {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "refusing to resolve a local or private host",
                ))
                    as Box<dyn std::error::Error + Send + Sync>);
            }
            let mut addrs: Vec<std::net::SocketAddr> =
                tokio::net::lookup_host((host.as_str(), 0u16))
                    .await?
                    .collect();
            addrs.retain(|a| !crate::security::is_blocked_ip(&a.ip()));
            if addrs.is_empty() {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "host resolved only to private addresses",
                ))
                    as Box<dyn std::error::Error + Send + Sync>);
            }
            Ok(Box::new(addrs.into_iter())
                as Box<dyn Iterator<Item = std::net::SocketAddr> + Send>)
        })
    }
}

/// Decide whether a redirect may be followed.
fn redirect_allowed(url: &Url, previous: usize, max: usize) -> bool {
    if previous >= max {
        return false;
    }
    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }
    !matches!(url.host_str(), Some(host) if is_blocked_host(host))
}

/// Redirect policy: HTTPS/HTTP only, bounded count, no local destinations.
pub fn redirect_policy(max: usize) -> Policy {
    Policy::custom(move |attempt: Attempt| {
        if redirect_allowed(attempt.url(), attempt.previous().len(), max) {
            attempt.follow()
        } else {
            attempt.stop()
        }
    })
}

/// HTTP client used for every remote fetch made by the app.
pub fn build_client(timeout: Duration) -> reqwest::Result<Client> {
    Client::builder()
        .dns_resolver(Arc::new(GuardedResolver))
        .redirect(redirect_policy(5))
        .connect_timeout(Duration::from_secs(15))
        .timeout(timeout)
        .user_agent(concat!("linkFetch/", env!("CARGO_PKG_VERSION")))
        .build()
}

/// ENOSPC as reported by the OS.
pub const ENOSPC: i32 = 28;

/// Translate an IO error into a user facing one (notably "disk full").
pub fn map_io_error(e: io::Error) -> crate::error::AppError {
    if e.raw_os_error() == Some(ENOSPC) {
        return crate::error::AppError::NoSpace;
    }
    e.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redirects_are_bounded_and_safe() {
        let public = Url::parse("https://example.com/a").unwrap();
        let local = Url::parse("http://127.0.0.1:8080/a").unwrap();
        let ftp = Url::parse("ftp://example.com/a").unwrap();
        assert!(redirect_allowed(&public, 0, 5));
        assert!(!redirect_allowed(&public, 5, 5));
        assert!(!redirect_allowed(&local, 0, 5));
        assert!(!redirect_allowed(&ftp, 0, 5));
    }
}
