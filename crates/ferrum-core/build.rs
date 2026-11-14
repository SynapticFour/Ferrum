// SPDX-License-Identifier: Apache-2.0
// Re-run when migrations change.
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
