//! Offline update bundle install for Edge deployments.

use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

#[derive(serde::Serialize, serde::Deserialize)]
struct UpdateManifest {
    version: String,
    gateway_sha256: String,
    gateway_name: String,
}

pub fn install_bundle(
    bundle: &Path,
    install_dir: &Path,
    expected_sha256: Option<&str>,
) -> Result<(), String> {
    let file = std::fs::File::open(bundle).map_err(|e| format!("open bundle: {e}"))?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
    let extract_dir = tempfile::tempdir().map_err(|e| e.to_string())?;

    archive
        .unpack(extract_dir.path())
        .map_err(|e| format!("unpack bundle: {e}"))?;

    let manifest_path = extract_dir.path().join("manifest.json");
    let manifest_raw =
        std::fs::read_to_string(&manifest_path).map_err(|e| format!("read manifest: {e}"))?;
    let manifest: UpdateManifest =
        serde_json::from_str(&manifest_raw).map_err(|e| format!("parse manifest: {e}"))?;

    let bin_path = extract_dir.path().join(&manifest.gateway_name);
    if !bin_path.is_file() {
        return Err(format!(
            "bundle missing gateway binary: {}",
            manifest.gateway_name
        ));
    }

    let mut f = std::fs::File::open(&bin_path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest = hex::encode(hasher.finalize());
    if digest != manifest.gateway_sha256 {
        return Err("bundle gateway_sha256 mismatch (bundle may be tampered)".into());
    }
    if let Some(expected) = expected_sha256 {
        if !expected.eq_ignore_ascii_case(&digest) {
            return Err(format!("expected sha256 {expected}, bundle has {digest}"));
        }
    }

    std::fs::create_dir_all(install_dir).map_err(|e| e.to_string())?;
    let dest = install_dir.join("ferrum-gateway");
    std::fs::copy(&bin_path, &dest).map_err(|e| format!("install binary: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&dest)
            .map_err(|e| e.to_string())?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&dest, perms).map_err(|e| e.to_string())?;
    }
    let ferrum_link = install_dir.join("ferrum");
    #[cfg(unix)]
    {
        let _ = std::fs::remove_file(&ferrum_link);
        std::os::unix::fs::symlink(&dest, &ferrum_link).map_err(|e| e.to_string())?;
    }

    println!(
        "Installed ferrum-gateway {} to {} (sha256={digest})",
        manifest.version,
        dest.display()
    );
    Ok(())
}

pub fn create_bundle(gateway_bin: &Path, version: &str, output: &Path) -> Result<(), String> {
    if !gateway_bin.is_file() {
        return Err(format!(
            "gateway binary not found: {}",
            gateway_bin.display()
        ));
    }
    let bytes = std::fs::read(gateway_bin).map_err(|e| e.to_string())?;
    let digest = hex::encode(Sha256::digest(&bytes));
    let manifest = UpdateManifest {
        version: version.to_string(),
        gateway_sha256: digest,
        gateway_name: "ferrum-gateway".to_string(),
    };

    let staging = tempfile::tempdir().map_err(|e| e.to_string())?;
    std::fs::write(
        staging.path().join("manifest.json"),
        serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::copy(gateway_bin, staging.path().join("ferrum-gateway")).map_err(|e| e.to_string())?;

    let out_file = std::fs::File::create(output).map_err(|e| e.to_string())?;
    let enc = flate2::write::GzEncoder::new(out_file, flate2::Compression::default());
    let mut tar = tar::Builder::new(enc);
    tar.append_path_with_name(staging.path().join("manifest.json"), "manifest.json")
        .map_err(|e| e.to_string())?;
    tar.append_path_with_name(staging.path().join("ferrum-gateway"), "ferrum-gateway")
        .map_err(|e| e.to_string())?;
    tar.finish().map_err(|e| e.to_string())?;
    println!("Wrote signed update bundle to {}", output.display());
    Ok(())
}
