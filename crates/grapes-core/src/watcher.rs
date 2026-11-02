use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::error::{GrapesError, Result};
use crate::types::ReconcileReport;
use crate::workspace::Workspace;

type EventCallback = dyn Fn(std::result::Result<ReconcileReport, String>) + Send + Sync;

pub struct WorkspaceWatcher {
    _watcher: RecommendedWatcher,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl WorkspaceWatcher {
    pub fn start<F>(workspace: Arc<Mutex<Workspace>>, callback: F) -> Result<Self>
    where
        F: Fn(std::result::Result<ReconcileReport, String>) + Send + Sync + 'static,
    {
        let root = workspace
            .lock()
            .map_err(|_| GrapesError::State("工作区锁已损坏".into()))?
            .root()
            .to_owned();
        let sidecar = root.join(".grapes");
        let (sender, receiver) = mpsc::channel::<()>();
        let watched_root = root.clone();
        let watched_sidecar = sidecar.clone();

        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                let Ok(event) = event else {
                    let _ = sender.send(());
                    return;
                };
                if event
                    .paths
                    .iter()
                    .any(|path| relevant_path(path, &watched_root, &watched_sidecar))
                {
                    let _ = sender.send(());
                }
            })?;
        watcher.watch(&root, RecursiveMode::Recursive)?;

        let stop = Arc::new(AtomicBool::new(false));
        let stop_for_thread = stop.clone();
        let callback: Arc<EventCallback> = Arc::new(callback);
        let thread = thread::spawn(move || {
            while !stop_for_thread.load(Ordering::Relaxed) {
                match receiver.recv_timeout(Duration::from_millis(200)) {
                    Ok(()) => {
                        // Debounce editor/copy bursts while preserving the last event.
                        while receiver.recv_timeout(Duration::from_millis(350)).is_ok() {}

                        let result = workspace
                            .lock()
                            .map_err(|_| "工作区锁已损坏".to_owned())
                            .and_then(|mut workspace| {
                                workspace.reconcile().map_err(|error| error.to_string())
                            });
                        callback(result);
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });

        Ok(Self {
            _watcher: watcher,
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for WorkspaceWatcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn relevant_path(path: &Path, root: &Path, sidecar: &Path) -> bool {
    if path.starts_with(sidecar) {
        return false;
    }
    if path == root {
        return true;
    }

    // Some platforms occasionally report relative paths from the watch root.
    let absolute = if path.is_absolute() {
        PathBuf::from(path)
    } else {
        root.join(path)
    };
    !absolute.starts_with(sidecar)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;

    #[test]
    fn watcher_reconciles_external_rename_and_keeps_asset_id() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("before.jpg"), b"stable photo").expect("photo");
        let workspace = Workspace::open_and_reconcile(temp.path()).expect("workspace");
        let original_id = workspace.snapshot().expect("snapshot").assets[0].id.clone();
        let workspace = Arc::new(Mutex::new(workspace));

        let (sender, receiver) = mpsc::channel();
        let _watcher = WorkspaceWatcher::start(workspace.clone(), move |result| {
            let _ = sender.send(result);
        })
        .expect("watcher");

        fs::rename(
            temp.path().join("before.jpg"),
            temp.path().join("after.jpg"),
        )
        .expect("external rename");

        let result = receiver
            .recv_timeout(Duration::from_secs(8))
            .expect("watcher report");
        assert!(result.is_ok(), "watcher failed: {result:?}");

        let snapshot = workspace
            .lock()
            .expect("lock")
            .snapshot()
            .expect("snapshot");
        assert_eq!(snapshot.assets.len(), 1);
        assert_eq!(snapshot.assets[0].id, original_id);
        assert_eq!(snapshot.assets[0].filename, "after.jpg");

        // Ensure repeated events and DB writes cannot create a reconcile loop.
        thread::sleep(Duration::from_millis(900));
        while receiver.try_recv().is_ok() {}
    }
}
