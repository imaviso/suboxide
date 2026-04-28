//! Library scanning API handlers (startScan, getScanStatus)

use axum::response::IntoResponse;

use crate::api::auth::SubsonicAuth;
use crate::api::response::{ScanStatusData, SubsonicResponse};
use crate::scanner::{ScanResult, Scanner};

/// Build a `ScanStatusData` from the current scan state.
fn build_scan_status_data(auth: &SubsonicAuth) -> ScanStatusData {
    let scan_state = auth.scan_state();
    ScanStatusData {
        scanning: scan_state.is_scanning(),
        count: scan_state.get_count(),
        total: scan_state.get_total(),
        phase: scan_state.get_phase(),
        folder: scan_state.get_current_folder(),
    }
}

/// GET/POST /rest/startScan[.view]
///
/// Initiates a media library scan. If a scan is already in progress,
/// returns the current status without starting a new scan.
///
/// Returns: scanStatus with scanning=true/false and count of items scanned.
pub async fn start_scan(auth: SubsonicAuth) -> impl IntoResponse {
    let scan_state = auth.scan_state().clone();
    let pool = auth.pool().clone();

    // Spawn background task to run the scan.
    // The actual start check (try_start) happens inside spawn_blocking so the
    // ScanGuard lives for the full scan duration.
    tokio::spawn(async move {
        let result = tokio::task::spawn_blocking(move || {
            let scanner = Scanner::new(pool);
            let Some(_guard) = scan_state.try_start() else {
                return Ok(ScanResult::default());
            };
            scanner.scan_all_with_state(Some(scan_state.get()))
        })
        .await;

        match result {
            Ok(Ok(stats)) => {
                tracing::info!(
                    name = "scan.manual.completed",
                    tracks.found = stats.tracks_found,
                    tracks.added = stats.tracks_added,
                    tracks.failed = stats.tracks_failed,
                    "manual scan completed"
                );
            }
            Ok(Err(e)) => {
                tracing::error!(name = "scan.manual.failed", error = %e, "manual scan failed");
            }
            Err(e) => {
                tracing::error!(
                    name = "scan.manual.task_panic",
                    error = %e,
                    "manual scan task panicked"
                );
            }
        }
    });

    // Return current status (scanning should be true now)
    let data = build_scan_status_data(&auth);
    SubsonicResponse::scan_status(auth.format, data)
}

/// GET/POST /rest/getScanStatus[.view]
///
/// Returns the current status of the media library scan.
///
/// Returns: scanStatus with scanning=true/false and count of items scanned.
pub async fn get_scan_status(auth: SubsonicAuth) -> impl IntoResponse {
    let data = build_scan_status_data(&auth);
    SubsonicResponse::scan_status(auth.format, data)
}
