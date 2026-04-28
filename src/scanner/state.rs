//! Scanner state management.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Shared state for tracking scan progress across API requests.
///
/// This is designed to be shared across threads (wrapped in Arc) and
/// provides atomic operations for checking and updating scan status.
#[derive(Debug, Default)]
pub struct ScanState {
    /// Whether a scan is currently in progress.
    pub(crate) scanning: AtomicBool,
    /// Number of items scanned so far.
    pub(crate) count: AtomicU64,
    /// Total number of items to scan (0 if unknown/discovery phase).
    pub(crate) total: AtomicU64,
    /// Current scan phase.
    pub(crate) phase: std::sync::RwLock<ScanPhase>,
    /// Current folder being scanned (if any).
    pub(crate) current_folder: std::sync::RwLock<Option<String>>,
}

/// A handle to a shared scan state.
///
/// This is a cheap-to-clone handle that provides access to the scan state
/// without exposing the internal `Arc`.
#[derive(Debug, Clone)]
pub struct ScanStateHandle(Arc<ScanState>);

impl ScanStateHandle {
    /// Create a new handle wrapping the given scan state.
    #[must_use]
    pub fn new(state: ScanState) -> Self {
        Self(Arc::new(state))
    }

    /// Get a reference to the underlying scan state.
    #[must_use]
    pub fn get(&self) -> &ScanState {
        &self.0
    }

    /// Check if a scan is currently in progress.
    #[must_use]
    pub fn is_scanning(&self) -> bool {
        self.0.is_scanning()
    }

    /// Get the current item count.
    #[must_use]
    pub fn get_count(&self) -> u64 {
        self.0.get_count()
    }

    /// Get the total item count (0 if unknown).
    #[must_use]
    pub fn get_total(&self) -> u64 {
        self.0.get_total()
    }

    /// Get the current scan phase.
    #[must_use]
    pub fn get_phase(&self) -> ScanPhase {
        self.0.get_phase()
    }

    /// Get the current folder being scanned.
    #[must_use]
    pub fn get_current_folder(&self) -> Option<String> {
        self.0.get_current_folder()
    }

    /// Try to start a scan. Returns false if a scan is already in progress.
    #[must_use]
    pub fn try_start(&self) -> bool {
        self.0.try_start()
    }

    /// Mark the scan as complete.
    pub fn finish(&self) {
        self.0.finish();
    }

    /// Reset the count to 0.
    pub fn reset_count(&self) {
        self.0.reset_count();
    }

    /// Reset all progress state for a new scan.
    pub fn reset(&self) {
        self.0.reset();
    }

    /// Increment the count by 1 and return the new value.
    #[must_use]
    pub fn increment_count(&self) -> u64 {
        self.0.increment_count()
    }

    /// Set the count to a specific value.
    pub fn set_count(&self, value: u64) {
        self.0.set_count(value);
    }

    /// Set the total item count.
    pub fn set_total(&self, value: u64) {
        self.0.set_total(value);
    }

    /// Set the current scan phase.
    pub fn set_phase(&self, phase: ScanPhase) {
        self.0.set_phase(phase);
    }

    /// Set the current folder being scanned.
    pub fn set_current_folder(&self, folder: Option<String>) {
        self.0.set_current_folder(folder);
    }
}

/// Scan phase for progress tracking.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ScanPhase {
    /// Not scanning.
    #[default]
    Idle,
    /// Discovering files on disk.
    Discovering,
    /// Processing/importing tracks.
    Processing,
    /// Cleaning up orphaned records.
    Cleaning,
}

impl ScanPhase {
    /// Get the phase as a string for API responses.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Discovering => "discovering",
            Self::Processing => "processing",
            Self::Cleaning => "cleaning",
        }
    }
}

impl ScanState {
    /// Create a new scan state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            scanning: AtomicBool::new(false),
            count: AtomicU64::new(0),
            total: AtomicU64::new(0),
            phase: std::sync::RwLock::new(ScanPhase::Idle),
            current_folder: std::sync::RwLock::new(None),
        }
    }

    /// Check if a scan is currently in progress.
    pub fn is_scanning(&self) -> bool {
        self.scanning.load(Ordering::SeqCst)
    }

    /// Get the current item count.
    pub fn get_count(&self) -> u64 {
        self.count.load(Ordering::SeqCst)
    }

    /// Get the total item count (0 if unknown).
    pub fn get_total(&self) -> u64 {
        self.total.load(Ordering::SeqCst)
    }

    /// Get the current scan phase.
    pub fn get_phase(&self) -> ScanPhase {
        self.phase
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Get the current folder being scanned.
    pub fn get_current_folder(&self) -> Option<String> {
        self.current_folder
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Try to start a scan. Returns false if a scan is already in progress.
    pub fn try_start(&self) -> bool {
        self.scanning
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// Mark the scan as complete.
    pub fn finish(&self) {
        self.scanning.store(false, Ordering::SeqCst);
        *self
            .phase
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = ScanPhase::Idle;
        *self
            .current_folder
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }

    /// Reset the count to 0.
    pub fn reset_count(&self) {
        self.count.store(0, Ordering::SeqCst);
    }

    /// Reset all progress state for a new scan.
    pub fn reset(&self) {
        self.count.store(0, Ordering::SeqCst);
        self.total.store(0, Ordering::SeqCst);
        *self
            .phase
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = ScanPhase::Idle;
        *self
            .current_folder
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }

    /// Increment the count by 1 and return the new value.
    pub fn increment_count(&self) -> u64 {
        self.count.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Set the count to a specific value.
    pub fn set_count(&self, value: u64) {
        self.count.store(value, Ordering::SeqCst);
    }

    /// Set the total item count.
    pub fn set_total(&self, value: u64) {
        self.total.store(value, Ordering::SeqCst);
    }

    /// Set the current scan phase.
    pub fn set_phase(&self, phase: ScanPhase) {
        *self
            .phase
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = phase;
    }

    /// Set the current folder being scanned.
    pub fn set_current_folder(&self, folder: Option<String>) {
        *self
            .current_folder
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = folder;
    }
}

#[cfg(test)]
mod tests {
    use super::{ScanPhase, ScanState};

    #[test]
    fn scan_phase_strings_are_stable_api_values() {
        assert_eq!(ScanPhase::Idle.as_str(), "idle");
        assert_eq!(ScanPhase::Discovering.as_str(), "discovering");
        assert_eq!(ScanPhase::Processing.as_str(), "processing");
        assert_eq!(ScanPhase::Cleaning.as_str(), "cleaning");
    }

    #[test]
    fn scan_state_tracks_progress_then_resets_on_finish() {
        let state = ScanState::new();

        assert!(state.try_start());
        assert!(!state.try_start());
        state.set_total(10);
        state.set_phase(ScanPhase::Processing);
        state.set_current_folder(Some("Music/A".into()));
        assert_eq!(state.increment_count(), 1);
        assert_eq!(state.increment_count(), 2);

        assert!(state.is_scanning());
        assert_eq!(state.get_count(), 2);
        assert_eq!(state.get_total(), 10);
        assert_eq!(state.get_phase(), ScanPhase::Processing);
        assert_eq!(state.get_current_folder().as_deref(), Some("Music/A"));

        state.finish();

        assert!(!state.is_scanning());
        assert_eq!(state.get_count(), 2);
        assert_eq!(state.get_total(), 10);
        assert_eq!(state.get_phase(), ScanPhase::Idle);
        assert_eq!(state.get_current_folder(), None);
    }

    #[test]
    fn scan_state_reset_clears_progress_without_starting_scan() {
        let state = ScanState::new();

        assert!(state.try_start());
        state.set_total(5);
        state.set_count(4);
        state.set_phase(ScanPhase::Cleaning);
        state.set_current_folder(Some("Music/B".into()));

        state.reset();

        assert!(state.is_scanning());
        assert_eq!(state.get_count(), 0);
        assert_eq!(state.get_total(), 0);
        assert_eq!(state.get_phase(), ScanPhase::Idle);
        assert_eq!(state.get_current_folder(), None);
    }
}
