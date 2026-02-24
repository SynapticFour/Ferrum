//! DRS URI resolver: drs://hostname/object_id format.

/// Parse hostname-based DRS URI: drs://hostname/object_id
pub fn parse_drs_uri(uri: &str) -> Option<(String, String)> {
    let uri = uri.trim();
    if !uri.starts_with("drs://") {
        return None;
    }
    let rest = uri.strip_prefix("drs://")?;
    let (host, id) = rest.split_once('/')?;
    Some((host.to_string(), id.to_string()))
}

/// Build hostname-based DRS URI.
pub fn build_drs_uri(hostname: &str, object_id: &str) -> String {
    format!("drs://{}/{}", hostname, object_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_parse() {
        let (h, id) = parse_drs_uri("drs://drs.example.org/314159").unwrap();
        assert_eq!(h, "drs.example.org");
        assert_eq!(id, "314159");
    }
}
