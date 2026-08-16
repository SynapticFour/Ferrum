// SPDX-License-Identifier: BUSL-1.1
//! Interactive and non-interactive `ferrum meta init`.

use ferrum_meta_connect::{
    build_init_template, submission_to_yaml, validate_submission, InitParams, MetaProfile,
};
use std::io::{self, Write};
use std::path::Path;

pub fn run_meta_init(
    profile: MetaProfile,
    output: &Path,
    params: InitParams,
    interactive: bool,
) -> Result<(), String> {
    let mut params = params;
    if interactive {
        prompt_init_params(profile, &mut params)?;
    }
    let doc = build_init_template(profile, &params);
    let report = validate_submission(&doc, Some(profile));
    if !report.valid {
        return Err(format!(
            "generated submission failed validation ({} errors)",
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
        "Wrote {} submission to {}",
        profile.as_str(),
        output.display()
    );
    Ok(())
}

fn prompt_init_params(profile: MetaProfile, params: &mut InitParams) -> Result<(), String> {
    let stdin = io::stdin();
    let mut line = String::new();

    print!(
        "Study title [{}]: ",
        params.study_title.as_deref().unwrap_or("")
    );
    io::stdout().flush().map_err(|e| e.to_string())?;
    line.clear();
    stdin.read_line(&mut line).map_err(|e| e.to_string())?;
    if !line.trim().is_empty() {
        params.study_title = Some(line.trim().to_string());
    }

    print!(
        "Sample alias [{}]: ",
        params.sample_alias.as_deref().unwrap_or("sample001")
    );
    io::stdout().flush().map_err(|e| e.to_string())?;
    line.clear();
    stdin.read_line(&mut line).map_err(|e| e.to_string())?;
    if !line.trim().is_empty() {
        params.sample_alias = Some(line.trim().to_string());
    }

    match profile {
        MetaProfile::Pathogen => {
            print!(
                "Collection site [{}]: ",
                params.collection_site.as_deref().unwrap_or("field_site")
            );
            io::stdout().flush().map_err(|e| e.to_string())?;
            line.clear();
            stdin.read_line(&mut line).map_err(|e| e.to_string())?;
            if !line.trim().is_empty() {
                params.collection_site = Some(line.trim().to_string());
            }
            print!(
                "Pathogen organism [{}]: ",
                params.pathogen_organism.as_deref().unwrap_or("unknown")
            );
            io::stdout().flush().map_err(|e| e.to_string())?;
            line.clear();
            stdin.read_line(&mut line).map_err(|e| e.to_string())?;
            if !line.trim().is_empty() {
                params.pathogen_organism = Some(line.trim().to_string());
            }
        }
        MetaProfile::H3Africa => {
            print!(
                "Country [{}]: ",
                params.country.as_deref().unwrap_or("Kenya")
            );
            io::stdout().flush().map_err(|e| e.to_string())?;
            line.clear();
            stdin.read_line(&mut line).map_err(|e| e.to_string())?;
            if !line.trim().is_empty() {
                params.country = Some(line.trim().to_string());
            }
            print!(
                "Consent type [{}]: ",
                params.consent_type.as_deref().unwrap_or("H3AFRICA_BROAD")
            );
            io::stdout().flush().map_err(|e| e.to_string())?;
            line.clear();
            stdin.read_line(&mut line).map_err(|e| e.to_string())?;
            if !line.trim().is_empty() {
                params.consent_type = Some(line.trim().to_string());
            }
        }
        MetaProfile::Core => {}
    }

    Ok(())
}
