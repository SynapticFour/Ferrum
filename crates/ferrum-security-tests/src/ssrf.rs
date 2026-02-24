//! A10: SSRF is_private_ip and SafeHttpClient behavior.

use ferrum_core::is_private_ip;
use std::net::IpAddr;

#[test]
fn private_ip_loopback() {
    assert!(is_private_ip(&"127.0.0.1".parse::<IpAddr>().unwrap()));
    assert!(is_private_ip(&"::1".parse::<IpAddr>().unwrap()));
}

#[test]
fn private_ip_rfc1918() {
    assert!(is_private_ip(&"10.0.0.1".parse::<IpAddr>().unwrap()));
    assert!(is_private_ip(&"192.168.1.1".parse::<IpAddr>().unwrap()));
    assert!(is_private_ip(&"172.16.0.1".parse::<IpAddr>().unwrap()));
}

#[test]
fn public_ip_allowed() {
    assert!(!is_private_ip(&"8.8.8.8".parse::<IpAddr>().unwrap()));
}
