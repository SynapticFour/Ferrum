// SPDX-License-Identifier: BUSL-1.1

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn default_path() -> PathBuf {
    PathBuf::from("docs/openapi/ferrum.openapi.json")
}

fn write_spec(path: &Path, json: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    fs::write(path, json)
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let check = args.iter().any(|a| a == "--check");
    let path = args
        .iter()
        .find(|a| !a.starts_with('-'))
        .map(PathBuf::from)
        .unwrap_or_else(default_path);

    let json = ferrum_openapi::ferrum_openapi_json();

    if check {
        let existing = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("cannot read {}: {e}", path.display());
                return ExitCode::from(2);
            }
        };
        if existing == json {
            println!("OpenAPI contract matches {}", path.display());
            return ExitCode::SUCCESS;
        }
        eprintln!(
            "{} is stale. Regenerate with: cargo run -p ferrum-openapi -- {}",
            path.display(),
            path.display()
        );
        return ExitCode::from(1);
    }

    if let Err(e) = write_spec(&path, &json) {
        eprintln!("cannot write {}: {e}", path.display());
        return ExitCode::from(2);
    }
    let _ = writeln!(io::stderr(), "wrote {}", path.display());
    ExitCode::SUCCESS
}
