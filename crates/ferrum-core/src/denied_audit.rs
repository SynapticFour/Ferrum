// SPDX-License-Identifier: BUSL-1.1
//! `authorization_denied` residency events (ADR-032).
//!
//! Appends denied/anonymous authorization failures to the residency hash chain without
//! storing tokens or raw `Authorization` headers.
//!
//! Event encoding (the residency table has no dedicated columns for these fields):
//! - `event_type` = `authorization_denied`
//! - `drs_id` = `reason=<reason>[;aud=<audience>]` (not a DRS object id)
//! - `requester` = token subject when a token parsed far enough, else `None`
//! - `destination` = minimized client address (see [`minimize_client_ip`]), else `None`
//! - `data_left_node` = `false`
//!
//! Client addresses are taken from `X-Forwarded-For` / `X-Real-IP` when present; those headers
//! are client-controlled unless a trusted proxy overwrites them, so treat them as hints.

use crate::config::AuditConfig;
use crate::residency::ResidencyAuditLog;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Residency `event_type` for denied authorization attempts.
pub const EVENT_AUTHORIZATION_DENIED: &str = "authorization_denied";

/// Why authorization was denied (ADR-032).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeniedReason {
    MissingBearer,
    InvalidToken,
    WrongAudience,
    Forbidden,
}

impl DeniedReason {
    pub fn as_str(self) -> &'static str {
        match self {
            DeniedReason::MissingBearer => "missing_bearer",
            DeniedReason::InvalidToken => "invalid_token",
            DeniedReason::WrongAudience => "wrong_audience",
            DeniedReason::Forbidden => "forbidden",
        }
    }
}

/// How to record the client address on denied events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClientIpMode {
    /// Do not record an address (default; also used for unknown config values).
    #[default]
    Omit,
    /// IPv4 `/24` (last octet zeroed), IPv6 `/32` (first two hextets kept).
    Truncated,
    /// Full parsed address.
    Full,
}

impl ClientIpMode {
    /// Parse `omit` | `truncated` | `full` (case-insensitive). Unknown values fall back to
    /// [`ClientIpMode::Omit`] (privacy-preserving, fail-closed).
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "truncated" => ClientIpMode::Truncated,
            "full" => ClientIpMode::Full,
            _ => ClientIpMode::Omit,
        }
    }
}

/// Minimize a client address for the audit chain.
///
/// Returns `None` for [`ClientIpMode::Omit`] and for unparseable input (never records a
/// string we could not validate as an IP address).
pub fn minimize_client_ip(ip: &str, mode: ClientIpMode) -> Option<String> {
    if mode == ClientIpMode::Omit {
        return None;
    }
    let addr: IpAddr = ip.trim().parse().ok()?;
    match mode {
        ClientIpMode::Omit => None,
        ClientIpMode::Full => Some(addr.to_string()),
        ClientIpMode::Truncated => Some(match addr {
            IpAddr::V4(v4) => {
                let o = v4.octets();
                format!("{}.{}.{}.0/24", o[0], o[1], o[2])
            }
            IpAddr::V6(v6) => {
                let s = v6.segments();
                format!("{:x}:{:x}::/32", s[0], s[1])
            }
        }),
    }
}

/// Fixed-window (one minute) process-wide limiter for denied-event appends.
#[derive(Debug)]
pub struct DeniedRateLimiter {
    limit: u32,
    window: Duration,
    state: Mutex<(Option<Instant>, u32)>,
}

impl DeniedRateLimiter {
    /// `limit` appends per minute; `0` disables denied logging entirely.
    pub fn new(limit: u32) -> Self {
        Self::with_window(limit, Duration::from_secs(60))
    }

    pub fn with_window(limit: u32, window: Duration) -> Self {
        Self {
            limit,
            window,
            state: Mutex::new((None, 0)),
        }
    }

    pub fn try_acquire(&self) -> bool {
        self.try_acquire_at(Instant::now())
    }

    /// Time-injected variant for tests.
    pub fn try_acquire_at(&self, now: Instant) -> bool {
        if self.limit == 0 {
            return false;
        }
        let mut guard = match self.state.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let in_window = guard
            .0
            .is_some_and(|s| now.saturating_duration_since(s) < self.window);
        if !in_window {
            *guard = (Some(now), 0);
        }
        if guard.1 >= self.limit {
            return false;
        }
        guard.1 += 1;
        true
    }
}

fn sanitize_field(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() && *c != ';')
        .take(128)
        .collect()
}

/// Append one `authorization_denied` event. No rate limiting here; see [`DeniedAuditSink`].
pub async fn record_authorization_denied(
    audit: &ResidencyAuditLog,
    reason: DeniedReason,
    subject: Option<&str>,
    audience: Option<&str>,
    client_ip_minimized: Option<&str>,
) {
    let mut detail = format!("reason={}", reason.as_str());
    if let Some(aud) = audience.map(sanitize_field).filter(|a| !a.is_empty()) {
        detail.push_str(";aud=");
        detail.push_str(&aud);
    }
    let subject = subject.map(sanitize_field).filter(|s| !s.is_empty());
    audit
        .append_warn(
            EVENT_AUTHORIZATION_DENIED,
            Some(&detail),
            subject.as_deref(),
            client_ip_minimized,
            false,
            None,
        )
        .await;
}

/// Rate-limited, config-aware writer used by the auth middleware.
pub struct DeniedAuditSink {
    audit: Arc<ResidencyAuditLog>,
    limiter: DeniedRateLimiter,
    ip_mode: ClientIpMode,
}

impl DeniedAuditSink {
    pub fn new(audit: Arc<ResidencyAuditLog>, cfg: &AuditConfig) -> Self {
        Self {
            audit,
            limiter: DeniedRateLimiter::new(cfg.denied_events_per_minute),
            ip_mode: ClientIpMode::parse(&cfg.anonymous_client_ip),
        }
    }

    /// Record a denied event inline (awaits the append). Returns false when rate limited.
    pub async fn record(
        &self,
        reason: DeniedReason,
        subject: Option<&str>,
        audience: Option<&str>,
        raw_client_ip: Option<&str>,
    ) -> bool {
        if !self.limiter.try_acquire() {
            return false;
        }
        let ip = raw_client_ip.and_then(|ip| minimize_client_ip(ip, self.ip_mode));
        record_authorization_denied(&self.audit, reason, subject, audience, ip.as_deref()).await;
        true
    }

    /// Fire-and-forget variant so a denied response is not delayed by the audit write.
    pub fn record_spawn(
        self: &Arc<Self>,
        reason: DeniedReason,
        subject: Option<String>,
        audience: Option<String>,
        raw_client_ip: Option<String>,
    ) {
        if !self.limiter.try_acquire() {
            return;
        }
        let this = Arc::clone(self);
        let ip = raw_client_ip.and_then(|ip| minimize_client_ip(&ip, this.ip_mode));
        tokio::spawn(async move {
            record_authorization_denied(
                &this.audit,
                reason,
                subject.as_deref(),
                audience.as_deref(),
                ip.as_deref(),
            )
            .await;
        });
    }
}

impl std::fmt::Debug for DeniedAuditSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeniedAuditSink")
            .field("ip_mode", &self.ip_mode)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimize_omit_returns_none() {
        assert_eq!(minimize_client_ip("203.0.113.9", ClientIpMode::Omit), None);
    }

    #[test]
    fn minimize_truncated_ipv4_zeroes_last_octet() {
        assert_eq!(
            minimize_client_ip("203.0.113.9", ClientIpMode::Truncated).as_deref(),
            Some("203.0.113.0/24")
        );
    }

    #[test]
    fn minimize_truncated_ipv6_keeps_32_bits() {
        assert_eq!(
            minimize_client_ip("2001:db8:abcd:12::1", ClientIpMode::Truncated).as_deref(),
            Some("2001:db8::/32")
        );
    }

    #[test]
    fn minimize_full_keeps_address() {
        assert_eq!(
            minimize_client_ip("203.0.113.9", ClientIpMode::Full).as_deref(),
            Some("203.0.113.9")
        );
    }

    #[test]
    fn minimize_rejects_non_ip_strings() {
        assert_eq!(minimize_client_ip("not-an-ip", ClientIpMode::Full), None);
        assert_eq!(
            minimize_client_ip("Bearer secret", ClientIpMode::Truncated),
            None
        );
    }

    #[test]
    fn mode_parse_unknown_is_omit() {
        assert_eq!(ClientIpMode::parse("truncated"), ClientIpMode::Truncated);
        assert_eq!(ClientIpMode::parse("FULL"), ClientIpMode::Full);
        assert_eq!(ClientIpMode::parse("omit"), ClientIpMode::Omit);
        assert_eq!(ClientIpMode::parse("bogus"), ClientIpMode::Omit);
    }

    #[test]
    fn rate_limiter_caps_per_window_and_resets() {
        let rl = DeniedRateLimiter::with_window(2, Duration::from_secs(60));
        let t0 = Instant::now();
        assert!(rl.try_acquire_at(t0));
        assert!(rl.try_acquire_at(t0));
        assert!(!rl.try_acquire_at(t0 + Duration::from_secs(10)));
        assert!(rl.try_acquire_at(t0 + Duration::from_secs(61)));
    }

    #[test]
    fn rate_limiter_zero_disables() {
        let rl = DeniedRateLimiter::new(0);
        assert!(!rl.try_acquire());
    }

    #[test]
    fn reason_strings_are_stable() {
        assert_eq!(DeniedReason::InvalidToken.as_str(), "invalid_token");
        assert_eq!(DeniedReason::MissingBearer.as_str(), "missing_bearer");
        assert_eq!(DeniedReason::WrongAudience.as_str(), "wrong_audience");
        assert_eq!(DeniedReason::Forbidden.as_str(), "forbidden");
    }
}
