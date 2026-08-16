// SPDX-License-Identifier: BUSL-1.1
//! Generate demo node Crypt4GH keys (node.sec / node.pub). Used by ferrum-init.
use std::path::PathBuf;

fn main() {
    let dir: PathBuf = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/data/ferrum/keys"));
    let key_id = std::env::var("CRYPT4GH_MASTER_KEY_ID").unwrap_or_else(|_| "node".to_string());
    let sec = dir.join(format!("{key_id}.sec"));
    let pub_key = dir.join(format!("{key_id}.pub"));
    if sec.is_file() && pub_key.is_file() {
        eprintln!(
            "ferrum-node-keygen: keys already present at {}",
            dir.display()
        );
        return;
    }
    ferrum_crypt4gh::generate_keypair(&sec, &pub_key, None).unwrap_or_else(|e| {
        eprintln!("ferrum-node-keygen: failed: {e}");
        std::process::exit(1);
    });
    eprintln!(
        "ferrum-node-keygen: wrote {} and {}",
        sec.display(),
        pub_key.display()
    );
}
