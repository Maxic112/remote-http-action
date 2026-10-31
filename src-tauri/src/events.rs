//! Events pushed from the core to the UI.

use serde::Serialize;

use crate::model::Run;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppEvent {
    /// Stored data changed (possibly from CLI or HTTP); the UI should refetch.
    DataChanged {
        entity: &'static str,
    },
    RunUpdated {
        run: Box<Run>,
    },
    /// A flow with a start delay is waiting; the UI shows a countdown with a cancel button.
    RunDelayed {
        run_id: String,
        flow_name: String,
        seconds: u64,
        cancelable: bool,
    },
    ServerStatusChanged,
    /// Update check result or download progress changed.
    UpdateChanged,
}

pub trait EventSink: Send + Sync {
    fn emit(&self, event: AppEvent);
}

/// Sink that drops all events (headless use and tests).
pub struct NoopSink;

impl EventSink for NoopSink {
    fn emit(&self, _event: AppEvent) {}
}

#[cfg(test)]
pub mod testing {
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    pub struct RecordingSink(pub Mutex<Vec<AppEvent>>);

    impl EventSink for RecordingSink {
        fn emit(&self, event: AppEvent) {
            self.0.lock().unwrap().push(event);
        }
    }
}
