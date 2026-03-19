//! Live log streaming: broadcast channel per run and SSE.

use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::broadcast;
use tokio::sync::RwLock as AsyncRwLock;

/// One line of log output (stdout or stderr).
#[derive(Debug, Clone, Serialize)]
pub struct LogEvent {
    pub stream: String,
    pub data: String,
}

impl LogEvent {
    pub fn stdout(data: String) -> Self {
        Self {
            stream: "stdout".to_string(),
            data,
        }
    }
    pub fn stderr(data: String) -> Self {
        Self {
            stream: "stderr".to_string(),
            data,
        }
    }
}

/// Sink given to executors: send log lines and optional file write path.
pub struct LogSink {
    pub tx: broadcast::Sender<LogEvent>,
    pub work_dir: PathBuf,
}

/// Registry of live log streams by run_id. Create on submit, subscribe for SSE.
pub struct LogStreamRegistry {
    /// run_id -> (sender, work_dir for reference)
    streams: AsyncRwLock<std::collections::HashMap<String, (broadcast::Sender<LogEvent>, PathBuf)>>,
    capacity: usize,
}

impl LogStreamRegistry {
    pub fn new(capacity: usize) -> Self {
        Self {
            streams: AsyncRwLock::new(std::collections::HashMap::new()),
            capacity: capacity.max(16),
        }
    }

    /// Create a new log stream for a run. Returns the sink to pass to the executor and stores the sender for subscribe.
    pub async fn create(&self, run_id: &str, work_dir: PathBuf) -> Arc<LogSink> {
        let (tx, _rx) = broadcast::channel(self.capacity);
        self.streams
            .write()
            .await
            .insert(run_id.to_string(), (tx.clone(), work_dir.clone()));
        Arc::new(LogSink { tx, work_dir })
    }

    /// Subscribe to a run's log stream. Returns None if run is not streaming (e.g. not started or already finished).
    pub async fn subscribe(&self, run_id: &str) -> Option<broadcast::Receiver<LogEvent>> {
        self.streams
            .read()
            .await
            .get(run_id)
            .map(|(tx, _)| tx.subscribe())
    }

    /// Remove stream when run ends (terminal state). Call from RunManager.
    pub async fn remove(&self, run_id: &str) {
        self.streams.write().await.remove(run_id);
    }
}

/// Pipe stdout/stderr from a child process: write to work_dir/stdout.txt and stderr.txt, and send to the sink.
/// Spawns two background tasks; call after spawning the process.
pub fn pipe_child_logs(
    stdout: Option<tokio::process::ChildStdout>,
    stderr: Option<tokio::process::ChildStderr>,
    sink: Arc<LogSink>,
) {
    if let Some(s) = stdout {
        let sink = Arc::clone(&sink);
        tokio::spawn(async move {
            let mut reader = BufReader::new(s).lines();
            let path = sink.work_dir.join("stdout.txt");
            let _ = tokio::fs::File::create(&path).await;
            let mut file = match tokio::fs::File::create(&path).await {
                Ok(f) => f,
                Err(_) => return,
            };
            while let Ok(Some(line)) = reader.next_line().await {
                let line = line + "\n";
                let _ = tokio::io::AsyncWriteExt::write_all(&mut file, line.as_bytes()).await;
                let _ = sink.tx.send(LogEvent::stdout(line));
            }
        });
    }
    if let Some(s) = stderr {
        let sink = Arc::clone(&sink);
        tokio::spawn(async move {
            let mut reader = BufReader::new(s).lines();
            let path = sink.work_dir.join("stderr.txt");
            let _ = tokio::fs::File::create(&path).await;
            let mut file = match tokio::fs::File::create(&path).await {
                Ok(f) => f,
                Err(_) => return,
            };
            while let Ok(Some(line)) = reader.next_line().await {
                let line = line + "\n";
                let _ = tokio::io::AsyncWriteExt::write_all(&mut file, line.as_bytes()).await;
                let _ = sink.tx.send(LogEvent::stderr(line));
            }
        });
    }
}
