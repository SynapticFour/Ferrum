// Re-run when migrations change.
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
