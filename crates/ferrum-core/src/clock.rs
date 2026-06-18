//! System clock integrity probe for field Edge nodes (NTP skew warning).

use chrono::{DateTime, Utc};
use serde::Serialize;
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

pub const DEFAULT_NTP_HOST: &str = "pool.ntp.org";
pub const DEFAULT_MAX_SKEW_SECS: i64 = 300;

#[derive(Debug, Clone, Serialize)]
pub struct ClockStatus {
    pub system_time_utc: String,
    pub ntp_reachable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_time_utc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skew_seconds: Option<i64>,
    pub warn_skew: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Probe NTP (best-effort) and compare to system clock.
pub fn clock_status(ntp_host: &str, max_skew_secs: i64) -> ClockStatus {
    let now = Utc::now();
    let system_time_utc = now.to_rfc3339();

    match query_ntp_time(ntp_host) {
        Some(ref_time) => {
            let skew = (now - ref_time).num_seconds().abs();
            let warn = skew > max_skew_secs;
            ClockStatus {
                system_time_utc,
                ntp_reachable: true,
                reference_time_utc: Some(ref_time.to_rfc3339()),
                skew_seconds: Some(skew),
                warn_skew: warn,
                note: if warn {
                    Some(format!(
                        "clock skew {skew}s exceeds threshold {max_skew_secs}s; residency audit timestamps may be unreliable"
                    ))
                } else {
                    None
                },
            }
        }
        None => ClockStatus {
            system_time_utc,
            ntp_reachable: false,
            reference_time_utc: None,
            skew_seconds: None,
            warn_skew: false,
            note: Some(
                "NTP unreachable (offline); verify system clock manually before exporting audit chains"
                    .into(),
            ),
        },
    }
}

fn query_ntp_time(host: &str) -> Option<DateTime<Utc>> {
    let addr: SocketAddr = format!("{host}:123").parse().ok()?;
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    socket
        .set_write_timeout(Some(Duration::from_secs(2)))
        .ok()?;

    let mut packet = [0u8; 48];
    packet[0] = 0x1B; // LI=0, VN=3, Mode=3 (client)

    socket.send_to(&packet, addr).ok()?;
    let mut buf = [0u8; 48];
    let (n, _) = socket.recv_from(&mut buf).ok()?;
    if n < 48 {
        return None;
    }

    let seconds = u32::from_be_bytes([buf[40], buf[41], buf[42], buf[43]]);
    ntp_seconds_to_datetime(seconds)
}

/// Convert NTP era seconds (1900-based) to UTC.
fn ntp_seconds_to_datetime(ntp_secs: u32) -> Option<DateTime<Utc>> {
    const NTP_UNIX_OFFSET: i64 = 2_208_988_800;
    let unix = i64::from(ntp_secs).checked_sub(NTP_UNIX_OFFSET)?;
    DateTime::from_timestamp(unix, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_clock_status_has_note() {
        let status = clock_status("127.0.0.1:9", DEFAULT_MAX_SKEW_SECS);
        assert!(!status.ntp_reachable);
        assert!(status.note.is_some());
    }
}
