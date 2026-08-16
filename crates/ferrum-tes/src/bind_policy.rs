// SPDX-License-Identifier: BUSL-1.1
//! TES Docker host binds from the task request are opt-in via an operator allowlist.
//!
//! Client-supplied `volumes` must not become arbitrary host mounts. Operator-controlled
//! binds (`FERRUM_TES_EXTRA_BINDS`, docker.sock, CLI path) are configured separately.

use crate::error::{Result, TesError};

/// Host-path prefixes that TES request `volumes` may bind (comma-separated env).
/// `FERRUM_WES_TES_WORK_HOST_PREFIX` is always included when set so the WES TES
/// backend can mount per-run work dirs without a second variable.
pub fn allowed_bind_prefixes() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(s) = std::env::var("FERRUM_TES_ALLOWED_BIND_PREFIXES") {
        for p in s.split(',') {
            let p = p.trim();
            if !p.is_empty() {
                out.push(p.to_string());
            }
        }
    }
    if let Ok(s) = std::env::var("FERRUM_WES_TES_WORK_HOST_PREFIX") {
        let p = s.trim();
        if !p.is_empty() && !out.iter().any(|x| x == p) {
            out.push(p.to_string());
        }
    }
    out
}

fn host_path_from_bind(bind: &str) -> Result<&str> {
    let bind = bind.trim();
    if bind.is_empty() {
        return Err(TesError::Validation("empty volume bind".into()));
    }
    let host = bind.split(':').next().unwrap_or("");
    if host.is_empty() {
        return Err(TesError::Validation("volume bind missing host path".into()));
    }
    if host.contains("..") {
        return Err(TesError::Validation(
            "volume host path must not contain '..'".into(),
        ));
    }
    Ok(host)
}

fn prefix_allows(host: &str, prefix: &str) -> bool {
    let prefix = prefix.trim_end_matches('/');
    host == prefix || host.starts_with(&format!("{prefix}/"))
}

/// Convert TES `volumes` JSON into Docker bind strings, or reject if not allowlisted.
pub fn request_volume_binds(
    volumes: &[serde_json::Value],
    allowed_prefixes: &[String],
) -> Result<Vec<String>> {
    if volumes.is_empty() {
        return Ok(Vec::new());
    }
    if allowed_prefixes.is_empty() {
        return Err(TesError::Validation(
            "TES request volumes are disabled unless FERRUM_TES_ALLOWED_BIND_PREFIXES \
             (or FERRUM_WES_TES_WORK_HOST_PREFIX) is set"
                .into(),
        ));
    }
    let mut binds = Vec::new();
    for v in volumes {
        let bind = if let Some(s) = v.as_str() {
            s.to_string()
        } else if let (Some(h), Some(c)) = (
            v.get("hostPath").and_then(|x| x.as_str()),
            v.get("containerPath").and_then(|x| x.as_str()),
        ) {
            format!("{h}:{c}")
        } else {
            return Err(TesError::Validation(
                "TES volume must be a bind string or {hostPath, containerPath}".into(),
            ));
        };
        let host = host_path_from_bind(&bind)?;
        if !allowed_prefixes.iter().any(|p| prefix_allows(host, p)) {
            return Err(TesError::Validation(format!(
                "TES volume host path {host:?} is not under an allowed bind prefix"
            )));
        }
        binds.push(bind);
    }
    Ok(binds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rejects_volumes_without_prefixes() {
        let err = request_volume_binds(&[json!("/etc:/etc")], &[]).unwrap_err();
        assert!(err.to_string().contains("disabled"));
    }

    #[test]
    fn rejects_path_outside_prefix() {
        let err = request_volume_binds(&[json!("/etc:/mnt")], &["/data/tes".into()]).unwrap_err();
        assert!(err.to_string().contains("not under"));
    }

    #[test]
    fn rejects_dotdot() {
        let err = request_volume_binds(&[json!("/data/tes/../etc:/mnt")], &["/data/tes".into()])
            .unwrap_err();
        assert!(err.to_string().contains(".."));
    }

    #[test]
    fn allows_prefix_and_nested() {
        let binds = request_volume_binds(
            &[
                json!("/data/tes:/mnt"),
                json!("/data/tes/run1:/work:rw"),
                json!({"hostPath": "/data/tes/a", "containerPath": "/b"}),
            ],
            &["/data/tes".into()],
        )
        .unwrap();
        assert_eq!(binds.len(), 3);
    }
}
