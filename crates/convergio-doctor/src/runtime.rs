//! DoctorRuntime — shared state injected after extension registration.
//!
//! Holds manifests and scheduled task info populated by main.rs
//! after all extensions are registered. Doctor checks read from this
//! at `/api/doctor` request time.

use convergio_types::manifest::Manifest;
use std::sync::{Arc, RwLock};

/// Scheduled task info: (extension_id, task_names).
pub type ScheduledTaskInfo = Vec<(String, Vec<String>)>;

#[derive(Default)]
struct Inner {
    manifests: Vec<Manifest>,
    scheduled_tasks: ScheduledTaskInfo,
}

/// Thread-safe runtime data for doctor checks.
///
/// Created once, cloned into DoctorExtension and into main.rs.
/// Main.rs populates it after extension registration; doctor checks
/// read it when a `/api/doctor` request arrives.
#[derive(Clone, Default)]
pub struct DoctorRuntime {
    inner: Arc<RwLock<Inner>>,
}

impl DoctorRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set extension manifests (called from main.rs after registration).
    pub fn set_manifests(&self, manifests: Vec<Manifest>) {
        if let Ok(mut guard) = self.inner.write() {
            guard.manifests = manifests;
        }
    }

    /// Set scheduled task declarations (called from main.rs).
    pub fn set_scheduled_tasks(&self, tasks: ScheduledTaskInfo) {
        if let Ok(mut guard) = self.inner.write() {
            guard.scheduled_tasks = tasks;
        }
    }

    /// Read manifests snapshot.
    pub fn manifests(&self) -> Vec<Manifest> {
        self.inner
            .read()
            .map(|g| g.manifests.clone())
            .unwrap_or_default()
    }

    /// Read scheduled tasks snapshot.
    pub fn scheduled_tasks(&self) -> ScheduledTaskInfo {
        self.inner
            .read()
            .map(|g| g.scheduled_tasks.clone())
            .unwrap_or_default()
    }
}
