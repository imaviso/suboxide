//! Library scanning API handlers (startScan, getScanStatus)

use axum::response::IntoResponse;

use crate::api::auth::SubsonicContext;
use crate::api::response::{ScanStatusData, SubsonicResponse};
use crate::scanner::Scanner;

/// Build a `ScanStatusData` from the current scan state.
fn build_scan_status_data(auth: &SubsonicContext) -> ScanStatusData {
    let scan_state = auth.scan_state().snapshot();
    ScanStatusData {
        scanning: scan_state.scanning,
        count: scan_state.count,
        total: scan_state.total,
        phase: scan_state.phase,
        folder: scan_state.current_folder,
    }
}

/// GET/POST /rest/startScan[.view]
///
/// Initiates a media library scan. If a scan is already in progress,
/// returns the current status without starting a new scan.
///
/// Returns: scanStatus with scanning=true/false and count of items scanned.
pub async fn start_scan(auth: SubsonicContext) -> impl IntoResponse {
    let scan_state = auth.scan_state().clone();
    let pool = auth.pool().clone();
    let Some(guard) = scan_state.try_start() else {
        let data = build_scan_status_data(&auth);
        return SubsonicResponse::scan_status(auth.format, data);
    };

    // Spawn background task to run the scan.
    tokio::spawn(async move {
        let result = tokio::task::spawn_blocking(move || {
            let scanner = Scanner::new(pool);
            let _guard = guard;
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
pub async fn get_scan_status(auth: SubsonicContext) -> impl IntoResponse {
    let data = build_scan_status_data(&auth);
    SubsonicResponse::scan_status(auth.format, data)
}
