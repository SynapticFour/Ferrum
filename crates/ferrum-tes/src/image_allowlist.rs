// SPDX-License-Identifier: BUSL-1.1
//! Optional TES Docker image digest allowlist (`FERRUM_TES_ALLOWED_IMAGES`).
//!
//! When the env var is set (non-empty), only listed image references may run.
//! Prefer digest-pinned refs (`name@sha256:…`). Unset → no image restriction
//! (operator must not expose that mode on a public demo with shared Passports).

use crate::error::{Result, TesError};

/// Parse `FERRUM_TES_ALLOWED_IMAGES` (comma-separated). Empty / unset → `None` (unrestricted).
pub fn allowed_images_from_env() -> Option<Vec<String>> {
    let raw = std::env::var("FERRUM_TES_ALLOWED_IMAGES").ok()?;
    let list: Vec<String> = raw
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if list.is_empty() {
        None
    } else {
        Some(list)
    }
}

fn digest_of(image: &str) -> Option<&str> {
    image
        .rsplit_once("@sha256:")
        .map(|(_, d)| d)
        .filter(|d| !d.is_empty())
}

fn strip_registry_library(image: &str) -> &str {
    let s = image
        .strip_prefix("docker.io/library/")
        .or_else(|| image.strip_prefix("docker.io/"))
        .or_else(|| image.strip_prefix("library/"))
        .unwrap_or(image);
    s
}

/// True if `image` matches an allowlist entry (exact, digest, or normalized name).
pub fn image_is_allowed(image: &str, allowlist: &[String]) -> bool {
    let image = image.trim();
    if image.is_empty() {
        return false;
    }
    let img_digest = digest_of(image);
    let img_norm = strip_registry_library(image);
    for entry in allowlist {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        if entry == image || entry == img_norm {
            return true;
        }
        let ent_norm = strip_registry_library(entry);
        if ent_norm == img_norm || ent_norm == image {
            return true;
        }
        if let (Some(id), Some(ed)) = (img_digest, digest_of(entry)) {
            if id.eq_ignore_ascii_case(ed) {
                return true;
            }
        }
    }
    false
}

/// Fail closed when allowlist is configured and `image` is not listed.
pub fn ensure_image_allowed(image: &str) -> Result<()> {
    let Some(list) = allowed_images_from_env() else {
        return Ok(());
    };
    if image_is_allowed(image, &list) {
        return Ok(());
    }
    Err(TesError::Forbidden(format!(
        "TES image not on FERRUM_TES_ALLOWED_IMAGES allowlist: {image}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_match_ignores_registry_prefix() {
        let allow = vec![
            "docker.io/library/alpine:3.20.3@sha256:1e42bbe2508154c9126d48c2b8a75420c3544343bf86fd041fb7527e017a4b4a"
                .into(),
        ];
        assert!(image_is_allowed(
            "alpine:3.20.3@sha256:1e42bbe2508154c9126d48c2b8a75420c3544343bf86fd041fb7527e017a4b4a",
            &allow
        ));
        assert!(!image_is_allowed("alpine:3.20.3", &allow));
        assert!(!image_is_allowed(
            "alpine:3.20.3@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            &allow
        ));
    }
}
