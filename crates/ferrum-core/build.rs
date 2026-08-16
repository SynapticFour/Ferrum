// SPDX-License-Identifier: BUSL-1.1
// Re-run when migrations change.
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
