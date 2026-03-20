use crate::config::FerrumConfig;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::watch;
use tokio::task::JoinHandle;

/// Minimal config file watcher for "hot reload".
///
/// Learned from production: reload config on file change, validate by parsing,
/// and never publish an invalid config to the application.
pub struct ConfigWatcher;

impl ConfigWatcher {
    /// Spawn a watcher for `path` and return a `watch` receiver with the latest
    /// successfully parsed `FerrumConfig`.
    ///
    /// If the config file cannot be parsed after a change, we keep the previous
    /// config and only log.
    pub fn spawn(path: PathBuf) -> (watch::Receiver<Arc<FerrumConfig>>, JoinHandle<()>) {
        let initial = FerrumConfig::load_from_path(&path)
            .expect("initial config load must succeed for ConfigWatcher");
        let (tx, rx) = watch::channel(Arc::new(initial));

        // Use `spawn_blocking` because the notify event receiver is blocking.
        let handle = tokio::task::spawn_blocking(move || {
            // std channel for notify callback -> async loop.
            let (tx_events, rx_events) = std::sync::mpsc::channel::<notify::Result<Event>>();

            let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res| {
                // Callback must return `()`. We intentionally ignore send errors because
                // they only happen during shutdown.
                let _ = tx_events.send(res);
            })
            .expect("failed to create notify watcher");

            // Watch the parent directory so editors that "atomic replace" the file still trigger events.
            let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
            if let Err(e) = watcher.watch(parent, RecursiveMode::NonRecursive) {
                tracing::error!(error = %e, path = ?parent, "failed to watch config directory");
                return;
            }

            let mut last_sent = std::time::Instant::now();
            loop {
                // Block in this async task via recv() is okay because this watcher only needs to
                // process infrequent events; if you expect high frequency, move recv to spawn_blocking.
                let res = rx_events.recv();
                let Ok(ev_res) = res else {
                    // Sender dropped -> shutdown.
                    break;
                };
                let Ok(ev) = ev_res else {
                    continue;
                };

                if !Self::event_matters(&path, &ev) {
                    continue;
                }

                // Simple debounce to avoid double-sending on some editors.
                if last_sent.elapsed() < std::time::Duration::from_millis(200) {
                    continue;
                }

                if let Ok(new_cfg) = FerrumConfig::load_from_path(&path) {
                    last_sent = std::time::Instant::now();
                    let _ = tx.send(Arc::new(new_cfg));
                } else {
                    tracing::warn!(path = ?path, "config reload failed; keeping previous config");
                }
            }
        });

        (rx, handle)
    }

    fn event_matters(path: &PathBuf, ev: &Event) -> bool {
        match &ev.kind {
            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {}
            _ => return false,
        }

        // notify may report events for multiple files; filter by exact path match.
        let Some(ev_paths) = Some(&ev.paths) else {
            return false;
        };
        ev_paths.iter().any(|p| p == path)
    }
}

