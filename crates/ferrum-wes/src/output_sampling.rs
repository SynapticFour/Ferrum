//! Output sampling after run completion.
//!
//! Learned from Sapporo: after a run reaches a terminal success state, expose a
//! deterministic list of output files (best-effort) and ignore volatile artifacts
//! like `state.json`, logs, and temp files.

use serde_json::Value;
use std::path::Path;
/// Collect output file manifests for `work_dir` (recursive).
///
/// Returned value is a JSON array of objects containing:
/// - `file_id`: stable identifier used by RO-Crate export
/// - `location`: `file://...` URL
/// - `name`: base name
/// - `size`: bytes
pub fn collect_output_files(work_dir: &Path, ignore_globs: &[String]) -> Vec<Value> {
    let Some(work_dir) = work_dir.canonicalize().ok() else {
        return vec![];
    };
    let mut out = Vec::<Value>::new();
    let mut stack = vec![work_dir.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(read_dir) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in read_dir.flatten() {
            let path = entry.path();
            let file_name = entry
                .file_name()
                .to_str()
                .map(|s| s.to_string())
                .unwrap_or_default();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

            if should_ignore_name(is_dir, &file_name, ignore_globs) {
                continue;
            }

            if is_dir {
                stack.push(path);
                continue;
            }

            // Only include regular files.
            if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                continue;
            }

            let rel = path.strip_prefix(&work_dir).unwrap_or(&path);
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            let file_id = rel_str.replace('/', "__");
            let name = file_name.clone();

            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let location = format!("file://{}", path.display());

            out.push(serde_json::json!({
                "file_id": file_id,
                "location": location,
                "name": name,
                "size": size,
                "category": classify_output(&rel_str, &name),
                "path": rel_str,
            }));
        }
    }

    // Stable ordering for deterministic API/RO-Crate output.
    out.sort_by(|a, b| {
        let fa = a
            .get("file_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let fb = b
            .get("file_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        fa.cmp(fb)
    });

    out
}

/// Split collected files into result / artifact / log buckets for WES `outputs` JSON.
pub fn partition_output_files(files: Vec<Value>) -> (Vec<Value>, Vec<Value>, Vec<Value>) {
    let mut results = Vec::new();
    let mut artifacts = Vec::new();
    let mut logs = Vec::new();
    for f in files {
        match f.get("category").and_then(|c| c.as_str()) {
            Some("log") => logs.push(f),
            Some("artifact") => artifacts.push(f),
            _ => results.push(f),
        }
    }
    (results, artifacts, logs)
}

/// Build ignore globs for output sampling.
///
/// We always ignore:
/// - `state.json`, `stdout.txt`, `stderr.txt`
/// - `*.log`, common temp patterns (`*.tmp`, `*.temp`, `*~`, `*.swp`)
///
/// Additionally, if MultiQC is configured, we reuse its `*.log`-style globs
/// (this keeps behaviour aligned with existing service configuration).
pub fn default_ignore_globs(multiqc_scan_patterns: Option<&[String]>) -> Vec<String> {
    let mut out = vec![
        "state.json".to_string(),
        "stdout.txt".to_string(),
        "stderr.txt".to_string(),
        "*.log".to_string(),
        "*.tmp".to_string(),
        "*.temp".to_string(),
        "*~".to_string(),
        "*.swp".to_string(),
    ];

    if let Some(patterns) = multiqc_scan_patterns {
        // Only reuse patterns that look like logs; do not widen ignore beyond spec.
        for p in patterns {
            let pl = p.to_ascii_lowercase();
            if pl.contains(".log") || pl == "*.log" {
                out.push(p.clone());
            }
        }
    }

    out.sort();
    out.dedup();
    out
}

/// Classify a workdir file for the results UI: `result`, `artifact`, or `log`.
pub fn classify_output(rel_path: &str, file_name: &str) -> &'static str {
    if file_name == "stdout.txt"
        || file_name == "stderr.txt"
        || file_name.ends_with(".log")
        || file_name.starts_with(".command.")
        || file_name == ".exitcode"
    {
        return "log";
    }
    if rel_path.starts_with(".nextflow/")
        || rel_path.starts_with(".snakemake/")
        || file_name == "workflow.nf"
        || file_name == "nextflow.config"
        || file_name == "workflow.cwl"
        || file_name == "Snakefile"
        || file_name == "inputs.json"
        || file_name == "params.json"
        || file_name == "state.json"
        || file_name.starts_with("MANIFEST-")
        || matches!(file_name, "CURRENT" | "LOCK" | "history")
        || file_name.starts_with("index.")
    {
        return "artifact";
    }
    if rel_path.starts_with("work/") && !is_likely_result_name(file_name) {
        return "artifact";
    }
    "result"
}

fn is_likely_result_name(file_name: &str) -> bool {
    let n = file_name.to_lowercase();
    n.ends_with(".txt")
        || n.ends_with(".vcf")
        || n.ends_with(".bam")
        || n.ends_with(".cram")
        || n.ends_with(".gz")
        || n.ends_with(".html")
        || n.ends_with(".pdf")
        || n.ends_with(".tsv")
        || n.ends_with(".csv")
        || n.ends_with(".json")
        || n == "sorted.txt"
        || n == "hello.txt"
}

fn should_ignore_name(is_dir: bool, file_name: &str, ignore_globs: &[String]) -> bool {
    if file_name.is_empty() {
        return true;
    }

    // Explicit always-ignore file names.
    match file_name {
        "state.json" | "stdout.txt" | "stderr.txt" => return true,
        _ => {}
    }

    for pat in ignore_globs {
        if pat.is_empty() {
            continue;
        }
        // Handle directory patterns like "qualimap_report/".
        if pat.ends_with('/') {
            let base = pat.trim_end_matches('/');
            if is_dir && base == file_name {
                return true;
            }
            continue;
        }
        // Our glob implementation only supports single path segment matching.
        if glob_match_single(pat, file_name) {
            return true;
        }
    }

    // Extra temp directory guard (cheap and improves correctness).
    if is_dir && (file_name == "tmp" || file_name.starts_with("tmp_")) {
        return true;
    }

    false
}

/// Simple glob where `*` and `?` match within a single path segment (no '/').
fn glob_match_single(pattern: &str, name: &str) -> bool {
    if pattern.contains('/') {
        return false;
    }
    let pb = pattern.as_bytes();
    let nb = name.as_bytes();

    let mut pi = 0usize;
    let mut ni = 0usize;
    while pi < pb.len() {
        match pb[pi] {
            b'*' => {
                // Collapse repeated '*'.
                pi += 1;
                while pi < pb.len() && pb[pi] == b'*' {
                    pi += 1;
                }
                if pi >= pb.len() {
                    return true;
                }
                let next = pb[pi];
                while ni < nb.len() && nb[ni] != next {
                    ni += 1;
                }
                if ni >= nb.len() {
                    return false;
                }
            }
            b'?' => {
                if ni >= nb.len() {
                    return false;
                }
                pi += 1;
                ni += 1;
            }
            c => {
                if ni >= nb.len() || nb[ni] != c {
                    return false;
                }
                pi += 1;
                ni += 1;
            }
        }
    }
    ni >= nb.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_glob_match_single_basic() {
        assert!(glob_match_single("*.log", "x.log"));
        assert!(!glob_match_single("*.log", "x.txt"));
        assert!(glob_match_single("file?.txt", "file1.txt"));
        assert!(!glob_match_single("file?.txt", "file12.txt"));
    }

    #[test]
    fn test_collect_output_files_ignore_state_logs_and_temp() {
        let base = std::env::temp_dir().join(format!(
            "wes-output-sampling-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let _ = fs::create_dir_all(&base);
        fs::write(base.join("state.json"), b"{}").unwrap();
        fs::write(base.join("stdout.txt"), b"out").unwrap();
        fs::write(base.join("stderr.txt"), b"err").unwrap();
        fs::write(base.join("a.txt"), b"a").unwrap();
        fs::write(base.join("b.log"), b"log").unwrap();
        fs::write(base.join("c.tmp"), b"tmp").unwrap();
        fs::write(base.join("d~"), b"~").unwrap();

        let sub = base.join("sub");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("e.txt"), b"e").unwrap();

        let qualimap = base.join("qualimap_report");
        fs::create_dir_all(&qualimap).unwrap();
        fs::write(qualimap.join("q.html"), b"q").unwrap();

        let ignore_globs = vec![
            "state.json".to_string(),
            "stdout.txt".to_string(),
            "stderr.txt".to_string(),
            "*.log".to_string(),
            "*.tmp".to_string(),
            "*.temp".to_string(),
            "*~".to_string(),
            "qualimap_report/".to_string(),
        ];

        let files = collect_output_files(&base, &ignore_globs);
        let names: Vec<String> = files
            .iter()
            .filter_map(|v| {
                v.get("name")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string())
            })
            .collect();

        assert!(names.contains(&"a.txt".to_string()));
        assert!(names.contains(&"e.txt".to_string()));
        assert!(!names.contains(&"state.json".to_string()));
        assert!(!names.contains(&"b.log".to_string()));
        assert!(!names.contains(&"c.tmp".to_string()));
        assert!(!names.contains(&"d~".to_string()));
        assert!(!names.contains(&"q.html".to_string()));

        let _ = fs::remove_dir_all(&base);
    }
}
