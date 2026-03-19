//! Sample process CPU/memory for metrics (sysinfo, cross-platform).
//! Used by local executors to record_task_sample every 30s.

use crate::metrics::TaskSample;
use chrono::Utc;
use sysinfo::Pid;
use sysinfo::ProcessesToUpdate;
use sysinfo::System;

/// Sample current CPU % and memory (MB) for a process. Returns None if process not found or error.
/// cpu_pct is per-process (0–100 per core; can exceed 100 on multi-core).
pub fn sample_process(pid: u32) -> Option<TaskSample> {
    let mut sys = System::new_all();
    sys.refresh_processes(ProcessesToUpdate::All);
    let proc = sys.process(Pid::from_u32(pid))?;
    let cpu_pct = proc.cpu_usage() as f64;
    let memory_mb = (proc.memory() / (1024 * 1024)) as i64;
    Some(TaskSample {
        task_id: pid.to_string(),
        task_name: proc.name().to_string_lossy().into_owned(),
        cpu_pct,
        memory_mb,
        read_bytes: 0, // sysinfo doesn't expose I/O on all platforms
        write_bytes: 0,
        timestamp: Utc::now(),
    })
}

/// Build a TaskSample with fixed task_id/task_name (e.g. "main" for whole-run sampling).
pub fn sample_process_as_task(pid: u32, task_id: &str, task_name: &str) -> Option<TaskSample> {
    let mut sys = System::new_all();
    sys.refresh_processes(ProcessesToUpdate::All);
    let proc = sys.process(Pid::from_u32(pid))?;
    let cpu_pct = proc.cpu_usage() as f64;
    let memory_mb = (proc.memory() / (1024 * 1024)) as i64;
    Some(TaskSample {
        task_id: task_id.to_string(),
        task_name: task_name.to_string(),
        cpu_pct,
        memory_mb,
        read_bytes: 0,
        write_bytes: 0,
        timestamp: Utc::now(),
    })
}
