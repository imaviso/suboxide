//! Library scanning API handlers (startScan, getScanStatus)

#![allow(clippy::unused_async)]

use axum::response::IntoResponse;

use crate::api::auth::SubsonicAuth;
use crate::api::response::{ScanStatusData, ok_scan_status};
use crate::scanner::Scanner;

/// Build a `ScanStatusData` from the current scan state.
fn build_scan_status_data(auth: &SubsonicAuth) -> ScanStatusData {
    let scan_state = auth.state.get_scan_state();
    ScanStatusData {
        scanning: scan_state.is_scanning(),
        count: scan_state.get_count(),
        total: scan_state.get_total(),
        phase: scan_state.get_phase().as_str().to_string(),
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
    let scan_state = auth.state.get_scan_state();

    // Try to start a new scan - returns false if one is already running
    if scan_state.try_start() {
        // Reset the counter for this new scan
        scan_state.reset_count();

        let pool = auth.state.get_db_pool();
        let scan_state_for_scanner = scan_state.clone();
        let scan_state_for_finish = scan_state;

        // Spawn background task to run the scan
        tokio::spawn(async move {
            // Run the scan in a blocking task since it's CPU-intensive
            let result = tokio::task::spawn_blocking(move || {
                let scanner = Scanner::new(pool);
                scanner.scan_all_with_state(Some(&scan_state_for_scanner))
            })
            .await;

            // Mark scan as complete
            scan_state_for_finish.finish();

            match result {
                Ok(Ok(stats)) => {
                    tracing::info!(
                        "Scan complete: {} tracks found, {} added, {} failed",
                        stats.tracks_found,
                        stats.tracks_added,
                        stats.tracks_failed
                    );
                }
                Ok(Err(e)) => {
                    tracing::error!("Scan failed: {}", e);
                }
                Err(e) => {
                    tracing::error!("Scan task panicked: {}", e);
                }
            }
        });
    }

    // Return current status (scanning should be true now)
    let data = build_scan_status_data(&auth);
    ok_scan_status(auth.format, data)
}

/// GET/POST /rest/getScanStatus[.view]
///
/// Returns the current status of the media library scan.
///
/// Returns: scanStatus with scanning=true/false and count of items scanned.
pub async fn get_scan_status(auth: SubsonicAuth) -> impl IntoResponse {
    let data = build_scan_status_data(&auth);
    ok_scan_status(auth.format, data)
}
