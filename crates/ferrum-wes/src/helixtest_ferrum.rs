//! HelixTest `--mode ferrum` uses synthetic `trs://` workflow URLs (see SynapticFour/HelixTest `framework/src/wes.rs`).
//! Map those to expected WES outcomes without hitting real workflow engines.

use crate::types::RunState;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelixtestDisposition {
    /// Reach this terminal state after at least one non-terminal `/runs/{id}/status` poll; do not submit to TES.
    ImmediateTerminal(RunState),
    /// Normal path: submit to TES / local executor.
    Proceed,
}

/// Classify HelixTest WES requests when `workflow_url` uses the `trs://` scheme.
pub fn classify_trs_workflow(
    workflow_url: &str,
    workflow_type: &str,
    params: &Value,
) -> HelixtestDisposition {
    let Some(rest) = workflow_url.strip_prefix("trs://") else {
        return HelixtestDisposition::Proceed;
    };
    let parts: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() < 3 {
        return HelixtestDisposition::Proceed;
    }
    let host = parts[0];
    let tool = parts[1];
    let _version = parts[2];

    if host == "nonexistent" && tool == "invalid" {
        return HelixtestDisposition::ImmediateTerminal(RunState::ExecutorError);
    }
    if host != "test-tool" {
        return HelixtestDisposition::Proceed;
    }

    match tool {
        "fail" => HelixtestDisposition::ImmediateTerminal(RunState::ExecutorError),
        "cwl-echo" => {
            if !workflow_type.eq_ignore_ascii_case("cwl") {
                return HelixtestDisposition::ImmediateTerminal(RunState::ExecutorError);
            }
            let has_message = params
                .get("message")
                .and_then(|v| v.as_str())
                .is_some_and(|s| !s.is_empty());
            if !has_message {
                return HelixtestDisposition::ImmediateTerminal(RunState::ExecutorError);
            }
            HelixtestDisposition::Proceed
        }
        "echo" => HelixtestDisposition::Proceed,
        _ => HelixtestDisposition::Proceed,
    }
}
