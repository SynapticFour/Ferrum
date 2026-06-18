//! `ferrum meta import` — CSV to ferrum-meta YAML.

use ferrum_meta_connect::{import_csv_to_submission, submission_to_yaml, MetaProfile};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

pub fn run_meta_import(profile: MetaProfile, csv: &Path, output: &Path) -> Result<(), String> {
    let file = File::open(csv).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    let (doc, report) = import_csv_to_submission(profile, reader)?;
    if !report.valid {
        return Err(format!(
            "import validation failed with {} errors",
            report.error_count()
        ));
    }
    let yaml = submission_to_yaml(&doc)?;
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    std::fs::write(output, yaml).map_err(|e| e.to_string())?;
    println!(
        "Imported {} submission from {} → {}",
        profile.as_str(),
        csv.display(),
        output.display()
    );
    Ok(())
}
