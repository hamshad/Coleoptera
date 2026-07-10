use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::watch;

/// Tracks in-flight crawls so the UI can request cancellation.
#[derive(Default)]
pub struct AppState {
    pub sessions: Mutex<HashMap<String, watch::Sender<bool>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    /// Register a new crawl session; returns the sender used to signal cancel.
    pub fn register(&self, id: String) -> watch::Sender<bool> {
        let (tx, _rx) = watch::channel(false);
        self.sessions.lock().unwrap().insert(id, tx.clone());
        tx
    }

    pub fn cancel(&self, id: &str) -> bool {
        if let Some(tx) = self.sessions.lock().unwrap().get(id) {
            let _ = tx.send(true);
            true
        } else {
            false
        }
    }

    pub fn finish(&self, id: &str) {
        self.sessions.lock().unwrap().remove(id);
    }
}
