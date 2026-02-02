//! Music library scanner module.

pub mod engine;
pub mod lyrics;
pub mod state;
pub mod types;

// Re-export core types for backward compatibility
pub use engine::{AutoScanHandle, AutoScanner, Scanner};
pub use state::{ScanPhase, ScanState, ScanStateHandle};
pub use types::{ScanError, ScanMode, ScanResult, ScannedTrack};
